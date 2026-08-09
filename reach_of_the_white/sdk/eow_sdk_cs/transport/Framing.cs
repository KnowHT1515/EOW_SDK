using System.Buffers.Binary;

namespace EowSdk.Transport;

public enum FrameAbort { ZeroLength, Oversize, BadSentinel }
public enum DecodeStatus { Frame, NeedMore, Abort }

public sealed record DecodeResult(
    DecodeStatus Status,
    byte[]? Body = null,
    int Consumed = 0,
    FrameAbort? AbortReason = null);

/// <summary>Content-agnostic TCP frame codec.</summary>
public static class FrameCodec
{
    public static byte[] Encode(ReadOnlySpan<byte> body)
    {
        if (body.IsEmpty)
            throw new FramedIoException(FramedIoErrorKind.EmptyBody,
                "zero-length frame bodies are forbidden");
        if (body.Length > TransportLimits.MaxFrameBodyBytes)
            throw new FramedIoException(FramedIoErrorKind.BodyOversize,
                $"frame body is {body.Length} bytes; maximum is {TransportLimits.MaxFrameBodyBytes}")
            { ActualBytes = body.Length, MaximumBytes = TransportLimits.MaxFrameBodyBytes };

        byte[] framed = new byte[4 + body.Length + TransportLimits.Sentinel.Length];
        BinaryPrimitives.WriteUInt32BigEndian(framed, (uint)body.Length);
        body.CopyTo(framed.AsSpan(4));
        TransportLimits.Sentinel.CopyTo(framed.AsSpan(4 + body.Length));
        return framed;
    }

    public static DecodeResult Decode(ReadOnlySpan<byte> input)
    {
        if (input.Length < 4)
            return new DecodeResult(DecodeStatus.NeedMore);

        uint bodyLength = BinaryPrimitives.ReadUInt32BigEndian(input);
        if (bodyLength == 0)
            return new DecodeResult(DecodeStatus.Abort, AbortReason: FrameAbort.ZeroLength);
        if (bodyLength > TransportLimits.MaxFrameBodyBytes)
            return new DecodeResult(DecodeStatus.Abort, AbortReason: FrameAbort.Oversize);

        int total = checked(4 + (int)bodyLength + TransportLimits.Sentinel.Length);
        if (input.Length < total)
            return new DecodeResult(DecodeStatus.NeedMore);
        if (!input.Slice(4 + (int)bodyLength, TransportLimits.Sentinel.Length)
                .SequenceEqual(TransportLimits.Sentinel))
            return new DecodeResult(DecodeStatus.Abort, AbortReason: FrameAbort.BadSentinel);

        return new DecodeResult(DecodeStatus.Frame,
            input.Slice(4, (int)bodyLength).ToArray(), total);
    }
}

/// <summary>An incremental decoder that permanently remembers a framing abort.</summary>
public sealed class Deframer
{
    private byte[] _buffer = Array.Empty<byte>();
    private int _count;
    private FrameAbort? _aborted;

    public int BufferedLength => _count;
    public bool IsAborted => _aborted.HasValue;

    public void Push(ReadOnlySpan<byte> bytes)
    {
        ThrowIfAborted();
        EnsureCapacity(_count + bytes.Length);
        bytes.CopyTo(_buffer.AsSpan(_count));
        _count += bytes.Length;
    }

    public byte[]? NextFrame()
    {
        ThrowIfAborted();
        DecodeResult result = FrameCodec.Decode(_buffer.AsSpan(0, _count));
        if (result.Status == DecodeStatus.NeedMore)
            return null;
        if (result.Status == DecodeStatus.Abort)
        {
            _aborted = result.AbortReason;
            throw AbortException(_aborted!.Value);
        }

        int remaining = _count - result.Consumed;
        if (remaining > 0)
            _buffer.AsSpan(result.Consumed, remaining).CopyTo(_buffer);
        _count = remaining;
        return result.Body;
    }

    private void EnsureCapacity(int required)
    {
        if (_buffer.Length >= required)
            return;
        int capacity = Math.Max(required, Math.Max(256, _buffer.Length * 2));
        Array.Resize(ref _buffer, capacity);
    }

    private void ThrowIfAborted()
    {
        if (_aborted is FrameAbort abort)
            throw AbortException(abort);
    }

    private static FramedIoException AbortException(FrameAbort abort) =>
        new(FramedIoErrorKind.FrameAbort, $"framing aborted: {abort}") { AbortReason = abort };
}
