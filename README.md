# EOW_SDK

EOW_SDK connects external applications to Echoes of the White (EOW), allowing game mods, plugins, and other host applications to communicate with EOW.

Host applications can use the SDK to submit context and state, register available actions, receive action requests, and report execution results. The SDK handles TCP communication, message envelopes, and response correlation. The host application is responsible for collecting state, handling action requests, and executing actions.

A C# implementation is currently available, targeting **.NET 6 and .NET 8**.

## C# SDK

### Available Methods

| Method | Purpose |
|---|---|
| `Startup()` | Connect, complete the handshake, and register the SDK instance |
| `Shutdown()` | Unregister the SDK instance and close the connection |
| `Context()` | Submit context |
| `StateSnapshot()` | Submit a state snapshot |
| `ActionRegister()` | Register actions |
| `ActionUnregister()` | Unregister specified actions |
| `ActionUpdate()` | Update action definitions |
| `ActionClear()` | Clear registered actions |
| `ActionDemand()` | Request a selection from the specified actions |
| `ActionResult()` | Report an action's execution result |

Each method builds and sends its envelope internally. `ActionDemand()` returns `(Ack, Action)`, with the matching action on success and `Action` set to `null` if the server rejects the demand. Other action requests arrive through the `ActionReceived` event. Wait for the current demand to finish or time out before sending another.

See the [API and protocol reference](reach_of_the_white/sdk/README.md) for data types, fields, and message formats.

### Build

Install a `.NET SDK` that supports the target frameworks. From the repository root, run:

```powershell
dotnet build .\reach_of_the_white\sdk\eow_sdk_cs\EowSdk.csproj -c Release
```

The project targets both `net6.0` and `net8.0`. You can also build either target separately:

```powershell
dotnet build .\reach_of_the_white\sdk\eow_sdk_cs\EowSdk.csproj -c Release -f net6.0
dotnet build .\reach_of_the_white\sdk\eow_sdk_cs\EowSdk.csproj -c Release -f net8.0
```

Build output is placed in `bin\Release\<target-framework>\` under the C# SDK project directory.

### Use in a Host Application

The SDK is a class library that a host application references and calls. The following example uses a .NET 8 console application.

From the repository root, create a sample host and add a project reference:

```powershell
dotnet new console -n EowSdkDemo -o .\temp\EowSdkDemo -f net8.0

dotnet add .\temp\EowSdkDemo\EowSdkDemo.csproj reference .\reach_of_the_white\sdk\eow_sdk_cs\EowSdk.csproj
```

Replace the sample host's `Program.cs` with:

```csharp
using System.Text.Json;
using Eow.Sdk;
using Eow.Sdk.Transport;

EowSdk sdk = new(sdkId: "sdk_demo", port: 7707);

Envelope startup = await sdk.Startup();
Console.WriteLine(JsonSerializer.Serialize(startup));

if (startup.Payload.Type == "ack" && !startup.Payload.Data!.Value.GetProperty("ok").GetBoolean()) {
    return;
}

try {
    Envelope response = await sdk.Context(
        new ContextData(
            content: "This is a test context from EOW_SDK.",
            silent: false
        )
    );

    Console.WriteLine(JsonSerializer.Serialize(response));
    Console.WriteLine("Press Enter to disconnect.");
    Console.ReadLine();
}
finally {
    await sdk.Shutdown();
}
```

The constructor accepts the following parameters:

- `sdkId`: The identifier for this SDK instance.
- `port`: The server's SDK connection port, supplied by the host.
- `timeout`: An optional communication timeout, defaulting to 30 seconds.

The SDK connects to `127.0.0.1` using the supplied port. The example uses `7707`, the port configured for EOW_Lite's Adapter.

Start a server that supports the SDK protocol, then run the sample:

```powershell
dotnet run --project .\temp\EowSdkDemo\EowSdkDemo.csproj
```

The sample prints the responses to `Startup()` and `Context()`. For an `ack` response, `payload.data.ok` indicates whether the request succeeded.

## Quick Testing with EOW_Lite

[EOW_Lite](https://github.com/KnowHT1515/EOW_Lite) is a lightweight EOW runtime which can be used for SDK development and testing.

> **Current integration status**
>
> EOW_Lite's Adapter currently handles process startup and shutdown only; SDK connections and protocol translation are not yet implemented. Full SDK-to-Lite integration testing requires the Adapter implementation.

### 1. Prepare EOW_Lite

Run the following commands in Windows PowerShell. Install `Rust` and `Cargo` first; EOW_Lite currently requires `Rust` 1.96 or later.

```powershell
git clone https://github.com/KnowHT1515/EOW_Lite.git
cd EOW_Lite

cargo build --workspace --bins --release
```

For the initial setup, copy the model configuration and prompt templates:

```powershell
Copy-Item .\glimpse_of_the_white\lite\config\example\core.toml .\glimpse_of_the_white\lite\config\core.toml

Copy-Item .\glimpse_of_the_white\lite\config\example\prompt .\glimpse_of_the_white\lite\config\prompt -Recurse
```

Edit the runtime configuration at `glimpse_of_the_white/lite/config/core.toml`. Set the full request URL, model name, and API key under `[LLM_Model]`.

Put real credentials in the runtime configuration. Keep placeholder values in the templates under `config/example/`.

### 2. Start and Verify Lite

From the EOW_Lite repository root, run:

```powershell
.\eow-lite.cmd link
```

To stop Lite, open another terminal in the same repository and run:

```powershell
.\eow-lite.cmd unlink
```

After adding the EOW_Lite repository root to `PATH`, you can use:

```powershell
eow-lite link
eow-lite unlink
```

### 3. Connect the SDK: Pending Adapter Implementation

Keep Lite running and launch the sample host from the EOW_SDK repository:

```powershell
dotnet run --project .\temp\EowSdkDemo\EowSdkDemo.csproj
```

Check the following results:

1. The SDK completes the TCP handshake, and `Startup()` returns an `ack` with `payload.data.ok` set to `true`.
2. `Context()` submits the test context and returns an `ack` with `payload.data.ok` set to `true`.
3. Pressing Enter in the sample host calls `Shutdown()` to unregister and disconnect.
