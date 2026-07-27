use crate::ValidationError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

const MAX_SCHEMA_ID_LEN: usize = 128;

/// A bounded, explicitly versioned serialized-contract identifier.
///
/// The accepted grammar is:
///
/// ```text
/// segment.segment[.segment...].vN
/// ```
///
/// Each ordinary segment starts with a lowercase ASCII letter and continues
/// with lowercase letters, digits, `_`, or `-`. The final segment is `v`
/// followed by a canonical decimal version (`v0` or a non-zero digit followed
/// by digits). Examples include `nq.witness.v1` and
/// `zab2nq.monitor-to-witness.report.v1`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SchemaId(String);

impl SchemaId {
    /// Validate and construct a schema identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        validate(&value)?;
        Ok(Self(value))
    }

    /// Borrow the canonical identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return its canonical string.
    pub fn into_string(self) -> String {
        self.0
    }
}

fn validate(value: &str) -> Result<(), ValidationError> {
    let invalid = |reason| ValidationError::SchemaId {
        value: value.to_owned(),
        reason,
    };
    if value.is_empty() {
        return Err(invalid("must not be empty"));
    }
    if value.len() > MAX_SCHEMA_ID_LEN {
        return Err(invalid("must be at most 128 bytes"));
    }
    if !value.is_ascii() {
        return Err(invalid("must contain only ASCII characters"));
    }

    let segments: Vec<&str> = value.split('.').collect();
    if segments.len() < 3 {
        return Err(invalid(
            "must contain a namespace, name, and explicit version segment",
        ));
    }
    let (version, ordinary) = segments
        .split_last()
        .expect("the non-empty segment count was checked");
    for segment in ordinary {
        validate_ordinary_segment(segment).map_err(invalid)?;
    }
    validate_version_segment(version).map_err(invalid)
}

fn validate_ordinary_segment(segment: &str) -> Result<(), &'static str> {
    let mut chars = segment.bytes();
    if !matches!(chars.next(), Some(b'a'..=b'z')) {
        return Err("each non-version segment must start with a lowercase letter");
    }
    if !chars.all(|byte| {
        matches!(
            byte,
            b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'
        )
    }) {
        return Err("non-version segments may contain only lowercase letters, digits, '_' or '-'");
    }
    Ok(())
}

fn validate_version_segment(segment: &str) -> Result<(), &'static str> {
    let digits = segment
        .strip_prefix('v')
        .ok_or("the final segment must be an explicit vN version")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("the final segment must be an explicit vN version");
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return Err("the version must use canonical decimal form without leading zeroes");
    }
    Ok(())
}

impl AsRef<str> for SchemaId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SchemaId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for SchemaId {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for SchemaId {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for SchemaId {
    type Error = ValidationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Serialize for SchemaId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SchemaId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::value::{Error as DeError, StrDeserializer};

    #[test]
    fn accepts_explicit_versioned_identifiers() {
        for value in [
            "nq.witness.v0",
            "nq.witness.v1",
            "nq.inquiry_receipt.v12",
            "zab2nq.monitor-to-witness.report.v1",
        ] {
            assert_eq!(SchemaId::new(value).unwrap().as_str(), value);
        }
    }

    #[test]
    fn rejects_implicit_or_noncanonical_versions() {
        for value in [
            "",
            "nq",
            "nq.witness",
            "NQ.witness.v1",
            "nq..v1",
            "nq.witness.1",
            "nq.witness.v",
            "nq.witness.v01",
            "nq.witness.v1.extra",
            "nq.witness.v１",
        ] {
            assert!(SchemaId::new(value).is_err(), "{value:?} was accepted");
        }
    }

    #[test]
    fn deserialization_runs_validation() {
        let malformed = StrDeserializer::<DeError>::new("nq.witness.latest");
        assert!(SchemaId::deserialize(malformed).is_err());
    }
}
