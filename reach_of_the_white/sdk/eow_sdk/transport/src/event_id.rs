//! Twelve-digit SDK event IDs and injectable generation sources.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventId(String);

impl EventId {
    pub fn parse(value: impl Into<String>) -> Result<Self, InvalidEventId> {
        let value = value.into();
        if Self::is_valid(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidEventId)
        }
    }

    pub fn is_valid(value: &str) -> bool {
        value.len() == 12 && value.bytes().all(|byte| byte.is_ascii_digit())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidEventId;

impl fmt::Display for InvalidEventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("event ID must contain exactly 12 ASCII decimal digits")
    }
}

impl std::error::Error for InvalidEventId {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UtcTime {
    hour: u8,
    minute: u8,
    second: u8,
    millisecond: u16,
}

impl UtcTime {
    pub fn new(hour: u8, minute: u8, second: u8, millisecond: u16) -> Option<Self> {
        (hour < 24 && minute < 60 && second < 60 && millisecond < 1000).then_some(Self {
            hour,
            minute,
            second,
            millisecond,
        })
    }
}

pub trait EventIdClock {
    fn utc_time(&mut self) -> UtcTime;
}

#[derive(Debug, Default)]
pub struct SystemUtcClock;

impl EventIdClock for SystemUtcClock {
    fn utc_time(&mut self) -> UtcTime {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let seconds_in_day = elapsed.as_secs() % 86_400;
        UtcTime {
            hour: (seconds_in_day / 3_600) as u8,
            minute: ((seconds_in_day % 3_600) / 60) as u8,
            second: (seconds_in_day % 60) as u8,
            millisecond: elapsed.subsec_millis() as u16,
        }
    }
}

/// Implementations must return a value in `0..=99`.
pub trait RandomNumber {
    fn next_00_99(&mut self) -> u8;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomNumberOutOfRange(pub u8);

impl fmt::Display for RandomNumberOutOfRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "event ID random number {} is outside 0..=99", self.0)
    }
}

impl std::error::Error for RandomNumberOutOfRange {}

/// Generates `[7][HHMMSSMMM][RR]` IDs from injectable sources.
#[derive(Debug)]
pub struct EventIdGenerator<C, R> {
    clock: C,
    random: R,
}

impl<C, R> EventIdGenerator<C, R>
where
    C: EventIdClock,
    R: RandomNumber,
{
    pub fn new(clock: C, random: R) -> Self {
        Self { clock, random }
    }

    pub fn generate(&mut self) -> Result<EventId, RandomNumberOutOfRange> {
        let time = self.clock.utc_time();
        let random = self.random.next_00_99();
        if random > 99 {
            return Err(RandomNumberOutOfRange(random));
        }
        let value = format!(
            "7{:02}{:02}{:02}{:03}{:02}",
            time.hour, time.minute, time.second, time.millisecond, random
        );
        Ok(EventId(value))
    }

    pub fn into_sources(self) -> (C, R) {
        (self.clock, self.random)
    }
}

static SYSTEM_RANDOM_SEED: AtomicU64 = AtomicU64::new(1);

/// Small process-local pseudo-random source for the two-digit event ID suffix.
#[derive(Debug)]
pub struct SystemRandomNumber {
    state: u64,
}

impl Default for SystemRandomNumber {
    fn default() -> Self {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let sequence = SYSTEM_RANDOM_SEED.fetch_add(1, Ordering::Relaxed);
        let state = (elapsed.as_nanos() as u64) ^ sequence.rotate_left(17) | 1;
        Self { state }
    }
}

impl RandomNumber for SystemRandomNumber {
    fn next_00_99(&mut self) -> u8 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state = value;
        (value % 100) as u8
    }
}
