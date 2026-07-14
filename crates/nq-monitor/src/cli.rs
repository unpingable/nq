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
    /// Run the aggregator + web UI
    Serve(ServeCmd),
    /// Run a read-only SQL query against the DB
    Query(QueryCmd),
    /// Ask a profile-governed question. Report profiles read existing NQ state;
    /// the bounded TLS-certificate profile acquires only its declared targets.
    Inquire(InquireCmd),
    /// Compile a closed typed utterance into an existing candidate inquiry
    /// plan, clarification, or compiler-local refusal. Executes nothing.
    Intent(IntentCmd),
    /// Explicitly emit an annotation-only escalation request from an existing
    /// sealed inquiry receipt. This does not execute an inquiry or alter a grant.
    EmitEscalation(EmitEscalationCmd),
    /// Run all saved checks against the DB and report results
    Check(CheckCmd),
    /// Run the liveness sentinel — watches NQ's liveness artifact and
    /// alerts on staleness/silence from outside NQ's failure boundary.
    Sentinel(SentinelCmd),
    /// Consumer-facing finding surface (canonical JSON export).
    /// See docs/working/gaps/FINDING_EXPORT_GAP.md.
    Findings(FindingsCmd),
    /// Consumer-facing liveness surface (canonical JSON export of
    /// the sentinel liveness artifact). Single-instance today; the
    /// shape is forward-compatible with a future multi-instance
    /// registry via the `instance_id` field.
    Liveness(LivenessCmd),
    /// Fleet index — comparison surface for declared NQ targets. Reads
    /// each target's liveness artifact via the manifest URL and renders
    /// one row per target. No merged authority, no synthetic fleet
    /// rollup. See `docs/working/gaps/FLEET_INDEX_GAP.md`.
    Fleet(FleetCmd),
    /// Maintenance — declare expected disturbance windows that annotate
    /// findings as `covered` (in window) or `overrun` (window past, finding
    /// persists). Annotation only — never suppresses or hides findings.
    /// See `docs/working/gaps/MAINTENANCE_DECLARATION_GAP.md`.
    Maintenance(MaintenanceCmd),
    /// Source — explicit evidence-retirement verb. `retire` withdraws a
    /// deliberately torn-down source and transitions the findings it backs to
    /// `retired`; `unretire` reverses the current state (to `unknown`, never
    /// auto-`live`) while preserving the audit trail. Retirement is explicit,
    /// never inferred from silence. See `docs/working/gaps/EVIDENCE_RETIREMENT_GAP.md`.
    Source(SourceCmd),
    /// Claim preflight — bounded verdict against existing NQ testimony for a
    /// structured claim kind. V1 supports `disk_state`. NQ testifies; NQ does
    /// not authorize consequence. See `docs/working/decisions/CLAIM_PREFLIGHT.md`.
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
    /// one `dns_observations` row. See `docs/working/gaps/DNS_WITNESS_FAMILY_GAP.md`.
    /// Decoupled from the aggregator publish transaction; the row is
    /// recorded in the latest existing generation context.
    Probe(ProbeCmd),
    /// **D0-Origin (cross-repo bridge to AG, 2026-06-09)**: invoke the
    /// real production evaluator pipeline against a staged sandbox
    /// substrate and emit `FindingSnapshot` JSON stamped with a chosen
    /// `origin_mode` from the closed vocabulary
    /// `{observed, drill, replay, synthetic}` (NQ migration 057).
    ///
    /// This is **not** a synthetic-finding generator. The same
    /// `sqlite_health::collect`, `publish_batch`, `detect::run_all`,
    /// and `update_warning_state_with_origin_mode` code paths the
    /// `serve` loop uses are invoked here in-process. The condition is
    /// operator-staged (smoke machine), but the observation is real
    /// (no fake alarm). See
    /// `~/git/agent_gov/working/nq-custody-gap-origin-discriminator.md`
    /// for the forcing case and
    /// `~/git/agent_gov/working/campaign-standing-before-spendability.md`
    /// §3 D0-Origin for the cross-repo slice spec.
    Drill(DrillCmd),
}

#[derive(Debug, Args)]
pub struct DrillCmd {
    #[command(subcommand)]
    pub action: DrillAction,
}

