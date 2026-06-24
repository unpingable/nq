//! Live read for the gateway-report-vs-path specimen (Phase 2).
//!
//! The verdict core (`gateway_path_probe`) is pure; this module fills its
//! inputs from reality, the way the lease-presence transport fed its core:
//!
//!   1. one read-only SSH call to pfSense: enumerate `/var/run/dpinger_*.sock`
//!      and read each daemon socket's raw status line (`<name> <rtt_us>
//!      <stddev_us> <loss_pct>`) via `nc -U`;
//!   2. pure parsers turn the socket FILENAME into the gateway identity
//!      (name / source IP / monitor IP) and the status line into raw metrics,
//!      yielding a [`DpingerReport`] with explicit custody;
//!   3. independent path probes from THIS host — the named vantage — to the
//!      dpinger monitor IP (role `MonitorTarget`) and a fixed public anchor
//!      (role `EgressAnchor`), each contributing `ObservedReachability`;
//!   4. [`evaluate_gateway_path`] decides the non-lift verdict.
//!
//! Read-only and non-mutating by construction: the only remote work is an
//! `ls` and reading the dpinger status sockets (the same thing pfSense's own
//! status page does). No `pfctl`, no service control, no gateway reload, no
//! config write, and — per operator custody direction — NO pfSense PHP
//! classification: the raw daemon socket is the first-class witness.
//!
//! Source typing travels with every datum: the dpinger metrics are a pfSense
//! self-report; only the local probes are observed reachability, and only
//! from this vantage at this time. dpinger reaching its monitor is never "the
//! internet is up"; a disagreement is never "WAN down."

use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, Context};
use time::OffsetDateTime;

use crate::gateway_path_probe::{
    evaluate_gateway_path, ClockBasis, DpingerCustody, DpingerReport, GatewayPathReceipt,
    PathMethod, PathObservation, PathOutcome, PathRole,
};
// The SSH target shape is identical across pfSense reads; reuse it rather than
// minting a second one (a shared probe-transport home is deferred until a
// third SSH consumer — name early, ratify lazily).
pub use crate::lease_presence_transport::SshTarget;

// ─────────────────────── pure parsers (tested) ───────────────────────

/// The identity dpinger encodes in a socket FILENAME:
/// `dpinger_<NAME>~<SOURCE_IP>~<MONITOR_IP>.sock`. The gateway name may itself
/// contain underscores (e.g. `WAN_DHCP`); the `~` separators are unambiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketIdentity {
    pub gateway_name: String,
    pub source_ip: Option<String>,
    pub monitor_ip: Option<String>,
    /// The full socket path, kept for the read and the receipt `source`.
    pub socket_path: String,
}

/// Parse a dpinger socket path/filename into its identity. Returns `None` if
/// it does not match the `dpinger_<name>~<src>~<mon>.sock` shape.
pub fn parse_socket_identity(path: &str) -> Option<SocketIdentity> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.strip_prefix("dpinger_")?.strip_suffix(".sock")?;
    // Split on '~' into name, source, monitor. dpinger emits exactly these
    // three fields; if a future shape adds more we keep name + refuse to
    // invent src/mon (custody honesty).
    let parts: Vec<&str> = stem.split('~').collect();
    let (name, source_ip, monitor_ip) = match parts.as_slice() {
        [name, src, mon] => (
            name.to_string(),
            non_empty(src),
            non_empty(mon),
        ),
        // Unexpected field count: keep whatever precedes the first '~' as the
        // name, leave addresses unknown rather than guess.
        _ => (
            stem.split('~').next().unwrap_or(stem).to_string(),
            None,
            None,
        ),
    };
    if name.is_empty() {
        return None;
    }
    Some(SocketIdentity {
        gateway_name: name,
        source_ip,
        monitor_ip,
        socket_path: path.to_string(),
    })
}

/// dpinger's raw status line: `<name> <rtt_us> <stddev_us> <loss_pct>`. The
/// numbers are whitespace-separated; loss is an integer percent in practice
/// but parsed as f64 to be tolerant. Returns `None` if it does not parse —
/// the caller turns that into `UnknownCustody`, never a fabricated metric.
#[derive(Debug, Clone, PartialEq)]
pub struct DpingerMetrics {
    pub gateway_name: String,
    pub rtt_us: u64,
    pub stddev_us: u64,
    pub loss_pct: f64,
}

