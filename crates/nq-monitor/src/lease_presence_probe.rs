//! Lease-vs-presence reachable-drift probe — verdict core (`nq.probe.lease_presence.v1`).
//!
//! First reachable-drift specimen (`PFSENSE_REACHABLE_DRIFT_SPECIMEN.md`,
//! Step-0 inventory). Deliberately the *cheap non-lift* one: it exists to
//! make a refusal legible, not to manufacture a contradiction.
//!
//!   An active DHCP lease does NOT establish current host presence.
//!   Leases outlive hosts; ARP residue is the box's own self-report; a
//!   silent probe is not a down host.
//!
//! This module is the **pure verdict core**, split from the live read the
//! way the TLS probe split its verdict from the rustls transport. It turns
//! (a pfSense lease report, zero or more presence observations, the probe
//! clock) into a typed receipt — fully fixture-testable, no SSH, no network.
//! The live slice (read leases over SSH `pfctl`/lease file, ARP via
//! `arp -an`, an optional probe from a named vantage) fills these inputs.
//!
//! Source typing (per the Step-0 inventory):
//!   - the DHCP lease is a `pfSenseRuntimeReport` (active) / `HistoricalReport` (expired)
//!   - ARP/NDP residue is a `pfSenseRuntimeReport` (the box saw a MAC)
//!   - an NQ probe from a named vantage is `ObservedReachability`
//!   - a mismatch is at most `lease_uncorroborated` — never `host_down`.
//!
//! Lane separation (non-negotiable): active-witness lane, receipt only. No
//! write to the passive collector's evidence tables, no coercion to an
//! operational/green status — the output is a typed verdict, never `is_ok()`.
//!
//! Concrete receipt, not a generic `ProbeReceipt<T>`. This is the 2nd probe
//! family after TLS; the shared-column extraction (`ClockBasis`,
//! `Perturbation`, response/vantage basis) is now genuinely pressured
//! (see `INTEGRATION_SURFACE_GAP.md` / WITNESS_SURFACE) but is NOT done here
//! — promote a shared home only when a third consumer needs it.

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const LEASE_PRESENCE_PROBE_SCHEMA: &str = "nq.probe.lease_presence.v1";

/// The clock the receipt was pinned to — point-in-time presence rides on it.
/// An unwitnessed clock makes "observed at T" theatre with a timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClockBasis {
    /// e.g. `system_ntp`, `unknown`.
    pub source: String,
    /// `recorded` when NTP sync state was observed; `unknown` otherwise.
    pub ntp_status: String,
}

/// DHCP lease state as pfSense reports it. `Static` = a static mapping (no
/// expiry); `Active`/`Expired`/`Released` are dynamic lease states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseState {
    Active,
    Expired,
    Released,
    /// Static mapping (claims an occupant by config). Not produced by the
    /// ISC dynamic-lease reader yet — a config-read slice would mint it.
    #[allow(dead_code)]
    Static,
    /// The reader could not classify the lease state.
    Unknown,
}

impl LeaseState {
    /// Whether the lease currently *claims* an occupant. `Static` mappings
    /// claim an address by configuration; dynamic `Active` claims it by a
    /// live lease. Both are claims to corroborate, not presence facts.
    fn claims_occupant(self) -> bool {
        matches!(self, LeaseState::Active | LeaseState::Static)
    }
}

/// A DHCP lease as pfSense reports it. `pfSenseRuntimeReport` when active,
/// `HistoricalReport` when expired/released. The live reader fills this from
/// the lease file (ISC `dhcpd.leases`) or the Kea lease store, over SSH.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LeaseReport {
    /// Hostname pfSense recorded for the lease, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    /// The leased address — the subject whose presence we try to corroborate.
    pub ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    pub state: LeaseState,
    /// Where this lease report came from (e.g. `ssh:<pfsense-host> isc-dhcpd`).
    pub source: String,
}

/// How a presence observation was attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceMethod {
    /// pfSense's own ARP table (a `pfSenseRuntimeReport` of L2 residue).
    PfsenseArp,
    /// pfSense's own NDP table (IPv6). Not produced by the v1 ARP reader yet.
    #[allow(dead_code)]
    PfsenseNdp,
    /// An NQ probe from an independent vantage (`ObservedReachability`).
    IcmpEcho,
    TcpConnect,
}

impl PresenceMethod {
    /// ARP/NDP are the box's own reports; probes are independent reachability.
    fn testimony_type(self) -> &'static str {
        match self {
            PresenceMethod::PfsenseArp | PresenceMethod::PfsenseNdp => {
                "pfsense_runtime_report"
            }
            PresenceMethod::IcmpEcho | PresenceMethod::TcpConnect => "observed_reachability",
        }
    }
}

