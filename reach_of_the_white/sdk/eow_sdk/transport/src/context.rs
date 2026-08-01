//! Process-wide SDK identity and shared transport resources.

use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use crate::endpoint::InstanceRegistry;
use crate::envelope::SdkEnvelope;
use crate::error::InitError;
use crate::event_id::{EventIdGenerator, SystemRandomNumber, SystemUtcClock};
use crate::runtime::TransportRuntime;

pub type InboundSink = Sender<SdkEnvelope>;

pub(crate) struct SdkContext {
    pub(crate) sdk_id: String,
    pub(crate) registry: InstanceRegistry,
    pub(crate) runtime: TransportRuntime,
    pub(crate) event_ids: Mutex<EventIdGenerator<SystemUtcClock, SystemRandomNumber>>,
}

static SDK_CONTEXT: OnceLock<SdkContext> = OnceLock::new();

pub fn initialize(sdk_id: impl Into<String>, inbound: InboundSink) -> Result<(), InitError> {
    initialize_with_registry(sdk_id, inbound, InstanceRegistry::bundled()?)
}

#[doc(hidden)]
pub fn initialize_with_registry(
    sdk_id: impl Into<String>,
    inbound: InboundSink,
    registry: InstanceRegistry,
) -> Result<(), InitError> {
    let sdk_id = sdk_id.into();
    if sdk_id.is_empty() {
        return Err(InitError::EmptySdkId);
    }

    let context = SdkContext {
        sdk_id,
        registry,
        runtime: TransportRuntime::new(inbound),
        event_ids: Mutex::new(EventIdGenerator::new(
            SystemUtcClock,
            SystemRandomNumber::default(),
        )),
    };

    SDK_CONTEXT
        .set(context)
        .map_err(|_| InitError::AlreadyInitialized)
}

pub(crate) fn context() -> Option<&'static SdkContext> {
    SDK_CONTEXT.get()
}

pub fn sdk_id() -> Option<&'static str> {
    context().map(|context| context.sdk_id.as_str())
}
