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

    out
}

pub fn render_json(r: &Receipt) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(r)
}

pub fn render_jsonl(r: &Receipt) -> Result<String, serde_json::Error> {
    serde_json::to_string(r)
}

/// GitHub-flavored markdown rendering of a receipt. Suitable for PR
/// comments and dashboards. Uses the external vocabulary only.
pub fn render_markdown(r: &Receipt) -> String {
    let mut out = String::new();
    out.push_str("## NQ Verification Receipt\n\n");
    out.push_str(&format!("**Claim:** `{}`  \n", r.claim));
    out.push_str(&format!("**Subject:** `{}`  \n", r.subject));
    out.push_str(&format!("**Status:** `{}`\n\n", status_word(r.status)));

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

    out
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
    use crate::receipt::{NotVerifiedEntry, Receipt, Status, StatusReason, WitnessRef};

    fn sample() -> Receipt {
        Receipt {
            schema: "nq.receipt.v1".into(),
            claim: "safe_to_merge".into(),
            subject: "repo:.".into(),
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
            witnesses: vec![WitnessRef {
                witness_type: "git_status".into(),
                digest: None,
                observed_at: Some("2026-05-15T14:00:00Z".into()),
            }],
            observed_at_min: Some("2026-05-15T13:59:51Z".into()),
            observed_at_max: Some("2026-05-15T14:00:02Z".into()),
            generated_at: "2026-05-15T14:00:04Z".into(),
            evaluator: None,
            freshness_horizon: None,
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
}
