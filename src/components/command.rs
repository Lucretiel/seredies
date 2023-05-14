mod length;

use lazy_format::lazy_format;
use serde::ser;
use serde_bytes::Bytes;

use crate::ser::util::TupleSeqAdapter;

use super::RedisString;

/**
Adapter type for serializing redis commands

Redis commands are always delivered as a list of strings, regardless of
the structure of the underlying command. This adapter type makes it easier
to implement commands by serializing the underlying type as though it was
a redis command. It uses the following rules & conventions:

- The type should be a struct or enum. The name of the struct or name of
  the enum will be used as the command name. Tuples, lists, and maps cannot
  be commands.
- The fields in the type will be serialized as arguments, using these rules:
  - Fields will be serialized in order, and the field names will be ignored.
  - Primitive types like strings and numbers will be serialized as strings.
  - Booleans are treated like flags, and will serialize the name of the
    field if true.
  - Options are treated like arguments. If the value is a primitive type,
    like a string or int, it will be serialized in a pair with its field
    name (if present); if it's a struct or enum type, the struct name or
    variant name will be used as the argument (see examples).
  - Enums will serialize the name of the enum, followed by its value (if
    present)
  - Lists will be flattened one level. Nested lists are an error.
  - Maps will be flattened to key-value sequences. Nested maps are an error.

# Examples

## `SET`

This example shows the Redis [`SET` command](https://redis.io/commands/set/).

```
use seredies::components::{RedisString, Command};

use serde::Serialize;
use serde_test::{assert_ser_tokens, Token};

/// The SET command. It includes a key and a value, as well as some optional
/// behavior flags. Notice the use of `serde(rename)` to match the redis flag
/// names, and `into="Command"` to wrap it in a command for serializing.
#[derive(Serialize)]
#[serde(rename = "SET")]
struct Set<T> {
    // The key to set
    key: String,

    // The value to set. `Command` can correctly handle most primitive types,
    // including strings and ints
    value: T,

    // An optional enum will be serialized using just the variant name itself.
    skip: Option<Skip>,

    // If true, a bool field is serialized as the field name itself
    #[serde(rename="GET")]
    get: bool,

    // An enum with a value will be serialized as a key-value pair, if present
    expiry: Option<Expiry>,
}

/// The `skip` parameter determines if the `SET` should be skipped
#[derive(Serialize)]
enum Skip {
    #[serde(rename="NX")]
    IfExists,

    #[serde(rename="XX")]
    IfAbsent,
}

/// The `expiry` parameter sets a time-to-live on the setting
#[derive(Serialize)]
enum Expiry {
    #[serde(rename = "EX")]
    Seconds(u64),

    #[serde(rename = "PX")]
    Millis(u64),

    #[serde(rename = "EXAT")]
    Timestamp(u64),

    #[serde(rename = "PXAT")]
    TimestampMillis(u64),

    #[serde(rename = "KEEPTTL")]
    Keep,
}

let command = Command(Set{
    key: "my-key".to_owned(),
    value: 36,

    skip: None,
    expiry: None,
    get: false,
});

// This will be serialized as a list of byte arrays, and can therefore be sent
// to the seredies RESP serializer
assert_ser_tokens(&command, &[
    Token::Seq { len: Some(3) },
    Token::Bytes(b"SET"),
    Token::Bytes(b"my-key"),
    Token::Bytes(b"36"),
    Token::SeqEnd,
]);

// A more complex example
let command = Command(Set{
    key: "my-key".to_owned(),
    value: 36,

    skip: Some(Skip::IfExists),
    expiry: Some(Expiry::Seconds(60)),
    get: true,
});

assert_ser_tokens(&command, &[
    Token::Seq{len: Some(7)},
    Token::Bytes(b"SET"),
    Token::Bytes(b"my-key"),
    Token::Bytes(b"36"),
    Token::Bytes(b"NX"),
    Token::Bytes(b"GET"),
    Token::Bytes(b"EX"),
    Token::Bytes(b"60"),
    Token::SeqEnd,
]);

```

## `SCAN`

This example shows the Redis [`SCAN` command](https://redis.io/commands/scan/).
In particular it shows the behavior of optional primitive types and how they
differ from optional enums.

```
use serde::Serialize;
use seredies::components::Command;
use serde_test::{assert_ser_tokens, Token};

#[derive(Serialize, Default)]
#[serde(rename="SCAN")]
struct Scan {
    cursor: u64,

    #[serde(rename="MATCH")]
    pattern: Option<String>,

    #[serde(rename="COUNT")]
    count: Option<u32>,

    #[serde(rename="TYPE")]
    kind: Option<String>,
}

let command = Command(Scan{
    cursor: 0,
    ..Default::default()
});

assert_ser_tokens(&command, &[
    Token::Seq { len: Some(2) },
    Token::Bytes(b"SCAN"),
    Token::Bytes(b"0"),
    Token::SeqEnd
]);

let command = Command(Scan{
    cursor: 10,
    count: Some(100),
    kind: Some("zkey".to_string()),
    pattern: None,
});

assert_ser_tokens(&command, &[
    Token::Seq { len: Some(6) },
    Token::Bytes(b"SCAN"),
    Token::Bytes(b"10"),
    Token::Bytes(b"COUNT"),
    Token::Bytes(b"100"),
    Token::Bytes(b"TYPE"),
    Token::Bytes(b"zkey"),
    Token::SeqEnd
]);
```
*/
#[derive(Debug, Copy, Clone, Default)]
pub struct Command<T>(pub T);

