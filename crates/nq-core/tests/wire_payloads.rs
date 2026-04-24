//! Tests for deserializing PublisherState from malformed and adversarial JSON payloads.

use nq_core::wire::PublisherState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn full_payload() -> String {
    r#"{
        "host": "node-1.prod",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "host": {
                "status": "ok",
                "collected_at": "2025-06-15T12:00:00Z",
                "error_message": null,
                "data": {
                    "cpu_load_1m": 0.5,
                    "cpu_load_5m": 0.3,
                    "mem_total_mb": 16384,
                    "mem_available_mb": 8192,
                    "mem_pressure_pct": 50.0,
                    "disk_total_mb": 512000,
                    "disk_avail_mb": 256000,
                    "disk_used_pct": 50.0,
                    "uptime_seconds": 86400,
                    "kernel_version": "6.8.0-94-generic",
                    "boot_id": "abc-123"
                }
            },
            "services": {
                "status": "ok",
                "collected_at": "2025-06-15T12:00:00Z",
                "error_message": null,
                "data": [
                    {
                        "service": "api-gateway",
                        "status": "up",
                        "health_detail_json": "{\"latency_ms\": 12}",
                        "pid": 1234,
                        "uptime_seconds": 43200,
                        "last_restart": "2025-06-14T12:00:00Z",
                        "eps": 150.5,
                        "queue_depth": 0,
                        "consumer_lag": 0,
                        "drop_count": 0
                    },
                    {
                        "service": "worker",
                        "status": "degraded",
                        "health_detail_json": null,
                        "pid": 5678,
                        "uptime_seconds": 3600,
                        "last_restart": null,
                        "eps": null,
                        "queue_depth": 42,
                        "consumer_lag": 100,
                        "drop_count": 3
                    }
                ]
            },
            "sqlite_health": {
                "status": "ok",
                "collected_at": "2025-06-15T12:00:00Z",
                "error_message": null,
                "data": [
                    {
                        "db_path": "/var/lib/app/main.db",
                        "db_size_mb": 128.5,
                        "wal_size_mb": 2.1,
                        "page_size": 4096,
                        "page_count": 32000,
                        "freelist_count": 10,
                        "journal_mode": "wal",
                        "auto_vacuum": "none",
                        "last_checkpoint": "2025-06-15T11:55:00Z",
                        "checkpoint_lag_s": 300,
                        "last_quick_check": "ok",
                        "last_integrity_check": "ok",
                        "last_integrity_at": "2025-06-15T06:00:00Z",
                        "db_mtime": "2025-06-15T11:55:00Z",
                        "wal_mtime": "2025-06-15T11:59:30Z"
                    }
                ]
            }
        }
    }"#
    .to_string()
}

// ---------------------------------------------------------------------------
// 1. Valid payload — full happy path
// ---------------------------------------------------------------------------

#[test]
fn valid_full_payload() {
    let json = full_payload();
    let state: PublisherState =
        serde_json::from_str(&json).expect("full valid payload should deserialize");
    assert_eq!(state.host, "node-1.prod");
    assert!(state.collectors.host.is_some());
    assert!(state.collectors.services.is_some());
    assert!(state.collectors.sqlite_health.is_some());

    let host_data = state.collectors.host.unwrap().data.unwrap();
    assert_eq!(host_data.cpu_load_1m, Some(0.5));
    assert_eq!(host_data.mem_total_mb, Some(16384));

    let services = state.collectors.services.unwrap().data.unwrap();
    assert_eq!(services.len(), 2);
    assert_eq!(services[0].service, "api-gateway");
}

// ---------------------------------------------------------------------------
// 2. Missing collector blocks — optional fields default to None
// ---------------------------------------------------------------------------

#[test]
fn missing_host_collector() {
    let json = r#"{
        "host": "node-2",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "services": {
                "status": "ok",
                "collected_at": "2025-06-15T12:00:00Z",
                "error_message": null,
                "data": []
            }
        }
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    assert!(state.collectors.host.is_none());
    assert!(state.collectors.services.is_some());
    assert!(state.collectors.sqlite_health.is_none());
}

