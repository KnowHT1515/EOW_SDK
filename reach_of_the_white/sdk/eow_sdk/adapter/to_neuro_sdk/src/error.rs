use std::fmt;
use std::io;

#[derive(Debug)]
pub enum ServerError {
    Io(io::Error),
    Spawn(io::Error),
    WorkerPanicked,
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "E2NA I/O failed: {error}"),
            Self::Spawn(error) => write!(formatter, "failed to spawn an E2NA worker: {error}"),
            Self::WorkerPanicked => formatter.write_str("an E2NA worker panicked"),
        }
    }
}

impl std::error::Error for ServerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) | Self::Spawn(error) => Some(error),
            Self::WorkerPanicked => None,
        }
    }
}

impl From<io::Error> for ServerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug)]
pub(crate) enum ProtocolError {
    Io(io::Error),
    Frame(&'static str),
    Json(serde_json::Error),
    Invalid(&'static str),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O failed: {error}"),
            Self::Frame(reason) => write!(formatter, "invalid frame: {reason}"),
            Self::Json(error) => write!(formatter, "invalid JSON: {error}"),
            Self::Invalid(field) => write!(formatter, "invalid protocol field: {field}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<io::Error> for ProtocolError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ProtocolError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