impl<T> From<T> for Command<T>
where
    T: ser::Serialize,
{
    fn from(cmd: T) -> Self {
        Self(cmd)
    }
}

impl<T> ser::Serialize for Command<T>
where
    T: ser::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let length = self
            .0
            .serialize(CommandSerializer {
                serializer: length::Serializer,
                length: None,
            })
            .map_err(|err| match err {
                length::Error::Custom(msg) => ser::Error::custom(msg),
                err => ser::Error::custom(err),
            })?;

        self.0.serialize(CommandSerializer {
            serializer,
            length: Some(length),
        })
    }
}

fn invalid_command_type<T, E: ser::Error>(kind: &str) -> Result<T, E> {
    Err(ser::Error::custom(lazy_format!(
        "cannot serialize {kind} as a Redis command"
    )))
}

struct CommandSerializer<S> {
    serializer: S,
    length: Option<usize>,
}

impl<S> ser::Serializer for CommandSerializer<S>
where
    S: ser::Serializer,
{
    type Ok = S::Ok;
    type Error = S::Error;

    type SerializeSeq = ser::Impossible<S::Ok, Self::Error>;
    type SerializeTuple = ser::Impossible<S::Ok, Self::Error>;
    type SerializeMap = ser::Impossible<S::Ok, Self::Error>;

    type SerializeStruct = CommandSequencer<S::SerializeSeq>;
    type SerializeTupleStruct = TupleSeqAdapter<CommandSequencer<S::SerializeSeq>>;
    type SerializeStructVariant = TupleSeqAdapter<CommandSequencer<S::SerializeSeq>>;
    type SerializeTupleVariant = TupleSeqAdapter<CommandSequencer<S::SerializeSeq>>;

    #[inline]
    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("a bool")
    }

    #[inline]
    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("an i8")
    }

    #[inline]
    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("an i16")
    }

    #[inline]
    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("an i32")
    }

    #[inline]
    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("an i64")
    }

    #[inline]
    fn serialize_i128(self, _v: i128) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("an i128")
    }

    #[inline]
    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("a u8")
    }

    #[inline]
    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("a u16")
    }

    #[inline]
    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("a u32")
    }

    #[inline]
    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("a u64")
    }

    #[inline]
    fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("a u128")
    }

    #[inline]
    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("an f32")
    }

    #[inline]
    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("an f64")
    }

    #[inline]
    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("a character")
    }

    #[inline]
    fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("a string")
    }

    #[inline]
    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("a bytes")
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("an option")
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        invalid_command_type("an option")
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        invalid_command_type("a unit")
    }

    #[inline]
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        use ser::SerializeStruct as _;

        self.serialize_struct(name, 0)?.end()
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
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        use ser::SerializeTupleStruct as _;

        let mut receiver = self.serialize_tuple_struct(name, 1)?;
        receiver.serialize_field(value)?;
        receiver.end()
    }

    #[inline]
    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        self.serialize_newtype_struct(variant, value)
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        invalid_command_type("a sequence")
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        use ser::SerializeSeq as _;

        let mut sequence = self.serializer.serialize_seq(self.length)?;
        sequence.serialize_element(RedisString::new_ref(name))?;
        Ok(TupleSeqAdapter::new(CommandSequencer { sequence }))
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.serialize_tuple_struct(variant, len)
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        invalid_command_type("a map")
    }

    #[inline]
    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        use ser::SerializeSeq as _;

        let mut sequence = self.serializer.serialize_seq(self.length)?;
        sequence.serialize_element(RedisString::new_ref(name))?;
        Ok(CommandSequencer { sequence })
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.serialize_struct(variant, len)
            .map(TupleSeqAdapter::new)
    }
}

