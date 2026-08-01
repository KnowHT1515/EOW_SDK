use std::sync::mpsc;

use eow_sdk_transport::{
    ActionDemand, ActionPriority, ActionResult, Assembly, EventId, SdkAction, SdkEnvelope,
    initialize,
};
use serde_json::{Map, json};

#[test]
fn assembly_uses_global_sdk_id_and_call_specific_targets() {
    let (inbound, _bucket) = mpsc::channel::<SdkEnvelope>();
    initialize("Cyberpunk", inbound).unwrap();
    let assembly = Assembly::global().unwrap();

    let startup: serde_json::Value =
        serde_json::from_str(&assembly.startup("silvy").unwrap()).unwrap();
    assert_eq!(startup["source"], "eow_sdk");
    assert_eq!(startup["target"], "silvy");
    assert_eq!(startup["payload"]["sdk_id"], "Cyberpunk");

    let context: serde_json::Value =
        serde_json::from_str(&assembly.context("rucia", "hello", true).unwrap()).unwrap();
    assert_eq!(context["target"], "rucia");
    assert_eq!(context["payload"]["data"]["silent"], true);

    let action = SdkAction {
        action_id: "open_door".into(),
        description: "Open the selected door".into(),
        arguments: Some(Map::from_iter([(
            "door".into(),
            json!({"type": "integer"}),
        )])),
        required: vec!["door".into()],
    };
    let register: serde_json::Value =
        serde_json::from_str(&assembly.action_register("silvy", vec![action]).unwrap()).unwrap();
    assert_eq!(register["payload"]["kind"], "register");
    assert_eq!(register["payload"]["data"][0]["action_id"], "open_door");

    let demand = ActionDemand {
        state: Some("combat".into()),
        goal: "survive".into(),
        permanent_context: false,
        priority: ActionPriority::Critical,
        actions: vec!["dodge".into()],
    };
    let demand: serde_json::Value =
        serde_json::from_str(&assembly.action_demand("rucia", demand).unwrap()).unwrap();
    assert_eq!(demand["payload"]["data"]["priority"], "critical");

    let result = ActionResult {
        id: Some("call-1".into()),
        ok: true,
        result: Some("done".into()),
    };
    let response: serde_json::Value = serde_json::from_str(
        &assembly
            .action_result("silvy", EventId::parse("712345678901").unwrap(), result)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(response["envelope_kind"], "response");
    assert_eq!(response["correlation_id"], "712345678901");

    assert!(assembly.startup("missing").is_err());
    assert!(assembly.startup("eow_sdk").is_err());
}
