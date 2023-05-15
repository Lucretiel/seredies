use std::{
    any::type_name,
    fmt::{self, Display},
    marker::PhantomData,
    mem,
    str::{from_utf8, FromStr},
};

use arrayvec::{ArrayString, ArrayVec};
use paste::paste;
use serde::{de, forward_to_deserialize_any, ser};

/**
Adapter type that serializes the contained value as a string.

Frequently, especially when sending commands, Redis will require data to be
passed as a string, even when the underlying data is something like an
integer. This type serializes its inner value as a string, and works
on most primitive types. This will serialize unit enum variants and unit
structs as their name.

Note that this type *cannot* distinguish a `[u8]` from other kinds of
slices; be sure to use a container like [`serde_bytes::Bytes`] to ensure
that these slices are serialized as bytes objects rather than sequences.

# Example

```
use seredies::components::RedisString;
use serde::{Serialize, Deserialize};
use serde_test::{assert_tokens, assert_ser_tokens, Token};

assert_tokens(&RedisString("Hello"), &[Token::BorrowedStr("Hello")]);
assert_tokens(&RedisString(5i32), &[Token::Str("5")]);
assert_tokens(&RedisString(4.5), &[Token::Str("4.5")]);

let s: &RedisString<str> = RedisString::new_ref("string");
assert_tokens(&s, &[Token::BorrowedStr("string")]);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct UnitStruct;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
enum Data {
    Foo,
    Bar
}

assert_tokens(&RedisString(UnitStruct), &[Token::Str("UnitStruct")]);
assert_tokens(&RedisString(Data::Bar), &[Token::Str("Bar")]);
```
*/
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct RedisString<T: ?Sized>(pub T);

impl<T: ?Sized> RedisString<T> {
    /// Convert a reference to some underlying type into a reference to a
    /// `RedisString` containing that object. This works even on unsized values
    /// and allows for the creation of things like `&RedisString<str>`.
    pub fn new_ref(value: &T) -> &Self {
        unsafe { mem::transmute(value) }
    }
}

impl<T> ser::Serialize for RedisString<T>
where
    T: ser::Serialize + ?Sized,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(RedisStringAdapter(serializer))
    }
}

impl<'de, T> de::Deserialize<'de> for RedisString<T>
where
    T: de::Deserialize<'de> + ?Sized,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        de::Deserialize::deserialize(RedisStringAdapter(deserializer)).map(RedisString)
    }
}

impl<'de, T: 'de> de::Deserialize<'de> for &'de RedisString<T>
where
    &'de T: de::Deserialize<'de>,
    T: ?Sized,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <&T>::deserialize(RedisStringAdapter(deserializer)).map(RedisString::new_ref)
    }
}

/// Internal adapter type for serializers, deserializers, visitors, etc.
struct RedisStringAdapter<T>(T);

impl<S> RedisStringAdapter<S>
where
    S: ser::Serializer,
{
    fn serialize_number(self, value: impl Display) -> Result<S::Ok, S::Error> {
        // 39 digits should be enough for even a 128 bit int, but we'll round
        // way up to be safe
        use fmt::Write as _;

        let mut buffer: ArrayString<64> = ArrayString::new();

        write!(&mut buffer, "{value}")
            .map_err(|_| ser::Error::custom("integer was more than 64 digits"))?;

        self.0.serialize_str(&buffer)
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
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.serialize_number(v)
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut buffer = [0; 4];
        self.serialize_str(v.encode_utf8(&mut buffer))
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_str(v)
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

impl<'de, D: de::Deserializer<'de>> de::Deserializer<'de> for RedisStringAdapter<D> {
    type Error = D::Error;

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_bytes(visitor)
    }

    forward_to_deserialize_any! {bool option unit seq tuple tuple_struct map struct identifier ignored_any}

    #[inline]
    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, i8>::new(visitor))
    }

    #[inline]
    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, i16>::new(visitor))
    }

    #[inline]
    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, i32>::new(visitor))
    }

    #[inline]
    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, i64>::new(visitor))
    }

    #[inline]
    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, i128>::new(visitor))
    }

    #[inline]
    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, u8>::new(visitor))
    }

    #[inline]
    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, u16>::new(visitor))
    }

    #[inline]
    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, u32>::new(visitor))
    }

    #[inline]
    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, u64>::new(visitor))
    }

    #[inline]
    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, u128>::new(visitor))
    }

    #[inline]
    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, f32>::new(visitor))
    }

    #[inline]
    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_bytes(FromStrVisitor::<_, f64>::new(visitor))
    }

    #[inline]
    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_bytes(StrBytesVisitor::new(visitor))
    }

    #[inline]
    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_bytes(StrBytesVisitor::new(visitor))
    }

    #[inline]
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_byte_buf(StrBytesVisitor::new(visitor))
    }

    #[inline]
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_bytes(visitor)
    }

    #[inline]
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_byte_buf(visitor)
    }

    #[inline]
    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        struct Visitor<V> {
            visitor: V,
            name: &'static str,
        }

        impl<'de, V: de::Visitor<'de>> de::Visitor<'de> for Visitor<V> {
            type Value = V::Value;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a byte slice containing {:?}", self.name)
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v == self.name.as_bytes() {
                    self.visitor.visit_unit()
                } else {
                    Err(de::Error::invalid_value(de::Unexpected::Bytes(v), &self))
                }
            }

            #[inline]
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_bytes(v.as_bytes())
            }
        }

        self.0.deserialize_bytes(Visitor { visitor, name })
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
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        struct Adapter<T, E> {
            inner: T,
            error: PhantomData<E>,
        }

        impl<'de, V: de::Visitor<'de>, E2> de::Visitor<'de> for Adapter<V, E2> {
            type Value = V::Value;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a redis byte slice containing ")?;
                self.inner.expecting(formatter)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_bytes(v.as_bytes())
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.inner.visit_enum(Adapter {
                    inner: v,
                    error: PhantomData,
                })
            }
        }

        impl<'de, 'a, E: de::Error> de::EnumAccess<'de> for Adapter<&'a [u8], E> {
            type Error = E;
            type Variant = Adapter<(), E>;

            fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
            where
                V: de::DeserializeSeed<'de>,
            {
                seed.deserialize(de::value::BytesDeserializer::new(self.inner))
                    .map(|value| {
                        (
                            value,
                            Adapter {
                                inner: (),
                                error: PhantomData,
                            },
                        )
                    })
            }
        }

        impl<'de, E: de::Error> de::VariantAccess<'de> for Adapter<(), E> {
            type Error = E;

            fn unit_variant(self) -> Result<(), Self::Error> {
                Ok(())
            }

            fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, Self::Error>
            where
                T: de::DeserializeSeed<'de>,
            {
                Err(de::Error::invalid_type(
                    de::Unexpected::UnitVariant,
                    &"a newtype variant",
                ))
            }

            fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
            where
                V: de::Visitor<'de>,
            {
                Err(de::Error::invalid_type(
                    de::Unexpected::UnitVariant,
                    &"a tuple variant",
                ))
            }

            fn struct_variant<V>(
                self,
                _fields: &'static [&'static str],
                _visitor: V,
            ) -> Result<V::Value, Self::Error>
            where
                V: de::Visitor<'de>,
            {
                Err(de::Error::invalid_type(
                    de::Unexpected::UnitVariant,
                    &"a struct variant",
                ))
            }
        }

        self.0.deserialize_bytes(Adapter {
            inner: visitor,
            error: PhantomData::<()>,
        })
    }
}

