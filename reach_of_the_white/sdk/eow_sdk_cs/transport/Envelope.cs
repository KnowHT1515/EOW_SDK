using System.Text.Json;
using System.Text.Json.Nodes;

namespace EowSdk.Transport;

public enum EnvelopeKind { Request, Response, Event }

public static class EnvelopeKindExtensions
{
    public static string ToWireValue(this EnvelopeKind kind) => kind switch
    {
        EnvelopeKind.Request => "request",
        EnvelopeKind.Response => "response",
        EnvelopeKind.Event => "event",
        _ => throw new ArgumentOutOfRangeException(nameof(kind))
    };

    public static bool TryParseWireValue(string? value, out EnvelopeKind kind)
    {
        kind = value switch
        {
            "request" => EnvelopeKind.Request,
            "response" => EnvelopeKind.Response,
            "event" => EnvelopeKind.Event,
            _ => default
        };
        return value is "request" or "response" or "event";
    }
}

public enum EnvelopeViolation
{
    EnvelopeSchema,
    EnvelopeKind,
    SourceTarget,
    EventId,
    CorrelationId,
    Payload
}

public static class EnvelopeViolationExtensions
{
    public static string ToWireValue(this EnvelopeViolation violation) => violation switch
    {
        EnvelopeViolation.EnvelopeSchema => "envelope_schema",
        EnvelopeViolation.EnvelopeKind => "envelope_kind",
        EnvelopeViolation.SourceTarget => "source_target",
        EnvelopeViolation.EventId => "event_id",
        EnvelopeViolation.CorrelationId => "correlation_id",
        EnvelopeViolation.Payload => "payload",
        _ => throw new ArgumentOutOfRangeException(nameof(violation))
    };
}

/// <summary>A statically validated SDK Envelope. Unknown fields are preserved.</summary>
public sealed class SdkEnvelope
{
    private readonly JsonObject _object;

    private SdkEnvelope(JsonObject value) => _object = value;

    public EnvelopeKind Kind
    {
        get
        {
            EnvelopeKindExtensions.TryParseWireValue(_object["envelope_kind"]!.GetValue<string>(), out var kind);
            return kind;
        }
    }

    public string Source => _object["source"]!.GetValue<string>();
    public string Target => _object["target"]!.GetValue<string>();
    public string EventId => _object["event_id"]!.GetValue<string>();
    public string? CorrelationId => _object["correlation_id"]?.GetValue<string>();
    private JsonObject PayloadObject => (JsonObject)_object["payload"]!;
    public JsonObject Payload => JsonNodeClone.Create(PayloadObject);
    public string? PayloadSdkId => PayloadObject["sdk_id"] is JsonValue value && value.TryGetValue(out string? id) ? id : null;

    /// <summary>Returns a deep copy, preventing callers from invalidating this envelope.</summary>
    public JsonObject ToJsonObject() => JsonNodeClone.Create(_object);

    public byte[] ToUtf8Bytes() => JsonSerializer.SerializeToUtf8Bytes(_object);
    public string ToJson() => _object.ToJsonString();

    public static SdkEnvelope Parse(ReadOnlySpan<byte> bytes) => Parse(bytes, InstanceRegistry.Bundled());

    public static SdkEnvelope Parse(ReadOnlySpan<byte> bytes, InstanceRegistry registry)
    {
        if (bytes.Length > TransportLimits.MaxEnvelopeBytes)
            throw Oversize(bytes.Length);

        JsonNode? node;
        try { node = JsonNode.Parse(bytes); }
        catch (JsonException error)
        {
            throw new EnvelopeException(EnvelopeErrorKind.Json, "invalid SDK Envelope JSON", inner: error);
        }
        return FromJsonNode(node, registry);
    }

    public static SdkEnvelope FromJsonNode(JsonNode? node) => FromJsonNode(node, InstanceRegistry.Bundled());

    public static SdkEnvelope FromJsonNode(JsonNode? node, InstanceRegistry registry)
    {
        ArgumentNullException.ThrowIfNull(registry);
        int serializedLength;
        try { serializedLength = JsonSerializer.SerializeToUtf8Bytes(node).Length; }
        catch (Exception error) when (error is JsonException or InvalidOperationException)
        {
            throw new EnvelopeException(EnvelopeErrorKind.Json, "invalid SDK Envelope JSON", inner: error);
        }
        if (serializedLength > TransportLimits.MaxEnvelopeBytes)
            throw Oversize(serializedLength);

        if (node is not JsonObject jsonObject)
            throw Invalid(Enum.GetValues<EnvelopeViolation>());

        JsonObject owned = JsonNodeClone.Create(jsonObject);
        List<EnvelopeViolation> violations = ValidateObject(owned, registry);
        if (violations.Count != 0)
            throw Invalid(violations);
        return new SdkEnvelope(owned);
    }

