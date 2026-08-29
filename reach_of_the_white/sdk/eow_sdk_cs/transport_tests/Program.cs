using System.Buffers.Binary;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Threading.Channels;
using EowSdk.Transport;
using EowTransport = EowSdk.Transport.Transport;

await TransportTests.RunAsync();

internal static class TransportTests
{
    private const string SdkId = "Cyberpunk";

    internal static async Task RunAsync()
    {
        TestFraming();
        await TestFramedStreamAsync();
        TestRegistry();
        TestEventIds();
        TestHandshake();
        TestEnvelope();
        await TestRuntimeAndAssemblyAsync();
        Console.WriteLine("All EowSdk.Transport tests passed.");
    }

    private static void TestFraming()
    {
        byte[] body = Encoding.UTF8.GetBytes("你好\\eow/世界");
        byte[] framed = FrameCodec.Encode(body);
        Equal((uint)body.Length, BinaryPrimitives.ReadUInt32BigEndian(framed), "UTF-8 frame length");
        True(framed.AsSpan(framed.Length - 5).SequenceEqual("\\eow/"u8), "sentinel");
        DecodeResult decoded = FrameCodec.Decode(framed);
        Equal(DecodeStatus.Frame, decoded.Status, "decode status");
        True(body.SequenceEqual(decoded.Body!), "decoded body");

        byte[] first = FrameCodec.Encode("first"u8);
        byte[] second = FrameCodec.Encode(Encoding.UTF8.GetBytes("第二帧"));
        byte[] both = [.. first, .. second];
        for (int split = 0; split <= both.Length; split++)
        {
            var deframer = new Deframer();
            var bodies = new List<byte[]>();
            deframer.Push(both.AsSpan(0, split));
            while (deframer.NextFrame() is byte[] value) bodies.Add(value);
            deframer.Push(both.AsSpan(split));
            while (deframer.NextFrame() is byte[] value) bodies.Add(value);
            Equal(2, bodies.Count, $"split {split} frame count");
            True(first.AsSpan(4, 5).SequenceEqual(bodies[0]), $"split {split} first body");
            True(Encoding.UTF8.GetBytes("第二帧").SequenceEqual(bodies[1]), $"split {split} second body");
        }

        Equal(FrameAbort.ZeroLength, FrameCodec.Decode(new byte[4]).AbortReason, "zero length abort");
        byte[] oversizedPrefix = new byte[4];
        BinaryPrimitives.WriteUInt32BigEndian(oversizedPrefix,
            TransportLimits.MaxFrameBodyBytes + 1u);
        Equal(FrameAbort.Oversize, FrameCodec.Decode(oversizedPrefix).AbortReason, "oversize abort");

        byte[] corrupt = FrameCodec.Encode("body"u8);
        corrupt[^1] = (byte)'!';
        var poisoned = new Deframer();
        poisoned.Push(corrupt);
        Throws<FramedIoException>(() => poisoned.NextFrame(),
            error => error.AbortReason == FrameAbort.BadSentinel, "bad sentinel");
        Throws<FramedIoException>(() => poisoned.Push("later"u8),
            error => error.AbortReason == FrameAbort.BadSentinel, "permanent abort");
    }

    private static async Task TestFramedStreamAsync()
    {
        using var stream = new MemoryStream();
        var writer = new FramedStream(stream);
        await writer.WriteFrameAsync("one"u8.ToArray());
        await writer.WriteFrameAsync(Encoding.UTF8.GetBytes("二"));
        stream.Position = 0;
        var reader = new FramedStream(stream, 1);
        byte[] firstBody = (await reader.ReadFrameAsync())!;
        True("one"u8.SequenceEqual(firstBody), "framed stream first");
        byte[] secondBody = (await reader.ReadFrameAsync())!;
        True(Encoding.UTF8.GetBytes("二").SequenceEqual(secondBody),
            "framed stream unicode");
        True(await reader.ReadFrameAsync() is null, "framed stream EOF");

        byte[] truncated = FrameCodec.Encode("partial"u8)[..^1];
        var truncatedReader = new FramedStream(new MemoryStream(truncated));
        await ThrowsAsync<FramedIoException>(() => truncatedReader.ReadFrameAsync().AsTask(),
            error => error.Kind == FramedIoErrorKind.TruncatedFrame, "truncated frame");
    }

