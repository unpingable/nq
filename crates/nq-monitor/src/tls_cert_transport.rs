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

    // SEPARATE step: validate the observed chain under WebPKI at the probe
    // clock. Observation (above, accept-all) and validation (here) are
    // distinct acts — a completed handshake is not a validated chain.
    let validation = validate_observed_chain(&chain, &target.sni, now);

    let facts = TlsCertFacts {
        delivery: DeliveryBasis { dns_answers, tcp_connected: true, tls_handshake_completed: true },
        response_horizon: ResponseHorizon {
            timeout_ms,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
        },
        presented_chain,
        validation,
    };
    finish(target, &facts, policy, clock, now)
}

/// Validate the OBSERVED chain under WebPKI against the bundled
/// `webpki-roots`, at the **injected probe clock** — not wall time. This is
/// the separate validation act; the transport's accept-all handshake only
/// observed the chain. Expiry is reported by WebPKI as a validation failure
/// here, but the verdict layer maps an expired leaf to
/// `expired_under_probe_clock` (checked before `chain_invalid`), so
/// `chain_invalid` is reserved for non-expiry trust failures.
fn validate_observed_chain(chain: &[Vec<u8>], sni: &str, now: OffsetDateTime) -> ChainValidation {
    use rustls::client::danger::ServerCertVerifier;
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

    if chain.is_empty() {
        return ChainValidation::Invalid { reason: "no certificate presented".to_string() };
    }

    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier =
        match rustls::client::WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
            .build()
        {
            Ok(v) => v,
            Err(e) => return ChainValidation::Invalid { reason: format!("verifier build: {e}") },
        };

    let ders: Vec<CertificateDer<'_>> =
        chain.iter().map(|d| CertificateDer::from(d.as_slice())).collect();
    let (leaf, intermediates) = ders.split_first().expect("non-empty checked above");

    let server_name = match ServerName::try_from(sni.to_string()) {
        Ok(n) => n,
        Err(e) => return ChainValidation::Invalid { reason: format!("invalid sni: {e}") },
    };
    let unix = UnixTime::since_unix_epoch(std::time::Duration::from_secs(
        now.unix_timestamp().max(0) as u64,
    ));

    match verifier.verify_server_cert(leaf, intermediates, &server_name, &[], unix) {
        Ok(_) => ChainValidation::Valid,
        Err(e) => ChainValidation::Invalid { reason: e.to_string() },
    }
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
    let validation_basis = match &facts.validation {
        ChainValidation::NotAttempted => {
            "trust validation not attempted — no chain was delivered to validate".to_string()
        }
        _ => "trust-chain validated under WebPKI against bundled webpki-roots, at the probe clock; \
              a successful TLS handshake is NOT a successful validation — the observed chain and \
              the validation_result are distinct"
            .to_string(),
    };
    receipt.non_claims.push(validation_basis);
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

    // ---- WebPKI validation at the probe clock (slice 2c) ----

    fn at(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &::time::format_description::well_known::Rfc3339).unwrap()
    }

    /// The real full chain nq.neutral.zone presents (captured 2026-06-19):
    /// leaf -> LE YE2 -> ISRG Root YE -> ISRG Root X2. X2 is a webpki-roots
    /// anchor, so this validates offline & deterministically in-window.
    const NQ_FULL_CHAIN_PEM: &str = "-----BEGIN CERTIFICATE-----
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
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
MIICjDCCAhGgAwIBAgIQTfOxXdbAeExQfNN7WObxFTAKBggqhkjOPQQDAzAuMQsw
CQYDVQQGEwJVUzENMAsGA1UEChMESVNSRzEQMA4GA1UEAxMHUm9vdCBZRTAeFw0y
NTA5MDMwMDAwMDBaFw0yODA5MDIyMzU5NTlaMDMxCzAJBgNVBAYTAlVTMRYwFAYD
VQQKEw1MZXQncyBFbmNyeXB0MQwwCgYDVQQDEwNZRTIwdjAQBgcqhkjOPQIBBgUr
gQQAIgNiAARxmrQzkdbEEL3MqXt3dJQttYc47axkdDTHud5TPqM2z5uSD5cmk0Wr
HlWXvnlvqBLqiB34kluxIbmMyAiq3/YD6e80/vV259K8XQIdjFXloYOa0mIU71f7
HQ09PvYDlw+jge4wgeswDgYDVR0PAQH/BAQDAgGGMBMGA1UdJQQMMAoGCCsGAQUF
BwMBMBIGA1UdEwEB/wQIMAYBAf8CAQAwHQYDVR0OBBYEFLlZ8o7PIvCG0zdI/3YU
GLqC2FWHMB8GA1UdIwQYMBaAFKPIJlqOoUzQNWP8myPIOq5W809WMDIGCCsGAQUF
BwEBBCYwJDAiBggrBgEFBQcwAoYWaHR0cDovL3llLmkubGVuY3Iub3JnLzATBgNV
HSAEDDAKMAgGBmeBDAECATAnBgNVHR8EIDAeMBygGqAYhhZodHRwOi8veWUuYy5s
ZW5jci5vcmcvMAoGCCqGSM49BAMDA2kAMGYCMQDIcnw5dcZLN9ffynXnnkLD/itS
JEycJPb3sRkzeqBowup7vOsAwaqoCnNn/jh9wycCMQCJM6CPlaOC4pQYYbJtVPYb
DKrIb2EKk5NpOpE6/XttQYZV/3gilB9l+Cc/DOVwmyg=
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
MIICpjCCAiugAwIBAgIRAIchZfw0tuX7qK3Vs3BftTowCgYIKoZIzj0EAwMwTzEL
MAkGA1UEBhMCVVMxKTAnBgNVBAoTIEludGVybmV0IFNlY3VyaXR5IFJlc2VhcmNo
IEdyb3VwMRUwEwYDVQQDEwxJU1JHIFJvb3QgWDIwHhcNMjYwNTEzMDAwMDAwWhcN
MzIwOTAyMjM1OTU5WjAuMQswCQYDVQQGEwJVUzENMAsGA1UEChMESVNSRzEQMA4G
A1UEAxMHUm9vdCBZRTB2MBAGByqGSM49AgEGBSuBBAAiA2IABDwS/6vhrcVqcbBo
+wgdI3fwn9x7DNJJOY/lTOti0vkwuRN87RhEhTH17E7XyFjWsPYhIPt/wzOqxTd2
b+4ZJNy9ID04YywF9U5zasDVyGSNErVNtz8uSGh5izW87j77GaOB6zCB6DAOBgNV
HQ8BAf8EBAMCAQYwEwYDVR0lBAwwCgYIKwYBBQUHAwEwDwYDVR0TAQH/BAUwAwEB
/zAdBgNVHQ4EFgQUo8gmWo6hTNA1Y/ybI8g6rlbzT1YwHwYDVR0jBBgwFoAUfEKW
rt5LSDv6kviejM9ti6lyN5UwMgYIKwYBBQUHAQEEJjAkMCIGCCsGAQUFBzAChhZo
dHRwOi8veDIuaS5sZW5jci5vcmcvMBMGA1UdIAQMMAowCAYGZ4EMAQIBMCcGA1Ud
HwQgMB4wHKAaoBiGFmh0dHA6Ly94Mi5jLmxlbmNyLm9yZy8wCgYIKoZIzj0EAwMD
aQAwZgIxAMU19WCtmxVND8UHBZRoma49Z7jPs64Dma0eTu1OChVbB/2J7GV3nvYK
Ax54uk1G9QIxAO0miLVJu8PLNiXXXkiE/gsK3CTRTF/aeo4bMX42Zw40csRU6AC2
6hSW1/IWaas6dg==
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
MIIEcDCCAligAwIBAgIQbI8dxyfHEX97r4U6yYD5zTANBgkqhkiG9w0BAQsFADBP
MQswCQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJuZXQgU2VjdXJpdHkgUmVzZWFy
Y2ggR3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCBYMTAeFw0yNjA1MTMwMDAwMDBa
Fw0zMjA5MDIyMzU5NTlaME8xCzAJBgNVBAYTAlVTMSkwJwYDVQQKEyBJbnRlcm5l
dCBTZWN1cml0eSBSZXNlYXJjaCBHcm91cDEVMBMGA1UEAxMMSVNSRyBSb290IFgy
MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAEzZvVn4CDCuwJSvMWSj5cz3es3mcFDR0H
ttwW+1qLFNvicWDEukWVEYmO6gbf9yoWHKS5xcUy4APgHoIYOIvXRdgKam7mAHf7
AlF9ItgKbppbd9/w+kHsOdx1ymgHDB/qo4H1MIHyMA4GA1UdDwEB/wQEAwIBBjAd
BgNVHSUEFjAUBggrBgEFBQcDAQYIKwYBBQUHAwIwDwYDVR0TAQH/BAUwAwEB/zAd
BgNVHQ4EFgQUfEKWrt5LSDv6kviejM9ti6lyN5UwHwYDVR0jBBgwFoAUebRZ5nu2
5eQBc4AIiMgaWPbpm24wMgYIKwYBBQUHAQEEJjAkMCIGCCsGAQUFBzAChhZodHRw
Oi8veDEuaS5sZW5jci5vcmcvMBMGA1UdIAQMMAowCAYGZ4EMAQIBMCcGA1UdHwQg
MB4wHKAaoBiGFmh0dHA6Ly94MS5jLmxlbmNyLm9yZy8wDQYJKoZIhvcNAQELBQAD
ggIBAD2/e9frmMxNpCV03qUHegg+MV2wz9644YoXdqtH8RyWYcBO7xfjjGEXdU1e
/o0OkEFiynUCOSIk/vLLo7ttz6CPAeNlWfC0XNkoGeWgK6jjXvozBaGuGH5n0Ufo
shMeWTuURqNN5G00sSXDTBrpp2+mgvdZQjb8K11TYMA25QA+YHNfbIEL0BniAhKS
2gsnJjSzrdZLI+EZ7SEyqdR2rkjd1KutLDU+n3TFyxjniZVGur4YlhMP3mY/dV95
IruAkkjOZier6hGBdEgZXXvaCz9u9iVEadsIE75pAGL8oHV5vxdARDiotRpul1IN
/UZwzAbrfUFcw1HkAcYD/mlZfnQ2ieCF2MS7j3Vhv7JPDKp45fmykmzYNSrumRW0
upFFKDBOoF7hsOb7oLyHS+Uft6jOUfOrogj8YUx38hKb2K20r42OgsSdDdxdeYWc
MS3Sb6mwJeSZEYxJ2gaXnDSPaKhhrNkYwljyVQyr4Nq+MEJytXNTnHqaAcrNwZlV
pcJL1KBnMrMjP7eanvUwL3FYj3cF17jtboLt7gLoi4+2rWZFvn+w54jmd/FIuhhZ
cEaU/wvU6BUNMtcVquVGHp7itQeDth5j+XL3j4WJ2SABwzUl6OeYdgpIt/ITZa+p
TT0mQ/r5XyA4MEAiabn7XJjvCERlF2dcn2wqJw+CreTkkQ2R
-----END CERTIFICATE-----";

    fn nq_full_chain_der() -> Vec<Vec<u8>> {
        let mut rest = NQ_FULL_CHAIN_PEM.as_bytes();
        let mut out = Vec::new();
        while !rest.iter().all(|b| b.is_ascii_whitespace()) {
            let (remaining, pem) =
                x509_parser::pem::parse_x509_pem(rest).expect("pem bundle parses");
            out.push(pem.contents);
            rest = remaining;
        }
        out
    }

    #[test]
    fn webpki_valid_full_chain_in_window() {
        let chain = nq_full_chain_der();
        assert_eq!(chain.len(), 4, "full presented chain");
        let v = validate_observed_chain(&chain, "nq.neutral.zone", at("2026-06-19T00:00:00Z"));
        assert_eq!(v, ChainValidation::Valid, "real chain must validate in-window: {v:?}");
    }

    #[test]
    fn webpki_invalid_incomplete_chain() {
        // Leaf alone cannot build a path to any trust anchor.
        let chain = nq_full_chain_der();
        let leaf_only = vec![chain[0].clone()];
        let v = validate_observed_chain(&leaf_only, "nq.neutral.zone", at("2026-06-19T00:00:00Z"));
        assert!(matches!(v, ChainValidation::Invalid { .. }), "leaf-only must not validate: {v:?}");
    }

    #[test]
    fn webpki_clock_drives_validation() {
        let chain = nq_full_chain_der();
        // In-window -> Valid.
        assert_eq!(
            validate_observed_chain(&chain, "nq.neutral.zone", at("2026-06-19T00:00:00Z")),
            ChainValidation::Valid
        );
        // After the leaf's notAfter (2026-08-29) -> Invalid (expired). Same
        // bytes, different injected clock: the clock drives validation.
        assert!(matches!(
            validate_observed_chain(&chain, "nq.neutral.zone", at("2026-09-15T00:00:00Z")),
            ChainValidation::Invalid { .. }
        ));
    }

    #[test]
    fn webpki_name_mismatch_is_invalid() {
        let chain = nq_full_chain_der();
        let v = validate_observed_chain(&chain, "wrong.example.com", at("2026-06-19T00:00:00Z"));
        assert!(matches!(v, ChainValidation::Invalid { .. }), "wrong name must not validate: {v:?}");
    }
}
