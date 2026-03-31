//! Bit-stream serialization types for Space Engineers network protocol.
//!
//! These types provide Deku-based serialization for SE's bit-packed network protocol.
//! They are designed to be transparent for serde/proto while providing correct
//! bit-level serialization with Deku.

use deku::bitvec::{AsBits, BitField as _, BitVec, Lsb0};
use deku::ctx::Order;
use deku::prelude::{Reader, Writer};
use deku::{DekuError, DekuReader, DekuWriter};
use proto_rs::bytes::Buf;
use proto_rs::encoding::{DecodeContext, WireType};
use proto_rs::DecodeError;
use proto_rs::{
    ProtoArchive, ProtoDecoder, ProtoDefault, ProtoEncode, ProtoExt, ProtoKind, ProtoShadowDecode,
    ProtoShadowEncode, RevWriter,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Debug;
use std::io::{Read, Seek, Write};

// ============================================================================
// Varint<T> - Variable-length quantity encoding
// ============================================================================

/// Variable-length quantity encoding (7-bit chunks with continuation bit).
///
/// Transparent for serde/proto - serializes as the inner value.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Varint<T>(pub T)
where
    T: Copy;

impl<T: Copy + Default> Default for Varint<T> {
    fn default() -> Self {
        Varint(T::default())
    }
}

impl<T: Copy> Varint<T> {
    pub fn new(value: T) -> Self {
        Varint(value)
    }

    pub fn into_inner(self) -> T {
        self.0
    }

    pub fn get(&self) -> T {
        self.0
    }
}

impl<T: Copy> std::ops::Deref for Varint<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Copy> std::ops::DerefMut for Varint<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: Copy> AsRef<T> for Varint<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: Copy> From<T> for Varint<T> {
    fn from(value: T) -> Self {
        Varint(value)
    }
}

// ---- Serde (transparent) ----

impl<T: Copy + Serialize> Serialize for Varint<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Copy + Deserialize<'de>> Deserialize<'de> for Varint<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Varint)
    }
}

// ---- Deku ----

impl<T> DekuReader<'_, ()> for Varint<T>
where
    T: Into<u64> + TryFrom<u64> + Copy,
    <T as TryFrom<u64>>::Error: Debug,
{
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError>
    where
        Self: Sized,
    {
        let mut value: u64 = 0;
        let mut shift = 0;

        for _ in 0..10 {
            let vec = reader.read_bits(8, Order::Lsb0)?;
            let byte = vec.unwrap().as_bitslice().load::<u8>();

            value |= ((byte & 0x7F) as u64) << shift;

            if (byte & 0x80) == 0 {
                return Ok(Varint(T::try_from(value).expect("VLQ conversion failed")));
            }

            shift += 7;
        }

        Err(DekuError::Parse(Cow::from(
            "VLQ overflow: more than 10 continuation bytes",
        )))
    }
}

impl<T> DekuWriter<()> for Varint<T>
where
    T: Into<u64> + Copy,
{
    fn to_writer<W: Write + Seek>(
        &self,
        writer: &mut Writer<W>,
        _ctx: (),
    ) -> Result<(), DekuError> {
        let mut value = self.0.into();
        let mut bytes = Vec::new();

        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;

            if value != 0 {
                byte |= 0x80;
            }

            bytes.push(byte);

            if value == 0 {
                break;
            }
        }
        let data = BitVec::from_iter(bytes.as_bits::<Lsb0>().iter().rev());

        writer.write_bits_order(&data, Order::Lsb0)
    }
}

// ============================================================================
// BitAligned<T> - Fixed-size numeric types with bit-level alignment
// ============================================================================

/// Bit-stream wrapper for fixed-size numeric types.
///
/// Reads/writes values byte-by-byte at the bit level, matching SE's
/// `BitStream.ReadInternal(n)`. Unlike deku's default byte-aligned reads
/// or `#[deku(bits = N)]`, this correctly handles non-byte-aligned positions.
///
/// Transparent for serde/proto - serializes as the inner value.
#[derive(Copy, Clone, PartialEq, Default)]
pub struct BitAligned<T: Copy>(pub T);