    private static void TestRegistry()
    {
        InstanceRegistry registry = InstanceRegistry.Bundled();
        Equal(3, registry.Count, "bundled registry count");
        Equal((ushort)7107, registry.Port("silvy"), "silvy port");
        True(registry.IsEnvelopeEndpoint("eow_sdk"), "reserved envelope endpoint");
        True(!registry.IsRegisteredInstance("eow_sdk"), "reserved endpoint not connectable");

        Throws<ConfigException>(() => InstanceRegistry.FromJson(
                "[{\"instance\":\"silvy\",\"port\":\"7107\"},{\"instance\":\"silvy\",\"port\":\"7108\"}]"),
            error => error.Kind == ConfigErrorKind.DuplicateInstance, "duplicate instance");
        Throws<ConfigException>(() => InstanceRegistry.FromJson(
                "[{\"instance\":\"silvy\",\"port\":7107}]"),
            error => error.Kind == ConfigErrorKind.InvalidPort, "numeric port");
        Throws<ConfigException>(() => InstanceRegistry.FromJson(
                "[{\"instance\":\"Silvy\",\"port\":\"7107\"}]"),
            error => error.Kind == ConfigErrorKind.InvalidInstance, "invalid instance");
    }

    private static void TestEventIds()
    {
        Equal("712345678901", EventId.Parse("712345678901").Value, "event ID parse");
        True(!EventId.IsValid("１２３４５６７８９０１２"), "ASCII-only event ID");
        var generator = new EventIdGenerator(new FixedClock(), new FixedRandom(5));
        Equal("701020300405", generator.Generate().Value, "event ID layout");
        Throws<InvalidOperationException>(
            () => new EventIdGenerator(new FixedClock(), new FixedRandom(100)).Generate(),
            _ => true, "event random range");
    }

    private static void TestHandshake()
    {
        var request = new HandshakeRequest(SdkId);
        Equal(request, HandshakeRequest.Parse(request.ToBody()), "handshake request round trip");
        var accept = new HandshakeResponse.Accept("silvy");
        Equal(accept, HandshakeResponse.Parse(accept.ToBody()), "accept round trip");
        var deny = new HandshakeResponse.Deny();
        Equal(deny, HandshakeResponse.Parse(deny.ToBody()), "deny round trip");
        Throws<HandshakeException>(() => HandshakeResponse.Parse(
                "{\"handshake\":{\"schema\":\"1.0\",\"result\":\"deny\",\"instance\":\"silvy\"}}"u8),
            error => error.Kind == HandshakeErrorKind.InvalidShape, "deny instance forbidden");
    }

    private static void TestEnvelope()
    {
        JsonObject valid = ValidEnvelope();
        valid["extension"] = new JsonObject { ["future"] = true };
        valid["payload"]!["nested"] = new JsonObject { ["value"] = 7 };
        SdkEnvelope envelope = SdkEnvelope.FromJsonNode(valid);
        Equal(EnvelopeKind.Request, envelope.Kind, "envelope kind");
        True(envelope.ToJsonObject()["extension"]!["future"]!.GetValue<bool>(),
            "unknown field preserved");
        True(!envelope.ToJsonObject().ContainsKey("ts_utc"), "timestamp remains optional");

        JsonObject detachedPayload = envelope.Payload;
        detachedPayload["sdk_id"] = "mutated";
        detachedPayload["nested"]!["value"] = 42;
        Equal(SdkId, envelope.PayloadSdkId, "payload clone protects SDK ID");
        Equal(7, envelope.Payload["nested"]!["value"]!.GetValue<int>(),
            "payload clone protects nested content");

        JsonObject detachedEnvelope = envelope.ToJsonObject();
        detachedEnvelope["target"] = "rucia";
        Equal("silvy", envelope.Target, "envelope clone protects routing fields");

        JsonObject response = ValidEnvelope();
        response["envelope_kind"] = "response";
        Throws<EnvelopeException>(() => SdkEnvelope.FromJsonNode(response),
            error => error.Violations.SequenceEqual([EnvelopeViolation.CorrelationId]),
            "response correlation required");
        response["correlation_id"] = "712345678900";
        Equal("712345678900", SdkEnvelope.FromJsonNode(response).CorrelationId,
            "response correlation accepted");

        Throws<EnvelopeException>(() => SdkEnvelope.FromJsonNode(new JsonObject()),
            error => error.Violations.SequenceEqual(Enum.GetValues<EnvelopeViolation>()),
            "stable six violations");
    }

