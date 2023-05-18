/*!
`seredies` is a low-level implementation of
[RESP](https://redis.io/docs/reference/protocol-spec/), the Redis
Serialization Protocol. It is implemented using the [serde] data model, as
a [`Serializer`][crate::ser::Serializer] and
[`Deserializer`][crate::de::Deserializer].

See the [de] and [ser] modules for examples on how to serialize and deserialize
RESP data.

# Faithful

`seredies` is a mostly faithful serde implementation of RESP. This means that
it (mostly) doesn't try to go above and beyond what the RESP data model can
express, which is mostly strings, integers, and arrays. In particular it's not
capable of deserializing structs, maps, or complex enums. Instead, `seredies`
provides a collection of [components][crate::components], which translate
common patterns into Redis's minimal data model. This ensures that developers
should never be surprised by the deserializer trying to do something
unexpectedly "clever", but can opt-in to more streamlined behavior.

## Supported types

These are the types supported by the [serializer][ser::Serializer] and
[deserializer][de::Deserializer].

- `bool` (treated as an integer 0 or 1).
- All integers (though note that RESP only supports integers in the signed
  64 bit range).
- Unit (treated as null).
- Sequences, tuples, and tuple structs.
- Bytes and string types.
    - See the [RedisString][crate::components::RedisString] component for a
      wrapper type that converts any primitive value to or from a Redis string.
    - RESP is totally binary safe, so it's easy to deserialize `&str` and other
      borrowed data from the payload.
- [`Result`] (see below).
- [`Option`]: similar to JSON, an [`Option`] is handled as either a null or as
  an untagged value
- Unit variants: these are encoded as strings.

## Unsupported types

- Floats.
    - Consider [RedisString][crate::components::RedisString] for the common
      case that Redis is treating your float data as a string.
- Maps, structs, complex enums.
    - Consider [KeyValuePairs][crate::components::KeyValuePairs] for the common
      case that your key-value data is being treated by Redis as a flattened
      array of key-value pairs.

If you're trying to serialize a Redis command, consider additionally using the
[Command][crate::components::Command] component; it handles converting all
of the command data into a list of strings, using conventions that follow the
typical redis command conventions.

# Errors and Results

RESP includes an [error type], which is delivered in the response when
something has gone wrong. By default, when deserializing, this error type
is treated as a deserialize error, and appears as the
[`Error::Redis`][crate::de::Error::Redis] variant when encountered. However,
you can handle them by (de)serializing a [`Result`] directly; in this case,
the [`Ok`] variant will contain the data, and a successfully (de)serialized
[`Err`] variant will contain a redis error.

Additionally, seredies ubiquitously uses the simple string "OK" to signal an
uninteresting success. This pattern is so common that `seredies` supports
(de)serializing it directly to an `Ok(())` [`Result`] value.

[error type]: https://redis.io/docs/reference/protocol-spec/#resp-errors
*/

#![deny(missing_docs)]
#![cfg_attr(not(feature="std"), no_std)]

pub mod components;
pub mod de;
pub mod ser;
