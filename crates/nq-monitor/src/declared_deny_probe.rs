//! Declared-deny reachable-drift probe — verdict core
//! (`nq.probe.declared_deny.v1`).
//!
//! The gold reachable-drift specimen (`PFSENSE_REACHABLE_DRIFT_STEP0_INVENTORY.md`
//! check #1). This is **not** a firewall-correctness test. It is a
//! declaration-vs-observation **custody** test:
//!
//!   A declared policy refusal must remain a refusal under observation, and a
//!   successful path probe must not silently erase that declaration. The
//!   declared denial (a pfSense `block` rule loaded into the ruleset) and an
//!   observed reachability cannot both be quietly promoted into one coherent
//!   policy claim.
//!
//! Source typing (per the Step-0 inventory):
//!   - the loaded `block` rule is a `pfSenseDeclaration` — what the box is
//!     configured to refuse, never proof a specific path was refused
//!   - the rule's evaluation/packet/state counters are a `pfSenseRuntimeReport`
//!     (the box's own self-report), not an independent path observation
//!   - an NQ probe from a named vantage is `ObservedReachability`
//!   - the verdict is the custody reconciliation of the three.
//!
//! Asymmetry discipline (load-bearing, from the parent candidate): only a
//! SUBJECT probe that GETS THROUGH — a real handshake to the declared-denied
//! target — is an unambiguous contradiction (`declared_deny_observed_reachable`).
//! A blocked/refused/timeout subject is admissible as
//! `declared_deny_observed_blocked` ONLY with a passing CONTROL probe (a
//! known-allowed target proving the vantage has ordinary egress); without it,
//! "blocked" cannot be distinguished from "the vantage has no egress" or "the
//! target is down" → `cannot_testify_*`. A refused result is never silently
//! promoted to "denied as declared."
//!
//! Lane separation (non-negotiable): active-witness lane, receipt only. No
//! write to the passive collector's evidence tables, no coercion to an
//! operational/green status — the output is a typed verdict, never `is_ok()`.
//! Read-only: this core never mutates the firewall; the live reader reads
//! `pfctl -sr`/`-vv` and runs bounded probes, nothing else.

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const DECLARED_DENY_PROBE_SCHEMA: &str = "nq.probe.declared_deny.v1";

/// The clock the receipt was pinned to. An unwitnessed clock makes "observed
/// at T" theatre with a timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClockBasis {
    pub source: String,
    pub ntp_status: String,
}

/// Custody of the declared-policy surface — whether we could read a declared
/// denial at all, kept distinct from any observation of the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyCustody {
    /// A declared `block` rule matching the requested table/identifier was
    /// read and parsed from the loaded ruleset.
    Present,
    /// The policy surface was readable, but no matching declared denial was
    /// found — the default, not a gap to paper over (and NOT "allowed").
    Absent,
    /// The policy surface could not be read/parsed (custody in doubt).
    UnknownSurface,
}

/// A declared `block` rule as read from the loaded pfSense ruleset — a
/// `pfSenseDeclaration`. The counters (`evaluations`/`blocked_packets`/
/// `states`) ride along as the box's own `pfSenseRuntimeReport`, recorded for
/// context, NEVER promoted to an independent observation of the path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeclaredDenyRule {
    /// The pfSense rule label(s), e.g. `USER_RULE: pfB_PRI1_v4`.
    pub label: String,
    /// The rule identifier, e.g. `1770008176`.
    pub ridentifier: String,
    /// The interface the rule is bound to (the source segment), e.g. `igc1`.
    pub interface: String,
    /// Direction (`in`/`out`).
    pub direction: String,
    /// The block action verbatim, e.g. `block return` / `block drop`.
    pub action: String,
    /// Whether the rule is `quick` (decides immediately, before later passes).
    pub quick: bool,
    /// The source spec verbatim, e.g. `any`.
    pub source_spec: String,
    /// The destination spec verbatim, e.g. `<pfB_PRI1_v4>`.
    pub dest_spec: String,
    /// The destination table name, if the dest is a table (e.g. `pfB_PRI1_v4`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_table: Option<String>,
    /// How many entries the destination table holds, if read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_entry_count: Option<u64>,
    /// Box self-report: how many times the rule was evaluated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluations: Option<u64>,
    /// Box self-report: packets the rule blocked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_packets: Option<u64>,
    /// Box self-report: states the rule created (0 == nothing established).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub states: Option<u64>,
    pub custody: PolicyCustody,
    /// Where the declaration came from (e.g. `ssh:<host> pfctl -sr -vv`).
    pub source: String,
}

