/*!
Serde serializer for turning Rust data structures into RESP encoded data.

This module contains a faithful implementation of the
[Redis Serialization Protocol](https://redis.io/docs/reference/protocol-spec/).

See the [crate docs][crate] for an overview of how seredies maps the RESP data
model to the serde data model.

# Basic example

```
use serde::Serialize;
use seredies::ser::to_vec;

#[derive(Serialize)]
struct Data<'a>(
    String,
    i32,
    &'a str,
    Vec<Option<String>>
);

let data = Data(
    "OK".to_owned(),
    24,
    "Borrowed",
    Vec::from([
        None,
        Some("Hello,".to_owned()),
        Some("World!".to_owned()),
    ]),
);

let resp = to_vec(&data).expect("shouldn't fail to serialize");

assert_eq!(
    &resp,
    b"\
        *4\r\n\
        $2\r\nOK\r\n\
        :24\r\n\
        $8\r\nBorrowed\r\n\
        *3\r\n\
            $-1\r\n\
            $6\r\nHello,\r\n\
            $6\r\nWorld!\r\n\
    "
)
```

# Error example

It's unlikely that you'll ever need to *serialize* a `Result` object as a Redis
error, but we provide for it for completeness and for pairity with the
[deserializer][crate::de::Deserializer].

```
use seredies::ser::to_vec;

let data: Result<Vec<i32>, String> = Ok(Vec::from([1, 2, 3]));
assert_eq!(
    to_vec(&data).expect("serialize shouldn't fail"),
    b"*3\r\n:1\r\n:2\r\n:3\r\n"
);

let data: Result<Vec<i32>, String> = Err("ERROR".to_owned());
assert_eq!(
    to_vec(&data).expect("serialize shouldn't fail"),
    b"-ERROR\r\n",
);

let data: Result<(), String> = Ok(());
assert_eq!(
    to_vec(&data).expect("serialize shouldn't fail"),
    b"+OK\r\n"
)
```
*/

mod output;
pub mod util;

use core::fmt;

use arrayvec::ArrayString;
use displaydoc::Display;
use paste::paste;
use serde::ser;

pub use self::output::Output;
use self::output::Writable;

#[cfg(feature = "std")]
pub use self::output::IoWrite;

#[cfg(feature = "std")]
use thiserror::Error;

use self::util::TupleSeqAdapter;

/// Serialize an object as a RESP byte buffer.
#[cfg(feature = "std")]
pub fn to_vec<T>(data: &T) -> Result<Vec<u8>, Error>
where
    T: ser::Serialize + ?Sized,
{
    let mut buffer = Vec::new();
    let serializer = Serializer::new(&mut buffer);
    data.serialize(serializer)?;
    Ok(buffer)
}

/// Serialize an object as a RESP byte buffer in a [`String`].
///
/// Note that RESP is a binary protocol, so if there is any non-UTF-8
/// data in `data`, the serialization will fail with
/// [`Error::Utf8Encode`]. Most data should be fine, though.
#[cfg(feature = "std")]
pub fn to_string<T>(data: &T) -> Result<String, Error>
where
    T: ser::Serialize + ?Sized,
{
    let mut buffer = String::new();
    let serializer = Serializer::new(&mut buffer);
    data.serialize(serializer)?;
    Ok(buffer)
}

/// Serialize an object as RESP data to an [`io::Write`] destination, such as a
/// [`File`][std::fs::File].
#[cfg(feature = "std")]
pub fn to_writer<T>(data: &T, dest: impl std::io::Write) -> Result<(), Error>
where
    T: ser::Serialize + ?Sized,
{
    let mut dest = IoWrite(dest);
    let serializer = Serializer::new(&mut dest);
    data.serialize(serializer)
}

/// When serializing `Ok(())`, we prefer to serialize it as `"+OK\r\n"`
/// instead of as a null. This trait switches the behavior for serializing a
/// unit, allowing for this behavior
trait UnitBehavior: Sized {
    #[must_use]
    fn unit_payload(self) -> &'static str;
}

/// Serialize a unit as `"$-1\r\n"`
struct NullUnit;

