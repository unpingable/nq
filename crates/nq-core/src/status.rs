//! Compatibility reexports for monitor-owned status vocabulary.
//!
//! Removal condition: database and API consumers import these statuses from
//! `nq-monitor-check` and no production code imports `nq_core::status`.

pub use nq_monitor_check::status::*;
