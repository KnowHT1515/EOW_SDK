//! Protocol foundations for the EOW SDK transport.

pub mod assembly;
pub mod context;
pub mod endpoint;
pub mod envelope;
pub mod error;
pub mod event_id;
pub mod frame;
pub mod handshake;
pub mod io;
pub mod limits;
pub mod runtime;

pub use assembly::{
    ActionDemand, ActionPriority, ActionResult, Assembly, AssemblyResult, SdkAction,
};
pub use context::{InboundSink, initialize, initialize_with_registry, sdk_id};
pub use endpoint::{EndpointConfig, InstanceRegistry};
pub use envelope::{EnvelopeKind, EnvelopeViolation, SdkEnvelope};
pub use error::{
    AssemblyError, ConfigError, ConnectError, DisconnectError, EnvelopeError, FramedIoError,
    HandshakeError, InitError,
};
pub use event_id::{EventId, EventIdClock, EventIdGenerator, RandomNumber, SystemUtcClock};
pub use event_id::{InvalidEventId, RandomNumberOutOfRange, SystemRandomNumber, UtcTime};
pub use frame::{DecodeResult, Deframer, FrameAbort};
pub use handshake::{HandshakeRequest, HandshakeResponse};
pub use io::{FramedReader, FramedWriter};
pub use runtime::{SendResult, connect, connect_at, disconnect, send};
