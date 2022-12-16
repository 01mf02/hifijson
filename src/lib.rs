//! High-fidelity JSON lexer and parser.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod escape;
pub mod num;
#[cfg(feature = "alloc")]
pub mod parse;
mod read;
pub mod str;
#[cfg(feature = "alloc")]
pub mod string;
pub mod token;

use read::Read;
pub use token::Token;

pub trait Lex: token::Lex + num::Lex + str::Lex {}
#[cfg(feature = "alloc")]
pub trait LexAlloc: Lex + string::Lex {}

impl<T> Lex for T where T: token::Lex + num::Lex + str::Lex {}
#[cfg(feature = "alloc")]
impl<T> LexAlloc for T where T: Lex + string::Lex {}

/// JSON lexer from a shared byte slice.
pub struct SliceLexer<'a> {
    slice: &'a [u8],
}

impl<'a> SliceLexer<'a> {
    pub fn new(slice: &'a [u8]) -> Self {
        Self { slice }
    }
}

#[cfg(feature = "alloc")]
/// JSON lexer from an iterator over (fallible) bytes.
pub struct IterLexer<E, I> {
    bytes: I,
    last: Option<u8>,
    error: Option<E>,
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> IterLexer<E, I> {
    pub fn new(iter: I) -> Self {
        Self {
            bytes: iter,
            last: None,
            error: None,
        }
    }

    fn read(&mut self) -> Option<u8> {
        match self.bytes.next()? {
            Ok(b) => Some(b),
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }
}
