//! Separability receipt for Track 4 Slice B.3.
//!
//! Proves that after the `nq-witness` extraction, the witness's
//! emit:
//!
//! 1. Produces a structurally complete `PublisherState` — all
//!    collector slots present, every payload shaped like
//!    `CollectorPayload<T>` (status + optional data + optional
//!    error_message).
//! 2. Round-trips through serde — serialize to JSON, deserialize
//!    back via `nq_core::wire::PublisherState`, identity holds at
//!    the JSON level.
//! 3. Carries the documented `nq.witness_packet.v1` envelope schema —
//!    the `schema` field is stamped by `PublisherState::current` and
//!    consumed by `nq_witness_api::fetch_state` (and therefore by
//!    `nq-monitor`'s pull path). This honors the load-bearing versioned-
//!    wire promise in `docs/architecture/COMPATIBILITY.md`. (The `schema`
//!    field was added in the 2026-06-18 schema-reconciliation slice; it is
//!    the one intentional addition over the pre-extraction baseline.)
//!
//! This test deliberately drives every collector against empty /
//! missing substrate so the emit is deterministic across hosts —
//! the assertions check structural shape, not substrate content.

use nq_core::wire::{PublisherState, PUBLISHER_STATE_SCHEMA};
use nq_core::PublisherConfig;
use nq_witness::collect::collect_state;
use std::sync::Arc;

/// Build a PublisherConfig that exercises every collector but
/// points each at empty / missing substrate, so the resulting
/// `PublisherState` is shape-only and host-independent.
fn empty_publisher_config() -> Arc<PublisherConfig> {
    let json = serde_json::json!({
        "bind_addr": "127.0.0.1:0",
        "sqlite_paths": [],
        "service_health_urls": [],
        "prometheus_targets": [],
        "log_sources": [],
        "sqlite_wal_targets": [],
        "sqlite_wal_proc_locks_enabled": true,
    });
    let cfg: PublisherConfig = serde_json::from_value(json).expect("config parses");
    Arc::new(cfg)
}

#[test]
fn witness_emits_structurally_complete_publisher_state() {
    let cfg = empty_publisher_config();
    let state: PublisherState = collect_state(&cfg);

    // Top-level shape
    assert!(!state.host.is_empty(), "host must be populated");
    // `collected_at` is a timestamp — its mere presence is enforced
    // by the struct's type; nothing further to assert here.

    let c = &state.collectors;
    assert!(c.host.is_some(), "host collector slot must be present");
    assert!(c.services.is_some(), "services slot must be present");
    assert!(c.sqlite_health.is_some(), "sqlite_health slot must be present");
    assert!(c.prometheus.is_some(), "prometheus slot must be present");
    assert!(c.logs.is_some(), "logs slot must be present");
    assert!(c.zfs_witness.is_some(), "zfs_witness slot must be present");
    assert!(c.smart_witness.is_some(), "smart_witness slot must be present");
    assert!(
        c.sqlite_wal_observations.is_some(),
        "sqlite_wal_observations slot must be present"
    );
    assert!(
        c.nq_binary_observations.is_some(),
        "nq_binary_observations slot must be present"
    );
}

#[test]
fn witness_emit_round_trips_through_serde() {
    let cfg = empty_publisher_config();
    let state: PublisherState = collect_state(&cfg);

    let json = serde_json::to_value(&state).expect("serializes");
    let restored: PublisherState = serde_json::from_value(json.clone()).expect("deserializes");

    // Re-serialize and compare structurally — confirms no fields
    // were lost or renamed across the round-trip.
    let json2 = serde_json::to_value(&restored).expect("re-serializes");
    assert_eq!(
        json, json2,
        "PublisherState JSON identity must hold across serialize→deserialize→serialize"
    );
}

#[test]
fn witness_emit_carries_versioned_envelope_schema() {
    // The actual GET /state payload must carry the documented
    // `nq.witness_packet.v1` envelope schema (COMPATIBILITY.md). Before the
    // 2026-06-18 reconciliation slice the payload carried no schema at all,
    // while the compat doc promised a load-bearing versioned wire.
    let cfg = empty_publisher_config();
    let state: PublisherState = collect_state(&cfg);

    assert_eq!(
        state.schema.as_deref(),
        Some(PUBLISHER_STATE_SCHEMA),
        "producer must stamp the envelope schema",
    );
    assert_eq!(PUBLISHER_STATE_SCHEMA, "nq.witness_packet.v1");

    let json = serde_json::to_value(&state).expect("serializes");
    let obj = json.as_object().expect("/state payload is a JSON object");

    // Golden top-level wire shape: exactly these keys, schema first-class.
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["collected_at", "collectors", "host", "schema"],
        "GET /state top-level wire shape changed — update COMPATIBILITY.md \
         and bump the envelope version if this is intentional",
    );
    assert_eq!(
        obj.get("schema").and_then(serde_json::Value::as_str),
        Some("nq.witness_packet.v1"),
        "emitted /state must carry the documented versioned schema string",
    );
}

#[test]
fn witness_state_path_matches_witness_api_contract() {
    // Belt-and-braces: the path constant the server registers must
    // be exactly the path the api crate advertises to consumers.
    // If these ever drift, the witness becomes invisible to
    // nq-monitor's pull loop.
    assert_eq!(
        nq_witness_api::STATE_PATH,
        "/state",
        "consumer contract: GET /state"
    );
}
