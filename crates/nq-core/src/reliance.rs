//! Compatibility surface for consumer-indexed NQ reliance decisions.
//!
//! The decision law and its public artifacts are owned by the independent
//! `nq` package. `nq-core` retains this module path while transitional
//! consumers migrate; it contains no duplicate decision implementation.

pub use nq::reliance::*;
