# Neuro-SDK

---

## Send

---

### startup

```json
{
    "command": "startup",
    "game": "<game_name>"
}
```

---

### context

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

### actions/register

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

### actions/unregister

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

### actions/force

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

### action/result

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

## Recv

---

### startup

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

### action

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