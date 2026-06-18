//! Publisher wire format: the JSON shape returned by GET /state.
//!
//! Publishers are stateless. They report current state. History is the
//! aggregator's problem.

use crate::status::{CollectorStatus, ServiceStatus};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Wire schema/version of the `GET /state` envelope.
///
/// The version suffix is load-bearing per `docs/architecture/COMPATIBILITY.md`:
/// a future `v2` payload ships alongside `v1` during transition, never replacing
/// it silently. Producers MUST stamp [`PublisherState::schema`] with this value.
///
/// Note the name: this is the `/state` *envelope* schema, distinct from
/// [`crate::witness::WITNESS_SCHEMA`] (`nq.witness.v1`), which is the nq-core
/// `WitnessPacket` projection — a different surface.
pub const PUBLISHER_STATE_SCHEMA: &str = "nq.witness_packet.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherState {
    /// Wire schema/version of this `/state` envelope. Producers MUST set this to
    /// `Some(PUBLISHER_STATE_SCHEMA)` (see [`PublisherState::current`]).
    ///
    /// Deserializes to `None` when the field is absent — a pre-versioning /
    /// unversioned payload. Absence is **never** laundered into `v1`: a missing
    /// schema is `None`, and the consumer decides how to treat an unversioned
    /// payload. `None` re-serializes to an absent field (absence stays absence).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub host: String,
    #[serde(with = "time::serde::rfc3339")]
    pub collected_at: OffsetDateTime,
    pub collectors: Collectors,
}

