use nq_protocol::{ArtifactRef, ContentDigest, Refusal, RefusalCode, SchemaId, UtcTimestamp};
use serde::ser::{Error as _, Impossible, SerializeStruct};
use serde::{Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
enum WireValue {
    Bool(bool),
    String(String),
    Struct(Vec<(&'static str, WireValue)>),
}

#[derive(Debug)]
struct CaptureError(String);

impl fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CaptureError {}

impl serde::ser::Error for CaptureError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self(message.to_string())
    }
}

struct CaptureSerializer;

impl Serializer for CaptureSerializer {
    type Ok = WireValue;
    type Error = CaptureError;
    type SerializeSeq = Impossible<WireValue, CaptureError>;
    type SerializeTuple = Impossible<WireValue, CaptureError>;
    type SerializeTupleStruct = Impossible<WireValue, CaptureError>;
    type SerializeTupleVariant = Impossible<WireValue, CaptureError>;
    type SerializeMap = Impossible<WireValue, CaptureError>;
    type SerializeStruct = CaptureStruct;
    type SerializeStructVariant = Impossible<WireValue, CaptureError>;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        Ok(WireValue::Bool(value))
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        Ok(WireValue::String(value.to_owned()))
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(CaptureStruct {
            fields: Vec::with_capacity(len),
        })
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "the tested wire contract omits optional None fields",
        ))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "unit is outside the tested wire contract",
        ))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "variants are outside the tested wire contract",
        ))
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(Self::Error::custom(
            "variants are outside the tested wire contract",
        ))
    }

    fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "integer is outside the tested wire contract",
        ))
    }

    fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "integer is outside the tested wire contract",
        ))
    }

    fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "integer is outside the tested wire contract",
        ))
    }

    fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "integer is outside the tested wire contract",
        ))
    }

    fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "integer is outside the tested wire contract",
        ))
    }

    fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "integer is outside the tested wire contract",
        ))
    }

    fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "integer is outside the tested wire contract",
        ))
    }

    fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "integer is outside the tested wire contract",
        ))
    }

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "float is outside the tested wire contract",
        ))
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "float is outside the tested wire contract",
        ))
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "bytes are outside the tested wire contract",
        ))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Self::Error::custom(
            "sequences are outside the tested wire contract",
        ))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Self::Error::custom(
            "tuples are outside the tested wire contract",
        ))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Self::Error::custom(
            "tuple structs are outside the tested wire contract",
        ))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Self::Error::custom(
            "tuple variants are outside the tested wire contract",
        ))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Self::Error::custom(
            "maps are outside the tested wire contract",
        ))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Self::Error::custom(
            "struct variants are outside the tested wire contract",
        ))
    }
}

struct CaptureStruct {
    fields: Vec<(&'static str, WireValue)>,
}

impl SerializeStruct for CaptureStruct {
    type Ok = WireValue;
    type Error = CaptureError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.fields.push((key, value.serialize(CaptureSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(WireValue::Struct(self.fields))
    }
}

fn digest() -> ContentDigest {
    ContentDigest::new("sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        .unwrap()
}

#[test]
fn newtypes_serialize_as_canonical_strings() {
    let schema = SchemaId::new("nq.witness.v1").unwrap();
    assert_eq!(
        schema.serialize(CaptureSerializer).unwrap(),
        WireValue::String("nq.witness.v1".to_string())
    );
    assert_eq!(
        digest().serialize(CaptureSerializer).unwrap(),
        WireValue::String(digest().to_string())
    );
    let timestamp = UtcTimestamp::parse("2026-07-27T00:15:30-04:00").unwrap();
    assert_eq!(
        timestamp.serialize(CaptureSerializer).unwrap(),
        WireValue::String("2026-07-27T04:15:30Z".to_string())
    );
}

#[test]
fn artifact_reference_has_a_fixed_minimal_wire_shape() {
    let reference = ArtifactRef::new(
        SchemaId::new("nq.witness.v1").unwrap(),
        "witness:host-a:42",
        digest(),
    )
    .unwrap();
    assert_eq!(
        reference.serialize(CaptureSerializer).unwrap(),
        WireValue::Struct(vec![
            ("schema", WireValue::String("nq.witness.v1".to_string())),
            (
                "artifact_id",
                WireValue::String("witness:host-a:42".to_string())
            ),
            ("digest", WireValue::String(digest().to_string())),
        ])
    );
}

#[test]
fn refusal_without_artifact_omits_the_optional_field() {
    let refusal = Refusal::new(
        RefusalCode::new("impact_unknown").unwrap(),
        "Current impact is not established.",
        false,
        None,
    )
    .unwrap();
    assert_eq!(
        refusal.serialize(CaptureSerializer).unwrap(),
        WireValue::Struct(vec![
            ("code", WireValue::String("impact_unknown".to_string())),
            (
                "message",
                WireValue::String("Current impact is not established.".to_string())
            ),
            ("retryable", WireValue::Bool(false)),
        ])
    );
}

#[test]
fn refusal_with_artifact_embeds_the_immutable_reference_last() {
    let reference = ArtifactRef::new(
        SchemaId::new("nq.witness.v1").unwrap(),
        "witness:host-a:42",
        digest(),
    )
    .unwrap();
    let refusal = Refusal::new(
        RefusalCode::new("witness.unsupported_schema").unwrap(),
        "The witness schema is not supported.",
        false,
        Some(reference),
    )
    .unwrap();
    assert_eq!(
        refusal.serialize(CaptureSerializer).unwrap(),
        WireValue::Struct(vec![
            (
                "code",
                WireValue::String("witness.unsupported_schema".to_string())
            ),
            (
                "message",
                WireValue::String("The witness schema is not supported.".to_string())
            ),
            ("retryable", WireValue::Bool(false)),
            (
                "artifact_ref",
                WireValue::Struct(vec![
                    ("schema", WireValue::String("nq.witness.v1".to_string())),
                    (
                        "artifact_id",
                        WireValue::String("witness:host-a:42".to_string())
                    ),
                    ("digest", WireValue::String(digest().to_string())),
                ])
            ),
        ])
    );
}

#[test]
fn repeated_serialization_is_identical() {
    let refusal = Refusal::new(
        RefusalCode::new("source_unavailable").unwrap(),
        "The declared source could not be read.",
        true,
        Some(
            ArtifactRef::new(
                SchemaId::new("nq.witness.v1").unwrap(),
                "witness:host-a:43",
                digest(),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let first = refusal.serialize(CaptureSerializer).unwrap();
    let second = refusal.serialize(CaptureSerializer).unwrap();
    assert_eq!(first, second);
}
