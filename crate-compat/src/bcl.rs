//! C# BCL (Base Class Library) proto types.
//!
//! Support for protobuf-net's bcl.proto types: DateTime, TimeSpan, Guid, Decimal.
//! See: <https://github.com/protobuf-net/protobuf-net/blob/main/src/Tools/bcl.proto>

use chrono::{DateTime as ChronoDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

const TICKS_PER_SECOND: i64 = 10_000_000;

// ============================================================================
// TimeSpanScale
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[proto_rs::proto_message]
pub enum TimeSpanScale {
    #[default]
    Days = 0,
    Hours = 1,
    Minutes = 2,
    Seconds = 3,
    Milliseconds = 4,
    Ticks = 5,
    MinMax = 15,
}

// ============================================================================
// DateTimeKind
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[proto_rs::proto_message]
pub enum DateTimeKind {
    #[default]
    Unspecified = 0,
    Utc = 1,
    Local = 2,
}

// ============================================================================
// DateTime
// ============================================================================

#[derive(Clone, PartialEq, Eq, Debug)]
#[proto_rs::proto_message]
pub struct DateTime(
    #[proto(tag = "1")] i64,           // value
    #[proto(tag = "2")] TimeSpanScale, // scale
    #[proto(tag = "3")] DateTimeKind,  // kind
);

impl DateTime {
    pub fn from_chrono(datetime: ChronoDateTime<Utc>) -> Self {
        let timestamp = datetime.timestamp();
        if timestamp % 86400 == 0 {
            DateTime(timestamp / 86400, TimeSpanScale::Days, DateTimeKind::Utc)
        } else if timestamp % 3600 == 0 {
            DateTime(timestamp / 3600, TimeSpanScale::Hours, DateTimeKind::Utc)
        } else if timestamp % 60 == 0 {
            DateTime(timestamp / 60, TimeSpanScale::Minutes, DateTimeKind::Utc)
        } else {
            DateTime(timestamp, TimeSpanScale::Seconds, DateTimeKind::Utc)
        }
    }

    pub fn to_chrono(&self) -> ChronoDateTime<Utc> {
        let seconds = match self.1 {
            TimeSpanScale::Days => self.0 * 86400,
            TimeSpanScale::Hours => self.0 * 3600,
            TimeSpanScale::Minutes => self.0 * 60,
            TimeSpanScale::Seconds => self.0,
            TimeSpanScale::Milliseconds => self.0 / 1000,
            TimeSpanScale::Ticks => self.0 / TICKS_PER_SECOND,
            TimeSpanScale::MinMax => 0,
        };
        ChronoDateTime::<Utc>::from_timestamp(seconds, 0).unwrap_or_default()
    }
}

impl Default for DateTime {
    fn default() -> Self {
        DateTime(0, TimeSpanScale::Days, DateTimeKind::Utc)
    }
}

impl From<ChronoDateTime<Utc>> for DateTime {
    fn from(datetime: ChronoDateTime<Utc>) -> Self {
        DateTime::from_chrono(datetime)
    }
}

impl From<DateTime> for ChronoDateTime<Utc> {
    fn from(datetime: DateTime) -> Self {
        datetime.to_chrono()
    }
}

impl Serialize for DateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let chrono_dt = self.to_chrono();
        serializer.serialize_str(&chrono_dt.to_rfc3339())
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        use std::fmt;

        struct DateTimeVisitor;

        impl<'de2> Visitor<'de2> for DateTimeVisitor {
            type Value = DateTime;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a datetime string or empty element")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
                if s.is_empty() {
                    return Ok(DateTime::default());
                }

                if let Ok(dt) = ChronoDateTime::parse_from_rfc3339(s) {
                    return Ok(DateTime::from_chrono(dt.with_timezone(&Utc)));
                }

                if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
                    return Ok(DateTime::from_chrono(naive.and_utc()));
                }

                if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
                    return Ok(DateTime::from_chrono(naive.and_utc()));
                }

                Err(de::Error::custom(format!("failed to parse datetime: {s}")))
            }

            fn visit_string<E: de::Error>(self, s: String) -> Result<Self::Value, E> {
                self.visit_str(&s)
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(DateTime::default())
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(DateTime::default())
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de2>,
            {
                while map.next_entry::<de::IgnoredAny, de::IgnoredAny>()?.is_some() {}
                Ok(DateTime::default())
            }
        }

        deserializer.deserialize_any(DateTimeVisitor)
    }
}