impl UnitBehavior for NullUnit {
    #[inline(always)]
    #[must_use]
    fn unit_payload(self) -> &'static str {
        "$-1\r\n"
    }
}

/// Serialize a unit as `"+OK\r\n"`
struct ResultOkUnit;

impl UnitBehavior for ResultOkUnit {
    #[inline(always)]
    #[must_use]
    fn unit_payload(self) -> &'static str {
        "+OK\r\n"
    }
}

macro_rules! forward_one {
    ($type:ident) => {
        forward_one!{ $type(v: $type) }
    };

    ($method:ident $(<$Generic:ident>)? ($($arg:ident: $type:ty),*)) => {
        forward_one!{ $method $(<$Generic>)? ($($arg: $type),*) -> Ok }
    };

    ($method:ident $(<$Generic:ident>)? ($($arg:ident: $type:ty),*) -> $Ret:ty) => {
        paste! {
            #[inline]
            fn [<serialize_ $method>] $(<$Generic>)? (
                self,
                $($arg: $type,)*
            ) -> Result<Self::$Ret, Self::Error>
            $(
                where $Generic: ser::Serialize + ?Sized
            )?
            {
                self.inner.[<serialize_ $method>]($($arg,)*)
            }
        }
    };
}

macro_rules! forward {
    ($($method:ident $(<$Generic:ident>)? $( ( $($($arg:ident: $type:ty),+ $(,)?)? ) $(-> $Ret:ty)? )?)*) => {
        $(
            forward_one! { $method $(<$Generic>)? $(($($($arg : $type),+)?) $(-> $Ret)? )? }
        )*
    };
}

/// A RESP Serializer.
///
/// This is the core serde [`Serializer`][ser::Serializer] for RESP data.
/// It writes encoded RESP data to any object implementing [`Output`]. This
/// includes [`Vec<u8>`] and [`String`], as well as [`io::Write`] objects
/// (via [`IoWrite`])
///
/// A single `Serializer` can be used to serialize at most one RESP value. They
/// are trivially cheap to create, though, so a new `Serializer` can be used
/// for each additional value.
pub struct Serializer<'a, O> {
    inner: BaseSerializer<'a, O, NullUnit>,
}

impl<'a, O> Serializer<'a, O>
where
    O: Output,
{
    /// Create a new RESP serializer that will write the serialized data to
    /// the given writer.
    #[inline]
    #[must_use]
    pub fn new(writer: &'a mut O) -> Self {
        Self {
            inner: BaseSerializer::new(writer),
        }
    }
}

