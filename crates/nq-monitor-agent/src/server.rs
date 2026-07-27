//! Axum HTTP server that exposes one witness endpoint: `GET /state`.
//!
//! Each request runs `collect::collect_state` against the supplied
//! `PublisherConfig` and returns the resulting `PublisherState` as
//! JSON. There is no caching, no batching, no scheduling — the
//! witness is stateless w.r.t. the wire and idempotent per request.
//! Aggregator-side pulls drive cadence.

use crate::collect;
use axum::http::StatusCode;
use axum::{routing::get, Json, Router};
use nq_core::PublisherConfig;
use nq_monitor_check::wire::PublisherState;
use nq_monitor_check::PackConfigError;
use nq_witness_api::STATE_PATH;
use std::sync::Arc;

/// Build the witness's HTTP router. Caller owns the listener and the
/// `axum::serve` loop — keeping the router pure makes it reusable
/// inside in-process tests as well as the `nq-witness` binary.
pub fn build_router(config: Arc<PublisherConfig>) -> Result<Router, PackConfigError> {
    collect::validate_legacy_storage_config(&config)?;
    Ok(Router::new().route(
        STATE_PATH,
        get({
            let config = config.clone();
            move || handle_state(config.clone())
        }),
    ))
}

async fn handle_state(
    config: Arc<PublisherConfig>,
) -> Result<Json<PublisherState>, (StatusCode, String)> {
    collect::collect_state(&config).map(Json).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("publisher configuration refused before collection: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_legacy_storage_config_cannot_build_runtime_router() {
        let config = PublisherConfig::from_json_str(
            r#"{
              "smart_witness": {
                "helper_path": "relative/smart-witness",
                "timeout_ms": 100
              }
            }"#,
        )
        .expect("legacy parser historically accepts the relative helper");

        let error = build_router(Arc::new(config))
            .err()
            .expect("runtime construction must apply storage-pack preconditions");
        assert_eq!(error.field, "smart_witness.helper_path");
    }
}
