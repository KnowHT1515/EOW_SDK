# Transport

---

## Overview

`Transport` provides the communication foundation for `EOW_SDK` to communicate with either `Echoes_of_the_White` or `Neuro-sama`, in accordance with the `Transport` specification defined by the main repository.

---

## SDK_Envelope

`EOW_SDK` uses the standard `SDK_Envelope` format to communicate with `Echoes_of_the_White`.

The format is as follows:

```json
{
    "envelope_schema": "1.0",
    "envelope_kind": "request" | "response" | "event",
    "source": "eow_sdk" | "<instance>",
    "target": "<instance>" | "eow_sdk",
    "event_id": "<12digit_string>",
    "correlation_id": null | "<12digit_string>",
    "ts_utc": "<iso8601_utc>",
    "payload": { ... }
}
```

In particular, `SDK_Envelope` may also be used for `Neuro-SDK compatible` communication.

In this case, the communication format is:

```json
{
    "envelope_schema": "1.0",
    "envelope_kind": "request" | "response" | "event",
    "source": "eow_sdk" | "neuro",
    "target": "neuro" | "eow_sdk",
    "event_id": "<12digit_string>",
    "correlation_id": null | "<12digit_string>",
    "ts_utc": "<iso8601_utc>",
    "payload": { ... }
}
```

Every endpoint other than the reserved `eow_sdk` value must be registered in `transport/instance.json`. The values of `target` and `source` determine which instance or adapter handles the Envelope.

`Transport` strictly validates the following:

- Whether the value of `envelope_schema` is within the currently supported range
- Whether the value of `envelope_kind` is a valid enumeration value
- Whether both `source` and `target` are present, have different values, and are either `eow_sdk` or an instance registered in `instance.json`
- Whether `event_id` is a `12-digit numeric string`
- Whether `correlation_id` is a `12-digit numeric string` for `response`, and `null` for `request` or `event`
- Whether `payload` is present and is a non-empty object

If any validation fails, the `SDK_Envelope` must not be created.

Additionally, the size of a single serialized `SDK_Envelope`, measured in `UTF-8` bytes, must not exceed `15 * 1024 * 1024 bytes`. Any `SDK_Envelope` exceeding this limit is considered invalid and will be rejected by `Transport`.

---

### SDK_Envelope Assembly

`Transport` explicitly provides assembly entry points for each type of `SDK_Envelope`. `Assembly` references the process-wide immutable `sdk_id`, accepts `target` on every assembly call, populates the remaining required fields, and returns a specification-compliant `SDK_Envelope`.

`Assembly` uses the SDK-wide `ts_utc` and `event_id` generators so all target instances share one event identity source.

Using `Rust` as an example, the following section shows all `SDK_Envelope` assembly methods provided by `Assembly`.

```rust
fn startup(
    &self,
    target: &str,
) -> AssemblyResult

fn shutdown(
    &self,
    target: &str,
) -> AssemblyResult

fn context(
    &self,
    target: &str,
    content: &str,
    silent: bool,
) -> AssemblyResult

fn action_register(
    &self,
    target: &str,
    actions: Vec<SdkAction>,
) -> AssemblyResult

fn action_unregister(
    &self,
    target: &str,
    action_ids: Vec<String>,
) -> AssemblyResult

fn action_update(
    &self,
    target: &str,
    actions: Vec<SdkAction>,
) -> AssemblyResult

fn action_clear(
    &self,
    target: &str,
) -> AssemblyResult

fn action_demand(
    &self,
    target: &str,
    demand: ActionDemand,
) -> AssemblyResult

fn action_result(
    &self,
    target: &str,
    correlation_id: EventId,
    result: ActionResult
) -> AssemblyResult

type AssemblyResult = Result<String, AssemblyError>;
```

---

## TCP Transmission

`EOW_SDK` communicates with the main `Echoes_of_the_White` repository through a `full-duplex TCP channel`.

---

### TCP Framing

TCP byte-stream framing within the project follows the specification below:

> [ `length_prefix` : 4 bytes ] [ `body` : `length_prefix` bytes ] [ `sentinel` : 5 bytes ]

---

#### length_prefix

The message length prefix has a fixed size of `4` bytes and is encoded as an unsigned integer in big-endian byte order, also known as network byte order.

Its value represents the number of bytes in the immediately following `body` segment. It does not include the `length_prefix` itself or the `sentinel`.

`length_prefix` represents the byte length of the `body` after serialization into `UTF-8`, rather than the number of characters. The sender must first serialize the message content into a `UTF-8` byte sequence and then calculate the length of that byte sequence. The number of characters or code units must not be used as the value of `length_prefix`.

The value of `length_prefix` must not exceed `20 * 1024 * 1024 bytes`. Otherwise, the entire `TCP` connection must be aborted.

The value of `length_prefix` must not be `0`. Otherwise, the entire `TCP` connection must be aborted.

---

#### body

The message body has the byte length specified by `length_prefix` and contains the `UTF-8` byte sequence of one complete serialized message.

The framing layer treats the `body` segment as opaque. It only extracts the specified number of bytes according to `length_prefix` and does not parse the contents. Once extracted, the segment is passed to the upper layer for parsing.

---

#### sentinel

The message-end sentinel is the fixed byte sequence `\eow/`, encoded in `UTF-8` as `5` bytes: `0x5C` `0x65` `0x6F` `0x77` `0x2F`.

The `sentinel` does not participate in frame boundary determination. Message boundaries are determined solely by `length_prefix`.

The `sentinel` is used only to verify frame integrity. After reading the number of `body` bytes specified by `length_prefix`, the receiver must read an additional fixed `5` bytes and verify that they are equal to `\eow/`.

