//! Live TLS-cert probe transport (slice 2b).
//!
//! Connects the pure [`crate::tls_cert_probe`] verdict core to reality:
//! one TCP + TLS handshake from this vantage, **observe** the presented
//! certificate chain, parse it, and hand the facts to the clock-injected
//! evaluator.
//!
//! **Observed ≠ validated.** This transport performs the handshake with an
//! accept-all verifier so it can *observe* the chain a server presents even
//! when that chain is expired, wrong-named, or untrusted — `transport_
//! observed_chain`. It does **not** perform independent WebPKI trust
//! validation (`policy_validated_chain`): `validation` is recorded as
//! `NotAttempted`, and the receipt carries a loud non-claim saying so. That
//! keeps the liar-on-the-phone out by *disclosure* — the receipt confesses
//! exactly what it did not check — rather than by pretending a TLS-library
//! wrapper conferred trust. WebPKI validation tied to the probe clock is a
//! documented follow-up.
//!
//! Receipt-only: no DB writes. No operational-status coercion (the output is
//! a typed verdict from the pure core).

use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};
use sha2::{Digest, Sha256};
// `::time` (the external crate) — the `x509_parser::prelude::*` glob also
// exports a `time` module (asn1-rs re-export), so we must disambiguate.
use ::time::OffsetDateTime;
use x509_parser::prelude::*;

use crate::tls_cert_probe::{
    evaluate_tls_cert, ChainValidation, ClockBasis, DeliveryBasis, PresentedCert, ResponseHorizon,
    TlsCertFacts, TlsCertPolicy, TlsCertProbeReceipt, TlsCertTarget,
};

/// Probe one TLS surface and return its receipt. Always returns a receipt:
/// delivery failures (DNS / TCP / handshake) are encoded as facts and turned
/// into the corresponding negative verdict by the pure core, never lost.
pub fn probe_tls_cert(
    host: &str,
    port: u16,
    target: &TlsCertTarget,
    policy: &TlsCertPolicy,
    timeout: Duration,
    clock: &ClockBasis,
    now: OffsetDateTime,
) -> TlsCertProbeReceipt {
    let started = Instant::now();
    let timeout_ms = timeout.as_millis() as u64;

    // --- DNS ---
    let addrs: Vec<std::net::SocketAddr> = match (host, port).to_socket_addrs() {
        Ok(it) => it.collect(),
        Err(_) => Vec::new(),
    };
    let dns_answers: Vec<String> = addrs.iter().map(|a| a.ip().to_string()).collect();

    let fail = |delivery: DeliveryBasis, elapsed_ms: Option<u64>| {
        let facts = TlsCertFacts {
            delivery,
            response_horizon: ResponseHorizon { timeout_ms, elapsed_ms },
            presented_chain: vec![],
            validation: ChainValidation::NotAttempted,
        };
        finish(target, &facts, policy, clock, now)
    };

    let Some(addr) = addrs.first().copied() else {
        return fail(
            DeliveryBasis { dns_answers, tcp_connected: false, tls_handshake_completed: false },
            Some(started.elapsed().as_millis() as u64),
        );
    };

    // --- TCP ---
    let mut sock = match TcpStream::connect_timeout(&addr, timeout) {
        Ok(s) => s,
        Err(_) => {
            return fail(
                DeliveryBasis { dns_answers, tcp_connected: false, tls_handshake_completed: false },
                Some(started.elapsed().as_millis() as u64),
            );
        }
    };
    let _ = sock.set_read_timeout(Some(timeout));
    let _ = sock.set_write_timeout(Some(timeout));

    // --- TLS handshake (observe-only) ---
    let chain = match observe_chain(&mut sock, &target.sni) {
        Ok(c) => c,
        Err(_) => {
            return fail(
                DeliveryBasis { dns_answers, tcp_connected: true, tls_handshake_completed: false },
                Some(started.elapsed().as_millis() as u64),
            );
        }
    };

    let presented_chain: Vec<PresentedCert> = chain
        .iter()
        .filter_map(|der| parse_presented_cert(der).ok())
        .collect();

    let facts = TlsCertFacts {
        delivery: DeliveryBasis { dns_answers, tcp_connected: true, tls_handshake_completed: true },
        response_horizon: ResponseHorizon {
            timeout_ms,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
        },
        presented_chain,
        validation: ChainValidation::NotAttempted,
    };
    finish(target, &facts, policy, clock, now)
}

