use eow_sdk_transport::limits::MAX_ENVELOPE_BYTES;
use eow_sdk_transport::{
    EnvelopeError, EnvelopeKind, EnvelopeViolation, InstanceRegistry, SdkEnvelope,
};
use serde_json::{Value, json};

fn valid() -> Value {
    json!({
        "envelope_schema": "1.0",
        "envelope_kind": "request",
        "source": "eow_sdk",
        "target": "silvy",
        "event_id": "712345678901",
        "correlation_id": null,
        "payload": {"sdk_id": "Cyberpunk", "type": "startup"}
    })
}

fn violations(value: Value) -> Vec<EnvelopeViolation> {
    match SdkEnvelope::try_from_value(value).unwrap_err() {
        EnvelopeError::Invalid(violations) => violations,
        other => panic!("expected static violations, got {other:?}"),
    }
}

#[test]
fn accepts_without_ts_utc_and_preserves_unknown_fields() {
    let mut value = valid();
    value["extension"] = json!({"future": true});
    let envelope = SdkEnvelope::try_from_value(value).unwrap();
    assert_eq!(envelope.envelope_kind(), EnvelopeKind::Request);
    assert_eq!(envelope.as_object()["extension"]["future"], true);
    assert!(envelope.as_object().get("ts_utc").is_none());
}

#[test]
fn endpoints_must_be_registered_instances_or_eow_sdk() {
    let mut value = valid();
    value["target"] = json!("custom");
    assert_eq!(violations(value), [EnvelopeViolation::SourceTarget]);

    let mut no_port_is_still_registered = valid();
    no_port_is_still_registered["target"] = json!("neuro");
    assert!(SdkEnvelope::try_from_value(no_port_is_still_registered).is_ok());
}

#[test]
fn connection_direction_and_sdk_id_are_contextual_rules() {
    let registry = InstanceRegistry::bundled().unwrap();
    let outbound = SdkEnvelope::try_from_value(valid()).unwrap();
    assert!(
        outbound
            .outbound_violations("Cyberpunk", &registry)
            .is_empty()
    );
    assert_eq!(
        outbound.outbound_violations("Other", &registry),
        ["payload"]
    );

    let mut inbound = valid();
    inbound["source"] = json!("silvy");
    inbound["target"] = json!("eow_sdk");
    let inbound = SdkEnvelope::try_from_value(inbound).unwrap();
    assert!(
        inbound
            .inbound_violations("Cyberpunk", "silvy", &registry)
            .is_empty()
    );
    assert_eq!(
        inbound.inbound_violations("Cyberpunk", "rucia", &registry),
        ["source_target"]
    );
}

#[test]
fn each_documented_rule_has_exactly_one_violation_class() {
    let cases = [
        (
            "envelope_schema",
            json!("2.0"),
            EnvelopeViolation::EnvelopeSchema,
        ),
        (
            "envelope_kind",
            json!("stream"),
            EnvelopeViolation::EnvelopeKind,
        ),
        ("event_id", json!("123"), EnvelopeViolation::EventId),
        (
            "correlation_id",
            json!(42),
            EnvelopeViolation::CorrelationId,
        ),
        ("payload", json!({}), EnvelopeViolation::Payload),
    ];

    for (field, replacement, expected) in cases {
        let mut value = valid();
        value[field] = replacement;
        assert_eq!(violations(value), [expected], "field {field}");
    }

    let mut equal_endpoints = valid();
    equal_endpoints["target"] = equal_endpoints["source"].clone();
    assert_eq!(
        violations(equal_endpoints),
        [EnvelopeViolation::SourceTarget]
    );
}

#[test]
fn missing_correlation_id_is_not_null_and_is_rejected() {
    let mut value = valid();
    value.as_object_mut().unwrap().remove("correlation_id");
    assert_eq!(violations(value), [EnvelopeViolation::CorrelationId]);
}

#[test]
fn event_ids_require_ascii_digits_not_unicode_digits() {
    for bad in [
        "12345678901",
        "1234567890123",
        "12345678901x",
        "????????????",
    ] {
        let mut value = valid();
        value["event_id"] = json!(bad);
        assert_eq!(violations(value), [EnvelopeViolation::EventId]);
    }
}

#[test]
fn collects_all_six_violations_in_stable_order() {
    assert_eq!(
        violations(json!({})),
        [
            EnvelopeViolation::EnvelopeSchema,
            EnvelopeViolation::EnvelopeKind,
            EnvelopeViolation::SourceTarget,
            EnvelopeViolation::EventId,
            EnvelopeViolation::CorrelationId,
            EnvelopeViolation::Payload,
        ]
    );
}

#[test]
fn raw_input_over_size_limit_is_rejected_before_json_parsing() {
    let bytes = vec![b' '; MAX_ENVELOPE_BYTES + 1];
    assert!(matches!(
        SdkEnvelope::parse(&bytes),
        Err(EnvelopeError::Oversize { actual, maximum })
            if actual == MAX_ENVELOPE_BYTES + 1 && maximum == MAX_ENVELOPE_BYTES
    ));
}

#[test]
fn serialized_value_honors_exact_size_boundary() {
    let mut value = valid();
    value["payload"]["pad"] = json!("");
    let base_len = serde_json::to_vec(&value).unwrap().len();
    let pad_len = MAX_ENVELOPE_BYTES - base_len;
    value["payload"]["pad"] = json!("x".repeat(pad_len));
    assert_eq!(
        serde_json::to_vec(&value).unwrap().len(),
        MAX_ENVELOPE_BYTES
    );
    assert!(SdkEnvelope::try_from_value(value.clone()).is_ok());

    value["payload"]["pad"] = json!("x".repeat(pad_len + 1));
    assert!(matches!(
        SdkEnvelope::try_from_value(value),
        Err(EnvelopeError::Oversize { .. })
    ));
}
