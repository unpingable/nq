use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "nq", about = "notquery: operational workbench")]
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
    /// Path to the notquery database (local mode)
    #[arg(long, group = "target")]
    pub db: Option<PathBuf>,

    /// Remote notquery server URL (e.g., http://host:9848)
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
