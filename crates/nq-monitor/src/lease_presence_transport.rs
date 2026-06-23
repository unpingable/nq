//! Live read for the lease-vs-presence specimen (Phase 2).
//!
//! The verdict core (`lease_presence_probe`) is pure; this module fills its
//! inputs from reality, the way the TLS transport fed the TLS core:
//!
//!   1. one read-only SSH call to pfSense: detect the DHCP backend, read the
//!      lease store, read the ARP table (`arp -an`);
//!   2. pure parsers turn that text into a [`LeaseReport`] + presence from
//!      the box's own ARP residue (a `pfSenseRuntimeReport`);
//!   3. an OPTIONAL presence probe from THIS host — the named vantage —
//!      contributes `ObservedReachability` (ICMP echo or a TCP connect);
//!   4. [`evaluate_lease_presence`] decides the non-lift verdict.
//!
//! Read-only and non-mutating by construction: the only remote commands are
//! a backend check, a `cat` of the lease file, and `arp -an`. No `pfctl -d`,
//! no `-f`, no service control, no config write. SSH command execution is a
//! transition (a login/auth-log line) — accounted, but it changes no state.
//!
//! Source typing travels with every datum: the lease and ARP are pfSense
//! self-reports; only the local probe is observed reachability, and only
//! from this vantage at this time. A lease is never presence; an
//! uncorroborated lease is never "host down."

use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, Context};
use time::{OffsetDateTime, PrimitiveDateTime};

use crate::lease_presence_probe::{
    evaluate_lease_presence, ClockBasis, LeasePresenceReceipt, LeaseReport, LeaseState,
    PresenceMethod, PresenceObservation, PresenceOutcome,
};

/// Where to SSH, and with what identity. NQ runs on an independent box; this
/// is never pfSense running nq-monitor.
#[derive(Debug, Clone)]
pub struct SshTarget {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: PathBuf,
    /// Connect timeout for the SSH read, seconds.
    pub timeout_seconds: u64,
}

/// The DHCP backend detected on the box. pfSense 2.8.x can run either; this
/// build parses ISC and detects (but does not yet parse) Kea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhcpBackend {
    Isc,
    Kea,
    Unknown,
}

/// The optional presence probe NQ runs from THIS host (the vantage).
#[derive(Debug, Clone, Copy)]
pub enum ProbeSpec {
    /// ICMP echo via the system `ping` (no raw-socket privilege needed).
    Icmp,
    /// TCP connect to a port (presence = the connect completes/refuses vs
    /// times out). A refused connect still proves the host answered.
    Tcp(u16),
}

// ───────────────────────── pure parsers (tested) ─────────────────────────

/// One ISC lease as parsed from `dhcpd.leases`. The current state of an IP
/// is the LAST block for it (the file is append-history).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IscLease {
    pub ip: String,
    /// The `binding state` token verbatim (e.g. `active`, `free`).
    pub binding_state: String,
    pub ends: Option<OffsetDateTime>,
    pub hardware: Option<String>,
    pub hostname: Option<String>,
}

/// Parse an ISC `dhcpd.leases` file. Returns the CURRENT lease per IP (last
/// block wins). Times are UTC (ISC writes leases in UTC).
pub fn parse_isc_leases(text: &str) -> Vec<IscLease> {
    // Preserve first-seen order of IPs while letting later blocks overwrite.
    let mut order: Vec<String> = Vec::new();
    let mut by_ip: std::collections::HashMap<String, IscLease> = std::collections::HashMap::new();

    let mut cur: Option<IscLease> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("lease ") {
            // `lease <ip> {`
            let ip = rest.trim_end_matches('{').trim().to_string();
            cur = Some(IscLease {
                ip,
                binding_state: String::new(),
                ends: None,
                hardware: None,
                hostname: None,
            });
        } else if line == "}" {
            if let Some(l) = cur.take() {
                if !by_ip.contains_key(&l.ip) {
                    order.push(l.ip.clone());
                }
                by_ip.insert(l.ip.clone(), l);
            }
        } else if let Some(l) = cur.as_mut() {
            if let Some(v) = line.strip_prefix("binding state ") {
                l.binding_state = v.trim_end_matches(';').trim().to_string();
            } else if let Some(v) = line.strip_prefix("ends ") {
                l.ends = parse_isc_time(v.trim_end_matches(';').trim());
            } else if let Some(v) = line.strip_prefix("hardware ethernet ") {
                l.hardware = Some(v.trim_end_matches(';').trim().to_string());
            } else if let Some(v) = line.strip_prefix("client-hostname ") {
                l.hostname = Some(v.trim_end_matches(';').trim().trim_matches('"').to_string());
            }
        }
    }

    order
        .into_iter()
        .filter_map(|ip| by_ip.remove(&ip))
        .collect()
}

