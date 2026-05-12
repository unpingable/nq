//! DURABLE_ARTIFACT_SUBSTRATE_GAP V1 acceptance tests.
//!
//! Covers the synthetic-producer slice:
//!
//! - Clean round-trip: fixture → ingest → export → origin envelope preserved
//!   on the wire, both clocks present.
//! - Refusal: under-versioned / wrong-schema / unparseable fixtures emit one
//!   `inbound_export_unparsable` finding, ingest zero findings.
//! - SILENCE_UNIFICATION composition: `extraction_stale` emits with the
//!   shared envelope (`scope`, `basis`, `duration_s`, `expected`) when the
//!   producer's extraction time exceeds the configured threshold.
//! - Legacy-detector tolerance: a legacy silence-shaped row carries no
//!   `silence` block on export (the "not yet unified" semantics — consumers
//!   must read absence as deferred migration, not "not silence").
//! - Inversion: every emitted shape allows downstream deny / defer /
//!   revalidate / admit without NQ encoding the verdict.

use nq_db::{
    export_findings_from_conn, ingest_finding_import, migrate, open_rw, ExportFilter, IngestConfig,
};

const FIXTURE_CLEAN: &str = include_str!("fixtures/synthetic_producer_import.json");
const FIXTURE_UNDER_VERSIONED: &str = include_str!("fixtures/synthetic_producer_under_versioned.json");
const FIXTURE_WRONG_SCHEMA: &str = include_str!("fixtures/synthetic_producer_wrong_schema.json");
const FIXTURE_STALE: &str = include_str!("fixtures/synthetic_producer_stale.json");

fn test_db() -> nq_db::WriteDb {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.into_path().join("test.db");
    let mut db = open_rw(&db_path).unwrap();
    migrate(&mut db).unwrap();
    // Seed generation 1 so export's current_generation lookup has a row.
    db.conn()
        .execute(
            "INSERT INTO generations (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES (1, ?1, ?1, 'complete', 1, 1, 0, 10)",
            rusqlite::params!["2026-05-12T10:00:30Z"],
        )
        .unwrap();
    db
}

#[test]
fn clean_ingest_round_trip_preserves_origin_envelope() {
    let db = test_db();
    let result = ingest_finding_import(
        db.conn(),
        FIXTURE_CLEAN,
        1,
        "2026-05-12T10:00:30Z",
        &IngestConfig::default(),
    )
    .unwrap();

    assert_eq!(result.ingested_count, 2, "fixture has two findings");
    assert!(!result.refused);
    assert!(
        !result.extraction_stale_emitted,
        "producer_extraction_time is fresh — extraction_stale must not fire"
    );

    // Export and verify origin envelope round-trips through the wire shape.
    let filter = ExportFilter {
        observations_limit: 0,
        ..Default::default()
    };
    let snapshots = export_findings_from_conn(db.conn(), &filter).unwrap();
    assert_eq!(snapshots.len(), 2, "both findings exported");
    for snap in &snapshots {
        let origin = snap
            .origin
            .as_ref()
            .expect("ingested findings carry origin envelope");
        assert_eq!(origin.source, "import");
        assert_eq!(origin.producer_id, "synthetic-corpus-extractor");
        assert_eq!(origin.extraction_run_id, "run-20260512T100000Z-abc123");
        assert_eq!(origin.producer_extraction_time, "2026-05-12T10:00:00Z");
        assert_eq!(origin.import_contract_version, 1);

        // Two-clock invariant: producer clock is the manifest header time;
        // NQ clock is the ingest "now". Both visible separately.
        assert_ne!(
            snap.lifecycle.first_seen_at, origin.producer_extraction_time,
            "lifecycle.first_seen_at is NQ ingest time, NOT producer extraction time"
        );
        assert_eq!(
            snap.lifecycle.first_seen_at, "2026-05-12T10:00:30Z",
            "lifecycle.first_seen_at grounds in NQ clock"
        );

        // Ingested findings carry no silence envelope (only `extraction_stale`
        // populates it in V1, and only when the producer is old).
        assert!(
            snap.silence.is_none(),
            "ingested findings have no silence envelope unless they're extraction_stale"
        );
    }
}

#[test]
fn under_versioned_fixture_refused_with_one_finding() {
    let db = test_db();
    let result = ingest_finding_import(
        db.conn(),
        FIXTURE_UNDER_VERSIONED,
        1,
        "2026-05-12T10:00:30Z",
        &IngestConfig::default(),
    )
    .unwrap();

    assert_eq!(result.ingested_count, 0, "no findings ingest when refused");
    assert!(result.refused);
    assert!(
        result
            .refusal_reason
            .as_ref()
            .unwrap()
            .contains("contract_version is 99"),
        "refusal reason names the contract mismatch"
    );

    // Exactly one inbound_export_unparsable finding present.
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM warning_state WHERE kind = 'inbound_export_unparsable'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "exactly one refusal finding");

    // Refusal finding is NQ-internal (origin=nq, no origin envelope on export).
    let origin_source: String = db
        .conn()
        .query_row(
            "SELECT origin_source FROM warning_state WHERE kind = 'inbound_export_unparsable'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(origin_source, "nq");

    // No imported findings.
    let imported: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM warning_state WHERE origin_source = 'import'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(imported, 0);
}

#[test]
fn wrong_schema_fixture_refused() {
    let db = test_db();
    let result = ingest_finding_import(
        db.conn(),
        FIXTURE_WRONG_SCHEMA,
        1,
        "2026-05-12T10:00:30Z",
        &IngestConfig::default(),
    )
    .unwrap();

    assert!(result.refused);
    assert_eq!(result.ingested_count, 0);
    assert!(result
        .refusal_reason
        .as_ref()
        .unwrap()
        .contains("nq.finding_snapshot.v1"));
}

