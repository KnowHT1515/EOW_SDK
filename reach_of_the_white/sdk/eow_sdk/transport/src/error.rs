//! Error types shared by the protocol foundations and runtime.

use std::fmt;
use std::io;

use crate::envelope::EnvelopeViolation;
use crate::event_id::RandomNumberOutOfRange;
use crate::frame::FrameAbort;

#[derive(Debug)]
pub enum EnvelopeError {
    Json(serde_json::Error),
    Oversize { actual: usize, maximum: usize },
    Invalid(Vec<EnvelopeViolation>),
    Config(ConfigError),
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
            Self::Config(error) => write!(f, "could not load instance registry: {error}"),
        }
    }
}

impl std::error::Error for EnvelopeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Config(error) => Some(error),
            Self::Oversize { .. } | Self::Invalid(_) => None,
        }
    }
}

impl From<serde_json::Error> for EnvelopeError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<ConfigError> for EnvelopeError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

#[derive(Debug)]
pub enum HandshakeError {
    Json(serde_json::Error),
    InvalidShape,
    InvalidSdkId,
    InvalidInstance,
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid handshake JSON: {error}"),
            Self::InvalidShape => write!(f, "invalid handshake message shape"),
            Self::InvalidSdkId => write!(f, "invalid sdk_id in handshake"),
            Self::InvalidInstance => write!(f, "invalid instance in handshake"),
        }
    }
}

impl std::error::Error for HandshakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::InvalidShape | Self::InvalidSdkId | Self::InvalidInstance => None,
        }
    }
}

impl From<serde_json::Error> for HandshakeError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Json(serde_json::Error),
    Io(io::Error),
    InvalidRoot,
    InvalidEntry {
        index: usize,
    },
    InvalidInstance {
        index: usize,
        instance: String,
    },
    MissingRequiredPort {
        index: usize,
        instance: String,
    },
    InvalidPort {
        instance: String,
        port: String,
    },
    DuplicateInstance(String),
    DuplicatePort {
        port: u16,
        first_instance: String,
        second_instance: String,
    },
    ReservedInstance(String),
    UnknownInstance(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid instance registry JSON: {error}"),
            Self::Io(error) => write!(f, "could not read instance registry: {error}"),
            Self::InvalidRoot => write!(f, "instance registry must be a JSON array"),
            Self::InvalidEntry { index } => {
                write!(f, "invalid instance registry entry at index {index}")
            }
            Self::InvalidInstance { index, instance } => {
                write!(
                    f,
                    "invalid instance {instance:?} in registry entry at index {index}"
                )
            }
            Self::MissingRequiredPort { index, instance } => {
                write!(
                    f,
                    "instance {instance:?} has no port in registry entry at index {index}"
                )
            }
            Self::InvalidPort { instance, port } => {
                write!(f, "invalid port {port:?} for instance {instance:?}")
            }
            Self::DuplicateInstance(instance) => {
                write!(f, "duplicate instance in registry: {instance:?}")
            }
            Self::DuplicatePort {
                port,
                first_instance,
                second_instance,
            } => write!(
                f,
                "duplicate port {port} in registry for instances {first_instance:?} and {second_instance:?}"
            ),
            Self::ReservedInstance(instance) => {
                write!(
                    f,
                    "reserved SDK endpoint cannot be registered: {instance:?}"
                )
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

#[derive(Debug)]
pub enum InitError {
    InvalidSdkId,
    AlreadyInitialized,
    Config(ConfigError),
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSdkId => f.write_str(
                "sdk_id must be 1..=128 UTF-8 bytes without surrounding whitespace or control characters",
            ),
            Self::AlreadyInitialized => f.write_str("EOW SDK transport is already initialized"),
            Self::Config(error) => write!(f, "could not initialize instance registry: {error}"),
        }
    }
}

impl std::error::Error for InitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ConfigError> for InitError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

#[derive(Debug)]
pub enum ConnectError {
    NotInitialized,
    Config(ConfigError),
    AlreadyConnected(String),
    Io(io::Error),
    Framed(FramedIoError),
    Handshake(HandshakeError),
    HandshakeClosed,
    HandshakeDenied,
    HandshakeTimeout,
    UnsupportedSchema(String),
    InstanceMismatch { expected: String, actual: String },
    Spawn(io::Error),
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => f.write_str("EOW SDK transport is not initialized"),
            Self::Config(error) => write!(f, "could not resolve EOW instance: {error}"),
            Self::AlreadyConnected(instance) => {
                write!(f, "EOW instance is already connected: {instance:?}")
            }
            Self::Io(error) => write!(f, "TCP connection setup failed: {error}"),
            Self::Framed(error) => write!(f, "handshake frame I/O failed: {error}"),
            Self::Handshake(error) => write!(f, "handshake response failed: {error}"),
            Self::HandshakeClosed => f.write_str("EOW closed before replying to the handshake"),
            Self::HandshakeDenied => f.write_str("EOW denied the SDK handshake"),
            Self::HandshakeTimeout => {
                f.write_str("TCP connection and handshake exceeded the absolute deadline")
            }
            Self::UnsupportedSchema(schema) => {
                write!(f, "unsupported handshake schema: {schema:?}")
            }
            Self::InstanceMismatch { expected, actual } => write!(
                f,
                "handshake instance mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::Spawn(error) => write!(f, "could not spawn connection worker: {error}"),
        }
    }
}

impl std::error::Error for ConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(error) => Some(error),
            Self::Io(error) | Self::Spawn(error) => Some(error),
            Self::Framed(error) => Some(error),
            Self::Handshake(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ConfigError> for ConnectError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<FramedIoError> for ConnectError {
    fn from(error: FramedIoError) -> Self {
        Self::Framed(error)
    }
}

impl From<HandshakeError> for ConnectError {
    fn from(error: HandshakeError) -> Self {
        Self::Handshake(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectError {
    NotInitialized,
    NotConnected(String),
    WorkerPanicked(String),
}

impl fmt::Display for DisconnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => f.write_str("EOW SDK transport is not initialized"),
            Self::NotConnected(instance) => {
                write!(f, "EOW instance is not connected: {instance:?}")
            }
            Self::WorkerPanicked(instance) => {
                write!(f, "connection worker panicked for instance {instance:?}")
            }
        }
    }
}

impl std::error::Error for DisconnectError {}

#[derive(Debug)]
pub enum AssemblyError {
    NotInitialized,
    InvalidTarget(String),
    EventId(RandomNumberOutOfRange),
    Envelope(EnvelopeError),
}

impl fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => f.write_str("EOW SDK transport is not initialized"),
            Self::InvalidTarget(target) => {
                write!(
                    f,
                    "Assembly target is not a registered instance: {target:?}"
                )
            }
            Self::EventId(error) => write!(f, "could not generate event_id: {error}"),
            Self::Envelope(error) => write!(f, "could not assemble SDK Envelope: {error}"),
        }
    }
}

impl std::error::Error for AssemblyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EventId(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RandomNumberOutOfRange> for AssemblyError {
    fn from(error: RandomNumberOutOfRange) -> Self {
        Self::EventId(error)
    }
}
impl From<EnvelopeError> for AssemblyError {
    fn from(error: EnvelopeError) -> Self {
        Self::Envelope(error)
    }
}
