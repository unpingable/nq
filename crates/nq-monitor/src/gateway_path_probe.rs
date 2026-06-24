//! Gateway-report-vs-path reachable-drift probe — verdict core
//! (`nq.probe.gateway_path.v1`).
//!
//! Third reachable-drift specimen (`PFSENSE_REACHABLE_DRIFT_STEP0_INVENTORY.md`
//! candidate check #3). Like the lease-presence core, it is deliberately a
//! *cheap non-lift*: it exists to make a refusal legible, not to manufacture
//! an outage verdict.
//!
//!   A pfSense gateway report (dpinger says WAN = up, RTT r, loss l) is the
//!   box's OWN active-probe self-report from a single vantage — never
//!   internet-reachability truth. dpinger pinging its monitor IP and an
//!   NQ probe traversing the path to some destination are different
//!   witnesses; a mismatch is PATH AMBIGUITY, not "WAN down" / "ISP outage"
//!   / "the internet is down" / "users are impacted."
//!
//! This module is the **pure verdict core**, split from the live read the
//! same way the TLS and lease-presence probes split verdict from transport.
//! It turns (a pfSense gateway report, zero or more external path
//! observations, the probe clock) into a typed receipt — fully
//! fixture-testable, no SSH, no network. The live slice (read dpinger
//! gateway status over read-only SSH; run an ICMP/TCP/HTTP path probe from a
//! named independent vantage) fills these inputs.
//!
//! Source typing (per the Step-0 inventory):
//!   - the dpinger gateway status is a `pfSenseRuntimeReport` (dpinger is
//!     itself an active probe with the classic active-probe ambiguity)
//!   - an NQ path probe from a named vantage is `ObservedReachability`
//!   - a mismatch is at most `gateway_uncorroborated_path_fails` — never
//!     `wan_down` / `internet_down`.
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

/// Gateway status as pfSense (dpinger) reports it. dpinger pings a monitor IP
/// and classifies the gateway; `Up`/`Degraded` both *claim a working path*
/// (Degraded = up-but-lossy/high-latency warning), `Down`/`Unknown` do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayStatus {
    /// dpinger reports the gateway online (no loss/latency alarm).
    Up,
    /// dpinger reports the gateway online but in a packetloss/high-latency
    /// warning state — still a claim that a path exists.
    Degraded,
    /// dpinger reports the gateway down.
    Down,
    /// The reader could not classify the dpinger status.
    Unknown,
}

impl GatewayStatus {
    /// Whether the box is *claiming* a working path. `Up`/`Degraded` claim one
    /// (a claim to corroborate); `Down`/`Unknown` are not a claim of up.
    fn claims_path_up(self) -> bool {
        matches!(self, GatewayStatus::Up | GatewayStatus::Degraded)
    }
}

/// A pfSense gateway report as dpinger reports it — a `pfSenseRuntimeReport`.
/// The live reader fills this from `dpinger` status / the gateway status API
/// over read-only SSH. RTT/loss are dpinger's measurements to ITS monitor IP,
/// recorded for context, never a service-quality verdict.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GatewayReport {
    /// Gateway name as pfSense knows it (e.g. `WAN_DHCP`).
    pub name: String,
    /// The monitor IP dpinger pings to derive the status (e.g. `1.1.1.1`).
    /// This is what dpinger reaches — NOT what an NQ path probe targets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor_ip: Option<String>,
    pub status: GatewayStatus,
    /// dpinger's reported round-trip time in milliseconds, if read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<f64>,
    /// dpinger's reported loss percentage, if read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loss_pct: Option<f64>,
    /// Where this gateway report came from (e.g. `ssh:<pfsense-host> dpinger`).
    pub source: String,
}

/// How a path observation was attempted, from a named independent vantage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathMethod {
    /// ICMP echo to a target across the path.
    IcmpEcho,
    /// TCP connect to a target:port across the path.
    TcpConnect,
    /// HTTP(S) GET to a target across the path.
    HttpGet,
}

