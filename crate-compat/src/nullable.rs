//! Nullable wrapper for C# nullable value types.
//!
//! In C#, `Nullable<T>` (or `T?`) represents a value type that can be null.
//! This wrapper provides similar semantics for Rust with serde/proto/deku support.
//!
//! Internally uses `Option<T>` for proper null semantics.

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

/// A nullable value type, similar to C#'s `Nullable<T>` or `T?`.
///
/// Internally uses `Option<T>` for proper null semantics. Provides `Deref` to
/// `Option<T>` so all `Option` methods are available.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Nullable<T>(pub Option<T>);

impl<T> Nullable<T> {
    /// Creates a `Nullable` containing the given value.
    pub fn some(value: T) -> Self {
        Nullable(Some(value))
    }

    /// Creates an empty `Nullable`.
    pub fn none() -> Self {
        Nullable(None)
    }

    /// Returns `true` if the nullable contains a value.
    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    /// Returns `true` if the nullable is empty.
    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }
}

impl<T> std::ops::Deref for Nullable<T> {
    type Target = Option<T>;
    fn deref(&self) -> &Option<T> {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Nullable<T> {
    fn deref_mut(&mut self) -> &mut Option<T> {
        &mut self.0
    }
}

impl<T> From<T> for Nullable<T> {
    fn from(value: T) -> Self {
        Nullable(Some(value))
    }
}

impl<T> From<Option<T>> for Nullable<T> {
    fn from(opt: Option<T>) -> Self {
        Nullable(opt)
    }
}

// ---- Serde ----

impl<T: Serialize> Serialize for Nullable<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match &self.0 {
            Some(value) => serializer.serialize_some(value),
            None => serializer.serialize_none(),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Nullable<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        use std::fmt;
        use std::marker::PhantomData;

        struct NullableVisitor<U>(PhantomData<U>);

        impl<'de2, U: Deserialize<'de2>> Visitor<'de2> for NullableVisitor<U> {
            type Value = Nullable<U>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a nullable value, xsi:nil element, or empty element")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de2>,
            {
                while map.next_entry::<de::IgnoredAny, de::IgnoredAny>()?.is_some() {}
                Ok(Nullable::none())
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
                if s.is_empty() {
                    Ok(Nullable::none())
                } else {
                    use serde::de::IntoDeserializer;
                    U::deserialize(s.into_deserializer()).map(Nullable::some)
                }
            }

            fn visit_string<E: de::Error>(self, s: String) -> Result<Self::Value, E> {
                self.visit_str(&s)
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Nullable::none())
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Nullable::none())
            }

            fn visit_some<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
            where
                D2: serde::Deserializer<'de2>,
            {
                U::deserialize(deserializer).map(Nullable::some)
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                use serde::de::IntoDeserializer;
                U::deserialize(v.into_deserializer()).map(Nullable::some)
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                use serde::de::IntoDeserializer;
                U::deserialize(v.into_deserializer()).map(Nullable::some)
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                use serde::de::IntoDeserializer;
                U::deserialize(v.into_deserializer()).map(Nullable::some)
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                use serde::de::IntoDeserializer;
                U::deserialize(v.into_deserializer()).map(Nullable::some)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de2>,
            {
                if let Some(value) = seq.next_element::<U>()? {
                    while seq.next_element::<de::IgnoredAny>()?.is_some() {}
                    Ok(Nullable::some(value))
                } else {
                    Ok(Nullable::none())
                }
            }
        }

        deserializer.deserialize_any(NullableVisitor(PhantomData))
    }
}

// ---- Deku ----

impl<T> DekuReader<'_, ()> for Nullable<T>
where
    T: for<'a> DekuReader<'a> + DekuWriter,
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
            Ok(Nullable::some(T::from_reader_with_ctx(reader, ctx)?))
        } else {
            Ok(Nullable::none())
        }
    }
}

