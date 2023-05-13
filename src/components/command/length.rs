use serde::ser;
use thiserror::Error;

use crate::ser::util::TupleSeqAdapter;

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("overflowed a usize")]
    Overflow,

    #[error("tried to serialize {0} into a redis command; only sequences and bytes are allowed")]
    InvalidType(&'static str),

    #[error("error from serialized type: {0}")]
    Custom(String),
}

impl ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Custom(msg.to_string())
    }
}

/// Helper serializer that allows us to pre-compute the length of a command.
///
/// Redis arrays must know the length ahead of time in order to serialize;
/// this serializer can be used with a CommandSerializer to compute this
/// length.
pub struct Serializer;

impl ser::Serializer for Serializer {
    type Ok = usize;
    type Error = Error;

    type SerializeSeq = Accumulator;
    type SerializeTuple = TupleSeqAdapter<Accumulator>;

    type SerializeTupleStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = ser::Impossible<Self::Ok, Self::Error>;

    #[inline]
    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("a bool"))
    }

    #[inline]
    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("an i8"))
    }

    #[inline]
    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("an i16"))
    }

    #[inline]
    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("an i32"))
    }

    #[inline]
    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("an i64"))
    }

    #[inline]
    fn serialize_i128(self, _v: i128) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("an i128"))
    }

    #[inline]
    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("a u8"))
    }

    #[inline]
    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("a u16"))
    }

    #[inline]
    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("a u32"))
    }

    #[inline]
    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("a u64"))
    }

    #[inline]
    fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("a u128"))
    }

    #[inline]
    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("an f32"))
    }

    #[inline]
    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("an f64"))
    }

    #[inline]
    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Ok(1)
    }

    #[inline]
    fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(1)
    }

    #[inline]
    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(1)
    }

    #[inline]
    fn collect_str<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: std::fmt::Display,
    {
        Ok(1)
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("an option"))
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Err(Error::InvalidType("an option"))
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidType("a unit"))
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(1)
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(1)
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Err(Error::InvalidType("a newtype struct"))
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
        Err(Error::InvalidType("a data enum"))
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(Accumulator::new())
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(TupleSeqAdapter::new(Accumulator::new()))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::InvalidType("a tuple struct"))
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::InvalidType("a data enum"))
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::InvalidType("a map"))
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::InvalidType("a struct"))
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::InvalidType("a data enum"))
    }
}

pub struct Accumulator {
    length: usize,
}

impl Accumulator {
    #[inline]
    fn new() -> Self {
        Self { length: 0 }
    }
}

impl ser::SerializeSeq for Accumulator {
    type Ok = usize;
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.length = self
            .length
            .checked_add(value.serialize(Serializer)?)
            .ok_or(Error::Overflow)?;

        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.length)
    }
}
