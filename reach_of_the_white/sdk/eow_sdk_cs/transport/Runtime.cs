using System.Net;
using System.Net.Sockets;
using System.Threading.Channels;

namespace EowSdk.Transport;

/// <summary>The result of synchronously submitting an outbound Envelope.</summary>
public sealed record SendResult(bool Ok, IReadOnlyList<string> Violation)
{
    internal static SendResult Accepted() => new(true, Array.Empty<string>());
    internal static SendResult Rejected(params string[] violations) => new(false, violations);
    internal static SendResult Rejected(IReadOnlyList<string> violations) => new(false, violations);
}

internal sealed class TransportRuntime
{
    private readonly object _gate = new();
    private readonly Dictionary<string, ConnectionHandle> _connections =
        new(StringComparer.Ordinal);
    private readonly ChannelWriter<SdkEnvelope> _inbound;

    internal TransportRuntime(ChannelWriter<SdkEnvelope> inbound) => _inbound = inbound;

    internal async Task ConnectAsync(
        string sdkId,
        InstanceRegistry registry,
        string instance,
        IPEndPoint endpoint,
        TimeSpan handshakeTimeout,
        CancellationToken cancellationToken)
    {
        ConnectionHandle? stale = null;
        lock (_gate)
        {
            if (_connections.TryGetValue(instance, out ConnectionHandle? existing))
            {
                if (existing.IsAlive)
                    throw AlreadyConnected(instance);
                _connections.Remove(instance);
                stale = existing;
            }
        }
        if (stale is not null)
            await stale.CloseAsync().ConfigureAwait(false);

        ConnectionHandle connection = await ConnectionHandle.EstablishAsync(
            sdkId, registry, instance, endpoint, handshakeTimeout, _inbound, cancellationToken)
            .ConfigureAwait(false);

        bool conflict;
        lock (_gate)
        {
            conflict = _connections.TryGetValue(instance, out ConnectionHandle? existing) && existing.IsAlive;
            if (!conflict)
                _connections[instance] = connection;
        }
        if (conflict)
        {
            await connection.CloseAsync().ConfigureAwait(false);
            throw AlreadyConnected(instance);
        }
    }

    internal async Task DisconnectAsync(string instance)
    {
        ConnectionHandle connection;
        lock (_gate)
        {
            if (!_connections.Remove(instance, out connection!))
                throw new DisconnectException(DisconnectErrorKind.NotConnected,
                    $"EOW instance is not connected: '{instance}'")
                { Instance = instance };
        }
        await connection.CloseAsync().ConfigureAwait(false);
    }

    internal bool TryEnqueue(string target, byte[] body, OutboundClass outboundClass)
    {
        lock (_gate)
            return _connections.TryGetValue(target, out ConnectionHandle? connection) &&
                   connection.TryEnqueue(body, outboundClass);
    }

    private static ConnectException AlreadyConnected(string instance) =>
        new(ConnectErrorKind.AlreadyConnected,
            $"EOW instance is already connected: '{instance}'")
        { Instance = instance };
}

internal enum OutboundKind { Reliable, Startup, Snapshot }

internal readonly record struct OutboundClass(OutboundKind Kind, ulong Revision)
{
    internal static OutboundClass Reliable => new(OutboundKind.Reliable, 0);
    internal static OutboundClass Startup => new(OutboundKind.Startup, 0);
    internal static OutboundClass Snapshot(ulong revision) => new(OutboundKind.Snapshot, revision);
}

internal sealed class OutboundQueue
{
    private sealed record QueuedBody(ulong Sequence, byte[] Body);
    private sealed record QueuedSnapshot(ulong Sequence, ulong Session, ulong Revision, byte[] Body);

    private readonly object _gate = new();
    private readonly Queue<QueuedBody> _reliable = new();
    private readonly SemaphoreSlim _ready = new(0);
    private QueuedSnapshot? _snapshot;
    private ulong _nextSequence;
    private ulong _session;
    private bool _closed;

