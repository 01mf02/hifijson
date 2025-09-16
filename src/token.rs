//! Tokens.

/// What we expected to get, but did not get.
#[derive(Debug, PartialEq, Eq)]
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
    /// ASCII letter
    Letter,
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
    /// `-`
    Minus,
    /// `0`--`9`
    Digit,
    /// anything else
    Error,
}

impl core::fmt::Display for Token {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        self.as_str().fmt(f)
    }
}

impl Token {
    fn as_str(&self) -> &'static str {
        use Token::*;
        match self {
            Comma => ",",
            Colon => ":",
            LSquare => "[",
            RSquare => "]",
            LCurly => "{",
            RCurly => "}",
            Quote => "\"",
            Minus => "-",
            Letter => "letter",
            Digit => "digit",
            Error => "unknown token",
        }
    }

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
        self.skip_until(|c| !matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
    }

    /// Skip potential whitespace and return the following token if there is some.
    fn ws_token(&mut self) -> Option<Token> {
        self.eat_whitespace();
        self.peek_next().map(|next| self.token(next))
    }

    /// Parse a JSON token starting with a letter.
    fn null_or_bool(&mut self) -> Option<Option<bool>> {
        // we are calling this function without having advanced before
        Some(match self.take_next() {
            Some(b'n') if self.strip_prefix(b"ull") => None,
            Some(b't') if self.strip_prefix(b"rue") => Some(true),
            Some(b'f') if self.strip_prefix(b"alse") => Some(false),
            _ => return None,
        })
    }

    /// Convert a character to a token, such as '`:`' to [`Token::Colon`].
    ///
    /// Only when the token consists of a single character that can be
    /// reconstructed losslessly from the token, such as `[` or `:`,
    /// then the current character is consumed.
    /// For all other tokens, like [`Token::Letter`], the character is preserved.
    fn token(&mut self, c: u8) -> Token {
        let token = match c {
            // it is important to `return` here in order not to take the next byte,
            // like we do for the regular, single-character tokens
            b'a'..=b'z' | b'A'..=b'Z' => return Token::Letter,
            b'0'..=b'9' => return Token::Digit,
            b'-' => Token::Minus,
            b'"' => Token::Quote,
            b'[' => Token::LSquare,
            b']' => Token::RSquare,
            b'{' => Token::LCurly,
            b'}' => Token::RCurly,
            b',' => Token::Comma,
            b':' => Token::Colon,
            _ => return Token::Error,
        };
        self.take_next();
        token
    }

    /// Parse a string with given function, followed by a colon.
    fn str_colon<T, E: From<Expect>, TF, F>(&mut self, token: Token, tf: TF, f: F) -> Result<T, E>
    where
        TF: FnOnce(&mut Self) -> Option<Token>,
        F: FnOnce(&mut Self) -> Result<T, E>,
    {
        token.equals_or(Token::Quote, Expect::String)?;
        let key = f(self)?;

        let colon = tf(self).filter(|t| *t == Token::Colon);
        colon.ok_or(Expect::Colon)?;

        Ok(key)
    }

    /// Execute `f` for every item in the comma-separated sequence until `end`.
    fn seq<E: From<Expect>, TF, F>(&mut self, end: Token, mut tf: TF, mut f: F) -> Result<(), E>
    where
        TF: FnMut(&mut Self) -> Option<Token>,
        F: FnMut(Token, &mut Self) -> Result<(), E>,
    {
        let mut token = tf(self).ok_or(Expect::ValueOrEnd)?;
        if token == end {
            return Ok(());
        };

        loop {
            f(token, self)?;
            token = tf(self).ok_or(Expect::CommaOrEnd)?;
            if token == end {
                return Ok(());
            } else if token == Token::Comma {
                token = tf(self).ok_or(Expect::Value)?;
            } else {
                return Err(Expect::CommaOrEnd)?;
            }
        }
    }

    /// Parse once using given function and assure that the function has consumed all tokens.
    fn exactly_one<T, E: From<Expect>, TF, F>(&mut self, mut tf: TF, f: F) -> Result<T, E>
    where
        TF: FnMut(&mut Self) -> Option<Token>,
        F: FnOnce(Token, &mut Self) -> Result<T, E>,
    {
        let token = tf(self).ok_or(Expect::Value)?;
        let v = f(token, self)?;
        self.eat_whitespace();
        match self.peek_next() {
            None => Ok(v),
            Some(_) => Err(Expect::Eof)?,
        }
    }
}

impl<T> Lex for T where T: crate::Read {}
