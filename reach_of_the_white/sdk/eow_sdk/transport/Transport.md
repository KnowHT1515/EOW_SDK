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
    "source": "eow_sdk" | "eow",
    "target": "eow" | "eow_sdk",
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

The values of `target` and `source` determine whether `EOW_SDK` applies `Neuro-SDK compatible` conversion to the envelope.

`Transport` strictly validates the following:

- Whether the value of `envelope_schema` is within the currently supported range
- Whether the value of `envelope_kind` is a valid enumeration value
- Whether both `source` and `target` are present and have different values
- Whether `event_id` is a `12-digit numeric string`
- Whether `correlation_id` is either `null` or a `12-digit numeric string`
- Whether `payload` is present and is a non-empty object

If any validation fails, the `SDK_Envelope` must not be created.

Additionally, the size of a single serialized `SDK_Envelope`, measured in `UTF-8` bytes, must not exceed `15 * 1024 * 1024 bytes`. Any `SDK_Envelope` exceeding this limit is considered invalid and will be rejected by `Transport`.

## TCP Transmission

`EOW_SDK` communicates with the main `Echoes_of_the_White` repository through a `full-duplex TCP channel`.

### TCP Framing

TCP byte-stream framing within the project follows the specification below:

> [ `length_prefix` : 4 bytes ] [ `body` : `length_prefix` bytes ] [ `sentinel` : 5 bytes ]

#### length_prefix

The message length prefix has a fixed size of `4` bytes and is encoded as an unsigned integer in big-endian byte order, also known as network byte order.

Its value represents the number of bytes in the immediately following `body` segment. It does not include the `length_prefix` itself or the `sentinel`.

`length_prefix` represents the byte length of the `body` after serialization into `UTF-8`, rather than the number of characters. The sender must first serialize the message content into a `UTF-8` byte sequence and then calculate the length of that byte sequence. The number of characters or code units must not be used as the value of `length_prefix`.

The value of `length_prefix` must not exceed `20 * 1024 * 1024 bytes`. Otherwise, the entire `TCP` connection must be aborted.

The value of `length_prefix` must not be `0`. Otherwise, the entire `TCP` connection must be aborted.

#### body

The message body has the byte length specified by `length_prefix` and contains the `UTF-8` byte sequence of one complete serialized message.

The framing layer treats the `body` segment as opaque. It only extracts the specified number of bytes according to `length_prefix` and does not parse the contents. Once extracted, the segment is passed to the upper layer for parsing.

#### sentinel

The message-end sentinel is the fixed byte sequence `\eow/`, encoded in `UTF-8` as `5` bytes: `0x5C` `0x65` `0x6F` `0x77` `0x2F`.

The `sentinel` does not participate in frame boundary determination. Message boundaries are determined solely by `length_prefix`.

The `sentinel` is used only to verify frame integrity. After reading the number of `body` bytes specified by `length_prefix`, the receiver must read an additional fixed `5` bytes and verify that they are equal to `\eow/`.

- If validation succeeds, the frame is considered complete and the stream is considered correctly aligned. The message is accepted, and the receiver continues reading the `length_prefix` of the next message.
- If validation fails, the byte stream is considered misaligned and no longer trustworthy. Because the location of subsequent `length_prefix` fields can no longer be trusted and no reliable resynchronization anchor exists, the receiver must abort the entire `TCP` connection. It must not attempt to skip the frame or search the stream for the next `sentinel` to resynchronize.

### TCP Connection

`Echoes_of_the_White` acts as the server and listens on a designated port. `EOW_SDK` then acts as the client and connects to the server.

After the connection has been established, an additional handshake must be performed to establish the `sdk_id` identity of the `EOW_SDK` side and the `instance` identity of the `EOW` side.

The connection between the main repository and the SDK repository is considered fully established only after all of the above steps have been completed.

By default, the project uses `127.0.0.1` over IPv4 as the connection address.

#### Ports

An `EOW_SDK` instance connects to a specified `EOW` instance using either externally supplied parameters or internally predefined settings.

The mapping between `EOW` instances and ports is stored in the local `transport/port.json` file.

#### Handshake

The handshake between `EOW_SDK` and `Echoes_of_the_White` must follow the specification below.

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
