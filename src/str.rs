//! Strings.
//!
//! Converting JSON strings to Rust strings can require allocation, because
//! escape sequences (such as `\n` or `\\`) in the JSON input
//! have to be converted to Rust characters.
//! For example,
//! `\n` is mapped to the new line character, and
//! `\\` is mapped to a single backslash.
//!
//! To provide flexibility, this module provides
//! three different traits to parse JSON strings:
//!
//! * `Lex`: This is the most basic trait and allows only to
//!   lex a string and discard its contents.
//!   This can be useful if you know beforehand that
//!   you do not care about the contents of the string,
//!   because it is very fast and does not allocate memory.
//! * `LexWrite`: This trait lexes a string,
//!   but does not map escape sequences to the corresponding Rust characters.
//!   This never allocates memory when lexing from slices,
//!   but it always allocates memory when lexing from an iterator.
//! * `LexAlloc`: This trait lexes a string,
//!   mapping escape sequences to corresponding Rust characters.
//!   Like `LexWrite`, this always allocates memory when lexing from an iterator,
//!   but it allocates memory when lexing from a slice *only* if the input string contains at least one escape sequence.
//!
//! When in doubt, go for `LexAlloc`.

use crate::escape;
use crate::{Read, Write};
use core::fmt;
use core::ops::Deref;

/// Wrapper type to facilitate printing strings as JSON.
pub struct Display<Str>(Str);

impl<Str> Display<Str> {
    /// Create a new string to be printed as JSON string.
    pub fn new(s: Str) -> Self {
        Self(s)
    }
}

impl<Str: Deref<Target = str>> fmt::Display for Display<Str> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        '"'.fmt(f)?;
        for c in self.0.chars() {
            match c {
                '\\' | '"' | '\n' | '\r' | '\t' => c.escape_default().try_for_each(|c| c.fmt(f)),
                c if (c as u32) < 20 => write!(f, "\\u{:04x}", c as u16),
                c => c.fmt(f),
            }?
        }
        '"'.fmt(f)
    }
}

/// String lexing error.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// ASCII control sequence (between 0 and 0x1F) was found
    Control,
    /// escape sequence (starting with `'\n'`) could not be decoded
    Escape(escape::Error),
    /// string was not terminated
    Eof,
    /// string is not in UTF-8
    Utf8(core::str::Utf8Error),
}

impl Error {
    /// True if the string is not in UTF-8 or an UTF-16 escape sequence is invalid.
    ///
    /// These errors do never occur when parsing strings via
    /// [`Lex::str_ignore`] or [`LexWrite::str_bytes`].
    /// However, they can occur when parsing strings via
    /// [`LexAlloc::str_string`].
    pub fn is_unicode_error(&self) -> bool {
        use escape::Error::*;
        matches!(
            self,
            Self::Utf8(_) | Self::Escape(InvalidChar(_) | ExpectedLowSurrogate)
        )
    }
}

impl_from!(escape::Error, Error, Error::Escape);

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use Error::*;
        match self {
            Control => "invalid string control character".fmt(f),
            Escape(e) => e.fmt(f),
            Eof => "unterminated string".fmt(f),
            Utf8(e) => e.fmt(f),
        }
    }
}

/// String lexing state machine.
#[derive(Default)]
struct State {
    /// Are we in an escape sequence, and if so,
    /// are we in a unicode escape sequence, and if so,
    /// at which position in the hex code are we?
    escape: Option<Option<u8>>,
    /// Did we encounter an error so far?
    error: Option<Error>,
}

impl State {
    /// Process the next character of a string,
    /// return whether the string is finished or an error occurred.
    fn process(&mut self, c: u8) -> bool {
        // are we in an escape sequence (started by '\')?
        if let Some(unicode) = &mut self.escape {
            // are we in a Unicode escape sequence (started by "\u")?
            if let Some(hex_pos) = unicode {
                if escape::decode_hex(c).is_none() {
                    self.error = Some(escape::Error::InvalidHex(c).into())
                } else if *hex_pos < 3 {
                    *hex_pos += 1
                } else {
                    self.escape = None
                }
            } else {
                // we are about to enter a new escape sequence,
                // let us see which kind of sequence ...
                if c == b'u' {
                    *unicode = Some(0)
                } else if escape::Lit::try_from(c).is_some() {
                    self.escape = None
                } else {
                    self.error = Some(escape::Error::InvalidKind(c).into())
                }
            }
        } else {
            // we are not in any escape sequence
            match c {
                b'"' => return true,
                b'\\' => self.escape = Some(None),
                0..=0x1F => self.error = Some(Error::Control),
                _ => return false,
            };
        }
        self.error.is_some()
    }

