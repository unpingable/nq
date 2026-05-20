use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "nq", about = "nq: local-first diagnostic monitor")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run a publisher daemon on this host (serves GET /state)
    Publish(PublishCmd),
    /// Run the aggregator + web UI
    Serve(ServeCmd),
    /// Run a read-only SQL query against the DB
    Query(QueryCmd),
    /// Run collectors once and print the JSON payload to stdout
    Collect(CollectCmd),
    /// Run all saved checks against the DB and report results
    Check(CheckCmd),
    /// Run the liveness sentinel — watches NQ's liveness artifact and
    /// alerts on staleness/silence from outside NQ's failure boundary.
    Sentinel(SentinelCmd),
    /// Consumer-facing finding surface (canonical JSON export).
    /// See docs/gaps/FINDING_EXPORT_GAP.md.
    Findings(FindingsCmd),
    /// Consumer-facing liveness surface (canonical JSON export of
    /// the sentinel liveness artifact). Single-instance today; the
    /// shape is forward-compatible with a future multi-instance
    /// registry via the `instance_id` field.
    Liveness(LivenessCmd),
    /// Fleet index — comparison surface for declared NQ targets. Reads
    /// each target's liveness artifact via the manifest URL and renders
    /// one row per target. No merged authority, no synthetic fleet
    /// rollup. See `docs/gaps/FLEET_INDEX_GAP.md`.
    Fleet(FleetCmd),
    /// Maintenance — declare expected disturbance windows that annotate
    /// findings as `covered` (in window) or `overrun` (window past, finding
    /// persists). Annotation only — never suppresses or hides findings.
    /// See `docs/gaps/MAINTENANCE_DECLARATION_GAP.md`.
    Maintenance(MaintenanceCmd),
    /// Claim preflight — bounded verdict against existing NQ testimony for a
    /// structured claim kind. V1 supports `disk_state`. NQ testifies; NQ does
    /// not authorize consequence. See `docs/CLAIM_PREFLIGHT.md`.
    Preflight(PreflightCmd),
    /// Validate a caller-supplied witness packet (`nq.witness.v1`).
    /// Reads a JSON file, checks the envelope, and reports problems.
    /// See `docs/architecture/SHARED_SPINE.md`.
    ValidateWitness(ValidateWitnessCmd),
    /// Verify a claim against caller-supplied witness packets.
    /// Reads `nq.witness.v1` files, evaluates against the registered
    /// claim, and emits an `nq.receipt.v1`. Default posture is
    /// informational; blocking modes live behind `--strict` /
    /// `--fail-on STATUS`. See `docs/architecture/SHARED_SPINE.md`.
    Verify(VerifyCmd),
    /// Produce a witness packet from a local source (git, pytest, ...).
    /// Writes `nq.witness.v1` JSON to stdout by default. Witnesses
    /// report observations; they do not name claims.
    Witness(WitnessCmd),
    /// Render an existing `nq.receipt.v1` document in a chosen format
    /// (human, json, markdown). Useful for posting receipts to
    /// downstream consumers (PR comments, dashboards) without
    /// re-running verification.
    Receipt(ReceiptCmd),
    /// Operator-facing contract smokes against a running monitor's
    /// HTTP API. Verifies the operator-facing surface honors its
    /// contract; safe to run repeatedly against production. Exits
    /// zero on contract success regardless of the verdict NQ minted;
    /// honest refusals (`cannot_testify`, `contradictory_testimony`)
    /// are not smoke failures.
    Smoke(SmokeCmd),
    /// Probe an external substrate and write the observation into NQ's
    /// DB. V0 supports `dns` — one DNS query per invocation, writing
    /// one `dns_observations` row. See `docs/gaps/DNS_WITNESS_FAMILY_GAP.md`.
    /// Decoupled from the aggregator publish transaction; the row is
    /// recorded in the latest existing generation context.
    Probe(ProbeCmd),
}

#[derive(Debug, Args)]
pub struct SmokeCmd {
    #[command(subcommand)]
    pub action: SmokeAction,
}

#[derive(Debug, Subcommand)]
pub enum SmokeAction {
    /// Smoke the disk_state preflight surface for a host. Hits
    /// `<url>/api/host/<host>` on the running monitor, parses the
    /// nested `disk_state_preflight` envelope, and validates it
    /// against the `nq.preflight.disk_state.v1` contract.
    PreflightDiskState(SmokePreflightDiskStateCmd),
}

