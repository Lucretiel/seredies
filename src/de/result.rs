// Helpers for deserializing results from RESP values (and in particular
// for handling `-ERR message\r\n` as an error)

use serde::{de, forward_to_deserialize_any};

use super::{Error, PreParsedDeserializer};

pub(super) struct ResultAccess<T> {
    access: T,
}

impl<T> ResultAccess<T> {
    #[inline]
    #[must_use]
    fn new(access: T) -> Self {
        Self { access }
    }
}

impl ResultAccess<ResultPlainOkPattern> {
    #[inline]
    #[must_use]
    pub fn new_plain_ok() -> Self {
        Self::new(ResultPlainOkPattern)
    }
}

impl<'a, 'de> ResultAccess<ResultOkPattern<'a, 'de>> {
    #[inline]
    #[must_use]
    pub fn new_ok(deserializer: PreParsedDeserializer<'a, 'de>) -> Self {
        Self::new(ResultOkPattern { deserializer })
    }
}

impl<'de> ResultAccess<ResultErrPattern<'de>> {
    #[inline]
    #[must_use]
    pub fn new_err(message: &'de [u8]) -> Self {
        Self::new(ResultErrPattern { message })
    }
}

impl<'de, T: ResultAccessPattern<'de>> de::EnumAccess<'de> for ResultAccess<T> {
    type Error = Error;
    type Variant = Self;

    #[inline]
    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        seed.deserialize(de::value::BorrowedStrDeserializer::new(T::VARIANT))
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
        Err(de::Error::invalid_type(
            de::Unexpected::NewtypeVariant,
            &"unit variant for Result::Ok",
        ))
    }

    #[inline]
    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(de::Error::invalid_type(
            de::Unexpected::NewtypeVariant,
            &visitor,
        ))
    }

    #[inline]
    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(de::Error::invalid_type(
            de::Unexpected::NewtypeVariant,
            &visitor,
        ))
    }
}

trait ResultAccessPattern<'de> {
    /// The name of the result variant being accessed, either `Ok` or `Err`.
    const VARIANT: &'static str;

    fn value<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: de::DeserializeSeed<'de>;
}

pub struct ResultPlainOkPattern;

impl<'de> ResultAccessPattern<'de> for ResultPlainOkPattern {
    const VARIANT: &'static str = "Ok";

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
        option unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any enum
    }

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_borrowed_str("OK")
    }

    #[inline]
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(b"OK")
    }

    #[inline]
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    #[inline]
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

pub struct ResultOkPattern<'a, 'de> {
    deserializer: PreParsedDeserializer<'a, 'de>,
}

impl<'de> ResultAccessPattern<'de> for ResultOkPattern<'_, 'de> {
    const VARIANT: &'static str = "Ok";

    #[inline]
    fn value<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self.deserializer)
    }
}

pub struct ResultErrPattern<'de> {
    message: &'de [u8],
}

impl<'de> ResultAccessPattern<'de> for ResultErrPattern<'de> {
    const VARIANT: &'static str = "Err";

    #[inline]
    fn value<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(de::value::BorrowedBytesDeserializer::new(self.message))
    }
}
