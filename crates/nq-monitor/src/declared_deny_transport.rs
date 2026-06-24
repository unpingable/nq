//! Live read for the declared-deny specimen (the gold check #1).
//!
//! The verdict core (`declared_deny_probe`) is pure; this module fills its
//! inputs from reality:
//!
//!   1. one read-only SSH call to pfSense: `pfctl -sr -vv` (the loaded
//!      ruleset with per-rule counters) — the declared-policy surface;
//!   2. pure parsers find the declared `block` rule matching the requested
//!      table/identifier and read its counters into a [`DeclaredDenyRule`]
//!      with explicit custody;
//!   3. a CONTROL probe from THIS host to a known-allowed target (proving the
//!      vantage has ordinary egress);
//!   4. a SUBJECT probe is DELIBERATELY NOT RUN by default — the declared-
//!      denied path is left unbound unless an explicit benign/operator-owned
//!      target is supplied (active subject probing toward a real denied
//!      destination is its own perturbation packet, parked for a scratch/lab
//!      firewall — we do not SYN a malware-blocklist member to make the
//!      specimen spicy);
//!   5. [`evaluate_declared_deny`] decides the custody verdict.
//!
//! Read-only and non-mutating by construction: the only remote command is
//! `pfctl -sr -vv`. No `pfctl -f`/`-d`, no rule edit, no service control, no
//! config write. Reading the ruleset is the box's own status surface.

use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context};
use time::OffsetDateTime;

use crate::declared_deny_probe::{
    evaluate_declared_deny, ClockBasis, DeclaredDenyReceipt, DeclaredDenyRule, DenyRole,
    PathObservation, PolicyCustody, ProbeMethod, ProbeOutcome,
};
pub use crate::lease_presence_transport::SshTarget;

/// How to select the declared-deny rule out of the loaded ruleset.
#[derive(Debug, Clone)]
pub enum RuleSelector {
    /// Match the rule whose destination is this table name (e.g. `pfB_PRI1_v4`).
    Table(String),
    /// Match the rule with this `ridentifier`.
    Ridentifier(String),
}

// ─────────────────────── pure parsers (tested) ───────────────────────

/// A `block` rule parsed out of one `pfctl -sr -vv` rule line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRule {
    pub action: String,
    pub quick: bool,
    pub direction: String,
    pub interface: String,
    pub source_spec: String,
    pub dest_spec: String,
    pub dest_table: Option<String>,
    pub table_entry_count: Option<u64>,
    pub ridentifier: Option<String>,
    pub label: Option<String>,
}

/// Parse one `@N block ...` rule line. Returns `None` if it is not a `block`
/// rule. Tolerant of field order for the trailing `label`/`ridentifier`
/// tokens; positional for the `block <action> <dir> ... on <if> ... from <src>
/// to <dst>` head.
pub fn parse_rule_line(line: &str) -> Option<ParsedRule> {
    // Strip an optional leading "@<n> " index from -vv output.
    let line = line.trim();
    let body = match line.split_once(' ') {
        Some((head, rest)) if head.starts_with('@') => rest,
        _ => line,
    };
    let toks: Vec<&str> = body.split_whitespace().collect();
    if toks.first() != Some(&"block") {
        return None;
    }
    // action: "block" plus "return"/"drop" if present.
    let action = match toks.get(1) {
        Some(&"return") | Some(&"drop") => format!("block {}", toks[1]),
        _ => "block".to_string(),
    };
    let direction = if toks.contains(&"out") {
        "out".to_string()
    } else {
        "in".to_string()
    };
    let quick = toks.contains(&"quick");
    let interface = after_token(&toks, "on").unwrap_or_default();

    // source/dest specs: the spans after "from" and "to" (single token each
    // here — pfSense emits `from any to <table>`; richer specs degrade to the
    // first token, which is enough for the receipt + table extraction).
    let source_spec = after_token(&toks, "from").unwrap_or_default();
    let dest_spec_raw = after_token(&toks, "to").unwrap_or_default();
    let (dest_spec, dest_table, table_entry_count) = parse_dest(&dest_spec_raw);

    let ridentifier = after_token(&toks, "ridentifier");
    let label = parse_label(body);

    Some(ParsedRule {
        action,
        quick,
        direction,
        interface,
        source_spec,
        dest_spec,
        dest_table,
        table_entry_count,
        ridentifier,
        label,
    })
}

