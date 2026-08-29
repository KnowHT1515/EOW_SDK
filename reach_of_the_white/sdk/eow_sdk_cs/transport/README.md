# EowSdk.Transport

`EowSdk.Transport` is the multi-targeted .NET port of the Rust `eow-sdk-transport` crate. It keeps the same wire protocol and validation rules while exposing asynchronous TCP lifecycle methods appropriate for C#.

## Supported frameworks

| Target framework | Support level |
| --- | --- |
| .NET 6 (`net6.0`) | Legacy compatibility; the runtime is no longer supported by Microsoft |
| .NET 8 (`net8.0`) | Supported |
| .NET 10 (`net10.0`) | Supported |

Use the .NET 10 SDK to build all target frameworks in one command:

```powershell
dotnet build EowSdk.Transport.csproj
```

The three builds expose the same public API and wire behavior. Consumers should reference the target selected by their project or NuGet tooling.

Run the transport test suite once per target framework. The matching runtime must be installed for each command:

```powershell
dotnet run --project ..\transport_tests\EowSdk.Transport.Tests.csproj -f net6.0
dotnet run --project ..\transport_tests\EowSdk.Transport.Tests.csproj -f net8.0
dotnet run --project ..\transport_tests\EowSdk.Transport.Tests.csproj -f net10.0
```

## Quick start

```csharp
using System.Text.Json.Nodes;
using EowSdk.Transport;
using EowTransport = EowSdk.Transport.Transport;

ChannelReader<SdkEnvelope> inbound = EowTransport.Initialize("my-sdk-id");
await EowTransport.ConnectAsync("silvy");

Assembly assembly = Assembly.Global();
SendResult accepted = EowTransport.Send(assembly.Startup("silvy"));
SdkEnvelope startupAck = await inbound.ReadAsync();
accepted = EowTransport.Send(assembly.StateSnapshot("silvy", 0,
    new JsonObject { ["room"] = "start", ["hp"] = 3 }));

SdkEnvelope message = await inbound.ReadAsync();
await EowTransport.DisconnectAsync("silvy");
```

`SendResult.Ok` means that the validated Envelope was accepted by the target's outbound policy. It does not guarantee delivery to the peer. Ordinary Envelopes use the reliable FIFO; `state.snapshot` uses a separate one-element latest-only slot, and an accepted nonzero snapshot may be superseded before transmission. Revision zero is protected as the startup-session baseline.

`Assembly.StateSnapshot` accepts a caller-managed `ulong` revision. Start from zero after the target's startup succeeds and advance it for each complete snapshot produced; gaps observed by the receiver are valid after queue coalescing.

## Public components

- `Transport`: one-time initialization, connect/disconnect, per-instance routing, send acceptance.
- `Assembly`: startup, shutdown, context, state snapshot, and action register/unregister/update/clear/demand/result Envelopes.
- `SdkEnvelope`: strict static and contextual validation with unknown-field preservation.
- `InstanceRegistry`: bundled or custom instance/port registry validation.
- `FrameCodec`, `Deframer`, `FramedStream`: `u32` big-endian framing and `\\eow/` integrity sentinel.
- `HandshakeRequest`, `HandshakeResponse`: handshake wire shapes.
- `EventId`, `EventIdGenerator`: validated 12-digit event identities.

The default registry is embedded from `instance.json`. `Transport.Initialize` is process-wide and may be called only once, matching the Rust implementation.
