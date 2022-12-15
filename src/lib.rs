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

mod escape;

#[cfg(feature = "alloc")]
pub use iterlexer::IterLexer;
#[cfg(feature = "alloc")]
pub use parse::{parse, parse_many, parse_single, Error, Value};
#[cfg(feature = "alloc")]
pub use strparser::LexerStr;

pub use escape::Escape;

#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    Null,
    True,
    False,
    Comma,
    Colon,
    LSquare,
    RSquare,
    LCurly,
    RCurly,
    String,
    Number,
    Error,
}

#[derive(Debug, Default)]
pub struct NumParts {
    dot: Option<NonZeroUsize>,
    exp: Option<NonZeroUsize>,
}

#[derive(Debug)]
pub enum SeqError {
    ExpectedItem,
    ExpectedItemOrEnd,
    ExpectedCommaOrEnd,
}

#[derive(Debug)]
pub enum NumError {
    ExpectedDigit,
}

#[derive(Debug)]
pub enum EscapeError {
    Eof,
    UnknownKind,
    InvalidHex,
    InvalidChar(u32),
    ExpectedLowSurrogate,
}

#[derive(Debug)]
pub enum StrError {
    Control,
    Escape(EscapeError),
    Eof,
    Utf8(core::str::Utf8Error),
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
            b'n' => return self.lex_exact([b'u', b'l', b'l'], Token::Null),
            b't' => return self.lex_exact([b'r', b'u', b'e'], Token::True),
            b'f' => return self.lex_exact([b'a', b'l', b's', b'e'], Token::False),
            b'0'..=b'9' | b'-' => return Token::Number,
            b'"' => Token::String,
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

    fn lex_number(&mut self, bytes: &mut Self::Bytes) -> Result<NumParts, NumError>;

    /// Read to bytes until `stop` yields true.
    fn read_until(&mut self, bytes: &mut Self::Bytes, stop: impl FnMut(u8) -> bool);

    fn peek_byte(&self) -> Option<&u8>;
    fn read_byte(&mut self) -> Option<u8>;

    /// Read an escape sequence such as "\n" or "\u0009" (without leading '\').
    fn lex_escape(&mut self) -> Result<Escape, EscapeError>;

    fn parse_escape(&mut self, escape: Escape) -> Result<char, EscapeError> {
        let escape = match escape {
            Escape::Unicode(high @ (0xD800..=0xDBFF)) => {
                if self.read_byte() != Some(b'\\') {
                    return Err(EscapeError::ExpectedLowSurrogate);
                }
                if let Escape::Unicode(low @ (0xDC00..=0xDFFF)) = self.lex_escape()? {
                    ((high - 0xD800) * 0x400 + (low - 0xDC00)) as u32 + 0x10000
                } else {
                    return Err(EscapeError::ExpectedLowSurrogate);
                }
            }
            e => e.as_u16() as u32,
        };
        char::from_u32(escape).ok_or(EscapeError::InvalidChar(escape))
    }

    fn lex_string_raw(&mut self, bytes: &mut Self::Bytes) -> Result<(), StrError> {
        let mut escaped = false;
        let mut unicode = false;
        let mut hex_pos = 0;
        let mut error = None;

        self.read_until(bytes, |c| {
            if escaped {
                if unicode {
                    if escape::decode_hex(c).is_none() {
                        error = Some(StrError::Escape(EscapeError::InvalidHex));
                        return true;
                    }
                    if hex_pos < 3 {
                        hex_pos += 1
                    } else {
                        escaped = false;
                        unicode = false;
                        hex_pos = 0;
                    }
                    return false;
                }
                match Escape::try_from(c) {
                    Some(Escape::Unicode(_)) => unicode = true,
                    Some(_) => escaped = false,
                    None => {
                        error = Some(StrError::Control);
                        return true;
                    }
                }
                return false;
            };
            match c {
                b'"' => return true,
                b'\\' => escaped = true,
                _ => (),
            };
            false
        });
        match error {
            Some(e) => Err(e),
            None if escaped || self.read_byte() != Some(b'"') => Err(StrError::Eof),
            _ => Ok(()),
        }
    }

    fn lex_string<E: From<StrError>, T>(
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
        match self.read_byte().ok_or(StrError::Eof)? {
            b'\\' => (),
            b'"' => return Ok(out),
            _ => return Err(StrError::Control)?,
        }
        loop {
            let escape = self.lex_escape().map_err(StrError::Escape)?;
            fe(self, escape, &mut out)?;
            self.read_until(&mut bytes, string_end);
            fb(&mut bytes, &mut out)?;
            match self.read_byte().ok_or(StrError::Eof)? {
                b'\\' => continue,
                b'"' => return Ok(out),
                _ => return Err(StrError::Control)?,
            }
        }
    }

    fn parse_number(&mut self) -> Result<(Self::Num, NumParts), NumError>;

