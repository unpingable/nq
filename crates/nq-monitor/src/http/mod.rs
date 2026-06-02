pub mod query_api;
pub mod routes;

use nq_db::{ReadDb, WriteDb};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn serve_with_write(read_db: ReadDb, write_db: Arc<Mutex<WriteDb>>, bind: &str) -> anyhow::Result<()> {
    let read_db = Arc::new(Mutex::new(read_db));
    let app = routes::router_with_write(read_db, write_db);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Bind the read-only router only. Used by `nq-monitor serve --http-only` for safe
/// live smoke against a running monitor's DB: no write_db, no saved-query
/// or finding-transition routes, and the caller skips the pull / publish /
/// detector / notification loop entirely.
pub async fn serve_read_only(read_db: ReadDb, bind: &str) -> anyhow::Result<()> {
    let read_db = Arc::new(Mutex::new(read_db));
    let app = routes::router(read_db);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
