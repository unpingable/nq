//! ORIGIN_MODE_DISCRIMINATOR (migration 057) acceptance tests.
//!
//! Forcing case: AG-side provenance audit closed 2026-06-09 with
//! recommendation D — NQ has no closed-vocabulary discriminator at
//! finding mint distinguishing drilled / fault-injected / replayed /
//! synthetic findings from authentic observations. The fix lands here.
//!
//! These tests prove the four-row distinctness invariant the operator
//! named: drilled / imported-real / replayed / observed remain
//! distinguishable in DB rows AND in the public `FindingSnapshot` DTO.
//!
//! See `~/git/agent_gov/working/nq-custody-gap-origin-discriminator.md`.

use nq_db::{
    export_findings_from_conn, ingest_finding_import, migrate, open_rw, ExportFilter, IngestConfig,
};

fn test_db() -> nq_db::WriteDb {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.keep().join("test.db");
    let mut db = open_rw(&db_path).unwrap();
    migrate(&mut db).unwrap();
    db.conn()
        .execute(
            "INSERT INTO generations (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES (1, ?1, ?1, 'complete', 1, 1, 0, 10)",
            rusqlite::params!["2026-06-09T10:00:00Z"],
        )
        .unwrap();
    db
}

fn insert_native_observed_finding(db: &nq_db::WriteDb, host: &str, kind: &str, subject: &str) {
    db.conn()
        .execute(
            "INSERT INTO warning_state (
                host, kind, subject, domain, message, severity,
                first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                consecutive_gens, finding_class, absent_gens,
                visibility_state, basis_state, state_kind,
                failure_class, service_impact, action_bias, synopsis, why_care
             ) VALUES (?1, ?2, ?3, 'Δg', 'native observation', 'warning',
                      1, '2026-06-09T10:00:00Z', 1, '2026-06-09T10:00:00Z',
                      1, 'signal', 0,
                      'observed', 'unknown', 'incident',
                      'Accumulation', 'NoneCurrent', 'InvestigateBusinessHours',
                      'native test', 'native test')",
            rusqlite::params![host, kind, subject],
        )
        .unwrap();
}

fn import_manifest_with_origin_mode(
    origin_mode: Option<&str>,
    host: &str,
    subject: &str,
) -> String {
    let mode_field = match origin_mode {
        Some(m) => format!(", \"origin_mode\": \"{}\"", m),
        None => String::new(),
    };
    format!(
        r#"{{
            "schema": "nq.finding_import.v1",
            "contract_version": 1,
            "producer_id": "test-producer",
            "extraction_run_id": "run-test-{}",
            "producer_extraction_time": "2026-06-09T09:59:30Z",
            "findings": [
                {{
                    "identity": {{
                        "host": "{}",
                        "detector": "wal_bloat",
                        "subject": "{}",
                        "rule_hash": null
                    }},
                    "severity": "warning",
                    "message": "test import"
                    {}
                }}
            ]
        }}"#,
        host, host, subject, mode_field
    )
}

// ---------------------------------------------------------------------------
// CHECK constraint — the SQL closed vocabulary.
// ---------------------------------------------------------------------------

#[test]
fn origin_mode_defaults_to_observed_for_native_findings() {
    let db = test_db();
    insert_native_observed_finding(&db, "host-1", "wal_bloat", "/db");

    let mode: String = db
        .conn()
        .query_row(
            "SELECT origin_mode FROM warning_state WHERE host = 'host-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        mode, "observed",
        "native findings default to origin_mode='observed' (backward compat)"
    );
}

#[test]
fn origin_mode_check_constraint_rejects_unknown_values() {
    let db = test_db();
    // Direct insert bypassing the import path — should fail at the CHECK.
    let res = db.conn().execute(
        "INSERT INTO warning_state (
            host, kind, subject, domain, message, severity,
            first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
            consecutive_gens, finding_class, absent_gens,
            visibility_state, basis_state, state_kind, origin_mode
         ) VALUES ('h', 'k', 's', '', 'm', 'warning',
                   1, '2026-06-09T10:00:00Z', 1, '2026-06-09T10:00:00Z',
                   1, 'signal', 0,
                   'observed', 'unknown', 'incident', 'exercise')",
        [],
    );
    assert!(
        res.is_err(),
        "origin_mode='exercise' must be refused by the CHECK constraint"
    );
}

