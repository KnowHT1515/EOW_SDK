use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

pub const MAX_ENVELOPE_BYTES: usize = 15 * 1024 * 1024;
const ENVELOPE_SCHEMA: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeKind {
    Request,
    Response,
    Event,
}

impl EnvelopeKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "request" => Some(Self::Request),
            "response" => Some(Self::Response),
            "event" => Some(Self::Event),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Response => "response",
            Self::Event => "event",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Envelope {
    object: Map<String, Value>,
}

impl Envelope {
    pub fn parse(body: &[u8]) -> Result<Self, EnvelopeError> {
        if body.len() > MAX_ENVELOPE_BYTES {
            return Err(EnvelopeError::Oversize {
                actual: body.len(),
                maximum: MAX_ENVELOPE_BYTES,
            });
        }

        let value: Value = serde_json::from_slice(body)?;
        let object = value
            .as_object()
            .cloned()
            .ok_or_else(|| EnvelopeError::Invalid(vec!["object"]))?;
        let violations = validate_object(&object);
        if !violations.is_empty() {
            return Err(EnvelopeError::Invalid(violations));
        }
        Ok(Self { object })
    }

    pub fn validate_inbound_for_eow(&self) -> Result<(), EnvelopeError> {
        if self.source() == "eow_sdk" && self.target() == "eow" {
            Ok(())
        } else {
            Err(EnvelopeError::WrongRoute {
                source: self.source().to_owned(),
                target: self.target().to_owned(),
            })
        }
    }

    pub fn ready(instance: &str, metadata: &EventMetadataGenerator) -> Self {
        let generated = metadata.next();
        let payload = json!({
            "type": "fake_eow.ready",
            "instance": instance,
        })
        .as_object()
        .cloned()
        .expect("static payload is an object");

        Self::from_fields(
            EnvelopeKind::Event,
            "eow",
            "eow_sdk",
            generated.event_id,
            None,
            generated.timestamp,
            payload,
        )
    }

    pub fn echo_response(
        request: &Self,
        metadata: &EventMetadataGenerator,
    ) -> Result<Self, EnvelopeError> {
        if request.kind() != EnvelopeKind::Request {
            return Err(EnvelopeError::ExpectedRequest);
        }

        let generated = metadata.next();
        let request_id = request.event_id().to_owned();
        let mut payload = Map::new();
        payload.insert("type".into(), Value::String("fake_eow.echo".into()));
        payload.insert(
            "received_event_id".into(),
            Value::String(request_id.clone()),
        );
        payload.insert(
            "received_payload".into(),
            Value::Object(request.payload().clone()),
        );

        Ok(Self::from_fields(
            EnvelopeKind::Response,
            "eow",
            "eow_sdk",
            generated.event_id,
            Some(request_id),
            generated.timestamp,
            payload,
        ))
    }

    pub fn kind(&self) -> EnvelopeKind {
        EnvelopeKind::parse(
            self.object["envelope_kind"]
                .as_str()
                .expect("validated envelope kind is a string"),
        )
        .expect("validated envelope kind is known")
    }

    pub fn event_id(&self) -> &str {
        self.object["event_id"]
            .as_str()
            .expect("validated event ID is a string")
    }

    pub fn correlation_id(&self) -> Option<&str> {
        self.object["correlation_id"].as_str()
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

    pub fn payload(&self) -> &Map<String, Value> {
        self.object["payload"]
            .as_object()
            .expect("validated payload is an object")
    }

    pub fn to_json_vec(&self) -> Vec<u8> {
        serde_json::to_vec(&self.object).expect("JSON values always serialize")
    }

    #[allow(clippy::too_many_arguments)]
    fn from_fields(
        kind: EnvelopeKind,
        source: &str,
        target: &str,
        event_id: String,
        correlation_id: Option<String>,
        timestamp: String,
        payload: Map<String, Value>,
    ) -> Self {
        let mut object = Map::new();
        object.insert(
            "envelope_schema".into(),
            Value::String(ENVELOPE_SCHEMA.into()),
        );
        object.insert("envelope_kind".into(), Value::String(kind.as_str().into()));
        object.insert("source".into(), Value::String(source.into()));
        object.insert("target".into(), Value::String(target.into()));
        object.insert("event_id".into(), Value::String(event_id));
        object.insert(
            "correlation_id".into(),
            correlation_id.map(Value::String).unwrap_or(Value::Null),
        );
        object.insert("ts_utc".into(), Value::String(timestamp));
        object.insert("payload".into(), Value::Object(payload));
        Self { object }
    }
}

fn validate_object(object: &Map<String, Value>) -> Vec<&'static str> {
    let mut violations = Vec::new();

    if object.get("envelope_schema").and_then(Value::as_str) != Some(ENVELOPE_SCHEMA) {
        violations.push("envelope_schema");
    }
    if object
        .get("envelope_kind")
        .and_then(Value::as_str)
        .and_then(EnvelopeKind::parse)
        .is_none()
    {
        violations.push("envelope_kind");
    }

    let source = object.get("source").and_then(Value::as_str);
    let target = object.get("target").and_then(Value::as_str);
    if source.is_none() || target.is_none() || source == target {
        violations.push("source_target");
    }
    if !object
        .get("event_id")
        .and_then(Value::as_str)
        .is_some_and(valid_event_id)
    {
        violations.push("event_id");
    }

