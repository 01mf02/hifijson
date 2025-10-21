//! Positive numbers.

use crate::{Read, Write};
use core::fmt::{self, Display};
use core::num::NonZeroUsize;

/// Number lexing error.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The only thing that can go wrong during number lexing is
    /// that we are not reading even a single digit.
    /// Once a single digit has been read,
    /// unexpected sequences afterwards are ignored by this lexer.
    /// For example, if the lexer encounters `42abc`,
    /// it returns only `42` and does not touch `abc`.
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
    pub dot: Option<NonZeroUsize>,
    /// position of the exponent character (`e`/`E`)
    pub exp: Option<NonZeroUsize>,
}

impl Parts {
    /// Return true if the number contains neither a dot not an exponent.
    pub fn is_int(&self) -> bool {
        self.dot.is_none() && self.exp.is_none()
    }
}

/// Number lexing, ignoring the number.
pub trait Lex: Read {
    /// Perform `f` for every digit read and return the number of read bytes.
    fn digits_foreach(&mut self, mut f: impl FnMut(u8)) -> usize {
        let mut len = 0;
        while let Some(digit @ (b'0'..=b'9')) = self.peek_next() {
            f(digit);
            self.take_next();
            len += 1;
        }
        len
    }

    /// Run function for every digit, fail if no digit encountered.
    fn digits1_foreach(&mut self, f: impl FnMut(u8)) -> Result<NonZeroUsize, Error> {
        NonZeroUsize::new(self.digits_foreach(f)).ok_or(Error::ExpectedDigit)
    }

    /// Run function for each character of a number.
    fn num_foreach(&mut self, mut f: impl FnMut(u8)) -> Result<Parts, Error> {
        let mut pos = 0;
        let mut parts = Parts::default();

        match self.take_next() {
            Some(b'0') => {
                f(b'0');
                pos += 1;
            }
            Some(digit @ b'1'..=b'9') => {
                f(digit);
                pos += 1 + self.digits_foreach(&mut f);
            }
            _ => return Err(Error::ExpectedDigit),
        }

        loop {
            match self.peek_next() {
                Some(b'.') if parts.is_int() => {
                    parts.dot = Some(NonZeroUsize::new(pos).unwrap());
                    f(b'.');
                    self.take_next();
                    pos += 1 + self.digits1_foreach(&mut f)?.get();
                }

                Some(exp @ (b'e' | b'E')) if parts.exp.is_none() => {
                    parts.exp = Some(NonZeroUsize::new(pos).unwrap());
                    f(exp);
                    self.take_next();

                    if let Some(sign @ (b'+' | b'-')) = self.peek_next() {
                        f(sign);
                        self.take_next();
                        pos += 1;
                    }

                    pos += 1 + self.digits1_foreach(&mut f)?.get();
                }
                _ => return Ok(parts),
            }
        }
    }

    /// Lex a number and ignore its contents, saving only its parts.
    fn num_ignore(&mut self) -> Result<Parts, Error> {
        self.num_foreach(|_| ())
    }
}

impl<T> Lex for T where T: Read {}

/// Number lexing, keeping the number.
pub trait LexWrite: Lex + Write {
    /// String type to save numbers as.
    type Num: core::ops::Deref<Target = str>;

    /// Write a prefix and a number to bytes and save its parts.
    ///
    /// `prefix` must be a suffix of the previously consumed input.
    /// Normally, you pass `b"-"` as prefix if you read "-" just before.
    /// This allows you to include "-" in the bytes without allocation.
    fn num_bytes(&mut self, bytes: &mut Self::Bytes, prefix: &[u8]) -> Result<Parts, Error>;
    /// Write a prefix and a number to a string and save its parts.
    fn num_string(&mut self, prefix: &str) -> Result<(Self::Num, Parts), Error>;
}

fn digits(s: &[u8]) -> usize {
    s.iter()
        .position(|c| !c.is_ascii_digit())
        .unwrap_or(s.len())
}

impl<'a> LexWrite for crate::SliceLexer<'a> {
    type Num = &'a str;

    fn num_bytes(&mut self, bytes: &mut Self::Bytes, prefix: &[u8]) -> Result<Parts, Error> {
        // rewind by prefix length
        self.slice = &self.whole[self.offset() - prefix.len()..];
        assert!(self.slice.starts_with(prefix));

        let mut pos = prefix.len();
        let mut parts = Parts::default();

        let digits1 = |s| NonZeroUsize::new(digits(s)).ok_or(Error::ExpectedDigit);

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

    fn num_string(&mut self, prefix: &str) -> Result<(Self::Num, Parts), Error> {
        let mut num = Default::default();
        let parts = self.num_bytes(&mut num, prefix.as_bytes())?;
        // SAFETY: conversion to UTF-8 always succeeds because
        // lex_number validates everything it writes to num
        Ok((core::str::from_utf8(num).unwrap(), parts))
    }
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> crate::IterLexer<E, I> {
    fn digits(&mut self, num: &mut <Self as Write>::Bytes) -> Result<(), Error> {
        let mut some_digit = false;
        while let Some(digit @ (b'0'..=b'9')) = self.peek_next() {
            some_digit = true;
            num.push(digit);
            self.take_next();
        }
        if some_digit && self.error.is_none() {
            Ok(())
        } else {
            Err(Error::ExpectedDigit)
        }
    }
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> LexWrite for crate::IterLexer<E, I> {
    type Num = alloc::string::String;

    fn num_bytes(&mut self, num: &mut Self::Bytes, prefix: &[u8]) -> Result<Parts, Error> {
        num.extend(prefix);
        let mut parts = Parts::default();

        if self.peek_next() == Some(b'0') {
            num.push(b'0');
            self.take_next();
        } else {
            self.digits(num)?;
        }

        loop {
            match self.peek_next() {
                Some(b'.') if parts.dot.is_none() && parts.exp.is_none() => {
                    parts.dot = Some(NonZeroUsize::new(num.len()).unwrap());
                    num.push(b'.');
                    self.take_next();

                    self.digits(num)?;
                }

                Some(e @ (b'e' | b'E')) if parts.exp.is_none() => {
                    parts.exp = Some(NonZeroUsize::new(num.len()).unwrap());
                    num.push(e);
                    self.take_next();

                    if let Some(sign @ (b'+' | b'-')) = self.peek_next() {
                        num.push(sign);
                        self.take_next();
                    }

                    self.digits(num)?;
                }
                _ => return Ok(parts),
            }
        }
    }

    fn num_string(&mut self, prefix: &str) -> Result<(Self::Num, Parts), Error> {
        let mut num = Default::default();
        let parts = self.num_bytes(&mut num, prefix.as_bytes())?;
        // SAFETY: conversion to UTF-8 always succeeds because
        // lex_number validates everything it writes to num
        Ok((alloc::string::String::from_utf8(num).unwrap(), parts))
    }
}
