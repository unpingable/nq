//! `dns_state` V0 probe: smallest DNS prober that writes one
//! `dns_observations` row per query into an existing aggregator
//! generation context.
//!
//! See `docs/working/gaps/DNS_WITNESS_FAMILY_GAP.md`. This module owns the
//! mockable boundary between the DNS wire layer and the substrate
//! writer:
//!
//! - `WireOutcome` enum: closed taxonomy that wire-level code
//!   (`UdpDnsClient` or test mocks) produces. One variant per
//!   `ResponseKind` family the substrate may store.
//! - `DnsClient` trait: query boundary. The real implementation is a
//!   tiny hand-rolled UDP DNS client (V0 only; no TCP fallback, no
//!   DNSSEC, no recursive resolver of resolver hostnames).
//! - `outcome_from_wire`: pure mapping `WireOutcome → ProbeOutcome`.
//!   The classifier under unit test.
//! - `record_probe`: orchestrator. Asks the client, builds the row,
//!   writes via `insert_dns_observation`. Tests pass a mock client.
//!
//! V0 boundaries (do not widen here):
//!   * one query per invocation;
//!   * `vantage_host` is passed explicitly — never inferred from
//!     `gethostname()` or any other guess;
//!   * resolver is an IP literal (IPv4 / IPv6, optional port). Hostname
//!     resolvers would force a recursive lookup of the resolver itself,
//!     which is meta and out of V0 scope;
//!   * the row is written into the latest existing aggregator
//!     generation (`generations.status` CHECK forbids a probe-only
//!     status string; sharing the aggregator's pulse inherits retention
//!     cascade and does not pollute `ingest_state`);
//!   * no TCP fallback (TC bit responds as `transport_error`);
//!   * no DNSSEC validation (the `validation_failure` slot is reserved
//!     in `ResponseKind` but V0 never emits it);
//!   * no service-health, endpoint-reachability, or "DNS healthy"
//!     claims — those refusals live in `dns_state_cannot_testify`.

use nq_core::preflight::ResponseKind;
use nq_db::{insert_dns_observation, DnsObservation};
use rusqlite::Connection;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Outcome of one probe attempt, mirroring the `dns_observations` row
/// fields that depend on the wire-level result. Pure data; built by
/// `outcome_from_wire`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutcome {
    pub response_kind: ResponseKind,
    pub rcode: Option<i64>,
    pub answer_summary: Option<String>,
    pub min_ttl_seconds: Option<i64>,
    pub duration_ms: i64,
    pub error_detail: Option<String>,
}

/// Wire-level result of one DNS query. Closed enum; the mockable
/// boundary between the DNS protocol layer (real or test) and the
/// substrate writer. One variant per `ResponseKind` family the
/// substrate can store.
///
/// `validation_failure` is **not** present here — V0 collectors do not
/// validate DNSSEC. The `ResponseKind::ValidationFailure` slot is
/// reserved for a future validating probe; reaching it from V0 wire
/// code would be a misuse and is impossible by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireOutcome {
    /// Resolver returned an answer with records matching the query
    /// type. `answer_summary` is a sorted, comma-joined fingerprint of
    /// the rdata (IPs for A/AAAA; `<TYPE/Nbytes>` for other types).
    Answer {
        rcode: i64,
        answer_summary: String,
        min_ttl_seconds: Option<i64>,
    },
    /// Resolver returned a clean negative answer. The `kind` narrows
    /// which negative shape; `rcode` is the raw DNS RCODE.
    Negative { kind: NegativeKind, rcode: i64 },
    /// Resolver did not respond within the configured budget.
    Timeout,
    /// Vantage could not reach the resolver (socket-level failure,
    /// truncated UDP response with no TCP fallback, etc.). `detail` is
    /// a short human-readable error string.
    TransportError { detail: String },
}

/// Narrowing of `WireOutcome::Negative`. One variant per RCODE the
/// substrate stores as a distinct negative testimony — conflating any
/// of them is the bug `dns_state` exists to refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegativeKind {
    Nodata,
    Nxdomain,
    Servfail,
    Refused,
}

impl NegativeKind {
    fn to_response_kind(self) -> ResponseKind {
        match self {
            Self::Nodata => ResponseKind::Nodata,
            Self::Nxdomain => ResponseKind::Nxdomain,
            Self::Servfail => ResponseKind::Servfail,
            Self::Refused => ResponseKind::Refused,
        }
    }
}

/// Project a `WireOutcome` plus an elapsed-time measurement onto a
/// `ProbeOutcome`. Pure; unit-tested for all variants.
pub fn outcome_from_wire(wire: WireOutcome, duration_ms: i64) -> ProbeOutcome {
    match wire {
        WireOutcome::Answer {
            rcode,
            answer_summary,
            min_ttl_seconds,
        } => ProbeOutcome {
            response_kind: ResponseKind::Success,
            rcode: Some(rcode),
            answer_summary: Some(answer_summary),
            min_ttl_seconds,
            duration_ms,
            error_detail: None,
        },
        WireOutcome::Negative { kind, rcode } => ProbeOutcome {
            response_kind: kind.to_response_kind(),
            rcode: Some(rcode),
            answer_summary: None,
            min_ttl_seconds: None,
            duration_ms,
            error_detail: None,
        },
        WireOutcome::Timeout => ProbeOutcome {
            response_kind: ResponseKind::Timeout,
            rcode: None,
            answer_summary: None,
            min_ttl_seconds: None,
            duration_ms,
            error_detail: None,
        },
        WireOutcome::TransportError { detail } => ProbeOutcome {
            response_kind: ResponseKind::TransportError,
            rcode: None,
            answer_summary: None,
            min_ttl_seconds: None,
            duration_ms,
            error_detail: Some(detail),
        },
    }
}

/// DNS client boundary. The real impl (`UdpDnsClient`) hand-rolls UDP
/// queries; tests pass a mock that returns a canned `WireOutcome`.
pub trait DnsClient {
    fn query(&self, resolver: &str, name: &str, qtype: u16, timeout: Duration) -> WireOutcome;
}

/// Orchestrator: time the query, build the observation row, write it.
/// Tests call this with a mock client; the CLI calls it with
/// `UdpDnsClient`.
pub fn record_probe(
    conn: &Connection,
    gen_id: i64,
    vantage: &str,
    resolver: &str,
    name: &str,
    qtype_str: &str,
    qtype: u16,
    timeout: Duration,
    client: &dyn DnsClient,
) -> anyhow::Result<DnsObservation> {
    let start = Instant::now();
    let wire = client.query(resolver, name, qtype, timeout);
    let duration_ms = start.elapsed().as_millis() as i64;
    let outcome = outcome_from_wire(wire, duration_ms);
    let observed_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)?;

    let obs = DnsObservation {
        observation_id: None,
        generation_id: gen_id,
        vantage_host: vantage.to_string(),
        resolver: resolver.to_string(),
        query_name: name.to_string(),
        query_type: qtype_str.to_string(),
        response_kind: outcome.response_kind,
        rcode: outcome.rcode,
        answer_summary: outcome.answer_summary,
        min_ttl_seconds: outcome.min_ttl_seconds,
        duration_ms: outcome.duration_ms,
        observed_at,
        error_detail: outcome.error_detail,
    };
    let id = insert_dns_observation(conn, &obs)?;
    Ok(DnsObservation {
        observation_id: Some(id),
        ..obs
    })
}

