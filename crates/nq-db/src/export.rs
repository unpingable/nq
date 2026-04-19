//! Finding export — canonical consumer-facing `FindingSnapshot` DTO.
//!
//! Per `docs/gaps/FINDING_EXPORT_GAP.md`. This is the typed, versioned
//! surface external consumers (Night Shift first, others later) use to
//! read NQ's finding state without coupling to internal schema.
//!
//! Invariants:
//!
//! 1. **NQ findings are evidence, not commands.** A `FindingSnapshot` is
//!    admissible evidence for downstream reconciliation, not an
//!    authorization token. Consumers must re-check current state
//!    before acting on a stale snapshot.
//!
//! 2. **Schema-versioned from day one.** Every emitted snapshot carries
//!    `schema: "nq.finding_snapshot.v1"` and `contract_version: 1`.
//!    Breaking changes bump the version and coexist with the old.
//!
//! 3. **Stable identity.** The primary key is `finding_key`, the
//!    URL-encoded `{scope}/{host}/{detector}/{subject}` already used
//!    internally. See `publish::compute_finding_key`.
//!
//! Divergences from the 2026-04-16 gap draft, documented here so the
//! drift is visible rather than silent:
//!
//! - `regime` includes `co_occurrence` and `resolution` payloads. The
//!   gap draft was written before those features landed (commits
//!   6f70f69 and 90a941d, both 2026-04-17). Exporting a regime
//!   summary that silently omits half the regime features would
//!   defeat the point of the contract.
//! - `lifecycle.condition_state` is derived on the fly from
//!   `consecutive_gens + absent_gens + visibility_state` rather than
//!   read from a dedicated column (which does not exist yet). The
//!   derivation is kept intentionally coarse — suppressed / clear /
//!   open — until the lifecycle machine tracks the finer pending_*
//!   states explicitly.

use crate::regime::{
    CoOccurrencePayload, PersistencePayload, RecoveryPayload, ResolutionPayload, TrajectoryPayload,
};
use crate::ReadDb;
use serde::Serialize;

pub const SCHEMA_ID: &str = "nq.finding_snapshot.v1";
pub const CONTRACT_VERSION: u32 = 1;

/// Minimum DB schema version the v1 export contract can read against.
/// Bumped when the export references a column added by a later migration.
/// Kept distinct from `CURRENT_SCHEMA_VERSION` so consumers can run
/// against a slightly-older DB as long as every column the exporter
/// touches is present.
///
/// v1 touches: `warning_state.absent_gens` (migration 020),
/// `warning_state.failure_class` etc. (027), `warning_state.stability`
/// (028), and `regime_features` (030). The most recent of those is
/// 30; exporter requires `>= 30`.
pub const MIN_SCHEMA_FOR_EXPORT: u32 = 30;

// ---------------------------------------------------------------------------
// Filter — what export_findings accepts.
// ---------------------------------------------------------------------------

/// Filter for `export_findings`. Construct via `ExportFilter::default()`
/// and mutate fields, or via the field-by-field `Default`.
#[derive(Debug, Clone, Default)]
pub struct ExportFilter {
    /// Only findings whose `last_seen_gen` exceeds this value are returned.
    pub changed_since_generation: Option<i64>,
    /// Restrict to a specific detector (e.g. "wal_bloat").
    pub detector: Option<String>,
    /// Restrict to a specific host.
    pub host: Option<String>,
    /// Exact match on the canonical `finding_key`. Wins over other filters.
    pub finding_key: Option<String>,
    /// Include cleared findings (`absent_gens > 0` with no streak). Default false.
    pub include_cleared: bool,
    /// Include findings with `visibility_state = 'suppressed'`. Default false.
    pub include_suppressed: bool,
    /// Maximum number of recent observations to embed per snapshot.
    /// Caller-chosen; the gap spec's CLI default is 10.
    pub observations_limit: usize,
}

