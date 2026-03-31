//! Space Engineers collection types.
//!
//! Contains VarMap and Tuple types used in SE serialization.

use deku::prelude::{Reader, Writer};
use deku::{DekuError, DekuReader, DekuWriter};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use std::io::{Read, Seek, Write};

use crate::deku::Varint;

// ============================================================================
// VarMap<K, V> - Length-prefixed key-value dictionary
// ============================================================================

/// A dictionary/map wrapper for Space Engineers serialization.
///
/// Binary format: `[VarInt count][Key₁][Value₁][Key₂][Value₂]...[Keyₙ][Valueₙ]`
///
/// This is the Deku-compatible wrapper for `HashMap<K, V>`, similar to how
/// `VarVec<T>` wraps `Vec<T>` and `VarString` wraps `String`.
#[derive(Debug, Clone, PartialEq)]
#[proto_rs::proto_message]
pub struct VarMap<K: Hash + Eq, V>(#[proto(tag = 1)] pub HashMap<K, V>);

impl<K: Hash + Eq, V> Default for VarMap<K, V> {
    fn default() -> Self {
        VarMap(HashMap::new())
    }
}

impl<K: Hash + Eq, V> VarMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<K: Hash + Eq, V> std::ops::Deref for VarMap<K, V> {
    type Target = HashMap<K, V>;
    fn deref(&self) -> &HashMap<K, V> {
        &self.0
    }
}

impl<K: Hash + Eq, V> std::ops::DerefMut for VarMap<K, V> {
    fn deref_mut(&mut self) -> &mut HashMap<K, V> {
        &mut self.0
    }
}

impl<K: Hash + Eq, V> From<HashMap<K, V>> for VarMap<K, V> {
    fn from(map: HashMap<K, V>) -> Self {
        VarMap(map)
    }
}

impl<K: Hash + Eq, V> From<VarMap<K, V>> for HashMap<K, V> {
    fn from(var_map: VarMap<K, V>) -> Self {
        var_map.0
    }
}

impl<K: Hash + Eq + Serialize, V: Serialize> Serialize for VarMap<K, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename = "item")]
        struct VarMapEntryRef<'a, T, U> {
            #[serde(rename = "Key")]
            k: &'a T,
            #[serde(rename = "Value")]
            v: &'a U,
        }

        let mut state = serializer.serialize_struct("SerializableDictionary", 1)?;
        let entries_iter = self
            .0
            .iter()
            .map(|(k, v)| VarMapEntryRef { k, v });
        let entries: Vec<_> = entries_iter.collect();
        SerializeStruct::serialize_field(&mut state, "dictionary", &entries)?;
        SerializeStruct::end(state)
    }
}

impl<'de, K: Hash + Eq + Deserialize<'de>, V: Deserialize<'de>> Deserialize<'de>
    for VarMap<K, V>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename = "item")]
        struct VarMapEntry<T, U> {
            #[serde(rename = "Key")]
            k: T,
            #[serde(rename = "Value")]
            v: U,
        }

        #[allow(clippy::unnecessary_wraps)]
        fn deserialize_entries<'de, T: Deserialize<'de>, U: Deserialize<'de>, D>(
            deserializer: D,
        ) -> Result<Vec<VarMapEntry<T, U>>, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(Vec::<VarMapEntry<T, U>>::deserialize(deserializer)
                .unwrap_or_default())
        }

        fn empty_vec<T>() -> Vec<T> {
            Vec::new()
        }

        #[derive(Deserialize)]
        #[serde(rename = "Dictionary")]
        #[serde(bound(deserialize = "T: Deserialize<'de>, U: Deserialize<'de>"))]
        struct Helper<T, U> {
            #[serde(
                rename = "dictionary",
                default = "empty_vec",
                deserialize_with = "deserialize_entries"
            )]
            items: Vec<VarMapEntry<T, U>>,
        }

        let helper = Helper::deserialize(deserializer)?;
        let map = helper
            .items
            .into_iter()
            .map(|entry| (entry.k, entry.v))
            .collect();
        Ok(VarMap(map))
    }
}

// ============================================================================
// Tuple<K, V>
// ============================================================================

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
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

impl<K, V> Tuple<K, V> {
    pub fn new(item1: K, item2: V) -> Self {
        Tuple { item1, item2 }
    }
}

impl<K, V> From<(K, V)> for Tuple<K, V> {
    fn from((item1, item2): (K, V)) -> Self {
        Tuple { item1, item2 }
    }
}

impl<K, V> From<Tuple<K, V>> for (K, V) {
    fn from(tuple: Tuple<K, V>) -> Self {
        (tuple.item1, tuple.item2)
    }
}

// ---- Deku implementations ----

impl<K, V> DekuReader<'_, ()> for VarMap<K, V>
where
    K: Hash + Eq + for<'a> DekuReader<'a, ()>,
    V: for<'a> DekuReader<'a, ()>,
{
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError> {
        let len = Varint::<u32>::from_reader_with_ctx(reader, ())?.0 as usize;
        let mut map = HashMap::with_capacity(len);
        for _ in 0..len {
            let key = K::from_reader_with_ctx(reader, ())?;
            let value = V::from_reader_with_ctx(reader, ())?;
            map.insert(key, value);
        }
        Ok(VarMap(map))
    }
}

impl<K, V> DekuWriter<()> for VarMap<K, V>
where
    K: Hash + Eq + DekuWriter<()>,
    V: DekuWriter<()>,
{
    fn to_writer<W: Write + Seek>(
        &self,
        writer: &mut Writer<W>,
        _ctx: (),
    ) -> Result<(), DekuError> {
        Varint(self.0.len() as u32).to_writer(writer, ())?;
        for (key, value) in &self.0 {
            key.to_writer(writer, ())?;
            value.to_writer(writer, ())?;
        }
        Ok(())
    }
}

impl<K, V> DekuReader<'_, ()> for Tuple<K, V>
where
    K: for<'a> DekuReader<'a, ()>,
    V: for<'a> DekuReader<'a, ()>,
{
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError> {
        let item1 = K::from_reader_with_ctx(reader, ())?;
        let item2 = V::from_reader_with_ctx(reader, ())?;
        Ok(Tuple { item1, item2 })
    }
}

impl<K, V> DekuWriter<()> for Tuple<K, V>
where
    K: DekuWriter<()>,
    V: DekuWriter<()>,
{
    fn to_writer<W: Write + Seek>(
        &self,
        writer: &mut Writer<W>,
        _ctx: (),
    ) -> Result<(), DekuError> {
        self.item1.to_writer(writer, ())?;
        self.item2.to_writer(writer, ())?;
        Ok(())
    }
}
