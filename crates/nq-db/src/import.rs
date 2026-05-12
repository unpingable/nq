//! Inbound finding import — DURABLE_ARTIFACT_SUBSTRATE_GAP V1.
//!
//! Mirror of `export.rs` for the inbound direction. NQ accepts
//! `nq.finding_import.v1`-shaped JSON manifests from external producers
//! (synthetic in V1; real producers deferred) and writes the findings
//! into `warning_state` with `origin_source = 'import'`.
//!
//! ## Lifecycle posture: raw passthrough with origin tag
//!
//! V1 locks the raw-passthrough posture (gap doc §"Open questions" §1).
//! Ingested findings keep their producer-provided identity and clock;
//! the `origin` block on `FindingSnapshot` is the discriminator that
//! tells consumers to apply two-clock semantics. NQ does not re-emit
//! ingested findings as its own; it just stores them with provenance.
//!
//! ## Two-clock provenance
//!
//! - `producer_extraction_time` (RFC3339 UTC) — the producer's clock.
//!   Governs basis recency on window-bearing fields. Stored in
//!   `warning_state.origin_producer_extraction_time`.
//! - `first_seen_at` / `last_seen_at` (RFC3339 UTC) — NQ's clock.
//!   Governs lifecycle recency. Stored in the existing columns.
//!
//! ## Refusal mode
//!
//! A malformed, unversioned, or under-versioned manifest emits one
//! `inbound_export_unparsable` finding (origin=nq) and ingests zero
//! observations. It does not raise an error to the caller — the gap
//! doc invariant is "refusal does not fail the publish cycle."

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Wire shape ID for inbound manifests. Mirror of `SCHEMA_ID` on the
/// export side.
pub const IMPORT_SCHEMA_ID: &str = "nq.finding_import.v1";

/// Wire contract version. Bumps on breaking change; older versions
/// coexist via separate `IMPORT_SCHEMA_ID` values.
pub const IMPORT_CONTRACT_VERSION: u32 = 1;

/// Minimum DB schema version required to accept an inbound import.
/// Mirror of `MIN_SCHEMA_FOR_EXPORT`. V1 requires the `origin_*` and
/// `silence_*` columns added in migration 046.
pub const MIN_SCHEMA_FOR_IMPORT: u32 = 46;

/// Refusal finding kind emitted when an import is malformed,
/// unversioned, or under-versioned.
pub const REFUSAL_FINDING_KIND: &str = "inbound_export_unparsable";

/// `nq.finding_import.v1` manifest. Producer-emitted top-level shape.
///
/// Header fields ground the two-clock provenance: `producer_extraction_time`
/// is the producer's clock at extraction; `producer_id` and `extraction_run_id`
/// identify which producer and which extraction run this manifest belongs to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingImportManifest {
    pub schema: String,
    pub contract_version: u32,
    pub producer_id: String,
    pub extraction_run_id: String,
    pub producer_extraction_time: String,
    pub findings: Vec<ImportedFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedFinding {
    pub identity: ImportedFindingIdentity,
    pub severity: String,
    pub message: String,
    #[serde(default = "default_finding_class")]
    pub finding_class: String,
}

