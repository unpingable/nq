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

use crate::cli::{
    ProbeAction, ProbeCmd, ProbeDeclaredDenyCmd, ProbeDnsCmd, ProbeGatewayPathCmd,
    ProbeLeasePresenceCmd, ProbeTlsCertCmd,
};
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
        ProbeAction::LeasePresence(c) => run_lease_presence(c),
        ProbeAction::GatewayPath(c) => run_gateway_path(c),
        ProbeAction::DeclaredDeny(c) => run_declared_deny(c),
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

    // Optional manual append-only series sink. stdout above is unchanged.
    if let Some(out_dir) = &cmd.out_dir {
        let path = crate::tls_cert_series::persist_receipt(out_dir, &receipt)?;
        eprintln!("appended receipt to series: {}", path.display());
    }
    Ok(())
}

/// `nq-monitor probe lease-presence` — live lease-vs-presence read.
/// Read-only over SSH (detect backend + cat the lease file + `arp -an`),
/// plus an optional presence probe from the named vantage. Receipt-only;
/// no DB write. The verdict is a non-lift: an active lease is not presence,
/// and an uncorroborated lease is not a down host.
fn run_lease_presence(cmd: ProbeLeasePresenceCmd) -> anyhow::Result<()> {
    use crate::lease_presence_probe::ClockBasis;
    use crate::lease_presence_transport::{live_lease_presence, persist_receipt, ProbeSpec, SshTarget};

    let target = SshTarget {
        host: cmd.host.clone(),
        port: cmd.port,
        user: cmd.user.clone(),
        key_path: cmd.key.clone(),
        timeout_seconds: cmd.timeout_seconds,
    };
    // TCP probe wins if both are given; otherwise ICMP iff --probe; else no
    // independent probe (ARP-only — comparing two pfSense self-reports).
    let probe = match (cmd.probe_tcp, cmd.probe) {
        (Some(port), _) => Some(ProbeSpec::Tcp(port)),
        (None, true) => Some(ProbeSpec::Icmp),
        (None, false) => None,
    };
    // NTP sync state is not observable from here; record it honestly.
    let clock = ClockBasis {
        source: "system_wall".to_string(),
        ntp_status: "unknown".to_string(),
    };
    let now = ::time::OffsetDateTime::now_utc();

    let receipt = live_lease_presence(&target, &cmd.vantage, &cmd.subject, probe, &clock, now)?;

    // Receipt-only emit. No DB write (active-witness lane).
    println!("{}", serde_json::to_string_pretty(&receipt)?);

    if let Some(out_dir) = &cmd.out_dir {
        let path = persist_receipt(out_dir, &receipt)?;
        eprintln!("appended receipt to series: {}", path.display());
    }
    Ok(())
}

/// `nq-monitor probe declared-deny` — live declared-deny custody read.
/// Read-only over SSH (`pfctl -sr -vv`), plus a control probe (proving egress)
/// and — only if `--subject` is bound — a subject probe of the declared-denied
/// path. Receipt-only; no DB write. The verdict is declaration-vs-observation
/// custody: a got-through is the contradiction, an unbound subject is
/// cannot_testify, a missing rule is cannot_testify (never "allowed").
fn run_declared_deny(cmd: ProbeDeclaredDenyCmd) -> anyhow::Result<()> {
    use crate::declared_deny_probe::ClockBasis;
    use crate::declared_deny_transport::{live_declared_deny, persist_receipt, RuleSelector, SshTarget};

    let selector = match (cmd.table.as_deref(), cmd.ridentifier.as_deref()) {
        (Some(t), None) => RuleSelector::Table(t.to_string()),
        (None, Some(r)) => RuleSelector::Ridentifier(r.to_string()),
        (Some(_), Some(_)) => {
            anyhow::bail!("pass exactly one of --table / --ridentifier, not both")
        }
        (None, None) => anyhow::bail!("a rule selector is required: --table <name> or --ridentifier <id>"),
    };

    let target = SshTarget {
        host: cmd.host.clone(),
        port: cmd.port,
        user: cmd.user.clone(),
        key_path: cmd.key.clone(),
        timeout_seconds: cmd.timeout_seconds,
    };
    // NTP sync state is not observable from here; record it honestly.
    let clock = ClockBasis {
        source: "system_wall".to_string(),
        ntp_status: "unknown".to_string(),
    };
    let now = ::time::OffsetDateTime::now_utc();

    let receipt = live_declared_deny(
        &target,
        &cmd.vantage,
        &selector,
        &cmd.control,
        cmd.subject.as_deref(),
        &clock,
        now,
    )?;

    // Receipt-only emit. No DB write (active-witness lane).
    println!("{}", serde_json::to_string_pretty(&receipt)?);

    if let Some(out_dir) = &cmd.out_dir {
        let path = persist_receipt(out_dir, &receipt)?;
        eprintln!("appended receipt to series: {}", path.display());
    }
    Ok(())
}

/// `nq-monitor probe gateway-path` — live gateway-report-vs-path read.
/// Read-only over SSH (`ls` + read the `dpinger` status socket(s)), plus
/// independent path probes from the named vantage to the dpinger monitor IP
/// and a fixed public anchor. Receipt-only; no DB write. The verdict is a
/// non-lift: a disagreement is path ambiguity, never a down WAN, and a
/// missing/mute socket is `cannot_testify`, not gateway-down.
fn run_gateway_path(cmd: ProbeGatewayPathCmd) -> anyhow::Result<()> {
    use crate::gateway_path_probe::ClockBasis;
    use crate::gateway_path_transport::{live_gateway_path, persist_receipt, SshTarget};

    let target = SshTarget {
        host: cmd.host.clone(),
        port: cmd.port,
        user: cmd.user.clone(),
        key_path: cmd.key.clone(),
        timeout_seconds: cmd.timeout_seconds,
    };
    // NTP sync state is not observable from here; record it honestly.
    let clock = ClockBasis {
        source: "system_wall".to_string(),
        ntp_status: "unknown".to_string(),
    };
    let now = ::time::OffsetDateTime::now_utc();

    let receipt = live_gateway_path(
        &target,
        &cmd.vantage,
        cmd.gateway.as_deref(),
        &cmd.anchor,
        cmd.tcp_fallback_port,
        &clock,
        now,
    )?;

    // Receipt-only emit. No DB write (active-witness lane).
    println!("{}", serde_json::to_string_pretty(&receipt)?);

    if let Some(out_dir) = &cmd.out_dir {
        let path = persist_receipt(out_dir, &receipt)?;
        eprintln!("appended receipt to series: {}", path.display());
    }

    // Packet #7c: optionally fold an external-arrival position (the #7b beacon
    // status) into a combined report. Additive — the LAN-side receipt above is
    // unchanged. An unparseable/unknown beacon verdict yields `None` (honest
    // absence), which combines to `cannot_classify` rather than a fabricated
    // position.
    if let Some(status_path) = &cmd.external_beacon_status {
        use crate::gateway_path_probe::{
            combine_gateway_path_with_external, external_basis_from_beacon_status,
        };
        let raw = std::fs::read_to_string(status_path)
            .with_context(|| format!("reading external beacon status {:?}", status_path))?;
        let external = external_basis_from_beacon_status(&raw);
        let combined = combine_gateway_path_with_external(&receipt, external);
        println!("{}", serde_json::to_string_pretty(&combined)?);
    }
    Ok(())
}