    private static async Task TestRuntimeAndAssemblyAsync()
    {
        True(!EowTransport.Send("{}").Ok, "send before initialization");
        Throws<AssemblyException>(() => Assembly.Global(),
            error => error.Kind == AssemblyErrorKind.NotInitialized, "assembly before initialization");

        await using var silvy = new FakePeer("silvy", SdkId);
        await using var rucia = new FakePeer("rucia", SdkId);
        ChannelReader<SdkEnvelope> inbound = EowTransport.Initialize(SdkId);
        Assembly assembly = Assembly.Global();

        var argumentOwner = new JsonObject
        {
            ["arguments"] = new JsonObject
            {
                ["door"] = new JsonObject { ["type"] = "integer" }
            }
        };
        var parentedArguments = (JsonObject)argumentOwner["arguments"]!;
        string registeredAction = assembly.ActionRegister("silvy",
        [
            new SdkAction("open_door", "Open the selected door", parentedArguments, ["door"])
        ]);
        Equal("integer", JsonNode.Parse(registeredAction)!["payload"]!["data"]![0]!["arguments"]!["door"]!["type"]!.GetValue<string>(),
            "action arguments are cloned from their existing owner");

        await EowTransport.ConnectAtAsync("silvy", silvy.EndPoint);
        await NextAsync(inbound, "silvy", "fake_eow.ready");
        await EowTransport.ConnectAtAsync("rucia", rucia.EndPoint);
        await NextAsync(inbound, "rucia", "fake_eow.ready");

        await ThrowsAsync<ConnectException>(() => EowTransport.ConnectAtAsync("silvy", silvy.EndPoint),
            error => error.Kind == ConnectErrorKind.AlreadyConnected, "duplicate connection");

        JsonObject startup = JsonNode.Parse(assembly.Startup("silvy"))!.AsObject();
        Equal("eow_sdk", startup["source"]!.GetValue<string>(), "assembly source");
        Equal("silvy", startup["target"]!.GetValue<string>(), "assembly target");
        Equal(SdkId, startup["payload"]!["sdk_id"]!.GetValue<string>(), "assembly SDK ID");
        True(EowTransport.Send(startup.ToJsonString()).Ok, "send silvy accepted");
        await NextAsync(inbound, "silvy", "fake_eow.echo");

        string context = assembly.Context("rucia", "hello", true);
        True(EowTransport.Send(context).Ok, "send rucia accepted");
        await NextAsync(inbound, "rucia", "fake_eow.echo");

        string demand = assembly.ActionDemand("rucia", new ActionDemand(
            "survive", false, ActionPriority.Critical, ["dodge"], "combat"));
        Equal("critical", JsonNode.Parse(demand)!["payload"]!["data"]!["priority"]!.GetValue<string>(),
            "assembly demand priority");

        await EowTransport.DisconnectAsync("silvy");
        True(!EowTransport.Send(assembly.ActionClear("silvy")).Ok, "disconnected target rejected");
        True(EowTransport.Send(assembly.ActionClear("rucia")).Ok, "other target remains active");
        await NextAsync(inbound, "rucia", "fake_eow.echo");

        await EowTransport.ConnectAtAsync("silvy", silvy.EndPoint);
        await NextAsync(inbound, "silvy", "fake_eow.ready");
        await EowTransport.DisconnectAsync("silvy");
        await EowTransport.DisconnectAsync("rucia");

        await TestAbsoluteHandshakeTimeoutAsync();
    }

    private static async Task TestAbsoluteHandshakeTimeoutAsync()
    {
        var listener = new TcpListener(IPAddress.Loopback, 0);
        listener.Start();
        Task peer = Task.Run(async () =>
        {
            using TcpClient client = await listener.AcceptTcpClientAsync();
            await Task.Delay(300);
        });
        try
        {
            var endpoint = (IPEndPoint)listener.LocalEndpoint;
            await ThrowsAsync<ConnectException>(
                () => EowTransport.ConnectAtAsync("silvy", endpoint, TimeSpan.FromMilliseconds(60)),
                error => error.Kind == ConnectErrorKind.HandshakeTimeout,
                "absolute handshake timeout");
        }
        finally
        {
            listener.Stop();
            await peer;
        }
    }