    public static SdkEnvelope FromFields(
        EnvelopeKind kind,
        string source,
        string target,
        EventId eventId,
        EventId? correlationId,
        JsonNode? timestampUtc,
        JsonObject payload,
        InstanceRegistry? registry = null)
    {
        var value = new JsonObject
        {
            ["envelope_schema"] = TransportLimits.SdkEnvelopeSchema,
            ["envelope_kind"] = kind.ToWireValue(),
            ["source"] = source,
            ["target"] = target,
            ["event_id"] = eventId.Value,
            ["correlation_id"] = correlationId?.Value,
            ["ts_utc"] = timestampUtc is null ? null : JsonNodeClone.Create(timestampUtc),
            ["payload"] = JsonNodeClone.Create(payload)
        };
        return FromJsonNode(value, registry ?? InstanceRegistry.Bundled());
    }

    public IReadOnlyList<string> OutboundViolations(string sdkId, InstanceRegistry registry)
    {
        var violations = new List<string>();
        if (Source != TransportLimits.ReservedSdkEndpoint || !registry.IsRegisteredInstance(Target))
            violations.Add(EnvelopeViolation.SourceTarget.ToWireValue());
        if (PayloadSdkId != sdkId)
            violations.Add(EnvelopeViolation.Payload.ToWireValue());
        return violations;
    }

    public IReadOnlyList<string> InboundViolations(
        string sdkId,
        string connectedInstance,
        InstanceRegistry registry)
    {
        var violations = new List<string>();
        if (Source != connectedInstance || Target != TransportLimits.ReservedSdkEndpoint ||
            !registry.IsRegisteredInstance(connectedInstance))
            violations.Add(EnvelopeViolation.SourceTarget.ToWireValue());
        if (PayloadSdkId != sdkId)
            violations.Add(EnvelopeViolation.Payload.ToWireValue());
        return violations;
    }

    private static List<EnvelopeViolation> ValidateObject(JsonObject value, InstanceRegistry registry)
    {
        var violations = new List<EnvelopeViolation>(6);

        if (!TryString(value["envelope_schema"], out string? schema) || schema != TransportLimits.SdkEnvelopeSchema)
            violations.Add(EnvelopeViolation.EnvelopeSchema);

        EnvelopeKind kind = default;
        bool kindValid = TryString(value["envelope_kind"], out string? kindText) &&
                         EnvelopeKindExtensions.TryParseWireValue(kindText, out kind);
        if (!kindValid)
            violations.Add(EnvelopeViolation.EnvelopeKind);

        bool sourceValid = TryString(value["source"], out string? source);
        bool targetValid = TryString(value["target"], out string? target);
        if (!sourceValid || !targetValid || source == target ||
            (sourceValid && !registry.IsEnvelopeEndpoint(source!)) ||
            (targetValid && !registry.IsEnvelopeEndpoint(target!)))
            violations.Add(EnvelopeViolation.SourceTarget);

        if (!TryString(value["event_id"], out string? eventId) || !EowSdk.Transport.EventId.IsValid(eventId))
            violations.Add(EnvelopeViolation.EventId);

        bool hasCorrelation = value.TryGetPropertyValue("correlation_id", out JsonNode? correlationNode);
        bool correlationValid;
        if (kindValid && kind == EnvelopeKind.Response)
            correlationValid = hasCorrelation && TryString(correlationNode, out string? correlation) &&
                               EowSdk.Transport.EventId.IsValid(correlation);
        else if (kindValid)
            correlationValid = hasCorrelation && correlationNode is null;
        else
            correlationValid = hasCorrelation && (correlationNode is null ||
                (TryString(correlationNode, out string? correlation) && EowSdk.Transport.EventId.IsValid(correlation)));
        if (!correlationValid)
            violations.Add(EnvelopeViolation.CorrelationId);

        if (value["payload"] is not JsonObject payload || payload.Count == 0)
            violations.Add(EnvelopeViolation.Payload);

        return violations;
    }

    private static bool TryString(JsonNode? node, out string? value)
    {
        if (node is JsonValue jsonValue && jsonValue.TryGetValue(out string? text))
        {
            value = text;
            return true;
        }
        value = null;
        return false;
    }

    private static EnvelopeException Oversize(int actual) =>
        new(EnvelopeErrorKind.Oversize,
            $"SDK Envelope is {actual} bytes; maximum is {TransportLimits.MaxEnvelopeBytes}")
        { ActualBytes = actual, MaximumBytes = TransportLimits.MaxEnvelopeBytes };

    private static EnvelopeException Invalid(IEnumerable<EnvelopeViolation> violations)
    {
        EnvelopeViolation[] array = violations.ToArray();
        return new EnvelopeException(EnvelopeErrorKind.Invalid,
            $"SDK Envelope failed static validation: {string.Join(", ", array)}", array);
    }
}
