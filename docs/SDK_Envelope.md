# SDK_Envelope

---

## Send

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

### startup

Requests registration of an `EOW_SDK` instance.

```json
{
    "envelope_kind": "request",
    "payload": { 
        "sdk_id": "<sdk_id>",
        "type": "startup"
    }
}
```

Registers an `EOW_SDK` instance on the `EOW` side.

---

### shutdown

Requests unregistration of an `EOW_SDK` instance.

```json
{
    "envelope_kind": "request",
    "payload": { 
        "sdk_id": "<sdk_id>",
        "type": "shutdown"
    }
}
```

Unregisters an `EOW_SDK` instance on the `EOW` side.

---

### context

Requests the addition of contextual information.

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

Provides additional contextual information to `EOW`. `silent: true` means that `EOW` will remain silent, while `silent: false` means that `EOW` may respond.

---

### action.register

Requests registration of an `action`, making it available.

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

Registers the `action` under this `EOW_SDK` instance on the `EOW` side, making it available for invocation at any time.

---

### action.unregister

Requests unregistration of an `action`, making it unavailable.

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

Unregisters the `action` under this `EOW_SDK` instance on the `EOW` side, making it no longer available.

---

### action.update

Requests an update to the list of available `action`s.

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

Fully replaces the list of available `action`s under this `EOW_SDK` instance on the `EOW` side.

---

### action.clear

Requests that the list of available `action`s be cleared.

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

Clears the entire list of available `action`s under this `EOW_SDK` instance on the `EOW` side.

---

### action.demand

Requests that `EOW` select and use an `action`.

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

Requests that `EOW` select and use an `action` from the `actions` list. If `permanent_context: true`, the current request will be added to the persistent context of `EOW`; otherwise, it will be treated only as a temporary value and discarded after use.

---

### action.result

Returns information about the execution of an `action` and its result.

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

Returns the execution information and result of the `action` to `EOW`.

---

## Recv

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

`<type>` includes `[startup, action, ack]`

---

### startup

Initialization information returned by `EOW` after a `startup` request. Currently, `EOW` returns an `ack` instead of this type of `SDK_Envelope`. This type of `SDK_Envelope` is retained for compatibility with `Neuro-SDK`.

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

---

### action

An `action` execution request initiated by `EOW`. The `EOW_SDK` instance processes the corresponding `action` behavior according to its own business logic.

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

### ack

General `request` acknowledgment information returned by `EOW`.

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

## event_id

The `event_id` is a 12-digit **numeric string** with the following structure:

> [1-digit code] + [9-digit timestamp] + [2-digit random number] = 12-digit numeric string

---

### Code

Fixed as `7`, the personal code assigned to KnowHT, the creator of this project.

---

### Timestamp

Formatted as `HHMMSSMMM`, representing hours, minutes, seconds, and milliseconds in UTC.

---

### Random Number

Range: `00`–`99`

---

## correlation_id

When `envelope_kind` is `response`, `correlation_id` must be a 12-digit ASCII numeric string matching `[0-9]{12}`.

When `envelope_kind` is `request` or `event`, `correlation_id` must be `null`.

`correlation_id` is transport-independent metadata. `Transport` validates only its shape and does not require it to identify a request seen on the same TCP connection, nor does it track whether a value is known, duplicated, or already consumed. A service or client may apply stronger request-response semantics within its own business logic.

When producing an acknowledgment or other direct response, implementations should normally copy the originating request's `event_id` into `correlation_id` for observability, but receivers must not depend on or enforce that convention at the transport layer.
