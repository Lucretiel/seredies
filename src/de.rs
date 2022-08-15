use std::fmt::Display;

use serde::{de, forward_to_deserialize_any};
use thiserror::Error;

use crate::parse::{self, ParseResult, Tag};

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("parsing error")]
    Parse(#[from] parse::Error),

    #[error("successfully deserialized a Redis Error containing this message")]
    Redis(Vec<u8>),

    #[error("an array or bulk string length was out of bounds")]
    Length,

    #[error("a sequence deserializer didn't consume every element in the array")]
    UnfinishedArray,

    #[error("tried to serialize a Result with a non-newtype variant")]
    InvalidResultDeserialize,

    #[error("something went wrong while deserializing the message of a redis error")]
    InvalidErrorCode,

    #[error("tried to deserialize an enum (consider using serde(variant_identifier) for string-like enums)")]
    EnumRequested,

    #[error("error from Deserialize type: {0}")]
    Custom(String),
}

impl de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::Custom(msg.to_string())
    }
}

pub struct Deserializer<'a, 'de> {
    input: &'a mut &'de [u8],
}

// Bulk strings can be up to 512 MB
const MAX_BULK_LENGTH: i64 = 512 * 1024 * 1024;

impl<'a, 'de> Deserializer<'a, 'de> {
    /// Create a new deserializer. After it successfully deserializes a single
    /// value, the input buffer will have been modified in place, with the
    /// deserialized prefix removed.
    #[inline]
    #[must_use]
    pub fn new(input: &'a mut &'de [u8]) -> Self {
        Self { input }
    }

    #[inline]
    fn apply_parser<T>(
        &mut self,
        parser: impl FnOnce(&'de [u8]) -> ParseResult<'de, T>,
    ) -> Result<T, parse::Error> {
        let (value, input) = parser(*self.input)?;
        *self.input = input;
        Ok(value)
    }
}

impl<'a, 'de> de::Deserializer<'de> for Deserializer<'a, 'de> {
    type Error = Error;

    forward_to_deserialize_any! {
        i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit_struct seq tuple unit
        tuple_struct map struct identifier ignored_any
    }

    fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.apply_parser(parse::read_tag)? {
            (Tag::SimpleString, payload) => visitor.visit_borrowed_bytes(payload),
            (Tag::Error, payload) => Err(Error::Redis(payload.to_owned())),
            (Tag::Integer, payload) => visitor.visit_i64(parse::parse_number(payload)?),
            (Tag::BulkString, payload) => match parse::parse_number(payload)? {
                -1 => visitor.visit_unit(),
                length if length > MAX_BULK_LENGTH => Err(Error::Length),
                length => {
                    let length = length.try_into().map_err(|_| Error::Length)?;
                    let payload = self.apply_parser(|input| parse::read_exact(length, input))?;
                    visitor.visit_borrowed_bytes(payload)
                }
            },
            (Tag::Array, payload) => {
                let length = parse::parse_number(payload)?
                    .try_into()
                    .map_err(|_| Error::Length)?;

                let mut seq = SeqAccess {
                    length,
                    input: self.input,
                };

                match visitor.visit_seq(&mut seq) {
                    Ok(..) if seq.length > 0 => Err(Error::UnfinishedArray),
                    Ok(value) => Ok(value),

                    // If there was an unexpected EOF from inside the array,
                    // increase the size. We know that the minimum size of a
                    // RESP value is 3 bytes, plus the array itself has a 2
                    // byte terminator.
                    Err(Error::Parse(parse::Error::UnexpectedEof(len))) => {
                        Err(Error::Parse(parse::Error::UnexpectedEof(
                            len.saturating_add(seq.length.saturating_mul(3))
                                .saturating_add(2),
                        )))
                    }

                    Err(err) => Err(err),
                }
            }
        }
    }

    #[inline]
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        // Use deserialize_any, but provide a variant `Visitor` that treats
        // 0 and 1 as true and false
        struct BoolVisitAdapter<V> {
            inner: V,
        }

        impl<'de, V> de::Visitor<'de> for BoolVisitAdapter<V>
        where
            V: de::Visitor<'de>,
        {
            type Value = V::Value;

            #[inline]
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                self.inner.expecting(formatter)
            }

            #[inline]
            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.inner.visit_bool(v)
            }

            #[inline]
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v {
                    0 => self.inner.visit_bool(false),
                    1 => self.inner.visit_bool(true),
                    _ => self.inner.visit_i64(v),
                }
            }

            #[inline]
            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v {
                    0 => self.inner.visit_bool(false),
                    1 => self.inner.visit_bool(true),
                    _ => self.inner.visit_u64(v),
                }
            }

            #[inline]
            fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.inner.visit_borrowed_bytes(v)
            }

            #[inline]
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.inner.visit_unit()
            }

            #[inline]
            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                self.inner.visit_seq(seq)
            }
        }

        self.deserialize_any(BoolVisitAdapter { inner: visitor })
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match parse::read_tag(*self.input)? {
            ((Tag::BulkString, payload), input) if parse::parse_number(payload)? == -1 => {
                *self.input = input;
                visitor.visit_none()
            }

            _ => visitor.visit_some(self),
        }
    }

    #[inline]
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

    #[inline]
    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match (name, variants) {
            ("Result", ["Ok", "Err"] | ["Err", "Ok"]) => {
                visitor.visit_enum(ResultEnumAccess::new(self.input))
            }
            _ => self.deserialize_any(visitor),
        }
    }
}

