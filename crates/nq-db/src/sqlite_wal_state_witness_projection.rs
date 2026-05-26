//! Project `wal_observations` rows into legacy-projection witness
//! packets.
//!
//! **Transitional substrate.** Same posture as the three prior
//! projectors: this module exists to carry `sqlite_wal_state` substrate
//! (probe-written `wal_observations` rows) across the witness-packet
//! boundary on the same custody contract that disk_state, ingest_state,
//! and dns_state already use. When a future native `sqlite_wal_probe`
//! witness emits packets directly at probe time, this module retires
//! with the projection layer.
//!
//! ## The custody contract
//!
//! A projector reads a `WalObservation` and emits a `WitnessPacket` with
//! `custody_basis == "legacy_projection"`. Per the preflight (§7) the
//! projector **refuses rather than fakes** when the substrate-time
//! `observed_at` cannot be recovered. `observed_at` on the packet comes
//! from `obs.observed_at` (the column the probe wrote), never from the
//! evaluator's wall-clock.
//!
//! `projection_limits` on every emitted packet includes the literal
//! `"native_witness_custody"` token (wire-enforced) plus
//! `"filesystem observation recovered from wal_observations row, not
//! first-person witness emission"`. That second token is the honest
//! description of what `sqlite_wal_state` projection is — the
//! filesystem stat happened at probe time; the packet is reconstructed
//! from the row by the evaluator at preflight time.
//!
//! ## Witness type vocabulary
//!
//! One value: `sqlite_wal_legacy_projection`. The witness is the
//! `sqlite_wal_probe` at one vantage; what it saw (WAL size, mtime
//! delta, pinned-reader status, `/proc` capability) varies and lives
//! in the observation body. Per the ratified keeper *"Witness type
//! names the witness. Observation fields report what it saw."*
//!
//! ## What this module does not do
//!
//! - Does not wire into the `sqlite_wal_state` evaluator. Slice 4 does
//!   the wiring.
//! - Does not enforce evaluator-level constitutional `cannot_testify`.
//!   The long constitutional list (application recovery, query
//!   correctness, future-checkpoint outcomes) belongs to the
//!   `sqlite_wal_state` claim kind, not to the `sqlite_wal_probe`
//!   witness profile.
//! - Does not classify substrate state as "bloated" / "pressured" /
//!   "pinned" / "warn" / "critical." The observation body reports
//!   what was observed; the evaluator (slice 4) classifies. Per the
//!   preflight §5 [[feedback_knob_facing]] discipline.

use crate::sqlite_wal_state::WalObservation;
use crate::witness_projection_support::ProjectionRefusal;
use nq_core::witness::{
    WitnessPacket, CUSTODY_BASIS_LEGACY_PROJECTION, PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY,
    WITNESS_SCHEMA,
};
use serde_json::json;

/// Single witness type for every projected `wal_observations` row.
/// Per the preflight (§2): the witness is the probe; the WAL size and
/// pinned-reader observations ride in the body.
pub const WITNESS_TYPE_SQLITE_WAL: &str = "sqlite_wal_legacy_projection";

/// Second `projection_limits` token alongside `"native_witness_custody"`.
/// Names the specific custody gap for sqlite_wal projections: the
/// observation came from a probe stat-ing a file, but the packet is
/// reconstructed by the evaluator from the row, not emitted
/// first-person by the probe at probe time.
///
/// Worded *"filesystem observation recovered from … not first-person
/// witness emission"* rather than *"probe self-testimony"* — the
/// earlier framing read too close to "the row authorizes itself,"
/// exactly the laundering shape the parent invariants refuse.
pub const PROJECTION_LIMIT_SQLITE_WAL_OBSERVATION_RECOVERY: &str =
    "filesystem observation recovered from wal_observations row, not first-person witness emission";