/// Outcome of one presence attempt. `NotAttempted` keeps an entry's basis
/// honest when a method was declared but not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceOutcome {
    /// The host/address was observed (ARP entry present, or probe answered).
    Observed,
    /// No observation within the method's basis (ARP absent, probe silent).
    /// NOT "host down" — silence is not absence.
    NotObserved,
    NotAttempted,
}

/// One presence observation, with the vantage and time it was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PresenceObservation {
    pub method: PresenceMethod,
    /// Where the observation was made (e.g. `pfsense_arp_table`,
    /// `nq-vantage-lan`). For probes this MUST be independent of the subject.
    pub vantage: String,
    pub outcome: PresenceOutcome,
    pub observed_at: String,
    /// The testimony type of this observation, recorded so a report cannot
    /// silently graduate to reachability truth.
    pub testimony_type: &'static str,
}

impl PresenceObservation {
    pub fn new(
        method: PresenceMethod,
        vantage: impl Into<String>,
        outcome: PresenceOutcome,
        observed_at: OffsetDateTime,
    ) -> Self {
        PresenceObservation {
            method,
            vantage: vantage.into(),
            outcome,
            observed_at: rfc3339(observed_at),
            testimony_type: method.testimony_type(),
        }
    }
}

/// Perturbation accounting — even reads are transitions. ARP reads are
/// passive; an ICMP/TCP presence probe leaves a trace (and, behind a
/// firewall running IDS/IPS, may be classified as a scan). The live slice
/// fills `observed_secondary_effects`; the core declares the expectation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Perturbation {
    pub class: &'static str,
    pub expected_side_effects: Vec<&'static str>,
}

/// Candidate verdict ladder — reality-derived, NOT a final taxonomy. Every
/// state is deliberately a *non-lift*: the strongest positive is "observed
/// present from this vantage at this time," and the interesting negative is
/// "lease report not corroborated," never "host down."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LeasePresenceVerdict {
    /// Lease does not currently claim an occupant (expired/released/unknown).
    /// Nothing to corroborate; the lease is a historical report.
    LeaseExpiredOrAbsent,
    /// Lease claims an occupant, but NO presence observation was attempted —
    /// no basis to corroborate or refute.
    CannotTestifyNoPresenceBasis,
    /// Lease claims an occupant; presence was attempted and NOT observed.
    /// The specimen's point: a lease report uncorroborated by a current
    /// presence observation. NOT host-down, NOT host-gone, NOT lease-wrong.
    LeaseUncorroborated,
    /// Lease claims an occupant AND a presence observation saw it — present
    /// from that vantage at that time (nothing stronger is claimed).
    LeaseCorroboratedByPresence,
}

/// `nq.probe.lease_presence.v1`. Receipt-only; typed verdict, no coercion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LeasePresenceReceipt {
    pub schema: &'static str,
    pub probe_kind: &'static str,
    pub lease: LeaseReport,
    pub observations: Vec<PresenceObservation>,
    pub probe_time: String,
    pub clock_basis: ClockBasis,
    pub perturbation: Perturbation,
    pub verdict: LeasePresenceVerdict,
    pub non_claims: Vec<String>,
}

fn rfc3339(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_default()
}

/// The fixed scope ceiling: what lease-vs-presence does NOT witness. This is
/// the load-bearing part of the specimen — the refusals are the product.
fn scope_ceiling_non_claims() -> Vec<String> {
    vec![
        "an active DHCP lease does not establish current host presence (leases outlive hosts)".to_string(),
        "cannot testify the host is gone".to_string(),
        "cannot testify the host is down".to_string(),
        "cannot testify the lease is wrong".to_string(),
        "ARP/NDP residue is a pfSense self-report; absence in it is not host-absence".to_string(),
        "a silent probe is silence from one vantage at one time, not a down host".to_string(),
        "host_absent / host_down would require a declared probe regime (vantage + schedule + scope) absent from this specimen".to_string(),
        "can testify only: whether the lease report is corroborated by a current presence observation from the named vantage(s)".to_string(),
    ]
}

