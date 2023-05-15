use serde::{de, forward_to_deserialize_any, ser};

/// Adapter for key-value pairs in Redis.
///
/// Commonly, Redis will express a set of key-value pairs as a flattened array
/// of the keys and values. For instance, `HGETALL` returns something
/// resembling `["key1", "value1", "key2", "value2"]`. [`KeyValuePairs`] allows
/// for rust maps and structs to be adapted in this way; a map or struct type
/// wrapped in `KeyValuePairs` will serialize to, and deserialize from, a
/// flattened array of key-value pairs.
///
/// # Example
///
/// ```
/// use serde::{Serialize, Deserialize};
/// use seredies::{de::Deserializer, ser::Serializer, components::KeyValuePairs};
///
/// let mut buffer: Vec<u8> = Vec::new();
///
/// let data = ["key1", "value1", "key2", "value2"];
/// data.serialize(Serializer::new(&mut buffer)).unwrap();
///
/// #[derive(Deserialize)]
/// struct Data {
///     key1: String,
///     key2: String,
/// }
///
/// let mut buffer = buffer.as_slice();
/// let deserializer = Deserializer::new(&mut buffer);
/// let KeyValuePairs(Data{key1, key2}) = Deserialize::deserialize(deserializer)
///     .expect("failed to deserialize");
///
/// assert_eq!(key1, "value1");
/// assert_eq!(key2, "value2");
/// ```
#[derive(Debug, Copy, Clone, Default)]
pub struct KeyValuePairs<T>(pub T);

impl<T> From<T> for KeyValuePairs<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T: ser::Serialize> ser::Serialize for KeyValuePairs<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(KeyValuePairsAdapter(serializer))
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct KeyValuePairsAdapter<T>(T);

impl<T> KeyValuePairsAdapter<T> {
    fn non_collection_serialize_error<O, E: ser::Error>(&self) -> Result<O, E> {
        Err(E::custom(
            "KeyValuePairsAdapter must deserialize a struct or map",
        ))
    }
}

fn double_len<E: ser::Error>(len: usize) -> Result<usize, E> {
    len.checked_mul(2)
        .ok_or_else(|| E::custom("overflowed a usize"))
}

impl<S: ser::Serializer> ser::Serializer for KeyValuePairsAdapter<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    type SerializeSeq = ser::Impossible<S::Ok, S::Error>;
    type SerializeTuple = ser::Impossible<S::Ok, S::Error>;

    type SerializeMap = KeyValuePairsAdapter<S::SerializeSeq>;
    type SerializeStruct = KeyValuePairsAdapter<S::SerializeTuple>;

    type SerializeTupleStruct = ser::Impossible<S::Ok, S::Error>;
    type SerializeTupleVariant = ser::Impossible<S::Ok, S::Error>;
    type SerializeStructVariant = ser::Impossible<S::Ok, S::Error>;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        self.non_collection_serialize_error()
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        self.0.serialize_newtype_struct(name, &KeyValuePairs(value))
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        self.non_collection_serialize_error()
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.non_collection_serialize_error()
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.0
            .serialize_seq(len.map(double_len).transpose()?)
            .map(KeyValuePairsAdapter)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.0
            .serialize_tuple(double_len(len)?)
            .map(KeyValuePairsAdapter)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.non_collection_serialize_error()
    }
}

impl<S: ser::SerializeSeq> ser::SerializeMap for KeyValuePairsAdapter<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.0.serialize_element(key)
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.0.serialize_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<S: ser::SerializeTuple> ser::SerializeStruct for KeyValuePairsAdapter<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.0.serialize_element(key)?;
        self.0.serialize_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<'de, T> de::Deserialize<'de> for KeyValuePairs<T>
where
    T: de::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(KeyValuePairsAdapter(deserializer)).map(Self)
    }
}

impl<'de, D> de::Deserializer<'de> for KeyValuePairsAdapter<D>
where
    D: de::Deserializer<'de>,
{
    type Error = D::Error;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit_struct seq tuple unit option enum
        tuple_struct identifier ignored_any
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_any(KeyValuePairsAdapter(visitor))
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_seq(KeyValuePairsAdapter(visitor))
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_tuple_struct(
            name,
            fields
                .len()
                .checked_mul(2)
                .ok_or_else(|| de::Error::custom("overflowed a usize"))?,
            KeyValuePairsAdapter(visitor),
        )
    }
}

impl<'de, V> de::Visitor<'de> for KeyValuePairsAdapter<V>
where
    V: de::Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "an flattened array of key-value pairs")?;
        self.0.expecting(formatter)
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        self.0.visit_map(KeyValuePairsAdapter(seq))
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        self.0.visit_map(map)
    }
}

impl<'de, S> de::MapAccess<'de> for KeyValuePairsAdapter<S>
where
    S: de::SeqAccess<'de>,
{
    type Error = S::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        self.0.next_element_seed(seed)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        self.0.next_element_seed(seed)?.ok_or_else(|| {
            de::Error::custom(
                "underlying array contained an odd number of \
                elements while deserializing as key value pairs",
            )
        })
    }
}
