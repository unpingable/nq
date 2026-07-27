//! Monitor-agent side of NQ: collectors that observe host substrates and
//! emit `PublisherState` observations, plus the local HTTP server that
//! exposes the latest snapshot on `GET /state`.
//!
//! The installed compatibility binary remains `nq-witness`. The package is
//! named `nq-monitor-agent` because collector execution and `/state`
//! transport are monitor mechanics, not ownership of the immutable
//! `nq.witness.v1` artifact.
//!
//! Keeper:
//! > `nq-monitor-agent` produces bounded observations.
//! > It does not evaluate, store, or render admissibility.

pub mod collect;
pub mod server;
