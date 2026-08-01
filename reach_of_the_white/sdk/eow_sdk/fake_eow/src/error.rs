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
    LocalAddress(io::Error),
    Accept(io::Error),
    Spawn(io::Error),
    WorkerPanicked,
}

impl fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => write!(formatter, "could not load fake EOW config: {error}"),
            Self::Bind { address, source } => {
                write!(formatter, "could not bind fake EOW to {address}: {source}")
            }
            Self::LocalAddress(error) => {
                write!(formatter, "could not inspect fake EOW address: {error}")
            }
            Self::Accept(error) => write!(formatter, "could not accept SDK connection: {error}"),
            Self::Spawn(error) => write!(formatter, "could not spawn fake EOW worker: {error}"),
            Self::WorkerPanicked => formatter.write_str("fake EOW worker panicked"),
        }
    }
}

impl std::error::Error for StartupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(error) => Some(error),
            Self::Bind { source, .. } => Some(source),
            Self::LocalAddress(error) | Self::Accept(error) | Self::Spawn(error) => Some(error),
            Self::WorkerPanicked => None,
        }
    }
}

impl From<ConfigError> for StartupError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}
