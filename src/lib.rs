//! High-fidelity JSON lexer and parser.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod read;
mod write;

pub mod escape;
pub mod num;
#[cfg(feature = "alloc")]
pub mod parse;
pub mod str;
#[cfg(feature = "alloc")]
pub mod string;
pub mod token;

use read::Read;
pub use token::Token;
use write::Write;

pub trait Lex: token::Lex + num::Lex + str::Lex {}
pub trait LexWrite: Lex + num::LexWrite + str::LexWrite {}
#[cfg(feature = "alloc")]
pub trait LexAlloc: LexWrite + string::LexAlloc {}

impl<T> Lex for T where T: token::Lex + num::Lex + str::Lex {}
impl<T> LexWrite for T where T: Lex + num::LexWrite + str::LexWrite {}
#[cfg(feature = "alloc")]
impl<T> LexAlloc for T where T: LexWrite + string::LexAlloc {}

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
    error: Option<E>,
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
