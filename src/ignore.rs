//! Discarding values.

use crate::{Error, Expect, Lex};

/// Parse and discard a value.
pub fn parse<L: Lex>(next: u8, lexer: &mut L) -> Result<(), Error> {
    match next {
        b'a'..=b'z' => Ok(lexer.null_or_bool().map(|_| ()).ok_or(Expect::Value)?),
        b'0'..=b'9' => Ok(lexer.num_ignore().map(|_| ())?),
        b'-' => Ok(lexer.discarded().num_ignore().map(|_| ())?),
        b'"' => Ok(lexer.discarded().str_ignore()?),
        b'[' => lexer.discarded().seq(b']', L::ws_peek, parse),
        b'{' => lexer.discarded().seq(b'}', L::ws_peek, |next, lexer| {
            lexer.expect(|_| Some(next), b'"').ok_or(Expect::String)?;
            lexer.str_ignore().map_err(Error::Str)?;
            lexer.expect(L::ws_peek, b':').ok_or(Expect::Colon)?;
            parse(lexer.ws_peek().ok_or(Expect::Value)?, lexer)
        }),
        _ => Err(Expect::Value)?,
    }
}
