use std::collections::BTreeMap;

use serde::Serialize;

use crate::transport::SdkError;

#[derive(Clone, Debug, Serialize)]
pub struct ContextData {
    pub content: String,
    pub silent: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct RegisterData(pub Vec<SdkAction>);

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct UnregisterData(pub Vec<String>);

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct UpdateData(pub Vec<SdkAction>);

#[derive(Clone, Debug, Serialize)]
pub struct DemandData {
    pub id: String,
    pub goal: String,
    pub actions: Vec<String>,
    pub permanent_context: bool,
    pub respond_asap: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResultData {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SdkAction {
    action_id: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<BTreeMap<String, Argument>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<Vec<String>>,
}

impl SdkAction {

    pub fn new(action_id: impl Into<String>, description: impl Into<String>, arguments: Option<BTreeMap<String, Argument>>, required: Option<Vec<String>>) -> Result<Self, SdkError> {
        if let Some(names) = &required {
            for name in names {
                if !arguments.as_ref().is_some_and(|arguments: &BTreeMap<String, Argument>| arguments.contains_key(name)) {
                    return Err(SdkError::InvalidData(format!("Required argument '{name}' is not declared")));
                }
            }
        }
        Ok(Self { action_id: action_id.into(), description: description.into(), arguments, required })
    }

    pub fn action_id(&self) -> &str {
        &self.action_id
    }

}

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct Argument(ArgumentType);

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ArgumentType {
    String {
        #[serde(rename = "enum")]
        values: Vec<String>,
        description: String,
    },
    Integer {
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum: Option<i32>,
        description: String,
    },
    Boolean {
        description: String,
    },
}

impl Argument {

    pub fn string(description: impl Into<String>, r#enum: Vec<String>) -> Self {
        Self(ArgumentType::String { values: r#enum, description: description.into() })
    }

    pub fn integer(description: impl Into<String>, minimum: Option<i32>, maximum: Option<i32>) -> Result<Self, SdkError> {
        if let (Some(minimum), Some(maximum)) = (minimum, maximum)
            && minimum > maximum {
            return Err(SdkError::InvalidData(format!("Maximum ({maximum}) must be greater than or equal to minimum ({minimum})")));
        }
        Ok(Self(ArgumentType::Integer { minimum, maximum, description: description.into() }))
    }

    pub fn boolean(description: impl Into<String>) -> Self {
        Self(ArgumentType::Boolean { description: description.into() })
    }

}
