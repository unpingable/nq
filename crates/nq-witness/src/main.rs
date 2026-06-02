//! `nq-witness` binary entry point. Reads a `PublisherConfig` from a
//! JSON file and serves the witness's `/state` HTTP endpoint until
//! killed. One config, one HTTP server, no subcommands.

use clap::Parser;
use nq_core::PublisherConfig;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "nq-witness", about = "Observe local substrates; serve /state")]
struct Cli {
    /// Path to publisher config file
    #[arg(long, short)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let config_text = std::fs::read_to_string(&cli.config)?;
    let config: PublisherConfig = serde_json::from_str(&config_text)?;
    let bind_addr = config.bind_addr.clone();
    let config = Arc::new(config);

    let app = nq_witness::server::build_router(config);

    info!(bind = %bind_addr, "nq-witness starting");
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
