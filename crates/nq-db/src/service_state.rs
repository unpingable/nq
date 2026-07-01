//! `service_state` witness family — native service-state observations and the
//! evaluator over them. Sibling of `dns.rs` / `sqlite_wal_state.rs`.
//! See `docs/working/decisions/preflights/SERVICE_STATE.md`.
//!
//! The witness records the manager's NATIVE state verbatim; the evaluator
//! interprets it into a verdict at witness scope only. It testifies that a
//! manager reported a service in a native state at T0 — never recovered /
//! healthy / safe / coverage / future-liveness / consequence (those are the
//! constitutional `service_state_cannot_testify`, preloaded by the skeleton).

use crate::service_state_witness_projection::project_service_observation;
use crate::witness_projection_support::{make_projection_refusal_exclusion, packet_identity};
use crate::ReadDb;
use anyhow::{anyhow, Context};
use nq_core::preflight::{
    freshness_horizon_from, ClaimKind, PreflightCoverage, PreflightResult, PreflightSupport,
    PreflightTarget, Verdict,
};
use rusqlite::{params, Connection, OptionalExtension, Row};

/// Staleness threshold for the latest row, seconds. Matches `dns_state`'s 300s
/// heuristic (5× a 60s cycle). Bespoke for V0.
pub const SERVICE_STATE_STALE_THRESHOLD_SECONDS: i64 = 300;

/// One native service-state observation row. `observation_id` is `None` for an
/// unwritten record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceObservation {
    pub observation_id: Option<i64>,
    pub generation_id: i64,
    pub host: String,
    pub service_manager: String, // systemd | docker | process | unknown
    pub service_name: String,
    pub active_state: String, // native manager state verbatim
    pub sub_state: Option<String>,
    pub load_state: Option<String>,
    pub unit_file_state: Option<String>,
    pub observed_at: String,
}

/// The identity that selects "one service under one manager on one host".
#[derive(Debug, Clone, Copy)]
pub struct ServiceObservationTuple<'a> {
    pub host: &'a str,
    pub service_manager: &'a str,
    pub service_name: &'a str,
}

fn from_row(row: &Row) -> rusqlite::Result<ServiceObservation> {
    Ok(ServiceObservation {
        observation_id: Some(row.get("observation_id")?),
        generation_id: row.get("generation_id")?,
        host: row.get("host")?,
        service_manager: row.get("service_manager")?,
        service_name: row.get("service_name")?,
        active_state: row.get("active_state")?,
        sub_state: row.get("sub_state")?,
        load_state: row.get("load_state")?,
        unit_file_state: row.get("unit_file_state")?,
        observed_at: row.get("observed_at")?,
    })
}

/// True when two observations carry the same NATIVE state (the identity key is
/// compared by the caller; this compares what was observed).
fn same_state(a: &ServiceObservation, b: &ServiceObservation) -> bool {
    a.active_state == b.active_state
        && a.sub_state == b.sub_state
        && a.load_state == b.load_state
        && a.unit_file_state == b.unit_file_state
}

/// Insert one observation with **idempotent / explicit-conflict** semantics on
/// the identity key `(generation_id, host, service_manager, service_name)`:
///
/// - no existing row → insert, return the new id;
/// - existing row with the SAME native state → idempotent: return the existing
///   id, no write (a re-observation of the same state is not a new fact);
/// - existing row with a DIFFERENT native state → **explicit error**, never a
///   silent overwrite (two contradictory observations under one identity key in
///   one generation is a fault to surface, not to paper over).
pub fn insert_service_observation(
    conn: &Connection,
    obs: &ServiceObservation,
) -> anyhow::Result<i64> {
    let existing: Option<ServiceObservation> = conn
        .query_row(
            "SELECT observation_id, generation_id, host, service_manager, service_name,
                    active_state, sub_state, load_state, unit_file_state, observed_at
             FROM service_observations
             WHERE generation_id = ?1 AND host = ?2 AND service_manager = ?3 AND service_name = ?4",
            params![
                obs.generation_id,
                obs.host,
                obs.service_manager,
                obs.service_name
            ],
            from_row,
        )
        .optional()
        .with_context(|| "lookup existing service_observation")?;

    if let Some(prior) = existing {
        if same_state(&prior, obs) {
            return Ok(prior.observation_id.expect("row from db has id"));
        }
        return Err(anyhow!(
            "conflicting service_state observation for (gen={}, host={}, manager={}, service={}): \
             existing active_state={:?}/sub={:?} vs new active_state={:?}/sub={:?} — refusing to overwrite",
            obs.generation_id, obs.host, obs.service_manager, obs.service_name,
            prior.active_state, prior.sub_state, obs.active_state, obs.sub_state
        ));
    }

    conn.execute(
        "INSERT INTO service_observations
            (generation_id, host, service_manager, service_name,
             active_state, sub_state, load_state, unit_file_state, observed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            obs.generation_id,
            obs.host,
            obs.service_manager,
            obs.service_name,
            obs.active_state,
            obs.sub_state,
            obs.load_state,
            obs.unit_file_state,
            obs.observed_at,
        ],
    )
    .with_context(|| {
        format!(
            "insert service_observation gen={} ({},{},{})",
            obs.generation_id, obs.host, obs.service_manager, obs.service_name
        )
    })?;
    Ok(conn.last_insert_rowid())
}

