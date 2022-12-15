//! Lexer errors.

#[derive(Debug)]
pub enum Num {
    ExpectedDigit,
}

#[derive(Debug)]
pub enum Str {
    Control,
    Escape(Escape),
    Eof,
    Utf8(core::str::Utf8Error),
}

#[derive(Debug)]
pub enum Escape {
    Eof,
    UnknownKind,
    InvalidHex,
    InvalidChar(u32),
    ExpectedLowSurrogate,
}

#[derive(Debug)]
pub enum Seq {
    ExpectedItem,
    ExpectedItemOrEnd,
    ExpectedCommaOrEnd,
}
