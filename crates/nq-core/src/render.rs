//! Receipt renderers. Human format for terminal output; JSON for
//! machine consumption.
//!
//! See `docs/architecture/SHARED_SPINE.md` — a renderer describes the
//! receipt, it does not adjudicate. No renderer is allowed to compute
//! a status NQ did not put in the receipt.

use crate::receipt::{Receipt, Status, StatusReason};

pub fn render_human(r: &Receipt) -> String {
    let mut out = String::new();
    out.push_str("NQ Verification Receipt\n");
    out.push('\n');
    out.push_str(&format!("Claim:    {}\n", r.claim));
    out.push_str(&format!("Subject:  {}\n", r.subject));
    out.push_str(&format!("Status:   {}\n", status_word(r.status)));
    if !r.status_reasons.is_empty() {
        let reasons: Vec<&str> = r.status_reasons.iter().map(|x| reason_word(*x)).collect();
        out.push_str(&format!("Reasons:  {}\n", reasons.join(", ")));
    }
    out.push('\n');

    if let Some(t) = &r.target {
        out.push_str("Target:\n");
        out.push_str(&format!("  Host:  {}\n", t.host));
        out.push_str(&format!("  Scope: {}\n", t.scope));
        if let Some(id) = &t.id {
            out.push_str(&format!("  Id:    {id}\n"));
        }
        out.push('\n');
    }

    if !r.verified.is_empty() {
        out.push_str("Verified:\n");
        for v in &r.verified {
            out.push_str(&format!("  - {v}\n"));
        }
        out.push('\n');
    }

    if !r.not_verified.is_empty() {
        out.push_str("Not verified:\n");
        for n in &r.not_verified {
            match &n.detail {
                Some(d) => out.push_str(&format!("  - {} ({}): {}\n", n.claim, n.reason, d)),
                None => out.push_str(&format!("  - {} ({})\n", n.claim, n.reason)),
            }
        }
        out.push('\n');
    }

    if !r.cannot_testify.is_empty() {
        out.push_str("Refused claims:\n");
        for c in &r.cannot_testify {
            out.push_str(&format!("  - {c}\n"));
        }
        out.push('\n');
    }

    if !r.suggested_weaker_claims.is_empty() {
        out.push_str("Suggested weaker claims:\n");
        for s in &r.suggested_weaker_claims {
            out.push_str(&format!("  - {s}\n"));
        }
        out.push('\n');
    }

    if !r.supported_status.is_empty() {
        out.push_str("Supported status:\n");
        out.push_str(&format!("  {}\n", r.supported_status));
        out.push('\n');
    }

    if let Some(signals) = &r.signals {
        out.push_str("Signals:\n");
        render_signals_plain(signals, "  ", &mut out);
        out.push('\n');
    }

    if !r.witnesses.is_empty() {
        out.push_str("Witnesses:\n");
        for w in &r.witnesses {
            out.push_str(&format!("  - {}\n", w.witness_type));
        }
        out.push('\n');
    }

    if r.observed_at_min.is_some() || r.observed_at_max.is_some() {
        let min = r.observed_at_min.as_deref().unwrap_or("-");
        let max = r.observed_at_max.as_deref().unwrap_or("-");
        out.push_str(&format!("Observed:  {min} → {max}\n"));
    }
    out.push_str(&format!("Generated: {}\n", r.generated_at));
    if let Some(ev) = &r.evaluator {
        out.push_str(&format!("Evaluator: {} v{}\n", ev.evaluator, ev.version));
    }
    // Timestamp only. The renderer does not compute live/stale posture —
    // that would require a clock injection it does not have.
    if let Some(horizon) = &r.freshness_horizon {
        out.push_str(&format!("Freshness horizon: {horizon}\n"));
    }

    out
}

pub fn render_json(r: &Receipt) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(r)
}

pub fn render_jsonl(r: &Receipt) -> Result<String, serde_json::Error> {
    serde_json::to_string(r)
}

