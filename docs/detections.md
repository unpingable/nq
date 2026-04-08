# Detections

Running index of real things the observatory has surfaced. Each entry links
to a detailed writeup. This is the "is the instrument actually finding real
things?" calibration record.

## Confirmed

- [Checkpoint false alarm](../detections/2026-04-08-checkpoint-false-alarm.md) — bake gate WAL check too sensitive for continuous-write workloads. Fixed.
- [Labeler decay: hailey.at and skywatch.blue](../detections/2026-04-08-labeler-decay.md) — reference labelers failing by disappearance and attenuation. Ecosystem signal, not infra fault.
- [Hosting skew: first-pass distribution](../detections/2026-04-08-hosting-skew.md) — brid.gy over-labeled (distributed), blacksky under-labeled (structural), sprk.so single-labeler dependency.

## Under observation

- blacksky.app vs skywatch.blue boundary fight — READY tier but held pending skywatch stability. Recheck week of 2026-04-15.
- sprk.so governance blind spot — dependent on skywatch.blue recovery.
