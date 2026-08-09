using System.Text.Json;
using System.Text.Json.Nodes;

namespace EowSdk.Transport;

public sealed record HandshakeRequest(string Schema, string Id)
{
    public HandshakeRequest(string id) : this(TransportLimits.HandshakeSchema, id) { }

    public byte[] ToBody() => JsonSerializer.SerializeToUtf8Bytes(new
    {
        handshake = new { schema = Schema, id = Id }
    });

    public static HandshakeRequest Parse(ReadOnlySpan<byte> body)
    {
        JsonObject handshake = ParseHandshakeObject(body);
        if (!TryNonEmptyString(handshake["schema"], out string? schema) ||
            !TryString(handshake["id"], out string? id))
            throw InvalidShape();
        if (!Transport.IsValidSdkId(id!))
            throw new HandshakeException(HandshakeErrorKind.InvalidSdkId,
                "invalid sdk_id in handshake");
        return new HandshakeRequest(schema!, id!);
    }

    internal static JsonObject ParseHandshakeObject(ReadOnlySpan<byte> body)
    {
        try
        {
            JsonNode? top = JsonNode.Parse(body);
            return top is JsonObject root && root["handshake"] is JsonObject handshake
                ? handshake
                : throw InvalidShape();
        }
        catch (HandshakeException) { throw; }
        catch (JsonException error)
        {
            throw new HandshakeException(HandshakeErrorKind.Json, "invalid handshake JSON", error);
        }
    }

    internal static bool TryString(JsonNode? node, out string? value)
    {
        if (node is JsonValue jsonValue && jsonValue.TryGetValue(out string? text))
        {
            value = text;
            return true;
        }
        value = null;
        return false;
    }

    internal static bool TryNonEmptyString(JsonNode? node, out string? value) =>
        TryString(node, out value) && value!.Length != 0;

    internal static HandshakeException InvalidShape() =>
        new(HandshakeErrorKind.InvalidShape, "invalid handshake message shape");
}

public abstract record HandshakeResponse(string Schema)
{
    public sealed record Accept(string Instance, string AcceptedSchema = TransportLimits.HandshakeSchema)
        : HandshakeResponse(AcceptedSchema);
    public sealed record Deny(string DeniedSchema = TransportLimits.HandshakeSchema)
        : HandshakeResponse(DeniedSchema);

    public string? AcceptedInstance => this is Accept accept ? accept.Instance : null;

    public byte[] ToBody() => this switch
    {
        Accept accept => JsonSerializer.SerializeToUtf8Bytes(new
        {
            handshake = new { schema = accept.Schema, result = "accept", instance = accept.Instance }
        }),
        Deny deny => JsonSerializer.SerializeToUtf8Bytes(new
        {
            handshake = new { schema = deny.Schema, result = "deny" }
        }),
        _ => throw new InvalidOperationException("unknown handshake response")
    };

    public static HandshakeResponse Parse(ReadOnlySpan<byte> body)
    {
        JsonObject handshake = HandshakeRequest.ParseHandshakeObject(body);
        if (!HandshakeRequest.TryNonEmptyString(handshake["schema"], out string? schema) ||
            !HandshakeRequest.TryString(handshake["result"], out string? result))
            throw HandshakeRequest.InvalidShape();

        if (result == "accept")
        {
            if (!HandshakeRequest.TryString(handshake["instance"], out string? instance))
                throw HandshakeRequest.InvalidShape();
            if (!InstanceRegistry.IsValidInstanceName(instance) ||
                instance == TransportLimits.ReservedSdkEndpoint)
                throw new HandshakeException(HandshakeErrorKind.InvalidInstance,
                    "invalid instance in handshake");
            return new Accept(instance!, schema!);
        }

        if (result == "deny" && !handshake.ContainsKey("instance"))
            return new Deny(schema!);
        throw HandshakeRequest.InvalidShape();
    }
}
