using System.Text.Json;
using System.Text.Json.Serialization;

namespace Eow.Sdk.Transport;

public abstract class EnvelopeData {
    public abstract JsonElement ToJson();
}

public sealed class ContextData : EnvelopeData {
    public ContextData(string content, bool silent) {
        Content = content ?? throw new ArgumentNullException(nameof(content));
        Silent = silent;
    }

    [JsonPropertyName("content")]
    public string Content { get; }

    [JsonPropertyName("silent")]
    public bool Silent { get; }

    public override JsonElement ToJson() => JsonSerializer.SerializeToElement(this);
}

public abstract class SnapshotData : EnvelopeData { }

public sealed class RegisterData : EnvelopeData {
    public RegisterData(IReadOnlyList<SdkAction> actions) {
        DataValidation.ValidateList(actions, nameof(actions));
        Actions = actions;
    }

    public IReadOnlyList<SdkAction> Actions { get; }

    public override JsonElement ToJson() => JsonSerializer.SerializeToElement(Actions);
}

public sealed class UnregisterData : EnvelopeData {
    public UnregisterData(IReadOnlyList<string> actionIds) {
        DataValidation.ValidateList(actionIds, nameof(actionIds));
        ActionIds = actionIds;
    }

    public IReadOnlyList<string> ActionIds { get; }

    public override JsonElement ToJson() => JsonSerializer.SerializeToElement(ActionIds);
}

public sealed class UpdateData : EnvelopeData {
    public UpdateData(IReadOnlyList<SdkAction> actions) {
        DataValidation.ValidateList(actions, nameof(actions));
        Actions = actions;
    }

    public IReadOnlyList<SdkAction> Actions { get; }

    public override JsonElement ToJson() => JsonSerializer.SerializeToElement(Actions);
}

public sealed class DemandData : EnvelopeData {
    public DemandData(
        string id,
        string goal,
        IReadOnlyList<string> actions,
        bool permanentContext,
        bool respondAsap,
        string? state = null
    ) {
        Id = id ?? throw new ArgumentNullException(nameof(id));
        Goal = goal ?? throw new ArgumentNullException(nameof(goal));
        DataValidation.ValidateList(actions, nameof(actions));
        Actions = actions;
        PermanentContext = permanentContext;
        RespondAsap = respondAsap;
        State = state;
    }

    [JsonPropertyName("id")]
    public string Id { get; }

    [JsonPropertyName("goal")]
    public string Goal { get; }

    [JsonPropertyName("actions")]
    public IReadOnlyList<string> Actions { get; }

    [JsonPropertyName("permanent_context")]
    public bool PermanentContext { get; }

    [JsonPropertyName("respond_asap")]
    public bool RespondAsap { get; }

    [JsonPropertyName("state"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? State { get; }

    public override JsonElement ToJson() => JsonSerializer.SerializeToElement(this);
}

public sealed class ResultData : EnvelopeData {
    public ResultData(bool ok, string? result = null) {
        Ok = ok;
        Result = result;
    }

    [JsonPropertyName("ok")]
    public bool Ok { get; }

    [JsonPropertyName("result"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Result { get; }

    public override JsonElement ToJson() => JsonSerializer.SerializeToElement(this);
}

public sealed class SdkAction {
    readonly IReadOnlyDictionary<string, Argument>? _arguments;

    public SdkAction(
        string actionId,
        string description,
        IReadOnlyDictionary<string, Argument>? arguments = null
    ) {
        ActionId = actionId ?? throw new ArgumentNullException(nameof(actionId));
        Description = description ?? throw new ArgumentNullException(nameof(description));

        if (arguments is not null) {
            foreach (var (argumentId, argument) in arguments) {
                if (argumentId is null) {
                    throw new ArgumentException("Argument names must not be null.", nameof(arguments));
                }

                if (argument is null) {
                    throw new ArgumentException($"Argument '{argumentId}' must have a definition.", nameof(arguments));
                }
            }
        }

        _arguments = arguments;
    }

    [JsonPropertyName("action_id")]
    public string ActionId { get; }

    [JsonPropertyName("description")]
    public string Description { get; }

    [JsonPropertyName("arguments"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public IReadOnlyDictionary<string, Argument>? Arguments => _arguments is { Count: > 0 } ? _arguments : null;
}

public sealed class Argument {
    private Argument(
        string type,
        string description,
        bool required,
        IReadOnlyList<string>? @enum = null,
        int? minimum = null,
        int? maximum = null
    ) {
        Type = type ?? throw new ArgumentNullException(nameof(type));
        Description = description ?? throw new ArgumentNullException(nameof(description));

        if (minimum is int min && maximum is int max && min > max) {
            throw new ArgumentException($"Maximum ({max}) must be greater than or equal to minimum ({min}).", nameof(maximum));
        }

        Enum = @enum;
        Minimum = minimum;
        Maximum = maximum;
        Required = required;
    }

    [JsonPropertyName("type")]
    public string Type { get; }

    [JsonPropertyName("enum"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public IReadOnlyList<string>? Enum { get; }

    [JsonPropertyName("minimum"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public int? Minimum { get; }

    [JsonPropertyName("maximum"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public int? Maximum { get; }

    [JsonPropertyName("description")]
    public string Description { get; }

    [JsonPropertyName("required")]
    public bool Required { get; }

    public static Argument String(string description, bool required, IReadOnlyList<string> @enum) {
        ArgumentNullException.ThrowIfNull(@enum, nameof(@enum));
        var values = @enum.ToArray();
        if (values.Length == 0) {
            throw new ArgumentException("Enum must contain at least one value.", nameof(@enum));
        }

        DataValidation.ValidateList(values, nameof(@enum));
        return new("string", description, required, @enum: Array.AsReadOnly(values));
    }

    public static Argument Integer(string description, bool required, int? minimum = null, int? maximum = null) => new("integer", description, required, minimum: minimum, maximum: maximum);

    public static Argument Boolean(string description, bool required) => new("boolean", description, required);
}

static class DataValidation {
    internal static IReadOnlyList<T> ValidateList<T>(IReadOnlyList<T> items, string parameterName) where T : class {
        ArgumentNullException.ThrowIfNull(items, parameterName);

        for (var index = 0; index < items.Count; index++) {
            if (items[index] is null) {
                throw new ArgumentException($"Item at index {index} must not be null.", parameterName);
            }
        }

        return items;
    }
}
