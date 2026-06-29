//! Gateway-report-vs-path reachable-drift probe — verdict core
//! (`nq.probe.gateway_path.v1`).
//!
//! Third reachable-drift specimen (`PFSENSE_REACHABLE_DRIFT_STEP0_INVENTORY.md`
//! check #3). Like the lease-presence core, it is deliberately a *cheap
//! non-lift*: it exists to make a refusal legible, not to manufacture an
//! outage verdict.
//!
//!   pfSense's gateway-monitoring daemon (`dpinger`) pings a configured
//!   monitor IP and exposes raw metrics (`<name> <rtt_us> <stddev_us>
//!   <loss_pct>`) on a unix socket. That is a `pfSenseRuntimeReport` from ONE
//!   vantage about ONE monitored path — never internet-reachability truth.
//!   dpinger reaching its monitor and an NQ probe reaching some destination
//!   are different witnesses; a disagreement is PATH AMBIGUITY, not "WAN
//!   down" / "ISP outage" / "internet down" / "user impact."
//!
//! Witness-custody discipline (operator-directed, 2026-06-24): the dpinger
//! socket is the FIRST-CLASS witness — raw daemon metrics, read directly. We
//! do NOT reimplement pfSense's UI classification (`online`/`loss`/`delay`/
//! `highloss`/...) and treat it as authority; that report is a second-order
//! projection over this same daemon/config state, admissible only later as a
//! separate "pfSense *reports* status X" comparator receipt, never the base
//! specimen. The only thing we read from the raw metrics is whether the
//! daemon is currently receiving replies from its monitor (`loss < 100`) —
//! the minimal signal, not a health grade.
//!
//! Lost custody is first-class: socket-absent, socket-unreadable, and
//! unknown-custody are distinct `cannot_testify` verdicts — "we could not
//! read the daemon" must never collapse into "the gateway is down."
//!
//! This module is the **pure verdict core**, split from the live read the
//! same way the TLS and lease-presence probes split verdict from transport.
//! It turns (a dpinger report, zero or more independent path observations,
//! the probe clock) into a typed receipt — fully fixture-testable, no SSH, no
//! network. The live slice fills these inputs.
//!
//! Source typing (per the Step-0 inventory):
//!   - the dpinger socket metrics are a `pfSenseRuntimeReport`
//!   - an NQ path probe from a named vantage is `ObservedReachability`
//!   - a disagreement is at most `path_ambiguous` — never `wan_down`.
//!
//! Lane separation (non-negotiable): active-witness lane, receipt only. No
//! write to the passive collector's evidence tables, no coercion to an
//! operational/green status — the output is a typed verdict, never `is_ok()`.
//!
//! Concrete receipt, not a generic `ProbeReceipt<T>`. This is the 3rd probe
//! family after TLS and lease-presence; the shared-column extraction
//! (`ClockBasis`, `Perturbation`, vantage/observation basis) is now visibly
//! repeated across three consumers (see `INTEGRATION_SURFACE_GAP.md` /
//! WITNESS_SURFACE). A shared home is genuinely pressured now — but promotion
//! is its own packet, NOT done here (name early, ratify lazily).

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const GATEWAY_PATH_PROBE_SCHEMA: &str = "nq.probe.gateway_path.v1";

/// The clock the receipt was pinned to — point-in-time reachability rides on
/// it. An unwitnessed clock makes "observed at T" theatre with a timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClockBasis {
    /// e.g. `system_ntp`, `unknown`.
    pub source: String,
    /// `recorded` when NTP sync state was observed; `unknown` otherwise.
    pub ntp_status: String,
}

/// Custody of the dpinger gateway-monitor witness. Keeps "we couldn't read
/// the daemon" strictly distinct from "the gateway is down." Lost custody is
/// a refusal, never an outage verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DpingerCustody {
    /// A parseable status line was read from the daemon socket.
    MetricsPresent,
    /// No dpinger socket for this gateway (daemon not running / no such
    /// gateway). The witness is absent — not down.
    SocketAbsent,
    /// Socket present but unreadable (timeout / empty / refused). The witness
    /// is present but mute — not down.
    SocketUnreadable,
    /// Socket present and readable, but the content/identity did not reconcile
    /// (unparseable line, or the line's gateway name disagreed with the socket
    /// filename). Custody is in doubt — not down.
    UnknownCustody,
}