pub fn parse_dpinger_status(line: &str) -> Option<DpingerMetrics> {
    let mut it = line.split_whitespace();
    let name = it.next()?.to_string();
    let rtt_us: u64 = it.next()?.parse().ok()?;
    let stddev_us: u64 = it.next()?.parse().ok()?;
    let loss_pct: f64 = it.next()?.parse().ok()?;
    if name.is_empty() {
        return None;
    }
    Some(DpingerMetrics {
        gateway_name: name,
        rtt_us,
        stddev_us,
        loss_pct,
    })
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Build the [`DpingerReport`] for one gateway by reconciling the socket
/// identity (from the filename) against the metrics line (from the socket
/// content). Custody is explicit: present+parsed = `MetricsPresent`; a present
/// socket with no/empty content = `SocketUnreadable`; an unparseable line or a
/// name that disagrees with the filename = `UnknownCustody`. (`SocketAbsent`
/// is decided one level up, when no socket matches the requested gateway.)
pub fn reconcile_report(
    identity: &SocketIdentity,
    raw_status: Option<&str>,
    ssh_host: &str,
) -> DpingerReport {
    let source = format!("ssh:{} dpinger:{}", ssh_host, identity.socket_path);
    let base = DpingerReport {
        gateway_name: identity.gateway_name.clone(),
        monitor_ip: identity.monitor_ip.clone(),
        source_ip: identity.source_ip.clone(),
        custody: DpingerCustody::SocketUnreadable,
        rtt_us: None,
        stddev_us: None,
        loss_pct: None,
        source,
    };
    let Some(raw) = raw_status.map(str::trim).filter(|s| !s.is_empty()) else {
        return base; // present but mute
    };
    match parse_dpinger_status(raw) {
        None => DpingerReport {
            custody: DpingerCustody::UnknownCustody,
            ..base
        },
        Some(m) if m.gateway_name != identity.gateway_name => DpingerReport {
            // The daemon named a different gateway than the socket filename —
            // custody is in doubt; record metrics but refuse to certify.
            custody: DpingerCustody::UnknownCustody,
            ..base
        },
        Some(m) => DpingerReport {
            custody: DpingerCustody::MetricsPresent,
            rtt_us: Some(m.rtt_us),
            stddev_us: Some(m.stddev_us),
            loss_pct: Some(m.loss_pct),
            ..base
        },
    }
}

// ─────────────────────────── ssh read (live) ───────────────────────────

const SECTION_SOCKS: &str = "===NQ_SOCKS===";
const SECTION_STATUS: &str = "===NQ_STATUS===";
const SECTION_END: &str = "===NQ_END===";
const SOCK_MARK: &str = "--NQ_SOCK ";

/// The single read-only remote script: list the dpinger sockets, then read
/// each one's status line via `nc -U`. One login. Strictly read-only — `ls`
/// and reading status sockets, nothing else.
fn remote_read_script() -> String {
    format!(
        "echo {s}; ls -1 /var/run/dpinger_*.sock 2>/dev/null; echo {st}; \
         for sock in /var/run/dpinger_*.sock; do [ -S \"$sock\" ] || continue; \
         echo '{mark}'\"$sock\"; echo | nc -U -w 2 \"$sock\" 2>/dev/null | head -c 200; echo; \
         done; echo {e}",
        s = SECTION_SOCKS,
        st = SECTION_STATUS,
        mark = SOCK_MARK.trim_end(),
        e = SECTION_END,
    )
}

/// Run the read-only gather over SSH and return raw stdout.
pub fn ssh_gather(target: &SshTarget) -> anyhow::Result<String> {
    let dest = format!("{}@{}", target.user, target.host);
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
            &remote_read_script(),
        ])
        .output()
        .context("spawn ssh")?;
    if !out.status.success() {
        return Err(anyhow!(
            "ssh read failed (status {:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Split the gather into the socket list and a per-socket status map (keyed by
/// socket path). Pure — fixture-testable.
pub fn split_gather(gather: &str) -> (Vec<String>, Vec<(String, String)>) {
    fn between<'a>(s: &'a str, start: &str, end: &str) -> &'a str {
        let Some(a) = s.find(start) else { return "" };
        let after = &s[a + start.len()..];
        match after.find(end) {
            Some(b) => &after[..b],
            None => after,
        }
    }
    let socks_text = between(gather, SECTION_SOCKS, SECTION_STATUS);
    let socket_paths: Vec<String> = socks_text
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("/var/run/dpinger_") && l.ends_with(".sock"))
        .map(|l| l.to_string())
        .collect();

    let status_text = between(gather, SECTION_STATUS, SECTION_END);
    let mut status: Vec<(String, String)> = Vec::new();
    let mut current: Option<String> = None;
    let mut buf = String::new();
    for line in status_text.lines() {
        if let Some(rest) = line.trim().strip_prefix(SOCK_MARK.trim()) {
            if let Some(path) = current.take() {
                status.push((path, buf.trim().to_string()));
                buf.clear();
            }
            current = Some(rest.trim().to_string());
        } else if current.is_some() {
            buf.push_str(line.trim());
            buf.push('\n');
        }
    }
    if let Some(path) = current.take() {
        status.push((path, buf.trim().to_string()));
    }
    (socket_paths, status)
}

