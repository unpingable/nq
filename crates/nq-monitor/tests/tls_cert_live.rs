//! Live TLS-cert probe integration test (slice 2b).
//!
//! **Ignored by default** — it makes a real outbound TLS handshake, so it
//! must not run in plain `cargo test` (per the operator authorization for
//! 2b: live network is a manual/review step, never default behavior). It is
//! the only thing that exercises the rustls handshake + chain-extraction
//! path end to end, which the offline unit tests cannot.
//!
//! Run manually:
//!
//! ```text
//! cargo test -p nq-monitor --test tls_cert_live -- --ignored
//! ```
//!
//! The equivalent operator command (receipt to stdout):
//!
//! ```text
//! nq-monitor probe tls-cert --host nq.neutral.zone --vantage <where-you-are>
//! ```
//!
//! NOTE on vantage: run from a box that is NOT the target's host. This test
//! records a placeholder vantage; the receipt's own non-claim discloses that
//! the vantage basis is operator-provided/manual.

use std::time::Duration;

use nq_monitor::tls_cert_probe::{
    ChainValidation, ClockBasis, TlsCertPolicy, TlsCertTarget, TlsCertVerdict, ValidationPolicy,
};
use nq_monitor::tls_cert_transport::probe_tls_cert;
use time::OffsetDateTime;

#[test]
#[ignore = "makes a real outbound TLS handshake; run with --ignored"]
fn live_probe_nq_neutral_zone_observes_a_valid_chain() {
    let host = "nq.neutral.zone";
    let target = TlsCertTarget {
        target: format!("{host}:443"),
        sni: host.to_string(),
        vantage: "ci-or-dev-box (manual)".to_string(),
    };
    let policy = TlsCertPolicy {
        expected_names: vec![host.to_string()],
        warning_threshold_days: 14,
        validation_policy: ValidationPolicy::Webpki,
    };
    let clock = ClockBasis {
        source: "system_wall".to_string(),
        ntp_status: "unknown".to_string(),
    };

    let receipt = probe_tls_cert(
        host,
        443,
        &target,
        &policy,
        Duration::from_secs(10),
        &clock,
        OffsetDateTime::now_utc(),
    );

    // The probe reached the surface and observed a presented chain.
    assert!(receipt.delivery_basis.tcp_connected, "tcp must connect: {receipt:?}");
    assert!(
        receipt.delivery_basis.tls_handshake_completed,
        "handshake must complete: {receipt:?}"
    );
    assert!(
        !receipt.chain_subjects.is_empty(),
        "a chain must be observed: {receipt:?}"
    );
    assert_eq!(receipt.leaf_not_after.is_some(), true, "leaf dates parsed");

    // Whatever the band, the live verdict must be in the valid family while
    // the cert is current (this test is not a renewal-cycle harness).
    assert!(
        matches!(
            receipt.verdict,
            TlsCertVerdict::ValidAtProbeTime | TlsCertVerdict::ValidButWithinWarningHorizon
        ),
        "expected a valid-family verdict for a live, current cert, got {:?}",
        receipt.verdict
    );

    // A live, current LE cert must VALIDATE under WebPKI at the probe clock
    // (slice 2c) — handshake success is not enough; the chain is validated.
    assert_eq!(
        receipt.validation_result,
        ChainValidation::Valid,
        "live LE chain must validate under webpki-roots at the probe clock: {receipt:?}"
    );
    // Receipt discloses the validation basis (anchor + probe-clock).
    assert!(
        receipt
            .non_claims
            .iter()
            .any(|c| c.contains("webpki-roots")),
        "receipt must disclose the trust-anchor basis"
    );
}
