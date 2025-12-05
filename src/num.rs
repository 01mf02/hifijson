//! Positive numbers.
//!
//! The lexers in this modules, in particular
//! [`Lex::num_ignore`] and
//! [`LexWrite::num_string`],
//! accept numbers corresponding to the regex
//! `\d+(\.\d+)?([eE]\d+)?`.
//! This is strictly more general than the JSON specification,
//! which specifies numbers as
//! `(0|[1-9]\d*)(\.\d+)?([eE]\d+)?`.
//! That excludes numbers like `007`, which are accepted by the former regex.
//!
//! If you require stricter conformance to JSON numbers,
//! you can rule out lexed numbers that start with `0\d` manually after lexing.

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

/// Position of `.` and `e`/`E` in the string representation of a number.
///
/// Because a number cannot start with `.` or `e`/`E`,
/// these positions must always be greater than zero.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parts {
    /// position of the dot
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
            parts.num_end(len, prev, c) || {
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
    /// Returns whether the next character `c` terminates the current number.
    fn num_end(&mut self, len: usize, prev: Option<u8>, c: u8) -> bool {
        let prev_digit = || prev.map_or(false, |c| c.is_ascii_digit());
        match c {
            _ if c.is_ascii_digit() => false,
            b'.' if self.dot.is_none() && self.exp.is_none() && prev_digit() => {
                self.dot = Some(len);
                false
            }
            b'e' | b'E' if self.exp.is_none() && prev_digit() => {
                self.exp = Some(len);
                false
            }
            _ => true,
        }
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
        self.append_until(num, |b, c| parts.num_end(b.len(), b.last().copied(), c));
        let valid = num.last().map_or(false, u8::is_ascii_digit);
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
