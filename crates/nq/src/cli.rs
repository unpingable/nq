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
