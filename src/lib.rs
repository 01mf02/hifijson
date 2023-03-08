//! High-fidelity JSON lexer and parser.
//!
//! # Examples
//!
//! ## Parsing strings to values
//!
//! Let us consider a very simple usage:
//! Parsing a JSON value from a string.
//! For this, we first have to create a lexer from the string,
//! then call the value parser on the lexer:
//!
//! ~~~
//! // our input JSON that we want to parse
//! let json = br#"[null, true, false, "hello", 0, 3.1415, [1, 2], {"x": 1, "y": 2}]"#;
//!
//! // the lexer on our input -- just creating it does not actually run it yet
//! let mut lexer = hifijson::SliceLexer::new(json);
//!
//! use hifijson::token::Lex;
//! // now we are going -- we try to
//! // obtain exactly one JSON value from the lexer and
//! // parse it to a value, allowing for arbitrarily deep (unbounded) nesting
//! let value = lexer.exactly_one(|token, lexer| hifijson::value::parse_unbounded(token, lexer));
//! let value = value.expect("parse");
//!
//! // yay, we got an array!
//! assert!(matches!(value, hifijson::value::Value::Array(_)));
//! assert_eq!(
//!     value.to_string(),
//!     // printing a value yields a compact representation with minimal spaces
//!     r#"[null,true,false,"hello",0,3.1415,[1,2],{"x":1,"y":2}]"#
//! );
//! ~~~
//!
//! ## Parsing files and streams
//!
//! One important feature of hifijson is that it allows to read from both slices and iterators.
//! This is useful when your application should support reading from both files and standard input.
//!
//! The following example reads JSON from a file if an argument is given,
//! otherwise from standard input:
//!
//! ~~~ no_run
//! /// Parse a single JSON value and print it.
//! ///
//! /// Note that the `LexAlloc` trait indicates that this lexer allocates memory.
//! fn process(mut lexer: impl hifijson::LexAlloc) {
//!     let value = lexer.exactly_one(|token, lexer| hifijson::value::parse_unbounded(token, lexer));
//!     let value = value.expect("parse");
//!     println!("{}", value);
//! }
//!
//! let filename = std::env::args().nth(1);
//! if let Some(filename) = filename {
//!     let file = std::fs::read_to_string(filename).expect("read file");
//!     process(hifijson::SliceLexer::new(file.as_bytes()))
//! } else {
//!     use std::io::Read;
//!     process(hifijson::IterLexer::new(std::io::stdin().bytes()))
//! }
//! ~~~
//!
//! ## Operating on the lexer
//!
//! Often, it is better for performance to operate directly on the tokens that the lexer yields.
//! For example, the following example counts the number of values in the input JSON.
//! Furthermore, it does so while requiring only constant memory!
//!
//! ~~~
//! use hifijson::{Token, Error};
//!
//! /// Recursively count the number of values in the value starting with `token`.
//! ///
//! /// The `Lex` trait indicates that this lexer does *not* allocate memory.
//! fn count(token: Token, lexer: &mut impl hifijson::Lex) -> Result<usize, Error> {
//!     match token {
//!         // the JSON values "null", "true", and "false"
//!         Token::Null | Token::True | Token::False => Ok(1),
//!
//!         // the lexer reads only the first character of numbers and strings,
//!         // therefore, we have to consume the rest ourselves
//!         Token::DigitOrMinus => Ok(lexer.num_ignore().map(|_| 1)?),
//!         Token::Quote => Ok(lexer.str_ignore().map(|_| 1)?),
//!
//!         // start of array ('[')
//!         Token::LSquare => {
//!             // an array is a value itself, so start with 1
//!             let mut sum = 1;
//!             // perform the following for every item of the array
//!             lexer.seq(Token::RSquare, |token, lexer| {
//!                 sum += count(token, lexer)?;
//!                 Ok::<_, hifijson::Error>(())
//!             })?;
//!             Ok(sum)
//!         }
//!
//!         // start of object ('{')
//!         Token::LCurly => {
//!             let mut sum = 1;
//!             // perform the following for every key-value pair of the object
//!             lexer.seq(Token::RCurly, |token, lexer| {
//!                 /// read the key, ignoring it, and then the ':' after it
//!                 lexer.str_colon(token, |lexer| lexer.str_ignore().map_err(Error::Str))?;
//!                 /// now read the token after ':'
//!                 let token = lexer.ws_token().ok_or(hifijson::Expect::Value)?;
//!                 sum += count(token, lexer)?;
//!                 Ok::<_, hifijson::Error>(())
//!             })?;
//!             Ok(sum)
//!         }
//!         _ => Err(hifijson::Expect::Value)?,
//!     }
//! }
//!
//! fn process(mut lexer: impl hifijson::Lex) -> Result<usize, hifijson::Error> {
//!     lexer.exactly_one(|token, lexer| count(token, lexer))
//! }
//!
//! let json = br#"[null, true, false, "hello", 0, 3.1415, [1, 2], {"x": 1, "y": 2}]"#;
//! let mut lexer = hifijson::SliceLexer::new(json);
//! let n = process(lexer).unwrap();
//! assert_eq!(n, 13)
//! ~~~

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