/// GitHub-flavored markdown rendering of a receipt. Suitable for PR
/// comments and dashboards. Uses the external vocabulary only — wire
/// vocabulary (`cannot_testify`, `admissible`, `admissibility`) does
/// not appear as the renderer's own words. The constitutional
/// refusals list (`cannot_testify`) is surfaced under the heading
/// "Refused claims" so consumers can see what the evaluator declines
/// to mint, without the renderer leaking doctrine vocabulary.
pub fn render_markdown(r: &Receipt) -> String {
    let mut out = String::new();
    out.push_str("## NQ Verification Receipt\n\n");
    out.push_str(&format!("**Claim:** `{}`  \n", r.claim));
    out.push_str(&format!("**Subject:** `{}`  \n", r.subject));
    out.push_str(&format!("**Status:** `{}`\n\n", status_word(r.status)));

    // Freshness horizon is load-bearing for consumers that may otherwise
    // read historical testimony as present-tense. Surface near the top
    // metadata, not buried in the footer. Timestamp only — no live/stale
    // computation in the renderer.
    if let Some(horizon) = &r.freshness_horizon {
        out.push_str(&format!("**Freshness horizon:** `{horizon}`\n\n"));
    }

    if let Some(t) = &r.target {
        out.push_str("### Target\n\n");
        out.push_str(&format!("- **Host:** `{}`\n", t.host));
        out.push_str(&format!("- **Scope:** `{}`\n", t.scope));
        if let Some(id) = &t.id {
            out.push_str(&format!("- **Id:** `{id}`\n"));
        }
        out.push('\n');
    }

    if !r.verified.is_empty() {
        out.push_str("### Verified\n\n");
        for v in &r.verified {
            out.push_str(&format!("- `{v}`\n"));
        }
        out.push('\n');
    }

    if !r.not_verified.is_empty() {
        out.push_str("### Not verified\n\n");
        for n in &r.not_verified {
            match &n.detail {
                Some(d) => out.push_str(&format!("- `{}` — {} ({})\n", n.claim, n.reason, d)),
                None => out.push_str(&format!("- `{}` — {}\n", n.claim, n.reason)),
            }
        }
        out.push('\n');
    }

    if !r.cannot_testify.is_empty() {
        out.push_str("### Refused claims\n\n");
        out.push_str("These claims the evaluator declines to make from this receipt's substrate:\n\n");
        for c in &r.cannot_testify {
            out.push_str(&format!("- {c}\n"));
        }
        out.push('\n');
    }

    if !r.suggested_weaker_claims.is_empty() {
        out.push_str("### Suggested weaker claims\n\n");
        for s in &r.suggested_weaker_claims {
            out.push_str(&format!("- `{s}`\n"));
        }
        out.push('\n');
    }

    if !r.supported_status.is_empty() {
        out.push_str("### Supported status\n\n");
        out.push_str(&format!("> {}\n\n", r.supported_status));
    }

    if let Some(signals) = &r.signals {
        out.push_str("### Signals\n\n");
        render_signals_markdown(signals, &mut out);
    }

    if !r.witnesses.is_empty() {
        out.push_str("### Witnesses\n\n");
        for w in &r.witnesses {
            match &w.observed_at {
                Some(t) => out.push_str(&format!("- `{}` (observed `{}`)\n", w.witness_type, t)),
                None => out.push_str(&format!("- `{}`\n", w.witness_type)),
            }
        }
        out.push('\n');
    }

    if !r.status_reasons.is_empty() {
        let reasons: Vec<&str> = r.status_reasons.iter().map(|x| reason_word(*x)).collect();
        out.push_str(&format!("<sub>Reason codes: {}</sub>  \n", reasons.join(", ")));
    }
    out.push_str(&format!(
        "<sub>Generated `{}` from `{}`.</sub>\n",
        r.generated_at, r.schema
    ));
    if let Some(ev) = &r.evaluator {
        out.push_str(&format!(
            "<sub>Evaluator: `{}` v{}</sub>\n",
            ev.evaluator, ev.version
        ));
    }
    if let Some(hash) = &r.content_hash {
        out.push_str(&format!("<sub>Receipt hash: `{hash}`</sub>\n"));
    }

    out
}

