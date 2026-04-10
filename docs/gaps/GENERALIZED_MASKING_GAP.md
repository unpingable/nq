# Gap: Generalized Masking — beyond `stale_host`

**Status:** specified, not yet built
**Depends on:** schema v24 (visibility_state), v26 (lineage)
**Build phase:** structural prep, follows EVIDENCE_LAYER and GENERATION_LINEAGE
**Blocks:** `DOMINANCE_PROJECTION_GAP`, `FEDERATION_GAP`
**Last updated:** 2026-04-10

## The Problem

The visibility/masking machinery shipped in migration 024 handles exactly one parent finding kind: `stale_host`. The logic is hardcoded:

```rust
let stale_hosts: HashSet<String> = {
    // SELECT host FROM warning_state WHERE kind = 'stale_host'
    //   AND visibility_state = 'observed'
};

// In the recovery loop:
let host_masked = !host.is_empty()
    && kind != "stale_host"
    && stale_hosts.contains(host);
if host_masked {
    suppress.execute(/* reason: 'host_unreachable' */);
}
```

This was the right slice to ship first because it covered the most common observability-loss case (host disappears). But it leaves real masking gaps wide open:

1. **Source errors don't mask child findings.** When `source_error` fires for a publisher (the publisher is unreachable, returning HTTP errors, or returning malformed JSON), every downstream finding from that source is now stale, but the system happily ages them out via the recovery hysteresis path. They look like they "resolved" — exactly the lie the substrate rule forbids.

2. **Agent failures look like host failures, but aren't.** If `nq-publish` is down on a host that's still pingable, all agent-derived findings (host metrics, services, sqlite health) are unobservable. But the host itself is fine. Masking under "host unreachable" would be wrong; the host *is* reachable, just blind to us. The right reason is `agent_down`.

3. **Per-collector partitions aren't modeled at all.** If the prometheus collector breaks but the host collector still works, only metric findings should be suppressed. Today there's no way to express "this finding depends on this collector path."

4. **Log silence doesn't mask error_shift.** When `log_silence` fires, the `error_shift` detector for the same source produces "0 errors!" — which looks like the system got healthier. It didn't; the eyes shut.

The substrate rule applies to all of these: **loss of observability must reduce confidence, not fabricate health.** Today it only applies to host visibility loss. This gap extends it to source visibility loss, agent visibility loss, and (with smaller scope) per-detector visibility loss.

## What Already Exists

| Component | Location | Covers |
|---|---|---|
| `visibility_state` column | migrations/024 | observed / suppressed states on warning_state |
| `suppression_reason` column | migrations/024 | text label for *why* a finding is suppressed |
| `suppressed_since_gen` column | migrations/024 | when the current suppression started |
| `stale_host` masking path | publish.rs `update_warning_state_inner` | the only existing parent → child masking |
| `stale_host` detector | detect.rs | emits when a host hasn't reported in N generations |
| `source_error` detector | detect.rs | emits when a source has had errors recently |
| `log_silence` detector | detect.rs | emits when a log source goes quiet unexpectedly |
| `error_shift` detector | detect.rs | emits when log error rate spikes above baseline |
| `findings_suppressed` counter | migrations/026 | per-generation count of suppressed findings |

**The gap:** the masking machinery is wired to exactly one parent kind. The data model (`visibility_state`, `suppression_reason`) is general enough to support multiple reasons, but no code path actually produces them. Adding a second parent kind today requires copying and adapting the existing hardcoded `stale_hosts` HashSet logic — tolerable for two parents, painful for four, untenable for more.

## What Needs Building

### 1. A masking rule structure

Replace the hardcoded `stale_hosts` HashSet with a small data-driven structure that can handle multiple parent kinds. The simplest honest version:

