//! Axum HTTP server that exposes one witness endpoint: `GET /state`.
//!
//! Each request runs `collect::collect_state` against the supplied
//! `PublisherConfig` and returns the resulting `PublisherState` as
//! JSON. There is no caching, no batching, no scheduling — the
//! witness is stateless w.r.t. the wire and idempotent per request.
//! Aggregator-side pulls drive cadence.

use crate::collect;
use axum::{routing::get, Json, Router};
use nq_core::wire::PublisherState;
use nq_core::PublisherConfig;
use nq_witness_api::STATE_PATH;
use std::sync::Arc;

/// Build the witness's HTTP router. Caller owns the listener and the
/// `axum::serve` loop — keeping the router pure makes it reusable
/// inside in-process tests as well as the `nq-witness` binary.
pub fn build_router(config: Arc<PublisherConfig>) -> Router {
    Router::new().route(
        STATE_PATH,
        get({
            let config = config.clone();
            move || handle_state(config.clone())
        }),
    )
}

async fn handle_state(config: Arc<PublisherConfig>) -> Json<PublisherState> {
    let state = collect::collect_state(&config);
    Json(state)
}
