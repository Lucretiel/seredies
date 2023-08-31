use std::{fmt, io};

use super::Error;

/// The [`Output`] trait is used as a destination for writing bytes by the
/// [`Serializer`]. It serves a similar role as [`io::Write`] or [`fmt::Write`],
/// but allows for the serializer to work in `#[no_std]` contexts.
pub trait Output {
    /// Hint that there are upcoming writes totalling this number of
    /// bytes.
    fn reserve(&mut self, count: usize);

    /// Append string data to the output.
    fn write_str(&mut self, s: &str) -> Result<(), Error>;

    /// Append bytes data to the output.
    fn write_bytes(&mut self, b: &[u8]) -> Result<(), Error>;

    /// Append formatted data to the output. This method allows
    /// [`Output`] objects to be used as the destination of a [`write!`] call.
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> Result<(), Error> {
        if let Some(s) = fmt.as_str() {
            return self.write_str(s);
        }

        struct Adapter<T> {
            output: T,
            result: Result<(), Error>,
        }

        impl<T: Output> fmt::Write for Adapter<T> {
            #[inline]
            fn write_str(&mut self, s: &str) -> fmt::Result {
                self.output.write_str(s).map_err(|err| {
                    self.result = Err(err);
                    fmt::Error
                })
            }
        }

        let mut adapter = Adapter {
            output: self,
            result: Ok(()),
        };

        let res = fmt::write(&mut adapter, fmt);

        debug_assert!(match adapter.result.as_ref() {
            Ok(()) => res.is_ok(),
            Err(_) => res.is_err(),
        });

        adapter.result
    }

    // TODO: vectored write support
}

impl<T: Output + ?Sized> Output for &mut T {
    #[inline]
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        T::write_str(*self, s)
    }

    #[inline]
    fn write_bytes(&mut self, b: &[u8]) -> Result<(), Error> {
        T::write_bytes(*self, b)
    }

    #[inline]
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> Result<(), Error> {
        T::write_fmt(*self, fmt)
    }

    #[inline]
    fn reserve(&mut self, count: usize) {
        T::reserve(*self, count)
    }
}

impl Output for Vec<u8> {
    #[inline]
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        self.write_bytes(s.as_bytes())
    }

    #[inline]
    fn write_bytes(&mut self, s: &[u8]) -> Result<(), Error> {
        self.extend_from_slice(s);
        Ok(())
    }

    #[inline]
    fn reserve(&mut self, count: usize) {
        self.reserve(count)
    }
}

impl Output for String {
    #[inline]
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        self.push_str(s);
        Ok(())
    }

    #[inline]
    fn write_bytes(&mut self, b: &[u8]) -> Result<(), Error> {
        self.write_str(std::str::from_utf8(b).map_err(|_| Error::Utf8Encode)?)
    }

    #[inline]
    fn reserve(&mut self, count: usize) {
        self.reserve(count)
    }
}

/// [`Output`] adapter type for serializing to an [`io::Write`] object, such as a file
/// or pipeline.
#[derive(Debug, Clone, Copy, Default)]
pub struct IoWrite<T>(pub T);

impl<T: io::Write> Output for IoWrite<T> {
    #[inline]
    fn reserve(&mut self, _count: usize) {}

    #[inline]
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        self.write_bytes(s.as_bytes())
    }

    #[inline]
    fn write_bytes(&mut self, b: &[u8]) -> Result<(), Error> {
        self.0.write_all(b).map_err(Error::Io)
    }

    #[inline]
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> Result<(), Error> {
        self.0.write_fmt(fmt).map_err(Error::Io)
    }
}