/// Whether a probe is the control (a known-allowed target, proving the vantage
/// has egress) or the subject (the declared-denied target itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DenyRole {
    /// Proves the vantage has ordinary egress — without it, "subject blocked"
    /// is uninterpretable.
    Control,
    /// The declared-denied path under test. `Reached` here means a REAL
    /// handshake completed (NOT a firewall `block return` RST, which is
    /// `NotReached`); only a real got-through is the unambiguous contradiction.
    Subject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeMethod {
    /// Declared-deny probes are TCP — a completed handshake vs. a RST is the
    /// got-through/blocked distinction the asymmetry rests on.
    TcpConnect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeOutcome {
    /// Control: the host answered (egress works). Subject: a REAL handshake
    /// completed (got through — not a firewall RST).
    Reached,
    /// No reach within the method's basis. NOT a claim about why.
    NotReached,
    /// The probe was not run (e.g. subject target deliberately unbound).
    NotAttempted,
}

/// One probe observation from a named, independent vantage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathObservation {
    pub method: ProbeMethod,
    pub role: DenyRole,
    pub vantage: String,
    /// The destination probed, or `(unbound)` when the role's target was not
    /// bound for this slice.
    pub target: String,
    pub outcome: ProbeOutcome,
    pub observed_at: String,
    pub testimony_type: &'static str,
}

impl PathObservation {
    pub fn new(
        method: ProbeMethod,
        role: DenyRole,
        vantage: impl Into<String>,
        target: impl Into<String>,
        outcome: ProbeOutcome,
        observed_at: OffsetDateTime,
    ) -> Self {
        PathObservation {
            method,
            role,
            vantage: vantage.into(),
            target: target.into(),
            outcome,
            observed_at: rfc3339(observed_at),
            testimony_type: "observed_reachability",
        }
    }

    /// An explicit "subject target not bound" observation — the honest record
    /// that we deliberately did not probe the declared-denied path.
    pub fn subject_unbound(vantage: impl Into<String>, observed_at: OffsetDateTime) -> Self {
        PathObservation::new(
            ProbeMethod::TcpConnect,
            DenyRole::Subject,
            vantage,
            "(unbound)",
            ProbeOutcome::NotAttempted,
            observed_at,
        )
    }
}

/// Perturbation accounting. Reading `pfctl` is passive; a bounded probe leaves
/// a trace. The subject probe additionally traverses the firewall toward a
/// declared-denied destination — the real hazard, named here so it is never
/// run silently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Perturbation {
    pub class: &'static str,
    pub expected_side_effects: Vec<&'static str>,
}

/// Candidate verdict ladder — custody first; the refusals are the product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredDenyVerdict {
    /// The policy surface could not be read/parsed. NOT "no policy".
    UnknownCustodyPolicySurface,
    /// No declared denial found for the requested target. NOT "allowed".
    CannotTestifyDeclaredPolicyAbsent,
    /// A declared denial is present and custodied, but the subject path was
    /// not probed (target unbound) — no observation to reconcile. The verdict
    /// makes NO claim that the path is blocked or reachable.
    CannotTestifyProbeTargetUnbound,
    /// The subject was probed and blocked, but the CONTROL did not pass — the
    /// vantage's egress is unproven, so "blocked" is uninterpretable.
    CannotTestifyVantageUnbound,
    /// The subject was probed and blocked, but no control was run — egress
    /// unconfirmed, so the block cannot be attributed.
    DeclaredDenyProbeInconclusive,
    /// Declared deny AND the subject path GOT THROUGH (a real handshake). The
    /// unambiguous contradiction: declaration and observation cannot both be
    /// silently promoted into a coherent policy claim. NOT "firewall broken."
    DeclaredDenyObservedReachable,
    /// Declared deny AND the subject was blocked AND the control proved egress
    /// — the declaration is corroborated by observation at this vantage, at
    /// this time, for this target. NOT "firewall correct."
    DeclaredDenyObservedBlocked,
}