#[derive(Debug, Args)]
pub struct SmokePreflightDiskStateCmd {
    /// Base URL of the running monitor (e.g. `http://127.0.0.1:9848`).
    /// Trailing slash is tolerated.
    #[arg(long)]
    pub url: String,
    /// Host the preflight is for. The endpoint hit is
    /// `<url>/api/host/<host>`.
    #[arg(long)]
    pub host: String,
    /// Per-request timeout in seconds.
    #[arg(long, default_value_t = 5)]
    pub timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub struct ReceiptCmd {
    #[command(subcommand)]
    pub action: ReceiptAction,
}

#[derive(Debug, Subcommand)]
pub enum ReceiptAction {
    /// Render an `nq.receipt.v1` document. Reads from a file (or `-`
    /// for stdin) and writes to stdout.
    Render(ReceiptRenderCmd),
}

#[derive(Debug, Args)]
pub struct ReceiptRenderCmd {
    /// Path to an `nq.receipt.v1` JSON document. Pass `-` to read from
    /// stdin.
    pub path: String,
    /// Output format. `human` is the terminal rendering, `markdown` is
    /// GitHub-flavored (suitable for PR comments), `json`/`jsonl` are
    /// passthrough.
    #[arg(long, short, default_value = "human")]
    pub format: String,
}

#[derive(Debug, Args)]
pub struct ValidateWitnessCmd {
    /// Path to a witness packet JSON file. Pass `-` to read from stdin.
    pub path: String,
}

#[derive(Debug, Args)]
pub struct VerifyCmd {
    /// Claim to verify. Must be registered in the claim catalog. Use
    /// `--list-claims` to see what is available.
    #[arg(long)]
    pub claim: String,
    /// Subject the claim is about (e.g. `repo:.`, `host:storage01`).
    /// Witnesses whose `subject` does not match are ignored.
    #[arg(long)]
    pub subject: String,
    /// Witness packet file(s). Accepts multiple values; shell expansion
    /// of globs works (e.g. `--witness .nq/*.json`).
    #[arg(long, num_args = 1.., required = true)]
    pub witness: Vec<PathBuf>,
    /// Optional path to write the receipt JSON to (in addition to the
    /// rendered output sent to stdout).
    #[arg(long)]
    pub receipt: Option<PathBuf>,
    /// Output format. `human` is the terminal rendering. `json`/`jsonl`
    /// emit `nq.receipt.v1` to stdout.
    #[arg(long, short, default_value = "human")]
    pub format: String,
    /// Treat any status other than `verified` as a failure (exit 1).
    /// Default posture is informational: receipt is emitted, exit 0
    /// unless input is malformed.
    #[arg(long)]
    pub strict: bool,
    /// Treat the given status as a failure (exit 1). May be passed
    /// multiple times. Example: `--fail-on not_verified --fail-on
    /// needs_more_evidence`. Ignored if `--strict` is set.
    #[arg(long)]
    pub fail_on: Vec<String>,
}

#[derive(Debug, Args)]
pub struct WitnessCmd {
    #[command(subcommand)]
    pub action: WitnessAction,
}

#[derive(Debug, Subcommand)]
pub enum WitnessAction {
    /// Observe the local git working tree and emit a `git_status`
    /// witness packet. Runs `git status --porcelain` and
    /// `git rev-parse HEAD` in the current directory.
    GitStatus(WitnessGitStatusCmd),
    /// Run an external test command and emit a `pytest` witness packet
    /// recording the exit code. Pass the command after `--`, e.g.
    /// `nq witness pytest -- pytest -q`.
    Pytest(WitnessPytestCmd),
    /// Observe the file paths changed by a git diff and classify them
    /// against a declared scope. Emits a `diff_scope` witness packet.
    DiffScope(WitnessDiffScopeCmd),
}

#[derive(Debug, Args)]
pub struct WitnessGitStatusCmd {
    /// Subject name to record. Defaults to `repo:.` (current directory).
    #[arg(long, default_value = "repo:.")]
    pub subject: String,
    /// Working directory in which to run git. Defaults to the current
    /// process directory.
    #[arg(long)]
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct WitnessPytestCmd {
    /// Subject name to record. Defaults to `repo:.`.
    #[arg(long, default_value = "repo:.")]
    pub subject: String,
    /// Working directory to run the command in.
    #[arg(long)]
    pub cwd: Option<PathBuf>,
    /// The test command and its arguments, passed after `--`. If
    /// omitted, defaults to `pytest`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Args)]
pub struct WitnessDiffScopeCmd {
    /// Subject name to record. Defaults to `repo:.`.
    #[arg(long, default_value = "repo:.")]
    pub subject: String,
    /// Working directory to run git in.
    #[arg(long)]
    pub cwd: Option<PathBuf>,
    /// The scope the diff is declared to fall within. Phase 2 supports
    /// `docs-only`; additional scopes land as needed.
    #[arg(long)]
    pub declared: String,
    /// Diff base. If omitted, the producer tries `origin/main`,
    /// `origin/master`, `main`, `master` in order, and errors if none
    /// resolve.
    #[arg(long)]
    pub base: Option<String>,
}

#[derive(Debug, Args)]
pub struct PreflightCmd {
    #[command(subcommand)]
    pub action: PreflightAction,
}

#[derive(Debug, Subcommand)]
pub enum PreflightAction {
    /// Preflight a `disk_state` claim against existing ZFS + SMART +
    /// disk-pressure findings for a host (optionally narrowed to a pool,
    /// vdev, or device).
    DiskState(PreflightDiskStateCmd),
}

#[derive(Debug, Args)]
pub struct PreflightDiskStateCmd {
    /// Path to the nq database.
    #[arg(long)]
    pub db: PathBuf,

    /// Host the claim is about. Exact match.
    #[arg(long)]
    pub host: String,

    /// Optional subject to narrow the preflight: a pool name (e.g. `tank`),
    /// vdev identity (e.g. `tank/raidz2-0/ata-X`), or device path
    /// (`/dev/sdX`). When omitted the preflight covers the host.
    #[arg(long)]
    pub target: Option<String>,

    /// Output format. `human` is a terminal-friendly receipt rendering.
    /// `json` and `jsonl` emit an `nq.receipt.v1` document; `jsonl` is a
    /// single line for log streams. See `docs/architecture/SHARED_SPINE.md`.
    #[arg(long, short, default_value = "human")]
    pub format: String,
}

#[derive(Debug, Args)]
pub struct MaintenanceCmd {
    #[command(subcommand)]
    pub action: MaintenanceAction,
}

#[derive(Debug, Subcommand)]
pub enum MaintenanceAction {
    /// Declare a maintenance window. The CLI rejects past-dated `--start`
    /// per the V1 invariant: declaration must precede effect.
    Declare(MaintenanceDeclareCmd),
    /// List maintenance declarations. Default lists all rows; pass
    /// `--active` to restrict to declarations currently in window.
    List(MaintenanceListCmd),
}

#[derive(Debug, Args)]
pub struct MaintenanceDeclareCmd {
    /// Path to the nq database.
    #[arg(long)]
    pub db: PathBuf,
    /// Host the maintenance applies to. Exact match.
    #[arg(long)]
    pub host: String,
    /// Detector kind covered by the window. Exact match (e.g.
    /// `log_silence`, `service_status`).
    #[arg(long)]
    pub kind: String,
    /// Specific subject within the host+kind (e.g. service name, db path).
    /// Omit for a host+kind wildcard covering any subject.
    #[arg(long)]
    pub subject: Option<String>,
    /// Window start. Accepts ISO-8601 (`2026-05-08T18:00:00Z`), `now`, or
    /// `now+30m` / `now+1h` / `now+2d`. Must be `>= now` — past-dated
    /// starts are rejected (declaration precedes effect).
    #[arg(long)]
    pub start: String,
    /// Window end. Same parsing rules as `--start`. Must be after `--start`.
    #[arg(long)]
    pub end: String,
    /// Free-text reason ("VACUUM labelwatch sqlite stores post-unblock").
    #[arg(long)]
    pub reason: Option<String>,
    /// Operator or agent name declaring the window.
    #[arg(long = "declared-by")]
    pub declared_by: Option<String>,
}

#[derive(Debug, Args)]
pub struct MaintenanceListCmd {
    /// Path to the nq database.
    #[arg(long)]
    pub db: PathBuf,
    /// Restrict to declarations currently in window (start_at <= now <= end_at).
    /// Default: list all rows.
    #[arg(long)]
    pub active: bool,
}

#[derive(Debug, Args)]
pub struct FleetCmd {
    #[command(subcommand)]
    pub action: FleetAction,
}

#[derive(Debug, Subcommand)]
pub enum FleetAction {
    /// Render the fleet status table from the configured manifest.
    Status(FleetStatusCmd),
}

#[derive(Debug, Args)]
pub struct FleetStatusCmd {
    /// Path to the fleet manifest JSON (typically
    /// `~/.config/nq-fleet/targets.json`).
    #[arg(long, default_value = "~/.config/nq-fleet/targets.json", value_parser = expand_tilde)]
    pub manifest: std::path::PathBuf,

    /// Output format: `table` (default, operator-readable) or `json`
    /// (machine-readable, jsonpath-friendly).
    #[arg(long, short, default_value = "table")]
    pub format: String,

    /// Bounded per-target read timeout in seconds. One slow target
    /// must not block the others; this caps how long any single read
    /// can run.
    #[arg(long, default_value_t = 5)]
    pub timeout_seconds: u64,
}

fn expand_tilde(s: &str) -> Result<std::path::PathBuf, String> {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(std::path::PathBuf::from(home).join(rest));
        }
    }
    Ok(std::path::PathBuf::from(s))
}

#[derive(Debug, Args)]
pub struct FindingsCmd {
    #[command(subcommand)]
    pub action: FindingsAction,
}

#[derive(Debug, Subcommand)]
pub enum FindingsAction {
    /// Export finding snapshots as canonical JSON.
    ///
    /// Export is evidence, not authority. A `FindingSnapshot` is admissible
    /// evidence for downstream reconciliation, not an authorization token —
    /// consumers must reconcile against current state before acting on any
    /// snapshot. See docs/gaps/FINDING_EXPORT_GAP.md §"Consumer Semantics"
    /// for the full discipline.
    Export(FindingsExportCmd),
}

#[derive(Debug, Args)]
pub struct FindingsExportCmd {
    /// Path to the nq database.
    #[arg(long)]
    pub db: PathBuf,

