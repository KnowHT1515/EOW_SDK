use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::protocol::{BusinessMessage, SdkAction};

pub(crate) type ActionCache = BTreeMap<String, SdkAction>;

#[derive(Debug)]
pub(crate) struct Translation {
    pub messages: Vec<String>,
    pub next_actions: Option<ActionCache>,
}

pub(crate) fn translate(
    message: &BusinessMessage,
    sdk_id: &str,
    current_actions: &ActionCache,
) -> Translation {
    match message {
        BusinessMessage::Startup => Translation::one(json!({
            "command": "startup",
            "game": sdk_id,
        })),
        BusinessMessage::Shutdown => Translation::empty(),
        BusinessMessage::Context { content, silent } => Translation::one(json!({
            "command": "context",
            "game": sdk_id,
            "data": {
                "message": content,
                "silent": silent,
            }
        })),
        BusinessMessage::ActionRegister(actions) => {
            let mut next = current_actions.clone();
            for action in actions {
                next.insert(action.action_id.clone(), action.clone());
            }
            Translation::actions_register(sdk_id, actions, Some(next))
        }
        BusinessMessage::ActionUnregister(action_ids) => {
            let mut next = current_actions.clone();
            for action_id in action_ids {
                next.remove(action_id);
            }
            let messages = if action_ids.is_empty() {
                Vec::new()
            } else {
                vec![serialize(json!({
                    "command": "actions/unregister",
                    "game": sdk_id,
                    "data": {"action_names": action_ids},
                }))]
            };
            Translation {
                messages,
                next_actions: Some(next),
            }
        }
        BusinessMessage::ActionUpdate(actions) => {
            let target = actions
                .iter()
                .cloned()
                .map(|action| (action.action_id.clone(), action))
                .collect::<ActionCache>();
            let mut unregister = Vec::new();
            let mut register = Vec::new();

            for (action_id, current) in current_actions {
                match target.get(action_id) {
                    None => unregister.push(action_id.clone()),
                    Some(target) if target != current => unregister.push(action_id.clone()),
                    Some(_) => {}
                }
            }
            for (action_id, target_action) in &target {
                if current_actions.get(action_id) != Some(target_action) {
                    register.push(target_action.clone());
                }
            }

            let mut messages = Vec::with_capacity(2);
            if !unregister.is_empty() {
                messages.push(serialize(json!({
                    "command": "actions/unregister",
                    "game": sdk_id,
                    "data": {"action_names": unregister},
                })));
            }
            if !register.is_empty() {
                messages.push(actions_register_json(sdk_id, &register));
            }
            Translation {
                messages,
                next_actions: Some(target),
            }
        }
        BusinessMessage::ActionClear => {
            let action_names = current_actions.keys().cloned().collect::<Vec<_>>();
            let messages = if action_names.is_empty() {
                Vec::new()
            } else {
                vec![serialize(json!({
                    "command": "actions/unregister",
                    "game": sdk_id,
                    "data": {"action_names": action_names},
                }))]
            };
            Translation {
                messages,
                next_actions: Some(ActionCache::new()),
            }
        }
        BusinessMessage::ActionDemand {
            state,
            goal,
            permanent_context,
            priority,
            actions,
        } => {
            let mut data = Map::new();
            if let Some(state) = state {
                data.insert("state".into(), Value::String(state.clone()));
            }
            data.insert("query".into(), Value::String(goal.clone()));
            data.insert("ephemeral_context".into(), Value::Bool(!permanent_context));
            data.insert("priority".into(), Value::String(priority.clone()));
            data.insert(
                "action_names".into(),
                Value::Array(actions.iter().cloned().map(Value::String).collect()),
            );
            Translation::one(json!({
                "command": "actions/force",
                "game": sdk_id,
                "data": data,
            }))
        }
        BusinessMessage::ActionResult { id, ok, result } => {
            let mut data = Map::new();
            data.insert("id".into(), Value::String(id.clone()));
            data.insert("success".into(), Value::Bool(*ok));
            if let Some(result) = result {
                data.insert("message".into(), Value::String(result.clone()));
            }
            Translation::one(json!({
                "command": "action/result",
                "game": sdk_id,
                "data": data,
            }))
        }
    }
}

impl Translation {
    fn empty() -> Self {
        Self {
            messages: Vec::new(),
            next_actions: None,
        }
    }

    fn one(value: Value) -> Self {
        Self {
            messages: vec![serialize(value)],
            next_actions: None,
        }
    }

    fn actions_register(
        sdk_id: &str,
        actions: &[SdkAction],
        next_actions: Option<ActionCache>,
    ) -> Self {
        let messages = if actions.is_empty() {
            Vec::new()
        } else {
            vec![actions_register_json(sdk_id, actions)]
        };
        Self {
            messages,
            next_actions,
        }
    }
}

