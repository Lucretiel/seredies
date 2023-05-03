mod parse;
mod result;

use std::fmt::Display;

use serde::{de, forward_to_deserialize_any};
use thiserror::Error;

use self::parse::{ParseResult, TaggedHeader};
use self::result::ResultAccess;

/// Errors that can occur while deserializing RESP data
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum Error {
    /// There was an error during parsing (such as a \r without a \n)
    #[error("parsing error")]
    Parse(#[from] parse::Error),

    /// The length of an array or bulk string was out of bounds. It might
    /// have been negative, or exceeded the 512MB limit for bulk strings.
    #[error("an array or bulk string length was out of bounds")]
    Length,

    /// The `Deserialize` type successfully deserialized from a Redis array,
    /// but didn't consume the whole thing.
    #[error("the `Deserialize` type didn't consume the entire array")]
    UnfinishedArray,

    /// There was an error from the `Deserialize` type
    #[error("error from Deserialize type: {0}")]
    Custom(String),

    /// We *successfully* deserialized a Redis Error value (with the `-` tag)
    /// See the module docs on `Result` deserialization for how to avoid this
    /// error.
    #[error("successfully deserialized a Redis Error containing this message")]
    Redis(Vec<u8>),
}

impl de::Error for Error {
    #[inline]
    #[must_use]
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::Custom(msg.to_string())
    }
}

#[inline]
fn apply_parser<'de, T>(
    input: &mut &'de [u8],
    parser: impl FnOnce(&'de [u8]) -> ParseResult<'de, T>,
) -> Result<T, parse::Error> {
    parser(input).map(|(value, tail)| {
        *input = tail;
        value
    })
}

/// Trait that abstracts the header read operation. At various points during
/// a deserialize, the Deserializer might either need to parse a header, or
/// might already have one from a parse operation. For example, when
/// deserializing an `Option`, if the value is NOT null, the parsed header
/// is retained by the deserializer passed into `deserialize_some`. This trait
/// abstracts over the presence or absence of a parsed header.
pub trait ReadHeader<'de>: Sized {
    /// Read a header, possibly from the `input`.
    fn read_header(self, input: &mut &'de [u8]) -> Result<TaggedHeader<'de>, parse::Error>;
}

impl<'de> ReadHeader<'de> for TaggedHeader<'de> {
    /// A `TaggedHeader` can simply return itself without touching the input
    #[inline]
    fn read_header(self, _input: &mut &'de [u8]) -> Result<TaggedHeader<'de>, parse::Error> {
        Ok(self)
    }
}

pub struct ParseHeader;

impl<'de> ReadHeader<'de> for ParseHeader {
    /// We don't have a header; we must try to read one from the input.
    #[inline]
    fn read_header(self, input: &mut &'de [u8]) -> Result<TaggedHeader<'de>, parse::Error> {
        apply_parser(input, parse::read_header)
    }
}

pub struct BaseDeserializer<'a, 'de, H> {
    header: H,
    input: &'a mut &'de [u8],
}

pub type Deserializer<'a, 'de> = BaseDeserializer<'a, 'de, ParseHeader>;
type PreParsedDeserializer<'a, 'de> = BaseDeserializer<'a, 'de, TaggedHeader<'de>>;

impl<'a, 'de> Deserializer<'a, 'de> {
    #[inline]
    pub fn new(input: &'a mut &'de [u8]) -> Self {
        Self {
            input,
            header: ParseHeader,
        }
    }
}

impl<'a, 'de> PreParsedDeserializer<'a, 'de> {
    #[inline]
    fn new(header: TaggedHeader<'de>, input: &'a mut &'de [u8]) -> Self {
        Self { input, header }
    }
}

// Bulk strings can be up to 512 MB
const MAX_BULK_LENGTH: i64 = 512 * 1024 * 1024;

impl<'a, 'de, H: ReadHeader<'de>> BaseDeserializer<'a, 'de, H> {
    /// Read the header from a RESP value. The header consists of a single
    /// tag byte, followed by some kind of payload (which may not contain \r
    /// or \n), followed by \r\n.
    #[inline]
    fn read_header(self) -> Result<PreParsedDeserializer<'a, 'de>, parse::Error> {
        let input = self.input;

        self.header
            .read_header(input)
            .map(|header| PreParsedDeserializer::new(header, input))
    }
}

impl<'de, P: ReadHeader<'de>> de::Deserializer<'de> for BaseDeserializer<'_, 'de, P> {
    type Error = Error;

    forward_to_deserialize_any! {
        i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit_struct seq tuple unit
        tuple_struct map struct identifier ignored_any
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let parsed = self.read_header()?;

        match parsed.header {
            // Simple Strings are handled as byte arrays
            TaggedHeader::SimpleString(payload) => visitor.visit_borrowed_bytes(payload),

            // Errors are handled by default as actual deserialization errors.
            // (see deserialize_enum for how to circumvent this)
            TaggedHeader::Error(payload) => Err(Error::Redis(payload.to_owned())),

            // Integers are parsed then handled as i64. All Redis integers are
            // guaranteed to fit in a signed 64 bit int.
            TaggedHeader::Integer(value) => visitor.visit_i64(value),

            // Bulk strings are handled as byte arrays
            TaggedHeader::BulkString(len) if len > MAX_BULK_LENGTH => Err(Error::Length),
            TaggedHeader::BulkString(len) => visitor.visit_borrowed_bytes({
                let len = len.try_into().map_err(|_| Error::Length)?;
                apply_parser(parsed.input, |input| parse::read_exact(len, input))?
            }),

            // Arrays are handled as serde sequences.
            TaggedHeader::Array(length) => {
                let mut seq = SeqAccess {
                    input: parsed.input,
                    length,
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

            // Null (technically a Bulk String with a length of -1) is a unit
            TaggedHeader::Null => visitor.visit_unit(),
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
        let parsed = self.read_header()?;

        match parsed.header {
            TaggedHeader::Null => visitor.visit_none(),
            _ => visitor.visit_some(parsed),
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
                let parsed = self.read_header()?;

                match parsed.header {
                    // "+OK\r\n" can be deserialized to either Result::Ok("OK") or
                    // Result::OK(())
                    TaggedHeader::SimpleString(b"OK") => {
                        visitor.visit_enum(ResultAccess::new_plain_ok())
                    }

                    // "-ERR message\r\n" can be deserialized into:
                    // Err("ERR message")
                    TaggedHeader::Error(message) => {
                        visitor.visit_enum(ResultAccess::new_err(message))
                    }

                    // For everything else, deserialize inline as a Result::Ok
                    _ => visitor.visit_enum(ResultAccess::new_ok(parsed)),
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
        self.length = match self.length.checked_sub(1) {
            Some(length) => length,
            None => return Ok(None),
        };

        seed.deserialize(Deserializer::new(self.input)).map(Some)
    }

    #[inline]
    #[must_use]
    fn size_hint(&self) -> Option<usize> {
        Some(self.length)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::iter;

    use cool_asserts::assert_matches;
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
                    let mut vec = Vec::with_capacity(seq.size_hint().unwrap_or(0));

                    itertools::process_results(
                        iter::from_fn(|| seq.next_element().transpose()),
                        |iter| {
                            vec.extend(iter);
                            vec
                        },
                    )
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

    fn test_result_deserializer<'a, T, E>(mut input: &'a [u8], expected: Result<T, E>)
    where
        T: de::Deserialize<'a> + Eq + Debug,
        E: de::Deserialize<'a> + Eq + Debug,
    {
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
