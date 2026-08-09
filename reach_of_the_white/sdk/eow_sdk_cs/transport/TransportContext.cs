using System.Threading.Channels;

namespace EowSdk.Transport;

internal sealed class TransportContext
{
    internal string SdkId { get; }
    internal InstanceRegistry Registry { get; }
    internal TransportRuntime Runtime { get; }
    internal EventIdGenerator EventIds { get; } =
        new(new SystemUtcClock(), new SystemRandomNumber());

    internal TransportContext(string sdkId, InstanceRegistry registry, ChannelWriter<SdkEnvelope> inbound)
    {
        SdkId = sdkId;
        Registry = registry;
        Runtime = new TransportRuntime(inbound);
    }
}
