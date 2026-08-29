use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use crate::config::NEURO_INSTANCE;
use crate::error::ProtocolError;

pub(crate) const MAX_ENVELOPE_BYTES: usize = 15 * 1024 * 1024;
const ENVELOPE_SCHEMA: &str = "1.0";
const HANDSHAKE_SCHEMA: &str = "1.0";
const MAX_SDK_ID_BYTES: usize = 128;
static EVENT_SEQUENCE: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnvelopeKind {
    Request,
    Response,
    Event,
}

#[derive(Debug)]
pub(crate) struct SdkEnvelope {
    pub kind: EnvelopeKind,
    pub event_id: String,
    pub payload: Map<String, Value>,
}

impl SdkEnvelope {
    pub(crate) fn parse(body: &[u8], sdk_id: &str) -> Result<Self, ProtocolError> {
        if body.len() > MAX_ENVELOPE_BYTES {
            return Err(ProtocolError::Invalid("envelope_size"));
        }
        let value: Value = serde_json::from_slice(body)?;
        let object = value
            .as_object()
            .ok_or(ProtocolError::Invalid("envelope"))?;
        if object.get("envelope_schema").and_then(Value::as_str) != Some(ENVELOPE_SCHEMA) {
            return Err(ProtocolError::Invalid("envelope_schema"));
        }
        let kind = match object.get("envelope_kind").and_then(Value::as_str) {
            Some("request") => EnvelopeKind::Request,
            Some("response") => EnvelopeKind::Response,
            Some("event") => EnvelopeKind::Event,
            _ => return Err(ProtocolError::Invalid("envelope_kind")),
        };
        if object.get("source").and_then(Value::as_str) != Some("eow_sdk")
            || object.get("target").and_then(Value::as_str) != Some(NEURO_INSTANCE)
        {
            return Err(ProtocolError::Invalid("source_target"));
        }
        let event_id = object
            .get("event_id")
            .and_then(Value::as_str)
            .filter(|value| is_event_id(value))
            .ok_or(ProtocolError::Invalid("event_id"))?
            .to_owned();
        match (kind, object.get("correlation_id")) {
            (EnvelopeKind::Request | EnvelopeKind::Event, Some(Value::Null)) => None,
            (EnvelopeKind::Response, Some(Value::String(value))) if is_event_id(value) => {
                Some(value.clone())
            }
            _ => return Err(ProtocolError::Invalid("correlation_id")),
        };
        let payload = object
            .get("payload")
            .and_then(Value::as_object)
            .filter(|payload| !payload.is_empty())
            .ok_or(ProtocolError::Invalid("payload"))?
            .clone();
        if payload.get("sdk_id").and_then(Value::as_str) != Some(sdk_id) {
            return Err(ProtocolError::Invalid("payload.sdk_id"));
        }
        Ok(Self {
            kind,
            event_id,
            payload,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SdkAction {
    pub action_id: String,
    pub description: String,
    pub arguments: Option<Map<String, Value>>,
    pub required: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BusinessMessage {
    Startup,
    Shutdown,
    Context {
        content: String,
        silent: bool,
    },
    ActionRegister(Vec<SdkAction>),
    ActionUnregister(Vec<String>),
    ActionUpdate(Vec<SdkAction>),
    ActionClear,
    ActionDemand {
        state: Option<String>,
        goal: String,
        permanent_context: bool,
        priority: String,
        actions: Vec<String>,
    },
    ActionResult {
        id: String,
        ok: bool,
        result: Option<String>,
    },
}

pub(crate) fn parse_business(envelope: &SdkEnvelope) -> Result<BusinessMessage, ProtocolError> {
    let message_type = required_string(&envelope.payload, "type")?;
    match message_type {
        "startup" => require_kind(
            envelope.kind,
            EnvelopeKind::Request,
            BusinessMessage::Startup,
        ),
        "shutdown" => require_kind(
            envelope.kind,
            EnvelopeKind::Request,
            BusinessMessage::Shutdown,
        ),
        "context" => {
            ensure_kind(envelope.kind, EnvelopeKind::Request)?;
            let data = required_object(&envelope.payload, "data")?;
            Ok(BusinessMessage::Context {
                content: required_string(data, "content")?.to_owned(),
                silent: required_bool(data, "silent")?,
            })
        }
        "action" => parse_action(envelope),
        _ => Err(ProtocolError::Invalid("payload.type")),
    }
}

fn parse_action(envelope: &SdkEnvelope) -> Result<BusinessMessage, ProtocolError> {
    match envelope.payload.get("kind").and_then(Value::as_str) {
        Some("register") => {
            ensure_kind(envelope.kind, EnvelopeKind::Request)?;
            Ok(BusinessMessage::ActionRegister(parse_actions(
                envelope.payload.get("data"),
            )?))
        }
        Some("unregister") => {
            ensure_kind(envelope.kind, EnvelopeKind::Request)?;
            Ok(BusinessMessage::ActionUnregister(parse_string_array(
                envelope.payload.get("data"),
                "payload.data",
            )?))
        }
        Some("update") => {
            ensure_kind(envelope.kind, EnvelopeKind::Request)?;
            Ok(BusinessMessage::ActionUpdate(parse_actions(
                envelope.payload.get("data"),
            )?))
        }
        Some("clear") => require_kind(
            envelope.kind,
            EnvelopeKind::Request,
            BusinessMessage::ActionClear,
        ),
        Some("demand") => {
            ensure_kind(envelope.kind, EnvelopeKind::Request)?;
            let data = required_object(&envelope.payload, "data")?;
            let priority = required_string(data, "priority")?;
            if !matches!(priority, "low" | "medium" | "high" | "critical") {
                return Err(ProtocolError::Invalid("payload.data.priority"));
            }
            Ok(BusinessMessage::ActionDemand {
                state: optional_string(data, "state")?,
                goal: required_string(data, "goal")?.to_owned(),
                permanent_context: required_bool(data, "permanent_context")?,
                priority: priority.to_owned(),
                actions: parse_string_array(data.get("actions"), "payload.data.actions")?,
            })
        }
        Some("result") => {
            ensure_kind(envelope.kind, EnvelopeKind::Response)?;
            let data = required_object(&envelope.payload, "data")?;
            Ok(BusinessMessage::ActionResult {
                id: required_string(data, "id")?.to_owned(),
                ok: required_bool(data, "ok")?,
                result: optional_string(data, "result")?,
            })
        }
        _ => Err(ProtocolError::Invalid("payload.kind")),
    }
}

fn parse_actions(value: Option<&Value>) -> Result<Vec<SdkAction>, ProtocolError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or(ProtocolError::Invalid("payload.data"))?;
    let mut seen = HashSet::new();
    let mut actions = Vec::with_capacity(values.len());
    for value in values {
        let object = value
            .as_object()
            .ok_or(ProtocolError::Invalid("payload.data.action"))?;
        let action_id = required_string(object, "action_id")?;
        if action_id.is_empty() || !seen.insert(action_id.to_owned()) {
            return Err(ProtocolError::Invalid("payload.data.action_id"));
        }
        let arguments = match object.get("arguments") {
            None => None,
            Some(Value::Object(arguments)) => {
                validate_arguments(arguments)?;
                Some(arguments.clone())
            }
            Some(_) => return Err(ProtocolError::Invalid("payload.data.arguments")),
        };
        let required = match object.get("required") {
            None => Vec::new(),
            Some(value) => parse_string_array(Some(value), "payload.data.required")?,
        };
        let argument_names = arguments.as_ref().map_or_else(BTreeMap::new, |arguments| {
            arguments
                .keys()
                .map(|name| (name.as_str(), ()))
                .collect::<BTreeMap<_, _>>()
        });
        if required
            .iter()
            .any(|name| !argument_names.contains_key(name.as_str()))
        {
            return Err(ProtocolError::Invalid("payload.data.required"));
        }
        actions.push(SdkAction {
            action_id: action_id.to_owned(),
            description: required_string(object, "description")?.to_owned(),
            arguments,
            required,
        });
    }
    Ok(actions)
}

fn validate_arguments(arguments: &Map<String, Value>) -> Result<(), ProtocolError> {
    for schema in arguments.values() {
        let schema = schema
            .as_object()
            .ok_or(ProtocolError::Invalid("payload.data.arguments"))?;
        match required_string(schema, "type")? {
            "string" => {
                if let Some(values) = schema.get("enum") {
                    parse_string_array(Some(values), "payload.data.arguments.enum")?;
                }
            }
            "integer" => {
                let minimum = optional_i64(schema, "minimum")?;
                let maximum = optional_i64(schema, "maximum")?;
                if minimum
                    .zip(maximum)
                    .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err(ProtocolError::Invalid("payload.data.arguments.range"));
                }
            }
            "boolean" => {}
            _ => return Err(ProtocolError::Invalid("payload.data.arguments.type")),
        }
    }
    Ok(())
}

pub(crate) fn parse_handshake(body: &[u8]) -> Result<String, ProtocolError> {
    let value: Value = serde_json::from_slice(body)?;
    let handshake = value
        .get("handshake")
        .and_then(Value::as_object)
        .ok_or(ProtocolError::Invalid("handshake"))?;
    if handshake.get("schema").and_then(Value::as_str) != Some(HANDSHAKE_SCHEMA) {
        return Err(ProtocolError::Invalid("handshake.schema"));
    }
    let sdk_id = handshake
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| is_sdk_id(value))
        .ok_or(ProtocolError::Invalid("handshake.id"))?;
    Ok(sdk_id.to_owned())
}

pub(crate) fn handshake_response(accepted: bool) -> Vec<u8> {
    let value = if accepted {
        json!({
            "handshake": {
                "schema": HANDSHAKE_SCHEMA,
                "result": "accept",
                "instance": NEURO_INSTANCE,
            }
        })
    } else {
        json!({
            "handshake": {
                "schema": HANDSHAKE_SCHEMA,
                "result": "deny",
            }
        })
    };
    serde_json::to_vec(&value).expect("JSON values always serialize")
}

pub(crate) fn ack(sdk_id: &str, request_event_id: &str, ok: bool) -> Vec<u8> {
    let (event_id, timestamp) = metadata();
    serde_json::to_vec(&json!({
        "envelope_schema": ENVELOPE_SCHEMA,
        "envelope_kind": "response",
        "source": NEURO_INSTANCE,
        "target": "eow_sdk",
        "event_id": event_id,
        "correlation_id": request_event_id,
        "ts_utc": timestamp,
        "payload": {
            "sdk_id": sdk_id,
            "type": "ack",
            "data": {"ok": ok},
        }
    }))
    .expect("JSON values always serialize")
}

pub(crate) fn is_event_id(value: &str) -> bool {
    value.len() == 12 && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_sdk_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SDK_ID_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn metadata() -> (String, String) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds_in_day = now.as_secs() % 86_400;
    let hour = seconds_in_day / 3_600;
    let minute = seconds_in_day % 3_600 / 60;
    let second = seconds_in_day % 60;
    let millis = now.subsec_millis();
    let sequence = EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed) % 100;
    let event_id = format!("7{hour:02}{minute:02}{second:02}{millis:03}{sequence:02}");
    let days = (now.as_secs() / 86_400) as i64;
    let (year, month, day) = civil_date_from_unix_days(days);
    let timestamp =
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z");
    (event_id, timestamp)
}

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

fn ensure_kind(actual: EnvelopeKind, expected: EnvelopeKind) -> Result<(), ProtocolError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ProtocolError::Invalid("envelope_kind"))
    }
}