#[test]
fn missing_services_collector() {
    let json = r#"{
        "host": "node-3",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "host": {
                "status": "ok",
                "collected_at": null,
                "error_message": null,
                "data": null
            }
        }
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    assert!(state.collectors.host.is_some());
    assert!(state.collectors.services.is_none());
}

// ---------------------------------------------------------------------------
// 3. Empty collectors object — all collectors absent
// ---------------------------------------------------------------------------

#[test]
fn empty_collectors() {
    let json = r#"{
        "host": "node-4",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {}
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    assert!(state.collectors.host.is_none());
    assert!(state.collectors.services.is_none());
    assert!(state.collectors.sqlite_health.is_none());
}

// ---------------------------------------------------------------------------
// 4. Bad enum value for CollectorStatus (used as "status" in collector payload)
// ---------------------------------------------------------------------------

#[test]
fn bad_collector_status_value() {
    let json = r#"{
        "host": "node-5",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "host": {
                "status": "exploded",
                "collected_at": null,
                "error_message": null,
                "data": null
            }
        }
    }"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "unknown collector status should fail deserialization");
}

// ---------------------------------------------------------------------------
// 5. Bad enum value for ServiceStatus
// ---------------------------------------------------------------------------

#[test]
fn bad_service_status_value() {
    let json = r#"{
        "host": "node-6",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "services": {
                "status": "ok",
                "collected_at": "2025-06-15T12:00:00Z",
                "error_message": null,
                "data": [
                    {
                        "service": "api",
                        "status": "on_fire",
                        "health_detail_json": null,
                        "pid": null,
                        "uptime_seconds": null,
                        "last_restart": null,
                        "eps": null,
                        "queue_depth": null,
                        "consumer_lag": null,
                        "drop_count": null
                    }
                ]
            }
        }
    }"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "unknown service status should fail deserialization");
}

// ---------------------------------------------------------------------------
// 6. Null where struct expected — should deserialize as None
// ---------------------------------------------------------------------------

#[test]
fn null_host_collector() {
    let json = r#"{
        "host": "node-7",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "host": null
        }
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    assert!(state.collectors.host.is_none());
}

#[test]
fn null_services_collector() {
    let json = r#"{
        "host": "node-7b",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "services": null,
            "sqlite_health": null
        }
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    assert!(state.collectors.services.is_none());
    assert!(state.collectors.sqlite_health.is_none());
}

// ---------------------------------------------------------------------------
// 7. Collector says ok but data is null/absent
// ---------------------------------------------------------------------------

#[test]
fn collector_ok_but_data_null() {
    let json = r#"{
        "host": "node-8",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "host": {
                "status": "ok",
                "collected_at": "2025-06-15T12:00:00Z",
                "error_message": null,
                "data": null
            }
        }
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    let host_collector = state.collectors.host.unwrap();
    assert!(host_collector.data.is_none());
}

#[test]
fn collector_ok_but_data_absent() {
    let json = r#"{
        "host": "node-8b",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "host": {
                "status": "ok",
                "collected_at": "2025-06-15T12:00:00Z",
                "error_message": null
            }
        }
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    let host_collector = state.collectors.host.unwrap();
    assert!(host_collector.data.is_none());
}

// ---------------------------------------------------------------------------
// 8. Collector says error but data is present
// ---------------------------------------------------------------------------

#[test]
fn collector_error_with_data_present() {
    let json = r#"{
        "host": "node-9",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "host": {
                "status": "error",
                "collected_at": "2025-06-15T12:00:00Z",
                "error_message": "disk on fire",
                "data": {
                    "cpu_load_1m": 99.9,
                    "cpu_load_5m": null,
                    "mem_total_mb": null,
                    "mem_available_mb": null,
                    "mem_pressure_pct": null,
                    "disk_total_mb": null,
                    "disk_avail_mb": null,
                    "disk_used_pct": null,
                    "uptime_seconds": null,
                    "kernel_version": null,
                    "boot_id": null
                }
            }
        }
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    let host_collector = state.collectors.host.unwrap();
    assert_eq!(host_collector.error_message.as_deref(), Some("disk on fire"));
    assert!(host_collector.data.is_some());
    assert_eq!(host_collector.data.unwrap().cpu_load_1m, Some(99.9));
}

