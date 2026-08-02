//! Handshake control-message wire shapes.

use serde_json::{Value, json};

use crate::context::is_valid_sdk_id;
use crate::endpoint::{RESERVED_SDK_ENDPOINT, is_valid_instance_name};
use crate::error::HandshakeError;
use crate::limits::HANDSHAKE_SCHEMA;

/// Client-to-EOW handshake declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeRequest {
    pub schema: String,
    pub id: String,
}

impl HandshakeRequest {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            schema: HANDSHAKE_SCHEMA.into(),
            id: id.into(),
        }
    }

    pub fn with_schema(schema: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            schema: schema.into(),
            id: id.into(),
        }
    }

    pub fn to_body(&self) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "handshake": {
                "schema": self.schema,
                "id": self.id,
            }
        }))
        .expect("handshake request always serializes")
    }

    pub fn parse(body: &[u8]) -> Result<Self, HandshakeError> {
        let top: Value = serde_json::from_slice(body)?;
        let handshake = top
            .as_object()
            .and_then(|object| object.get("handshake"))
            .and_then(Value::as_object)
            .ok_or(HandshakeError::InvalidShape)?;
        let schema = handshake
            .get("schema")
            .and_then(Value::as_str)
            .filter(|schema| !schema.is_empty())
            .ok_or(HandshakeError::InvalidShape)?;
        let id = handshake
            .get("id")
            .and_then(Value::as_str)
            .ok_or(HandshakeError::InvalidShape)?;
        if !is_valid_sdk_id(id) {
            return Err(HandshakeError::InvalidSdkId);
        }
        Ok(Self::with_schema(schema, id))
    }
}

/// EOW-to-client handshake decision. Schema compatibility and expected-instance
/// checks belong to the future connection lifecycle rather than this wire parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeResponse {
    Accept { schema: String, instance: String },
    Deny { schema: String },
}

impl HandshakeResponse {
    pub fn accept(instance: impl Into<String>) -> Self {
        Self::Accept {
            schema: HANDSHAKE_SCHEMA.into(),
            instance: instance.into(),
        }
    }

    pub fn deny() -> Self {
        Self::Deny {
            schema: HANDSHAKE_SCHEMA.into(),
        }
    }

    pub fn parse(body: &[u8]) -> Result<Self, HandshakeError> {
        let top: Value = serde_json::from_slice(body)?;
        let handshake = top
            .as_object()
            .and_then(|object| object.get("handshake"))
            .and_then(Value::as_object)
            .ok_or(HandshakeError::InvalidShape)?;
        let schema = handshake
            .get("schema")
            .and_then(Value::as_str)
            .filter(|schema| !schema.is_empty())
            .ok_or(HandshakeError::InvalidShape)?
            .to_owned();
        match handshake.get("result").and_then(Value::as_str) {
            Some("accept") => {
                let instance = handshake
                    .get("instance")
                    .and_then(Value::as_str)
                    .ok_or(HandshakeError::InvalidShape)?;
                if !is_valid_instance_name(instance) || instance == RESERVED_SDK_ENDPOINT {
                    return Err(HandshakeError::InvalidInstance);
                }
                Ok(Self::Accept {
                    schema,
                    instance: instance.to_owned(),
                })
            }
            Some("deny") if !handshake.contains_key("instance") => Ok(Self::Deny { schema }),
            _ => Err(HandshakeError::InvalidShape),
        }
    }

    pub fn to_body(&self) -> Vec<u8> {
        let value = match self {
            Self::Accept { schema, instance } => json!({
                "handshake": {
                    "schema": schema,
                    "result": "accept",
                    "instance": instance,
                }
            }),
            Self::Deny { schema } => json!({
                "handshake": {
                    "schema": schema,
                    "result": "deny",
                }
            }),
        };
        serde_json::to_vec(&value).expect("handshake response always serializes")
    }

    pub fn accepted_instance(&self) -> Option<&str> {
        match self {
            Self::Accept { instance, .. } => Some(instance),
            Self::Deny { .. } => None,
        }
    }
}