/// Evaluate + append the transport's honesty non-claims.
fn finish(
    target: &TlsCertTarget,
    facts: &TlsCertFacts,
    policy: &TlsCertPolicy,
    clock: &ClockBasis,
    now: OffsetDateTime,
) -> TlsCertProbeReceipt {
    let mut receipt = evaluate_tls_cert(target, facts, policy, clock, now);
    receipt.non_claims.push(
        "transport observed the presented chain only; trust-chain (webpki) was NOT independently \
         validated by transport v1 — the verdict rests on name + probe-clock, so a chain from an \
         untrusted issuer would still read valid_at_probe_time (validation=not_attempted)"
            .to_string(),
    );
    receipt.non_claims.push(format!(
        "vantage basis is operator-provided/manual: '{}' — no independent external runner asserts it",
        target.vantage
    ));
    receipt
}

/// Drive a TLS handshake with an accept-all verifier and return the DER of
/// each certificate the server presented (leaf first). Accept-all is what
/// lets us observe an expired/untrusted chain instead of having the
/// handshake abort before we can witness it.
fn observe_chain(sock: &mut TcpStream, sni: &str) -> anyhow::Result<Vec<Vec<u8>>> {
    use rustls::pki_types::ServerName;

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(ObserveOnlyVerifier {
        schemes: provider.signature_verification_algorithms.supported_schemes(),
    });
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .context("rustls protocol versions")?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

    let server_name = ServerName::try_from(sni.to_string())
        .map_err(|e| anyhow!("invalid SNI {sni:?}: {e}"))?;
    let mut conn = rustls::ClientConnection::new(Arc::new(config), server_name)
        .context("rustls client connection")?;

    // Drive IO until the handshake completes. Capped so a wedged peer cannot
    // spin forever inside the response horizon.
    let mut rounds = 0;
    while conn.is_handshaking() {
        conn.complete_io(sock).context("tls handshake io")?;
        rounds += 1;
        if rounds > 16 {
            return Err(anyhow!("handshake did not converge"));
        }
    }
    // We never send application data; close politely.
    let _ = conn.writer().flush();

    let certs = conn
        .peer_certificates()
        .ok_or_else(|| anyhow!("no peer certificates presented"))?;
    Ok(certs.iter().map(|c| c.as_ref().to_vec()).collect())
}

