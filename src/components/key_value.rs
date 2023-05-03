use serde::{
    de,
    ser::{self, Error as _},
};

#[derive(Debug, Copy, Clone, Default)]
pub struct KeyValuePairs<T>(pub T);

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

fn double_len<E: ser::Error>(len: usize) -> Result<usize, E> {
    len.checked_mul(2)
        .ok_or_else(|| E::custom("overflowed a usize"))
}

impl<S: ser::Serializer> ser::Serializer for KeyValuePairsAdapter<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    type SerializeSeq = KeyValuePairsAdapter<S::SerializeSeq>;
    type SerializeTuple = KeyValuePairsAdapter<S::SerializeTuple>;

    type SerializeMap = KeyValuePairsAdapter<S::SerializeSeq>;
    type SerializeStruct = KeyValuePairsAdapter<S::SerializeTuple>;

    type SerializeTupleStruct = ser::Impossible<S::Ok, S::Error>;
    type SerializeTupleVariant = ser::Impossible<S::Ok, S::Error>;
    type SerializeStructVariant = ser::Impossible<S::Ok, S::Error>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
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
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.0
            .serialize_seq(len.map(double_len).transpose()?)
            .map(KeyValuePairsAdapter)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.0
            .serialize_tuple(double_len(len)?)
            .map(KeyValuePairsAdapter)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.0
            .serialize_seq(len.map(double_len).transpose()?)
            .map(KeyValuePairsAdapter)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.0
            .serialize_tuple(double_len(len)?)
            .map(KeyValuePairsAdapter)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Self::Error::custom(
            "KeyValuePairsAdapter must deserialize a collection",
        ))
    }
}

impl<S: ser::SerializeSeq> ser::SerializeSeq for KeyValuePairsAdapter<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(KeyValueEntryAdapter(&mut self.0))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<S: ser::SerializeTuple> ser::SerializeTuple for KeyValuePairsAdapter<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(KeyValueEntryAdapter(&mut self.0))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
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

#[derive(Debug, Clone, Copy, Default)]
struct KeyValueEntryAdapter<'a, T>(&'a mut T);

impl<S: ser::SerializeSeq> ser::Serializer for KeyValueEntryAdapter<'_, S> {
    type Ok = S::Ok;
    type Error = S::Error;

    type SerializeSeq;

    type SerializeTuple;

    type SerializeTupleStruct;

    type SerializeTupleVariant;

    type SerializeMap;

    type SerializeStruct;

    type SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("must serialize a pair"))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        todo!()
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        todo!()
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        todo!()
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        todo!()
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        todo!()
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        todo!()
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        todo!()
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        todo!()
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        todo!()
    }
}
