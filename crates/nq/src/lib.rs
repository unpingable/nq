//! Evidence evaluation and decision semantics for the NQ constellation.
//!
//! This package owns refusal vocabulary, evaluation dispositions, supporting
//! evidence identities, and consumer-indexed reliance decisions. It contains
//! no collectors, schedules, database access, dashboard behavior, notification
//! transport, or deployment configuration.

#![forbid(unsafe_code)]

mod disposition;
mod refusal;
pub mod reliance;

pub use disposition::{EvaluatedReceipt, EvaluationView, Status, StatusReason, WitnessRef};
pub use refusal::{ClaimRefusal, RefusalKind};
