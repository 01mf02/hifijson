//! Numbers.
//!
//! Conforming to the JSON specification, the lexers in this modules, in particular
//! [`Lex::num_ignore`] and
//! [`LexWrite::num_string`],
//! accept numbers corresponding to the regex
//! `-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?`.
//!
//! This leads numbers like `007` to be lexed as three separate numbers;
//! `0`, `0`, and `7`.
//! That is because after a leading `0`, the lexer expects only ".", "e" or "E",
//! so when it sees another digit (such as "0" or "7"),
//! it assumes that it is not part of the number.
//!
//! To prevent such behaviour, verify that numbers are not followed by a digit,
//! e.g. with [`crate::Read::peek_next`].
//! Alternatively, you can also instruct the number lexers to accept
//! more liberal formats that do not have the problem illustrated here.
//! See [`LexWrite::num_bytes_with`] for an example.
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

/// Parts of a number.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parts {
    /// leading zero (`0`)
    pub zero: bool,
    /// dot (`.`)
    pub dot: bool,
    /// exponent character (`e`/`E`)
    pub exp: bool,
}

/// Positions of various parts in the string representation of a number.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Num<R = u8> {
    read: R,
    parts: Parts,
}

impl Num {
    /// Parse numbers starting with `[+-]?\d+`.
    pub fn signed_digits() -> Self {
        Self::default().map_read(|_| b'e')
    }

    /// Parse numbers starting with `\d+`.
    pub fn unsigned_digits() -> Self {
        Self::default().map_read(|_| b'.')
    }

    /// Returns whether the next character `c` is part of the current number.
    fn num_part(&mut self, c: u8) -> bool {
        let Parts { zero, dot, exp } = &mut self.parts;
        match (self.read, c) {
            (0, b'-') => (),
            (0 | b'-', b'0') if !*dot && !*exp => *zero = true,
            (_, b'0'..=b'9') if !*zero || *dot || *exp => (),
            (b'0'..=b'9', b'.') if !*dot && !*exp => *dot = true,
            (b'0'..=b'9', b'e' | b'E') if !*exp => *exp = true,
            (b'e' | b'E', b'+' | b'-') => (),
            _ => return false,
        };
        self.read = c;
        true
    }

    /// Return the [`Parts`] of the number if it is valid.
    pub fn validate(self) -> Result<Parts, Error> {
        self.read
            .is_ascii_digit()
            .then(|| self.parts)
            .ok_or(Error::ExpectedDigit)
    }
}

impl<R> Num<R> {
    /// Return the contents and the [`Parts`] of the number, even if it is invalid.
    ///
    /// Because number lexing does not fail,
    /// this function can return contents that are not numbers, such as
    /// `""` and `"1."`.
    ///
    /// This can be useful if you wish to parse supersets of JSON,
    /// where you want to accept things like `"+Infinity"` as a number.
    pub fn unvalidated(self) -> (R, Parts) {
        (self.read, self.parts)
    }

    fn map_read<R2>(self, f: impl FnOnce(R) -> R2) -> Num<R2> {
        let read = f(self.read);
        let parts = self.parts;
        Num { read, parts }
    }
}

impl Parts {
    /// Return true if the number contains neither a dot not an exponent.
    pub fn is_int(&self) -> bool {
        !self.dot && !self.exp
    }
}

impl<B: core::convert::AsRef<[u8]>> Num<B> {
    /// Return the contents and the [`Parts`] of the number if it is valid.
    pub fn validated(self) -> Result<(B, Parts), Error> {
        let Self { read, parts } = self;
        let valid = read.as_ref().last().map_or(false, u8::is_ascii_digit);
        valid.then(|| (read, parts)).ok_or(Error::ExpectedDigit)
    }
}

/// Number lexing, ignoring the number.
pub trait Lex: Read {
    /// Lex a number without saving its contents.
    fn num_ignore(&mut self) -> Num {
        self.num_ignore_with(Num::default())
    }

    /// Lex a number without saving its contents, using an initial number state.
    fn num_ignore_with(&mut self, mut num: Num) -> Num {
        self.skip_until(|c| !num.num_part(c));
        num
    }
}

impl<T> Lex for T where T: Read {}

/// Number lexing, keeping the number.
pub trait LexWrite: Lex + Write {
    /// String type to save numbers as.
    type Num: core::ops::Deref<Target = str> + core::convert::AsRef<[u8]>;

    /// Write a number to bytes.
    fn num_bytes(&mut self) -> Num<Self::Bytes> {
        self.num_bytes_with(Num::default())
    }

    /// Write a number to bytes, using an initial number state.
    ///
    /// The initial number state allows you to lex number formats that
    /// diverge from the JSON specification.
    /// For example, pass [`Num::signed_digits`] to lex
    /// numbers starting with a `+` or `-` sign followed by
    /// an arbitrary sequence of digits.
    fn num_bytes_with(&mut self, mut num: Num) -> Num<Self::Bytes> {
        let mut read = Default::default();
        self.write_until(&mut read, |c| !num.num_part(c));
        num.map_read(|_| read)
    }

    /// Write a number to a string.
    fn num_string(&mut self) -> Num<Self::Num> {
        self.num_string_with(Num::default())
    }

    /// Write a number to a string, using an initial number state.
    fn num_string_with(&mut self, num: Num) -> Num<Self::Num>;
}

impl<'a> LexWrite for crate::SliceLexer<'a> {
    type Num = &'a str;

    fn num_string_with(&mut self, num: Num) -> Num<Self::Num> {
        // SAFETY: conversion to UTF-8 always succeeds because
        // num_bytes validates everything it writes to num
        self.num_bytes_with(num)
            .map_read(|read| core::str::from_utf8(read).unwrap())
    }
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> LexWrite for crate::IterLexer<E, I> {
    type Num = alloc::string::String;

    fn num_string_with(&mut self, num: Num) -> Num<Self::Num> {
        // SAFETY: conversion to UTF-8 always succeeds because
        // num_bytes validates everything it writes to num
        self.num_bytes_with(num)
            .map_read(|read| alloc::string::String::from_utf8(read).unwrap())
    }
}