/// Render the namespaced `signals` JSON object into markdown bullets.
///
/// Today the wire shape is `signals.{claim_kind}.{key}` — namespaced
/// per kind. We render each namespace as its own labeled block; inner
/// keys become a bullet list. `null` inner values are rendered as
/// em-dash so absence stays distinguishable from "0" or "false".
///
/// Falls back to a fenced JSON block if the top-level shape isn't an
/// object — defensive against future namespacing changes. The
/// renderer is not in the business of validating signals shapes; it
/// surfaces whatever the evaluator emitted.
fn render_signals_markdown(signals: &serde_json::Value, out: &mut String) {
    let Some(top) = signals.as_object() else {
        out.push_str("```json\n");
        out.push_str(&serde_json::to_string_pretty(signals).unwrap_or_default());
        out.push_str("\n```\n\n");
        return;
    };
    for (kind, payload) in top {
        out.push_str(&format!("**`{kind}`:**\n\n"));
        if let Some(inner) = payload.as_object() {
            for (key, val) in inner {
                out.push_str(&format!("- `{key}`: {}\n", format_signal_value(val)));
            }
        } else {
            out.push_str("```json\n");
            out.push_str(&serde_json::to_string_pretty(payload).unwrap_or_default());
            out.push_str("\n```\n");
        }
        out.push('\n');
    }
}

/// Plain-text version of [`render_signals_markdown`] for the
/// human renderer. Indented under the calling section. Same
/// namespace-aware traversal; no markdown syntax.
fn render_signals_plain(signals: &serde_json::Value, indent: &str, out: &mut String) {
    let Some(top) = signals.as_object() else {
        out.push_str(indent);
        out.push_str(&serde_json::to_string(signals).unwrap_or_default());
        out.push('\n');
        return;
    };
    for (kind, payload) in top {
        out.push_str(indent);
        out.push_str(&format!("{kind}:\n"));
        if let Some(inner) = payload.as_object() {
            for (key, val) in inner {
                out.push_str(indent);
                out.push_str(&format!("  {key}: {}\n", format_signal_value(val)));
            }
        } else {
            out.push_str(indent);
            out.push_str(&format!(
                "  {}\n",
                serde_json::to_string(payload).unwrap_or_default()
            ));
        }
    }
}

/// Format a JSON scalar for display. `null` becomes em-dash; strings
/// drop their quotes; everything else round-trips through `to_string`.
/// The em-dash for null is honest about absence — `0` and `false` are
/// distinguishable values, not stand-ins for "we didn't compute this."
fn format_signal_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "—".to_string(),
        serde_json::Value::String(s) => format!("`{s}`"),
        other => format!("`{other}`"),
    }
}

fn status_word(s: Status) -> &'static str {
    match s {
        Status::Verified => "verified",
        Status::PartiallyVerified => "partially_verified",
        Status::NeedsMoreEvidence => "needs_more_evidence",
        Status::NotVerified => "not_verified",
        Status::InvalidEvidence => "invalid_evidence",
    }
}

