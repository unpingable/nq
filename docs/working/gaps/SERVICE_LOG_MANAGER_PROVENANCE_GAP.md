# Gap: ServiceData / LogObservation manager-provenance + cannot_testify

**Status:** **named schema debt** — recognition ratified 2026-07-01; **build deferred**, recognition is not. This is **not** an enhancement candidate and **not** speculative architecture: provenance + field-admissibility on `ServiceData` / `LogObservation` is a known category requirement for NQ's portability model, and the *absence* of these fields is **already a correctness gap** (the wire is silently systemd/journald-shaped). What defers is the *build* — until a non-systemd service collector or non-journald log collector lands. What does NOT defer is the *recognition*:

> **Deferred build is fine. Deferred recognition is not.**
> Otherwise the next non-Linux collector grows another local workaround, and NQ has three dialects of "not Linux" before breakfast.

Provenance: operator + ChatGPT, 2026-07-01; sanity-checked against the repo (§ "What exists today"). The *exact enum surface, the wire default policy, collector specifics, and the cannot_testify persistence strategy* remain open design (§ "Recognized now vs still-open") — but the field-shaped hole itself is ratified debt, not a maybe.
**Composes with:** `PORTABILITY_GAP.md` (the non-systemd init / non-journald log collectors that would *populate* these fields — launchd/rc.d/openrc, unified logging/syslog, all currently `not_supported` on BSD), the shipped `HostData.cannot_testify` pattern (this is the same move at the service/log layer), and the capability-honesty doctrine.

## The doctrine line (the whole point)

> **`manager_kind` / `source_kind` is the provenance of semantics.**
> **`cannot_testify` is the refusal of false equivalence — which legacy fields must NOT be backfilled by imagination.**

`ServiceData` today is systemd-shaped; `LogObservation` today is journald/file-tail-shaped. Once launchd / rc.d / macOS unified logging enter, the old fields stop being universal. Two axes must stay separate:

- **Outcome** — `fetch_status`, `status`, `error_message`: *did this configured source fetch / is the thing up*. Already present. Do **not** overload these for provenance.
- **Provenance + admissibility** — `service_manager` / `log_source_kind` (*what kind of witness produced this claim*) and `cannot_testify` (*which fields this substrate must not be pretended to have*). This is the gap.

If the two axes collapse, future-you inherits `fetch_status="ok"` doing double duty as "and also it's a systemd unit," which is a lie waiting to happen.

## What exists today (sanity-check result, 2026-07-01)

- ✅ `HostData.cannot_testify: Vec<HostField>` (`wire.rs:119`) — the typed refusal pattern, shipped in the portability arc. This candidate is the same move for services/logs.
- 🟡 `service_manager` exists **only at the persistence/observation layer**: `service_observations.service_manager TEXT CHECK IN ('systemd','docker','process','unknown')` (migration 059) and `ServiceObservation.service_manager: String` (`service_state.rs:32`, untyped). It is **not** on the `ServiceData` wire struct, and there is no typed enum.
- ❌ `ServiceData` (`wire.rs:140`) carries bare `active_state`/`sub_state`/`load_state`/`unit_file_state: Option<String>` with **no `service_manager` discriminator and no `cannot_testify`**.
- ❌ `LogObservation` (`wire.rs:291`) carries only `fetch_status` + line counts — **no `log_source_kind`, no `cannot_testify`.**
- ❌ No `ServiceManager` / `ServiceField` / `LogSourceKind` / `LogField` enums anywhere.

**Reconciliation flagged for ratification:** the existing DB vocabulary is `systemd | docker | process | unknown`. ChatGPT proposed `pid_file` where the DB says `process`, and added `launchd` / `rcd` not yet in the CHECK. A typed wire `ServiceManager` enum must either adopt the existing DB spelling (`process`, extend the CHECK for new managers) or migrate it — **do not silently fork the vocabulary** (wire enum and DB CHECK must agree).

## Recognized now vs still-open (the split)

**Recognized debt (ratified — these SHOULD exist; their absence is the gap):**
- `ServiceData.manager_kind` (the provenance discriminator, on the wire — not just at persistence)
- `ServiceData.cannot_testify`
- a typed `ServiceManager`
- `LogObservation.source_kind`
- `LogObservation.cannot_testify`
- a typed `LogSourceKind`
- reconcile existing DB `process` vs proposed `pid_file` (one vocabulary, wire ↔ DB)
- stop overloading `fetch_status` (and `status` / `error_message`) as provenance

**Still open design (genuinely undecided — settle at build time):**
- the exact enum *surface* (variant lists below are a first cut, not frozen)
- whether the wire field defaults to `systemd` / `journald` or requires `Option<T>`
- launchd / rc.d / OpenRC / unified-logging collector specifics
- the persistence strategy for `cannot_testify` (JSON column vs join table vs stay wire-only)

## Proposed schema shape (candidate — synthesized best-of-both)

Typed enums (both ChatGPT cuts agreed on the shape; field lists merged):

```rust
// serde(rename_all = "snake_case") on all four.
enum ServiceManager { Systemd, Docker, Process /* == pid_file */, Launchd, Rcd, Unknown }
enum ServiceField   { ActiveState, SubState, LoadState, UnitFileState, MainPid, LastExitStatus, RestartPolicy, StartMode }
enum LogSourceKind  { Journald, FileTail, MacosUnifiedLog, SyslogFile, Unknown }
enum LogField       { Unit, Priority, Facility, SyslogIdentifier, Cursor, BootId,
                      MonotonicTimestamp, RealtimeTimestamp, Subsystem, Category, Process, Pid }
```

