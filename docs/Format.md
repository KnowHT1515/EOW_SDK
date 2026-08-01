# Format

---

## SDK_Envelope

---

### Send

```json
{
    "envelope_schema": "1.0",
    "envelope_kind": "request" | "response" | "event",
    "source": "eow_sdk",
    "target": "<instance>",
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

`<instance>` must be present in `transport/instance.json`.

`<type>` includes `[startup, shutdown, context, action]`

---

#### startup

```json
{
    "envelope_kind": "request",
    "payload": { 
        "sdk_id": "<sdk_id>",
        "type": "startup"
    }
}
```

---

#### shutdown

```json
{
    "envelope_kind": "request",
    "payload": { 
        "sdk_id": "<sdk_id>",
        "type": "shutdown"
    }
}
```

---

#### context

```json
{
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

#### action.register

```json
{
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

#### action.unregister

```json
{
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

#### action.update

```json
{
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

#### action.clear

```json
{   
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "kind": "clear"
    }
}
```

---

#### action.demand

```json
{
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "kind": "demand",
        "data": {
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

#### action.result

```json
{
    "envelope_kind": "response",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "kind": "result",
        "data": {
            "id": "<string>",               // Neuro-SDK compatible, not used in Echoes_of_the_White
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
    "source": "<instance>",
    "target": "eow_sdk",
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

`<type>` inclues `[startup, action, ack]`

---

#### startup

```json
{   /* Neuro-SDK compatible, not used in Echoes_of_the_White */ 
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

#### action

```json
{
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "data": {
            "id": "<string>",   // Neuro-SDK compatible, not used in Echoes_of_the_White
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

```json
{
    "envelope_kind": "response",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "ack",
        "data": {
            "ok": <bool>
        }
    }
}
```

---

### event_id

The `event_id` is a 12-digit **numeric string** with the following structure:

> [1-digit code] + [9-digit timestamp] + [2-digit random number] = 12-digit numeric string

---

#### Code

Fixed as `7`, the personal code assigned to KnowHT, the creator of this project.

---

#### Timestamp

Formatted as `HHMMSSMMM`, representing hours, minutes, seconds, and milliseconds in UTC.

---

#### Random Number

Range: `00`–`99`

---

### correlation_id

When `envelope_kind` is `response`, the value of `correlation_id` must be identical to the `event_id` of the corresponding original request, establishing a one-to-one mapping between the request and response.

When `envelope_kind` is not `response`, the value of `correlation_id` should be `null`. However, a non-null value will not cause any consequences, as `EOW_SDK` does not use the `correlation_id` field when processing non-response envelopes.

---

## Neuro-SDK

---

### Send

---

#### startup

```json
{
    "command": "startup",
    "game": "<game_name>"
}
```

---

#### context

```json
{
    "command": "context",
    "game": "<game_name>",
    "data": {
        "message": "<context_message>",
        "silent": <bool>
    }
}
```

---

#### actions/register

```json
{
    "command": "actions/register",
    "game": "<game_name>",
    "data": {
        "actions": [
            {
                "name": "<action_name>",
                "description": "<action_description>"
            },
            {
                "name": "<action_name>",
                "description": "<action_description>",
                "schema": {
                    "type": "object",
                    "properties": {
                        "<argument_name>": {
                            "type": "string",
                            "enum": [
                                "<value>",
                                "<value>",
                                ...
                            ]
                        },
                        "<argument_name>": {
                            "type": "integer",
                            "minimum": <int>,
                            "maximum": <int>
                        },
                        "<argument_name>": {
                            "type": "boolean"
                        },
                        ...
                    },
                    "required": [
                        "<argument_name>",
                        "<argument_name>",
                        ...
                    ]
                }
            },
            ...
        ]
    }
}
```

---

#### actions/unregister

```json
{
    "command": "actions/unregister",
    "game": "<game_name>",
    "data": {
        "action_names": [
            "<action_name>",
            "<action_name>",
            ...
        ]
    }
}
```

---

#### actions/force

```json
{
    "command": "actions/force",
    "game": "<game_name>",
    "data": {
        "state": "<game_state>",    // Optional
        "query": "<action_instruction>",
        "ephemeral_context": <bool>,
        "priority": "low" | "medium" | "high" | "critical",
        "action_names": [
            "<action_name>",
            "<action_name>",
            ...
        ]
    }
}
```

---

#### action/result

```json
{
    "command": "action/result",
    "game": "<game_name>",
    "data": {
        "id": "<action_instance_id>",
        "success": <bool>,
        "message": "<result_message>"
    }
}
```

---

### Recv

---

#### startup

```json
{
    "command": "startup",
    "data": {
        "session": {
            "sessionId": "<session_id>",
            "characterId": "<character_id>",
            "displayName": "<display_name>"
        }
    }
}
```

---

#### action

```json
{
    "command": "action",
    "data": {
        "id": "<action_instance_id>",
        "name": "<action_name>",
        "data": "{\"<argument_name>\":\"<value>\",\"<argument_name>\":<int>,\"<argument_name>\":<bool>,...}"
    }
}
```