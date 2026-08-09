using System.Net;
using System.Text;

namespace EowSdk.Transport;

/// <summary>Protocol limits and fixed wire constants.</summary>
public static class TransportLimits
{
    public const string SdkEnvelopeSchema = "1.0";
    public const string HandshakeSchema = "1.0";
    public static readonly TimeSpan DefaultHandshakeTimeout = TimeSpan.FromSeconds(5);
    public const int MaxEnvelopeBytes = 15 * 1024 * 1024;
    public const int MaxFrameBodyBytes = 20 * 1024 * 1024;
    public const string ReservedSdkEndpoint = "eow_sdk";
    public static ReadOnlySpan<byte> Sentinel => "\\eow/"u8;
    public static IPAddress DefaultAddress => IPAddress.Loopback;

    internal static readonly UTF8Encoding StrictUtf8 = new(false, true);
}
