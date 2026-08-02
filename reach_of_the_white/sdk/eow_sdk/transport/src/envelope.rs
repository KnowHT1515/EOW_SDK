//! SDK Envelope parsing, registry validation, and connection-route checks.

use serde_json::{Map, Value};

use crate::endpoint::InstanceRegistry;
use crate::error::EnvelopeError;
use crate::event_id::EventId;
use crate::limits::{MAX_ENVELOPE_BYTES, SDK_ENVELOPE_SCHEMA};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeKind {
    Request,
    Response,
    Event,
}

impl EnvelopeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Response => "response",
            Self::Event => "event",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "request" => Some(Self::Request),
            "response" => Some(Self::Response),
            "event" => Some(Self::Event),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeViolation {
    EnvelopeSchema,
    EnvelopeKind,
    SourceTarget,
    EventId,
    CorrelationId,
    Payload,
}

impl EnvelopeViolation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnvelopeSchema => "envelope_schema",
            Self::EnvelopeKind => "envelope_kind",
            Self::SourceTarget => "source_target",
            Self::EventId => "event_id",
            Self::CorrelationId => "correlation_id",
            Self::Payload => "payload",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SdkEnvelope {
    object: Map<String, Value>,
}

impl SdkEnvelope {
    pub fn parse(bytes: &[u8]) -> Result<Self, EnvelopeError> {
        let registry = InstanceRegistry::bundled()?;
        Self::parse_with_registry(bytes, &registry)
    }

    pub fn parse_with_registry(
        bytes: &[u8],
        registry: &InstanceRegistry,
    ) -> Result<Self, EnvelopeError> {
        if bytes.len() > MAX_ENVELOPE_BYTES {
            return Err(EnvelopeError::Oversize {
                actual: bytes.len(),
                maximum: MAX_ENVELOPE_BYTES,
            });
        }
        let value: Value = serde_json::from_slice(bytes)?;
        Self::try_from_value_with_registry(value, registry)
    }

    pub fn try_from_value(value: Value) -> Result<Self, EnvelopeError> {
        let registry = InstanceRegistry::bundled()?;
        Self::try_from_value_with_registry(value, &registry)
    }

