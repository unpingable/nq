//! Compatibility re-exports for receiver-owned external-projection receipts.
//!
//! The authoritative wire contract and validation now live in `nq-witness`.
//! New consumers should depend on that package directly.

pub use nq_witness::{
    ProjectionContradictionStatus, ProjectionMappingProfile, ProjectionReceipt,
    ProjectionReceiptMapping, ProjectionReceiptPacket, ProjectionReceiptReplay,
    ProjectionReceiptSource, ProjectionReceiptSubstitution, ProjectionReceiptValidationError,
    ProjectionReceiptValidationFailure, ProjectionSourceSystem,
    PROJECTION_RECEIPT_DOES_NOT_ESTABLISH, PROJECTION_RECEIPT_ESTABLISHES,
    PROJECTION_RECEIPT_SCHEMA,
};
