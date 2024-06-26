//! Numbers.

use crate::{Read, Write};
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
    /// Perform `f` for every digit read.
    fn digits_foreach(&mut self, mut f: impl FnMut(u8)) {
        while let Some(digit @ (b'0'..=b'9')) = self.peek_next() {
            f(*digit);
            self.read_next()
        }
    }

    /// Return number of digits read and fail if no digit was encountered.
    fn digits1_ignore(&mut self) -> Result<NonZeroUsize, Error> {
        let mut len = 0;
        self.digits_foreach(|_| len += 1);
        NonZeroUsize::new(len).ok_or(Error::ExpectedDigit)
    }

    /// Run function for every digit, fail if no digit encountered.
    fn digits1_foreach(&mut self, mut f: impl FnMut(u8)) -> Result<NonZeroUsize, Error> {
        let mut len = 0;
        self.digits_foreach(|d| {
            f(d);
            len += 1
        });
        NonZeroUsize::new(len).ok_or(Error::ExpectedDigit)
    }

    /// Run function for each character of a number.
    fn num_foreach(&mut self, mut f: impl FnMut(u8)) -> Result<Parts, Error> {
        let mut pos = 0;
        let mut parts = Parts::default();

        if let Some(b'-') = self.peek_next() {
            f(b'-');
            self.read_next();
            pos += 1;
        }

        match self.peek_next() {
            Some(b'0') => {
                f(b'0');
                self.read_next();
                pos += 1;
            }
            Some(digit @ b'1'..=b'9') => {
                f(*digit);
                self.read_next();
                pos += 1;
                self.digits_foreach(|digit| {
                    f(digit);
                    pos += 1
                })
            }
            _ => return Err(Error::ExpectedDigit),
        }

        loop {
            match self.peek_next() {
                Some(b'.') if parts.is_int() => {
                    parts.dot = Some(NonZeroUsize::new(pos).unwrap());
                    f(b'.');
                    self.read_next();
                    pos += 1 + self.digits1_foreach(&mut f)?.get();
                }

                Some(exp @ (b'e' | b'E')) if parts.exp.is_none() => {
                    parts.exp = Some(NonZeroUsize::new(pos).unwrap());
                    f(*exp);
                    self.read_next();

                    if let Some(sign @ (b'+' | b'-')) = self.peek_next() {
                        f(*sign);
                        self.read_next();
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

    /// Write a number to bytes and save its parts.
    fn num_bytes(&mut self, bytes: &mut Self::Bytes) -> Result<Parts, Error>;
    /// Read a number to a string and save its parts.
    fn num_string(&mut self) -> Result<(Self::Num, Parts), Error>;
}

fn digits(s: &[u8]) -> usize {
    s.iter()
        .position(|c| !matches!(c, b'0'..=b'9'))
        .unwrap_or(s.len())
}

impl<'a> LexWrite for crate::SliceLexer<'a> {
    type Num = &'a str;

    fn num_bytes(&mut self, bytes: &mut Self::Bytes) -> Result<Parts, Error> {
        let mut pos = usize::from(self.slice[0] == b'-');
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

    fn num_string(&mut self) -> Result<(Self::Num, Parts), Error> {
        let mut num = Default::default();
        let pos = self.num_bytes(&mut num)?;
        // SAFETY: conversion to UTF-8 always succeeds because
        // lex_number validates everything it writes to num
        Ok((core::str::from_utf8(num).unwrap(), pos))
    }
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> crate::IterLexer<E, I> {
    fn digits(&mut self, num: &mut <Self as Write>::Bytes) -> Result<(), Error> {
        let mut some_digit = false;
        while let Some(digit @ (b'0'..=b'9')) = self.last {
            some_digit = true;
            num.push(digit);
            self.last = self.read();
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

    fn num_bytes(&mut self, num: &mut Self::Bytes) -> Result<Parts, Error> {
        let mut parts = Parts::default();

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

    fn num_string(&mut self) -> Result<(Self::Num, Parts), Error> {
        let mut num = Default::default();
        let pos = self.num_bytes(&mut num)?;
        // SAFETY: conversion to UTF-8 always succeeds because
        // lex_number validates everything it writes to num
        Ok((alloc::string::String::from_utf8(num).unwrap(), pos))
    }
}
