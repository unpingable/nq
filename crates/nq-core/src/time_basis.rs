//! Cross-cycle `observed_at` regression check — `TIME_BASIS_POISONING_GAP` V0,
//! the **second** receiver-side sanity check.
//!
//! The gap names six checks and says "land one or two, not all six." The first
//! — `witness_observed_at` implausibly in the future of the evaluator clock — is
//! already landed and wired in
//! [`crate::preflight::PreflightResult::compute_time_basis`] (suspicion kind
//! `observed_at_future_of_evaluator`). This module adds the one a **single
//! snapshot cannot see**: `witness_observed_at` jumping sharply BACKWARD for the
//! same host / witness stream across cycles (the witness clock stepped back).
//!
//! **Annotation-only ("Mark").** It returns an operational suspicion; it NEVER
//! mints a verdict, refuses, downgrades, corrects a clock, mutates a receipt, or
//! notifies. Whether a suspicion poisons standing is the claim layer's decision,
//! deferred. It is **inert** until a future slice feeds it prior-cycle state
//! (the previous stored `observed_at` for a stream, e.g. at DB ingest). Absence
//! of a regression is `unknown`, never `verified` — it confers no standing.

use serde::Serialize;
use time::{Duration, OffsetDateTime};

/// A sharp backward step in `observed_at` for one host/witness stream.
/// `suspicion_kind` is operational (what NQ saw), not interpretive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObservedAtRegression {
    pub testimony_host: String,
    /// Operational kind, parallel to preflight's `observed_at_future_of_evaluator`.
    pub suspicion_kind: &'static str,
    /// How far `observed_at` moved backward vs the prior packet, in seconds.
    pub regression_seconds: i64,
    pub threshold_seconds: i64,
}

/// Detect a backward `observed_at` regression for the same host/witness stream.
/// Returns `Some` only when the backward step exceeds `threshold` (small
/// backward jitter is tolerated). Pure, annotation-only — never a verdict.
///
/// - `current_observed_at` — this packet's `witness_observed_at`.
/// - `prior_observed_at` — the previous packet's `observed_at` for the SAME
///   stream. Caller supplies it; `None` (no prior) yields `None` (nothing to
///   compare — `unknown`, not clean-verified).
pub fn observed_at_regression(
    testimony_host: &str,
    current_observed_at: OffsetDateTime,
    prior_observed_at: Option<OffsetDateTime>,
    threshold: Duration,
) -> Option<ObservedAtRegression> {
    let prior = prior_observed_at?;
    let back = prior - current_observed_at; // positive when observed_at went backward
    if back > threshold {
        Some(ObservedAtRegression {
            testimony_host: testimony_host.to_string(),
            suspicion_kind: "observed_at_backward_regression",
            regression_seconds: back.whole_seconds(),
            threshold_seconds: threshold.whole_seconds(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::format_description::well_known::Rfc3339;

    fn at(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).expect("rfc3339")
    }
    fn thresh() -> Duration {
        Duration::seconds(120)
    }

    #[test]
    fn no_prior_is_none_not_clean_verified() {
        assert!(observed_at_regression("sushi-k", at("2026-06-29T12:00:00Z"), None, thresh()).is_none());
    }

    #[test]
    fn forward_progress_is_none() {
        // observed_at moved forward vs prior — normal.
        let r = observed_at_regression(
            "sushi-k",
            at("2026-06-29T12:05:00Z"),
            Some(at("2026-06-29T12:00:00Z")),
            thresh(),
        );
        assert!(r.is_none());
    }

    #[test]
    fn small_backward_jitter_within_threshold_is_none() {
        // 30s back, threshold 120s — tolerated.
        let r = observed_at_regression(
            "sushi-k",
            at("2026-06-29T12:00:00Z"),
            Some(at("2026-06-29T12:00:30Z")),
            thresh(),
        );
        assert!(r.is_none());
    }

    #[test]
    fn sharp_backward_step_fires() {
        // observed_at jumped 1h back vs the prior packet for this stream.
        let r = observed_at_regression(
            "sushi-k",
            at("2026-06-29T12:00:00Z"),
            Some(at("2026-06-29T13:00:00Z")),
            thresh(),
        )
        .expect("regression fires");
        assert_eq!(r.suspicion_kind, "observed_at_backward_regression");
        assert_eq!(r.regression_seconds, 3600);
        assert_eq!(r.threshold_seconds, 120);
        assert_eq!(r.testimony_host, "sushi-k");
    }
}