    internal bool TryEnqueue(byte[] body, OutboundClass outboundClass)
    {
        bool inserted;
        bool signal;
        lock (_gate)
        {
            if (_closed)
                return false;

            bool wasEmpty = _reliable.Count == 0 && _snapshot is null;
            ulong sequence = _nextSequence;
            _nextSequence = checked(_nextSequence + 1);
            if (outboundClass.Kind != OutboundKind.Snapshot)
            {
                if (outboundClass.Kind == OutboundKind.Startup)
                    _session = checked(_session + 1);
                _reliable.Enqueue(new QueuedBody(sequence, body));
                inserted = true;
            }
            else if (_snapshot?.Revision == 0 &&
                     (outboundClass.Revision != 0 || _snapshot.Session == _session))
            {
                inserted = false;
            }
            else
            {
                _snapshot = new QueuedSnapshot(sequence, _session, outboundClass.Revision, body);
                inserted = true;
            }
            signal = inserted && wasEmpty;
        }

        if (signal)
            _ready.Release();
        return true;
    }

    internal async ValueTask<byte[]?> DequeueAsync()
    {
        while (true)
        {
            lock (_gate)
            {
                if (_reliable.Count != 0 &&
                    (_snapshot is null || _reliable.Peek().Sequence < _snapshot.Sequence))
                    return _reliable.Dequeue().Body;
                if (_snapshot is not null)
                {
                    byte[] body = _snapshot.Body;
                    _snapshot = null;
                    return body;
                }
                if (_closed)
                    return null;
            }

            await _ready.WaitAsync().ConfigureAwait(false);
        }
    }

    internal void Close()
    {
        bool signal;
        lock (_gate)
        {
            signal = !_closed;
            _closed = true;
        }
        if (signal)
            _ready.Release();
    }
}

internal sealed class ConnectionHandle
{
    private readonly object _stateGate = new();
    private readonly string _sdkId;
    private readonly InstanceRegistry _registry;
    private readonly string _instance;
    private readonly TcpClient _client;
    private readonly FramedStream _framed;
    private readonly ChannelWriter<SdkEnvelope> _inbound;
    private readonly OutboundQueue _outbound;
    private readonly Task _writerTask;
    private readonly Task _readerTask;
    private bool _alive = true;
    private bool _closing;

    internal bool IsAlive
    {
        get { lock (_stateGate) return _alive; }
    }

    private ConnectionHandle(
        string sdkId,
        InstanceRegistry registry,
        string instance,
        TcpClient client,
        FramedStream framed,
        ChannelWriter<SdkEnvelope> inbound)
    {
        _sdkId = sdkId;
        _registry = registry;
        _instance = instance;
        _client = client;
        _framed = framed;
        _inbound = inbound;
        _outbound = new OutboundQueue();
        _writerTask = Task.Run(WriterLoopAsync);
        _readerTask = Task.Run(ReaderLoopAsync);
    }

