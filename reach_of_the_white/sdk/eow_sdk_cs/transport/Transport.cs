using System.Net;
using System.Text;
using System.Text.Json.Nodes;
using System.Threading.Channels;

namespace EowSdk.Transport;

/// <summary>Process-wide multi-instance EOW transport facade.</summary>
public static class Transport
{
    private const int MaxSdkIdBytes = 128;
    private static readonly object InitializationGate = new();
    private static TransportContext? _context;

    public static string? SdkId => GetContext()?.SdkId;

    /// <summary>Initializes the transport and returns its unbounded inbound bucket.</summary>
    public static ChannelReader<SdkEnvelope> Initialize(string sdkId)
    {
        Channel<SdkEnvelope> channel = Channel.CreateUnbounded<SdkEnvelope>(
            new UnboundedChannelOptions { SingleWriter = false, SingleReader = false });
        Initialize(sdkId, channel.Writer, InstanceRegistry.Bundled());
        return channel.Reader;
    }

    public static void Initialize(string sdkId, ChannelWriter<SdkEnvelope> inbound) =>
        Initialize(sdkId, inbound, InstanceRegistry.Bundled());

    /// <summary>Initializes with an explicit registry, useful for hosts and tests.</summary>
    public static void Initialize(
        string sdkId,
        ChannelWriter<SdkEnvelope> inbound,
        InstanceRegistry registry)
    {
        if (!IsValidSdkId(sdkId))
            throw new TransportInitException(InitErrorKind.InvalidSdkId,
                "sdk_id must be 1..=128 UTF-8 bytes without surrounding whitespace or control characters");
        ArgumentNullException.ThrowIfNull(inbound);
        ArgumentNullException.ThrowIfNull(registry);

        lock (InitializationGate)
        {
            if (_context is not null)
                throw new TransportInitException(InitErrorKind.AlreadyInitialized,
                    "EOW SDK transport is already initialized");
            _context = new TransportContext(sdkId, registry, inbound);
        }
    }

    public static Task ConnectAsync(string instance, CancellationToken cancellationToken = default) =>
        ConnectAsync(instance, TransportLimits.DefaultHandshakeTimeout, cancellationToken);

    public static async Task ConnectAsync(
        string instance,
        TimeSpan timeout,
        CancellationToken cancellationToken = default)
    {
        TransportContext context = GetContext() ?? throw NotInitializedConnect();
        EndpointConfig endpoint;
        try { endpoint = context.Registry.Resolve(instance); }
        catch (ConfigException error)
        {
            throw new ConnectException(ConnectErrorKind.Config,
                "could not resolve EOW instance", error)
            { Instance = instance };
        }
        await context.Runtime.ConnectAsync(context.SdkId, context.Registry, instance,
            endpoint.Address, timeout, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Connects a registered instance through an explicit IPv4 endpoint.</summary>
    public static Task ConnectAtAsync(
        string instance,
        IPEndPoint endpoint,
        CancellationToken cancellationToken = default) =>
        ConnectAtAsync(instance, endpoint, TransportLimits.DefaultHandshakeTimeout, cancellationToken);

    public static async Task ConnectAtAsync(
        string instance,
        IPEndPoint endpoint,
        TimeSpan timeout,
        CancellationToken cancellationToken = default)
    {
        TransportContext context = GetContext() ?? throw NotInitializedConnect();
        EndpointConfig resolved;
        try { resolved = context.Registry.ResolveWithOverride(instance, endpoint); }
        catch (ConfigException error)
        {
            throw new ConnectException(ConnectErrorKind.Config,
                "could not resolve EOW instance", error)
            { Instance = instance };
        }
        await context.Runtime.ConnectAsync(context.SdkId, context.Registry, instance,
            resolved.Address, timeout, cancellationToken).ConfigureAwait(false);
    }

    public static async Task DisconnectAsync(string instance)
    {
        TransportContext context = GetContext() ??
            throw new DisconnectException(DisconnectErrorKind.NotInitialized,
                "EOW SDK transport is not initialized");
        await context.Runtime.DisconnectAsync(instance).ConfigureAwait(false);
    }

    /// <summary>Validates and synchronously queues one serialized SDK Envelope.</summary>
    public static SendResult Send(string sdkEnvelopeJson)
    {
        TransportContext? context = GetContext();
        if (context is null)
            return SendResult.Rejected(Array.Empty<string>());

        ArgumentNullException.ThrowIfNull(sdkEnvelopeJson);
        byte[] body;
        try { body = TransportLimits.StrictUtf8.GetBytes(sdkEnvelopeJson); }
        catch (EncoderFallbackException) { return SendResult.Rejected("<malformed_json>"); }

        SdkEnvelope envelope;
        try { envelope = SdkEnvelope.Parse(body, context.Registry); }
        catch (EnvelopeException error)
        {
            return error.Kind switch
            {
                EnvelopeErrorKind.Invalid => SendResult.Rejected(
                    error.Violations.Select(item => item.ToWireValue()).ToArray()),
                EnvelopeErrorKind.Oversize => SendResult.Rejected("<oversize>"),
                EnvelopeErrorKind.Json => SendResult.Rejected("<malformed_json>"),
                EnvelopeErrorKind.Config => SendResult.Rejected("<registry>"),
                _ => SendResult.Rejected(Array.Empty<string>())
            };
        }

        IReadOnlyList<string> violations = envelope.OutboundViolations(context.SdkId, context.Registry);
        if (violations.Count != 0)
            return SendResult.Rejected(violations);

        OutboundClass outboundClass = ClassifyOutbound(envelope);
        return context.Runtime.TryEnqueue(envelope.Target, body, outboundClass)
            ? SendResult.Accepted()
            : SendResult.Rejected(Array.Empty<string>());
    }

    private static OutboundClass ClassifyOutbound(SdkEnvelope envelope)
    {
        JsonObject payload = envelope.Payload;
        if (envelope.Kind == EnvelopeKind.Request &&
            payload["type"]?.GetValue<string>() == "startup")
            return OutboundClass.Startup;
        if (envelope.Kind == EnvelopeKind.Event &&
            payload["type"]?.GetValue<string>() == "state" &&
            payload["kind"]?.GetValue<string>() == "snapshot" &&
            payload["revision"] is JsonValue revisionValue &&
            revisionValue.TryGetValue(out ulong revision))
            return OutboundClass.Snapshot(revision);
        return OutboundClass.Reliable;
    }

    internal static TransportContext? GetContext() => Volatile.Read(ref _context);

    internal static bool IsValidSdkId(string? sdkId)
    {
        if (string.IsNullOrEmpty(sdkId) || sdkId.Trim() != sdkId || sdkId.Any(char.IsControl))
            return false;
        try { return TransportLimits.StrictUtf8.GetByteCount(sdkId) <= MaxSdkIdBytes; }
        catch (EncoderFallbackException) { return false; }
    }

    private static ConnectException NotInitializedConnect() =>
        new(ConnectErrorKind.NotInitialized, "EOW SDK transport is not initialized");
}