/// What pfSense's `dpinger` daemon reports for one gateway — a
/// `pfSenseRuntimeReport`. RAW metrics only; NOT pfSense's UI classification.
/// The daemon pinging its monitor IP is a scoped claim about THAT path, never
/// "the internet works." The live reader fills this from the dpinger socket.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DpingerReport {
    /// Gateway name as pfSense/dpinger knows it (e.g. `WAN_DHCP`).
    pub gateway_name: String,
    /// The monitor IP dpinger pings to derive its metrics (from the socket
    /// filename). This is what dpinger reaches — NOT what an NQ probe targets,
    /// unless a path observation is deliberately aimed at it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor_ip: Option<String>,
    /// The source/bind address dpinger pings from (the WAN-side address, from
    /// the socket filename). Recorded for custody, not used in the verdict.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,
    pub custody: DpingerCustody,
    /// dpinger's reported round-trip time in microseconds, if metrics present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtt_us: Option<u64>,
    /// dpinger's reported RTT standard deviation in microseconds, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stddev_us: Option<u64>,
    /// dpinger's reported loss percentage, if metrics present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loss_pct: Option<f64>,
    /// Where this report came from (e.g. `ssh:<pfsense-host> dpinger:<sock>`).
    pub source: String,
}

impl DpingerReport {
    /// Whether the daemon is currently receiving replies from its monitor —
    /// the minimal raw read (`loss < 100`), NOT pfSense's up/down threshold
    /// classification. `None` when custody can't testify, or when metrics are
    /// present but the loss figure is missing (we refuse to guess).
    fn reaches_monitor(&self) -> Option<bool> {
        match self.custody {
            DpingerCustody::MetricsPresent => self.loss_pct.map(|l| l < 100.0),
            _ => None,
        }
    }
}

/// Which path this observation tests, so the receipt keeps the two probes as
/// separate witnesses (operator-directed: "more observations, stricter claim
/// boundaries"), never blended into one "WAN health" blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathRole {
    /// Probing the same monitor IP dpinger watches — the direct comparator.
    MonitorTarget,
    /// Probing a fixed public anchor (e.g. `1.1.1.1`) — general WAN egress.
    EgressAnchor,
}

/// How a path observation was attempted, from a named independent vantage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathMethod {
    /// ICMP echo to a target across the path (preferred — lowest perturbation).
    IcmpEcho,
    /// TCP connect to a target:port across the path (fallback when ICMP cannot
    /// testify).
    TcpConnect,
}

impl PathMethod {
    /// Every path method here is an NQ probe from an independent vantage —
    /// `observed_reachability`. (The dpinger report is the box's own report,
    /// carried separately on `DpingerReport`, not as a `PathObservation`.)
    fn testimony_type(self) -> &'static str {
        match self {
            PathMethod::IcmpEcho | PathMethod::TcpConnect => "observed_reachability",
        }
    }
}

/// Outcome of one path attempt. `NotAttempted` keeps an entry's basis honest
/// when a method was declared but not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathOutcome {
    /// The target was reached across the path (echo replied / connect
    /// succeeded / HTTP responded).
    Reached,
    /// No response within the method's basis (silent / timed out / refused).
    /// NOT "WAN down" — silence is from one vantage to one target at one time.
    NotReached,
    NotAttempted,
}

/// One path observation from a named, independent vantage, with the role,
/// target, and time of the attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathObservation {
    pub method: PathMethod,
    pub role: PathRole,
    /// Where the observation was made (e.g. `sushi-k-lan`). MUST be
    /// independent of the pfSense box.
    pub vantage: String,
    /// The destination probed across the path (e.g. `198.51.100.1`, `1.1.1.1`).
    /// Recorded so "reached" can't be read as "the internet is up" — it is
    /// reachability to THIS target only.
    pub target: String,
    pub outcome: PathOutcome,
    pub observed_at: String,
    /// The testimony type of this observation, recorded so a report cannot
    /// silently graduate to reachability truth.
    pub testimony_type: &'static str,
}

impl PathObservation {
    pub fn new(
        method: PathMethod,
        role: PathRole,
        vantage: impl Into<String>,
        target: impl Into<String>,
        outcome: PathOutcome,
        observed_at: OffsetDateTime,
    ) -> Self {
        PathObservation {
            method,
            role,
            vantage: vantage.into(),
            target: target.into(),
            outcome,
            observed_at: rfc3339(observed_at),
            testimony_type: method.testimony_type(),
        }
    }
}

/// Perturbation accounting — even reads are transitions. Reading the dpinger
/// socket over SSH is passive; an independent ICMP/TCP/HTTP path probe leaves
/// a trace (and, across an upstream that runs IDS/IPS, may be classified as a
/// scan). The live slice fills `observed_secondary_effects`; the core declares
/// the expectation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Perturbation {
    pub class: &'static str,
    pub expected_side_effects: Vec<&'static str>,
}

