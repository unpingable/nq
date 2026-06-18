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

// ---------------------------------------------------------------------------
// Column-stability contract (NQ-on-NQ-001, completeness layer)
//
// The test above checks that public views *exist*. This one checks that the
// columns operators, dashboards, and exporters depend on are still present.
//
// Contract semantics (docs/operator/sql-contract.md §1 "Public contract
// views"): additive columns are non-breaking; removals, renames, and
// semantic reversals are breaking and must be announced in FEATURE_HISTORY.
// A rename or removal manifests here as a *missing expected column* — so the
// drift rule mirrors the view-existence test exactly:
//
//   missing expected column  -> fail (breaking drift)
//   extra observed column    -> allowed, informational (additive)
//
// This applies uniformly to stable and "public, evolving" views: the
// stable/evolving distinction governs whether a *removal* must be announced,
// not the pass/fail of an existence check. Column *type* stability, ordering,
// and semantics remain out of scope (see `does_not_check`).
// ---------------------------------------------------------------------------

/// Columns the operator SQL contract promises each public view will expose.
///
/// Baselined from the migrated schema (the same `migrate()` path the test
/// exercises), not from any single migration file — views are redefined
/// across migrations, so the migrated end-state is the source of truth.
///
/// Order is not significant: the check is set-based, matching the contract's
/// silence on column ordering. Adding a column to a view is non-breaking and
/// does NOT require updating this list (it surfaces as an allowed extra).
/// Removing or renaming one DOES require updating this list AND
/// `docs/operator/sql-contract.md` AND a `FEATURE_HISTORY.md` entry.
const PUBLIC_VIEW_COLUMNS: &[(&str, &[&str])] = &[
    (
        "v_hosts",
        &[
            "host", "cpu_load_1m", "cpu_load_5m", "mem_total_mb", "mem_available_mb",
            "mem_pressure_pct", "disk_total_mb", "disk_avail_mb", "disk_used_pct",
            "uptime_seconds", "kernel_version", "as_of_generation", "collected_at",
            "current_generation", "generations_behind", "age_s", "is_stale",
        ],
    ),
    (
        "v_services",
        &[
            "host", "service", "status", "pid", "uptime_seconds", "eps", "queue_depth",
            "consumer_lag", "drop_count", "as_of_generation", "collected_at",
            "current_generation", "generations_behind", "age_s", "is_stale",
        ],
    ),
    (
        "v_sqlite_dbs",
        &[
            "host", "db_path", "db_size_mb", "wal_size_mb", "page_size", "page_count",
            "freelist_count", "freelist_reclaimable_mb", "wal_pct", "freelist_pct",
            "journal_mode", "checkpoint_lag_s", "last_quick_check", "as_of_generation",
            "collected_at", "current_generation", "generations_behind", "age_s", "is_stale",
        ],
    ),
    (
        "v_sources",
        &[
            "source", "last_status", "last_received_at", "last_collected_at",
            "last_duration_ms", "last_error", "current_generation", "last_generation",
            "generations_behind",
        ],
    ),
    (
        "v_metrics",
        &[
            "host", "metric_name", "labels_json", "value", "metric_type",
            "scrape_target_name", "scrape_target_url", "scrape_target_collision",
            "as_of_generation", "collected_at", "series_id", "current_generation",
            "generations_behind", "age_s", "is_stale",
        ],
    ),
    (
        "v_warnings",
        &[
            "severity", "host", "kind", "subject", "message", "domain", "first_seen_at",
            "consecutive_gens", "acknowledged", "peak_value", "first_seen_gen",
            "last_seen_gen", "last_seen_at", "acknowledged_at", "work_state", "owner",
            "note", "external_ref", "work_state_at", "finding_class", "visibility_state",
            "suppression_reason", "suppressed_since_gen", "failure_class", "service_impact",
            "action_bias", "synopsis", "why_care", "stability", "basis_state",
            "basis_source_id", "basis_witness_id", "last_basis_generation", "basis_state_at",
            "state_kind", "degradation_kind", "degradation_metric", "degradation_value",
            "degradation_threshold", "recovery_state", "recovery_metric", "recovery_comparator",
            "recovery_threshold", "recovery_sustained_for_s", "recovery_evidence_since",
            "recovery_satisfied_at", "coverage_degraded_ref", "node_type", "cause_candidate",
            "evidence_finding_key", "suppressed_descendant_count", "suppression_kind",
            "suppression_declaration_id", "maintenance_state", "maintenance_id", "origin_source",
            "origin_producer_id", "origin_extraction_run_id", "origin_producer_extraction_time",
            "origin_import_contract_version", "origin_mode", "silence_scope", "silence_basis",
            "silence_duration_s", "silence_expected",
        ],
    ),
    (
        "v_host_state",
        &[
            "host", "dominant_kind", "dominant_subject", "dominant_severity",
            "dominant_failure_class", "dominant_service_impact", "dominant_action_bias",
            "dominant_stability", "dominant_synopsis", "dominant_consecutive_gens",
            "total_findings", "observed_findings", "suppressed_findings",
            "immediate_risk_count", "degraded_count", "flickering_count", "subordinate_count",
            "pressure_degraded_count", "accumulation_count",
        ],
    ),
    (
        "v_admissibility",
        &[
            "host", "kind", "subject", "admissibility", "suppression_kind", "ancestor_reason",
            "suppression_declaration_id", "suppressed_since_gen", "visibility_state", "severity",
            "finding_class", "last_seen_at", "last_seen_gen",
        ],
    ),
    (
        "v_smart_witness",
        &[
            "host", "witness_id", "witness_type", "witness_host", "observed_subject",
            "profile_version", "collection_mode", "privilege_model", "witness_status",
            "witness_collected_at", "duration_ms", "as_of_generation", "received_at",
            "received_age_s", "witness_age_s",
        ],
    ),
    (
        "v_smart_devices",
        &[
            "host", "subject", "device_path", "device_class", "protocol", "collection_outcome",
            "model", "serial_number", "firmware_version", "capacity_bytes", "logical_block_size",
            "smart_available", "smart_enabled", "smart_overall_passed", "temperature_c",
            "power_on_hours", "uncorrected_read_errors", "uncorrected_write_errors",
            "uncorrected_verify_errors", "media_errors", "nvme_percentage_used",
            "nvme_available_spare_pct", "nvme_critical_warning", "nvme_unsafe_shutdowns",
            "raw_truncated", "as_of_generation", "collected_at", "witness_status",
            "witness_collected_at", "received_age_s",
        ],
    ),
    (
        "v_zfs_witness",
        &[
            "host", "witness_id", "witness_type", "witness_host", "observed_subject",
            "profile_version", "collection_mode", "privilege_model", "witness_status",
            "witness_collected_at", "duration_ms", "as_of_generation", "received_at",
            "received_age_s", "witness_age_s",
        ],
    ),
    (
        "v_zfs_pools",
        &[
            "host", "pool", "state", "health_numeric", "size_bytes", "alloc_bytes",
            "free_bytes", "readonly", "fragmentation_ratio", "as_of_generation",
            "collected_at", "witness_status", "witness_collected_at",
        ],
    ),
];

