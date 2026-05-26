# Known Conditions

Substrate quirks and instrumentation artifacts that look like findings until
you understand the mechanism. Recording these prevents re-investigation and
keeps the detection index clean.

## WAL swell after container restart

**First seen**: 2026-04-08
**Scope**: driftwatch main DB

After container restart, WAL grows beyond the 64MB steady-state limit
(observed: 210MB) because no checkpoint has run yet on the fresh connection.
Settles back to 64MB within minutes once the first checkpoint cycle completes.

Not a problem. Do not alert on WAL size within ~5 minutes of restart.

## Checkpoint busy pages on continuous-write workloads

**First seen**: 2026-04-08
**Scope**: driftwatch main DB, bake gate

A PASSIVE checkpoint on a DB doing ~100 writes/sec will always show some
busy pages — the most recently written pages can't be checkpointed because
the writer is still active. This is normal SQLite WAL behavior.

The bake gate originally fired on `busy > 0`, which meant it always triggered.
Fixed to require >90% busy pages before alerting (commit f31b644).

Sampling at different moments gives wildly different busy ratios (observed:
5% to 90% depending on whether a facts export read was in progress). Point-in-time
checkpoint stats are noisy; trend over multiple samples before concluding there's
a real checkpoint stall.

## Facts work WAL size (2.1GB)

**First seen**: 2026-04-08
**Scope**: driftwatch facts_work.sqlite

The facts export working database maintains a large WAL (~2.1GB) because it
does bulk writes during recomputation and the snapshot interval is long enough
that WAL doesn't get truncated between cycles.

This is on a separate database file from the main labeler.sqlite and does not
affect main DB checkpoint behavior. The `journal_size_limit` pragma only takes
effect after a successful checkpoint, and the work DB's reader (labelwatch
ATTACHing the snapshot) doesn't hold locks on the work DB.

Monitor but don't treat as a problem unless it grows unboundedly or causes
disk pressure.