/// Read the latest aggregator generation_id from the DB. V0 probes
/// write their observations into the pulse the aggregator is already
/// running; if no generation exists, the probe errors clearly rather
/// than synthesizing one (which would either fail the
/// `generations.status` CHECK constraint or pollute the `ingest_state`
/// surface).
pub fn read_latest_generation_id(conn: &Connection) -> anyhow::Result<i64> {
    let row: Option<i64> = conn
        .query_row(
            "SELECT generation_id FROM generations ORDER BY completed_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    row.ok_or_else(|| {
        anyhow::anyhow!(
            "no generations exist in this DB; the V0 probe writes observations within the latest \
             aggregator generation. Start `nq-monitor serve` (or wait for one pull cycle) so a generation \
             row exists, then re-run the probe."
        )
    })
}

/// Map a textual query type to its DNS numeric type code. V0
/// accepts a small set of common types; the goal of the probe is
/// substrate, not record-type theater. Unknown types are rejected
/// at the CLI boundary.
pub fn parse_qtype(s: &str) -> anyhow::Result<u16> {
    match s.trim().to_ascii_uppercase().as_str() {
        "A" => Ok(1),
        "NS" => Ok(2),
        "CNAME" => Ok(5),
        "SOA" => Ok(6),
        "PTR" => Ok(12),
        "MX" => Ok(15),
        "TXT" => Ok(16),
        "AAAA" => Ok(28),
        "SRV" => Ok(33),
        other => Err(anyhow::anyhow!(
            "unsupported query type {other:?}; V0 accepts: A, AAAA, NS, CNAME, MX, TXT, SOA, PTR, SRV"
        )),
    }
}

/// Parse a resolver address. Accepts IPv4 (`8.8.8.8`), IPv4 with port
/// (`8.8.8.8:53`), IPv6 (`2001:4860:4860::8888`), and IPv6 with port
/// (`[2001:4860:4860::8888]:53`). Hostname resolvers are rejected —
/// resolving the resolver's name would force a recursive DNS lookup on
/// the same vantage, which V0 does not model.
pub fn parse_resolver(resolver: &str) -> anyhow::Result<SocketAddr> {
    if let Ok(ip) = resolver.parse::<std::net::IpAddr>() {
        return Ok(SocketAddr::new(ip, 53));
    }
    resolver.parse::<SocketAddr>().map_err(|e| {
        anyhow::anyhow!(
            "cannot parse resolver {resolver:?}: {e}. Expected an IP literal (8.8.8.8 or \
             2001:4860:4860::8888) or IP:port ([2001:db8::1]:53). V0 does not accept hostname \
             resolvers — resolving the resolver itself would force a recursive lookup."
        )
    })
}

// ---------------------------------------------------------------------------
// Hand-rolled UDP DNS client. V0 only — minimal protocol surface:
//   * single UDP datagram out, single UDP datagram in;
//   * no TCP fallback (TC bit → TransportError);
//   * no EDNS, no DNSSEC;
//   * answer parsing decodes A and AAAA into IP strings; other types
//     are counted into the summary as `<TYPE/Nbytes>` so the row
//     survives without lying about content.
// ---------------------------------------------------------------------------

/// Real UDP-based DNS client. Stateless; one socket per query.
pub struct UdpDnsClient;

impl DnsClient for UdpDnsClient {
    fn query(&self, resolver: &str, name: &str, qtype: u16, timeout: Duration) -> WireOutcome {
        let addr = match parse_resolver(resolver) {
            Ok(a) => a,
            Err(e) => {
                return WireOutcome::TransportError {
                    detail: e.to_string(),
                }
            }
        };
        let id = generate_query_id();
        let query = match build_query(id, name, qtype) {
            Ok(q) => q,
            Err(e) => return WireOutcome::TransportError { detail: e },
        };

        let bind_addr = if addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
        let socket = match UdpSocket::bind(bind_addr) {
            Ok(s) => s,
            Err(e) => {
                return WireOutcome::TransportError {
                    detail: format!("bind {bind_addr}: {e}"),
                }
            }
        };
        if let Err(e) = socket.set_read_timeout(Some(timeout)) {
            return WireOutcome::TransportError {
                detail: format!("set_read_timeout: {e}"),
            };
        }
        if let Err(e) = socket.send_to(&query, addr) {
            return WireOutcome::TransportError {
                detail: format!("send_to {addr}: {e}"),
            };
        }

        let mut buf = vec![0u8; 4096];
        match socket.recv(&mut buf) {
            Ok(n) => parse_response(&buf[..n], id, qtype),
            Err(e) => {
                let k = e.kind();
                if k == std::io::ErrorKind::WouldBlock || k == std::io::ErrorKind::TimedOut {
                    WireOutcome::Timeout
                } else {
                    WireOutcome::TransportError {
                        detail: format!("recv: {e}"),
                    }
                }
            }
        }
    }
}

fn generate_query_id() -> u16 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    ((nanos as u16) ^ (pid as u16)) | 0x0001 // ensure non-zero
}

/// Build a DNS query datagram. Returns an error if the name contains a
/// label longer than 63 bytes (DNS protocol limit).
pub fn build_query(id: u16, name: &str, qtype: u16) -> Result<Vec<u8>, String> {
    let trimmed = name.trim_end_matches('.');
    if trimmed.is_empty() {
        return Err("empty query name".to_string());
    }
    for label in trimmed.split('.') {
        if label.is_empty() {
            return Err(format!("empty label in {name:?}"));
        }
        if label.len() > 63 {
            return Err(format!(
                "label {label:?} exceeds DNS 63-byte limit in {name:?}"
            ));
        }
        if !label.is_ascii() {
            return Err(format!(
                "non-ASCII label {label:?} in {name:?}; IDN punycode encoding is the caller's job"
            ));
        }
    }
    let mut q = Vec::with_capacity(32 + trimmed.len());
    q.extend_from_slice(&id.to_be_bytes()); // ID
    q.extend_from_slice(&[0x01, 0x00]); // flags: RD=1
    q.extend_from_slice(&[0x00, 0x01]); // QDCOUNT
    q.extend_from_slice(&[0x00, 0x00]); // ANCOUNT
    q.extend_from_slice(&[0x00, 0x00]); // NSCOUNT
    q.extend_from_slice(&[0x00, 0x00]); // ARCOUNT
    for label in trimmed.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&qtype.to_be_bytes());
    q.extend_from_slice(&[0x00, 0x01]); // QCLASS = IN
    Ok(q)
}