#[test]
fn unparseable_json_refused_with_placeholder_identity() {
    let db = test_db();
    let result = ingest_finding_import(
        db.conn(),
        "{ this is not valid json",
        1,
        "2026-05-12T10:00:30Z",
        &IngestConfig::default(),
    )
    .unwrap();

    assert!(result.refused);
    assert_eq!(result.ingested_count, 0);

    let (host, subject): (String, String) = db
        .conn()
        .query_row(
            "SELECT host, subject FROM warning_state WHERE kind = 'inbound_export_unparsable'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(host, "unknown-producer");
    assert_eq!(subject, "unparseable");
}

#[test]
fn extraction_stale_fires_when_producer_is_old() {
    let db = test_db();
    let result = ingest_finding_import(
        db.conn(),
        FIXTURE_STALE,
        1,
        "2026-05-12T10:00:30Z",
        &IngestConfig::default(),
    )
    .unwrap();

    assert_eq!(result.ingested_count, 1);
    assert!(!result.refused);
    assert!(
        result.extraction_stale_emitted,
        "producer_extraction_time was 2026-01-01, threshold 86400s — must fire"
    );

    // The extraction_stale finding carries the SILENCE_UNIFICATION envelope.
    let snapshots = export_findings_from_conn(
        db.conn(),
        &ExportFilter {
            detector: Some("extraction_stale".to_string()),
            observations_limit: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(snapshots.len(), 1);
    let snap = &snapshots[0];

    let silence = snap
        .silence
        .as_ref()
        .expect("extraction_stale must carry silence envelope");
    assert_eq!(silence.scope, "extraction");
    assert_eq!(silence.basis, "age_threshold");
    assert!(
        silence.duration_s > 10_000_000,
        "duration is producer-clock delta (~131 days ≈ 11.3M seconds); got {}",
        silence.duration_s
    );
    assert_eq!(silence.expected, "none");

    // extraction_stale is NQ's testimony about the producer — origin_source=nq.
    assert!(
        snap.origin.is_none(),
        "extraction_stale is NQ's own finding; no origin envelope"
    );

    // Subject identifies the extraction run that triggered the silence.
    assert_eq!(snap.identity.host, "stale-corpus-extractor");
    assert_eq!(snap.identity.subject, "run-20260101T000000Z-old");
}

#[test]
fn extraction_stale_does_not_fire_when_producer_is_fresh() {
    let db = test_db();
    let result = ingest_finding_import(
        db.conn(),
        FIXTURE_CLEAN,
        1,
        "2026-05-12T10:00:30Z",
        &IngestConfig::default(),
    )
    .unwrap();

    assert!(!result.extraction_stale_emitted);

    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM warning_state WHERE kind = 'extraction_stale'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn legacy_silence_detector_has_no_silence_envelope_on_export() {
    // Insert a `stale_host`-shaped row directly. The migration leaves
    // silence_* columns NULL by default; the export must not synthesize
    // a silence envelope for a row that didn't populate the fields.
    // This is the "not yet unified" semantics — consumers must read
    // missing `silence` as deferred migration, not "not silence".
    let db = test_db();
    db.conn()
        .execute(
            "INSERT INTO warning_state (
                host, kind, subject, domain, message, severity,
                first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                consecutive_gens, finding_class, absent_gens,
                visibility_state, basis_state, state_kind
             ) VALUES ('host-1', 'stale_host', '', '', 'host silent for 3 gens', 'warning',
                       1, '2026-05-12T10:00:30Z', 1, '2026-05-12T10:00:30Z',
                       1, 'signal', 0, 'observed', 'unknown', 'incident')",
            [],
        )
        .unwrap();

    let snapshots = export_findings_from_conn(
        db.conn(),
        &ExportFilter {
            detector: Some("stale_host".to_string()),
            observations_limit: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(snapshots.len(), 1);
    assert!(
        snapshots[0].silence.is_none(),
        "legacy silence detector emits no envelope until its own migration"
    );
}

#[test]
fn inversion_test_shape_allows_downstream_verdict() {
    // The inversion invariant: every emitted shape allows downstream
    // (Governor / NS / AG) to deny, defer, revalidate, or admit without
    // NQ encoding the verdict in the wire shape itself.
    //
    // Concretely: no field on the import wire shape carries a verdict
    // verb (no "should_alert", "page_oncall", "block_release", etc.).
    // The wire shape is testimony only.
    let db = test_db();
    let _ = ingest_finding_import(
        db.conn(),
        FIXTURE_CLEAN,
        1,
        "2026-05-12T10:00:30Z",
        &IngestConfig::default(),
    )
    .unwrap();
    let snapshots = export_findings_from_conn(db.conn(), &ExportFilter::default()).unwrap();
    let json = serde_json::to_string(&snapshots).unwrap();

    // No verdict verbs anywhere in the wire shape.
    for verb in [
        "should_alert",
        "page_oncall",
        "block_release",
        "auto_remediate",
        "auto_retract",
    ] {
        assert!(
            !json.contains(verb),
            "wire shape must not encode verdict verb `{}`",
            verb
        );
    }

    // Origin presence is the import discriminator — consumers can branch
    // (defer to producer extraction time semantics) without NQ telling
    // them what to do.
    assert!(json.contains("\"origin\""));
    assert!(json.contains("\"source\":\"import\""));
}
