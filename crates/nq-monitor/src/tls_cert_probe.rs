//! TLS-certificate active-witness probe — verdict core (`nq.probe.tls_cert.v1`).
//!
//! First active-witness specimen (`ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md`):
//! cert expiry is a real *scheduled* negative — `notAfter` produces a
//! refutation on a contractually-pinned date, no lab sabotage required.
//!
//! This module is the **pure verdict core**, deliberately split from the
//! live TLS transport (the rustls handshake + chain extraction that fills
//! [`TlsCertFacts`] is the follow-up slice). The split keeps the verdict a
//! pure, **clock-injected** function of (observed facts, policy, probe
//! clock) — fully fixture-testable with no network, and honest about the
//! clock as a hidden witness: a cert is `expired_under_probe_clock`, never
//! `expired_absolutely`.
//!
//! Lane separation (non-negotiable, per the candidate): this is the
//! **active-witness lane**. It emits a receipt only. It does **not** write
//! into the passive collector's evidence tables (unlike the DNS probe),
//! and it does **not** coerce to any operational/green status — the output
//! is a typed verdict, never an `is_ok()`/`bool`.
//!
//! Concrete `TlsCertProbeReceipt`, not a generic `ProbeReceipt<T>`: the
//! generic is extracted only once a 2nd/3rd probe (pfSense/Plex) pressures
//! the shared columns (`WITNESS_SURFACE` / `INTEGRATION_SURFACE_GAP.md`).
//! Receipt-type custody is likewise local to the probe for now (custody
//! deferred there); promote to a shared home when a consumer needs it.

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const TLS_CERT_PROBE_SCHEMA: &str = "nq.probe.tls_cert.v1";

/// Trust anchor the validation was performed against. `webpki` anchors in
/// the public WebPKI (a trust anchor that fails independently of the
/// operator); `pinned_ca` anchors in the operator's own CA, so its verdict
/// is "valid under *our* PKI universe," not "valid, period."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationPolicy {
    Webpki,
    PinnedCa,
}

/// Outcome of chain validation as the transport performed it. `NotAttempted`
/// when the handshake never got far enough to validate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ChainValidation {
    Valid,
    Invalid { reason: String },
    NotAttempted,
}

/// How far the probe got reaching the surface — the delivery basis of the
/// admissible-negative tuple. A `*_failed` negative is only admissible
/// relative to this plus a declared [`ResponseHorizon`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeliveryBasis {
    pub dns_answers: Vec<String>,
    pub tcp_connected: bool,
    pub tls_handshake_completed: bool,
}

/// The response horizon: the declared timeout the prober actually waited.
/// Without it, a `tcp_failed` / `tls_handshake_failed` negative is
/// intervention-with-anecdotes, not witnessed absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResponseHorizon {
    pub timeout_ms: u64,
    /// `None` when the transport did not instrument elapsed time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

/// The clock the verdict was pinned to. The negative ("expired") rides on
/// this; an unwitnessed clock makes the negative theatre with timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClockBasis {
    /// e.g. `system_ntp`, `unknown`.
    pub source: String,
    /// `recorded` when NTP sync state was observed; `unknown` otherwise.
    pub ntp_status: String,
}

/// One certificate as presented in the chain (leaf → … → root-as-sent).
/// Produced by the transport; hand-built in fixture tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PresentedCert {
    pub subject: String,
    pub issuer: String,
    pub not_before: OffsetDateTime,
    pub not_after: OffsetDateTime,
    /// Subject Alternative Names (DNS) presented by this cert.
    pub sans: Vec<String>,
    pub sha256_fingerprint: String,
}

/// Observed facts a transport extracts from one TLS handshake. The live
/// rustls transport (follow-up slice) fills this; [`evaluate_tls_cert`]
/// turns it into a receipt. The leaf is `presented_chain[0]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsCertFacts {
    pub delivery: DeliveryBasis,
    pub response_horizon: ResponseHorizon,
    pub presented_chain: Vec<PresentedCert>,
    pub validation: ChainValidation,
}

/// What the probe is asked to witness against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsCertPolicy {
    /// Names the leaf must cover for the cert to be *for* this surface.
    pub expected_names: Vec<String>,
    /// Days-remaining at or below which the verdict downgrades to the
    /// warning horizon.
    pub warning_threshold_days: i64,
    pub validation_policy: ValidationPolicy,
}