/// Candidate verdict ladder — reality-derived, NOT a final taxonomy. Custody
/// refusals come first (a mute/absent daemon is never an outage). Every
/// non-refusal state is a *non-lift*: the strongest positive is "the monitored
/// path is corroborated from this vantage at this time," and the interesting
/// negatives are "path ambiguity" / "egress trouble (still not WAN-down)."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayPathVerdict {
    /// No dpinger socket — the gateway-monitor witness is absent. NOT down.
    CannotTestifyDpingerSocketAbsent,
    /// dpinger socket present but unreadable (timeout/empty). NOT down.
    CannotTestifyDpingerSocketUnreadable,
    /// dpinger readable but identity/format did not reconcile. NOT down.
    CannotTestifyUnknownCustody,
    /// dpinger testifies, but NO independent path observation was attempted —
    /// no basis to corroborate or contradict the monitored path.
    CannotTestifyNoPathBasis,
    /// dpinger reaches its monitor AND every attempted independent probe
    /// reached its target — the monitored path is corroborated from this
    /// vantage at this time (nothing stronger is claimed).
    CorroboratedByPath,
    /// dpinger and the independent probe(s) DISAGREE (any mismatch among the
    /// attempted observations and the daemon's monitor-reach). The specimen's
    /// point: path ambiguity from this vantage — NOT wan-down, NOT isp-outage,
    /// NOT internet-down, NOT user-impact. The per-role observations record
    /// WHICH path diverged.
    PathAmbiguous,
    /// dpinger is NOT reaching its monitor AND every attempted probe failed —
    /// stronger evidence of egress trouble, but STILL not proof of WAN-down
    /// (could be the box, the modem, the carrier/CGNAT, every target, or this
    /// vantage's own egress).
    EgressTroubleNotWanDown,
}

/// `nq.probe.gateway_path.v1`. Receipt-only; typed verdict, no coercion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GatewayPathReceipt {
    pub schema: &'static str,
    pub probe_kind: &'static str,
    pub dpinger: DpingerReport,
    pub observations: Vec<PathObservation>,
    pub probe_time: String,
    pub clock_basis: ClockBasis,
    pub perturbation: Perturbation,
    pub verdict: GatewayPathVerdict,
    pub non_claims: Vec<String>,
}

fn rfc3339(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_default()
}

/// The fixed scope ceiling: what gateway-report-vs-path does NOT witness. This
/// is the load-bearing part of the specimen — the refusals are the product.
fn scope_ceiling_non_claims() -> Vec<String> {
    vec![
        "a dpinger gateway report is pfSense's own monitoring daemon pinging its configured monitor IP from one vantage — a scoped claim about THAT path, not internet-reachability truth".to_string(),
        "dpinger rtt/stddev/loss are raw daemon metrics, not pfSense's up/down classification and not a service-quality verdict".to_string(),
        "dpinger reaching its monitor does not establish end-to-end reachability to any other destination".to_string(),
        "cannot testify the WAN is down".to_string(),
        "cannot testify the ISP is down or the internet is down".to_string(),
        "cannot testify users or services are impacted".to_string(),
        "cannot testify which hop fails (the box, the modem, the carrier/CGNAT, the target, or the prober's own egress)".to_string(),
        "an independent path probe that does not reach is failure from one vantage to one target at one time, not a down WAN".to_string(),
        "a disagreement between dpinger and an independent probe is path ambiguity, never proof of an outage".to_string(),
        "a dpinger socket that is absent or unreadable is cannot_testify (lost witness custody), not gateway-down".to_string(),
        "can testify only: whether dpinger's monitor view is corroborated by independent path observations from the named vantage(s) to the named target(s)".to_string(),
    ]
}

