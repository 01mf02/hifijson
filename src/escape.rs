//! Escape sequences.

use crate::Read;
use core::fmt;

/// Escape sequence, such as `\n` or `\u00d6`.
pub enum Escape {
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
    /// `\uHHHH`, where `HHHH` is a hexadecimal number
    Unicode(u16),
}

impl Escape {
    /// Try to interpret an ASCII character as first character of an escape sequence.
    pub fn try_from(c: u8) -> Option<Escape> {
        use Escape::*;
        Some(match c {
            b'"' => QuotationMark,
            b'\\' => ReverseSolidus,
            b'/' => Solidus,
            b'b' => Backspace,
            b'f' => FormFeed,
            b'n' => LineFeed,
            b'r' => CarriageReturn,
            b't' => Tab,
            b'u' => Unicode(0),
            _ => return None,
        })
    }

    fn as_char(&self) -> Result<char, u16> {
        use Escape::*;
        Ok(match self {
            QuotationMark => '"',
            ReverseSolidus => '\\',
            Solidus => '/',
            Backspace => 'b',
            FormFeed => 'f',
            LineFeed => 'n',
            CarriageReturn => 'r',
            Tab => 't',
            Unicode(u) => return Err(*u),
        })
    }

    /// Return escape sequence as UTF-16.
    pub fn as_u16(&self) -> u16 {
        use Escape::*;
        match self {
            QuotationMark => 0x0022,
            ReverseSolidus => 0x005C,
            Solidus => 0x002F,
            Backspace => 0x0008,
            FormFeed => 0x000C,
            LineFeed => 0x000A,
            CarriageReturn => 0x000D,
            Tab => 0x0009,
            Unicode(u) => *u,
        }
    }
}

impl fmt::Display for Escape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.as_char() {
            Ok(c) => write!(f, "\\{}", c),
            Err(u) => write!(f, "\\u{:04x}", u),
        }
    }
}

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
    UnknownKind,
    /// `\u000X`
    InvalidHex,
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
            UnknownKind => "unknown escape sequence type".fmt(f),
            InvalidHex => "invalid hexadecimal sequence".fmt(f),
            InvalidChar(c) => write!(f, "invalid character with index {}", c),
            ExpectedLowSurrogate => "expected low surrogate".fmt(f),
            Eof => "unterminated escape sequence".fmt(f),
        }
    }
}

/// Escape sequence lexing.
///
/// This does not require any allocation.
pub trait Lex: Read {
    /// Convert a read escape sequence to a char, potentially reading more.
    fn escape_char(&mut self, escape: Escape) -> Result<char, Error> {
        let escape = match escape {
            Escape::Unicode(high @ (0xD800..=0xDBFF)) => {
                if self.read() != Some(b'\\') {
                    return Err(Error::ExpectedLowSurrogate);
                }
                if let Escape::Unicode(low @ (0xDC00..=0xDFFF)) = self.escape()? {
                    ((high - 0xD800) as u32 * 0x400 + (low - 0xDC00) as u32) + 0x10000
                } else {
                    return Err(Error::ExpectedLowSurrogate);
                }
            }
            e => e.as_u16() as u32,
        };
        char::from_u32(escape).ok_or(Error::InvalidChar(escape))
    }

    /// Read an escape sequence such as `\n` or `\u0009` (without leading `\`).
    fn escape(&mut self) -> Result<Escape, Error> {
        let typ = self.read().ok_or(Error::Eof)?;
        let escape = Escape::try_from(typ).ok_or(Error::UnknownKind)?;
        if matches!(escape, Escape::Unicode(_)) {
            let mut hex = 0;
            for _ in 0..4 {
                let h = self.read().ok_or(Error::Eof)?;
                let h = decode_hex(h).ok_or(Error::InvalidHex)?;
                hex = (hex << 4) + (h as u16);
            }
            Ok(Escape::Unicode(hex))
        } else {
            Ok(escape)
        }
    }
}

impl<T> Lex for T where T: Read {}
