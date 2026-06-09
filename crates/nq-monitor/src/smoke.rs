//! Contract validators for `nq-monitor smoke` subcommands.
//!
//! These are pure functions over parsed JSON, separated from the CLI's
//! HTTP plumbing so they can be unit-tested against handcrafted bodies
//! and re-used from integration tests that hit the actual HTTP route.
//!
//! Smoke contract is intentionally narrow: confirm the operator-facing
//! surface honors the wire schema and the constitutional refusal
//! surface. The verdict NQ minted is *not* a smoke concern —
//! `cannot_testify` and `contradictory_testimony` are honest outcomes.

use anyhow::{anyhow, Result};
use serde_json::Value;

/// Wire schema the disk_state preflight envelope must advertise.
pub const EXPECTED_DISK_STATE_SCHEMA: &str = "nq.preflight.disk_state.v1";

/// Contract version the disk_state preflight envelope must advertise.
/// Bumped 1 -> 2 on 2026-06-09 with the typed-refusal migration; see
/// `docs/working/gaps/WITNESS_CLAIM_SCOPE_GAP.md`.
pub const EXPECTED_DISK_STATE_CONTRACT_VERSION: u64 = 2;

/// Wire schema the ingest_state preflight envelope must advertise.
pub const EXPECTED_INGEST_STATE_SCHEMA: &str = "nq.preflight.ingest_state.v1";

/// Contract version the ingest_state preflight envelope must advertise.
/// Bumped 1 -> 2 on 2026-06-09 with the typed-refusal migration.
pub const EXPECTED_INGEST_STATE_CONTRACT_VERSION: u64 = 2;

/// Substrings that must not appear in any `supports[].claim` on an
/// `ingest_state` envelope. Mirrors the constitutional `cannot_testify`
/// surface for ingest_state: a support that names restart /
/// reconfigure / deactivate / recover has laundered consequence
/// vocabulary onto substrate testimony. Upstream-source-health and
/// future-state laundering is harder to substring-match defensively
/// — those rely on the cannot_testify presence check rather than the
/// support-vocabulary anti-laundering scan.
pub const INGEST_FORBIDDEN_SUPPORT_SUBSTRINGS: &[&str] = &[
    "restart",
    "reconfigure",
    "deactivate",
    "recover",
    "closure",
    "incident closed",
];

/// Substrings that must not appear in any `supports[].claim`. Mirrors
/// the constitutional `cannot_testify` surface for disk_state: a
/// support that names physical death, replacement, recovery /
/// data-loss, "fine to keep," or incident closure has laundered
/// consequence vocabulary onto substrate testimony.
pub const FORBIDDEN_SUPPORT_SUBSTRINGS: &[&str] = &[
    "replace",
    "recover",
    "dead",
    "is fine",
    "fine to keep",
    "data loss",
    "data lost",
    "closure",
    "incident closed",
];

/// Summary returned on contract success. Carries the verdict and the
/// surface counts so the CLI can render a one-line OK message; nothing
/// in this struct is consulted to decide pass/fail.
#[derive(Debug, Clone)]
pub struct DiskStateSmokeReport {
    pub verdict: String,
    pub supports_count: usize,
    pub cannot_testify_count: usize,
    pub coverage_count: usize,
}