    /// Output format: `jsonl` (default, one FindingSnapshot per line —
    /// streaming-friendly) or `json` (pretty-printed array).
    #[arg(long, short, default_value = "jsonl")]
    pub format: String,

    /// Return only findings whose `last_seen_gen` exceeds this value.
    /// Consumers maintain a watermark and fetch deltas via this flag.
    #[arg(long)]
    pub changed_since_generation: Option<i64>,

    /// Restrict to a specific detector kind (e.g. `wal_bloat`).
    #[arg(long)]
    pub detector: Option<String>,

    /// Restrict to a specific host.
    #[arg(long)]
    pub host: Option<String>,

    /// Exact-match on the canonical finding_key. Wins over other filters.
    #[arg(long)]
    pub finding_key: Option<String>,

    /// Include cleared findings (default: false).
    #[arg(long, default_value_t = false)]
    pub include_cleared: bool,

    /// Include suppressed findings (default: false).
    #[arg(long, default_value_t = false)]
    pub include_suppressed: bool,

    /// Maximum observations to embed per snapshot (default: 10).
    #[arg(long, default_value_t = 10)]
    pub observations_limit: usize,
}

#[derive(Debug, Args)]
pub struct LivenessCmd {
    #[command(subcommand)]
    pub action: LivenessAction,
}

#[derive(Debug, Subcommand)]
pub enum LivenessAction {
    /// Export the liveness artifact as a canonical `LivenessSnapshot`.
    ///
    /// Output is admissible evidence about whether this NQ instance is
    /// still producing generations. It does not authorize the consumer
    /// to decide the observed system is dead — only that this witness
    /// has (or hasn't) reported recently.
    Export(LivenessExportCmd),
}

#[derive(Debug, Args)]
pub struct LivenessExportCmd {
    /// Path to the liveness artifact file (typically `liveness.json`
    /// alongside the NQ database, per the aggregator config).
    #[arg(long)]
    pub artifact: PathBuf,