/// Pure, clock-injected verdict. `now` is recorded as `probe_time`; it does
/// not change the verdict (reachability is point-in-time from the
/// observations), but it pins when the comparison was assessed. No network, no
/// SSH — the live reader supplies `dpinger` and `observations`.
pub fn evaluate_gateway_path(
    dpinger: &DpingerReport,
    observations: &[PathObservation],
    clock: &ClockBasis,
    now: OffsetDateTime,
) -> GatewayPathReceipt {
    let verdict = compute_verdict(dpinger, observations);

    // Perturbation expectation depends on whether any path probe was actually
    // run; reading the dpinger socket alone is passive.
    let used_path_probe = observations
        .iter()
        .any(|o| o.outcome != PathOutcome::NotAttempted);
    let perturbation = if used_path_probe {
        Perturbation {
            class: "active_path_probe",
            expected_side_effects: vec![
                "icmp_tcp_or_http_packet_to_target",
                "possible_upstream_or_ids_log",
            ],
        }
    } else {
        Perturbation {
            class: "passive_report_read",
            expected_side_effects: vec!["read_only_dpinger_socket"],
        }
    };

    GatewayPathReceipt {
        schema: GATEWAY_PATH_PROBE_SCHEMA,
        probe_kind: "gateway_path",
        dpinger: dpinger.clone(),
        observations: observations.to_vec(),
        probe_time: rfc3339(now),
        clock_basis: clock.clone(),
        perturbation,
        verdict,
        non_claims: scope_ceiling_non_claims(),
    }
}

/// Verdict logic. Custody refusals first — a daemon we could not read is never
/// an outage. With metrics present, compare dpinger's monitor-reach against
/// the *attempted* independent probes: full agreement positive corroborates;
/// full agreement negative is egress trouble (still not WAN-down); ANY
/// disagreement is path ambiguity. The per-role observations carry which path
/// diverged — the verdict deliberately stays a coarse non-lift.
fn compute_verdict(
    dpinger: &DpingerReport,
    observations: &[PathObservation],
) -> GatewayPathVerdict {
    let reaches = match dpinger.custody {
        DpingerCustody::SocketAbsent => {
            return GatewayPathVerdict::CannotTestifyDpingerSocketAbsent
        }
        DpingerCustody::SocketUnreadable => {
            return GatewayPathVerdict::CannotTestifyDpingerSocketUnreadable
        }
        DpingerCustody::UnknownCustody => {
            return GatewayPathVerdict::CannotTestifyUnknownCustody
        }
        // Metrics present, but a missing loss figure is itself unreconciled
        // custody — refuse rather than guess "reaches".
        DpingerCustody::MetricsPresent => match dpinger.reaches_monitor() {
            Some(r) => r,
            None => return GatewayPathVerdict::CannotTestifyUnknownCustody,
        },
    };

    let attempted: Vec<PathOutcome> = observations
        .iter()
        .map(|o| o.outcome)
        .filter(|o| *o != PathOutcome::NotAttempted)
        .collect();
    if attempted.is_empty() {
        return GatewayPathVerdict::CannotTestifyNoPathBasis;
    }

    let all_reached = attempted.iter().all(|o| *o == PathOutcome::Reached);
    let none_reached = attempted.iter().all(|o| *o == PathOutcome::NotReached);

    if reaches && all_reached {
        GatewayPathVerdict::CorroboratedByPath
    } else if !reaches && none_reached {
        GatewayPathVerdict::EgressTroubleNotWanDown
    } else {
        GatewayPathVerdict::PathAmbiguous
    }
}

// ---------------------------------------------------------------------------
// External-arrival corroboration (Packet #7c). ADDITIVE: the LAN-side verdict
// above is unchanged. This layer folds the egress-liveness witness (#7b beacon)
// in as a SECOND POSITION — corroborating or diverging — without changing cause
// semantics.
//
// Doctrine:
//   Position diversity can corroborate or create divergence. It cannot launder
//   absence into cause.
// ---------------------------------------------------------------------------

pub const GATEWAY_PATH_COMBINED_SCHEMA: &str = "nq.probe.gateway_path_combined.v1";

/// Coarse classification of the LAN-side verdict into the three shapes the
/// combination cares about. Deliberately collapses every refusal AND path
/// ambiguity into `LanUnknown` — the LAN basis must positively say alive or
/// not-alive before an external position can corroborate or diverge from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LanBasis {
    LanAlive,
    LanNotAlive,
    LanUnknown,
}

impl LanBasis {
    pub fn from_verdict(v: GatewayPathVerdict) -> Self {
        match v {
            GatewayPathVerdict::CorroboratedByPath => LanBasis::LanAlive,
            GatewayPathVerdict::EgressTroubleNotWanDown => LanBasis::LanNotAlive,
            // every cannot_testify AND path_ambiguity: the LAN basis does not
            // cleanly say alive/not-alive, so the combination cannot classify.
            _ => LanBasis::LanUnknown,
        }
    }
}

/// The external vantage's contribution, mirroring `beacon-status.sh`'s
/// `nq.beacon_status.v0` verdicts. `AbsenceAtVantage` is a POSITION fact
/// (external arrival not witnessed), never a cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalArrivalBasis {
    ArrivalWitnessed,
    AbsenceAtVantage,
    NoBasis,
}