fn default_finding_class() -> String {
    "signal".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedFindingIdentity {
    /// V1: producer-shaped identifier in the host slot. Corpus-shaped
    /// subject-identity vocabulary is deferred to PROVENANCE_GRAPH_PROFILE
    /// per the gap doc. The host slot is reused here for storage compat;
    /// the `origin.producer_id` field is the authoritative producer
    /// identifier on the wire.
    pub host: String,
    pub detector: String,
    pub subject: String,
    pub rule_hash: Option<String>,
}

/// Outcome of an ingest attempt.
#[derive(Debug, Clone)]
pub struct IngestResult {
    /// Number of findings successfully written to `warning_state`.
    pub ingested_count: usize,
    /// `true` if the manifest was refused (malformed, wrong schema, or
    /// unsupported contract version). On refusal, exactly one
    /// `inbound_export_unparsable` finding was emitted.
    pub refused: bool,
    /// Human-readable reason when `refused = true`.
    pub refusal_reason: Option<String>,
    /// `true` if the `extraction_stale` detector fired during ingest
    /// (producer_extraction_time exceeded the configured threshold).
    pub extraction_stale_emitted: bool,
}

/// Configuration for the ingest path.
#[derive(Debug, Clone)]
pub struct IngestConfig {
    /// Threshold in seconds. If `producer_extraction_time` is older than
    /// `now - extraction_stale_threshold_s`, emit a SILENCE_UNIFICATION-
    /// shaped `extraction_stale` finding alongside the ingested findings.
    /// Default 86400 (24 hours) — durable-artifact extractions are
    /// typically daily-cadence; tune per profile.
    pub extraction_stale_threshold_s: i64,
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            extraction_stale_threshold_s: 86400,
        }
    }
}

/// Ingest a `nq.finding_import.v1` manifest.
///
/// Schema preflight: refuses if the DB is below `MIN_SCHEMA_FOR_IMPORT`.
/// Manifest preflight: refuses if the wire shape is wrong, contract
/// version unsupported, or required fields missing. Refusal emits one
/// `inbound_export_unparsable` finding and ingests zero findings.
///
/// On clean ingest: each manifest finding becomes a row in `warning_state`
/// with `origin_source = 'import'` and the producer-clock fields populated.
/// If `producer_extraction_time` exceeds the configured threshold against
/// `now_utc`, also emits one `extraction_stale` finding (origin=nq, the
/// SILENCE_UNIFICATION-shaped composition).
///
/// `current_generation` is NQ's current publish generation; lifecycle
/// fields ground here. `now_utc_rfc3339` is NQ's wall-clock time at
/// ingest, used for both lifecycle timestamps and the stale-threshold
/// comparison.
pub fn ingest_finding_import(
    conn: &Connection,
    manifest_json: &str,
    current_generation: i64,
    now_utc_rfc3339: &str,
    cfg: &IngestConfig,
) -> anyhow::Result<IngestResult> {
    // Schema preflight: refuse honestly if the DB is too old to carry the
    // origin_* / silence_* columns.
    let schema_version = crate::migrate::read_schema_version(conn).unwrap_or(0);
    if schema_version < MIN_SCHEMA_FOR_IMPORT {
        anyhow::bail!(
            "nq database schema version {} is below the minimum {} required by the \
             v1 finding import contract (nq.finding_import.v1). Migrate the database \
             before attempting inbound ingest.",
            schema_version,
            MIN_SCHEMA_FOR_IMPORT,
        );
    }

    // Parse the manifest. On failure, try a permissive partial-parse to
    // extract producer_id + extraction_run_id for the refusal finding.
    let parsed: Result<FindingImportManifest, _> = serde_json::from_str(manifest_json);
    let manifest = match parsed {
        Ok(m) => m,
        Err(e) => {
            let (pid, run_id) = best_effort_producer_identity(manifest_json);
            let reason = format!("manifest JSON parse failed: {}", e);
            emit_refusal_finding(conn, &pid, &run_id, &reason, current_generation, now_utc_rfc3339)?;
            return Ok(IngestResult {
                ingested_count: 0,
                refused: true,
                refusal_reason: Some(reason),
                extraction_stale_emitted: false,
            });
        }
    };

    // Wire-shape preflight.
    if manifest.schema != IMPORT_SCHEMA_ID {
        let reason = format!(
            "manifest schema is `{}`; expected `{}`",
            manifest.schema, IMPORT_SCHEMA_ID
        );
        emit_refusal_finding(
            conn,
            &manifest.producer_id,
            &manifest.extraction_run_id,
            &reason,
            current_generation,
            now_utc_rfc3339,
        )?;
        return Ok(IngestResult {
            ingested_count: 0,
            refused: true,
            refusal_reason: Some(reason),
            extraction_stale_emitted: false,
        });
    }
    if manifest.contract_version != IMPORT_CONTRACT_VERSION {
        let reason = format!(
            "manifest contract_version is {}; this binary supports {}",
            manifest.contract_version, IMPORT_CONTRACT_VERSION
        );
        emit_refusal_finding(
            conn,
            &manifest.producer_id,
            &manifest.extraction_run_id,
            &reason,
            current_generation,
            now_utc_rfc3339,
        )?;
        return Ok(IngestResult {
            ingested_count: 0,
            refused: true,
            refusal_reason: Some(reason),
            extraction_stale_emitted: false,
        });
    }

    // Ingest each finding with origin envelope populated.
    let mut ingested_count = 0usize;
    for f in &manifest.findings {
        insert_imported_finding(
            conn,
            &manifest,
            f,
            current_generation,
            now_utc_rfc3339,
        )?;
        ingested_count += 1;
    }

    // SILENCE_UNIFICATION composition: extraction_stale detector. The
    // ingest path itself is the detector — if the manifest's producer
    // extraction time is older than the threshold, emit one
    // SILENCE_UNIFICATION-shaped finding.
    let extraction_stale_emitted = maybe_emit_extraction_stale(
        conn,
        &manifest,
        current_generation,
        now_utc_rfc3339,
        cfg,
    )?;

    Ok(IngestResult {
        ingested_count,
        refused: false,
        refusal_reason: None,
        extraction_stale_emitted,
    })
}