// ───────────────────── path probe (this vantage) ─────────────────────

/// Run an independent path probe to `ip` from THIS host. ICMP first (lowest
/// perturbation); fall back to a TCP connect ONLY if ICMP cannot testify
/// (could not execute) — operator-directed. Returns the outcome and the method
/// that produced it.
pub fn path_probe(ip: &str, tcp_fallback_port: u16, timeout: Duration) -> (PathOutcome, PathMethod) {
    let icmp = icmp_probe(ip, timeout);
    if icmp != PathOutcome::NotAttempted {
        return (icmp, PathMethod::IcmpEcho);
    }
    (tcp_probe(ip, tcp_fallback_port, timeout), PathMethod::TcpConnect)
}

fn icmp_probe(ip: &str, timeout: Duration) -> PathOutcome {
    let secs = timeout.as_secs().max(1).to_string();
    match Command::new("ping")
        .args(["-c", "1", "-W", &secs, ip])
        .output()
    {
        Ok(o) if o.status.success() => PathOutcome::Reached,
        Ok(_) => PathOutcome::NotReached,
        Err(_) => PathOutcome::NotAttempted, // ping unavailable -> cannot testify
    }
}

fn tcp_probe(ip: &str, port: u16, timeout: Duration) -> PathOutcome {
    let addr = format!("{ip}:{port}");
    match addr.to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(sa) => match TcpStream::connect_timeout(&sa, timeout) {
            // A completed connect OR a refusal both prove the path reached the
            // host (the host answered with SYN-ACK or RST).
            Ok(_) => PathOutcome::Reached,
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => PathOutcome::Reached,
            Err(_) => PathOutcome::NotReached,
        },
        None => PathOutcome::NotAttempted,
    }
}

// ─────────────────────────── orchestration ───────────────────────────