impl PathMethod {
    /// Every path method here is an NQ probe from an independent vantage —
    /// `observed_reachability`. (The dpinger report is the box's own report,
    /// carried separately on `GatewayReport`, not as a `PathObservation`.)
    fn testimony_type(self) -> &'static str {
        match self {
            PathMethod::IcmpEcho | PathMethod::TcpConnect | PathMethod::HttpGet => {
                "observed_reachability"
            }
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

/// One path observation from a named, independent vantage, with the target it
/// probed and the time it was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathObservation {
    pub method: PathMethod,
    /// Where the observation was made (e.g. `nq-vantage-lan`,
    /// `nq-vantage-external`). MUST be independent of the pfSense box.
    pub vantage: String,
    /// The destination probed across the path (e.g. `1.1.1.1`,
    /// `https://example.org`). Recorded so "reached" can't be read as "the
    /// internet is up" — it is reachability to THIS target only.
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
        vantage: impl Into<String>,
        target: impl Into<String>,
        outcome: PathOutcome,
        observed_at: OffsetDateTime,
    ) -> Self {
        PathObservation {
            method,
            vantage: vantage.into(),
            target: target.into(),
            outcome,
            observed_at: rfc3339(observed_at),
            testimony_type: method.testimony_type(),
        }
    }
}

/// Perturbation accounting — even reads are transitions. Reading the dpinger
/// status over SSH is passive; an external ICMP/TCP/HTTP path probe leaves a
/// trace (and, across an upstream that runs IDS/IPS, may be classified as a
/// scan). The live slice fills `observed_secondary_effects`; the core
/// declares the expectation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Perturbation {
    pub class: &'static str,
    pub expected_side_effects: Vec<&'static str>,
}

/// Candidate verdict ladder — reality-derived, NOT a final taxonomy. Every
/// state is deliberately a *non-lift*: the strongest positive is "path
/// reached from this vantage to this target at this time," and the
/// interesting negative is "gateway up-report not corroborated by the path,"
/// never "WAN down" / "internet down."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayPathVerdict {
    /// dpinger does NOT report the gateway up (down/unknown). The box itself
    /// isn't claiming a working path; there is no up-report to corroborate.
    /// This records the box's report — it does NOT independently witness an
    /// outage.
    GatewayNotReportedUp,
    /// Gateway reports up, but NO path observation was attempted — no basis to
    /// corroborate or refute the up-report.
    CannotTestifyNoPathBasis,
    /// Gateway reports up; a path observation was attempted and the path was
    /// NOT reached. The specimen's point: dpinger's up-report uncorroborated
    /// by an external path observation. NOT wan-down, NOT isp-outage, NOT
    /// internet-down, NOT user-impact — path ambiguity from one vantage.
    GatewayUncorroboratedPathFails,
    /// Gateway reports up AND a path observation reached its target — path
    /// reached from that vantage to that target at that time (nothing
    /// stronger is claimed).
    GatewayCorroboratedByPath,
}

/// `nq.probe.gateway_path.v1`. Receipt-only; typed verdict, no coercion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GatewayPathReceipt {
    pub schema: &'static str,
    pub probe_kind: &'static str,
    pub gateway: GatewayReport,
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
        "a pfSense gateway report (dpinger) is the box's own active-probe self-report from one vantage, not internet-reachability truth".to_string(),
        "a gateway reported up does not establish end-to-end reachability to any destination".to_string(),
        "cannot testify the WAN is down".to_string(),
        "cannot testify the ISP is down or the internet is down".to_string(),
        "cannot testify users or services are impacted".to_string(),
        "cannot testify which hop fails (the box, the modem, the ISP, the target, or the prober's own egress)".to_string(),
        "a path probe that does not reach is failure from one vantage to one target at one time, not a down WAN".to_string(),
        "dpinger RTT/loss are measurements to its own monitor IP, not a service-quality verdict".to_string(),
        "wan_down / internet_down would require a declared multi-vantage probe regime (vantages + targets + schedule + scope) absent from this specimen".to_string(),
        "can testify only: whether the gateway's up-report is corroborated by an external path observation from the named vantage(s) to the named target(s)".to_string(),
    ]
}

/// Pure, clock-injected verdict. `now` is recorded as `probe_time`; it does
/// not change the verdict (reachability is point-in-time from the
/// observations), but it pins when the corroboration was assessed. No
/// network, no SSH — the live reader supplies `gateway` and `observations`.
pub fn evaluate_gateway_path(
    gateway: &GatewayReport,
    observations: &[PathObservation],
    clock: &ClockBasis,
    now: OffsetDateTime,
) -> GatewayPathReceipt {
    let verdict = compute_verdict(gateway, observations);

    // Perturbation expectation depends on whether any path probe was actually
    // run; reading the dpinger status alone is passive.
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
            expected_side_effects: vec!["read_only_dpinger_gateway_status"],
        }
    };

    GatewayPathReceipt {
        schema: GATEWAY_PATH_PROBE_SCHEMA,
        probe_kind: "gateway_path",
        gateway: gateway.clone(),
        observations: observations.to_vec(),
        probe_time: rfc3339(now),
        clock_basis: clock.clone(),
        perturbation,
        verdict,
        non_claims: scope_ceiling_non_claims(),
    }
}