#[test]
fn origin_mode_accepts_all_four_closed_vocabulary_values() {
    let db = test_db();
    for (i, mode) in ["observed", "drill", "replay", "synthetic"].iter().enumerate() {
        db.conn()
            .execute(
                "INSERT INTO warning_state (
                    host, kind, subject, domain, message, severity,
                    first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                    consecutive_gens, finding_class, absent_gens,
                    visibility_state, basis_state, state_kind, origin_mode
                 ) VALUES (?1, 'wal_bloat', ?2, '', 'm', 'warning',
                           1, '2026-06-09T10:00:00Z', 1, '2026-06-09T10:00:00Z',
                           1, 'signal', 0,
                           'observed', 'unknown', 'incident', ?3)",
                rusqlite::params![format!("host-{}", i), format!("/db/{}", i), mode],
            )
            .unwrap();
    }
}

// ---------------------------------------------------------------------------
// Import path — the forcing site.
// ---------------------------------------------------------------------------

#[test]
fn imported_finding_without_origin_mode_defaults_to_observed() {
    let db = test_db();
    let manifest = import_manifest_with_origin_mode(None, "host-1", "/db");
    let result = ingest_finding_import(
        db.conn(),
        &manifest,
        1,
        "2026-06-09T10:00:00Z",
        &IngestConfig::default(),
    )
    .unwrap();
    assert_eq!(result.ingested_count, 1);
    assert!(!result.refused);

    let mode: String = db
        .conn()
        .query_row(
            "SELECT origin_mode FROM warning_state WHERE host = 'host-1' AND kind = 'wal_bloat'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        mode, "observed",
        "imports without origin_mode default to 'observed' (V1 fixture compat)"
    );
}