// ============================================================================
// TimeSpan
// ============================================================================

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[proto_rs::proto_message]
pub struct TimeSpan(#[proto(tag = "1")] i64, #[proto(tag = "2")] TimeSpanScale);

impl TimeSpan {
    pub fn from_duration(duration: Duration) -> Self {
        match duration {
            d if d == Duration::MAX => TimeSpan(i64::MAX, TimeSpanScale::MinMax),
            d if d == Duration::ZERO => TimeSpan(0, TimeSpanScale::MinMax),
            d if d.as_secs() % 86400 == 0 => {
                TimeSpan((d.as_secs() / 86400) as i64, TimeSpanScale::Days)
            }
            d if d.as_secs() % 3600 == 0 => {
                TimeSpan((d.as_secs() / 3600) as i64, TimeSpanScale::Hours)
            }
            d if d.as_secs() % 60 == 0 => {
                TimeSpan((d.as_secs() / 60) as i64, TimeSpanScale::Minutes)
            }
            d if d.subsec_millis() == 0 => TimeSpan(d.as_secs() as i64, TimeSpanScale::Seconds),
            d if d.subsec_nanos() == 0 => {
                TimeSpan(d.as_millis() as i64, TimeSpanScale::Milliseconds)
            }
            d if d.subsec_nanos() > 0 => {
                TimeSpan((d.as_nanos() / 100) as i64, TimeSpanScale::Ticks)
            }
            d => TimeSpan(d.as_secs() as i64, TimeSpanScale::Seconds),
        }
    }

    pub fn to_duration(&self) -> Duration {
        match self.1 {
            TimeSpanScale::Days => Duration::from_secs(self.0 as u64 * 86400),
            TimeSpanScale::Hours => Duration::from_secs(self.0 as u64 * 3600),
            TimeSpanScale::Minutes => Duration::from_secs(self.0 as u64 * 60),
            TimeSpanScale::Seconds => Duration::from_secs(self.0 as u64),
            TimeSpanScale::Milliseconds => Duration::from_millis(self.0 as u64),
            TimeSpanScale::Ticks => Duration::from_nanos(self.0 as u64 * 100),
            TimeSpanScale::MinMax => {
                if self.0 == i64::MAX {
                    Duration::MAX
                } else {
                    Duration::ZERO
                }
            }
        }
    }
}

impl Default for TimeSpan {
    fn default() -> Self {
        TimeSpan(0, TimeSpanScale::Seconds)
    }
}

impl From<Duration> for TimeSpan {
    fn from(duration: Duration) -> Self {
        TimeSpan::from_duration(duration)
    }
}

impl From<TimeSpan> for Duration {
    fn from(timespan: TimeSpan) -> Self {
        timespan.to_duration()
    }
}

impl Serialize for TimeSpan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let duration = self.to_duration();
        serializer.serialize_u64(duration.as_secs())
    }
}

impl<'de> Deserialize<'de> for TimeSpan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        let duration = Duration::from_secs(secs);
        Ok(TimeSpan::from_duration(duration))
    }
}

// ============================================================================
// Guid
// ============================================================================

#[derive(Clone, PartialEq, Eq, Debug)]
#[proto_rs::proto_message]
pub struct Guid(#[proto(tag = "1")] u64, #[proto(tag = "2")] u64);

impl Guid {
    pub fn from_uuid(uuid: &Uuid) -> Self {
        let bytes: &[u8; 16] = uuid.as_bytes();
        let lo = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let hi = u64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);
        Guid(lo, hi)
    }

    pub fn to_uuid(&self) -> Uuid {
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&self.0.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.1.to_le_bytes());
        Uuid::from_bytes(bytes)
    }
}

