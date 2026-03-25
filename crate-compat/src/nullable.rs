//! Nullable wrapper for C# nullable value types.
//!
//! In C#, `Nullable<T>` (or `T?`) represents a value type that can be null.
//! This wrapper provides similar semantics for Rust with serde/proto/deku support.

use deku::bitvec::{BitField as _, BitVec, Msb0};
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
use std::io::{Read, Seek, Write};

// ============================================================================
// Nullable<T>
// ============================================================================

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
        assert!(
            self.has_value(),
            "called `Nullable::unwrap_mut()` on a null value"
        );
        &mut self.0
    }

    pub fn unwrap(self) -> T {
        assert!(
            self.has_value(),
            "called `Nullable::unwrap()` on a null value"
        );
        self.0
    }
}

// ---- Serde ----

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

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de2>,
            {
                while map.next_entry::<de::IgnoredAny, de::IgnoredAny>()?.is_some() {}
                Ok(Nullable(U::default()))
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
                if s.is_empty() {
                    Ok(Nullable(U::default()))
                } else {
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
                U::deserialize(deserializer).map(Nullable)
            }

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
                if let Some(value) = seq.next_element::<U>()? {
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

// ---- Deku ----

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

// ---- Proto-rs ----

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nullable_has_value() {
        let n: Nullable<i32> = Nullable(42);
        assert!(n.has_value());

        let empty: Nullable<i32> = Nullable::default();
        assert!(!empty.has_value());
    }

    #[test]
    fn test_nullable_unwrap() {
        let n: Nullable<i32> = Nullable(42);
        assert_eq!(n.unwrap(), 42);
    }
}