impl<'a, O> ser::Serializer for Serializer<'a, O>
where
    O: Output,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SerializeSeq<'a, O>;
    type SerializeTuple = TupleSeqAdapter<SerializeSeq<'a, O>>;
    type SerializeTupleStruct = TupleSeqAdapter<SerializeSeq<'a, O>>;

    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;

    type SerializeStructVariant = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;

    forward! {
        bool
        i8 i16 i32 i64 i128
        u8 u16 u32 u64 u128
        f32 f64
        char
        str(v: &str)
        bytes(v: &[u8])
        none()
        some<T>(value: &T)
        unit()
        unit_struct(name: &'static str)
        unit_variant(
            name: &'static str,
            variant_index: u32,
            variant: &'static str
        )
        newtype_struct<T>(name: &'static str, value: &T)
        newtype_variant<T>(
            name: &'static str,
            variant_index: u32,
            variant: &'static str,
            value: &T
        )
        seq(len: Option<usize>) -> SerializeSeq
        tuple(len: usize) -> SerializeTuple
        tuple_struct(name: &'static str, len: usize) -> SerializeTupleStruct
        tuple_variant(
            name: &'static str,
            variant_index: u32,
            variant: &'static str,
            len: usize,
        ) -> SerializeTupleVariant
        map(len: Option<usize>) -> SerializeMap
        struct(name: &'static str, len: usize) -> SerializeStruct
        struct_variant(
            name: &'static str,
            variant_index: u32,
            variant: &'static str,
            len: usize
        ) -> SerializeStructVariant
    }

    #[inline]
    fn collect_map<K, V, I>(self, iter: I) -> Result<Self::Ok, Self::Error>
    where
        K: serde::Serialize,
        V: serde::Serialize,
        I: IntoIterator<Item = (K, V)>,
    {
        self.inner.collect_map(iter)
    }

    #[inline]
    fn collect_seq<I>(self, iter: I) -> Result<Self::Ok, Self::Error>
    where
        I: IntoIterator,
        <I as IntoIterator>::Item: serde::Serialize,
    {
        self.inner.collect_seq(iter)
    }

    #[inline]
    fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: fmt::Display,
    {
        self.inner.collect_str(value)
    }
}

struct BaseSerializer<'a, O, U> {
    output: &'a mut O,
    unit: U,
}

impl<'a, O> BaseSerializer<'a, O, NullUnit>
where
    O: Output,
{
    #[inline]
    #[must_use]
    pub fn new(writer: &'a mut O) -> Self {
        Self {
            output: writer,
            unit: NullUnit,
        }
    }
}

impl<'a, O> BaseSerializer<'a, O, ResultOkUnit>
where
    O: Output,
{
    #[inline]
    #[must_use]
    pub fn new_ok(writer: &'a mut O) -> Self {
        Self {
            output: writer,
            unit: ResultOkUnit,
        }
    }
}

/// Errors that can occur during serialization.
#[derive(Debug, Display)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
#[non_exhaustive]
pub enum Error {
    /// Certain types can't be serialized. The argument contains the kind of
    /// type that failed to serialize.
    #[displaydoc("can't serialize {0}")]
    UnsupportedType(&'static str),

    /// Attempted to serialize a number that was outside the range of a signed
    /// 64 bit integer. Redis integers always fit in this range.
    ///
    /// Don't forget that Redis commands are always a list of strings, even when
    /// they contain numeric data. Consider using
    /// [`RedisString`][crate::components::RedisString] or
    /// [`Command`][crate::components::Command] in this case.
    #[displaydoc("can't serialize numbers outside the range of a signed 64 bit integer")]
    NumberOutOfRange,

    /// Redis arrays are length-prefixed; they must know the length ahead of
    /// time. This error occurs when a sequence is serialized without a known
    /// length. Consider using [`Command`][crate::components::Command] if you're
    /// trying to serialize a Redis command, as it automatically handles
    /// efficiently computing the length of the array (without allocating).
    #[displaydoc("can't serialize sequences of unknown length")]
    UnknownSeqLength,

    /// Attempted to serialize too many or too few sequence elements. This error
    /// occurs when the number of serialized array elements differed from the
    /// prefix-reported length of the array.
    #[displaydoc("attempted to serialize too many or too few sequence elements")]
    BadSeqLength,

    /// Attempted to serialize a RESP [Simple String] or [Error] that contained
    /// a `\r` or `\n`.
    ///
    /// [Simple String]:
    ///     https://redis.io/docs/reference/protocol-spec/#resp-simple-strings
    /// [Error]: https://redis.io/docs/reference/protocol-spec/#resp-errors
    #[displaydoc("attempted to serialize a Simple String that contained a \\r or \\n")]
    BadSimpleString,

    /// There was an i/o error during serialization. Generally this can only
    /// happen when serializing to a "real" i/o device, like a file.
    #[displaydoc("i/o error during serialization")]
    #[cfg(feature = "std")]
    Io(#[from] std::io::Error),

    /// The data being serialized encountered some kind of error, separate from
    /// the RESP protocol.
    #[displaydoc("error from Serialize type: {0}")]
    #[cfg(feature = "std")]
    Custom(String),

    #[displaydoc("error from Serialize type: {0}")]
    #[cfg(not(feature = "std"))]
    Custom(&'static str),

    /// Attempted to serialize something other than a string, bytes, or unit
    /// enum as a RESP [Error].
    ///
    /// [Error]: https://redis.io/docs/reference/protocol-spec/#resp-errors
    #[displaydoc("invalid payload for a Result::Err. Must be a string or simple enum")]
    InvalidErrorPayload,

    /// Attempted to encode non-UTF-8 data. This error can only occur when the
    /// [`Output`] type must be UTF-8 data (such as a [`String`]); most output
    /// types can accept arbitrary bytes.
    #[displaydoc("attempted to encode non-UTF-8 data to a string-like destination")]
    Utf8Encode,
}

impl ser::Error for Error {
    #[inline]
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Self::Custom(msg.to_string())
    }
}

/// Write a redis header containing `value` to the `output`, using the `prefix`.
/// This method will reserve space in the `output` sufficient to contain the
/// header, plus additional space equal to `suffix_reserve`.
fn serialize_header(
    output: &mut impl Output,
    prefix: u8,
    value: impl TryInto<i64>,
    suffix_reserve: usize,
) -> Result<(), Error> {
    let prefix = prefix as char;
    debug_assert!("*:$".contains(prefix));

    let value: i64 = value.try_into().map_err(|_| Error::NumberOutOfRange)?;

    // TODO: better calculation how many digits / characters are required for
    // `value`. This can be based on ilog10 but there's a bunch of edge cases
    // that need to be handled (zero, negatives). For now we conservatively
    // assume it fits in 1 character.
    let width = suffix_reserve.saturating_add(4);

    output.reserve(width);
    write!(output, "{prefix}{value}\r\n")
}

#[inline]
fn serialize_number(output: &mut impl Output, value: impl TryInto<i64>) -> Result<(), Error> {
    serialize_header(output, b':', value, 0)
}

/// Given an array of length `len`, estimate how many bytes are reasonable
/// to reserve in an output buffer that will contain that array. This should
/// *mostly* be the lower bound but can make certain practical estimates about
/// the data that is *likely* to be contained.
#[inline]
#[must_use]
const fn estimate_array_reservation(len: usize) -> usize {
    // By far the most common thing we serialize is a bulk string (for a
    // command), and the smallest bulk string (an empty one) is 6 bytes, so
    // that's the factor we use.
    len.saturating_mul(6)
}

#[inline]
fn serialize_array_header(output: &mut impl Output, len: usize) -> Result<(), Error> {
    serialize_header(output, b'*', len, estimate_array_reservation(len))
}

fn serialize_bulk_string(
    output: &mut impl Output,
    value: &(impl Writable + ?Sized),
) -> Result<(), Error> {
    let len: i64 = value
        .len()
        .try_into()
        .map_err(|_| Error::NumberOutOfRange)?;

    serialize_header(output, b'$', len, value.len().saturating_add(2))?;
    value.write_to_output(output)?;
    output.write_str("\r\n")
}

fn serialize_error(dest: &mut impl Output, value: &(impl Writable + ?Sized)) -> Result<(), Error> {
    if value.safe() {
        dest.reserve(value.len().saturating_add(3));
        dest.write_str("-")?;
        value.write_to_output(dest)?;
        dest.write_str("\r\n")
    } else {
        Err(Error::BadSimpleString)
    }
}

impl<'a, O, U> ser::Serializer for BaseSerializer<'a, O, U>
where
    O: Output,
    U: UnitBehavior,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SerializeSeq<'a, O>;
    type SerializeTuple = TupleSeqAdapter<SerializeSeq<'a, O>>;
    type SerializeTupleStruct = TupleSeqAdapter<SerializeSeq<'a, O>>;

    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;

    type SerializeStructVariant = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, if v { 1 } else { 0 })
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, v)
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, v)
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, v)
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, v)
    }

    #[inline]
    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, v)
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, v)
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, v)
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, v)
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, v)
    }

    #[inline]
    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        serialize_number(self.output, v)
    }

    #[inline]
    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Error::UnsupportedType("f32"))
    }

    #[inline]
    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Error::UnsupportedType("f64"))
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        serialize_bulk_string(self.output, &v)
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        serialize_bulk_string(self.output, v)
    }

    fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: fmt::Display,
    {
        use fmt::Write as _;

        // We assume that things that need to be collected as strings are
        // usually pretty short, so we try first to serialize to a local buffer.
        // In the future we'll also have a #[no_std] switch here that fails
        // outright if Strings can't be created.
        let mut buffer: ArrayString<256> = ArrayString::new();

        match write!(&mut buffer, "{value}") {
            Ok(_) => self.serialize_str(&buffer),
            Err(_) => self.serialize_str(&value.to_string()),
        }
    }

    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        serialize_bulk_string(self.output, v)
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.output.write_str("$-1\r\n")
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
        self.output.write_str(self.unit.unit_payload())
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
            ("Result", "Ok") => value.serialize(BaseSerializer::new_ok(self.output)),
            ("Result", "Err") => value.serialize(SerializeResultError::new(self.output)),
            _ => Err(Error::UnsupportedType("data enum")),
        }
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.serialize_tuple(len.ok_or(Error::UnknownSeqLength)?)
            .map(|adapter| adapter.inner)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        serialize_array_header(self.output, len)?;
        Ok(TupleSeqAdapter::new(SerializeSeq::new(self.output, len)))
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
        Err(Error::UnsupportedType("data enum"))
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::UnsupportedType("map"))
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::UnsupportedType("struct"))
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::UnsupportedType("enum"))
    }
}