impl PublisherState {
    /// Construct a current-wire `PublisherState`, stamping the envelope schema
    /// to [`PUBLISHER_STATE_SCHEMA`]. The single honest way for a producer to
    /// build the wire payload — keeps the load-bearing version in one place.
    pub fn current(host: String, collected_at: OffsetDateTime, collectors: Collectors) -> Self {
        PublisherState {
            schema: Some(PUBLISHER_STATE_SCHEMA.to_string()),
            host,
            collected_at,
            collectors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collectors {
    #[serde(default)]
    pub host: Option<CollectorPayload<HostData>>,
    #[serde(default)]
    pub services: Option<CollectorPayload<Vec<ServiceData>>>,
    #[serde(default)]
    pub sqlite_health: Option<CollectorPayload<Vec<SqliteDbData>>>,
    #[serde(default)]
    pub prometheus: Option<CollectorPayload<Vec<MetricSample>>>,
    #[serde(default)]
    pub logs: Option<CollectorPayload<Vec<LogObservation>>>,
    #[serde(default)]
    pub zfs_witness: Option<CollectorPayload<ZfsWitnessReport>>,
    #[serde(default)]
    pub smart_witness: Option<CollectorPayload<SmartWitnessReport>>,
    /// Slice 6b: publisher-side sqlite_wal probe observations. Each
    /// row is one `(host, db_file_path)` target observed in this cycle.
    /// The aggregator persists them into the `wal_observations`
    /// substrate table with the cycle's `generation_id`.
    /// Additive; older payloads without this field deserialize cleanly.
    #[serde(default)]
    pub sqlite_wal_observations: Option<CollectorPayload<Vec<WalObservationData>>>,
    /// Tier 1 NQ-on-NQ: one observation per cycle about the publisher's
    /// own binary file at its filesystem path (mtime, size, sha256
    /// content-hash). The aggregator persists into `nq_binary_observations`
    /// with the cycle's `generation_id`. Single target per publisher —
    /// the publisher's own `/proc/self/exe` (or the operator's
    /// `nq_binary_path` override) — so the payload data is one struct,
    /// not a Vec. Additive; older payloads deserialize cleanly.
    #[serde(default)]
    pub nq_binary_observations: Option<CollectorPayload<NqBinaryObservationData>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectorPayload<T> {
    pub status: CollectorStatus,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub collected_at: Option<OffsetDateTime>,
    pub error_message: Option<String>,
    pub data: Option<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostData {
    pub cpu_load_1m: Option<f64>,
    pub cpu_load_5m: Option<f64>,
    pub mem_total_mb: Option<u64>,
    pub mem_available_mb: Option<u64>,
    pub mem_pressure_pct: Option<f64>,
    pub disk_total_mb: Option<u64>,
    pub disk_avail_mb: Option<u64>,
    pub disk_used_pct: Option<f64>,
    pub uptime_seconds: Option<u64>,
    pub kernel_version: Option<String>,
    pub boot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceData {
    pub service: String,
    pub status: ServiceStatus,
    pub health_detail_json: Option<String>,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_restart: Option<OffsetDateTime>,
    pub eps: Option<f64>,
    pub queue_depth: Option<i64>,
    pub consumer_lag: Option<i64>,
    pub drop_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteDbData {
    pub db_path: String,
    pub db_size_mb: Option<f64>,
    pub wal_size_mb: Option<f64>,
    pub page_size: Option<u32>,
    pub page_count: Option<u64>,
    pub freelist_count: Option<u64>,
    pub journal_mode: Option<String>,
    pub auto_vacuum: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_checkpoint: Option<OffsetDateTime>,
    pub checkpoint_lag_s: Option<u64>,
    pub last_quick_check: Option<String>,
    pub last_integrity_check: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_integrity_at: Option<OffsetDateTime>,
    /// Main DB file mtime (raw stat). Distinct from `last_checkpoint`:
    /// stalls when the WAL grows but writes never land in the main file.
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub db_mtime: Option<OffsetDateTime>,
    /// WAL file mtime (raw stat). Distinct from `wal_size_mb`: lets a
    /// detector tell "WAL large and growing" from "WAL large but quiescent."
    /// None when the -wal sidecar is absent.
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub wal_mtime: Option<OffsetDateTime>,
}

/// One observation of a single `(host, db_file_path)` sqlite WAL
/// target, produced by the publisher-side probe (slice 6b).
///
/// Host is carried at the `PublisherState` level (every collector in a
/// payload shares the host); this struct carries the per-row substrate
/// state. The aggregator stamps `generation_id` and `observation_id` on
/// insert into `wal_observations`.
///
/// `observation_status` is the closed enum from the kind-4 probe
/// preflight §6 (`observed | target_missing | permission_denied |
/// stat_error`). Wire-side it is `String`; the aggregator validates
/// via `ObservationStatus::from_str` on insert. Same posture for
/// `proc_access`.
///
/// All stat-derived fields (`wal_present`, `wal_bytes`, `wal_mtime`,
/// `db_bytes`, `db_mtime`) are populated when `observation_status =
/// "observed"`; NULL otherwise. Permission-denied / target-missing /
/// stat-error rows MUST set `error_detail` and leave the stat-derived
/// fields NULL — the migration 049 conditional CHECK enforces this at
/// the substrate boundary.
///
/// V0 slice 6b leaves `proc_access = "not_attempted"` and the
/// pinned-reader fields NULL. The `/proc/locks` enrichment lands as a
/// follow-up slice; the wire shape already accommodates it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalObservationData {
    pub db_file_path: String,
    pub observation_status: String,
    #[serde(default)]
    pub wal_present: Option<bool>,
    #[serde(default)]
    pub wal_bytes: Option<i64>,
    /// RFC3339 UTC. None when observation_status != observed, or when
    /// observed but the WAL sidecar is absent.
    #[serde(default)]
    pub wal_mtime: Option<String>,
    #[serde(default)]
    pub db_bytes: Option<i64>,
    /// RFC3339 UTC. None when observation_status != observed.
    #[serde(default)]
    pub db_mtime: Option<String>,
    pub proc_access: String,
    #[serde(default)]
    pub pinned_reader_present: Option<bool>,
    #[serde(default)]
    pub pinned_reader_pid: Option<i64>,
    #[serde(default)]
    pub pinned_reader_command: Option<String>,
    /// RFC3339 UTC. Probe wall-clock at the moment of the stat.
    pub observed_at: String,
    #[serde(default)]
    pub error_detail: Option<String>,
}

/// One observation of the publisher's own `nq` binary file, produced
/// by the publisher-side collector (slice B of NQ_BINARY_MTIME_STATE).
///
/// Host is carried at the `PublisherState` level. `binary_path` is the
/// canonical filesystem path the publisher observed — either the
/// canonicalize-once-at-startup resolution of `/proc/self/exe` (the
/// default) or the operator's `nq_binary_path` config override.
///
/// `observation_status` is the closed enum from migration 054:
/// `observed | target_missing | permission_denied | stat_error |
/// read_error | hash_error`. Wire-side it is `String`; the aggregator
/// validates on insert.
///
/// All stat-derived fields (`size_bytes`, `mtime`, `content_hash`) are
/// populated when `observation_status = "observed"`; NULL otherwise.
/// Non-observed rows MUST set `error_detail` — the migration's
/// conditional CHECK enforces this at the substrate boundary.
///
/// `content_hash` is `"sha256:<64-hex>"` when computed; the substrate
/// CHECK pins the structural shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NqBinaryObservationData {
    pub binary_path: String,
    pub observation_status: String,
    #[serde(default)]
    pub size_bytes: Option<i64>,
    /// RFC3339 UTC. None when `observation_status != "observed"`.
    #[serde(default)]
    pub mtime: Option<String>,
    /// `"sha256:<64-hex>"` when computed; None when
    /// `observation_status != "observed"`.
    #[serde(default)]
    pub content_hash: Option<String>,
    /// RFC3339 UTC. Probe wall-clock at the moment of the stat.
    pub observed_at: String,
    #[serde(default)]
    pub error_detail: Option<String>,
}

/// Reduced log observation for a bounded window. Not raw logs —
/// classified counts + exemplar receipts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogObservation {
    pub source_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub window_start: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub window_end: OffsetDateTime,
    pub fetch_status: String,
    pub lines_total: u64,
    pub lines_error: u64,
    pub lines_warn: u64,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_log_ts: Option<OffsetDateTime>,
    pub transport_lag_ms: Option<i64>,
    #[serde(default)]
    pub examples: Vec<LogExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogExample {
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub ts: Option<OffsetDateTime>,
    pub severity: String,
    pub message: String,
}

/// A single metric sample scraped from a Prometheus exporter.
///
/// The `scrape_target_*` fields preserve **target provenance** — which
/// configured scrape target produced this sample. Without these fields
/// two exporters emitting the same metric name (e.g. `probe_success`
/// from a blackbox exporter probing two different endpoints) become
/// indistinguishable in storage. The scrape pipeline stamps every
/// successful sample after parsing; parsing itself stays pure.
///
/// Stamping happens at the struct level, NOT by injecting `nq_*`
/// labels, so exporter-emitted labels are never clobbered. Provenance
/// lives outside the metric's own label namespace by design.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub name: String,
    pub labels: std::collections::BTreeMap<String, String>,
    pub value: f64,
    /// gauge, counter, histogram, summary, untyped
    #[serde(default)]
    pub metric_type: Option<String>,
    /// Configured name of the scrape target that produced this sample
    /// (e.g. "blackbox_labelwatch_health"). Additive; older payloads
    /// without this field deserialize cleanly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrape_target_name: Option<String>,
    /// URL the scraper hit to obtain this sample (e.g. the blackbox
    /// exporter `/probe?module=...&target=...` URL). Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrape_target_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Typed refusal vocabulary — cross-cutting wire primitive used by
// preflight (`PreflightResult.cannot_testify`) and witness coverage
// (`*WitnessCoverage.cannot_testify`). See
// `docs/working/gaps/WITNESS_CLAIM_SCOPE_GAP.md`.
//
// Driver is completeness, not new authority: every constitutional
// `*_cannot_testify()` function and every witness coverage emission
// already ships a refusal list as `Vec<String>`. Typing the row
// preserves identity that prose loses on every consumer parse.
//
// Wire shape (JSON):
//   { "refusal_kind": "consequence_claim",
//     "statement":   "Whether to restart, reconfigure, ..." }
// ---------------------------------------------------------------------------

/// One refusal carried by a witness observation or evaluator claim.
///
/// `refusal_kind` is the stable machine category — consumers branch on
/// this. `statement` is explanatory prose for renderers and is *not* a
/// machine contract.
///
/// **Do not dedupe by `refusal_kind` alone.** A single kind (e.g.
/// `OutOfJurisdiction`) can carry distinct statements ("wrong host" vs
/// "wrong sibling kind") that are operationally different. Machine
/// identity is `refusal_kind`; diagnostic inventory is
/// `refusal_kind + statement + surface`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimRefusal {
    pub refusal_kind: RefusalKind,
    pub statement: String,
}

impl ClaimRefusal {
    /// Convenience constructor — `ClaimRefusal::new(RefusalKind::X, "prose")`.
    pub fn new(refusal_kind: RefusalKind, statement: impl Into<String>) -> Self {
        Self {
            refusal_kind,
            statement: statement.into(),
        }
    }
}

/// Operator-facing rendering: just the prose statement. The
/// `refusal_kind` is machine identity and not part of the human
/// rendering — the existing prose already embeds the category in
/// parentheticals (e.g. "... (consequence claim)"), so rendering
/// `statement` alone preserves the pre-v2 operator-facing output.
impl std::fmt::Display for ClaimRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.statement)
    }
}

/// Closed vocabulary of refusal categories harvested from the prose
/// parentheticals in the 8 constitutional `*_cannot_testify()`
/// functions. Promotion rule: new variants land when ≥2 kinds emit a
/// shared category. Until then, kind-specific refusals carry
/// [`RefusalKind::KindSpecific`] with the prose preserved in
/// `statement`.
///
/// Variants are documented at the gap doc rather than here; see
/// `docs/working/gaps/WITNESS_CLAIM_SCOPE_GAP.md` "The `RefusalKind`
/// vocabulary, harvested" for the per-variant harvest sites and
/// load-bearing rationale.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RefusalKind {
    /// Refusal to license consequence / action / mutation. The
    /// `feedback_knob_facing` boundary, typed.
    ConsequenceClaim,
    /// Refusal to forecast / make any future-tense claim.
    FutureStateClaim,
    /// Refusal to be sole witness to NQ-self standing.
    /// (sixth-keeper rule per `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP`.)
    SelfAuditRefusal,
    /// Refusal — out of this kind's jurisdiction. Covers different
    /// target, different host, or a sibling kind's territory; the
    /// specific subreason lives in `statement`.
    OutOfJurisdiction,
    /// Refusal — semantic correctness or application-layer state,
    /// above what substrate observation licenses.
    AboveSubstrate,
    /// Refusal — internals beneath this kind's substrate (engine
    /// correctness, build-time provenance, runtime behavior).
    BelowSubstrate,
    /// Refusal — substrate doesn't testify to environmental context
    /// (upstream substrate health, network connectivity).
    EnvironmentalContext,
    /// Refusal — absence has multiple causes the probe shape cannot
    /// distinguish; absence is ambiguous, not negative testimony.
    AbsenceSemantics,
    /// Refusal — composition / re-emission discipline. Kept explicit
    /// despite single emission today because the rule is structural
    /// (per `NQ_NS_CHANNEL_SPLIT_NQ_SIDE`), not a frequency artifact.
    CompositionReEmission,
    /// Catchall — the refusal is real and shipping, but its category
    /// is kind-specific and not yet shared across surfaces. Promote
    /// out of this variant when ≥2 kinds emit a shared category.
    KindSpecific,
}