/// `ends <weekday> YYYY/MM/DD HH:MM:SS` (UTC) or `never`.
fn parse_isc_time(s: &str) -> Option<OffsetDateTime> {
    if s == "never" {
        return None;
    }
    // Drop the leading weekday digit if present: "4 2026/06/23 15:12:00".
    let datetime = s.splitn(2, ' ').nth(1).unwrap_or(s).trim();
    let fmt = time::format_description::parse(
        "[year]/[month]/[day] [hour]:[minute]:[second]",
    )
    .ok()?;
    PrimitiveDateTime::parse(datetime, &fmt)
        .ok()
        .map(|p| p.assume_utc())
}

/// Map an ISC binding state (+ lease end vs the probe clock) to the core's
/// [`LeaseState`]. Only `active` with a future `ends` claims a live occupant;
/// an `active` lease whose `ends` has passed is reported as expired.
pub fn isc_lease_state(binding_state: &str, ends: Option<OffsetDateTime>, now: OffsetDateTime) -> LeaseState {
    match binding_state {
        "active" => match ends {
            Some(end) if end <= now => LeaseState::Expired,
            _ => LeaseState::Active,
        },
        "free" | "released" => LeaseState::Released,
        "expired" => LeaseState::Expired,
        "" => LeaseState::Unknown,
        _ => LeaseState::Unknown,
    }
}

/// One ARP/NDP table entry as the box reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArpEntry {
    pub ip: String,
    /// `None` for an incomplete entry (the box has no MAC for this IP).
    pub mac: Option<String>,
    pub iface: Option<String>,
    pub permanent: bool,
}

/// Parse BSD `arp -an` output (example format, anonymized):
/// `? (10.0.0.50) at 02:00:00:00:00:50 on em0 expires in 1020 seconds [ethernet]`
/// `? (10.0.0.9) at (incomplete) on em0 [ethernet]`
pub fn parse_arp(text: &str) -> Vec<ArpEntry> {
    let mut out = Vec::new();
    for line in text.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        // Find "(ip)" then "at" <mac> then optional "on" <iface>.
        let Some(ip_tok) = toks.iter().find(|t| t.starts_with('(') && t.ends_with(')')) else {
            continue;
        };
        let ip = ip_tok.trim_matches(|c| c == '(' || c == ')').to_string();
        if ip.is_empty() {
            continue;
        }
        let mac = toks
            .iter()
            .position(|t| *t == "at")
            .and_then(|i| toks.get(i + 1).copied())
            .filter(|m| *m != "(incomplete)")
            .map(|m| m.to_string());
        let iface = toks
            .iter()
            .position(|t| *t == "on")
            .and_then(|i| toks.get(i + 1).copied())
            .map(|s| s.to_string());
        out.push(ArpEntry {
            ip,
            mac,
            iface,
            permanent: line.contains("permanent"),
        });
    }
    out
}

// ─────────────────────────── ssh read (live) ───────────────────────────

const SECTION_BACKEND: &str = "===NQ_BACKEND===";
const SECTION_LEASES: &str = "===NQ_LEASES===";
const SECTION_ARP: &str = "===NQ_ARP===";
const SECTION_END: &str = "===NQ_END===";

/// The single read-only remote script. Backend detection + lease cat + ARP,
/// one login. Strictly read-only.
fn remote_read_script() -> String {
    format!(
        "echo {b}; if [ -f /var/dhcpd/var/db/dhcpd.leases ]; then echo isc; \
         elif [ -f /var/db/kea/kea-leases4.csv ]; then echo kea; else echo unknown; fi; \
         echo {l}; cat /var/dhcpd/var/db/dhcpd.leases 2>/dev/null; \
         echo {a}; arp -an; echo {e}",
        b = SECTION_BACKEND,
        l = SECTION_LEASES,
        a = SECTION_ARP,
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

/// Split the gather output into (backend, leases_text, arp_text).
pub fn split_sections(gather: &str) -> (DhcpBackend, String, String) {
    fn between<'a>(s: &'a str, start: &str, end: &str) -> &'a str {
        let Some(a) = s.find(start) else { return "" };
        let after = &s[a + start.len()..];
        match after.find(end) {
            Some(b) => &after[..b],
            None => after,
        }
    }
    let backend = match between(gather, SECTION_BACKEND, SECTION_LEASES).trim() {
        "isc" => DhcpBackend::Isc,
        "kea" => DhcpBackend::Kea,
        _ => DhcpBackend::Unknown,
    };
    let leases = between(gather, SECTION_LEASES, SECTION_ARP).trim().to_string();
    let arp = between(gather, SECTION_ARP, SECTION_END).trim().to_string();
    (backend, leases, arp)
}

