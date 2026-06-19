//! Manual receipt-series sink for the TLS-cert probe (slice 2d-a).
//!
//! The smallest append-only persistence for probe receipts: one timestamped
//! directory per probe, one `<host>.json` file inside, **never overwritten**.
//! Append-only-ness is enforced by refusing to write over an existing file.
//!
//! This is **manual collection, not monitoring.** There is no scheduler, no
//! timer, no DB, no alerting. A *missing* receipt is **not** a negative here —
//! the series makes no completeness claim. The driver is the operator running
//! `nq-monitor probe tls-cert --out-dir <dir>` by hand. Deciding cadence,
//! external runner, retention, and what an absent receipt means is a separate
//! step (2d-b), deliberately not taken here.
//!
//! Storage shape (timestamped files, not JSONL — easier to inspect, and one
//! malformed receipt does not poison the rest):
//!
//! ```text
//! runs/tls-cert-probe/<YYYYMMDDTHHMMSSZ>/<host_slug>.json
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::tls_cert_probe::TlsCertProbeReceipt;

/// Append a receipt to the series under `base`. Returns the path written.
/// Append-only: if the target file already exists, this refuses rather than
/// overwrite, so no datapoint is ever silently clobbered.
pub fn persist_receipt(base: &Path, receipt: &TlsCertProbeReceipt) -> anyhow::Result<PathBuf> {
    let dir = base.join(run_stamp(&receipt.probe_time));
    fs::create_dir_all(&dir).with_context(|| format!("create series dir {}", dir.display()))?;

    let path = dir.join(format!("{}.json", host_slug(&receipt.target)));
    if path.exists() {
        anyhow::bail!(
            "refusing to overwrite existing receipt {} — the series is append-only",
            path.display()
        );
    }

    let json = serde_json::to_string_pretty(receipt).context("serialize receipt")?;
    fs::write(&path, format!("{json}\n")).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// Compress an RFC3339 probe time into a filesystem-friendly run stamp:
/// `2026-06-19T22:49:06.864Z` -> `20260619T224906Z`. Subseconds dropped;
/// second-granularity is enough for a manual series.
fn run_stamp(probe_time: &str) -> String {
    let base = probe_time
        .split('.')
        .next()
        .unwrap_or(probe_time)
        .trim_end_matches('Z');
    let cleaned: String = base.chars().filter(|c| *c != '-' && *c != ':').collect();
    format!("{cleaned}Z")
}

/// Make a `host:port` target safe as a filename: `nq.neutral.zone:443` ->
/// `nq.neutral.zone_443`.
fn host_slug(target: &str) -> String {
    target
        .chars()
        .map(|c| match c {
            ':' | '/' | '\\' => '_',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls_cert_probe::{
        ChainValidation, ClockBasis, DeliveryBasis, Perturbation, ResponseHorizon, TlsCertVerdict,
        ValidationPolicy,
    };

    fn sample_receipt() -> TlsCertProbeReceipt {
        TlsCertProbeReceipt {
            schema: "nq.probe.tls_cert.v1",
            probe_kind: "tls_cert",
            target: "nq.neutral.zone:443".to_string(),
            sni: "nq.neutral.zone".to_string(),
            expected_names: vec!["nq.neutral.zone".to_string()],
            vantage: "operator-dev-box".to_string(),
            probe_time: "2026-06-19T22:49:06.864672162Z".to_string(),
            clock_basis: ClockBasis { source: "system_wall".to_string(), ntp_status: "unknown".to_string() },
            delivery_basis: DeliveryBasis {
                dns_answers: vec!["192.46.223.21".to_string()],
                tcp_connected: true,
                tls_handshake_completed: true,
            },
            response_horizon: ResponseHorizon { timeout_ms: 10_000, elapsed_ms: Some(199) },
            chain_subjects: vec!["CN=nq.neutral.zone".to_string()],
            chain_fingerprints: vec!["AA:BB".to_string()],
            leaf_not_before: Some("2026-05-31T19:08:53Z".to_string()),
            leaf_not_after: Some("2026-08-29T19:08:52Z".to_string()),
            issuer: Some("LE".to_string()),
            days_remaining: Some(70),
            warning_threshold_days: 14,
            validation_policy: ValidationPolicy::Webpki,
            validation_result: ChainValidation::Valid,
            perturbation: Perturbation { class: "read_only_tls_handshake", expected_side_effects: vec!["access_log"] },
            verdict: TlsCertVerdict::ValidAtProbeTime,
            non_claims: vec![],
        }
    }

    #[test]
    fn run_stamp_compresses_rfc3339() {
        assert_eq!(run_stamp("2026-06-19T22:49:06.864672162Z"), "20260619T224906Z");
        assert_eq!(run_stamp("2026-06-19T22:49:06Z"), "20260619T224906Z");
    }

    #[test]
    fn host_slug_is_filename_safe() {
        assert_eq!(host_slug("nq.neutral.zone:443"), "nq.neutral.zone_443");
        assert_eq!(host_slug("[::1]:443"), "[__1]_443");
    }

    #[test]
    fn persist_writes_a_timestamped_receipt() {
        let dir = tempfile::tempdir().unwrap();
        let path = persist_receipt(dir.path(), &sample_receipt()).unwrap();
        assert!(path.ends_with("20260619T224906Z/nq.neutral.zone_443.json"), "{}", path.display());
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("\"schema\": \"nq.probe.tls_cert.v1\""));
        assert!(body.ends_with('\n'));
    }

    #[test]
    fn append_only_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let r = sample_receipt();
        persist_receipt(dir.path(), &r).unwrap();
        // Same probe_time + target -> same path -> must refuse, not clobber.
        let err = persist_receipt(dir.path(), &r).unwrap_err();
        assert!(err.to_string().contains("append-only"), "{err}");
    }
}