/// Parse a DNS response into a `WireOutcome`. The closed taxonomy is
/// preserved: NXDOMAIN, SERVFAIL, REFUSED, NODATA, success, and
/// truncation each route to a distinct outcome.
pub fn parse_response(buf: &[u8], expected_id: u16, qtype: u16) -> WireOutcome {
    if buf.len() < 12 {
        return WireOutcome::TransportError {
            detail: format!("response truncated: {} bytes < 12-byte header", buf.len()),
        };
    }
    let id = u16::from_be_bytes([buf[0], buf[1]]);
    if id != expected_id {
        return WireOutcome::TransportError {
            detail: format!("response id mismatch: expected {expected_id}, got {id}"),
        };
    }
    if (buf[2] & 0x80) == 0 {
        return WireOutcome::TransportError {
            detail: "QR bit not set; response is a query, not a reply".to_string(),
        };
    }
    if (buf[2] & 0x02) != 0 {
        // TC: truncated. V0 has no TCP fallback.
        return WireOutcome::TransportError {
            detail: "response truncated (TC bit set); V0 has no TCP fallback".to_string(),
        };
    }
    let qdcount = u16::from_be_bytes([buf[4], buf[5]]);
    if qdcount != 1 {
        // V0 always asks one question; a response with a different
        // QDCOUNT cannot be parsed safely (the answer section would
        // begin at an unknown offset).
        return WireOutcome::TransportError {
            detail: format!(
                "unexpected QDCOUNT={qdcount}; V0 only handles single-question responses"
            ),
        };
    }
    let rcode = (buf[3] & 0x0F) as i64;
    let ancount = u16::from_be_bytes([buf[6], buf[7]]);

    // Always validate the question section parses before trusting any
    // header-claimed outcome. A truncated or malformed question makes
    // the header's rcode/ancount uncorroborable — categorizing such
    // a packet as Nodata / Nxdomain / etc. would launder a malformed
    // datagram into testimony.
    let mut pos = 12;
    let Some(p) = skip_name(buf, pos) else {
        return WireOutcome::TransportError {
            detail: "malformed question name".to_string(),
        };
    };
    if p + 4 > buf.len() {
        return WireOutcome::TransportError {
            detail: "question QTYPE/QCLASS truncated".to_string(),
        };
    }
    pos = p + 4;

    match rcode {
        3 => return WireOutcome::Negative { kind: NegativeKind::Nxdomain, rcode },
        2 => return WireOutcome::Negative { kind: NegativeKind::Servfail, rcode },
        5 => return WireOutcome::Negative { kind: NegativeKind::Refused, rcode },
        0 => {} // continue
        // Other RCODEs (FormErr=1, NotImp=4, NotAuth=9, ...) are
        // resolver-side errors; classify as Servfail so the closed
        // taxonomy holds. The raw rcode is preserved on the row.
        _ => return WireOutcome::Negative { kind: NegativeKind::Servfail, rcode },
    }
    // rcode == 0
    if ancount == 0 {
        return WireOutcome::Negative { kind: NegativeKind::Nodata, rcode };
    }

    let mut summaries: Vec<String> = Vec::new();
    let mut min_ttl: Option<u32> = None;
    for _ in 0..ancount {
        let Some(p) = skip_name(buf, pos) else {
            return WireOutcome::TransportError {
                detail: "malformed answer name".to_string(),
            };
        };
        pos = p;
        if pos + 10 > buf.len() {
            return WireOutcome::TransportError {
                detail: "answer record truncated before RR header".to_string(),
            };
        }
        let rtype = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let ttl = u32::from_be_bytes([buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]]);
        let rdlen = u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]) as usize;
        pos += 10;
        if pos + rdlen > buf.len() {
            return WireOutcome::TransportError {
                detail: "answer record truncated within RDATA".to_string(),
            };
        }
        if rtype == qtype {
            min_ttl = Some(min_ttl.map_or(ttl, |m| m.min(ttl)));
            if rtype == 1 && rdlen == 4 {
                let ip = std::net::Ipv4Addr::new(buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]);
                summaries.push(ip.to_string());
            } else if rtype == 28 && rdlen == 16 {
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&buf[pos..pos + 16]);
                summaries.push(std::net::Ipv6Addr::from(octets).to_string());
            } else {
                summaries.push(format!("<type{rtype}/{rdlen}B>"));
            }
        }
        pos += rdlen;
    }

    if summaries.is_empty() {
        // ANCOUNT > 0 but none matched the query type — treat as
        // Nodata for the asked type. The resolver returned records,
        // just not what was asked. This is testimony about the resolver
        // and the asked (name, type) tuple together.
        return WireOutcome::Negative { kind: NegativeKind::Nodata, rcode: 0 };
    }
    summaries.sort();
    WireOutcome::Answer {
        rcode: 0,
        answer_summary: summaries.join(","),
        min_ttl_seconds: min_ttl.map(|t| t as i64),
    }
}

