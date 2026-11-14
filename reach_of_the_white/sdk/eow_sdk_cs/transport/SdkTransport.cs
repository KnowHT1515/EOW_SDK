using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text.Json;

namespace Eow.Sdk.Transport;

sealed class SdkTransport {
    const string Schema = "1.0";
    readonly string _sdkId;
    readonly int _port;
    readonly TimeSpan _timeout;
    readonly Action<Envelope> _onAction;
    readonly Action _onDisconnected;
    TcpConnection? _connection;

    internal SdkTransport(
        string sdkId,
        int port,
        TimeSpan timeout,
        Action<Envelope> onAction,
        Action onDisconnected
    ) {
        _sdkId = sdkId;
        _port = port;
        _timeout = timeout;
        _onAction = onAction;
        _onDisconnected = onDisconnected;
    }

    internal object ConnectionLock { get; } = new();

    internal TcpConnection Connection {
        get {
            lock (ConnectionLock) {
                return _connection ?? throw new InvalidOperationException("The EOW SDK is disconnected.");
            }
        }
    }

    internal Envelope EnvelopeBuild(
        EnvelopeType type,
        EnvelopeData? data = null,
        string? correlationId = null,
        int? revision = null
    ) {
        (string kind, string payloadType, string? payloadKind) = type switch {
            EnvelopeType.Startup => ("request", "startup", null),
            EnvelopeType.Shutdown => ("request", "shutdown", null),
            EnvelopeType.Context => ("request", "context", null),
            EnvelopeType.StateSnapshot => ("event", "state", "snapshot"),
            EnvelopeType.ActionRegister => ("request", "action", "register"),
            EnvelopeType.ActionUnregister => ("request", "action", "unregister"),
            EnvelopeType.ActionUpdate => ("request", "action", "update"),
            EnvelopeType.ActionClear => ("request", "action", "clear"),
            EnvelopeType.ActionDemand => ("request", "action", "demand"),
            EnvelopeType.ActionResult => ("response", "action", "result"),
            _ => throw new ArgumentOutOfRangeException(nameof(type))
        };
        var now = DateTime.UtcNow;
        var timestamp = now.ToString("HHmmssfff", CultureInfo.InvariantCulture);
        var suffix = Random.Shared.Next(100).ToString("D2", CultureInfo.InvariantCulture);

        return new Envelope(
            Schema,
            kind,
            "eow_sdk",
            "eow",
            $"7{timestamp}{suffix}",
            kind == "response" ? correlationId : null,
            now,
            new EnvelopePayload(_sdkId, payloadType, payloadKind, revision, data?.ToJson())
        );
    }

    internal async Task<TcpConnection> Connect() {
        lock (ConnectionLock) {
            if (_connection is not null) {
                throw new InvalidOperationException("The EOW SDK is already connected.");
            }
        }

        using var deadline = new CancellationTokenSource(_timeout);
        var cancellationToken = deadline.Token;
        var client = new TcpClient(AddressFamily.InterNetwork);
        try {
            await client.ConnectAsync(IPAddress.Loopback, _port, cancellationToken).ConfigureAwait(false);
            var stream = client.GetStream();
            var handshake = JsonSerializer.SerializeToUtf8Bytes(new { handshake = new { schema = Schema, sdk_id = _sdkId } });
            await TcpFraming.Write(stream, handshake, cancellationToken).ConfigureAwait(false);
            using var reply = JsonDocument.Parse(await TcpFraming.Read(stream, cancellationToken).ConfigureAwait(false));
            var result = reply.RootElement.GetProperty("handshake");
            if (result.GetProperty("result").GetString() != "accept") {
                var reason = result.GetProperty("reason").GetString();
                throw new IOException($"EOW denied the handshake: {reason}.");
            }

            var connection = new TcpConnection(client, stream, ConnectionLock, _timeout, _onAction, Disconnected);
            lock (ConnectionLock) {
                _connection = connection;
            }

            connection.StartReceiving();
            return connection;
        }
        catch {
            client.Dispose();
            throw;
        }
    }

    void Disconnected() {
        _connection = null;
        _onDisconnected();
    }
}
