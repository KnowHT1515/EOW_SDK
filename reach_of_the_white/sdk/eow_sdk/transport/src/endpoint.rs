//! Registered SDK Envelope endpoints and optional TCP connection ports.

use std::collections::HashMap;
use std::fs;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::Path;

use serde_json::Value;

use crate::error::ConfigError;
use crate::limits::DEFAULT_ADDRESS;

const BUNDLED_INSTANCE_REGISTRY: &str = include_str!("../instance.json");
const RESERVED_SDK_ENDPOINT: &str = "eow_sdk";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointConfig {
    pub instance: String,
    pub address: SocketAddrV4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceRegistry {
    instances: HashMap<String, Option<u16>>,
}

impl InstanceRegistry {
    pub fn bundled() -> Result<Self, ConfigError> {
        Self::from_json(BUNDLED_INSTANCE_REGISTRY)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        Self::from_json(&fs::read_to_string(path)?)
    }

    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        let value: Value = serde_json::from_str(json)?;
        let entries = value.as_array().ok_or(ConfigError::InvalidRoot)?;
        let mut instances = HashMap::with_capacity(entries.len());

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
            if instance == RESERVED_SDK_ENDPOINT {
                return Err(ConfigError::ReservedInstance(instance.to_owned()));
            }

            let port =
                match object.get("port") {
                    None => None,
                    Some(Value::String(port_text)) => Some(port_text.parse::<u16>().map_err(
                        |_| ConfigError::InvalidPort {
                            instance: instance.to_owned(),
                            port: port_text.to_owned(),
                        },
                    )?),
                    Some(other) => {
                        return Err(ConfigError::InvalidPort {
                            instance: instance.to_owned(),
                            port: other.to_string(),
                        });
                    }
                };
            if port == Some(0) {
                return Err(ConfigError::InvalidPort {
                    instance: instance.to_owned(),
                    port: "0".to_owned(),
                });
            }
            if instances.insert(instance.to_owned(), port).is_some() {
                return Err(ConfigError::DuplicateInstance(instance.to_owned()));
            }
        }

        Ok(Self { instances })
    }

    pub fn contains(&self, instance: &str) -> bool {
        instance == RESERVED_SDK_ENDPOINT || self.instances.contains_key(instance)
    }

    pub fn port(&self, instance: &str) -> Option<u16> {
        self.instances.get(instance).copied().flatten()
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
            .instances
            .get(instance)
            .ok_or_else(|| ConfigError::UnknownInstance(instance.to_owned()))?
            .ok_or_else(|| ConfigError::MissingPort(instance.to_owned()))?;
        Ok(EndpointConfig {
            instance: instance.to_owned(),
            address: SocketAddrV4::new(address, port),
        })
    }

    /// An explicit socket endpoint takes precedence over the registered port, but
    /// the instance itself must still be registered.
    pub fn resolve_with_override(
        &self,
        instance: &str,
        explicit: Option<SocketAddrV4>,
    ) -> Result<EndpointConfig, ConfigError> {
        match explicit {
            Some(address) if self.instances.contains_key(instance) => Ok(EndpointConfig {
                instance: instance.to_owned(),
                address,
            }),
            Some(_) => Err(ConfigError::UnknownInstance(instance.to_owned())),
            None => self.resolve(instance),
        }
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
}
