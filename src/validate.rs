use crate::{Error, Lex, Token};

pub fn from_token<L: Lex>(lexer: &mut L, token: Token) -> Result<(), Error> {
    match token {
        Token::Null => Ok(()),
        Token::True => Ok(()),
        Token::False => Ok(()),
        Token::DigitOrMinus => Ok(lexer.num_ignore().map(|_| ())?),
        Token::Quote => Ok(lexer.str_ignore()?),
        Token::LSquare => lexer.seq(Token::RSquare, |lexer, token| from_token(lexer, token)),
        Token::LCurly => lexer.seq(Token::RCurly, |lexer, token| {
            token.equals_or(Token::Quote, Error::ExpectedString)?;
            lexer.str_ignore()?;
            let colon = lexer.ws_token().filter(|t| *t == Token::Colon);
            colon.ok_or(Error::ExpectedColon)?;
            let token = lexer.ws_token().ok_or(Error::ExpectedValue)?;
            from_token(lexer, token)
        }),
        token => Err(Error::Token(token)),
    }
}

pub fn exactly_one<L: Lex>(lexer: &mut L) -> Result<(), Error> {
    let token = lexer.ws_token().ok_or(Error::ExpectedValue)?;
    from_token(lexer, token)?;
    lexer.eat_whitespace();
    match lexer.peek_next() {
        None => Ok(()),
        Some(_) => Err(Error::ExpectedEof),
    }
}

pub fn many<L: Lex>(lexer: &mut L) -> Result<(), Error> {
    while let Some(token) = lexer.ws_token() {
        from_token(lexer, token)?
    }
    Ok(())
}
