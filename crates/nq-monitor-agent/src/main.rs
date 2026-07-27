//! `nq-witness` compatibility binary entry point. Reads a `PublisherConfig` from a
//! JSON file and serves the witness's `/state` HTTP endpoint until
//! killed. One config, one HTTP server, no subcommands.

use anyhow::Context;
use clap::{Parser, Subcommand};
use nq_core::PublisherConfig;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Parser)]
#[command(
    name = "nq-witness",
    about = "Observe local substrates; serve /state",
    version
)]
struct Cli {
    /// Path to publisher config file (compatibility startup form).
    #[arg(long, short)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect configuration without starting collection.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    /// Parse and validate publisher configuration with no side effects.
    Validate {
        /// Path to publisher JSON configuration.
        #[arg(long, short)]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    if cli.command.is_some() && cli.config.is_some() {
        anyhow::bail!(
            "choose one mode: start with `--config <path>` or validate with `config validate --config <path>`"
        );
    }
    if let Some(Command::Config {
        action: ConfigAction::Validate { config },
    }) = cli.command
    {
        let config_text = read_config(&config)?;
        parse_and_validate_publisher_config(&config_text).with_context(|| {
            format!(
                "publisher configuration `{}` was refused; no listener was started and no checks ran",
                config.display()
            )
        })?;
        println!(
            "configuration valid: {} (publisher; no state changed)",
            config.display()
        );
        return Ok(());
    }

    let config_path = cli.config.ok_or_else(|| {
        anyhow::anyhow!(
            "missing publisher configuration; use `nq-witness --config <path>` to start or `nq-witness config validate --config <path>` to validate"
        )
    })?;
    let config_text = read_config(&config_path)?;
    let config = parse_and_validate_publisher_config(&config_text).with_context(|| {
        format!(
            "publisher configuration `{}` was refused; no listener was started and no checks ran",
            config_path.display()
        )
    })?;
    let bind_addr = config.bind_addr.clone();
    let config = Arc::new(config);
    let app = nq_monitor_agent::server::build_router(config)
        .context("publisher configuration was refused before the listener started")?;

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| {
            format!(
                "cannot bind publisher listener `{bind_addr}`; no checks ran and no state was changed"
            )
        })?;

    info!(bind = %bind_addr, "nq-witness starting");
    axum::serve(listener, app).await?;
    Ok(())
}

fn parse_and_validate_publisher_config(input: &str) -> anyhow::Result<PublisherConfig> {
    let config = PublisherConfig::from_json_str(input)?;
    nq_monitor_agent::collect::validate_legacy_storage_config(&config)?;
    Ok(config)
}

fn read_config(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| {
        format!(
            "cannot read publisher configuration `{}`; no state was changed",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_parser_applies_storage_pack_preconditions() {
        let input = r#"{
          "bind_addr": "127.0.0.1:9847",
          "gpu_witness": {
            "nvidia_smi_path": "relative/nvidia-smi",
            "timeout_ms": 100
          }
        }"#;
        PublisherConfig::from_json_str(input)
            .expect("the legacy publisher parser historically accepts this path");

        let error = parse_and_validate_publisher_config(input)
            .expect_err("startup must apply the extracted storage-pack contract");
        assert!(
            error.to_string().contains("gpu_witness.nvidia_smi_path"),
            "error: {error:#}"
        );
    }
}