impl<T: Copy> BitAligned<T> {
    pub fn new(value: T) -> Self {
        BitAligned(value)
    }
    
    pub fn into_inner(self) -> T {
        self.0
    }
    
    pub fn get(&self) -> T {
        self.0
    }
}

impl<T: Copy> std::ops::Deref for BitAligned<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Copy> std::ops::DerefMut for BitAligned<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: Copy> AsRef<T> for BitAligned<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: Debug + Copy> Debug for BitAligned<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Eq + Copy> Eq for BitAligned<T> {}

impl<T: std::hash::Hash + Copy> std::hash::Hash for BitAligned<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: PartialOrd + Copy> PartialOrd for BitAligned<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<T: Ord + Copy> Ord for BitAligned<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> From<T> for BitAligned<T>
where
    T: Copy,
{
    fn from(value: T) -> Self {
        BitAligned(value)
    }
}

// ---- Serde (transparent) ----

impl<T: Copy + Serialize> Serialize for BitAligned<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Copy + Deserialize<'de>> Deserialize<'de> for BitAligned<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(BitAligned)
    }
}

// ---- Deku ----

/// Trait for types that can be read/written as little-endian byte arrays.
trait LeBytes: Sized + Copy {
    const SIZE: usize;
    fn to_le_bytes_vec(self) -> Vec<u8>;
    fn from_le_bytes_slice(bytes: &[u8]) -> Self;
}

macro_rules! impl_le_bytes {
    ($t:ty, $n:expr) => {
        impl LeBytes for $t {
            const SIZE: usize = $n;
            fn to_le_bytes_vec(self) -> Vec<u8> {
                self.to_le_bytes().to_vec()
            }
            fn from_le_bytes_slice(bytes: &[u8]) -> Self {
                let mut arr = [0u8; $n];
                arr.copy_from_slice(bytes);
                Self::from_le_bytes(arr)
            }
        }
    };
}

impl_le_bytes!(u8, 1);
impl_le_bytes!(i8, 1);
impl_le_bytes!(u16, 2);
impl_le_bytes!(i16, 2);
impl_le_bytes!(i32, 4);
impl_le_bytes!(u32, 4);
impl_le_bytes!(f32, 4);
impl_le_bytes!(i64, 8);
impl_le_bytes!(u64, 8);
impl_le_bytes!(f64, 8);

impl<T: LeBytes> DekuReader<'_, ()> for BitAligned<T> {
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError> {
        let mut bytes = vec![0u8; T::SIZE];
        for b in bytes.iter_mut() {
            *b = reader.read_bits(8, Order::Lsb0)?.unwrap().load_le();
        }
        Ok(BitAligned(T::from_le_bytes_slice(&bytes)))
    }
}

impl<T: LeBytes> DekuWriter<()> for BitAligned<T> {
    fn to_writer<W: Write + Seek>(
        &self,
        writer: &mut Writer<W>,
        _ctx: (),
    ) -> Result<(), DekuError> {
        let bytes = self.0.to_le_bytes_vec();
        let data = BitVec::from_iter(bytes.as_bits::<Lsb0>().iter().rev());
        writer.write_bits_order(&data, Order::Lsb0)
    }
}

// ============================================================================
// VarBytes - Variable-length byte array
// ============================================================================

/// A variable-length byte array prefixed with a VLQ length.
///
/// Transparent for serde - serializes as Vec<u8>.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct VarBytes(pub Vec<u8>);

impl VarBytes {
    pub fn new(data: Vec<u8>) -> Self {
        VarBytes(data)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for VarBytes {
    fn from(value: Vec<u8>) -> Self {
        VarBytes(value)
    }
}

impl From<VarBytes> for Vec<u8> {
    fn from(value: VarBytes) -> Self {
        value.0
    }
}

impl std::ops::Deref for VarBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl std::ops::DerefMut for VarBytes {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

// ---- Serde (transparent) ----

impl Serialize for VarBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for VarBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<u8>::deserialize(deserializer).map(VarBytes)
    }
}

// ---- Deku ----

impl DekuReader<'_, ()> for VarBytes {
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError>
    where
        Self: Sized,
    {
        let length = Varint::<u32>::from_reader_with_ctx(reader, ())?.0 as usize;

        let mut buffer = vec![0u8; length];
        for slot in buffer.iter_mut() {
            *slot = reader.read_bits(8, Order::Lsb0)?.unwrap().load_le();
        }

        Ok(VarBytes(buffer))
    }
}

