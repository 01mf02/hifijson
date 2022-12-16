//! Strings without allocation.

use crate::escape::{self, Escape};

#[derive(Debug)]
pub enum Error {
    Control,
    Escape(escape::Error),
    Eof,
    Utf8(core::str::Utf8Error),
}

impl<T> Lex for T where T: escape::Lex {}

#[derive(Default)]
struct State {
    escape: Option<Option<usize>>,
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
}

pub trait Lex: escape::Lex {
    /// Read a string to bytes, copying escape sequences one-to-one.
    fn str_bytes(&mut self, bytes: &mut Self::Bytes) -> Result<(), Error> {
        let mut state = State::default();
        self.read_until(bytes, |c| state.process(c));
        match state.error {
            Some(e) => Err(e),
            None if state.escape.is_some() || self.read_byte() != Some(b'"') => Err(Error::Eof),
            None => Ok(()),
        }
    }

    /// Read a string without saving it.
    fn str_ignore(&mut self) -> Result<(), Error> {
        let mut state = State::default();
        self.skip_until(|c| state.process(c));
        match state.error {
            Some(e) => Err(e),
            None if state.escape.is_some() || self.read_byte() != Some(b'"') => Err(Error::Eof),
            None => Ok(()),
        }
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
        self.read_until(&mut bytes, string_end);
        on_string(&mut bytes, &mut out)?;
        match self.read_byte().ok_or(Error::Eof)? {
            b'\\' => (),
            b'"' => return Ok(out),
            _ => return Err(Error::Control)?,
        }
        loop {
            let escape = self.escape().map_err(Error::Escape)?;
            on_escape(self, escape, &mut out)?;
            self.read_until(&mut bytes, string_end);
            on_string(&mut bytes, &mut out)?;
            match self.read_byte().ok_or(Error::Eof)? {
                b'\\' => continue,
                b'"' => return Ok(out),
                _ => return Err(Error::Control)?,
            }
        }
    }
}