/// Parse a `nq.beacon_status.v0` document (from `beacon-status.sh`) into an
/// external basis. Returns `None` if the document is unparseable or carries an
/// unknown verdict — honest absence beats a fabricated position.
pub fn external_basis_from_beacon_status(json: &str) -> Option<ExternalArrivalBasis> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    match v.get("verdict")?.as_str()? {
        "arrival_witnessed" => Some(ExternalArrivalBasis::ArrivalWitnessed),
        "absence_at_vantage" => Some(ExternalArrivalBasis::AbsenceAtVantage),
        "cannot_classify_no_arrivals_basis" => Some(ExternalArrivalBasis::NoBasis),
        _ => None,
    }
}

/// The combined position. Never a cause, never a WAN state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CombinedPosition {
    /// Two independent positions agree (alive-concordant, or trouble-concordant).
    Corroborated,
    /// The two positions disagree — ambiguity to investigate, NOT proof of cause.
    Divergent,
    /// Not enough basis to combine: the LAN side cannot positively classify, or
    /// there is no usable external basis. The external vantage never overrides a
    /// LAN basis that itself cannot testify.
    CannotClassify,
}

/// `nq.probe.gateway_path_combined.v1`. Reporting-only: carries both positions
/// and their combination. `cause_not_inferred` is always true — absence and
/// divergence are never laundered into cause here.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CombinedGatewayPathReport {
    pub schema: &'static str,
    /// The unchanged LAN-side verdict this combination is built over.
    pub lan_verdict: GatewayPathVerdict,
    pub lan_basis: LanBasis,
    /// `None` when no external witness was supplied at all.
    pub external_basis: Option<ExternalArrivalBasis>,
    pub combined: CombinedPosition,
    pub cause_not_inferred: bool,
    pub non_claims: Vec<String>,
}

fn combined_non_claims() -> Vec<String> {
    vec![
        "absence_at_vantage is a position fact (external arrival not witnessed), never a cause — it does not mean WAN-down, ISP-down, or any specific failed hop; it can be the emitter, SSH/key, the vantage host, a route change, or egress".to_string(),
        "divergence between the LAN-side basis and the external position is ambiguity to investigate, never proof of cause or outage".to_string(),
        "the external vantage does not outrank the LAN-side basis; when the LAN basis cannot testify, the combination cannot classify (it is not rescued by the external position)".to_string(),
        "corroboration — including trouble-concordant agreement — reports that two positions agree, not a verified WAN/internet/user-impact state".to_string(),
    ]
}

