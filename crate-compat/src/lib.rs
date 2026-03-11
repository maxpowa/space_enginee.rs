use chrono::{DateTime as ChronoDateTime, Utc};
use deku::bitvec::{BitField as _, BitVec, Msb0};
use deku::ctx::Order;
use deku::prelude::{Reader, Writer};
use deku::{DekuError, DekuReader, DekuWriter};
use enumflags2::{BitFlag, BitFlags};
use proto_rs::bytes::Buf;
use proto_rs::encoding::{DecodeContext, WireType};
use proto_rs::DecodeError;
use proto_rs::{
    ProtoArchive, ProtoDecoder, ProtoDefault, ProtoEncode, ProtoExt, ProtoKind, ProtoShadowDecode,
    ProtoShadowEncode, RevWriter,
};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;
use std::hash::Hash;
use std::io::{Read, Seek, Write};
use std::time::Duration;
use uuid::Uuid;

pub mod direction;
pub mod math;

// Support for C# bcl.proto types: DateTime, TimeSpan, Guid, Decimal
// Also includes support for SerializableDictionary<K, V> (from Space Engineers itself)

const TICKS_PER_SECOND: i64 = 10_000_000;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[proto_rs::proto_message]
pub enum TimeSpanScale {
    Days = 0,
    Hours = 1,
    Minutes = 2,
    Seconds = 3,
    Milliseconds = 4,
    Ticks = 5,
    MinMax = 15,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[proto_rs::proto_message]
pub enum DateTimeKind {
    Unspecified = 0,
    Utc = 1,
    Local = 2,
}

#[derive(Clone, PartialEq, Eq, Debug)]
#[proto_rs::proto_message]
pub struct DateTime(
    #[proto(tag = "1")] i64,           // value
    #[proto(tag = "2")] TimeSpanScale, // scale
    #[proto(tag = "3")] DateTimeKind,  // kind
);

impl DateTime {
    pub fn from_chrono(datetime: ChronoDateTime<Utc>) -> Self {
        // Convert with some smarts, preferring as little loss as possible
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
        // The offset here is from 1970-01-01T00:00:00Z
        let seconds = match self.1 {
            TimeSpanScale::Days => self.0 * 86400,
            TimeSpanScale::Hours => self.0 * 3600,
            TimeSpanScale::Minutes => self.0 * 60,
            TimeSpanScale::Seconds => self.0,
            TimeSpanScale::Milliseconds => self.0 / 1000,
            TimeSpanScale::Ticks => self.0 / TICKS_PER_SECOND,
            TimeSpanScale::MinMax => 0, // Not a valid DateTime representation
        };
        ChronoDateTime::<Utc>::from_timestamp(seconds, 0).unwrap_or_default()
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

impl ::serde::Serialize for DateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let chrono_dt = self.to_chrono();
        serializer.serialize_str(&chrono_dt.to_rfc3339())
    }
}

impl<'de> ::serde::Deserialize<'de> for DateTime {
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
                
                // Try RFC3339 first (with timezone)
                if let Ok(dt) = ChronoDateTime::parse_from_rfc3339(s) {
                    return Ok(DateTime::from_chrono(dt.with_timezone(&Utc)));
                }
                
                // Try ISO 8601 without timezone (common in Space Engineers)
                // Format: 2081-01-01T07:00:00
                if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
                    return Ok(DateTime::from_chrono(naive.and_utc()));
                }
                
                // Try with fractional seconds
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

            // Handle xsi:nil="true" elements which appear as maps
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de2>,
            {
                // Consume all entries (likely just xsi:nil attribute)
                while map.next_entry::<de::IgnoredAny, de::IgnoredAny>()?.is_some() {}
                Ok(DateTime::default())
            }
        }

        deserializer.deserialize_any(DateTimeVisitor)
    }
}

impl Default for DateTime {
    fn default() -> Self {
        DateTime(0, TimeSpanScale::Days, DateTimeKind::Utc)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[proto_rs::proto_message]
pub struct TimeSpan(#[proto(tag = "1")] i64, #[proto(tag = "2")] TimeSpanScale);

impl TimeSpan {
    pub fn from_duration(duration: Duration) -> Self {
        // Convert with some smarts, preferring as little loss as possible
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
            TimeSpanScale::Days => Duration::from_secs(self.0 as u64 * 86400), // Days
            TimeSpanScale::Hours => Duration::from_secs(self.0 as u64 * 3600), // Hours
            TimeSpanScale::Minutes => Duration::from_secs(self.0 as u64 * 60), // Minutes
            TimeSpanScale::Seconds => Duration::from_secs(self.0 as u64),      // Seconds
            TimeSpanScale::Milliseconds => Duration::from_millis(self.0 as u64), // Milliseconds
            TimeSpanScale::Ticks => Duration::from_nanos(self.0 as u64 * 100), // Ticks
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

impl ::serde::Serialize for TimeSpan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let duration = self.to_duration();
        serializer.serialize_u64(duration.as_secs())
    }
}

impl<'de> ::serde::Deserialize<'de> for TimeSpan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        let duration = Duration::from_secs(secs);
        Ok(TimeSpan::from_duration(duration))
    }
}

impl Default for TimeSpan {
    fn default() -> Self {
        TimeSpan(0, TimeSpanScale::Seconds)
    }
}

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

impl Default for Guid {
    fn default() -> Self {
        Uuid::nil().into()
    }
}

/// protobuf-net bcl.proto Decimal representation.
/// See: <https://github.com/protobuf-net/protobuf-net/blob/main/src/Tools/bcl.proto>
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
        // Simple round-trip via f64 (lossy for very large decimals)
        let val = f64::deserialize(deserializer)?;
        let sign = u32::from(val < 0.0);
        let abs = val.abs();
        // Use scale=4 as a reasonable default
        let scale = 4u32;
        let raw = (abs * 10f64.powi(scale as i32)).round() as u128;
        Ok(Decimal {
            lo: raw as u64,
            hi: (raw >> 64) as u32,
            sign_scale: (scale << 1) | sign,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitField<T: BitFlag>(BitFlags<T>);

impl<T: BitFlag> Default for BitField<T> {
    fn default() -> Self {
        BitField(T::from_bits_truncate(T::DEFAULT))
    }
}

impl<T: BitFlag> From<BitFlags<T>> for BitField<T> {
    fn from(flags: BitFlags<T>) -> Self {
        BitField(flags)
    }
}

impl<T: BitFlag> ::serde::Serialize for BitField<T>
where
    T::Numeric: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        T::Numeric::serialize(&self.0.bits(), serializer)
    }
}

impl<'de, T: BitFlag> ::serde::Deserialize<'de> for BitField<T>
where
    T::Numeric: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = T::Numeric::deserialize(deserializer)?;
        let flags = BitFlags::from_bits_truncate(bits);
        Ok(BitField(flags))
    }
}

