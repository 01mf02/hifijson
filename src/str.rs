use crate::escape::{self, Escape};

#[derive(Debug)]
pub enum Error {
    Control,
    Escape(escape::Error),
    Eof,
    Utf8(core::str::Utf8Error),
}

impl<T> Lex for T where T: escape::Lex {}

pub trait Lex: escape::Lex {
    /// Read a string to bytes, copying escape sequences one-to-one.
    fn str_bytes(&mut self, bytes: &mut Self::Bytes) -> Result<(), Error> {
        let mut escaped = false;
        let mut unicode = false;
        let mut hex_pos = 0;
        let mut error = None;

        self.read_until(bytes, |c| {
            if escaped {
                if unicode {
                    if escape::decode_hex(c).is_none() {
                        error = Some(Error::Escape(escape::Error::InvalidHex));
                    } else if hex_pos < 3 {
                        hex_pos += 1
                    } else {
                        escaped = false;
                        unicode = false;
                        hex_pos = 0;
                    }
                } else {
                    match Escape::try_from(c) {
                        Some(Escape::Unicode(_)) => unicode = true,
                        Some(_) => escaped = false,
                        None => error = Some(Error::Escape(escape::Error::UnknownKind)),
                    }
                }
            } else {
                match c {
                    b'"' => return true,
                    b'\\' => escaped = true,
                    0..=19 => error = Some(Error::Control),
                    _ => (),
                };
            }
            error.is_some()
        });
        match error {
            Some(e) => Err(e),
            None if escaped || self.read_byte() != Some(b'"') => Err(Error::Eof),
            _ => Ok(()),
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
