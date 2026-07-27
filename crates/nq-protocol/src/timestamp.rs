use crate::ValidationError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

/// An RFC 3339 timestamp normalized to UTC.
///
/// Equivalent inputs with non-UTC offsets are accepted and normalized.
/// Serialization always emits one deterministic UTC representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcTimestamp(OffsetDateTime);

impl UtcTimestamp {
    /// Construct a timestamp from an instant, normalizing its offset to UTC.
    pub fn new(value: OffsetDateTime) -> Result<Self, ValidationError> {
        let normalized = value.to_offset(UtcOffset::UTC);
        normalized
            .format(&Rfc3339)
            .map_err(|_| ValidationError::Timestamp {
                value: format!("{value:?}"),
            })?;
        Ok(Self(normalized))
    }

    /// Parse an RFC 3339 timestamp and normalize it to UTC.
    pub fn parse(value: &str) -> Result<Self, ValidationError> {
        let parsed =
            OffsetDateTime::parse(value, &Rfc3339).map_err(|_| ValidationError::Timestamp {
                value: value.to_owned(),
            })?;
        Self::new(parsed).map_err(|_| ValidationError::Timestamp {
            value: value.to_owned(),
        })
    }

    /// Return the normalized instant.
    pub fn as_offset_date_time(&self) -> OffsetDateTime {
        self.0
    }

    /// Format the timestamp in canonical RFC 3339 UTC form.
    pub fn to_rfc3339(self) -> String {
        self.0
            .format(&Rfc3339)
            .expect("every OffsetDateTime can be formatted as RFC3339")
    }
}

impl fmt::Display for UtcTimestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_rfc3339())
    }
}

impl FromStr for UtcTimestamp {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<OffsetDateTime> for UtcTimestamp {
    type Error = ValidationError;

    fn try_from(value: OffsetDateTime) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Serialize for UtcTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

impl<'de> Deserialize<'de> for UtcTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::value::{Error as DeError, StrDeserializer};

    #[test]
    fn normalizes_equivalent_offsets() {
        let eastern = UtcTimestamp::parse("2026-07-27T00:15:30-04:00").unwrap();
        let utc = UtcTimestamp::parse("2026-07-27T04:15:30Z").unwrap();
        assert_eq!(eastern, utc);
        assert_eq!(eastern.to_rfc3339(), "2026-07-27T04:15:30Z");
    }

    #[test]
    fn preserves_subsecond_precision_canonically() {
        let timestamp = UtcTimestamp::parse("2026-07-27T04:15:30.120000000Z").unwrap();
        assert_eq!(timestamp.to_rfc3339(), "2026-07-27T04:15:30.12Z");
    }

    #[test]
    fn normalizes_the_rfc3339_space_separator() {
        let timestamp = UtcTimestamp::parse("2026-07-27 04:15:30Z").unwrap();
        assert_eq!(timestamp.to_rfc3339(), "2026-07-27T04:15:30Z");
    }

    #[test]
    fn rejects_non_rfc3339_and_missing_offsets() {
        for value in ["", "2026-07-27", "2026-07-27T04:15:30", "not-a-time"] {
            assert!(
                UtcTimestamp::parse(value).is_err(),
                "{value:?} was accepted"
            );
        }
    }

    #[test]
    fn deserialization_runs_validation() {
        let malformed = StrDeserializer::<DeError>::new("yesterday");
        assert!(UtcTimestamp::deserialize(malformed).is_err());
    }
}