/// Skip past a DNS name in `buf`, starting at `pos`. Returns the
/// position immediately after the name, or `None` if the encoding is
/// malformed. Handles the two label forms the V0 parser sees in
/// practice: length-prefixed labels (terminated by a zero byte) and
/// compression pointers (a 2-byte sequence beginning with `11`).
fn skip_name(buf: &[u8], mut pos: usize) -> Option<usize> {
    let mut steps = 0;
    while pos < buf.len() {
        steps += 1;
        if steps > 128 {
            // Defensive: pathological name encodings.
            return None;
        }
        let len = buf[pos];
        if len == 0 {
            return Some(pos + 1);
        }
        if (len & 0xC0) == 0xC0 {
            if pos + 1 >= buf.len() {
                return None;
            }
            return Some(pos + 2);
        }
        if (len & 0xC0) != 0 {
            // 01 / 10 prefixes are EDNS/reserved label types; not
            // expected in V0 answers.
            return None;
        }
        pos += 1 + len as usize;
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nq_db::{evaluate_dns_state_preflight_from_conn, DnsObservationTuple};
    use nq_core::preflight::Verdict;
    use rusqlite::params;

    /// Test DnsClient that records its inputs and returns a canned
    /// `WireOutcome`. Lets the orchestration tests exercise the
    /// `record_probe → insert_dns_observation → evaluator` pipeline
    /// without any network I/O.
    struct MockDnsClient {
        canned: WireOutcome,
        recorded: std::cell::RefCell<Vec<(String, String, u16, Duration)>>,
    }

    impl MockDnsClient {
        fn new(canned: WireOutcome) -> Self {
            Self {
                canned,
                recorded: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl DnsClient for MockDnsClient {
        fn query(
            &self,
            resolver: &str,
            name: &str,
            qtype: u16,
            timeout: Duration,
        ) -> WireOutcome {
            self.recorded
                .borrow_mut()
                .push((resolver.to_string(), name.to_string(), qtype, timeout));
            self.canned.clone()
        }
    }

    fn migrated_db() -> nq_db::WriteDb {
        let mut db = nq_db::open_rw(std::path::Path::new(":memory:")).unwrap();
        nq_db::migrate(&mut db).unwrap();
        db
    }

    fn seed_generation(conn: &Connection, gen_id: i64) {
        conn.execute(
            "INSERT INTO generations
               (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES (?1, '2026-05-20T00:00:00Z', '2026-05-20T00:00:00Z', 'complete', 1, 1, 0, 0)",
            params![gen_id],
        )
        .unwrap();
    }

    // -------------------- outcome_from_wire (pure classifier) --------------------

    #[test]
    fn outcome_from_wire_success_carries_summary_and_ttl() {
        let out = outcome_from_wire(
            WireOutcome::Answer {
                rcode: 0,
                answer_summary: "1.2.3.4".into(),
                min_ttl_seconds: Some(60),
            },
            42,
        );
        assert_eq!(out.response_kind, ResponseKind::Success);
        assert_eq!(out.rcode, Some(0));
        assert_eq!(out.answer_summary.as_deref(), Some("1.2.3.4"));
        assert_eq!(out.min_ttl_seconds, Some(60));
        assert_eq!(out.duration_ms, 42);
        assert_eq!(out.error_detail, None);
    }

    #[test]
    fn outcome_from_wire_negative_kinds_map_to_distinct_response_kinds() {
        // Each negative kind must map to its own ResponseKind — the
        // closed taxonomy that dns_state exists to refuse collapsing.
        let cases = [
            (NegativeKind::Nodata, 0, ResponseKind::Nodata),
            (NegativeKind::Nxdomain, 3, ResponseKind::Nxdomain),
            (NegativeKind::Servfail, 2, ResponseKind::Servfail),
            (NegativeKind::Refused, 5, ResponseKind::Refused),
        ];
        for (kind, rcode, expected) in cases {
            let out = outcome_from_wire(WireOutcome::Negative { kind, rcode }, 10);
            assert_eq!(out.response_kind, expected, "{kind:?}");
            assert_eq!(out.rcode, Some(rcode));
            assert!(out.answer_summary.is_none());
            assert!(out.min_ttl_seconds.is_none());
            assert!(out.error_detail.is_none());
        }
    }

    #[test]
    fn outcome_from_wire_timeout_has_no_rcode_no_summary_no_detail() {
        let out = outcome_from_wire(WireOutcome::Timeout, 5_001);
        assert_eq!(out.response_kind, ResponseKind::Timeout);
        assert!(out.rcode.is_none());
        assert!(out.answer_summary.is_none());
        assert!(out.min_ttl_seconds.is_none());
        assert!(out.error_detail.is_none());
        assert_eq!(out.duration_ms, 5_001);
    }

    #[test]
    fn outcome_from_wire_transport_error_carries_detail() {
        let out = outcome_from_wire(
            WireOutcome::TransportError {
                detail: "connection refused".into(),
            },
            3,
        );
        assert_eq!(out.response_kind, ResponseKind::TransportError);
        assert!(out.rcode.is_none());
        assert!(out.answer_summary.is_none());
        assert_eq!(out.error_detail.as_deref(), Some("connection refused"));
    }

    // -------------------- parse_response (wire-level classifier) --------------------

    fn header(id: u16, flags: u16, ancount: u16) -> Vec<u8> {
        let mut h = Vec::with_capacity(12);
        h.extend_from_slice(&id.to_be_bytes());
        h.extend_from_slice(&flags.to_be_bytes());
        h.extend_from_slice(&[0x00, 0x01]); // QDCOUNT
        h.extend_from_slice(&ancount.to_be_bytes());
        h.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // NS, AR
        h
    }

    fn encode_name_no_compression(name: &str) -> Vec<u8> {
        let mut out = Vec::new();
        for label in name.trim_end_matches('.').split('.') {
            out.push(label.len() as u8);
            out.extend_from_slice(label.as_bytes());
        }
        out.push(0);
        out
    }

    fn question(name: &str, qtype: u16) -> Vec<u8> {
        let mut q = encode_name_no_compression(name);
        q.extend_from_slice(&qtype.to_be_bytes());
        q.extend_from_slice(&[0x00, 0x01]); // IN
        q
    }

    #[test]
    fn parse_response_truncated_header_is_transport_error() {
        let out = parse_response(&[0; 8], 0x1234, 1);
        assert!(matches!(out, WireOutcome::TransportError { .. }));
    }

    #[test]
    fn parse_response_id_mismatch_is_transport_error() {
        let mut pkt = header(0x5555, 0x8180, 0); // QR=1, RA=1, RCODE=0
        pkt.extend(question("example.com", 1));
        let out = parse_response(&pkt, 0x1234, 1);
        match out {
            WireOutcome::TransportError { detail } => assert!(detail.contains("id mismatch")),
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_qr_bit_unset_is_transport_error() {
        let mut pkt = header(0x1234, 0x0100, 0); // QR=0, RD=1
        pkt.extend(question("example.com", 1));
        let out = parse_response(&pkt, 0x1234, 1);
        match out {
            WireOutcome::TransportError { detail } => assert!(detail.contains("QR bit not set")),
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_truncated_tc_bit_is_transport_error() {
        let mut pkt = header(0x1234, 0x8380, 0); // QR=1, TC=1, RA=1
        pkt.extend(question("example.com", 1));
        let out = parse_response(&pkt, 0x1234, 1);
        match out {
            WireOutcome::TransportError { detail } => assert!(detail.contains("TC bit set")),
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_rcode_nxdomain_yields_nxdomain() {
        let mut pkt = header(0x1234, 0x8183, 0); // QR=1, RA=1, RCODE=3
        pkt.extend(question("example.invalid", 1));
        let out = parse_response(&pkt, 0x1234, 1);
        assert_eq!(
            out,
            WireOutcome::Negative {
                kind: NegativeKind::Nxdomain,
                rcode: 3
            }
        );
    }

    #[test]
    fn parse_response_rcode_servfail_yields_servfail() {
        let mut pkt = header(0x1234, 0x8182, 0); // RCODE=2
        pkt.extend(question("example.com", 1));
        let out = parse_response(&pkt, 0x1234, 1);
        assert_eq!(
            out,
            WireOutcome::Negative {
                kind: NegativeKind::Servfail,
                rcode: 2
            }
        );
    }

    #[test]
    fn parse_response_rcode_refused_yields_refused() {
        let mut pkt = header(0x1234, 0x8185, 0); // RCODE=5
        pkt.extend(question("example.com", 1));
        let out = parse_response(&pkt, 0x1234, 1);
        assert_eq!(
            out,
            WireOutcome::Negative {
                kind: NegativeKind::Refused,
                rcode: 5
            }
        );
    }

    #[test]
    fn parse_response_rcode_zero_with_zero_answers_yields_nodata() {
        let mut pkt = header(0x1234, 0x8180, 0); // QR=1, RA=1, RCODE=0
        pkt.extend(question("example.com", 28)); // AAAA, no AAAA records
        let out = parse_response(&pkt, 0x1234, 28);
        assert_eq!(
            out,
            WireOutcome::Negative {
                kind: NegativeKind::Nodata,
                rcode: 0
            }
        );
    }

    #[test]
    fn parse_response_a_answer_decodes_ipv4_and_min_ttl() {
        let mut pkt = header(0x1234, 0x8180, 2);
        pkt.extend(question("example.com", 1));
        // Answer 1: name pointer (0xC00C → offset 12, the question name),
        // TYPE=A(1), CLASS=IN(1), TTL=300, RDLEN=4, RDATA=1.2.3.4
        pkt.extend_from_slice(&[0xC0, 0x0C]);
        pkt.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        pkt.extend_from_slice(&300u32.to_be_bytes());
        pkt.extend_from_slice(&[0x00, 0x04]);
        pkt.extend_from_slice(&[1, 2, 3, 4]);
        // Answer 2: same shape, TTL=120, RDATA=5.6.7.8 (smaller TTL wins)
        pkt.extend_from_slice(&[0xC0, 0x0C]);
        pkt.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        pkt.extend_from_slice(&120u32.to_be_bytes());
        pkt.extend_from_slice(&[0x00, 0x04]);
        pkt.extend_from_slice(&[5, 6, 7, 8]);

        let out = parse_response(&pkt, 0x1234, 1);
        match out {
            WireOutcome::Answer {
                rcode,
                answer_summary,
                min_ttl_seconds,
            } => {
                assert_eq!(rcode, 0);
                // Sorted, comma-joined IPs.
                assert_eq!(answer_summary, "1.2.3.4,5.6.7.8");
                assert_eq!(min_ttl_seconds, Some(120));
            }
            other => panic!("expected Answer, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_ancount_nonzero_but_no_matching_qtype_is_nodata() {
        // Resolver returned records (e.g. CNAME) for an A query but no
        // A records. Surface this as Nodata for the asked type — the
        // resolver answered "no A records here" via composition.
        let mut pkt = header(0x1234, 0x8180, 1);
        pkt.extend(question("example.com", 1)); // asked for A
        pkt.extend_from_slice(&[0xC0, 0x0C]);
        pkt.extend_from_slice(&[0x00, 0x05, 0x00, 0x01]); // TYPE=CNAME(5)
        pkt.extend_from_slice(&300u32.to_be_bytes());
        pkt.extend_from_slice(&[0x00, 0x02]);
        pkt.extend_from_slice(&[0xC0, 0x0C]); // CNAME rdata = pointer
        let out = parse_response(&pkt, 0x1234, 1);
        assert_eq!(
            out,
            WireOutcome::Negative {
                kind: NegativeKind::Nodata,
                rcode: 0
            }
        );
    }

    #[test]
    fn parse_response_unknown_rcode_routes_to_servfail() {
        // FormErr (1), NotImp (4), NotAuth (9), etc. The taxonomy is
        // closed; uncommon error codes fold into Servfail so the
        // ResponseKind set stays bounded. The raw rcode is preserved.
        let mut pkt = header(0x1234, 0x8184, 0); // RCODE=4 (NotImp)
        pkt.extend(question("example.com", 1));
        let out = parse_response(&pkt, 0x1234, 1);
        assert_eq!(
            out,
            WireOutcome::Negative {
                kind: NegativeKind::Servfail,
                rcode: 4
            }
        );
    }

    // -------------------- parse_response: hostile inputs --------------------
    //
    // The V0 parser is hand-rolled, so malformed UDP datagrams must
    // produce boring outcomes: never panic, never loop, never classify
    // garbled bytes as a categorized answer. Each test below names the
    // hostile shape it exercises; the expected behavior is always
    // either a bounded `WireOutcome::TransportError` or a bounded
    // `Negative` outcome derived from a *well-formed* header.

    #[test]
    fn parse_response_empty_buffer_is_transport_error() {
        // Zero-byte packet: less than the 12-byte header.
        let out = parse_response(&[], 0x1234, 1);
        match out {
            WireOutcome::TransportError { detail } => {
                assert!(detail.contains("12-byte header"), "{detail}");
            }
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_sub_header_lengths_are_all_transport_error() {
        // Every length 1..12 must reject without panic.
        for n in 1..12 {
            let buf = vec![0u8; n];
            match parse_response(&buf, 0x1234, 1) {
                WireOutcome::TransportError { .. } => {}
                other => panic!("len={n} yielded {other:?} instead of TransportError"),
            }
        }
    }

    #[test]
    fn parse_response_header_only_with_rcode_zero_is_transport_error_not_nodata() {
        // Before hardening, a 12-byte header with rcode=0 and ancount=0
        // returned Nodata without checking that a question even
        // existed. The parser must refuse to corroborate the header's
        // claim from a packet with no question section.
        let pkt = header(0x1234, 0x8180, 0); // QR=1, RA=1, RCODE=0
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { .. } => {}
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_header_only_with_rcode_nxdomain_is_transport_error_not_nxdomain() {
        // The same hardening must apply to error rcodes: a truncated
        // NXDOMAIN response is malformed, not authoritative testimony.
        let pkt = header(0x1234, 0x8183, 0); // RCODE=3
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { .. } => {}
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_qdcount_not_one_is_transport_error() {
        // V0 always asks one question; a response that claims a
        // different QDCOUNT is non-standard and cannot be parsed
        // safely (we would misjudge where the answer section begins).
        let mut zero_q = header(0x1234, 0x8180, 0);
        zero_q[4..6].copy_from_slice(&0u16.to_be_bytes());
        zero_q.extend(question("example.com", 1));
        match parse_response(&zero_q, 0x1234, 1) {
            WireOutcome::TransportError { detail } => {
                assert!(detail.contains("QDCOUNT"), "{detail}");
            }
            other => panic!("QDCOUNT=0: expected TransportError, got {other:?}"),
        }

        let mut two_q = header(0x1234, 0x8180, 0);
        two_q[4..6].copy_from_slice(&2u16.to_be_bytes());
        two_q.extend(question("example.com", 1));
        two_q.extend(question("example.org", 1));
        match parse_response(&two_q, 0x1234, 1) {
            WireOutcome::TransportError { detail } => {
                assert!(detail.contains("QDCOUNT"), "{detail}");
            }
            other => panic!("QDCOUNT=2: expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_truncated_question_name_is_transport_error() {
        // Header says QDCOUNT=1 but the question name is interrupted
        // mid-label (length byte says 7, but only 3 bytes follow).
        let mut pkt = header(0x1234, 0x8180, 0);
        pkt.push(7); // length byte
        pkt.extend_from_slice(b"abc"); // only 3 of 7 promised bytes
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { .. } => {}
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_truncated_question_qtype_qclass_is_transport_error() {
        // Question name parses, but QTYPE/QCLASS bytes are missing.
        let mut pkt = header(0x1234, 0x8180, 0);
        pkt.extend(encode_name_no_compression("example.com"));
        // Add only 2 of the 4 trailing question bytes.
        pkt.extend_from_slice(&[0x00, 0x01]);
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { detail } => {
                assert!(detail.contains("QTYPE") || detail.contains("truncated"), "{detail}");
            }
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_truncated_answer_name_is_transport_error() {
        let mut pkt = header(0x1234, 0x8180, 1);
        pkt.extend(question("example.com", 1));
        // Answer name interrupted: length=5 then EOF.
        pkt.push(5);
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { .. } => {}
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_truncated_answer_rr_header_is_transport_error() {
        let mut pkt = header(0x1234, 0x8180, 1);
        pkt.extend(question("example.com", 1));
        // Valid pointer to question name, then only 5 of the 10 RR
        // header bytes (TYPE+CLASS+TTL+RDLENGTH) follow.
        pkt.extend_from_slice(&[0xC0, 0x0C]);
        pkt.extend_from_slice(&[0x00, 0x01, 0x00, 0x01, 0x00]);
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { detail } => {
                assert!(detail.contains("truncated"), "{detail}");
            }
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_truncated_answer_rdata_is_transport_error() {
        let mut pkt = header(0x1234, 0x8180, 1);
        pkt.extend(question("example.com", 1));
        // Valid pointer + RR header claiming RDLENGTH=10, then only
        // 3 RDATA bytes provided.
        pkt.extend_from_slice(&[0xC0, 0x0C]);
        pkt.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        pkt.extend_from_slice(&300u32.to_be_bytes());
        pkt.extend_from_slice(&[0x00, 0x0A]); // RDLENGTH=10
        pkt.extend_from_slice(&[1, 2, 3]); // only 3 bytes
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { detail } => {
                assert!(detail.contains("RDATA") || detail.contains("truncated"), "{detail}");
            }
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_ancount_claims_more_answers_than_present_is_transport_error() {
        // Header promises 3 answers; only 1 is present. The for loop
        // must fail bounds-checking on the second iteration and yield
        // TransportError — never a partial Answer with bogus content.
        let mut pkt = header(0x1234, 0x8180, 3);
        pkt.extend(question("example.com", 1));
        // One valid A record:
        pkt.extend_from_slice(&[0xC0, 0x0C]);
        pkt.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        pkt.extend_from_slice(&300u32.to_be_bytes());
        pkt.extend_from_slice(&[0x00, 0x04]);
        pkt.extend_from_slice(&[1, 2, 3, 4]);
        // Then EOF — answers 2 and 3 missing.
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { .. } => {}
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_compression_pointer_at_end_of_buffer_is_transport_error() {
        // Answer name is a pointer's first byte (0xC0) with the
        // second (offset) byte missing — pointer is out of bounds
        // because the buffer ends before the pointer is complete.
        let mut pkt = header(0x1234, 0x8180, 1);
        pkt.extend(question("example.com", 1));
        pkt.push(0xC0); // pointer byte 1; byte 2 (offset) missing
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { .. } => {}
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_compression_pointer_offset_far_past_buffer_is_handled() {
        // The pointer's offset byte points way past the end of the
        // buffer. V0 does not follow pointers (only skips past them),
        // so this case must not panic. The pointer itself is 2 bytes
        // and is treated as a valid name skip; the parser then
        // continues with the RR header. Since no RR header follows,
        // truncation detection kicks in.
        let mut pkt = header(0x1234, 0x8180, 1);
        pkt.extend(question("example.com", 1));
        pkt.extend_from_slice(&[0xC0, 0xFF]); // pointer to offset 255 — past buffer
        // No RR header bytes after the pointer.
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { .. } => {}
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_compression_pointer_self_loop_does_not_loop_or_panic() {
        // Pointer at offset N references offset N (itself). A parser
        // that followed pointers would loop forever; V0's parser only
        // *skips* past pointers (returns Some(pos+2)), so self-loops
        // are impossible by construction. This test pins that
        // invariant: the parser must terminate cleanly. The pointer
        // counts as a valid name skip; the parser then tries to read
        // the RR header from positions after the pointer, which is
        // also a self-referencing pattern — bounded by buffer length.
        let mut pkt = header(0x1234, 0x8180, 1);
        pkt.extend(question("example.com", 1));
        let answer_start = pkt.len();
        // 0xC0 with offset = answer_start = self-reference.
        pkt.push(0xC0);
        pkt.push(answer_start as u8);
        // Provide a partial RR header so the parser proceeds past
        // the pointer skip but fails bounds-check on truncation —
        // whatever outcome it produces, it must be deterministic and
        // not a panic / infinite loop.
        pkt.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        let _ = parse_response(&pkt, 0x1234, 1);
        // No assertion on outcome other than "the call returned" —
        // this test is a tripwire for infinite-loop / panic regressions.
    }

    #[test]
    fn parse_response_pointer_chain_in_consecutive_answers_terminates() {
        // Two answer records, each with a pointer name. Even if a
        // pointer-following parser would chain through them, V0 only
        // skips past each pointer and reads the RR header
        // immediately after. The full ANCOUNT loop must terminate.
        let mut pkt = header(0x1234, 0x8180, 2);
        pkt.extend(question("example.com", 1));
        for _ in 0..2 {
            pkt.extend_from_slice(&[0xC0, 0x0C]); // pointer to question
            pkt.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
            pkt.extend_from_slice(&60u32.to_be_bytes());
            pkt.extend_from_slice(&[0x00, 0x04]);
            pkt.extend_from_slice(&[1, 2, 3, 4]);
        }
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::Answer { answer_summary, .. } => {
                // Both A records decode to 1.2.3.4; sorted-joined.
                assert_eq!(answer_summary, "1.2.3.4,1.2.3.4");
            }
            other => panic!("expected Answer, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_invalid_label_high_bits_is_transport_error() {
        // The 01 and 10 prefixes on the length byte are EDNS /
        // reserved label types we do not parse. Encountering one in
        // a question or answer name must yield TransportError, not
        // a wild advance into the next field.
        let mut pkt = header(0x1234, 0x8180, 0);
        pkt.push(0x40); // 01-prefixed; invalid for V0
        pkt.push(0x00);
        match parse_response(&pkt, 0x1234, 1) {
            WireOutcome::TransportError { .. } => {}
            other => panic!("expected TransportError, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_all_sixteen_rcodes_are_deterministic_and_bounded() {
        // RCODE is 4 bits, values 0..16. Every value must map
        // deterministically to a single bounded outcome — no panic,
        // no Answer, no surprise. The closed taxonomy folds unknown
        // codes into Servfail with the raw rcode preserved on the
        // observation row.
        for rcode in 0u8..16 {
            let flags = 0x8180u16 | rcode as u16;
            let mut pkt = header(0x1234, flags, 0);
            pkt.extend(question("example.com", 1));
            let out = parse_response(&pkt, 0x1234, 1);
            match (rcode, &out) {
                (0, WireOutcome::Negative { kind: NegativeKind::Nodata, rcode: 0 }) => {}
                (2, WireOutcome::Negative { kind: NegativeKind::Servfail, rcode: 2 }) => {}
                (3, WireOutcome::Negative { kind: NegativeKind::Nxdomain, rcode: 3 }) => {}
                (5, WireOutcome::Negative { kind: NegativeKind::Refused, rcode: 5 }) => {}
                (
                    r,
                    WireOutcome::Negative {
                        kind: NegativeKind::Servfail,
                        rcode: rr,
                    },
                ) if *rr == r as i64 => {}
                _ => panic!("rcode={rcode}: unexpected outcome {out:?}"),
            }
        }
    }

    #[test]
    fn parse_response_arbitrary_short_garbage_does_not_panic() {
        // Deterministic-pseudo-random short buffers; never panic,
        // never classify as Answer. The parser is the unsupervised
        // surface for hostile-resolver bytes — must stay boring.
        let mut state: u32 = 0xACE5_F00D;
        for _ in 0..2_000 {
            // Tiny LCG (Numerical Recipes) — no external dep.
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let len = (state >> 16) as usize % 64; // 0..64
            let mut buf = Vec::with_capacity(len);
            let mut s = state;
            for _ in 0..len {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                buf.push((s >> 16) as u8);
            }
            let out = parse_response(&buf, 0x1234, 1);
            // The parser must never claim Answer for arbitrary bytes
            // unless the bytes happen to perfectly encode one. The
            // probability of a random 0..64-byte buffer parsing as a
            // valid full Answer with id=0x1234, QR=1, TC=0, QDCOUNT=1,
            // ANCOUNT>=1, a valid question, and a complete answer
            // record is vanishingly small — but we don't gamble:
            // assert the outcome is one of the closed variants and
            // move on (the goal is "doesn't panic / doesn't loop").
            match out {
                WireOutcome::Answer { .. }
                | WireOutcome::Negative { .. }
                | WireOutcome::Timeout
                | WireOutcome::TransportError { .. } => {}
            }
        }
    }

    #[test]
    fn parse_response_garbage_after_valid_header_does_not_panic() {
        // Valid header + arbitrary trailing bytes of varying length.
        // Forces the question/answer parser onto hostile bytes.
        let mut state: u32 = 0xBEEF_CAFE;
        for len in 0..200 {
            let mut pkt = header(0x1234, 0x8180, ((len % 5) + 1) as u16);
            for _ in 0..len {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                pkt.push((state >> 16) as u8);
            }
            let _ = parse_response(&pkt, 0x1234, 1);
        }
    }

    // -------------------- build_query --------------------

    #[test]
    fn build_query_rejects_empty_name() {
        assert!(build_query(0x1234, "", 1).is_err());
        assert!(build_query(0x1234, ".", 1).is_err());
    }

    #[test]
    fn build_query_rejects_label_over_63_bytes() {
        let too_long = format!("{}.example.com", "x".repeat(64));
        let err = build_query(0x1234, &too_long, 1).unwrap_err();
        assert!(err.contains("63-byte limit"), "{err}");
    }

    #[test]
    fn build_query_encodes_recursive_a_query() {
        let q = build_query(0x1234, "example.com", 1).unwrap();
        // Header: id, flags(0x0100), QDCOUNT(1), AN/NS/AR=0
        assert_eq!(&q[0..2], &[0x12, 0x34]);
        assert_eq!(&q[2..4], &[0x01, 0x00]);
        assert_eq!(&q[4..6], &[0x00, 0x01]);
        assert_eq!(&q[6..12], &[0; 6]);
        // Question: 7,e,x,a,m,p,l,e, 3,c,o,m, 0, 0,1, 0,1
        let expected_q = b"\x07example\x03com\x00\x00\x01\x00\x01";
        assert_eq!(&q[12..], expected_q);
    }

    // -------------------- parse_qtype --------------------

    #[test]
    fn parse_qtype_supports_v0_set_case_insensitively() {
        assert_eq!(parse_qtype("A").unwrap(), 1);
        assert_eq!(parse_qtype("a").unwrap(), 1);
        assert_eq!(parse_qtype("AAAA").unwrap(), 28);
        assert_eq!(parse_qtype("aaaa").unwrap(), 28);
        assert_eq!(parse_qtype("MX").unwrap(), 15);
    }

    #[test]
    fn parse_qtype_rejects_unknown_and_lists_supported() {
        let err = parse_qtype("ANY").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("A, AAAA"), "{msg}");
        assert!(msg.contains("ANY"), "{msg}");
    }

    // -------------------- parse_resolver --------------------

    #[test]
    fn parse_resolver_accepts_ipv4_and_appends_port_53() {
        let s = parse_resolver("8.8.8.8").unwrap();
        assert_eq!(s, "8.8.8.8:53".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn parse_resolver_accepts_ipv4_with_port() {
        let s = parse_resolver("8.8.8.8:5353").unwrap();
        assert_eq!(s, "8.8.8.8:5353".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn parse_resolver_accepts_ipv6() {
        let s = parse_resolver("2001:4860:4860::8888").unwrap();
        assert_eq!(
            s,
            "[2001:4860:4860::8888]:53".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn parse_resolver_rejects_hostname_with_useful_message() {
        let err = parse_resolver("dns.google").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("hostname"), "{msg}");
        assert!(msg.contains("recursive lookup"), "{msg}");
    }

    // -------------------- read_latest_generation_id --------------------

    #[test]
    fn read_latest_generation_id_errors_on_empty_db_with_actionable_message() {
        let db = migrated_db();
        let err = read_latest_generation_id(db.conn()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("no generations"), "{msg}");
        assert!(msg.contains("nq-monitor serve") || msg.contains("aggregator"), "{msg}");
    }

    #[test]
    fn read_latest_generation_id_returns_most_recent_by_completed_at() {
        let db = migrated_db();
        db.conn()
            .execute(
                "INSERT INTO generations
                   (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (1, '2026-05-19T00:00:00Z', '2026-05-19T00:00:00Z', 'complete', 1, 1, 0, 0),
                        (2, '2026-05-20T00:00:00Z', '2026-05-20T00:00:00Z', 'complete', 1, 1, 0, 0)",
                [],
            )
            .unwrap();
        assert_eq!(read_latest_generation_id(db.conn()).unwrap(), 2);
    }

    // -------------------- record_probe pipeline --------------------

    #[test]
    fn record_probe_writes_a_row_a_mock_client_can_drive() {
        let db = migrated_db();
        seed_generation(db.conn(), 100);
        let client = MockDnsClient::new(WireOutcome::Answer {
            rcode: 0,
            answer_summary: "23.92.30.41".into(),
            min_ttl_seconds: Some(300),
        });
        let written = record_probe(
            db.conn(),
            100,
            "sushi-k",
            "8.8.8.8",
            "nq.neutral.zone",
            "A",
            1,
            Duration::from_secs(5),
            &client,
        )
        .unwrap();
        assert!(written.observation_id.is_some(), "row written with rowid");
        assert_eq!(written.response_kind, ResponseKind::Success);
        assert_eq!(written.answer_summary.as_deref(), Some("23.92.30.41"));
        assert_eq!(written.min_ttl_seconds, Some(300));
        assert_eq!(written.rcode, Some(0));
        assert_eq!(written.vantage_host, "sushi-k");
        assert_eq!(written.resolver, "8.8.8.8");
        assert_eq!(written.query_name, "nq.neutral.zone");
        assert_eq!(written.query_type, "A");
        // The mock saw the inputs exactly.
        let calls = client.recorded.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "8.8.8.8");
        assert_eq!(calls[0].1, "nq.neutral.zone");
        assert_eq!(calls[0].2, 1);
    }

    /// For each WireOutcome variant, drive record_probe with a mock,
    /// then run the dns_state evaluator over the latest row. The
    /// substrate ↔ evaluator contract must hold end-to-end without
    /// any live DNS — this is the closed loop the V0 slice is for.
    #[test]
    fn mock_outcomes_flow_through_to_evaluator_verdicts() {
        let cases: &[(WireOutcome, Verdict)] = &[
            (
                WireOutcome::Answer {
                    rcode: 0,
                    answer_summary: "1.1.1.1".into(),
                    min_ttl_seconds: Some(30),
                },
                Verdict::AdmissibleWithScope,
            ),
            (
                WireOutcome::Negative {
                    kind: NegativeKind::Nodata,
                    rcode: 0,
                },
                Verdict::AdmissibleWithScope,
            ),
            (
                WireOutcome::Negative {
                    kind: NegativeKind::Nxdomain,
                    rcode: 3,
                },
                Verdict::AdmissibleWithScope,
            ),
            (
                WireOutcome::Negative {
                    kind: NegativeKind::Servfail,
                    rcode: 2,
                },
                Verdict::AdmissibleWithScope,
            ),
            (
                WireOutcome::Negative {
                    kind: NegativeKind::Refused,
                    rcode: 5,
                },
                Verdict::AdmissibleWithScope,
            ),
            (WireOutcome::Timeout, Verdict::InsufficientCoverage),
            (
                WireOutcome::TransportError {
                    detail: "connection refused".into(),
                },
                Verdict::CannotTestify,
            ),
        ];

        for (wire, expected_verdict) in cases {
            let db = migrated_db();
            seed_generation(db.conn(), 100);
            let client = MockDnsClient::new(wire.clone());
            let _ = record_probe(
                db.conn(),
                100,
                "sushi-k",
                "8.8.8.8",
                "nq.neutral.zone",
                "A",
                1,
                Duration::from_secs(5),
                &client,
            )
            .unwrap();

            let r = evaluate_dns_state_preflight_from_conn(
                db.conn(),
                &DnsObservationTuple {
                    vantage_host: "sushi-k",
                    resolver: "8.8.8.8",
                    query_name: "nq.neutral.zone",
                    query_type: "A",
                },
            )
            .unwrap();
            assert_eq!(
                r.verdict, *expected_verdict,
                "wire={wire:?} expected verdict {expected_verdict:?} got {:?}",
                r.verdict
            );
        }
    }

    #[test]
    fn record_probe_negative_wire_results_persist_distinct_response_kinds() {
        // The substrate's response_kind column carries the closed
        // taxonomy; the round-trip test confirms the probe writes
        // the distinct kind, not a collapsed "DNS failed" sentinel.
        let db = migrated_db();
        seed_generation(db.conn(), 100);
        let kinds = [
            (NegativeKind::Nodata, ResponseKind::Nodata),
            (NegativeKind::Nxdomain, ResponseKind::Nxdomain),
            (NegativeKind::Servfail, ResponseKind::Servfail),
            (NegativeKind::Refused, ResponseKind::Refused),
        ];
        for (i, (kind, expected)) in kinds.iter().enumerate() {
            let name = format!("name{i}.example");
            let client = MockDnsClient::new(WireOutcome::Negative {
                kind: *kind,
                rcode: 0,
            });
            let obs = record_probe(
                db.conn(),
                100,
                "sushi-k",
                "8.8.8.8",
                &name,
                "A",
                1,
                Duration::from_secs(5),
                &client,
            )
            .unwrap();
            assert_eq!(obs.response_kind, *expected, "{kind:?}");
        }
    }

    // ───────── parse_response vs REAL resolver bytes (lab-backed) ─────────
    // Hex captured from BIND 9.18.49 (docker). Validates the hand-rolled wire
    // decoder against real datagrams — especially the success-vs-nodata split
    // (RCODE 0 with vs without a matching answer). Compatibility evidence, not
    // live testimony. See tests/fixtures/dns/README.md.

    fn from_hex(s: &str) -> Vec<u8> {
        let s = s.trim();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
            .collect()
    }

    const REAL_SUCCESS_A: &str = include_str!("../tests/fixtures/dns/success_a.hex");
    const REAL_NODATA_AAAA: &str = include_str!("../tests/fixtures/dns/nodata_aaaa.hex");
    const REAL_NXDOMAIN_A: &str = include_str!("../tests/fixtures/dns/nxdomain_a.hex");
    const REAL_REFUSED_A: &str = include_str!("../tests/fixtures/dns/refused_a.hex");

    #[test]
    fn parse_response_real_bind_success_decodes_a_record() {
        // qid 0x1234, qtype A(1)
        match parse_response(&from_hex(REAL_SUCCESS_A), 0x1234, 1) {
            WireOutcome::Answer {
                rcode,
                answer_summary,
                min_ttl_seconds,
            } => {
                assert_eq!(rcode, 0);
                assert!(
                    answer_summary.contains("10.1.2.3"),
                    "summary was {answer_summary:?}"
                );
                assert_eq!(min_ttl_seconds, Some(60));
            }
            other => panic!("expected Answer, got {other:?}"),
        }
    }

    #[test]
    fn parse_response_real_bind_nodata_is_not_success() {
        // qid 0x2345, qtype AAAA(28): RCODE 0 but no matching answer. The
        // load-bearing split — a naive RCODE-0-means-success decoder fails here.
        assert_eq!(
            parse_response(&from_hex(REAL_NODATA_AAAA), 0x2345, 28),
            WireOutcome::Negative {
                kind: NegativeKind::Nodata,
                rcode: 0
            }
        );
    }

    #[test]
    fn parse_response_real_bind_nxdomain() {
        assert_eq!(
            parse_response(&from_hex(REAL_NXDOMAIN_A), 0x3456, 1),
            WireOutcome::Negative {
                kind: NegativeKind::Nxdomain,
                rcode: 3
            }
        );
    }

    #[test]
    fn parse_response_real_bind_refused() {
        assert_eq!(
            parse_response(&from_hex(REAL_REFUSED_A), 0x4567, 1),
            WireOutcome::Negative {
                kind: NegativeKind::Refused,
                rcode: 5
            }
        );
    }

    #[test]
    fn parse_response_rejects_id_mismatch_on_real_bytes() {
        // A real success datagram presented with the WRONG expected id is a
        // transport error (possible off-path / stale reply), never an answer.
        match parse_response(&from_hex(REAL_SUCCESS_A), 0x9999, 1) {
            WireOutcome::TransportError { .. } => {}
            other => panic!("expected TransportError on id mismatch, got {other:?}"),
        }
    }
}
