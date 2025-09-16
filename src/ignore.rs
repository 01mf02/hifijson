//! Discarding values.

use crate::{Error, Expect, Lex, Token};

/// Parse and discard a value.
pub fn parse<L: Lex>(token: Token, lexer: &mut L) -> Result<(), Error> {
    match token {
        Token::Letter => Ok(lexer.null_or_bool().map(|_| ()).ok_or(Expect::Value)?),
        Token::Digit | Token::Minus => Ok(lexer.num_ignore().map(|_| ())?),
        Token::Quote => Ok(lexer.str_ignore()?),
        Token::LSquare => lexer.seq(Token::RSquare, L::ws_token, parse),
        Token::LCurly => lexer.seq(Token::RCurly, L::ws_token, |token, lexer| {
            lexer.str_colon(token, L::ws_token, |lexer| {
                lexer.str_ignore().map_err(Error::Str)
            })?;
            parse(lexer.ws_token().ok_or(Expect::Value)?, lexer)
        }),
        _ => Err(Expect::Value)?,
    }
}