/// This type implements SerializeSeq, SerializeStruct, etc. It's used to
/// sequence the set of arguments passed to a command. This object is created
/// *after* the command name itself is serialized.
struct CommandSequencer<S: ser::SerializeSeq> {
    sequence: S,
}

impl<S> ser::SerializeSeq for CommandSequencer<S>
where
    S: ser::SerializeSeq,
{
    type Ok = S::Ok;
    type Error = S::Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(AnonymousParameterSerializer::new(&mut self.sequence))
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.sequence.end()
    }
}

impl<S> ser::SerializeStruct for CommandSequencer<S>
where
    S: ser::SerializeSeq,
{
    type Ok = S::Ok;
    type Error = S::Error;

    #[inline]
    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(NamedParameterSerializer::new(key, &mut self.sequence))
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.sequence.end()
    }
}

trait ParameterName: Copy {
    #[must_use]
    fn get(self) -> Option<&'static str>;
}

impl ParameterName for () {
    #[inline(always)]
    #[must_use]
    fn get(self) -> Option<&'static str> {
        None
    }
}

impl ParameterName for &'static str {
    #[inline(always)]
    #[must_use]
    fn get(self) -> Option<&'static str> {
        Some(self)
    }
}

/// This serializer handles a single parameter. It especially handles all the
/// logic for variadic parameters (as in a list of keys for MGET), optional
/// parameters, etc.
struct CommandParameterSerializer<'a, S, N: ParameterName> {
    sequence: &'a mut S,
    name: N,
}

type AnonymousParameterSerializer<'a, S> = CommandParameterSerializer<'a, S, ()>;
type NamedParameterSerializer<'a, S> = CommandParameterSerializer<'a, S, &'static str>;

impl<'a, S: ser::SerializeSeq> AnonymousParameterSerializer<'a, S> {
    #[inline]
    #[must_use]
    pub fn new(sequence: &'a mut S) -> Self {
        Self { sequence, name: () }
    }
}

impl<'a, S: ser::SerializeSeq> NamedParameterSerializer<'a, S> {
    #[inline]
    #[must_use]
    pub fn new(name: &'static str, sequence: &'a mut S) -> Self {
        Self { sequence, name }
    }
}

impl<'a, S, N> CommandParameterSerializer<'a, S, N>
where
    N: ParameterName,
{
    #[inline]
    fn name<E: ser::Error>(&self) -> Result<&'static str, E> {
        self.name.get().ok_or_else(|| {
            ser::Error::custom(
                "can't serialize a bool, optional parameter, \
                or unit from a tuple struct",
            )
        })
    }
}

impl<'a, S, N> ser::Serializer for CommandParameterSerializer<'a, S, N>
where
    S: ser::SerializeSeq,
    N: ParameterName,
{
    type Ok = ();
    type Error = S::Error;

    type SerializeSeq = VariadicParameter<'a, S>;
    type SerializeTuple = TupleSeqAdapter<VariadicParameter<'a, S>>;
    type SerializeTupleStruct = TupleSeqAdapter<VariadicParameter<'a, S>>;

    type SerializeMap = VariadicParameter<'a, S>;
    type SerializeStruct = VariadicParameter<'a, S>;

    type SerializeTupleVariant = ser::Impossible<(), Self::Error>;
    type SerializeStructVariant = ser::Impossible<(), Self::Error>;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        // Even if `v` is false, we want to disallow serializing a bool parameter
        // without a name, so compute it eagerly
        let name = self.name()?;

        match v {
            true => self.sequence.serialize_element(RedisString::new_ref(name)),
            false => Ok(()),
        }
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(&RedisString(v))
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut buf = [0; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(RedisString::new_ref(v))
    }

    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.sequence.serialize_element(Bytes::new(v))
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(OptionalParameterSerializer {
            name: self.name,
            sequence: self.sequence,
        })
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        let name = self.name()?;
        self.serialize_unit_struct(name)
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
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        self.sequence
            .serialize_element(RedisString::new_ref(variant))?;
        self.sequence.serialize_element(RedisString::new_ref(value))
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(VariadicParameter {
            sequence: self.sequence,
        })
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len)).map(TupleSeqAdapter::new)
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
        Err(ser::Error::custom(
            "can't serialize complex enums as Redis command parameters",
        ))
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(VariadicParameter {
            sequence: self.sequence,
        })
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(VariadicParameter {
            sequence: self.sequence,
        })
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(ser::Error::custom(
            "can't serialize complex enums as Redis command parameters",
        ))
    }
}

