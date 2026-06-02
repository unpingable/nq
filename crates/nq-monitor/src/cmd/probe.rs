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

use crate::cli::{ProbeAction, ProbeCmd, ProbeDnsCmd};
use crate::probe::{
    parse_qtype, parse_resolver, read_latest_generation_id, record_probe, UdpDnsClient,
};
use anyhow::Context;
use nq_db::open_rw;
use std::time::Duration;

pub fn run(cmd: ProbeCmd) -> anyhow::Result<()> {
    match cmd.action {
        ProbeAction::Dns(c) => run_dns(c),
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
