//! Low level parser implementations for RESP.
//!
//! The parsers and data types are available here for authors who want to write
//! their own Redis abstractions, but they're designed to be very low-level.
//! Usually you'll prefer to use the seredies
//! [`Deserializer`][crate::de::Deserializer].
//!
//! The parsers here are modeled after [nom](https://docs.rs/nom).

use memchr::memchr2;
use thiserror::Error;

/// Parse errors that can occur while attempting to deserialize RESP data.
///
/// Of especial note to protocol library authors is the
/// [`UnexpectedEof`][Error::UnexpectedEof] variant; it includes a minimum
/// amount of additional bytes that must be read, after which the parse can
/// be retried.
#[derive(Debug, Clone, Copy, Error)]
#[non_exhaustive]
pub enum Error {
    /// The data wasn't malformed, but it ended before the parse could complete.
    /// The value in the error is the minimum number of additional bytes
    /// required for a successful parse.
    #[error("unexpected end of input; read at least {0} more bytes and try again")]
    UnexpectedEof(usize),

    /// A newline was expected and was malformed somehow. It might have been
    /// entirely missing, or was only a `\n`.
    #[error("malformed newline during parsing (all redis newlines are \\r\\n")]
    MalformedNewline,

    /// A header tag byte wasn't one of the recognized RESP tag bytes.
    #[error("unrecognized tag byte {0:#x}")]
    BadTag(u8),

    /// A decimal number failed to parse.
    #[error("failed to parse a decimal integer")]
    Number,
}

/// A parsed RESP "header".
///
/// In RESP, all data includes a header, which consists of some tag byte,
/// a header payload, followed by `\r\n`. This type contains the result of
/// parsing this header. For `BulkString` and `Array` objects, the header
/// indicates how much additional data will be read; for the other types,
/// the header is the entirety of the data.
///
/// See the [protocol specification](https://redis.io/docs/reference/protocol-spec/)
/// for details.
#[derive(Debug, Clone, Copy)]
pub enum TaggedHeader<'a> {
    /// A RESP [Simple String](https://redis.io/docs/reference/protocol-spec/#resp-simple-strings).
    /// These are often used to communicate trivial response information.
    SimpleString(&'a [u8]),

    /// A RESP [Error](https://redis.io/docs/reference/protocol-spec/#resp-errors).
    /// This is used in responses to indicate that something went wrong. The
    /// seredies `Deserializer` can automatically treat these either as
    /// deserialize errors or deserialize them into [`Result`] objects.
    Error(&'a [u8]),

    /// A RESP [Integer](https://redis.io/docs/reference/protocol-spec/#resp-integers).
    /// These are used as numeric response information. Note that redis commands
    /// are *always* a list of strings; integers only appear in responses.
    Integer(i64),

    /// A RESP [Bulk String](https://redis.io/docs/reference/protocol-spec/#resp-bulk-strings)
    /// header. Most data in RESP is sent as Bulk Strings, as they're binary-safe.
    /// The value in the header is the number of bytes in the bulk string.
    BulkString(i64),

    /// A RESP [Array](https://redis.io/docs/reference/protocol-spec/#resp-arrays).
    /// Redis commands are sent as RESP arrays, and many commands can return
    /// collections of data as arrays. The value in the header is the number
    /// of items in the array, and arrays can contain arbitrary RESP objects
    /// as items.
    Array(i64),

    /// Null is a special case of a Bulk String, and is used to indicate the
    /// absence of a value (such as a `GET` for a key that doesn't exist)
    Null,
}

/// The result of a parse, which can either be a parse error, or a successful
/// parse that includes the parsed value and the unparsed tail of the input.
pub type ParseResult<'a, O> = Result<(O, &'a [u8]), Error>;

/// Read a single \r\n from the input.
fn read_endline(input: &[u8]) -> ParseResult<'_, ()> {
    match input {
        [b'\r', b'\n', input @ ..] => Ok(((), input)),
        [b'\r'] => Err(Error::UnexpectedEof(1)),
        [] => Err(Error::UnexpectedEof(2)),
        _ => Err(Error::MalformedNewline),
    }
}

/**
Read a tag and its payload, followed by an `\r\n`.

# Example

```
use seredies::de::parse::{read_header, TaggedHeader};
use cool_asserts::assert_matches;

assert_matches!(
    read_header(b"+OK\r\nabc"),
    Ok((TaggedHeader::SimpleString(b"OK"), b"abc"))
);
```
*/
pub fn read_header(input: &[u8]) -> ParseResult<'_, TaggedHeader<'_>> {
    // Fast path for these common cases
    match try_split_at(input, 5) {
        Some((b"+OK\r\n", tail)) => return Ok((TaggedHeader::SimpleString(b"OK"), tail)),
        Some((b"$-1\r\n", tail)) => return Ok((TaggedHeader::Null, tail)),
        _ => {}
    };

    let (&tag, input) = input.split_first().ok_or(Error::UnexpectedEof(3))?;
    let (payload, input) = {
        let idx = memchr2(b'\r', b'\n', input).ok_or(Error::UnexpectedEof(2))?;
        input.split_at(idx)
    };
    let ((), input) = read_endline(input)?;

    match tag {
        b'+' => Ok(TaggedHeader::SimpleString(payload)),
        b'-' => Ok(TaggedHeader::Error(payload)),
        b':' => parse_number(payload).map(TaggedHeader::Integer),
        b'$' => parse_number(payload).map(|len| match len {
            -1 => TaggedHeader::Null,
            len => TaggedHeader::BulkString(len),
        }),
        b'*' => parse_number(payload).map(|len| match len {
            -1 => TaggedHeader::Null,
            len => TaggedHeader::Array(len),
        }),
        tag => Err(Error::BadTag(tag)),
    }
    .map(|header| (header, input))
}

