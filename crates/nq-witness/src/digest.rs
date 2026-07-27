use nq_protocol::ContentDigest;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::DIGEST_ALGORITHM_PREFIX;

/// Failure while serializing or canonicalizing an artifact for identity.
///
/// This retains the pre-extraction message field so decision-side callers can
/// migrate without a lockstep source change. Artifact-boundary refusals remain
/// typed by [`crate::WitnessAdoptionError`] and
/// [`crate::PacketSetAdoptionError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestError {
    /// Serialization or canonicalization diagnostic.
    pub message: String,
}

impl std::fmt::Display for DigestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DigestError {}

pub(crate) fn digest_jcs<T>(value: &T) -> Result<ContentDigest, DigestError>
where
    T: ?Sized + Serialize,
{
    let bytes = serde_jcs::to_vec(value).map_err(|error| DigestError {
        message: format!("JCS canonicalization failed: {error}"),
    })?;
    Ok(digest_bytes(&bytes))
}

pub(crate) fn digest_bytes(bytes: &[u8]) -> ContentDigest {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let value = format!(
        "{DIGEST_ALGORITHM_PREFIX}{}",
        hex::encode(hasher.finalize())
    );
    ContentDigest::new(value).expect("SHA-256 always produces the protocol digest shape")
}
