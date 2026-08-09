using System.Globalization;
using System.Security.Cryptography;

namespace EowSdk.Transport;

/// <summary>A validated 12-digit SDK event identifier.</summary>
public readonly record struct EventId
{
    public string Value { get; }

    private EventId(string value) => Value = value;

    public static EventId Parse(string value) =>
        IsValid(value) ? new EventId(value) : throw new FormatException(
            "event ID must contain exactly 12 ASCII decimal digits");

    public static bool TryParse(string? value, out EventId eventId)
    {
        if (value is not null && IsValid(value))
        {
            eventId = new EventId(value);
            return true;
        }

        eventId = default;
        return false;
    }

    public static bool IsValid(string? value) =>
        value is { Length: 12 } && value.All(character => character is >= '0' and <= '9');

    public override string ToString() => Value ?? string.Empty;
}

public readonly record struct UtcTime
{
    public byte Hour { get; }
    public byte Minute { get; }
    public byte Second { get; }
    public ushort Millisecond { get; }

    public UtcTime(byte hour, byte minute, byte second, ushort millisecond)
    {
        if (hour >= 24 || minute >= 60 || second >= 60 || millisecond >= 1000)
            throw new ArgumentOutOfRangeException(nameof(hour), "UTC time components are invalid");
        Hour = hour;
        Minute = minute;
        Second = second;
        Millisecond = millisecond;
    }
}

public interface IEventIdClock { UtcTime GetUtcTime(); }
public interface IRandomNumber { byte Next00To99(); }

public sealed class SystemUtcClock : IEventIdClock
{
    public UtcTime GetUtcTime()
    {
        DateTime now = DateTime.UtcNow;
        return new UtcTime((byte)now.Hour, (byte)now.Minute, (byte)now.Second, (ushort)now.Millisecond);
    }
}

public sealed class SystemRandomNumber : IRandomNumber
{
    public byte Next00To99() => (byte)RandomNumberGenerator.GetInt32(100);
}

/// <summary>Generates IDs in the documented [7][HHMMSSMMM][RR] layout.</summary>
public sealed class EventIdGenerator
{
    private readonly IEventIdClock _clock;
    private readonly IRandomNumber _random;

    public EventIdGenerator(IEventIdClock clock, IRandomNumber random)
    {
        _clock = clock ?? throw new ArgumentNullException(nameof(clock));
        _random = random ?? throw new ArgumentNullException(nameof(random));
    }

    public EventId Generate()
    {
        UtcTime time = _clock.GetUtcTime();
        byte random = _random.Next00To99();
        if (random > 99)
            throw new InvalidOperationException($"event ID random number {random} is outside 0..=99");

        string value = string.Create(CultureInfo.InvariantCulture,
            $"7{time.Hour:00}{time.Minute:00}{time.Second:00}{time.Millisecond:000}{random:00}");
        return EventId.Parse(value);
    }
}
