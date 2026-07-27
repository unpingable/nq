use crate::{ArtifactRef, ValidationError};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

const MAX_REFUSAL_CODE_LEN: usize = 96;
const MAX_REFUSAL_MESSAGE_LEN: usize = 1024;

/// A stable, machine-readable refusal code.
///
/// Codes are lowercase ASCII segments separated by `.`. Each segment starts
/// with a letter and may continue with lowercase letters, digits, or `_`.
/// Examples: `unsupported_schema` and `witness.source_unavailable`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefusalCode(String);

impl RefusalCode {
    /// Validate and construct a refusal code.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_code(&value)?;
        Ok(Self(value))
    }

    /// Borrow the stable code.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return its stable string.
    pub fn into_string(self) -> String {
        self.0
    }
}

fn validate_code(value: &str) -> Result<(), ValidationError> {
    let invalid = |reason| ValidationError::RefusalCode {
        value: value.to_owned(),
        reason,
    };
    if value.is_empty() {
        return Err(invalid("must not be empty"));
    }
    if value.len() > MAX_REFUSAL_CODE_LEN {
        return Err(invalid("must be at most 96 bytes"));
    }
    if !value.is_ascii() {
        return Err(invalid("must contain only ASCII characters"));
    }
    for segment in value.split('.') {
        let mut bytes = segment.bytes();
        if !matches!(bytes.next(), Some(b'a'..=b'z')) {
            return Err(invalid("each segment must start with a lowercase letter"));
        }
        if !bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_')) {
            return Err(invalid(
                "segments may contain only lowercase letters, digits, or '_'",
            ));
        }
    }
    Ok(())
}

impl AsRef<str> for RefusalCode {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RefusalCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RefusalCode {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for RefusalCode {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for RefusalCode {
    type Error = ValidationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Serialize for RefusalCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RefusalCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// A structured refusal returned at an artifact boundary.
///
/// The stable code is for programmatic handling; the message is bounded
/// operator-facing context. `retryable` does not authorize or schedule a
/// retry. `artifact_ref`, when present, identifies the specific immutable
/// artifact to which the refusal applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    code: RefusalCode,
    message: String,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact_ref: Option<ArtifactRef>,
}

impl Refusal {
    /// Validate and construct a structured refusal.
    pub fn new(
        code: RefusalCode,
        message: impl Into<String>,
        retryable: bool,
        artifact_ref: Option<ArtifactRef>,
    ) -> Result<Self, ValidationError> {
        let message = message.into();
        validate_message(&message)?;
        Ok(Self {
            code,
            message,
            retryable,
            artifact_ref,
        })
    }

    /// Return the stable machine-readable code.
    pub fn code(&self) -> &RefusalCode {
        &self.code
    }

    /// Return the bounded operator-facing explanation.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Report whether the producer considers a later retry meaningful.
    ///
    /// This is testimony only; it does not authorize or schedule a retry.
    pub fn retryable(&self) -> bool {
        self.retryable
    }

    /// Return the immutable artifact associated with the refusal, if any.
    pub fn artifact_ref(&self) -> Option<&ArtifactRef> {
        self.artifact_ref.as_ref()
    }

    /// Consume the refusal into its wire components.
    pub fn into_parts(self) -> (RefusalCode, String, bool, Option<ArtifactRef>) {
        (self.code, self.message, self.retryable, self.artifact_ref)
    }
}

fn validate_message(value: &str) -> Result<(), ValidationError> {
    let invalid = |reason| ValidationError::RefusalMessage { reason };
    if value.is_empty() {
        return Err(invalid("must not be empty"));
    }
    if value.len() > MAX_REFUSAL_MESSAGE_LEN {
        return Err(invalid("must be at most 1024 bytes"));
    }
    if value.trim() != value {
        return Err(invalid("must not have leading or trailing whitespace"));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid("must not contain control characters"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct RawRefusal {
    code: RefusalCode,
    message: String,
    retryable: bool,
    #[serde(default)]
    artifact_ref: Option<ArtifactRef>,
}

impl<'de> Deserialize<'de> for Refusal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawRefusal::deserialize(deserializer)?;
        Self::new(raw.code, raw.message, raw.retryable, raw.artifact_ref)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::value::{Error as DeError, StrDeserializer};

    #[test]
    fn accepts_stable_simple_and_namespaced_codes() {
        for value in [
            "unsupported_schema",
            "source_unavailable",
            "witness.source_unavailable",
            "monitor.low_coverage2",
        ] {
            assert_eq!(RefusalCode::new(value).unwrap().as_str(), value);
        }
    }

    #[test]
    fn rejects_unstable_code_spellings() {
        for value in [
            "",
            "UnsupportedSchema",
            "unsupported-schema",
            "unsupported schema",
            ".unsupported",
            "unsupported.",
            "unsupported..schema",
            "2unsupported",
            "ünsupported",
        ] {
            assert!(RefusalCode::new(value).is_err(), "{value:?} was accepted");
        }
    }

    #[test]
    fn refusal_preserves_unknown_without_action_semantics() {
        let refusal = Refusal::new(
            RefusalCode::new("impact_unknown").unwrap(),
            "Current service impact is not established.",
            false,
            None,
        )
        .unwrap();
        assert_eq!(refusal.code().as_str(), "impact_unknown");
        assert_eq!(
            refusal.message(),
            "Current service impact is not established."
        );
        assert!(!refusal.retryable());
        assert!(refusal.artifact_ref().is_none());
    }

    #[test]
    fn rejects_empty_padded_multiline_and_unbounded_messages() {
        for message in [
            "".to_string(),
            " padded".to_string(),
            "padded ".to_string(),
            "line one\nline two".to_string(),
            "x".repeat(MAX_REFUSAL_MESSAGE_LEN + 1),
        ] {
            assert!(Refusal::new(
                RefusalCode::new("invalid_input").unwrap(),
                message,
                false,
                None
            )
            .is_err());
        }
    }

    #[test]
    fn code_deserialization_runs_validation() {
        let malformed = StrDeserializer::<DeError>::new("Unsupported");
        assert!(RefusalCode::deserialize(malformed).is_err());
    }
}
