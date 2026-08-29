using System.Text.Json.Nodes;

namespace EowSdk.Transport;

internal static class JsonNodeClone
{
    internal static T Create<T>(T node) where T : JsonNode
    {
#if NET8_0_OR_GREATER
        return (T)node.DeepClone();
#else
        return (T)JsonNode.Parse(node.ToJsonString())!;
#endif
    }
}