#[derive(Debug, Subcommand)]
pub enum DrillAction {
    /// Run the WAL-bloat detector against a staged sandbox SQLite
    /// substrate. The detector is the production
    /// `nq_db::detect::detect_wal_bloat` reading
    /// `monitored_dbs_current`; the sandbox is populated via the
    /// production `nq_witness::collect::sqlite_health::collect` reader
    /// followed by `nq_db::publish_batch`. The resulting findings are
    /// stamped with the supplied `origin_mode`, then exported as
    /// `nq.finding_snapshot.v1` JSON on stdout.
    WalBloat(DrillWalBloatCmd),
}

#[derive(Debug, Args)]
pub struct DrillWalBloatCmd {
    /// Path to the sandbox SQLite DB file. The collector also reads the
    /// sibling `<path>-wal` file; the operator is responsible for
    /// ensuring the WAL is bloated (Night Shift's
    /// `wal_bloat_stager.rs` does this in the campaign D0-Origin slice).
    #[arg(long)]
    pub sandbox_db: PathBuf,

    /// Path to NQ's own DB (where `monitored_dbs_current` and
    /// `warning_state` live). A fresh path is fine — the command runs
    /// migrations first. Default: a tmp dir per invocation.
    #[arg(long)]
    pub db: Option<PathBuf>,

    /// Origin-mode discriminator to stamp on every emitted finding's
    /// `warning_state.origin_mode` column. Must be in the closed
    /// vocabulary `{observed, drill, replay, synthetic}` (migration
    /// 057). Default: `drill` because that is the forcing case D0-Origin
    /// landed this command for.
    #[arg(long, default_value = "drill")]
    pub origin_mode: String,

    /// Host label to record on the staged batch. Defaults to a stable
    /// fixture value so the deterministic-transcript invariant on the
    /// AG side does not flap on a real hostname.
    #[arg(long, default_value = "host-drill")]
    pub host: String,