/// Best-effort partial parse to extract producer_id + extraction_run_id
/// from a manifest whose full parse failed. Used to give the refusal
/// finding a meaningful subject when the parse error wasn't at the top
/// level.
fn best_effort_producer_identity(manifest_json: &str) -> (String, String) {
    #[derive(Deserialize)]
    struct PartialManifest {
        #[serde(default)]
        producer_id: Option<String>,
        #[serde(default)]
        extraction_run_id: Option<String>,
    }
    match serde_json::from_str::<PartialManifest>(manifest_json) {
        Ok(p) => (
            p.producer_id.unwrap_or_else(|| "unknown-producer".to_string()),
            p.extraction_run_id.unwrap_or_else(|| "unparseable".to_string()),
        ),
        Err(_) => (
            "unknown-producer".to_string(),
            "unparseable".to_string(),
        ),
    }
}

/// Emit one `inbound_export_unparsable` finding into `warning_state`.
/// origin_source = 'nq' (this is NQ's finding about the refusal, not the
/// producer's testimony).
fn emit_refusal_finding(
    conn: &Connection,
    producer_id: &str,
    extraction_run_id: &str,
    reason: &str,
    current_generation: i64,
    now_utc_rfc3339: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO warning_state (
            host, kind, subject, domain, message, severity,
            first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
            consecutive_gens, finding_class, absent_gens,
            visibility_state, basis_state, state_kind
         )
         VALUES (?1, ?2, ?3, '', ?4, 'warning',
                 ?5, ?6, ?5, ?6,
                 1, 'meta', 0,
                 'observed', 'unknown', 'incident')
         ON CONFLICT(host, kind, subject) DO UPDATE SET
             message       = excluded.message,
             last_seen_gen = excluded.last_seen_gen,
             last_seen_at  = excluded.last_seen_at,
             consecutive_gens = warning_state.consecutive_gens + 1",
        rusqlite::params![
            producer_id,
            REFUSAL_FINDING_KIND,
            extraction_run_id,
            reason,
            current_generation,
            now_utc_rfc3339,
        ],
    )?;
    Ok(())
}

