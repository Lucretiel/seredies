use std::fmt::Display;

use derive_new::new;
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

#[derive(new)]
pub struct Deserializer<'a, 'de> {
    input: &'a mut &'de [u8],
}

// Bulk strings can be up to 512 MB
const MAX_BULK_LENGTH: i64 = 512 * 1024 * 1024;

impl<'a, 'de> Deserializer<'a, 'de> {
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
                length => visitor.visit_borrowed_bytes({
                    let length = length.try_into().map_err(|_| Error::Length)?;
                    self.apply_parser(|input| parse::read_exact(length, input))?
                }),
            },
            (Tag::Array, payload) => {
                let mut seq = SeqAccess {
                    length: parse::parse_number(payload)?
                        .try_into()
                        .map_err(|_| Error::Length)?,
                    input: self.input,
                };

                match visitor.visit_seq(&mut seq) {
                    Ok(..) if seq.length > 0 => Err(Error::UnfinishedArray),
                    Ok(value) => Ok(value),

                    // If there was an unexpected EOF from inside the array,
                    // increase the size. We know that the minimum size of a
                    // RESP value is 3 bytes, plus the array itself has a 2
                    // byte terminator.
                    // TODO: include both a minimum and recommended byte count
                    // (since in practice data in an array will usually be
                    // bulk strings, which are minimum 5 bytes)
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
                match parse::read_tag(*self.input)? {
                    // "+OK\r\n" can be deserialized to either Result::Ok("OK") or
                    // Result::OK(())
                    ((Tag::SimpleString, b"OK"), input) => {
                        *self.input = input;
                        visitor.visit_enum(ResultAccess::new(ResultPlainOkPattern))
                    }

                    // "-ERR message\r\n" can be deserialized into:
                    // Err("ERR message")
                    ((Tag::Error, payload), input) => {
                        *self.input = input;
                        visitor.visit_enum(ResultAccess::new(ResultErrPattern::new(payload)))
                    }

                    _ => visitor.visit_enum(ResultAccess::new(ResultOkPattern::new(self))),
                }
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

    #[inline]
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        self.length
            .checked_sub(1)
            .map(move |new_length| {
                self.length = new_length;
                seed.deserialize(Deserializer::new(self.input))
            })
            .transpose()
    }

    #[inline]
    #[must_use]
    fn size_hint(&self) -> Option<usize> {
        Some(self.length)
    }
}

trait ResultAccessPattern<'de> {
    fn variant(&self) -> Result<(), ()>;
    fn value<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: de::DeserializeSeed<'de>;
}

#[derive(new)]
struct ResultAccess<T> {
    access: T,
}

impl<'de, T: ResultAccessPattern<'de>> de::EnumAccess<'de> for ResultAccess<T> {
    type Error = Error;
    type Variant = Self;

    #[inline]
    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        seed.deserialize(de::value::BorrowedStrDeserializer::new(
            match self.access.variant() {
                Ok(()) => "Ok",
                Err(()) => "Err",
            },
        ))
        .map(|value| (value, self))
    }
}

impl<'de, T: ResultAccessPattern<'de>> de::VariantAccess<'de> for ResultAccess<T> {
    type Error = Error;

    #[inline]
    fn newtype_variant_seed<S>(self, seed: S) -> Result<S::Value, Self::Error>
    where
        S: de::DeserializeSeed<'de>,
    {
        self.access.value(seed)
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

#[derive(new)]
struct ResultPlainOkPattern;

impl<'de> ResultAccessPattern<'de> for ResultPlainOkPattern {
    #[inline]
    #[must_use]
    fn variant(&self) -> Result<(), ()> {
        Ok(())
    }

    #[inline]
    fn value<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self)
    }
}

impl<'de> de::Deserializer<'de> for ResultPlainOkPattern {
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

#[derive(new)]
struct ResultOkPattern<'a, 'de> {
    deserializer: Deserializer<'a, 'de>,
}

impl<'de> ResultAccessPattern<'de> for ResultOkPattern<'_, 'de> {
    #[inline]
    #[must_use]
    fn variant(&self) -> Result<(), ()> {
        Ok(())
    }

    #[inline]
    fn value<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self.deserializer)
    }
}

#[derive(new)]
struct ResultErrPattern<'de> {
    message: &'de [u8],
}