/// The token immediately following `key`, if present.
fn after_token(toks: &[&str], key: &str) -> Option<String> {
    toks.iter()
        .position(|t| *t == key)
        .and_then(|i| toks.get(i + 1))
        .map(|s| s.to_string())
}

/// Parse a destination spec. `<name:count>` -> (`<name>`, name, count);
/// `<name>` -> (`<name>`, name, None); anything else -> (raw, None, None).
fn parse_dest(raw: &str) -> (String, Option<String>, Option<u64>) {
    if let Some(inner) = raw.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
        match inner.split_once(':') {
            Some((name, count)) => (
                format!("<{name}>"),
                Some(name.to_string()),
                count.parse().ok(),
            ),
            None => (format!("<{inner}>"), Some(inner.to_string()), None),
        }
    } else {
        (raw.to_string(), None, None)
    }
}

/// The first human `label "..."` that is not an `id:...` label.
fn parse_label(body: &str) -> Option<String> {
    let mut rest = body;
    while let Some(i) = rest.find("label \"") {
        let after = &rest[i + 7..];
        if let Some(end) = after.find('"') {
            let val = &after[..end];
            if !val.starts_with("id:") {
                return Some(val.to_string());
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    None
}

/// Parse a `[ Evaluations: N   Packets: N   Bytes: N   States: N ]` counter
/// line into (evaluations, blocked_packets, states).
pub fn parse_counter_line(line: &str) -> (Option<u64>, Option<u64>, Option<u64>) {
    fn field(line: &str, key: &str) -> Option<u64> {
        let i = line.find(key)?;
        line[i + key.len()..]
            .split_whitespace()
            .next()?
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()
    }
    (
        field(line, "Evaluations:"),
        field(line, "Packets:"),
        field(line, "States:"),
    )
}

/// Find the declared-deny rule in a `pfctl -sr -vv` dump and reconcile it into
/// a [`DeclaredDenyRule`] with custody. Empty/unreadable dump ->
/// `UnknownSurface`; no matching block rule -> `Absent`.
pub fn find_declared_deny(
    vv_text: &str,
    selector: &RuleSelector,
    ssh_host: &str,
) -> DeclaredDenyRule {
    let source = format!("ssh:{ssh_host} pfctl -sr -vv");
    let unknown = DeclaredDenyRule {
        label: String::new(),
        ridentifier: String::new(),
        interface: String::new(),
        direction: String::new(),
        action: String::new(),
        quick: false,
        source_spec: String::new(),
        dest_spec: String::new(),
        dest_table: None,
        table_entry_count: None,
        evaluations: None,
        blocked_packets: None,
        states: None,
        custody: PolicyCustody::UnknownSurface,
        source: source.clone(),
    };
    if vv_text.trim().is_empty() {
        return unknown;
    }

    // Walk lines; a rule line (`block ...`) is followed by indented counter
    // lines until the next rule. Match the first block rule that satisfies the
    // selector, then read its counter line.
    let lines: Vec<&str> = vv_text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let Some(rule) = parse_rule_line(line) else { continue };
        let matches = match selector {
            RuleSelector::Table(t) => rule.dest_table.as_deref() == Some(t.as_str()),
            RuleSelector::Ridentifier(r) => rule.ridentifier.as_deref() == Some(r.as_str()),
        };
        if !matches {
            continue;
        }
        // Counters: the next line containing "Evaluations:".
        let (evaluations, blocked_packets, states) = lines
            .get(i + 1..)
            .and_then(|tail| tail.iter().take(3).find(|l| l.contains("Evaluations:")))
            .map(|l| parse_counter_line(l))
            .unwrap_or((None, None, None));

        return DeclaredDenyRule {
            label: rule.label.unwrap_or_default(),
            ridentifier: rule.ridentifier.unwrap_or_default(),
            interface: rule.interface,
            direction: rule.direction,
            action: rule.action,
            quick: rule.quick,
            source_spec: rule.source_spec,
            dest_spec: rule.dest_spec,
            dest_table: rule.dest_table,
            table_entry_count: rule.table_entry_count,
            evaluations,
            blocked_packets,
            states,
            custody: PolicyCustody::Present,
            source,
        };
    }

    DeclaredDenyRule {
        custody: PolicyCustody::Absent,
        ..unknown
    }
}

// ─────────────────────────── ssh read (live) ───────────────────────────

const SECTION_RULES: &str = "===NQ_RULES_VV===";
const SECTION_END: &str = "===NQ_END===";

/// Read-only: dump the loaded ruleset with counters. One login, one command.
pub fn ssh_read_policy(target: &SshTarget) -> anyhow::Result<String> {
    let dest = format!("{}@{}", target.user, target.host);
    let script = format!("echo {SECTION_RULES}; pfctl -sr -vv 2>/dev/null; echo {SECTION_END}");
    let out = Command::new("ssh")
        .args([
            "-i",
            &target.key_path.to_string_lossy(),
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "BatchMode=yes",
            "-o",
            &format!("ConnectTimeout={}", target.timeout_seconds),
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-p",
            &target.port.to_string(),
            &dest,
            &script,
        ])
        .output()
        .context("spawn ssh")?;
    if !out.status.success() {
        return Err(anyhow!(
            "ssh policy read failed (status {:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let raw = String::from_utf8_lossy(&out.stdout).into_owned();
    Ok(between(&raw, SECTION_RULES, SECTION_END).trim().to_string())
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> &'a str {
    let Some(a) = s.find(start) else { return "" };
    let after = &s[a + start.len()..];
    match after.find(end) {
        Some(b) => &after[..b],
        None => after,
    }
}

// ───────────────────── probes (this vantage) ─────────────────────

/// Split a `host:port` target. Defaults the port to 443 if absent.
fn split_hostport(target: &str) -> (String, u16) {
    match target.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(443)),
        None => (target.to_string(), 443),
    }
}

/// CONTROL probe: any answer (connect OR refusal) proves the vantage reached
/// the host — egress works.
pub fn control_probe(target: &str, timeout: Duration) -> ProbeOutcome {
    let (host, port) = split_hostport(target);
    match tcp_answer(&host, port, timeout) {
        TcpAnswer::Connected | TcpAnswer::Refused => ProbeOutcome::Reached,
        TcpAnswer::Silent => ProbeOutcome::NotReached,
        TcpAnswer::Unresolved => ProbeOutcome::NotAttempted,
    }
}

/// SUBJECT probe: ONLY a completed handshake counts as a got-through. A
/// refusal is NOT a got-through (a `block return` RST looks identical at the
/// socket), so it is `NotReached`. (Not invoked unless a subject is bound.)
pub fn subject_probe(target: &str, timeout: Duration) -> ProbeOutcome {
    let (host, port) = split_hostport(target);
    match tcp_answer(&host, port, timeout) {
        TcpAnswer::Connected => ProbeOutcome::Reached,
        TcpAnswer::Refused | TcpAnswer::Silent => ProbeOutcome::NotReached,
        TcpAnswer::Unresolved => ProbeOutcome::NotAttempted,
    }
}

enum TcpAnswer {
    Connected,
    Refused,
    Silent,
    Unresolved,
}

fn tcp_answer(host: &str, port: u16, timeout: Duration) -> TcpAnswer {
    let addr = format!("{host}:{port}");
    match addr.to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(sa) => match TcpStream::connect_timeout(&sa, timeout) {
            Ok(_) => TcpAnswer::Connected,
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => TcpAnswer::Refused,
            Err(_) => TcpAnswer::Silent,
        },
        None => TcpAnswer::Unresolved,
    }
}

// ─────────────────────────── orchestration ───────────────────────────

/// Full live read. Reads the declared policy; runs a control probe; leaves the
/// subject unbound unless `subject_target` is supplied. Receipt-only.
#[allow(clippy::too_many_arguments)]
pub fn live_declared_deny(
    target: &SshTarget,
    vantage: &str,
    selector: &RuleSelector,
    control_target: &str,
    subject_target: Option<&str>,
    clock: &ClockBasis,
    now: OffsetDateTime,
) -> anyhow::Result<DeclaredDenyReceipt> {
    let vv = ssh_read_policy(target)?;
    let rule = find_declared_deny(&vv, selector, &target.host);

    let probe_timeout = Duration::from_secs(target.timeout_seconds);
    let mut observations = Vec::new();

    // Control: a known-allowed target, proving ordinary egress.
    let control_outcome = control_probe(control_target, probe_timeout);
    observations.push(PathObservation::new(
        ProbeMethod::TcpConnect,
        DenyRole::Control,
        vantage,
        control_target,
        control_outcome,
        now,
    ));

    // Subject: only if an explicit (benign/operator-owned) target is bound.
    match subject_target {
        Some(t) => {
            let outcome = subject_probe(t, probe_timeout);
            observations.push(PathObservation::new(
                ProbeMethod::TcpConnect,
                DenyRole::Subject,
                vantage,
                t,
                outcome,
                now,
            ));
        }
        None => observations.push(PathObservation::subject_unbound(vantage, now)),
    }

    Ok(evaluate_declared_deny(&rule, &observations, clock, now))
}

// ─────────────────────── append-only receipt sink ───────────────────────

/// Append a receipt under `base/<YYYYMMDDTHHMMSSZ>/<rule>.json`. Append-only.
pub fn persist_receipt(base: &Path, receipt: &DeclaredDenyReceipt) -> anyhow::Result<PathBuf> {
    let stamp = run_stamp(&receipt.probe_time);
    let dir = base.join(stamp);
    std::fs::create_dir_all(&dir).with_context(|| format!("create series dir {}", dir.display()))?;
    let slug = host_slug(
        receipt
            .rule
            .dest_table
            .as_deref()
            .unwrap_or(&receipt.rule.ridentifier),
    );
    let name = if slug.is_empty() { "declared_deny".to_string() } else { slug };
    let path = dir.join(format!("{name}.json"));
    if path.exists() {
        return Err(anyhow!(
            "refusing to overwrite existing receipt {} — the series is append-only",
            path.display()
        ));
    }
    let json = serde_json::to_string_pretty(receipt).context("serialize receipt")?;
    std::fs::write(&path, format!("{json}\n")).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

fn run_stamp(probe_time: &str) -> String {
    let base = probe_time.split('.').next().unwrap_or(probe_time).trim_end_matches('Z');
    let cleaned: String = base.chars().filter(|c| *c != '-' && *c != ':').collect();
    format!("{cleaned}Z")
}

fn host_slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::format_description::well_known::Rfc3339;

    fn at(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).expect("rfc3339")
    }

    // The real captured rule (2026-06-24), with its -vv counter line. The
    // shape is real; the table name is the real pfBlocker table (not private
    // host data — it is a table NAME, no member IPs).
    const RULE: &str = "@47 block return in log quick on igc1 inet from any to <pfB_PRI1_v4:17007> label \"USER_RULE: pfB_PRI1_v4\" label \"id:1770008176\" ridentifier 1770008176";
    const COUNTERS: &str = "  [ Evaluations: 3186099   Packets: 8         Bytes: 512         States: 0     ]";

    #[test]
    fn rule_line_parses_block_return_quick() {
        let r = parse_rule_line(RULE).expect("parse");
        assert_eq!(r.action, "block return");
        assert!(r.quick);
        assert_eq!(r.direction, "in");
        assert_eq!(r.interface, "igc1");
        assert_eq!(r.source_spec, "any");
        assert_eq!(r.dest_spec, "<pfB_PRI1_v4>");
        assert_eq!(r.dest_table.as_deref(), Some("pfB_PRI1_v4"));
        assert_eq!(r.table_entry_count, Some(17007));
        assert_eq!(r.ridentifier.as_deref(), Some("1770008176"));
        assert_eq!(r.label.as_deref(), Some("USER_RULE: pfB_PRI1_v4")); // not the id: label
    }

    #[test]
    fn non_block_rule_is_none() {
        assert!(parse_rule_line("@1 pass in quick on igc1 inet from any to any").is_none());
        assert!(parse_rule_line("  [ Evaluations: 1 ]").is_none());
    }

    #[test]
    fn block_drop_action_parsed() {
        let r = parse_rule_line("block drop in log inet all label \"Default deny rule IPv4\"")
            .expect("parse");
        assert_eq!(r.action, "block drop");
        assert!(!r.quick);
        // No "from"/"to" tokens in this default-deny line -> specs empty, no table.
        assert_eq!(r.source_spec, "");
        assert_eq!(r.dest_spec, "");
        assert_eq!(r.dest_table, None);
    }

    #[test]
    fn counter_line_parses_fields() {
        let (e, p, s) = parse_counter_line(COUNTERS);
        assert_eq!(e, Some(3186099));
        assert_eq!(p, Some(8));
        assert_eq!(s, Some(0));
    }

    #[test]
    fn find_by_table_reconciles_present_with_counters() {
        let vv = format!("{RULE}\n{COUNTERS}\n  [ Inserted: uid 0 pid 0 ]\n");
        let rule = find_declared_deny(&vv, &RuleSelector::Table("pfB_PRI1_v4".to_string()), "pf.example");
        assert_eq!(rule.custody, PolicyCustody::Present);
        assert_eq!(rule.action, "block return");
        assert_eq!(rule.table_entry_count, Some(17007));
        assert_eq!(rule.evaluations, Some(3186099));
        assert_eq!(rule.blocked_packets, Some(8));
        assert_eq!(rule.states, Some(0));
        assert!(rule.source.contains("pfctl -sr -vv"));
    }

    #[test]
    fn find_by_ridentifier_also_matches() {
        let vv = format!("{RULE}\n{COUNTERS}\n");
        let rule = find_declared_deny(
            &vv,
            &RuleSelector::Ridentifier("1770008176".to_string()),
            "pf.example",
        );
        assert_eq!(rule.custody, PolicyCustody::Present);
    }

    #[test]
    fn missing_rule_is_absent_not_unknown() {
        let vv = format!("{RULE}\n{COUNTERS}\n");
        let rule = find_declared_deny(&vv, &RuleSelector::Table("does_not_exist".to_string()), "pf.example");
        assert_eq!(rule.custody, PolicyCustody::Absent);
    }

    #[test]
    fn empty_dump_is_unknown_surface() {
        let rule = find_declared_deny("   ", &RuleSelector::Table("x".to_string()), "pf.example");
        assert_eq!(rule.custody, PolicyCustody::UnknownSurface);
    }

    #[test]
    fn run_stamp_and_slug_are_filesystem_safe() {
        assert_eq!(run_stamp("2026-06-24T17:00:01.5Z"), "20260624T170001Z");
        assert_eq!(host_slug("pfB_PRI1_v4"), "pfB_PRI1_v4");
        assert_eq!(host_slug("a/b:c"), "a_b_c");
    }

    #[test]
    fn split_hostport_defaults_443() {
        assert_eq!(split_hostport("1.1.1.1:443"), ("1.1.1.1".to_string(), 443));
        assert_eq!(split_hostport("1.1.1.1"), ("1.1.1.1".to_string(), 443));
        assert_eq!(split_hostport("1.1.1.1:80"), ("1.1.1.1".to_string(), 80));
    }

    #[test]
    fn persist_refuses_overwrite() {
        let vv = format!("{RULE}\n{COUNTERS}\n");
        let rule = find_declared_deny(&vv, &RuleSelector::Table("pfB_PRI1_v4".to_string()), "pf.example");
        let clock = ClockBasis {
            source: "system_wall".to_string(),
            ntp_status: "unknown".to_string(),
        };
        let obs = vec![PathObservation::subject_unbound("sushi-k-lan", at("2026-06-24T17:00:00Z"))];
        let receipt = evaluate_declared_deny(&rule, &obs, &clock, at("2026-06-24T17:00:01Z"));
        let tmp = std::env::temp_dir().join(format!("nq-deny-test-{}", std::process::id()));
        let p1 = persist_receipt(&tmp, &receipt).expect("first write");
        assert!(p1.exists());
        assert!(persist_receipt(&tmp, &receipt).is_err(), "append-only");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
