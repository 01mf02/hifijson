//! Parsing and values.

use crate::token::Expect;
use crate::{num, str, Error, LexAlloc};
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

impl<Num: Deref<Target = str>, Str: Deref<Target = str>> fmt::Display for Value<Num, Str> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Value::*;
        match self {
            Null => "null".fmt(f),
            Bool(b) => b.fmt(f),
            Number((n, _)) => n.fmt(f),
            String(s) => str::Display::new(&**s).fmt(f),
            Array(a) => {
                "[".fmt(f)?;
                let mut iter = a.iter();
                iter.next().iter().try_for_each(|v| write!(f, "{}", v))?;
                iter.try_for_each(|v| write!(f, ",{}", v))?;
                "]".fmt(f)
            }
            Object(o) => {
                "{".fmt(f)?;
                let mut iter = o.iter().map(|(k, v)| (str::Display::new(&**k), v));
                iter.next()
                    .iter()
                    .try_for_each(|(k, v)| write!(f, "{}:{}", k, v))?;
                iter.try_for_each(|(k, v)| write!(f, ",{}:{}", k, v))?;
                "}".fmt(f)
            }
        }
    }
}

/// Parse a value, using `f` to parse recursive values inside arrays / objects.
fn parse<L: LexAlloc>(
    next: u8,
    lexer: &mut L,
    f: impl Fn(u8, &mut L) -> Result<Value<L::Num, L::Str>, Error>,
) -> Result<Value<L::Num, L::Str>, Error> {
    let nob = |o: Option<bool>| o.map(Value::Bool).unwrap_or(Value::Null);
    match next {
        b'a'..=b'z' => Ok(lexer.null_or_bool().map(nob).ok_or(Expect::Value)?),
        b'-' => Ok(Value::Number(lexer.discarded().num_string("-")?)),
        b'0'..=b'9' => Ok(Value::Number(lexer.num_string("")?)),
        b'"' => Ok(Value::String(lexer.discarded().str_string()?)),
        b'[' => Ok(Value::Array({
            let mut arr = Vec::new();
            lexer.discarded().seq(b']', L::ws_peek, |next, lexer| {
                arr.push(f(next, lexer)?);
                Ok::<_, Error>(())
            })?;
            arr
        })),
        b'{' => Ok(Value::Object({
            let mut obj = Vec::new();
            lexer.discarded().seq(b'}', L::ws_peek, |next, lexer| {
                lexer.expect(|_| Some(next), b'"').ok_or(Expect::String)?;
                let key = lexer.str_string().map_err(Error::Str)?;
                lexer.expect(L::ws_peek, b':').ok_or(Expect::Colon)?;
                let value = f(lexer.ws_peek().ok_or(Expect::Value)?, lexer)?;
                obj.push((key, value));
                Ok::<_, Error>(())
            })?;
            obj
        })),
        _ => Err(Expect::Value)?,
    }
}

/// Parse a value, not limiting the recursion depth.
///
/// To prevent stack overflows, consider using [`parse_bounded`].
pub fn parse_unbounded<L: LexAlloc>(
    next: u8,
    lexer: &mut L,
) -> Result<Value<L::Num, L::Str>, Error> {
    parse(next, lexer, parse_unbounded)
}

/// Parse an value, limiting the recursion to `depth`.
///
/// This serves to prevent stack overflows.
pub fn parse_bounded<L: LexAlloc>(
    depth: usize,
    next: u8,
    lexer: &mut L,
) -> Result<Value<L::Num, L::Str>, Error> {
    let d = depth.checked_sub(1).ok_or(Error::Depth)?;
    parse(next, lexer, |next, lexer| parse_bounded(d, next, lexer))
}