impl Default for Guid {
    fn default() -> Self {
        Uuid::nil().into()
    }
}

impl From<Guid> for Uuid {
    fn from(guid: Guid) -> Self {
        guid.to_uuid()
    }
}

impl From<Uuid> for Guid {
    fn from(uuid: Uuid) -> Self {
        Guid::from_uuid(&uuid)
    }
}

impl Serialize for Guid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let uuid: Uuid = self.to_uuid();
        serializer.serialize_str(&uuid.to_string())
    }
}

impl<'de> Deserialize<'de> for Guid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(Guid::from_uuid(&uuid))
    }
}

// ============================================================================
// Decimal
// ============================================================================

/// protobuf-net bcl.proto Decimal representation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[proto_rs::proto_message]
pub struct Decimal {
    #[proto(tag = 1)]
    pub lo: u64,
    #[proto(tag = 2)]
    pub hi: u32,
    #[proto(tag = 3)]
    pub sign_scale: u32,
}

impl Decimal {
    /// Reconstruct a 96-bit integer + sign/scale from the protobuf fields.
    pub fn to_f64(&self) -> f64 {
        let sign = if self.sign_scale & 0x0001 != 0 {
            -1.0
        } else {
            1.0
        };
        let scale = ((self.sign_scale >> 1) & 0xFF) as i32;
        let raw = (self.hi as u128) << 64 | self.lo as u128;
        sign * (raw as f64) / 10f64.powi(scale)
    }
}

impl Serialize for Decimal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_f64(self.to_f64())
    }
}

impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = f64::deserialize(deserializer)?;
        let sign = u32::from(val < 0.0);
        let abs = val.abs();
        let scale = 4u32;
        let raw = (abs * 10f64.powi(scale as i32)).round() as u128;
        Ok(Decimal {
            lo: raw as u64,
            hi: (raw >> 64) as u32,
            sign_scale: (scale << 1) | sign,
        })
    }
}

// ============================================================================
// Deku implementations for SE network protocol
// ============================================================================

use crate::deku::BitAligned;
use deku::prelude::{Reader, Writer};
use deku::{DekuError, DekuReader, DekuWriter};
use std::io::{Read, Seek, Write};

impl DekuReader<'_, ()> for DateTime {
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError> {
        // SE serializes DateTime as i64 ticks
        let ticks = BitAligned::<i64>::from_reader_with_ctx(reader, ())?;
        Ok(DateTime(ticks.0, TimeSpanScale::Ticks, DateTimeKind::Utc))
    }
}

impl DekuWriter<()> for DateTime {
    fn to_writer<W: Write + Seek>(&self, writer: &mut Writer<W>, _ctx: ()) -> Result<(), DekuError> {
        // Convert to ticks for serialization
        let ticks = match self.1 {
            TimeSpanScale::Days => self.0 * 86400 * TICKS_PER_SECOND,
            TimeSpanScale::Hours => self.0 * 3600 * TICKS_PER_SECOND,
            TimeSpanScale::Minutes => self.0 * 60 * TICKS_PER_SECOND,
            TimeSpanScale::Seconds => self.0 * TICKS_PER_SECOND,
            TimeSpanScale::Milliseconds => self.0 * (TICKS_PER_SECOND / 1000),
            TimeSpanScale::Ticks => self.0,
            TimeSpanScale::MinMax => self.0,
        };
        BitAligned(ticks).to_writer(writer, ())
    }
}

impl DekuReader<'_, ()> for TimeSpan {
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError> {
        // SE serializes TimeSpan as i64 ticks
        let ticks = BitAligned::<i64>::from_reader_with_ctx(reader, ())?;
        Ok(TimeSpan(ticks.0, TimeSpanScale::Ticks))
    }
}