    /// Ensure that once `process` has returned `true`, the string has actually terminated.
    fn finish(self, mut next: impl FnMut() -> Option<u8>) -> Result<(), Error> {
        match self.error {
            Some(e) => Err(e),
            None if self.escape.is_some() => Err(escape::Error::Eof)?,
            None if next() != Some(b'"') => Err(Error::Eof),
            None => Ok(()),
        }
    }
}

/// String lexing that does never allocate.
pub trait Lex: escape::Lex {
    /// Read a string without saving it.
    fn str_ignore(&mut self) -> Result<(), Error> {
        self.str_foreach(|_| ())
    }

    /// Run a function for every character of the string.
    fn str_foreach(&mut self, f: impl FnMut(u8)) -> Result<(), Error> {
        let mut state = State::default();
        self.foreach_until(f, |c| state.process(c));
        state.finish(|| self.take_next())
    }
}

impl<T> Lex for T where T: escape::Lex {}

/// String lexing that allocates only when lexing from iterators.
pub trait LexWrite: escape::Lex + Read + Write {
    /// Read a string to bytes, copying escape sequences one-to-one.
    fn str_bytes(&mut self, bytes: &mut Self::Bytes) -> Result<(), Error> {
        let mut state = State::default();
        self.write_until(bytes, |c| state.process(c));
        state.finish(|| self.take_next())
    }

    /// Lex a string by executing `on_string` on every string and `on_bytes` on every escape sequence.
    fn str_fold<E: From<Error>, T>(
        &mut self,
        mut out: T,
        on_string: impl Fn(&mut Self::Bytes, &mut T) -> Result<(), E>,
        on_escape: impl Fn(&mut Self, &mut T) -> Result<(), E>,
    ) -> Result<T, E> {
        fn string_end(c: u8) -> bool {
            matches!(c, b'\\' | b'"' | 0..=0x1F)
        }

        let mut bytes = Self::Bytes::default();
        self.write_until(&mut bytes, string_end);
        on_string(&mut bytes, &mut out)?;
        match self.take_next().ok_or(Error::Eof)? {
            b'\\' => (),
            b'"' => return Ok(out),
            0..=0x1F => return Err(Error::Control)?,
            _ => unreachable!(),
        }
        loop {
            on_escape(self, &mut out)?;
            self.write_until(&mut bytes, string_end);
            on_string(&mut bytes, &mut out)?;
            match self.take_next().ok_or(Error::Eof)? {
                b'\\' => continue,
                b'"' => return Ok(out),
                0..=0x1F => return Err(Error::Control)?,
                _ => unreachable!(),
            }
        }
    }
}

impl<T> LexWrite for T where T: Read + Write {}

/// String lexing that always allocates when lexing from iterators and
/// allocates when lexing from slices that contain escape sequences.
pub trait LexAlloc: LexWrite {
    /// The type of string that we are lexing into.
    type Str: Deref<Target = str>;

    /// Lex a JSON string to a Rust string.
    fn str_string(&mut self) -> Result<Self::Str, Error>;
}

#[cfg(feature = "alloc")]
impl<'a> LexAlloc for crate::SliceLexer<'a> {
    type Str = alloc::borrow::Cow<'a, str>;

    fn str_string(&mut self) -> Result<Self::Str, Error> {
        use alloc::borrow::Cow;

        let on_string = |bytes: &mut Self::Bytes, out: &mut Self::Str| {
            match core::str::from_utf8(bytes).map_err(Error::Utf8)? {
                "" => (),
                s if out.is_empty() => *out = Cow::Borrowed(s),
                s => out.to_mut().push_str(s),
            };
            Ok::<_, Error>(())
        };
        use crate::escape::Lex;
        self.str_fold(Cow::Borrowed(""), on_string, |lexer, out| {
            let next = lexer.take_next().ok_or(escape::Error::Eof)?;
            out.to_mut()
                .push(lexer.escape(next).map_err(Error::Escape)?);
            Ok(())
        })
    }
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> LexAlloc for crate::IterLexer<E, I> {
    type Str = alloc::string::String;

    fn str_string(&mut self) -> Result<Self::Str, Error> {
        use alloc::string::String;

        let on_string = |bytes: &mut Self::Bytes, out: &mut Self::Str| {
            match bytes {
                b if b.is_empty() => (),
                b if out.is_empty() => {
                    *out = String::from_utf8(core::mem::take(b))
                        .map_err(|e| Error::Utf8(e.utf8_error()))?
                }
                b => out.push_str(core::str::from_utf8(b).map_err(Error::Utf8)?),
            }
            Ok::<_, Error>(())
        };
        use crate::escape::Lex;
        self.str_fold(Self::Str::new(), on_string, |lexer, out| {
            let next = lexer.take_next().ok_or(escape::Error::Eof)?;
            out.push(lexer.escape(next).map_err(Error::Escape)?);
            Ok(())
        })
    }
}
