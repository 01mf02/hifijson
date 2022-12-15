//! High-fidelity JSON lexer and parser.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::num::NonZeroUsize;
use core::ops::Deref;

#[cfg(feature = "alloc")]
mod iterlexer;
#[cfg(feature = "alloc")]
mod parse;
#[cfg(feature = "alloc")]
mod strparser;

pub mod error;
mod escape;
mod slicelexer;

#[cfg(feature = "alloc")]
pub use iterlexer::IterLexer;
#[cfg(feature = "alloc")]
pub use parse::{parse, parse_many, parse_single, Error, Value};
#[cfg(feature = "alloc")]
pub use strparser::LexerStr;

pub use escape::Escape;
pub use slicelexer::SliceLexer;

/// JSON lexer token.
#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    /// `null`
    Null,
    /// `true`
    True,
    /// `false`
    False,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `[`
    LSquare,
    /// `]`
    RSquare,
    /// `{`
    LCurly,
    /// `}`
    RCurly,
    /// `"`
    Quote,
    /// a digit (0-9) or a minus (`-`)
    DigitOrMinus,
    /// anything else
    Error,
}

/// Position of `.` and `e`/`E` in the string representation of a number.
///
/// Because a number cannot start with `.` or `e`/`E`,
/// these positions must always be greater than zero.
#[derive(Debug, Default)]
pub struct NumParts {
    /// position of the dot
    pub dot: Option<NonZeroUsize>,
    /// position of the exponent character (`e`/`E`)
    pub exp: Option<NonZeroUsize>,
}

pub trait Lexer {
    type Bytes: Deref<Target = [u8]> + Default;
    type Num: Deref<Target = str>;

    /// Return `out` if the given byte sequence is read, otherwise an error.
    fn lex_exact<const N: usize>(&mut self, s: [u8; N], out: Token) -> Token;

    /// Return the earliest non-whitespace character.
    fn eat_whitespace(&mut self);

    /// Read (optional) whitespace, followed by a token.
    fn ws_token(&mut self) -> Option<Token> {
        self.eat_whitespace();
        Some(self.token(*self.peek_byte()?))
    }

    fn token(&mut self, c: u8) -> Token {
        let token = match c {
            // it is important to `return` here in order not to read a byte,
            // like we do for the regular, single-character tokens
            b'n' => return self.lex_exact([b'u', b'l', b'l'], Token::Null),
            b't' => return self.lex_exact([b'r', b'u', b'e'], Token::True),
            b'f' => return self.lex_exact([b'a', b'l', b's', b'e'], Token::False),
            b'0'..=b'9' | b'-' => return Token::DigitOrMinus,
            b'"' => Token::Quote,
            b'[' => Token::LSquare,
            b']' => Token::RSquare,
            b'{' => Token::LCurly,
            b'}' => Token::RCurly,
            b',' => Token::Comma,
            b':' => Token::Colon,
            _ => Token::Error,
        };
        self.read_byte();
        token
    }

    fn lex_number(&mut self, bytes: &mut Self::Bytes) -> Result<NumParts, error::Num>;

    /// Read to bytes until `stop` yields true.
    fn read_until(&mut self, bytes: &mut Self::Bytes, stop: impl FnMut(u8) -> bool);

    fn peek_byte(&self) -> Option<&u8>;
    fn read_byte(&mut self) -> Option<u8>;

    /// Read an escape sequence such as "\n" or "\u0009" (without leading '\').
    fn lex_escape(&mut self) -> Result<Escape, error::Escape>;

    fn parse_escape(&mut self, escape: Escape) -> Result<char, error::Escape> {
        let escape = match escape {
            Escape::Unicode(high @ (0xD800..=0xDBFF)) => {
                if self.read_byte() != Some(b'\\') {
                    return Err(error::Escape::ExpectedLowSurrogate);
                }
                if let Escape::Unicode(low @ (0xDC00..=0xDFFF)) = self.lex_escape()? {
                    ((high - 0xD800) * 0x400 + (low - 0xDC00)) as u32 + 0x10000
                } else {
                    return Err(error::Escape::ExpectedLowSurrogate);
                }
            }
            e => e.as_u16() as u32,
        };
        char::from_u32(escape).ok_or(error::Escape::InvalidChar(escape))
    }

    fn lex_string_raw(&mut self, bytes: &mut Self::Bytes) -> Result<(), error::Str> {
        let mut escaped = false;
        let mut unicode = false;
        let mut hex_pos = 0;
        let mut error = None;

        self.read_until(bytes, |c| {
            if escaped {
                if unicode {
                    if escape::decode_hex(c).is_none() {
                        error = Some(error::Str::Escape(error::Escape::InvalidHex));
                    } else if hex_pos < 3 {
                        hex_pos += 1
                    } else {
                        escaped = false;
                        unicode = false;
                        hex_pos = 0;
                    }
                } else {
                    match Escape::try_from(c) {
                        Some(Escape::Unicode(_)) => unicode = true,
                        Some(_) => escaped = false,
                        None => error = Some(error::Str::Escape(error::Escape::UnknownKind)),
                    }
                }
            } else {
                match c {
                    b'"' => return true,
                    b'\\' => escaped = true,
                    0..=19 => error = Some(error::Str::Control),
                    _ => (),
                };
            }
            error.is_some()
        });
        match error {
            Some(e) => Err(e),
            None if escaped || self.read_byte() != Some(b'"') => Err(error::Str::Eof),
            _ => Ok(()),
        }
    }

    fn lex_string<E: From<error::Str>, T>(
        &mut self,
        mut out: T,
        fb: impl Fn(&mut Self::Bytes, &mut T) -> Result<(), E>,
        fe: impl Fn(&mut Self, Escape, &mut T) -> Result<(), E>,
    ) -> Result<T, E> {
        fn string_end(c: u8) -> bool {
            matches!(c, b'"' | b'\\' | 0..=19)
        }

        let mut bytes = Self::Bytes::default();
        self.read_until(&mut bytes, string_end);
        fb(&mut bytes, &mut out)?;
        match self.read_byte().ok_or(error::Str::Eof)? {
            b'\\' => (),
            b'"' => return Ok(out),
            _ => return Err(error::Str::Control)?,
        }
        loop {
            let escape = self.lex_escape().map_err(error::Str::Escape)?;
            fe(self, escape, &mut out)?;
            self.read_until(&mut bytes, string_end);
            fb(&mut bytes, &mut out)?;
            match self.read_byte().ok_or(error::Str::Eof)? {
                b'\\' => continue,
                b'"' => return Ok(out),
                _ => return Err(error::Str::Control)?,
            }
        }
    }

    fn parse_number(&mut self) -> Result<(Self::Num, NumParts), error::Num>;

    fn seq<E: From<error::Seq>, F>(&mut self, until: Token, mut f: F) -> Result<(), E>
    where
        F: FnMut(&mut Self, Token) -> Result<(), E>,
    {
        let mut token = self.ws_token().ok_or(error::Seq::ExpectedItemOrEnd)?;
        if token == until {
            return Ok(());
        };

        loop {
            f(self, token)?;
            token = self.ws_token().ok_or(error::Seq::ExpectedCommaOrEnd)?;
            if token == until {
                return Ok(());
            } else if token == Token::Comma {
                token = self.ws_token().ok_or(error::Seq::ExpectedItem)?;
            } else {
                return Err(error::Seq::ExpectedCommaOrEnd)?;
            }
        }
    }
}