struct SeqAccess<'a, 'de> {
    length: usize,
    input: &'a mut &'de [u8],
}

impl<'de> de::SeqAccess<'de> for SeqAccess<'_, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.length {
            0 => Ok(None),
            ref mut length => {
                *length -= 1;

                seed.deserialize(Deserializer { input: self.input })
                    .map(Some)
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> Option<usize> {
        Some(self.length)
    }
}

struct ResultEnumAccess<'a, 'de> {
    input: &'a mut &'de [u8],
}

impl<'a, 'de> ResultEnumAccess<'a, 'de> {
    #[inline]
    pub fn new(input: &'a mut &'de [u8]) -> Self {
        Self { input }
    }
}

impl<'a, 'de> de::EnumAccess<'de> for ResultEnumAccess<'a, 'de> {
    type Error = Error;
    type Variant = ResultVariantAccess<'a, 'de>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        match parse::read_tag(*self.input)? {
            // "-ERRORCODE message\r\n" can be deserialized into:
            // Err("ERRORCODE message")
            // Err(ErrorKind::ERRORCODE("message"))
            ((Tag::Error, payload), input) => {
                *self.input = input;

                seed.deserialize(de::value::BorrowedStrDeserializer::new("Err"))
                    .map(|value| (value, ResultVariantAccess::ErrorValue { payload }))
            }
            // "+OK\r\n" can be deserialized to either Result::Ok("OK") or
            // Result::OK(())
            ((Tag::SimpleString, b"OK"), input) => {
                *self.input = input;
                seed.deserialize(de::value::BorrowedStrDeserializer::new("Ok"))
                    .map(|value| (value, ResultVariantAccess::PlainOk))
            }
            _ => seed
                .deserialize(de::value::BorrowedStrDeserializer::new("Ok"))
                .map(|value| (value, ResultVariantAccess::Ok { input: self.input })),
        }
    }
}

enum ResultVariantAccess<'a, 'de> {
    ErrorValue { payload: &'de [u8] },
    PlainOk,
    Ok { input: &'a mut &'de [u8] },
}

impl<'de> de::VariantAccess<'de> for ResultVariantAccess<'_, 'de> {
    type Error = Error;

    #[inline]
    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self {
            ResultVariantAccess::Ok { input } => seed.deserialize(Deserializer::new(input)),
            ResultVariantAccess::PlainOk => seed.deserialize(PlainOkDeserializer),
            ResultVariantAccess::ErrorValue { payload } => {
                seed.deserialize(ResultErrPayloadDeserializer { payload })
            }
        }
    }

    #[inline]
    fn unit_variant(self) -> Result<(), Self::Error> {
        Err(Error::InvalidResultDeserialize)
    }

    #[inline]
    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::InvalidResultDeserialize)
    }

    #[inline]
    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::InvalidResultDeserialize)
    }
}

struct PlainOkDeserializer;

impl<'de> de::Deserializer<'de> for PlainOkDeserializer {
    type Error = Error;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any enum
    }

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(b"OK")
    }

    #[inline]
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

struct ResultErrPayloadDeserializer<'de> {
    payload: &'de [u8],
}

impl<'de> de::Deserializer<'de> for ResultErrPayloadDeserializer<'de> {
    type Error = Error;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(self.payload)
    }

    #[inline]
    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_enum(ResultErrCodeEnumAccess::new(self.payload))
    }
}

struct ResultErrCodeEnumAccess<'de> {
    code: &'de [u8],
    message: &'de [u8],
}

impl<'de> ResultErrCodeEnumAccess<'de> {
    #[inline]
    pub fn new(payload: &'de [u8]) -> Self {
        let (code, message) = split_at_pred(payload, |&b| b.is_ascii_whitespace());
        let (_, message) = split_at_pred(message, |&b| !b.is_ascii_whitespace());

        Self { code, message }
    }
}

impl<'de> de::EnumAccess<'de> for ResultErrCodeEnumAccess<'de> {
    type Error = Error;
    type Variant = ResultErrMessageVariantAccess<'de>;

    #[inline]
    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        seed.deserialize(de::value::BorrowedBytesDeserializer::new(self.code))
            .map(|value| (value, ResultErrMessageVariantAccess::new(self.message)))
    }
}

struct ResultErrMessageVariantAccess<'de> {
    message: &'de [u8],
}

impl<'de> ResultErrMessageVariantAccess<'de> {
    #[inline]
    pub fn new(message: &'de [u8]) -> Self {
        Self { message }
    }
}

impl<'de> de::VariantAccess<'de> for ResultErrMessageVariantAccess<'de> {
    type Error = Error;

    #[inline]
    fn unit_variant(self) -> Result<(), Self::Error> {
        match self.message {
            &[] => Ok(()),
            _ => Err(Error::InvalidErrorCode),
        }
    }

    #[inline]
    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(de::value::BorrowedBytesDeserializer::new(self.message))
    }

    #[inline]
    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::InvalidErrorCode)
    }

    #[inline]
    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::InvalidErrorCode)
    }
}

// Split an input in half at the first item that matches a predicate.
#[inline]
fn split_at_pred<T>(input: &[T], pred: impl Fn(&T) -> bool) -> (&[T], &[T]) {
    input.split_at(input.iter().position(pred).unwrap_or(input.len()))
}