    pub fn try_from_value_with_registry(
        value: Value,
        registry: &InstanceRegistry,
    ) -> Result<Self, EnvelopeError> {
        let serialized_len = serde_json::to_vec(&value)?.len();
        if serialized_len > MAX_ENVELOPE_BYTES {
            return Err(EnvelopeError::Oversize {
                actual: serialized_len,
                maximum: MAX_ENVELOPE_BYTES,
            });
        }

        let object = match value {
            Value::Object(object) => object,
            _ => {
                return Err(EnvelopeError::Invalid(vec![
                    EnvelopeViolation::EnvelopeSchema,
                    EnvelopeViolation::EnvelopeKind,
                    EnvelopeViolation::SourceTarget,
                    EnvelopeViolation::EventId,
                    EnvelopeViolation::CorrelationId,
                    EnvelopeViolation::Payload,
                ]));
            }
        };

        let violations = validate_object(&object, registry);
        if violations.is_empty() {
            Ok(Self { object })
        } else {
            Err(EnvelopeError::Invalid(violations))
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_fields(
        kind: EnvelopeKind,
        source: impl Into<String>,
        target: impl Into<String>,
        event_id: EventId,
        correlation_id: Option<EventId>,
        ts_utc: Value,
        payload: Map<String, Value>,
    ) -> Result<Self, EnvelopeError> {
        let registry = InstanceRegistry::bundled()?;
        Self::from_fields_with_registry(
            kind,
            source,
            target,
            event_id,
            correlation_id,
            ts_utc,
            payload,
            &registry,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_fields_with_registry(
        kind: EnvelopeKind,
        source: impl Into<String>,
        target: impl Into<String>,
        event_id: EventId,
        correlation_id: Option<EventId>,
        ts_utc: Value,
        payload: Map<String, Value>,
        registry: &InstanceRegistry,
    ) -> Result<Self, EnvelopeError> {
        let mut object = Map::new();
        object.insert(
            "envelope_schema".into(),
            Value::String(SDK_ENVELOPE_SCHEMA.into()),
        );
        object.insert("envelope_kind".into(), Value::String(kind.as_str().into()));
        object.insert("source".into(), Value::String(source.into()));
        object.insert("target".into(), Value::String(target.into()));
        object.insert("event_id".into(), Value::String(event_id.into_string()));
        object.insert(
            "correlation_id".into(),
            correlation_id
                .map(|id| Value::String(id.into_string()))
                .unwrap_or(Value::Null),
        );
        object.insert("ts_utc".into(), ts_utc);
        object.insert("payload".into(), Value::Object(payload));
        Self::try_from_value_with_registry(Value::Object(object), registry)
    }

    pub fn outbound_violations(&self, sdk_id: &str, registry: &InstanceRegistry) -> Vec<String> {
        let mut violations = Vec::new();
        if self.source() != "eow_sdk" || !registry.is_registered_instance(self.target()) {
            violations.push(EnvelopeViolation::SourceTarget.as_str().to_owned());
        }
        if self.payload_sdk_id() != Some(sdk_id) {
            violations.push(EnvelopeViolation::Payload.as_str().to_owned());
        }
        violations
    }

    pub fn inbound_violations(
        &self,
        sdk_id: &str,
        connected_instance: &str,
        registry: &InstanceRegistry,
    ) -> Vec<String> {
        let mut violations = Vec::new();
        if self.source() != connected_instance
            || self.target() != "eow_sdk"
            || !registry.is_registered_instance(connected_instance)
        {
            violations.push(EnvelopeViolation::SourceTarget.as_str().to_owned());
        }
        if self.payload_sdk_id() != Some(sdk_id) {
            violations.push(EnvelopeViolation::Payload.as_str().to_owned());
        }
        violations
    }

    pub fn as_object(&self) -> &Map<String, Value> {
        &self.object
    }

    pub fn envelope_kind(&self) -> EnvelopeKind {
        let value = self.object["envelope_kind"]
            .as_str()
            .expect("validated envelope_kind is a string");
        EnvelopeKind::parse(value).expect("validated envelope_kind is known")
    }

    pub fn source(&self) -> &str {
        self.object["source"]
            .as_str()
            .expect("validated source is a string")
    }

    pub fn target(&self) -> &str {
        self.object["target"]
            .as_str()
            .expect("validated target is a string")
    }

    pub fn event_id(&self) -> &str {
        self.object["event_id"]
            .as_str()
            .expect("validated event_id is a string")
    }

    pub fn correlation_id(&self) -> Option<&str> {
        self.object["correlation_id"].as_str()
    }

    pub fn payload(&self) -> &Map<String, Value> {
        self.object["payload"]
            .as_object()
            .expect("validated payload is an object")
    }

    pub fn payload_sdk_id(&self) -> Option<&str> {
        self.payload().get("sdk_id").and_then(Value::as_str)
    }

    pub fn to_json_vec(&self) -> Vec<u8> {
        serde_json::to_vec(&self.object).expect("serde_json::Value always serializes")
    }

    pub fn into_value(self) -> Value {
        Value::Object(self.object)
    }
}

fn validate_object(
    object: &Map<String, Value>,
    registry: &InstanceRegistry,
) -> Vec<EnvelopeViolation> {
    let mut violations = Vec::new();

    if object.get("envelope_schema").and_then(Value::as_str) != Some(SDK_ENVELOPE_SCHEMA) {
        violations.push(EnvelopeViolation::EnvelopeSchema);
    }
    let envelope_kind = object
        .get("envelope_kind")
        .and_then(Value::as_str)
        .and_then(EnvelopeKind::parse);
    if envelope_kind.is_none() {
        violations.push(EnvelopeViolation::EnvelopeKind);
    }

    let source = object.get("source").and_then(Value::as_str);
    let target = object.get("target").and_then(Value::as_str);
    if source.is_none()
        || target.is_none()
        || source == target
        || source.is_some_and(|value| !registry.is_envelope_endpoint(value))
        || target.is_some_and(|value| !registry.is_envelope_endpoint(value))
    {
        violations.push(EnvelopeViolation::SourceTarget);
    }
    if !object
        .get("event_id")
        .and_then(Value::as_str)
        .is_some_and(EventId::is_valid)
    {
        violations.push(EnvelopeViolation::EventId);
    }

    let correlation_valid = match (envelope_kind, object.get("correlation_id")) {
        (Some(EnvelopeKind::Response), Some(Value::String(value))) => EventId::is_valid(value),
        (Some(EnvelopeKind::Request | EnvelopeKind::Event), Some(Value::Null)) => true,
        (None, Some(Value::Null)) => true,
        (None, Some(Value::String(value))) => EventId::is_valid(value),
        _ => false,
    };
    if !correlation_valid {
        violations.push(EnvelopeViolation::CorrelationId);
    }
    if object
        .get("payload")
        .and_then(Value::as_object)
        .is_none_or(|payload| payload.is_empty())
    {
        violations.push(EnvelopeViolation::Payload);
    }

    violations
}
