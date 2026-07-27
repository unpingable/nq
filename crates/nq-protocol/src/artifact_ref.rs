use crate::{ContentDigest, SchemaId, ValidationError};
use serde::{Deserialize, Deserializer, Serialize};

const MAX_ARTIFACT_ID_LEN: usize = 256;

/// An immutable reference to a serialized artifact.
///
/// `schema` names the artifact's versioned wire contract, `artifact_id` is a
/// producer-scoped stable identity, and `digest` pins the exact content.
/// Construction does not prove that the artifact exists or that its bytes
/// match the supplied digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ArtifactRef {
    schema: SchemaId,
    artifact_id: String,
    digest: ContentDigest,
}

impl ArtifactRef {
    /// Validate and construct an immutable artifact reference.
    pub fn new(
        schema: SchemaId,
        artifact_id: impl Into<String>,
        digest: ContentDigest,
    ) -> Result<Self, ValidationError> {
        let artifact_id = artifact_id.into();
        validate_artifact_id(&artifact_id)?;
        Ok(Self {
            schema,
            artifact_id,
            digest,
        })
    }

    /// Return the referenced artifact's versioned schema.
    pub fn schema(&self) -> &SchemaId {
        &self.schema
    }

    /// Return the producer-scoped stable artifact identity.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Return the digest that makes the reference immutable.
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// Consume the reference into its three wire components.
    pub fn into_parts(self) -> (SchemaId, String, ContentDigest) {
        (self.schema, self.artifact_id, self.digest)
    }
}

fn validate_artifact_id(value: &str) -> Result<(), ValidationError> {
    let invalid = |reason| ValidationError::ArtifactId {
        value: value.to_owned(),
        reason,
    };
    if value.is_empty() {
        return Err(invalid("must not be empty"));
    }
    if value.len() > MAX_ARTIFACT_ID_LEN {
        return Err(invalid("must be at most 256 bytes"));
    }
    if !value.is_ascii() {
        return Err(invalid("must contain only ASCII characters"));
    }
    let mut bytes = value.bytes();
    if !matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9')) {
        return Err(invalid("must start with an ASCII letter or digit"));
    }
    if !bytes.all(|byte| {
        matches!(
            byte,
            b'a'..=b'z'
                | b'A'..=b'Z'
                | b'0'..=b'9'
                | b'.'
                | b'_'
                | b'-'
                | b':'
                | b'/'
                | b'@'
                | b'+'
        )
    }) {
        return Err(invalid(
            "may contain only ASCII letters, digits, '.', '_', '-', ':', '/', '@', or '+'",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct RawArtifactRef {
    schema: SchemaId,
    artifact_id: String,
    digest: ContentDigest,
}

impl<'de> Deserialize<'de> for ArtifactRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawArtifactRef::deserialize(deserializer)?;
        Self::new(raw.schema, raw.artifact_id, raw.digest).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest() -> ContentDigest {
        ContentDigest::new(
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap()
    }

    #[test]
    fn retains_all_immutable_identity_parts() {
        let reference = ArtifactRef::new(
            SchemaId::new("nq.witness.v1").unwrap(),
            "zab2nq:record:zbx-7@archive",
            digest(),
        )
        .unwrap();
        assert_eq!(reference.schema().as_str(), "nq.witness.v1");
        assert_eq!(reference.artifact_id(), "zab2nq:record:zbx-7@archive");
        assert_eq!(reference.digest(), &digest());
    }

    #[test]
    fn rejects_blank_unicode_whitespace_and_shell_syntax() {
        for value in [
            "",
            " artifact",
            "artifact ",
            "artifact id",
            "artifáct",
            "artifact?query",
            "artifact#fragment",
            "artifact;$HOME",
        ] {
            assert!(
                ArtifactRef::new(SchemaId::new("nq.witness.v1").unwrap(), value, digest()).is_err(),
                "{value:?} was accepted"
            );
        }
    }
}