/// Most recent observation for the tuple, or `None`. Absence is **not** stored
/// as a sentinel; the evaluator reads `None` as `insufficient_coverage`.
pub fn latest_service_observation_for_tuple(
    conn: &Connection,
    key: &ServiceObservationTuple<'_>,
) -> anyhow::Result<Option<ServiceObservation>> {
    conn.query_row(
        "SELECT observation_id, generation_id, host, service_manager, service_name,
                active_state, sub_state, load_state, unit_file_state, observed_at
         FROM service_observations
         WHERE host = ?1 AND service_manager = ?2 AND service_name = ?3
         ORDER BY observed_at DESC, observation_id DESC
         LIMIT 1",
        params![key.host, key.service_manager, key.service_name],
        from_row,
    )
    .optional()
    .with_context(|| "select latest service_observation")
}

fn make_support(obs: &ServiceObservation) -> PreflightSupport {
    let claim = format!(
        "{} reported service '{}' in native state '{}'{}{} at observed_at {}",
        obs.service_manager,
        obs.service_name,
        obs.active_state,
        obs.sub_state
            .as_deref()
            .map(|s| format!(" (sub={s})"))
            .unwrap_or_default(),
        obs.load_state
            .as_deref()
            .map(|s| format!(" (load={s})"))
            .unwrap_or_default(),
        obs.observed_at,
    );
    PreflightSupport {
        claim,
        finding_kind: "service_state_observed".to_string(),
        subject: format!("{}/{}:{}", obs.host, obs.service_manager, obs.service_name),
        observed_at: Some(obs.observed_at.clone()),
        freshness: None,
        admissibility_state: Some("admissible_with_scope".to_string()),
        witness_packet: None, // projection into nq.witness.v1 is a follow-on slice
    }
}

/// Public `ReadDb` API.
pub fn evaluate_service_state_preflight(
    db: &ReadDb,
    key: &ServiceObservationTuple<'_>,
) -> anyhow::Result<PreflightResult> {
    evaluate_service_state_preflight_from_conn(db.conn(), key)
}

/// Clock-injected `ReadDb` API.
pub fn evaluate_service_state_preflight_at(
    db: &ReadDb,
    key: &ServiceObservationTuple<'_>,
    now: time::OffsetDateTime,
) -> anyhow::Result<PreflightResult> {
    evaluate_service_state_preflight_from_conn_at(db.conn(), key, now)
}

/// Raw-`Connection` form (tests + HTTP route). Ambient clock.
pub fn evaluate_service_state_preflight_from_conn(
    conn: &Connection,
    key: &ServiceObservationTuple<'_>,
) -> anyhow::Result<PreflightResult> {
    evaluate_service_state_preflight_from_conn_at(conn, key, time::OffsetDateTime::now_utc())
}

