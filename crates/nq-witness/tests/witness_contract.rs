use nq_witness::{
    adopt_packet_set, PacketSetAdoptionError, ValidatedWitness, WitnessPacket, WitnessPosition,
    WitnessValidationError, WitnessValidationFailure, CUSTODY_BASIS_EXTERNAL_PROJECTION,
    CUSTODY_BASIS_NATIVE, PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY, WITNESS_SCHEMA,
    WITNESS_SET_SCHEMA,
};

fn native_packet(subject: &str) -> WitnessPacket {
    WitnessPacket {
        schema: WITNESS_SCHEMA.to_string(),
        witness_type: "pytest".to_string(),
        subject: subject.to_string(),
        access_path: "local_command".to_string(),
        observed_at: "2026-05-15T14:00:00Z".to_string(),
        generated_at: "2026-05-15T14:00:03Z".to_string(),
        observations: vec![serde_json::json!({
            "type": "pytest_run",
            "exit_code": 0
        })],
        coverage_limits: vec!["does not observe production behavior".to_string()],
        dependencies: vec![],
        custody_basis: None,
        source_finding_ref: None,
        projection_limits: vec![],
        position: None,
    }
}

fn zab2nq_fixture() -> WitnessPacket {
    serde_json::from_slice(include_bytes!("fixtures/zab2nq-external-projection.json")).unwrap()
}

#[test]
fn pre_cutover_wire_and_digest_remain_exact() {
    let packet = native_packet("repo:.");
    packet.validate().unwrap();
    let value = serde_json::to_value(&packet).unwrap();
    assert!(value.get("custody_basis").is_none());
    assert!(value.get("source_finding_ref").is_none());
    assert!(value.get("projection_limits").is_none());
    assert!(value.get("position").is_none());
    assert_eq!(
        packet.digest().unwrap(),
        "sha256:598d44eeea65fa1a5e4bb9bbb5571733f6e6758ae858ba0ed1df5bbcf1ba5959"
    );
}

#[test]
fn validated_wrapper_is_obtained_by_validation_and_preserves_wire_shape() {
    let packet = native_packet("host:one");
    let expected = serde_json::to_value(&packet).unwrap();
    let validated = packet.into_validated().unwrap();
    assert_eq!(serde_json::to_value(&validated).unwrap(), expected);

    let decoded: ValidatedWitness = serde_json::from_value(expected).unwrap();
    assert_eq!(decoded.digest(), validated.digest());

    let mut invalid = serde_json::to_value(decoded).unwrap();
    invalid["observations"][0]["claim"] = serde_json::json!("tests_passed");
    assert!(serde_json::from_value::<ValidatedWitness>(invalid).is_err());
}

#[test]
fn validation_failures_are_typed_refusals() {
    let mut packet = native_packet("host:one");
    packet.schema = "nq.witness.v2".to_string();
    let error = packet.validate_typed().unwrap_err();
    assert!(matches!(
        error,
        WitnessValidationFailure::UnsupportedSchema { .. }
    ));
    assert_eq!(
        error.refusal().code().as_str(),
        "witness.unsupported_schema"
    );

    let mut packet = native_packet("host:one");
    packet.observations[0]["supports"] = serde_json::json!(["claim:x"]);
    assert!(matches!(
        packet.validate_typed().unwrap_err(),
        WitnessValidationFailure::ObservationNamesClaim {
            key: "supports",
            ..
        }
    ));
}

#[test]
fn pre_extraction_message_error_remains_source_constructible() {
    let error = WitnessValidationError {
        message: "legacy caller diagnostic".to_string(),
    };
    assert_eq!(error.message, error.to_string());

    let mut packet = native_packet("host:one");
    packet.schema = "nq.witness.v2".to_string();
    let error = packet.validate().unwrap_err();
    assert!(error.message.contains("nq.witness.v2"));
}

#[test]
fn native_and_projection_custody_remain_distinct() {
    let mut native = native_packet("host:one");
    native.custody_basis = Some(CUSTODY_BASIS_NATIVE.to_string());
    native.validate().unwrap();

    native.source_finding_ref = Some("finding:legacy".to_string());
    assert!(matches!(
        native.validate_typed().unwrap_err(),
        WitnessValidationFailure::NativeCarriesSourceReference
    ));

    let mut projection = native_packet("host:one");
    projection.custody_basis = Some(CUSTODY_BASIS_EXTERNAL_PROJECTION.to_string());
    projection.source_finding_ref = Some("external:record:one".to_string());
    projection.projection_limits = vec![PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string()];
    projection.validate().unwrap();

    projection.projection_limits.clear();
    assert!(matches!(
        projection.validate_typed().unwrap_err(),
        WitnessValidationFailure::ProjectionLimitsRequired { .. }
    ));
}

