//! Escape sequences.

use crate::Read;
use core::fmt;
use core::ops::{Add, Shl};

/// Escape sequence, such as `\n` or `\u00d6`.
pub enum Escape {
    /// literal
    Lit(Lit),
    /// `\uHHHH` or `\hHH`, where `HH`/`HHHH` are hexadecimal numbers
    Hex(Hex<u16>),
}

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

/// Escape sequence containing a hexadecimal number, such as `\u00d6`.
#[derive(Copy, Clone)]
pub enum Hex<U, B = u8> {
    /// `\uHHHH`
    Unicode(U),
    /// `\xHH` --- this is not part of the JSON standard
    Byte(B),
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

    fn as_char(&self) -> char {
        use Lit::*;
        match self {
            QuotationMark => '"',
            ReverseSolidus => '\\',
            Solidus => '/',
            Backspace => 'b',
            FormFeed => 'f',
            LineFeed => 'n',
            CarriageReturn => 'r',
            Tab => 't',
        }
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

impl<U, B> Hex<U, B> {
    /// Return Unicode value if possible, else byte value.
    pub fn as_unicode(self) -> Result<U, B> {
        match self {
            Self::Unicode(u) => Ok(u),
            Self::Byte(b) => Err(b),
        }
    }
}

impl Escape {
    /// Try to interpret an ASCII character as first character of an escape sequence.
    pub fn try_from(c: u8) -> Option<Escape> {
        Lit::try_from(c).map(Escape::Lit).or_else(|| {
            Some(Escape::Hex(match c {
                b'x' => Hex::Byte(0),
                b'u' => Hex::Unicode(0),
                _ => return None,
            }))
        })
    }
}

impl fmt::Display for Escape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Lit(l) => write!(f, "\\{}", l.as_char()),
            Self::Hex(Hex::Unicode(u)) => write!(f, "\\u{u:04x}"),
            Self::Hex(Hex::Byte(b)) => write!(f, "\\x{b:02x}"),
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

fn hexn<T: From<u8> + Shl<Output = T> + Add<Output = T>>(
    lex: &mut (impl Lex + ?Sized),
) -> Result<T, Error> {
    let mut hex: T = 0.into();
    for _ in 0..core::mem::size_of::<T>() * 2 {
        let h = lex.take_next().ok_or(Error::Eof)?;
        let h = decode_hex(h).ok_or(Error::InvalidHex)?;
        hex = (hex << 4.into()) + h.into();
    }
    Ok(hex)
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
    fn escape_char(&mut self, escape: Escape) -> Result<Hex<char>, Error> {
        let escape = match escape {
            Escape::Hex(Hex::Unicode(high @ (0xD800..=0xDBFF))) => {
                if self.take_next() != Some(b'\\') {
                    return Err(Error::ExpectedLowSurrogate);
                }
                if let Escape::Hex(Hex::Unicode(low @ (0xDC00..=0xDFFF))) = self.escape()? {
                    ((high - 0xD800) as u32 * 0x400 + (low - 0xDC00) as u32) + 0x10000
                } else {
                    return Err(Error::ExpectedLowSurrogate);
                }
            }
            Escape::Hex(Hex::Byte(b)) => return Ok(Hex::Byte(b)),
            Escape::Lit(l) => l.as_u8().into(),
            Escape::Hex(Hex::Unicode(u)) => u.into(),
        };
        char::from_u32(escape)
            .map(Hex::Unicode)
            .ok_or(Error::InvalidChar(escape))
    }

    /// Read an escape sequence such as `\n` or `\u0009` (without leading `\`).
    fn escape(&mut self) -> Result<Escape, Error> {
        let typ = self.take_next().ok_or(Error::Eof)?;
        let escape = Escape::try_from(typ).ok_or(Error::UnknownKind)?;
        match escape {
            Escape::Hex(Hex::Unicode(_)) => hexn(self).map(|x| Escape::Hex(Hex::Unicode(x))),
            Escape::Hex(Hex::Byte(_)) => hexn(self).map(|x| Escape::Hex(Hex::Byte(x))),
            _ => Ok(escape),
        }
    }
}

impl<T> Lex for T where T: Read {}