struct VariadicParameter<'a, S> {
    sequence: &'a mut S,
}

impl<'a, S> ser::SerializeSeq for VariadicParameter<'a, S>
where
    S: ser::SerializeSeq,
{
    type Ok = ();
    type Error = S::Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.sequence.serialize_element(RedisString::new_ref(value))
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a, S: ser::SerializeSeq> ser::SerializeMap for VariadicParameter<'a, S> {
    type Ok = ();
    type Error = S::Error;

    #[inline]
    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.sequence.serialize_element(RedisString::new_ref(key))
    }

    #[inline]
    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.sequence.serialize_element(RedisString::new_ref(value))
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a, S> ser::SerializeStruct for VariadicParameter<'a, S>
where
    S: ser::SerializeSeq,
{
    type Ok = ();
    type Error = S::Error;

    #[inline]
    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.sequence.serialize_element(RedisString::new_ref(key))?;
        self.sequence.serialize_element(RedisString::new_ref(value))
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

/// Serializer for an optional parameter that is Some. Optional parameters
/// always need to have a name associated with them, but in some cases the
/// name is part of the type, rather than part of the containing struct.
/// additionally, optionals can't contain variadic data; otherwise we'd probably
/// just reuse CommandParameterSerializer. We do
struct OptionalParameterSerializer<'a, S, N: ParameterName> {
    sequence: &'a mut S,
    name: N,
}

impl<'a, N: ParameterName, S> OptionalParameterSerializer<'a, S, N> {
    #[inline]
    fn name<E: ser::Error>(&self) -> Result<&'static str, E> {
        self.name.get().ok_or_else(|| {
            ser::Error::custom(
                "can't serialize an optional primitive value \
                in a tuple struct command; it needs a name",
            )
        })
    }
}

impl<'a, S: ser::SerializeSeq, N: ParameterName> OptionalParameterSerializer<'a, S, N> {
    #[inline]
    fn serialize_anonymous_value<T: ser::Serialize + ?Sized>(
        self,
        value: &T,
    ) -> Result<(), S::Error> {
        let name = self.name()?;
        self.serialize_named_value(name, value)
    }

    #[inline]
    fn serialize_named_value<T: ser::Serialize + ?Sized>(
        self,
        name: &str,
        value: &T,
    ) -> Result<(), S::Error> {
        self.sequence
            .serialize_element(RedisString::new_ref(name))?;
        self.sequence.serialize_element(RedisString::new_ref(value))
    }

    #[inline]
    fn serialize_just_anonymous(self) -> Result<(), S::Error> {
        let name = self.name()?;
        self.serialize_just_name(name)
    }

    #[inline]
    fn serialize_just_name(self, name: &str) -> Result<(), S::Error> {
        self.sequence.serialize_element(RedisString::new_ref(name))
    }
}