impl<'de> ResultAccessPattern<'de> for ResultErrPattern<'de> {
    #[inline]
    #[must_use]
    fn variant(&self) -> Result<(), ()> {
        Err(())
    }

    #[inline]
    fn value<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(de::value::BorrowedBytesDeserializer::new(self.message))
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::iter;

    use cool_asserts::assert_matches;
    use itertools::Itertools as _;
    use serde::Deserialize as _;

    use super::*;

    #[derive(PartialEq, Eq, Debug)]
    enum Data<'a> {
        Null,
        String(&'a [u8]),
        Integer(i64),
        Array(Vec<Data<'a>>),
    }

    use Data::Null;

    impl<'a> From<&'a [u8]> for Data<'a> {
        fn from(string: &'a [u8]) -> Self {
            Self::String(string)
        }
    }

    impl<'a> From<&'a str> for Data<'a> {
        fn from(string: &'a str) -> Self {
            string.as_bytes().into()
        }
    }

    impl From<i64> for Data<'_> {
        fn from(value: i64) -> Self {
            Self::Integer(value)
        }
    }

    impl<'a, T: Into<Data<'a>>, const N: usize> From<[T; N]> for Data<'a> {
        fn from(array: [T; N]) -> Self {
            Self::Array(array.into_iter().map(|value| value.into()).collect())
        }
    }

    impl<'de> de::Deserialize<'de> for Data<'de> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct Visitor;

            impl<'de> de::Visitor<'de> for Visitor {
                type Value = Data<'de>;

                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    write!(f, "a byte string, integer, or array")
                }

                fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(Data::String(v))
                }

                fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(Data::Integer(v))
                }

                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: de::SeqAccess<'de>,
                {
                    iter::from_fn(move || seq.next_element().transpose())
                        .try_collect()
                        .map(Data::Array)
                }

                fn visit_unit<E>(self) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(Data::Null)
                }
            }

            deserializer.deserialize_any(Visitor)
        }
    }

    fn test_basic_deserialize<'a>(
        input: &'a (impl AsRef<[u8]> + ?Sized),
        expected: impl Into<Data<'a>>,
    ) {
        let mut input = input.as_ref();
        let deserializer = Deserializer::new(&mut input);
        let result = Data::deserialize(deserializer).expect("Failed to deserialize");
        assert_eq!(result, expected.into());
        assert!(input.is_empty());
    }

    macro_rules! data_tests {
        ($(
            $name:ident: $value:literal => $expected:expr;
        )*) => {
            $(
                #[test]
                fn $name() {
                    test_basic_deserialize($value, $expected);
                }
            )*
        }
    }

    data_tests! {
        simple_string: "+Hello, World\r\n" => "Hello, World";
        empty_simple_string: "+\r\n" => "";
        integer: ":1000\r\n" => 1000;
        negative_int: ":-1000\r\n" => -1000;
        bulk_string: "$5\r\nhello\r\n" => "hello";
        empty_bulk_string: "$0\r\n\r\n" => "";
        null: "$-1\r\n" => Null;
        weird_null: "$-001\r\n" => Null;
        array: "*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n" => ["hello", "world"];
        heterogeneous: b"*3\r\n:10\r\n$5\r\nhello\r\n$-1\r\n" => [Data::Integer(10), Data::String(b"hello"), Null];
    }

    #[test]
    fn test_bool() {
        let input = b":1\r\n";
        let mut input = &input[..];
        let deserializer = Deserializer::new(&mut input);
        assert!(bool::deserialize(deserializer).expect("failed to deserialize"));
        assert!(input.is_empty());
    }

    #[test]
    fn test_options() {
        let input = b"*3\r\n:3\r\n$-1\r\n$5\r\nhello\r\n";
        let mut input = &input[..];
        let deserializer = Deserializer::new(&mut input);
        let result: Vec<Option<Data<'_>>> =
            Vec::deserialize(deserializer).expect("Failed to deserialize");

        assert_eq!(
            result,
            [Some(Data::Integer(3)), None, Some(Data::String(b"hello"))]
        );

        assert!(input.is_empty());
    }

    #[test]
    fn test_error() {
        let input = b"-ERROR bad data\r\n";
        let mut input = &input[..];
        let deserializer = Deserializer::new(&mut input);
        let result =
            i32::deserialize(deserializer).expect_err("deserialization unexpectedly succeeded");

        assert_matches!(result, Error::Redis(message) => assert_eq!(message, b"ERROR bad data"));
    }

    fn test_result_deserializer<'a, T, E>(
        input: &'a (impl AsRef<[u8]> + ?Sized),
        expected: Result<T, E>,
    ) where
        T: de::Deserialize<'a> + Eq + Debug,
        E: de::Deserialize<'a> + Eq + Debug,
    {
        let mut input = input.as_ref();
        let deserializer = Deserializer::new(&mut input);
        let result: Result<T, E> =
            Result::deserialize(deserializer).expect("Failed to deserialize");
        assert_eq!(result, expected);
        assert!(input.is_empty());
    }

    #[test]
    fn test_result_ok() {
        test_result_deserializer::<&str, String>(b"$5\r\nhello\r\n", Ok("hello"));
    }

    #[test]
    fn test_result_some() {
        test_result_deserializer::<Option<&str>, String>(b"$5\r\nhello\r\n", Ok(Some("hello")));
    }

    #[test]
    fn test_result_none() {
        test_result_deserializer::<Option<&str>, String>(b"$-1\r\n", Ok(None));
    }

    #[test]
    fn test_result_unit() {
        test_result_deserializer::<(), String>(b"+OK\r\n", Ok(()));
    }

    #[test]
    fn test_result_unit_str() {
        test_result_deserializer::<String, String>(b"+OK\r\n", Ok("OK".to_owned()));
    }

    #[test]
    fn test_result_error_msg() {
        test_result_deserializer::<&str, &str>(b"-ERROR bad data\r\n", Err("ERROR bad data"));
    }
}