/// Where the probe stood and what it aimed at. The vantage must be
/// independent of the target's box — a co-located prober rebuilds the
/// correlated failure. (Enforcement is operational, not type-level.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsCertTarget {
    /// `host:port`.
    pub target: String,
    pub sni: String,
    /// Prober identity — NOT the target's box.
    pub vantage: String,
}

/// Candidate verdict states — **reality-derived, not a final ladder.**
/// `renewed_since_prior_probe` needs prior-probe state and is deferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsCertVerdict {
    ProbeNotAttempted,
    DnsFailed,
    TcpFailed,
    TlsHandshakeFailed,
    NoCertificatePresented,
    NameMismatch,
    ChainInvalid,
    /// Expired relative to the probe clock — never `expired_absolutely`.
    ExpiredUnderProbeClock,
    ValidButWithinWarningHorizon,
    ValidAtProbeTime,
}

/// `nq.probe.tls_cert.v1`. Receipt-only; carries a typed [`TlsCertVerdict`],
/// never an operational/green coercion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TlsCertProbeReceipt {
    pub schema: &'static str,
    pub probe_kind: &'static str,
    pub target: String,
    pub sni: String,
    pub expected_names: Vec<String>,
    pub vantage: String,
    pub probe_time: String,
    pub clock_basis: ClockBasis,
    pub delivery_basis: DeliveryBasis,
    pub response_horizon: ResponseHorizon,
    pub chain_subjects: Vec<String>,
    pub chain_fingerprints: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaf_not_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaf_not_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_remaining: Option<i64>,
    pub warning_threshold_days: i64,
    pub validation_policy: ValidationPolicy,
    pub perturbation: Perturbation,
    pub verdict: TlsCertVerdict,
    pub non_claims: Vec<String>,
}

/// Perturbation accounting — a probe is a transition. A TLS cert probe is a
/// read-only handshake whose only expected side effect is an access-log line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Perturbation {
    pub class: &'static str,
    pub expected_side_effects: Vec<&'static str>,
}

impl Perturbation {
    fn read_only_handshake() -> Self {
        Perturbation {
            class: "read_only_tls_handshake",
            expected_side_effects: vec!["access_log"],
        }
    }
}

/// The fixed scope ceiling: what a TLS-cert probe does NOT witness.
fn scope_ceiling_non_claims(policy: ValidationPolicy) -> Vec<String> {
    let mut v = vec![
        "does not witness service-logic integrity".to_string(),
        "does not witness host integrity or root custody (a root attacker holding the key still presents a valid cert)".to_string(),
        "absence on a route is not its cause ('the box is down' is unproven)".to_string(),
        "validity is relative to name/chain/trust-anchor/probe-clock/vantage only".to_string(),
    ];
    if policy == ValidationPolicy::PinnedCa {
        v.push("under pinned_ca: valid only under the operator's own PKI universe, not 'valid, period'".to_string());
    }
    v
}

fn rfc3339(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_default()
}

/// Pure, clock-injected verdict. `now` drives both `days_remaining` and the
/// expiry check; it is recorded as `probe_time`. No ambient clock, no
/// network — the transport supplies `facts`, this decides the verdict.
pub fn evaluate_tls_cert(
    target: &TlsCertTarget,
    facts: &TlsCertFacts,
    policy: &TlsCertPolicy,
    clock: &ClockBasis,
    now: OffsetDateTime,
) -> TlsCertProbeReceipt {
    let leaf = facts.presented_chain.first();

    let chain_subjects: Vec<String> =
        facts.presented_chain.iter().map(|c| c.subject.clone()).collect();
    let chain_fingerprints: Vec<String> =
        facts.presented_chain.iter().map(|c| c.sha256_fingerprint.clone()).collect();

    // days_remaining is always reported relative to the injected clock when
    // a leaf exists — even on a negative verdict, so the receipt stays
    // self-describing.
    let days_remaining = leaf.map(|l| (l.not_after - now).whole_days());

    let verdict = compute_verdict(facts, policy, leaf, now);

    TlsCertProbeReceipt {
        schema: TLS_CERT_PROBE_SCHEMA,
        probe_kind: "tls_cert",
        target: target.target.clone(),
        sni: target.sni.clone(),
        expected_names: policy.expected_names.clone(),
        vantage: target.vantage.clone(),
        probe_time: rfc3339(now),
        clock_basis: clock.clone(),
        delivery_basis: facts.delivery.clone(),
        response_horizon: facts.response_horizon,
        chain_subjects,
        chain_fingerprints,
        leaf_not_before: leaf.map(|l| rfc3339(l.not_before)),
        leaf_not_after: leaf.map(|l| rfc3339(l.not_after)),
        issuer: leaf.map(|l| l.issuer.clone()),
        days_remaining,
        warning_threshold_days: policy.warning_threshold_days,
        validation_policy: policy.validation_policy,
        perturbation: Perturbation::read_only_handshake(),
        verdict,
        non_claims: scope_ceiling_non_claims(policy.validation_policy),
    }
}

