//! Wire contract between `nq-witness` (testimony producer) and any
//! consumer that wants to ingest its `/state` payload.
//!
//! This crate owns the *consumer-facing* surface: the endpoint path,
//! the HTTP client, the deserialized type, the typed vocabulary
//! consumers bind on, and the witness-side input contract for
//! evaluator liveness probing.
//!
//! Having this contract live in its own crate is the structural
//! enforcement of the W/E (witness/evaluator) boundary: a consumer
//! that depends on `nq-witness-api` cannot accidentally reach into
//! the witness's collector code. Library callers depend on the wire
//! shape, not on how the wire shape was produced.
//!
//! ## Consumer surface
//!
//! Everything an external consumer should ever import from
//! `nq-witness-api`. Reaching into `nq-core` directly bypasses the
//! W/E boundary; if a needed type is missing here, re-export it
//! here rather than punching through.
//!
//! | Item | Purpose |
//! |---|---|
//! | [`STATE_PATH`] | The HTTP path the witness exposes. Shared by witness server and consumer client. |
//! | [`fetch_state`] | The HTTP pull. Returns a deserialized [`PublisherState`]. |
//! | [`WitnessPosition`] | Producer-declared layer of the stack a witness observes (substrate / application-internal / platform). Consumers render by position. |
//! | [`ClaimRefusal`] + [`RefusalKind`] | Typed refusal vocabulary carried on `PreflightResult.cannot_testify` and `Receipt.cannot_testify`. Consumers branch on `refusal_kind`. |
//! | [`fixtures`] | Witness-side input contract for `nq_evaluator_state` liveness probing. |
//!
//! Today's in-workspace consumers:
//! - `nq-monitor::pull` calls [`fetch_state`] on the publisher pull path.
//! - `nq-monitor::nq_evaluator_probe` imports [`fixtures::ALL_FIXTURES`] for the per-kind evaluator-liveness probe.
//! - `nq-witness::server` uses [`STATE_PATH`] as the route constant on the server side.
//!
//! The [`WitnessPosition`] and [`ClaimRefusal`] / [`RefusalKind`]
//! re-exports are the surface external consumers (Nightshift,
//! labelwatch, future MCP) bind to without an in-workspace caller
//! today.
//!
//! ## Fixtures
//!
//! The [`fixtures`] module owns the witness-side input contract for
//! `nq_evaluator_state` liveness probing. See
//! `docs/working/decisions/preflights/NQ_EVALUATOR_STATE.md` §9.
//! Fixtures live here — in the contract crate — so the per-kind
//! evaluator under test cannot author or mutate its own fixture.

pub mod fixtures;

use nq_core::wire::PublisherState;

/// Producer-declared layer of the stack a witness observes. Re-exported
/// from [`nq_core::witness`] (where it lives next to `WitnessPacket`)
/// so consumers can bind via the contract crate.
///
/// Wire shape is additive: the underlying packet field is
/// `Option<WitnessPosition>` with `skip_serializing_if = Option::is_none`.
/// Legacy on-wire packets without the field deserialize to `None`
/// (unclassified). Production constructions set `Some(...)` explicitly;
/// there is no silent default to `Substrate`.
///
/// See the type docstring in `nq-core::witness` for the per-variant
/// substrate / application-internal / platform definitions and the
/// position cut-over history.
pub use nq_core::witness::WitnessPosition;

/// Typed refusal vocabulary carried on the preflight + receipt
/// surface (`PreflightResult.cannot_testify` and `Receipt.cannot_testify`).
/// Re-exported from [`nq_core::wire`] so consumers bind via the
/// contract crate.
///
/// Consumers branch on [`ClaimRefusal::refusal_kind`] for the stable
/// machine category; [`ClaimRefusal::statement`] is render-time prose
/// and not a machine contract. Do not dedupe by `refusal_kind` alone —
/// the same kind can carry distinct statements that are operationally
/// different testimony (machine identity = kind; diagnostic inventory
/// = kind + statement + surface).
///
/// Wire shape is pinned by `PREFLIGHT_CONTRACT_VERSION = 2` (bumped
/// 1 → 2 on 2026-06-09). See
/// `docs/working/gaps/WITNESS_CLAIM_SCOPE_GAP.md` for the migration
/// record and per-variant harvest rationale.
///
/// The witness-coverage surface (`SmartWitnessCoverage.cannot_testify`,
/// `ZfsWitnessCoverage.cannot_testify`) carries its own `Vec<String>`
/// of snake_case observation-shape identifiers — not [`ClaimRefusal`]
/// — because the vocabulary there is shape identity, not prose refusal.
/// See "Why witness coverage is not a sibling" in the gap doc.
pub use nq_core::wire::{ClaimRefusal, RefusalKind};

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
