//! `nq-monitor probe dns` — V0 DNS probe subcommand.
//!
//! Single tuple per invocation. Writes one `dns_observations` row
//! into the latest existing aggregator generation. See
//! `docs/working/gaps/DNS_WITNESS_FAMILY_GAP.md` for the V0 scope and the
//! constitutional refusal surface.
//!
//! The CLI parses the inputs, dispatches to the library
//! `nq::probe::record_probe` with the real `UdpDnsClient`, and prints
//! a one-line summary. Network and classification logic live in
//! `nq::probe`; this module is the user-facing shim.

use crate::cli::{ProbeAction, ProbeCmd, ProbeDnsCmd, ProbeTlsCertCmd};
use crate::probe::{
    parse_qtype, parse_resolver, read_latest_generation_id, record_probe, UdpDnsClient,
};
use crate::tls_cert_probe::{ClockBasis, TlsCertPolicy, TlsCertTarget, ValidationPolicy};
use crate::tls_cert_transport::probe_tls_cert;
use anyhow::Context;
use nq_db::open_rw;
use std::time::Duration;

pub fn run(cmd: ProbeCmd) -> anyhow::Result<()> {
    match cmd.action {
        ProbeAction::Dns(c) => run_dns(c),
        ProbeAction::TlsCert(c) => run_tls_cert(c),
    }
}

fn run_dns(cmd: ProbeDnsCmd) -> anyhow::Result<()> {
    // Validate inputs at the boundary so the library never sees bad
    // values. Resolver parsing is enforced here (CLI-friendly error)
    // but the same call lives inside UdpDnsClient::query as a
    // defense-in-depth check; both layers agree.
    let _resolver_addr = parse_resolver(&cmd.resolver)?;
    let qtype = parse_qtype(&cmd.query_type)?;

    let db = open_rw(&cmd.db).with_context(|| format!("open_rw {:?}", cmd.db))?;
    let gen_id = read_latest_generation_id(db.conn())?;

    let client = UdpDnsClient;
    let observed = record_probe(
        db.conn(),
        gen_id,
        &cmd.vantage,
        &cmd.resolver,
        &cmd.name,
        &cmd.query_type,
        qtype,
        Duration::from_secs(cmd.timeout_seconds),
        &client,
    )?;

    // One-line summary. Closed taxonomy stays visible to the reader —
    // the response_kind is the load-bearing word, not "ok" / "failed".
    let summary = observed
        .answer_summary
        .as_deref()
        .map(|s| format!(" answer={s}"))
        .unwrap_or_default();
    let ttl = observed
        .min_ttl_seconds
        .map(|t| format!(" min_ttl={t}s"))
        .unwrap_or_default();
    let detail = observed
        .error_detail
        .as_deref()
        .map(|d| format!(" detail={d:?}"))
        .unwrap_or_default();
    println!(
        "OK gen={} vantage={} resolver={} ({}, {}) response_kind={} duration={}ms{}{}{}",
        gen_id,
        observed.vantage_host,
        observed.resolver,
        observed.query_name,
        observed.query_type,
        observed.response_kind.as_str(),
        observed.duration_ms,
        summary,
        ttl,
        detail,
    );

    Ok(())
}

/// `nq-monitor probe tls-cert` — live TLS-cert active-witness probe.
/// Receipt-only: prints the `nq.probe.tls_cert.v1` receipt to stdout, no
/// DB write. Observes the presented chain (no independent trust
/// validation — the receipt says so) and lets the clock-injected core
/// decide name + expiry.
fn run_tls_cert(cmd: ProbeTlsCertCmd) -> anyhow::Result<()> {
    let sni = cmd.sni.clone().unwrap_or_else(|| cmd.host.clone());
    let expected_names = if cmd.expected_names.is_empty() {
        vec![cmd.host.clone()]
    } else {
        cmd.expected_names.clone()
    };

    let target = TlsCertTarget {
        target: format!("{}:{}", cmd.host, cmd.port),
        sni,
        vantage: cmd.vantage.clone(),
    };
    let policy = TlsCertPolicy {
        expected_names,
        warning_threshold_days: cmd.warning_days,
        validation_policy: ValidationPolicy::Webpki,
    };
    // The probe clock. NTP sync state is not observable from inside this
    // CLI, so record it honestly as unknown rather than claiming sync.
    let clock = ClockBasis {
        source: "system_wall".to_string(),
        ntp_status: "unknown".to_string(),
    };
    let now = ::time::OffsetDateTime::now_utc();

    let receipt = probe_tls_cert(
        &cmd.host,
        cmd.port,
        &target,
        &policy,
        Duration::from_secs(cmd.timeout_seconds),
        &clock,
        now,
    );

    // Receipt-only emit. No DB write (this is the active-witness lane).
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