    private static async Task<SdkEnvelope> NextAsync(
        ChannelReader<SdkEnvelope> reader,
        string source,
        string messageType)
    {
        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(3));
        while (true)
        {
            SdkEnvelope envelope = await reader.ReadAsync(timeout.Token);
            if (envelope.Source == source &&
                envelope.Payload["type"]?.GetValue<string>() == messageType)
                return envelope;
        }
    }

    private static JsonObject ValidEnvelope() => new()
    {
        ["envelope_schema"] = "1.0",
        ["envelope_kind"] = "request",
        ["source"] = "eow_sdk",
        ["target"] = "silvy",
        ["event_id"] = "712345678901",
        ["correlation_id"] = null,
        ["payload"] = new JsonObject { ["sdk_id"] = SdkId, ["type"] = "startup" }
    };

    private static void True(bool condition, string message)
    {
        if (!condition) throw new Exception($"Assertion failed: {message}");
    }

    private static void Equal<T>(T expected, T actual, string message)
    {
        if (!EqualityComparer<T>.Default.Equals(expected, actual))
            throw new Exception($"Assertion failed ({message}): expected {expected}, actual {actual}");
    }

    private static void Throws<T>(Action action, Func<T, bool> predicate, string message)
        where T : Exception
    {
        try { action(); }
        catch (T error) when (predicate(error)) { return; }
        throw new Exception($"Assertion failed: expected {typeof(T).Name} ({message})");
    }

    private static async Task ThrowsAsync<T>(Func<Task> action, Func<T, bool> predicate, string message)
        where T : Exception
    {
        try { await action(); }
        catch (T error) when (predicate(error)) { return; }
        throw new Exception($"Assertion failed: expected {typeof(T).Name} ({message})");
    }

    private sealed class FixedClock : IEventIdClock
    {
        public UtcTime GetUtcTime() => new(1, 2, 3, 4);
    }

    private sealed class FixedRandom(byte value) : IRandomNumber
    {
        public byte Next00To99() => value;
    }
}

internal sealed class FakePeer : IAsyncDisposable
{
    private readonly string _instance;
    private readonly string _sdkId;
    private readonly TcpListener _listener;
    private readonly CancellationTokenSource _stopping = new();
    private readonly Task _acceptLoop;
    private long _eventSequence = 100;

    internal IPEndPoint EndPoint => (IPEndPoint)_listener.LocalEndpoint;

    internal FakePeer(string instance, string sdkId)
    {
        _instance = instance;
        _sdkId = sdkId;
        _listener = new TcpListener(IPAddress.Loopback, 0);
        _listener.Start();
        _acceptLoop = Task.Run(AcceptLoopAsync);
    }

    private async Task AcceptLoopAsync()
    {
        try
        {
            while (!_stopping.IsCancellationRequested)
            {
                TcpClient client = await _listener.AcceptTcpClientAsync(_stopping.Token);
                _ = Task.Run(() => HandleClientAsync(client));
            }
        }
        catch (OperationCanceledException) { }
        catch (ObjectDisposedException) { }
    }

    private async Task HandleClientAsync(TcpClient client)
    {
        using (client)
        {
            var framed = new FramedStream(client.GetStream());
            byte[]? requestBody = await framed.ReadFrameAsync();
            if (requestBody is null)
                return;
            HandshakeRequest request = HandshakeRequest.Parse(requestBody);
            if (request.Id != _sdkId)
            {
                await framed.WriteFrameAsync(new HandshakeResponse.Deny().ToBody());
                return;
            }

            byte[] accept = FrameCodec.Encode(new HandshakeResponse.Accept(_instance).ToBody());
            byte[] ready = FrameCodec.Encode(InboundEnvelope("fake_eow.ready"));
            byte[] prefetched = [.. accept, .. ready];
            await client.GetStream().WriteAsync(prefetched);

            try
            {
                while (await framed.ReadFrameAsync() is byte[] body)
                {
                    _ = SdkEnvelope.Parse(body);
                    await framed.WriteFrameAsync(InboundEnvelope("fake_eow.echo"));
                }
            }
            catch (Exception error) when (error is IOException or SocketException or FramedIoException) { }
        }
    }

    private byte[] InboundEnvelope(string messageType)
    {
        long sequence = Interlocked.Increment(ref _eventSequence);
        return JsonSerializer.SerializeToUtf8Bytes(new
        {
            envelope_schema = "1.0",
            envelope_kind = "event",
            source = _instance,
            target = "eow_sdk",
            event_id = $"7{sequence:00000000000}",
            correlation_id = (string?)null,
            payload = new { sdk_id = _sdkId, type = messageType }
        });
    }

    public async ValueTask DisposeAsync()
    {
        _stopping.Cancel();
        _listener.Stop();
        await _acceptLoop;
        _stopping.Dispose();
    }
}
