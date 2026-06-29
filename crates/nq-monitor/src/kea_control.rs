//! Kea DHCP control-socket lease backend — a SIBLING of the memfile reader
//! (`lease_presence_transport::parse_kea_leases`). It reaches the same
//! [`KeaLease`] shape via the Kea control socket's `lease4-get-all` command
//! instead of the `kea-leases4.csv` file, so it feeds the unchanged
//! lease-presence verdict core. No new abstraction — same struct, second source.
//!
//! Surface captured from a real `kea-dhcp4` 2.2.0 instance (`lease_cmds` hook):
//! `lease4-get-all` returns `{ "result": 0, "arguments": { "leases": [ ... ] } }`
//! where each lease carries `ip-address`, `hw-address`, `hostname`, `state`,
//! `cltt`, `valid-lft`. Absolute expiry is `cltt + valid-lft` (the memfile's
//! `expire`), so the two backends agree for the same lease.
//!
//! Lab-backed compatibility, not live testimony: real Kea integration is gated
//! behind `--ignored` (`NQ_KEA_CTRL_SOCKET`); the default tests use a fake
//! in-process unix-socket server.

use crate::lease_presence_transport::KeaLease;
use serde::Deserialize;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

/// Failure modes of a control-socket lease read. Each is distinct so the caller
/// (and a future receipt) can say which boundary failed — a missing socket is
/// not a refused connection is not an unsupported command.
#[derive(Debug)]
pub enum KeaControlError {
    /// The socket path does not exist (Kea not running / wrong path).
    SocketMissing,
    /// The path exists but nothing is accepting (Kea down).
    ConnectionRefused,
    /// No complete response within the timeout.
    Timeout,
    /// Other transport/IO failure.
    Io(String),
    /// Response was not parseable JSON, or truncated.
    MalformedResponse(String),
    /// `lease_cmds` hook not loaded — `lease4-get-all` is `result: 2`.
    UnsupportedCommand,
    /// Any other non-zero Kea `result` code (1 = error, etc.).
    KeaResult { code: i64, text: String },
}

impl std::fmt::Display for KeaControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SocketMissing => write!(f, "kea control socket missing"),
            Self::ConnectionRefused => write!(f, "kea control socket refused connection"),
            Self::Timeout => write!(f, "kea control socket timed out"),
            Self::Io(d) => write!(f, "kea control socket io: {d}"),
            Self::MalformedResponse(d) => write!(f, "kea malformed response: {d}"),
            Self::UnsupportedCommand => write!(f, "kea lease4-get-all unsupported (lease_cmds hook not loaded)"),
            Self::KeaResult { code, text } => write!(f, "kea result {code}: {text}"),
        }
    }
}
impl std::error::Error for KeaControlError {}

// --- wire shapes (only the fields this backend consumes) ---

#[derive(Deserialize)]
struct Lease4GetAllResponse {
    result: i64,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    arguments: Option<Lease4Args>,
}

#[derive(Deserialize)]
struct Lease4Args {
    #[serde(default)]
    leases: Vec<ApiLease>,
}

#[derive(Deserialize)]
struct ApiLease {
    #[serde(rename = "ip-address")]
    ip_address: String,
    #[serde(rename = "hw-address", default)]
    hw_address: Option<String>,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    state: Option<u8>,
    #[serde(default)]
    cltt: Option<i64>,
    #[serde(rename = "valid-lft", default)]
    valid_lft: Option<i64>,
}

/// The exact command this backend issues (all subnets).
pub const LEASE4_GET_ALL_COMMAND: &str = r#"{"command":"lease4-get-all"}"#;

