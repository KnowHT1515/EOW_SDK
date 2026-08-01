//! SDK Envelope parsing and the six static validation rules.


use serde_json::{Map, Value};

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

/// The six and only six static Envelope violation classes.
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

/// A JSON object that passed the documented size limit and six static rules.
/// Unknown and deliberately unchecked fields are retained verbatim.
#[derive(Debug, Clone, PartialEq)]
pub struct SdkEnvelope {
    object: Map<String, Value>,
}

impl SdkEnvelope {
    pub fn parse(bytes: &[u8]) -> Result<Self, EnvelopeError> {
        if bytes.len() > MAX_ENVELOPE_BYTES {
            return Err(EnvelopeError::Oversize {
                actual: bytes.len(),
                maximum: MAX_ENVELOPE_BYTES,
            });
        }
        let value: Value = serde_json::from_slice(bytes)?;
        Self::try_from_value(value)
    }

    pub fn try_from_value(value: Value) -> Result<Self, EnvelopeError> {
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

        let violations = validate_object(&object);
        if violations.is_empty() {
            Ok(Self { object })
        } else {
            Err(EnvelopeError::Invalid(violations))
        }
    }

    /// Construct a normal outbound object, then pass it through the same validator.
    /// `ts_utc` is inserted but intentionally not inspected.
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
        Self::try_from_value(Value::Object(object))
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

    pub fn to_json_vec(&self) -> Vec<u8> {
        serde_json::to_vec(&self.object).expect("serde_json::Value always serializes")
    }

    pub fn into_value(self) -> Value {
        Value::Object(self.object)
    }
}

fn validate_object(object: &Map<String, Value>) -> Vec<EnvelopeViolation> {
    let mut violations = Vec::new();

    if object.get("envelope_schema").and_then(Value::as_str) != Some(SDK_ENVELOPE_SCHEMA) {
        violations.push(EnvelopeViolation::EnvelopeSchema);
    }
    if object
        .get("envelope_kind")
        .and_then(Value::as_str)
        .and_then(EnvelopeKind::parse)
        .is_none()
    {
        violations.push(EnvelopeViolation::EnvelopeKind);
    }

    let source = object.get("source").and_then(Value::as_str);
    let target = object.get("target").and_then(Value::as_str);
    if source.is_none() || target.is_none() || source == target {
        violations.push(EnvelopeViolation::SourceTarget);
    }
    if !object
        .get("event_id")
        .and_then(Value::as_str)
        .is_some_and(EventId::is_valid)
    {
        violations.push(EnvelopeViolation::EventId);
    }

    let correlation_valid = match object.get("correlation_id") {
        Some(Value::Null) => true,
        Some(Value::String(value)) => EventId::is_valid(value),
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