/// The RESP sequence serializer. This is used by the [`Serializer`] to create
/// RESP arrays. You should rarely need to interact with this type directly.
#[derive(Debug)]
pub struct SerializeSeq<'a, O> {
    remaining: usize,
    output: &'a mut O,
}

impl<'a, O> SerializeSeq<'a, O>
where
    O: Output,
{
    #[inline]
    #[must_use]
    fn new(output: &'a mut O, length: usize) -> Self {
        Self {
            output,
            remaining: length,
        }
    }
}

impl<O> ser::SerializeSeq for SerializeSeq<'_, O>
where
    O: Output,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.output
            .reserve(estimate_array_reservation(self.remaining));

        match self.remaining.checked_sub(1) {
            Some(remain) => self.remaining = remain,
            None => return Err(Error::BadSeqLength),
        }

        value.serialize(BaseSerializer::new(self.output))
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self.remaining {
            0 => Ok(()),
            _ => Err(Error::BadSeqLength),
        }
    }
}

/// An error serializer only accepts strings / bytes or similar payloads and
/// serializes them as Redis error values.
struct SerializeResultError<'a, O> {
    output: &'a mut O,
}

impl<'a, O> SerializeResultError<'a, O>
where
    O: Output,
{
    pub fn new(output: &'a mut O) -> Self {
        Self { output }
    }
}

