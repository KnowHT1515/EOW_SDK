//! Protocol foundations for the EOW SDK transport.

pub mod endpoint;
pub mod envelope;
pub mod error;
pub mod event_id;
pub mod frame;
pub mod handshake;
pub mod io;
pub mod limits;

pub use endpoint::{EndpointConfig, PortMap};
pub use envelope::{EnvelopeKind, EnvelopeViolation, SdkEnvelope};
pub use error::{ConfigError, EnvelopeError, FramedIoError, HandshakeError};
pub use event_id::{EventId, EventIdClock, EventIdGenerator, RandomNumber, SystemUtcClock};
pub use event_id::{InvalidEventId, RandomNumberOutOfRange, UtcTime};
pub use frame::{DecodeResult, Deframer, FrameAbort};
pub use handshake::{HandshakeRequest, HandshakeResponse};
pub use io::{FramedReader, FramedWriter};
