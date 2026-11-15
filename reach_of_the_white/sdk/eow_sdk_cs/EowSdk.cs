using Eow.Sdk.Transport;

namespace Eow.Sdk;

public sealed class EowSdk {
    readonly SdkTransport _transport;
    readonly SemaphoreSlim _startup = new(1);
    int _snapshotRevision;

    public EowSdk(string sdkId, int port, TimeSpan? timeout = null) {
        SdkId = sdkId;
        _transport = new SdkTransport(
            SdkId,
            port,
            timeout ?? TimeSpan.FromSeconds(30),
            action => ActionReceived?.Invoke(action),
            () => _snapshotRevision = 0
        );
    }

    public string SdkId { get; }

    public event Action<Envelope>? ActionReceived;

    public async Task<Envelope> Startup() {
        await _startup.WaitAsync().ConfigureAwait(false);
        try {
            var connection = await _transport.Connect().ConfigureAwait(false);
            try {
                var envelope = _transport.EnvelopeBuild(EnvelopeType.Startup);
                using var pending = connection.ExpectResponse(envelope);
                await connection.EnvelopeSend(envelope).ConfigureAwait(false);
                var (response, _) = await pending.Wait().ConfigureAwait(false);
                if (response.Payload.Type == "ack" && !response.AckOk) {
                    connection.Close();
                }

                return response;
            }
            catch {
                connection.Close();
                throw;
            }
        }
        finally {
            _startup.Release();
        }
    }

    public async Task<Envelope> Shutdown() {
        var connection = _transport.Connection;
        try {
            var envelope = _transport.EnvelopeBuild(EnvelopeType.Shutdown);
            using var pending = connection.ExpectResponse(envelope);
            await connection.EnvelopeSend(envelope).ConfigureAwait(false);
            var (response, _) = await pending.Wait().ConfigureAwait(false);
            return response;
        }
        finally {
            connection.Close();
        }
    }

    public Task<Envelope> Context(string content, bool silent) => Context(new ContextData(content, silent));

    public async Task<Envelope> Context(ContextData data) {
        var connection = _transport.Connection;
        var envelope = _transport.EnvelopeBuild(EnvelopeType.Context, data);
        using var pending = connection.ExpectResponse(envelope);
        await connection.EnvelopeSend(envelope).ConfigureAwait(false);
        var (response, _) = await pending.Wait().ConfigureAwait(false);
        return response;
    }

    public Task StateSnapshot(SnapshotData data) {
        lock (_transport.ConnectionLock) {
            var revision = _snapshotRevision++;
            var connection = _transport.Connection;
            var envelope = _transport.EnvelopeBuild(EnvelopeType.StateSnapshot, data, revision: revision);
            return connection.EnvelopeSend(envelope);
        }
    }

    public async Task<Envelope> ActionRegister(RegisterData data) {
        var connection = _transport.Connection;
        var envelope = _transport.EnvelopeBuild(EnvelopeType.ActionRegister, data);
        using var pending = connection.ExpectResponse(envelope);
        await connection.EnvelopeSend(envelope).ConfigureAwait(false);
        var (response, _) = await pending.Wait().ConfigureAwait(false);
        return response;
    }

    public async Task<Envelope> ActionUnregister(UnregisterData data) {
        var connection = _transport.Connection;
        var envelope = _transport.EnvelopeBuild(EnvelopeType.ActionUnregister, data);
        using var pending = connection.ExpectResponse(envelope);
        await connection.EnvelopeSend(envelope).ConfigureAwait(false);
        var (response, _) = await pending.Wait().ConfigureAwait(false);
        return response;
    }

    public async Task<Envelope> ActionUpdate(UpdateData data) {
        var connection = _transport.Connection;
        var envelope = _transport.EnvelopeBuild(EnvelopeType.ActionUpdate, data);
        using var pending = connection.ExpectResponse(envelope);
        await connection.EnvelopeSend(envelope).ConfigureAwait(false);
        var (response, _) = await pending.Wait().ConfigureAwait(false);
        return response;
    }

    public async Task<Envelope> ActionClear() {
        var connection = _transport.Connection;
        var envelope = _transport.EnvelopeBuild(EnvelopeType.ActionClear);
        using var pending = connection.ExpectResponse(envelope);
        await connection.EnvelopeSend(envelope).ConfigureAwait(false);
        var (response, _) = await pending.Wait().ConfigureAwait(false);
        return response;
    }

    public async Task<(Envelope Ack, Envelope? Action)> ActionDemand(DemandData data) {
        var connection = _transport.Connection;
        var envelope = _transport.EnvelopeBuild(EnvelopeType.ActionDemand, data);
        using var pending = connection.ExpectResponse(envelope, data.Id);
        await connection.EnvelopeSend(envelope).ConfigureAwait(false);
        return await pending.Wait().ConfigureAwait(false);
    }

    public Task ActionResult(string correlationId, ResultData data) {
        var connection = _transport.Connection;
        var envelope = _transport.EnvelopeBuild(EnvelopeType.ActionResult, data, correlationId);
        return connection.EnvelopeSend(envelope);
    }
}
