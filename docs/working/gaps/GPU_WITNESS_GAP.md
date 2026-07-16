# Gap: GPU Witness — a monitor that watches a GPU box must be able to see the GPU

**Status:** V0 shipped (2026-07-16) — embedded nvidia-smi collector (`crates/nq-witness/src/collect/gpu.rs`), wire family (`GpuWitnessReport`), migration 064 (`gpu_*_current` tables + `v_gpu_witness`/`v_gpu_devices`/`v_gpu_compute_apps`), ingest (`publish_gpu_witness`), crow-fixture tests incl. the tri-state honesty cases. **Design deviation from the SMART/ZFS twins, deliberate:** no external helper — nvidia-smi needs no privilege, and the helper indirection's stale-path failure mode was live on sushi-k's SMART witness for weeks. `collection_mode: "embedded"` per SPEC vocabulary. Detectors remain V1 (below).
**Depends on:** capability-honesty contract (`NotSupported` collector status, shipped 2026-06-30) — absence of the substrate is typed testimony, not silence
**Related:** SMART witness family (sibling — external-tool testimony with typed header/observations/errors), ZFS witness family (sibling), COVERAGE_HONESTY_GAP (what an unconfigured GPU witness means for coverage claims)
**Blocks:** honest visibility on GPU-hosting hosts; thermal/memory/throttle findings for LLM workloads; V1 GPU detectors
**Last updated:** 2026-07-16

## The Problem

NQ now witnesses a host whose defining workload is a GPU (crow: RTX 5060 Ti,
driver 570.211.01, running LLM experiments), and it cannot see the GPU at all.
The closest it gets is `nvidia-persistenced` unit state — which testifies that
a daemon is running, not that a device exists, is cool, has memory left, or is
being throttled.

Per the recognition-scope litmus: CPU/memory/disk/network for a monitor is
definitional scope, not speculative expansion. On a GPU host, the GPU is in
that list. This is adapter coverage, not live-authority speculation — the
substrate is live and the operator asked.

### Concrete forcing case

2026-07-16, crow first deploy: the box was mid-experiment — GPU at 86%
utilization, 68°C, 12.9/16.3 GB, P1, 127 W of a 180 W limit, one compute
process (ollama, 12.8 GB). None of that was witnessable. The same first
generation caught disk at 96.8% via the host collector — the GPU-side
equivalent (VRAM nearly full, sustained thermal load) had no witness.

## Substrate survey

| Substrate | Verdict |
|---|---|
| `nvidia-smi` CLI | **V0 choice.** Ships with the driver, unprivileged read, stable `--query-gpu` CSV interface, one subprocess — same idiom as the SMART/ZFS helpers minus the sudo wrapper. |
| NVML (FFI) | Richer + faster, but a native dependency and a linkage question for musl static builds. Named for V2+, not needed to recognize the substrate. |
| DCGM | Datacenter-grade, heavyweight daemon. Wrong size for this estate. |
| `/proc/driver/nvidia/*` | Thin, undocumented, driver-version-dependent. Rejected as primary; possible cross-check later. |

## Fixture facts (captured from crow, 2026-07-16)

- `--query-gpu=... --format=csv,noheader,nounits` yields one CSV row per GPU;
  fields can be real values, `[N/A]`, or `[Not Supported]` — **per-field
  absence is typed absent, not an error** (crow: `ecc.errors.corrected` =
  `[N/A]` on consumer silicon while everything else is live).
- `clocks_throttle_reasons.active` is a hex bitmask (`0x0000000000000000`).
- Unknown field → exit 2, error text on stdout. Absent binary → shell 127 /
  not-found. Driver-dead-but-binary-present is a distinct failure (exit ≠ 0,
  "couldn't communicate with the NVIDIA driver") — failed testimony, not
  NotSupported.
- `--query-compute-apps=pid,process_name,used_memory` names the processes
  holding VRAM. **Custody note:** process names/paths are island-local
  evidence; they must never ship to the public box (same discipline as
  `runs/` lease receipts).

## Design stance

- **Typed witness family**, same shape as SMART/ZFS: report header
  (profile version, collection mode, privilege model, duration), per-device
  observations, typed errors. Config-gated in `publisher.json`; an
  unconfigured GPU witness is not coverage.
- **Capability honesty:** binary absent → `NotSupported`; binary present but
  driver unreachable → failed testimony; per-field `[N/A]` → absent field in
  an otherwise admissible observation. Three different truths, three
  different shapes.
- **Boundaries (what this witness cannot testify to):** utilization is not
  progress; VRAM used is not VRAM needed; a cool idle GPU is not a healthy
  model server. The witness reports device state; inference QoS, model
  health, and CUDA correctness remain refused.
- **V0 observations:** identity (index, uuid, name, driver version), thermal
  (temp, fan), utilization (gpu, mem), memory (total, used), power (draw,
  limit), pstate, sm clock, persistence/compute mode, throttle bitmask, ECC
  corrected (where supported), compute apps (pid, process, VRAM).

## V1+ (named, deferred)

- Detectors: sustained thermal load, VRAM pressure, throttle-reason-active,
  device-disappeared (was witnessed, now absent), ECC corrected rising.
- Multi-GPU estates; NVML backend; AMD (`rocm-smi`) and Intel recognition —
  same family, different probes ("build the zoo; keep the cages labeled").
