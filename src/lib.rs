//! `seredies` is a low-level implementation of
//! [RESP](https://redis.io/docs/reference/protocol-spec/), the Redis
//! Serialization Protocol. It is implemented using the [serde] data model, as
//! a [`Serializer`][crate::ser::Serializer] and
//! [`Deserializer`][crate::de::Deserializer].
//!
//! Because these are faithful, low-level implementations of the protocol,
//! `seredies` also provides a collection of [components][components]. These
//! are wrapper types implementing common patterns, especially related to
//! serializing Redis commands (which are technically ordinary RESP objects
//! but which follow a distinct set of conventions from ordinary RESP data).

//#![deny(missing_docs)]

pub mod components;
pub mod de;
pub mod ser;