#[inline]
#[must_use]
fn try_split_at(input: &[u8], idx: usize) -> Option<(&[u8], &[u8])> {
    let head = input.get(..idx)?;
    let tail = input.get(idx..)?;

    Some((head, tail))
}

/**
Read precisely `length` bytes, followed by `\r\n`.

# Example

```
use seredies::de::parse::{read_exact, TaggedHeader};
use cool_asserts::assert_matches;

assert_matches!(
    read_exact(4, b"ABCD\r\n123"),
    Ok((b"ABCD", b"123"))
);
```
*/
pub fn read_exact(length: usize, input: &[u8]) -> ParseResult<'_, &[u8]> {
    let (payload, input) = try_split_at(input, length)
        .ok_or_else(|| Error::UnexpectedEof((length - input.len()).saturating_add(2)))?;

    let ((), input) = read_endline(input)?;

    Ok((payload, input))
}

#[inline]
#[must_use]
const fn ascii_to_digit(b: u8) -> Option<i64> {
    match b {
        b'0'..=b'9' => Some((b - b'0') as i64),
        _ => None,
    }
}

fn parse_number(payload: &[u8]) -> Result<i64, Error> {
    let (payload, positive) = match payload.split_first().ok_or(Error::Number)? {
        (&b'-', tail) => (tail, false),
        (&b'+', tail) => (tail, true),
        _ => (payload, true),
    };

    payload
        .iter()
        .copied()
        .try_fold(0i64, move |accum, b| {
            let digit = ascii_to_digit(b)?;
            let digit = if positive { digit } else { -digit };
            let accum = accum.checked_mul(10)?;
            accum.checked_add(digit)
        })
        .ok_or(Error::Number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cool_asserts::assert_matches;

    macro_rules! test_cases {
        ($($name:ident: $input:expr, $expected:pat,)*) => {$(
            #[test]
            fn $name() {
                assert_matches!($input, $expected)
            }
        )*};
    }

    mod read_endline {
        use super::*;

        macro_rules! endline_test_cases {
            ($($name:ident: $input:literal == $expected:pat,)*) => {
                test_cases!{$(
                    $name: read_endline($input), $expected,
                )*}
            };
        }

        endline_test_cases! {
            basic: b"\r\nabc" == Ok(((), b"abc")),
            missing: b"" == Err(Error::UnexpectedEof(2)),
            partial_missing: b"\r" == Err(Error::UnexpectedEof(1)),
            malformed1: b"\n\r" == Err(Error::MalformedNewline),
            malformed2: b"\r\r" == Err(Error::MalformedNewline),
            malformed3: b"abc" == Err(Error::MalformedNewline),
        }
    }

    mod read_header {
        use super::*;

        macro_rules! header_test_cases {
            ($($name:ident: $input:literal == $expected:pat,)*) => {
                test_cases!{$(
                    $name: read_header($input), $expected,
                )*}
            };
        }

        header_test_cases! {
            simple_string: b"+OK\r\nabc" == Ok((TaggedHeader::SimpleString(b"OK"), b"abc")),
            error: b"-CODE message\r\nabc" == Ok((TaggedHeader::Error(b"CODE message"), b"abc")),
            number: b":123\r\nabc" == Ok((TaggedHeader::Integer(123), b"abc")),
            negative: b":-123\r\nabc" == Ok((TaggedHeader::Integer(-123), b"abc")),
            bulk_string: b"$3\r\nabc\r\n" == Ok((TaggedHeader::BulkString(3), b"abc\r\n")),
            null: b"$-1\r\nabc\r\n" == Ok((TaggedHeader::Null, b"abc\r\n")),
            array: b"*1\r\n+OK\r\n" == Ok((TaggedHeader::Array(1), b"+OK\r\n")),
            null_array: b"*-1\r\nabc\r\n" == Ok((TaggedHeader::Null, b"abc\r\n")),
            bad_tag: b"xABC\r\n" == Err(Error::BadTag(b'x')),
            incomplete: b"+OK\r" == Err(Error::UnexpectedEof(1)),
        }
    }

    mod read_exact {
        use super::*;

        macro_rules! exact_test_cases {
            ($($name:ident: $amount:literal @ $input:literal == $expected:pat,)*) => {
                test_cases!{$(
                    $name: read_exact($amount, $input), $expected,
                )*}
            };
        }

        exact_test_cases! {
            basic: 4 @ b"abcd\r\n123" == Ok((b"abcd", b"123")),
            incomplete: 10 @ b"abc" == Err(Error::UnexpectedEof(9)),
            incomplete_newline: 4 @ b"abcd" == Err(Error::UnexpectedEof(2)),
            malformed: 4 @ b"abcdef\r\n" == Err(Error::MalformedNewline),
        }
    }
}