impl DekuWriter<()> for TimeSpan {
    fn to_writer<W: Write + Seek>(&self, writer: &mut Writer<W>, _ctx: ()) -> Result<(), DekuError> {
        // Convert to ticks for serialization
        let ticks = match self.1 {
            TimeSpanScale::Days => self.0 * 86400 * TICKS_PER_SECOND,
            TimeSpanScale::Hours => self.0 * 3600 * TICKS_PER_SECOND,
            TimeSpanScale::Minutes => self.0 * 60 * TICKS_PER_SECOND,
            TimeSpanScale::Seconds => self.0 * TICKS_PER_SECOND,
            TimeSpanScale::Milliseconds => self.0 * (TICKS_PER_SECOND / 1000),
            TimeSpanScale::Ticks => self.0,
            TimeSpanScale::MinMax => self.0,
        };
        BitAligned(ticks).to_writer(writer, ())
    }
}

impl DekuReader<'_, ()> for Guid {
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError> {
        // SE serializes Guid as 2 u64 (lo, hi)
        let lo = BitAligned::<u64>::from_reader_with_ctx(reader, ())?;
        let hi = BitAligned::<u64>::from_reader_with_ctx(reader, ())?;
        Ok(Guid(lo.0, hi.0))
    }
}

impl DekuWriter<()> for Guid {
    fn to_writer<W: Write + Seek>(&self, writer: &mut Writer<W>, _ctx: ()) -> Result<(), DekuError> {
        BitAligned(self.0).to_writer(writer, ())?;
        BitAligned(self.1).to_writer(writer, ())
    }
}

impl DekuReader<'_, ()> for Decimal {
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError> {
        let lo = BitAligned::<u64>::from_reader_with_ctx(reader, ())?;
        let hi = BitAligned::<u32>::from_reader_with_ctx(reader, ())?;
        let sign_scale = BitAligned::<u32>::from_reader_with_ctx(reader, ())?;
        Ok(Decimal { lo: lo.0, hi: hi.0, sign_scale: sign_scale.0 })
    }
}

impl DekuWriter<()> for Decimal {
    fn to_writer<W: Write + Seek>(&self, writer: &mut Writer<W>, _ctx: ()) -> Result<(), DekuError> {
        BitAligned(self.lo).to_writer(writer, ())?;
        BitAligned(self.hi).to_writer(writer, ())?;
        BitAligned(self.sign_scale).to_writer(writer, ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datetime_from_chrono_days() {
        let chrono_dt = ChronoDateTime::from_timestamp(86400 * 5, 0).unwrap();
        let dt = DateTime::from_chrono(chrono_dt);
        assert_eq!(dt.0, 5);
        assert_eq!(dt.1, TimeSpanScale::Days);
        assert_eq!(dt.2, DateTimeKind::Utc);
    }

    #[test]
    fn test_datetime_from_chrono_hours() {
        let chrono_dt = ChronoDateTime::from_timestamp(3600 * 10, 0).unwrap();
        let dt = DateTime::from_chrono(chrono_dt);
        assert_eq!(dt.0, 10);
        assert_eq!(dt.1, TimeSpanScale::Hours);
        assert_eq!(dt.2, DateTimeKind::Utc);
    }

    #[test]
    fn test_datetime_roundtrip() {
        let chrono_dt = ChronoDateTime::from_timestamp(1234567890, 0).unwrap();
        let dt = DateTime::from_chrono(chrono_dt);
        let roundtrip = dt.to_chrono();
        assert_eq!(chrono_dt, roundtrip);
    }

    #[test]
    fn test_timespan_from_duration_days() {
        let ts = TimeSpan::from_duration(Duration::from_secs(86400 * 7));
        assert_eq!(ts.0, 7);
        assert_eq!(ts.1, TimeSpanScale::Days);
    }

    #[test]
    fn test_timespan_from_duration_roundtrip() {
        let ts = TimeSpan::from_duration(Duration::from_secs(86400 * 7));
        assert_eq!(ts.to_duration(), Duration::from_secs(86400 * 7));
    }

    #[test]
    fn test_guid_roundtrip() {
        let uuid = Uuid::new_v4();
        let guid = Guid::from_uuid(&uuid);
        let roundtrip = guid.to_uuid();
        assert_eq!(uuid, roundtrip);
    }
}
