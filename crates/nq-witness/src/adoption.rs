use nq_protocol::{ContentDigest, Refusal, RefusalCode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::{digest::digest_jcs, DigestError, WitnessPacket, WitnessValidationFailure};

/// Versioned identity domain for a deterministically adopted witness set.
pub const WITNESS_SET_SCHEMA: &str = "nq.witness_set.v1";

/// A structurally valid, content-identified witness packet.
///
/// The packet field is private, and deserialization validates before creating
/// this wrapper. Holding this type establishes structural witness validity
/// only; it does not establish source truth, evidence sufficiency, freshness,
/// or a disposition.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedWitness {
    packet: WitnessPacket,
    digest: ContentDigest,
}

impl ValidatedWitness {
    /// Validate and content-identify a witness packet.
    pub fn adopt(packet: WitnessPacket) -> Result<Self, WitnessAdoptionError> {
        packet
            .validate_typed()
            .map_err(WitnessAdoptionError::Validation)?;
        let digest = packet
            .content_digest()
            .map_err(WitnessAdoptionError::Digest)?;
        Ok(Self { packet, digest })
    }

    /// Borrow the exact validated packet.
    pub fn packet(&self) -> &WitnessPacket {
        &self.packet
    }

    /// Borrow the immutable JCS/SHA-256 packet identity.
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// Consume the wrapper and recover the exact packet.
    pub fn into_packet(self) -> WitnessPacket {
        self.packet
    }

    fn from_validated_parts(packet: WitnessPacket, digest: ContentDigest) -> Self {
        Self { packet, digest }
    }
}

impl TryFrom<WitnessPacket> for ValidatedWitness {
    type Error = WitnessAdoptionError;

    fn try_from(packet: WitnessPacket) -> Result<Self, Self::Error> {
        Self::adopt(packet)
    }
}

impl Serialize for ValidatedWitness {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.packet.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ValidatedWitness {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let packet = WitnessPacket::deserialize(deserializer)?;
        Self::adopt(packet).map_err(serde::de::Error::custom)
    }
}

/// Failure while validating and identifying one witness packet.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WitnessAdoptionError {
    /// Structural witness validation failed.
    #[error(transparent)]
    Validation(#[from] WitnessValidationFailure),

    /// Canonical packet identity could not be computed.
    #[error(transparent)]
    Digest(#[from] DigestError),
}

impl WitnessAdoptionError {
    /// Convert this adoption failure into a bounded typed refusal.
    pub fn refusal(&self) -> Refusal {
        match self {
            Self::Validation(error) => error.refusal(),
            Self::Digest(_) => refusal(
                "witness.digest_failed",
                "The witness packet could not be canonically identified.",
            ),
        }
    }
}

/// A deterministically ordered set of adopted witness artifacts.
///
/// Ordering is by packet content digest, independent of input order. The set
/// identity is the JCS/SHA-256 digest of a versioned
/// `nq.witness_set.v1` identity object containing that ordered digest array.
/// This type records artifact adoption only; it does not combine observations
/// or infer that packets agree about the world.
#[derive(Debug, Clone, PartialEq)]
pub struct AdoptedWitnessSet {
    witnesses: Vec<ValidatedWitness>,
    digest: ContentDigest,
}

impl AdoptedWitnessSet {
    /// Return the identity-domain schema used to compute this set's digest.
    pub fn schema(&self) -> &'static str {
        WITNESS_SET_SCHEMA
    }

    /// Return adopted witnesses in deterministic content-digest order.
    pub fn witnesses(&self) -> &[ValidatedWitness] {
        &self.witnesses
    }

    /// Return the deterministic packet-set identity.
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// Return the number of adopted witness packets.
    pub fn len(&self) -> usize {
        self.witnesses.len()
    }

    /// Report whether the adopted set contains no packets.
    pub fn is_empty(&self) -> bool {
        self.witnesses.is_empty()
    }
}