/// Insert one ingested finding with the origin envelope populated.
/// origin_source = 'import'; producer-clock fields carry the manifest's
/// header values.
fn insert_imported_finding(
    conn: &Connection,
    manifest: &FindingImportManifest,
    f: &ImportedFinding,
    current_generation: i64,
    now_utc_rfc3339: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO warning_state (
            host, kind, subject, domain, message, severity,
            first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
            consecutive_gens, finding_class, rule_hash, absent_gens,
            visibility_state, basis_state, state_kind,
            origin_source, origin_producer_id, origin_extraction_run_id,
            origin_producer_extraction_time, origin_import_contract_version
         )
         VALUES (?1, ?2, ?3, '', ?4, ?5,
                 ?6, ?7, ?6, ?7,
                 1, ?8, ?9, 0,
                 'observed', 'unknown', 'incident',
                 'import', ?10, ?11, ?12, ?13)
         ON CONFLICT(host, kind, subject) DO UPDATE SET
             message       = excluded.message,
             severity      = excluded.severity,
             last_seen_gen = excluded.last_seen_gen,
             last_seen_at  = excluded.last_seen_at,
             consecutive_gens = warning_state.consecutive_gens + 1,
             origin_source = 'import',
             origin_producer_id              = excluded.origin_producer_id,
             origin_extraction_run_id        = excluded.origin_extraction_run_id,
             origin_producer_extraction_time = excluded.origin_producer_extraction_time,
             origin_import_contract_version  = excluded.origin_import_contract_version",
        rusqlite::params![
            f.identity.host,
            f.identity.detector,
            f.identity.subject,
            f.message,
            f.severity,
            current_generation,
            now_utc_rfc3339,
            f.finding_class,
            f.identity.rule_hash,
            manifest.producer_id,
            manifest.extraction_run_id,
            manifest.producer_extraction_time,
            manifest.contract_version as i64,
        ],
    )?;
    Ok(())
}

/// Emit `extraction_stale` if the manifest's producer_extraction_time is
/// older than the configured threshold. Returns whether the finding was
/// emitted.
///
/// **SILENCE_UNIFICATION composition.** This finding populates the
/// shared silence envelope (`silence_scope = 'extraction'`,
/// `silence_basis = 'age_threshold'`, `silence_duration_s = delta`,
/// `silence_expected = 'none'`). It is NQ's testimony about the
/// producer's silence — `origin_source = 'nq'` — not part of the
/// ingested findings.
fn maybe_emit_extraction_stale(
    conn: &Connection,
    manifest: &FindingImportManifest,
    current_generation: i64,
    now_utc_rfc3339: &str,
    cfg: &IngestConfig,
) -> anyhow::Result<bool> {
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    let prod_time = match OffsetDateTime::parse(&manifest.producer_extraction_time, &Rfc3339) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    let now = match OffsetDateTime::parse(now_utc_rfc3339, &Rfc3339) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    let delta_s = (now - prod_time).whole_seconds();
    if delta_s <= cfg.extraction_stale_threshold_s {
        return Ok(false);
    }

    conn.execute(
        "INSERT INTO warning_state (
            host, kind, subject, domain, message, severity,
            first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
            consecutive_gens, finding_class, absent_gens,
            visibility_state, basis_state, state_kind,
            silence_scope, silence_basis, silence_duration_s, silence_expected
         )
         VALUES (?1, 'extraction_stale', ?2, '', ?3, 'warning',
                 ?4, ?5, ?4, ?5,
                 1, 'meta', 0,
                 'observed', 'unknown', 'incident',
                 'extraction', 'age_threshold', ?6, 'none')
         ON CONFLICT(host, kind, subject) DO UPDATE SET
             message       = excluded.message,
             last_seen_gen = excluded.last_seen_gen,
             last_seen_at  = excluded.last_seen_at,
             consecutive_gens = warning_state.consecutive_gens + 1,
             silence_duration_s = excluded.silence_duration_s",
        rusqlite::params![
            manifest.producer_id,
            manifest.extraction_run_id,
            format!(
                "producer extraction last seen at {} ({} s ago); threshold {} s",
                manifest.producer_extraction_time, delta_s, cfg.extraction_stale_threshold_s
            ),
            current_generation,
            now_utc_rfc3339,
            delta_s,
        ],
    )?;
    Ok(true)
}
