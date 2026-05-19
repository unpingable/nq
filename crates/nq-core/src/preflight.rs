//! Claim preflight types — operator-facing surface that consumes existing
//! NQ testimony and returns a bounded verdict + evidence bundle.
//!
//! See `docs/CLAIM_PREFLIGHT.md`, `docs/VERDICTS.md`, `docs/WITNESS_PACKET.md`,
//! and `docs/gaps/CLAIM_KIND_DISK_STATE_GAP.md` for the doctrine. This module
//! defines the consumer-facing DTO shape; the evaluator (`nq-db::preflight`)
//! computes a `PreflightResult` from existing findings and standing state.
//!
//! V1 covers one claim kind: `disk_state`. The eight-verdict vocabulary is
//! shared across claim kinds. Structured `ClaimKind` only — no operator-phrase
//! intake at this layer.

use serde::{Deserialize, Serialize};

/// Wire schema identifier for `disk_state` preflight results.
pub const PREFLIGHT_DISK_STATE_SCHEMA: &str = "nq.preflight.disk_state.v1";

/// Wire schema identifier for `ingest_state` preflight results. NQ
/// testifies about its own ingest pulse structure (the aggregator's
/// `generations` and `source_runs` rows). It does **not** testify
/// about upstream source substrate or about its own overall health.
pub const PREFLIGHT_INGEST_STATE_SCHEMA: &str = "nq.preflight.ingest_state.v1";

/// Contract version for the preflight wire shape. Bumps on breaking change.
pub const PREFLIGHT_CONTRACT_VERSION: u32 = 1;

/// Structured claim kind. V2 covers `DiskState` and `IngestState`. New
/// kinds require a separate ratified change. The bespoke per-kind
/// pattern stands until the third claim kind creates a concrete
/// pressure point for registry generalization (see
/// `docs/gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    DiskState,
    IngestState,
}

/// The eight verdicts from `docs/VERDICTS.md`. Non-overlapping in primary
/// trigger; the more-specific one wins when two could apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Admissible,
    AdmissibleWithScope,
    UnsupportedAsStated,
    ClaimExceedsTestimony,
    InsufficientCoverage,
    StaleTestimony,
    ContradictoryTestimony,
    CannotTestify,
}

/// What the preflight is being asked to evaluate. `scope` is the granularity
/// of the target identity; `id` is the specific subject when scope is finer
/// than host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightTarget {
    pub host: String,
    /// One of `host`, `pool`, `vdev`, `device`.
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// One admissible weaker claim, with provenance back to the underlying
/// finding. The `claim` text is scoped — it carries witness, subject, and
/// observed_at — so a consumer that quotes only the `claim` field cannot
/// launder the scope away.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightSupport {
    pub claim: String,
    pub finding_kind: String,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admissibility_state: Option<String>,
}

/// A finding that exists for the target but is not being admitted as a
/// supporting weaker claim. `reason` says why (suppressed by ancestor /
/// declaration, cleared, stale, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightExclusion {
    pub finding_kind: String,
    pub subject: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Standing report for one witness family. `standing` is one of `observable`,
/// `silent`, `node_unobservable`, or `absent`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightCoverage {
    pub witness: String,
    pub standing: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Preflight result. Constitutional `cannot_testify` entries are always
/// populated regardless of substrate state — they are the refusal surface
/// the claim kind exists to maintain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightResult {
    pub schema: String,
    pub contract_version: u32,
    pub claim_kind: ClaimKind,
    pub target: PreflightTarget,
    pub verdict: Verdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict_note: Option<String>,
    pub supports: Vec<PreflightSupport>,
    pub excludes: Vec<PreflightExclusion>,
    /// Constitutional refusal surface for this claim kind. Always populated.
    /// Per `CLAIM_KIND_DISK_STATE_GAP.md`, no combination of witness output
    /// licenses any of these conclusions.
    pub cannot_testify: Vec<String>,
    pub coverage: Vec<PreflightCoverage>,
    pub generated_at: String,
    /// Oldest `observed_at` among `supports[]`. `None` when supports is
    /// empty or no support carries an observed_at. This is evidence-window
    /// disclosure only — it does not imply validity, freshness policy, or
    /// any deadline. NQ exposes when testimony was observed; consumers
    /// decide what to do with that information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at_min: Option<String>,
    /// Newest `observed_at` among `supports[]`. Same semantics as
    /// `observed_at_min`: window disclosure, no validity claim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at_max: Option<String>,
}

