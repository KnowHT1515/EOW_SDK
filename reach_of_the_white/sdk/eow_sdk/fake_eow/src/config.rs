//! Fake EOW instance configuration.

use std::collections::HashSet;
use std::fmt;
use std::net::{Ipv4Addr, SocketAddrV4};

use serde_json::Value;

const BUNDLED_INSTANCE_REGISTRY: &str = include_str!("../instance.json");
const TEST_INSTANCE: &str = "knowht";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeEowConfig {
    pub instance: String,
    pub address: SocketAddrV4,
}

impl FakeEowConfig {
    pub fn new(instance: impl Into<String>, address: SocketAddrV4) -> Self {
        Self {
            instance: instance.into(),
            address,
        }
    }

    pub fn bundled() -> Result<Self, ConfigError> {
        Self::from_json(BUNDLED_INSTANCE_REGISTRY)
    }

    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        let value: Value = serde_json::from_str(json)?;
        let entries = value.as_array().ok_or(ConfigError::InvalidRoot)?;
        let mut seen = HashSet::with_capacity(entries.len());
        let mut selected = None;

        for (index, entry) in entries.iter().enumerate() {
            let object = entry
                .as_object()
                .ok_or(ConfigError::InvalidEntry { index })?;
            let instance = object
                .get("instance")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(ConfigError::InvalidEntry { index })?;

            if !seen.insert(instance.to_owned()) {
                return Err(ConfigError::DuplicateInstance(instance.to_owned()));
            }

            let port = match object.get("port") {
                None => None,
                Some(Value::String(port)) => Some(
                    port.parse::<u16>()
                        .ok()
                        .filter(|port| *port != 0)
                        .ok_or(ConfigError::InvalidPort { index })?,
                ),
                Some(_) => return Err(ConfigError::InvalidPort { index }),
            };

            if instance == TEST_INSTANCE {
                let port = port.ok_or(ConfigError::MissingPort { index })?;
                selected = Some(Self {
                    instance: instance.to_owned(),
                    address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, port),
                });
            }
        }

        selected.ok_or_else(|| ConfigError::MissingTestInstance(TEST_INSTANCE.to_owned()))
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Json(serde_json::Error),
    InvalidRoot,
    InvalidEntry { index: usize },
    InvalidPort { index: usize },
    MissingPort { index: usize },
    DuplicateInstance(String),
    MissingTestInstance(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid instance.json: {error}"),
            Self::InvalidRoot => formatter.write_str("instance.json root must be an array"),
            Self::InvalidEntry { index } => {
                write!(formatter, "invalid instance.json entry at index {index}")
            }
            Self::InvalidPort { index } => {
                write!(formatter, "invalid string port at index {index}")
            }
            Self::MissingPort { index } => {
                write!(formatter, "test instance has no port at index {index}")
            }
            Self::DuplicateInstance(instance) => {
                write!(
                    formatter,
                    "duplicate instance in instance.json: {instance:?}"
                )
            }
            Self::MissingTestInstance(instance) => {
                write!(
                    formatter,
                    "missing test instance in instance.json: {instance:?}"
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_config_resolves_the_fixed_test_endpoint() {
        let config = FakeEowConfig::bundled().unwrap();
        assert_eq!(config.instance, "knowht");
        assert_eq!(config.address, "127.0.0.1:7707".parse().unwrap());
    }

    #[test]
    fn config_requires_a_string_nonzero_port_for_the_test_instance() {
        assert!(matches!(
            FakeEowConfig::from_json(r#"[{"instance":"knowht","port":7707}]"#),
            Err(ConfigError::InvalidPort { .. })
        ));
        assert!(matches!(
            FakeEowConfig::from_json(r#"[{"instance":"knowht","port":"0"}]"#),
            Err(ConfigError::InvalidPort { .. })
        ));
        assert!(matches!(
            FakeEowConfig::from_json(r#"[{"instance":"knowht"}]"#),
            Err(ConfigError::MissingPort { .. })
        ));
    }

    #[test]
    fn config_rejects_duplicate_instances() {
        assert!(matches!(
            FakeEowConfig::from_json(
                r#"[
                    {"instance":"knowht","port":"7707"},
                    {"instance":"knowht","port":"7708"}
                ]"#
            ),
            Err(ConfigError::DuplicateInstance(instance)) if instance == "knowht"
        ));
    }
}