fn reason_word(r: StatusReason) -> &'static str {
    match r {
        StatusReason::AllRequirementsVerified => "all_requirements_verified",
        StatusReason::PartialComposite => "partial_composite",
        StatusReason::MissingRequiredClaim => "missing_required_claim",
        StatusReason::ClaimConditionFailed => "claim_condition_failed",
        StatusReason::StaleObservation => "stale_observation",
        StatusReason::ContradictoryObservation => "contradictory_observation",
        StatusReason::NonMintable => "non_mintable",
        StatusReason::SuggestedWeakerClaimAvailable => "suggested_weaker_claim_available",
        StatusReason::InvalidWitness => "invalid_witness",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipt::{EvaluatorBinding, NotVerifiedEntry, Receipt, Status, StatusReason, WitnessRef};

    fn sample() -> Receipt {
        Receipt {
            schema: "nq.receipt.v1".into(),
            claim: "safe_to_merge".into(),
            subject: "repo:.".into(),
            target: None,
            status: Status::NotVerified,
            status_reasons: vec![
                StatusReason::NonMintable,
                StatusReason::SuggestedWeakerClaimAvailable,
            ],
            verified: vec!["repo_clean".into(), "tests_passed".into()],
            not_verified: vec![NotVerifiedEntry {
                claim: "no_behavior_change".into(),
                reason: "missing_witness".into(),
                detail: None,
            }],
            suggested_weaker_claims: vec!["ready_for_review".into()],
            supported_status: "Repo is clean and tests passed.".into(),
            cannot_testify: vec![],
            witnesses: vec![WitnessRef {
                witness_type: "git_status".into(),
                digest: None,
                observed_at: Some("2026-05-15T14:00:00Z".into()),
                custody_basis: None,
            }],
            observed_at_min: Some("2026-05-15T13:59:51Z".into()),
            observed_at_max: Some("2026-05-15T14:00:02Z".into()),
            generated_at: "2026-05-15T14:00:04Z".into(),
            evaluator: None,
            freshness_horizon: None,
            signals: None,
            content_hash: None,
        }
    }

    #[test]
    fn human_mentions_status_word() {
        let s = render_human(&sample());
        assert!(s.contains("not_verified"));
        assert!(s.contains("non_mintable"));
        assert!(s.contains("ready_for_review"));
        assert!(s.contains("Supported status"));
    }

    #[test]
    fn json_roundtrips() {
        let r = sample();
        let s = render_json(&r).unwrap();
        let back: Receipt = serde_json::from_str(&s).unwrap();
        assert_eq!(back.claim, r.claim);
        assert_eq!(back.status, r.status);
    }

    #[test]
    fn jsonl_is_single_line() {
        let s = render_jsonl(&sample()).unwrap();
        assert!(!s.contains('\n'));
    }

    #[test]
    fn markdown_uses_external_vocabulary_only() {
        let s = render_markdown(&sample());
        assert!(s.contains("Verification Receipt"));
        assert!(s.contains("not_verified"));
        assert!(s.contains("`ready_for_review`"));
        assert!(s.contains("Supported status"));
        // External vocabulary discipline: doctrine words stay out of
        // the user-facing markdown body. (They may appear in reason
        // codes when the user-facing word is present already.)
        assert!(!s.contains("admissible"));
        assert!(!s.contains("cannot_testify"));
        assert!(!s.contains("admissibility"));
    }

    #[test]
    fn markdown_omits_empty_sections() {
        let mut r = sample();
        r.not_verified.clear();
        r.suggested_weaker_claims.clear();
        let s = render_markdown(&r);
        assert!(!s.contains("Not verified"));
        assert!(!s.contains("Suggested weaker"));
    }

    // -----------------------------------------------------------------
    // Gap #8 — markdown + human render the consumer-contract fields
    // added in 6186ca0: `target`, `cannot_testify`, `signals`.
    //
    // External-vocabulary discipline preserved: the wire word
    // `cannot_testify` does not appear as the renderer's own
    // vocabulary; the user-facing heading is "Refused claims."
    // -----------------------------------------------------------------

    use crate::preflight::PreflightTarget;

    fn sample_with_consumer_fields() -> Receipt {
        let mut r = sample();
        r.target = Some(PreflightTarget {
            host: "labelwatch-host".into(),
            scope: "sqlite_wal".into(),
            id: Some("/opt/nq/nq.db".into()),
        });
        r.cannot_testify = vec![
            "Whether the application that owns this DB will recover (application-state claim)".into(),
            "Whether to restart, repoint, kill the pinned reader, or page (consequence claim)".into(),
        ];
        r.signals = Some(serde_json::json!({
            "sqlite_wal_state": {
                "threshold_band": "bounded",
                "pinned_reader": "unobserved",
                "samples": 50,
                "samples_required": 100,
                "window_seconds": null,
            }
        }));
        r.evaluator = Some(EvaluatorBinding {
            evaluator: "sqlite_wal_state".into(),
            version: 1,
        });
        r.freshness_horizon = Some("2026-05-28T14:32:00Z".into());
        r.content_hash = Some(
            "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".into(),
        );
        r
    }

    #[test]
    fn markdown_renders_target_section() {
        let s = render_markdown(&sample_with_consumer_fields());
        assert!(s.contains("### Target"));
        assert!(s.contains("**Host:**"));
        assert!(s.contains("`labelwatch-host`"));
        assert!(s.contains("**Scope:**"));
        assert!(s.contains("`sqlite_wal`"));
        assert!(s.contains("**Id:**"));
        assert!(s.contains("`/opt/nq/nq.db`"));
    }

    #[test]
    fn markdown_renders_refused_claims_section_under_external_heading() {
        let s = render_markdown(&sample_with_consumer_fields());
        assert!(s.contains("### Refused claims"));
        // Refusal entries appear verbatim (they're already user-facing
        // text). The wire word `cannot_testify` must NOT appear as the
        // renderer's heading — external-vocabulary discipline.
        assert!(!s.contains("cannot_testify"));
        assert!(s.contains("application-state claim"));
        assert!(s.contains("consequence claim"));
    }

    #[test]
    fn markdown_renders_signals_section_namespace_aware() {
        let s = render_markdown(&sample_with_consumer_fields());
        assert!(s.contains("### Signals"));
        assert!(s.contains("**`sqlite_wal_state`:**"));
        assert!(s.contains("`threshold_band`: `bounded`"));
        assert!(s.contains("`pinned_reader`: `unobserved`"));
        assert!(s.contains("`samples`: `50`"));
        // null values render as em-dash, not "null" — distinguishable
        // from 0 / false / explicit-string-"null".
        assert!(s.contains("`window_seconds`: —"));
        assert!(!s.contains("`window_seconds`: null"));
    }

    #[test]
    fn markdown_omits_consumer_fields_when_absent() {
        // Same sample as the existing tests use — None/empty for the
        // three new fields. None of the new section headers should
        // appear.
        let s = render_markdown(&sample());
        assert!(!s.contains("### Target"));
        assert!(!s.contains("### Refused claims"));
        assert!(!s.contains("### Signals"));
    }

    #[test]
    fn markdown_external_vocabulary_holds_with_consumer_fields_populated() {
        // Re-run the external-vocabulary discipline against a fully
        // populated receipt: even with cannot_testify, signals, and
        // target set, the renderer's own vocabulary stays external.
        let s = render_markdown(&sample_with_consumer_fields());
        assert!(!s.contains("cannot_testify"));
        assert!(!s.contains("admissible"));
        assert!(!s.contains("admissibility"));
    }

    #[test]
    fn markdown_signals_falls_back_to_json_for_non_object_payload() {
        // Defensive: if signals carries a scalar / array (off-pattern
        // shape from a future kind), render it as a fenced JSON block
        // rather than crashing or silently dropping it.
        let mut r = sample();
        r.signals = Some(serde_json::json!(["unexpected", "shape"]));
        let s = render_markdown(&r);
        assert!(s.contains("### Signals"));
        assert!(s.contains("```json"));
        assert!(s.contains("unexpected"));
    }

    #[test]
    fn human_renders_target_section() {
        let s = render_human(&sample_with_consumer_fields());
        assert!(s.contains("Target:"));
        assert!(s.contains("Host:  labelwatch-host"));
        assert!(s.contains("Scope: sqlite_wal"));
        assert!(s.contains("Id:    /opt/nq/nq.db"));
    }

    #[test]
    fn human_renders_refused_claims_section() {
        let s = render_human(&sample_with_consumer_fields());
        assert!(s.contains("Refused claims:"));
        assert!(s.contains("application-state claim"));
        assert!(s.contains("consequence claim"));
    }

    #[test]
    fn human_renders_signals_section_namespace_aware() {
        let s = render_human(&sample_with_consumer_fields());
        assert!(s.contains("Signals:"));
        assert!(s.contains("sqlite_wal_state:"));
        assert!(s.contains("threshold_band: `bounded`"));
        assert!(s.contains("pinned_reader: `unobserved`"));
        assert!(s.contains("window_seconds: —"));
    }

    #[test]
    fn human_omits_consumer_fields_when_absent() {
        let s = render_human(&sample());
        assert!(!s.contains("Target:"));
        assert!(!s.contains("Refused claims:"));
        assert!(!s.contains("Signals:"));
    }

    // -----------------------------------------------------------------
    // Receipt metadata parity — render freshness_horizon, evaluator,
    // and content_hash. Per-surface policy:
    //
    //   human:    freshness_horizon (load-bearing) + evaluator (provenance).
    //             content_hash omitted (terminal noise).
    //   markdown: all three. freshness_horizon near top (consumer reads
    //             top-down); evaluator + content_hash in the <sub> footer.
    //
    // External-vocabulary discipline holds: timestamp shown, no
    // computed stale/live posture in the renderer.
    // -----------------------------------------------------------------

    #[test]
    fn human_renders_freshness_horizon_when_present() {
        let s = render_human(&sample_with_consumer_fields());
        assert!(s.contains("Freshness horizon: 2026-05-28T14:32:00Z"));
    }

    #[test]
    fn human_renders_evaluator_when_present() {
        let s = render_human(&sample_with_consumer_fields());
        assert!(s.contains("Evaluator: sqlite_wal_state v1"));
    }

    #[test]
    fn human_does_not_render_content_hash() {
        // content_hash is intentionally markdown-only — terminal noise
        // when present, and consumers who need it can fall through to
        // JSON. Pinning the omission so a well-meaning future edit
        // doesn't sprinkle 64-hex characters across `nq` CLI output.
        let s = render_human(&sample_with_consumer_fields());
        assert!(!s.contains("Receipt hash"));
        assert!(!s.contains("sha256:abcdef"));
    }

    #[test]
    fn human_omits_horizon_and_evaluator_when_absent() {
        let s = render_human(&sample());
        assert!(!s.contains("Freshness horizon"));
        assert!(!s.contains("Evaluator:"));
    }

    #[test]
    fn markdown_renders_freshness_horizon_near_top() {
        let s = render_markdown(&sample_with_consumer_fields());
        assert!(s.contains("**Freshness horizon:** `2026-05-28T14:32:00Z`"));
        // Top placement: horizon appears before any ### section header.
        let horizon_idx = s.find("Freshness horizon").expect("horizon present");
        if let Some(first_section) = s.find("###") {
            assert!(
                horizon_idx < first_section,
                "freshness horizon must appear before the first ### section so \
                 consumers reading top-down see it before substantive content"
            );
        }
    }

    #[test]
    fn markdown_renders_evaluator_in_footer() {
        let s = render_markdown(&sample_with_consumer_fields());
        assert!(s.contains("<sub>Evaluator: `sqlite_wal_state` v1</sub>"));
    }

    #[test]
    fn markdown_renders_content_hash_in_footer() {
        let s = render_markdown(&sample_with_consumer_fields());
        assert!(s.contains(
            "<sub>Receipt hash: `sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789`</sub>"
        ));
    }

    #[test]
    fn markdown_omits_horizon_evaluator_hash_when_absent() {
        let s = render_markdown(&sample());
        assert!(!s.contains("Freshness horizon"));
        assert!(!s.contains("Evaluator:"));
        assert!(!s.contains("Receipt hash"));
    }

    #[test]
    fn renderer_does_not_compute_stale_or_live_from_horizon() {
        // The renderer must not derive a stale/live verdict from the
        // horizon. Doing so would turn the renderer into an evaluator
        // with an implicit clock — exactly the kind of laundering the
        // external-vocabulary discipline exists to refuse.
        let h = render_human(&sample_with_consumer_fields());
        let m = render_markdown(&sample_with_consumer_fields());
        // "Fresh" is excluded because the field label itself is
        // "Freshness horizon". The verdict-shaped words we're guarding
        // against are derivations *from* the horizon ("stale", "live",
        // "expired", etc.) — not the label naming the horizon.
        for forbidden in ["stale", "live", "expired", "valid", "outdated"] {
            assert!(
                !h.to_lowercase().contains(forbidden),
                "human renderer must not contain verdict-shaped freshness word {forbidden:?}"
            );
            assert!(
                !m.to_lowercase().contains(forbidden),
                "markdown renderer must not contain verdict-shaped freshness word {forbidden:?}"
            );
        }
    }
}