/// Pure, clock-injected verdict. `now` is recorded as `probe_time`; it does
/// not change the verdict (presence is point-in-time from the observations),
/// but it pins when the corroboration was assessed. No network, no SSH — the
/// live reader supplies `lease` and `observations`.
pub fn evaluate_lease_presence(
    lease: &LeaseReport,
    observations: &[PresenceObservation],
    clock: &ClockBasis,
    now: OffsetDateTime,
) -> LeasePresenceReceipt {
    let verdict = compute_verdict(lease, observations);

    // Perturbation expectation depends on whether any active probe was used;
    // ARP/NDP-only reads are passive.
    let used_active_probe = observations.iter().any(|o| {
        matches!(
            o.method,
            PresenceMethod::IcmpEcho | PresenceMethod::TcpConnect
        ) && o.outcome != PresenceOutcome::NotAttempted
    });
    let perturbation = if used_active_probe {
        Perturbation {
            class: "active_presence_probe",
            expected_side_effects: vec![
                "icmp_or_tcp_packet_to_subject",
                "possible_firewall_or_ids_log",
            ],
        }
    } else {
        Perturbation {
            class: "passive_report_read",
            expected_side_effects: vec!["read_only_lease_and_arp_report"],
        }
    };

    LeasePresenceReceipt {
        schema: LEASE_PRESENCE_PROBE_SCHEMA,
        probe_kind: "lease_presence",
        lease: lease.clone(),
        observations: observations.to_vec(),
        probe_time: rfc3339(now),
        clock_basis: clock.clone(),
        perturbation,
        verdict,
        non_claims: scope_ceiling_non_claims(),
    }
}

