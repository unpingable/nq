//! `nq_sql_contract_state` preflight evaluator (NQ-on-NQ-002).
//!
//! Consumes a `nq.sql_contract.public_views.v1` receipt emitted at the
//! test boundary by `crates/nq-db/tests/sql_contract.rs` and turns its
//! pass/fail into a preflight verdict.
//!
//! The receipt is the substrate. This evaluator does **not** introspect
//! the database — that would collapse the test/runtime separation the
//! receipt boundary exists to maintain. The receipt is produced beside
//! tests; the verdict is rendered at runtime; the two layers never
//! meet.
//!
//! ## Verdict mapping
//!
//! | Trigger                                         | Verdict                |
//! |-------------------------------------------------|------------------------|
//! | artifact file missing / unreadable              | `CannotTestify`        |
//! | file present but not valid JSON                 | `CannotTestify`        |
//! | JSON valid but `schema` field absent / wrong    | `InsufficientCoverage` |
//! | schema matches, `result` malformed              | `CannotTestify`        |
//! | schema matches, `result` = `"pass"`             | `AdmissibleWithScope`  |
//! | schema matches, `result` = `"fail"`             | `UnsupportedAsStated`  |
//!
//! `UnsupportedAsStated` is the closest variant to "Contradicted" in the
//! nq-core 8-verdict taxonomy: the claim "the public SQL contract
//! holds" is not supported by the available evidence when the receipt
//! reports missing views. `ContradictoryTestimony` is reserved for two
//! pieces of evidence disagreeing; here there is only one receipt.
//!
//! ## Negative scope preservation
//!
//! The receipt's `scope.does_not_check` list is preserved verbatim in
//! `signals.nq_sql_contract_state.scope_does_not_check`. The
//! kind-level constitutional refusals live in `cannot_testify` per
//! `nq_core::preflight::nq_sql_contract_state_cannot_testify`. Both
//! layers travel with the verdict so consumers cannot inflate "public
//! view existence holds" into "the operator SQL contract is fully
//! satisfied."

use nq_core::preflight::{ClaimKind, PreflightResult, PreflightTarget, Verdict};
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Wire schema the receipt must carry for this evaluator to consume it.
pub const EXPECTED_RECEIPT_SCHEMA: &str = "nq.sql_contract.public_views.v1";

/// Target identity for the SQL contract preflight: which NQ host's
/// receipt to consume, and where the artifact lives on the filesystem.
#[derive(Debug, Clone)]
pub struct NqSqlContractStateTarget {
    pub host: String,
    pub artifact_path: PathBuf,
}

/// Public entry point. Reads the artifact at `target.artifact_path`,
/// classifies it against the four-verdict mapping above, and returns a
/// `PreflightResult` carrying the kind-level constitutional refusals
/// plus the per-receipt negative scope.
pub fn evaluate_nq_sql_contract_state_preflight(
    target: &NqSqlContractStateTarget,
) -> PreflightResult {
    let now = OffsetDateTime::now_utc();
    let generated_at = now.format(&Rfc3339).unwrap_or_default();

    let preflight_target = PreflightTarget {
        host: target.host.clone(),
        scope: "artifact".to_string(),
        id: Some(target.artifact_path.display().to_string()),
    };
    let mut result = PreflightResult::skeleton(
        ClaimKind::NqSqlContractState,
        preflight_target,
        generated_at,
    );

    classify(&mut result, &target.artifact_path);
    result
}

