use memchr::memchr2;
use thiserror::Error;

#[derive(Debug, Clone, Copy, Error)]
pub enum Error {
    #[error("unexpected end of input; read at least {0} more bytes and try again")]
    UnexpectedEof(usize),

    #[error("malformed newline during parsing (all redis newlines are \\r\\n")]
    MalformedNewline,

    #[error("unrecognized tag byte {0:#x}")]
    BadTag(u8),

    #[error("failed to parse a decimal integer")]
    Number,
}

#[derive(Debug, Clone, Copy)]
pub enum TaggedHeader<'a> {
    SimpleString(&'a [u8]),
    Error(&'a [u8]),
    Integer(i64),
    BulkString(i64),
    Array(i64),
    Null,
}

pub type ParseResult<'a, O> = Result<(O, &'a [u8]), Error>;

/// Read a single \r\n from the input.
pub fn read_endline(input: &[u8]) -> ParseResult<'_, ()> {
    match input {
        [b'\r', b'\n', input @ ..] => Ok(((), input)),
        [b'\r'] => Err(Error::UnexpectedEof(1)),
        [] => Err(Error::UnexpectedEof(2)),
        _ => Err(Error::MalformedNewline),
    }
}

/// Read a tag and its payload, followed by an endline
pub fn read_header(input: &[u8]) -> ParseResult<TaggedHeader<'_>> {
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
        b'*' => parse_number(payload).map(TaggedHeader::Array),
        tag => Err(Error::BadTag(tag)),
    }
}

#[inline]
#[must_use]
fn try_split_at(input: &[u8], idx: usize) -> Option<(&[u8], &[u8])> {
    input.get(..idx).map(|head| (head, &input[idx..]))
}

/// Read a chunk of a specific length, followed by endline
pub fn read_exact(length: usize, input: &[u8]) -> ParseResult<'_, &[u8]> {
    let (payload, input) = try_split_at(input, length)
        .ok_or_else(|| Error::UnexpectedEof((length - input.len()).saturating_add(2)))?;

    let ((), input) = read_endline(input)?;

    Ok((payload, input))
}

#[inline]
#[must_use]
pub const fn ascii_to_digit(b: u8) -> Option<i64> {
    match b {
        b'0'..=b'9' => Some((b - b'0') as i64),
        _ => None,
    }
}

#[must_use]
fn parse_number(payload: &[u8]) -> Result<i64, Error> {
    let (payload, positive) = match payload.split_first().ok_or(Error::Number)? {
        (&b'-', tail) => (tail, false),
        (&b'+', tail) => (tail, true),
        _ => (payload, true),
    };

    payload
        .iter()
        .copied()
        .try_fold(0, move |accum, b| {
            let digit = ascii_to_digit(b)?;
            let digit = if positive { digit } else { -digit };
            let accum = accum.checked_mul(10)?;
            accum.checked_add(digit)
        })
        .ok_or(Error::Number)
}
