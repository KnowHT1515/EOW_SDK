# Fake EOW

`fake-eow` is a small TCP stand-in for `Echoes_of_the_White`. It exists tomake SDK connection behavior observable without starting the real repository.

It intentionally implements the wire format independently from `eow-sdk-transport`. This keeps it useful as an external protocol peer instead of making both sides share the same framing and Envelope implementation.

## Fixed endpoint

The bundled [port.json](port.json) defines the test endpoint:

- instance: `knowht`
- address: `127.0.0.1`
- port: `7707`

Run the server from the SDK workspace:

```powershell
cargo run -p fake-eow
```

The process runs until it is interrupted. Each SDK connection is handled in its own thread, so one invalid or disconnected client does not stop the listener.

## Behavior

1. The client sends a framed handshake request.
2. The server accepts schema `1.0` with any non-empty SDK ID.
3. An accepted connection receives a `fake_eow.ready` event.
4. Every valid inbound SDK Envelope is logged.
5. An inbound request receives a `fake_eow.echo` response whose `correlation_id` is the request's `event_id`.
6. Inbound events and responses are logged without an automatic reply.

Unsupported handshake schemas and empty SDK IDs receive `deny`. Malformed framing, handshakes, Envelopes, or Envelope routes close only the affected connection.

Envelope JSON in logs is capped at 4096 characters so a maximum-size payload cannot flood the terminal.

## Scope

This crate contains only the fake EOW server and its own unit tests. It does not implement the SDK Transport connection lifecycle and does not contain Transport-to-fake-EOW integration tests.
