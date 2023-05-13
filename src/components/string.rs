use std::{fmt::Display, io::Write as _, mem};

use arrayvec::ArrayVec;
use serde::ser;

/// Adapter type that serializes the contained value as a string.
///
/// Frequently, especially when sending commands, Redis will require data to be
/// passed as a string, even when the underlying data is something like an
/// integer. This type serializes its inner value as a string, and works
/// on most primitive types.
pub struct RedisString<T: ?Sized>(pub T);

impl<T: ?Sized> RedisString<T> {
    pub fn new_ref(value: &T) -> &Self {
        unsafe { mem::transmute(value) }
    }
}

impl<T: ser::Serialize + ?Sized> ser::Serialize for RedisString<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(RedisStringAdapter(serializer))
    }
}

/// Internal adapter type for serializers, deserializers, visitors, etc.
struct RedisStringAdapter<T>(T);

impl<S> RedisStringAdapter<S>
where
    S: ser::Serializer,
{
    fn serialize_int(self, value: impl Display) -> Result<S::Ok, S::Error> {
        // 39 digits should be enough for even a 128 bit int, but we'll round
        // way up to be safe
        let mut buffer: ArrayVec<u8, 64> = ArrayVec::new();

        write!(&mut buffer, "{value}")
            .map_err(|_| ser::Error::custom("integer was more than 64 digits"))?;

        self.0.serialize_bytes(&buffer)
    }
}

impl<S> ser::Serializer for RedisStringAdapter<S>
where
    S: ser::Serializer,
{
    type Ok = S::Ok;
    type Error = S::Error;

    type SerializeSeq = ser::Impossible<S::Ok, S::Error>;
    type SerializeTuple = ser::Impossible<S::Ok, S::Error>;
    type SerializeTupleStruct = ser::Impossible<S::Ok, S::Error>;
    type SerializeTupleVariant = ser::Impossible<S::Ok, S::Error>;
    type SerializeMap = ser::Impossible<S::Ok, S::Error>;
    type SerializeStruct = ser::Impossible<S::Ok, S::Error>;
    type SerializeStructVariant = ser::Impossible<S::Ok, S::Error>;

    #[inline]
    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(ser::Error::custom(
            "can't serialize booleans as redis strings",
        ))
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.serialize_int(v)
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut buffer = [0; 4];
        self.serialize_str(v.encode_utf8(&mut buffer))
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.serialize_bytes(v.as_bytes())
    }

    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_bytes(v)
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(ser::Error::custom(
            "can't serialize options as redis strings",
        ))
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Err(ser::Error::custom(
            "can't serialize options as redis strings",
        ))
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(ser::Error::custom("can't serialize units as redis strings"))
    }

    #[inline]
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(name)
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit_struct(variant)
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
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
        Err(ser::Error::custom(
            "can't serialize data enums as redis strings",
        ))
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(ser::Error::custom("can't serialize lists as redis strings"))
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(ser::Error::custom(
            "can't serialize tuples as redis strings",
        ))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(ser::Error::custom(
            "can't serialize structs as redis strings",
        ))
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(ser::Error::custom(
            "can't serialize data enums as redis strings",
        ))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(ser::Error::custom("can't serialize maps as redis strings"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(ser::Error::custom(
            "can't serialize structs as redis strings",
        ))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(ser::Error::custom(
            "can't serialize data enums as redis strings",
        ))
    }
}
