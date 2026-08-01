use std::fmt;

use serde_json::{Value, json};

pub const HANDSHAKE_SCHEMA: &str = "1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeRequest {
    pub schema: String,
    pub id: String,
}

impl HandshakeRequest {
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
            .ok_or(HandshakeError::InvalidShape)?;
        let id = handshake
            .get("id")
            .and_then(Value::as_str)
            .ok_or(HandshakeError::InvalidShape)?;

        Ok(Self {
            schema: schema.to_owned(),
            id: id.to_owned(),
        })
    }

    pub fn is_acceptable(&self) -> bool {
        self.schema == HANDSHAKE_SCHEMA && !self.id.is_empty()
    }
}

pub fn accept_body(instance: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "handshake": {
            "schema": HANDSHAKE_SCHEMA,
            "result": "accept",
            "instance": instance,
        }
    }))
    .expect("static handshake response always serializes")
}

pub fn deny_body() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "handshake": {
            "schema": HANDSHAKE_SCHEMA,
            "result": "deny",
        }
    }))
    .expect("static handshake response always serializes")
}

#[derive(Debug)]
pub enum HandshakeError {
    Json(serde_json::Error),
    InvalidShape,
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid handshake JSON: {error}"),
            Self::InvalidShape => formatter.write_str("invalid handshake message shape"),
        }
    }
}

impl std::error::Error for HandshakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::InvalidShape => None,
        }
    }
}

impl From<serde_json::Error> for HandshakeError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_schema_and_nonempty_id() {
        let request =
            HandshakeRequest::parse(br#"{"handshake":{"schema":"1.0","id":"example-sdk"}}"#)
                .unwrap();
        assert!(request.is_acceptable());
    }

    #[test]
    fn rejects_unknown_schema_or_empty_id() {
        let unknown =
            HandshakeRequest::parse(br#"{"handshake":{"schema":"2.0","id":"sdk"}}"#).unwrap();
        let empty = HandshakeRequest::parse(br#"{"handshake":{"schema":"1.0","id":""}}"#).unwrap();
        assert!(!unknown.is_acceptable());
        assert!(!empty.is_acceptable());
    }

    #[test]
    fn response_bodies_have_the_documented_shape() {
        let accept: Value = serde_json::from_slice(&accept_body("knowht")).unwrap();
        assert_eq!(accept["handshake"]["result"], "accept");
        assert_eq!(accept["handshake"]["instance"], "knowht");

        let deny: Value = serde_json::from_slice(&deny_body()).unwrap();
        assert_eq!(deny["handshake"]["result"], "deny");
        assert!(deny["handshake"].get("instance").is_none());
    }
}
