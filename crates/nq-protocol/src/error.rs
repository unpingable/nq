use thiserror::Error;

/// Validation failure for an NQ protocol primitive.
///
/// The variants distinguish invalid concepts so callers can reject malformed
/// artifacts without parsing human-readable error text.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationError {
    /// A schema identifier is not a bounded, explicitly versioned identifier.
    #[error("invalid schema identifier {value:?}: {reason}")]
    SchemaId {
        /// Rejected input.
        value: String,
        /// Stable explanation of the violated rule.
        reason: &'static str,
    },

    /// A content digest is not a canonical SHA-256 digest.
    #[error("invalid content digest {value:?}: expected sha256:<64 lowercase hex digits>")]
    ContentDigest {
        /// Rejected input.
        value: String,
    },

    /// An artifact identifier is empty, unbounded, or contains unsafe syntax.
    #[error("invalid artifact identifier {value:?}: {reason}")]
    ArtifactId {
        /// Rejected input.
        value: String,
        /// Stable explanation of the violated rule.
        reason: &'static str,
    },

    /// A timestamp is not a valid RFC 3339 timestamp.
    #[error("invalid RFC3339 timestamp {value:?}")]
    Timestamp {
        /// Rejected input.
        value: String,
    },

    /// A refusal code is not a stable lowercase code.
    #[error("invalid refusal code {value:?}: {reason}")]
    RefusalCode {
        /// Rejected input.
        value: String,
        /// Stable explanation of the violated rule.
        reason: &'static str,
    },

    /// A refusal message is empty, unbounded, or contains control characters.
    #[error("invalid refusal message: {reason}")]
    RefusalMessage {
        /// Stable explanation of the violated rule.
        reason: &'static str,
    },
}
