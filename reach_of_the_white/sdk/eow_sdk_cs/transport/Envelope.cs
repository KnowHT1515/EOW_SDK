using System.Text.Json;
using System.Text.Json.Serialization;

namespace Eow.Sdk.Transport;

public enum EnvelopeType {
    Startup,
    Shutdown,
    Context,
    StateSnapshot,
    ActionRegister,
    ActionUnregister,
    ActionUpdate,
    ActionClear,
    ActionDemand,
    ActionResult
}

public sealed record Envelope(
    [property: JsonPropertyName("envelope_schema")] string EnvelopeSchema,
    [property: JsonPropertyName("envelope_kind")] string EnvelopeKind,
    [property: JsonPropertyName("source")] string Source,
    [property: JsonPropertyName("target")] string Target,
    [property: JsonPropertyName("event_id")] string EventId,
    [property: JsonPropertyName("correlation_id")] string? CorrelationId,
    [property: JsonPropertyName("ts_utc")] DateTime TsUtc,
    [property: JsonPropertyName("payload")] EnvelopePayload Payload
) {
    internal bool AckOk => Payload.Data!.Value.GetProperty("ok").GetBoolean();
}

public sealed record EnvelopePayload(
    [property: JsonPropertyName("sdk_id")] string SdkId,
    [property: JsonPropertyName("type")] string Type,
    [property: JsonPropertyName("kind"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] string? Kind = null,
    [property: JsonPropertyName("revision"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] int? Revision = null,
    [property: JsonPropertyName("data"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] JsonElement? Data = null
) {
    internal string? GetDataString(string name) {
        if (
            Data is { ValueKind: JsonValueKind.Object } data &&
            data.TryGetProperty(name, out var value) &&
            value.ValueKind == JsonValueKind.String
        ) {
            return value.GetString();
        }

        return null;
    }
}
