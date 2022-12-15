use crate::escape::{decode_hex4, Escape};
use crate::{error, Lexer, NumParts};
use core::num::NonZeroUsize;

/// JSON lexer from an iterator over (fallible) bytes.
pub struct IterLexer<E, I> {
    bytes: I,
    last: Option<u8>,
    error: Option<E>,
}

impl<E, I: Iterator<Item = Result<u8, E>>> IterLexer<E, I> {
    pub fn new(iter: I) -> Self {
        Self {
            bytes: iter,
            last: None,
            error: None,
        }
    }

    fn read(&mut self) -> Option<u8> {
        match self.bytes.next()? {
            Ok(b) => Some(b),
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }

    fn digits(&mut self, num: &mut <Self as Lexer>::Bytes) -> Result<(), error::Num> {
        let mut some_digit = false;
        while let Some(digit @ (b'0'..=b'9')) = self.last {
            some_digit = true;
            num.push(digit);
            self.last = self.read();
        }
        if some_digit && self.error.is_none() {
            Ok(())
        } else {
            Err(error::Num::ExpectedDigit)
        }
    }
}

impl<E, I: Iterator<Item = Result<u8, E>>> Lexer for IterLexer<E, I> {
    type Bytes = alloc::vec::Vec<u8>;
    type Num = alloc::string::String;

    fn lex_exact<const N: usize, T: Default>(&mut self, s: [u8; N], out: T) -> T {
        self.read_byte();
        for c1 in s {
            match self.read() {
                Some(c2) if c1 == c2 => continue,
                Some(_) | None => return T::default(),
            }
        }
        out
    }

    fn eat_whitespace(&mut self) {
        let is_space = |c| matches!(c, b' ' | b'\t' | b'\r' | b'\n');

        match self.last {
            Some(last) if !is_space(last) => return,
            _ => self.last = None,
        }

        for c in self.bytes.by_ref() {
            match c {
                Ok(c) if is_space(c) => continue,
                Ok(c) => self.last = Some(c),
                Err(e) => {
                    self.last = Some(b' ');
                    self.error = Some(e);
                }
            }
            return;
        }
    }

    fn num_bytes(&mut self, num: &mut Self::Bytes) -> Result<NumParts, error::Num> {
        let mut parts = NumParts::default();

        if self.last == Some(b'-') {
            num.push(b'-');
            self.last = self.read();
        }

        if self.last == Some(b'0') {
            num.push(b'0');
            self.last = self.read();
        } else {
            self.digits(num)?;
        }

        loop {
            match self.last {
                Some(b'.') if parts.dot.is_none() && parts.exp.is_none() => {
                    parts.dot = Some(NonZeroUsize::new(num.len()).unwrap());
                    num.push(b'.');
                    self.last = self.read();

                    self.digits(num)?;
                }

                Some(e @ (b'e' | b'E')) if parts.exp.is_none() => {
                    parts.exp = Some(NonZeroUsize::new(num.len()).unwrap());
                    num.push(e);
                    self.last = self.read();

                    if let Some(sign @ (b'+' | b'-')) = self.last {
                        num.push(sign);
                        self.last = self.read();
                    }

                    self.digits(num)?;
                }
                _ => return Ok(parts),
            }
        }
    }

    fn num_string(&mut self) -> Result<(Self::Num, NumParts), error::Num> {
        let mut num = Default::default();
        let pos = self.num_bytes(&mut num)?;
        // SAFETY: conversion to UTF-8 always succeeds because
        // lex_number validates everything it writes to num
        Ok((alloc::string::String::from_utf8(num).unwrap(), pos))
    }

    fn read_until(&mut self, bytes: &mut Self::Bytes, mut stop: impl FnMut(u8) -> bool) {
        while let Some(c) = self.read() {
            if stop(c) {
                self.last = Some(c);
                return;
            } else {
                bytes.push(c)
            }
        }
    }

    fn read_byte(&mut self) -> Option<u8> {
        self.last.take()
    }

    fn peek_byte(&self) -> Option<&u8> {
        self.last.as_ref()
    }

    fn escape(&mut self) -> Result<Escape, error::Escape> {
        let typ = self.read().ok_or(error::Escape::Eof)?;
        let escape = Escape::try_from(typ).ok_or(error::Escape::UnknownKind)?;
        if matches!(escape, Escape::Unicode(_)) {
            let mut hex = [0; 4];
            for h in &mut hex {
                *h = self.read().ok_or(error::Escape::Eof)?;
            }
            let hex = decode_hex4(hex).ok_or(error::Escape::InvalidHex)?;
            Ok(Escape::Unicode(hex))
        } else {
            Ok(escape)
        }
    }
}
