use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 1219;
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
pub const NEURO_INSTANCE: &str = "neuro";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerConfig {
    pub address: SocketAddrV4,
    pub handshake_timeout: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEFAULT_PORT),
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
        }
    }
}