impl DekuWriter<()> for VarBytes {
    fn to_writer<W: Write + Seek>(&self, writer: &mut Writer<W>, _: ()) -> Result<(), DekuError> {
        Varint::from(self.0.len() as u32).to_writer(writer, ())?;

        let data = BitVec::from_iter(self.0.as_slice().as_bits::<Lsb0>().iter().rev());
        writer.write_bits_order(&data, Order::Lsb0)?;
        Ok(())
    }
}

// ============================================================================
// VarString - Variable-length UTF-8 string
// ============================================================================

/// A variable-length string prefixed with a VLQ length (UTF-8 encoded).
///
/// Transparent for serde - serializes as String.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct VarString(pub String);

impl VarString {
    pub fn new(s: String) -> Self {
        VarString(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl Debug for VarString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for VarString {
    fn from(value: String) -> Self {
        VarString(value)
    }
}

impl From<&str> for VarString {
    fn from(value: &str) -> Self {
        VarString(value.to_string())
    }
}

impl From<VarString> for String {
    fn from(value: VarString) -> Self {
        value.0
    }
}

impl std::ops::Deref for VarString {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::ops::DerefMut for VarString {
    fn deref_mut(&mut self) -> &mut str {
        &mut self.0
    }
}

// ---- Serde (transparent) ----

impl Serialize for VarString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for VarString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(VarString)
    }
}

// ---- Deku ----

impl DekuReader<'_, ()> for VarString {
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<VarString, DekuError> {
        let length = Varint::<u32>::from_reader_with_ctx(reader, ())?.0 as usize;
        let mut buffer = vec![0u8; length];
        for slot in buffer.iter_mut() {
            *slot = reader.read_bits(8, Order::Lsb0)?.unwrap().load_le();
        }
        let str = String::from_utf8(buffer);

        match str {
            Ok(str) => Ok(VarString(str)),
            Err(e) => Err(DekuError::Parse(Cow::from(e.to_string()))),
        }
    }
}

impl DekuWriter<()> for VarString {
    fn to_writer<W: Write + Seek>(
        &self,
        writer: &mut Writer<W>,
        _ctx: (),
    ) -> Result<(), DekuError> {
        Varint::from(self.0.len() as u64).to_writer(writer, ())?;

        let data = BitVec::from_iter(self.0.as_bytes().as_bits::<Lsb0>().iter().rev());
        writer.write_bits_order(&data, Order::Lsb0)?;
        Ok(())
    }
}

// ============================================================================
// Proto-rs trait implementations (transparent wrappers)
// ============================================================================

// ---- Varint<T> ----

#[doc(hidden)]
pub struct VarintShadow<S>(S);

impl<S: ProtoExt> ProtoExt for VarintShadow<S> {
    const KIND: ProtoKind = S::KIND;
}

impl<S: ProtoArchive> ProtoArchive for VarintShadow<S> {
    #[inline]
    fn is_default(&self) -> bool {
        self.0.is_default()
    }
    #[inline]
    fn archive<const TAG: u32>(&self, w: &mut impl RevWriter) {
        self.0.archive::<TAG>(w);
    }
}

impl<'a, T> ProtoShadowEncode<'a, Varint<T>> for VarintShadow<<T as ProtoEncode>::Shadow<'a>>
where
    T: ProtoEncode + Copy,
{
    #[inline]
    fn from_sun(value: &'a Varint<T>) -> Self {
        VarintShadow(<T as ProtoEncode>::Shadow::from_sun(&value.0))
    }
}

impl<T: ProtoExt + Copy> ProtoExt for Varint<T> {
    const KIND: ProtoKind = T::KIND;
}

impl<T> ProtoEncode for Varint<T>
where
    T: ProtoEncode + Copy,
    for<'a> <T as ProtoEncode>::Shadow<'a>: ProtoArchive + ProtoExt,
{
    type Shadow<'a> = VarintShadow<<T as ProtoEncode>::Shadow<'a>>;
}

impl<T: ProtoDefault + Copy> ProtoDefault for Varint<T> {
    #[inline]
    fn proto_default() -> Self {
        Varint(T::proto_default())
    }
}

impl<T: Copy> ProtoShadowDecode<Varint<T>> for Varint<T> {
    #[inline]
    fn to_sun(self) -> Result<Varint<T>, DecodeError> {
        Ok(self)
    }
}

impl<T: ProtoDecoder + Copy> ProtoDecoder for Varint<T> {
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

// ---- BitAligned<T> ----

#[doc(hidden)]
pub struct BitAlignedShadow<S>(S);

impl<S: ProtoExt> ProtoExt for BitAlignedShadow<S> {
    const KIND: ProtoKind = S::KIND;
}

impl<S: ProtoArchive> ProtoArchive for BitAlignedShadow<S> {
    #[inline]
    fn is_default(&self) -> bool {
        self.0.is_default()
    }
    #[inline]
    fn archive<const TAG: u32>(&self, w: &mut impl RevWriter) {
        self.0.archive::<TAG>(w);
    }
}

impl<'a, T> ProtoShadowEncode<'a, BitAligned<T>> for BitAlignedShadow<<T as ProtoEncode>::Shadow<'a>>
where
    T: ProtoEncode + Copy,
{
    #[inline]
    fn from_sun(value: &'a BitAligned<T>) -> Self {
        BitAlignedShadow(<T as ProtoEncode>::Shadow::from_sun(&value.0))
    }
}

impl<T: ProtoExt + Copy> ProtoExt for BitAligned<T> {
    const KIND: ProtoKind = T::KIND;
}

impl<T> ProtoEncode for BitAligned<T>
where
    T: ProtoEncode + Copy,
    for<'a> <T as ProtoEncode>::Shadow<'a>: ProtoArchive + ProtoExt,
{
    type Shadow<'a> = BitAlignedShadow<<T as ProtoEncode>::Shadow<'a>>;
}

// Direct ProtoArchive impl for BitAligned - needed for Vec<BitAligned<T>> to work
impl<T: ProtoArchive + Copy> ProtoArchive for BitAligned<T> {
    #[inline]
    fn is_default(&self) -> bool {
        self.0.is_default()
    }
    #[inline]
    fn archive<const TAG: u32>(&self, w: &mut impl RevWriter) {
        self.0.archive::<TAG>(w);
    }
}

impl<T: ProtoDefault + Copy> ProtoDefault for BitAligned<T> {
    #[inline]
    fn proto_default() -> Self {
        BitAligned(T::proto_default())
    }
}

impl<T: Copy> ProtoShadowDecode<BitAligned<T>> for BitAligned<T> {
    #[inline]
    fn to_sun(self) -> Result<BitAligned<T>, DecodeError> {
        Ok(self)
    }
}

impl<T: ProtoDecoder + Copy> ProtoDecoder for BitAligned<T> {
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

// ---- VarBytes ----

#[doc(hidden)]
pub struct VarBytesShadow<'a>(<Vec<u8> as ProtoEncode>::Shadow<'a>);

impl ProtoExt for VarBytesShadow<'_> {
    const KIND: ProtoKind = <Vec<u8> as ProtoExt>::KIND;
}

impl ProtoArchive for VarBytesShadow<'_> {
    #[inline]
    fn is_default(&self) -> bool {
        self.0.is_default()
    }
    #[inline]
    fn archive<const TAG: u32>(&self, w: &mut impl RevWriter) {
        self.0.archive::<TAG>(w);
    }
}

