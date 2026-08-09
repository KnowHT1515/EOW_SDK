using System.Globalization;
using System.Text.Json.Nodes;

namespace EowSdk.Transport;

public sealed record SdkAction(
    string ActionId,
    string Description,
    JsonObject? Arguments = null,
    IReadOnlyList<string>? Required = null)
{
    internal JsonObject ToJsonObject()
    {
        var value = new JsonObject
        {
            ["action_id"] = ActionId,
            ["description"] = Description
        };
        if (Arguments is not null)
            value["arguments"] = Arguments.DeepClone();
        if (Required is { Count: > 0 })
            value["required"] = new JsonArray(Required.Select(item => JsonValue.Create(item)).ToArray());
        return value;
    }
}

public enum ActionPriority { Low, Medium, High, Critical }

public static class ActionPriorityExtensions
{
    public static string ToWireValue(this ActionPriority priority) => priority switch
    {
        ActionPriority.Low => "low",
        ActionPriority.Medium => "medium",
        ActionPriority.High => "high",
        ActionPriority.Critical => "critical",
        _ => throw new ArgumentOutOfRangeException(nameof(priority))
    };
}

public sealed record ActionDemand(
    string Goal,
    bool PermanentContext,
    ActionPriority Priority,
    IReadOnlyList<string> Actions,
    string? State = null);

public sealed record ActionResult(bool Ok, string? Id = null, string? Result = null);

/// <summary>Builds specification-compliant SDK Envelopes from the process-wide identity.</summary>
public sealed class Assembly
{
    private readonly TransportContext _context;

    private Assembly(TransportContext context) => _context = context;

    public static Assembly Global() => new(Transport.GetContext() ??
        throw new AssemblyException(AssemblyErrorKind.NotInitialized,
            "EOW SDK transport is not initialized"));

    public string Startup(string target) => Request(target, PayloadBase("startup"));
    public string Shutdown(string target) => Request(target, PayloadBase("shutdown"));

    public string Context(string target, string content, bool silent)
    {
        JsonObject payload = PayloadBase("context");
        payload["data"] = new JsonObject { ["content"] = content, ["silent"] = silent };
        return Request(target, payload);
    }

    public string ActionRegister(string target, IEnumerable<SdkAction> actions) =>
        ActionList(target, "register", actions);

    public string ActionUnregister(string target, IEnumerable<string> actionIds)
    {
        JsonObject payload = ActionBase("unregister");
        payload["data"] = new JsonArray(actionIds.Select(item => JsonValue.Create(item)).ToArray());
        return Request(target, payload);
    }

    public string ActionUpdate(string target, IEnumerable<SdkAction> actions) =>
        ActionList(target, "update", actions);

    public string ActionClear(string target) => Request(target, ActionBase("clear"));

    public string ActionDemand(string target, ActionDemand demand)
    {
        var data = new JsonObject
        {
            ["goal"] = demand.Goal,
            ["permanent_context"] = demand.PermanentContext,
            ["priority"] = demand.Priority.ToWireValue(),
            ["actions"] = new JsonArray(demand.Actions.Select(item => JsonValue.Create(item)).ToArray())
        };
        if (demand.State is not null)
            data["state"] = demand.State;

        JsonObject payload = ActionBase("demand");
        payload["data"] = data;
        return Request(target, payload);
    }

    public string ActionResult(string target, EventId correlationId, ActionResult result)
    {
        var data = new JsonObject { ["ok"] = result.Ok };
        if (result.Id is not null)
            data["id"] = result.Id;
        if (result.Result is not null)
            data["result"] = result.Result;

        JsonObject payload = ActionBase("result");
        payload["data"] = data;
        return Assemble(EnvelopeKind.Response, target, correlationId, payload);
    }

    private string ActionList(string target, string kind, IEnumerable<SdkAction> actions)
    {
        JsonObject payload = ActionBase(kind);
        payload["data"] = new JsonArray(actions.Select(action => action.ToJsonObject()).ToArray<JsonNode?>());
        return Request(target, payload);
    }

    private string Request(string target, JsonObject payload) =>
        Assemble(EnvelopeKind.Request, target, null, payload);

    private string Assemble(
        EnvelopeKind kind,
        string target,
        EventId? correlationId,
        JsonObject payload)
    {
        if (!_context.Registry.IsRegisteredInstance(target))
            throw new AssemblyException(AssemblyErrorKind.InvalidTarget,
                $"Assembly target is not a registered instance: '{target}'")
            { Target = target };

        EventId eventId;
        try
        {
            lock (_context.EventIds)
                eventId = _context.EventIds.Generate();
        }
        catch (Exception error)
        {
            throw new AssemblyException(AssemblyErrorKind.EventId,
                "could not generate event_id", error);
        }

        try
        {
            SdkEnvelope envelope = SdkEnvelope.FromFields(
                kind,
                TransportLimits.ReservedSdkEndpoint,
                target,
                eventId,
                correlationId,
                JsonValue.Create(DateTime.UtcNow.ToString("yyyy-MM-dd'T'HH:mm:ss.fff'Z'",
                    CultureInfo.InvariantCulture)),
                payload,
                _context.Registry);
            return envelope.ToJson();
        }
        catch (EnvelopeException error)
        {
            throw new AssemblyException(AssemblyErrorKind.Envelope,
                "could not assemble SDK Envelope", error);
        }
    }

    private JsonObject PayloadBase(string messageType) => new()
    {
        ["sdk_id"] = _context.SdkId,
        ["type"] = messageType
    };

    private JsonObject ActionBase(string kind)
    {
        JsonObject payload = PayloadBase("action");
        payload["kind"] = kind;
        return payload;
    }
}