/// Set-based column diff. Returns `(missing, extra)`:
/// `missing` = expected columns absent from `actual` (breaking drift);
/// `extra`   = observed columns not in `expected` (allowed additive).
fn diff_columns(expected: &[&str], actual: &[String]) -> (Vec<String>, Vec<String>) {
    let missing: Vec<String> = expected
        .iter()
        .filter(|c| !actual.iter().any(|a| a == **c))
        .map(|c| c.to_string())
        .collect();
    let extra: Vec<String> = actual
        .iter()
        .filter(|a| !expected.iter().any(|c| c == a))
        .cloned()
        .collect();
    (missing, extra)
}

#[derive(Serialize)]
struct ColumnReceipt {
    schema: &'static str,
    claim_kind: &'static str,
    producer: &'static str,
    contract_doc: &'static str,
    check_source: &'static str,
    observed_source: &'static str,
    views: Vec<ViewColumnReport>,
    missing_total: usize,
    result: &'static str,
    scope: Scope,
}

#[derive(Serialize)]
struct ViewColumnReport {
    view: &'static str,
    expected_columns: Vec<String>,
    observed_columns: Vec<String>,
    missing_columns: Vec<String>,
    unexpected_columns: Vec<String>,
}

#[test]
fn public_contract_view_columns_stable_after_migration() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("contract.db");

    {
        let mut wdb = open_rw(&db_path).unwrap();
        migrate(&mut wdb).unwrap();
    }

    let rdb = open_ro(&db_path).unwrap();

    let mut reports: Vec<ViewColumnReport> = Vec::new();
    let mut all_missing: Vec<(&str, Vec<String>)> = Vec::new();

    for (view, expected_cols) in PUBLIC_VIEW_COLUMNS {
        let mut stmt = rdb
            .conn()
            .prepare("SELECT name FROM pragma_table_info(?1)")
            .unwrap();
        let actual: Vec<String> = stmt
            .query_map([view], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        let (missing, extra) = diff_columns(expected_cols, &actual);
        if !missing.is_empty() {
            all_missing.push((view, missing.clone()));
        }

        reports.push(ViewColumnReport {
            view,
            expected_columns: expected_cols.iter().map(|s| s.to_string()).collect(),
            observed_columns: actual,
            missing_columns: missing,
            unexpected_columns: extra,
        });
    }

    let missing_total: usize = all_missing.iter().map(|(_, m)| m.len()).sum();
    let result = if missing_total == 0 { "pass" } else { "fail" };

    // Emit a sibling receipt before asserting, so a failed drift run still
    // produces the artifact. Distinct schema from the view-existence receipt
    // (`nq.sql_contract.public_views.v1`) so the NQ-on-NQ-002 consumer in
    // nq-monitor (keyed strictly on that schema) is unaffected.
    if let Ok(receipt_path) = std::env::var("NQ_EMIT_SQL_COLUMN_CONTRACT_RECEIPT") {
        let receipt = ColumnReceipt {
            schema: "nq.sql_contract.public_columns.v1",
            claim_kind: "nq_sql_public_column_contract_state",
            producer:
                "nq-db::sql_contract::public_contract_view_columns_stable_after_migration",
            contract_doc: "docs/operator/sql-contract.md",
            check_source: "Rust drift test inventory",
            observed_source: "pragma_table_info",
            views: reports,
            missing_total,
            result,
            scope: Scope {
                checks: vec!["public view column existence"],
                does_not_check: vec![
                    "column type stability",
                    "column ordering",
                    "semantic query compatibility",
                    "performance",
                    "migration history correctness",
                    "operator-visible storage tables",
                    "internal derived views",
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
        all_missing.is_empty(),
        "SQL column contract drift: public view(s) missing promised columns: {all_missing:?}.\n\
         A removal or rename is breaking. Either restore the column, or update \
         PUBLIC_VIEW_COLUMNS AND docs/operator/sql-contract.md AND add a \
         FEATURE_HISTORY entry per the contract's \"Adding to the contract\" section.",
    );
}

#[test]
fn diff_columns_detects_missing_and_allows_extra() {
    // Negative coverage for the drift logic itself, independent of the live
    // schema: a removed/renamed column is reported missing; an added column is
    // reported as an allowed extra, not a failure.
    let expected = &["host", "status", "renamed_away"];
    let actual = vec![
        "host".to_string(),
        "status".to_string(),
        "newly_added".to_string(),
    ];

    let (missing, extra) = diff_columns(expected, &actual);

    assert_eq!(missing, vec!["renamed_away".to_string()], "removal must drift");
    assert_eq!(extra, vec!["newly_added".to_string()], "addition is allowed");

    // Exact-match case: no drift, no extras.
    let (none_missing, none_extra) =
        diff_columns(&["a", "b"], &["a".to_string(), "b".to_string()]);
    assert!(none_missing.is_empty() && none_extra.is_empty());
}
