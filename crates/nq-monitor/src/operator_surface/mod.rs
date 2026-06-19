//! Operator-surface seam.
//!
//! This module is the **only** path from a witness evaluator to the
//! operator-facing HTTP surface. `http/routes.rs` must not call
//! `evaluate_*_preflight` directly (enforced by
//! `tests/operator_surface_boundary.rs`); it calls
//! [`preflight`] here instead.
//!
//! Why the seam exists: the dashboard/viewer had become a verdict
//! factory — nine route handlers re-derived preflight verdicts inline,
//! at request time, against ad-hoc wall clocks, with no boundary where a
//! projection/type-wall could live. See
//! `docs/working/decisions/OPERATOR_SURFACE_SPLIT_TRIPWIRE.md` and
//! `MONITORING_PROJECTION_SEAM_CANDIDATE.md`.
//!
//! This slice does the structural cut only. It does **not** persist or
//! seal anything; the surfaced preflight is an honest, request-time
//! re-derivation (see [`preflight::EvaluationBasis`]). Sealed,
//! generation-pinned preflight snapshots are a later slice
//! (`PREFLIGHT_SNAPSHOT_SEALING_CANDIDATE.md`).

pub mod preflight;
