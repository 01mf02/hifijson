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

impl<NumL: PartialEq<NumR>, NumR, StrL: PartialEq<StrR>, StrR> PartialEq<Value<NumR, StrR>>
    for Value<NumL, StrL>
{
    fn eq(&self, other: &Value<NumR, StrR>) -> bool {
        use Value::*;
        match (self, other) {
            (Null, Null) => true,
            (Bool(l), Bool(r)) => l == r,
            (Number((nl, pl)), Number((nr, pr))) => nl == nr && pl == pr,
            (String(l), String(r)) => l == r,
            (Array(l), Array(r)) => l == r,
            (Object(l), Object(r)) => {
                let mut lr = l.iter().zip(r.iter());
                l.len() == r.len() && lr.all(|((kl, vl), (kr, vr))| kl == kr && vl == vr)
            }
            _ => false,
        }
    }
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
                iter.next().iter().try_for_each(|v| write!(f, "{}", v))?;
                iter.try_for_each(|v| write!(f, ",{}", v))?;
                "]".fmt(f)
            }
            Object(o) => {
                "{".fmt(f)?;
                let mut iter = o.iter().map(|(k, v)| (JsonString(&**k), v));
                iter.next()
                    .iter()
                    .try_for_each(|(k, v)| write!(f, "{}:{}", k, v))?;
                iter.try_for_each(|(k, v)| write!(f, ",{}:{}", k, v))?;
                "}".fmt(f)
            }
        }
    }
}

pub fn from_token<L: LexAlloc>(
    token: Token,
    lexer: &mut L,
) -> Result<Value<L::Num, L::Str>, Error> {
    match token {
        Token::Null => Ok(Value::Null),
        Token::True => Ok(Value::Bool(true)),
        Token::False => Ok(Value::Bool(false)),
        Token::DigitOrMinus => Ok(Value::Number(lexer.num_string()?)),
        Token::Quote => Ok(Value::String(lexer.str_string()?)),
        Token::LSquare => Ok(Value::Array({
            let mut arr = Vec::new();
            lexer.seq(Token::RSquare, |token, lexer| {
                arr.push(from_token(token, lexer)?);
                Ok::<_, Error>(())
            })?;
            arr
        })),
        Token::LCurly => Ok(Value::Object({
            let mut obj = Vec::new();
            lexer.seq(Token::RCurly, |token, lexer| {
                let key = lexer.str_colon(token, |lexer| lexer.str_string().map_err(Error::Str))?;

                let token = lexer.ws_token().ok_or(Error::ExpectedValue)?;
                let value = from_token(token, lexer)?;
                obj.push((key, value));
                Ok::<_, Error>(())
            })?;
            obj
        })),
        token => Err(Error::Token(token)),
    }
}

pub fn parse_depth<L: LexAlloc>(
    depth: usize,
    token: Token,
    lexer: &mut L,
) -> Result<Value<L::Num, L::Str>, Error> {
    if depth == 0 {
        return Err(Error::Depth);
    };

    match token {
        Token::LSquare => Ok(Value::Array({
            let mut arr = Vec::new();
            lexer.seq(Token::RSquare, |token, lexer| {
                arr.push(parse_depth(depth - 1, token, lexer)?);
                Ok::<_, Error>(())
            })?;
            arr
        })),
        Token::LCurly => Ok(Value::Object({
            let mut obj = Vec::new();
            lexer.seq(Token::RCurly, |token, lexer| {
                let key = lexer.str_colon(token, |lexer| lexer.str_string().map_err(Error::Str))?;

                let token = lexer.ws_token().ok_or(Error::ExpectedValue)?;
                obj.push((key, parse_depth(depth - 1, token, lexer)?));
                Ok::<_, Error>(())
            })?;
            obj
        })),
        token => from_token(token, lexer),
    }
}

pub fn many<L: LexAlloc>(
    lexer: &mut L,
) -> impl Iterator<Item = Result<Value<L::Num, L::Str>, Error>> + '_ {
    core::iter::from_fn(|| Some(from_token(lexer.ws_token()?, lexer)))
}
