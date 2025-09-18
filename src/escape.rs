//! Escape sequences.

use crate::Read;
use core::ops::{Add, Shl};

/// Escape literal, such as `\n`.
pub enum Lit {
    /// `\"`
    QuotationMark,
    /// `\\`
    ReverseSolidus,
    /// `\/`
    Solidus,
    /// `\b`
    Backspace,
    /// `\f`
    FormFeed,
    /// `\n`
    LineFeed,
    /// `\t`
    Tab,
    /// `\r`
    CarriageReturn,
}

impl Lit {
    /// Try to interpret an ASCII character as first character of an escape sequence.
    pub fn try_from(c: u8) -> Option<Self> {
        use Lit::*;
        Some(match c {
            b'"' => QuotationMark,
            b'\\' => ReverseSolidus,
            b'/' => Solidus,
            b'b' => Backspace,
            b'f' => FormFeed,
            b'n' => LineFeed,
            b'r' => CarriageReturn,
            b't' => Tab,
            _ => return None,
        })
    }

    fn as_u8(&self) -> u8 {
        use Lit::*;
        match self {
            QuotationMark => 0x22,
            ReverseSolidus => 0x5C,
            Solidus => 0x2F,
            Backspace => 0x08,
            FormFeed => 0x0C,
            LineFeed => 0x0A,
            CarriageReturn => 0x0D,
            Tab => 0x09,
        }
    }
}

/// Parse a hexadecimal digit.
pub(crate) fn decode_hex(val: u8) -> Option<u8> {
    match val {
        b'0'..=b'9' => Some(val - b'0'),
        b'a'..=b'f' => Some(val - b'a' + 10),
        b'A'..=b'F' => Some(val - b'A' + 10),
        _ => None,
    }
}

/// Escape sequence lexing error.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// `\x` or `\U`
    InvalidKind(u8),
    /// `\u000X`
    InvalidHex(u8),
    /// `\uDC37`
    InvalidChar(u32),
    /// `\uD801`
    ExpectedLowSurrogate,
    /// `\` or `\u00`
    Eof,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use Error::*;
        match self {
            InvalidKind(b) => write!(f, "invalid escape character {}", char::from(*b)),
            InvalidHex(b) => write!(f, "invalid hexadecimal character {}", char::from(*b)),
            InvalidChar(c) => write!(f, "invalid character with index {}", c),
            ExpectedLowSurrogate => "expected low surrogate".fmt(f),
            Eof => "unexpected end of file".fmt(f),
        }
    }
}

/// Escape sequence lexing.
///
/// This does not require any allocation.
pub trait Lex: Read {
    /// Given a high surrogate, parse a low surrogate and combine them.
    fn low_surrogate(&mut self, high: u16) -> Result<u32, Error> {
        if !self.strip_prefix(b"\\u") {
            Err(Error::ExpectedLowSurrogate)
        } else if let low @ (0xDC00..=0xDFFF) = self.hex::<u16>()? {
            Ok(((high - 0xD800) as u32 * 0x400 + (low - 0xDC00) as u32) + 0x10000)
        } else {
            Err(Error::ExpectedLowSurrogate)
        }
    }

    /// Convert an escape sequence, such as `\n` or `\u0009` (without leading `\`),
    /// to a char, potentially reading more.
    fn escape(&mut self, c: u8) -> Result<char, Error> {
        if c == b'u' {
            let u = match self.hex()? {
                high @ (0xD800..=0xDBFF) => self.low_surrogate(high)?,
                u => u.into(),
            };
            return char::from_u32(u).ok_or(Error::InvalidChar(u));
        }
        Lit::try_from(c)
            .ok_or(Error::InvalidKind(c))
            .map(|lit| char::from(lit.as_u8()))
    }

    /// Parse a hexadecimal number into an unsigned integer type `T`.
    ///
    /// This can be used to parse hexadecimal numbers into `u8` and `u16`, for example.
    fn hex<T: From<u8> + Shl<Output = T> + Add<Output = T>>(&mut self) -> Result<T, Error> {
        let mut hex: T = 0.into();
        for _ in 0..core::mem::size_of::<T>() * 2 {
            let h = self.take_next().ok_or(Error::Eof)?;
            let h = decode_hex(h).ok_or(Error::InvalidHex(h))?;
            hex = (hex << 4.into()) + h.into();
        }
        Ok(hex)
    }
}

impl<T> Lex for T where T: Read {}
