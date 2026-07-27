//! Compatibility re-exports for the independently owned witness artifact
//! contract.
//!
//! New code should depend on `nq-witness` directly. This module preserves the
//! pre-extraction `nq_core::witness` source path while downstream consumers
//! migrate; it contains no witness semantics of its own.

pub use nq_witness::{
    AdoptedWitnessSet, DigestError, PacketSetAdoptionError, ValidatedWitness, WitnessAdoptionError,
    WitnessPacket, WitnessPosition, WitnessValidationError, WitnessValidationFailure,
    CUSTODY_BASIS_EXTERNAL_PROJECTION, CUSTODY_BASIS_LEGACY_PROJECTION, CUSTODY_BASIS_NATIVE,
    DIGEST_ALGORITHM_PREFIX, PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY, WITNESS_SCHEMA,
    WITNESS_SET_SCHEMA,
};