    /// Output format: `jsonl` (default, one-line compact) or `json`
    /// (pretty-printed).
    #[arg(long, short, default_value = "jsonl")]
    pub format: String,

    /// Staleness threshold in seconds. When provided, the snapshot's
    /// `freshness.fresh` field reflects the verdict. When omitted,
    /// `fresh` is null and the caller must apply their own policy.
    #[arg(long)]
    pub stale_threshold_seconds: Option<i64>,
}

#[derive(Debug, Args)]
pub struct PublishCmd {
    /// Path to publisher config file
    #[arg(long, short)]
    pub config: PathBuf,
}

#[derive(Debug, Args)]
pub struct ServeCmd {
    /// Path to aggregator config file
    #[arg(long, short)]
    pub config: PathBuf,

    /// Bind only the HTTP server. Skip source pull, publish_batch,
    /// detector runs, notifications, and liveness updates. The DB is
    /// opened read-only. Use for safe live preflight smoke against a
    /// running monitor's DB.
    #[arg(long)]
    pub http_only: bool,
}

#[derive(Debug, Args)]
pub struct QueryCmd {
    /// Path to the nq database (local mode)
    #[arg(long, group = "target")]
    pub db: Option<PathBuf>,

    /// Remote nq server URL (e.g., http://host:9848)
    #[arg(long, group = "target")]
    pub remote: Option<String>,

