//! Protocol limits and fixed wire constants.

use std::net::Ipv4Addr;

/// The only SDK Envelope schema supported by this revision.
pub const SDK_ENVELOPE_SCHEMA: &str = "1.0";

/// The handshake schema emitted by this revision.
pub const HANDSHAKE_SCHEMA: &str = "1.0";

/// Maximum serialized UTF-8 size of one SDK Envelope.
pub const MAX_ENVELOPE_BYTES: usize = 15 * 1024 * 1024;

/// Maximum body size representable by one TCP frame.
pub const MAX_FRAME_BODY: usize = 20 * 1024 * 1024;

/// Fixed frame-integrity sentinel.
pub const SENTINEL: [u8; 5] = *b"\\eow/";

/// Default address used for an EOW instance endpoint.
pub const DEFAULT_ADDRESS: Ipv4Addr = Ipv4Addr::LOCALHOST;
