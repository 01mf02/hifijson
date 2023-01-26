//! Tokens.

/// What we expected to get, but did not get.
#[derive(Debug)]
pub enum Expect {
    /// `   ` or `]` or `,`
    Value,
    /// `[` or `{`
    ValueOrEnd,
    /// `[1` or `[1 2`
    CommaOrEnd,
    /// `{0: 1}`
    String,
    /// `{"a" 1}`
    Colon,
    /// `true false` (when parsing exactly one value)
    Eof,
}

impl core::fmt::Display for Expect {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use Expect::*;
        match self {
            Value => "value".fmt(f),
            ValueOrEnd => "value or end of sequence".fmt(f),
            CommaOrEnd => "comma or end of sequence".fmt(f),
            String => "string".fmt(f),
            Colon => "colon".fmt(f),
            Eof => "end of file".fmt(f),
        }
    }
}

/// JSON lexer token.
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

/// Lexing that does not require allocation.
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

    /// Return `out` if the input matches `s`, otherwise return an error.
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

    /// Parse a string with given function, followed by a colon.
    fn str_colon<T, E: From<Expect>, F>(&mut self, token: Token, f: F) -> Result<T, E>
    where
        F: FnOnce(&mut Self) -> Result<T, E>,
    {
        token.equals_or(Token::Quote, Expect::String)?;
        let key = f(self)?;

        let colon = self.ws_token().filter(|t| *t == Token::Colon);
        colon.ok_or(Expect::Colon)?;

        Ok(key)
    }

    /// Execute `f` for every item in the comma-separated sequence until `end`.
    fn seq<E: From<Expect>, F>(&mut self, end: Token, mut f: F) -> Result<(), E>
    where
        F: FnMut(Token, &mut Self) -> Result<(), E>,
    {
        let mut token = self.ws_token().ok_or(Expect::ValueOrEnd)?;
        if token == end {
            return Ok(());
        };

        loop {
            f(token, self)?;
            token = self.ws_token().ok_or(Expect::CommaOrEnd)?;
            if token == end {
                return Ok(());
            } else if token == Token::Comma {
                token = self.ws_token().ok_or(Expect::Value)?;
            } else {
                return Err(Expect::CommaOrEnd)?;
            }
        }
    }

    /// Parse once using given function and assure that the function has consumed all tokens.
    fn exactly_one<T, E: From<Expect>, F>(&mut self, f: F) -> Result<T, E>
    where
        F: FnOnce(Token, &mut Self) -> Result<T, E>,
    {
        let token = self.ws_token().ok_or(Expect::Value)?;
        let v = f(token, self)?;
        self.eat_whitespace();
        match self.peek_next() {
            None => Ok(v),
            Some(_) => Err(Expect::Eof)?,
        }
    }
}

impl<T> Lex for T where T: crate::Read {}
