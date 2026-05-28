//! Claim preflight types — operator-facing surface that consumes existing
//! NQ testimony and returns a bounded verdict + evidence bundle.
//!
//! See `docs/working/decisions/CLAIM_PREFLIGHT.md`, `docs/operator/VERDICTS.md`, `docs/architecture/WITNESS_PACKET.md`,
//! and `docs/working/gaps/CLAIM_KIND_DISK_STATE_GAP.md` for the doctrine. This module
//! defines the consumer-facing DTO shape; the evaluator (`nq-db::preflight`)
//! computes a `PreflightResult` from existing findings and standing state.
//!
//! V1 covers one claim kind: `disk_state`. The eight-verdict vocabulary is
//! shared across claim kinds. Structured `ClaimKind` only — no operator-phrase
//! intake at this layer.

use serde::{Deserialize, Serialize};

/// Wire schema identifier for `disk_state` preflight results.
pub const PREFLIGHT_DISK_STATE_SCHEMA: &str = "nq.preflight.disk_state.v1";

/// Wire schema identifier for `ingest_state` preflight results. NQ
/// testifies about its own ingest pulse structure (the aggregator's
/// `generations` and `source_runs` rows). It does **not** testify
/// about upstream source substrate or about its own overall health.
pub const PREFLIGHT_INGEST_STATE_SCHEMA: &str = "nq.preflight.ingest_state.v1";

/// Wire schema identifier for `dns_state` preflight results. One envelope
/// per `(vantage_host, resolver, query_name, query_type)` tuple. NQ
/// testifies to what the resolver said from one vantage at one instant.
/// It does **not** testify to endpoint reachability, service health,
/// global DNS truth, future resolution, or registrar/account status.
pub const PREFLIGHT_DNS_STATE_SCHEMA: &str = "nq.preflight.dns_state.v1";

/// Wire schema identifier for `sqlite_wal_state` preflight results. One
/// envelope per `(host, db_file_path)` target. NQ testifies to SQLite
/// WAL substrate state observed by a probe over a recent observation
/// window. It does **not** testify to application recovery, query
/// correctness, downstream artifact freshness, future checkpoint
/// outcomes, or any consequence claim — those refusals are
/// constitutional, see `sqlite_wal_state_cannot_testify`.
pub const PREFLIGHT_SQLITE_WAL_STATE_SCHEMA: &str = "nq.preflight.sqlite_wal_state.v1";

/// Wire schema identifier for `component_testimony_observation_loop_alive`
/// preflight results. One envelope per `(component_id, subject_id)` target.
/// First component-testimony kind in the namespace; emitted by a component
/// about its own observation-loop pulse and consumed externally to classify
/// absence under declared coverage. Refusals are constitutional, see
/// `component_testimony_observation_loop_alive_cannot_testify`.
pub const PREFLIGHT_COMPONENT_TESTIMONY_OBSERVATION_LOOP_ALIVE_SCHEMA: &str =
    "nq.preflight.component_testimony_observation_loop_alive.v1";

/// Contract version for the preflight wire shape. Bumps on breaking change.
pub const PREFLIGHT_CONTRACT_VERSION: u32 = 1;

/// Threshold (milliseconds) for the V1 receiver-side time-basis sanity
/// check `observed_at_future_of_evaluator`. A support whose `observed_at`
/// exceeds the evaluator's `generated_at` by more than this many
/// milliseconds is flagged as suspect. The default (300_000 ms = 5
/// minutes) mirrors the Kerberos clock-skew tolerance — large enough to
/// absorb ordinary network and clock-update jitter, small enough to
/// catch gross drift. See `docs/working/gaps/TIME_BASIS_POISONING_GAP.md` §
/// "Internal sanity checks" for the discipline.
pub const TIME_BASIS_DRIFT_THRESHOLD_MS: i64 = 300_000;

/// Structured claim kind. V3 covers `DiskState`, `IngestState`, and
/// `DnsState`. New kinds require a separate ratified change. The
/// bespoke per-kind pattern stands; the four concrete registry-pressure
/// points named in `docs/working/gaps/DNS_WITNESS_FAMILY_GAP.md` move the
/// forcing case to kind 4 (see also
/// `docs/working/gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` for the eight
/// guardrails that govern the registry shape when it does land).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    DiskState,
    IngestState,
    DnsState,
    SqliteWalState,
    /// First component-testimony kind. NQ-on-NQ observation-loop heartbeat;
    /// see `docs/working/decisions/preflights/NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md`.
    /// The `component_testimony_` prefix is a **claim namespace**, not an
    /// axis declaration — discriminates this kind family from future
    /// external-component / app-level observers that might otherwise
    /// collide on bare names (operator decision 2026-05-28, scope question A).
    ComponentTestimonyObservationLoopAlive,
}

impl ClaimKind {
    /// Snake-case string form. Must match the serde serialization above.
    /// Used by the receipt `evaluator` binding (Slice 1b) to name which
    /// Track A evaluator produced a given receipt.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DiskState => "disk_state",
            Self::IngestState => "ingest_state",
            Self::DnsState => "dns_state",
            Self::SqliteWalState => "sqlite_wal_state",
            Self::ComponentTestimonyObservationLoopAlive => {
                "component_testimony_observation_loop_alive"
            }
        }
    }
}

/// Kind of response observed for a single DNS query against a single
/// resolver from a single vantage. Closed enum; new variants require a
/// ratified change. The negative-answer taxonomy (`Nodata`, `Nxdomain`,
/// `Servfail`, `Refused`, `Timeout`, `TransportError`) is the
/// load-bearing DNS-specific witness contribution — conflating them is
/// the bug `dns_state` exists to refuse. See
/// `docs/working/gaps/DNS_WITNESS_FAMILY_GAP.md` for verdict mapping and
/// constitutional refusals.
///
/// `ValidationFailure` is reserved for a future DNSSEC-validating
/// probe; V0 collectors never emit it. The slot exists so adding
/// validation later is not a wire-breaking change.
///
/// "No row exists for this tuple" is **not** a `ResponseKind` — that
/// case belongs to the evaluator layer (`insufficient_coverage`
/// verdict). Persisting a sentinel for absence would launder absence
/// into testimony.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseKind {
    Success,
    Nodata,
    Nxdomain,
    Servfail,
    Refused,
    Timeout,
    TransportError,
    ValidationFailure,
}

