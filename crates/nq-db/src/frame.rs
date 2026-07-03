//! Human Now Frame — a composed operational frame over delayed witnesses.
//!
//! See `docs/working/gaps/HUMAN_NOW_FRAME_SCOPE.md`. A Human Now Frame is a
//! **derived render artifact**, not primary evidence: it is a bounded,
//! present-tense composition built from already-governed witness standing so a
//! human does not have to perform temporal binding by hand. It lives in the
//! view/read layer (alongside `OverviewVm`/`HostFreshnessVm`) precisely because
//! it is a view-model, not a `nq-core` wire/evidence type. It is never
//! persisted, never fed back into a witness/finding, and never treated as
//! authority substrate.
//!
//! This module ships the **Host V0** slice: `host_now_frame` generalizes the
//! ratified C2 two-clock host standing
//! (`docs/working/decisions/DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md`,
//! reused via [`crate::host_evidence_standing`]) into a composed frame. It is a
//! pure function over view-models with an injectable `now` — no DB handle, no
//! write path.
//!
//! `settling` and `split` are honest doctrine vocabulary here, **not** derived
//! for a single-witness host packet: `settling` needs real bounded transition
//! evidence (the host VM does not yet project `boot_id`; only `uptime_seconds`),
//! and `split` needs fresh claim-relevant disagreement between ≥2 witnesses.
//! Both are exercised for real in the Service V1 slice.

use serde::Serialize;

use crate::views::{HostEvidenceStanding, HostFreshnessVm, HostSummaryVm, WarningVm};
use crate::HOST_STATE_STALE_THRESHOLD_SECONDS;

/// Composed operational state of a frame. Generalizes the C2 host standing
/// (admissible / stale-testimony / unknown) into the full frame vocabulary.
///
/// The honesty rule (binding, see the gap doc's Design law): `Settling` must be
/// backed by real bounded transition evidence and `Split` by fresh
/// claim-relevant disagreement. Neither may be synthesized from "a value
/// changed" or "freshness exists". Host V0 never returns either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameState {
    /// Relevant witnesses bind inside the declared window.
    Coherent,
    /// A bounded transition is in progress and the window has not closed.
    /// (Doctrine vocabulary; not derived in Host V0.)
    Settling,
    /// Evidence is not contradictory but too old to assert as current.
    Stale,
    /// Fresh witnesses disagree in a claim-relevant way.
    /// (Doctrine vocabulary; not derived in Host V0.)
    Split,
    /// A required witness class is missing or outside testimony scope.
    Unbound,
    /// Insufficient evidence to classify more specifically.
    Unknown,
}

/// The epistemic class of the composed claim. A composed claim may guide
/// operators but retains receipt drilldown; it never becomes primary evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimClass {
    /// A direct readout admissible as observed.
    Observed,
    /// Composed from an admissible readout plus folded findings.
    Composed,
    /// (Doctrine vocabulary; not produced by Host V0.)
    Inferred,
    /// (Doctrine vocabulary; not produced by Host V0.)
    Projected,
    /// The readout exists but is too old to assert as current.
    Stale,
    /// No admissible testimony to compose a present claim.
    CannotTestify,
}

/// Derived action guidance. **Not evidence.** A posture is a recommendation
/// traceable to frame state; it does not itself testify to anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorPosture {
    Ignore,
    Observe,
    Investigate,
    Page,
    Suppress,
    CannotDecide,
}

/// One witness bound (or considered) by the frame, with a drilldown route back
/// to its underlying evidence. Keeps the frame honest: every composed claim can
/// be traced to receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WitnessBinding {
    /// Witness/subject kind, e.g. `"host_readout"` or a finding kind.
    pub kind: String,
    /// Subject identity (host name, finding subject, …).
    pub subject: String,
    /// Observation time of this witness (RFC3339), when known.
    pub observed_at: Option<String>,
    /// Age of the observation in seconds, when known.
    pub age_s: Option<i64>,
    /// Drilldown route to the underlying evidence (e.g. `/finding/{kind}/{host}`).
    pub drilldown_ref: Option<String>,
}