// ---- proto_rs 0.11 trait impls for BitField<T> ----
// BitField<T> acts as an i32 on the wire (varint-encoded bitflags).

// Encoding shadow type: borrows BitField and presents it as an i32 for archiving.
#[doc(hidden)]
pub struct BitFieldShadow(i32);

impl ProtoExt for BitFieldShadow {
    const KIND: ProtoKind = <i32 as ProtoExt>::KIND;
}

impl ProtoArchive for BitFieldShadow {
    #[inline]
    fn is_default(&self) -> bool {
        self.0 == 0
    }
    #[inline]
    fn archive<const TAG: u32>(&self, w: &mut impl RevWriter) {
        // Delegate to i32's archive (varint encoding)
        self.0.archive::<TAG>(w);
    }
}

impl<'a, T: BitFlag> ProtoShadowEncode<'a, BitField<T>> for BitFieldShadow
where
    T::Numeric: Into<u32>,
{
    #[inline]
    fn from_sun(value: &'a BitField<T>) -> Self {
        let u32_val: u32 = value.0.bits().into();
        BitFieldShadow(u32_val as i32)
    }
}

impl<T: BitFlag> ProtoExt for BitField<T> {
    const KIND: ProtoKind = <i32 as ProtoExt>::KIND;
}

impl<T: BitFlag> ProtoEncode for BitField<T>
where
    T::Numeric: Into<u32>,
{
    type Shadow<'a> = BitFieldShadow;
}

impl<T: BitFlag> ProtoDefault for BitField<T> {
    #[inline]
    fn proto_default() -> Self {
        BitField(BitFlags::empty())
    }
}

impl<T: BitFlag> ProtoShadowDecode<BitField<T>> for BitField<T> {
    #[inline]
    fn to_sun(self) -> Result<BitField<T>, DecodeError> {
        Ok(self)
    }
}

impl<T: BitFlag> ProtoDecoder for BitField<T>
where
    T::Numeric: Into<u32>,
    u32: TryInto<T::Numeric>,
    <u32 as TryInto<T::Numeric>>::Error: Debug,
{
    #[inline]
    fn merge_field(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        if tag == 1 {
            let mut i32_val = 0i32;
            proto_rs::encoding::int32::merge(wire_type, &mut i32_val, buf, ctx)?;
            let u32_val = i32_val as u32;
            let numeric: T::Numeric = u32_val.try_into().map_err(|err| {
                DecodeError::new(format!(
                    "Failed to convert u32 to flag numeric type: {err:?}"
                ))
            })?;
            value.0 = BitFlags::from_bits_truncate(numeric);
            Ok(())
        } else {
            proto_rs::encoding::skip_field(wire_type, tag, buf, ctx)
        }
    }

    #[inline]
    fn merge(
        &mut self,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        let mut i32_val = 0i32;
        proto_rs::encoding::int32::merge(wire_type, &mut i32_val, buf, ctx)?;
        let u32_val = i32_val as u32;
        let numeric: T::Numeric = u32_val.try_into().map_err(|err| {
            DecodeError::new(format!(
                "Failed to convert u32 to flag numeric type: {err:?}"
            ))
        })?;
        self.0 = BitFlags::from_bits_truncate(numeric);
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nullable<T: Default + PartialEq>(pub T);

impl<T: Default + PartialEq> From<T> for Nullable<T> {
    fn from(value: T) -> Self {
        Nullable(value)
    }
}

impl<T: Default + PartialEq> Nullable<T> {
    pub fn has_value(&self) -> bool {
        T::default() != self.0
    }
    pub fn unwrap_mut(&mut self) -> &mut T {
        assert!(self.has_value(), "called `Nullable::unwrap_mut()` on a null value");
        &mut self.0
    }
    pub fn unwrap(self) -> T {
        assert!(self.has_value(), "called `Nullable::unwrap()` on a null value");
        self.0
    }
}

impl<T: Default + PartialEq + Serialize> Serialize for Nullable<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.has_value() {
            serializer.serialize_some(&self.0)
        } else {
            serializer.serialize_none()
        }
    }
}