    /// Output format. `jsonl` (default, one snapshot per line) or
    /// `json` (pretty-printed array).
    #[arg(long, short, default_value = "jsonl")]
    pub format: String,
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
    /// Smoke the ingest_state preflight surface. Hits
    /// `<url>/api/preflight/ingest-state` on the running monitor.
    /// The route is not host-scoped — the witness is the monitor's
    /// own pull-cycle substrate. Validates the response against the
    /// `nq.preflight.ingest_state.v1` contract.
    PreflightIngestState(SmokePreflightIngestStateCmd),
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
pub struct SmokePreflightIngestStateCmd {
    /// Base URL of the running monitor (e.g. `http://127.0.0.1:9848`).
    /// Trailing slash is tolerated.
    #[arg(long)]
    pub url: String,
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
    /// Structurally verify an `nq.receipt.v1` document against supplied
    /// witness packets. Does NOT replay the evaluator, re-ratify the
    /// claim, or authorize action.
    ///
    /// A stale receipt is not a forged receipt. A forged receipt is not
    /// a stale receipt.
    Check(ReceiptCheckCmd),
    /// Semantically replay an `nq.receipt.v1` document: re-run a
    /// compatible evaluator against supplied witness material and
    /// compare the semantic decision (verdict, supported claims,
    /// witness set) to the receipt's. Does NOT renew freshness or
    /// authorize action.
    ///
    /// Replay failure is not forgery. Replay success is not fresh
    /// authorization.
    Replay(ReceiptReplayCmd),
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
pub struct ReceiptReplayCmd {
    /// Path to the `nq.receipt.v1` JSON document to replay. Use `-`
    /// for stdin.
    #[arg(long)]
    pub receipt: String,
    /// Path(s) to `nq.witness.v1` packet files. Duplicates (same
    /// digest) are de-duplicated before replay. Track A receipts
    /// ignore supplied packets — replay is not applicable until
    /// Track A cuts over to the shared spine.
    #[arg(long, num_args = 0..)]
    pub witness: Vec<std::path::PathBuf>,
    /// Treat warn-shaped outcomes (duplicate packets, freshness
    /// horizon absent under `--fresh`) as failures. Replay-fatal
    /// statuses are unaffected.
    #[arg(long)]
    pub strict: bool,
    /// Evaluate freshness (orthogonal to replay). A receipt may
    /// replay successfully yet be stale; the freshness axis is
    /// reported independently.
    #[arg(long)]
    pub fresh: bool,
    /// RFC3339 UTC timestamp at which to evaluate freshness. Implies
    /// `--fresh`. Default: wall-clock now.
    #[arg(long)]
    pub as_of: Option<String>,
    /// Emit machine-readable JSON instead of human-readable output.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ReceiptCheckCmd {
    /// Path to the `nq.receipt.v1` JSON document to check. Use `-` for
    /// stdin.
    #[arg(long)]
    pub receipt: String,
    /// Path(s) to `nq.witness.v1` packet files. May be specified
    /// multiple times; matching is by digest, not order. Extra packets
    /// (no matching WitnessRef in the receipt) are reported as warnings
    /// (or failures under `--strict`).
    #[arg(long, num_args = 0..)]
    pub witness: Vec<std::path::PathBuf>,
    /// Treat warn-shaped outcomes (unanchored receipt, unanchored
    /// witness ref, missing witness packet, extra witness packet,
    /// freshness-horizon absent under --fresh) as failures. Broken
    /// integrity is always a failure regardless of this flag.
    #[arg(long)]
    pub strict: bool,
    /// Evaluate freshness: compare `as_of` (default: now) against the
    /// receipt's `freshness_horizon` (when present). Without this flag,
    /// freshness is not consulted.
    #[arg(long)]
    pub fresh: bool,
    /// RFC3339 UTC timestamp at which to evaluate freshness. Implies
    /// `--fresh`. Default: wall-clock now.
    #[arg(long)]
    pub as_of: Option<String>,
    /// Emit machine-readable JSON instead of human-readable output.
    #[arg(long)]
    pub json: bool,
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
    /// `nq-monitor witness pytest -- pytest -q`.
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
pub struct SourceCmd {
    #[command(subcommand)]
    pub action: SourceAction,
}

#[derive(Debug, Subcommand)]
pub enum SourceAction {
    /// Retire a source: record it as deliberately withdrawn and transition
    /// every finding it backs to `retired`, atomically, with a per-finding
    /// audit row. `--reason` is required — explicit retirement carries a why.
    Retire(SourceRetireCmd),
    /// Unretire a source: remove its current-state retirement and return its
    /// findings to `unknown` (never auto-`live`). The retirement audit trail is
    /// preserved — unretire is not retroactive laundering.
    Unretire(SourceUnretireCmd),
}

#[derive(Debug, Args)]
pub struct SourceRetireCmd {
    /// Path to the nq database.
    #[arg(long)]
    pub db: PathBuf,
    /// The source identity to retire (matches findings' `basis_source_id`).
    #[arg(long = "source-id")]
    pub source_id: String,
    /// Why the source is being withdrawn. Required — retirement is explicit.
    #[arg(long)]
    pub reason: String,
}

#[derive(Debug, Args)]
pub struct SourceUnretireCmd {
    /// Path to the nq database.
    #[arg(long)]
    pub db: PathBuf,
    /// The source identity to unretire.
    #[arg(long = "source-id")]
    pub source_id: String,
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
    /// snapshot. See docs/working/gaps/FINDING_EXPORT_GAP.md §"Consumer Semantics"
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
pub struct InquireCmd {
    /// Path to the existing NQ database. Required only by passive report
    /// profiles, opened read-only, and never consulted by active profiles.
    #[arg(long)]
    pub db: Option<PathBuf>,

    /// Path to an nq.inquiry_grant.v0 JSON document. Required at execution
    /// time for active profiles and not applicable to passive report profiles.
    #[arg(long)]
    pub grant: Option<PathBuf>,

    /// Path to an nq.inquiry_plan.v0 JSON document. The plan supplies the
    /// profile selector, as_of intake, and optional exact target subset. L0
    /// report operators may write "latest"; execution freezes it to the newest
    /// generation's completed_at inside the inquiry read snapshot.
    #[arg(long)]
    pub plan: PathBuf,

    /// Path to an nq.inquiry_profile_catalog.v0 JSON document. Alias
    /// resolution occurs in core after the whole catalog is loaded.
    #[arg(long = "profile-catalog")]
    pub profile_catalog: PathBuf,

