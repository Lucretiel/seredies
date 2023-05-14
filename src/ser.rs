// mod primitives;
// mod formatters;
pub mod util;

use std::io;

use memchr::memchr2;
use serde::ser;
use thiserror::Error;

use self::util::TupleSeqAdapter;

pub fn to_vec<T>(data: &T) -> Result<Vec<u8>, Error>
where
    T: ser::Serialize + ?Sized,
{
    let mut buffer = Vec::new();
    let serializer = Serializer::new(&mut buffer);
    data.serialize(serializer)?;
    Ok(buffer)
}

pub struct Serializer<'a, W> {
    writer: &'a mut W,
}

impl<'a, W: io::Write> Serializer<'a, W> {
    #[inline]
    #[must_use]
    pub fn new(writer: &'a mut W) -> Self {
        Self { writer }
    }
}

/// Errors that can occur during serialization
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Complex enums can't be serialized (only unit enums are supported)
    #[error("can't serialize enums (other than unit variants)")]
    UnsupportedEnumType,

    /// Map types can't be serialized (consider using
    /// [`KeyValuePairs`][crate::components::KeyValuePairs] to flatten them,
    /// or [`Command`][crate::components::Command] if you're trying to
    /// construct a redis command containing a map)
    #[error("can't serialize maps")]
    UnsupportedMapType,

    /// Float types can't be serialized. Consider using
    /// [`RedisString`][crate::components::RedisString] to convert them to a
    /// Redis string, if that's appropriate for your use case.
    #[error("can't serialize floats")]
    UnsupportedFloatType,

    /// Attempted to serialize a number that was outside the range of a signed
    /// 64 bit integer. Redis integers always fit in this range.
    ///
    /// Don't forget that Redis commands are always a list of strings, even
    /// when they contain numeric data. Consider using
    /// [`RedisString`][crate::components::RedisString] or
    /// [`Command`][crate::components::Command] in this case.
    #[error("can't serialize numbers outside the range of a signed 64 bit integer")]
    NumberOutOfRange,

    /// Redis arrays are length-prefixed; they must know the length ahead of
    /// time. Consider using [`Command`][crate::components::Command] if you're
    /// trying to serialize a Redis command, as it automatically handles
    /// efficiently computing the length of the array (without allocating).
    #[error("can't serialize sequences of unknown length")]
    UnknownSeqLength,

    /// Attempted to serialize too many or too few sequence elements. This
    /// error occurs when the number of serialized array elements differed
    /// from the prefix-reported length of the array.
    #[error("attempted to serialize too many or too few sequence elements")]
    BadSeqLength,

    /// Attempted to serialize a RESP [Simple String] or [Error] that contained
    /// a `\r` or `\n`.
    ///
    /// [Simple String]: https://redis.io/docs/reference/protocol-spec/#resp-simple-strings
    /// [Error]: https://redis.io/docs/reference/protocol-spec/#resp-errors
    #[error("attempted to serialize a Simple String that contained a \\r or \\n")]
    BadSimpleString,

    /// There was an i/o error during serialization.
    #[error("i/o error during serialization")]
    Io(#[from] io::Error),

    /// The data being serialized encountered some kind of error, separate from
    /// the RESP protocol.
    #[error("error from Serialize type: {0}")]
    Custom(String),

    /// Attempted to serialize something other than a string, bytes, or
    /// unit enum as a RESP [Error].
    ///
    /// [Error]: https://redis.io/docs/reference/protocol-spec/#resp-errors
    #[error("invalid payload for a Result::Err. Must be a string or simple enum")]
    InvalidErrorPayload,
}

impl ser::Error for Error {
    #[inline]
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Custom(msg.to_string())
    }
}

#[inline]
fn serialize_number(dest: &mut impl io::Write, value: impl TryInto<i64>) -> Result<(), Error> {
    let value = value.try_into().map_err(|_| Error::NumberOutOfRange)?;
    write!(dest, ":{value}\r\n",).map_err(Error::Io)
}