impl<'de, T: Default + PartialEq + Deserialize<'de>> Deserialize<'de> for Nullable<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        use std::fmt;
        use std::marker::PhantomData;

        struct NullableVisitor<U>(PhantomData<U>);

        impl<'de2, U: Default + PartialEq + Deserialize<'de2>> Visitor<'de2> for NullableVisitor<U> {
            type Value = Nullable<U>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a nullable value, xsi:nil element, or empty element")
            }

            // xsi:nil="true" is presented as a map with the attribute
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de2>,
            {
                // Consume all entries - for xsi:nil there's just the attribute
                while map.next_entry::<de::IgnoredAny, de::IgnoredAny>()?.is_some() {}
                Ok(Nullable(U::default()))
            }

            // Empty elements are presented as empty strings
            fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
                if s.is_empty() {
                    Ok(Nullable(U::default()))
                } else {
                    // Try to deserialize from the string using U's Deserialize
                    use serde::de::IntoDeserializer;
                    U::deserialize(s.into_deserializer()).map(Nullable)
                }
            }

            fn visit_string<E: de::Error>(self, s: String) -> Result<Self::Value, E> {
                self.visit_str(&s)
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Nullable(U::default()))
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Nullable(U::default()))
            }

            fn visit_some<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
            where
                D2: serde::Deserializer<'de2>,
            {
                // Recursively try to deserialize
                U::deserialize(deserializer).map(Nullable)
            }

            // Primitives
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                use serde::de::IntoDeserializer;
                U::deserialize(v.into_deserializer()).map(Nullable)
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                use serde::de::IntoDeserializer;
                U::deserialize(v.into_deserializer()).map(Nullable)
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                use serde::de::IntoDeserializer;
                U::deserialize(v.into_deserializer()).map(Nullable)
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                use serde::de::IntoDeserializer;
                U::deserialize(v.into_deserializer()).map(Nullable)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de2>,
            {
                // Try to deserialize from the sequence
                // If there's content, use it; otherwise return default
                if let Some(value) = seq.next_element::<U>()? {
                    // Drain remaining elements
                    while seq.next_element::<de::IgnoredAny>()?.is_some() {}
                    Ok(Nullable(value))
                } else {
                    Ok(Nullable(U::default()))
                }
            }
        }

        deserializer.deserialize_any(NullableVisitor(PhantomData))
    }
}

impl<T> DekuReader<'_, ()> for Nullable<T>
where
    T: Default + for<'a> DekuReader<'a> + DekuWriter + PartialEq,
{
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        ctx: (),
    ) -> Result<Self, DekuError>
    where
        Self: Sized,
    {
        let has_value = reader.read_bits(1, Order::Lsb0)?.unwrap().load::<u8>() == 1u8;
        if has_value {
            return Ok(Nullable(T::from_reader_with_ctx(reader, ctx)?));
        }
        Ok(Nullable(T::default()))
    }
}

impl<T> DekuWriter<()> for Nullable<T>
where
    T: Default + for<'a> DekuReader<'a> + DekuWriter + PartialEq,
{
    fn to_writer<W: Write + Seek>(&self, writer: &mut Writer<W>, ctx: ()) -> Result<(), DekuError> {
        let mut entry = BitVec::<u8, Msb0>::with_capacity(1);
        entry.push(self.has_value());
        writer.write_bits_order(&entry, Order::Lsb0)?;
        if self.has_value() {
            self.0.to_writer(writer, ctx)?;
        }

        Ok(())
    }
}

// ---- proto_rs 0.11 trait impls for Nullable<T> ----
// Nullable<T> is a transparent wrapper, so it delegates all proto ops to T.

#[doc(hidden)]
pub struct NullableShadow<S>(S);

impl<S: ProtoExt> ProtoExt for NullableShadow<S> {
    const KIND: ProtoKind = S::KIND;
}

impl<S: ProtoArchive> ProtoArchive for NullableShadow<S> {
    #[inline]
    fn is_default(&self) -> bool {
        self.0.is_default()
    }
    #[inline]
    fn archive<const TAG: u32>(&self, w: &mut impl RevWriter) {
        self.0.archive::<TAG>(w);
    }
}

impl<'a, T> ProtoShadowEncode<'a, Nullable<T>> for NullableShadow<<T as ProtoEncode>::Shadow<'a>>
where
    T: ProtoEncode + Default + PartialEq,
{
    #[inline]
    fn from_sun(value: &'a Nullable<T>) -> Self {
        NullableShadow(<T as ProtoEncode>::Shadow::from_sun(&value.0))
    }
}

impl<T> ProtoExt for Nullable<T>
where
    T: ProtoExt + Default + PartialEq,
{
    const KIND: ProtoKind = T::KIND;
}

impl<T> ProtoEncode for Nullable<T>
where
    T: ProtoEncode + Default + PartialEq,
    for<'a> <T as ProtoEncode>::Shadow<'a>: ProtoArchive + ProtoExt,
{
    type Shadow<'a> = NullableShadow<<T as ProtoEncode>::Shadow<'a>>;
}

impl<T> ProtoDefault for Nullable<T>
where
    T: ProtoDefault + Default + PartialEq,
{
    #[inline]
    fn proto_default() -> Self {
        Nullable(T::proto_default())
    }
}

impl<T> ProtoShadowDecode<Nullable<T>> for Nullable<T>
where
    T: Default + PartialEq,
{
    #[inline]
    fn to_sun(self) -> Result<Nullable<T>, DecodeError> {
        Ok(self)
    }
}

impl<T> ProtoDecoder for Nullable<T>
where
    T: ProtoDecoder + Default + PartialEq,
{
    #[inline]
    fn merge_field(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        T::merge_field(&mut value.0, tag, wire_type, buf, ctx)
    }

    #[inline]
    fn merge(
        &mut self,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        T::merge(&mut self.0, wire_type, buf, ctx)
    }
}

#[derive(Debug, Clone, PartialEq)]
#[proto_rs::proto_message]
pub struct SerializableDictionary<K: Hash + Eq, V>(#[proto(tag = 1)] pub HashMap<K, V>);

impl<K: Hash + Eq + ::serde::Serialize, V: ::serde::Serialize> ::serde::Serialize
    for SerializableDictionary<K, V>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(::serde::Serialize)]
        #[serde(rename = "item")]
        struct SerializableDictionaryEntryRef<'a, T, U> {
            #[serde(rename = "Key")]
            k: &'a T,
            #[serde(rename = "Value")]
            v: &'a U,
        }

        let mut state = serializer.serialize_struct("SerializableDictionary", 1)?;
        let entries_iter = self
            .0
            .iter()
            .map(|(k, v)| SerializableDictionaryEntryRef { k, v });
        let entries: Vec<_> = entries_iter.collect();
        SerializeStruct::serialize_field(&mut state, "dictionary", &entries)?;
        SerializeStruct::end(state)
    }
}

