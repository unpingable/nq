//! Versioned witness artifacts, validation, canonical identity, and projection
//! adoption for the NQ constellation.
//!
//! This crate establishes what a witness artifact is. It does not decide what
//! any witness is sufficient to prove.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod adoption;
mod digest;
mod projection_receipt;
mod witness;

pub use adoption::{
    adopt_packet_set, AdoptedWitnessSet, PacketSetAdoptionError, ValidatedWitness,
    WitnessAdoptionError, WITNESS_SET_SCHEMA,
};
pub use digest::DigestError;
pub use projection_receipt::{
    ProjectionContradictionStatus, ProjectionMappingProfile, ProjectionReceipt,
    ProjectionReceiptMapping, ProjectionReceiptPacket, ProjectionReceiptReplay,
    ProjectionReceiptSource, ProjectionReceiptSubstitution, ProjectionReceiptValidationError,
    ProjectionReceiptValidationFailure, ProjectionSourceSystem,
    PROJECTION_RECEIPT_DOES_NOT_ESTABLISH, PROJECTION_RECEIPT_ESTABLISHES,
    PROJECTION_RECEIPT_SCHEMA,
};
pub use witness::{
    WitnessPacket, WitnessPosition, WitnessValidationError, WitnessValidationFailure,
    CUSTODY_BASIS_EXTERNAL_PROJECTION, CUSTODY_BASIS_LEGACY_PROJECTION, CUSTODY_BASIS_NATIVE,
    DIGEST_ALGORITHM_PREFIX, PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY, WITNESS_SCHEMA,
};