Additive wire fields (never rename or drop the existing systemd-shaped fields):

```rust
// on ServiceData:
#[serde(default)] pub service_manager: Option<ServiceManager>,
#[serde(default, skip_serializing_if = "Vec::is_empty")] pub cannot_testify: Vec<ServiceField>,

// on LogObservation:
#[serde(default)] pub log_source_kind: Option<LogSourceKind>,
#[serde(default, skip_serializing_if = "Vec::is_empty")] pub cannot_testify: Vec<LogField>,
```

**OPEN — default policy (ratification decision).** The two cuts disagreed: `Option<T>` (backward-compatible, but tolerates a runtime "unknown" that rots into a junk drawer) vs `#[serde(default = systemd/journald)]` (old rows deserialize as the implicit shape). Because `service_observations` **already** distinguishes docker/process, blindly defaulting old wire rows to `systemd` would be wrong for those; lean `Option<T>` on the wire + require new collectors to always populate it, and let the DB layer keep its explicit value. Settle at ratification.

## Per-substrate cannot_testify semantics (the meat)

**Services** — a partial witness without manager cosplay:

| `service_manager` | can testify (roughly) | `cannot_testify` |
|---|---|---|
| `systemd` | active_state, sub_state, load_state, unit_file_state, main_pid, exec status | `[]` |
| `docker` | container exists / running / status, pid | sub_state, load_state, unit_file_state (+ maybe restart_policy) |
| `process` (pid_file) | pid-file exists, pid present, pid appears alive | most systemd-native fields |
| `launchd` | label known, pid/running-ish, last_exit_status where available | active_state, sub_state, load_state, unit_file_state — **must not** map "launchd loaded" → "systemd active" |
| `rcd` | script/status exit, `enabled` if rc.conf queried explicitly | the systemd state taxonomy — **must not** map status text → systemd state machine |

Better V0 shape for non-systemd: `running: Some(bool)`, `main_pid: Some(pid)` where known, `last_exit_status: Some(code)` where known, `active_state: None`, and the systemd-native fields listed in `cannot_testify`. Don't force `active_state = "active"` without a narrow declared mapping.

**Logs** — same pattern:

| `log_source_kind` | can testify (roughly) | `cannot_testify` |
|---|---|---|
| `journald` | full structured metadata | `[]` |
| `file_tail` | file path, line text, read offset | unit, priority, cursor, boot_id, monotonic_timestamp |
| `macos_unified_log` | process, subsystem, category, timestamp, message (where configured) | journald cursor, systemd unit, (maybe) boot_id |
| `syslog_file` | parsed syslog line (if parser), file source | journald cursor, unit, boot_id |

## Persistence split (candidate)

- **Persist the discriminator soon** — `service_manager` is already persisted; add `log_observations.log_source_kind TEXT` (cheap). Consumers genuinely care whether a service claim came from systemd vs launchd vs docker vs pid_file; a finding that depends on `active_state` needs to know whether it's absent because the manager *cannot testify*.
- **`cannot_testify` stays wire-only for V0** — persisting a vector means a JSON column / join table / migration mess; don't pay it until a reader queries field-admissibility. `ServiceData` leans persist-`service_manager`-now; `LogObservation` leans wire-only until a finding needs source-kind.

## Finding semantics (named, load-bearing, NOT this slice)

Findings should not assert systemd-shaped universals across substrates. Not `service foo degraded`, but `service foo not_running via launchd` / `service foo pid_absent via pid_file` / `service foo systemd_inactive via systemd` — different claims, different admissibility. A normalized status enum may follow later:

```rust
// FUTURE / deferred — do not build in the schema slice:
enum ServiceObservationKind { RunningObserved, NotRunningObserved, ManagerReportedFailed, ConfiguredButUnobserved, NotSupported }
```

## Anti-scope (explicit — this is a schema record, not a build)

- **No collectors.** Do not implement launchd / rc.d / openrc / macOS unified logging / syslog-file collectors here. Those are `PORTABILITY_GAP` slices; this only names the schema they'd emit into.
- **No renaming** of existing systemd-shaped fields; the change is purely additive.
- **No manager cosplay** — never pretend pid_file / docker / launchd / rc.d are systemd-like.
- **No `ServiceObservationKind`** normalized-status enum yet (named above as future).
- **No launchd inventory / no `launchctl print` stable parsing / no unified-logging firehose** — when those collectors do land, V0 is: configured labels/predicates only, bounded windows, ndjson/json parse only, narrow runtime testimony.
- **cannot_testify column** deferred until a reader needs it (wire-only V0).

## This is debt, not a maybe (build trigger ≠ recognition trigger)

The debt is recognized now. The **build** lands when the first non-systemd service collector or non-journald log collector does (a `PORTABILITY_GAP` slice) — that is the trigger for *writing the enums + fields*, not for *deciding they're needed*. Until then this record does real work: it keeps the next collector from being forced into systemd/journald shape, keeps `fetch_status` from being drafted into provenance duty, and prevents the "three dialects of not-Linux" fragmentation that happens when each collector invents its own local workaround for the missing provenance/admissibility fields.

## References

- `wire.rs` — `HostData` (§`cannot_testify`, the shipped precedent), `ServiceData` (:140), `LogObservation` (:291).
- `crates/nq-db/migrations/059_service_observations.sql` — the existing `service_manager` vocabulary to reconcile against.
- `PORTABILITY_GAP.md` — the collector slices (launchd/rc.d, unified logging/syslog) that would populate these fields.
- `DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md` — sibling "don't let one field do two jobs" discipline (there: display vs authority freshness; here: outcome vs provenance/admissibility).