    /// Render the admitted inquiry envelope and exit without opening a
    /// database or dispatching active acquisition.
    #[arg(long)]
    pub preflight: bool,

    /// Output format: human (operator table) or json (JCS canonical artifact).
    #[arg(long, short, default_value = "human")]
    pub format: String,
}

#[derive(Debug, Args)]
pub struct IntentCmd {
    /// Path to an nq.inquiry_intent.v0 JSON document.
    #[arg(long)]
    pub utterance: PathBuf,

    /// Path to an nq.inquiry_profile_catalog.v0 JSON document.
    #[arg(long = "profile-catalog")]
    pub profile_catalog: PathBuf,

    /// Output format: human or json (JCS canonical resolution artifact).
    #[arg(long, short, default_value = "human")]
    pub format: String,

    /// Write only the resolved nq.inquiry_plan.v0 as canonical JSON. A
    /// clarification or refusal is a hard error when this option is present.
    #[arg(long = "emit-plan")]
    pub emit_plan: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct EmitEscalationCmd {
    /// Path to a sealed nq.inquiry_receipt.v0 JSON document.
    #[arg(long)]
    pub receipt: PathBuf,

    /// Path to the operator's requested successor envelope and receipt-local
    /// citations. The emitted candidate is written as canonical JSON to stdout.
    #[arg(long = "requested-envelope")]
    pub requested_envelope: PathBuf,
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

    /// Probe one TLS surface and print its `nq.probe.tls_cert.v1` receipt
    /// to stdout. Receipt-only — no DB write. The probe OBSERVES the
    /// presented certificate chain (it does not independently validate
    /// trust); the verdict rests on name + the probe clock. `--vantage`
    /// is required and never inferred: a probe must declare where it
    /// stood, and an external vantage is the admissible one.
    TlsCert(ProbeTlsCertCmd),

    /// Read a pfSense DHCP lease over SSH and compare it against presence
    /// (the box's ARP residue, plus an optional probe from `--vantage`),
    /// printing an `nq.probe.lease_presence.v1` receipt. Read-only:
    /// detect-backend + `cat` the lease file + `arp -an`, nothing else. The
    /// specimen is a NON-LIFT — an active lease does not establish presence,
    /// and an uncorroborated lease is not "host down." `--vantage` is
    /// required and never inferred.
    LeasePresence(ProbeLeasePresenceCmd),

    /// Read pfSense's `dpinger` gateway-monitor socket over SSH and compare
    /// its raw metrics against independent path probes from `--vantage` (to
    /// the dpinger monitor IP and a fixed public anchor), printing an
    /// `nq.probe.gateway_path.v1` receipt. Read-only: `ls` + read the status
    /// socket(s), nothing else (NO pfSense PHP classification). The specimen
    /// is a NON-LIFT — a disagreement is path ambiguity, never "WAN down,"
    /// and a missing/mute socket is `cannot_testify`, not gateway-down.
    /// `--vantage` is required and never inferred.
    GatewayPath(ProbeGatewayPathCmd),

    /// Read a declared `block` rule from pfSense's loaded ruleset over SSH and
    /// reconcile it against observation: a CONTROL probe (proving the vantage
    /// has egress) and — only if an explicit benign target is bound — a
    /// SUBJECT probe of the declared-denied path. Read-only (`pfctl -sr -vv`),
    /// receipt-only. This is a declaration-vs-observation CUSTODY test, not a
    /// firewall-correctness test: a got-through is `declared_deny_observed_
    /// reachable`, an unbound subject is `cannot_testify_probe_target_unbound`,
    /// and a missing rule is `cannot_testify_declared_policy_absent` (never
    /// "allowed"). The subject path is NOT probed unless `--subject` is given.
    DeclaredDeny(ProbeDeclaredDenyCmd),
}

#[derive(Debug, Args)]
pub struct ProbeTlsCertCmd {
    /// Host to connect to (e.g. `nq.neutral.zone`).
    #[arg(long)]
    pub host: String,

    /// TCP port.
    #[arg(long, default_value_t = 443)]
    pub port: u16,

    /// SNI / server name for the TLS handshake. Defaults to `--host`.
    #[arg(long)]
    pub sni: Option<String>,