impl<'de, K: Hash + Eq + ::serde::Deserialize<'de>, V: ::serde::Deserialize<'de>>
    ::serde::Deserialize<'de> for SerializableDictionary<K, V>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Owned version for deserialization
        #[derive(::serde::Deserialize)]
        #[serde(rename = "item")]
        struct SerializableDictionaryEntry<T, U> {
            #[serde(rename = "Key")]
            k: T,
            #[serde(rename = "Value")]
            v: U,
        }

        /// Wrapper around `<dictionary>` that tolerates `<dictionary />` (empty).
        ///
        /// When quick-xml sees `<dictionary />`, it delivers an empty text event
        /// instead of child `<item>` elements.  A plain `Vec<Entry>` would then
        /// try to parse `""` as an Entry and fail with "missing field `Key`".
        ///
        /// We use `deserialize_with` on the items field so that each
        /// `<dictionary>` element is first attempted as an `Entry`; if that
        /// fails (e.g. for `<dictionary />`), we silently skip it.
        #[allow(clippy::unnecessary_wraps)] // serde deserialize_with requires Result
        fn deserialize_entries<'de, T: ::serde::Deserialize<'de>, U: ::serde::Deserialize<'de>, D>(
            deserializer: D,
        ) -> Result<Vec<SerializableDictionaryEntry<T, U>>, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            // Try to deserialize as a Vec; if a single empty element like
            // `<dictionary />` is present, the whole Vec parse may fail.
            // In that case, fall back to an empty Vec.
            Ok(
                Vec::<SerializableDictionaryEntry<T, U>>::deserialize(deserializer)
                    .unwrap_or_default(),
            )
        }

        fn empty_vec<T>() -> Vec<T> {
            Vec::new()
        }
        #[derive(::serde::Deserialize)]
        #[serde(rename = "Dictionary")]
        #[serde(bound(deserialize = "T: ::serde::Deserialize<'de>, U: ::serde::Deserialize<'de>"))]
        struct Helper<T, U> {
            #[serde(
                rename = "dictionary",
                default = "empty_vec",
                deserialize_with = "deserialize_entries"
            )]
            items: Vec<SerializableDictionaryEntry<T, U>>,
        }
        let helper = Helper::deserialize(deserializer)?;
        let map = helper
            .items
            .into_iter()
            .map(|entry| (entry.k, entry.v))
            .collect();
        Ok(SerializableDictionary(map))
    }
}

impl<K: Hash + Eq, V> Default for SerializableDictionary<K, V> {
    fn default() -> Self {
        SerializableDictionary(HashMap::new())
    }
}

#[derive(Debug, Default, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
#[proto_rs::proto_message]
#[serde(rename = "MyTuple")]
pub struct Tuple<K, V> {
    #[proto(tag = 1)]
    #[serde(rename = "Item1")]
    pub item1: K,
    #[proto(tag = 2)]
    #[serde(rename = "Item2")]
    pub item2: V,
}

/// Generates a module containing `serialize` and `deserialize` functions that
/// handle C#'s `[XmlArrayItem("name")]` pattern for `Vec<T>` fields.
///
/// In C# Space Engineers types, a field like:
/// ```csharp
/// [XmlArrayItem("Warning")]
/// public List<string> SuppressedWarnings = new List<string>();
/// ```
/// serializes as:
/// ```xml
/// <SuppressedWarnings>
///   <Warning>value1</Warning>
///   <Warning>value2</Warning>
/// </SuppressedWarnings>
/// ```
///
/// Usage (generated by codegen):
/// ```ignore
/// crate::compat::define_xml_array_item!(Warning);
///
/// #[serde(serialize_with = "Warning::serialize",
///         deserialize_with = "Warning::deserialize")]
/// pub my_field: Vec<String>,
/// ```
///
/// The field type remains `Vec<T>` — no wrapper types, no proto changes.
#[macro_export]
macro_rules! define_xml_array_item {
    ($name:ident) => {
        #[allow(non_snake_case)]
        pub mod $name {
            pub fn serialize<T, S>(vec: &Vec<T>, serializer: S) -> Result<S::Ok, S::Error>
            where
                T: ::serde::Serialize,
                S: ::serde::Serializer,
            {
                use ::serde::ser::SerializeStruct;
                let mut state = serializer.serialize_struct("wrapper", 1)?;
                state.serialize_field(stringify!($name), vec)?;
                state.end()
            }

            pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
            where
                T: ::serde::Deserialize<'de>,
                D: ::serde::Deserializer<'de>,
            {
                use ::serde::de::{self, MapAccess, Visitor};
                use ::std::marker::PhantomData;

                struct ArrayVisitor<U>(PhantomData<U>);

                impl<'de2, U: ::serde::Deserialize<'de2>> Visitor<'de2> for ArrayVisitor<U> {
                    type Value = Vec<U>;

                    fn expecting(
                        &self,
                        formatter: &mut ::std::fmt::Formatter,
                    ) -> ::std::fmt::Result {
                        write!(formatter, "a sequence of <{}> elements", stringify!($name))
                    }

                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: MapAccess<'de2>,
                    {
                        let mut items = Vec::new();
                        while let Some(key) = map.next_key::<String>()? {
                            if key == stringify!($name) {
                                items.push(map.next_value()?);
                            } else {
                                // Skip unknown elements
                                map.next_value::<de::IgnoredAny>()?;
                            }
                        }
                        Ok(items)
                    }

                    // Handle the case where the element is empty / self-closing
                    fn visit_str<E: de::Error>(self, _: &str) -> Result<Self::Value, E> {
                        Ok(Vec::new())
                    }

                    fn visit_string<E: de::Error>(self, _: String) -> Result<Self::Value, E> {
                        Ok(Vec::new())
                    }

                    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                        Ok(Vec::new())
                    }
                }

                deserializer.deserialize_map(ArrayVisitor(PhantomData))
            }
        }
    };
}

