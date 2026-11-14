# EOW_SDK

## Public API

- `startup()`
- `shutdown()`
- `context(content: string, silent: bool)`
- `state_snapshot(data: SnapshotData)`
- `action_register(data: RegisterData)`
- `action_unregister(data: UnregisterData)`
- `action_update(data: UpdateData)`
- `action_clear()`
- `action_demand(data: DemandData)`
- `action_result(correlation_id: string, data: ResultData)`

---

## C# SDK

The SDK targets .NET 6 and .NET 8. Create an `Eow.Sdk.EowSdk` with an SDK ID and the local EOW port selected by the host.

```csharp
var sdk = new EowSdk(sdkId: "stardew_mod", port: 7707);
await sdk.Startup();
```

---

## SDK_Envelope

```json
{
    "envelope_schema": "1.0",
    "envelope_kind": "request" | "response" | "event",
    "source": "<source>",
    "target": "<target>",
    "event_id": "<12digit_string>",
    "correlation_id": null | "<12digit_string>",
    "ts_utc": "<iso8601_utc>",
    "payload": { 
        "sdk_id": "<sdk_id>",
        "type": "<type>",
        ...
    }
}
```

---

### Send

```json
{
    "envelope_schema": "1.0",
    "envelope_kind": "request" | "response" | "event",
    "source": "eow_sdk",
    "target": "eow",
    "event_id": "<12digit_string>",
    "correlation_id": null | "<12digit_string>",
    "ts_utc": "<iso8601_utc>",
    "payload": { 
        "sdk_id": "<sdk_id>",
        "type": "<type>",
        ...
    }
}
```

---

#### startup

Requests registration of an `EOW_SDK` instance.

```json
{   // -> startup / ack
    "envelope_kind": "request",
    "payload": { 
        "sdk_id": "<sdk_id>",
        "type": "startup"
    }
}
```

---

#### shutdown

Requests unregistration of an `EOW_SDK` instance.

```json
{   // -> ack
    "envelope_kind": "request",
    "payload": { 
        "sdk_id": "<sdk_id>",
        "type": "shutdown"
    }
}
```

---

### context

Requests the addition of contextual information.

```json
{   // -> ack
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "context",
        "data": {
            "content": "<content>",
            "silent": <bool>
        }
    }
}
```

---

#### state

##### snapshot

Publishes one complete game-state snapshot.

`data` is a `JSON object` defined by the host. `revision` starts at `0`, increments with each snapshot sent, and resets to `0` when restarts.

```json
{   // -> None
    "envelope_kind": "event",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "state",
        "kind": "snapshot",
        "revision": <int>,
        "data": {
            ...
        }
    }
}
```

---

#### action

##### register

Requests registration of `action`(s).

```json
{   // -> ack
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "kind": "register",
        "data": [
            {
                "action_id": "<action_id>",
                "description": "<action_description>"
            },
            {
                "action_id": "<action_id>",
                "description": "<action_description>",
                "arguments": {
                    "<argument_id>": {
                        "type": "string",
                        "enum": [
                            "<argument>",
                            "<argument>",
                            ...
                        ]
                    },
                    "<argument_id>": {
                        "type": "integer",
                        "minimum": <int>,
                        "maximum": <int>,
                    },
                    "<argument_id>": {
                        "type": "boolean"
                    },
                    ...
                },
                "required": [
                    "<argument_id>",
                    "<argument_id>",
                    ...
                ]
            },
            ...
        ]
    }
}
```

---

##### unregister

Requests unregistration of `action`(s).

```json
{   // -> ack
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "kind": "unregister",
        "data": [
            "<action_id>",
            "<action_id>",
            ...
        ]
    }
}
```

---

##### update

Requests an update to the list of available `action`s.

Fully replaces the list of available `action`s.

```json
{   // -> ack
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "kind": "update",
        "data": [
            {
                "action_id": "<action_id>",
                "description": "<action_description>"
            },
            {
                "action_id": "<action_id>",
                "description": "<action_description>",
                "arguments": {
                    "<argument_id>": {
                        "type": "string",
                        "enum": [
                            "<argument>",
                            "<argument>",
                            ...
                        ]
                    },
                    "<argument_id>": {
                        "type": "integer",
                        "minimum": <int>,
                        "maximum": <int>,
                    },
                    "<argument_id>": {
                        "type": "boolean"
                    },
                    ...
                },
                "required": [
                    "<argument_id>",
                    "<argument_id>",
                    ...
                ]
            },
            ...
        ]
    }
}
```

---

##### clear

Requests that the list of available `action`s be cleared.

```json
{   // -> ack
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "kind": "clear"
    }
}
```

---

##### demand

Requests that `EOW` select and use an `action`.

If `permanent_context: true`, the current request will be added to the persistent context of `EOW`.

```json
{   // -> ack & action
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "kind": "demand",
        "data": {
            "id": "<demand_id>",
            "state": "<state_instruction>",     // Optional
            "goal": "<goal_description>",
            "permanent_context": <bool>,
            "priority": "low" | "medium" | "high" | "critical",
            "actions": [
                "<action_id>",
                "<action_id>",
                ...
            ]
        }
    }
}
```

---

##### result

Returns information about the execution of an `action` and its result.

```json
{   // <- action
    "envelope_kind": "response",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "kind": "result",
        "data": {
            "ok": <bool>,
            "result": "<result_message>"    // Optional
        }
    }
}
```

---

### Recv

```json
{
    "envelope_schema": "1.0",
    "envelope_kind": "request" | "response" | "event",
    "source": "eow",
    "target": "eow_sdk",
    "event_id": "<12digit_string>",
    "correlation_id": null | "<12digit_string>",
    "ts_utc": "<iso8601_utc>",
    "payload": { 
        "sdk_id": "<sdk_id>",
        "type": "startup" | "action" | "ack",
        ...
    }
}
```

---

#### startup

Initialization information returned by `EOW` instance after a `startup` request. Currently, `EOW` returns an `ack` instead of this one. This is retained for compatibility with `Neuro-SDK`.

```json
{
    "envelope_kind": "response",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "startup",
        "data": {
            "sessionId": "<session_id>",
            "characterId": "<character_id>",
            "displayName": "<display_name>"
        }
    }
}
```

---

#### action

An `action` execution request initiated by `EOW` instance. The `EOW_SDK` instance processes the corresponding `action` behavior according to its own logic.

```json
{   // -> action.result
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "data": {
            "demand_id": "<demand_id>",   // Optional.
            "action_id": "<action_id>",
            "arguments": {      // Optional
                "<argument_id>": "<argument>",
                "<argument_id>": <int>,
                "<argumnet_id>": <bool>,
                ...
            }                   
        }
    }
}
```

---

#### ack

General `request` acknowledgment information returned by `EOW`.

```json
{
    "envelope_kind": "response",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "ack",
        "data": {
            "ok": <bool>,
            "result": { ... }
        }
    }
}
```
