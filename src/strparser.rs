use crate::{IterLexer, Lexer, SliceLexer, StrError};
use alloc::{borrow::Cow, string::String};

pub trait LexerStr: Lexer {
    type Str: core::ops::Deref<Target = str>;

    fn parse_string(&mut self) -> Result<Self::Str, StrError>;
}

impl<'a> LexerStr for SliceLexer<'a> {
    type Str = alloc::borrow::Cow<'a, str>;

    fn parse_string(&mut self) -> Result<Self::Str, StrError> {
        self.lex_string(
            Cow::Borrowed(""),
            |bytes, out| {
                match core::str::from_utf8(bytes).map_err(StrError::Utf8)? {
                    s if s.is_empty() => (),
                    s if out.is_empty() => *out = Cow::Borrowed(s),
                    s => out.to_mut().push_str(s),
                };
                Ok::<_, StrError>(())
            },
            |lexer, escape, out| {
                out.to_mut()
                    .push(lexer.parse_escape(escape).map_err(StrError::Escape)?);
                Ok(())
            },
        )
    }
}

impl<E, I: Iterator<Item = Result<u8, E>>> LexerStr for IterLexer<E, I> {
    type Str = alloc::string::String;

    fn parse_string(&mut self) -> Result<Self::Str, StrError> {
        self.lex_string(
            Self::Str::new(),
            |bytes, out| {
                if bytes.is_empty() {
                    return Ok(());
                }
                if out.is_empty() {
                    *out = String::from_utf8(core::mem::take(bytes))
                        .map_err(|e| StrError::Utf8(e.utf8_error()))?;
                } else {
                    out.push_str(core::str::from_utf8(bytes).map_err(StrError::Utf8)?);
                    bytes.clear();
                };
                Ok::<_, StrError>(())
            },
            |lexer, escape, out| {
                out.push(lexer.parse_escape(escape).map_err(StrError::Escape)?);
                Ok(())
            },
        )
    }
}
