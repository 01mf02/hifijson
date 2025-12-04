//! Deserialisation via serde.
//!
//! Example usage:
//!
//! ~~~
//! let input = b"[0, 1]";
//! let mut lexer = hifijson::SliceLexer::new(input);
//! let value: Vec<_> = hifijson::serde::exactly_one(&mut lexer).unwrap();
//! assert_eq!(vec![0, 1], value);
//! ~~~

use crate::{Expect, Lex, LexAlloc};

use alloc::string::{String, ToString};
use core::fmt;
use serde::de::{self, Deserialize, DeserializeSeed, Visitor};

/// Deserialisation error.
#[derive(Debug)]
pub enum Error {
    /// parse error
    Parse(crate::Error),
    /// error produced by serde
    Custom(String),
    /// `2e1000` (we were not able to fit a number into its type)
    Number(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            Parse(e) => e.fmt(f),
            Custom(e) => e.fmt(f),
            Number(n) => write!(f, "number overflow: {}", n),
        }
    }
}

impl_from!(crate::Error, Error, Error::Parse);
impl_from!(Expect, Error, |e| Error::Parse(crate::Error::Token(e)));
impl_error!(Error);

type Result<T> = core::result::Result<T, Error>;

impl de::Error for Error {
    fn custom<T: core::fmt::Display>(e: T) -> Self {
        Self::Custom(e.to_string())
    }
}

struct TokenLexer<L> {
    next: u8,
    lexer: L,
}

fn parse_number<T: core::str::FromStr>(n: &str) -> Result<T> {
    n.parse().map_err(|_| Error::Number(n.to_string()))
}

macro_rules! deserialize_number {
    ($deserialize:ident, $visit:ident) => {
        fn $deserialize<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            let (prefix, lexer) = match self.next {
                b'-' => ("-", self.lexer.discarded()),
                _ => ("", self.lexer),
            };
            let (n, _parts) = lexer.num_string(prefix).map_err(crate::Error::Num)?;
            visitor.$visit(parse_number(&n)?)
        }
    };
}

impl<'de, 'a, L: LexAlloc + 'de> de::Deserializer<'de> for TokenLexer<&'a mut L> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let num = |lexer: &mut L, visitor: V, prefix: &'static str| {
            let (n, parts) = lexer.num_string(prefix).map_err(Num)?;
            match (parts.is_int(), prefix) {
                (true, "-") => visitor.visit_i64(parse_number(&n)?),
                (true, _) => visitor.visit_u64(parse_number(&n)?),
                (false, _) => visitor.visit_f64(parse_number(&n)?),
            }
        };

        use crate::Error::{Num, Str};
        match self.next {
            b'a'..=b'z' => match self.lexer.null_or_bool().ok_or(Expect::Value)? {
                None => visitor.visit_unit(),
                Some(b) => visitor.visit_bool(b),
            },
            b'0'..=b'9' => num(self.lexer, visitor, ""),
            b'-' => num(self.lexer.discarded(), visitor, "-"),
            b'"' => visitor.visit_str(&self.lexer.discarded().str_string().map_err(Str)?),
            b'[' => visitor.visit_seq(CommaSeparated::new(self.lexer.discarded())),
            b'{' => visitor.visit_map(CommaSeparated::new(self.lexer.discarded())),
            _ => Err(Expect::Value)?,
        }
    }

    serde::forward_to_deserialize_any! {
        bool char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }

    deserialize_number!(deserialize_u8, visit_u8);
    deserialize_number!(deserialize_u16, visit_u16);
    deserialize_number!(deserialize_u32, visit_u32);
    deserialize_number!(deserialize_u64, visit_u64);
    deserialize_number!(deserialize_u128, visit_u128);

    deserialize_number!(deserialize_i8, visit_i8);
    deserialize_number!(deserialize_i16, visit_i16);
    deserialize_number!(deserialize_i32, visit_i32);
    deserialize_number!(deserialize_i64, visit_i64);
    deserialize_number!(deserialize_i128, visit_i128);

    deserialize_number!(deserialize_f32, visit_f32);
    deserialize_number!(deserialize_f64, visit_f64);
}

struct CommaSeparated<'a, L> {
    lexer: &'a mut L,
    first: bool,
}

impl<'a, L> CommaSeparated<'a, L> {
    fn new(lexer: &'a mut L) -> Self {
        CommaSeparated { lexer, first: true }
    }
}

impl<'a, L: Lex> CommaSeparated<'a, L> {
    // Comma is required before every element except the first.
    fn comma(&mut self, next: &mut u8) -> Result<()> {
        if !core::mem::take(&mut self.first) {
            self.lexer
                .expect(|_| Some(*next), b',')
                .ok_or(Expect::CommaOrEnd)?;
            *next = self.lexer.ws_peek().ok_or(Expect::Value)?
        }
        Ok(())
    }
}

impl<'de, 'a, L: LexAlloc + 'de> de::SeqAccess<'de> for CommaSeparated<'a, L> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        let mut next = self.lexer.ws_peek().ok_or(Expect::ValueOrEnd)?;
        if next == b']' {
            self.lexer.take_next();
            return Ok(None);
        };
        self.comma(&mut next)?;

        let lexer = &mut *self.lexer;
        seed.deserialize(TokenLexer { next, lexer }).map(Some)
    }
}

impl<'de, 'a, L: LexAlloc + 'de> de::MapAccess<'de> for CommaSeparated<'a, L> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        let mut next = self.lexer.ws_peek().ok_or(Expect::ValueOrEnd)?;
        if next == b'}' {
            self.lexer.take_next();
            return Ok(None);
        };
        self.comma(&mut next)?;

        if next != b'"' {
            Err(Expect::String)?
        }

        let lexer = &mut *self.lexer;
        seed.deserialize(TokenLexer { next, lexer }).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        let lexer = &mut *self.lexer;
        lexer.expect(L::ws_peek, b':').ok_or(Expect::Colon)?;
        let next = lexer.ws_peek().ok_or(Expect::Value)?;
        seed.deserialize(TokenLexer { next, lexer })
    }
}

/// Deserialise a single value.
pub fn exactly_one<'a, T: Deserialize<'a>, L: LexAlloc + 'a>(lexer: &mut L) -> Result<T> {
    lexer.exactly_one(L::ws_peek, |next, lexer| {
        T::deserialize(TokenLexer { next, lexer })
    })
}
