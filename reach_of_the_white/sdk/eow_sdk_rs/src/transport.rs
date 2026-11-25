use std::{error::Error, fmt, io, sync::Arc};

pub mod envelope;
pub mod envelope_data;
pub mod sdk_transport;
pub mod tcp_connection;
pub mod tcp_framing;

#[derive(Clone, Debug)]
pub enum SdkError {
    Io(Arc<io::Error>),
    Json(Arc<serde_json::Error>),
    Protocol(&'static str),
    InvalidData(String),
    HandshakeDenied(Option<String>),
    AlreadyConnected,
    Disconnected,
    DemandPending,
    EventIdExhausted,
    Timeout,
}

impl fmt::Display for SdkError {

    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "EOW transport I/O error: {error}"),
            Self::Json(error) => write!(formatter, "EOW JSON error: {error}"),
            Self::Protocol(message) => write!(formatter, "EOW protocol error: {message}"),
            Self::InvalidData(message) => formatter.write_str(message),
            Self::HandshakeDenied(Some(reason)) => write!(formatter, "EOW denied the handshake: {reason}"),
            Self::HandshakeDenied(None) => formatter.write_str("EOW denied the handshake"),
            Self::AlreadyConnected => formatter.write_str("The EOW SDK is already connected"),
            Self::Disconnected => formatter.write_str("The EOW connection is closed"),
            Self::DemandPending => formatter.write_str("An action demand is already pending"),
            Self::EventIdExhausted => formatter.write_str("No unused EOW event ID is available"),
            Self::Timeout => formatter.write_str("The EOW SDK operation timed out"),
        }
    }

}

impl Error for SdkError {

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error.as_ref()),
            Self::Json(error) => Some(error.as_ref()),
            _ => None,
        }
    }

}

impl From<io::Error> for SdkError {

    fn from(error: io::Error) -> Self {
        Self::Io(Arc::new(error))
    }

}

impl From<serde_json::Error> for SdkError {

    fn from(error: serde_json::Error) -> Self {
        Self::Json(Arc::new(error))
    }

}