impl<'a, W: io::Write> ser::Serializer for Serializer<'a, W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SerializeSeq<'a, W>;
    type SerializeTuple = TupleSeqAdapter<SerializeSeq<'a, W>>;
    type SerializeTupleStruct = TupleSeqAdapter<SerializeSeq<'a, W>>;

    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;

    type SerializeStructVariant = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, if v { 1 } else { 0 })
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, v)
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, v)
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, v)
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, v)
    }

    #[inline]
    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, v)
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, v)
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, v)
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, v)
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, v)
    }

    #[inline]
    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.writer, v)
    }

    #[inline]
    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Error::UnsupportedFloatType)
    }

    #[inline]
    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Error::UnsupportedFloatType)
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut buf = [0; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.serialize_bytes(v.as_bytes())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        let len: i64 = v.len().try_into().map_err(|_| Error::NumberOutOfRange)?;

        write!(self.writer, "${len}\r\n")?;
        self.writer.write_all(v)?;
        self.writer.write_all(b"\r\n")?;

        Ok(())
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.writer.write_all(b"$-1\r\n").map_err(Error::Io)
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.writer.write_all(b"$-1\r\n").map_err(Error::Io)
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
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
        // TODO: use special newtype struct to handle simple strings
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        match (name, variant) {
            ("Result", "Ok") => value.serialize(SerializeResultOk { inner: self }),
            ("Result", "Err") => value.serialize(SerializeResultError {
                writer: self.writer,
            }),
            _ => Err(Error::UnsupportedEnumType),
        }
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.serialize_tuple(len.ok_or(Error::UnknownSeqLength)?)
            .map(|adapter| adapter.inner)
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        write!(self.writer, "*{len}\r\n")?;
        Ok(TupleSeqAdapter::new(SerializeSeq::new(self.writer, len)))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_tuple(len)
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::UnsupportedEnumType)
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::UnsupportedMapType)
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::UnsupportedMapType)
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::UnsupportedEnumType)
    }
}

#[derive(Debug)]
pub struct SerializeSeq<'a, W> {
    remaining: usize,
    writer: &'a mut W,
}

impl<'a, W: io::Write> SerializeSeq<'a, W> {
    #[inline]
    #[must_use]
    pub fn new(writer: &'a mut W, length: usize) -> Self {
        Self {
            writer,
            remaining: length,
        }
    }
}

impl<W: io::Write> ser::SerializeSeq for SerializeSeq<'_, W> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        match self.remaining.checked_sub(1) {
            Some(remain) => self.remaining = remain,
            None => return Err(Error::BadSeqLength),
        }

        value.serialize(Serializer {
            writer: self.writer,
        })
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self.remaining {
            0 => Ok(()),
            _ => Err(Error::BadSeqLength),
        }
    }
}

/// This is basically identical to [`Serializer`], but it separately handles
/// the unit type as `+OK\r\n`, following the redis convention for ordinary
/// success.
struct SerializeResultOk<'a, W> {
    inner: Serializer<'a, W>,
}

impl<'a, W: io::Write> ser::Serializer for SerializeResultOk<'a, W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SerializeSeq<'a, W>;
    type SerializeTuple = TupleSeqAdapter<SerializeSeq<'a, W>>;
    type SerializeTupleStruct = TupleSeqAdapter<SerializeSeq<'a, W>>;

    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;

    type SerializeStructVariant = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_bool(v)
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_i8(v)
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_i16(v)
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_i32(v)
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_i64(v)
    }

    #[inline]
    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_i128(v)
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_u8(v)
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_u16(v)
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_u32(v)
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_u64(v)
    }

    #[inline]
    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_u128(v)
    }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_f32(v)
    }

    #[inline]
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_f64(v)
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_char(v)
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_str(v)
    }

    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_bytes(v)
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_none()
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        self.inner.serialize_some(value)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.inner.writer.write_all(b"+OK\r\n").map_err(Error::Io)
    }

    #[inline]
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_struct(name)
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.inner
            .serialize_unit_variant(name, variant_index, variant)
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        self.inner.serialize_newtype_struct(name, value)
    }

    #[inline]
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
        self.inner
            .serialize_newtype_variant(name, variant_index, variant, value)
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.inner.serialize_seq(len)
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.inner.serialize_tuple(len)
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.inner.serialize_tuple_struct(name, len)
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.inner
            .serialize_tuple_variant(name, variant_index, variant, len)
    }

    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.inner.serialize_map(len)
    }

    #[inline]
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.inner.serialize_struct(name, len)
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.inner
            .serialize_struct_variant(name, variant_index, variant, len)
    }
}

/// An error serializer only accepts strings / bytes or similar payloads and
/// serializes them as Redis error values.
struct SerializeResultError<'a, W: io::Write> {
    writer: &'a mut W,
}

impl<W: io::Write> ser::Serializer for SerializeResultError<'_, W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    #[inline]
    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_i128(self, _v: i128) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut buf = [0; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.serialize_bytes(v.as_bytes())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        if memchr2(b'\r', b'\n', v).is_some() {
            Err(Error::BadSimpleString)
        } else {
            self.writer.write_all(b"-")?;
            self.writer.write_all(v)?;
            self.writer.write_all(b"\r\n")?;
            Ok(())
        }
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, _v: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::InvalidErrorPayload)
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
        self.serialize_str(variant)
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
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::InvalidErrorPayload)
    }
}
