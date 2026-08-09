namespace EowSdk.Transport;

public enum ConfigErrorKind
{
    Json, Io, InvalidRoot, InvalidEntry, InvalidInstance, MissingRequiredPort,
    InvalidPort, DuplicateInstance, DuplicatePort, ReservedInstance, UnknownInstance
}

public sealed class ConfigException : Exception
{
    public ConfigErrorKind Kind { get; }
    public int? Index { get; init; }
    public string? Instance { get; init; }
    public string? Port { get; init; }

    public ConfigException(ConfigErrorKind kind, string message, Exception? inner = null)
        : base(message, inner) => Kind = kind;
}

public enum EnvelopeErrorKind { Json, Oversize, Invalid, Config }

public sealed class EnvelopeException : Exception
{
    public EnvelopeErrorKind Kind { get; }
    public IReadOnlyList<EnvelopeViolation> Violations { get; }
    public int? ActualBytes { get; init; }
    public int? MaximumBytes { get; init; }

    public EnvelopeException(
        EnvelopeErrorKind kind,
        string message,
        IReadOnlyList<EnvelopeViolation>? violations = null,
        Exception? inner = null) : base(message, inner)
    {
        Kind = kind;
        Violations = violations ?? Array.Empty<EnvelopeViolation>();
    }
}

public enum HandshakeErrorKind { Json, InvalidShape, InvalidSdkId, InvalidInstance }

public sealed class HandshakeException : Exception
{
    public HandshakeErrorKind Kind { get; }

    public HandshakeException(HandshakeErrorKind kind, string message, Exception? inner = null)
        : base(message, inner) => Kind = kind;
}

public enum FramedIoErrorKind { Io, FrameAbort, EmptyBody, BodyOversize, TruncatedFrame }

public sealed class FramedIoException : Exception
{
    public FramedIoErrorKind Kind { get; }
    public FrameAbort? AbortReason { get; init; }
    public int? ActualBytes { get; init; }
    public int? MaximumBytes { get; init; }

    public FramedIoException(FramedIoErrorKind kind, string message, Exception? inner = null)
        : base(message, inner) => Kind = kind;
}

public enum InitErrorKind { InvalidSdkId, AlreadyInitialized, Config }

public sealed class TransportInitException : Exception
{
    public InitErrorKind Kind { get; }

    public TransportInitException(InitErrorKind kind, string message, Exception? inner = null)
        : base(message, inner) => Kind = kind;
}

public enum ConnectErrorKind
{
    NotInitialized, Config, AlreadyConnected, Io, Framed, Handshake,
    HandshakeClosed, HandshakeDenied, HandshakeTimeout, UnsupportedSchema, InstanceMismatch
}

public sealed class ConnectException : Exception
{
    public ConnectErrorKind Kind { get; }
    public string? Instance { get; init; }
    public string? ExpectedInstance { get; init; }
    public string? ActualInstance { get; init; }

    public ConnectException(ConnectErrorKind kind, string message, Exception? inner = null)
        : base(message, inner) => Kind = kind;
}

public enum DisconnectErrorKind { NotInitialized, NotConnected, WorkerFailed }

public sealed class DisconnectException : Exception
{
    public DisconnectErrorKind Kind { get; }
    public string? Instance { get; init; }

    public DisconnectException(DisconnectErrorKind kind, string message, Exception? inner = null)
        : base(message, inner) => Kind = kind;
}

public enum AssemblyErrorKind { NotInitialized, InvalidTarget, EventId, Envelope }

public sealed class AssemblyException : Exception
{
    public AssemblyErrorKind Kind { get; }
    public string? Target { get; init; }

    public AssemblyException(AssemblyErrorKind kind, string message, Exception? inner = null)
        : base(message, inner) => Kind = kind;
}
