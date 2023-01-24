use crate::{Error, Lex, Token};

pub fn from_token<L: Lex>(token: Token, lexer: &mut L) -> Result<(), Error> {
    match token {
        Token::Null | Token::True | Token::False => Ok(()),
        Token::DigitOrMinus => Ok(lexer.num_ignore().map(|_| ())?),
        Token::Quote => Ok(lexer.str_ignore()?),
        Token::LSquare => lexer.seq(Token::RSquare, from_token),
        Token::LCurly => lexer.seq(Token::RCurly, |token, lexer| {
            lexer.str_colon(token, |lexer| lexer.str_ignore().map_err(Error::Str))?;
            from_token(lexer.ws_token().ok_or(Error::ExpectedValue)?, lexer)
        }),
        token => Err(Error::Token(token)),
    }
}

pub fn many<L: Lex>(lexer: &mut L) -> Result<(), Error> {
    while let Some(token) = lexer.ws_token() {
        from_token(token, lexer)?
    }
    Ok(())
}