#[test]
fn imported_drill_finding_stores_drill_origin_mode() {
    let db = test_db();
    let manifest = import_manifest_with_origin_mode(Some("drill"), "host-drill", "/db/drill");
    let result = ingest_finding_import(
        db.conn(),
        &manifest,
        1,
        "2026-06-09T10:00:00Z",
        &IngestConfig::default(),
    )
    .unwrap();
    assert_eq!(result.ingested_count, 1);
    assert!(!result.refused);

    let mode: String = db
        .conn()
        .query_row(
            "SELECT origin_mode FROM warning_state WHERE host = 'host-drill'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mode, "drill", "drill harness imports must store 'drill'");
}

#[test]
fn imported_replay_finding_stores_replay_origin_mode() {
    let db = test_db();
    let manifest = import_manifest_with_origin_mode(Some("replay"), "host-replay", "/db");
    let result = ingest_finding_import(
        db.conn(),
        &manifest,
        1,
        "2026-06-09T10:00:00Z",
        &IngestConfig::default(),
    )
    .unwrap();
    assert_eq!(result.ingested_count, 1);

    let mode: String = db
        .conn()
        .query_row(
            "SELECT origin_mode FROM warning_state WHERE host = 'host-replay'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mode, "replay");
}

#[test]
fn imported_synthetic_finding_stores_synthetic_origin_mode() {
    let db = test_db();
    let manifest = import_manifest_with_origin_mode(Some("synthetic"), "host-syn", "/db");
    let result = ingest_finding_import(
        db.conn(),
        &manifest,
        1,
        "2026-06-09T10:00:00Z",
        &IngestConfig::default(),
    )
    .unwrap();
    assert_eq!(result.ingested_count, 1);

    let mode: String = db
        .conn()
        .query_row(
            "SELECT origin_mode FROM warning_state WHERE host = 'host-syn'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mode, "synthetic");
}

#[test]
fn imported_finding_with_unknown_origin_mode_refused_with_one_finding() {
    let db = test_db();
    let manifest = import_manifest_with_origin_mode(Some("exercise"), "host-bad", "/db");
    let result = ingest_finding_import(
        db.conn(),
        &manifest,
        1,
        "2026-06-09T10:00:00Z",
        &IngestConfig::default(),
    )
    .unwrap();
    assert_eq!(
        result.ingested_count, 0,
        "no findings ingest when origin_mode is unknown"
    );
    assert!(result.refused, "manifest with unknown origin_mode is refused");
    assert!(result
        .refusal_reason
        .as_ref()
        .unwrap()
        .contains("origin_mode `exercise`"));

    let refusal_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM warning_state WHERE kind = 'inbound_export_unparsable'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(refusal_count, 1, "exactly one refusal finding emitted");

    let ingested_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM warning_state WHERE host = 'host-bad'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        ingested_count, 0,
        "the under-the-bad-mode finding is NOT ingested"
    );
}

// ---------------------------------------------------------------------------
// The custody-bridge invariant — distinguishability across mint paths.
// ---------------------------------------------------------------------------

#[test]
fn drilled_and_imported_real_findings_are_not_byte_identical() {
    // This is the load-bearing invariant from the operator: "no
    // byte-identical drill/import rows to observed producer rows."
    // We construct one drilled import and one real-observed import,
    // matching every other manifest field, and assert the stored rows
    // disagree on origin_mode.
    let db = test_db();
    let drill = import_manifest_with_origin_mode(Some("drill"), "host-drill", "/db");
    let real = import_manifest_with_origin_mode(Some("observed"), "host-real", "/db");
    ingest_finding_import(
        db.conn(),
        &drill,
        1,
        "2026-06-09T10:00:00Z",
        &IngestConfig::default(),
    )
    .unwrap();
    ingest_finding_import(
        db.conn(),
        &real,
        1,
        "2026-06-09T10:00:00Z",
        &IngestConfig::default(),
    )
    .unwrap();

    let drill_mode: String = db
        .conn()
        .query_row(
            "SELECT origin_mode FROM warning_state WHERE host = 'host-drill'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let real_mode: String = db
        .conn()
        .query_row(
            "SELECT origin_mode FROM warning_state WHERE host = 'host-real'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(drill_mode, "drill");
    assert_eq!(real_mode, "observed");
    assert_ne!(
        drill_mode, real_mode,
        "drilled and observed imports MUST be distinguishable in the DB"
    );
}

// ---------------------------------------------------------------------------
// Export wire DTO — the bridge surface.
// ---------------------------------------------------------------------------

#[test]
fn export_dto_carries_origin_mode_for_native_findings() {
    let db = test_db();
    insert_native_observed_finding(&db, "host-1", "wal_bloat", "/db");

    let snapshots = export_findings_from_conn(
        db.conn(),
        &ExportFilter {
            observations_limit: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        snapshots[0].origin_mode, "observed",
        "native finding wire DTO carries origin_mode='observed'"
    );
}

#[test]
fn export_dto_carries_origin_mode_for_drilled_imports() {
    let db = test_db();
    let manifest = import_manifest_with_origin_mode(Some("drill"), "host-drill", "/db");
    ingest_finding_import(
        db.conn(),
        &manifest,
        1,
        "2026-06-09T10:00:00Z",
        &IngestConfig::default(),
    )
    .unwrap();

    let snapshots = export_findings_from_conn(
        db.conn(),
        &ExportFilter {
            observations_limit: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(snapshots.len(), 1);
    let snap = &snapshots[0];
    assert_eq!(snap.origin_mode, "drill");
    // The DURABLE_ARTIFACT_SUBSTRATE origin block is still present
    // (this is an import row); origin_mode is on a different axis and
    // co-exists.
    assert!(snap.origin.is_some());
}

#[test]
fn export_dto_distinguishes_all_four_origin_modes_simultaneously() {
    // The load-bearing wire-shape distinguishability test. One DB,
    // four findings — native observation, imported drill, imported
    // replay, imported synthetic. All four ride the same exporter
    // path and arrive at the consumer with distinct origin_mode values.
    let db = test_db();
    insert_native_observed_finding(&db, "host-observed", "wal_bloat", "/db");
    for (host, mode) in [
        ("host-drill", "drill"),
        ("host-replay", "replay"),
        ("host-syn", "synthetic"),
    ] {
        let manifest = import_manifest_with_origin_mode(Some(mode), host, "/db");
        ingest_finding_import(
            db.conn(),
            &manifest,
            1,
            "2026-06-09T10:00:00Z",
            &IngestConfig::default(),
        )
        .unwrap();
    }

    let snapshots = export_findings_from_conn(
        db.conn(),
        &ExportFilter {
            observations_limit: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(snapshots.len(), 4);

    let modes: std::collections::HashMap<String, String> = snapshots
        .iter()
        .map(|s| (s.identity.host.clone(), s.origin_mode.clone()))
        .collect();
    assert_eq!(modes.get("host-observed"), Some(&"observed".to_string()));
    assert_eq!(modes.get("host-drill"), Some(&"drill".to_string()));
    assert_eq!(modes.get("host-replay"), Some(&"replay".to_string()));
    assert_eq!(modes.get("host-syn"), Some(&"synthetic".to_string()));
}

#[test]
fn json_roundtrip_preserves_origin_mode() {
    let db = test_db();
    let manifest = import_manifest_with_origin_mode(Some("drill"), "host-drill", "/db");
    ingest_finding_import(
        db.conn(),
        &manifest,
        1,
        "2026-06-09T10:00:00Z",
        &IngestConfig::default(),
    )
    .unwrap();

    let snapshots = export_findings_from_conn(
        db.conn(),
        &ExportFilter {
            observations_limit: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let json = serde_json::to_string(&snapshots[0]).unwrap();
    let back: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back["origin_mode"].as_str().unwrap(),
        "drill",
        "origin_mode survives serde roundtrip — consumers see the discriminator"
    );
}
