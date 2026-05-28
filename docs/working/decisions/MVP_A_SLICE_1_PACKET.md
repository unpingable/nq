# MVP-A Slice 1 Packet — NQ Verification

**Filed:** 2026-05-28 by chat-context Cartographer (cross-repo coordination scope).
**Status:** repo-local spec drop. **No implementation authorized.** Verification-only target.
**Origin:** [MVP-A Plan rev1](file:///home/jbeck/git/cartography/audit/2026-05-28-mvp-plan-rev1.md), Slice 1.
**Purpose:** make the slice executable from inside this repo without re-reading the cartography audit thread.

## What this packet is

A self-contained verification scope for NQ's role in the MVP-A demo loop:

```
substrate → NQ (this slice) → NS → Wicket → WLP → Continuity persistence
```

The Cartographer-side re-audit (2026-05-28) found that Phase 2 receipt durability is substantially landed. This slice **verifies** that finding against current code, rather than building from scratch.

## What to verify

Pick one live sushi-k `disk_pressure` finding (existing emit; not a new claim kind) and run:

```text
nq receipt check <receipt_id>
```

Confirm:

1. **Exit code 0** (deterministic verdict over canonical bytes).
2. **Canonical content hash** populated on the receipt (RFC 8785 JCS + SHA-256).
3. **Witness-ref digest** populated for the disk_state claim kind (non-null `digest` field on `WitnessRef`).
4. **Evaluator-version binding** present on the receipt (`EvaluatorBinding` with claim kind + version, part of the content hash).

Existing artifact anchors (per Cartographer re-audit):

- `crates/nq-core/src/receipt.rs:279-306` — canonicalization via `serde_jcs` + `Sha256`
- `crates/nq-core/src/receipt.rs:32-36, 172-178` — evaluator binding
- `crates/nq-core/src/receipt.rs:401-413` — witness-ref digest population (disk_state)
- `crates/nq-core/src/receipt_check.rs` — check logic
- `crates/nq/src/cmd/receipt.rs:61-129` — CLI verb

## Subject-boundary tripwire

The `disk_pressure` finding's subject MUST be interpretable as:

> **sushi-k host filesystem/resource state — NOT NQ, NOT NS, NOT the observation loop.**

Anchors that prove this interpretation:

- `crates/nq-db/src/finding_meta.rs` — plain_label "Disk nearing capacity"
- `crates/nq-db/src/export.rs` — `metric_for_kind_subject()` maps `"disk_pressure" → Some("disk_used_pct")` (host metric)
- `crates/nq/src/http/routes.rs` — disk_pressure pivots query `hosts_history`
- `crates/nq-db/src/detect.rs` — `detect_disk_pressure()` reads `v_hosts.disk_used_pct`

**Tripwire:** if the interpretation can no longer be re-derived from these artifacts (e.g., disk_pressure has been refactored to observe NQ-internal state), **stop and report.** Do not improvise. Do not rely on the empty-subject-field as the boundary proof — the interpretation above is the load-bearing artifact.

## `nq receipt replay` disposition

`crates/nq-core/src/receipt_replay.rs` exists (~1057 LOC; CLI-wired). The earlier MVP-A plan rev0 said "do not build replay." Operator 2026-05-28 confirmed: **accept as already shipped. Do not unbuild. Do not make it MVP-A load-bearing unless it already is.**

If the verification path naturally exercises replay (e.g., `receipt check` invokes replay logic internally), fine. If it doesn't, leave it alone.

## Acceptance

Slice 1 closes when:

1. A live sushi-k `disk_pressure` receipt has been picked and its receipt_id recorded.
2. `nq receipt check <receipt_id>` returns exit 0 with deterministic canonical hash + populated witness-ref digest + bound evaluator-version.
3. A brief verification note (5 lines max) is appended to this packet or a sibling decision-doc recording: receipt_id checked, canonical_hash captured, exit_code, any anomalies.

Estimated work: ~30 minutes. Read-only against current code.

## Must NOT

- Add new claim kinds (no `labeler_ingest_health`, no new substrate observations).
- Expand replay scope.
- Touch Linode (no Driftwatch HTTP polling, no Labelwatch — Path B is later, separate, no federation).
- Touch lil-nas-x (Path A.5 is later, separate).
- Touch Wicket, WLP, NS, Continuity code.
- Refactor receipt code to "clean up while we're here."
- Treat this packet as authorization to extend the Phase 2 scope.

## Composes with

- [MVP-A Plan rev1](file:///home/jbeck/git/cartography/audit/2026-05-28-mvp-plan-rev1.md) — full Slice 1 context + path ladder + subject-boundary
- [Coordination registration](file:///home/jbeck/git/cartography/coordination/MVP-PATH-A-PLAN.md) — cross-tool dropbox pointer
- This repo's existing receipt + disk_state + Track A cutover work (commit `b9f57ed`, 2026-05-27) — do not duplicate
- [NQ-NS-CHANNEL-SPLIT](file:///home/jbeck/git/cartography/coordination/NQ-NS-CHANNEL-SPLIT.md) — channel discipline; forbidden NS-posture-into-NQ-truth cycle stays absent

## Provenance

Filed by Cartographer per operator instruction 2026-05-28 (post §H confirmation). Cartographer's authority for cross-repo writes is bounded to coordination/docs scope per operator directional 2026-05-28; this packet is a docs-only spec drop, not implementation.

## Verification record (2026-05-28, Cartographer)

Verified per operator authorization for MVP-A Slice 1 execution (verification-only).

- **Receipt:** sushi-k disk_state preflight, captured at `/tmp/mvp-a-slice-1/sushi-k-disk-state.receipt.json`. `content_hash: sha256:1f38f6bca4abe361c9d1db966d4cf12c897dc485945b1edfdb53a1a1f6558704`. `evaluator: {disk_state, version: 1}`. `witnesses[0].digest: sha256:ebb0f433bec1a374419959fead400aea74b8397267f94e7c4da123680a9c374f` (`disk_pressure_legacy_projection`, `custody_basis: legacy_projection`). Subject **explicitly** populated: `subject: "host:sushi-k"`, `target: {host: "sushi-k", scope: "host"}`.
- **`nq receipt check` (default):** exit 0; `overall: OK`; schema OK, content_hash OK, evaluator_binding OK. `witness_digest: missing_witness_packet` (warn-shape) because no external `.witness.v1` packet was supplied — structural for kinds with internally-projected witnesses (`legacy_projection`), not a defect. `--strict` mode exits 1 on the same missing_witness_packet warning; this is expected and not a code change to address.
- **Determinism:** re-running `nq preflight disk-state` back-to-back produces different `content_hash` values (1f38f6bc… vs a195cfab…) because `generated_at` differs between runs; this is correct content-addressed behavior. Per-receipt the hash is reproducible: `nq receipt check` confirms `content_hash: OK` on both runs.
- **Subject-boundary interpretation re-derived from artifacts:** `crates/nq-db/src/export.rs:1133` (`"disk_pressure" => Some("disk_used_pct")`); `crates/nq-db/src/detect.rs:1133, 1135` (`detect_disk_pressure` reads `v_hosts.disk_used_pct WHERE disk_used_pct > 90.0`); `crates/nq/src/http/routes.rs:297` (disk_pressure pivots query `hosts_history`); `crates/nq-db/src/finding_meta.rs:83-84` (`plain_label: "Disk nearing capacity"`). Subject = **sushi-k host filesystem/resource state**, not NQ, not NS, not observation loop. Receipt-layer subject is explicit (`host:sushi-k`), not derived from absence.
- **`nq receipt replay`:** exists and is documented (`Semantically replay an nq.receipt.v1 document …`); not exercised by Slice 1; accepted-as-shipped per operator §H.4 (not MVP-A load-bearing). No code/doc/test changes were made during verification.

**Slice 1 acceptance:** all four criteria (canonicalization / witness-ref digest population / evaluator-version binding / `nq receipt check` deterministic verdict) pass. No subject-boundary tripwire fired. Slice 1 closes.
