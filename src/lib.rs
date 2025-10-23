//! High-fidelity JSON lexer and parser.
//!
//!
//! # Introduction
//!
//! JSON is a data format that is underspecified and sometimes contradictory.
//! As reference, I recommend the excellent article "[Parsing JSON is a Minefield]".
//! In particular, it is ambiguous how to parse strings and numbers.
//! For example, JSON does not impose any restriction on the maximal size of numbers,
//! but in reality, most JSON parsers use a lossy representation,
//! for example 64-bit floating point.
//! This is allowed by the JSON specification; however,
//! at the same time, if we are allowed to fix arbitrary maximal sizes,
//! then a parser that fails on every input is a valid parser!
//! I hope that I could convince you at this point that this is all quite a mess.
//! The best I can do to help you around this mess is to give *you*
//! a tool to deal with this mess in the way that suits you most.
//! hifijson is this tool.
//!
//! [Parsing JSON is a Minefield]: http://seriot.ch/projects/parsing_json.html
//!
//! What makes hifijson so flexible is that unlike most other JSON parsers,
//! it exposes its basic building blocks, called [*lexers*](#lexers),
//! that allow you to build your own parsers on top of them.
//!
//! Because hifijson exposes a variety of lexers and parsers,
//! you can combine them in a way that allows you to achieve your desired behaviour,
//! without having to write everything from scratch.
//! For example, suppose that your input data does not contain escape sequences (`\n`, `\uxxxx`);
//! then you can use the [`str::LexWrite::str_bytes`] function that is
//! guaranteed to never allocate memory when lexing from a slice,
//! making it suitable for usage in embedded environments.
//! Or suppose that you are reading an object `{"title": ..., "reviews": ...}`,
//! and you do not feel like caring about reviews today.
//! Then you can simply skip reading the value for reviews by using [`ignore::parse`].
//! Going wild and stretching the syntax a bit, you can also make
//! a parser that accepts any value (instead of only strings as mandated by JSON) as object key.
//! Or, if you just want to have a complete JSON value, then
//! you can use [`value::parse_unbounded`].
//! The choice is yours.
//!
//! In summary, hifijson aims to give you the tools to interpret JSON-like data
//! flexibly and performantly.
//!
//! ## Lexers
//!
//! The hardest part of lexing JSON are strings and numbers.
//! hifijson offers many different string/number lexers,
//! which differ most prominently in their memory allocation behaviour.
//! For example,
//! * [`str::Lex::str_ignore`] discards a string,
//! * [`str::LexWrite::str_bytes`] reads a string, but does not interpret escape sequences, and
//! * [`str::LexAlloc::str_string`] reads a string and interprets escape sequences.
//!
//! In particular,
//! lexers that implement the [`Lex`] trait do *never* allocate memory;
//! lexers that implement the [`LexWrite`] trait only allocate memory when lexing from iterators,
//! and
//! lexers that implement the [`LexAlloc`] trait may allocate memory when lexing from both
//! iterators and slices.
//!
//! ## Slices and Iterators
//!
//! One important feature of hifijson is that it allows to read from both
//! [slices](SliceLexer) and [iterators](IterLexer) over bytes.
//! This is useful when your application should support reading from both
//! files and streams (such as standard input).
//!
//! ## Feature Flags
//!
//! If you build hifijson without the feature flag `alloc`, you disable any allocation.
//! If you build hifijson with the feature flag `serde`,
//! then you can use hifijson to deserialise JSON to data types implementing `serde::Deserialize`.
//!
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
//! let value = lexer.exactly_one(Lex::ws_peek, hifijson::value::parse_unbounded);
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
//! The following example reads JSON from a file if an argument is given,
//! otherwise from standard input:
//!
//! ~~~ no_run
//! /// Parse a single JSON value and print it.
//! ///
//! /// Note that the `LexAlloc` trait indicates that this lexer allocates memory.
//! fn process<L: hifijson::LexAlloc>(mut lexer: L) {
//!     let value = lexer.exactly_one(L::ws_peek, hifijson::value::parse_unbounded);
//!     let value = value.expect("parse");
//!     println!("{}", value);
//! }
//!
//! let filename = std::env::args().nth(1);
//! if let Some(filename) = filename {
//!     let file = std::fs::read(filename).expect("read file");
//!     process(hifijson::SliceLexer::new(&file))
//! } else {
//!     use std::io::Read;
//!     process(hifijson::IterLexer::new(std::io::stdin().bytes()))
//! }
//! ~~~
//!
//! We just made a pretty printer (stretching the definition of pretty pretty far).
//!
//! ## Operating on the lexer
//!
//! Often, it is better for performance to operate directly on the non-whitespace characters that the lexer yields
//! rather than parsing everything into a value and then processing the value.
//! For example, the following example counts the number of values in the input JSON.
//! Unlike the previous examples, it requires only constant memory!
//!
//! ~~~
//! use hifijson::{Error, Expect, Lex};
//!
//! /// Recursively count the number of values in the value starting with the `next` character.
//! ///
//! /// The `Lex` trait indicates that this lexer does *not* allocate memory.
//! fn count<L: Lex>(next: u8, lexer: &mut L) -> Result<usize, Error> {
//!     match next {
//!         // the JSON values "null", "true", and "false"
//!         b'a'..=b'z' => Ok(lexer.null_or_bool().map(|_| 1).ok_or(Expect::Value)?),
//!         b'0'..=b'9' => Ok(lexer.num_ignore().map(|_| 1)?),
//!         b'-' => count(b'0', lexer.discarded()),
//!         b'"' => Ok(lexer.discarded().str_ignore().map(|_| 1)?),
//!
//!         // start of array
//!         b'[' => {
//!             // an array is a value itself, so start with 1
//!             let mut sum = 1;
//!             // perform the following for every item of the array
//!             lexer.discarded().seq(b']', L::ws_peek, |next, lexer| {
//!                 sum += count(next, lexer)?;
//!                 Ok::<_, Error>(())
//!             })?;
//!             Ok(sum)
//!         }
//!
//!         // start of object
//!         b'{' => {
//!             let mut sum = 1;
//!             // perform the following for every key-value pair of the object
//!             lexer.discarded().seq(b'}', L::ws_peek, |next, lexer| {
//!                 /// read the key, ignoring it, and then the ':' after it
//!                 lexer.str_colon(next, L::ws_peek, |lexer| lexer.str_ignore().map_err(Error::Str))?;
//!                 /// now peek the next non-whitespace character after ':'
//!                 let next = lexer.ws_peek().ok_or(Expect::Value)?;
//!                 sum += count(next, lexer)?;
//!                 Ok::<_, Error>(())
//!             })?;
//!             Ok(sum)
//!         }
//!         _ => Err(Expect::Value)?,
//!     }
//! }
//!
//! fn process<L: Lex>(mut lexer: L) -> Result<usize, Error> {
//!     lexer.exactly_one(L::ws_peek, count)
//! }
//!
//! let json = br#"[null, true, false, "hello", 0, 3.1415, [1, 2], {"x": 1, "y": 2}]"#;
//! let mut lexer = hifijson::SliceLexer::new(json);
//! let n = process(lexer).unwrap();
//! assert_eq!(n, 13)
//! ~~~
//!
//! ## More Examples
//!
//! See the `cat` example for a more worked version of a JSON "pretty" printer
//! that can be also used to lazily filter parts of the data based on a path.
//! hifijson also powers all JSON reading in the [jaq](https://crates.io/crates/jaq) crate,
//! for which it was originally created.

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