/// Clock-injected `_from_conn` form. `now` drives both the staleness verdict and
/// `generated_at`.
pub fn evaluate_service_state_preflight_from_conn_at(
    conn: &Connection,
    key: &ServiceObservationTuple<'_>,
    now: time::OffsetDateTime,
) -> anyhow::Result<PreflightResult> {
    let generated_at = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();

    let target = PreflightTarget {
        host: key.host.to_string(),
        scope: "service".to_string(),
        id: Some(format!(
            "manager={};service={}",
            key.service_manager, key.service_name
        )),
    };
    let mut result =
        PreflightResult::skeleton(ClaimKind::ServiceState, target, generated_at.clone());

    let Some(obs) = latest_service_observation_for_tuple(conn, key)? else {
        // Absence is read as insufficient_coverage — "no witness", not "false".
        result.verdict = Verdict::InsufficientCoverage;
        result.verdict_note = Some(format!(
            "No service_observations row exists for (host={}, manager={}, service={}); \
             no witness has observed this service. Absence is not affirmative testimony.",
            key.host, key.service_manager, key.service_name
        ));
        result.coverage.push(PreflightCoverage {
            witness: "service_manager".to_string(),
            standing: "absent".to_string(),
            note: Some(format!("no observation row for {}", key.service_name)),
        });
        result.compute_time_basis();
        return Ok(result);
    };

    let parsed = time::OffsetDateTime::parse(
        &obs.observed_at,
        &time::format_description::well_known::Rfc3339,
    )
    .ok();
    let age_seconds = parsed.map(|t| (now - t).whole_seconds());
    let stale = matches!(age_seconds, Some(age) if age > SERVICE_STATE_STALE_THRESHOLD_SECONDS);

    result.observed_at_min = Some(obs.observed_at.clone());
    result.observed_at_max = Some(obs.observed_at.clone());
    result.freshness_horizon = freshness_horizon_from(
        result.observed_at_max.as_deref(),
        SERVICE_STATE_STALE_THRESHOLD_SECONDS,
    );

    let projected_packet = match project_service_observation(&obs, &generated_at) {
        Ok(packet) => packet,
        Err(refusal) => {
            result.excludes.push(make_projection_refusal_exclusion(
                "service_state_observed".to_string(),
                format!("{}/{}:{}", obs.host, obs.service_manager, obs.service_name),
                &refusal,
            ));
            result.coverage.push(PreflightCoverage {
                witness: "service_manager".to_string(),
                standing: "observable".to_string(),
                note: Some(
                    "latest service_observations row refused projection - see excludes for the custody constraint"
                        .to_string(),
                ),
            });
            result.verdict = Verdict::InsufficientCoverage;
            result.verdict_note = Some(
                "Latest service_observations row could not be projected into an admissible \
                 witness packet; service_state evidence is in custody refusal."
                    .to_string(),
            );
            result.compute_time_basis();
            return Ok(result);
        }
    };

    let mut support = make_support(&obs);
    support.witness_packet = packet_identity(&projected_packet);

    if stale {
        support.freshness = Some("stale".to_string());
        result.supports.push(support);
        result.coverage.push(PreflightCoverage {
            witness: "service_manager".to_string(),
            standing: "stale".to_string(),
            note: Some(format!(
                "most recent observation for {} is older than {}s",
                obs.service_name, SERVICE_STATE_STALE_THRESHOLD_SECONDS
            )),
        });
        result.verdict = Verdict::StaleTestimony;
        result.verdict_note = Some(format!(
            "Latest service_observations row at {} is {}s old (> {}s); service_state testimony is stale.",
            obs.observed_at,
            age_seconds.unwrap_or_default(),
            SERVICE_STATE_STALE_THRESHOLD_SECONDS
        ));
        result.compute_time_basis();
        return Ok(result);
    }

    support.freshness = Some("fresh".to_string());
    result.supports.push(support);
    result.coverage.push(PreflightCoverage {
        witness: "service_manager".to_string(),
        standing: "observable".to_string(),
        note: Some(format!(
            "{} reported {} within the freshness window",
            obs.service_manager, obs.service_name
        )),
    });
    result.verdict = Verdict::AdmissibleWithScope;
    result.verdict_note = Some(format!(
        "{} reported service '{}' in native state '{}' at {}; admissible only at witness scope. \
         active does not imply healthy; inactive does not imply broken; recovery/health/safety/\
         coverage remain refused (see cannot_testify).",
        obs.service_manager, obs.service_name, obs.active_state, obs.observed_at
    ));
    result.compute_time_basis();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};

    fn make_db_gen() -> crate::WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        db.conn
            .execute(
                "INSERT OR IGNORE INTO generations
                    (generation_id, started_at, completed_at, status,
                     sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (1, '2026-06-29T00:00:00Z', '2026-06-29T00:00:00Z', 'complete', 1, 1, 0, 0)",
                [],
            )
            .unwrap();
        db
    }

    fn obs(active: &str, at: &str) -> ServiceObservation {
        ServiceObservation {
            observation_id: None,
            generation_id: 1,
            host: "sushi-k".into(),
            service_manager: "systemd".into(),
            service_name: "kea-dhcp4".into(),
            active_state: active.into(),
            sub_state: Some("running".into()),
            load_state: Some("loaded".into()),
            unit_file_state: Some("enabled".into()),
            observed_at: at.into(),
        }
    }
    fn tuple() -> ServiceObservationTuple<'static> {
        ServiceObservationTuple {
            host: "sushi-k",
            service_manager: "systemd",
            service_name: "kea-dhcp4",
        }
    }
    fn at(s: &str) -> time::OffsetDateTime {
        time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).unwrap()
    }

    #[test]
    fn insert_then_read_latest_roundtrips() {
        let db = make_db_gen();
        let id =
            insert_service_observation(&db.conn, &obs("active", "2026-06-29T12:00:00Z")).unwrap();
        assert!(id > 0);
        let got = latest_service_observation_for_tuple(&db.conn, &tuple())
            .unwrap()
            .unwrap();
        assert_eq!(got.active_state, "active");
        assert_eq!(got.sub_state.as_deref(), Some("running"));
    }

    #[test]
    fn exact_duplicate_write_is_idempotent() {
        let db = make_db_gen();
        let id1 =
            insert_service_observation(&db.conn, &obs("active", "2026-06-29T12:00:00Z")).unwrap();
        // Same identity key + same native state -> idempotent, returns the same id, no second row.
        let id2 =
            insert_service_observation(&db.conn, &obs("active", "2026-06-29T12:00:05Z")).unwrap();
        assert_eq!(id1, id2);
        let n: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM service_observations", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn conflicting_state_under_same_identity_fails_explicitly() {
        let db = make_db_gen();
        insert_service_observation(&db.conn, &obs("active", "2026-06-29T12:00:00Z")).unwrap();
        let err = insert_service_observation(&db.conn, &obs("failed", "2026-06-29T12:00:01Z"))
            .unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("conflicting"),
            "{err}"
        );
        // No silent overwrite: the original row stands.
        let got = latest_service_observation_for_tuple(&db.conn, &tuple())
            .unwrap()
            .unwrap();
        assert_eq!(got.active_state, "active");
    }

    #[test]
    fn missing_observation_is_insufficient_coverage_not_false() {
        let db = make_db_gen();
        let r = evaluate_service_state_preflight_from_conn(&db.conn, &tuple()).unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(r.supports.is_empty());
    }

    #[test]
    fn observed_state_is_admissible_at_witness_scope_with_refusals() {
        let db = make_db_gen();
        insert_service_observation(&db.conn, &obs("active", "2026-06-29T12:00:00Z")).unwrap();
        let r = evaluate_service_state_preflight_from_conn_at(
            &db.conn,
            &tuple(),
            at("2026-06-29T12:00:30Z"),
        )
        .unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        assert_eq!(r.supports.len(), 1);
        assert!(r.supports[0].claim.contains("native state 'active'"));
        let wp = r.supports[0]
            .witness_packet
            .as_ref()
            .expect("admitted service_state support must carry projected packet identity");
        assert_eq!(
            wp.witness_type,
            crate::service_state_witness_projection::WITNESS_TYPE_SERVICE_MANAGER
        );
        assert_eq!(
            wp.custody_basis.as_deref(),
            Some(nq_core::witness::CUSTODY_BASIS_LEGACY_PROJECTION)
        );
        assert!(wp.digest.starts_with("sha256:"));
        assert_eq!(wp.observed_at, "2026-06-29T12:00:00Z");
        // The constitutional refusals are present (recovered / healthy / safe).
        let refusals: String = r
            .cannot_testify
            .iter()
            .map(|c| c.statement.clone())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(refusals.to_lowercase().contains("recovery"));
        assert!(refusals.to_lowercase().contains("health"));
        assert!(refusals.to_lowercase().contains("safety"));
    }

    #[test]
    fn stale_row_is_stale_testimony() {
        let db = make_db_gen();
        insert_service_observation(&db.conn, &obs("active", "2026-06-29T12:00:00Z")).unwrap();
        // > 300s later.
        let r = evaluate_service_state_preflight_from_conn_at(
            &db.conn,
            &tuple(),
            at("2026-06-29T12:10:00Z"),
        )
        .unwrap();
        assert_eq!(r.verdict, Verdict::StaleTestimony);
    }

    #[test]
    fn projection_refusal_is_exclusion_not_support() {
        let db = make_db_gen();
        db.conn
            .execute(
                "INSERT INTO service_observations
                    (generation_id, host, service_manager, service_name,
                     active_state, sub_state, load_state, unit_file_state, observed_at)
                 VALUES (1, 'sushi-k', 'systemd', 'kea-dhcp4',
                         'active', 'running', 'loaded', 'enabled', 'not-a-timestamp')",
                [],
            )
            .unwrap();

        let r = evaluate_service_state_preflight_from_conn_at(
            &db.conn,
            &tuple(),
            at("2026-06-29T12:00:30Z"),
        )
        .unwrap();

        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(
            r.supports.is_empty(),
            "projection-refused service_state row must not appear in supports"
        );
        let refusal = r
            .excludes
            .iter()
            .find(|e| e.finding_kind == "service_state_observed")
            .expect("projection-refused row must appear in excludes");
        assert!(
            refusal.reason.contains("projection refused"),
            "exclusion reason must name projection refusal: {}",
            refusal.reason
        );
    }
}