    /// Name the leaf must cover (repeatable). Defaults to `--host`.
    #[arg(long = "expected-name")]
    pub expected_names: Vec<String>,

    /// Days-remaining at or below which the verdict downgrades to the
    /// warning horizon.
    #[arg(long, default_value_t = 14)]
    pub warning_days: i64,

    /// Vantage identity to record on the receipt. Required; NQ does not
    /// infer it. Must NOT be the target's own box for an admissible
    /// external witness.
    #[arg(long)]
    pub vantage: String,

    /// Connect + handshake response horizon, in seconds.
    #[arg(long, default_value_t = 10)]
    pub timeout_seconds: u64,

    /// If set, also append the receipt to a manual append-only series under
    /// this directory (e.g. `runs/tls-cert-probe`), as
    /// `<dir>/<YYYYMMDDTHHMMSSZ>/<host>.json`. stdout still prints the
    /// receipt. This is manual collection, NOT scheduled monitoring — a
    /// missing receipt is not a negative.
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ProbeLeasePresenceCmd {
    /// pfSense host to SSH to (the management address or hostname).
    #[arg(long)]
    pub host: String,

    /// SSH port.
    #[arg(long, default_value_t = 22)]
    pub port: u16,

    /// SSH user (e.g. `admin`).
    #[arg(long)]
    pub user: String,

    /// Path to the SSH private key.
    #[arg(long)]
    pub key: PathBuf,

    /// Vantage identity to record for the OPTIONAL presence probe — the
    /// independent host NQ probes from. Required; NQ does not infer it.
    #[arg(long)]
    pub vantage: String,

    /// The leased IP whose presence to assess against its lease.
    #[arg(long)]
    pub subject: String,

    /// Also run an ICMP-echo presence probe from this host (the vantage).
    #[arg(long, default_value_t = false)]
    pub probe: bool,

    /// Instead of ICMP, probe presence with a TCP connect to this port.
    #[arg(long)]
    pub probe_tcp: Option<u16>,

    /// SSH connect + probe timeout, seconds.
    #[arg(long, default_value_t = 10)]
    pub timeout_seconds: u64,

    /// If set, also append the receipt to a manual append-only series under
    /// this directory (e.g. `runs/lease-presence`). stdout still prints it.
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ProbeGatewayPathCmd {
    /// pfSense host to SSH to (the management address or hostname).
    #[arg(long)]
    pub host: String,

    /// SSH port.
    #[arg(long, default_value_t = 22)]
    pub port: u16,

    /// SSH user (e.g. `admin`).
    #[arg(long)]
    pub user: String,

    /// Path to the SSH private key.
    #[arg(long)]
    pub key: PathBuf,

    /// Vantage identity to record for the independent path probes — the host
    /// NQ probes from. Required; NQ does not infer it.
    #[arg(long)]
    pub vantage: String,

    /// Which dpinger gateway to read (e.g. `WAN_DHCP`). Optional: if exactly
    /// one dpinger socket is present, it is used; required when several exist.
    #[arg(long)]
    pub gateway: Option<String>,

    /// Fixed public egress anchor for the general-reachability observation.
    #[arg(long, default_value = "1.1.1.1")]
    pub anchor: String,

    /// TCP port used ONLY as the fallback path probe when ICMP cannot testify
    /// (could not execute). A completed or refused connect proves the path
    /// reached the host.
    #[arg(long, default_value_t = 443)]
    pub tcp_fallback_port: u16,

    /// SSH connect + probe timeout, seconds.
    #[arg(long, default_value_t = 10)]
    pub timeout_seconds: u64,

    /// If set, also append the receipt to a manual append-only series under
    /// this directory (e.g. `runs/gateway-path`). stdout still prints it.
    #[arg(long)]
    pub out_dir: Option<PathBuf>,

    /// Optional path to an `nq.beacon_status.v0` document (from
    /// `scripts/beacon/beacon-status.sh` on the external vantage). When given,
    /// a combined `nq.probe.gateway_path_combined.v1` report is also emitted,
    /// folding the external-arrival position in as corroboration/divergence —
    /// never as cause, never overriding the LAN-side verdict.
    #[arg(long)]
    pub external_beacon_status: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ProbeDeclaredDenyCmd {
    /// pfSense host to SSH to (the management address or hostname).
    #[arg(long)]
    pub host: String,

    /// SSH port.
    #[arg(long, default_value_t = 22)]
    pub port: u16,

    /// SSH user (e.g. `admin`).
    #[arg(long)]
    pub user: String,

    /// Path to the SSH private key.
    #[arg(long)]
    pub key: PathBuf,

    /// Vantage identity to record for the probes — the host NQ probes from.
    /// Required; NQ does not infer it.
    #[arg(long)]
    pub vantage: String,

    /// Select the declared-deny rule by destination TABLE name (e.g.
    /// `pfB_PRI1_v4`). Exactly one of --table / --ridentifier is required.
    #[arg(long)]
    pub table: Option<String>,

    /// Select the declared-deny rule by `ridentifier`.
    #[arg(long)]
    pub ridentifier: Option<String>,

    /// Control target `host[:port]` — a KNOWN-ALLOWED destination that proves
    /// the vantage has ordinary egress. Without a passing control, a blocked
    /// subject is uninterpretable.
    #[arg(long, default_value = "1.1.1.1:443")]
    pub control: String,

    /// OPTIONAL subject target `host[:port]` — the declared-denied path to
    /// probe. OMITTED BY DEFAULT: the live home firewall must not SYN a
    /// malware-blocklist member. Supply only a benign / operator-owned target
    /// (e.g. on a scratch/lab firewall). When omitted, the subject is left
    /// unbound and the verdict is `cannot_testify_probe_target_unbound`.
    #[arg(long)]
    pub subject: Option<String>,

    /// SSH connect + probe timeout, seconds.
    #[arg(long, default_value_t = 10)]
    pub timeout_seconds: u64,

    /// If set, also append the receipt to a manual append-only series under
    /// this directory (e.g. `runs/declared-deny`). stdout still prints it.
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
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

    /// `nq-monitor probe dns` must accept `--type`, matching the wire-side
    /// `?type=` query-parameter on `/api/preflight/dns-state`. The
    /// struct field is `query_type` only because `type` is a Rust
    /// keyword; the flag must NOT be `--query-type`. Caught by the
    /// 2026-05-20 live smoke when the operator-typed `--type A`
    /// hit a clap "unexpected argument" error.
    #[test]
    fn probe_dns_accepts_type_flag_matching_wire_vocabulary() {
        let cli = Cli::try_parse_from([
            "nq",
            "probe",
            "dns",
            "--db",
            "/tmp/x.db",
            "--vantage",
            "sushi-k",
            "--resolver",
            "8.8.8.8",
            "--name",
            "nq.neutral.zone",
            "--type",
            "AAAA",
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
                other => panic!("expected Probe(Dns(_)), got {other:?}"),
            },
            other => panic!("expected Probe(Dns(_)), got {other:?}"),
        }
    }

    /// The legacy `--query-type` flag must NOT silently coexist with
    /// `--type` — single source of truth at the CLI surface.
    #[test]
    fn probe_dns_rejects_query_type_legacy_flag() {
        let err = Cli::try_parse_from([
            "nq",
            "probe",
            "dns",
            "--db",
            "/tmp/x.db",
            "--vantage",
            "v",
            "--resolver",
            "8.8.8.8",
            "--name",
            "example.com",
            "--query-type",
            "A",
        ])
        .expect_err("--query-type must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("--query-type") || msg.contains("unexpected"),
            "error must name the unexpected flag: {msg}"
        );
    }

    #[test]
    fn inquire_accepts_explicit_passive_db_target() {
        let cli = Cli::try_parse_from([
            "nq",
            "inquire",
            "--db",
            "/tmp/nq.db",
            "--plan",
            "/tmp/plan.v0.json",
            "--profile-catalog",
            "/tmp/profiles.v0.json",
            "--format",
            "json",
        ])
        .expect("nq inquire arguments must parse");
        match cli.command {
            Command::Inquire(cmd) => {
                assert_eq!(cmd.db, Some(PathBuf::from("/tmp/nq.db")));
                assert_eq!(cmd.grant, None);
                assert_eq!(cmd.plan, PathBuf::from("/tmp/plan.v0.json"));
                assert_eq!(cmd.profile_catalog, PathBuf::from("/tmp/profiles.v0.json"));
                assert!(!cmd.preflight);
                assert_eq!(cmd.format, "json");
            }
            other => panic!("expected Inquire, got {other:?}"),
        }
    }

    #[test]
    fn inquire_allows_active_profile_without_db_target() {
        let cli = Cli::try_parse_from([
            "nq",
            "inquire",
            "--plan",
            "/tmp/plan.v0.json",
            "--profile-catalog",
            "/tmp/profiles.v0.json",
            "--grant",
            "/tmp/grant.v0.json",
            "--format",
            "json",
        ])
        .expect("active nq inquire arguments must parse without --db");
        match cli.command {
            Command::Inquire(cmd) => {
                assert_eq!(cmd.db, None);
                assert_eq!(cmd.grant, Some(PathBuf::from("/tmp/grant.v0.json")));
                assert_eq!(cmd.plan, PathBuf::from("/tmp/plan.v0.json"));
                assert_eq!(cmd.profile_catalog, PathBuf::from("/tmp/profiles.v0.json"));
                assert!(!cmd.preflight);
            }
            other => panic!("expected Inquire, got {other:?}"),
        }
    }

    #[test]
    fn inquire_accepts_preflight_mode_with_a_db_target() {
        let cli = Cli::try_parse_from([
            "nq",
            "inquire",
            "--db",
            "/definitely/missing/nq.db",
            "--plan",
            "/tmp/plan.v0.json",
            "--profile-catalog",
            "/tmp/profiles.v0.json",
            "--preflight",
        ])
        .expect("nq inquire --preflight arguments must parse");
        match cli.command {
            Command::Inquire(cmd) => {
                assert!(cmd.preflight);
                assert_eq!(cmd.db, Some(PathBuf::from("/definitely/missing/nq.db")));
                assert_eq!(cmd.grant, None);
            }
            other => panic!("expected Inquire, got {other:?}"),
        }
    }

    #[test]
    fn intent_accepts_only_compiler_artifact_paths() {
        let cli = Cli::try_parse_from([
            "nq",
            "intent",
            "--utterance",
            "/tmp/utterance.v0.json",
            "--profile-catalog",
            "/tmp/profiles.v0.json",
            "--format",
            "json",
            "--emit-plan",
            "/tmp/plan.v0.json",
        ])
        .expect("nq intent arguments must parse");
        match cli.command {
            Command::Intent(cmd) => {
                assert_eq!(cmd.utterance, PathBuf::from("/tmp/utterance.v0.json"));
                assert_eq!(cmd.profile_catalog, PathBuf::from("/tmp/profiles.v0.json"));
                assert_eq!(cmd.format, "json");
                assert_eq!(cmd.emit_plan, Some(PathBuf::from("/tmp/plan.v0.json")));
            }
            other => panic!("expected Intent, got {other:?}"),
        }
    }

    #[test]
    fn intent_rejects_db_and_grant_arguments() {
        for forbidden in ["--db", "--grant"] {
            let err = Cli::try_parse_from([
                "nq",
                "intent",
                "--utterance",
                "/tmp/utterance.v0.json",
                "--profile-catalog",
                "/tmp/profiles.v0.json",
                forbidden,
                "/tmp/forbidden",
            ])
            .expect_err("nq intent must not accept database or grant arguments");
            assert!(
                err.to_string().contains(forbidden),
                "error must name forbidden argument {forbidden}: {err}"
            );
        }
    }

    #[test]
    fn emit_escalation_accepts_explicit_artifact_paths() {
        let cli = Cli::try_parse_from([
            "nq",
            "emit-escalation",
            "--receipt",
            "/tmp/receipt.v0.json",
            "--requested-envelope",
            "/tmp/successor.v0.json",
        ])
        .expect("nq emit-escalation arguments must parse");
        match cli.command {
            Command::EmitEscalation(cmd) => {
                assert_eq!(cmd.receipt, PathBuf::from("/tmp/receipt.v0.json"));
                assert_eq!(
                    cmd.requested_envelope,
                    PathBuf::from("/tmp/successor.v0.json")
                );
            }
            other => panic!("expected EmitEscalation, got {other:?}"),
        }
    }
}
