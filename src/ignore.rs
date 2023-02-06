//! Discarding values.

use crate::{Error, Expect, Lex, Token};

/// Parse and discard a value.
pub fn parse<L: Lex>(token: Token, lexer: &mut L) -> Result<(), Error> {
    match token {
        Token::Null | Token::True | Token::False => Ok(()),
        Token::DigitOrMinus => Ok(lexer.num_ignore().map(|_| ())?),
        Token::Quote => Ok(lexer.str_ignore()?),
        Token::LSquare => lexer.seq(Token::RSquare, parse),
        Token::LCurly => lexer.seq(Token::RCurly, |token, lexer| {
            lexer.str_colon(token, |lexer| lexer.str_ignore().map_err(Error::Str))?;
            parse(lexer.ws_token().ok_or(Expect::Value)?, lexer)
        }),
        _ => Err(Expect::Value)?,
    }
}
