#[derive(Debug)]
pub enum Error {
    ExpectedItem,
    ExpectedItemOrEnd,
    ExpectedCommaOrEnd,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    /// `null`
    Null,
    /// `true`
    True,
    /// `false`
    False,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `[`
    LSquare,
    /// `]`
    RSquare,
    /// `{`
    LCurly,
    /// `}`
    RCurly,
    /// `"`
    Quote,
    /// a digit (0-9) or a minus (`-`)
    DigitOrMinus,
    /// anything else
    Error,
}

pub trait Lex: crate::Read {
    /// Skip input until the earliest non-whitespace character.
    fn eat_whitespace(&mut self) {
        self.skip_next_until(|c| !matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
    }

    /// Skip potential whitespace and return the following token if there is some.
    fn ws_token(&mut self) -> Option<Token> {
        self.eat_whitespace();
        Some(self.token(*self.peek_next()?))
    }

    fn exact<const N: usize>(&mut self, s: [u8; N], out: Token) -> Token {
        // we are calling this function without having advanced before
        self.take_next();
        if self.strip_prefix(s) {
            out
        } else {
            Token::Error
        }
    }

    /// Convert a character to a token, such as '`:`' to `Token::Colon`.
    ///
    /// When the token consists of several characters, such as
    /// `null`, `true`, or `false`,
    /// also consume the following characters.
    fn token(&mut self, c: u8) -> Token {
        let token = match c {
            // it is important to `return` here in order not to read a byte,
            // like we do for the regular, single-character tokens
            b'n' => return self.exact([b'u', b'l', b'l'], Token::Null),
            b't' => return self.exact([b'r', b'u', b'e'], Token::True),
            b'f' => return self.exact([b'a', b'l', b's', b'e'], Token::False),
            b'0'..=b'9' | b'-' => return Token::DigitOrMinus,
            b'"' => Token::Quote,
            b'[' => Token::LSquare,
            b']' => Token::RSquare,
            b'{' => Token::LCurly,
            b'}' => Token::RCurly,
            b',' => Token::Comma,
            b':' => Token::Colon,
            _ => Token::Error,
        };
        self.take_next();
        token
    }

    /// Execute `f` for every item in the comma-separated sequence until `end`.
    fn seq<E: From<Error>, F>(&mut self, end: Token, mut f: F) -> Result<(), E>
    where
        F: FnMut(&mut Self, Token) -> Result<(), E>,
    {
        let mut token = self.ws_token().ok_or(Error::ExpectedItemOrEnd)?;
        if token == end {
            return Ok(());
        };

        loop {
            f(self, token)?;
            token = self.ws_token().ok_or(Error::ExpectedCommaOrEnd)?;
            if token == end {
                return Ok(());
            } else if token == Token::Comma {
                token = self.ws_token().ok_or(Error::ExpectedItem)?;
            } else {
                return Err(Error::ExpectedCommaOrEnd)?;
            }
        }
    }
}

impl<T> Lex for T where T: crate::Read {}