impl<'a> ProtoShadowEncode<'a, VarBytes> for VarBytesShadow<'a> {
    #[inline]
    fn from_sun(value: &'a VarBytes) -> Self {
        VarBytesShadow(<Vec<u8> as ProtoEncode>::Shadow::from_sun(&value.0))
    }
}

impl ProtoExt for VarBytes {
    const KIND: ProtoKind = <Vec<u8> as ProtoExt>::KIND;
}

impl ProtoEncode for VarBytes {
    type Shadow<'a> = VarBytesShadow<'a>;
}

impl ProtoDefault for VarBytes {
    #[inline]
    fn proto_default() -> Self {
        VarBytes(Vec::new())
    }
}

impl ProtoShadowDecode<VarBytes> for VarBytes {
    #[inline]
    fn to_sun(self) -> Result<VarBytes, DecodeError> {
        Ok(self)
    }
}

impl ProtoDecoder for VarBytes {
    #[inline]
    fn merge_field(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        Vec::<u8>::merge_field(&mut value.0, tag, wire_type, buf, ctx)
    }

    #[inline]
    fn merge(
        &mut self,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        Vec::<u8>::merge(&mut self.0, wire_type, buf, ctx)
    }
}

// ---- VarString ----

#[doc(hidden)]
pub struct VarStringShadow<'a>(<String as ProtoEncode>::Shadow<'a>);