/// Fold an optional external-arrival basis into the LAN-side gateway-path
/// verdict. Pure, additive, cause-free. The LAN verdict is read, never changed.
pub fn combine_gateway_path_with_external(
    receipt: &GatewayPathReceipt,
    external: Option<ExternalArrivalBasis>,
) -> CombinedGatewayPathReport {
    let lan_basis = LanBasis::from_verdict(receipt.verdict);
    let combined = match (lan_basis, external) {
        // No usable external basis -> cannot combine.
        (_, None) | (_, Some(ExternalArrivalBasis::NoBasis)) => CombinedPosition::CannotClassify,
        // LAN can't positively classify -> external never overrides it.
        (LanBasis::LanUnknown, _) => CombinedPosition::CannotClassify,
        // Concordance (alive-concordant or trouble-concordant).
        (LanBasis::LanAlive, Some(ExternalArrivalBasis::ArrivalWitnessed))
        | (LanBasis::LanNotAlive, Some(ExternalArrivalBasis::AbsenceAtVantage)) => {
            CombinedPosition::Corroborated
        }
        // Discordance.
        (LanBasis::LanAlive, Some(ExternalArrivalBasis::AbsenceAtVantage))
        | (LanBasis::LanNotAlive, Some(ExternalArrivalBasis::ArrivalWitnessed)) => {
            CombinedPosition::Divergent
        }
    };

    CombinedGatewayPathReport {
        schema: GATEWAY_PATH_COMBINED_SCHEMA,
        lan_verdict: receipt.verdict,
        lan_basis,
        external_basis: external,
        combined,
        cause_not_inferred: true,
        non_claims: combined_non_claims(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).expect("rfc3339 fixture")
    }

    fn clock() -> ClockBasis {
        ClockBasis {
            source: "system_ntp".to_string(),
            ntp_status: "recorded".to_string(),
        }
    }

    // NOTE: fixtures are synthetic-but-realistic, anonymized. They mirror the
    // real captured socket shape (`<name> <rtt_us> <stddev_us> <loss_pct>`)
    // without committing the real WAN/monitor addresses — the live read
    // (gitignored runs/) grounds these in the real box.
    fn dpinger_reaching() -> DpingerReport {
        DpingerReport {
            gateway_name: "WAN_DHCP".to_string(),
            monitor_ip: Some("198.51.100.1".to_string()),
            source_ip: Some("198.51.100.129".to_string()),
            custody: DpingerCustody::MetricsPresent,
            rtt_us: Some(3049),
            stddev_us: Some(1866),
            loss_pct: Some(0.0),
            source: "synthetic:fixture".to_string(),
        }
    }

    fn dpinger_not_reaching() -> DpingerReport {
        DpingerReport {
            loss_pct: Some(100.0),
            ..dpinger_reaching()
        }
    }

    fn monitor_probe(target: &str, outcome: PathOutcome) -> PathObservation {
        PathObservation::new(
            PathMethod::IcmpEcho,
            PathRole::MonitorTarget,
            "sushi-k-lan",
            target,
            outcome,
            at("2026-06-24T12:00:00Z"),
        )
    }

    fn anchor_probe(outcome: PathOutcome) -> PathObservation {
        PathObservation::new(
            PathMethod::IcmpEcho,
            PathRole::EgressAnchor,
            "sushi-k-lan",
            "1.1.1.1",
            outcome,
            at("2026-06-24T12:00:00Z"),
        )
    }

    fn eval(d: &DpingerReport, obs: &[PathObservation]) -> GatewayPathReceipt {
        evaluate_gateway_path(d, obs, &clock(), at("2026-06-24T12:00:01Z"))
    }

    #[test]
    fn socket_absent_cannot_testify_not_down() {
        let mut d = dpinger_reaching();
        d.custody = DpingerCustody::SocketAbsent;
        let r = eval(&d, &[anchor_probe(PathOutcome::NotReached)]);
        assert_eq!(r.verdict, GatewayPathVerdict::CannotTestifyDpingerSocketAbsent);
    }

    #[test]
    fn socket_unreadable_cannot_testify() {
        let mut d = dpinger_reaching();
        d.custody = DpingerCustody::SocketUnreadable;
        let r = eval(&d, &[anchor_probe(PathOutcome::Reached)]);
        assert_eq!(
            r.verdict,
            GatewayPathVerdict::CannotTestifyDpingerSocketUnreadable
        );
    }

    #[test]
    fn unknown_custody_cannot_testify() {
        let mut d = dpinger_reaching();
        d.custody = DpingerCustody::UnknownCustody;
        let r = eval(&d, &[anchor_probe(PathOutcome::Reached)]);
        assert_eq!(r.verdict, GatewayPathVerdict::CannotTestifyUnknownCustody);
    }

    /// Metrics present but no loss figure -> we refuse to guess "reaches".
    #[test]
    fn metrics_present_but_no_loss_is_unknown_custody() {
        let mut d = dpinger_reaching();
        d.loss_pct = None;
        let r = eval(&d, &[anchor_probe(PathOutcome::Reached)]);
        assert_eq!(r.verdict, GatewayPathVerdict::CannotTestifyUnknownCustody);
    }

    #[test]
    fn cannot_testify_when_no_path_attempted() {
        let r = eval(&dpinger_reaching(), &[]);
        assert_eq!(r.verdict, GatewayPathVerdict::CannotTestifyNoPathBasis);
        // A declared-but-not-run probe is still no basis.
        let r2 = eval(
            &dpinger_reaching(),
            &[anchor_probe(PathOutcome::NotAttempted)],
        );
        assert_eq!(r2.verdict, GatewayPathVerdict::CannotTestifyNoPathBasis);
    }

    /// dpinger reaches its monitor AND both independent probes reach -> the
    /// monitored path is corroborated from this vantage. The real captured
    /// box (loss 0) lands here.
    #[test]
    fn corroborated_when_dpinger_reaches_and_all_probes_reach() {
        let obs = vec![
            monitor_probe("198.51.100.1", PathOutcome::Reached),
            anchor_probe(PathOutcome::Reached),
        ];
        let r = eval(&dpinger_reaching(), &obs);
        assert_eq!(r.verdict, GatewayPathVerdict::CorroboratedByPath);
    }

    /// dpinger reaches, monitor probe reaches, but the egress anchor fails ->
    /// anchor/path-specific ambiguity (the monitored path agrees, general
    /// egress diverges). NOT WAN-down.
    #[test]
    fn ambiguous_when_anchor_diverges_from_monitor() {
        let obs = vec![
            monitor_probe("198.51.100.1", PathOutcome::Reached),
            anchor_probe(PathOutcome::NotReached),
        ];
        let r = eval(&dpinger_reaching(), &obs);
        assert_eq!(r.verdict, GatewayPathVerdict::PathAmbiguous);
    }

    /// dpinger reaches its monitor, but our independent probe to that same
    /// monitor fails -> monitor-path ambiguity (firewall reaches it, this
    /// vantage doesn't). NOT WAN-down.
    #[test]
    fn ambiguous_when_dpinger_reaches_but_probe_to_monitor_fails() {
        let obs = vec![monitor_probe("198.51.100.1", PathOutcome::NotReached)];
        let r = eval(&dpinger_reaching(), &obs);
        assert_eq!(r.verdict, GatewayPathVerdict::PathAmbiguous);
    }

    /// dpinger is NOT reaching its monitor (loss 100) yet both probes reach ->
    /// firewall-monitor/custody ambiguity. NOT WAN-down.
    #[test]
    fn ambiguous_when_dpinger_silent_but_probes_reach() {
        let obs = vec![
            monitor_probe("198.51.100.1", PathOutcome::Reached),
            anchor_probe(PathOutcome::Reached),
        ];
        let r = eval(&dpinger_not_reaching(), &obs);
        assert_eq!(r.verdict, GatewayPathVerdict::PathAmbiguous);
    }

    /// dpinger NOT reaching its monitor AND every probe fails -> stronger
    /// evidence of egress trouble, but still explicitly NOT WAN-down.
    #[test]
    fn egress_trouble_when_dpinger_silent_and_all_probes_fail() {
        let obs = vec![
            monitor_probe("198.51.100.1", PathOutcome::NotReached),
            anchor_probe(PathOutcome::NotReached),
        ];
        let r = eval(&dpinger_not_reaching(), &obs);
        assert_eq!(r.verdict, GatewayPathVerdict::EgressTroubleNotWanDown);
    }

    /// `reaches_monitor` is the minimal raw read (loss < 100), not a health
    /// grade: 99.9% loss still counts as "receiving some replies".
    #[test]
    fn reaches_monitor_is_loss_below_one_hundred() {
        let mut d = dpinger_reaching();
        d.loss_pct = Some(99.9);
        assert_eq!(d.reaches_monitor(), Some(true));
        d.loss_pct = Some(100.0);
        assert_eq!(d.reaches_monitor(), Some(false));
    }

    #[test]
    fn probe_time_reflects_the_injected_clock() {
        let r = eval(&dpinger_reaching(), &[]);
        assert_eq!(r.probe_time, "2026-06-24T12:00:01Z");
    }

    #[test]
    fn socket_read_is_passive_path_probe_is_active() {
        let r = eval(&dpinger_reaching(), &[]);
        assert_eq!(r.perturbation.class, "passive_report_read");
        let r2 = eval(&dpinger_reaching(), &[anchor_probe(PathOutcome::Reached)]);
        assert_eq!(r2.perturbation.class, "active_path_probe");
    }

    /// The refusals are the product: the receipt must carry the non-lift
    /// non-claims, and must NOT coerce to a green/ok/wan-down status.
    #[test]
    fn receipt_carries_refusals_and_no_coercion() {
        let r = eval(&dpinger_reaching(), &[]);
        assert!(r
            .non_claims
            .iter()
            .any(|c| c.contains("not internet-reachability truth")));
        assert!(r
            .non_claims
            .iter()
            .any(|c| c.contains("cannot testify the WAN is down")));
        assert!(r
            .non_claims
            .iter()
            .any(|c| c.contains("lost witness custody")));

        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["verdict"], "cannot_testify_no_path_basis"); // typed enum, not a bool
        let s = serde_json::to_string(&r).unwrap();
        assert!(
            !s.contains("\"is_ok\"")
                && !s.contains("\"healthy\"")
                && !s.contains("\"green\"")
                && !s.contains("wan_down")
        );
    }

    /// Source typing + role travel on each observation, so a report cannot
    /// silently graduate to observed reachability and the two probes stay
    /// separate witnesses.
    #[test]
    fn observations_carry_role_and_testimony_type() {
        let m = monitor_probe("198.51.100.1", PathOutcome::Reached);
        let a = anchor_probe(PathOutcome::Reached);
        assert_eq!(m.role, PathRole::MonitorTarget);
        assert_eq!(a.role, PathRole::EgressAnchor);
        assert_eq!(m.testimony_type, "observed_reachability");
    }

    // --- Packet #7c: external-arrival corroboration ---

    /// Build a receipt carrying a chosen LAN verdict (the combiner only reads
    /// `.verdict`; constructing via `eval` then overriding keeps the rest real).
    fn receipt_with(v: GatewayPathVerdict) -> GatewayPathReceipt {
        let mut r = eval(&dpinger_reaching(), &[anchor_probe(PathOutcome::Reached)]);
        r.verdict = v;
        r
    }

    fn combine(
        v: GatewayPathVerdict,
        ext: Option<ExternalArrivalBasis>,
    ) -> CombinedGatewayPathReport {
        combine_gateway_path_with_external(&receipt_with(v), ext)
    }

    #[test]
    fn alive_plus_arrival_is_corroborated() {
        let c = combine(
            GatewayPathVerdict::CorroboratedByPath,
            Some(ExternalArrivalBasis::ArrivalWitnessed),
        );
        assert_eq!(c.lan_basis, LanBasis::LanAlive);
        assert_eq!(c.combined, CombinedPosition::Corroborated);
        assert!(c.cause_not_inferred);
    }

    #[test]
    fn trouble_plus_absence_is_corroborated_negative_concordance() {
        let c = combine(
            GatewayPathVerdict::EgressTroubleNotWanDown,
            Some(ExternalArrivalBasis::AbsenceAtVantage),
        );
        assert_eq!(c.lan_basis, LanBasis::LanNotAlive);
        assert_eq!(c.combined, CombinedPosition::Corroborated);
    }

    #[test]
    fn alive_plus_absence_is_divergent_not_cause() {
        let c = combine(
            GatewayPathVerdict::CorroboratedByPath,
            Some(ExternalArrivalBasis::AbsenceAtVantage),
        );
        assert_eq!(c.combined, CombinedPosition::Divergent);
        assert!(c.cause_not_inferred);
        // absence is never laundered into a cause.
        assert!(c
            .non_claims
            .iter()
            .any(|s| s.contains("never a cause")));
    }

    #[test]
    fn trouble_plus_arrival_is_divergent() {
        let c = combine(
            GatewayPathVerdict::EgressTroubleNotWanDown,
            Some(ExternalArrivalBasis::ArrivalWitnessed),
        );
        assert_eq!(c.combined, CombinedPosition::Divergent);
    }

    /// The load-bearing refusal: a LAN basis that cannot testify is NOT rescued
    /// by an external arrival. The external vantage never outranks the LAN side.
    #[test]
    fn external_never_overrides_lan_unknown() {
        for v in [
            GatewayPathVerdict::CannotTestifyDpingerSocketAbsent,
            GatewayPathVerdict::CannotTestifyDpingerSocketUnreadable,
            GatewayPathVerdict::CannotTestifyUnknownCustody,
            GatewayPathVerdict::CannotTestifyNoPathBasis,
            GatewayPathVerdict::PathAmbiguous,
        ] {
            let c = combine(v, Some(ExternalArrivalBasis::ArrivalWitnessed));
            assert_eq!(c.lan_basis, LanBasis::LanUnknown);
            assert_eq!(c.combined, CombinedPosition::CannotClassify, "{v:?}");
        }
    }

    #[test]
    fn no_external_or_no_basis_cannot_classify() {
        assert_eq!(
            combine(GatewayPathVerdict::CorroboratedByPath, None).combined,
            CombinedPosition::CannotClassify
        );
        assert_eq!(
            combine(
                GatewayPathVerdict::CorroboratedByPath,
                Some(ExternalArrivalBasis::NoBasis)
            )
            .combined,
            CombinedPosition::CannotClassify
        );
    }

    #[test]
    fn beacon_status_parses_to_external_basis() {
        assert_eq!(
            external_basis_from_beacon_status(r#"{"schema":"nq.beacon_status.v0","verdict":"arrival_witnessed","age_s":1}"#),
            Some(ExternalArrivalBasis::ArrivalWitnessed)
        );
        assert_eq!(
            external_basis_from_beacon_status(r#"{"verdict":"absence_at_vantage"}"#),
            Some(ExternalArrivalBasis::AbsenceAtVantage)
        );
        assert_eq!(
            external_basis_from_beacon_status(r#"{"verdict":"cannot_classify_no_arrivals_basis"}"#),
            Some(ExternalArrivalBasis::NoBasis)
        );
        // Unparseable / unknown verdict -> honest None, never a fabricated position.
        assert_eq!(external_basis_from_beacon_status("not json"), None);
        assert_eq!(
            external_basis_from_beacon_status(r#"{"verdict":"something_else"}"#),
            None
        );
    }
}