impl ResponseKind {
    /// Snake-case string form. Must match the JSON serialization above
    /// and the values in the `dns_observations.response_kind` CHECK
    /// constraint (migration 047).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Nodata => "nodata",
            Self::Nxdomain => "nxdomain",
            Self::Servfail => "servfail",
            Self::Refused => "refused",
            Self::Timeout => "timeout",
            Self::TransportError => "transport_error",
            Self::ValidationFailure => "validation_failure",
        }
    }
}

impl std::str::FromStr for ResponseKind {
    type Err = UnknownResponseKind;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "success" => Ok(Self::Success),
            "nodata" => Ok(Self::Nodata),
            "nxdomain" => Ok(Self::Nxdomain),
            "servfail" => Ok(Self::Servfail),
            "refused" => Ok(Self::Refused),
            "timeout" => Ok(Self::Timeout),
            "transport_error" => Ok(Self::TransportError),
            "validation_failure" => Ok(Self::ValidationFailure),
            other => Err(UnknownResponseKind(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownResponseKind(pub String);

impl std::fmt::Display for UnknownResponseKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown dns response_kind: {:?}", self.0)
    }
}

impl std::error::Error for UnknownResponseKind {}

/// The eight verdicts from `docs/operator/VERDICTS.md`. Non-overlapping in primary
/// trigger; the more-specific one wins when two could apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Admissible,
    AdmissibleWithScope,
    UnsupportedAsStated,
    ClaimExceedsTestimony,
    InsufficientCoverage,
    StaleTestimony,
    ContradictoryTestimony,
    CannotTestify,
}

/// Status of receiver-side time-basis sanity over the supports in a
/// `PreflightResult`. Per `docs/working/gaps/TIME_BASIS_POISONING_GAP.md` §
/// "Default posture": **`Unknown` is not poisoned.** Absence of an
/// active suspicion does not constitute a clean bill of time-basis
/// health; it is silence about the question. Refusal or downgrade
/// fires only when an active suspicion is recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeBasisStatus {
    /// No active suspicion. Either no sanity check fired, or no
    /// observable time-basis evidence was available (e.g. supports
    /// carry no `observed_at`). Default posture for routine claims
    /// without corroborating time-basis evidence.
    Unknown,
    /// One or more receiver-side sanity checks fired. The annotation
    /// names which checks; the consumer applies its own posture.
    Suspect,
}

/// Receiver-side time-basis annotation attached to a `PreflightResult`.
/// Populated by `PreflightResult::compute_time_basis`. The annotation
/// is testimony about the standing of *other* testimony; per the
/// anti-laundering rules in `TIME_BASIS_POISONING_GAP.md`, it does not
/// authorize discarding affected receipts, forcing a clock correction,
/// or any other consequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBasisAnnotation {
    pub status: TimeBasisStatus,
    /// Controlled-vocabulary identifiers for the sanity checks that
    /// fired. Empty when `status` is `Unknown`. V1 vocabulary:
    /// - `observed_at_future_of_evaluator` — a support's `observed_at`
    ///   is more than `threshold_ms` in the future of `generated_at`.
    pub suspicion_kinds: Vec<String>,
    /// The largest `observed_at - generated_at` across supports, in
    /// signed milliseconds (positive = observed_at in the future of
    /// generated_at). `None` when no support carried an `observed_at`.
    /// Negative values are recorded as 0 in V1 (the only check is
    /// future-of-evaluator); the field is reserved for symmetric checks
    /// in future versions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_observation_delta_ms: Option<i64>,
    /// Threshold used for the future-of-evaluator check, in milliseconds.
    pub threshold_ms: i64,
}

/// What the preflight is being asked to evaluate. `scope` is the granularity
/// of the target identity; `id` is the specific subject when scope is finer
/// than host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightTarget {
    pub host: String,
    /// One of `host`, `pool`, `vdev`, `device`.
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Identity of the witness packet a support is anchored to.
///
/// Populated on `disk_state` supports under the Slice 2 cut-over: the
/// evaluator projects each admitted finding into a legacy-projection
/// witness packet, retains the packet's wire identity (witness type,
/// JCS+SHA-256 digest, substrate-time observed_at), and stamps it here
/// so `From<PreflightResult>` can build one `WitnessRef` per admitted
/// support. Absent on supports from evaluators that have not yet cut
/// over (today: `ingest_state`, `dns_state`); those receipts continue
/// to carry coverage-derived WitnessRefs. See
/// `docs/working/decisions/preflights/TRACK_A_WITNESS_PACKET_CUTOVER.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportingWitnessPacket {
    pub witness_type: String,
    pub digest: String,
    pub observed_at: String,
    /// Custody basis copied from the underlying `WitnessPacket`. Today:
    /// `Some("legacy_projection")` for `disk_state` post-cut-over;
    /// `Some("native_observation")` for packets that explicitly declare
    /// it; `None` for packets that predate the Slice 2 cut-over
    /// distinction. Threaded onto `WitnessRef.custody_basis` by
    /// `From<PreflightResult>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custody_basis: Option<String>,
}

/// One admissible weaker claim, with provenance back to the underlying
/// finding. The `claim` text is scoped — it carries witness, subject, and
/// observed_at — so a consumer that quotes only the `claim` field cannot
/// launder the scope away.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightSupport {
    pub claim: String,
    pub finding_kind: String,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admissibility_state: Option<String>,
    /// Slice 2 cut-over: the projected witness packet identity behind
    /// this support. Populated on `disk_state` supports after Slice 2;
    /// absent on supports from pre-cut-over evaluators.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub witness_packet: Option<SupportingWitnessPacket>,
}

