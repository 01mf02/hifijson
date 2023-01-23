//! Tokens.

#[derive(Debug)]
pub enum Error {
    ExpectedItem,
    ExpectedItemOrEnd,
    ExpectedCommaOrEnd,
    ExpectedToken,
    ExpectedEof,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use Error::*;
        match self {
            ExpectedItem => "item expected".fmt(f),
            ExpectedItemOrEnd => "item or end of sequence expected".fmt(f),
            ExpectedCommaOrEnd => "comma or end of sequence expected".fmt(f),
            _ => todo!(),
        }
    }
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

impl core::fmt::Display for Token {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use Token::*;
        match self {
            Null => "null".fmt(f),
            True => "true".fmt(f),
            False => "false".fmt(f),
            Comma => ",".fmt(f),
            Colon => ":".fmt(f),
            LSquare => "[".fmt(f),
            RSquare => "]".fmt(f),
            LCurly => "{".fmt(f),
            RCurly => "}".fmt(f),
            Quote => '"'.fmt(f),
            DigitOrMinus => "number".fmt(f),
            Error => "unknown token".fmt(f),
        }
    }
}

impl Token {
    /// Return `Ok(())` if `self` equals `token`, else return `Err(err)`.
    pub fn equals_or<E>(&self, token: Token, err: E) -> Result<(), E> {
        if *self == token {
            Ok(())
        } else {
            Err(err)
        }
    }
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
        F: FnMut(Token, &mut Self) -> Result<(), E>,
    {
        let mut token = self.ws_token().ok_or(Error::ExpectedItemOrEnd)?;
        if token == end {
            return Ok(());
        };

        loop {
            f(token, self)?;
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

    fn exactly_one<T, E: From<Error>, F>(&mut self, f: F) -> Result<T, E>
    where
        F: FnOnce(Token, &mut Self) -> Result<T, E>,
    {
        let token = self.ws_token().ok_or(Error::ExpectedToken)?;
        let v = f(token, self)?;
        self.eat_whitespace();
        match self.peek_next() {
            None => Ok(v),
            Some(_) => Err(Error::ExpectedEof)?,
        }
    }
}

impl<T> Lex for T where T: crate::Read {}
