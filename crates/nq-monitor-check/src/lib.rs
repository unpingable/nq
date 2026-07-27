//! Monitor-owned contracts shared by the monitor runtime and check packs.
//!
//! This leaf owns pack registration and temporarily houses the established
//! monitor transport vocabulary. The composite `wire::Collectors` shape,
//! family-specific wire DTOs, and closed `status::CollectorKind` enum are
//! compatibility debt for `nq.witness_packet.v1`, not the target independent
//! pack observation schema. It contains no collector implementation, storage
//! access, decision law, dashboard behavior, or deployment configuration.

pub mod pack;
pub mod status;
pub mod wire;

pub use pack::{
    CheckCost, CheckDescriptor, CheckId, CheckLocality, CheckPackDefinition, CheckPackRegistry,
    CheckPrivilege, EnabledPack, ExecutableCheckPack, PackConfigError, PackDefaultPolicy,
    PackDescriptor, PackId, PackSelection, PackSelectionEntry, RegistryError, ResolvedPacks,
    CHECK_PACK_CONTRACT_VERSION,
};
pub use status::{
    CollectorKind, CollectorStatus, GenerationStatus, Platform, ServiceStatus, SourceStatus,
};
