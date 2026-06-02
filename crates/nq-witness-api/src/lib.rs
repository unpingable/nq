//! Wire contract between `nq-witness` (testimony producer) and any
//! consumer that wants to ingest its `/state` payload.
//!
//! This crate owns the *consumer-facing* surface: the endpoint path,
//! the HTTP client, and the deserialized type. The witness binary
//! re-uses the same endpoint constants on the server side; consumers
//! (today: `nq-monitor`'s pull path) reach a witness only through
//! `fetch_state`.
//!
//! Having this contract live in its own crate is the structural
//! enforcement of the W/E (witness/evaluator) boundary: a consumer
//! that depends on `nq-witness-api` cannot accidentally reach into
//! the witness's collector code. Library callers depend on the wire
//! shape, not on how the wire shape was produced.

use nq_core::wire::PublisherState;

/// HTTP path the witness binary exposes for its testimony payload.
pub const STATE_PATH: &str = "/state";

/// Fetch one `PublisherState` snapshot from a witness's `/state`
/// endpoint. The caller controls the `reqwest::Client` (timeout,
/// connection pool, proxy settings). `base_url` may include or omit
/// a trailing slash.
pub async fn fetch_state(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<PublisherState, reqwest::Error> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), STATE_PATH);
    client.get(&url).send().await?.json::<PublisherState>().await
}
