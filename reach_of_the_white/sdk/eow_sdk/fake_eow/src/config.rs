use std::collections::HashSet;
use std::fmt;
use std::net::{Ipv4Addr, SocketAddrV4};

use serde_json::Value;

const BUNDLED_PORT_MAP: &str = include_str!("../port.json");
const TEST_INSTANCE: &str = "knowht";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeEowConfig {
    pub instance: String,
    pub address: SocketAddrV4,
}

impl FakeEowConfig {
    pub fn bundled() -> Result<Self, ConfigError> {
        Self::from_json(BUNDLED_PORT_MAP)
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
            let port = object
                .get("port")
                .and_then(Value::as_u64)
                .filter(|port| (1..=u16::MAX as u64).contains(port))
                .ok_or(ConfigError::InvalidPort { index })? as u16;

            if !seen.insert(instance.to_owned()) {
                return Err(ConfigError::DuplicateInstance(instance.to_owned()));
            }
            if instance == TEST_INSTANCE {
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
    DuplicateInstance(String),
    MissingTestInstance(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid port.json: {error}"),
            Self::InvalidRoot => formatter.write_str("port.json root must be an array"),
            Self::InvalidEntry { index } => {
                write!(formatter, "invalid port.json entry at index {index}")
            }
            Self::InvalidPort { index } => {
                write!(formatter, "invalid numeric port at index {index}")
            }
            Self::DuplicateInstance(instance) => {
                write!(formatter, "duplicate instance in port.json: {instance:?}")
            }
            Self::MissingTestInstance(instance) => {
                write!(
                    formatter,
                    "missing test instance in port.json: {instance:?}"
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
    fn config_requires_numeric_nonzero_port_and_known_instance() {
        assert!(matches!(
            FakeEowConfig::from_json(r#"[{"instance":"knowht","port":"7707"}]"#),
            Err(ConfigError::InvalidPort { .. })
        ));
        assert!(matches!(
            FakeEowConfig::from_json(r#"[{"instance":"knowht","port":0}]"#),
            Err(ConfigError::InvalidPort { .. })
        ));
        assert!(matches!(
            FakeEowConfig::from_json(r#"[{"instance":"other","port":7707}]"#),
            Err(ConfigError::MissingTestInstance(_))
        ));
    }

    #[test]
    fn config_rejects_duplicate_instances() {
        assert!(matches!(
            FakeEowConfig::from_json(
                r#"[
                    {"instance":"knowht","port":7707},
                    {"instance":"knowht","port":7708}
                ]"#
            ),
            Err(ConfigError::DuplicateInstance(instance)) if instance == "knowht"
        ));
    }
}
