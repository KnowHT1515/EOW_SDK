using System.Text.Json;

namespace Eow.Sdk.Transport;

public abstract class EnvelopeData {
    public abstract JsonElement ToJson();
}

sealed class ContextData : EnvelopeData {
    readonly string _content;
    readonly bool _silent;

    internal ContextData(string content, bool silent) {
        _content = content;
        _silent = silent;
    }

    public override JsonElement ToJson() => JsonSerializer.SerializeToElement(new { content = _content, silent = _silent });
}

public abstract class SnapshotData : EnvelopeData { }
public abstract class RegisterData : EnvelopeData { }
public abstract class UnregisterData : EnvelopeData { }
public abstract class UpdateData : EnvelopeData { }
public abstract class DemandData : EnvelopeData { }
public abstract class ResultData : EnvelopeData { }
