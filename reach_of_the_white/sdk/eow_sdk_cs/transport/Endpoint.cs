using System.Net;
using System.Reflection;
using System.Text.Json;

namespace EowSdk.Transport;

public sealed record EndpointConfig(string Instance, IPEndPoint Address);

/// <summary>Validated instance names and TCP ports.</summary>
public sealed class InstanceRegistry
{
    private readonly IReadOnlyDictionary<string, ushort> _instances;

    private InstanceRegistry(Dictionary<string, ushort> instances) => _instances = instances;

    public int Count => _instances.Count;
    public bool IsEmpty => _instances.Count == 0;

    public static InstanceRegistry Bundled()
    {
        using Stream stream = System.Reflection.Assembly.GetExecutingAssembly()
            .GetManifestResourceStream("EowSdk.Transport.instance.json")
            ?? throw new ConfigException(ConfigErrorKind.Io, "bundled instance registry is missing");
        using var reader = new StreamReader(stream);
        return FromJson(reader.ReadToEnd());
    }

    public static InstanceRegistry Load(string path)
    {
        try { return FromJson(File.ReadAllText(path)); }
        catch (ConfigException) { throw; }
        catch (IOException error)
        {
            throw new ConfigException(ConfigErrorKind.Io, "could not read instance registry", error);
        }
    }

    public static InstanceRegistry FromJson(string json)
    {
        JsonDocument document;
        try { document = JsonDocument.Parse(json); }
        catch (JsonException error)
        {
            throw new ConfigException(ConfigErrorKind.Json, "invalid instance registry JSON", error);
        }

        using (document)
        {
            if (document.RootElement.ValueKind != JsonValueKind.Array)
                throw new ConfigException(ConfigErrorKind.InvalidRoot, "instance registry must be a JSON array");

            var instances = new Dictionary<string, ushort>(StringComparer.Ordinal);
            var ports = new Dictionary<ushort, string>();
            int index = 0;
            foreach (JsonElement entry in document.RootElement.EnumerateArray())
            {
                if (entry.ValueKind != JsonValueKind.Object ||
                    !entry.TryGetProperty("instance", out JsonElement instanceNode) ||
                    instanceNode.ValueKind != JsonValueKind.String)
                    throw new ConfigException(ConfigErrorKind.InvalidEntry,
                        $"invalid instance registry entry at index {index}")
                    { Index = index };

                string instance = instanceNode.GetString()!;
                if (!IsValidInstanceName(instance))
                    throw new ConfigException(ConfigErrorKind.InvalidInstance,
                        $"invalid instance '{instance}' in registry entry at index {index}")
                    { Index = index, Instance = instance };
                if (instance == TransportLimits.ReservedSdkEndpoint)
                    throw new ConfigException(ConfigErrorKind.ReservedInstance,
                        $"reserved SDK endpoint cannot be registered: '{instance}'")
                    { Instance = instance };

                if (!entry.TryGetProperty("port", out JsonElement portNode))
                    throw new ConfigException(ConfigErrorKind.MissingRequiredPort,
                        $"instance '{instance}' has no port in registry entry at index {index}")
                    { Index = index, Instance = instance };

                string portText = portNode.ValueKind == JsonValueKind.String
                    ? portNode.GetString()!
                    : portNode.GetRawText();
                if (portNode.ValueKind != JsonValueKind.String || portText.Length == 0 ||
                    portText.Any(character => character is < '0' or > '9') ||
                    !ushort.TryParse(portText, out ushort port) || port == 0)
                    throw new ConfigException(ConfigErrorKind.InvalidPort,
                        $"invalid port '{portText}' for instance '{instance}'")
                    { Instance = instance, Port = portText };

                if (instances.ContainsKey(instance))
                    throw new ConfigException(ConfigErrorKind.DuplicateInstance,
                        $"duplicate instance in registry: '{instance}'")
                    { Instance = instance };
                if (ports.TryGetValue(port, out string? first))
                    throw new ConfigException(ConfigErrorKind.DuplicatePort,
                        $"duplicate port {port} for instances '{first}' and '{instance}'")
                    { Instance = instance, Port = portText };

                instances.Add(instance, port);
                ports.Add(port, instance);
                index++;
            }

            return new InstanceRegistry(instances);
        }
    }

    public bool IsRegisteredInstance(string instance) => _instances.ContainsKey(instance);
    public bool IsEnvelopeEndpoint(string endpoint) =>
        endpoint == TransportLimits.ReservedSdkEndpoint || IsRegisteredInstance(endpoint);
    public ushort? Port(string instance) => _instances.TryGetValue(instance, out ushort port) ? port : null;

    public EndpointConfig Resolve(string instance) => ResolveAt(instance, TransportLimits.DefaultAddress);

    public EndpointConfig ResolveAt(string instance, IPAddress address)
    {
        if (!_instances.TryGetValue(instance, out ushort port))
            throw new ConfigException(ConfigErrorKind.UnknownInstance,
                $"unknown EOW instance: '{instance}'")
            { Instance = instance };
        if (address.AddressFamily != System.Net.Sockets.AddressFamily.InterNetwork)
            throw new ArgumentException("EOW transport endpoints must use IPv4", nameof(address));
        return new EndpointConfig(instance, new IPEndPoint(address, port));
    }

    public EndpointConfig ResolveWithOverride(string instance, IPEndPoint? endpoint)
    {
        if (endpoint is null)
            return Resolve(instance);
        if (!IsRegisteredInstance(instance))
            throw new ConfigException(ConfigErrorKind.UnknownInstance,
                $"unknown EOW instance: '{instance}'")
            { Instance = instance };
        if (endpoint.AddressFamily != System.Net.Sockets.AddressFamily.InterNetwork)
            throw new ArgumentException("EOW transport endpoints must use IPv4", nameof(endpoint));
        return new EndpointConfig(instance, endpoint);
    }

    public static bool IsValidInstanceName(string? instance)
    {
        if (instance is null || instance.Length is < 1 or > 64 || instance[0] is < 'a' or > 'z')
            return false;
        return instance.Skip(1).All(character =>
            character is >= 'a' and <= 'z' or >= '0' and <= '9' or '_' or '-');
    }
}