/// Full live read for one gateway. One SSH login to read the dpinger
/// socket(s); then independent path probes from this vantage to the monitor IP
/// and the egress anchor.
///
/// `gateway` optionally names which gateway to read; if `None` and exactly one
/// dpinger socket exists, that one is used. The caller supplies the egress
/// `anchor` (the CLI defaults it to `1.1.1.1`).
#[allow(clippy::too_many_arguments)]
pub fn live_gateway_path(
    target: &SshTarget,
    vantage: &str,
    gateway: Option<&str>,
    anchor: &str,
    tcp_fallback_port: u16,
    clock: &ClockBasis,
    now: OffsetDateTime,
) -> anyhow::Result<GatewayPathReceipt> {
    let gather = ssh_gather(target)?;
    let (socket_paths, status_map) = split_gather(&gather);

    // Resolve which socket/gateway we are reporting on.
    let identities: Vec<SocketIdentity> =
        socket_paths.iter().filter_map(|p| parse_socket_identity(p)).collect();

    let chosen = match gateway {
        Some(name) => identities.iter().find(|i| i.gateway_name == name).cloned(),
        None => match identities.as_slice() {
            [only] => Some(only.clone()),
            [] => None,
            _ => {
                let names: Vec<&str> = identities.iter().map(|i| i.gateway_name.as_str()).collect();
                return Err(anyhow!(
                    "multiple dpinger gateways present ({}); pass --gateway to choose one",
                    names.join(", ")
                ));
            }
        },
    };

    let dpinger = match chosen {
        Some(identity) => {
            let raw = status_map
                .iter()
                .find(|(path, _)| *path == identity.socket_path)
                .map(|(_, s)| s.as_str());
            reconcile_report(&identity, raw, &target.host)
        }
        None => {
            // No socket matched -> the witness is absent. We still emit a
            // receipt (cannot_testify), naming the gateway we looked for.
            DpingerReport {
                gateway_name: gateway.unwrap_or("(unknown)").to_string(),
                monitor_ip: None,
                source_ip: None,
                custody: DpingerCustody::SocketAbsent,
                rtt_us: None,
                stddev_us: None,
                loss_pct: None,
                source: format!("ssh:{} dpinger:(no socket)", target.host),
            }
        }
    };

    // Independent path probes. The monitor probe only runs when the report
    // gave us a monitor IP to aim at (custody honesty — we do not invent one).
    let probe_timeout = Duration::from_secs(target.timeout_seconds);
    let mut observations: Vec<PathObservation> = Vec::new();

    if let Some(monitor_ip) = dpinger.monitor_ip.clone() {
        let (outcome, method) = path_probe(&monitor_ip, tcp_fallback_port, probe_timeout);
        observations.push(PathObservation::new(
            method,
            PathRole::MonitorTarget,
            vantage,
            monitor_ip,
            outcome,
            now,
        ));
    }
    {
        let (outcome, method) = path_probe(anchor, tcp_fallback_port, probe_timeout);
        observations.push(PathObservation::new(
            method,
            PathRole::EgressAnchor,
            vantage,
            anchor,
            outcome,
            now,
        ));
    }

    Ok(evaluate_gateway_path(&dpinger, &observations, clock, now))
}

// ─────────────────────── append-only receipt sink ───────────────────────

