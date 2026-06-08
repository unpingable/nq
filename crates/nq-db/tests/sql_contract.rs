//! SQL contract drift detection.
//!
//! Asserts that every view named in `docs/operator/sql-contract.md` as
//! part of the public SQL contract exists in the migrated database.
//!
//! Fails on public-contract drift only: a missing public view breaks the
//! test; an extra undocumented view does not. Internal/derived views are
//! allowed to come and go without notice per the contract.
//!
//! If a public view is intentionally removed, update both this list and
//! `docs/operator/sql-contract.md`, and add a `FEATURE_HISTORY.md` entry
//! per the contract's "Adding to the contract" section.
//!
//! If a new view should be public, add it here AND to sql-contract.md.
//!
//! ## Receipt emission (NQ-on-NQ-001)
//!
//! When `NQ_EMIT_SQL_CONTRACT_RECEIPT=<path>` is set, the test writes a
//! `nq.sql_contract.public_views.v1` JSON receipt to that path. The
//! receipt is the artifact a future NQ-on-NQ consumer will ingest to
//! make an admissibility claim about NQ's own operator contract.
//!
//! The receipt is emitted on both pass and fail so a downstream
//! consumer can render `Contradicted` when drift is present. Test pass
//! / fail semantics are unchanged; the env var only adds an artifact.
//!
//! Negative scope is part of the receipt by design — it prevents
//! consumers from inflating "public view existence holds" into
//! "the operator SQL contract is fully satisfied."

use nq_db::{migrate, open_ro, open_rw};
use serde::Serialize;

/// Views the operator SQL contract promises will exist.
///
/// Must match the public-tier listings in `docs/operator/sql-contract.md`.
/// Domain-specific public views (SMART, ZFS) are listed too — they exist
/// in the schema regardless of whether their collector emits rows.
const PUBLIC_CONTRACT_VIEWS: &[&str] = &[
    // Public contract views
    "v_hosts",
    "v_services",
    "v_sqlite_dbs",
    "v_sources",
    "v_metrics",
    // Public, evolving
    "v_warnings",
    "v_host_state",
    "v_admissibility",
    // Public, domain-specific
    "v_smart_witness",
    "v_smart_devices",
    "v_zfs_witness",
    "v_zfs_pools",
];

#[derive(Serialize)]
struct Receipt {
    schema: &'static str,
    claim_kind: &'static str,
    producer: &'static str,
    contract_doc: &'static str,
    check_source: &'static str,
    observed_source: &'static str,
    expected_public_views: Vec<String>,
    observed_public_views: Vec<String>,
    missing_public_views: Vec<String>,
    unexpected_public_views: Vec<String>,
    result: &'static str,
    scope: Scope,
}

#[derive(Serialize)]
struct Scope {
    checks: Vec<&'static str>,
    does_not_check: Vec<&'static str>,
}

#[test]
fn public_contract_views_exist_after_migration() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("contract.db");

    {
        let mut wdb = open_rw(&db_path).unwrap();
        migrate(&mut wdb).unwrap();
    }

    let rdb = open_ro(&db_path).unwrap();
    let mut stmt = rdb
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type='view' ORDER BY name")
        .unwrap();
    let actual: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    let expected: Vec<String> = PUBLIC_CONTRACT_VIEWS.iter().map(|s| s.to_string()).collect();

    // Observed public-tier views: intersection of actual with expected.
    let observed_public: Vec<String> = expected
        .iter()
        .filter(|v| actual.contains(v))
        .cloned()
        .collect();

    // Missing public-tier views: expected minus actual. Drift if non-empty.
    let missing: Vec<String> = expected
        .iter()
        .filter(|v| !actual.contains(v))
        .cloned()
        .collect();

    // Unexpected views: actual minus expected. Informational — these may
    // be internal derived views per the contract; existence is allowed.
    let unexpected: Vec<String> = actual
        .iter()
        .filter(|v| !expected.contains(v))
        .cloned()
        .collect();

    let result = if missing.is_empty() { "pass" } else { "fail" };

    // Emit receipt before asserting so a failed drift run still writes
    // the artifact a consumer needs to render `Contradicted`.
    if let Ok(receipt_path) = std::env::var("NQ_EMIT_SQL_CONTRACT_RECEIPT") {
        let receipt = Receipt {
            schema: "nq.sql_contract.public_views.v1",
            claim_kind: "nq_sql_public_contract_state",
            producer: "nq-db::sql_contract::public_contract_views_exist_after_migration",
            contract_doc: "docs/operator/sql-contract.md",
            check_source: "Rust drift test inventory",
            observed_source: "sqlite_master",
            expected_public_views: expected.clone(),
            observed_public_views: observed_public,
            missing_public_views: missing.clone(),
            unexpected_public_views: unexpected,
            result,
            scope: Scope {
                checks: vec!["public view existence"],
                does_not_check: vec![
                    "column stability",
                    "operator-visible tables",
                    "internal derived views",
                    "semantic query compatibility",
                    "performance",
                    "migration history correctness",
                ],
            },
        };
        let path = std::path::PathBuf::from(&receipt_path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).expect("create receipt parent dir");
            }
        }
        let json = serde_json::to_string_pretty(&receipt).expect("serialize receipt");
        std::fs::write(&path, json).expect("write receipt");
    }

    assert!(
        missing.is_empty(),
        "SQL contract drift: public views missing from migrated schema: {:?}.\n\
         Actual views present: {:?}.\n\
         Either restore the view, or remove it from PUBLIC_CONTRACT_VIEWS \
         AND docs/operator/sql-contract.md AND add a FEATURE_HISTORY entry.",
        missing,
        actual,
    );
}