fn actions_register_json(sdk_id: &str, actions: &[SdkAction]) -> String {
    serialize(json!({
        "command": "actions/register",
        "game": sdk_id,
        "data": {
            "actions": actions.iter().map(action_to_neuro).collect::<Vec<_>>(),
        }
    }))
}

fn action_to_neuro(action: &SdkAction) -> Value {
    let mut value = Map::new();
    value.insert("name".into(), Value::String(action.action_id.clone()));
    value.insert(
        "description".into(),
        Value::String(action.description.clone()),
    );
    if let Some(arguments) = &action.arguments {
        let mut schema = Map::new();
        schema.insert("type".into(), Value::String("object".into()));
        schema.insert("properties".into(), Value::Object(arguments.clone()));
        if !action.required.is_empty() {
            schema.insert(
                "required".into(),
                Value::Array(action.required.iter().cloned().map(Value::String).collect()),
            );
        }
        value.insert("schema".into(), Value::Object(schema));
    }
    Value::Object(value)
}

fn serialize(value: Value) -> String {
    serde_json::to_string(&value).expect("JSON values always serialize")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str, description: &str) -> SdkAction {
        SdkAction {
            action_id: id.to_owned(),
            description: description.to_owned(),
            arguments: None,
            required: Vec::new(),
        }
    }

    #[test]
    fn update_reconciles_the_complete_target_set() {
        let current = [
            ("keep".into(), action("keep", "same")),
            ("change".into(), action("change", "old")),
            ("remove".into(), action("remove", "old")),
        ]
        .into_iter()
        .collect();
        let update = BusinessMessage::ActionUpdate(vec![
            action("keep", "same"),
            action("change", "new"),
            action("add", "new"),
        ]);

        let translated = translate(&update, "game", &current);
        assert_eq!(translated.messages.len(), 2);
        let unregister: Value = serde_json::from_str(&translated.messages[0]).unwrap();
        assert_eq!(
            unregister["data"]["action_names"],
            json!(["change", "remove"])
        );
        let register: Value = serde_json::from_str(&translated.messages[1]).unwrap();
        assert_eq!(register["data"]["actions"][0]["name"], "add");
        assert_eq!(register["data"]["actions"][1]["name"], "change");
        assert_eq!(translated.next_actions.unwrap().len(), 3);
    }

    #[test]
    fn identical_update_is_an_empty_transaction() {
        let current = [("keep".into(), action("keep", "same"))]
            .into_iter()
            .collect();
        let translated = translate(
            &BusinessMessage::ActionUpdate(vec![action("keep", "same")]),
            "game",
            &current,
        );
        assert!(translated.messages.is_empty());
        assert_eq!(translated.next_actions.unwrap(), current);
    }

    #[test]
    fn explicit_register_and_unregister_use_neuro_shapes() {
        let mut properties = Map::new();
        properties.insert(
            "height".into(),
            json!({"type": "integer", "minimum": 1, "maximum": 3}),
        );
        let action = SdkAction {
            action_id: "jump".into(),
            description: "Jump".into(),
            arguments: Some(properties),
            required: vec!["height".into()],
        };
        let registered = translate(
            &BusinessMessage::ActionRegister(vec![action]),
            "game",
            &ActionCache::new(),
        );
        let value: Value = serde_json::from_str(&registered.messages[0]).unwrap();
        assert_eq!(value["command"], "actions/register");
        assert_eq!(value["data"]["actions"][0]["name"], "jump");
        assert_eq!(
            value["data"]["actions"][0]["schema"]["required"],
            json!(["height"])
        );

        let unregistered = translate(
            &BusinessMessage::ActionUnregister(vec!["jump".into()]),
            "game",
            &registered.next_actions.unwrap(),
        );
        let value: Value = serde_json::from_str(&unregistered.messages[0]).unwrap();
        assert_eq!(value["command"], "actions/unregister");
        assert_eq!(value["data"]["action_names"], json!(["jump"]));
        assert!(unregistered.next_actions.unwrap().is_empty());
    }

    #[test]
    fn demand_inverts_permanent_context() {
        let translated = translate(
            &BusinessMessage::ActionDemand {
                state: None,
                goal: "choose".into(),
                permanent_context: true,
                priority: "high".into(),
                actions: vec!["jump".into()],
            },
            "game",
            &ActionCache::new(),
        );
        let value: Value = serde_json::from_str(&translated.messages[0]).unwrap();
        assert_eq!(value["data"]["ephemeral_context"], false);
        assert_eq!(value["data"]["query"], "choose");
    }
}
