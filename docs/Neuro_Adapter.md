# Neuro Adapter

> Echoes_of_the_White <--> EOW_SDK <--> Neuro Adapter <--> Neuro-SDK <--> Neuro Game Adapter <--> Game
> Neuro-sama <--> Neuro-SDK <--> Neuro Adapter <--> EOW_SDK <--> EOW Game Adapter <--> Game

---

## Mapping

| EOW_SDK | Neuro-SDK |
| ------: | :-------- |
| **send** | |
| startup | startup |
| shutdown | - |
| context | context |
| action.register | actions/register |
| action.unregister | actions/unregister |
| action.update | actions/unregister + actions/register |
| action.clear | actions/unregister |
| action.demand | actions/force |
| action.result | action/result |
| **recv** | |
| startup | startup |
| action | action |
| ack | - |

---

## Send

---

### startup

```json
{   /* (from EOW_SDK) to Neuro-SDK */
    "command": "startup",
    "game": "<sdk_id>"
}
```


```json
{   /* from Neuro-SDK (to EOW_SDK) */
    "envelope_kind": "request",
    "payload": { 
        "sdk_id": "<game_name>",
        "type": "startup"
    }
}
```

---

### shutdown

There is no corresponding `Neuro-SDK` message.

---

### context

```json
{   /* (from EOW_SDK) to Neuro-SDK */
    "command": "context",
    "game": "<sdk_id>",
    "data": {
        "message": "<content>",
        "silent": <bool>
    }
}
```


```json
{   /* from Neuro-SDK (to EOW_SDK) */
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<game_name>",
        "type": "context",
        "data": {
            "content": "<content>",
            "silent": <bool>
        }
    }
}
```

---

### action.register

```json
{   /* (from EOW_SDK) to Neuro-SDK */
    "command": "actions/register",
    "game": "<sdk_id>",
    "data": {
        "actions": [
            {
                "name": "<action_id>",
                "description": "<action_description>"
            },
            {
                "name": "<action_id>",
                "description": "<action_description>",
                "schema": {
                    "type": "object",
                    "properties": {
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
                            "maximum": <int>
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
                }
            },
            ...
        ]
    }
}
```


```json
{   /* from Neuro-SDK (to EOW_SDK) */
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<game_name>",
        "type": "action",
        "kind": "register",
        "data": [
            {
                "action_id": "<action_name>",
                "description": "<action_description>"
            },
            {
                "action_id": "<action_name>",
                "description": "<action_description>",
                "arguments": {
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
                        "maximum": <int>,
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
            },
            ...
        ]
    }
}
```

---

### action.unregister

```json
{   /* (from EOW_SDK) to Neuro-SDK */
    "command": "actions/unregister",
    "game": "<sdk_id>",
    "data": {
        "action_names": [
            "<action_id>",
            "<action_id>",
            ...
        ]
    }
}
```


```json
{   /* from Neuro-SDK (to EOW_SDK) */
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<game_name>",
        "type": "action",
        "kind": "unregister",
        "data": [
            "<action_name>",
            "<action_name>",
            ...
        ]
    }
}
```

---

### action.update

```json
{   /* (from EOW_SDK) to Neuro-SDK  1/2 */
    "command": "actions/unregister",
    "game": "<sdk_id>",
    "data": {
        "action_names": [
            "<action_id>",
            "<action_id>",
            ...
        ]
    }
}
```

```json
{   /* (from EOW_SDK) to Neuro-SDK  2/2 */
    "command": "actions/register",
    "game": "<sdk_id>",
    "data": {
        "actions": [
            {
                "name": "<action_id>",
                "description": "<action_description>"
            },
            {
                "name": "<action_id>",
                "description": "<action_description>",
                "schema": {
                    "type": "object",
                    "properties": {
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
                            "maximum": <int>
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
                }
            },
            ...
        ]
    }
}
```

There is no unambiguous reverse mapping. `Neuro-SDK` `actions/unregister` and `actions/register` messages are converted independently into` action.unregister` and `action.register`. The Adapter must not infer `action.update`.

---

### action.clear

```json
{   /* (from EOW_SDK) to Neuro-SDK */
    "command": "actions/unregister",
    "game": "<sdk_id>",
    "data": {
        "action_names": [
            "<action_id>",
            "<action_id>",
            ...
        ]
    }
}
```

There is no unambiguous reverse mapping. A `Neuro-SDK` `actions/unregister` message is always converted into `action.unregister`. The Adapter must not infer `action.clear`.

---

### action.demand

```json
{   /* (from EOW_SDK) to Neuro-SDK */
    "command": "actions/force",
    "game": "<sdk_id>",
    "data": {
        "state": "<state_instruction>",    // Optional
        "query": "<goal_description>",
        "ephemeral_context": <bool>,       // ephemeral_context = !permanent_context
        "priority": "low" | "medium" | "high" | "critical",
        "action_names": [
            "<action_id>",
            "<action_id>",
            ...
        ]
    }
}
```


```json
{   /* from Neuro-SDK (to EOW_SDK) */
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<game_name>",
        "type": "action",
        "kind": "demand",
        "data": {
            "state": "<game_state>",        // Optional
            "goal": "<goal_description>",
            "permanent_context": <bool>,    // permanent_context = !ephemeral_context
            "priority": "low" | "medium" | "high" | "critical",
            "actions": [
                "<action_name>",
                "<action_name>",
                ...
            ]
        }
    }
}
```

---

### action.result

```json
{   /* (from EOW_SDK) to Neuro-SDK */
    "command": "action/result",
    "game": "<sdk_id>",
    "data": {
        "id": "<string>",
        "success": <bool>,
        "message": "<result_message>"   // Optional
    }
}
```


```json
{   /* from Neuro-SDK (to EOW_SDK) */
    "envelope_kind": "response",
    "payload": {
        "sdk_id": "<game_name>",
        "type": "action",
        "kind": "result",
        "data": {
            "id": "<action_instance_id>",
            "ok": <bool>,
            "result": "<result_message>"    // Optional
        }
    }
}
```

---

## Recv

---

### startup

```json
{   /* (from EOW_SDK) to Neuro-SDK */
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

```json
{   /* from Neuro-SDK (to EOW_SDK) */
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

### action

```json
{   /* (from EOW_SDK) to Neuro-SDK */
    "command": "action",
    "data": {
        "id": "<string>",
        "name": "<action_id>",
        "data": "{\"<argument_id>\":\"<argument>\",\"<argument_id>\":<int>,\"<argument_id>\":<bool>,...}"   // Optional
    }
}
```


```json
{   /* from Neuro-SDK (to EOW_SDK) */
    "envelope_kind": "request",
    "payload": {
        "sdk_id": "<sdk_id>",
        "type": "action",
        "data": {
            "id": "<action_instance_id>",
            "action_id": "<action_name>",
            "arguments": {  // Optional
                "<argument_name>": "<value>",
                "<argument_name>": <int>,
                "<argument_name>": <bool>,
                ...
            }                   
        }
    }
}
```

---

### ack

There is no corresponding `Neuro-SDK` message.