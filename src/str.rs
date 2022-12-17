//! Strings.

use crate::escape::{self, Escape};
use crate::{IterLexer, Read, SliceLexer, Write};

#[derive(Debug)]
pub enum Error {
    Control,
    Escape(escape::Error),
    Eof,
    Utf8(core::str::Utf8Error),
}

#[derive(Default)]
struct State {
    // are we in an escape sequence, and if so,
    // are we in a unicode escape sequence, and if so,
    // at which position in the hex code are we?
    escape: Option<Option<u8>>,
    // did we encounter an error so far?
    error: Option<Error>,
}

impl State {
    fn process(&mut self, c: u8) -> bool {
        if let Some(unicode) = &mut self.escape {
            if let Some(hex_pos) = unicode {
                if escape::decode_hex(c).is_none() {
                    self.error = Some(Error::Escape(escape::Error::InvalidHex));
                } else if *hex_pos < 3 {
                    *hex_pos += 1
                } else {
                    self.escape = None;
                }
            } else {
                match Escape::try_from(c) {
                    Some(Escape::Unicode(_)) => *unicode = Some(0),
                    Some(_) => self.escape = None,
                    None => self.error = Some(Error::Escape(escape::Error::UnknownKind)),
                }
            }
        } else {
            match c {
                b'"' => return true,
                b'\\' => self.escape = Some(None),
                0..=19 => self.error = Some(Error::Control),
                _ => return false,
            };
        }
        self.error.is_some()
    }

    fn finish(self, mut next: impl FnMut() -> Option<u8>) -> Result<(), Error> {
        match self.error {
            Some(e) => Err(e),
            None if self.escape.is_some() || next() != Some(b'"') => Err(Error::Eof),
            None => Ok(()),
        }
    }
}

pub trait Lex: escape::Lex {
    /// Read a string without saving it.
    fn str_ignore(&mut self) -> Result<(), Error> {
        let mut state = State::default();
        self.skip_until(|c| state.process(c));
        state.finish(|| self.take_next())
    }
}

impl<T> Lex for T where T: escape::Lex {}

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
        on_escape: impl Fn(&mut Self, Escape, &mut T) -> Result<(), E>,
    ) -> Result<T, E> {
        fn string_end(c: u8) -> bool {
            matches!(c, b'"' | b'\\' | 0..=19)
        }

        let mut bytes = Self::Bytes::default();
        self.write_until(&mut bytes, string_end);
        on_string(&mut bytes, &mut out)?;
        match self.take_next().ok_or(Error::Eof)? {
            b'\\' => (),
            b'"' => return Ok(out),
            _ => return Err(Error::Control)?,
        }
        loop {
            let escape = self.escape().map_err(Error::Escape)?;
            on_escape(self, escape, &mut out)?;
            self.write_until(&mut bytes, string_end);
            on_string(&mut bytes, &mut out)?;
            match self.take_next().ok_or(Error::Eof)? {
                b'\\' => continue,
                b'"' => return Ok(out),
                _ => return Err(Error::Control)?,
            }
        }
    }
}

impl<T> LexWrite for T where T: Read + Write {}

pub trait LexAlloc: LexWrite {
    type Str: core::ops::Deref<Target = str>;

    fn str_string(&mut self) -> Result<Self::Str, Error>;
}

#[cfg(feature = "alloc")]
impl<'a> LexAlloc for SliceLexer<'a> {
    type Str = alloc::borrow::Cow<'a, str>;

    fn str_string(&mut self) -> Result<Self::Str, Error> {
        use alloc::borrow::Cow;

        let on_string = |bytes: &mut Self::Bytes, out: &mut Self::Str| {
            match core::str::from_utf8(bytes).map_err(Error::Utf8)? {
                s if s.is_empty() => (),
                s if out.is_empty() => *out = Cow::Borrowed(s),
                s => out.to_mut().push_str(s),
            };
            Ok::<_, Error>(())
        };
        use crate::escape::Lex;
        self.str_fold(Cow::Borrowed(""), on_string, |lexer, escape, out| {
            out.to_mut()
                .push(lexer.escape_char(escape).map_err(Error::Escape)?);
            Ok(())
        })
    }
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> LexAlloc for IterLexer<E, I> {
    type Str = alloc::string::String;

    fn str_string(&mut self) -> Result<Self::Str, Error> {
        use alloc::string::String;

        let on_string = |bytes: &mut Self::Bytes, out: &mut Self::Str| {
            if bytes.is_empty() {
                return Ok(());
            }
            if out.is_empty() {
                *out = String::from_utf8(core::mem::take(bytes))
                    .map_err(|e| Error::Utf8(e.utf8_error()))?;
            } else {
                out.push_str(core::str::from_utf8(bytes).map_err(Error::Utf8)?);
                bytes.clear();
            };
            Ok::<_, Error>(())
        };
        use crate::escape::Lex;
        self.str_fold(Self::Str::new(), on_string, |lexer, escape, out| {
            out.push(lexer.escape_char(escape).map_err(Error::Escape)?);
            Ok(())
        })
    }
}
