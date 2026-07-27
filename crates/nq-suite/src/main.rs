use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(
    name = "nq-suite",
    about = "Validate and plan an explicit NQ constellation composition",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate suite configuration without launching components or checks.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Emit the deterministic, machine-readable composition plan.
    Plan {
        /// Versioned NQ suite JSON configuration.
        #[arg(long, short)]
        config: PathBuf,
        /// Pretty-print JSON; compact JSON is the stable machine default.
        #[arg(long)]
        pretty: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    /// Parse, version-check, resolve packs, and validate all selected config.
    Validate {
        /// Versioned NQ suite JSON configuration.
        #[arg(long, short)]
        config: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Config {
            action: ConfigAction::Validate { config },
        } => {
            let input = read_config(&config)?;
            let plan = nq_suite::plan_from_json(&input).with_context(|| {
                format!(
                    "suite configuration `{}` was refused; no listener, database, source, or check was touched",
                    config.display()
                )
            })?;
            println!(
                "configuration valid: {} ({} enabled pack(s); no state changed)",
                config.display(),
                plan.enabled_packs.len()
            );
        }
        Command::Plan { config, pretty } => {
            let input = read_config(&config)?;
            let plan = nq_suite::plan_from_json(&input).with_context(|| {
                format!(
                    "suite configuration `{}` was refused; no listener, database, source, or check was touched",
                    config.display()
                )
            })?;
            if pretty {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!("{}", serde_json::to_string(&plan)?);
            }
        }
    }
    Ok(())
}

fn read_config(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| {
        format!(
            "cannot read suite configuration `{}`; no state was changed",
            path.display()
        )
    })
}
