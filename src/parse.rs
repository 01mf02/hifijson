use crate::{error, LexerStr, NumParts, Token};
use alloc::vec::Vec;
use core::fmt;
use core::ops::Deref;

/// Parse error.
#[derive(Debug)]
pub enum Error {
    ExpectedValue,
    ExpectedString,
    ExpectedColon,
    ExpectedEof,
    Num(error::Num),
    Str(error::Str),
    Seq(error::Seq),
    Token(Token),
}

impl From<error::Num> for Error {
    fn from(e: error::Num) -> Self {
        Error::Num(e)
    }
}

impl From<error::Str> for Error {
    fn from(e: error::Str) -> Self {
        Error::Str(e)
    }
}

impl From<error::Seq> for Error {
    fn from(e: error::Seq) -> Self {
        Error::Seq(e)
    }
}

/// JSON value.
#[derive(Debug)]
pub enum Value<Num, Str> {
    /// `null`
    Null,
    /// `true` or `false`
    Bool(bool),
    /// string representation of a number with positional information
    Number((Num, NumParts)),
    /// string
    String(Str),
    /// array
    Array(Vec<Self>),
    /// mapping from strings to values
    Object(Vec<(Str, Self)>),
}

/// Wrapper type to facilitate string printing.
struct JsonString<Str>(Str);

impl<Str: Deref<Target = str>> fmt::Display for JsonString<Str> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        '"'.fmt(f)?;
        for c in self.0.chars() {
            match c {
                '\\' | '"' | '\n' | '\r' | '\t' => c.escape_default().try_for_each(|c| c.fmt(f)),
                c if (c as u32) < 20 => write!(f, "\\u{:04x}", c as u16),
                c => c.fmt(f),
            }?
        }
        '"'.fmt(f)
    }
}

impl<Num: Deref<Target = str>, Str: Deref<Target = str>> fmt::Display for Value<Num, Str> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Value::*;
        match self {
            Null => "null".fmt(f),
            Bool(b) => b.fmt(f),
            Number((n, _)) => n.fmt(f),
            String(s) => JsonString(&**s).fmt(f),
            Array(a) => {
                "[".fmt(f)?;
                let mut iter = a.iter();
                iter.next().iter().try_for_each(|v| write!(f, "{v}"))?;
                iter.try_for_each(|v| write!(f, ",{v}"))?;
                "]".fmt(f)
            }
            Object(o) => {
                "{".fmt(f)?;
                let mut iter = o.iter().map(|(k, v)| (JsonString(&**k), v));
                iter.next()
                    .iter()
                    .try_for_each(|(k, v)| write!(f, "{k}:{v}"))?;
                iter.try_for_each(|(k, v)| write!(f, ",{k}:{v}"))?;
                "}".fmt(f)
            }
        }
    }
}

pub fn parse<L: LexerStr>(lexer: &mut L, token: Token) -> Result<Value<L::Num, L::Str>, Error> {
    match token {
        Token::Null => Ok(Value::Null),
        Token::True => Ok(Value::Bool(true)),
        Token::False => Ok(Value::Bool(false)),
        Token::DigitOrMinus => Ok(Value::Number(lexer.parse_number()?)),
        Token::Quote => Ok(Value::String(lexer.parse_string()?)),
        Token::LSquare => Ok(Value::Array({
            let mut arr = Vec::new();
            lexer.seq(Token::RSquare, |lexer, token| {
                arr.push(parse(lexer, token)?);
                Ok::<_, Error>(())
            })?;
            arr
        })),
        Token::LCurly => Ok(Value::Object({
            let mut obj = Vec::new();
            lexer.seq(Token::RCurly, |lexer, token| {
                let key = match token {
                    Token::Quote => lexer.parse_string()?,
                    _ => return Err(Error::ExpectedString),
                };
                if lexer.ws_token() != Some(Token::Colon) {
                    return Err(Error::ExpectedColon);
                }
                let value = match lexer.ws_token() {
                    Some(token) => parse(lexer, token)?,
                    _ => return Err(Error::ExpectedValue),
                };
                obj.push((key, value));
                Ok(())
            })?;
            obj
        })),
        token => Err(Error::Token(token)),
    }
}

pub fn parse_single<L: LexerStr>(lexer: &mut L) -> Result<Value<L::Num, L::Str>, Error> {
    let token = lexer.ws_token().ok_or(Error::ExpectedValue)?;
    let v = parse(lexer, token)?;
    lexer.eat_whitespace();
    match lexer.peek_byte() {
        None => Ok(v),
        Some(_) => Err(Error::ExpectedEof),
    }
}

pub fn parse_many<L: LexerStr>(
    lexer: &mut L,
) -> impl Iterator<Item = Result<Value<L::Num, L::Str>, Error>> + '_ {
    core::iter::from_fn(|| {
        let token = lexer.ws_token()?;
        Some(parse(lexer, token))
    })
}