/// Typed failure while adopting a packet set.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PacketSetAdoptionError {
    /// An input packet names an unsupported witness schema.
    #[error("packet {packet_digest} uses unsupported schema {found:?}")]
    UnsupportedSchema {
        /// Digest of the rejected serialized packet.
        packet_digest: ContentDigest,
        /// Unsupported schema value.
        found: String,
    },

    /// A packet failed a non-version structural validation rule.
    #[error("packet {packet_digest} is invalid: {source}")]
    InvalidPacket {
        /// Digest of the rejected serialized packet.
        packet_digest: ContentDigest,
        /// Typed validation failure.
        #[source]
        source: WitnessValidationFailure,
    },

    /// The identical packet artifact appeared more than once.
    #[error("duplicate witness packet {digest}")]
    DuplicatePacket {
        /// Repeated packet identity.
        digest: ContentDigest,
    },

    /// Canonical identity could not be computed.
    #[error(transparent)]
    Digest(#[from] DigestError),
}

impl PacketSetAdoptionError {
    /// Return the stable machine-readable refusal code.
    pub fn refusal_code(&self) -> RefusalCode {
        let code = match self {
            Self::UnsupportedSchema { .. } => "witness.unsupported_schema",
            Self::InvalidPacket { .. } => "witness.invalid_packet",
            Self::DuplicatePacket { .. } => "witness.duplicate_packet",
            Self::Digest(_) => "witness.digest_failed",
        };
        RefusalCode::new(code).expect("packet-set refusal codes are protocol-valid constants")
    }

    /// Convert the set-adoption failure into a bounded typed refusal.
    pub fn refusal(&self) -> Refusal {
        let message = match self {
            Self::UnsupportedSchema { .. } => "A witness packet uses an unsupported schema.",
            Self::InvalidPacket { .. } => "A witness packet failed structural validation.",
            Self::DuplicatePacket { .. } => {
                "The packet set contains the same witness artifact more than once."
            }
            Self::Digest(_) => "The witness packet set could not be canonically identified.",
        };
        Refusal::new(self.refusal_code(), message, false, None)
            .expect("packet-set refusal messages are bounded constants")
    }
}

/// Validate and deterministically adopt a set of witness packets.
///
/// Exact packet duplicates are refused. Packets are not grouped by subject,
/// source reference, or time because `nq.witness.v1` deliberately defines
/// packet identity but no observation-equivalence or generic contradiction
/// relation. Producer-specific source collisions and explicit projection
/// substitution refusals remain distinct typed contracts.
pub fn adopt_packet_set<I>(packets: I) -> Result<AdoptedWitnessSet, PacketSetAdoptionError>
where
    I: IntoIterator<Item = WitnessPacket>,
{
    let mut identified = packets
        .into_iter()
        .map(|packet| {
            let digest = packet.content_digest()?;
            Ok((digest, packet))
        })
        .collect::<Result<Vec<_>, DigestError>>()?;
    identified.sort_by(|left, right| left.0.cmp(&right.0));

    let mut validated = Vec::with_capacity(identified.len());
    for (digest, packet) in identified {
        match packet.validate_typed() {
            Ok(()) => {}
            Err(WitnessValidationFailure::UnsupportedSchema { found }) => {
                return Err(PacketSetAdoptionError::UnsupportedSchema {
                    packet_digest: digest,
                    found,
                });
            }
            Err(source) => {
                return Err(PacketSetAdoptionError::InvalidPacket {
                    packet_digest: digest,
                    source,
                });
            }
        }
        validated.push(ValidatedWitness::from_validated_parts(packet, digest));
    }

    for pair in validated.windows(2) {
        if pair[0].digest == pair[1].digest {
            return Err(PacketSetAdoptionError::DuplicatePacket {
                digest: pair[0].digest.clone(),
            });
        }
    }

    let packet_digests: Vec<&str> = validated
        .iter()
        .map(|witness| witness.digest.as_str())
        .collect();
    #[derive(Serialize)]
    struct SetIdentity<'a> {
        schema: &'static str,
        packet_digests: Vec<&'a str>,
    }
    let digest = digest_jcs(&SetIdentity {
        schema: WITNESS_SET_SCHEMA,
        packet_digests,
    })?;
    Ok(AdoptedWitnessSet {
        witnesses: validated,
        digest,
    })
}

fn refusal(code: &str, message: &str) -> Refusal {
    Refusal::new(
        RefusalCode::new(code).expect("witness refusal code is a protocol-valid constant"),
        message,
        false,
        None,
    )
    .expect("witness refusal message is a bounded constant")
}