/// Append a receipt under `base/<YYYYMMDDTHHMMSSZ>/<gateway>.json`.
/// Append-only: refuses to overwrite. Mirrors the lease-presence / TLS series
/// discipline; a shared sink is deferred until the shared-probe home lands.
pub fn persist_receipt(base: &Path, receipt: &GatewayPathReceipt) -> anyhow::Result<PathBuf> {
    let stamp = run_stamp(&receipt.probe_time);
    let dir = base.join(stamp);
    std::fs::create_dir_all(&dir).with_context(|| format!("create series dir {}", dir.display()))?;
    let path = dir.join(format!("{}.json", host_slug(&receipt.dpinger.gateway_name)));
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
        .map(|c| if c == ':' || c == '/' || c == '\\' || c == '~' { '_' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::format_description::well_known::Rfc3339;

    fn at(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).expect("rfc3339")
    }

    // Modeled on the real captured socket shape (2026-06-24):
    //   /var/run/dpinger_WAN_DHCP~<wan-src-ip>~<monitor-ip>.sock
    //   "WAN_DHCP <rtt_us> <stddev_us> <loss_pct>"
    // Fixtures below use TEST-NET (198.51.100/24) addresses; the real
    // WAN/monitor IPs live only in gitignored runs/.
    const SOCK: &str = "/var/run/dpinger_WAN_DHCP~198.51.100.129~198.51.100.1.sock";

    #[test]
    fn socket_identity_parses_name_with_underscore_and_addrs() {
        let id = parse_socket_identity(SOCK).expect("parse");
        assert_eq!(id.gateway_name, "WAN_DHCP");
        assert_eq!(id.source_ip.as_deref(), Some("198.51.100.129"));
        assert_eq!(id.monitor_ip.as_deref(), Some("198.51.100.1"));
    }

    #[test]
    fn socket_identity_rejects_non_dpinger() {
        assert!(parse_socket_identity("/var/run/other.sock").is_none());
        assert!(parse_socket_identity("/var/run/dpinger_WAN.txt").is_none());
    }

    #[test]
    fn status_line_parses_four_fields() {
        let m = parse_dpinger_status("WAN_DHCP 3049 1866 0").expect("parse");
        assert_eq!(m.gateway_name, "WAN_DHCP");
        assert_eq!(m.rtt_us, 3049);
        assert_eq!(m.stddev_us, 1866);
        assert_eq!(m.loss_pct, 0.0);
    }

    #[test]
    fn status_line_rejects_garbage() {
        assert!(parse_dpinger_status("").is_none());
        assert!(parse_dpinger_status("WAN_DHCP not a number 0").is_none());
        assert!(parse_dpinger_status("WAN_DHCP 3049").is_none()); // too few fields
    }

    #[test]
    fn reconcile_metrics_present_on_good_line() {
        let id = parse_socket_identity(SOCK).unwrap();
        let r = reconcile_report(&id, Some("WAN_DHCP 3049 1866 0"), "pf.example");
        assert_eq!(r.custody, DpingerCustody::MetricsPresent);
        assert_eq!(r.rtt_us, Some(3049));
        assert_eq!(r.loss_pct, Some(0.0));
        assert_eq!(r.monitor_ip.as_deref(), Some("198.51.100.1"));
        assert!(r.source.contains("dpinger:/var/run/dpinger_WAN_DHCP"));
    }

    #[test]
    fn reconcile_unreadable_when_socket_mute() {
        let id = parse_socket_identity(SOCK).unwrap();
        let r = reconcile_report(&id, Some("   "), "pf.example");
        assert_eq!(r.custody, DpingerCustody::SocketUnreadable);
        let r2 = reconcile_report(&id, None, "pf.example");
        assert_eq!(r2.custody, DpingerCustody::SocketUnreadable);
    }

    #[test]
    fn reconcile_unknown_custody_on_unparseable_or_name_mismatch() {
        let id = parse_socket_identity(SOCK).unwrap();
        let bad = reconcile_report(&id, Some("garbage line here"), "pf.example");
        assert_eq!(bad.custody, DpingerCustody::UnknownCustody);
        // Daemon names a different gateway than the socket filename.
        let mism = reconcile_report(&id, Some("WAN2_PPP 10 2 0"), "pf.example");
        assert_eq!(mism.custody, DpingerCustody::UnknownCustody);
    }

    #[test]
    fn split_gather_pulls_sockets_and_status() {
        let gather = format!(
            "{s}\n{sock}\n{st}\n{mark}{sock}\nWAN_DHCP 3049 1866 0\n{e}\n",
            s = SECTION_SOCKS,
            st = SECTION_STATUS,
            mark = SOCK_MARK.trim(),
            sock = SOCK,
            e = SECTION_END,
        );
        let (socks, status) = split_gather(&gather);
        assert_eq!(socks, vec![SOCK.to_string()]);
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].0, SOCK);
        assert_eq!(status[0].1, "WAN_DHCP 3049 1866 0");
    }

    #[test]
    fn run_stamp_and_slug_are_filesystem_safe() {
        assert_eq!(run_stamp("2026-06-24T12:00:01.5Z"), "20260624T120001Z");
        assert_eq!(host_slug("WAN_DHCP"), "WAN_DHCP");
        assert_eq!(host_slug("a~b/c"), "a_b_c");
    }

    #[test]
    fn persist_refuses_overwrite() {
        // Build a minimal receipt via the core to exercise the sink.
        use crate::gateway_path_probe::evaluate_gateway_path;
        let id = parse_socket_identity(SOCK).unwrap();
        let dpinger = reconcile_report(&id, Some("WAN_DHCP 3049 1866 0"), "pf.example");
        let clock = ClockBasis {
            source: "system_wall".to_string(),
            ntp_status: "unknown".to_string(),
        };
        let receipt =
            evaluate_gateway_path(&dpinger, &[], &clock, at("2026-06-24T12:00:01Z"));
        let tmp = std::env::temp_dir().join(format!("nq-gwpath-test-{}", std::process::id()));
        let p1 = persist_receipt(&tmp, &receipt).expect("first write");
        assert!(p1.exists());
        let second = persist_receipt(&tmp, &receipt);
        assert!(second.is_err(), "append-only: refuse overwrite");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
