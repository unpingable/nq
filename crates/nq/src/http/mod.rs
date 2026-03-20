pub mod query_api;
pub mod routes;

use nq_db::ReadDb;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn serve(db: ReadDb, bind: &str) -> anyhow::Result<()> {
    let db = Arc::new(Mutex::new(db));
    let app = routes::router(db);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
