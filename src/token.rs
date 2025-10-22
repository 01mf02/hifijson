//! Tokens.

/// What we expected to get, but did not get.
#[derive(Debug, PartialEq, Eq)]
pub enum Expect {
    /// `   ` or `]` or `,`
    Value,
    /// `[` or `{`
    ValueOrEnd,
    /// `[1` or `[1 2`
    CommaOrEnd,
    /// `{0: 1}`
    String,
    /// `{"a" 1}`
    Colon,
    /// `true false` (when parsing exactly one value)
    Eof,
}

impl core::fmt::Display for Expect {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use Expect::*;
        match self {
            Value => "value".fmt(f),
            ValueOrEnd => "value or end of sequence".fmt(f),
            CommaOrEnd => "comma or end of sequence".fmt(f),
            String => "string".fmt(f),
            Colon => "colon".fmt(f),
            Eof => "end of file".fmt(f),
        }
    }
}

/// Lexing that does not require allocation.
pub trait Lex: crate::Read {
    /// Skip input until the earliest non-whitespace character.
    fn eat_whitespace(&mut self) {
        self.skip_until(|c| !matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
    }

    /// Skip whitespace and peek at the following character.
    fn ws_peek(&mut self) -> Option<u8> {
        self.eat_whitespace();
        self.peek_next()
    }

    /// Parse a JSON token starting with a letter.
    fn null_or_bool(&mut self) -> Option<Option<bool>> {
        // we are calling this function without having advanced before
        Some(match self.take_next() {
            Some(b'n') if self.strip_prefix(b"ull") => None,
            Some(b't') if self.strip_prefix(b"rue") => Some(true),
            Some(b'f') if self.strip_prefix(b"alse") => Some(false),
            _ => return None,
        })
    }

    /// Take next token, discard it, and return mutable handle to lexer.
    ///
    /// This is useful in particular when parsing negative numbers,
    /// where you want to discard `-` and immediately continue.
    fn discarded(&mut self) -> &mut Self {
        self.take_next();
        self
    }

    /// Parse a string with given function, followed by a colon.
    fn str_colon<T, E: From<Expect>, PF, F>(&mut self, next: u8, pf: PF, f: F) -> Result<T, E>
    where
        PF: FnOnce(&mut Self) -> Option<u8>,
        F: FnOnce(&mut Self) -> Result<T, E>,
    {
        if next == b'"' {
            self.take_next();
        } else {
            Err(Expect::String)?
        }
        let key = f(self)?;

        if pf(self) == Some(b':') {
            self.take_next();
        } else {
            Err(Expect::Colon)?
        }

        Ok(key)
    }

    /// Execute `f` for every item in the comma-separated sequence until `end`.
    fn seq<E: From<Expect>, PF, F>(&mut self, end: u8, mut pf: PF, mut f: F) -> Result<(), E>
    where
        PF: FnMut(&mut Self) -> Option<u8>,
        F: FnMut(u8, &mut Self) -> Result<(), E>,
    {
        let mut next = pf(self).ok_or(Expect::ValueOrEnd)?;
        if next == end {
            self.take_next();
            return Ok(());
        };

        loop {
            f(next, self)?;
            next = pf(self).ok_or(Expect::CommaOrEnd)?;
            if next == end {
                self.take_next();
                return Ok(());
            } else if next == b',' {
                self.take_next();
                next = pf(self).ok_or(Expect::Value)?;
            } else {
                return Err(Expect::CommaOrEnd)?;
            }
        }
    }

    /// Parse once using given function and assure that the function has consumed all tokens.
    fn exactly_one<T, E: From<Expect>, PF, F>(&mut self, mut pf: PF, f: F) -> Result<T, E>
    where
        PF: FnMut(&mut Self) -> Option<u8>,
        F: FnOnce(u8, &mut Self) -> Result<T, E>,
    {
        let next = pf(self).ok_or(Expect::Value)?;
        let v = f(next, self)?;
        match pf(self) {
            None => Ok(v),
            Some(_) => Err(Expect::Eof)?,
        }
    }
}

impl<T> Lex for T where T: crate::Read {}
