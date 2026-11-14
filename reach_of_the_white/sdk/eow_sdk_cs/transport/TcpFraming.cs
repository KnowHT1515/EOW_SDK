using System.Buffers.Binary;

namespace Eow.Sdk.Transport;

static class TcpFraming {
    const uint MaxBodyLength = 20 * 1024 * 1024;
    static readonly byte[] Sentinel = { 0x5C, 0x65, 0x6F, 0x77, 0x2F };

    internal static async Task<byte[]> Read(Stream stream, CancellationToken cancellationToken = default) {
        var prefix = new byte[sizeof(uint)];
        await ReadExactly(stream, prefix, cancellationToken).ConfigureAwait(false);
        var length = BinaryPrimitives.ReadUInt32BigEndian(prefix);
        ValidateLength(length);

        var body = new byte[length];
        await ReadExactly(stream, body, cancellationToken).ConfigureAwait(false);
        var tail = new byte[Sentinel.Length];
        await ReadExactly(stream, tail, cancellationToken).ConfigureAwait(false);
        if (!tail.AsSpan().SequenceEqual(Sentinel)) {
            throw new InvalidDataException("Invalid EOW frame sentinel.");
        }

        return body;
    }

    internal static async Task Write(Stream stream, byte[] body, CancellationToken cancellationToken) {
        var length = (uint)body.Length;
        ValidateLength(length);

        var prefix = new byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32BigEndian(prefix, length);
        await stream.WriteAsync(prefix, cancellationToken).ConfigureAwait(false);
        await stream.WriteAsync(body, cancellationToken).ConfigureAwait(false);
        await stream.WriteAsync(Sentinel, cancellationToken).ConfigureAwait(false);
    }

    static async Task ReadExactly(Stream stream, Memory<byte> buffer, CancellationToken cancellationToken) {
        while (!buffer.IsEmpty) {
            var read = await stream.ReadAsync(buffer, cancellationToken).ConfigureAwait(false);
            if (read == 0) {
                throw new EndOfStreamException("The EOW stream ended before the frame was complete.");
            }

            buffer = buffer[read..];
        }
    }

    static void ValidateLength(uint length) {
        if (length is 0 or > MaxBodyLength) {
            throw new InvalidDataException($"Invalid EOW frame body length: {length}.");
        }
    }
}