trait VisitTo {
    fn apply_to_visitor<'de, V, E>(self, visitor: V) -> Result<V::Value, E>
    where
        V: de::Visitor<'de>,
        E: de::Error;
}

macro_rules! impl_visit_to {
    ($($type:ident)*) => {
        $(
            paste!{
                impl VisitTo for $type {
                    fn apply_to_visitor<'de, V, E>(self, visitor: V) -> Result<V::Value, E>
                    where
                        V: de::Visitor<'de>,
                        E: de::Error
                    {
                        visitor.[<visit_ $type>](self)
                    }
                }
            }
        )*
    }
}

impl_visit_to! {
    u8 u16 u32 u64 u128
    i8 i16 i32 i64 i128
    f32 f64
}

/// A visitor that tries to convert the bytes data it receives into a string
/// before forwarding it to the underlying visitor.
struct StrBytesVisitor<V> {
    visitor: V,
}

impl<'de, V: de::Visitor<'de>> StrBytesVisitor<V> {
    pub fn new(visitor: V) -> Self {
        Self { visitor }
    }
}

impl<'de, V> de::Visitor<'de> for StrBytesVisitor<V>
where
    V: de::Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.visitor.expecting(formatter)
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let s = from_utf8(v)
            .map_err(|_err| de::Error::invalid_value(de::Unexpected::Bytes(v), &self.visitor))?;
        self.visitor.visit_str(s)
    }

    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let s = from_utf8(v)
            .map_err(|_err| de::Error::invalid_value(de::Unexpected::Bytes(v), &self.visitor))?;
        self.visitor.visit_borrowed_str(s)
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let s = String::from_utf8(v).map_err(|err| {
            de::Error::invalid_value(de::Unexpected::Bytes(err.as_bytes()), &self.visitor)
        })?;

        self.visitor.visit_string(s)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visitor.visit_str(v)
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visitor.visit_borrowed_str(v)
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visitor.visit_string(v)
    }
}

/// A visitor that expects a string and converts it to `T` with `FromStr` and
/// forwards it to the underlying visitor.
struct FromStrVisitor<V, T> {
    inner: V,
    kind: PhantomData<T>,
}

impl<'de, V, T> FromStrVisitor<V, T>
where
    V: de::Visitor<'de>,
    T: FromStr + VisitTo,
{
    fn new(visitor: V) -> Self {
        Self {
            inner: visitor,
            kind: PhantomData,
        }
    }
}

impl<'de, V, T> de::Visitor<'de> for FromStrVisitor<V, T>
where
    V: de::Visitor<'de>,
    T: FromStr + VisitTo,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "{}", type_name::<T>())
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let value: T = v
            .parse()
            .map_err(|_err| de::Error::invalid_value(de::Unexpected::Str(v), &self))?;

        value.apply_to_visitor(self.inner)
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let s = from_utf8(v)
            .map_err(|_err| de::Error::invalid_value(de::Unexpected::Bytes(v), &self.inner))?;

        self.visit_str(s)
    }
}