/// A finding that exists for the target but is not being admitted as a
/// supporting weaker claim. `reason` says why (suppressed by ancestor /
/// declaration, cleared, stale, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightExclusion {
    pub finding_kind: String,
    pub subject: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Standing report for one witness family. `standing` is one of `observable`,
/// `silent`, `node_unobservable`, or `absent`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightCoverage {
    pub witness: String,
    pub standing: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Preflight result. Constitutional `cannot_testify` entries are always
/// populated regardless of substrate state — they are the refusal surface
/// the claim kind exists to maintain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightResult {
    pub schema: String,
    pub contract_version: u32,
    pub claim_kind: ClaimKind,
    pub target: PreflightTarget,
    pub verdict: Verdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict_note: Option<String>,
    pub supports: Vec<PreflightSupport>,
    pub excludes: Vec<PreflightExclusion>,
    /// Constitutional refusal surface for this claim kind. Always populated.
    /// Per `CLAIM_KIND_DISK_STATE_GAP.md`, no combination of witness output
    /// licenses any of these conclusions.
    pub cannot_testify: Vec<String>,
    pub coverage: Vec<PreflightCoverage>,
    pub generated_at: String,
    /// Oldest `observed_at` among `supports[]`. `None` when supports is
    /// empty or no support carries an observed_at. This is evidence-window
    /// disclosure only — it does not imply validity, freshness policy, or
    /// any deadline. NQ exposes when testimony was observed; consumers
    /// decide what to do with that information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at_min: Option<String>,
    /// Newest `observed_at` among `supports[]`. Same semantics as
    /// `observed_at_min`: window disclosure, no validity claim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at_max: Option<String>,
    /// Evaluator-provided per-claim deadline, when that evaluator defines
    /// one. RFC3339 UTC. Today: `dns_state` and `ingest_state` evaluators
    /// emit `observed_at_max + claim-kind-specific threshold` here.
    /// `disk_state` does not — its freshness model is per-finding
    /// admissibility, not a per-claim deadline.
    ///
    /// `freshness_horizon` is not a universal freshness model. Absence of
    /// this field means no per-claim deadline was emitted by this
    /// evaluator; it does not mean stale-immune, verified fresh, or
    /// freshness-unbounded.
    ///
    /// Anchored to `observed_at_max`, never to `generated_at` — packet-time
    /// is not an honest substitute for observation-time. When
    /// `observed_at_max` is absent, this field is also absent.
    ///
    /// Carried through to [`crate::receipt::Receipt::freshness_horizon`]
    /// by `From<PreflightResult>`. Verification (e.g. `now > horizon`) is
    /// Slice 1d territory; 1c populates only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness_horizon: Option<String>,
    /// Receiver-side time-basis sanity annotation, populated by
    /// `compute_time_basis`. `None` when no time-basis check has run;
    /// `Some(Unknown)` when checks ran and found nothing to flag (this
    /// is NOT a clean bill of time-basis health — see the default-
    /// posture rule in `TIME_BASIS_POISONING_GAP.md`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_basis: Option<TimeBasisAnnotation>,
    /// Receipt-side consumer-convenience field carrying structured
    /// signals computed by the evaluator. **Namespaced by claim kind**
    /// (`signals.sqlite_wal_state.<field>`), untyped (`Option<Value>`),
    /// not a claim-definition surface. See `Receipt.signals` for the
    /// full contract — this field carries through unchanged.
    ///
    /// Populated today only by the `sqlite_wal_state` evaluator (the
    /// kind whose verdict_note carries multiple decoration booleans
    /// the consumer-preflight beat showed agents NLP-parsing). Other
    /// kinds may adopt structured signals later; this field remains
    /// `None` for them until they do.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signals: Option<serde_json::Value>,
}

impl PreflightResult {
    /// Construct an empty result skeleton for the given claim kind and target.
    /// The caller fills in supports / excludes / coverage / verdict.
    /// `cannot_testify` is preloaded with the constitutional refusal list for
    /// the claim kind.
    pub fn skeleton(claim_kind: ClaimKind, target: PreflightTarget, generated_at: String) -> Self {
        let (schema, cannot_testify) = match claim_kind {
            ClaimKind::DiskState => (
                PREFLIGHT_DISK_STATE_SCHEMA.to_string(),
                disk_state_cannot_testify(),
            ),
            ClaimKind::IngestState => (
                PREFLIGHT_INGEST_STATE_SCHEMA.to_string(),
                ingest_state_cannot_testify(),
            ),
            ClaimKind::DnsState => (
                PREFLIGHT_DNS_STATE_SCHEMA.to_string(),
                dns_state_cannot_testify(),
            ),
            ClaimKind::SqliteWalState => (
                PREFLIGHT_SQLITE_WAL_STATE_SCHEMA.to_string(),
                sqlite_wal_state_cannot_testify(),
            ),
            ClaimKind::ComponentTestimonyObservationLoopAlive => (
                PREFLIGHT_COMPONENT_TESTIMONY_OBSERVATION_LOOP_ALIVE_SCHEMA.to_string(),
                component_testimony_observation_loop_alive_cannot_testify(),
            ),
        };
        Self {
            schema,
            contract_version: PREFLIGHT_CONTRACT_VERSION,
            claim_kind,
            target,
            verdict: Verdict::InsufficientCoverage,
            verdict_note: None,
            supports: Vec::new(),
            excludes: Vec::new(),
            cannot_testify,
            coverage: Vec::new(),
            generated_at,
            observed_at_min: None,
            observed_at_max: None,
            freshness_horizon: None,
            time_basis: None,
            signals: None,
        }
    }

