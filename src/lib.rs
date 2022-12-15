//! High-fidelity JSON lexer and parser.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::num::NonZeroUsize;

#[cfg(feature = "alloc")]
mod iterlexer;
#[cfg(feature = "alloc")]
mod parse;
#[cfg(feature = "alloc")]
mod strparser;

pub mod error;
mod escape;
mod lexer;
mod slicelexer;

#[cfg(feature = "alloc")]
pub use iterlexer::IterLexer;
#[cfg(feature = "alloc")]
pub use parse::{parse, parse_many, parse_single, Error, Value};
#[cfg(feature = "alloc")]
pub use strparser::LexerStr;

pub use escape::Escape;
pub use lexer::{Lexer, Token};
pub use slicelexer::SliceLexer;

/// Position of `.` and `e`/`E` in the string representation of a number.
///
/// Because a number cannot start with `.` or `e`/`E`,
/// these positions must always be greater than zero.
#[derive(Debug, Default)]
pub struct NumParts {
    /// position of the dot
    pub dot: Option<NonZeroUsize>,
    /// position of the exponent character (`e`/`E`)
    pub exp: Option<NonZeroUsize>,
}