/// Pure parser: a `lease4-get-all` JSON response → `Vec<KeaLease>`. Maps Kea
/// result codes to typed errors (2 → unsupported, other non-zero → KeaResult;
/// 3 = "empty" is success with no leases). `expire = cltt + valid-lft`.
pub fn parse_lease4_get_all(json: &str) -> Result<Vec<KeaLease>, KeaControlError> {
    let resp: Lease4GetAllResponse = serde_json::from_str(json)
        .map_err(|e| KeaControlError::MalformedResponse(e.to_string()))?;

    match resp.result {
        0 | 3 => {} // 0 = success, 3 = empty (no leases) — both not errors
        2 => return Err(KeaControlError::UnsupportedCommand),
        code => {
            return Err(KeaControlError::KeaResult {
                code,
                text: resp.text.unwrap_or_default(),
            })
        }
    }

    let leases = resp.arguments.map(|a| a.leases).unwrap_or_default();
    Ok(leases
        .into_iter()
        .map(|l| KeaLease {
            ip: l.ip_address,
            mac: l.hw_address.filter(|s| !s.is_empty()),
            expire: match (l.cltt, l.valid_lft) {
                (Some(c), Some(v)) => Some(c + v),
                _ => None,
            },
            state: l.state,
            hostname: l.hostname.filter(|s| !s.is_empty()),
        })
        .collect())
}