/// Verdict logic. The lease must claim an occupant for presence to matter;
/// then the strongest *attempted* observation decides corroboration. A
/// positive on any observation corroborates; an all-attempted-none-observed
/// set is the uncorroborated specimen; no attempt at all cannot testify.
fn compute_verdict(
    lease: &LeaseReport,
    observations: &[PresenceObservation],
) -> LeasePresenceVerdict {
    if !lease.state.claims_occupant() {
        return LeasePresenceVerdict::LeaseExpiredOrAbsent;
    }

    let any_observed = observations
        .iter()
        .any(|o| o.outcome == PresenceOutcome::Observed);
    if any_observed {
        return LeasePresenceVerdict::LeaseCorroboratedByPresence;
    }

    let any_attempted = observations
        .iter()
        .any(|o| o.outcome != PresenceOutcome::NotAttempted);
    if any_attempted {
        LeasePresenceVerdict::LeaseUncorroborated
    } else {
        LeasePresenceVerdict::CannotTestifyNoPresenceBasis
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

    // NOTE: fixtures are synthetic-but-realistic. No pfSense was read to
    // build them — the live SSH read slice (gated on access) grounds these
    // in real lease/ARP data, the way the TLS core was grounded in the real
    // step-0 cert.
    fn active_lease() -> LeaseReport {
        LeaseReport {
            hostname: Some("nas-01".to_string()),
            ip: "10.0.0.50".to_string(),
            mac: Some("02:00:00:00:00:50".to_string()),
            state: LeaseState::Active,
            source: "synthetic:fixture".to_string(),
        }
    }

    #[test]
    fn corroborated_when_arp_observes_the_lease() {
        let obs = vec![PresenceObservation::new(
            PresenceMethod::PfsenseArp,
            "pfsense_arp_table",
            PresenceOutcome::Observed,
            at("2026-06-23T18:00:00Z"),
        )];
        let r = evaluate_lease_presence(&active_lease(), &obs, &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r.verdict, LeasePresenceVerdict::LeaseCorroboratedByPresence);
    }

    #[test]
    fn corroborated_when_probe_answers_from_a_vantage() {
        let obs = vec![PresenceObservation::new(
            PresenceMethod::IcmpEcho,
            "nq-vantage-lan",
            PresenceOutcome::Observed,
            at("2026-06-23T18:00:00Z"),
        )];
        let r = evaluate_lease_presence(&active_lease(), &obs, &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r.verdict, LeasePresenceVerdict::LeaseCorroboratedByPresence);
    }

    /// The specimen's reason to exist: active lease, presence attempted and
    /// not observed -> uncorroborated, NOT host-down.
    #[test]
    fn uncorroborated_when_presence_attempted_but_not_observed() {
        let obs = vec![
            PresenceObservation::new(
                PresenceMethod::PfsenseArp,
                "pfsense_arp_table",
                PresenceOutcome::NotObserved,
                at("2026-06-23T18:00:00Z"),
            ),
            PresenceObservation::new(
                PresenceMethod::IcmpEcho,
                "nq-vantage-lan",
                PresenceOutcome::NotObserved,
                at("2026-06-23T18:00:00Z"),
            ),
        ];
        let r = evaluate_lease_presence(&active_lease(), &obs, &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r.verdict, LeasePresenceVerdict::LeaseUncorroborated);
    }

    #[test]
    fn cannot_testify_when_no_presence_attempted() {
        let r = evaluate_lease_presence(&active_lease(), &[], &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r.verdict, LeasePresenceVerdict::CannotTestifyNoPresenceBasis);
        // A declared-but-not-run method is still no basis.
        let obs = vec![PresenceObservation::new(
            PresenceMethod::TcpConnect,
            "nq-vantage-lan",
            PresenceOutcome::NotAttempted,
            at("2026-06-23T18:00:00Z"),
        )];
        let r2 = evaluate_lease_presence(&active_lease(), &obs, &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r2.verdict, LeasePresenceVerdict::CannotTestifyNoPresenceBasis);
    }

    #[test]
    fn expired_lease_has_nothing_to_corroborate() {
        let mut lease = active_lease();
        lease.state = LeaseState::Expired;
        let obs = vec![PresenceObservation::new(
            PresenceMethod::PfsenseArp,
            "pfsense_arp_table",
            PresenceOutcome::NotObserved,
            at("2026-06-23T18:00:00Z"),
        )];
        let r = evaluate_lease_presence(&lease, &obs, &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r.verdict, LeasePresenceVerdict::LeaseExpiredOrAbsent);
    }

    /// A static mapping claims an occupant by configuration; absence of
    /// presence is still only uncorroborated.
    #[test]
    fn static_mapping_claims_an_occupant() {
        let mut lease = active_lease();
        lease.state = LeaseState::Static;
        let obs = vec![PresenceObservation::new(
            PresenceMethod::PfsenseArp,
            "pfsense_arp_table",
            PresenceOutcome::NotObserved,
            at("2026-06-23T18:00:00Z"),
        )];
        let r = evaluate_lease_presence(&lease, &obs, &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r.verdict, LeasePresenceVerdict::LeaseUncorroborated);
    }

    /// Observed wins over a not-observed sibling — one positive corroborates.
    #[test]
    fn mixed_observations_corroborate_on_any_positive() {
        let obs = vec![
            PresenceObservation::new(
                PresenceMethod::PfsenseArp,
                "pfsense_arp_table",
                PresenceOutcome::NotObserved,
                at("2026-06-23T18:00:00Z"),
            ),
            PresenceObservation::new(
                PresenceMethod::TcpConnect,
                "nq-vantage-lan",
                PresenceOutcome::Observed,
                at("2026-06-23T18:00:00Z"),
            ),
        ];
        let r = evaluate_lease_presence(&active_lease(), &obs, &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r.verdict, LeasePresenceVerdict::LeaseCorroboratedByPresence);
    }

    #[test]
    fn probe_time_reflects_the_injected_clock() {
        let r = evaluate_lease_presence(&active_lease(), &[], &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r.probe_time, "2026-06-23T18:00:01Z");
    }

    #[test]
    fn arp_only_read_is_passive_probe_is_active() {
        let arp = vec![PresenceObservation::new(
            PresenceMethod::PfsenseArp,
            "pfsense_arp_table",
            PresenceOutcome::Observed,
            at("2026-06-23T18:00:00Z"),
        )];
        let r = evaluate_lease_presence(&active_lease(), &arp, &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r.perturbation.class, "passive_report_read");

        let probe = vec![PresenceObservation::new(
            PresenceMethod::IcmpEcho,
            "nq-vantage-lan",
            PresenceOutcome::Observed,
            at("2026-06-23T18:00:00Z"),
        )];
        let r2 = evaluate_lease_presence(&active_lease(), &probe, &clock(), at("2026-06-23T18:00:01Z"));
        assert_eq!(r2.perturbation.class, "active_presence_probe");
    }

    /// The refusals are the product: the receipt must carry the non-lift
    /// non-claims, and must NOT coerce to a green/ok status.
    #[test]
    fn receipt_carries_refusals_and_no_coercion() {
        let r = evaluate_lease_presence(&active_lease(), &[], &clock(), at("2026-06-23T18:00:01Z"));
        assert!(r
            .non_claims
            .iter()
            .any(|c| c.contains("does not establish current host presence")));
        assert!(r.non_claims.iter().any(|c| c.contains("cannot testify the host is down")));

        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["verdict"], "cannot_testify_no_presence_basis"); // typed enum, not a bool
        let s = serde_json::to_string(&r).unwrap();
        assert!(!s.contains("\"is_ok\"") && !s.contains("\"healthy\"") && !s.contains("\"green\""));
    }

    /// Source typing travels on each observation so a pfSense report cannot
    /// silently graduate to observed reachability.
    #[test]
    fn observations_carry_testimony_type() {
        let obs = vec![
            PresenceObservation::new(
                PresenceMethod::PfsenseArp,
                "pfsense_arp_table",
                PresenceOutcome::Observed,
                at("2026-06-23T18:00:00Z"),
            ),
            PresenceObservation::new(
                PresenceMethod::IcmpEcho,
                "nq-vantage-lan",
                PresenceOutcome::Observed,
                at("2026-06-23T18:00:00Z"),
            ),
        ];
        assert_eq!(obs[0].testimony_type, "pfsense_runtime_report");
        assert_eq!(obs[1].testimony_type, "observed_reachability");
    }
}
