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
            Ok(c) => write!(f, "\\{c}"),
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

pub(crate) fn decode_hex4(val: [u8; 4]) -> Option<u16> {
    val.iter()
        .try_fold(0, |acc, x| Some((acc << 4) + decode_hex(*x)? as u16))
}
