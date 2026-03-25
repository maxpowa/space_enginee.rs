//! BitField wrapper for enumflags2::BitFlags.
//!
//! Provides a serde/proto/deku-compatible wrapper around enumflags2's BitFlags type.

use deku::prelude::Reader;
use deku::{DekuError, DekuReader, DekuWriter};
use enumflags2::{BitFlag, BitFlags};
use proto_rs::bytes::Buf;
use proto_rs::encoding::{DecodeContext, WireType};
use proto_rs::DecodeError;
use proto_rs::{
    ProtoArchive, ProtoDecoder, ProtoDefault, ProtoEncode, ProtoExt, ProtoKind, ProtoShadowDecode,
    ProtoShadowEncode, RevWriter,
};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::fmt::Debug;
use std::io::{Read, Seek, Write};

// ============================================================================
// BitField<T>
// ============================================================================

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

impl<T: BitFlag> BitField<T> {
    pub fn bits(&self) -> T::Numeric {
        self.0.bits()
    }

    pub fn contains(&self, flag: T) -> bool {
        self.0.contains(flag)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ---- Serde ----

impl<T: BitFlag> Serialize for BitField<T>
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

impl<'de, T: BitFlag> Deserialize<'de> for BitField<T>
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

// ---- Deku ----

use crate::deku::BitAligned;

// Specialized implementation for () context - uses BitAligned for SE's bitstream protocol
impl<'a, T> DekuReader<'a, ()> for BitField<T>
where
    T: BitFlag,
    BitAligned<T::Numeric>: DekuReader<'a, ()>,
{
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError>
    where
        Self: Sized,
    {
        let bits = BitAligned::<T::Numeric>::from_reader_with_ctx(reader, ())?;
        let flags = BitFlags::from_bits_truncate(bits.0);
        Ok(BitField(flags))
    }
}

impl<T> DekuWriter<()> for BitField<T>
where
    T: BitFlag,
    BitAligned<T::Numeric>: DekuWriter<()>,
{
    fn to_writer<W: Write + Seek>(
        &self,
        writer: &mut deku::prelude::Writer<W>,
        _ctx: (),
    ) -> Result<(), DekuError> {
        BitAligned(self.0.bits()).to_writer(writer, ())
    }
}

// ---- Proto-rs ----

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