/// `nq.probe.declared_deny.v1`. Receipt-only; typed verdict, no coercion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeclaredDenyReceipt {
    pub schema: &'static str,
    pub probe_kind: &'static str,
    pub rule: DeclaredDenyRule,
    pub observations: Vec<PathObservation>,
    pub probe_time: String,
    pub clock_basis: ClockBasis,
    pub perturbation: Perturbation,
    pub verdict: DeclaredDenyVerdict,
    pub non_claims: Vec<String>,
}

fn rfc3339(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_default()
}

/// The fixed scope ceiling — the load-bearing refusals.
fn scope_ceiling_non_claims() -> Vec<String> {
    vec![
        "a declared block rule is a pfSense declaration of intent loaded into the ruleset — what the box is configured to refuse, not proof any specific path was refused under observation".to_string(),
        "rule counters (evaluations / blocked packets / states) are the box's own self-report (pfSenseRuntimeReport), not an independent observation of the path".to_string(),
        "cannot testify the firewall is correct or broken — this is declaration-vs-observation custody, not a firewall-correctness test".to_string(),
        "a successful control probe proves only that this vantage has ordinary egress, not that any declared-denied path behaves as declared".to_string(),
        "only a subject probe that gets through (a real handshake to the declared-denied target) is an unambiguous contradiction; a block-return RST is not a got-through".to_string(),
        "a blocked / refused / timeout subject cannot by itself distinguish a firewall block from a down target or a dead vantage — it is admissible as declared_deny_observed_blocked only with a passing control".to_string(),
        "absence of a declared denial for this vantage is cannot_testify_declared_policy_absent, not 'allowed'".to_string(),
        "when the subject target is unbound, no claim is made that the declared-denied path is blocked OR reachable".to_string(),
        "can testify only: that a declared denial is present and custodied, and (with a bound subject + a passing control) whether it holds under observation from the named vantage at this time".to_string(),
    ]
}

/// Pure, clock-injected verdict. `now` is recorded as `probe_time`. No network,
/// no SSH — the live reader supplies the rule and observations.
pub fn evaluate_declared_deny(
    rule: &DeclaredDenyRule,
    observations: &[PathObservation],
    clock: &ClockBasis,
    now: OffsetDateTime,
) -> DeclaredDenyReceipt {
    let verdict = compute_verdict(rule, observations);

    let subject_attempted = observations.iter().any(|o| {
        o.role == DenyRole::Subject && o.outcome != ProbeOutcome::NotAttempted
    });
    let perturbation = if subject_attempted {
        Perturbation {
            class: "active_subject_probe_traverses_firewall",
            expected_side_effects: vec![
                "packet_toward_declared_denied_destination",
                "rule_counter_increment",
                "possible_block_return_rst",
            ],
        }
    } else if observations
        .iter()
        .any(|o| o.role == DenyRole::Control && o.outcome != ProbeOutcome::NotAttempted)
    {
        Perturbation {
            class: "control_probe_only",
            expected_side_effects: vec!["packet_to_known_allowed_target", "no_subject_path_touched"],
        }
    } else {
        Perturbation {
            class: "passive_policy_read",
            expected_side_effects: vec!["read_only_pfctl_rule_and_table"],
        }
    };

    DeclaredDenyReceipt {
        schema: DECLARED_DENY_PROBE_SCHEMA,
        probe_kind: "declared_deny",
        rule: rule.clone(),
        observations: observations.to_vec(),
        probe_time: rfc3339(now),
        clock_basis: clock.clone(),
        perturbation,
        verdict,
        non_claims: scope_ceiling_non_claims(),
    }
}