```rust
/// A rule for how a parent finding kind masks child findings.
struct MaskingRule {
    /// The kind of finding that acts as a parent (e.g. "stale_host").
    parent_kind: &'static str,
    /// What suppression_reason to write on masked children.
    suppression_reason: &'static str,
    /// What scope of children this parent masks.
    scope: MaskScope,
}

enum MaskScope {
    /// Mask any finding with the same `host` field, except findings of the
    /// parent kind itself. Used by stale_host and (after this gap) source_error.
    SameHost,
    /// Mask any finding with the same `host` field AND a dependency_class
    /// indicating the local agent. Used by future agent_down.
    /// Reserved; not implemented in this gap.
    SameHostAgentLocal,
    /// Mask any finding with a specific `subject` (the log source ID) and
    /// kind in a small set. Used by log_silence → error_shift.
    /// Reserved; this gap implements it as a special case if scope is small enough.
    SameLogSource,
}

const MASKING_RULES: &[MaskingRule] = &[
    MaskingRule {
        parent_kind: "stale_host",
        suppression_reason: "host_unreachable",
        scope: MaskScope::SameHost,
    },
    MaskingRule {
        parent_kind: "source_error",
        suppression_reason: "source_unreachable",
        scope: MaskScope::SameHost,
    },
];
```

This is data-driven enough that adding a third or fourth parent in the future is one entry in the table, not a refactor of the masking loop.

### 2. The masking pass

Replace the single hardcoded `stale_hosts` HashSet with a more general structure: a `HashMap<String, &'static str>` mapping `host` → `suppression_reason`. The first matching rule wins (rules are evaluated in order).

```rust
// At the top of the recovery hysteresis section:
let mut masking: HashMap<String, &'static str> = HashMap::new();
for rule in MASKING_RULES {
    let mut stmt = tx.prepare(
        "SELECT host FROM warning_state
         WHERE kind = ?1 AND visibility_state = 'observed'"
    )?;
    let hosts: Vec<String> = stmt.query_map([rule.parent_kind], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for host in hosts {
        masking.entry(host).or_insert(rule.suppression_reason);
    }
}

// In the recovery loop, replace the host_masked check:
let masking_reason = if !host.is_empty() {
    masking.get(host.as_str()).copied()
        .filter(|_| !MASKING_RULES.iter().any(|r| r.parent_kind == kind))
        // a parent never masks itself
} else {
    None
};
if let Some(reason) = masking_reason {
    suppress_with_reason.execute(rusqlite::params![host, kind, subject, generation_id, reason])?;
} else if /* recovery window or delete */ ...
```

The `suppress` prepared statement needs to take the reason as a bound parameter instead of the current hardcoded `'host_unreachable'`.

### 3. Source_error host identity

`source_error` currently fires per source with `host=""`. To mask findings on the same host, source_error needs to know its host identity. There are two options:

- **Option A:** Update the `source_error` detector to emit with `host=source_name`. This is the simplest move and matches the identity contract (canonical host = source name). Side effect: source_error findings will start showing up in per-host views, which is arguably correct.
- **Option B:** Look up the source-to-host mapping at masking time from `source_runs` or config. More complex, no clear win.

This gap MUST take Option A. The change is one line in the detector. The side effect of source_error appearing in host views is desired behavior — it's a per-host condition.

### 4. Suppression reason taxonomy

After this gap, the valid `suppression_reason` values are:

- `host_unreachable` — masked by `stale_host`
- `source_unreachable` — masked by `source_error`
- (reserved for future) `agent_down`, `collector_partition`, `parent_mask`, `maintenance`, `low_coverage`

Document this in a comment in publish.rs and in the migration if there's anything to migrate.

### 5. Tests

Required tests in `crates/nq-db/src/publish.rs`:

1. **`source_error` masks findings on the same host.** Build up a finding on host-1, then fire `source_error` for "host-1" (via source_runs failure or direct insert). Assert that the child finding becomes suppressed with reason `source_unreachable`.
2. **Multiple parents — first rule wins.** If both `stale_host` and `source_error` fire for the same host, the child findings are suppressed with the FIRST reason in `MASKING_RULES` order (`host_unreachable`). This is deterministic, not order-dependent on query results.
3. **Recovery from source_error unsuppresses children.** When `source_error` clears, child findings on that host return to `observed`, with persistence preserved (same invariant as the stale_host round-trip test from EVIDENCE_LAYER work).
4. **Source_error does not mask itself.** A `source_error` finding for host-1 must not be marked as suppressed by its own rule.
5. **Generation lineage updates correctly.** After source_error masking, `findings_suppressed` in the generations row reflects the new suppressed count (composed test against `GENERATION_LINEAGE_GAP`).
6. **Existing stale_host behavior unchanged.** All four existing visibility tests still pass. The refactor is structure-preserving.

## Why This Matters

