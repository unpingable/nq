//! Witness side of NQ: collectors that observe host substrates and
//! emit `PublisherState` testimony, plus the local HTTP server that
//! exposes the latest snapshot on `GET /state`.
//!
//! The witness crate is structurally separated from the evaluator
//! (nq-monitor) — `nq-monitor` does NOT depend on this crate at link
//! time. The cross-process contract lives in `nq-witness-api`; the
//! consumer reaches a witness only over HTTP.
//!
//! Keeper:
//! > `nq-witness` produces testimony.
//! > It does not evaluate, store, or render admissibility.

pub mod collect;
pub mod server;