/// Verdict logic. Policy custody first. Then the subject path: unbound →
/// cannot-testify; got-through → the contradiction; blocked → corroborated
/// ONLY when a control proves egress (else cannot-testify / inconclusive).
fn compute_verdict(
    rule: &DeclaredDenyRule,
    observations: &[PathObservation],
) -> DeclaredDenyVerdict {
    match rule.custody {
        PolicyCustody::UnknownSurface => return DeclaredDenyVerdict::UnknownCustodyPolicySurface,
        PolicyCustody::Absent => return DeclaredDenyVerdict::CannotTestifyDeclaredPolicyAbsent,
        PolicyCustody::Present => {}
    }

    let subject = observations.iter().find(|o| o.role == DenyRole::Subject);
    let subject_outcome = subject.map(|o| o.outcome).unwrap_or(ProbeOutcome::NotAttempted);

    match subject_outcome {
        ProbeOutcome::NotAttempted => DeclaredDenyVerdict::CannotTestifyProbeTargetUnbound,
        ProbeOutcome::Reached => DeclaredDenyVerdict::DeclaredDenyObservedReachable,
        ProbeOutcome::NotReached => {
            let control = observations
                .iter()
                .find(|o| o.role == DenyRole::Control)
                .map(|o| o.outcome);
            match control {
                Some(ProbeOutcome::Reached) => DeclaredDenyVerdict::DeclaredDenyObservedBlocked,
                Some(ProbeOutcome::NotReached) => DeclaredDenyVerdict::CannotTestifyVantageUnbound,
                _ => DeclaredDenyVerdict::DeclaredDenyProbeInconclusive,
            }
        }
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
            source: "system_wall".to_string(),
            ntp_status: "unknown".to_string(),
        }
    }

    // Modeled on the real captured rule (2026-06-24), anonymized:
    //   block return in log quick on igc1 inet from any to <pfB_PRI1_v4:17007>
    //   label "USER_RULE: pfB_PRI1_v4" ... ridentifier 1770008176
    //   [ Evaluations: 3186099  Packets: 8  ...  States: 0 ]
    fn present_rule() -> DeclaredDenyRule {
        DeclaredDenyRule {
            label: "USER_RULE: pfB_PRI1_v4".to_string(),
            ridentifier: "1770008176".to_string(),
            interface: "igc1".to_string(),
            direction: "in".to_string(),
            action: "block return".to_string(),
            quick: true,
            source_spec: "any".to_string(),
            dest_spec: "<pfB_PRI1_v4>".to_string(),
            dest_table: Some("pfB_PRI1_v4".to_string()),
            table_entry_count: Some(17007),
            evaluations: Some(3186099),
            blocked_packets: Some(8),
            states: Some(0),
            custody: PolicyCustody::Present,
            source: "synthetic:fixture".to_string(),
        }
    }

    fn control(outcome: ProbeOutcome) -> PathObservation {
        PathObservation::new(
            ProbeMethod::TcpConnect,
            DenyRole::Control,
            "sushi-k-lan",
            "1.1.1.1:443",
            outcome,
            at("2026-06-24T17:00:00Z"),
        )
    }

    fn subject(outcome: ProbeOutcome) -> PathObservation {
        PathObservation::new(
            ProbeMethod::TcpConnect,
            DenyRole::Subject,
            "sushi-k-lan",
            "203.0.113.7:443",
            outcome,
            at("2026-06-24T17:00:00Z"),
        )
    }

    fn eval(rule: &DeclaredDenyRule, obs: &[PathObservation]) -> DeclaredDenyReceipt {
        evaluate_declared_deny(rule, obs, &clock(), at("2026-06-24T17:00:01Z"))
    }

    /// This slice's outcome: declared policy present, control egress observed,
    /// subject target deliberately unbound -> cannot testify subject path.
    #[test]
    fn subject_unbound_cannot_testify_even_with_control_passing() {
        let obs = vec![
            control(ProbeOutcome::Reached),
            PathObservation::subject_unbound("sushi-k-lan", at("2026-06-24T17:00:00Z")),
        ];
        let r = eval(&present_rule(), &obs);
        assert_eq!(r.verdict, DeclaredDenyVerdict::CannotTestifyProbeTargetUnbound);
        // The control is recorded but does NOT promote to a blocked/reachable claim.
        assert_eq!(r.perturbation.class, "control_probe_only");
    }

    #[test]
    fn no_observations_is_target_unbound() {
        let r = eval(&present_rule(), &[]);
        assert_eq!(r.verdict, DeclaredDenyVerdict::CannotTestifyProbeTargetUnbound);
        assert_eq!(r.perturbation.class, "passive_policy_read");
    }

    /// The spicy contradiction: declared deny, subject got through.
    #[test]
    fn subject_reached_is_the_contradiction() {
        let obs = vec![control(ProbeOutcome::Reached), subject(ProbeOutcome::Reached)];
        let r = eval(&present_rule(), &obs);
        assert_eq!(r.verdict, DeclaredDenyVerdict::DeclaredDenyObservedReachable);
        assert_eq!(
            r.perturbation.class,
            "active_subject_probe_traverses_firewall"
        );
    }

    /// Declared deny, subject blocked, control proves egress -> corroborated.
    #[test]
    fn subject_blocked_with_control_is_corroborated() {
        let obs = vec![control(ProbeOutcome::Reached), subject(ProbeOutcome::NotReached)];
        let r = eval(&present_rule(), &obs);
        assert_eq!(r.verdict, DeclaredDenyVerdict::DeclaredDenyObservedBlocked);
    }

    /// Subject blocked but control ALSO fails -> the vantage egress is unproven,
    /// so "blocked" is uninterpretable. The asymmetry's whole point.
    #[test]
    fn subject_blocked_without_egress_cannot_testify() {
        let obs = vec![control(ProbeOutcome::NotReached), subject(ProbeOutcome::NotReached)];
        let r = eval(&present_rule(), &obs);
        assert_eq!(r.verdict, DeclaredDenyVerdict::CannotTestifyVantageUnbound);
    }

    /// Subject blocked, no control at all -> inconclusive (egress unconfirmed).
    #[test]
    fn subject_blocked_without_control_is_inconclusive() {
        let r = eval(&present_rule(), &[subject(ProbeOutcome::NotReached)]);
        assert_eq!(r.verdict, DeclaredDenyVerdict::DeclaredDenyProbeInconclusive);
    }

    #[test]
    fn absent_policy_is_not_allowed() {
        let mut rule = present_rule();
        rule.custody = PolicyCustody::Absent;
        // Even a subject that got through cannot speak to a denial that isn't declared.
        let r = eval(&rule, &[subject(ProbeOutcome::Reached)]);
        assert_eq!(r.verdict, DeclaredDenyVerdict::CannotTestifyDeclaredPolicyAbsent);
    }

    #[test]
    fn unreadable_surface_is_unknown_custody() {
        let mut rule = present_rule();
        rule.custody = PolicyCustody::UnknownSurface;
        let r = eval(&rule, &[]);
        assert_eq!(r.verdict, DeclaredDenyVerdict::UnknownCustodyPolicySurface);
    }

    #[test]
    fn probe_time_reflects_the_injected_clock() {
        let r = eval(&present_rule(), &[]);
        assert_eq!(r.probe_time, "2026-06-24T17:00:01Z");
    }

    /// The refusals are the product, and nothing coerces to ok/green/correct.
    #[test]
    fn receipt_carries_refusals_and_no_coercion() {
        let r = eval(&present_rule(), &[]);
        assert!(r
            .non_claims
            .iter()
            .any(|c| c.contains("not a firewall-correctness test")));
        assert!(r
            .non_claims
            .iter()
            .any(|c| c.contains("not 'allowed'")));
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["verdict"], "cannot_testify_probe_target_unbound");
        let s = serde_json::to_string(&r).unwrap();
        assert!(
            !s.contains("\"is_ok\"")
                && !s.contains("\"healthy\"")
                && !s.contains("\"green\"")
                && !s.contains("firewall_correct")
                && !s.contains("firewall_broken")
        );
    }

    /// Counters ride as a self-report; they do not by themselves decide the verdict.
    #[test]
    fn counters_are_context_not_verdict() {
        let mut rule = present_rule();
        rule.blocked_packets = Some(999999);
        rule.states = Some(0);
        // High block count + zero states does NOT become declared_deny_observed_blocked
        // without an actual subject observation.
        let r = eval(&rule, &[control(ProbeOutcome::Reached)]);
        assert_eq!(r.verdict, DeclaredDenyVerdict::CannotTestifyProbeTargetUnbound);
    }
}
