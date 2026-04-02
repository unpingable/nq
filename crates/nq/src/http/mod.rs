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
