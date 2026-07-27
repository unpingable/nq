use crate::ValidationError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

const SHA256_PREFIX: &str = "sha256:";
const SHA256_HEX_LEN: usize = 64;

/// A canonical SHA-256 content-digest string.
///
/// This type validates the wire representation
/// `sha256:<64 lowercase hexadecimal digits>`. It deliberately does not hash
/// bytes or assert that any bytes match the digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentDigest(String);

impl ContentDigest {
    /// Validate and construct a canonical SHA-256 digest.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        let hex = value
            .strip_prefix(SHA256_PREFIX)
            .filter(|hex| {
                hex.len() == SHA256_HEX_LEN
                    && hex
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            })
            .ok_or_else(|| ValidationError::ContentDigest {
                value: value.clone(),
            })?;
        debug_assert_eq!(hex.len(), SHA256_HEX_LEN);
        Ok(Self(value))
    }

    /// Borrow the canonical prefixed digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Borrow the 64 lowercase hexadecimal digits without the algorithm
    /// prefix.
    pub fn hex(&self) -> &str {
        &self.0[SHA256_PREFIX.len()..]
    }

    /// Consume the wrapper and return its canonical string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for ContentDigest {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ContentDigest {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for ContentDigest {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ContentDigest {
    type Error = ValidationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Serialize for ContentDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ContentDigest {
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

    const ZERO: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn accepts_exact_canonical_shape() {
        let digest = ContentDigest::new(ZERO).unwrap();
        assert_eq!(digest.as_str(), ZERO);
        assert_eq!(digest.hex(), &ZERO[7..]);
    }

    #[test]
    fn rejects_other_algorithms_lengths_and_case() {
        for value in [
            "",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "sha512:0000000000000000000000000000000000000000000000000000000000000000",
            "sha256:00",
            "sha256:00000000000000000000000000000000000000000000000000000000000000000",
            "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        ] {
            assert!(ContentDigest::new(value).is_err(), "{value:?} was accepted");
        }
    }

    #[test]
    fn deserialization_runs_validation() {
        let malformed = StrDeserializer::<DeError>::new("sha256:ABC");
        assert!(ContentDigest::deserialize(malformed).is_err());
    }
}