impl<'a, S: ser::SerializeSeq, N: ParameterName> ser::Serializer
    for OptionalParameterSerializer<'a, S, N>
{
    type Ok = ();
    type Error = S::Error;

    type SerializeSeq = ser::Impossible<(), S::Error>;
    type SerializeTuple = ser::Impossible<(), S::Error>;
    type SerializeTupleStruct = ser::Impossible<(), S::Error>;
    type SerializeTupleVariant = ser::Impossible<(), S::Error>;
    type SerializeMap = ser::Impossible<(), S::Error>;
    type SerializeStruct = ser::Impossible<(), S::Error>;
    type SerializeStructVariant = ser::Impossible<(), S::Error>;

    #[inline]
    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(ser::Error::custom("can't serialize an Option<bool>"))
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(&v)
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(v)
    }

    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.serialize_anonymous_value(Bytes::new(v))
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_just_anonymous()
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        self.serialize_anonymous_value(value)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_just_anonymous()
    }

    #[inline]
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_just_name(name)
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_just_name(variant)
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
        self.serialize_named_value(name, value)
    }

    #[inline]
    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        self.serialize_named_value(variant, value)
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(ser::Error::custom("can't serialize optional sequences"))
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(ser::Error::custom("can't serialize optional tuples"))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(ser::Error::custom(
            "can't serialize optional complex structs",
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
        Err(ser::Error::custom("can't serialize optional complex enums"))
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(ser::Error::custom("can't serialize optional maps"))
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(ser::Error::custom("can't serialize optional structs"))
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(ser::Error::custom("can't serialize optional complex enums"))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    use serde::Serialize;
    use serde_test::{assert_ser_tokens, assert_ser_tokens_error, Token};

    #[derive(Serialize)]
    #[allow(dead_code)]
    enum Skip {
        NX,
        XX,
    }

    #[derive(Serialize)]
    #[allow(dead_code)]
    enum Expiry {
        #[serde(rename = "EX")]
        Seconds(u64),

        #[serde(rename = "PX")]
        Millis(u64),

        #[serde(rename = "EXAT")]
        Timestamp(u64),

        #[serde(rename = "PXAT")]
        TimestampMillis(u64),

        #[serde(rename = "KEEPTTL")]
        Keep,
    }

    #[derive(Serialize)]
    #[serde(rename = "SET")]
    struct Set {
        key: String,
        value: i32,
        /// If true, return the old value of the key after the SET
        #[serde(rename = "GET")]
        get: bool,
        skip: Option<Skip>,
        expiry: Option<Expiry>,
    }

    #[test]
    fn test_basic_set() {
        let command = Command(Set {
            key: "my-key".to_owned(),
            value: 36,
            get: false,
            skip: None,
            expiry: None,
        });

        assert_ser_tokens(
            &command,
            &[
                Token::Seq { len: Some(3) },
                Token::Bytes(b"SET"),
                Token::Bytes(b"my-key"),
                Token::Bytes(b"36"),
                Token::SeqEnd,
            ],
        );
    }

    #[test]
    fn test_set_params() {
        let command = Command(Set {
            key: "my-key".to_owned(),
            value: -10,
            get: true,
            skip: Some(Skip::XX),
            expiry: Some(Expiry::Seconds(60)),
        });

        assert_ser_tokens(
            &command,
            &[
                Token::Seq { len: Some(7) },
                Token::Bytes(b"SET"),
                Token::Bytes(b"my-key"),
                Token::Bytes(b"-10"),
                Token::Bytes(b"GET"),
                Token::Bytes(b"XX"),
                Token::Bytes(b"EX"),
                Token::Bytes(b"60"),
                Token::SeqEnd,
            ],
        )
    }

    #[derive(Serialize)]
    #[serde(rename = "HMSET")]
    struct HashMultiSet {
        key: &'static str,
        values: BTreeMap<&'static str, &'static str>,
    }

    #[test]
    fn test_variadic() {
        let command = Command(HashMultiSet {
            key: "hash-key",
            values: BTreeMap::from([
                ("key1", "value1"),
                ("key2", "value2"),
                ("key3", "value3"),
                ("key4", "value4"),
            ]),
        });

        assert_ser_tokens(
            &command,
            &[
                Token::Seq { len: Some(10) },
                Token::Bytes(b"HMSET"),
                Token::Bytes(b"hash-key"),
                Token::Bytes(b"key1"),
                Token::Bytes(b"value1"),
                Token::Bytes(b"key2"),
                Token::Bytes(b"value2"),
                Token::Bytes(b"key3"),
                Token::Bytes(b"value3"),
                Token::Bytes(b"key4"),
                Token::Bytes(b"value4"),
                Token::SeqEnd,
            ],
        )
    }

    #[derive(Serialize)]
    struct Fake {
        data: Vec<Vec<u8>>,
    }

    #[test]
    fn disallow_nested_lists() {
        let command = Command(Fake {
            data: Vec::from([Vec::from([1, 2, 3]), Vec::from([4, 5, 6])]),
        });

        assert_ser_tokens_error(&command, &[], "can't serialize lists as redis strings");
    }
}