impl ProtoExt for VarStringShadow<'_> {
    const KIND: ProtoKind = <String as ProtoExt>::KIND;
}

impl ProtoArchive for VarStringShadow<'_> {
    #[inline]
    fn is_default(&self) -> bool {
        self.0.is_default()
    }
    #[inline]
    fn archive<const TAG: u32>(&self, w: &mut impl RevWriter) {
        self.0.archive::<TAG>(w);
    }
}

impl<'a> ProtoShadowEncode<'a, VarString> for VarStringShadow<'a> {
    #[inline]
    fn from_sun(value: &'a VarString) -> Self {
        VarStringShadow(<String as ProtoEncode>::Shadow::from_sun(&value.0))
    }
}

impl ProtoExt for VarString {
    const KIND: ProtoKind = <String as ProtoExt>::KIND;
}

impl ProtoEncode for VarString {
    type Shadow<'a> = VarStringShadow<'a>;
}

impl ProtoDefault for VarString {
    #[inline]
    fn proto_default() -> Self {
        VarString(String::new())
    }
}

impl ProtoShadowDecode<VarString> for VarString {
    #[inline]
    fn to_sun(self) -> Result<VarString, DecodeError> {
        Ok(self)
    }
}

impl ProtoDecoder for VarString {
    #[inline]
    fn merge_field(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        String::merge_field(&mut value.0, tag, wire_type, buf, ctx)
    }

    #[inline]
    fn merge(
        &mut self,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        String::merge(&mut self.0, wire_type, buf, ctx)
    }
}

// ============================================================================
// BitBool - Single-bit boolean for SE's bitstream protocol
// ============================================================================

/// Single-bit boolean for Space Engineers' bitstream protocol.
///
/// Unlike Rust/Deku's default bool which reads 1 byte, SE's `BitStream.ReadBool()`
/// reads exactly 1 bit via `ReadInternal(1)`.
///
/// Transparent for serde - serializes as bool.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct BitBool(pub bool);

impl BitBool {
    pub fn new(value: bool) -> Self {
        BitBool(value)
    }
    
    /// Returns the inner bool value. Useful in deku conditions to avoid double-deref.
    pub fn get(&self) -> bool {
        self.0
    }
}

impl std::ops::Deref for BitBool {
    type Target = bool;
    fn deref(&self) -> &bool {
        &self.0
    }
}

impl std::ops::DerefMut for BitBool {
    fn deref_mut(&mut self) -> &mut bool {
        &mut self.0
    }
}

impl Debug for BitBool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<bool> for BitBool {
    fn from(value: bool) -> Self {
        BitBool(value)
    }
}

impl From<BitBool> for bool {
    fn from(value: BitBool) -> Self {
        value.0
    }
}

impl std::ops::Not for BitBool {
    type Output = bool;
    fn not(self) -> bool {
        !self.0
    }
}

impl PartialEq<bool> for BitBool {
    fn eq(&self, other: &bool) -> bool {
        self.0 == *other
    }
}