impl<O> ser::Serializer for SerializeResultError<'_, O>
where
    O: Output,
{
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
        serialize_error(self.output, v)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        serialize_error(self.output, v)
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

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use std::io::Write;

    use super::*;

    use serde::Serialize;
    use serde_bytes::Bytes;
    use tempfile::tempfile;

    #[derive(Debug, Serialize)]
    #[serde(untagged)]
    enum Data<'a> {
        Null,
        String(&'a Bytes),
        Integer(i64),
        Array(Vec<Data<'a>>),
    }

    use Data::Null;

    impl<'a> From<&'a [u8]> for Data<'a> {
        fn from(value: &'a [u8]) -> Self {
            Self::String(Bytes::new(value))
        }
    }

    impl<'a> From<&'a str> for Data<'a> {
        fn from(value: &'a str) -> Self {
            value.as_bytes().into()
        }
    }

    impl From<i64> for Data<'_> {
        fn from(value: i64) -> Self {
            Self::Integer(value)
        }
    }

    impl<'a, T: Into<Data<'a>>, const N: usize> From<[T; N]> for Data<'a> {
        fn from(value: [T; N]) -> Self {
            Self::Array(value.into_iter().map(Into::into).collect())
        }
    }

    fn test_basic_serialize<'a>(
        input: impl Into<Data<'a>>,
        expected: &'a (impl AsRef<[u8]> + ?Sized),
    ) {
        let input: Data = input.into();
        let mut out = Vec::new();
        let serializer = Serializer::new(&mut out);
        input.serialize(serializer).expect("failed to serialize");
        assert_eq!(out, expected.as_ref())
    }

    macro_rules! data_tests {
        ($($name:ident: $value:expr => $expected:literal;)*) => {
            $(
                #[test]
                fn $name() {
                    test_basic_serialize($value, $expected)
                }
            )*
        };
    }

    data_tests! {
        integer: 1000 => ":1000\r\n";
        negative_int: -1000 => ":-1000\r\n";
        bulk_string: "Hello, World!" => "$13\r\nHello, World!\r\n";
        empty_bulk_string: "" => "$0\r\n\r\n";
        null: Null => "$-1\r\n";
        array: ["hello", "world"] => "\
            *2\r\n\
            $5\r\nhello\r\n\
            $5\r\nworld\r\n\
        ";
        heterogeneous: [
            Data::Integer(10),
            Data::String(Bytes::new(b"hello")),
            Null
        ] => "\
            *3\r\n\
            :10\r\n\
            $5\r\nhello\
            \r\n$-1\r\n\
        ";
        nested_array: [
            ["hello", "world"],
            ["goodbye", "night"],
            ["abc", "def"]
        ] => "\
            *3\r\n\
            *2\r\n\
                $5\r\nhello\r\n\
                $5\r\nworld\r\n\
            *2\r\n\
                $7\r\ngoodbye\r\n\
                $5\r\nnight\r\n\
            *2\r\n\
                $3\r\nabc\r\n\
                $3\r\ndef\r\n\
        ";
    }

    #[test]
    fn test_bool() {
        let mut buffer = Vec::new();
        let serializer = Serializer::new(&mut buffer);
        true.serialize(serializer).expect("failed to serialize");
        assert_eq!(buffer, b":1\r\n");
    }

    #[test]
    fn test_options() {
        let mut buffer = Vec::new();
        let serializer = Serializer::new(&mut buffer);
        let data = Vec::from([
            Some(Data::Integer(3)),
            None,
            Some(Data::String(Bytes::new(b"hello"))),
        ]);
        data.serialize(serializer).expect("failed to serialize");
        assert_eq!(
            buffer,
            b"\
                *3\r\n\
                    :3\r\n\
                    $-1\r\n\
                    $5\r\nhello\r\n\
            "
        );
    }

    fn test_result_serializer<T, E>(input: Result<T, E>, expected: &[u8])
    where
        T: ser::Serialize,
        E: ser::Serialize,
    {
        let mut buffer = Vec::new();
        let serializer = Serializer::new(&mut buffer);
        input.serialize(serializer).expect("failed to serialize");
        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_result_ok() {
        test_result_serializer::<&str, &str>(Ok("hello"), b"$5\r\nhello\r\n");
    }

    #[test]
    fn test_result_some() {
        test_result_serializer::<Option<&str>, &str>(Ok(Some("hello")), b"$5\r\nhello\r\n");
    }

    #[test]
    fn test_result_none() {
        test_result_serializer::<Option<&str>, &str>(Ok(None), b"$-1\r\n");
    }

    #[test]
    fn test_result_unit() {
        test_result_serializer::<(), &str>(Ok(()), b"+OK\r\n");
    }

    #[test]
    fn test_result_error() {
        test_result_serializer::<(), &str>(Err("ERROR bad data"), b"-ERROR bad data\r\n")
    }

    #[test]
    fn test_to_writer() {
        use std::io::Read as _;
        use std::io::Seek as _;

        let mut file = tempfile().expect("failed to create tempfile");

        let data = Vec::from([
            Data::Integer(-5),
            Data::Null,
            Data::String(Bytes::new(b"data")),
        ]);

        to_writer(&data, &mut file).expect("failed to serialize to a file");
        file.flush().expect("failed to flush the file");
        file.rewind().expect("failed to seek the file");
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .expect("failed to read the file");

        assert_eq!(
            buffer,
            b"\
                *3\r\n\
                :-5\r\n\
                $-1\r\n\
                $4\r\ndata\r\n\
            "
        )
    }
}