/// Validate a `disk_state_preflight` envelope against the contract.
///
/// Returns `Ok` if the envelope is contract-shaped, regardless of the
/// verdict it carries (a `cannot_testify` envelope with a populated
/// constitutional refusal surface is a contract success). Returns
/// `Err` only on contract failure: schema mismatch, missing
/// `contract_version`, missing `verdict`, missing or empty
/// `cannot_testify`, missing `supports` / `excludes` / `coverage`,
/// laundered consequence vocabulary in `supports[].claim`, or
/// `observed_at_min`/`observed_at_max` discipline violations.
pub fn validate_disk_state_envelope(envelope: &Value) -> Result<DiskStateSmokeReport> {
    let schema = envelope
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("envelope missing `schema`"))?;
    if schema != EXPECTED_DISK_STATE_SCHEMA {
        return Err(anyhow!(
            "schema mismatch: expected {EXPECTED_DISK_STATE_SCHEMA:?}, got {schema:?}"
        ));
    }

    let contract_version = envelope
        .get("contract_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("envelope missing `contract_version`"))?;
    if contract_version != EXPECTED_DISK_STATE_CONTRACT_VERSION {
        return Err(anyhow!(
            "contract_version mismatch: expected {EXPECTED_DISK_STATE_CONTRACT_VERSION}, got {contract_version}"
        ));
    }

    let verdict = envelope
        .get("verdict")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("envelope missing `verdict`"))?
        .to_string();

    let supports = envelope
        .get("supports")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("envelope missing `supports[]`"))?;

    // `excludes` and `coverage` must be arrays; the smoke does not
    // examine their contents beyond presence + shape.
    envelope
        .get("excludes")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("envelope missing `excludes[]`"))?;

    let coverage = envelope
        .get("coverage")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("envelope missing `coverage[]`"))?;

    let cannot_testify = envelope
        .get("cannot_testify")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("envelope missing `cannot_testify[]`"))?;
    if cannot_testify.is_empty() {
        return Err(anyhow!(
            "cannot_testify is empty; the constitutional refusal surface must always be populated"
        ));
    }

    // Anti-laundering on supports[].claim.
    for (i, support) in supports.iter().enumerate() {
        let claim = support
            .get("claim")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("supports[{i}] missing `claim`"))?;
        let lower = claim.to_ascii_lowercase();
        for needle in FORBIDDEN_SUPPORT_SUBSTRINGS {
            if lower.contains(needle) {
                return Err(anyhow!(
                    "supports[{i}].claim laundered consequence vocabulary ({needle:?}): {claim:?}"
                ));
            }
        }
    }

    // Observation-window discipline. Empty supports must omit both
    // window fields entirely (absent testimony must not advertise a
    // window). Non-empty supports must carry both, and min must not
    // exceed max.
    let observed_min = envelope.get("observed_at_min");
    let observed_max = envelope.get("observed_at_max");
    if supports.is_empty() {
        if observed_min.is_some() {
            return Err(anyhow!(
                "observed_at_min present with empty supports; absent testimony must not advertise a window"
            ));
        }
        if observed_max.is_some() {
            return Err(anyhow!(
                "observed_at_max present with empty supports; absent testimony must not advertise a window"
            ));
        }
    } else {
        let min = observed_min
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("supports is non-empty but observed_at_min is missing"))?;
        let max = observed_max
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("supports is non-empty but observed_at_max is missing"))?;
        if min > max {
            return Err(anyhow!(
                "observed_at_min ({min}) exceeds observed_at_max ({max})"
            ));
        }
    }

    Ok(DiskStateSmokeReport {
        verdict,
        supports_count: supports.len(),
        cannot_testify_count: cannot_testify.len(),
        coverage_count: coverage.len(),
    })
}

/// Summary returned on `ingest_state` contract success. Same shape as
/// `DiskStateSmokeReport`; the type is kept distinct so the CLI can
/// label the output appropriately and so future per-kind smokes don't
/// have to retrofit a generic envelope.
#[derive(Debug, Clone)]
pub struct IngestStateSmokeReport {
    pub verdict: String,
    pub supports_count: usize,
    pub cannot_testify_count: usize,
    pub coverage_count: usize,
}