    /// Run receiver-side time-basis sanity checks over `self.supports`
    /// and set `self.time_basis` accordingly. See
    /// `docs/working/gaps/TIME_BASIS_POISONING_GAP.md` § "Internal sanity checks"
    /// for the discipline.
    ///
    /// V1 implements one check: `observed_at_future_of_evaluator`. A
    /// support whose `observed_at` is more than
    /// `TIME_BASIS_DRIFT_THRESHOLD_MS` in the future of
    /// `self.generated_at` fires the check and the annotation becomes
    /// `Suspect`. Otherwise the annotation is `Unknown` — per the
    /// default-posture rule, that is *silence about time basis*, not a
    /// clean bill of health.
    ///
    /// The annotation is testimony about the standing of *other*
    /// testimony; the verdict itself is NOT changed by this method. A
    /// future slice may add verdict downgrade behavior; V1 is
    /// additive-annotation only.
    pub fn compute_time_basis(&mut self) {
        let now = match parse_rfc3339(&self.generated_at) {
            Some(t) => t,
            None => {
                self.time_basis = None;
                return;
            }
        };

        let mut max_future_delta_ms: i64 = 0;
        let mut had_any_observed_at = false;
        for s in &self.supports {
            let obs = match s.observed_at.as_deref().and_then(parse_rfc3339) {
                Some(t) => t,
                None => continue,
            };
            had_any_observed_at = true;
            let delta_ms = (obs - now).whole_milliseconds() as i64;
            if delta_ms > max_future_delta_ms {
                max_future_delta_ms = delta_ms;
            }
        }

        let threshold_ms = TIME_BASIS_DRIFT_THRESHOLD_MS;
        let (status, suspicion_kinds) = if max_future_delta_ms > threshold_ms {
            (
                TimeBasisStatus::Suspect,
                vec!["observed_at_future_of_evaluator".to_string()],
            )
        } else {
            (TimeBasisStatus::Unknown, Vec::new())
        };

        self.time_basis = Some(TimeBasisAnnotation {
            status,
            suspicion_kinds,
            max_observation_delta_ms: if had_any_observed_at {
                Some(max_future_delta_ms)
            } else {
                None
            },
            threshold_ms,
        });
    }
}

fn parse_rfc3339(s: &str) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
}