// ---------------------------------------------------------------------------
// 9. Duplicate service names
// ---------------------------------------------------------------------------

#[test]
fn duplicate_service_names() {
    let json = r#"{
        "host": "node-10",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "services": {
                "status": "ok",
                "collected_at": "2025-06-15T12:00:00Z",
                "error_message": null,
                "data": [
                    {
                        "service": "api",
                        "status": "up",
                        "health_detail_json": null,
                        "pid": 100,
                        "uptime_seconds": null,
                        "last_restart": null,
                        "eps": null,
                        "queue_depth": null,
                        "consumer_lag": null,
                        "drop_count": null
                    },
                    {
                        "service": "api",
                        "status": "down",
                        "health_detail_json": null,
                        "pid": 200,
                        "uptime_seconds": null,
                        "last_restart": null,
                        "eps": null,
                        "queue_depth": null,
                        "consumer_lag": null,
                        "drop_count": null
                    }
                ]
            }
        }
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    let services = state.collectors.services.unwrap().data.unwrap();
    assert_eq!(services.len(), 2);
    assert_eq!(services[0].service, "api");
    assert_eq!(services[1].service, "api");
}

// ---------------------------------------------------------------------------
// 10. Giant string in health_detail_json (1 MB)
// ---------------------------------------------------------------------------

#[test]
fn giant_health_detail_json() {
    let big_string = "x".repeat(1_000_000);
    let json = format!(
        r#"{{
            "host": "node-11",
            "collected_at": "2025-06-15T12:00:00Z",
            "collectors": {{
                "services": {{
                    "status": "ok",
                    "collected_at": "2025-06-15T12:00:00Z",
                    "error_message": null,
                    "data": [
                        {{
                            "service": "bloated",
                            "status": "up",
                            "health_detail_json": "{}",
                            "pid": null,
                            "uptime_seconds": null,
                            "last_restart": null,
                            "eps": null,
                            "queue_depth": null,
                            "consumer_lag": null,
                            "drop_count": null
                        }}
                    ]
                }}
            }}
        }}"#,
        big_string
    );
    let state: PublisherState =
        serde_json::from_str(&json).expect("1MB health_detail_json should deserialize");
    let services = state.collectors.services.unwrap().data.unwrap();
    assert_eq!(services[0].health_detail_json.as_ref().unwrap().len(), 1_000_000);
}

// ---------------------------------------------------------------------------
// 11. Future timestamp — year 2099
// ---------------------------------------------------------------------------

#[test]
fn future_timestamp() {
    let json = r#"{
        "host": "node-12",
        "collected_at": "2099-12-31T23:59:59Z",
        "collectors": {}
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    assert_eq!(state.collected_at.year(), 2099);
}

// ---------------------------------------------------------------------------
// 12. Very old timestamp — epoch
// ---------------------------------------------------------------------------

#[test]
fn epoch_timestamp() {
    let json = r#"{
        "host": "node-13",
        "collected_at": "1970-01-01T00:00:00Z",
        "collectors": {}
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    assert_eq!(state.collected_at.year(), 1970);
}

// ---------------------------------------------------------------------------
// 13. Missing required fields — no host field
// ---------------------------------------------------------------------------

#[test]
fn missing_host_field() {
    let json = r#"{
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {}
    }"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "missing 'host' field should fail");
}

#[test]
fn missing_collected_at_field() {
    let json = r#"{
        "host": "node-14",
        "collectors": {}
    }"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "missing 'collected_at' field should fail");
}

#[test]
fn missing_collectors_field() {
    let json = r#"{
        "host": "node-15",
        "collected_at": "2025-06-15T12:00:00Z"
    }"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "missing 'collectors' field should fail");
}

// ---------------------------------------------------------------------------
// 14. Extra unknown fields — should be silently ignored
// ---------------------------------------------------------------------------