    fn seq<E: From<SeqError>, F>(&mut self, until: Token, mut f: F) -> Result<(), E>
    where
        F: FnMut(&mut Self, Token) -> Result<(), E>,
    {
        let mut token = self.ws_token().ok_or(SeqError::ExpectedItemOrEnd)?;
        if token == until {
            return Ok(());
        };

        loop {
            f(self, token)?;
            token = self.ws_token().ok_or(SeqError::ExpectedCommaOrEnd)?;
            if token == until {
                return Ok(());
            } else if token == Token::Comma {
                token = self.ws_token().ok_or(SeqError::ExpectedItem)?;
            } else {
                return Err(SeqError::ExpectedCommaOrEnd)?;
            }
        }
    }
}

pub struct SliceLexer<'a> {
    slice: &'a [u8],
}

impl<'a> SliceLexer<'a> {
    pub fn new(slice: &'a [u8]) -> Self {
        Self { slice }
    }
}

fn digits(s: &[u8]) -> usize {
    s.iter()
        .position(|c| !matches!(c, b'0'..=b'9'))
        .unwrap_or(s.len())
}

impl<'a> Lexer for SliceLexer<'a> {
    type Bytes = &'a [u8];
    type Num = &'a str;

    fn lex_exact<const N: usize>(&mut self, s: [u8; N], out: Token) -> Token {
        // we are calling this function without having advanced before
        self.read_byte();
        if let Some(rest) = self.slice.strip_prefix(&s) {
            self.slice = rest;
            out
        } else {
            Token::Error
        }
    }

    fn eat_whitespace(&mut self) {
        let is_space = |c| matches!(c, b' ' | b'\t' | b'\r' | b'\n');
        self.read_until(&mut &[][..], |c| !is_space(c))
    }

    fn lex_number(&mut self, bytes: &mut Self::Bytes) -> Result<NumParts, NumError> {
        let mut pos = usize::from(self.slice[0] == b'-');
        let mut parts = NumParts::default();

        let digits1 = |s| NonZeroUsize::new(digits(s)).ok_or(NumError::ExpectedDigit);

        pos += if self.slice.get(pos) == Some(&b'0') {
            1
        } else {
            digits1(&self.slice[pos..])?.get()
        };

        loop {
            match self.slice.get(pos) {
                Some(b'.') if parts.dot.is_none() && parts.exp.is_none() => {
                    parts.dot = Some(NonZeroUsize::new(pos).unwrap());
                    pos += 1;
                    pos += digits1(&self.slice[pos..])?.get()
                }
                Some(b'e' | b'E') if parts.exp.is_none() => {
                    parts.exp = Some(NonZeroUsize::new(pos).unwrap());
                    pos += 1;
                    if matches!(self.slice.get(pos), Some(b'+' | b'-')) {
                        pos += 1;
                    }
                    pos += digits1(&self.slice[pos..])?.get()
                }
                None | Some(_) => {
                    *bytes = &self.slice[..pos];
                    self.slice = &self.slice[pos..];
                    return Ok(parts);
                }
            }
        }
    }

    fn parse_number(&mut self) -> Result<(Self::Num, NumParts), NumError> {
        let mut num = Default::default();
        let pos = self.lex_number(&mut num)?;
        // SAFETY: conversion to UTF-8 always succeeds because
        // lex_number validates everything it writes to num
        Ok((core::str::from_utf8(num).unwrap(), pos))
    }

    fn peek_byte(&self) -> Option<&u8> {
        self.slice.first()
    }

    fn read_byte(&mut self) -> Option<u8> {
        let (head, rest) = self.slice.split_first()?;
        self.slice = rest;
        Some(*head)
    }

    fn read_until(&mut self, bytes: &mut &'a [u8], mut stop: impl FnMut(u8) -> bool) {
        let pos = self.slice.iter().position(|c| stop(*c));
        let pos = pos.unwrap_or(self.slice.len());
        *bytes = &self.slice[..pos];
        self.slice = &self.slice[pos..]
    }

    fn lex_escape(&mut self) -> Result<Escape, EscapeError> {
        let typ = self.slice.first().ok_or(EscapeError::Eof)?;
        self.slice = &self.slice[1..];
        let escape = Escape::try_from(*typ).ok_or(EscapeError::UnknownKind)?;
        if matches!(escape, Escape::Unicode(_)) {
            let hex = self.slice.get(..4).ok_or(EscapeError::Eof)?;
            // SAFETY: `unwrap()` always succeeds, because `slice.get(..4)`
            // must return a slice of size 4 if it succeeds
            let hex: [u8; 4] = hex.try_into().unwrap();
            self.slice = &self.slice[4..];
            let hex = escape::decode_hex4(hex).ok_or(EscapeError::InvalidHex)?;
            Ok(Escape::Unicode(hex))
        } else {
            Ok(escape)
        }
    }
}
