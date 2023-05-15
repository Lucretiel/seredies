//! Utility types to support the serializer implementations.

use serde::ser;

/// Utility type intended to reduce serde serializer boilerplate.
///
/// Many of the trait serialization traits are functionally identical. This
/// type wraps some inner type and forwards the trait implementations
/// accordingly. It forwards the following traits:
///
/// - From [`ser::SerializeSeq`]:
///   - [`ser::SerializeTuple`]
///   - [`ser::SerializeTupleStruct`]
///   - [`ser::SerializeTupleVariant`]
/// - From [`ser::SerializeStruct`]:
///   - [`ser::SerializeStructVariant`]
#[derive(Debug, Clone, Copy, Default)]
pub struct TupleSeqAdapter<T> {
    /// The wrapped serializer type. This should implement
    /// [`ser::SerializeSeq`] or [`ser::SerializeStruct`] (or both)
    pub inner: T,
}

impl<T> TupleSeqAdapter<T> {
    /// Create a new `TupleSeqAdapter`
    #[inline]
    #[must_use]
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<S: ser::SerializeSeq> ser::SerializeTuple for TupleSeqAdapter<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.inner.serialize_element(value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<S: ser::SerializeSeq> ser::SerializeTupleStruct for TupleSeqAdapter<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.inner.serialize_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<S: ser::SerializeSeq> ser::SerializeTupleVariant for TupleSeqAdapter<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.inner.serialize_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<S: ser::SerializeStruct> ser::SerializeStructVariant for TupleSeqAdapter<S> {
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.inner.serialize_field(key, value)
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.inner.skip_field(key)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}