impl PreflightResult {
    /// Construct an empty result skeleton for the given claim kind and target.
    /// The caller fills in supports / excludes / coverage / verdict.
    /// `cannot_testify` is preloaded with the constitutional refusal list for
    /// the claim kind.
    pub fn skeleton(claim_kind: ClaimKind, target: PreflightTarget, generated_at: String) -> Self {
        let (schema, cannot_testify) = match claim_kind {
            ClaimKind::DiskState => (
                PREFLIGHT_DISK_STATE_SCHEMA.to_string(),
                disk_state_cannot_testify(),
            ),
            ClaimKind::IngestState => (
                PREFLIGHT_INGEST_STATE_SCHEMA.to_string(),
                ingest_state_cannot_testify(),
            ),
        };
        Self {
            schema,
            contract_version: PREFLIGHT_CONTRACT_VERSION,
            claim_kind,
            target,
            verdict: Verdict::InsufficientCoverage,
            verdict_note: None,
            supports: Vec::new(),
            excludes: Vec::new(),
            cannot_testify,
            coverage: Vec::new(),
            generated_at,
            observed_at_min: None,
            observed_at_max: None,
        }
    }
}

/// Constitutional refusal surface for `ingest_state`. Each entry
/// corresponds to a conclusion the `generations` / `source_runs`
/// substrate does not license, regardless of which generation rows
/// are present. NQ testifies that its own pull cycle ran (or
/// failed); it does not testify about upstream substrate, semantic
/// content, or its own overall health. The "NQ itself is healthy"
/// refusal is the self-witness firewall: a witness about itself is
/// circular, and `ingest_state` is one channel among many that a
/// downstream system might (separately) read.
pub fn ingest_state_cannot_testify() -> Vec<String> {
    vec![
        "Upstream source substrate health (NQ observed its own pull attempt; the source's actual state is upstream and beyond witness)".to_string(),
        "Future ingest success or failure".to_string(),
        "Semantic correctness of ingested data (the pull cycle's structural state is testifiable; the content's truth is not)".to_string(),
        "Network connectivity health".to_string(),
        "Whether to restart, reconfigure, or deactivate a failing source (consequence claim)".to_string(),
        "NQ's own overall health (the witness cannot be its own complete audit)".to_string(),
        "Whether ingest will recover from the current failure shape (future-state claim)".to_string(),
    ]
}

/// Constitutional refusal surface for `disk_state`. Each entry corresponds to
/// a conclusion no combination of ZFS / SMART / disk-pressure witness output
/// licenses, regardless of how many findings light up. Mirrors the
/// `cannot_testify` enumeration in `docs/gaps/CLAIM_KIND_DISK_STATE_GAP.md`.
pub fn disk_state_cannot_testify() -> Vec<String> {
    vec![
        "Physical disk death".to_string(),
        "Replacement workflow (authorization, initiation, skipping, completion, closure-criteria satisfaction)".to_string(),
        "Physical component identity beyond witness coverage (sled / slot / enclosure / asset-record)".to_string(),
        "Data loss occurrence, recoverability, or unrecoverability".to_string(),
        "Future failure probability".to_string(),
        "Incident closure readiness".to_string(),
        "Drive is fine to keep / no action required (mirror consequence claim)".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_state_skeleton_has_constitutional_refusals() {
        let target = PreflightTarget {
            host: "h".into(),
            scope: "host".into(),
            id: None,
        };
        let r = PreflightResult::skeleton(ClaimKind::DiskState, target, "2026-05-14T00:00:00Z".into());
        assert_eq!(r.schema, PREFLIGHT_DISK_STATE_SCHEMA);
        assert_eq!(r.contract_version, PREFLIGHT_CONTRACT_VERSION);
        // The seven constitutional refusals must be present.
        assert!(r.cannot_testify.iter().any(|s| s.contains("Physical disk death")));
        assert!(r.cannot_testify.iter().any(|s| s.starts_with("Replacement workflow")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("Incident closure")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("Drive is fine to keep")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("Data loss")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("Future failure probability")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("Physical component identity")));
    }

    #[test]
    fn verdict_serializes_snake_case() {
        let v = Verdict::AdmissibleWithScope;
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, "\"admissible_with_scope\"");
        let v = Verdict::CannotTestify;
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, "\"cannot_testify\"");
    }

    #[test]
    fn claim_kind_serializes_snake_case() {
        let k = ClaimKind::DiskState;
        let s = serde_json::to_string(&k).unwrap();
        assert_eq!(s, "\"disk_state\"");
        let k = ClaimKind::IngestState;
        let s = serde_json::to_string(&k).unwrap();
        assert_eq!(s, "\"ingest_state\"");
    }

    #[test]
    fn ingest_state_skeleton_has_constitutional_refusals() {
        let target = PreflightTarget {
            host: "monitor".into(),
            scope: "ingest".into(),
            id: None,
        };
        let r = PreflightResult::skeleton(
            ClaimKind::IngestState,
            target,
            "2026-05-19T00:00:00Z".into(),
        );
        assert_eq!(r.schema, PREFLIGHT_INGEST_STATE_SCHEMA);
        assert_eq!(r.contract_version, PREFLIGHT_CONTRACT_VERSION);
        // The self-witness firewall and upstream-substrate refusal must be
        // present — they are the constitutional shape of this claim kind.
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Upstream source substrate")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("NQ's own overall health")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Future ingest")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Semantic correctness")));
    }
}