// ---- Serde (transparent) ----

impl Serialize for BitBool {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BitBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        bool::deserialize(deserializer).map(BitBool)
    }
}

// ---- Deku (single bit) ----

impl DekuReader<'_, ()> for BitBool {
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError>
    where
        Self: Sized,
    {
        let bits = reader.read_bits(1, Order::Lsb0)?.unwrap();
        Ok(BitBool(bits.load::<u8>() != 0))
    }
}

impl DekuWriter<()> for BitBool {
    fn to_writer<W: Write + Seek>(&self, writer: &mut Writer<W>, _ctx: ()) -> Result<(), DekuError> {
        let mut entry = BitVec::<u8, deku::bitvec::Msb0>::with_capacity(1);
        entry.push(self.0);
        writer.write_bits_order(&entry, Order::Lsb0)
    }
}

// ---- BitBool ProtoShadow ----

#[doc(hidden)]
pub struct BitBoolShadow(<bool as ProtoEncode>::Shadow<'static>);

impl ProtoExt for BitBoolShadow {
    const KIND: ProtoKind = <bool as ProtoExt>::KIND;
}

impl ProtoArchive for BitBoolShadow {
    #[inline]
    fn is_default(&self) -> bool {
        self.0.is_default()
    }
    #[inline]
    fn archive<const TAG: u32>(&self, w: &mut impl RevWriter) {
        self.0.archive::<TAG>(w);
    }
}

impl<'a> ProtoShadowEncode<'a, BitBool> for BitBoolShadow {
    #[inline]
    fn from_sun(value: &'a BitBool) -> Self {
        BitBoolShadow(<bool as ProtoEncode>::Shadow::from_sun(&value.0))
    }
}

impl ProtoExt for BitBool {
    const KIND: ProtoKind = <bool as ProtoExt>::KIND;
}

impl ProtoEncode for BitBool {
    type Shadow<'a> = BitBoolShadow;
}

impl ProtoArchive for BitBool {
    #[inline]
    fn is_default(&self) -> bool {
        !self.0
    }
    #[inline]
    fn archive<const TAG: u32>(&self, w: &mut impl RevWriter) {
        self.0.archive::<TAG>(w);
    }
}

impl ProtoDefault for BitBool {
    #[inline]
    fn proto_default() -> Self {
        BitBool(false)
    }
}

impl ProtoShadowDecode<BitBool> for BitBool {
    #[inline]
    fn to_sun(self) -> Result<BitBool, DecodeError> {
        Ok(self)
    }
}

impl ProtoDecoder for BitBool {
    #[inline]
    fn merge_field(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        bool::merge_field(&mut value.0, tag, wire_type, buf, ctx)
    }

    #[inline]
    fn merge(
        &mut self,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        bool::merge(&mut self.0, wire_type, buf, ctx)
    }
}

// ============================================================================
// VarVec<T> - Variable-length array with VarInt length prefix
// ============================================================================

/// Variable-length array with VarInt length prefix.
///
/// SE's network protocol serializes arrays as: VarInt(count) + count × element.
/// This wrapper provides that behavior for Deku while being transparent to serde.
///
/// Transparent for serde - serializes as Vec<T>.
#[derive(Clone, PartialEq, Default)]
pub struct VarVec<T>(pub Vec<T>);

impl<T> VarVec<T> {
    pub fn new(items: Vec<T>) -> Self {
        VarVec(items)
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<T> {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T: Debug> Debug for VarVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> From<Vec<T>> for VarVec<T> {
    fn from(value: Vec<T>) -> Self {
        VarVec(value)
    }
}

impl<T> From<VarVec<T>> for Vec<T> {
    fn from(value: VarVec<T>) -> Self {
        value.0
    }
}

impl<T> std::ops::Deref for VarVec<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for VarVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// ---- Serde (transparent) ----

impl<T: Serialize> Serialize for VarVec<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for VarVec<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Use xml_vec-style deserialization to handle self-closing XML elements
        // like `<Stations />` which quick_xml presents as empty text content.
        crate::xml::xml_vec::deserialize(deserializer).map(VarVec)
    }
}

// ---- Deku ----

impl<'a, T> DekuReader<'a, ()> for VarVec<T>
where
    T: DekuReader<'a, ()>,
{
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError>
    where
        Self: Sized,
    {
        let length = Varint::<u32>::from_reader_with_ctx(reader, ())?.0 as usize;
        let mut items = Vec::with_capacity(length);
        for _ in 0..length {
            items.push(T::from_reader_with_ctx(reader, ())?);
        }
        Ok(VarVec(items))
    }
}

impl<T> DekuWriter<()> for VarVec<T>
where
    T: DekuWriter<()>,
{
    fn to_writer<W: Write + Seek>(&self, writer: &mut Writer<W>, _ctx: ()) -> Result<(), DekuError> {
        Varint::from(self.0.len() as u32).to_writer(writer, ())?;
        for item in &self.0 {
            item.to_writer(writer, ())?;
        }
        Ok(())
    }
}

// ---- Proto-rs (transparent via Vec<T>) ----

impl<T: ProtoExt> ProtoExt for VarVec<T> {
    const KIND: ProtoKind = <Vec<T> as ProtoExt>::KIND;
}

impl<T: 'static> ProtoEncode for VarVec<T>
where
    T: ProtoEncode + ProtoExt + ProtoArchive,
    for<'a> <T as ProtoEncode>::Shadow<'a>: ProtoArchive + ProtoExt,
    for<'a> &'a [T]: ProtoExt + ProtoArchive,
{
    type Shadow<'a> = <Vec<T> as ProtoEncode>::Shadow<'a>;
}

impl<'a, T: 'static> ProtoShadowEncode<'a, VarVec<T>> for <Vec<T> as ProtoEncode>::Shadow<'a>
where
    T: ProtoEncode + ProtoExt + ProtoArchive,
    for<'b> <T as ProtoEncode>::Shadow<'b>: ProtoArchive + ProtoExt,
    for<'b> &'b [T]: ProtoExt + ProtoArchive,
{
    #[inline]
    fn from_sun(value: &'a VarVec<T>) -> Self {
        <Vec<T> as ProtoEncode>::Shadow::from_sun(&value.0)
    }
}

impl<T: ProtoDefault> ProtoDefault for VarVec<T> {
    #[inline]
    fn proto_default() -> Self {
        VarVec(Vec::proto_default())
    }
}

impl<T> ProtoShadowDecode<VarVec<T>> for VarVec<T> {
    #[inline]
    fn to_sun(self) -> Result<VarVec<T>, DecodeError> {
        Ok(self)
    }
}

impl<T: ProtoDecoder + ProtoDefault> ProtoDecoder for VarVec<T> {
    #[inline]
    fn merge_field(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        Vec::<T>::merge_field(&mut value.0, tag, wire_type, buf, ctx)
    }

    #[inline]
    fn merge(
        &mut self,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        Vec::<T>::merge(&mut self.0, wire_type, buf, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_default() {
        let v: Varint<u32> = Varint::default();
        assert_eq!(v.0, 0);
    }

    #[test]
    fn test_varint_new() {
        let v = Varint::new(42u32);
        assert_eq!(v.0, 42);
    }

    #[test]
    fn test_bit_aligned_deref() {
        let b = BitAligned(42i32);
        assert_eq!(*b, 42);
    }

    #[test]
    fn test_bit_aligned_from() {
        let b: BitAligned<i32> = 42.into();
        assert_eq!(*b, 42);
    }

    #[test]
    fn test_var_string_from() {
        let s = VarString::from("hello");
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn test_var_string_into_string() {
        let s = VarString::from("hello");
        let owned: String = s.into_string();
        assert_eq!(owned, "hello");
    }

    #[test]
    fn test_var_bytes_from() {
        let b = VarBytes::from(vec![1, 2, 3]);
        assert_eq!(b.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_var_bytes_into_vec() {
        let b = VarBytes::from(vec![1, 2, 3]);
        let owned: Vec<u8> = b.into();
        assert_eq!(owned, vec![1, 2, 3]);
    }
}