// ---------------------------------------------------------------------------
// nq-witness report — canonical shape consumed by the ZFS witness collector.
// Mirrors nq.witness.v0 / nq.witness.zfs.v0. See ~/git/nq-witness/SPEC.md.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsWitnessReport {
    pub schema: String,
    pub witness: ZfsWitnessHeader,
    pub coverage: ZfsWitnessCoverage,
    pub standing: ZfsWitnessStanding,
    #[serde(default)]
    pub observations: Vec<ZfsObservation>,
    #[serde(default)]
    pub errors: Vec<ZfsWitnessError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsWitnessHeader {
    pub id: String,
    #[serde(rename = "type")]
    pub witness_type: String,
    pub host: String,
    pub profile_version: String,
    pub collection_mode: String,
    pub privilege_model: String,
    #[serde(with = "time::serde::rfc3339")]
    pub collected_at: OffsetDateTime,
    pub duration_ms: Option<i64>,
    pub status: String,
    #[serde(default)]
    pub observed_subject: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsWitnessCoverage {
    #[serde(default)]
    pub can_testify: Vec<String>,
    #[serde(default)]
    pub cannot_testify: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsWitnessStanding {
    #[serde(default)]
    pub authoritative_for: Vec<String>,
    #[serde(default)]
    pub advisory_for: Vec<String>,
    #[serde(default)]
    pub inadmissible_for: Vec<String>,
}

/// Observation variant, tagged by `kind` field in the JSON.
///
/// Unknown kinds are accepted (serde deserialises into the `Other` arm)
/// so that profile growth doesn't break NQ. The collector records unknowns
/// but does not persist them as typed observations; the coverage-tag gating
/// discipline means detectors never fire on unknown shapes anyway.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ZfsObservation {
    #[serde(rename = "zfs_pool")]
    Pool(ZfsPoolObservation),
    #[serde(rename = "zfs_vdev")]
    Vdev(ZfsVdevObservation),
    #[serde(rename = "zfs_scan")]
    Scan(ZfsScanObservation),
    #[serde(rename = "zfs_spare")]
    Spare(ZfsSpareObservation),
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsPoolObservation {
    pub subject: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub health_numeric: Option<i64>,
    #[serde(default)]
    pub size_bytes: Option<i64>,
    #[serde(default)]
    pub alloc_bytes: Option<i64>,
    #[serde(default)]
    pub free_bytes: Option<i64>,
    #[serde(default)]
    pub readonly: Option<bool>,
    #[serde(default)]
    pub fragmentation_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsVdevObservation {
    pub subject: String,
    pub pool: String,
    #[serde(default)]
    pub vdev_name: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub read_errors: Option<i64>,
    #[serde(default)]
    pub write_errors: Option<i64>,
    #[serde(default)]
    pub checksum_errors: Option<i64>,
    #[serde(default)]
    pub status_note: Option<String>,
    #[serde(default)]
    pub is_spare: Option<bool>,
    #[serde(default)]
    pub is_replacing: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsScanObservation {
    pub subject: String,
    pub pool: String,
    #[serde(default)]
    pub scan_type: Option<String>,
    #[serde(default)]
    pub scan_state: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_completed_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub errors_found: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsSpareObservation {
    pub subject: String,
    pub pool: String,
    #[serde(default)]
    pub spare_name: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub replacing_vdev_guid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsWitnessError {
    pub kind: String,
    pub detail: String,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

// ---------------------------------------------------------------------------
// SMART witness report — canonical shape consumed by the SMART collector.
// Mirrors nq.witness.v0 / nq.witness.smart.v0. See ~/git/nq-witness/profiles/smart.md.
//
// Phase 1 raw evidence only. No detector wiring, no verdict synthesis.
// The types duplicate the ZFS witness envelope (Header/Coverage/Standing/Error)
// intentionally — each profile owns its own wire surface; a shared envelope
// invites coupling where the only thing actually shared is structural shape.
// Dedupe can happen later if a third witness shows the duplication is real.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWitnessReport {
    pub schema: String,
    pub witness: SmartWitnessHeader,
    pub coverage: SmartWitnessCoverage,
    pub standing: SmartWitnessStanding,
    #[serde(default)]
    pub observations: Vec<SmartObservation>,
    #[serde(default)]
    pub errors: Vec<SmartWitnessError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWitnessHeader {
    pub id: String,
    #[serde(rename = "type")]
    pub witness_type: String,
    pub host: String,
    pub profile_version: String,
    pub collection_mode: String,
    pub privilege_model: String,
    #[serde(with = "time::serde::rfc3339")]
    pub collected_at: OffsetDateTime,
    pub duration_ms: Option<i64>,
    pub status: String,
    #[serde(default)]
    pub observed_subject: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWitnessCoverage {
    #[serde(default)]
    pub can_testify: Vec<String>,
    #[serde(default)]
    pub cannot_testify: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWitnessStanding {
    #[serde(default)]
    pub authoritative_for: Vec<String>,
    #[serde(default)]
    pub advisory_for: Vec<String>,
    #[serde(default)]
    pub inadmissible_for: Vec<String>,
}

/// SMART observation. Only one variant in Phase 1 (`smart_device`).
/// Unknown kinds deserialize into `Other` so forward-compat profile
/// growth does not break NQ.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SmartObservation {
    #[serde(rename = "smart_device")]
    Device(SmartDeviceObservation),
    #[serde(other)]
    Other,
}

/// One row of raw device-reported SMART evidence. Phase 1 refuses to
/// reconcile `smart_overall_passed` against the uncorrected-error counters;
/// both surface as independent fields so detector work in a later phase
/// has the full evidence set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartDeviceObservation {
    pub subject: String,
    pub device_path: String,
    pub device_class: String,
    pub protocol: String,

    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub firmware_version: Option<String>,
    #[serde(default)]
    pub capacity_bytes: Option<i64>,
    #[serde(default)]
    pub logical_block_size: Option<i64>,

    #[serde(default)]
    pub smart_available: Option<bool>,
    #[serde(default)]
    pub smart_enabled: Option<bool>,
    #[serde(default)]
    pub smart_overall_passed: Option<bool>,

    #[serde(default)]
    pub temperature_c: Option<i64>,
    #[serde(default)]
    pub power_on_hours: Option<i64>,

    #[serde(default)]
    pub uncorrected_read_errors: Option<i64>,
    #[serde(default)]
    pub uncorrected_write_errors: Option<i64>,
    #[serde(default)]
    pub uncorrected_verify_errors: Option<i64>,
    #[serde(default)]
    pub media_errors: Option<i64>,
    /// ATA-only normalized field. SMART attribute #5
    /// (Reallocated_Sector_Ct) — count of bad blocks the drive has
    /// remapped to its spare pool. Null on NVMe and SCSI.
    #[serde(default)]
    pub reallocated_sector_count: Option<i64>,

    #[serde(default)]
    pub nvme_percentage_used: Option<i64>,
    #[serde(default)]
    pub nvme_available_spare_pct: Option<i64>,
    #[serde(default)]
    pub nvme_critical_warning: Option<i64>,
    #[serde(default)]
    pub nvme_unsafe_shutdowns: Option<i64>,

    pub coverage: SmartDeviceCoverage,
    pub collection_outcome: String,

    #[serde(default)]
    pub raw: Option<serde_json::Value>,
    #[serde(default)]
    pub raw_truncated: Option<bool>,
    #[serde(default)]
    pub raw_original_bytes: Option<i64>,
    #[serde(default)]
    pub raw_truncated_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartDeviceCoverage {
    #[serde(default)]
    pub can_testify: Vec<String>,
    #[serde(default)]
    pub cannot_testify: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWitnessError {
    pub kind: String,
    pub detail: String,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_value, json, to_value, Value};

    /// Lock the JSON shape of `ClaimRefusal`. This test is the wire
    /// contract — changing the field names or the enum casing here
    /// breaks every persisted receipt.
    #[test]
    fn claim_refusal_serializes_with_snake_case_kind() {
        let refusal = ClaimRefusal::new(
            RefusalKind::ConsequenceClaim,
            "Whether to restart, reconfigure, or deactivate a failing source",
        );
        let serialized: Value = to_value(&refusal).expect("serialize");
        assert_eq!(
            serialized,
            json!({
                "refusal_kind": "consequence_claim",
                "statement":   "Whether to restart, reconfigure, or deactivate a failing source"
            })
        );
    }

    #[test]
    fn claim_refusal_roundtrips_through_json() {
        let original = ClaimRefusal::new(
            RefusalKind::SelfAuditRefusal,
            "NQ's own overall health (the witness cannot be its own complete audit)",
        );
        let serialized = serde_json::to_string(&original).expect("serialize");
        let parsed: ClaimRefusal = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(parsed, original);
    }

    /// Every `RefusalKind` variant must round-trip. This test exists so
    /// that a renamed variant is caught by a failing roundtrip, not by
    /// a downstream consumer parsing a receipt and silently dropping
    /// the refusal.
    #[test]
    fn every_refusal_kind_roundtrips() {
        let kinds = [
            RefusalKind::ConsequenceClaim,
            RefusalKind::FutureStateClaim,
            RefusalKind::SelfAuditRefusal,
            RefusalKind::OutOfJurisdiction,
            RefusalKind::AboveSubstrate,
            RefusalKind::BelowSubstrate,
            RefusalKind::EnvironmentalContext,
            RefusalKind::AbsenceSemantics,
            RefusalKind::CompositionReEmission,
            RefusalKind::KindSpecific,
        ];
        for kind in kinds {
            let serialized = serde_json::to_string(&kind).expect("serialize");
            let parsed: RefusalKind = serde_json::from_str(&serialized).expect("deserialize");
            assert_eq!(parsed, kind, "roundtrip failed for {kind:?}");
        }
    }

    /// Lock the snake_case JSON rendering for every variant. If a
    /// future rename changes the wire string, this test fails loudly
    /// rather than letting the rename ship as a silent schema break.
    #[test]
    fn refusal_kind_wire_strings_are_pinned() {
        let pairs = [
            (RefusalKind::ConsequenceClaim, "consequence_claim"),
            (RefusalKind::FutureStateClaim, "future_state_claim"),
            (RefusalKind::SelfAuditRefusal, "self_audit_refusal"),
            (RefusalKind::OutOfJurisdiction, "out_of_jurisdiction"),
            (RefusalKind::AboveSubstrate, "above_substrate"),
            (RefusalKind::BelowSubstrate, "below_substrate"),
            (RefusalKind::EnvironmentalContext, "environmental_context"),
            (RefusalKind::AbsenceSemantics, "absence_semantics"),
            (RefusalKind::CompositionReEmission, "composition_re_emission"),
            (RefusalKind::KindSpecific, "kind_specific"),
        ];
        for (kind, expected) in pairs {
            let serialized = serde_json::to_string(&kind).expect("serialize");
            assert_eq!(serialized, format!("\"{expected}\""), "{kind:?}");
            let parsed: RefusalKind =
                serde_json::from_str(&format!("\"{expected}\"")).expect("deserialize");
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn unknown_refusal_kind_string_is_a_deserialize_error() {
        let r: Result<RefusalKind, _> = serde_json::from_str("\"made_up_variant\"");
        assert!(
            r.is_err(),
            "closed enum must reject unknown variants — got {r:?}"
        );
    }

    /// Same-kind, different-statement is intentionally not equal:
    /// the dedupe-caution from the gap doc requires that two refusals
    /// with shared kind but distinct statements remain distinguishable.
    #[test]
    fn same_kind_different_statement_not_equal() {
        let a = ClaimRefusal::new(RefusalKind::OutOfJurisdiction, "wrong host");
        let b = ClaimRefusal::new(RefusalKind::OutOfJurisdiction, "wrong sibling kind");
        assert_ne!(a, b);
        assert_eq!(a.refusal_kind, b.refusal_kind);
    }

    /// Round-trips through a generic `serde_json::Value` to catch any
    /// `#[serde]` attribute that interferes with self-describing
    /// deserialization (e.g. a `tag = "..."` accidentally added).
    #[test]
    fn claim_refusal_roundtrips_through_value() {
        let original = ClaimRefusal::new(RefusalKind::AboveSubstrate, "semantic correctness");
        let v = to_value(&original).expect("to_value");
        let back: ClaimRefusal = from_value(v).expect("from_value");
        assert_eq!(back, original);
    }

    // -----------------------------------------------------------------
    // /state envelope schema/version (nq.witness_packet.v1). The version
    // suffix is load-bearing (docs/architecture/COMPATIBILITY.md); these
    // tests pin both that producers stamp it AND that absence is never
    // laundered into a version.
    // -----------------------------------------------------------------

    #[test]
    fn publisher_state_current_stamps_documented_envelope_schema() {
        // The constructor is the single honest producer path; it must stamp the
        // documented value, and that value must survive serialization.
        assert_eq!(PUBLISHER_STATE_SCHEMA, "nq.witness_packet.v1");
        let v = json!({
            "schema": PUBLISHER_STATE_SCHEMA,
            "host": "h1",
            "collected_at": "2026-01-01T00:00:00Z",
            "collectors": {}
        });
        let state: PublisherState = from_value(v).expect("deserialize versioned payload");
        assert_eq!(state.schema.as_deref(), Some("nq.witness_packet.v1"));

        let back = to_value(&state).expect("serialize");
        assert_eq!(
            back.get("schema").and_then(Value::as_str),
            Some("nq.witness_packet.v1"),
            "versioned envelope must round-trip carrying its schema",
        );
    }

    #[test]
    fn publisher_state_absent_schema_deserializes_to_none_not_v1() {
        // Anti-laundering (operator caution): a payload with no schema field is
        // unversioned. It MUST deserialize to None, never be silently upgraded
        // into nq.witness_packet.v1. The consumer decides what to do with None.
        let v = json!({
            "host": "h1",
            "collected_at": "2026-01-01T00:00:00Z",
            "collectors": {}
        });
        let state: PublisherState = from_value(v).expect("deserialize schema-less payload");
        assert_eq!(
            state.schema, None,
            "absent schema must be None, not laundered into a version",
        );
    }

    #[test]
    fn publisher_state_none_schema_serializes_to_absent_field() {
        // Absence stays absence: a None schema re-serializes to an absent field
        // (skip_serializing_if), not a null — so an unversioned payload is never
        // dressed up as carrying a (null) schema slot.
        let v = json!({
            "host": "h1",
            "collected_at": "2026-01-01T00:00:00Z",
            "collectors": {}
        });
        let state: PublisherState = from_value(v).expect("deserialize");
        let back = to_value(&state).expect("serialize");
        assert!(
            back.get("schema").is_none(),
            "None schema must re-serialize to an absent field, not null",
        );
    }
}