/// A composed, present-tense operational frame over delayed witnesses.
///
/// Carries the full contract field set even where a given subject fills only
/// some of it (Host V0 leaves `split_witnesses` empty and `witness_skew_s` at
/// `Some(0)`/`None` because it binds essentially one packet plus folded
/// findings). It is a render artifact — never primary evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HumanNowFrame {
    /// Subject class, `"host"` for V0.
    pub subject_kind: String,
    /// Subject identity, the host name for V0.
    pub subject_id: String,
    /// When this frame was composed (RFC3339, from injectable `now`).
    pub rendered_at: String,
    /// The operational present the frame asserts through (RFC3339): the newest
    /// binding witness's observation time. `None` when nothing binds.
    pub operational_now: Option<String>,
    /// The coherence window, in seconds, inside which witnesses must fall to
    /// bind. Host V0 uses the C2 host freshness horizon.
    pub binding_window_s: i64,
    pub oldest_relevant_witness_at: Option<String>,
    pub newest_relevant_witness_at: Option<String>,
    /// Spread between oldest and newest binding witness, in seconds. `Some(0)`
    /// for a single-witness host frame; meaningful once ≥2 witnesses bind.
    pub witness_skew_s: Option<i64>,
    pub frame_state: FrameState,
    pub claim_class: ClaimClass,
    pub operator_posture: OperatorPosture,
    /// A human sentence describing what can honestly be composed right now.
    pub composed_claim: String,
    pub supporting_witnesses: Vec<WitnessBinding>,
    pub stale_witnesses: Vec<WitnessBinding>,
    /// Fresh claim-relevant disagreements. Always empty for Host V0.
    pub split_witnesses: Vec<WitnessBinding>,
    /// What this frame's claim structurally does not license.
    pub cannot_testify: Vec<String>,
    /// What an operator must NOT infer from this frame.
    pub cannot_infer: Vec<String>,
    /// Drilldown routes to underlying evidence.
    pub receipt_refs: Vec<String>,
}

/// Is this warning an *open signal* finding for the host frame to fold in?
/// Signal (not meta), observed (not suppressed), and not explicitly retired.
fn is_open_signal(w: &WarningVm) -> bool {
    w.finding_class.as_deref().unwrap_or("signal") == "signal"
        && w.visibility_state == "observed"
        && w.basis_state != "retired"
}

/// Drilldown route for a finding: `/finding/{kind}/{host}[/{subject}]`.
fn finding_ref(w: &WarningVm) -> String {
    match &w.subject {
        Some(s) if !s.is_empty() => format!("/finding/{}/{}/{}", w.category, w.host, s),
        _ => format!("/finding/{}/{}", w.category, w.host),
    }
}

fn rfc3339(t: time::OffsetDateTime) -> Option<String> {
    t.format(&time::format_description::well_known::Rfc3339).ok()
}

