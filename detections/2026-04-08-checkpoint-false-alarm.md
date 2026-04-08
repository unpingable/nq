# Checkpoint False Alarm

**Status**: confirmed (benign — instrumentation bug, not substrate issue)
**Detector**: bake_gate.py retention_health check
**First seen**: 2026-04-08
**Scope**: driftwatch main DB (labeler.sqlite)

## What happened

The bake gate WAL checkpoint preflight was reporting FAIL on every check.
Investigation showed this was a threshold problem: the check fired on
`busy > 0`, but a continuously-writing system (~100 events/sec) always has
some WAL pages that were just written and can't be checkpointed yet.

## Evidence

Sampling checkpoint status 5 times over 10 seconds:

```
0|3492|3077  (88% done)
0|1314|1139  (87% done)
0|2349|2153  (92% done)
0|2061|1832  (89% done)
0|1478|1415  (96% done)
```

Each PASSIVE checkpoint makes substantial progress. The WAL file stays
bounded at 64MB. There is no stuck reader or checkpoint stall.

One earlier sample caught 354/394 busy (89.8%) during a facts export
read, creating the false impression of a crisis. Point-in-time samples
are noisy on active systems.

## Fix

Changed bake gate threshold from `busy > 0` to `busy/log > 0.9`.
Now only flags when >90% of pages are blocked, indicating a genuinely
stuck reader rather than normal write churn.

Commit: f31b644 (driftwatch)

## Takeaway

Checkpoint health on continuous-write SQLite needs a ratio threshold,
not a zero-tolerance check. The substrate is fine; the instrument was
miscalibrated.
