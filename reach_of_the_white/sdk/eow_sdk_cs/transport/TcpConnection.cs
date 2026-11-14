using System.Diagnostics;
using System.Net.Sockets;
using System.Text.Json;

namespace Eow.Sdk.Transport;

sealed class TcpConnection {
    readonly TcpClient _client;
    readonly NetworkStream _stream;
    readonly object _connectionLock;
    readonly TimeSpan _timeout;
    readonly Action<Envelope> _onAction;
    readonly Action _onDisconnected;
    readonly SemaphoreSlim _writer = new(1);
    readonly Dictionary<string, PendingResponse> _pending = new();
    PendingResponse? _demand;
    bool _closed;

    internal TcpConnection(
        TcpClient client,
        NetworkStream stream,
        object connectionLock,
        TimeSpan timeout,
        Action<Envelope> onAction,
        Action onDisconnected
    ) {
        _client = client;
        _stream = stream;
        _connectionLock = connectionLock;
        _timeout = timeout;
        _onAction = onAction;
        _onDisconnected = onDisconnected;
    }

    internal void StartReceiving() => _ = Receive();

    internal PendingResponse ExpectResponse(Envelope envelope, string? demandId = null) {
        lock (_connectionLock) {
            if (_closed) {
                throw new ObjectDisposedException(typeof(TcpConnection).FullName);
            }

            if (demandId is not null && _demand is not null) {
                throw new InvalidOperationException("An action demand is already pending.");
            }

            var pending = new PendingResponse(this, envelope.EventId, _timeout, demandId);
            _pending.Add(envelope.EventId, pending);
            if (demandId is not null) {
                _demand = pending;
            }

            return pending;
        }
    }

    internal async Task EnvelopeSend(Envelope envelope) {
        var body = JsonSerializer.SerializeToUtf8Bytes(envelope);
        using var deadline = new CancellationTokenSource(_timeout);
        await _writer.WaitAsync(deadline.Token).ConfigureAwait(false);
        try {
            await TcpFraming.Write(_stream, body, deadline.Token).ConfigureAwait(false);
        }
        catch (Exception error) {
            Close(error);
            throw;
        }
        finally {
            _writer.Release();
        }
    }

    internal void Close(Exception? error = null) {
        lock (_connectionLock) {
            if (_closed) {
                return;
            }

            _closed = true;
            _client.Dispose();
            var failure = error ?? new IOException("The EOW connection was closed.");
            foreach (var pending in _pending.Values) {
                pending.Completion.TrySetException(failure);
            }

            _pending.Clear();
            _demand = null;
            _onDisconnected();
        }
    }

    async Task Receive() {
        try {
            while (true) {
                var body = await TcpFraming.Read(_stream).ConfigureAwait(false);
                var envelope = JsonSerializer.Deserialize<Envelope>(body)
                    ?? throw new InvalidDataException("An EOW envelope must be a JSON object.");
                Dispatch(envelope);
            }
        }
        catch (Exception error) {
            Close(error);
        }
    }

    void Dispatch(Envelope envelope) {
        lock (_connectionLock) {
            if (envelope.EnvelopeKind == "response") {
                if (envelope.CorrelationId is { } id && _pending.TryGetValue(id, out var pending)) {
                    if (pending == _demand && envelope.AckOk) {
                        pending.Ack = envelope;
                    }
                    else {
                        pending.Completion.TrySetResult((envelope, null));
                    }
                }

                return;
            }

            if (envelope.EnvelopeKind != "request" || envelope.Payload.Type != "action") {
                return;
            }

            if (_demand is { Ack: { } ack } demand && envelope.Payload.GetDataString("demand_id") == demand.DemandId) {
                demand.Ack = null;
                demand.Completion.TrySetResult((ack, envelope));
                return;
            }
        }

        _ = Task.Run(() => _onAction(envelope));
    }

    void StopWaiting(string eventId, PendingResponse pending) {
        lock (_connectionLock) {
            _pending.Remove(eventId);
            if (_demand == pending) {
                _demand = null;
            }
        }
    }

    internal sealed class PendingResponse : IDisposable {
        readonly TcpConnection _connection;
        readonly string _eventId;
        readonly TimeSpan _timeout;
        readonly Stopwatch _elapsed = Stopwatch.StartNew();

        internal PendingResponse(TcpConnection connection, string eventId, TimeSpan timeout, string? demandId) {
            _connection = connection;
            _eventId = eventId;
            _timeout = timeout;
            DemandId = demandId;
        }

        internal string? DemandId { get; }
        internal Envelope? Ack { get; set; }
        internal TaskCompletionSource<(Envelope Ack, Envelope? Action)> Completion { get; } =
            new(TaskCreationOptions.RunContinuationsAsynchronously);

        internal Task<(Envelope Ack, Envelope? Action)> Wait() {
            var remaining = _timeout - _elapsed.Elapsed;
            return Completion.Task.WaitAsync(remaining > TimeSpan.Zero ? remaining : TimeSpan.Zero);
        }

        public void Dispose() => _connection.StopWaiting(_eventId, this);
    }
}