/// Build the Host Human Now Frame for one host. Pure over view-models with an
/// injectable `now` (mirrors [`crate::host_evidence_standing`]). Reuses the C2
/// Regime A standing already computed in `freshness`; Regime B display
/// staleness stays on `host.stale`.
///
/// `findings` is the full overview warning set; this filters to `host`.
pub fn host_now_frame(
    host: &HostSummaryVm,
    freshness: Option<&HostFreshnessVm>,
    findings: &[WarningVm],
    now: time::OffsetDateTime,
) -> HumanNowFrame {
    let standing = freshness
        .map(|f| f.evidence_standing.clone())
        .unwrap_or(HostEvidenceStanding::Unknown);
    let observed_age_s = freshness.and_then(|f| f.observed_age_s);

    // Observed timestamp of the host readout packet (the one binding witness),
    // reconstructed from now - age. `None` when standing is Unknown.
    let observed_at = observed_age_s
        .map(|age| now - time::Duration::seconds(age.max(0)))
        .and_then(rfc3339);

    let open: Vec<&WarningVm> = findings
        .iter()
        .filter(|w| w.host == host.host && is_open_signal(w))
        .collect();
    let open_count = open.len();

    let host_witness = WitnessBinding {
        kind: "host_readout".to_string(),
        subject: host.host.clone(),
        observed_at: observed_at.clone(),
        age_s: observed_age_s,
        drilldown_ref: Some(format!("/api/host/{}", host.host)),
    };
    let finding_bindings: Vec<WitnessBinding> = open
        .iter()
        .map(|w| WitnessBinding {
            kind: w.category.clone(),
            subject: w.subject.clone().unwrap_or_default(),
            observed_at: w.first_seen_at.clone(),
            age_s: None,
            drilldown_ref: Some(finding_ref(w)),
        })
        .collect();
    let receipt_refs: Vec<String> = std::iter::once(format!("/api/host/{}", host.host))
        .chain(open.iter().map(|w| finding_ref(w)))
        .collect();

    // Host scope cannot testify about anything above the substrate readout.
    let cannot_infer = vec![
        "external reachability".to_string(),
        "service-path health".to_string(),
        "application-internal state".to_string(),
    ];

    let age_human = observed_age_s
        .map(|a| nq_core::humanize_duration_s(a.max(0)))
        .unwrap_or_else(|| "unknown".to_string());

    let (frame_state, claim_class, posture, composed_claim, supporting, stale, cannot_testify) =
        match standing {
            HostEvidenceStanding::Unknown => (
                FrameState::Unbound,
                ClaimClass::CannotTestify,
                OperatorPosture::CannotDecide,
                format!(
                    "Cannot compose a present operational claim for host {}: \
                     no admissible observation timestamp.",
                    host.host
                ),
                vec![],
                vec![],
                vec![format!(
                    "host {} has no admissible observation timestamp",
                    host.host
                )],
            ),
            HostEvidenceStanding::StaleTestimony => (
                // Stale evidence must NOT render as current healthy state.
                FrameState::Stale,
                ClaimClass::Stale,
                OperatorPosture::Investigate,
                format!(
                    "Host {} last testified {} ago — evidence is too old to assert \
                     current state (binding window {}s).",
                    host.host, age_human, HOST_STATE_STALE_THRESHOLD_SECONDS
                ),
                vec![],
                vec![host_witness.clone()],
                vec![],
            ),
            HostEvidenceStanding::Admissible => {
                let (claim_class, posture) = if open_count == 0 {
                    (ClaimClass::Observed, OperatorPosture::Ignore)
                } else {
                    (ClaimClass::Composed, OperatorPosture::Observe)
                };
                let claim = if open_count == 0 {
                    format!(
                        "Host {} readout admissible, observed {} ago; no open findings.",
                        host.host, age_human
                    )
                } else {
                    format!(
                        "Host {} readout admissible, observed {} ago; {} open finding(s) folded.",
                        host.host, age_human, open_count
                    )
                };
                let mut supporting = vec![host_witness.clone()];
                supporting.extend(finding_bindings.clone());
                (
                    FrameState::Coherent,
                    claim_class,
                    posture,
                    claim,
                    supporting,
                    vec![],
                    vec![],
                )
            }
        };

    HumanNowFrame {
        subject_kind: "host".to_string(),
        subject_id: host.host.clone(),
        rendered_at: rfc3339(now).unwrap_or_default(),
        operational_now: observed_at.clone(),
        binding_window_s: HOST_STATE_STALE_THRESHOLD_SECONDS,
        oldest_relevant_witness_at: observed_at.clone(),
        newest_relevant_witness_at: observed_at,
        // Single binding witness (the host packet) → zero skew when known.
        witness_skew_s: observed_age_s.map(|_| 0),
        frame_state,
        claim_class,
        operator_posture: posture,
        composed_claim,
        supporting_witnesses: supporting,
        stale_witnesses: stale,
        split_witnesses: vec![],
        cannot_testify,
        cannot_infer,
        receipt_refs,
    }
}

impl FrameState {
    /// Stable label for rendering / JSON. Never an unqualified `stale`
    /// (honors the C2 UI rule).
    pub fn label(&self) -> &'static str {
        match self {
            FrameState::Coherent => "coherent",
            FrameState::Settling => "settling",
            FrameState::Stale => "stale (evidence too old to assert now)",
            FrameState::Split => "split",
            FrameState::Unbound => "unbound (cannot compose a present claim)",
            FrameState::Unknown => "unknown",
        }
    }
}

impl OperatorPosture {
    pub fn label(&self) -> &'static str {
        match self {
            OperatorPosture::Ignore => "ignore",
            OperatorPosture::Observe => "observe",
            OperatorPosture::Investigate => "investigate",
            OperatorPosture::Page => "page",
            OperatorPosture::Suppress => "suppress",
            OperatorPosture::CannotDecide => "cannot decide",
        }
    }
}

impl ClaimClass {
    pub fn label(&self) -> &'static str {
        match self {
            ClaimClass::Observed => "observed",
            ClaimClass::Composed => "composed",
            ClaimClass::Inferred => "inferred",
            ClaimClass::Projected => "projected",
            ClaimClass::Stale => "stale",
            ClaimClass::CannotTestify => "cannot testify",
        }
    }
}