    let correlation_valid = match object.get("correlation_id") {
        Some(Value::Null) => true,
        Some(Value::String(value)) => valid_event_id(value),
        _ => false,
    };
    if !correlation_valid {
        violations.push("correlation_id");
    }
    if object
        .get("payload")
        .and_then(Value::as_object)
        .is_none_or(Map::is_empty)
    {
        violations.push("payload");
    }

    violations
}

fn valid_event_id(value: &str) -> bool {
    value.len() == 12 && value.bytes().all(|byte| byte.is_ascii_digit())
}

#[derive(Debug)]
pub enum EnvelopeError {
    Json(serde_json::Error),
    Oversize { actual: usize, maximum: usize },
    Invalid(Vec<&'static str>),
    WrongRoute { source: String, target: String },
    ExpectedRequest,
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid Envelope JSON: {error}"),
            Self::Oversize { actual, maximum } => {
                write!(
                    formatter,
                    "Envelope is {actual} bytes; maximum is {maximum}"
                )
            }
            Self::Invalid(violations) => {
                write!(formatter, "Envelope failed validation: {violations:?}")
            }
            Self::WrongRoute { source, target } => {
                write!(
                    formatter,
                    "Envelope route must be eow_sdk -> eow, got {source} -> {target}"
                )
            }
            Self::ExpectedRequest => formatter.write_str("echo response requires a request"),
        }
    }
}

impl std::error::Error for EnvelopeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for EnvelopeError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Debug, Default)]
pub struct EventMetadataGenerator {
    sequence: AtomicU64,
}

impl EventMetadataGenerator {
    pub fn next(&self) -> EventMetadata {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let seconds_in_day = now.as_secs() % 86_400;
        let hour = seconds_in_day / 3_600;
        let minute = (seconds_in_day % 3_600) / 60;
        let second = seconds_in_day % 60;
        let suffix = self.sequence.fetch_add(1, Ordering::Relaxed) % 100;
        let event_id = format!(
            "7{hour:02}{minute:02}{second:02}{:03}{suffix:02}",
            now.subsec_millis()
        );

        EventMetadata {
            event_id,
            timestamp: format_utc(now.as_secs(), now.subsec_millis()),
        }
    }
}

#[derive(Debug)]
pub struct EventMetadata {
    pub event_id: String,
    pub timestamp: String,
}

fn format_utc(epoch_seconds: u64, milliseconds: u32) -> String {
    let days = (epoch_seconds / 86_400) as i64;
    let seconds_in_day = epoch_seconds % 86_400;
    let hour = seconds_in_day / 3_600;
    let minute = (seconds_in_day % 3_600) / 60;
    let second = seconds_in_day % 60;
    let (year, month, day) = civil_date_from_unix_days(days);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milliseconds:03}Z")
}

// Gregorian civil date conversion for days since 1970-01-01.
fn civil_date_from_unix_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(source: &str, target: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "envelope_schema": "1.0",
            "envelope_kind": "request",
            "source": source,
            "target": target,
            "event_id": "712345678901",
            "correlation_id": null,
            "ts_utc": "2026-08-01T00:00:00.000Z",
            "payload": {"action": "ping"}
        }))
        .unwrap()
    }

    #[test]
    fn parses_and_route_checks_an_sdk_request() {
        let envelope = Envelope::parse(&request("eow_sdk", "eow")).unwrap();
        assert_eq!(envelope.kind(), EnvelopeKind::Request);
        assert!(envelope.validate_inbound_for_eow().is_ok());

        let wrong = Envelope::parse(&request("eow", "eow_sdk")).unwrap();
        assert!(matches!(
            wrong.validate_inbound_for_eow(),
            Err(EnvelopeError::WrongRoute { .. })
        ));
    }

    #[test]
    fn rejects_static_envelope_violations() {
        let mut value: Value = serde_json::from_slice(&request("eow_sdk", "eow")).unwrap();
        value["event_id"] = Value::String("short".into());
        value["payload"] = json!({});

        assert!(matches!(
            Envelope::parse(&serde_json::to_vec(&value).unwrap()),
            Err(EnvelopeError::Invalid(violations))
                if violations == ["event_id", "payload"]
        ));
    }

    #[test]
    fn builds_ready_event_and_correlated_echo_response() {
        let metadata = EventMetadataGenerator::default();
        let ready = Envelope::ready("knowht", &metadata);
        assert_eq!(ready.kind(), EnvelopeKind::Event);
        assert_eq!(ready.source(), "eow");
        assert_eq!(ready.target(), "eow_sdk");
        assert_eq!(ready.payload()["type"], "fake_eow.ready");

        let request = Envelope::parse(&request("eow_sdk", "eow")).unwrap();
        let response = Envelope::echo_response(&request, &metadata).unwrap();
        assert_eq!(response.kind(), EnvelopeKind::Response);
        assert_eq!(response.correlation_id(), Some("712345678901"));
        assert_eq!(response.payload()["received_payload"]["action"], "ping");
    }

    #[test]
    fn generated_metadata_uses_wire_formats() {
        let metadata = EventMetadataGenerator::default().next();
        assert!(valid_event_id(&metadata.event_id));
        assert_eq!(metadata.timestamp.len(), 24);
        assert!(metadata.timestamp.ends_with('Z'));
        assert_eq!(format_utc(0, 0), "1970-01-01T00:00:00.000Z");
        assert_eq!(format_utc(951_782_400, 123), "2000-02-29T00:00:00.123Z");
    }
}