// ---------------------------------------------------------------------------
// DTO structures — serialize to the canonical JSON shape.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct FindingSnapshot {
    pub schema: &'static str,
    pub contract_version: u32,
    pub finding_key: String,
    pub identity: FindingIdentity,
    pub lifecycle: FindingLifecycle,
    pub diagnosis: Option<FindingDiagnosisExport>,
    pub regime: FindingRegimeContext,
    pub observations: ObservationsSummary,
    pub generation: GenerationContext,
    pub export: ExportMetadata,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingIdentity {
    pub scope: String,
    pub host: String,
    pub detector: String,
    pub subject: String,
    pub rule_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingLifecycle {
    pub first_seen_gen: i64,
    pub first_seen_at: String,
    pub last_seen_gen: i64,
    pub last_seen_at: String,
    pub consecutive_gens: i64,
    pub absent_gens: i64,
    pub severity: String,
    pub visibility_state: String,
    pub condition_state: String,
    pub finding_class: String,
    pub stability: Option<String>,
    pub peak_value: Option<f64>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingDiagnosisExport {
    pub failure_class: String,
    pub service_impact: String,
    pub action_bias: String,
    pub synopsis: String,
    pub why_care: String,
}

/// Regime context for the finding. Each field is `None` when NQ has not
/// computed that regime feature for this subject. Null payloads are a
/// fact about the witness, not an error.
#[derive(Debug, Clone, Serialize, Default)]
pub struct FindingRegimeContext {
    pub trajectory: Option<TrajectoryPayload>,
    pub persistence: Option<PersistencePayload>,
    pub recovery: Option<RecoveryPayload>,
    pub co_occurrence: Option<CoOccurrencePayload>,
    pub resolution: Option<ResolutionPayload>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservationsSummary {
    pub total_count: i64,
    pub recent: Vec<ObservationRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservationRecord {
    pub generation_id: i64,
    pub observed_at: String,
    pub value: Option<f64>,
    pub severity: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenerationContext {
    pub generation_id: i64,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub status: Option<String>,
    pub sources_expected: Option<i64>,
    pub sources_ok: Option<i64>,
    pub sources_failed: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportMetadata {
    pub exported_at: String,
    pub changed_since: Option<i64>,
    pub source: &'static str,
    pub contract_version: u32,
}

// ---------------------------------------------------------------------------
// Derivation helpers.
// ---------------------------------------------------------------------------

/// Derive `condition_state` from stored lifecycle fields. Coarse by
/// design; see module-level doc for the rationale.
fn derive_condition_state(
    visibility_state: &str,
    consecutive_gens: i64,
    absent_gens: i64,
) -> &'static str {
    if visibility_state == "suppressed" {
        return "suppressed";
    }
    if consecutive_gens >= 1 && absent_gens == 0 {
        return "open";
    }
    if consecutive_gens == 0 && absent_gens >= 1 {
        return "clear";
    }
    // Both nonzero (finding recently present AND absent in window) —
    // the coarse mapping calls this "open" because the most-recent
    // observation wins; refine to pending_close when the lifecycle
    // machine tracks that state explicitly.
    "open"
}

// ---------------------------------------------------------------------------
// Primary entry point — query warning_state, fan out to regime + observations.
// ---------------------------------------------------------------------------

/// Export findings matching the filter. One query per finding for the
/// supporting regime + observation payloads — simple, correct for MVP
/// scale; a single-join query is a later optimization.
pub fn export_findings(
    db: &ReadDb,
    filter: &ExportFilter,
) -> anyhow::Result<Vec<FindingSnapshot>> {
    export_findings_from_conn(db.conn(), filter)
}

/// Variant that accepts a raw `Connection`, usable from either a
/// `ReadDb` or (for test fixtures) a `WriteDb`. See the `ReadDb`
/// wrapper above for the public entry point.
pub fn export_findings_from_conn(
    conn: &rusqlite::Connection,
    filter: &ExportFilter,
) -> anyhow::Result<Vec<FindingSnapshot>> {
    // Preflight: refuse to run against a DB whose schema predates the
    // export contract's requirements. A silent query against a missing
    // column would produce an opaque "no such column" error that tells
    // the consumer nothing about remediation. This check replaces that
    // with a specific, actionable message. First-contact scar from
    // nightshift Phase 1 consumer work (2026-04-18).
    let schema_version = crate::migrate::read_schema_version(conn).unwrap_or(0);
    if schema_version < MIN_SCHEMA_FOR_EXPORT {
        anyhow::bail!(
            "nq database schema version {} is below the minimum {} required by the \
             v1 finding export contract (nq.finding_snapshot.v1). Open this database \
             with a writable NQ binary to apply pending migrations (e.g. `nq publish` \
             or `nq serve` against this database path will migrate on startup). \
             Aborting export rather than producing partial or mis-shaped output.",
            schema_version,
            MIN_SCHEMA_FOR_EXPORT,
        );
    }

    let current_generation = conn
        .query_row(
            "SELECT COALESCE(MAX(generation_id), 0) FROM generations",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);

    let exported_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "".to_string());

    let gen_ctx = load_generation_context(conn, current_generation)?;

    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(fk) = &filter.finding_key {
        // finding_key filter wins over others; we still apply it via the
        // denormalized columns since warning_state does not store finding_key.
        let (scope, host, kind, subject) = parse_finding_key(fk)?;
        if scope != "local" {
            // v1 only knows local scope; federation is future work.
            return Ok(Vec::new());
        }
        clauses.push("host = ?".to_string());
        params.push(host.into());
        clauses.push("kind = ?".to_string());
        params.push(kind.into());
        clauses.push("subject = ?".to_string());
        params.push(subject.into());
    } else {
        if let Some(h) = &filter.host {
            clauses.push("host = ?".to_string());
            params.push(h.clone().into());
        }
        if let Some(d) = &filter.detector {
            clauses.push("kind = ?".to_string());
            params.push(d.clone().into());
        }
        if let Some(g) = filter.changed_since_generation {
            clauses.push("last_seen_gen > ?".to_string());
            params.push(g.into());
        }
        if !filter.include_suppressed {
            clauses.push("visibility_state != 'suppressed'".to_string());
        }
        if !filter.include_cleared {
            // Cleared means visibility 'stale' / absent_gens > 0 with no streak.
            // The strict filter: finding currently has a streak.
            clauses.push("consecutive_gens >= 1".to_string());
        }
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };

    let sql = format!(
        "SELECT host, kind, subject, severity, domain, message,
                first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                consecutive_gens, absent_gens, peak_value,
                visibility_state, finding_class, rule_hash, stability,
                failure_class, service_impact, action_bias, synopsis, why_care
         FROM warning_state{} ORDER BY host, kind, subject",
        where_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> =
        params.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(WarningStateRow {
            host: row.get(0)?,
            kind: row.get(1)?,
            subject: row.get(2)?,
            severity: row.get(3)?,
            domain: row.get(4)?,
            message: row.get(5)?,
            first_seen_gen: row.get(6)?,
            first_seen_at: row.get(7)?,
            last_seen_gen: row.get(8)?,
            last_seen_at: row.get(9)?,
            consecutive_gens: row.get(10)?,
            absent_gens: row.get::<_, Option<i64>>(11)?.unwrap_or(0),
            peak_value: row.get(12)?,
            visibility_state: row.get(13)?,
            finding_class: row.get(14)?,
            rule_hash: row.get(15)?,
            stability: row.get(16)?,
            failure_class: row.get(17)?,
            service_impact: row.get(18)?,
            action_bias: row.get(19)?,
            synopsis: row.get(20)?,
            why_care: row.get(21)?,
        })
    })?;

    let mut snapshots: Vec<FindingSnapshot> = Vec::new();
    for r in rows {
        let r = r?;
        let finding_key =
            crate::publish::compute_finding_key("local", &r.host, &r.kind, &r.subject);

        let diagnosis = match (
            r.failure_class.as_deref(),
            r.service_impact.as_deref(),
            r.action_bias.as_deref(),
            r.synopsis.as_deref(),
            r.why_care.as_deref(),
        ) {
            (Some(fc), Some(si), Some(ab), Some(syn), Some(why))
                if !fc.is_empty() && !syn.is_empty() =>
            {
                Some(FindingDiagnosisExport {
                    failure_class: fc.to_string(),
                    service_impact: si.to_string(),
                    action_bias: ab.to_string(),
                    synopsis: syn.to_string(),
                    why_care: why.to_string(),
                })
            }
            _ => None,
        };

        let regime = load_regime_context(conn, &finding_key, &r.host, &r.kind, &r.subject)?;
        let observations =
            load_observations_summary(conn, &finding_key, filter.observations_limit)?;

        let condition_state =
            derive_condition_state(&r.visibility_state, r.consecutive_gens, r.absent_gens)
                .to_string();

        snapshots.push(FindingSnapshot {
            schema: SCHEMA_ID,
            contract_version: CONTRACT_VERSION,
            finding_key,
            identity: FindingIdentity {
                scope: "local".to_string(),
                host: r.host,
                detector: r.kind,
                subject: r.subject,
                rule_hash: r.rule_hash,
            },
            lifecycle: FindingLifecycle {
                first_seen_gen: r.first_seen_gen,
                first_seen_at: r.first_seen_at,
                last_seen_gen: r.last_seen_gen,
                last_seen_at: r.last_seen_at,
                consecutive_gens: r.consecutive_gens,
                absent_gens: r.absent_gens,
                severity: r.severity,
                visibility_state: r.visibility_state,
                condition_state,
                finding_class: r.finding_class,
                stability: r.stability,
                peak_value: r.peak_value,
                message: r.message,
            },
            diagnosis,
            regime,
            observations,
            generation: gen_ctx.clone(),
            export: ExportMetadata {
                exported_at: exported_at.clone(),
                changed_since: filter.changed_since_generation,
                source: "nq",
                contract_version: CONTRACT_VERSION,
            },
        });
    }

    Ok(snapshots)
}

struct WarningStateRow {
    host: String,
    kind: String,
    subject: String,
    severity: String,
    #[allow(dead_code)]
    domain: String,
    message: String,
    first_seen_gen: i64,
    first_seen_at: String,
    last_seen_gen: i64,
    last_seen_at: String,
    consecutive_gens: i64,
    absent_gens: i64,
    peak_value: Option<f64>,
    visibility_state: String,
    finding_class: String,
    rule_hash: Option<String>,
    stability: Option<String>,
    failure_class: Option<String>,
    service_impact: Option<String>,
    action_bias: Option<String>,
    synopsis: Option<String>,
    why_care: Option<String>,
}

fn parse_finding_key(key: &str) -> anyhow::Result<(String, String, String, String)> {
    // compute_finding_key uses "{scope}/{enc(host)}/{enc(detector)}/{enc(subject)}"
    // Splitting on '/' is safe because each component is URL-encoded.
    let parts: Vec<&str> = key.splitn(4, '/').collect();
    if parts.len() != 4 {
        anyhow::bail!("finding_key must have four components, got {}", parts.len());
    }
    let decode = |s: &str| -> anyhow::Result<String> {
        // Simple percent-decoding — matches the encoding used by compute_finding_key.
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                let h = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .map_err(|_| anyhow::anyhow!("invalid percent-encoding"))?;
                let b = u8::from_str_radix(h, 16)
                    .map_err(|_| anyhow::anyhow!("invalid percent-encoding"))?;
                out.push(b);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        Ok(String::from_utf8(out)?)
    };
    Ok((
        parts[0].to_string(),
        decode(parts[1])?,
        decode(parts[2])?,
        decode(parts[3])?,
    ))
}

fn load_generation_context(
    conn: &rusqlite::Connection,
    generation_id: i64,
) -> anyhow::Result<GenerationContext> {
    if generation_id == 0 {
        return Ok(GenerationContext {
            generation_id: 0,
            started_at: None,
            completed_at: None,
            status: None,
            sources_expected: None,
            sources_ok: None,
            sources_failed: None,
        });
    }
    let row = conn
        .query_row(
            "SELECT started_at, completed_at, status, sources_expected, sources_ok, sources_failed
             FROM generations WHERE generation_id = ?1",
            rusqlite::params![generation_id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .ok();
    let (started_at, completed_at, status, sources_expected, sources_ok, sources_failed) =
        row.unwrap_or((None, None, None, None, None, None));
    Ok(GenerationContext {
        generation_id,
        started_at,
        completed_at,
        status,
        sources_expected,
        sources_ok,
        sources_failed,
    })
}

fn load_regime_context(
    conn: &rusqlite::Connection,
    finding_key: &str,
    host: &str,
    kind: &str,
    subject: &str,
) -> anyhow::Result<FindingRegimeContext> {
    let persistence = load_regime_payload::<PersistencePayload>(
        conn,
        "finding",
        finding_key,
        "persistence",
    )?;
    let recovery =
        load_regime_payload::<RecoveryPayload>(conn, "finding", finding_key, "recovery")?;
    let co_occurrence = load_regime_payload::<CoOccurrencePayload>(
        conn,
        "host",
        host,
        "co_occurrence",
    )?;

    // Trajectory + resolution are per-host-metric. Only attach if the
    // detector subject IS a recognised host pressure metric — for
    // generic findings (wal_bloat on a file path, etc.), these stay None
    // so the consumer is not misled by an unrelated metric's regime.
    let (trajectory, resolution) = match metric_for_kind_subject(kind, subject) {
        Some(metric) => {
            let subject_id = format!("{host}/{metric}");
            let t = load_regime_payload::<TrajectoryPayload>(
                conn,
                "host_metric",
                &subject_id,
                "trajectory",
            )?;
            let r = load_regime_payload::<ResolutionPayload>(
                conn,
                "host_metric",
                &subject_id,
                "resolution",
            )?;
            (t, r)
        }
        None => (None, None),
    };

    Ok(FindingRegimeContext {
        trajectory,
        persistence,
        recovery,
        co_occurrence,
        resolution,
    })
}

fn load_regime_payload<T: serde::de::DeserializeOwned>(
    conn: &rusqlite::Connection,
    subject_kind: &str,
    subject_id: &str,
    feature_type: &str,
) -> anyhow::Result<Option<T>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT payload_json FROM regime_features
             WHERE subject_kind = ?1 AND subject_id = ?2 AND feature_type = ?3
             ORDER BY generation_id DESC LIMIT 1",
            rusqlite::params![subject_kind, subject_id, feature_type],
            |row| row.get::<_, String>(0),
        )
        .ok();
    Ok(json.and_then(|s| serde_json::from_str(&s).ok()))
}

/// Map a (detector, subject) pair to the host metric name it corresponds
/// to, if any. Conservative: returns `Some` only for detectors whose
/// subject is definitionally the metric itself. Extend as new
/// metric-scoped detectors land.
fn metric_for_kind_subject(kind: &str, _subject: &str) -> Option<&'static str> {
    match kind {
        "disk_pressure" => Some("disk_used_pct"),
        "mem_pressure" => Some("mem_pressure_pct"),
        _ => None,
    }
}

fn load_observations_summary(
    conn: &rusqlite::Connection,
    finding_key: &str,
    limit: usize,
) -> anyhow::Result<ObservationsSummary> {
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM finding_observations WHERE finding_key = ?1",
            rusqlite::params![finding_key],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let recent = if limit == 0 {
        Vec::new()
    } else {
        let mut stmt = conn.prepare(
            "SELECT generation_id, observed_at, value, severity, message
             FROM finding_observations
             WHERE finding_key = ?1
             ORDER BY generation_id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![finding_key, limit as i64], |row| {
            Ok(ObservationRecord {
                generation_id: row.get(0)?,
                observed_at: row.get(1)?,
                value: row.get(2)?,
                severity: row.get(3)?,
                message: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    Ok(ObservationsSummary {
        total_count: total,
        recent,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};

    fn make_db() -> crate::WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        db
    }

    fn ensure_generation(db: &crate::WriteDb, gen_id: i64) {
        db.conn.execute(
            "INSERT OR IGNORE INTO generations
               (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES (?1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'complete', 1, 1, 0, 0)",
            rusqlite::params![gen_id],
        ).unwrap();
    }

    fn insert_warning_state(
        db: &crate::WriteDb,
        host: &str,
        kind: &str,
        subject: &str,
        streak: i64,
    ) {
        db.conn.execute(
            "INSERT INTO warning_state
               (host, kind, subject, domain, message, severity,
                first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                consecutive_gens, finding_class, absent_gens, visibility_state,
                failure_class, service_impact, action_bias, synopsis, why_care)
             VALUES (?1, ?2, ?3, 'Δg', 'test', 'warning', 1, '2026-01-01', 100, '2026-01-01',
                     ?4, 'signal', 0, 'observed', 'Accumulation', 'NoneCurrent',
                     'InvestigateBusinessHours', 'test synopsis', 'test why_care')",
            rusqlite::params![host, kind, subject, streak],
        ).unwrap();
    }

    #[test]
    fn snapshot_has_schema_and_contract_version() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);

        let filter = ExportFilter {
            observations_limit: 5,
            ..Default::default()
        };
        let snapshots = export_findings_from_conn(&db.conn, &filter).unwrap();
        assert_eq!(snapshots.len(), 1);
        let s = &snapshots[0];
        assert_eq!(s.schema, "nq.finding_snapshot.v1");
        assert_eq!(s.contract_version, 1);
        assert_eq!(s.export.source, "nq");
    }

    #[test]
    fn finding_key_matches_compute_finding_key() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/opt/db", 10);

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        let expected = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/opt/db");
        assert_eq!(snapshots[0].finding_key, expected);
    }

    #[test]
    fn unicode_and_special_chars_round_trip() {
        let db = make_db();
        ensure_generation(&db, 100);
        let subject = "/data/café ☕.sqlite";
        insert_warning_state(&db, "host-1", "wal_bloat", subject, 10);

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].identity.subject, subject);

        // JSON round-trip preserves the identity.
        let json = serde_json::to_string(&snapshots[0]).unwrap();
        let back: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(back["identity"]["subject"].as_str().unwrap(), subject);
    }

    #[test]
    fn include_suppressed_default_excludes() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);
        db.conn
            .execute(
                "UPDATE warning_state SET visibility_state = 'suppressed' WHERE kind = 'wal_bloat'",
                [],
            )
            .unwrap();

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 0, "suppressed excluded by default");

        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                include_suppressed: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].lifecycle.condition_state, "suppressed");
    }

    #[test]
    fn include_cleared_default_excludes() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 0);
        // consecutive_gens = 0 means cleared.
        db.conn
            .execute(
                "UPDATE warning_state SET absent_gens = 5 WHERE kind = 'wal_bloat'",
                [],
            )
            .unwrap();

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 0);

        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                include_cleared: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].lifecycle.condition_state, "clear");
    }

    #[test]
    fn changed_since_filters_by_last_seen_gen() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);
        db.conn
            .execute("UPDATE warning_state SET last_seen_gen = 50", [])
            .unwrap();

        // Before the last_seen_gen — included.
        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                changed_since_generation: Some(40),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 1);

        // After the last_seen_gen — excluded.
        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                changed_since_generation: Some(50),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn host_and_detector_filters() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/a", 5);
        insert_warning_state(&db, "host-2", "wal_bloat", "/b", 5);
        insert_warning_state(&db, "host-1", "disk_pressure", "", 5);

        let by_host = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                host: Some("host-1".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_host.len(), 2);

        let by_detector = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                detector: Some("wal_bloat".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_detector.len(), 2);

        let both = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                host: Some("host-1".to_string()),
                detector: Some("wal_bloat".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(both.len(), 1);
    }

    #[test]
    fn finding_key_filter_returns_exactly_one() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/a", 5);
        insert_warning_state(&db, "host-2", "wal_bloat", "/b", 5);
        let fk = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/a");

        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                finding_key: Some(fk.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].finding_key, fk);
    }

    #[test]
    fn finding_key_filter_empty_when_no_match_is_not_error() {
        let db = make_db();
        ensure_generation(&db, 100);
        let fk = crate::publish::compute_finding_key("local", "nonesuch", "wal_bloat", "/a");

        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                finding_key: Some(fk),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn observations_limit_is_respected() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);
        let fk = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");
        for g in 1..=20 {
            ensure_generation(&db, g);
            db.conn
                .execute(
                    "INSERT INTO finding_observations
                       (generation_id, finding_key, scope, detector_id, host, subject, domain, finding_class, observed_at, value)
                     VALUES (?1, ?2, 'local', 'wal_bloat', 'host-1', '/db', 'Δg', 'signal', '2026-01-01T00:00:00Z', ?3)",
                    rusqlite::params![g, &fk, g as f64 * 10.0],
                )
                .unwrap();
        }

        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                observations_limit: 5,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].observations.total_count, 20);
        assert_eq!(snapshots[0].observations.recent.len(), 5);
        // Most-recent-first ordering preserved.
        assert_eq!(snapshots[0].observations.recent[0].generation_id, 20);
    }

    #[test]
    fn diagnosis_populates_when_present() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 5);

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        let d = snapshots[0].diagnosis.as_ref().expect("diagnosis populated");
        assert_eq!(d.failure_class, "Accumulation");
        assert_eq!(d.synopsis, "test synopsis");
    }

    #[test]
    fn diagnosis_none_when_fields_missing() {
        let db = make_db();
        ensure_generation(&db, 100);
        // Insert without diagnosis fields.
        db.conn
            .execute(
                "INSERT INTO warning_state
                   (host, kind, subject, domain, message, severity,
                    first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                    consecutive_gens, finding_class, absent_gens, visibility_state)
                 VALUES ('host-1', 'legacy_kind', '', 'Δg', 'no diagnosis', 'warning',
                         1, '2026-01-01', 100, '2026-01-01', 5, 'signal', 0, 'observed')",
                [],
            )
            .unwrap();

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert!(snapshots[0].diagnosis.is_none());
    }

    #[test]
    fn regime_is_none_when_no_features_computed() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 5);

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        let r = &snapshots[0].regime;
        assert!(r.persistence.is_none());
        assert!(r.recovery.is_none());
        assert!(r.trajectory.is_none());
        assert!(r.co_occurrence.is_none());
        assert!(r.resolution.is_none());
    }

    #[test]
    fn parse_finding_key_roundtrips_unicode() {
        let fk = crate::publish::compute_finding_key("local", "host", "wal_bloat", "/a/café");
        let (scope, host, detector, subject) = parse_finding_key(&fk).unwrap();
        assert_eq!(scope, "local");
        assert_eq!(host, "host");
        assert_eq!(detector, "wal_bloat");
        assert_eq!(subject, "/a/café");
    }

    #[test]
    fn empty_db_is_not_an_error() {
        let db = make_db();
        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn condition_state_derivation() {
        assert_eq!(derive_condition_state("observed", 5, 0), "open");
        assert_eq!(derive_condition_state("observed", 0, 3), "clear");
        assert_eq!(derive_condition_state("suppressed", 5, 0), "suppressed");
        assert_eq!(derive_condition_state("observed", 5, 3), "open");
    }

    // ------------------------------------------------------------------
    // Regression guard: upstream DB shape not ready.
    //
    // First-contact scar from nightshift Phase 1 (2026-04-18). The
    // exporter must refuse, with a specific and actionable error, to
    // run against a DB whose schema predates MIN_SCHEMA_FOR_EXPORT.
    // Opaque "no such column" SQL errors tell a consumer nothing.
    // ------------------------------------------------------------------

    fn make_unmigrated_db() -> crate::WriteDb {
        // Open a fresh SQLite file without calling migrate(). user_version
        // is 0. Any table the export touches will be missing, so the
        // preflight must intercept BEFORE any query executes.
        crate::open_rw(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn export_refuses_when_schema_version_below_minimum() {
        let db = make_unmigrated_db();
        let err = export_findings_from_conn(&db.conn, &ExportFilter::default())
            .expect_err("unmigrated DB must error");
        let msg = err.to_string();
        assert!(
            msg.contains("schema version"),
            "error message must name the schema-version problem: {msg}"
        );
        assert!(
            msg.contains("finding export contract"),
            "error message must name the contract: {msg}"
        );
        assert!(
            msg.contains("migration"),
            "error message must suggest remediation via migration: {msg}"
        );
    }

    #[test]
    fn export_works_after_migration() {
        // Paired positive case: same DB, after migrate(), succeeds cleanly.
        // Proves the preflight is the gate, not an always-fail.
        let db = make_db(); // make_db calls migrate() internally
        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn preflight_reads_user_version_correctly() {
        // Direct check of the helper that backs the preflight.
        let db = make_unmigrated_db();
        let v = crate::migrate::read_schema_version(&db.conn).unwrap();
        assert_eq!(v, 0, "fresh unmigrated DB has user_version 0");

        let db = make_db();
        let v = crate::migrate::read_schema_version(&db.conn).unwrap();
        assert_eq!(v, crate::CURRENT_SCHEMA_VERSION);
    }
}
