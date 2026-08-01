use std::fmt;
use std::io;
use std::net::SocketAddrV4;

use crate::config::ConfigError;

#[derive(Debug)]
pub enum StartupError {
    Config(ConfigError),
    Bind {
        address: SocketAddrV4,
        source: io::Error,
    },
    Accept(io::Error),
    Spawn(io::Error),
}

impl fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => write!(formatter, "could not load fake EOW config: {error}"),
            Self::Bind { address, source } => {
                write!(formatter, "could not bind fake EOW to {address}: {source}")
            }
            Self::Accept(error) => write!(formatter, "could not accept SDK connection: {error}"),
            Self::Spawn(error) => write!(formatter, "could not spawn SDK session: {error}"),
        }
    }
}

impl std::error::Error for StartupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(error) => Some(error),
            Self::Bind { source, .. } => Some(source),
            Self::Accept(error) => Some(error),
            Self::Spawn(error) => Some(error),
        }
    }
}

impl From<ConfigError> for StartupError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}
