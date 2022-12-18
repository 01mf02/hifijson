//! Parsing and values.

use crate::{num, str, Error, LexAlloc, Token};
use alloc::vec::Vec;
use core::fmt;
use core::ops::Deref;

/// JSON value.
#[derive(Debug)]
pub enum Value<Num, Str> {
    /// `null`
    Null,
    /// `true` or `false`
    Bool(bool),
    /// string representation of a number with positional information
    Number((Num, num::Parts)),
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

pub fn from_token<L: LexAlloc>(
    lexer: &mut L,
    token: Token,
) -> Result<Value<L::Num, L::Str>, Error> {
    match token {
        Token::Null => Ok(Value::Null),
        Token::True => Ok(Value::Bool(true)),
        Token::False => Ok(Value::Bool(false)),
        Token::DigitOrMinus => Ok(Value::Number(lexer.num_string()?)),
        Token::Quote => Ok(Value::String(lexer.str_string()?)),
        Token::LSquare => Ok(Value::Array({
            let mut arr = Vec::new();
            lexer.seq(Token::RSquare, |lexer, token| {
                arr.push(from_token(lexer, token)?);
                Ok::<_, Error>(())
            })?;
            arr
        })),
        Token::LCurly => Ok(Value::Object({
            let mut obj = Vec::new();
            lexer.seq(Token::RCurly, |lexer, token| {
                token.equals_or(Token::Quote, Error::ExpectedString)?;
                let key = lexer.str_string()?;

                let colon = lexer.ws_token().filter(|t| *t == Token::Colon);
                colon.ok_or(Error::ExpectedColon)?;

                let token = lexer.ws_token().ok_or(Error::ExpectedValue)?;
                let value = from_token(lexer, token)?;
                obj.push((key, value));
                Ok::<_, Error>(())
            })?;
            obj
        })),
        token => Err(Error::Token(token)),
    }
}

pub fn exactly_one<L: LexAlloc>(lexer: &mut L) -> Result<Value<L::Num, L::Str>, Error> {
    let token = lexer.ws_token().ok_or(Error::ExpectedValue)?;
    let v = from_token(lexer, token)?;
    lexer.eat_whitespace();
    match lexer.peek_next() {
        None => Ok(v),
        Some(_) => Err(Error::ExpectedEof),
    }
}

pub fn many<L: LexAlloc>(
    lexer: &mut L,
) -> impl Iterator<Item = Result<Value<L::Num, L::Str>, Error>> + '_ {
    core::iter::from_fn(|| {
        let token = lexer.ws_token()?;
        Some(from_token(lexer, token))
    })
}
