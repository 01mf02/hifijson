use crate::{token, Error, LexAlloc, SliceLexer, Token};

use serde::de::{self, DeserializeSeed, Visitor};
use serde::Deserialize;

type Result<T> = core::result::Result<T, Error>;

impl de::Error for Error {
    fn custom<T: core::fmt::Display>(_: T) -> Self {
        // TODO: change this to a more descriptive type
        Error::ExpectedValue
    }
}

struct TokenLexer<L> {
    token: Token,
    lexer: L,
}

impl<'de, 'a, L: LexAlloc + 'de> de::Deserializer<'de> for TokenLexer<&'a mut L> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        use alloc::string::ToString;
        match self.token {
            Token::Null => visitor.visit_unit(),
            Token::True => visitor.visit_bool(true),
            Token::False => visitor.visit_bool(false),
            Token::Quote => visitor.visit_string(self.lexer.str_string()?.to_string()),
            Token::DigitOrMinus => {
                let (n, parts) = self.lexer.num_string()?;
                // TODO: eliminate unwrap
                if parts.dot.is_none() && parts.exp.is_none() {
                    if n.starts_with('-') {
                        visitor.visit_i64(n.parse().unwrap())
                    } else {
                        visitor.visit_u64(n.parse().unwrap())
                    }
                } else {
                    visitor.visit_f64(n.parse().unwrap())
                }
            }
            Token::LSquare => visitor.visit_seq(CommaSeparated::new(self.lexer)),
            Token::LCurly => visitor.visit_map(CommaSeparated::new(self.lexer)),
            token => Err(Error::Token(token)),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
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
                *token = self.lexer.ws_token().ok_or(Error::ExpectedValue)?;
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

        let lexer = &mut *self.lexer;
        seed.deserialize(TokenLexer { token, lexer }).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        let lexer = &mut *self.lexer;
        let colon = lexer.ws_token().filter(|t| *t == Token::Colon);
        colon.ok_or(Error::Seq(token::Error::ExpectedColon))?;

        let token = lexer.ws_token().ok_or(Error::ExpectedValue)?;
        seed.deserialize(TokenLexer { token, lexer })
    }
}

pub fn from_slice<'a, T: Deserialize<'a>>(s: &'a [u8]) -> Result<T> {
    use crate::token::Lex;
    SliceLexer::new(s).exactly_one(|token, lexer| T::deserialize(TokenLexer { token, lexer }))
}
