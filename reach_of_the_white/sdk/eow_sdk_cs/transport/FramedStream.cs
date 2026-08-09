namespace EowSdk.Transport;

/// <summary>Reads and writes complete frames over a stream while retaining prefetched bytes.</summary>
public sealed class FramedStream
{
    private const int DefaultReadChunk = 8 * 1024;
    private readonly Stream _stream;
    private readonly Deframer _deframer = new();
    private readonly byte[] _chunk;
    private readonly SemaphoreSlim _writeLock = new(1, 1);

    public FramedStream(Stream stream, int readChunkSize = DefaultReadChunk)
    {
        _stream = stream ?? throw new ArgumentNullException(nameof(stream));
        _chunk = new byte[Math.Max(1, readChunkSize)];
    }

    public Stream InnerStream => _stream;

    public async ValueTask<byte[]?> ReadFrameAsync(CancellationToken cancellationToken = default)
    {
        while (true)
        {
            byte[]? body = _deframer.NextFrame();
            if (body is not null)
                return body;

            int read;
            try { read = await _stream.ReadAsync(_chunk, cancellationToken).ConfigureAwait(false); }
            catch (OperationCanceledException) { throw; }
            catch (IOException error)
            {
                throw new FramedIoException(FramedIoErrorKind.Io, "framed I/O read failed", error);
            }

            if (read == 0)
            {
                if (_deframer.BufferedLength == 0)
                    return null;
                throw new FramedIoException(FramedIoErrorKind.TruncatedFrame,
                    "connection ended with a partial frame");
            }
            _deframer.Push(_chunk.AsSpan(0, read));
        }
    }

    public async ValueTask WriteFrameAsync(
        ReadOnlyMemory<byte> body,
        CancellationToken cancellationToken = default)
    {
        byte[] framed = FrameCodec.Encode(body.Span);
        await _writeLock.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await _stream.WriteAsync(framed, cancellationToken).ConfigureAwait(false);
        }
        catch (OperationCanceledException) { throw; }
        catch (IOException error)
        {
            throw new FramedIoException(FramedIoErrorKind.Io, "framed I/O write failed", error);
        }
        finally { _writeLock.Release(); }
    }

    public async ValueTask FlushAsync(CancellationToken cancellationToken = default)
    {
        try { await _stream.FlushAsync(cancellationToken).ConfigureAwait(false); }
        catch (OperationCanceledException) { throw; }
        catch (IOException error)
        {
            throw new FramedIoException(FramedIoErrorKind.Io, "framed I/O flush failed", error);
        }
    }
}
