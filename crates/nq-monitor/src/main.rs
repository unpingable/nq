mod cli;
mod cmd;
mod http;
mod nq_evaluator_probe;
mod probe;
mod pull;
mod smoke;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = cli::Cli::parse();
    cmd::run(cli).await
}
