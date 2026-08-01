# Fake EOW

`fake-eow` is an independent TCP stand-in for `Echoes_of_the_White`. It deliberately does not reuse `eow-sdk-transport` framing, handshake, or Envelope code, so integration tests exercise the real wire contract from both sides.

## Fixed endpoint

The bundled [instance.json](instance.json) defines:

- instance: `knowht`
- address: `127.0.0.1`
- port: `7707`

Run it from the SDK workspace:

```powershell
cargo run -p fake-eow
```

The process runs until interrupted. Tests may instead use `spawn_with_config` with port `0` to obtain an ephemeral endpoint and a stoppable server handle.

## Behavior

1. The client sends a framed handshake containing schema and `sdk_id`.
2. Schema `1.0` and a non-empty, currently inactive SDK ID are accepted.
3. The session binds all later Envelopes to that handshake `sdk_id` and its own instance.
4. An accepted connection receives an instance-sourced `fake_eow.ready` event.
5. Ordinary requests receive a correlated `fake_eow.echo` response.
6. A shutdown request receives a correlated `ack` and closes that session.
7. Events and responses are logged without an automatic reply.

Malformed framing, handshakes, Envelopes, source/target routes, or mismatched payload `sdk_id` values close only the affected connection. Envelope JSON in logs is capped at 4096 characters.

## Scope

The crate is a protocol peer and controllable integration fixture. SDK connection lifecycle, target routing, Recv Bucket ownership, and reconnect policy remain implemented and tested in `eow-sdk-transport`.