/// Validate an `ingest_state` PreflightResult envelope against the
/// contract. Same posture as [`validate_disk_state_envelope`]: an
/// envelope carrying `verdict: cannot_testify` with a populated
/// constitutional refusal surface is a contract success. The
/// vocabulary-laundering scan uses [`INGEST_FORBIDDEN_SUPPORT_SUBSTRINGS`];
/// the schema/contract_version/supports/coverage/cannot_testify shape
/// is otherwise identical.
pub fn validate_ingest_state_envelope(envelope: &Value) -> Result<IngestStateSmokeReport> {
    let schema = envelope
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("envelope missing `schema`"))?;
    if schema != EXPECTED_INGEST_STATE_SCHEMA {
        return Err(anyhow!(
            "schema mismatch: expected {EXPECTED_INGEST_STATE_SCHEMA:?}, got {schema:?}"
        ));
    }

    let contract_version = envelope
        .get("contract_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("envelope missing `contract_version`"))?;
    if contract_version != EXPECTED_INGEST_STATE_CONTRACT_VERSION {
        return Err(anyhow!(
            "contract_version mismatch: expected {EXPECTED_INGEST_STATE_CONTRACT_VERSION}, got {contract_version}"
        ));
    }

    let verdict = envelope
        .get("verdict")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("envelope missing `verdict`"))?
        .to_string();

    let supports = envelope
        .get("supports")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("envelope missing `supports[]`"))?;

    envelope
        .get("excludes")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("envelope missing `excludes[]`"))?;

    let coverage = envelope
        .get("coverage")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("envelope missing `coverage[]`"))?;

    let cannot_testify = envelope
        .get("cannot_testify")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("envelope missing `cannot_testify[]`"))?;
    if cannot_testify.is_empty() {
        return Err(anyhow!(
            "cannot_testify is empty; the constitutional refusal surface must always be populated"
        ));
    }

    for (i, support) in supports.iter().enumerate() {
        let claim = support
            .get("claim")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("supports[{i}] missing `claim`"))?;
        let lower = claim.to_ascii_lowercase();
        for needle in INGEST_FORBIDDEN_SUPPORT_SUBSTRINGS {
            if lower.contains(needle) {
                return Err(anyhow!(
                    "supports[{i}].claim laundered consequence vocabulary ({needle:?}): {claim:?}"
                ));
            }
        }
    }

    // Observation-window discipline. Same shape as disk_state: empty
    // supports must omit both window fields; non-empty supports must
    // carry both with min <= max.
    let observed_min = envelope.get("observed_at_min");
    let observed_max = envelope.get("observed_at_max");
    if supports.is_empty() {
        if observed_min.is_some() {
            return Err(anyhow!(
                "observed_at_min present with empty supports; absent testimony must not advertise a window"
            ));
        }
        if observed_max.is_some() {
            return Err(anyhow!(
                "observed_at_max present with empty supports; absent testimony must not advertise a window"
            ));
        }
    } else {
        let min = observed_min
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("supports is non-empty but observed_at_min is missing"))?;
        let max = observed_max
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("supports is non-empty but observed_at_max is missing"))?;
        if min > max {
            return Err(anyhow!(
                "observed_at_min ({min}) exceeds observed_at_max ({max})"
            ));
        }
    }

    Ok(IngestStateSmokeReport {
        verdict,
        supports_count: supports.len(),
        cannot_testify_count: cannot_testify.len(),
        coverage_count: coverage.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn admissible_envelope() -> Value {
        json!({
            "schema": "nq.preflight.disk_state.v1",
            "contract_version": 2,
            "claim_kind": "disk_state",
            "target": { "host": "lil-nas-x", "scope": "host" },
            "verdict": "admissible_with_scope",
            "supports": [
                {
                    "claim": "ZFS reports pool 'tank' state as DEGRADED at observed_at 2026-05-14T00:00:00Z",
                    "finding_kind": "zfs_pool_degraded",
                    "subject": "tank",
                    "observed_at": "2026-05-14T00:00:00Z"
                },
                {
                    "claim": "SMART reports rising reallocated-sector count on '/dev/sdX' at observed_at 2026-05-14T00:10:00Z",
                    "finding_kind": "smart_reallocated_sectors_rising",
                    "subject": "/dev/sdX",
                    "observed_at": "2026-05-14T00:10:00Z"
                }
            ],
            "excludes": [],
            "cannot_testify": [
                { "refusal_kind": "kind_specific",     "statement": "Physical disk death" },
                { "refusal_kind": "consequence_claim", "statement": "Replacement workflow" }
            ],
            "coverage": [
                { "witness": "zfs_witness", "standing": "observable" }
            ],
            "generated_at": "2026-05-14T00:11:00Z",
            "observed_at_min": "2026-05-14T00:00:00Z",
            "observed_at_max": "2026-05-14T00:10:00Z"
        })
    }

    fn cannot_testify_envelope() -> Value {
        json!({
            "schema": "nq.preflight.disk_state.v1",
            "contract_version": 2,
            "claim_kind": "disk_state",
            "target": { "host": "ghost", "scope": "host" },
            "verdict": "cannot_testify",
            "verdict_note": "Host is unobservable",
            "supports": [],
            "excludes": [],
            "cannot_testify": [
                { "refusal_kind": "kind_specific",     "statement": "Physical disk death" },
                { "refusal_kind": "consequence_claim", "statement": "Replacement workflow" },
                { "refusal_kind": "above_substrate",   "statement": "Data loss occurrence, recoverability, or unrecoverability" }
            ],
            "coverage": [
                { "witness": "zfs_witness", "standing": "node_unobservable" }
            ],
            "generated_at": "2026-05-14T00:11:00Z"
        })
    }

    #[test]
    fn admissible_with_scope_response_passes_contract() {
        let r = validate_disk_state_envelope(&admissible_envelope()).expect("contract ok");
        assert_eq!(r.verdict, "admissible_with_scope");
        assert_eq!(r.supports_count, 2);
        assert_eq!(r.cannot_testify_count, 2);
        assert_eq!(r.coverage_count, 1);
    }

    #[test]
    fn cannot_testify_response_passes_contract() {
        // The verdict is a refusal but the envelope is contract-shaped:
        // smoke must not flag honest refusals as failures.
        let r = validate_disk_state_envelope(&cannot_testify_envelope()).expect("contract ok");
        assert_eq!(r.verdict, "cannot_testify");
        assert_eq!(r.supports_count, 0);
    }

    #[test]
    fn wrong_schema_fails() {
        let mut env = admissible_envelope();
        env["schema"] = json!("nq.preflight.disk_state.v0");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(err.to_string().contains("schema mismatch"), "{err}");
    }

    #[test]
    fn missing_schema_fails() {
        let mut env = admissible_envelope();
        env.as_object_mut().unwrap().remove("schema");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(err.to_string().contains("missing `schema`"), "{err}");
    }

    #[test]
    fn wrong_contract_version_fails() {
        let mut env = admissible_envelope();
        // Anything other than EXPECTED_DISK_STATE_CONTRACT_VERSION must
        // be rejected. v1 (the pre-typed-refusal shape) is the canonical
        // wrong version a producer might still emit.
        env["contract_version"] = json!(1);
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string().contains("contract_version mismatch"),
            "{err}"
        );
    }

    #[test]
    fn missing_cannot_testify_fails() {
        let mut env = admissible_envelope();
        env.as_object_mut().unwrap().remove("cannot_testify");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(err.to_string().contains("cannot_testify"), "{err}");
    }

    #[test]
    fn empty_cannot_testify_fails() {
        let mut env = admissible_envelope();
        env["cannot_testify"] = json!([]);
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string().contains("constitutional refusal surface"),
            "{err}"
        );
    }

    #[test]
    fn laundering_replace_vocabulary_fails() {
        let mut env = admissible_envelope();
        env["supports"][0]["claim"] = json!("replace this drive immediately");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string().contains("laundered consequence vocabulary"),
            "{err}"
        );
        assert!(err.to_string().contains("\"replace\""), "{err}");
    }

    #[test]
    fn laundering_dead_vocabulary_fails() {
        let mut env = admissible_envelope();
        env["supports"][0]["claim"] = json!("drive is dead per substrate evidence");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string().contains("laundered consequence vocabulary"),
            "{err}"
        );
    }

    #[test]
    fn laundering_data_loss_vocabulary_fails() {
        let mut env = admissible_envelope();
        env["supports"][0]["claim"] = json!("data loss is implied by these counters");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string().contains("laundered consequence vocabulary"),
            "{err}"
        );
    }

    #[test]
    fn laundering_closure_vocabulary_fails() {
        let mut env = admissible_envelope();
        env["supports"][0]["claim"] = json!("incident closure is warranted");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string().contains("laundered consequence vocabulary"),
            "{err}"
        );
    }

    #[test]
    fn laundering_fine_to_keep_vocabulary_fails() {
        let mut env = admissible_envelope();
        env["supports"][0]["claim"] = json!("drive is fine to keep, no action needed");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string().contains("laundered consequence vocabulary"),
            "{err}"
        );
    }

    #[test]
    fn observed_at_min_with_empty_supports_fails() {
        let mut env = cannot_testify_envelope();
        env["observed_at_min"] = json!("2026-05-14T00:00:00Z");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string()
                .contains("absent testimony must not advertise a window"),
            "{err}"
        );
    }

    #[test]
    fn observed_at_min_missing_with_supports_fails() {
        let mut env = admissible_envelope();
        env.as_object_mut().unwrap().remove("observed_at_min");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(err.to_string().contains("observed_at_min"), "{err}");
    }

    #[test]
    fn observed_at_min_exceeds_max_fails() {
        let mut env = admissible_envelope();
        env["observed_at_min"] = json!("2026-05-14T00:10:00Z");
        env["observed_at_max"] = json!("2026-05-14T00:00:00Z");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(err.to_string().contains("exceeds observed_at_max"), "{err}");
    }

    #[test]
    fn missing_supports_array_fails() {
        let mut env = admissible_envelope();
        env.as_object_mut().unwrap().remove("supports");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(err.to_string().contains("supports"), "{err}");
    }

    #[test]
    fn support_missing_claim_field_fails() {
        let mut env = admissible_envelope();
        env["supports"][0].as_object_mut().unwrap().remove("claim");
        let err = validate_disk_state_envelope(&env).unwrap_err();
        assert!(err.to_string().contains("missing `claim`"), "{err}");
    }

    // -----------------------------------------------------------------
    // ingest_state smoke — V2 symmetry with disk_state. The witness is
    // the monitor itself (own pull-cycle substrate), so the route is
    // host-agnostic and the body is the PreflightResult directly (no
    // nested `*_preflight` envelope key).
    // -----------------------------------------------------------------

    fn ingest_admissible_envelope() -> Value {
        json!({
            "schema": "nq.preflight.ingest_state.v1",
            "contract_version": 2,
            "claim_kind": "ingest_state",
            "target": { "host": "monitor", "scope": "ingest" },
            "verdict": "admissible_with_scope",
            "supports": [
                {
                    "claim": "Monitor recorded successful pull cycle at observed_at 2026-05-14T00:00:00Z (source=labelwatch-host, status=ok)",
                    "finding_kind": "source_pull_ok",
                    "subject": "labelwatch-host",
                    "observed_at": "2026-05-14T00:00:00Z"
                }
            ],
            "excludes": [],
            "cannot_testify": [
                { "refusal_kind": "environmental_context", "statement": "Upstream source substrate health" },
                { "refusal_kind": "future_state_claim",    "statement": "Future ingest success or failure" },
                { "refusal_kind": "above_substrate",       "statement": "Semantic correctness of ingested data" }
            ],
            "coverage": [
                { "witness": "monitor_pull_cycles", "standing": "observable" }
            ],
            "generated_at": "2026-05-14T00:01:00Z",
            "observed_at_min": "2026-05-14T00:00:00Z",
            "observed_at_max": "2026-05-14T00:00:00Z"
        })
    }

    fn ingest_cannot_testify_envelope() -> Value {
        json!({
            "schema": "nq.preflight.ingest_state.v1",
            "contract_version": 2,
            "claim_kind": "ingest_state",
            "target": { "host": "monitor", "scope": "ingest" },
            "verdict": "cannot_testify",
            "verdict_note": "No pull cycles in the observation window",
            "supports": [],
            "excludes": [],
            "cannot_testify": [
                { "refusal_kind": "environmental_context", "statement": "Upstream source substrate health" },
                { "refusal_kind": "future_state_claim",    "statement": "Future ingest success or failure" },
                { "refusal_kind": "future_state_claim",    "statement": "Whether ingest will recover from the current failure shape" }
            ],
            "coverage": [
                { "witness": "monitor_pull_cycles", "standing": "no_rows" }
            ],
            "generated_at": "2026-05-14T00:01:00Z"
        })
    }

    #[test]
    fn ingest_admissible_response_passes_contract() {
        let r =
            validate_ingest_state_envelope(&ingest_admissible_envelope()).expect("contract ok");
        assert_eq!(r.verdict, "admissible_with_scope");
        assert_eq!(r.supports_count, 1);
        assert!(r.cannot_testify_count >= 1);
        assert_eq!(r.coverage_count, 1);
    }

    #[test]
    fn ingest_cannot_testify_response_passes_contract() {
        let r = validate_ingest_state_envelope(&ingest_cannot_testify_envelope())
            .expect("contract ok");
        assert_eq!(r.verdict, "cannot_testify");
        assert_eq!(r.supports_count, 0);
    }

    #[test]
    fn ingest_wrong_schema_fails() {
        let mut env = ingest_admissible_envelope();
        env["schema"] = json!("nq.preflight.disk_state.v1");
        let err = validate_ingest_state_envelope(&env).unwrap_err();
        assert!(err.to_string().contains("schema mismatch"), "{err}");
    }

    #[test]
    fn ingest_empty_cannot_testify_fails() {
        let mut env = ingest_admissible_envelope();
        env["cannot_testify"] = json!([]);
        let err = validate_ingest_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string().contains("constitutional refusal surface"),
            "{err}"
        );
    }

    #[test]
    fn ingest_laundering_restart_vocabulary_fails() {
        // ingest_state's cannot_testify refuses "restart, reconfigure,
        // or deactivate a failing source" as consequence. A support
        // claim that names that is laundering.
        let mut env = ingest_admissible_envelope();
        env["supports"][0]["claim"] = json!("operator should restart the failing source");
        let err = validate_ingest_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string().contains("laundered consequence vocabulary"),
            "{err}"
        );
        assert!(err.to_string().contains("\"restart\""), "{err}");
    }

    #[test]
    fn ingest_laundering_recover_vocabulary_fails() {
        let mut env = ingest_admissible_envelope();
        env["supports"][0]["claim"] = json!("ingest will recover when the source returns");
        let err = validate_ingest_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string().contains("laundered consequence vocabulary"),
            "{err}"
        );
    }

    #[test]
    fn ingest_observed_at_min_with_empty_supports_fails() {
        let mut env = ingest_cannot_testify_envelope();
        env["observed_at_min"] = json!("2026-05-14T00:00:00Z");
        let err = validate_ingest_state_envelope(&env).unwrap_err();
        assert!(
            err.to_string()
                .contains("absent testimony must not advertise a window"),
            "{err}"
        );
    }

    #[test]
    fn ingest_observed_at_min_exceeds_max_fails() {
        let mut env = ingest_admissible_envelope();
        env["observed_at_min"] = json!("2026-05-14T00:10:00Z");
        env["observed_at_max"] = json!("2026-05-14T00:00:00Z");
        let err = validate_ingest_state_envelope(&env).unwrap_err();
        assert!(err.to_string().contains("exceeds observed_at_max"), "{err}");
    }
}
