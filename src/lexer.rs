use crate::{error, escape, Escape, NumParts};
use core::ops::Deref;

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

impl Default for Token {
    fn default() -> Self {
        Token::Error
    }
}

pub trait Lexer {
    type Bytes: Deref<Target = [u8]> + Default;
    type Num: Deref<Target = str>;

    /// Read to bytes until `stop` yields true.
    fn read_until(&mut self, bytes: &mut Self::Bytes, stop: impl FnMut(u8) -> bool);

    /// Look at the next byte.
    fn peek_byte(&self) -> Option<&u8>;
    /// Consume the next byte.
    fn read_byte(&mut self) -> Option<u8>;

    /// Return the earliest non-whitespace character.
    fn eat_whitespace(&mut self);

    /// Read (optional) whitespace and return the following token if there is some.
    fn ws_token(&mut self) -> Option<Token> {
        self.eat_whitespace();
        Some(self.token(*self.peek_byte()?))
    }

    /// Return `out` if the given byte sequence is read, otherwise default.
    fn lex_exact<const N: usize, T: Default>(&mut self, s: [u8; N], out: T) -> T;

    /// Convert a character to a token, such as '`:`' to `Token::Colon`.
    ///
    /// When the token consists of several characters, such as
    /// `null`, `true`, or `false`,
    /// also consume the following characters.
    fn token(&mut self, c: u8) -> Token {
        let token = match c {
            // it is important to `return` here in order not to read a byte,
            // like we do for the regular, single-character tokens
            b'n' => return self.lex_exact([b'u', b'l', b'l'], Token::Null),
            b't' => return self.lex_exact([b'r', b'u', b'e'], Token::True),
            b'f' => return self.lex_exact([b'a', b'l', b's', b'e'], Token::False),
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
        self.read_byte();
        token
    }

    fn num_bytes(&mut self, bytes: &mut Self::Bytes) -> Result<NumParts, error::Num>;
    fn num_string(&mut self) -> Result<(Self::Num, NumParts), error::Num>;

    /// Read an escape sequence such as "\n" or "\u0009" (without leading '\').
    fn escape(&mut self) -> Result<Escape, error::Escape>;

    /// Convert a read escape sequence to a char, potentially reading more.
    fn escape_char(&mut self, escape: Escape) -> Result<char, error::Escape> {
        let escape = match escape {
            Escape::Unicode(high @ (0xD800..=0xDBFF)) => {
                if self.read_byte() != Some(b'\\') {
                    return Err(error::Escape::ExpectedLowSurrogate);
                }
                if let Escape::Unicode(low @ (0xDC00..=0xDFFF)) = self.escape()? {
                    ((high - 0xD800) * 0x400 + (low - 0xDC00)) as u32 + 0x10000
                } else {
                    return Err(error::Escape::ExpectedLowSurrogate);
                }
            }
            e => e.as_u16() as u32,
        };
        char::from_u32(escape).ok_or(error::Escape::InvalidChar(escape))
    }

    /// Read a string to bytes, copying escape sequences one-to-one.
    fn str_bytes(&mut self, bytes: &mut Self::Bytes) -> Result<(), error::Str> {
        let mut escaped = false;
        let mut unicode = false;
        let mut hex_pos = 0;
        let mut error = None;

        self.read_until(bytes, |c| {
            if escaped {
                if unicode {
                    if escape::decode_hex(c).is_none() {
                        error = Some(error::Str::Escape(error::Escape::InvalidHex));
                    } else if hex_pos < 3 {
                        hex_pos += 1
                    } else {
                        escaped = false;
                        unicode = false;
                        hex_pos = 0;
                    }
                } else {
                    match Escape::try_from(c) {
                        Some(Escape::Unicode(_)) => unicode = true,
                        Some(_) => escaped = false,
                        None => error = Some(error::Str::Escape(error::Escape::UnknownKind)),
                    }
                }
            } else {
                match c {
                    b'"' => return true,
                    b'\\' => escaped = true,
                    0..=19 => error = Some(error::Str::Control),
                    _ => (),
                };
            }
            error.is_some()
        });
        match error {
            Some(e) => Err(e),
            None if escaped || self.read_byte() != Some(b'"') => Err(error::Str::Eof),
            _ => Ok(()),
        }
    }

    /// Lex a string by executing `on_string` on every string and `on_bytes` on every escape sequence.
    fn str_fold<E: From<error::Str>, T>(
        &mut self,
        mut out: T,
        on_string: impl Fn(&mut Self::Bytes, &mut T) -> Result<(), E>,
        on_escape: impl Fn(&mut Self, Escape, &mut T) -> Result<(), E>,
    ) -> Result<T, E> {
        fn string_end(c: u8) -> bool {
            matches!(c, b'"' | b'\\' | 0..=19)
        }

        let mut bytes = Self::Bytes::default();
        self.read_until(&mut bytes, string_end);
        on_string(&mut bytes, &mut out)?;
        match self.read_byte().ok_or(error::Str::Eof)? {
            b'\\' => (),
            b'"' => return Ok(out),
            _ => return Err(error::Str::Control)?,
        }
        loop {
            let escape = self.escape().map_err(error::Str::Escape)?;
            on_escape(self, escape, &mut out)?;
            self.read_until(&mut bytes, string_end);
            on_string(&mut bytes, &mut out)?;
            match self.read_byte().ok_or(error::Str::Eof)? {
                b'\\' => continue,
                b'"' => return Ok(out),
                _ => return Err(error::Str::Control)?,
            }
        }
    }

    fn seq<E: From<error::Seq>, F>(&mut self, until: Token, mut f: F) -> Result<(), E>
    where
        F: FnMut(&mut Self, Token) -> Result<(), E>,
    {
        let mut token = self.ws_token().ok_or(error::Seq::ExpectedItemOrEnd)?;
        if token == until {
            return Ok(());
        };

        loop {
            f(self, token)?;
            token = self.ws_token().ok_or(error::Seq::ExpectedCommaOrEnd)?;
            if token == until {
                return Ok(());
            } else if token == Token::Comma {
                token = self.ws_token().ok_or(error::Seq::ExpectedItem)?;
            } else {
                return Err(error::Seq::ExpectedCommaOrEnd)?;
            }
        }
    }
}
