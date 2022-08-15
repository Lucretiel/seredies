use std::str;

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
pub enum Tag {
    SimpleString,
    Error,
    Integer,
    BulkString,
    Array,
}

pub type ParseResult<'a, O> = Result<(O, &'a [u8]), Error>;

/// Read a single \r\n from the input
pub fn read_endline(input: &[u8]) -> ParseResult<'_, ()> {
    match input {
        [b'\r', b'\n', input @ ..] => Ok(((), input)),
        [b'\r'] => Err(Error::UnexpectedEof(1)),
        [] => Err(Error::UnexpectedEof(2)),
        _ => Err(Error::MalformedNewline),
    }
}

/// Read a tag and its payload, followed by an endline
pub fn read_tag(input: &[u8]) -> ParseResult<(Tag, &[u8])> {
    let (&tag, input) = input.split_first().ok_or(Error::UnexpectedEof(3))?;

    let tag = match tag {
        b'+' => Tag::SimpleString,
        b'-' => Tag::Error,
        b':' => Tag::Integer,
        b'$' => Tag::BulkString,
        b'*' => Tag::Array,
        tag => return Err(Error::BadTag(tag)),
    };

    let idx = memchr2(b'\r', b'\n', input).ok_or(Error::UnexpectedEof(2))?;
    let (payload, input) = input.split_at(idx);
    let ((), input) = read_endline(input)?;

    Ok(((tag, payload), input))
}

fn try_split_at(input: &[u8], idx: usize) -> Option<(&[u8], &[u8])> {
    input.get(..idx).map(|head| (head, &input[idx..]))
}

/// Read a chunk of a specific length, followed by endline
pub fn read_exact(length: usize, input: &[u8]) -> ParseResult<'_, &[u8]> {
    let (payload, input) = try_split_at(input, length)
        .ok_or_else(|| Error::UnexpectedEof(length + 2 - input.len()))?;

    let ((), input) = read_endline(input)?;

    Ok((payload, input))
}

pub fn parse_number(payload: &[u8]) -> Result<i64, Error> {
    str::from_utf8(payload)
        .map_err(|_| Error::Number)?
        .parse()
        .map_err(|_| Error::Number)
}
