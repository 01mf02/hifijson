//! High-fidelity JSON lexer and parser.

#![no_std]
#![forbid(unsafe_code)]

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

pub use token::Token;

pub mod validate;
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
    pub fn new(slice: &'a [u8]) -> Self {
        Self { slice }
    }
}

/// JSON lexer from an iterator over (fallible) bytes.
pub struct IterLexer<E, I> {
    bytes: I,
    last: Option<u8>,
    pub error: Option<E>,
}

impl<E, I: Iterator<Item = Result<u8, E>>> IterLexer<E, I> {
    pub fn new(iter: I) -> Self {
        Self {
            bytes: iter,
            last: None,
            error: None,
        }
    }
}

/// Parse error.
#[derive(Debug)]
pub enum Error {
    ExpectedValue,
    Depth,
    Num(num::Error),
    Str(str::Error),
    Seq(token::Error),
    Token(Token),
}

impl From<num::Error> for Error {
    fn from(e: num::Error) -> Self {
        Error::Num(e)
    }
}

impl From<str::Error> for Error {
    fn from(e: str::Error) -> Self {
        Error::Str(e)
    }
}

impl From<token::Error> for Error {
    fn from(e: token::Error) -> Self {
        Error::Seq(e)
    }
}

use core::fmt::{self, Display};

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            ExpectedValue => "expected value".fmt(f),
            Depth => "maximal depth exceeded".fmt(f),
            Num(num::Error::ExpectedDigit) => "expected digit".fmt(f),
            Str(e) => e.fmt(f),
            Seq(e) => e.fmt(f),
            Token(t) => write!(f, "unexpected token: {}", t),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