#[test]
fn extra_unknown_fields_top_level() {
    let json = r#"{
        "host": "node-16",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {},
        "magic_field": 42,
        "another_thing": [1, 2, 3]
    }"#;
    let state: PublisherState =
        serde_json::from_str(json).expect("unknown top-level fields should be ignored");
    assert_eq!(state.host, "node-16");
}

#[test]
fn extra_unknown_fields_in_collectors() {
    let json = r#"{
        "host": "node-16b",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "network": {"status": "ok"},
            "gpu": {"temp": 85}
        }
    }"#;
    let state: PublisherState =
        serde_json::from_str(json).expect("unknown collector keys should be ignored");
    assert!(state.collectors.host.is_none());
}

#[test]
fn extra_unknown_fields_in_host_data() {
    let json = r#"{
        "host": "node-16c",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {
            "host": {
                "status": "ok",
                "collected_at": null,
                "error_message": null,
                "data": {
                    "cpu_load_1m": 1.0,
                    "cpu_load_5m": null,
                    "mem_total_mb": null,
                    "mem_available_mb": null,
                    "mem_pressure_pct": null,
                    "disk_total_mb": null,
                    "disk_avail_mb": null,
                    "disk_used_pct": null,
                    "uptime_seconds": null,
                    "kernel_version": null,
                    "boot_id": null,
                    "gpu_temp": 85,
                    "alien_signal": true
                }
            }
        }
    }"#;
    let state: PublisherState =
        serde_json::from_str(json).expect("unknown fields in host data should be ignored");
    assert_eq!(state.collectors.host.unwrap().data.unwrap().cpu_load_1m, Some(1.0));
}

// ---------------------------------------------------------------------------
// 15. Empty string for host
// ---------------------------------------------------------------------------

#[test]
fn empty_string_host() {
    let json = r#"{
        "host": "",
        "collected_at": "2025-06-15T12:00:00Z",
        "collectors": {}
    }"#;
    let state: PublisherState = serde_json::from_str(json).unwrap();
    assert_eq!(state.host, "");
}

// ---------------------------------------------------------------------------
// 16. Completely wrong JSON shape
// ---------------------------------------------------------------------------

#[test]
fn completely_wrong_shape() {
    let json = r#"{"garbage": true}"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "completely wrong shape should fail");
}

#[test]
fn empty_object() {
    let json = r#"{}"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "empty object should fail (missing required fields)");
}

// ---------------------------------------------------------------------------
// 17. Valid JSON but not an object
// ---------------------------------------------------------------------------

#[test]
fn json_array_not_object() {
    let json = r#"[1, 2, 3]"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "JSON array should fail");
}

#[test]
fn json_string_not_object() {
    let json = r#""just a string""#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "JSON string should fail");
}

#[test]
fn json_number_not_object() {
    let json = r#"42"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "JSON number should fail");
}

#[test]
fn json_null() {
    let json = r#"null"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "JSON null should fail");
}

#[test]
fn json_bool() {
    let json = r#"true"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "JSON bool should fail");
}

// ---------------------------------------------------------------------------
// 18. Timestamp in wrong format — unix epoch number instead of RFC 3339
// ---------------------------------------------------------------------------

#[test]
fn timestamp_as_unix_epoch_number() {
    let json = r#"{
        "host": "node-18",
        "collected_at": 1718452800,
        "collectors": {}
    }"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "numeric timestamp should fail (expects RFC 3339 string)");
}

#[test]
fn timestamp_as_unix_epoch_string() {
    let json = r#"{
        "host": "node-18b",
        "collected_at": "1718452800",
        "collectors": {}
    }"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "bare numeric string should fail RFC 3339 parsing");
}

#[test]
fn timestamp_wrong_format_human_readable() {
    let json = r#"{
        "host": "node-18c",
        "collected_at": "June 15, 2025 12:00:00",
        "collectors": {}
    }"#;
    let result = serde_json::from_str::<PublisherState>(json);
    assert!(result.is_err(), "human-readable date should fail RFC 3339 parsing");
}