/// Read current leases over the Kea control socket. Connects, sends
/// `lease4-get-all`, reads the response within `timeout`, and parses it. Errors
/// are typed per boundary (missing / refused / timeout / malformed / kea-result).
pub fn fetch_leases_via_control_socket(
    socket_path: &Path,
    timeout: Duration,
) -> Result<Vec<KeaLease>, KeaControlError> {
    if !socket_path.exists() {
        return Err(KeaControlError::SocketMissing);
    }
    let mut stream = match UnixStream::connect(socket_path) {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
            return Err(KeaControlError::ConnectionRefused)
        }
        Err(e) => return Err(KeaControlError::Io(e.to_string())),
    };
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|_| stream.set_write_timeout(Some(timeout)))
        .map_err(|e| KeaControlError::Io(e.to_string()))?;
    stream
        .write_all(LEASE4_GET_ALL_COMMAND.as_bytes())
        .map_err(|e| KeaControlError::Io(e.to_string()))?;

    // Read until a complete JSON object parses, EOF, or timeout. Kea may keep
    // the connection open after the response, so we cannot rely on EOF alone.
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break, // EOF
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                match serde_json::from_slice::<serde::de::IgnoredAny>(&buf) {
                    Ok(_) => break,                 // a complete JSON value is present
                    Err(e) if e.is_eof() => continue, // incomplete — read more
                    Err(e) => return Err(KeaControlError::MalformedResponse(e.to_string())),
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                return Err(KeaControlError::Timeout)
            }
            Err(e) => return Err(KeaControlError::Io(e.to_string())),
        }
    }
    if buf.is_empty() {
        return Err(KeaControlError::MalformedResponse("empty response".into()));
    }
    let text = String::from_utf8_lossy(&buf);
    parse_lease4_get_all(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;

    const REAL_GET_ALL: &str = include_str!("../tests/fixtures/kea/lease4_get_all.json");

    // ---- pure parser (against the real captured response + error shapes) ----

    #[test]
    fn parses_real_lease4_get_all_to_kea_leases() {
        let leases = parse_lease4_get_all(REAL_GET_ALL).expect("parse");
        assert_eq!(leases.len(), 3);
        let a = leases.iter().find(|l| l.ip == "10.99.0.100").unwrap();
        assert_eq!(a.mac.as_deref(), Some("08:00:27:aa:bb:01"));
        assert_eq!(a.hostname.as_deref(), Some("lab-host-active"));
        assert_eq!(a.state, Some(0));
        // expire = cltt(1782746554) + valid-lft(3600) = 1782750154 — same as memfile.
        assert_eq!(a.expire, Some(1782750154));
        let declined = leases.iter().find(|l| l.ip == "10.99.0.102").unwrap();
        assert_eq!(declined.state, Some(1));
    }

    #[test]
    fn control_socket_and_memfile_agree_for_same_leases() {
        // Cross-backend consistency: the API response and the memfile fixture
        // describe the same three leases and must produce identical KeaLease values.
        let api = parse_lease4_get_all(REAL_GET_ALL).unwrap();
        let memfile = crate::lease_presence_transport::parse_kea_leases(include_str!(
            "../tests/fixtures/kea/kea-leases4.csv"
        ));
        let key = |l: &KeaLease| (l.ip.clone(), l.mac.clone(), l.expire, l.state, l.hostname.clone());
        let mut a: Vec<_> = api.iter().map(key).collect();
        let mut m: Vec<_> = memfile.iter().map(key).collect();
        a.sort();
        m.sort();
        assert_eq!(a, m);
    }

    #[test]
    fn unsupported_command_result_2() {
        let j = r#"{ "result": 2, "text": "'lease4-get-all' command not supported." }"#;
        assert!(matches!(
            parse_lease4_get_all(j),
            Err(KeaControlError::UnsupportedCommand)
        ));
    }

    #[test]
    fn nonzero_result_is_kea_result_error() {
        let j = r#"{ "result": 1, "text": "boom" }"#;
        match parse_lease4_get_all(j) {
            Err(KeaControlError::KeaResult { code, text }) => {
                assert_eq!(code, 1);
                assert_eq!(text, "boom");
            }
            other => panic!("expected KeaResult, got {other:?}"),
        }
    }

    #[test]
    fn empty_result_3_is_no_leases_not_error() {
        let j = r#"{ "result": 3, "text": "0 IPv4 lease(s) found." }"#;
        assert_eq!(parse_lease4_get_all(j).unwrap().len(), 0);
    }

    #[test]
    fn malformed_json_is_malformed_response() {
        assert!(matches!(
            parse_lease4_get_all("not json"),
            Err(KeaControlError::MalformedResponse(_))
        ));
    }

    // ---- fake control socket (no real Kea) ----

    /// Spawn a one-shot fake Kea control socket that returns `response` and
    /// closes. Returns the socket path (in a temp dir kept alive by the caller).
    fn fake_socket(dir: &Path, response: &'static str) -> std::path::PathBuf {
        let path = dir.join("kea-ctrl.sock");
        let listener = UnixListener::bind(&path).expect("bind fake socket");
        thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                let mut scratch = [0u8; 1024];
                let _ = conn.read(&mut scratch); // drain the command
                let _ = conn.write_all(response.as_bytes());
                // drop -> close, signalling EOF to the reader
            }
        });
        path
    }

    #[test]
    fn fetch_over_fake_socket_returns_leases() {
        let dir = tempfile::tempdir().unwrap();
        let path = fake_socket(dir.path(), REAL_GET_ALL);
        let leases =
            fetch_leases_via_control_socket(&path, Duration::from_secs(2)).expect("fetch");
        assert_eq!(leases.len(), 3);
    }

    #[test]
    fn fetch_missing_socket_is_socket_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.sock");
        assert!(matches!(
            fetch_leases_via_control_socket(&path, Duration::from_secs(1)),
            Err(KeaControlError::SocketMissing)
        ));
    }

    #[test]
    fn fetch_malformed_over_fake_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = fake_socket(dir.path(), "{ this is not valid json");
        assert!(matches!(
            fetch_leases_via_control_socket(&path, Duration::from_secs(2)),
            Err(KeaControlError::MalformedResponse(_))
        ));
    }

    #[test]
    fn fetch_times_out_when_server_never_responds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kea-silent.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let _server = thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                let mut scratch = [0u8; 1024];
                let _ = conn.read(&mut scratch); // read command, then never reply
                thread::sleep(Duration::from_secs(2));
            }
        });
        assert!(matches!(
            fetch_leases_via_control_socket(&path, Duration::from_millis(200)),
            Err(KeaControlError::Timeout)
        ));
    }

    #[test]
    fn fetch_refused_when_socket_file_has_no_listener() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kea-stale.sock");
        // Bind then drop: the socket file persists, but nothing accepts.
        let listener = UnixListener::bind(&path).unwrap();
        drop(listener);
        assert!(matches!(
            fetch_leases_via_control_socket(&path, Duration::from_secs(1)),
            Err(KeaControlError::ConnectionRefused)
        ));
    }

    /// Real-Kea integration, gated. Set `NQ_KEA_CTRL_SOCKET=/run/kea/kea4-ctrl-socket`
    /// and run with `--ignored` against a live Kea with the lease_cmds hook.
    #[test]
    #[ignore]
    fn real_kea_control_socket_gated() {
        let Ok(sock) = std::env::var("NQ_KEA_CTRL_SOCKET") else {
            return;
        };
        let leases =
            fetch_leases_via_control_socket(Path::new(&sock), Duration::from_secs(5)).expect("live kea");
        println!("live kea returned {} leases", leases.len());
    }
}
