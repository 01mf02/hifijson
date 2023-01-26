use crate::{token, LexAlloc, SliceLexer, Token};

use alloc::string::{String, ToString};
use core::fmt;
use serde::de::{self, DeserializeSeed, Visitor};
use serde::Deserialize;

#[derive(Debug)]
pub enum Error {
    Parse(crate::Error),
    Custom(String),
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

impl From<crate::Error> for Error {
    fn from(e: crate::Error) -> Self {
        Self::Parse(e)
    }
}

impl From<token::Error> for Error {
    fn from(e: token::Error) -> Self {
        Self::Parse(crate::Error::Seq(e))
    }
}

impl std::error::Error for Error {}

type Result<T> = core::result::Result<T, Error>;

impl de::Error for Error {
    fn custom<T: core::fmt::Display>(e: T) -> Self {
        Self::Custom(e.to_string())
    }
}

struct TokenLexer<L> {
    token: Token,
    lexer: L,
}

fn parse_number<T: core::str::FromStr>(n: &str) -> Result<T> {
    n.parse().map_err(|_| Error::Number(n.to_string()))
}

macro_rules! deserialize_number {
    ($deserialize:ident, $visit:ident) => {
        fn $deserialize<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            let (n, _parts) = self.lexer.num_string().map_err(crate::Error::Num)?;
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
        use crate::Error::{Num, Str};
        match self.token {
            Token::Null => visitor.visit_unit(),
            Token::True => visitor.visit_bool(true),
            Token::False => visitor.visit_bool(false),
            Token::Quote => visitor.visit_str(&self.lexer.str_string().map_err(Str)?),
            Token::DigitOrMinus => {
                let (n, parts) = self.lexer.num_string().map_err(Num)?;
                if parts.dot.is_none() && parts.exp.is_none() {
                    if n.starts_with('-') {
                        visitor.visit_i64(parse_number(&n)?)
                    } else {
                        visitor.visit_u64(parse_number(&n)?)
                    }
                } else {
                    visitor.visit_f64(parse_number(&n)?)
                }
            }
            Token::LSquare => visitor.visit_seq(CommaSeparated::new(self.lexer)),
            Token::LCurly => visitor.visit_map(CommaSeparated::new(self.lexer)),
            token => Err(crate::Error::Token(token))?,
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

impl<'a, L: LexAlloc> CommaSeparated<'a, L> {
    // Comma is required before every element except the first.
    fn comma(&mut self, token: &mut Token) -> Result<()> {
        if !core::mem::take(&mut self.first) {
            if *token != Token::Comma {
                Err(token::Error::ExpectedCommaOrEnd)?
            } else {
                *token = self.lexer.ws_token().ok_or(crate::Error::ExpectedValue)?;
            }
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
        let token = self.lexer.ws_token();
        let mut token = token.ok_or(token::Error::ExpectedItemOrEnd)?;
        if token == Token::RSquare {
            return Ok(None);
        };
        self.comma(&mut token)?;

        let lexer = &mut *self.lexer;
        seed.deserialize(TokenLexer { token, lexer }).map(Some)
    }
}

impl<'de, 'a, L: LexAlloc + 'de> de::MapAccess<'de> for CommaSeparated<'a, L> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        let token = self.lexer.ws_token();
        let mut token = token.ok_or(token::Error::ExpectedItemOrEnd)?;
        if token == Token::RCurly {
            return Ok(None);
        };
        self.comma(&mut token)?;

        if token != Token::Quote {
            Err(token::Error::ExpectedString)?
        }

        let lexer = &mut *self.lexer;
        seed.deserialize(TokenLexer { token, lexer }).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        let lexer = &mut *self.lexer;
        let colon = lexer.ws_token().filter(|t| *t == Token::Colon);
        colon.ok_or(token::Error::ExpectedColon)?;

        let token = lexer.ws_token().ok_or(crate::Error::ExpectedValue)?;
        seed.deserialize(TokenLexer { token, lexer })
    }
}

pub fn from_slice<'a, T: Deserialize<'a>>(s: &'a [u8]) -> Result<T> {
    use crate::token::Lex;
    SliceLexer::new(s).exactly_one(|token, lexer| T::deserialize(TokenLexer { token, lexer }))
}
