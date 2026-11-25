use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCHEMA: &str = "1.0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeKind {
    Request,
    Response,
    Event,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvelopeType {
    Startup,
    Shutdown,
    Context,
    StateSnapshot,
    ActionRegister,
    ActionUnregister,
    ActionUpdate,
    ActionClear,
    ActionDemand,
    ActionResult,
}

impl EnvelopeType {

    pub fn fields(self) -> (EnvelopeKind, &'static str, Option<&'static str>) {
        match self {
            Self::Startup => (EnvelopeKind::Request, "startup", None),
            Self::Shutdown => (EnvelopeKind::Request, "shutdown", None),
            Self::Context => (EnvelopeKind::Request, "context", None),
            Self::StateSnapshot => (EnvelopeKind::Event, "state", Some("snapshot")),
            Self::ActionRegister => (EnvelopeKind::Request, "action", Some("register")),
            Self::ActionUnregister => (EnvelopeKind::Request, "action", Some("unregister")),
            Self::ActionUpdate => (EnvelopeKind::Request, "action", Some("update")),
            Self::ActionClear => (EnvelopeKind::Request, "action", Some("clear")),
            Self::ActionDemand => (EnvelopeKind::Request, "action", Some("demand")),
            Self::ActionResult => (EnvelopeKind::Response, "action", Some("result")),
        }
    }

}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Envelope {
    pub envelope_schema: String,
    pub envelope_kind: EnvelopeKind,
    pub source: String,
    pub target: String,
    pub event_id: String,
    pub correlation_id: Option<String>,
    pub ts_utc: String,
    pub payload: EnvelopePayload,
}

impl Envelope {

    pub fn ack_ok(&self) -> Option<bool> {
        if self.envelope_kind != EnvelopeKind::Response || self.payload.payload_type != "ack" {
            return None;
        }
        self.payload.data.as_ref()?.get("ok")?.as_bool()
    }

}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EnvelopePayload {
    pub sdk_id: String,
    #[serde(rename = "type")]
    pub payload_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