fn require_kind(
    actual: EnvelopeKind,
    expected: EnvelopeKind,
    message: BusinessMessage,
) -> Result<BusinessMessage, ProtocolError> {
    ensure_kind(actual, expected)?;
    Ok(message)
}

fn required_object<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
) -> Result<&'a Map<String, Value>, ProtocolError> {
    object
        .get(field)
        .and_then(Value::as_object)
        .ok_or(ProtocolError::Invalid(field))
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
) -> Result<&'a str, ProtocolError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or(ProtocolError::Invalid(field))
}

fn optional_string(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Option<String>, ProtocolError> {
    match object.get(field) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ProtocolError::Invalid(field)),
    }
}

fn required_bool(object: &Map<String, Value>, field: &'static str) -> Result<bool, ProtocolError> {
    object
        .get(field)
        .and_then(Value::as_bool)
        .ok_or(ProtocolError::Invalid(field))
}

fn optional_i64(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Option<i64>, ProtocolError> {
    match object.get(field) {
        None => Ok(None),
        Some(value) => value
            .as_i64()
            .map(Some)
            .ok_or(ProtocolError::Invalid(field)),
    }
}

fn parse_string_array(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Vec<String>, ProtocolError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or(ProtocolError::Invalid(field))?;
    let mut seen = HashSet::new();
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let value = value.as_str().ok_or(ProtocolError::Invalid(field))?;
        if value.is_empty() || !seen.insert(value.to_owned()) {
            return Err(ProtocolError::Invalid(field));
        }
        result.push(value.to_owned());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(kind: &str, correlation_id: Value, payload: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "envelope_schema": "1.0",
            "envelope_kind": kind,
            "source": "eow_sdk",
            "target": "neuro",
            "event_id": "712345678901",
            "correlation_id": correlation_id,
            "payload": payload,
        }))
        .unwrap()
    }

    #[test]
    fn correlation_validation_is_shape_only() {
        let response = envelope(
            "response",
            json!("700000000000"),
            json!({"sdk_id": "game", "type": "action", "kind": "result", "data": {"id": "a", "ok": true}}),
        );
        assert!(SdkEnvelope::parse(&response, "game").is_ok());
        let request = envelope(
            "request",
            json!("700000000000"),
            json!({"sdk_id": "game", "type": "startup"}),
        );
        assert!(matches!(
            SdkEnvelope::parse(&request, "game"),
            Err(ProtocolError::Invalid("correlation_id"))
        ));
    }

    #[test]
    fn update_is_parsed_as_a_complete_action_list() {
        let body = envelope(
            "request",
            Value::Null,
            json!({
                "sdk_id": "game",
                "type": "action",
                "kind": "update",
                "data": [{"action_id": "jump", "description": "Jump"}],
            }),
        );
        let envelope = SdkEnvelope::parse(&body, "game").unwrap();
        assert!(matches!(
            parse_business(&envelope).unwrap(),
            BusinessMessage::ActionUpdate(actions) if actions.len() == 1
        ));
    }

    #[test]
    fn state_snapshot_remains_neuro_sdk_incompatible() {
        let body = envelope(
            "event",
            Value::Null,
            json!({
                "sdk_id": "game",
                "type": "state",
                "kind": "snapshot",
                "revision": 0,
                "data": {"room": "start"},
            }),
        );
        let envelope = SdkEnvelope::parse(&body, "game").unwrap();
        assert!(matches!(
            parse_business(&envelope),
            Err(ProtocolError::Invalid("payload.type"))
        ));
    }

    #[test]
    fn generated_ack_uses_numeric_ids_and_request_correlation() {
        let ack: Value = serde_json::from_slice(&ack("game", "712345678901", true)).unwrap();
        assert!(is_event_id(ack["event_id"].as_str().unwrap()));
        assert_eq!(ack["correlation_id"], "712345678901");
        assert_eq!(ack["payload"]["data"]["ok"], true);
    }
}