// ───────────────────── presence probe (this vantage) ─────────────────────

/// Run the optional presence probe from THIS host against `ip`. Observed
/// reachability, only from this vantage at this time.
pub fn probe_presence(ip: &str, spec: ProbeSpec, timeout: Duration) -> PresenceOutcome {
    match spec {
        ProbeSpec::Icmp => {
            // System ping; -c1 one echo, -W timeout (Linux: seconds; min 1).
            let secs = timeout.as_secs().max(1).to_string();
            match Command::new("ping").args(["-c", "1", "-W", &secs, ip]).output() {
                Ok(o) if o.status.success() => PresenceOutcome::Observed,
                Ok(_) => PresenceOutcome::NotObserved,
                Err(_) => PresenceOutcome::NotAttempted,
            }
        }
        ProbeSpec::Tcp(port) => {
            let addr = format!("{ip}:{port}");
            match addr.to_socket_addrs().ok().and_then(|mut a| a.next()) {
                Some(sa) => match TcpStream::connect_timeout(&sa, timeout) {
                    // A completed connect OR a refusal both prove the host
                    // answered the packet (refusal is an active response).
                    Ok(_) => PresenceOutcome::Observed,
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                        PresenceOutcome::Observed
                    }
                    Err(_) => PresenceOutcome::NotObserved,
                },
                None => PresenceOutcome::NotAttempted,
            }
        }
    }
}

// ─────────────────────────── orchestration ───────────────────────────

/// Build a [`LeaseReport`] for `ip` from parsed leases. Returns the current
/// lease (last block); `None` if the IP has no lease at all.
pub fn lease_report_for(
    leases: &[IscLease],
    ip: &str,
    now: OffsetDateTime,
    source: &str,
) -> Option<LeaseReport> {
    leases.iter().find(|l| l.ip == ip).map(|l| LeaseReport {
        hostname: l.hostname.clone().filter(|h| !h.is_empty()),
        ip: l.ip.clone(),
        mac: l.hardware.clone(),
        state: isc_lease_state(&l.binding_state, l.ends, now),
        source: source.to_string(),
    })
}

/// Presence from the box's own ARP table — a `pfSenseRuntimeReport`. An entry
/// with a real MAC is `Observed`; absent / incomplete is `NotObserved`.
pub fn arp_presence(arp: &[ArpEntry], ip: &str, now: OffsetDateTime) -> PresenceObservation {
    let observed = arp.iter().any(|e| e.ip == ip && e.mac.is_some());
    PresenceObservation::new(
        PresenceMethod::PfsenseArp,
        "pfsense_arp_table",
        if observed {
            PresenceOutcome::Observed
        } else {
            PresenceOutcome::NotObserved
        },
        now,
    )
}

/// Full live read for one subject IP. One SSH login; optional local probe.
#[allow(clippy::too_many_arguments)]
pub fn live_lease_presence(
    target: &SshTarget,
    vantage: &str,
    subject_ip: &str,
    probe: Option<ProbeSpec>,
    clock: &ClockBasis,
    now: OffsetDateTime,
) -> anyhow::Result<LeasePresenceReceipt> {
    let gather = ssh_gather(target)?;
    let (backend, leases_text, arp_text) = split_sections(&gather);
    if backend != DhcpBackend::Isc {
        return Err(anyhow!(
            "DHCP backend is {:?}; this build parses ISC dhcpd.leases only (Kea read is a follow-up)",
            backend
        ));
    }

    let leases = parse_isc_leases(&leases_text);
    let arp = parse_arp(&arp_text);

    let source = format!("ssh:{} isc-dhcpd:/var/dhcpd/var/db/dhcpd.leases", target.host);
    let lease = lease_report_for(&leases, subject_ip, now, &source)
        .ok_or_else(|| anyhow!("no DHCP lease found for {subject_ip} (cannot run lease-presence)"))?;

    let mut observations = vec![arp_presence(&arp, subject_ip, now)];
    if let Some(spec) = probe {
        let outcome = probe_presence(subject_ip, spec, Duration::from_secs(target.timeout_seconds));
        let method = match spec {
            ProbeSpec::Icmp => PresenceMethod::IcmpEcho,
            ProbeSpec::Tcp(_) => PresenceMethod::TcpConnect,
        };
        observations.push(PresenceObservation::new(method, vantage, outcome, now));
    }

    Ok(evaluate_lease_presence(&lease, &observations, clock, now))
}