fn classify(result: &mut PreflightResult, artifact_path: &Path) {
    // 1. Read file.
    let contents = match std::fs::read_to_string(artifact_path) {
        Ok(c) => c,
        Err(e) => {
            result.verdict = Verdict::CannotTestify;
            result.verdict_note = Some(format!(
                "Artifact at {} unreadable: {}",
                artifact_path.display(),
                e
            ));
            result.signals = Some(serde_json::json!({
                "nq_sql_contract_state": {
                    "artifact_path": artifact_path.display().to_string(),
                    "io_error": e.to_string(),
                }
            }));
            return;
        }
    };

    // 2. Parse JSON.
    let receipt: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(e) => {
            result.verdict = Verdict::CannotTestify;
            result.verdict_note = Some(format!(
                "Artifact at {} is not valid JSON: {}",
                artifact_path.display(),
                e
            ));
            result.signals = Some(serde_json::json!({
                "nq_sql_contract_state": {
                    "artifact_path": artifact_path.display().to_string(),
                    "parse_error": e.to_string(),
                }
            }));
            return;
        }
    };

    // 3. Schema discrimination.
    let schema = receipt.get("schema").and_then(|v| v.as_str());
    match schema {
        None => {
            result.verdict = Verdict::InsufficientCoverage;
            result.verdict_note = Some(format!(
                "Artifact at {} has no `schema` field; cannot determine receipt kind.",
                artifact_path.display()
            ));
            result.signals = Some(signals_kind_only(artifact_path, None));
            return;
        }
        Some(s) if s != EXPECTED_RECEIPT_SCHEMA => {
            result.verdict = Verdict::InsufficientCoverage;
            result.verdict_note = Some(format!(
                "Artifact schema is {:?}, expected {:?}; receipt does not cover this claim kind.",
                s, EXPECTED_RECEIPT_SCHEMA,
            ));
            result.signals = Some(signals_kind_only(artifact_path, Some(s)));
            return;
        }
        Some(_) => {}
    }

    // 4. Pull required fields. Their absence is malformed-receipt =
    //    CannotTestify, distinct from schema-mismatch.
    let result_field = receipt.get("result").and_then(|v| v.as_str());
    let expected = string_array(&receipt, "expected_public_views");
    let observed = string_array(&receipt, "observed_public_views");
    let missing = string_array(&receipt, "missing_public_views");
    let unexpected = string_array(&receipt, "unexpected_public_views");
    let does_not_check = receipt
        .get("scope")
        .and_then(|v| v.get("does_not_check"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|e| e.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // 5. Verdict.
    match result_field {
        Some("pass") => {
            result.verdict = Verdict::AdmissibleWithScope;
            result.verdict_note = Some(format!(
                "Documented public SQL contract views ({}) all present in observed schema.",
                expected.len()
            ));
        }
        Some("fail") => {
            result.verdict = Verdict::UnsupportedAsStated;
            result.verdict_note = Some(format!(
                "Public SQL contract does not hold: {} view(s) missing: {:?}.",
                missing.len(),
                missing
            ));
        }
        Some(other) => {
            result.verdict = Verdict::CannotTestify;
            result.verdict_note = Some(format!(
                "Artifact `result` is {:?}; expected `pass` or `fail`.",
                other
            ));
        }
        None => {
            result.verdict = Verdict::CannotTestify;
            result.verdict_note = Some(
                "Artifact has no `result` field; malformed receipt.".to_string(),
            );
        }
    }

    result.signals = Some(serde_json::json!({
        "nq_sql_contract_state": {
            "artifact_path": artifact_path.display().to_string(),
            "schema": EXPECTED_RECEIPT_SCHEMA,
            "result": result_field,
            "expected_public_views": expected,
            "observed_public_views": observed,
            "missing_public_views": missing,
            "unexpected_public_views": unexpected,
            "contract_doc": receipt.get("contract_doc"),
            "producer": receipt.get("producer"),
            "scope_does_not_check": does_not_check,
        }
    }));
}

fn string_array(receipt: &serde_json::Value, key: &str) -> Vec<String> {
    receipt
        .get(key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|e| e.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn signals_kind_only(artifact_path: &Path, observed_schema: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "nq_sql_contract_state": {
            "artifact_path": artifact_path.display().to_string(),
            "expected_schema": EXPECTED_RECEIPT_SCHEMA,
            "observed_schema": observed_schema,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn target_for(path: &Path) -> NqSqlContractStateTarget {
        NqSqlContractStateTarget {
            host: "self".to_string(),
            artifact_path: path.to_path_buf(),
        }
    }

    fn write_artifact(dir: &tempfile::TempDir, name: &str, body: &str) -> PathBuf {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path
    }

    #[test]
    fn missing_artifact_yields_cannot_testify() {
        let target = NqSqlContractStateTarget {
            host: "self".to_string(),
            artifact_path: PathBuf::from("/nonexistent/path/to/receipt.json"),
        };
        let r = evaluate_nq_sql_contract_state_preflight(&target);
        assert_eq!(r.verdict, Verdict::CannotTestify);
        assert_eq!(r.claim_kind, ClaimKind::NqSqlContractState);
        assert!(!r.cannot_testify.is_empty(), "constitutional refusals must always populate");
    }

    #[test]
    fn malformed_json_yields_cannot_testify() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_artifact(&dir, "bad.json", "{ not valid json");
        let r = evaluate_nq_sql_contract_state_preflight(&target_for(&path));
        assert_eq!(r.verdict, Verdict::CannotTestify);
        assert!(r.verdict_note.as_ref().unwrap().contains("not valid JSON"));
    }

    #[test]
    fn missing_schema_field_yields_insufficient_coverage() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_artifact(&dir, "no_schema.json", r#"{"result": "pass"}"#);
        let r = evaluate_nq_sql_contract_state_preflight(&target_for(&path));
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(r.verdict_note.as_ref().unwrap().contains("no `schema` field"));
    }

    #[test]
    fn wrong_schema_yields_insufficient_coverage() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_artifact(
            &dir,
            "wrong_schema.json",
            r#"{"schema": "nq.preflight.disk_state.v1", "result": "pass"}"#,
        );
        let r = evaluate_nq_sql_contract_state_preflight(&target_for(&path));
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        let note = r.verdict_note.as_ref().unwrap();
        assert!(note.contains("nq.preflight.disk_state.v1"));
        assert!(note.contains("does not cover this claim kind"));
    }

    #[test]
    fn malformed_result_field_yields_cannot_testify() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_artifact(
            &dir,
            "bad_result.json",
            r#"{"schema": "nq.sql_contract.public_views.v1", "result": "maybe"}"#,
        );
        let r = evaluate_nq_sql_contract_state_preflight(&target_for(&path));
        assert_eq!(r.verdict, Verdict::CannotTestify);
        assert!(r.verdict_note.as_ref().unwrap().contains("\"maybe\""));
    }

    #[test]
    fn pass_receipt_yields_admissible_with_scope() {
        let dir = tempfile::tempdir().unwrap();
        let body = r#"{
            "schema": "nq.sql_contract.public_views.v1",
            "claim_kind": "nq_sql_public_contract_state",
            "producer": "nq-db::sql_contract::public_contract_views_exist_after_migration",
            "contract_doc": "docs/operator/sql-contract.md",
            "check_source": "Rust drift test inventory",
            "observed_source": "sqlite_master",
            "expected_public_views": ["v_hosts", "v_warnings"],
            "observed_public_views": ["v_hosts", "v_warnings"],
            "missing_public_views": [],
            "unexpected_public_views": ["v_log_observations"],
            "result": "pass",
            "scope": {
                "checks": ["public view existence"],
                "does_not_check": [
                    "column stability",
                    "operator-visible tables",
                    "internal derived views"
                ]
            }
        }"#;
        let path = write_artifact(&dir, "pass.json", body);
        let r = evaluate_nq_sql_contract_state_preflight(&target_for(&path));
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);

        // The negative scope must surface verbatim in signals — the wedge
        // against consumer inflation.
        let signals = r.signals.as_ref().expect("signals populated");
        let does_not_check = signals
            .get("nq_sql_contract_state")
            .and_then(|v| v.get("scope_does_not_check"))
            .and_then(|v| v.as_array())
            .expect("scope_does_not_check present");
        assert_eq!(does_not_check.len(), 3);
        assert!(does_not_check
            .iter()
            .any(|v| v.as_str() == Some("column stability")));
    }

    #[test]
    fn fail_receipt_yields_unsupported_as_stated() {
        let dir = tempfile::tempdir().unwrap();
        let body = r#"{
            "schema": "nq.sql_contract.public_views.v1",
            "claim_kind": "nq_sql_public_contract_state",
            "producer": "nq-db::sql_contract::public_contract_views_exist_after_migration",
            "contract_doc": "docs/operator/sql-contract.md",
            "check_source": "Rust drift test inventory",
            "observed_source": "sqlite_master",
            "expected_public_views": ["v_hosts", "v_warnings"],
            "observed_public_views": ["v_hosts"],
            "missing_public_views": ["v_warnings"],
            "unexpected_public_views": [],
            "result": "fail",
            "scope": {
                "checks": ["public view existence"],
                "does_not_check": ["column stability"]
            }
        }"#;
        let path = write_artifact(&dir, "fail.json", body);
        let r = evaluate_nq_sql_contract_state_preflight(&target_for(&path));
        assert_eq!(r.verdict, Verdict::UnsupportedAsStated);

        let note = r.verdict_note.as_ref().unwrap();
        assert!(note.contains("v_warnings"));

        let signals = r.signals.as_ref().expect("signals populated");
        let missing = signals
            .get("nq_sql_contract_state")
            .and_then(|v| v.get("missing_public_views"))
            .and_then(|v| v.as_array())
            .expect("missing_public_views present");
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].as_str(), Some("v_warnings"));
    }

    #[test]
    fn constitutional_refusals_always_populate() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_artifact(
            &dir,
            "any.json",
            r#"{"schema": "nq.sql_contract.public_views.v1", "result": "pass",
                "expected_public_views": [], "observed_public_views": [],
                "missing_public_views": [], "unexpected_public_views": [],
                "scope": {"checks": [], "does_not_check": []}}"#,
        );
        let r = evaluate_nq_sql_contract_state_preflight(&target_for(&path));

        // Constitutional refusals are the kind-level wedge — they MUST be
        // present regardless of receipt content or verdict.
        assert!(r.cannot_testify.len() >= 10);
        assert!(r.cannot_testify.iter().any(|s| s.contains("column")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("consequence")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("sixth-keeper")));
    }
}