/// Deserializes a `Vec<T>` from XML, gracefully handling self-closing elements
/// like `<Members />`.
///
/// Space Engineers serializes empty collections as self-closing XML elements.
/// `quick_xml` treats these as an element with empty text content and tries to
/// deserialize one `T` from it, which fails when `T` has required fields.
///
/// This function uses `deserialize_any` which in quick_xml's `MapValueDeserializer`
/// peeks the next event and dispatches:
/// - `Text` (self-closing) → `deserialize_str` → `visit_str` → empty Vec
/// - `Start` (populated)   → `deserialize_map` → `visit_map` → collect items
pub mod xml_vec {
    use serde::de::{self, Visitor};
    use std::fmt;
    use std::marker::PhantomData;

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        T: serde::Deserialize<'de>,
        D: serde::Deserializer<'de>,
    {
        struct VecVisitor<U>(PhantomData<U>);

        impl<'de2, U: serde::Deserialize<'de2>> Visitor<'de2> for VecVisitor<U> {
            type Value = Vec<U>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence of elements or an empty/self-closing element")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de2>,
            {
                let mut items = Vec::new();
                while let Some(_key) = map.next_key::<de::IgnoredAny>()? {
                    items.push(map.next_value()?);
                }
                Ok(items)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de2>,
            {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(items)
            }

            // Self-closing elements like `<Members />` are presented as empty text
            fn visit_str<E: de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Ok(Vec::new())
            }

            fn visit_string<E: de::Error>(self, _: String) -> Result<Self::Value, E> {
                Ok(Vec::new())
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Vec::new())
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Vec::new())
            }
        }

        deserializer.deserialize_any(VecVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime as ChronoDateTime, Utc};
    use std::time::Duration;
    use uuid::Uuid;

    // DateTime tests
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
    fn test_datetime_from_chrono_minutes() {
        let chrono_dt = ChronoDateTime::from_timestamp(60 * 30, 0).unwrap();
        let dt = DateTime::from_chrono(chrono_dt);
        assert_eq!(dt.0, 30);
        assert_eq!(dt.1, TimeSpanScale::Minutes);
        assert_eq!(dt.2, DateTimeKind::Utc);
    }

    #[test]
    fn test_datetime_from_chrono_seconds() {
        let chrono_dt = ChronoDateTime::from_timestamp(1234567, 0).unwrap();
        let dt = DateTime::from_chrono(chrono_dt);
        assert_eq!(dt.0, 1234567);
        assert_eq!(dt.1, TimeSpanScale::Seconds);
        assert_eq!(dt.2, DateTimeKind::Utc);
    }

    #[test]
    fn test_datetime_to_chrono_days() {
        let dt = DateTime(5, TimeSpanScale::Days, DateTimeKind::Utc);
        let chrono_dt = dt.to_chrono();
        assert_eq!(chrono_dt.timestamp(), 86400 * 5);
    }

    #[test]
    fn test_datetime_to_chrono_hours() {
        let dt = DateTime(10, TimeSpanScale::Hours, DateTimeKind::Utc);
        let chrono_dt = dt.to_chrono();
        assert_eq!(chrono_dt.timestamp(), 3600 * 10);
    }

    #[test]
    fn test_datetime_to_chrono_minutes() {
        let dt = DateTime(30, TimeSpanScale::Minutes, DateTimeKind::Utc);
        let chrono_dt = dt.to_chrono();
        assert_eq!(chrono_dt.timestamp(), 60 * 30);
    }

    #[test]
    fn test_datetime_to_chrono_seconds() {
        let dt = DateTime(1234567, TimeSpanScale::Seconds, DateTimeKind::Utc);
        let chrono_dt = dt.to_chrono();
        assert_eq!(chrono_dt.timestamp(), 1234567);
    }

    #[test]
    fn test_datetime_to_chrono_milliseconds() {
        let dt = DateTime(5000, TimeSpanScale::Milliseconds, DateTimeKind::Utc);
        let chrono_dt = dt.to_chrono();
        assert_eq!(chrono_dt.timestamp(), 5);
    }

    #[test]
    fn test_datetime_to_chrono_ticks() {
        let dt = DateTime(
            TICKS_PER_SECOND * 10,
            TimeSpanScale::Ticks,
            DateTimeKind::Utc,
        );
        let chrono_dt = dt.to_chrono();
        assert_eq!(chrono_dt.timestamp(), 10);
    }

    #[test]
    fn test_datetime_roundtrip() {
        let original = ChronoDateTime::from_timestamp(86400 * 7, 0).unwrap();
        let dt = DateTime::from_chrono(original);
        let result = dt.to_chrono();
        assert_eq!(original.timestamp(), result.timestamp());
    }

    #[test]
    fn test_datetime_default() {
        let dt = DateTime::default();
        assert_eq!(dt.0, 0);
        assert_eq!(dt.1, TimeSpanScale::Days);
        assert_eq!(dt.2, DateTimeKind::Utc);
    }

    #[test]
    fn test_datetime_from_trait() {
        let chrono_dt = ChronoDateTime::from_timestamp(86400, 0).unwrap();
        let dt: DateTime = chrono_dt.into();
        assert_eq!(dt.0, 1);
        assert_eq!(dt.1, TimeSpanScale::Days);
    }

    #[test]
    fn test_datetime_into_trait() {
        let dt = DateTime(1, TimeSpanScale::Days, DateTimeKind::Utc);
        let chrono_dt: ChronoDateTime<Utc> = dt.into();
        assert_eq!(chrono_dt.timestamp(), 86400);
    }

    #[test]
    fn test_datetime_serde_roundtrip() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        struct W {
            value: DateTime,
        }
        let w = W {
            value: DateTime(5, TimeSpanScale::Days, DateTimeKind::Utc),
        };
        let xml = quick_xml::se::to_string(&w).unwrap();
        let deserialized: W = quick_xml::de::from_str(&xml).unwrap();
        assert_eq!(w, deserialized);
    }

    // TimeSpan tests
    #[test]
    fn test_timespan_from_duration_days() {
        let duration = Duration::from_secs(86400 * 3);
        let ts = TimeSpan::from_duration(duration);
        assert_eq!(ts.0, 3);
        assert_eq!(ts.1, TimeSpanScale::Days);
    }

    #[test]
    fn test_timespan_from_duration_hours() {
        let duration = Duration::from_secs(3600 * 5);
        let ts = TimeSpan::from_duration(duration);
        assert_eq!(ts.0, 5);
        assert_eq!(ts.1, TimeSpanScale::Hours);
    }

    #[test]
    fn test_timespan_from_duration_minutes() {
        let duration = Duration::from_secs(60 * 15);
        let ts = TimeSpan::from_duration(duration);
        assert_eq!(ts.0, 15);
        assert_eq!(ts.1, TimeSpanScale::Minutes);
    }

    #[test]
    fn test_timespan_from_duration_seconds() {
        let duration = Duration::from_secs(123);
        let ts = TimeSpan::from_duration(duration);
        assert_eq!(ts.0, 123);
        assert_eq!(ts.1, TimeSpanScale::Seconds);
    }

    #[test]
    fn test_timespan_from_duration_ticks() {
        // 7 seconds + 1_500_100ns � secs not divisible by 60, sub-second nanos
        // with ms component so it reaches the Ticks branch
        // 1_500_100ns / 100 = 15001 ticks ? 15001 * 100 = 1_500_100ns � lossless
        let duration = Duration::new(7, 1_500_100);
        let ts = TimeSpan::from_duration(duration);
        assert_eq!(ts.to_duration(), duration);
    }

    #[test]
    fn test_timespan_from_duration_zero() {
        let duration = Duration::ZERO;
        let ts = TimeSpan::from_duration(duration);
        assert_eq!(ts.0, 0);
        assert_eq!(ts.1, TimeSpanScale::MinMax);
    }

    #[test]
    fn test_timespan_from_duration_max() {
        let duration = Duration::MAX;
        let ts = TimeSpan::from_duration(duration);
        assert_eq!(ts.0, i64::MAX);
        assert_eq!(ts.1, TimeSpanScale::MinMax);
    }

    #[test]
    fn test_timespan_to_duration_days() {
        let ts = TimeSpan(3, TimeSpanScale::Days);
        let duration = ts.to_duration();
        assert_eq!(duration.as_secs(), 86400 * 3);
    }

    #[test]
    fn test_timespan_to_duration_hours() {
        let ts = TimeSpan(5, TimeSpanScale::Hours);
        let duration = ts.to_duration();
        assert_eq!(duration.as_secs(), 3600 * 5);
    }

    #[test]
    fn test_timespan_to_duration_minutes() {
        let ts = TimeSpan(15, TimeSpanScale::Minutes);
        let duration = ts.to_duration();
        assert_eq!(duration.as_secs(), 60 * 15);
    }

    #[test]
    fn test_timespan_to_duration_seconds() {
        let ts = TimeSpan(123, TimeSpanScale::Seconds);
        let duration = ts.to_duration();
        assert_eq!(duration.as_secs(), 123);
    }

    #[test]
    fn test_timespan_to_duration_milliseconds() {
        let ts = TimeSpan(1500, TimeSpanScale::Milliseconds);
        let duration = ts.to_duration();
        assert_eq!(duration.as_millis(), 1500);
    }

    #[test]
    fn test_timespan_to_duration_ticks() {
        let ts = TimeSpan(10, TimeSpanScale::Ticks);
        let duration = ts.to_duration();
        assert_eq!(duration.as_nanos(), 1000);
    }

    #[test]
    fn test_timespan_to_duration_minmax_zero() {
        let ts = TimeSpan(0, TimeSpanScale::MinMax);
        let duration = ts.to_duration();
        assert_eq!(duration, Duration::ZERO);
    }

    #[test]
    fn test_timespan_to_duration_minmax_max() {
        let ts = TimeSpan(i64::MAX, TimeSpanScale::MinMax);
        let duration = ts.to_duration();
        assert_eq!(duration, Duration::MAX);
    }

    #[test]
    fn test_timespan_roundtrip() {
        let original = Duration::from_secs(86400 * 7);
        let ts = TimeSpan::from_duration(original);
        let result = ts.to_duration();
        assert_eq!(original, result);
    }

    #[test]
    fn test_timespan_default() {
        let ts = TimeSpan::default();
        assert_eq!(ts.0, 0);
        assert_eq!(ts.1, TimeSpanScale::Seconds);
    }

    #[test]
    fn test_timespan_from_trait() {
        let duration = Duration::from_secs(3600);
        let ts: TimeSpan = duration.into();
        assert_eq!(ts.0, 1);
        assert_eq!(ts.1, TimeSpanScale::Hours);
    }

    #[test]
    fn test_timespan_into_trait() {
        let ts = TimeSpan(5, TimeSpanScale::Hours);
        let duration: Duration = ts.into();
        assert_eq!(duration.as_secs(), 3600 * 5);
    }

    #[test]
    fn test_timespan_serde_roundtrip() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        struct W {
            value: TimeSpan,
        }
        let w = W {
            value: TimeSpan(3600, TimeSpanScale::Seconds),
        };
        let xml = quick_xml::se::to_string(&w).unwrap();
        let deserialized: W = quick_xml::de::from_str(&xml).unwrap();
        // Note: serde serializes as seconds, so scale info is lost
        assert_eq!(w.value.to_duration(), deserialized.value.to_duration());
    }

    // Guid tests
    #[test]
    fn test_guid_from_uuid() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let guid = Guid::from_uuid(&uuid);
        assert_eq!(guid.to_uuid(), uuid);
    }

    #[test]
    fn test_guid_to_uuid() {
        let uuid = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
        let guid = Guid::from_uuid(&uuid);
        let result_uuid = guid.to_uuid();
        assert_eq!(result_uuid, uuid);
    }

    #[test]
    fn test_guid_roundtrip() {
        let original = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
        let guid = Guid::from_uuid(&original);
        let result = guid.to_uuid();
        assert_eq!(original, result);
    }

    #[test]
    fn test_guid_nil() {
        let uuid = Uuid::nil();
        let guid = Guid::from_uuid(&uuid);
        assert_eq!(guid.0, 0);
        assert_eq!(guid.1, 0);
    }

    #[test]
    fn test_guid_default() {
        let guid = Guid::default();
        assert_eq!(guid.to_uuid(), Uuid::nil());
    }

    #[test]
    fn test_guid_from_trait() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let guid: Guid = uuid.into();
        assert_eq!(guid.to_uuid(), uuid);
    }

    #[test]
    fn test_guid_into_trait() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let guid = Guid::from_uuid(&uuid);
        let result_uuid: Uuid = guid.into();
        assert_eq!(result_uuid, uuid);
    }

    #[test]
    fn test_guid_serde_roundtrip() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        struct W {
            value: Guid,
        }
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let w = W {
            value: Guid::from_uuid(&uuid),
        };
        let xml = quick_xml::se::to_string(&w).unwrap();
        let deserialized: W = quick_xml::de::from_str(&xml).unwrap();
        assert_eq!(w, deserialized);
    }

    #[test]
    fn test_guid_clone_eq() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let guid1 = Guid::from_uuid(&uuid);
        let guid2 = guid1.clone();
        assert_eq!(guid1, guid2);
    }

    // BitField tests
    #[::enumflags2::bitflags]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    enum TestFlags {
        FlagA = 1 << 0,
        FlagB = 1 << 1,
        FlagC = 1 << 2,
        FlagD = 1 << 3,
    }

    #[test]
    fn test_bitfield_default() {
        let bf = BitField::<TestFlags>::default();
        assert_eq!(bf.0.bits(), 0);
    }

    #[test]
    fn test_bitfield_from_bitflags() {
        let flags: BitFlags<TestFlags> = TestFlags::FlagA | TestFlags::FlagB;
        let bf = BitField::from(flags);
        assert_eq!(bf.0.bits(), 0b11);
    }

    #[test]
    fn test_bitfield_clone_eq() {
        let flags: BitFlags<TestFlags> = TestFlags::FlagA | TestFlags::FlagC;
        let bf1 = BitField::from(flags);
        let bf2 = bf1.clone();
        assert_eq!(bf1, bf2);
    }

    #[test]
    fn test_bitfield_serde_roundtrip() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        struct W {
            value: BitField<TestFlags>,
        }
        let flags: BitFlags<TestFlags> = TestFlags::FlagA | TestFlags::FlagB | TestFlags::FlagC;
        let w = W {
            value: BitField::from(flags),
        };
        let xml = quick_xml::se::to_string(&w).unwrap();
        let deserialized: W = quick_xml::de::from_str(&xml).unwrap();
        assert_eq!(w, deserialized);
    }

    // Nullable tests

    /// Wrapper to give Nullable a parent element for XML serialization.
    #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
    struct NullableWrapper {
        #[serde(rename = "WorkshopId")]
        workshop_id: Nullable<i32>,
    }

    #[test]
    fn test_nullable_xsi_nil_self_closing() {
        // The exact pattern from Space Engineers: <WorkshopId xsi:nil="true" />
        let xml = r#"<NullableWrapper xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
            <WorkshopId xsi:nil="true" />
        </NullableWrapper>"#;
        let result: NullableWrapper = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(result.workshop_id, Nullable(0));
        assert!(!result.workshop_id.has_value());
    }

    #[test]
    fn test_nullable_xsi_nil_with_content() {
        // xsi:nil="true" should ignore element content
        let xml = r#"<NullableWrapper xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
            <WorkshopId xsi:nil="true">12345</WorkshopId>
        </NullableWrapper>"#;
        let result: NullableWrapper = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(result.workshop_id, Nullable(0));
        assert!(!result.workshop_id.has_value());
    }

    #[test]
    fn test_nullable_deserialize_value() {
        let xml = "<NullableWrapper><WorkshopId>42</WorkshopId></NullableWrapper>";
        let result: NullableWrapper = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(result.workshop_id, Nullable(42));
        assert!(result.workshop_id.has_value());
    }

    #[test]
    fn test_nullable_serde_roundtrip() {
        let original = NullableWrapper {
            workshop_id: Nullable(99),
        };
        let xml = quick_xml::se::to_string(&original).unwrap();
        let deserialized: NullableWrapper = quick_xml::de::from_str(&xml).unwrap();
        assert_eq!(original, deserialized);
    }

    // SerializableDictionary tests

    #[test]
    fn test_serializable_dictionary_empty_no_elements() {
        // When a dictionary is empty, no <dictionary> child elements are
        // present.  The `Option<Vec<�>>` in Helper deserializes as `None`,
        // which `.unwrap_or_default()` turns into an empty Vec.
        #[derive(Debug, PartialEq, ::serde::Deserialize)]
        #[serde(rename = "Root")]
        struct W {
            #[serde(rename = "Dict")]
            dict: SerializableDictionary<String, i32>,
        }
        let xml = r#"<Root><Dict></Dict></Root>"#;
        let result: W = quick_xml::de::from_str(xml).unwrap();
        assert!(result.dict.0.is_empty());
    }

    #[test]
    fn test_serializable_dictionary_empty_self_closing() {
        // <Dict /> (self-closing parent) should also deserialize to an empty
        // HashMap since there are no <dictionary> children.
        #[derive(Debug, PartialEq, ::serde::Deserialize)]
        #[serde(rename = "Root")]
        struct W {
            #[serde(rename = "Dict")]
            dict: SerializableDictionary<String, i32>,
        }
        let xml = r#"<Root><Dict /></Root>"#;
        let result: W = quick_xml::de::from_str(xml).unwrap();
        assert!(result.dict.0.is_empty());
    }

    #[test]
    fn test_serializable_dictionary_populated() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename = "Root")]
        struct W {
            #[serde(rename = "Dict")]
            dict: SerializableDictionary<String, i32>,
        }
        let xml = concat!(
            "<Root><Dict>",
            "<dictionary><Key>alpha</Key><Value>1</Value></dictionary>",
            "<dictionary><Key>beta</Key><Value>2</Value></dictionary>",
            "</Dict></Root>",
        );
        let result: W = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(result.dict.0.len(), 2);
        assert_eq!(result.dict.0["alpha"], 1);
        assert_eq!(result.dict.0["beta"], 2);
    }

    #[test]
    fn test_serializable_dictionary_serde_roundtrip() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename = "Root")]
        struct W {
            #[serde(rename = "Dict")]
            dict: SerializableDictionary<String, i32>,
        }
        let mut map = HashMap::new();
        map.insert("key1".to_string(), 10);
        map.insert("key2".to_string(), 20);
        let original = W {
            dict: SerializableDictionary(map),
        };
        let xml = quick_xml::se::to_string(&original).unwrap();
        let deserialized: W = quick_xml::de::from_str(&xml).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_serializable_dictionary_empty_xml() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename = "Root")]
        struct W {
            #[serde(rename = "Dict")]
            dict: SerializableDictionary<String, i32>,
        }
        // SE serializes empty dictionaries as self-closing <dictionary />
        let xml = r#"<Root><Dict><dictionary /></Dict></Root>"#;
        let deserialized: W = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(deserialized.dict, SerializableDictionary(HashMap::new()));
    }

    /// Pre-generated XmlArrayItem modules for element names used by Space Engineers.
    /// The codegen emits `define_xml_array_item!(Name)` calls for each unique
    /// `[XmlArrayItem("Name")]` it encounters.
    pub mod xml_array_item {
        define_xml_array_item!(Warning);
    }

    // XmlArrayItem serialize_with/deserialize_with tests
    // Uses the Warning module defined in xml_array_item

    #[test]
    fn test_xml_array_item_populated() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename = "Root")]
        struct W {
            #[serde(rename = "SuppressedWarnings", default)]
            #[serde(
                serialize_with = "crate::tests::xml_array_item::Warning::serialize",
                deserialize_with = "crate::tests::xml_array_item::Warning::deserialize"
            )]
            warnings: Vec<String>,
        }
        let xml = r#"<Root><SuppressedWarnings><Warning>w1</Warning><Warning>w2</Warning></SuppressedWarnings></Root>"#;
        let result: W = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(result.warnings, vec!["w1".to_string(), "w2".to_string()]);
    }

    #[test]
    fn test_xml_array_item_empty() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename = "Root")]
        struct W {
            #[serde(rename = "SuppressedWarnings", default)]
            #[serde(
                serialize_with = "crate::tests::xml_array_item::Warning::serialize",
                deserialize_with = "crate::tests::xml_array_item::Warning::deserialize"
            )]
            warnings: Vec<String>,
        }
        let xml = r#"<Root><SuppressedWarnings></SuppressedWarnings></Root>"#;
        let result: W = quick_xml::de::from_str(xml).unwrap();
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_xml_array_item_missing_field() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename = "Root")]
        struct W {
            #[serde(rename = "SuppressedWarnings", default)]
            #[serde(
                serialize_with = "crate::tests::xml_array_item::Warning::serialize",
                deserialize_with = "crate::tests::xml_array_item::Warning::deserialize"
            )]
            warnings: Vec<String>,
        }
        let xml = r#"<Root></Root>"#;
        let result: W = quick_xml::de::from_str(xml).unwrap();
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_xml_array_item_roundtrip() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename = "Root")]
        struct W {
            #[serde(rename = "SuppressedWarnings", default)]
            #[serde(
                serialize_with = "crate::tests::xml_array_item::Warning::serialize",
                deserialize_with = "crate::tests::xml_array_item::Warning::deserialize"
            )]
            warnings: Vec<String>,
        }
        let original = W {
            warnings: vec!["a".into(), "b".into()],
        };
        let xml = quick_xml::se::to_string(&original).unwrap();
        // Verify the XML preserves the element name
        assert!(xml.contains("<Warning>a</Warning>"), "XML was: {}", xml);
        assert!(xml.contains("<Warning>b</Warning>"), "XML was: {}", xml);
        let deserialized: W = quick_xml::de::from_str(&xml).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_xml_array_item_roundtrip_preserves_element_name() {
        #[derive(Debug, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename = "Root")]
        struct W {
            #[serde(rename = "Warnings", default)]
            #[serde(
                serialize_with = "crate::tests::xml_array_item::Warning::serialize",
                deserialize_with = "crate::tests::xml_array_item::Warning::deserialize"
            )]
            items: Vec<String>,
        }
        let xml_in =
            r#"<Root><Warnings><Warning>x</Warning><Warning>y</Warning></Warnings></Root>"#;
        let parsed: W = quick_xml::de::from_str(xml_in).unwrap();
        let xml_out = quick_xml::se::to_string(&parsed).unwrap();
        let reparsed: W = quick_xml::de::from_str(&xml_out).unwrap();
        assert_eq!(parsed, reparsed);
        assert!(xml_out.contains("<Warning>x</Warning>"));
        assert!(xml_out.contains("<Warning>y</Warning>"));
    }
}