/// Verdict precedence walks delivery → presentation → identity → validity.
/// Earliest failing rung wins; a clean cert falls through to the
/// clock-relative validity band.
fn compute_verdict(
    facts: &TlsCertFacts,
    policy: &TlsCertPolicy,
    leaf: Option<&PresentedCert>,
    now: OffsetDateTime,
) -> TlsCertVerdict {
    // Delivery rungs.
    if !facts.delivery.tcp_connected {
        return if facts.delivery.dns_answers.is_empty() {
            TlsCertVerdict::DnsFailed
        } else {
            TlsCertVerdict::TcpFailed
        };
    }
    if !facts.delivery.tls_handshake_completed {
        return TlsCertVerdict::TlsHandshakeFailed;
    }
    let Some(leaf) = leaf else {
        return TlsCertVerdict::NoCertificatePresented;
    };

    // Chain validity (the transport's webpki/pinned outcome).
    if let ChainValidation::Invalid { .. } = facts.validation {
        return TlsCertVerdict::ChainInvalid;
    }

    // Identity: the leaf must cover an expected name.
    if !name_matches(&leaf.sans, &policy.expected_names) {
        return TlsCertVerdict::NameMismatch;
    }

    // Validity relative to the PROBE CLOCK — never absolute.
    if now < leaf.not_before || now > leaf.not_after {
        return TlsCertVerdict::ExpiredUnderProbeClock;
    }

    let days_remaining = (leaf.not_after - now).whole_days();
    if days_remaining <= policy.warning_threshold_days {
        return TlsCertVerdict::ValidButWithinWarningHorizon;
    }

    TlsCertVerdict::ValidAtProbeTime
}

/// A leaf covers an expected name if any SAN matches it, honoring a single
/// leftmost wildcard label (`*.example.com` covers `a.example.com`, not
/// `example.com` or `a.b.example.com`).
fn name_matches(sans: &[String], expected_names: &[String]) -> bool {
    expected_names.iter().all(|want| {
        sans.iter().any(|san| san_covers(san, want))
    }) && !expected_names.is_empty()
}

