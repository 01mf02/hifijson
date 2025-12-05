//! Positive numbers.
//!
//! Conforming to the JSON specification, the lexers in this modules, in particular
//! [`Lex::num_ignore`] and
//! [`LexWrite::num_string`],
//! accept numbers corresponding to the regex
//! `(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?`.
//!
//! This leads numbers like `007` to be lexed as three separate numbers;
//! `0`, `0`, and `7`.
//! That is because after a leading `0`, the lexer expects only ".", "e" or "E",
//! so when it sees another digit (such as "0" or "7"),
//! it assumes that it is not part of the number.
//!
//! To prevent such behaviour, verify that numbers are not followed by a digit,
//! e.g. with [`crate::Read::peek_next`].
use crate::{Read, Write};
use core::fmt::{self, Display};
/// Number lexing error.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The only thing that can go wrong during number lexing is
    /// that we are not reading a digit where we expected one.
    /// For example:
    ///
    /// - `""`
    /// - `"0."`
    /// - `"0.1e"`
    ExpectedDigit,
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ExpectedDigit => "expected digit".fmt(f),
        }
    }
}

/// Positions of various parts in the string representation of a number.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parts {
    /// position of leading zero (`0`)
    pub zero: Option<usize>,
    /// position of the dot (`.`)
    pub dot: Option<usize>,
    /// position of the exponent character (`e`/`E`)
    pub exp: Option<usize>,
}

impl Parts {
    /// Return true if the number contains neither a dot not an exponent.
    pub fn is_int(&self) -> bool {
        self.dot.is_none() && self.exp.is_none()
    }
}

/// Number lexing, ignoring the number.
pub trait Lex: Read {
    /// Lex a number and ignore its contents, saving only its parts.
    fn num_ignore(&mut self) -> Result<Parts, Error> {
        let mut parts = Parts::default();
        let mut len = 0;
        let mut prev = None;
        self.skip_until(|c| {
            !parts.num_part(len, prev, c) || {
                len += 1;
                prev = Some(c);
                false
            }
        });
        let valid = prev.as_ref().map_or(false, u8::is_ascii_digit);
        valid.then(|| parts).ok_or(Error::ExpectedDigit)
    }
}

impl<T> Lex for T where T: Read {}

impl Parts {
    /// Returns whether the next character `c` is part of the current number.
    fn num_part(&mut self, len: usize, prev: Option<u8>, c: u8) -> bool {
        let Self { zero, exp, dot } = self;
        match (prev, c) {
            (None, b'0') => *zero = Some(len),
            (_, b'0'..=b'9') => return zero.is_none() || dot.or(*exp).is_some(),
            (Some(b'0'..=b'9'), b'.') if dot.or(*exp).is_none() => *dot = Some(len),
            (Some(b'0'..=b'9'), b'e' | b'E') if exp.is_none() => *exp = Some(len),
            (Some(b'e' | b'E'), b'+' | b'-') => return true,
            _ => return false,
        };
        true
    }
}

/// Number lexing, keeping the number.
pub trait LexWrite: Lex + Write {
    /// String type to save numbers as.
    type Num: core::ops::Deref<Target = str>;

    /// Lex a number, append it to `num`, and save its parts.
    ///
    /// `num` must be a suffix of the previously consumed input.
    /// Normally, you pass `b"-"` as prefix if you read "-" just before.
    /// This allows you to include "-" in the bytes without allocation.
    fn num_bytes(&mut self, num: &mut Self::Bytes) -> Result<Parts, Error> {
        let mut parts = Parts::default();
        let mut prev = None;
        self.append_until(num, |b, c| {
            !parts.num_part(b.len(), prev, c) || {
                prev = Some(c);
                false
            }
        });
        let valid = prev.as_ref().map_or(false, u8::is_ascii_digit);
        valid.then(|| parts).ok_or(Error::ExpectedDigit)
    }

    /// Write a prefix and a number to a string and save its parts.
    fn num_string(&mut self, prefix: &str) -> Result<(Self::Num, Parts), Error>;
}

impl<'a> LexWrite for crate::SliceLexer<'a> {
    type Num = &'a str;

    fn num_string(&mut self, prefix: &str) -> Result<(Self::Num, Parts), Error> {
        let mut num = &self.whole[self.offset() - prefix.as_bytes().len()..self.offset()];
        debug_assert_eq!(num, prefix.as_bytes());
        let parts = self.num_bytes(&mut num)?;
        // SAFETY: conversion to UTF-8 always succeeds because
        // lex_number validates everything it writes to num
        Ok((core::str::from_utf8(num).unwrap(), parts))
    }
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> LexWrite for crate::IterLexer<E, I> {
    type Num = alloc::string::String;

    fn num_string(&mut self, prefix: &str) -> Result<(Self::Num, Parts), Error> {
        let mut num = prefix.into();
        let parts = self.num_bytes(&mut num)?;
        // SAFETY: conversion to UTF-8 always succeeds because
        // lex_number validates everything it writes to num
        Ok((alloc::string::String::from_utf8(num).unwrap(), parts))
    }
}