impl<T> DekuWriter<()> for Nullable<T>
where
    T: for<'a> DekuReader<'a> + DekuWriter,
{
    fn to_writer<W: Write + Seek>(&self, writer: &mut Writer<W>, ctx: ()) -> Result<(), DekuError> {
        let mut entry = BitVec::<u8, Msb0>::with_capacity(1);
        entry.push(self.is_some());
        writer.write_bits_order(&entry, Order::Lsb0)?;
        if let Some(value) = &self.0 {
            value.to_writer(writer, ctx)?;
        }
        Ok(())
    }
}

// ---- Proto-rs ----

#[doc(hidden)]
pub struct NullableShadow<S>(Option<S>);

impl<S: ProtoExt> ProtoExt for NullableShadow<S> {
    const KIND: ProtoKind = S::KIND;
}

impl<S: ProtoArchive> ProtoArchive for NullableShadow<S> {
    #[inline]
    fn is_default(&self) -> bool {
        match &self.0 {
            Some(inner) => inner.is_default(),
            None => true,
        }
    }

    #[inline]
    fn archive<const TAG: u32>(&self, w: &mut impl RevWriter) {
        if let Some(inner) = &self.0 {
            inner.archive::<TAG>(w);
        }
    }
}

impl<'a, T> ProtoShadowEncode<'a, Nullable<T>> for NullableShadow<<T as ProtoEncode>::Shadow<'a>>
where
    T: ProtoEncode,
{
    #[inline]
    fn from_sun(value: &'a Nullable<T>) -> Self {
        NullableShadow(value.0.as_ref().map(|v| <T as ProtoEncode>::Shadow::from_sun(v)))
    }
}

impl<T> ProtoExt for Nullable<T>
where
    T: ProtoExt,
{
    const KIND: ProtoKind = T::KIND;
}

impl<T> ProtoEncode for Nullable<T>
where
    T: ProtoEncode,
    for<'a> <T as ProtoEncode>::Shadow<'a>: ProtoArchive + ProtoExt,
{
    type Shadow<'a> = NullableShadow<<T as ProtoEncode>::Shadow<'a>>;
}

impl<T> ProtoDefault for Nullable<T> {
    #[inline]
    fn proto_default() -> Self {
        Nullable::none()
    }
}

impl<T> ProtoShadowDecode<Nullable<T>> for Nullable<T> {
    #[inline]
    fn to_sun(self) -> Result<Nullable<T>, DecodeError> {
        Ok(self)
    }
}

impl<T> ProtoDecoder for Nullable<T>
where
    T: ProtoDecoder + Default,
{
    #[inline]
    fn merge_field(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        // Ensure we have a value to merge into
        let inner = value.0.get_or_insert_with(T::default);
        T::merge_field(inner, tag, wire_type, buf, ctx)
    }

    #[inline]
    fn merge(
        &mut self,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        // Ensure we have a value to merge into
        let inner = self.0.get_or_insert_with(T::default);
        T::merge(inner, wire_type, buf, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nullable_is_some() {
        let n: Nullable<i32> = Nullable::some(42);
        assert!(n.is_some());

        let empty: Nullable<i32> = Nullable::none();
        assert!(empty.is_none());
        
        let default: Nullable<i32> = Nullable::default();
        assert!(default.is_none());
    }

    #[test]
    fn test_nullable_unwrap() {
        let n: Nullable<i32> = Nullable::some(42);
        assert_eq!(n.unwrap(), 42);
    }
    
    #[test]
    fn test_nullable_from() {
        let n: Nullable<i32> = 42.into();
        assert!(n.is_some());
        assert_eq!(n.unwrap(), 42);
        
        let opt: Nullable<i32> = Some(42).into();
        assert_eq!(opt.unwrap(), 42);
        
        let none: Nullable<i32> = None.into();
        assert!(none.is_none());
    }
    
    #[test]
    fn test_nullable_deref() {
        let n: Nullable<i32> = Nullable::some(42);
        // Can use Option methods via Deref
        assert_eq!(n.map(|x| x * 2), Some(84));
        assert_eq!(n.unwrap_or(0), 42);
    }
}