fn san_covers(san: &str, want: &str) -> bool {
    let san = san.to_ascii_lowercase();
    let want = want.to_ascii_lowercase();
    if san == want {
        return true;
    }
    if let Some(suffix) = san.strip_prefix("*.") {
        // Wildcard matches exactly one leftmost label.
        if let Some((_label, rest)) = want.split_once('.') {
            return rest == suffix;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).expect("rfc3339 fixture")
    }

    fn ntp_clock() -> ClockBasis {
        ClockBasis {
            source: "system_ntp".to_string(),
            ntp_status: "recorded".to_string(),
        }
    }

    fn target() -> TlsCertTarget {
        TlsCertTarget {
            target: "nq.neutral.zone:443".to_string(),
            sni: "nq.neutral.zone".to_string(),
            // External vantage — NOT the target's box (operationally enforced).
            vantage: "operator-dev-box".to_string(),
        }
    }

    fn policy(threshold: i64) -> TlsCertPolicy {
        TlsCertPolicy {
            expected_names: vec!["nq.neutral.zone".to_string()],
            warning_threshold_days: threshold,
            validation_policy: ValidationPolicy::Webpki,
        }
    }

    /// The real leaf observed in step-0 (`ACTIVE_WITNESS_TLS_PROBE_STEP0.md`):
    /// Let's Encrypt YE2, valid 2026-05-31 → 2026-08-29.
    fn real_leaf() -> PresentedCert {
        PresentedCert {
            subject: "CN=nq.neutral.zone".to_string(),
            issuer: "C=US O=Let's Encrypt CN=YE2".to_string(),
            not_before: at("2026-05-31T19:08:53Z"),
            not_after: at("2026-08-29T19:08:52Z"),
            sans: vec!["nq.neutral.zone".to_string()],
            sha256_fingerprint:
                "A2:66:D0:66:94:3A:A9:A8:91:E1:D6:FD:88:FB:37:25:BB:EF:2C:3C:20:9A:07:9D:47:15:6F:0F:EF:A0:14:CA"
                    .to_string(),
        }
    }

    /// A delivered handshake presenting `chain`, validation Valid.
    fn delivered(chain: Vec<PresentedCert>) -> TlsCertFacts {
        TlsCertFacts {
            delivery: DeliveryBasis {
                dns_answers: vec!["192.46.223.21".to_string()],
                tcp_connected: true,
                tls_handshake_completed: true,
            },
            response_horizon: ResponseHorizon {
                timeout_ms: 10_000,
                elapsed_ms: None,
            },
            presented_chain: chain,
            validation: ChainValidation::Valid,
        }
    }

    fn verdict_at(now: &str, threshold: i64) -> TlsCertVerdict {
        evaluate_tls_cert(
            &target(),
            &delivered(vec![real_leaf()]),
            &policy(threshold),
            &ntp_clock(),
            at(now),
        )
        .verdict
    }

    // ---- validity band, driven by the injected probe clock ----

    #[test]
    fn valid_at_probe_time_with_real_step0_cert() {
        let r = evaluate_tls_cert(
            &target(),
            &delivered(vec![real_leaf()]),
            &policy(14),
            &ntp_clock(),
            at("2026-06-19T15:39:54Z"), // the real step-0 probe instant
        );
        assert_eq!(r.verdict, TlsCertVerdict::ValidAtProbeTime);
        assert_eq!(r.days_remaining, Some(71));
        assert_eq!(r.issuer.as_deref(), Some("C=US O=Let's Encrypt CN=YE2"));
        assert_eq!(r.schema, "nq.probe.tls_cert.v1");
    }

    #[test]
    fn within_warning_horizon_near_expiry() {
        // 4 days before notAfter, threshold 14.
        assert_eq!(
            verdict_at("2026-08-25T19:08:52Z", 14),
            TlsCertVerdict::ValidButWithinWarningHorizon
        );
    }

    #[test]
    fn expired_under_probe_clock_not_absolute() {
        // Past notAfter. The verdict is clock-relative, never "expired_absolutely".
        assert_eq!(
            verdict_at("2026-09-01T00:00:00Z", 14),
            TlsCertVerdict::ExpiredUnderProbeClock
        );
    }

    /// The same facts produce a different verdict under a different clock —
    /// proving the verdict is driven by the INJECTED clock, not an ambient one.
    #[test]
    fn injected_clock_drives_the_verdict() {
        let valid = verdict_at("2026-06-19T15:39:54Z", 14);
        let expired = verdict_at("2026-09-01T00:00:00Z", 14);
        assert_eq!(valid, TlsCertVerdict::ValidAtProbeTime);
        assert_eq!(expired, TlsCertVerdict::ExpiredUnderProbeClock);
        assert_ne!(valid, expired);

        // ...and identical (facts, now) is byte-identical (no ambient clock).
        let a = evaluate_tls_cert(&target(), &delivered(vec![real_leaf()]), &policy(14), &ntp_clock(), at("2026-06-19T15:39:54Z"));
        let b = evaluate_tls_cert(&target(), &delivered(vec![real_leaf()]), &policy(14), &ntp_clock(), at("2026-06-19T15:39:54Z"));
        assert_eq!(a, b);
    }

    // ---- identity ----

    #[test]
    fn name_mismatch_when_leaf_does_not_cover_expected() {
        let pol = TlsCertPolicy {
            expected_names: vec!["labelwatch.neutral.zone".to_string()],
            warning_threshold_days: 14,
            validation_policy: ValidationPolicy::Webpki,
        };
        let r = evaluate_tls_cert(
            &target(),
            &delivered(vec![real_leaf()]),
            &pol,
            &ntp_clock(),
            at("2026-06-19T15:39:54Z"),
        );
        assert_eq!(r.verdict, TlsCertVerdict::NameMismatch);
    }

    #[test]
    fn wildcard_san_covers_one_label() {
        assert!(san_covers("*.neutral.zone", "nq.neutral.zone"));
        assert!(!san_covers("*.neutral.zone", "neutral.zone"));
        assert!(!san_covers("*.neutral.zone", "a.b.neutral.zone"));
    }

    // ---- chain validity ----

    #[test]
    fn chain_invalid_from_transport_validation() {
        let mut facts = delivered(vec![real_leaf()]);
        facts.validation = ChainValidation::Invalid {
            reason: "untrusted issuer".to_string(),
        };
        let r = evaluate_tls_cert(&target(), &facts, &policy(14), &ntp_clock(), at("2026-06-19T15:39:54Z"));
        assert_eq!(r.verdict, TlsCertVerdict::ChainInvalid);
    }

    // ---- delivery negatives, admissible only under the recorded horizon ----

    #[test]
    fn dns_failed_when_no_answers_and_no_tcp() {
        let facts = TlsCertFacts {
            delivery: DeliveryBasis { dns_answers: vec![], tcp_connected: false, tls_handshake_completed: false },
            response_horizon: ResponseHorizon { timeout_ms: 10_000, elapsed_ms: Some(10_000) },
            presented_chain: vec![],
            validation: ChainValidation::NotAttempted,
        };
        let r = evaluate_tls_cert(&target(), &facts, &policy(14), &ntp_clock(), at("2026-06-19T15:39:54Z"));
        assert_eq!(r.verdict, TlsCertVerdict::DnsFailed);
        // The negative carries its horizon — not intervention-with-anecdotes.
        assert_eq!(r.response_horizon.timeout_ms, 10_000);
    }

    #[test]
    fn tcp_failed_when_resolved_but_no_connect() {
        let facts = TlsCertFacts {
            delivery: DeliveryBasis { dns_answers: vec!["192.46.223.21".to_string()], tcp_connected: false, tls_handshake_completed: false },
            response_horizon: ResponseHorizon { timeout_ms: 10_000, elapsed_ms: None },
            presented_chain: vec![],
            validation: ChainValidation::NotAttempted,
        };
        assert_eq!(
            evaluate_tls_cert(&target(), &facts, &policy(14), &ntp_clock(), at("2026-06-19T15:39:54Z")).verdict,
            TlsCertVerdict::TcpFailed
        );
    }

    #[test]
    fn tls_handshake_failed_when_connected_but_no_handshake() {
        let facts = TlsCertFacts {
            delivery: DeliveryBasis { dns_answers: vec!["192.46.223.21".to_string()], tcp_connected: true, tls_handshake_completed: false },
            response_horizon: ResponseHorizon { timeout_ms: 10_000, elapsed_ms: None },
            presented_chain: vec![],
            validation: ChainValidation::NotAttempted,
        };
        assert_eq!(
            evaluate_tls_cert(&target(), &facts, &policy(14), &ntp_clock(), at("2026-06-19T15:39:54Z")).verdict,
            TlsCertVerdict::TlsHandshakeFailed
        );
    }

    #[test]
    fn no_certificate_presented_when_handshake_but_empty_chain() {
        // The step-0 false-green: a 0-return handshake with no cert is a
        // NEGATIVE here, never a pass.
        let facts = delivered(vec![]); // delivered() with empty chain
        assert_eq!(
            evaluate_tls_cert(&target(), &facts, &policy(14), &ntp_clock(), at("2026-06-19T15:39:54Z")).verdict,
            TlsCertVerdict::NoCertificatePresented
        );
    }

    // ---- receipt shape: no operational coercion, schema stamped ----

    #[test]
    fn receipt_serializes_with_typed_verdict_and_no_bool_coercion() {
        let r = evaluate_tls_cert(&target(), &delivered(vec![real_leaf()]), &policy(14), &ntp_clock(), at("2026-06-19T15:39:54Z"));
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["schema"], "nq.probe.tls_cert.v1");
        assert_eq!(v["probe_kind"], "tls_cert");
        assert_eq!(v["verdict"], "valid_at_probe_time"); // typed enum, not a bool
        assert_eq!(v["validation_policy"], "webpki");
        assert_eq!(v["perturbation"]["class"], "read_only_tls_handshake");
        // Scope ceiling travels with the receipt.
        assert!(v["non_claims"].as_array().unwrap().iter().any(|c| c
            .as_str()
            .unwrap()
            .contains("host integrity")));
        // No green/ok/healthy boolean anywhere in the surface.
        let s = serde_json::to_string(&r).unwrap();
        assert!(!s.contains("\"is_ok\"") && !s.contains("\"healthy\"") && !s.contains("\"green\""));
    }

    #[test]
    fn pinned_ca_adds_operator_universe_non_claim() {
        let pol = TlsCertPolicy {
            expected_names: vec!["nq.neutral.zone".to_string()],
            warning_threshold_days: 14,
            validation_policy: ValidationPolicy::PinnedCa,
        };
        let r = evaluate_tls_cert(&target(), &delivered(vec![real_leaf()]), &pol, &ntp_clock(), at("2026-06-19T15:39:54Z"));
        assert!(r.non_claims.iter().any(|c| c.contains("operator's own PKI")));
    }
}
