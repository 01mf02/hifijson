//! High-fidelity JSON lexer and parser.

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

pub use token::{Token, Expect};

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
#[derive(Debug)]
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