// ─────────────────────── append-only receipt sink ───────────────────────

/// Append a receipt under `base/<YYYYMMDDTHHMMSSZ>/<ip>.json`. Append-only:
/// refuses to overwrite an existing file. Mirrors the TLS series discipline;
/// a shared sink is deferred until a third probe family needs it.
pub fn persist_receipt(base: &Path, receipt: &LeasePresenceReceipt) -> anyhow::Result<PathBuf> {
    let stamp = run_stamp(&receipt.probe_time);
    let dir = base.join(stamp);
    std::fs::create_dir_all(&dir).with_context(|| format!("create series dir {}", dir.display()))?;
    let path = dir.join(format!("{}.json", host_slug(&receipt.lease.ip)));
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
        .map(|c| if c == ':' || c == '/' || c == '\\' { '_' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::format_description::well_known::Rfc3339;

    fn at(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).expect("rfc3339")
    }

    // Fixtures are ANONYMIZED — modeled on the real pfSense 2.8.1 ISC
    // dhcpd.leases / `arp -an` format, with invented IPs/MACs/hostnames.
    // Real lease data (MACs, hostnames) is never committed.
    const LEASES: &str = "\
# isc-dhcp-4.4.3-P1
authoring-byte-order little-endian;

lease 10.0.0.5 {
  starts 3 2026/06/23 01:51:00;
  ends 4 2026/06/23 13:51:00;
  cltt 3 2026/06/23 02:19:43;
  binding state active;
  next binding state free;
  hardware ethernet 02:00:00:00:00:05;
  client-hostname \"alpha\";
}
lease 10.0.0.6 {
  starts 3 2026/06/20 01:51:00;
  ends 4 2026/06/20 13:51:00;
  binding state free;
  hardware ethernet 02:00:00:00:00:06;
}
lease 10.0.0.5 {
  starts 3 2026/06/23 14:00:00;
  ends 4 2026/06/24 02:00:00;
  binding state active;
  hardware ethernet 02:00:00:00:00:05;
  client-hostname \"alpha\";
}
";

    const ARP: &str = "\
? (10.0.0.5) at 02:00:00:00:00:05 on igc1 expires in 1020 seconds [ethernet]
? (10.0.0.9) at (incomplete) on igc1 [ethernet]
? (10.0.0.1) at 02:00:00:00:00:01 on igc1 permanent [ethernet]
";

    #[test]
    fn isc_parser_takes_last_block_per_ip() {
        let leases = parse_isc_leases(LEASES);
        let five: Vec<_> = leases.iter().filter(|l| l.ip == "10.0.0.5").collect();
        assert_eq!(five.len(), 1, "one current lease per IP");
        let l = five[0];
        // Last block wins -> the later ends time.
        assert_eq!(l.ends, Some(at("2026-06-24T02:00:00Z")));
        assert_eq!(l.hostname.as_deref(), Some("alpha"));
        assert_eq!(l.hardware.as_deref(), Some("02:00:00:00:00:05"));
        assert_eq!(l.binding_state, "active");
    }

    #[test]
    fn isc_state_mapping_respects_the_clock() {
        // active + future ends = Active.
        assert_eq!(
            isc_lease_state("active", Some(at("2026-06-24T02:00:00Z")), at("2026-06-23T18:00:00Z")),
            LeaseState::Active
        );
        // active + past ends = Expired (stale active block).
        assert_eq!(
            isc_lease_state("active", Some(at("2026-06-23T13:51:00Z")), at("2026-06-23T18:00:00Z")),
            LeaseState::Expired
        );
        // free = Released (no occupant claim).
        assert_eq!(
            isc_lease_state("free", None, at("2026-06-23T18:00:00Z")),
            LeaseState::Released
        );
    }

    #[test]
    fn arp_parser_extracts_ip_mac_iface_and_incomplete() {
        let arp = parse_arp(ARP);
        assert_eq!(arp.len(), 3);
        let a5 = arp.iter().find(|e| e.ip == "10.0.0.5").unwrap();
        assert_eq!(a5.mac.as_deref(), Some("02:00:00:00:00:05"));
        assert_eq!(a5.iface.as_deref(), Some("igc1"));
        let a9 = arp.iter().find(|e| e.ip == "10.0.0.9").unwrap();
        assert_eq!(a9.mac, None, "(incomplete) -> no mac");
        let a1 = arp.iter().find(|e| e.ip == "10.0.0.1").unwrap();
        assert!(a1.permanent);
    }

    #[test]
    fn arp_presence_is_observed_only_with_a_real_mac() {
        let arp = parse_arp(ARP);
        let now = at("2026-06-23T18:00:00Z");
        assert_eq!(arp_presence(&arp, "10.0.0.5", now).outcome, PresenceOutcome::Observed);
        assert_eq!(arp_presence(&arp, "10.0.0.9", now).outcome, PresenceOutcome::NotObserved); // incomplete
        assert_eq!(arp_presence(&arp, "10.0.0.250", now).outcome, PresenceOutcome::NotObserved); // absent
        // ARP presence is typed as a pfSense report, not observed reachability.
        assert_eq!(arp_presence(&arp, "10.0.0.5", now).testimony_type, "pfsense_runtime_report");
    }

    #[test]
    fn lease_report_maps_the_current_lease() {
        let leases = parse_isc_leases(LEASES);
        let now = at("2026-06-23T18:00:00Z");
        let r = lease_report_for(&leases, "10.0.0.5", now, "test").unwrap();
        assert_eq!(r.state, LeaseState::Active);
        assert_eq!(r.ip, "10.0.0.5");
        assert!(lease_report_for(&leases, "10.0.0.250", now, "test").is_none());
    }

    #[test]
    fn section_splitter_separates_backend_leases_arp() {
        let gather = format!(
            "{SECTION_BACKEND}\nisc\n{SECTION_LEASES}\n{LEASES}\n{SECTION_ARP}\n{ARP}\n{SECTION_END}\n"
        );
        let (backend, leases, arp) = split_sections(&gather);
        assert_eq!(backend, DhcpBackend::Isc);
        assert!(leases.contains("lease 10.0.0.5"));
        assert!(arp.contains("10.0.0.5"));
        assert!(!arp.contains(SECTION_END));
    }

    #[test]
    fn persist_is_append_only() {
        let dir = tempfile::tempdir().unwrap();
        let leases = parse_isc_leases(LEASES);
        let now = at("2026-06-23T18:00:00Z");
        let lease = lease_report_for(&leases, "10.0.0.5", now, "test").unwrap();
        let obs = vec![arp_presence(&parse_arp(ARP), "10.0.0.5", now)];
        let clock = ClockBasis { source: "system_ntp".into(), ntp_status: "recorded".into() };
        let r = evaluate_lease_presence(&lease, &obs, &clock, now);
        let p1 = persist_receipt(dir.path(), &r).unwrap();
        assert!(p1.exists());
        // Same probe_time + IP -> same path -> must refuse, not clobber.
        let err = persist_receipt(dir.path(), &r).unwrap_err();
        assert!(err.to_string().contains("append-only"), "{err}");
    }

    /// End-to-end over fixtures (no SSH, no probe): active lease + ARP
    /// observed -> corroborated.
    #[test]
    fn fixture_end_to_end_corroborated_via_arp() {
        let leases = parse_isc_leases(LEASES);
        let arp = parse_arp(ARP);
        let now = at("2026-06-23T18:00:00Z");
        let lease = lease_report_for(&leases, "10.0.0.5", now, "test").unwrap();
        let obs = vec![arp_presence(&arp, "10.0.0.5", now)];
        let clock = ClockBasis { source: "system_ntp".into(), ntp_status: "recorded".into() };
        let r = evaluate_lease_presence(&lease, &obs, &clock, now);
        assert_eq!(
            r.verdict,
            crate::lease_presence_probe::LeasePresenceVerdict::LeaseCorroboratedByPresence
        );
    }
}
