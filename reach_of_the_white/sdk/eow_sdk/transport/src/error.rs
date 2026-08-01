//! Error types shared by the protocol foundations.

use std::fmt;
use std::io;

use crate::envelope::EnvelopeViolation;
use crate::frame::FrameAbort;

/// Failure to parse or validate an SDK Envelope.
#[derive(Debug)]
pub enum EnvelopeError {
    Json(serde_json::Error),
    Oversize { actual: usize, maximum: usize },
    Invalid(Vec<EnvelopeViolation>),
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid SDK Envelope JSON: {error}"),
            Self::Oversize { actual, maximum } => {
                write!(f, "SDK Envelope is {actual} bytes; maximum is {maximum}")
            }
            Self::Invalid(violations) => {
                write!(f, "SDK Envelope failed static validation: {violations:?}")
            }
        }
    }
}

impl std::error::Error for EnvelopeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Oversize { .. } | Self::Invalid(_) => None,
        }
    }
}

impl From<serde_json::Error> for EnvelopeError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Failure to encode or parse a handshake control body.
#[derive(Debug)]
pub enum HandshakeError {
    Json(serde_json::Error),
    InvalidShape,
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid handshake JSON: {error}"),
            Self::InvalidShape => write!(f, "invalid handshake message shape"),
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

/// Failure to load or resolve an instance endpoint.
#[derive(Debug)]
pub enum ConfigError {
    Json(serde_json::Error),
    Io(io::Error),
    InvalidRoot,
    InvalidEntry { index: usize },
    EmptyInstance { index: usize },
    InvalidPort { instance: String, port: String },
    DuplicateInstance(String),
    UnknownInstance(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid port map JSON: {error}"),
            Self::Io(error) => write!(f, "could not read port map: {error}"),
            Self::InvalidRoot => write!(f, "port map must be a JSON array"),
            Self::InvalidEntry { index } => write!(f, "invalid port map entry at index {index}"),
            Self::EmptyInstance { index } => {
                write!(f, "empty instance in port map entry at index {index}")
            }
            Self::InvalidPort { instance, port } => {
                write!(f, "invalid port {port:?} for instance {instance:?}")
            }
            Self::DuplicateInstance(instance) => {
                write!(f, "duplicate instance in port map: {instance:?}")
            }
            Self::UnknownInstance(instance) => write!(f, "unknown EOW instance: {instance:?}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<io::Error> for ConfigError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Failure while reading or writing framed bytes.
#[derive(Debug)]
pub enum FramedIoError {
    Io(io::Error),
    FrameAbort(FrameAbort),
    EmptyBody,
    BodyOversize { actual: usize, maximum: usize },
    TruncatedFrame,
}

impl fmt::Display for FramedIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "framed IO error: {error}"),
            Self::FrameAbort(reason) => write!(f, "framing aborted: {reason:?}"),
            Self::EmptyBody => write!(f, "zero-length frame bodies are forbidden"),
            Self::BodyOversize { actual, maximum } => {
                write!(f, "frame body is {actual} bytes; maximum is {maximum}")
            }
            Self::TruncatedFrame => write!(f, "connection ended with a partial frame"),
        }
    }
}

impl std::error::Error for FramedIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for FramedIoError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
