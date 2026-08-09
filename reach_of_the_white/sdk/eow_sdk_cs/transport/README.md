# EowSdk.Transport

`EowSdk.Transport` is the .NET 8 port of the Rust `eow-sdk-transport` crate. It keeps the same wire protocol and validation rules while exposing asynchronous TCP lifecycle methods appropriate for C#.

## Quick start

```csharp
using EowSdk.Transport;
using EowTransport = EowSdk.Transport.Transport;

ChannelReader<SdkEnvelope> inbound = EowTransport.Initialize("my-sdk-id");
await EowTransport.ConnectAsync("silvy");

Assembly assembly = Assembly.Global();
SendResult accepted = EowTransport.Send(assembly.Startup("silvy"));

SdkEnvelope message = await inbound.ReadAsync();
await EowTransport.DisconnectAsync("silvy");
```

`SendResult.Ok` means that the validated Envelope entered the target writer queue. It does not guarantee delivery to the peer.

## Public components

- `Transport`: one-time initialization, connect/disconnect, per-instance routing, send acceptance.
- `Assembly`: startup, shutdown, context, action register/unregister/update/clear/demand/result Envelopes.
- `SdkEnvelope`: strict static and contextual validation with unknown-field preservation.
- `InstanceRegistry`: bundled or custom instance/port registry validation.
- `FrameCodec`, `Deframer`, `FramedStream`: `u32` big-endian framing and `\\eow/` integrity sentinel.
- `HandshakeRequest`, `HandshakeResponse`: handshake wire shapes.
- `EventId`, `EventIdGenerator`: validated 12-digit event identities.

The default registry is embedded from `instance.json`. `Transport.Initialize` is process-wide and may be called only once, matching the Rust implementation.