    /// SQL query to execute
    pub sql: String,

    /// Maximum rows to return
    #[arg(long, default_value_t = 500)]
    pub limit: usize,

    /// Output format: table, json, csv
    #[arg(long, short, default_value = "table")]
    pub format: String,
}

#[derive(Debug, Args)]
pub struct CollectCmd {
    /// Path to publisher config file
    #[arg(long, short)]
    pub config: PathBuf,
}

#[derive(Debug, Args)]
pub struct CheckCmd {
    /// Path to the nq database
    #[arg(long)]
    pub db: PathBuf,
}

#[derive(Debug, Args)]
pub struct SentinelCmd {
    /// Path to sentinel config file (JSON)
    #[arg(long, short)]
    pub config: PathBuf,
}

#[derive(Debug, Args)]
pub struct ProbeCmd {
    #[command(subcommand)]
    pub action: ProbeAction,
}

#[derive(Debug, Subcommand)]
pub enum ProbeAction {
    /// Probe one DNS tuple and write a `dns_observations` row. One
    /// query per invocation. The `--vantage` flag is required and
    /// never inferred — NQ does not guess what vantage it is running
    /// on.
    Dns(ProbeDnsCmd),
}

#[derive(Debug, Args)]
pub struct ProbeDnsCmd {
    /// Path to the nq database.
    #[arg(long)]
    pub db: PathBuf,

    /// Vantage host to record on the observation. Required; NQ does
    /// not infer this from `gethostname()` or similar — `dns_state`
    /// testimony is scoped to who asked, and "who" must be declared.
    #[arg(long)]
    pub vantage: String,

    /// Resolver IP literal. IPv4 (`8.8.8.8`), IPv4:port (`8.8.8.8:53`),
    /// IPv6 (`2001:4860:4860::8888`), or [IPv6]:port. Hostname
    /// resolvers are rejected — resolving the resolver's name would
    /// force a recursive lookup on the same vantage.
    #[arg(long)]
    pub resolver: String,

    /// Query name (e.g. `nq.neutral.zone`). Trailing dot tolerated.
    #[arg(long)]
    pub name: String,

    /// Query type. V0 accepts: A, AAAA, NS, CNAME, MX, TXT, SOA, PTR,
    /// SRV. Defaults to A. Flag name is `--type` (the field is
    /// `query_type` only because `type` is a Rust keyword); this
    /// matches the wire-side `?type=` query parameter on the HTTP
    /// route, so operators only have to learn one vocabulary.
    #[arg(long = "type", default_value = "A")]
    pub query_type: String,

    /// Per-query UDP read timeout in seconds.
    #[arg(long, default_value_t = 5)]
    pub timeout_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// `nq probe dns` must accept `--type`, matching the wire-side
    /// `?type=` query-parameter on `/api/preflight/dns-state`. The
    /// struct field is `query_type` only because `type` is a Rust
    /// keyword; the flag must NOT be `--query-type`. Caught by the
    /// 2026-05-20 live smoke when the operator-typed `--type A`
    /// hit a clap "unexpected argument" error.
    #[test]
    fn probe_dns_accepts_type_flag_matching_wire_vocabulary() {
        let cli = Cli::try_parse_from([
            "nq", "probe", "dns",
            "--db", "/tmp/x.db",
            "--vantage", "sushi-k",
            "--resolver", "8.8.8.8",
            "--name", "nq.neutral.zone",
            "--type", "AAAA",
        ])
        .expect("--type must parse");
        match cli.command {
            Command::Probe(p) => match p.action {
                ProbeAction::Dns(d) => {
                    assert_eq!(d.query_type, "AAAA");
                    assert_eq!(d.vantage, "sushi-k");
                    assert_eq!(d.resolver, "8.8.8.8");
                    assert_eq!(d.name, "nq.neutral.zone");
                }
            },
            other => panic!("expected Probe(Dns(_)), got {other:?}"),
        }
    }

    /// The legacy `--query-type` flag must NOT silently coexist with
    /// `--type` — single source of truth at the CLI surface.
    #[test]
    fn probe_dns_rejects_query_type_legacy_flag() {
        let err = Cli::try_parse_from([
            "nq", "probe", "dns",
            "--db", "/tmp/x.db",
            "--vantage", "v",
            "--resolver", "8.8.8.8",
            "--name", "example.com",
            "--query-type", "A",
        ])
        .expect_err("--query-type must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("--query-type") || msg.contains("unexpected"),
            "error must name the unexpected flag: {msg}"
        );
    }
}