    internal static async Task<ConnectionHandle> EstablishAsync(
        string sdkId,
        InstanceRegistry registry,
        string instance,
        IPEndPoint endpoint,
        TimeSpan timeout,
        ChannelWriter<SdkEnvelope> inbound,
        CancellationToken cancellationToken)
    {
        if (timeout <= TimeSpan.Zero)
            throw Timeout();

        var client = new TcpClient(AddressFamily.InterNetwork);
        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        deadline.CancelAfter(timeout);
        try
        {
            await client.ConnectAsync(endpoint.Address, endpoint.Port, deadline.Token)
                .ConfigureAwait(false);
            client.NoDelay = true;
            var framed = new FramedStream(client.GetStream());
            await framed.WriteFrameAsync(new HandshakeRequest(sdkId).ToBody(), deadline.Token)
                .ConfigureAwait(false);
            await framed.FlushAsync(deadline.Token).ConfigureAwait(false);

            byte[] body = await framed.ReadFrameAsync(deadline.Token).ConfigureAwait(false)
                ?? throw new ConnectException(ConnectErrorKind.HandshakeClosed,
                    "EOW closed before replying to the handshake");
            HandshakeResponse response;
            try { response = HandshakeResponse.Parse(body); }
            catch (HandshakeException error)
            {
                throw new ConnectException(ConnectErrorKind.Handshake,
                    "handshake response failed", error);
            }

            if (response.Schema != TransportLimits.HandshakeSchema)
                throw new ConnectException(ConnectErrorKind.UnsupportedSchema,
                    $"unsupported handshake schema: '{response.Schema}'");
            if (response is HandshakeResponse.Deny)
                throw new ConnectException(ConnectErrorKind.HandshakeDenied,
                    "EOW denied the SDK handshake");
            string actual = ((HandshakeResponse.Accept)response).Instance;
            if (actual != instance)
                throw new ConnectException(ConnectErrorKind.InstanceMismatch,
                    $"handshake instance mismatch: expected '{instance}', got '{actual}'")
                { ExpectedInstance = instance, ActualInstance = actual };

            return new ConnectionHandle(sdkId, registry, instance, client, framed, inbound);
        }
        catch (ConnectException)
        {
            client.Dispose();
            throw;
        }
        catch (OperationCanceledException error) when (!cancellationToken.IsCancellationRequested)
        {
            client.Dispose();
            throw Timeout(error);
        }
        catch (OperationCanceledException)
        {
            client.Dispose();
            throw;
        }
        catch (FramedIoException error)
        {
            client.Dispose();
            throw new ConnectException(ConnectErrorKind.Framed,
                "handshake frame I/O failed", error);
        }
        catch (Exception error) when (error is SocketException or IOException)
        {
            client.Dispose();
            throw new ConnectException(ConnectErrorKind.Io,
                "TCP connection setup failed", error);
        }
    }

    internal bool TryEnqueue(byte[] body, OutboundClass outboundClass)
    {
        lock (_stateGate)
            return _alive && _outbound.TryEnqueue(body, outboundClass);
    }

    internal async Task CloseAsync()
    {
        lock (_stateGate)
        {
            if (!_closing)
            {
                _closing = true;
                _alive = false;
                _outbound.Close();
            }
        }

        await _writerTask.ConfigureAwait(false);
        ShutdownSocket();
        await _readerTask.ConfigureAwait(false);
        _client.Dispose();
    }

    private async Task WriterLoopAsync()
    {
        try
        {
            while (await _outbound.DequeueAsync().ConfigureAwait(false) is byte[] body)
                await _framed.WriteFrameAsync(body).ConfigureAwait(false);
            await _framed.FlushAsync().ConfigureAwait(false);
        }
        catch (Exception error) when (error is FramedIoException or IOException or SocketException or ObjectDisposedException)
        {
            // Delivery failures terminate only this connection. Send() reports queue acceptance.
        }
        finally
        {
            MarkDead();
            ShutdownSocket();
        }
    }

    private async Task ReaderLoopAsync()
    {
        try
        {
            while (await _framed.ReadFrameAsync().ConfigureAwait(false) is byte[] body)
            {
                SdkEnvelope envelope = SdkEnvelope.Parse(body, _registry);
                if (envelope.InboundViolations(_sdkId, _instance, _registry).Count != 0)
                    break;
                if (!_inbound.TryWrite(envelope))
                    break;
            }
        }
        catch (Exception error) when (error is FramedIoException or EnvelopeException or
                                      IOException or SocketException or ObjectDisposedException or
                                      ChannelClosedException)
        {
            // Invalid input or a closed peer makes the stream untrustworthy.
        }
        finally
        {
            MarkDead();
            _outbound.Close();
        }
    }

    private void MarkDead()
    {
        lock (_stateGate) _alive = false;
    }

    private void ShutdownSocket()
    {
        try { _client.Client.Shutdown(SocketShutdown.Both); }
        catch (Exception error) when (error is SocketException or ObjectDisposedException) { }
    }

    private static ConnectException Timeout(Exception? inner = null) =>
        new(ConnectErrorKind.HandshakeTimeout,
            "TCP connection and handshake exceeded the absolute deadline", inner);
}