macro_rules! impl_from {
    ($from:ty, $to:ty, $proj:expr) => {
        impl From<$from> for $to {
            fn from(x: $from) -> Self {
                $proj(x)
            }
        }
    };
}

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod read;
mod write;

use read::Read;
use write::Write;

pub mod escape;
pub mod num;
pub mod str;
pub mod token;

pub use token::{Expect, Token};

pub mod ignore;
#[cfg(feature = "serde")]
pub mod serde;
#[cfg(feature = "alloc")]
pub mod value;

/// Lexing without any need for memory allocation.
pub trait Lex: token::Lex + num::Lex + str::Lex {}
impl<T> Lex for T where T: token::Lex + num::Lex + str::Lex {}

/// Lexing that does not allocate memory from slices, but from iterators.
pub trait LexWrite: Lex + num::LexWrite + str::LexWrite {}
impl<T> LexWrite for T where T: Lex + num::LexWrite + str::LexWrite {}

/// Lexing that allocates memory both from slices and iterators.
pub trait LexAlloc: LexWrite + str::LexAlloc {}
impl<T> LexAlloc for T where T: LexWrite + str::LexAlloc {}

/// JSON lexer from a shared byte slice.
pub struct SliceLexer<'a> {
    slice: &'a [u8],
}

impl<'a> SliceLexer<'a> {
    /// Create a new slice lexer.
    pub fn new(slice: &'a [u8]) -> Self {
        Self { slice }
    }
}

/// JSON lexer from an iterator over (fallible) bytes.
pub struct IterLexer<E, I> {
    bytes: I,
    last: Option<u8>,
    /// error occurred during reading a byte
    pub error: Option<E>,
}

impl<E, I: Iterator<Item = Result<u8, E>>> IterLexer<E, I> {
    /// Create a new iterator lexer.
    pub fn new(iter: I) -> Self {
        Self {
            bytes: iter,
            last: None,
            error: None,
        }
    }
}

/// Parse error.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// maximal parsing depth has been exceeded
    Depth,
    /// number lexing has failed
    Num(num::Error),
    /// string lexing has failed
    Str(str::Error),
    /// we did not obtain a token that we expected
    Token(token::Expect),
}

impl_from!(num::Error, Error, Error::Num);
impl_from!(str::Error, Error, Error::Str);
impl_from!(token::Expect, Error, Error::Token);

use core::fmt::{self, Display};

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            Depth => "maximal depth exceeded".fmt(f),
            Num(num::Error::ExpectedDigit) => "expected digit".fmt(f),
            Str(e) => e.fmt(f),
            Token(e) => write!(f, "{} expected", e),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