/// Project a `wal_observations` row into a legacy-projection witness
/// packet.
///
/// Returns `Err(ProjectionRefusal)` when:
///
/// - `obs.observation_id` is `None` or non-positive — the projector
///   only handles rows that have already been written and assigned an
///   id; a row without one cannot anchor a stable `source_finding_ref`.
/// - `obs.host` or `obs.db_file_path` is empty/whitespace.
/// - `obs.observed_at` is empty/whitespace/unparseable RFC3339.
/// - `obs.wal_mtime.is_some()` and the value is empty/whitespace or
///   unparseable RFC3339. (The `wal_present == false` case carries
///   `wal_mtime = None` by substrate invariant; that is honest absence,
///   not refusable.)
/// - `obs.db_mtime` is empty/whitespace/unparseable RFC3339.
/// - `obs.wal_bytes` or `obs.db_bytes` is negative.
/// - The resulting packet fails the wire validator (defensive only —
///   should be unreachable when the projector emits a well-formed
///   envelope).
///
/// Refusal is per-row. A single refused row degrades to a
/// `PreflightExclusion` at the evaluator layer; the window still
/// admits the other rows. Different from `ingest_state`, where one
/// refused generation row IS the entire substrate.
pub fn project_wal_observation(
    obs: &WalObservation,
    generated_at: &str,
) -> Result<WitnessPacket, ProjectionRefusal> {
    let observation_id = obs.observation_id.unwrap_or(0);
    let source_ref = format!("wal_observation:{observation_id}");
    let refuse = |reason: &str| ProjectionRefusal {
        reason: reason.to_string(),
        source_ref: source_ref.clone(),
    };

    if observation_id <= 0 {
        return Err(refuse(
            "observation_id is missing or non-positive; projection cannot anchor \
             source_finding_ref to a written row",
        ));
    }

    let host = obs.host.trim();
    let db_file_path = obs.db_file_path.trim();
    if host.is_empty() {
        return Err(refuse("host is empty; substrate identity is incomplete"));
    }
    if db_file_path.is_empty() {
        return Err(refuse(
            "db_file_path is empty; substrate identity is incomplete",
        ));
    }

    if obs.wal_bytes < 0 {
        return Err(refuse(&format!(
            "wal_bytes is negative ({}); projection would record impossible substrate",
            obs.wal_bytes
        )));
    }
    if obs.db_bytes < 0 {
        return Err(refuse(&format!(
            "db_bytes is negative ({}); projection would record impossible substrate",
            obs.db_bytes
        )));
    }

    let observed_at = obs.observed_at.trim();
    if observed_at.is_empty() {
        return Err(refuse(
            "wal_observations row has no substrate-time observed_at (empty); \
             projection would have to fabricate it",
        ));
    }
    let observed_at_parsed = time::OffsetDateTime::parse(
        observed_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| {
        refuse(&format!(
            "wal_observations observed_at is not RFC3339: {observed_at:?}; \
             projection would have to forge a parseable timestamp"
        ))
    })?;

    let db_mtime = obs.db_mtime.trim();
    if db_mtime.is_empty() {
        return Err(refuse(
            "wal_observations row has no substrate-time db_mtime (empty); \
             projection would have to fabricate it",
        ));
    }
    let db_mtime_parsed = time::OffsetDateTime::parse(
        db_mtime,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| {
        refuse(&format!(
            "wal_observations db_mtime is not RFC3339: {db_mtime:?}; \
             projection would have to forge a parseable timestamp"
        ))
    })?;

    // wal_mtime: present iff wal_present == true. When present, must
    // parse cleanly. When absent (wal_present == false), wal_mtime is
    // legitimately None and that absence rides into the observation
    // body as JSON null.
    if let Some(raw) = obs.wal_mtime.as_deref() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(refuse(
                "wal_observations row has wal_mtime set to empty/whitespace; \
                 projection cannot distinguish that from honest absence",
            ));
        }
        if time::OffsetDateTime::parse(trimmed, &time::format_description::well_known::Rfc3339)
            .is_err()
        {
            return Err(refuse(&format!(
                "wal_observations wal_mtime is not RFC3339: {trimmed:?}; \
                 projection would have to forge a parseable timestamp"
            )));
        }
    }

    // Derived fields per preflight §3:
    //   wal_db_ratio       = wal_bytes / db_bytes (None if db_bytes == 0)
    //   mtime_delta_seconds = observed_at - db_mtime (how stale the
    //                         main DB is at observation time; the
    //                         "main DB hasn't moved" condition reads
    //                         this).
    let wal_db_ratio: Option<f64> = if obs.db_bytes > 0 {
        Some(obs.wal_bytes as f64 / obs.db_bytes as f64)
    } else {
        None
    };
    let mtime_delta_seconds = (observed_at_parsed - db_mtime_parsed).whole_seconds();

    // Per preflight §6: subject is `host:{h}/db:{path}`. Adopts the
    // disk_state aesthetic verbatim; not a fifth subject vocabulary.
    let subject = format!("host:{host}/db:{db_file_path}");

    let observation = json!({
        "type": "sqlite_wal_observation_projected",
        "host": host,
        "db_file_path": db_file_path,
        "wal_present": obs.wal_present,
        "wal_bytes": obs.wal_bytes,
        "db_bytes": obs.db_bytes,
        "wal_db_ratio": wal_db_ratio,
        "wal_mtime": obs.wal_mtime,
        "db_mtime": obs.db_mtime,
        "mtime_delta_seconds": mtime_delta_seconds,
        "proc_access": obs.proc_access.as_str(),
        "pinned_reader_present": obs.pinned_reader_present,
        "pinned_reader_pid": obs.pinned_reader_pid,
        "pinned_reader_command": obs.pinned_reader_command,
        "error_detail": obs.error_detail,
    });

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.to_string(),
        witness_type: WITNESS_TYPE_SQLITE_WAL.to_string(),
        subject,
        access_path: "legacy_wal_observation_projection".to_string(),
        observed_at: observed_at.to_string(),
        generated_at: generated_at.to_string(),
        observations: vec![observation],
        coverage_limits: vec![
            "packet reconstructed from probe-written wal_observations row".to_string(),
            "native witness packet emission not implemented for sqlite_wal_state".to_string(),
        ],
        dependencies: vec![],
        custody_basis: Some(CUSTODY_BASIS_LEGACY_PROJECTION.to_string()),
        source_finding_ref: Some(source_ref.clone()),
        projection_limits: vec![
            PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string(),
            PROJECTION_LIMIT_SQLITE_WAL_OBSERVATION_RECOVERY.to_string(),
        ],
    };

    packet
        .validate()
        .map_err(|e| refuse(&format!("projected packet failed wire validation: {e}")))?;
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite_wal_state::ProcAccess;
    use nq_core::witness::{CUSTODY_BASIS_LEGACY_PROJECTION, WITNESS_SCHEMA};

    const GENERATED_AT: &str = "2026-05-26T14:00:00Z";
    const OBSERVED_AT: &str = "2026-05-26T13:59:30Z";
    const DB_MTIME: &str = "2026-04-17T12:00:00Z"; // intentionally stale
    const WAL_MTIME: &str = "2026-05-26T13:59:00Z";

    fn observed_obs(observation_id: i64) -> WalObservation {
        WalObservation {
            observation_id: Some(observation_id),
            generation_id: 100,
            host: "labelwatch.neutral.zone".into(),
            db_file_path: "/var/lib/labelwatch/labelwatch.db".into(),
            wal_present: true,
            wal_bytes: 38_000_000_000,
            wal_mtime: Some(WAL_MTIME.into()),
            db_bytes: 26_000_000_000,
            db_mtime: DB_MTIME.into(),
            proc_access: ProcAccess::Observed,
            pinned_reader_present: Some(true),
            pinned_reader_pid: Some(12345),
            pinned_reader_command: Some("labelwatch-discovery".into()),
            observed_at: OBSERVED_AT.into(),
            error_detail: None,
        }
    }

    // -- Happy path ---------------------------------------------------

    #[test]
    fn projects_observed_row_into_legacy_projection_packet() {
        let obs = observed_obs(7);
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();

        assert_eq!(pkt.schema, WITNESS_SCHEMA);
        assert_eq!(pkt.witness_type, WITNESS_TYPE_SQLITE_WAL);
        assert_eq!(
            pkt.subject,
            "host:labelwatch.neutral.zone/db:/var/lib/labelwatch/labelwatch.db",
            "preflight §6: host:{{h}}/db:{{path}} (disk_state aesthetic, no fifth vocabulary)"
        );
        assert_eq!(pkt.access_path, "legacy_wal_observation_projection");
        assert_eq!(pkt.observed_at, OBSERVED_AT);
        assert_eq!(pkt.generated_at, GENERATED_AT);
        assert_eq!(
            pkt.custody_basis.as_deref(),
            Some(CUSTODY_BASIS_LEGACY_PROJECTION)
        );
        assert_eq!(pkt.source_finding_ref.as_deref(), Some("wal_observation:7"));
        assert!(pkt
            .projection_limits
            .contains(&PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string()));
        assert!(pkt
            .projection_limits
            .contains(&PROJECTION_LIMIT_SQLITE_WAL_OBSERVATION_RECOVERY.to_string()));
    }

    #[test]
    fn projection_uses_substrate_observed_at_not_generated_at() {
        let obs = observed_obs(1);
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        assert_eq!(pkt.observed_at, OBSERVED_AT);
        assert_ne!(pkt.observed_at, GENERATED_AT);
    }

    #[test]
    fn projected_packet_passes_wire_validator() {
        let obs = observed_obs(1);
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        pkt.validate().unwrap();
    }

    // -- Derived-field discipline (preflight §3) ----------------------

    #[test]
    fn observation_body_carries_derived_wal_db_ratio() {
        let mut obs = observed_obs(1);
        obs.wal_bytes = 13;
        obs.db_bytes = 26;
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        let body = &pkt.observations[0];
        let ratio = body.get("wal_db_ratio").and_then(|v| v.as_f64()).unwrap();
        assert!((ratio - 0.5).abs() < 1e-9, "got {ratio}");
    }

    #[test]
    fn observation_body_carries_null_wal_db_ratio_when_db_empty() {
        // db_bytes == 0 means main DB is empty; ratio is undefined.
        // Reporting it as null is honest; computing wal_bytes/0 is not.
        let mut obs = observed_obs(1);
        obs.wal_bytes = 0;
        obs.db_bytes = 0;
        obs.wal_present = false;
        obs.wal_mtime = None;
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        let body = &pkt.observations[0];
        assert!(
            body.get("wal_db_ratio").map_or(false, |v| v.is_null()),
            "wal_db_ratio must be null when db_bytes == 0"
        );
    }

    #[test]
    fn observation_body_carries_derived_mtime_delta_seconds() {
        // OBSERVED_AT - DB_MTIME = 2026-05-26T13:59:30Z - 2026-04-17T12:00:00Z
        // That's 39 days, 1 hour, 59 min 30s. Compute and verify the
        // sign + magnitude; not the exact value (test-fixture-coupled).
        let obs = observed_obs(1);
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        let body = &pkt.observations[0];
        let delta = body
            .get("mtime_delta_seconds")
            .and_then(|v| v.as_i64())
            .unwrap();
        assert!(
            delta > 30 * 24 * 3600 && delta < 50 * 24 * 3600,
            "mtime_delta_seconds {delta} should be ~39 days (between 30 and 50 days in s)"
        );
    }

    // -- Observation body fidelity ------------------------------------

    #[test]
    fn observation_body_carries_substrate_identity_and_outcome() {
        let obs = observed_obs(42);
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        let body = &pkt.observations[0];
        assert_eq!(
            body.get("host").and_then(|v| v.as_str()),
            Some("labelwatch.neutral.zone")
        );
        assert_eq!(
            body.get("db_file_path").and_then(|v| v.as_str()),
            Some("/var/lib/labelwatch/labelwatch.db")
        );
        assert_eq!(
            body.get("wal_present").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            body.get("wal_bytes").and_then(|v| v.as_i64()),
            Some(38_000_000_000)
        );
        assert_eq!(
            body.get("db_bytes").and_then(|v| v.as_i64()),
            Some(26_000_000_000)
        );
        assert_eq!(
            body.get("proc_access").and_then(|v| v.as_str()),
            Some("observed")
        );
        assert_eq!(
            body.get("pinned_reader_present").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            body.get("pinned_reader_pid").and_then(|v| v.as_i64()),
            Some(12345)
        );
        assert_eq!(
            body.get("pinned_reader_command").and_then(|v| v.as_str()),
            Some("labelwatch-discovery")
        );
    }

    #[test]
    fn observation_body_carries_null_pinned_reader_when_proc_unavailable() {
        let mut obs = observed_obs(1);
        obs.proc_access = ProcAccess::Unavailable;
        obs.pinned_reader_present = None;
        obs.pinned_reader_pid = None;
        obs.pinned_reader_command = None;
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        let body = &pkt.observations[0];
        assert_eq!(
            body.get("proc_access").and_then(|v| v.as_str()),
            Some("unavailable")
        );
        assert!(body
            .get("pinned_reader_present")
            .map_or(false, |v| v.is_null()));
        assert!(body
            .get("pinned_reader_pid")
            .map_or(false, |v| v.is_null()));
        assert!(body
            .get("pinned_reader_command")
            .map_or(false, |v| v.is_null()));
    }

    #[test]
    fn observation_body_carries_null_wal_mtime_when_wal_absent() {
        // wal_present = false ⇒ wal_mtime = None (substrate invariant).
        // The body carries that absence as JSON null.
        let mut obs = observed_obs(1);
        obs.wal_present = false;
        obs.wal_bytes = 0;
        obs.wal_mtime = None;
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        let body = &pkt.observations[0];
        assert_eq!(body.get("wal_present").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(body.get("wal_bytes").and_then(|v| v.as_i64()), Some(0));
        assert!(body.get("wal_mtime").map_or(false, |v| v.is_null()));
    }

    // -- Refusal lanes (preflight §7) ---------------------------------

    #[test]
    fn refuses_missing_observation_id() {
        let mut obs = observed_obs(1);
        obs.observation_id = None;
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("observation_id"));
        assert_eq!(err.source_ref, "wal_observation:0");
    }

    #[test]
    fn refuses_nonpositive_observation_id() {
        let mut obs = observed_obs(1);
        obs.observation_id = Some(0);
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("observation_id"));
    }

    #[test]
    fn refuses_empty_host() {
        let mut obs = observed_obs(1);
        obs.host = "".into();
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("host"));
    }

    #[test]
    fn refuses_whitespace_db_file_path() {
        let mut obs = observed_obs(1);
        obs.db_file_path = "   ".into();
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("db_file_path"));
    }

    #[test]
    fn refuses_negative_wal_bytes() {
        let mut obs = observed_obs(1);
        obs.wal_bytes = -1;
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("wal_bytes"));
        assert!(
            err.reason.contains("impossible") || err.reason.contains("negative"),
            "refusal reason should name the impossible-substrate failure mode"
        );
    }

    #[test]
    fn refuses_negative_db_bytes() {
        let mut obs = observed_obs(1);
        obs.db_bytes = -1;
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("db_bytes"));
    }

    #[test]
    fn refuses_empty_observed_at() {
        let mut obs = observed_obs(1);
        obs.observed_at = "".into();
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("observed_at"));
        assert!(
            err.reason.contains("fabricate"),
            "refusal reason should name the laundering risk"
        );
    }

    #[test]
    fn refuses_unparseable_observed_at() {
        let mut obs = observed_obs(1);
        obs.observed_at = "yesterday".into();
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("RFC3339"));
    }

    #[test]
    fn refuses_empty_db_mtime() {
        let mut obs = observed_obs(1);
        obs.db_mtime = "".into();
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("db_mtime"));
    }

    #[test]
    fn refuses_unparseable_db_mtime() {
        let mut obs = observed_obs(1);
        obs.db_mtime = "soon".into();
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("RFC3339"));
    }

    #[test]
    fn refuses_empty_wal_mtime_when_present() {
        // wal_mtime: None is honest absence (wal_present = false).
        // Some("") is laundering — the projector cannot tell that
        // from honest absence. Refuse.
        let mut obs = observed_obs(1);
        obs.wal_mtime = Some("".into());
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("wal_mtime"));
    }

    #[test]
    fn refuses_unparseable_wal_mtime() {
        let mut obs = observed_obs(1);
        obs.wal_mtime = Some("recently".into());
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("RFC3339"));
    }

    #[test]
    fn refusal_display_includes_reason_and_source_ref() {
        let mut obs = observed_obs(13);
        obs.observed_at = "".into();
        let err = project_wal_observation(&obs, GENERATED_AT).unwrap_err();
        let rendered = format!("{err}");
        assert!(rendered.contains("observed_at"));
        assert!(rendered.contains("wal_observation:13"));
    }

    // -- Custody discipline -------------------------------------------

    #[test]
    fn projection_limits_carry_both_tokens() {
        let obs = observed_obs(1);
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        assert!(pkt
            .projection_limits
            .iter()
            .any(|l| l == PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY));
        assert!(pkt
            .projection_limits
            .iter()
            .any(|l| l == PROJECTION_LIMIT_SQLITE_WAL_OBSERVATION_RECOVERY));
        // No "self-testimony" wording — the projection-limits string
        // was explicitly worded around that demon, mirroring the
        // dns_state projector's belt-and-braces guard.
        for limit in &pkt.projection_limits {
            assert!(
                !limit.to_ascii_lowercase().contains("self-testimony"),
                "projection_limits must not narrate the row as self-testimony: {limit:?}"
            );
        }
    }

    #[test]
    fn coverage_limits_match_preflight_section_7() {
        let obs = observed_obs(1);
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        assert!(pkt
            .coverage_limits
            .iter()
            .any(|l| l == "packet reconstructed from probe-written wal_observations row"));
        assert!(pkt
            .coverage_limits
            .iter()
            .any(|l| l == "native witness packet emission not implemented for sqlite_wal_state"));
    }

    #[test]
    fn packet_declares_legacy_projection_custody() {
        let obs = observed_obs(1);
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        assert_eq!(
            pkt.custody_basis.as_deref(),
            Some(CUSTODY_BASIS_LEGACY_PROJECTION),
            "projected packet must declare legacy_projection custody"
        );
    }

    #[test]
    fn observation_body_does_not_name_any_claim_key() {
        // Parent invariant 1 (witnesses observe, do not promote) is
        // wire-enforced via WitnessPacket::validate — but a passing
        // validator does not prove the projector emits an observation-
        // only body. Belt-and-braces: scan for `claim` and `supports`.
        let obs = observed_obs(1);
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        let body = pkt.observations[0].as_object().unwrap();
        assert!(!body.contains_key("claim"));
        assert!(!body.contains_key("supports"));
    }

    #[test]
    fn observation_body_carries_no_verdict_shaped_fields() {
        // Per preflight §3: no `bloated`, `pinned`, `warn`, `critical`,
        // `unhealthy`, or similar in the observation body. The
        // observation describes what was observed; the evaluator
        // classifies. Belt-and-braces matching the dns_state pattern.
        let obs = observed_obs(1);
        let pkt = project_wal_observation(&obs, GENERATED_AT).unwrap();
        let body = pkt.observations[0].as_object().unwrap();
        for forbidden in [
            "bloated",
            "pinned",
            "warn",
            "critical",
            "unhealthy",
            "healthy",
            "pressured",
            "ok",
        ] {
            assert!(
                !body.contains_key(forbidden),
                "observation body must not contain verdict-shaped field {forbidden:?}"
            );
        }
    }
}