macro_rules! impl_error {
    ($ty:ty) => {
        #[cfg(feature = "std")]
        impl std::error::Error for $ty {}
    };
}

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use core::fmt::{self, Display};

mod read;
mod write;

pub use read::Read;
pub use write::Write;

pub mod escape;
pub mod num;
pub mod str;
pub mod token;

pub use token::Expect;

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
    whole: &'a [u8],
    slice: &'a [u8],
}

impl<'a> SliceLexer<'a> {
    /// Create a new slice lexer.
    ///
    /// A fast way to obtain the contents of a file as `&[u8]` is memory mapping;
    /// see for example the [memmap2](https://docs.rs/memmap2) crate.
    ///
    pub fn new(slice: &'a [u8]) -> Self {
        let whole = slice;
        Self { whole, slice }
    }

    /// Return remaining input as a subslice of the original data.
    ///
    /// This can be used to find the place where an error occurred.
    pub fn as_slice(&self) -> &'a [u8] {
        self.slice
    }

    /// Number of bytes consumed so far.
    fn offset(&self) -> usize {
        self.slice.as_ptr() as usize - self.whole.as_ptr() as usize
    }
}

/// JSON lexer from an iterator over (fallible) bytes.
///
/// This can be used to lex from a [`Read`](std::io::Read) as follows:
///
/// ~~~
/// use std::io::Read;
/// let read = std::io::stdin();
/// let lexer = hifijson::IterLexer::new(read.bytes());
/// ~~~
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

impl_error!(Error);
impl_error!(num::Error);
impl_error!(escape::Error);
impl_error!(str::Error);

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            Depth => "maximal depth exceeded".fmt(f),
            Num(e) => e.fmt(f),
            Str(e) => e.fmt(f),
            Token(e) => write!(f, "{} expected", e),
        }
    }
}
