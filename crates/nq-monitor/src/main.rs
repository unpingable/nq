mod artifact_registry;
mod cli;
mod cmd;
mod declared_deny_probe;
mod declared_deny_transport;
mod gateway_path_probe;
mod gateway_path_transport;
mod http;
mod inquiry;
mod lease_presence_probe;
mod lease_presence_transport;
mod nq_evaluator_probe;
mod nq_sql_contract_state;
mod operator_surface;
mod probe;
mod pull;
mod tls_cert_probe;
mod tls_cert_series;
mod tls_cert_transport;
mod served_surface_registry;
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