- If validation succeeds, the frame is considered complete and the stream is considered correctly aligned. The message is accepted, and the receiver continues reading the `length_prefix` of the next message.
- If validation fails, the byte stream is considered misaligned and no longer trustworthy. Because the location of subsequent `length_prefix` fields can no longer be trusted and no reliable resynchronization anchor exists, the receiver must abort the entire `TCP` connection. It must not attempt to skip the frame or search the stream for the next `sentinel` to resynchronize.

---

### TCP Connection

`Echoes_of_the_White` acts as the server and listens on a designated port. `EOW_SDK` then acts as the client and connects to the server.

After the connection has been established, an additional handshake must be performed to establish the `sdk_id` identity of the `EOW_SDK` side and the `instance` identity of the `EOW` side.

The connection between the main repository and the SDK repository is considered fully established only after all of the above steps have been completed.

By default, the project uses `127.0.0.1` over IPv4 as the connection address.

---

#### Ports

An `EOW_SDK` instance connects to a specified `EOW` instance using either externally supplied parameters or internally predefined settings.

Every registered instance in `transport/instance.json` must carry a unique `port`, represented as a string integer in `1..=65535`. Missing, empty, non-string, zero, out-of-range, and duplicate ports make the registry invalid and fail initialization.

An instance name must match `[a-z][a-z0-9_-]{0,63}`. The reserved `eow_sdk` endpoint is legal in an Envelope but must not be registered and cannot be connected.

---

#### Handshake

The handshake between `EOW_SDK` and `Echoes_of_the_White` must follow the specification below. The `sdk_id` must contain `1..=128` UTF-8 bytes without surrounding whitespace or control characters.

First, `EOW_SDK` sends the following message to `EOW`:

```json
{   /* EOW_SDK -> Echoes_of_the_White */
    "handshake": {
        "schema": "1.0",
        "id": "<sdk_id>"
    }
}
```

`EOW` then verifies whether `<sdk_id>` is a registered and valid value. If it is valid, `EOW` returns:

```json
{   /* Echoes_of_the_White -> EOW_SDK */
    "handshake": {
        "schema": "1.0",
        "result": "accept",
        "instance": "<instance>"
    }
}
```

If `<sdk_id>` is invalid, `EOW` returns:

```json
{   /* Echoes_of_the_White -> EOW_SDK */
    "handshake": {
        "schema": "1.0",
        "result": "deny"
    }
}
```

The connection is then closed.

Finally, when `result` is `accept`, `EOW_SDK` verifies whether the returned `<instance>` value matches the expected `<instance>` value.

If the value is valid, the connection is considered fully established. Otherwise, `EOW_SDK` closes the connection.

TCP establishment, writing the complete handshake request, and reading the complete response share one absolute deadline. The default is five seconds. Partial response bytes do not restart the deadline. Temporary socket timeouts are cleared after acceptance so a normally idle runtime connection remains valid.

## Correlation

`Transport` validates only the structural rule for `correlation_id`: it is `null` for `request` and `event`, and a 12-digit ASCII numeric string for `response`. It does not maintain pending-request sets or require a response to match a request observed on the same TCP connection. This permits request and response paths to use different transports or adapter processes.

Stronger correlation semantics, along with message-specific payload schemas such as `startup`, `context`, or `action.*`, remain the responsibility of the service or client business layer.

## Recv Bucket

Since inbound `SDK_Envelope`s are generally low-frequency, EOW_SDK owns one unbounded Recv Bucket and gives Transport its producer. Every connected instance writes validated Envelopes into that bucket. FIFO is guaranteed within each TCP connection; across instances, processing order is the actual order in which Envelopes enter the bucket. Transport does not expose a user-facing `recv` API.

---

## SDK_Transport_API

`Transport` exposes the following public API. EOW_SDK initializes the process-wide immutable `sdk_id` and Recv Bucket producer once before using these calls. One SDK runtime may maintain one active connection per registered instance; automatic reconnection is not performed.

---

### connect

Connects the process-wide SDK identity to one registered EOW instance. TCP connection, handshake acceptance, schema validation, and returned-instance validation must all succeed before the connection becomes active.

```rust
fn connect(instance: &str) -> Result<(), ConnectError>;
fn connect_timeout(instance: &str, timeout: Duration) -> Result<(), ConnectError>;
```

`connect` uses the five-second default absolute handshake deadline; `connect_timeout` accepts an explicit deadline duration. Connecting an already-active instance is rejected. Other instance connections are unaffected.

---

### disconnect

Closes one instance connection without sending a business-level shutdown Envelope. Already accepted outbound frames are drained in FIFO order before the socket and workers are closed.

```rust
fn disconnect(instance: &str) -> Result<(), DisconnectError>;
```

After disconnect completes, the caller may manually connect the same instance again. No automatic reconnect is attempted.

`shutdown(target)` remains the normal business lifecycle request. EOW_SDK waits for its correlated acknowledgement and then releases that target connection; `disconnect` is the fallback when shutdown cannot complete. `action.clear` never closes TCP.

---

### send

Submits a serialized `SDK_Envelope` to a running `EOW` instance.

```rust
fn send(sdk_envelope_json: String) -> SendResult;

struct SendResult {
    ok: bool,
    violation: Vec<String>,
}
```

`sdk_envelope_json` is a `UTF-8`-encoded `JSON` string that must conform to the `SDK_Envelope` specification. Its validity will be checked by `Transport` before it is actually sent.

`send` validates the Envelope, reads its `target`, and synchronously enqueues it on that instance connection. It returns an acceptance result as `SendResult`.

The semantics of `SendResult.ok` are **accepted**, not **delivered**. `SendResult.ok == true` only indicates that the `SDK_Envelope` has entered the target connection writer queue and does **not guarantee** that the `target` has received it. Delivery-level failures are not represented by `SendResult`.
