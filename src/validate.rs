use crate::{Error, Lex, Token};

pub fn from_token<L: Lex>(token: Token, lexer: &mut L) -> Result<(), Error> {
    match token {
        Token::Null => Ok(()),
        Token::True => Ok(()),
        Token::False => Ok(()),
        Token::DigitOrMinus => Ok(lexer.num_ignore().map(|_| ())?),
        Token::Quote => Ok(lexer.str_ignore()?),
        Token::LSquare => lexer.seq(Token::RSquare, |token, lexer| from_token(token, lexer)),
        Token::LCurly => lexer.seq(Token::RCurly, |token, lexer| {
            token.equals_or(Token::Quote, Error::ExpectedString)?;
            lexer.str_ignore()?;
            let colon = lexer.ws_token().filter(|t| *t == Token::Colon);
            colon.ok_or(Error::ExpectedColon)?;
            let token = lexer.ws_token().ok_or(Error::ExpectedValue)?;
            from_token(token, lexer)
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
