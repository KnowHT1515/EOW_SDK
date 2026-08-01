//! EOW instance-to-endpoint configuration.

use std::collections::HashMap;
use std::fs;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::Path;

use serde_json::Value;

use crate::error::ConfigError;
use crate::limits::DEFAULT_ADDRESS;

const BUNDLED_PORT_MAP: &str = include_str!("../port.json");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointConfig {
    pub instance: String,
    pub address: SocketAddrV4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortMap {
    ports: HashMap<String, u16>,
}

impl PortMap {
    pub fn bundled() -> Result<Self, ConfigError> {
        Self::from_json(BUNDLED_PORT_MAP)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        Self::from_json(&fs::read_to_string(path)?)
    }

    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        let value: Value = serde_json::from_str(json)?;
        let entries = value.as_array().ok_or(ConfigError::InvalidRoot)?;
        let mut ports = HashMap::with_capacity(entries.len());

        for (index, entry) in entries.iter().enumerate() {
            let object = entry
                .as_object()
                .ok_or(ConfigError::InvalidEntry { index })?;
            let instance = object
                .get("instance")
                .and_then(Value::as_str)
                .ok_or(ConfigError::InvalidEntry { index })?;
            if instance.is_empty() {
                return Err(ConfigError::EmptyInstance { index });
            }

            let port_text = object
                .get("port")
                .and_then(Value::as_str)
                .ok_or(ConfigError::InvalidEntry { index })?;
            let port = port_text
                .parse::<u16>()
                .map_err(|_| ConfigError::InvalidPort {
                    instance: instance.to_owned(),
                    port: port_text.to_owned(),
                })?;
            if port == 0 {
                return Err(ConfigError::InvalidPort {
                    instance: instance.to_owned(),
                    port: port_text.to_owned(),
                });
            }
            if ports.insert(instance.to_owned(), port).is_some() {
                return Err(ConfigError::DuplicateInstance(instance.to_owned()));
            }
        }

        Ok(Self { ports })
    }

    pub fn port(&self, instance: &str) -> Option<u16> {
        self.ports.get(instance).copied()
    }

    pub fn resolve(&self, instance: &str) -> Result<EndpointConfig, ConfigError> {
        self.resolve_at(instance, DEFAULT_ADDRESS)
    }

    pub fn resolve_at(
        &self,
        instance: &str,
        address: Ipv4Addr,
    ) -> Result<EndpointConfig, ConfigError> {
        let port = self
            .port(instance)
            .ok_or_else(|| ConfigError::UnknownInstance(instance.to_owned()))?;
        Ok(EndpointConfig {
            instance: instance.to_owned(),
            address: SocketAddrV4::new(address, port),
        })
    }

    /// An explicit socket endpoint takes precedence over the bundled instance map.
    pub fn resolve_with_override(
        &self,
        instance: &str,
        explicit: Option<SocketAddrV4>,
    ) -> Result<EndpointConfig, ConfigError> {
        match explicit {
            Some(address) => Ok(EndpointConfig {
                instance: instance.to_owned(),
                address,
            }),
            None => self.resolve(instance),
        }
    }

    pub fn len(&self) -> usize {
        self.ports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ports.is_empty()
    }
}
