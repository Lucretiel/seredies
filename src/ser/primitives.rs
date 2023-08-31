/*!
Basic implementations of serialize primitives for RESP
*/

use super::{Error, Output};

/// Helper trait for writing things to `Output`, using the best available
/// method. Abstracts over `str` and `[u8]`.
pub trait Writable {
    fn write_to_output(&self, output: impl Output) -> Result<(), Error>;

    #[must_use]
    fn len(&self) -> usize;
}

impl Writable for [u8] {
    #[inline]
    fn write_to_output(&self, mut output: impl Output) -> Result<(), Error> {
        output.write_bytes(self)
    }

    #[inline]
    #[must_use]
    fn len(&self) -> usize {
        self.len()
    }
}

impl Writable for str {
    #[inline]
    fn write_to_output(&self, mut output: impl Output) -> Result<(), Error> {
        output.write_str(self)
    }

    #[inline]
    #[must_use]
    fn len(&self) -> usize {
        self.len()
    }
}

/**
Return an estimate of how wide a number's representation is (i.e., how
many characters it will take to format the number)
*/
#[must_use]
pub const fn estimate_number_reservation(value: i64) -> usize {
    match value.saturating_abs().checked_ilog10() {
        None => 1,
        Some(len) if value.is_negative() => len as usize + 2,
        Some(len) => len as usize + 1,
    }
}

/**
Write a redis header containing a numeric `value` to the `output`, using the
`prefix`. This method will reserve space in the `output` sufficient to contain
the header, plus additional space equal to `suffix_reserve`.
*/
fn serialize_header(
    mut output: impl Output,
    prefix: u8,
    value: impl TryInto<i64>,
    suffix_reserve: usize,
) -> Result<(), Error> {
    let prefix = prefix as char;
    debug_assert!("*:$".contains(prefix));

    let value: i64 = value.try_into().map_err(|_| Error::NumberOutOfRange)?;

    let width = (estimate_number_reservation(value) as usize)
        .saturating_add(3) // the width of the prefix byte and the CRLF
        .saturating_add(suffix_reserve);

    output.reserve(width);
    write!(output, "{prefix}{value}\r\n")
}

/**
Serialize a plain Redis number
*/
#[inline]
pub fn serialize_number(output: impl Output, value: impl TryInto<i64>) -> Result<(), Error> {
    serialize_header(output, b':', value, 0)
}

/**
Given an array of length `len`, estimate how many bytes are reasonable
to reserve in an output buffer that will contain that array. This should
*mostly* be the lower bound but can make certain practical estimates about
the data that is *likely* to be contained.
*/
#[inline]
#[must_use]
pub const fn estimate_array_reservation(len: usize) -> usize {
    // By far the most common thing we serialize is a bulk string (for a
    // command), and the smallest bulk string (an empty one) is 6 bytes, so
    // that's the factor we use.
    len.saturating_mul(6)
}

/**
Serialize the header for an array of `len` elements. RESP will expect `len`
elements to be serialized after this header.
*/
#[inline]
pub fn serialize_array_header(output: impl Output, len: usize) -> Result<(), Error> {
    serialize_header(output, b'*', len, estimate_array_reservation(len))
}

/**
Serialize something writable as a Bulk String
*/
pub fn serialize_bulk_string(
    mut output: impl Output,
    value: &(impl Writable + ?Sized),
) -> Result<(), Error> {
    let len = value.len();

    serialize_header(&mut output, b'$', len, len.saturating_add(2))?;
    value.write_to_output(&mut output)?;
    output.write_str("\r\n")
}

/**
When writing a simple string or error string, the payload must not include
`'\r'` or `'\n'` characters. This `Output` adapter rejects any writes that
include these bytes.
*/
struct NewlineRejector<O: Output>(O);

impl<O: Output> Output for NewlineRejector<O> {
    #[inline]
    fn reserve(&mut self, count: usize) {
        self.0.reserve(count)
    }

    #[inline]
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        match has_newline(s.as_bytes()) {
            false => self.0.write_str(s),
            true => Err(Error::BadSimpleString),
        }
    }

    #[inline]
    fn write_bytes(&mut self, b: &[u8]) -> Result<(), Error> {
        match has_newline(b) {
            false => self.0.write_bytes(b),
            true => Err(Error::BadSimpleString),
        }
    }
}

#[inline]
#[must_use]
fn has_newline(data: &[u8]) -> bool {
    memchr::memchr2(b'\n', b'\r', data).is_some()
}

/**
Serialize a RESP error
*/
pub fn serialize_error(
    mut dest: impl Output,
    value: &(impl Writable + ?Sized),
) -> Result<(), Error> {
    dest.reserve(value.len().saturating_add(3));
    dest.write_str("-")?;
    value.write_to_output(NewlineRejector(&mut dest))?;
    dest.write_str("\r\n")
}