This is the gap that turns the substrate rule from "host disappearance is honest" into "observability loss in general is honest." The two operationally most common loss modes are:

1. **Host stops reporting** — covered by `stale_host` (already shipped)
2. **Source stops responding cleanly** — covered by `source_error` (this gap)

After this gap, both look like "we can't see this right now, here's why" instead of "everything resolved!" That's the bulk of the value. The remaining masking work (`agent_down`, `collector_partition`, `log_silence` → `error_shift`) is more specialized and can wait.

This is also a prerequisite for `DOMINANCE_PROJECTION_GAP` because the projection layer needs to know what's suppressed and *why* in order to roll up causes correctly. With only one suppression reason today, the projection has nothing to dominate over.

## Non-Goals

This gap explicitly does NOT include:

- The `agent_down` detector. That's a separate detection problem (how do you tell "agent dead" from "host dead"?) that needs its own design. Reserved as a `MaskScope` variant for later.
- The `collector_partition` detector. Same reason.
- `log_silence` → `error_shift` masking. The shape is right but log subjects have a different identity model than hosts; defer to a follow-up that handles subject-keyed masking.
- A `MaskRule` configuration file. The rules stay in code as a `const` array. Configurability is empire-brain at this stage.
- Multi-parent suppression with composed reasons (e.g. "suppressed by both stale_host AND source_error"). First rule wins; the loser is invisible. Document this and live with it.
- Cascading suppression (suppressed parents masking grandchildren). One level deep is enough for now.
- Any change to the UI surface. The yellow visibility banner already handles "suppressed with reason X" generically; new reasons just produce new banner text.

## Build Estimate

| Item | Lines |
|---|---|
| `MaskingRule` struct + const table | ~25 Rust |
| Masking pass refactor in `update_warning_state_inner` | ~30 Rust (mostly mechanical) |
| `source_error` detector emits with host | ~3 Rust |
| Updated `suppress` prepared statement (reason as parameter) | ~5 Rust |
| Tests (6 of them) | ~150 Rust |
| **Total** | **~210** |

Time: roughly 45 focused minutes. The atomicity and transaction wrapping from EVIDENCE_LAYER make this incremental rather than structural — the existing tests cover the contract, this gap just generalizes the producer side.

## Acceptance Criteria

1. The masking rule table exists as a `const` data structure. Adding a new parent kind is a single entry, not a code change.
2. `source_error` masks child findings on the same host with `suppression_reason='source_unreachable'`.
3. `source_error` detector emits with `host=source_name` (Option A from §3).
4. All 6 new tests pass.
5. All existing tests (114 of them after the lineage gap) still pass — particularly the four existing visibility tests, which prove the refactor is structure-preserving.
6. The live VM continues running normally. After redeployment, querying the live DB should show `source_error` findings with non-empty host fields, and (rarely, since labelwatch-host doesn't usually source-error) any source_error event would now propagate to suppress child findings.

## Open Questions

- **Should the rule order be configurable?** No. The first-rule-wins behavior is documented and deterministic. If users want a different precedence, they need to fork the const table. Not configurability bait.
- **What happens if a child finding has a valid suppression reason but its parent disappears between the rule scan and the recovery loop?** The transaction wrapping makes this impossible — both run inside the same `update_warning_state_inner` transaction with a consistent snapshot.
- **Should `source_error` masking apply when the source is in recovery (transient)?** Yes — `source_error` findings carry their own `consecutive_gens` count and severity escalation. If the masking is only one generation, it self-clears next cycle. The substrate rule applies whether the parent is acute or chronic.
- **Should we mask findings when a host has BOTH `stale_host` AND `source_error` open?** They produce the same effect; the first matching rule wins (`host_unreachable` comes first in the table). The other reason is invisible. Document this and accept it. A composed-reason model is empire-brain.

## References

- docs/gaps/EVIDENCE_LAYER_GAP.md (transactional substrate this builds on)
- docs/gaps/GENERATION_LINEAGE_GAP.md (suppression count is tracked per generation)
- crates/nq-db/migrations/024_visibility_state.sql (the columns this gap populates)
- crates/nq-db/src/publish.rs `update_warning_state_inner` (the function this gap modifies)
- memory/project_notification_roadmap.md (the priority stack this fits into)
- memory/project_federation_shape.md (why generalized masking is a federation prerequisite)