#[test]
fn zab2nq_external_projection_crosses_only_the_public_packet_boundary() {
    let packet = zab2nq_fixture();
    assert_eq!(packet.witness_type, "zab2nq_monitor_definition");
    assert_eq!(
        packet.custody_basis.as_deref(),
        Some(CUSTODY_BASIS_EXTERNAL_PROJECTION)
    );
    assert_eq!(packet.position, Some(WitnessPosition::Platform));
    assert!(packet
        .projection_limits
        .iter()
        .any(|limit| limit == PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY));

    let validated = packet.into_validated().unwrap();
    assert_eq!(
        validated.digest().as_str(),
        "sha256:8c47b3c89c598fd9d49620faa1836957ad4e666ddbda919f7e1364064110180c"
    );
}

#[test]
fn packet_set_adoption_is_order_independent_and_digest_ordered() {
    let first = native_packet("host:first");
    let second = native_packet("host:second");
    let forward = adopt_packet_set(vec![first.clone(), second.clone()]).unwrap();
    let reverse = adopt_packet_set(vec![second, first]).unwrap();

    assert_eq!(forward.digest(), reverse.digest());
    assert_eq!(forward.schema(), WITNESS_SET_SCHEMA);
    assert_eq!(
        forward.digest().as_str(),
        "sha256:8f282b49291836783e11ca4a06d3b24dc830296ba69b334caf9e12bb2c92b15d"
    );
    let forward_digests: Vec<&str> = forward
        .witnesses()
        .iter()
        .map(|witness| witness.digest().as_str())
        .collect();
    let reverse_digests: Vec<&str> = reverse
        .witnesses()
        .iter()
        .map(|witness| witness.digest().as_str())
        .collect();
    assert_eq!(forward_digests, reverse_digests);
    assert!(forward_digests.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn duplicate_and_unsupported_packets_have_distinct_typed_failures() {
    let packet = native_packet("host:one");
    let duplicate = adopt_packet_set(vec![packet.clone(), packet]).unwrap_err();
    assert!(matches!(
        duplicate,
        PacketSetAdoptionError::DuplicatePacket { .. }
    ));
    assert_eq!(
        duplicate.refusal().code().as_str(),
        "witness.duplicate_packet"
    );

    let mut unsupported = native_packet("host:two");
    unsupported.schema = "nq.witness.v2".to_string();
    let unsupported = adopt_packet_set(vec![unsupported]).unwrap_err();
    assert!(matches!(
        unsupported,
        PacketSetAdoptionError::UnsupportedSchema { .. }
    ));
    assert_eq!(
        unsupported.refusal().code().as_str(),
        "witness.unsupported_schema"
    );
}

#[test]
fn malformed_packet_is_not_laundered_as_unsupported() {
    let mut packet = native_packet("host:one");
    packet.observed_at = "recently".to_string();
    let error = adopt_packet_set(vec![packet]).unwrap_err();
    assert!(matches!(
        error,
        PacketSetAdoptionError::InvalidPacket {
            source: WitnessValidationFailure::InvalidTimestamp { .. },
            ..
        }
    ));
}

#[test]
fn packet_set_does_not_invent_observation_equivalence_or_contradiction() {
    let first = zab2nq_fixture();
    let mut reemitted = first.clone();
    reemitted.generated_at = "2026-07-27T03:35:17Z".to_string();
    assert_eq!(first.source_finding_ref, reemitted.source_finding_ref);
    assert_ne!(first.digest().unwrap(), reemitted.digest().unwrap());

    let set = adopt_packet_set(vec![first, reemitted]).unwrap();
    assert_eq!(set.len(), 2);
}

#[test]
fn jcs_sorts_object_keys_but_preserves_array_order() {
    let mut first = native_packet("host:one");
    first.observations = vec![
        serde_json::json!({"type": "a", "value": 1}),
        serde_json::json!({"type": "b", "value": 2}),
    ];
    let mut reordered_keys = first.clone();
    reordered_keys.observations[0] = serde_json::json!({"value": 1, "type": "a"});
    assert_eq!(first.digest().unwrap(), reordered_keys.digest().unwrap());

    let mut reordered_array = first.clone();
    reordered_array.observations.reverse();
    assert_ne!(first.digest().unwrap(), reordered_array.digest().unwrap());
}
