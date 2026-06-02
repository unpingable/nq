use crate::cli::PublishCmd;
use crate::collect;
use axum::{Json, Router, routing::get};
use nq_core::wire::PublisherState;
use nq_core::PublisherConfig;
use std::sync::Arc;
use tracing::info;

pub async fn run(cmd: PublishCmd) -> anyhow::Result<()> {
    let config_text = std::fs::read_to_string(&cmd.config)?;
    let config: PublisherConfig = serde_json::from_str(&config_text)?;
    let bind_addr = config.bind_addr.clone();
    let config = Arc::new(config);

    let app = Router::new().route(
        "/state",
        get({
            let config = config.clone();
            move || handle_state(config.clone())
        }),
    );

    info!(bind = %bind_addr, "publisher starting");
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_state(config: Arc<PublisherConfig>) -> Json<PublisherState> {
    let state = collect::collect_state(&config);
    Json(state)
}