/// Verdict logic. The gateway must report up (claim a path) for a path
/// observation to corroborate anything; then the strongest *attempted*
/// observation decides corroboration. A reach on any observation corroborates;
/// an all-attempted-none-reached set is the uncorroborated specimen; no
/// attempt at all cannot testify. A not-up report has no up-claim to test.
fn compute_verdict(
    gateway: &GatewayReport,
    observations: &[PathObservation],
) -> GatewayPathVerdict {
    if !gateway.status.claims_path_up() {
        return GatewayPathVerdict::GatewayNotReportedUp;
    }

    let any_reached = observations
        .iter()
        .any(|o| o.outcome == PathOutcome::Reached);
    if any_reached {
        return GatewayPathVerdict::GatewayCorroboratedByPath;
    }

    let any_attempted = observations
        .iter()
        .any(|o| o.outcome != PathOutcome::NotAttempted);
    if any_attempted {
        GatewayPathVerdict::GatewayUncorroboratedPathFails
    } else {
        GatewayPathVerdict::CannotTestifyNoPathBasis
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

    // NOTE: fixtures are synthetic-but-realistic. No pfSense was read to build
    // them — the live SSH read slice (gated on access) grounds these in real
    // dpinger status + real path-probe data, the way the lease-presence core
    // was grounded in the real box's lease/ARP.
    fn gateway_up() -> GatewayReport {
        GatewayReport {
            name: "WAN_DHCP".to_string(),
            monitor_ip: Some("1.1.1.1".to_string()),
            status: GatewayStatus::Up,
            rtt_ms: Some(12.4),
            loss_pct: Some(0.0),
            source: "synthetic:fixture".to_string(),
        }
    }

    #[test]
    fn corroborated_when_path_probe_reaches_a_target() {
        let obs = vec![PathObservation::new(
            PathMethod::IcmpEcho,
            "nq-vantage-external",
            "1.1.1.1",
            PathOutcome::Reached,
            at("2026-06-24T12:00:00Z"),
        )];
        let r = evaluate_gateway_path(&gateway_up(), &obs, &clock(), at("2026-06-24T12:00:01Z"));
        assert_eq!(r.verdict, GatewayPathVerdict::GatewayCorroboratedByPath);
    }

    /// The specimen's reason to exist: dpinger says up, external path probe
    /// attempted and does not reach -> uncorroborated path fails, NOT WAN-down.
    #[test]
    fn uncorroborated_when_path_attempted_but_not_reached() {
        let obs = vec![
            PathObservation::new(
                PathMethod::IcmpEcho,
                "nq-vantage-external",
                "1.1.1.1",
                PathOutcome::NotReached,
                at("2026-06-24T12:00:00Z"),
            ),
            PathObservation::new(
                PathMethod::HttpGet,
                "nq-vantage-external",
                "https://example.org",
                PathOutcome::NotReached,
                at("2026-06-24T12:00:00Z"),
            ),
        ];
        let r = evaluate_gateway_path(&gateway_up(), &obs, &clock(), at("2026-06-24T12:00:01Z"));
        assert_eq!(r.verdict, GatewayPathVerdict::GatewayUncorroboratedPathFails);
    }

    #[test]
    fn cannot_testify_when_no_path_attempted() {
        let r = evaluate_gateway_path(&gateway_up(), &[], &clock(), at("2026-06-24T12:00:01Z"));
        assert_eq!(r.verdict, GatewayPathVerdict::CannotTestifyNoPathBasis);
        // A declared-but-not-run method is still no basis.
        let obs = vec![PathObservation::new(
            PathMethod::TcpConnect,
            "nq-vantage-external",
            "1.1.1.1:443",
            PathOutcome::NotAttempted,
            at("2026-06-24T12:00:00Z"),
        )];
        let r2 = evaluate_gateway_path(&gateway_up(), &obs, &clock(), at("2026-06-24T12:00:01Z"));
        assert_eq!(r2.verdict, GatewayPathVerdict::CannotTestifyNoPathBasis);
    }

    /// A down/unknown gateway report has no up-claim to corroborate, and the
    /// verdict refuses to lift the box's report into a witnessed outage.
    #[test]
    fn gateway_not_up_has_no_up_claim_to_test() {
        for status in [GatewayStatus::Down, GatewayStatus::Unknown] {
            let mut gw = gateway_up();
            gw.status = status;
            // Even an external path that *fails* must not turn the box's own
            // report into a witnessed WAN-down verdict.
            let obs = vec![PathObservation::new(
                PathMethod::IcmpEcho,
                "nq-vantage-external",
                "1.1.1.1",
                PathOutcome::NotReached,
                at("2026-06-24T12:00:00Z"),
            )];
            let r = evaluate_gateway_path(&gw, &obs, &clock(), at("2026-06-24T12:00:01Z"));
            assert_eq!(r.verdict, GatewayPathVerdict::GatewayNotReportedUp);
        }
    }

    /// Degraded (packetloss/high-latency warning) still claims a path is up,
    /// so an unreached path is the uncorroborated specimen, not a pass.
    #[test]
    fn degraded_gateway_still_claims_a_path() {
        let mut gw = gateway_up();
        gw.status = GatewayStatus::Degraded;
        let obs = vec![PathObservation::new(
            PathMethod::IcmpEcho,
            "nq-vantage-external",
            "1.1.1.1",
            PathOutcome::NotReached,
            at("2026-06-24T12:00:00Z"),
        )];
        let r = evaluate_gateway_path(&gw, &obs, &clock(), at("2026-06-24T12:00:01Z"));
        assert_eq!(r.verdict, GatewayPathVerdict::GatewayUncorroboratedPathFails);
    }

    /// Reached wins over a not-reached sibling — one positive corroborates.
    #[test]
    fn mixed_observations_corroborate_on_any_reach() {
        let obs = vec![
            PathObservation::new(
                PathMethod::IcmpEcho,
                "nq-vantage-external",
                "1.1.1.1",
                PathOutcome::NotReached,
                at("2026-06-24T12:00:00Z"),
            ),
            PathObservation::new(
                PathMethod::TcpConnect,
                "nq-vantage-external",
                "1.1.1.1:443",
                PathOutcome::Reached,
                at("2026-06-24T12:00:00Z"),
            ),
        ];
        let r = evaluate_gateway_path(&gateway_up(), &obs, &clock(), at("2026-06-24T12:00:01Z"));
        assert_eq!(r.verdict, GatewayPathVerdict::GatewayCorroboratedByPath);
    }

    #[test]
    fn probe_time_reflects_the_injected_clock() {
        let r = evaluate_gateway_path(&gateway_up(), &[], &clock(), at("2026-06-24T12:00:01Z"));
        assert_eq!(r.probe_time, "2026-06-24T12:00:01Z");
    }

    #[test]
    fn report_read_is_passive_path_probe_is_active() {
        let r = evaluate_gateway_path(&gateway_up(), &[], &clock(), at("2026-06-24T12:00:01Z"));
        assert_eq!(r.perturbation.class, "passive_report_read");

        let probe = vec![PathObservation::new(
            PathMethod::IcmpEcho,
            "nq-vantage-external",
            "1.1.1.1",
            PathOutcome::Reached,
            at("2026-06-24T12:00:00Z"),
        )];
        let r2 = evaluate_gateway_path(&gateway_up(), &probe, &clock(), at("2026-06-24T12:00:01Z"));
        assert_eq!(r2.perturbation.class, "active_path_probe");
    }

    /// The refusals are the product: the receipt must carry the non-lift
    /// non-claims, and must NOT coerce to a green/ok status.
    #[test]
    fn receipt_carries_refusals_and_no_coercion() {
        let r = evaluate_gateway_path(&gateway_up(), &[], &clock(), at("2026-06-24T12:00:01Z"));
        assert!(r
            .non_claims
            .iter()
            .any(|c| c.contains("not internet-reachability truth")));
        assert!(r
            .non_claims
            .iter()
            .any(|c| c.contains("cannot testify the WAN is down")));

        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["verdict"], "cannot_testify_no_path_basis"); // typed enum, not a bool
        let s = serde_json::to_string(&r).unwrap();
        assert!(!s.contains("\"is_ok\"") && !s.contains("\"healthy\"") && !s.contains("\"green\""));
    }

    /// Source typing travels on each observation so a pfSense report cannot
    /// silently graduate to observed reachability.
    #[test]
    fn observations_carry_testimony_type() {
        let obs = vec![PathObservation::new(
            PathMethod::HttpGet,
            "nq-vantage-external",
            "https://example.org",
            PathOutcome::Reached,
            at("2026-06-24T12:00:00Z"),
        )];
        assert_eq!(obs[0].testimony_type, "observed_reachability");
    }
}
