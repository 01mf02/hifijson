//! Strings with allocation.

use crate::{str, IterLexer, SliceLexer, Write};
use alloc::{borrow::Cow, string::String};

pub trait LexAlloc: str::LexWrite + Write {
    type Str: core::ops::Deref<Target = str>;

    fn str_string(&mut self) -> Result<Self::Str, str::Error>;
}

impl<'a> LexAlloc for SliceLexer<'a> {
    type Str = alloc::borrow::Cow<'a, str>;

    fn str_string(&mut self) -> Result<Self::Str, str::Error> {
        let on_string = |bytes: &mut Self::Bytes, out: &mut Self::Str| {
            match core::str::from_utf8(bytes).map_err(str::Error::Utf8)? {
                s if s.is_empty() => (),
                s if out.is_empty() => *out = Cow::Borrowed(s),
                s => out.to_mut().push_str(s),
            };
            Ok::<_, str::Error>(())
        };
        use crate::{escape::Lex as _, str::LexWrite as _};
        self.str_fold(Cow::Borrowed(""), on_string, |lexer, escape, out| {
            out.to_mut()
                .push(lexer.escape_char(escape).map_err(str::Error::Escape)?);
            Ok(())
        })
    }
}

impl<E, I: Iterator<Item = Result<u8, E>>> LexAlloc for IterLexer<E, I> {
    type Str = alloc::string::String;

    fn str_string(&mut self) -> Result<Self::Str, str::Error> {
        let on_string = |bytes: &mut Self::Bytes, out: &mut Self::Str| {
            if bytes.is_empty() {
                return Ok(());
            }
            if out.is_empty() {
                *out = String::from_utf8(core::mem::take(bytes))
                    .map_err(|e| str::Error::Utf8(e.utf8_error()))?;
            } else {
                out.push_str(core::str::from_utf8(bytes).map_err(str::Error::Utf8)?);
                bytes.clear();
            };
            Ok::<_, str::Error>(())
        };
        use crate::{escape::Lex as _, str::LexWrite as _};
        self.str_fold(Self::Str::new(), on_string, |lexer, escape, out| {
            out.push(lexer.escape_char(escape).map_err(str::Error::Escape)?);
            Ok(())
        })
    }
}