/// Parse one DER certificate into the verdict core's [`PresentedCert`].
/// Pure (no network) — unit-tested against a real embedded leaf.
pub fn parse_presented_cert(der: &[u8]) -> anyhow::Result<PresentedCert> {
    let (_, cert) =
        X509Certificate::from_der(der).map_err(|e| anyhow!("x509 parse: {e}"))?;

    let sans = match cert.subject_alternative_name() {
        Ok(Some(ext)) => ext
            .value
            .general_names
            .iter()
            .filter_map(|gn| match gn {
                GeneralName::DNSName(n) => Some(n.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };

    let mut hasher = Sha256::new();
    hasher.update(der);
    let digest = hasher.finalize();
    let fingerprint = digest
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":");

    Ok(PresentedCert {
        subject: cert.subject().to_string(),
        issuer: cert.issuer().to_string(),
        not_before: cert.validity().not_before.to_datetime(),
        not_after: cert.validity().not_after.to_datetime(),
        sans,
        sha256_fingerprint: fingerprint,
    })
}

/// Accept-all verifier: completes the handshake without asserting trust, so
/// the transport can *observe* whatever chain is presented. Trust is NOT
/// conferred — the receipt records `validation = NotAttempted`.
#[derive(Debug)]
struct ObserveOnlyVerifier {
    schemes: Vec<rustls::SignatureScheme>,
}

impl rustls::client::danger::ServerCertVerifier for ObserveOnlyVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.schemes.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real nq.neutral.zone leaf (captured 2026-06-19). Parsing is a pure
    /// function of these bytes — the test pins the parser, not liveness.
    const NQ_LEAF_PEM: &str = "-----BEGIN CERTIFICATE-----
MIIDjDCCAxKgAwIBAgISBv8oalAEBkJfyAFcaOAMxi2sMAoGCCqGSM49BAMDMDMx
CzAJBgNVBAYTAlVTMRYwFAYDVQQKEw1MZXQncyBFbmNyeXB0MQwwCgYDVQQDEwNZ
RTIwHhcNMjYwNTMxMTkwODUzWhcNMjYwODI5MTkwODUyWjAaMRgwFgYDVQQDEw9u
cS5uZXV0cmFsLnpvbmUwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAARMwKV6EMpc
3jyYezUEfApqAQtJsgUlOnqJPPWzGWW0pOk6cW+JimWKdR06ItWdvBliRUJf6DOv
+TI88qxeoAEno4ICHTCCAhkwDgYDVR0PAQH/BAQDAgeAMBMGA1UdJQQMMAoGCCsG
AQUFBwMBMAwGA1UdEwEB/wQCMAAwHQYDVR0OBBYEFJk5DJZsY+Nyzyz2PJUAa6Yt
dRUCMB8GA1UdIwQYMBaAFLlZ8o7PIvCG0zdI/3YUGLqC2FWHMDMGCCsGAQUFBwEB
BCcwJTAjBggrBgEFBQcwAoYXaHR0cDovL3llMi5pLmxlbmNyLm9yZy8wGgYDVR0R
BBMwEYIPbnEubmV1dHJhbC56b25lMBMGA1UdIAQMMAowCAYGZ4EMAQIBMC4GA1Ud
HwQnMCUwI6AhoB+GHWh0dHA6Ly95ZTIuYy5sZW5jci5vcmcvNDUuY3JsMIIBDAYK
KwYBBAHWeQIEAgSB/QSB+gD4AHYAyKPEf8ezrbk1awE/anoSbeM6TkOlxkb5l605
dZkdz5oAAAGef6X3IQAABAMARzBFAiAt05r6B5sq9L28eW5HmorTZ/Z1F1s6WBD2
TZUZvzvv+QIhAPXKHXQz862PIt/evOdSC/DFIvPFG4kU9gVUERYgbcMBAH4AbP5Q
GUOoXqkWvFLRM+TcyR7xQRx9JYQg0XOAnhgY6zoAAAGef6X5KgAIAAAFAA5NLhoE
AwBHMEUCIQDPw3qiAFoOuWQOIrz6ZhDmDmD9Kw1H3Tusbf3Vja15GgIgNN4Zt3YC
iFVt9HunTH3Iq9+CJm4G/cNXhS2xaOsDlS4wCgYIKoZIzj0EAwMDaAAwZQIwf5uK
Xz2auDAxyKg4f3J45hdX4GYI+/1dJot6F3wvndyQWW3pMNt2r8ovtujTzRnrAjEA
ocyaPWz6N9u7AFEOjxt/pNUTy6rNwi0qmuYoIr3lN2c6oW7bpzOAukKy7VdfNUYT
-----END CERTIFICATE-----";

    fn nq_leaf_der() -> Vec<u8> {
        let (_, pem) = x509_parser::pem::parse_x509_pem(NQ_LEAF_PEM.as_bytes())
            .expect("pem fixture parses");
        pem.contents
    }

    #[test]
    fn parses_real_leaf_fields() {
        let der = nq_leaf_der();
        let c = parse_presented_cert(&der).expect("parse");
        assert!(c.subject.contains("nq.neutral.zone"), "subject: {}", c.subject);
        assert!(c.issuer.contains("Let's Encrypt"), "issuer: {}", c.issuer);
        assert!(c.issuer.contains("YE2"), "issuer: {}", c.issuer);
        assert_eq!(c.sans, vec!["nq.neutral.zone".to_string()]);
    }

    #[test]
    fn parses_real_leaf_validity_window() {
        let c = parse_presented_cert(&nq_leaf_der()).unwrap();
        // Immutable properties of this cert blob (matches the step-0 receipt).
        assert_eq!(
            c.not_before,
            OffsetDateTime::parse("2026-05-31T19:08:53Z", &::time::format_description::well_known::Rfc3339).unwrap()
        );
        assert_eq!(
            c.not_after,
            OffsetDateTime::parse("2026-08-29T19:08:52Z", &::time::format_description::well_known::Rfc3339).unwrap()
        );
    }

    #[test]
    fn fingerprint_is_colon_hex_sha256() {
        let c = parse_presented_cert(&nq_leaf_der()).unwrap();
        // 32 bytes -> 32 colon-separated hex pairs.
        assert_eq!(c.sha256_fingerprint.split(':').count(), 32);
        assert!(c.sha256_fingerprint.chars().all(|ch| ch.is_ascii_hexdigit() || ch == ':'));
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse_presented_cert(b"not a certificate").is_err());
    }
}
