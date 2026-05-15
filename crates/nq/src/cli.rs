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
