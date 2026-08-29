//! SDK Envelope assembly over the process-wide SDK identity.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use crate::context::{SdkContext, context};
use crate::envelope::{EnvelopeKind, SdkEnvelope};
use crate::error::AssemblyError;
use crate::event_id::EventId;

pub type AssemblyResult = Result<String, AssemblyError>;

#[derive(Debug, Clone, PartialEq)]
pub struct SdkAction {
    pub action_id: String,
    pub description: String,
    pub arguments: Option<Map<String, Value>>,
    pub required: Vec<String>,
}

impl SdkAction {
    fn into_value(self) -> Value {
        let mut value = Map::new();
        value.insert("action_id".into(), Value::String(self.action_id));
        value.insert("description".into(), Value::String(self.description));
        if let Some(arguments) = self.arguments {
            value.insert("arguments".into(), Value::Object(arguments));
        }
        if !self.required.is_empty() {
            value.insert(
                "required".into(),
                Value::Array(self.required.into_iter().map(Value::String).collect()),
            );
        }
        Value::Object(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl ActionPriority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDemand {
    pub state: Option<String>,
    pub goal: String,
    pub permanent_context: bool,
    pub priority: ActionPriority,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionResult {
    pub id: Option<String>,
    pub ok: bool,
    pub result: Option<String>,
}

#[derive(Clone, Copy)]
pub struct Assembly {
    context: &'static SdkContext,
}

impl Assembly {
    pub fn global() -> Result<Self, AssemblyError> {
        context()
            .map(|context| Self { context })
            .ok_or(AssemblyError::NotInitialized)
    }

    pub fn startup(&self, target: &str) -> AssemblyResult {
        self.request(target, payload_base(&self.context.sdk_id, "startup"))
    }

    pub fn shutdown(&self, target: &str) -> AssemblyResult {
        self.request(target, payload_base(&self.context.sdk_id, "shutdown"))
    }

    pub fn context(&self, target: &str, content: &str, silent: bool) -> AssemblyResult {
        let mut payload = payload_base(&self.context.sdk_id, "context");
        payload.insert(
            "data".into(),
            json!({
                "content": content,
                "silent": silent,
            }),
        );
        self.request(target, payload)
    }

    pub fn state_snapshot(
        &self,
        target: &str,
        revision: u64,
        data: Map<String, Value>,
    ) -> AssemblyResult {
        let mut payload = payload_base(&self.context.sdk_id, "state");
        payload.insert("kind".into(), Value::String("snapshot".into()));
        payload.insert("revision".into(), Value::Number(revision.into()));
        payload.insert("data".into(), Value::Object(data));
        self.assemble(EnvelopeKind::Event, target, None, payload)
    }

    pub fn action_register(&self, target: &str, actions: Vec<SdkAction>) -> AssemblyResult {
        self.action_list(target, "register", actions)
    }

    pub fn action_unregister(&self, target: &str, action_ids: Vec<String>) -> AssemblyResult {
        let mut payload = action_base(&self.context.sdk_id, "unregister");
        payload.insert(
            "data".into(),
            Value::Array(action_ids.into_iter().map(Value::String).collect()),
        );
        self.request(target, payload)
    }

    pub fn action_update(&self, target: &str, actions: Vec<SdkAction>) -> AssemblyResult {
        self.action_list(target, "update", actions)
    }

    pub fn action_clear(&self, target: &str) -> AssemblyResult {
        self.request(target, action_base(&self.context.sdk_id, "clear"))
    }

    pub fn action_demand(&self, target: &str, demand: ActionDemand) -> AssemblyResult {
        let mut data = Map::new();
        if let Some(state) = demand.state {
            data.insert("state".into(), Value::String(state));
        }
        data.insert("goal".into(), Value::String(demand.goal));
        data.insert(
            "permanent_context".into(),
            Value::Bool(demand.permanent_context),
        );
        data.insert(
            "priority".into(),
            Value::String(demand.priority.as_str().into()),
        );
        data.insert(
            "actions".into(),
            Value::Array(demand.actions.into_iter().map(Value::String).collect()),
        );

        let mut payload = action_base(&self.context.sdk_id, "demand");
        payload.insert("data".into(), Value::Object(data));
        self.request(target, payload)
    }

    pub fn action_result(
        &self,
        target: &str,
        correlation_id: EventId,
        result: ActionResult,
    ) -> AssemblyResult {
        let mut data = Map::new();
        if let Some(id) = result.id {
            data.insert("id".into(), Value::String(id));
        }
        data.insert("ok".into(), Value::Bool(result.ok));
        if let Some(result) = result.result {
            data.insert("result".into(), Value::String(result));
        }

        let mut payload = action_base(&self.context.sdk_id, "result");
        payload.insert("data".into(), Value::Object(data));
        self.assemble(
            EnvelopeKind::Response,
            target,
            Some(correlation_id),
            payload,
        )
    }

    fn action_list(&self, target: &str, kind: &str, actions: Vec<SdkAction>) -> AssemblyResult {
        let mut payload = action_base(&self.context.sdk_id, kind);
        payload.insert(
            "data".into(),
            Value::Array(actions.into_iter().map(SdkAction::into_value).collect()),
        );
        self.request(target, payload)
    }

    fn request(&self, target: &str, payload: Map<String, Value>) -> AssemblyResult {
        self.assemble(EnvelopeKind::Request, target, None, payload)
    }

    fn assemble(
        &self,
        kind: EnvelopeKind,
        target: &str,
        correlation_id: Option<EventId>,
        payload: Map<String, Value>,
    ) -> AssemblyResult {
        if !self.context.registry.is_registered_instance(target) {
            return Err(AssemblyError::InvalidTarget(target.to_owned()));
        }
        let event_id = self
            .context
            .event_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .generate()?;
        let envelope = SdkEnvelope::from_fields_with_registry(
            kind,
            "eow_sdk",
            target,
            event_id,
            correlation_id,
            Value::String(now_iso8601_utc()),
            payload,
            &self.context.registry,
        )?;
        Ok(String::from_utf8(envelope.to_json_vec())
            .expect("serialized JSON is always valid UTF-8"))
    }
}

fn payload_base(sdk_id: &str, message_type: &str) -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert("sdk_id".into(), Value::String(sdk_id.to_owned()));
    payload.insert("type".into(), Value::String(message_type.to_owned()));
    payload
}

fn action_base(sdk_id: &str, kind: &str) -> Map<String, Value> {
    let mut payload = payload_base(sdk_id, "action");
    payload.insert("kind".into(), Value::String(kind.to_owned()));
    payload
}

fn now_iso8601_utc() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let days = (now.as_secs() / 86_400) as i64;
    let seconds_in_day = now.as_secs() % 86_400;
    let hour = seconds_in_day / 3_600;
    let minute = (seconds_in_day % 3_600) / 60;
    let second = seconds_in_day % 60;
    let (year, month, day) = civil_date_from_unix_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:03}Z",
        now.subsec_millis()
    )
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
