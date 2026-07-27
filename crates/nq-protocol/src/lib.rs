//! Small, versioned wire primitives shared by independently released NQ
//! components.
//!
//! This crate validates syntax and preserves identity. It contains no
//! evidence-sufficiency, monitoring, storage, configuration, or policy logic.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod artifact_ref;
mod digest;
mod error;
mod refusal;
mod schema_id;
mod timestamp;

pub use artifact_ref::ArtifactRef;
pub use digest::ContentDigest;
pub use error::ValidationError;
pub use refusal::{Refusal, RefusalCode};
pub use schema_id::SchemaId;
pub use timestamp::UtcTimestamp;
