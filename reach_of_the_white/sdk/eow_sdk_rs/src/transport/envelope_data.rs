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
}

impl SdkAction {

    pub fn new(action_id: impl Into<String>, description: impl Into<String>, arguments: Option<BTreeMap<String, Argument>>) -> Self {
        let arguments: Option<BTreeMap<String, Argument>> = arguments.filter(|arguments: &BTreeMap<String, Argument>| !arguments.is_empty());
        Self { action_id: action_id.into(), description: description.into(), arguments }
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
        required: bool,
    },
    Integer {
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum: Option<i32>,
        description: String,
        required: bool,
    },
    Boolean {
        description: String,
        required: bool,
    },
}

impl Argument {

    pub fn string(description: impl Into<String>, required: bool, r#enum: Vec<String>) -> Result<Self, SdkError> {
        if r#enum.is_empty() {
            return Err(SdkError::InvalidData("Enum must contain at least one value".into()));
        }
        Ok(Self(ArgumentType::String { values: r#enum, description: description.into(), required }))
    }

    pub fn integer(description: impl Into<String>, required: bool, minimum: Option<i32>, maximum: Option<i32>) -> Result<Self, SdkError> {
        if let (Some(minimum), Some(maximum)) = (minimum, maximum)
            && minimum > maximum {
            return Err(SdkError::InvalidData(format!("Maximum ({maximum}) must be greater than or equal to minimum ({minimum})")));
        }
        Ok(Self(ArgumentType::Integer { minimum, maximum, description: description.into(), required }))
    }

    pub fn boolean(description: impl Into<String>, required: bool) -> Self {
        Self(ArgumentType::Boolean { description: description.into(), required })
    }

}