/// Compute a per-claim freshness horizon: `observed_at + threshold_seconds`,
/// rendered as RFC3339 UTC. Returns `None` if `observed_at_max` is `None`
/// or fails to parse — never anchor on `generated_at` as a fallback, since
/// packet-time substituting for observation-time would launder the meaning
/// of the horizon. Used by `dns_state` and `ingest_state` evaluators to
/// populate [`PreflightResult::freshness_horizon`]; carried through to
/// [`crate::receipt::Receipt::freshness_horizon`].
pub fn freshness_horizon_from(
    observed_at_max: Option<&str>,
    threshold_seconds: i64,
) -> Option<String> {
    let dt = parse_rfc3339(observed_at_max?)?;
    let horizon = dt + time::Duration::seconds(threshold_seconds);
    horizon
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

/// Constitutional refusal surface for `ingest_state`. Each entry
/// corresponds to a conclusion the `generations` / `source_runs`
/// substrate does not license, regardless of which generation rows
/// are present. NQ testifies that its own pull cycle ran (or
/// failed); it does not testify about upstream substrate, semantic
/// content, or its own overall health. The "NQ itself is healthy"
/// refusal is the self-witness firewall: a witness about itself is
/// circular, and `ingest_state` is one channel among many that a
/// downstream system might (separately) read.
pub fn ingest_state_cannot_testify() -> Vec<String> {
    vec![
        "Upstream source substrate health (NQ observed its own pull attempt; the source's actual state is upstream and beyond witness)".to_string(),
        "Future ingest success or failure".to_string(),
        "Semantic correctness of ingested data (the pull cycle's structural state is testifiable; the content's truth is not)".to_string(),
        "Network connectivity health".to_string(),
        "Whether to restart, reconfigure, or deactivate a failing source (consequence claim)".to_string(),
        "NQ's own overall health (the witness cannot be its own complete audit)".to_string(),
        "Whether ingest will recover from the current failure shape (future-state claim)".to_string(),
    ]
}

/// Constitutional refusal surface for `dns_state`. Each entry
/// corresponds to a conclusion no `response_kind` row licenses,
/// regardless of which kind was observed or how many tuples were
/// probed. Mirrors the `cannot_testify` enumeration in
/// `docs/working/gaps/DNS_WITNESS_FAMILY_GAP.md`. The last entry is the
/// `feedback_knob_facing` boundary preserved: `dns_state` classifies
/// world-state testimony; consequence stays downstream.
pub fn dns_state_cannot_testify() -> Vec<String> {
    vec![
        "Endpoint reachability for the resolved name (DNS is not TCP)".to_string(),
        "Service health at any address returned (DNS is not the service)".to_string(),
        "User-visible availability (anycast / split horizon / per-network views unobserved)".to_string(),
        "Global DNS truth for this name (one vantage, one resolver — not the world)".to_string(),
        "Authoritative-zone correctness (V0 likely reads recursive/cached answers; authority is upstream)".to_string(),
        "Future resolution (TTL is a hint, not a contract)".to_string(),
        "Permanence of negative answers (NXDOMAIN now ≠ NXDOMAIN forever; cached denial is dated)".to_string(),
        "Reverse mapping (address → name) for any A/AAAA result (PTR is a separate query)".to_string(),
        "Registrar / account / ownership status (DNS responses do not testify to custody)".to_string(),
        "DNSSEC validation outcome (V0 does not validate; reserve refusal slot for when it does)".to_string(),
        "Resolver-internal substrate health (SERVFAIL is testimony about the resolver, not about the name)".to_string(),
        "Recovery prediction for any error-class response (future-state claim)".to_string(),
        "Whether to repoint, fail over, retry, or page (consequence claim)".to_string(),
    ]
}

/// Constitutional refusal surface for `sqlite_wal_state`. Each entry
/// corresponds to a conclusion no `wal_observations` row (or window
/// thereof) licenses, regardless of how the WAL has moved. Mirrors the
/// `cannot_testify` enumeration in
/// `docs/working/decisions/preflights/KIND_4_SQLITE_WAL_STATE.md` §5. The last entry is
/// the [[feedback_knob_facing]] boundary preserved at the wire surface:
/// NQ classifies WAL substrate testimony; consumer-side consequence
/// (alert mapping, restart, repointing, page) stays with the consumer.
pub fn sqlite_wal_state_cannot_testify() -> Vec<String> {
    vec![
        "Whether the application that owns this DB will recover (application-state claim; the WAL substrate does not testify to it)".to_string(),
        "Whether queries against this DB will return correct results (query correctness is below substrate)".to_string(),
        "Whether reports / downstream artifacts derived from this DB are stale (application-layer claim, not WAL substrate)".to_string(),
        "Whether the WAL state on a different DB file is healthy (single-target jurisdiction)".to_string(),
        "Whether the WAL state will degrade in the future (future-state claim)".to_string(),
        "Whether checkpoint operations succeeded (the operation itself is below substrate; absence of effect is testifiable, the operation is not)".to_string(),
        "Why the `-wal` sidecar is absent on a given observation (a non-WAL `journal_mode`, post-checkpoint cleanup, and post-close cleanup all produce `wal_present=false`; the probe stat()s the path and cannot distinguish them from substrate state alone — see `KIND_4_SQLITE_WAL_PROBE.md` §8)".to_string(),
        "Whether the reader holding a pinned transaction is the right reader to hold it (operational-context claim)".to_string(),
        "Whether SQLite's behavior is correct given its inputs (DB engine correctness is below substrate)".to_string(),
        "Whether to restart, repoint, kill the pinned reader, or page (consequence claim)".to_string(),
    ]
}

/// Constitutional refusal surface for
/// `component_testimony_observation_loop_alive`. Each entry corresponds
/// to a conclusion the heartbeat does not license, regardless of how
/// often it arrives. Mirrors the cannot_testify list pinned in the
/// foundation preflight §4 (`NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md`).
///
/// The disciplinary line: the heartbeat says *the observation loop reached
/// a checkpoint at time T*. It says nothing else.
pub fn component_testimony_observation_loop_alive_cannot_testify() -> Vec<String> {
    vec![
        "Whether NQ is healthy (the observation loop running is one signal among many; an alive loop emitting heartbeats does not testify to NQ standing as a whole)".to_string(),
        "Whether other NQ loops (reconciler, ack, ingest, export) are alive (this kind testifies only to the observation loop; sibling loops need their own component-testimony kinds)".to_string(),
        "Whether NQ's stored claims are semantically correct (substrate observation only)".to_string(),
        "Whether NQ's ingested witnesses are truthful (NQ does not certify producer truthfulness)".to_string(),
        "Whether SQLite is an admissible architecture for this deployment (substrate-state observation does not endorse substrate-choice)".to_string(),
        "Whether to escalate, restart, or page (consequence claim; per the escalation_target field, lifecycle resolution lives outside NQ when the subject is NQ-self)".to_string(),
        "Whether absence of this testimony means NQ is unhealthy (absence under declared coverage is one of seven absence states; only the consumer routes it to escalation, NQ does not)".to_string(),
        "Whether NQ's future operation is safe (no future-state testimony)".to_string(),
        "Whether composed verdicts derived from this testimony may be re-emitted as claims (composition is read-side projection only; see NQ_NS_CHANNEL_SPLIT_NQ_SIDE §4 composition rule)".to_string(),
    ]
}

/// Constitutional refusal surface for `disk_state`. Each entry corresponds to
/// a conclusion no combination of ZFS / SMART / disk-pressure witness output
/// licenses, regardless of how many findings light up. Mirrors the
/// `cannot_testify` enumeration in `docs/working/gaps/CLAIM_KIND_DISK_STATE_GAP.md`.
pub fn disk_state_cannot_testify() -> Vec<String> {
    vec![
        "Physical disk death".to_string(),
        "Replacement workflow (authorization, initiation, skipping, completion, closure-criteria satisfaction)".to_string(),
        "Physical component identity beyond witness coverage (sled / slot / enclosure / asset-record)".to_string(),
        "Data loss occurrence, recoverability, or unrecoverability".to_string(),
        "Future failure probability".to_string(),
        "Incident closure readiness".to_string(),
        "Drive is fine to keep / no action required (mirror consequence claim)".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_state_skeleton_has_constitutional_refusals() {
        let target = PreflightTarget {
            host: "h".into(),
            scope: "host".into(),
            id: None,
        };
        let r = PreflightResult::skeleton(ClaimKind::DiskState, target, "2026-05-14T00:00:00Z".into());
        assert_eq!(r.schema, PREFLIGHT_DISK_STATE_SCHEMA);
        assert_eq!(r.contract_version, PREFLIGHT_CONTRACT_VERSION);
        // The seven constitutional refusals must be present.
        assert!(r.cannot_testify.iter().any(|s| s.contains("Physical disk death")));
        assert!(r.cannot_testify.iter().any(|s| s.starts_with("Replacement workflow")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("Incident closure")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("Drive is fine to keep")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("Data loss")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("Future failure probability")));
        assert!(r.cannot_testify.iter().any(|s| s.contains("Physical component identity")));
    }

    #[test]
    fn verdict_serializes_snake_case() {
        let v = Verdict::AdmissibleWithScope;
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, "\"admissible_with_scope\"");
        let v = Verdict::CannotTestify;
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, "\"cannot_testify\"");
    }

    #[test]
    fn claim_kind_serializes_snake_case() {
        let k = ClaimKind::DiskState;
        let s = serde_json::to_string(&k).unwrap();
        assert_eq!(s, "\"disk_state\"");
        let k = ClaimKind::IngestState;
        let s = serde_json::to_string(&k).unwrap();
        assert_eq!(s, "\"ingest_state\"");
        let k = ClaimKind::DnsState;
        let s = serde_json::to_string(&k).unwrap();
        assert_eq!(s, "\"dns_state\"");
        let k = ClaimKind::SqliteWalState;
        let s = serde_json::to_string(&k).unwrap();
        assert_eq!(s, "\"sqlite_wal_state\"");
        let k = ClaimKind::ComponentTestimonyObservationLoopAlive;
        let s = serde_json::to_string(&k).unwrap();
        assert_eq!(s, "\"component_testimony_observation_loop_alive\"");
        // as_str matches the serde form.
        assert_eq!(
            k.as_str(),
            "component_testimony_observation_loop_alive"
        );
    }

    #[test]
    fn claim_kind_round_trips_through_serde() {
        // Every variant survives serialize → deserialize. Pinned for the
        // new ComponentTestimonyObservationLoopAlive variant in particular
        // — if a future serde rename accidentally drops the prefix, this
        // would catch it before the wire surface diverges from the
        // claim-namespace discipline.
        for k in [
            ClaimKind::DiskState,
            ClaimKind::IngestState,
            ClaimKind::DnsState,
            ClaimKind::SqliteWalState,
            ClaimKind::ComponentTestimonyObservationLoopAlive,
        ] {
            let s = serde_json::to_string(&k).unwrap();
            let back: ClaimKind = serde_json::from_str(&s).unwrap();
            assert_eq!(back, k);
        }
    }

    #[test]
    fn component_testimony_observation_loop_alive_skeleton_has_constitutional_refusals() {
        let target = PreflightTarget {
            host: "nq.local".into(),
            scope: "component_testimony".into(),
            id: Some("observation_loop".into()),
        };
        let r = PreflightResult::skeleton(
            ClaimKind::ComponentTestimonyObservationLoopAlive,
            target,
            "2026-05-28T00:00:00Z".into(),
        );
        assert_eq!(
            r.schema,
            PREFLIGHT_COMPONENT_TESTIMONY_OBSERVATION_LOOP_ALIVE_SCHEMA
        );
        assert_eq!(r.contract_version, PREFLIGHT_CONTRACT_VERSION);
        // The pinned refusals must be present (sample from the §4 list).
        assert!(r.cannot_testify.iter().any(|s| s.contains("NQ is healthy")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("other NQ loops")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("composed verdicts")));
        // Verdict starts at InsufficientCoverage like other kinds.
        assert!(matches!(r.verdict, Verdict::InsufficientCoverage));
    }

    #[test]
    fn component_testimony_observation_loop_alive_cannot_testify_uses_no_alert_taxonomy() {
        // Wire-discipline test: the refusal list must not import alert /
        // health vocabulary as the renderer's own words. The phrase
        // "NQ is healthy" appears as a REFUSED claim, which is itself
        // a denial — not a positive assertion of vocabulary. Check that
        // no entry starts with a verdict-shaped word.
        let refusals = component_testimony_observation_loop_alive_cannot_testify();
        for entry in &refusals {
            let lower = entry.to_lowercase();
            // The refusal entries describe what NQ does NOT testify to;
            // they may MENTION verdict words inside denials, but they
            // must not be authored AS verdicts.
            assert!(
                entry.starts_with("Whether ") || entry.starts_with("Why "),
                "refusal entry must be phrased as a 'Whether/Why ...' \
                 (denial-shaped), got: {entry}"
            );
            // Hard-prohibit overtly action-shaped words at the start.
            for forbidden_lead in ["alert", "page", "escalate", "warn", "critical"] {
                assert!(
                    !lower.starts_with(forbidden_lead),
                    "refusal entry must not lead with action-vocabulary {forbidden_lead:?}, got: {entry}"
                );
            }
        }
    }

    #[test]
    fn sqlite_wal_state_skeleton_has_constitutional_refusals() {
        let target = PreflightTarget {
            host: "labelwatch.neutral.zone".into(),
            scope: "sqlite_wal".into(),
            id: Some("/var/lib/labelwatch/labelwatch.db".into()),
        };
        let r = PreflightResult::skeleton(
            ClaimKind::SqliteWalState,
            target,
            "2026-05-26T14:00:00Z".into(),
        );
        assert_eq!(r.schema, PREFLIGHT_SQLITE_WAL_STATE_SCHEMA);
        assert_eq!(r.contract_version, PREFLIGHT_CONTRACT_VERSION);
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("application that owns this DB")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("queries against this DB")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("WAL state will degrade in the future")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("checkpoint operations")));
        assert!(
            r.cannot_testify
                .iter()
                .any(|s| s.contains("`wal_present=false`")),
            "WAL-absence ambiguity refusal must be present (slice 6d wrinkle)"
        );
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("repoint, kill the pinned reader, or page")));
    }

    #[test]
    fn sqlite_wal_state_cannot_testify_uses_no_alert_taxonomy() {
        // Per preflight §5 and the [[feedback_knob_facing]] discipline:
        // the cannot_testify list itself must not use warn/critical/
        // alert language. The list refuses claims, not alert levels.
        for refusal in sqlite_wal_state_cannot_testify() {
            let lower = refusal.to_ascii_lowercase();
            for forbidden in ["warn", "critical", "alert", "incident", "p1", "p2"] {
                assert!(
                    !lower.contains(forbidden),
                    "cannot_testify entry contains alert-taxonomy term {forbidden:?}: {refusal:?}"
                );
            }
        }
    }

    #[test]
    fn dns_state_skeleton_has_constitutional_refusals() {
        let target = PreflightTarget {
            host: "sushi-k".into(),
            scope: "dns_query".into(),
            id: Some("resolver=8.8.8.8;name=nq.neutral.zone;type=A".into()),
        };
        let r = PreflightResult::skeleton(
            ClaimKind::DnsState,
            target,
            "2026-05-20T00:00:00Z".into(),
        );
        assert_eq!(r.schema, PREFLIGHT_DNS_STATE_SCHEMA);
        assert_eq!(r.contract_version, PREFLIGHT_CONTRACT_VERSION);
        // Sample the load-bearing refusals — endpoint reachability and
        // global DNS truth are the most common laundering targets; DNSSEC
        // and registrar/account are the most common scope-expansion
        // attempts.
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Endpoint reachability")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Global DNS truth")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("DNSSEC validation outcome")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Registrar / account")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.starts_with("Whether to repoint")));
    }

    #[test]
    fn response_kind_serializes_snake_case() {
        // The JSON form must match dns_observations.response_kind values
        // exactly — the migration's CHECK constraint enforces the same set.
        let cases = [
            (ResponseKind::Success, "\"success\""),
            (ResponseKind::Nodata, "\"nodata\""),
            (ResponseKind::Nxdomain, "\"nxdomain\""),
            (ResponseKind::Servfail, "\"servfail\""),
            (ResponseKind::Refused, "\"refused\""),
            (ResponseKind::Timeout, "\"timeout\""),
            (ResponseKind::TransportError, "\"transport_error\""),
            (ResponseKind::ValidationFailure, "\"validation_failure\""),
        ];
        for (k, expected) in cases {
            assert_eq!(serde_json::to_string(&k).unwrap(), expected, "{k:?}");
        }
    }

    #[test]
    fn response_kind_round_trips_through_as_str_and_from_str() {
        use std::str::FromStr;
        for k in [
            ResponseKind::Success,
            ResponseKind::Nodata,
            ResponseKind::Nxdomain,
            ResponseKind::Servfail,
            ResponseKind::Refused,
            ResponseKind::Timeout,
            ResponseKind::TransportError,
            ResponseKind::ValidationFailure,
        ] {
            let s = k.as_str();
            let parsed = ResponseKind::from_str(s).unwrap();
            assert_eq!(parsed, k, "round-trip {k:?} via {s:?}");
            // The string form also matches the JSON serialization (minus quotes).
            assert_eq!(format!("\"{s}\""), serde_json::to_string(&k).unwrap());
        }
    }

    #[test]
    fn response_kind_unknown_text_errors() {
        use std::str::FromStr;
        let err = ResponseKind::from_str("DNSSEC_PASSED").unwrap_err();
        assert_eq!(err.0, "DNSSEC_PASSED");
        // Display formatting names the bad value so a future migration
        // mistake fingerprints itself in logs.
        let msg = format!("{err}");
        assert!(msg.contains("DNSSEC_PASSED"), "display must name the bad value: {msg}");
    }

    #[test]
    fn ingest_state_skeleton_has_constitutional_refusals() {
        let target = PreflightTarget {
            host: "monitor".into(),
            scope: "ingest".into(),
            id: None,
        };
        let r = PreflightResult::skeleton(
            ClaimKind::IngestState,
            target,
            "2026-05-19T00:00:00Z".into(),
        );
        assert_eq!(r.schema, PREFLIGHT_INGEST_STATE_SCHEMA);
        assert_eq!(r.contract_version, PREFLIGHT_CONTRACT_VERSION);
        // The self-witness firewall and upstream-substrate refusal must be
        // present — they are the constitutional shape of this claim kind.
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Upstream source substrate")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("NQ's own overall health")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Future ingest")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Semantic correctness")));
    }

    #[test]
    fn compute_time_basis_unknown_when_no_supports() {
        let target = PreflightTarget {
            host: "h".into(),
            scope: "host".into(),
            id: None,
        };
        let mut r = PreflightResult::skeleton(
            ClaimKind::DiskState,
            target,
            "2026-05-21T12:00:00Z".into(),
        );
        r.compute_time_basis();
        let tb = r.time_basis.as_ref().expect("time_basis populated by compute");
        assert_eq!(tb.status, TimeBasisStatus::Unknown);
        assert!(tb.suspicion_kinds.is_empty());
        // No support carried observed_at, so the delta field is absent.
        assert!(tb.max_observation_delta_ms.is_none());
        assert_eq!(tb.threshold_ms, TIME_BASIS_DRIFT_THRESHOLD_MS);
    }

    #[test]
    fn compute_time_basis_unknown_when_observed_at_within_threshold() {
        let target = PreflightTarget {
            host: "h".into(),
            scope: "host".into(),
            id: None,
        };
        let mut r = PreflightResult::skeleton(
            ClaimKind::DiskState,
            target,
            "2026-05-21T12:00:00Z".into(),
        );
        // Observed 2 minutes in the future of generated_at — under the
        // 5-minute drift threshold, so Unknown (silence about time basis,
        // NOT a clean bill of health per the default-posture rule).
        r.supports.push(PreflightSupport {
            claim: "claim".into(),
            finding_kind: "k".into(),
            subject: "s".into(),
            observed_at: Some("2026-05-21T12:02:00Z".into()),
            freshness: None,
            admissibility_state: None,
            witness_packet: None,
        });
        r.compute_time_basis();
        let tb = r.time_basis.as_ref().expect("time_basis populated");
        assert_eq!(tb.status, TimeBasisStatus::Unknown);
        assert!(tb.suspicion_kinds.is_empty());
        assert_eq!(tb.max_observation_delta_ms, Some(120_000));
    }

    #[test]
    fn compute_time_basis_suspect_when_observed_at_far_future() {
        let target = PreflightTarget {
            host: "h".into(),
            scope: "host".into(),
            id: None,
        };
        let mut r = PreflightResult::skeleton(
            ClaimKind::DiskState,
            target,
            "2026-05-21T12:00:00Z".into(),
        );
        // Observed 10 minutes in the future — exceeds the 5-minute drift
        // threshold; the receiver-side sanity check fires.
        r.supports.push(PreflightSupport {
            claim: "claim".into(),
            finding_kind: "k".into(),
            subject: "s".into(),
            observed_at: Some("2026-05-21T12:10:00Z".into()),
            freshness: None,
            admissibility_state: None,
            witness_packet: None,
        });
        r.compute_time_basis();
        let tb = r.time_basis.as_ref().expect("time_basis populated");
        assert_eq!(tb.status, TimeBasisStatus::Suspect);
        assert!(tb
            .suspicion_kinds
            .iter()
            .any(|k| k == "observed_at_future_of_evaluator"));
        assert_eq!(tb.max_observation_delta_ms, Some(600_000));
        assert_eq!(tb.threshold_ms, TIME_BASIS_DRIFT_THRESHOLD_MS);
    }

    #[test]
    fn compute_time_basis_worst_case_across_supports() {
        let target = PreflightTarget {
            host: "h".into(),
            scope: "host".into(),
            id: None,
        };
        let mut r = PreflightResult::skeleton(
            ClaimKind::DiskState,
            target,
            "2026-05-21T12:00:00Z".into(),
        );
        // First support within threshold; second support 30 minutes ahead
        // (suspect). The worst-case delta wins; the result is Suspect.
        r.supports.push(PreflightSupport {
            claim: "a".into(),
            finding_kind: "k".into(),
            subject: "s1".into(),
            observed_at: Some("2026-05-21T12:01:00Z".into()),
            freshness: None,
            admissibility_state: None,
            witness_packet: None,
        });
        r.supports.push(PreflightSupport {
            claim: "b".into(),
            finding_kind: "k".into(),
            subject: "s2".into(),
            observed_at: Some("2026-05-21T12:30:00Z".into()),
            freshness: None,
            admissibility_state: None,
            witness_packet: None,
        });
        r.compute_time_basis();
        let tb = r.time_basis.as_ref().expect("time_basis populated");
        assert_eq!(tb.status, TimeBasisStatus::Suspect);
        assert_eq!(tb.max_observation_delta_ms, Some(1_800_000));
    }

    #[test]
    fn compute_time_basis_skipped_when_generated_at_unparseable() {
        let target = PreflightTarget {
            host: "h".into(),
            scope: "host".into(),
            id: None,
        };
        let mut r = PreflightResult::skeleton(
            ClaimKind::DiskState,
            target,
            "not-an-rfc3339-timestamp".into(),
        );
        r.compute_time_basis();
        assert!(
            r.time_basis.is_none(),
            "unparseable generated_at must leave time_basis None, not a fabricated annotation"
        );
    }

    #[test]
    fn time_basis_omitted_from_json_when_none() {
        let target = PreflightTarget {
            host: "h".into(),
            scope: "host".into(),
            id: None,
        };
        let r = PreflightResult::skeleton(
            ClaimKind::DiskState,
            target,
            "2026-05-21T12:00:00Z".into(),
        );
        // Skeleton has time_basis: None — the wire shape skips the field.
        let json = serde_json::to_string(&r).unwrap();
        assert!(
            !json.contains("time_basis"),
            "time_basis field must be omitted when None: {json}"
        );
    }

    #[test]
    fn time_basis_round_trips_when_present() {
        let target = PreflightTarget {
            host: "h".into(),
            scope: "host".into(),
            id: None,
        };
        let mut r = PreflightResult::skeleton(
            ClaimKind::DiskState,
            target,
            "2026-05-21T12:00:00Z".into(),
        );
        r.compute_time_basis();
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"time_basis\""), "time_basis present when computed");
        assert!(
            json.contains("\"status\":\"unknown\""),
            "default status serializes as unknown"
        );
        let r2: PreflightResult = serde_json::from_str(&json).unwrap();
        let tb = r2.time_basis.as_ref().expect("round-tripped");
        assert_eq!(tb.status, TimeBasisStatus::Unknown);
        assert_eq!(tb.threshold_ms, TIME_BASIS_DRIFT_THRESHOLD_MS);
    }

    #[test]
    fn time_basis_status_serializes_snake_case() {
        let s = serde_json::to_string(&TimeBasisStatus::Unknown).unwrap();
        assert_eq!(s, "\"unknown\"");
        let s = serde_json::to_string(&TimeBasisStatus::Suspect).unwrap();
        assert_eq!(s, "\"suspect\"");
    }

    // -----------------------------------------------------------------
    // Slice 1c — freshness_horizon helper. The receipt-side carry-through
    // and evaluator-side wiring are exercised by tests in nq-core::receipt
    // and nq-db (dns + ingest paths).
    // -----------------------------------------------------------------

    #[test]
    fn freshness_horizon_from_computes_observed_at_plus_threshold() {
        let h = freshness_horizon_from(Some("2026-05-15T14:00:00Z"), 300).unwrap();
        // 14:00:00 + 300s = 14:05:00.
        assert!(h.starts_with("2026-05-15T14:05:00"));
    }

    #[test]
    fn freshness_horizon_from_returns_none_when_observed_at_max_is_none() {
        // Guard against future drift toward anchoring on generated_at:
        // absent observed_at_max yields absent horizon, period.
        assert!(freshness_horizon_from(None, 300).is_none());
    }

    #[test]
    fn freshness_horizon_from_returns_none_on_unparseable_observed_at() {
        assert!(freshness_horizon_from(Some("not a timestamp"), 300).is_none());
    }

    #[test]
    fn skeleton_leaves_freshness_horizon_none() {
        // Skeleton is the universal entry point for evaluator construction;
        // horizon must start absent and be populated only by evaluators
        // that have a per-claim policy.
        let target = PreflightTarget {
            host: "h1".into(),
            scope: "host".into(),
            id: None,
        };
        let r = PreflightResult::skeleton(
            ClaimKind::DiskState,
            target,
            "2026-05-21T12:00:00Z".into(),
        );
        assert!(r.freshness_horizon.is_none());
    }
}
