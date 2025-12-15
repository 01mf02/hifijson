/// Low-level input operations.
pub trait Read {
    /// Return `true` if the given byte sequence is a prefix of the input.
    ///
    /// This function may advance the input even if it returns `false`.
    fn strip_prefix(&mut self, s: &[u8]) -> bool {
        for c1 in s {
            match self.take_next() {
                Some(c2) if *c1 == c2 => continue,
                Some(_) | None => return false,
            }
        }
        true
    }

    /// Run a function on current input until a certain condition is fulfilled.
    fn foreach_until(&mut self, mut f: impl FnMut(u8), mut stop: impl FnMut(u8) -> bool) {
        self.skip_until(|c| {
            stop(c) || {
                f(c);
                false
            }
        })
    }

    /// Ignore input until `stop` yields true.
    fn skip_until(&mut self, mut stop: impl FnMut(u8) -> bool) {
        while let Some(c) = self.peek_next() {
            if stop(c) {
                return;
            } else {
                self.take_next();
            }
        }
    }

    /// Peek at the next byte.
    fn peek_next(&mut self) -> Option<u8>;

    /// Take the next byte.
    fn take_next(&mut self) -> Option<u8>;
}

impl<'a> Read for crate::SliceLexer<'a> {
    fn strip_prefix(&mut self, s: &[u8]) -> bool {
        if let Some(rest) = self.slice.strip_prefix(s) {
            self.slice = rest;
            true
        } else {
            false
        }
    }

    fn skip_until(&mut self, mut stop: impl FnMut(u8) -> bool) {
        while let Some((_, tl)) = self.slice.split_first().filter(|(hd, _)| !stop(**hd)) {
            self.slice = tl
        }
    }

    fn take_next(&mut self) -> Option<u8> {
        let (head, rest) = self.slice.split_first()?;
        self.slice = rest;
        Some(*head)
    }

    fn peek_next(&mut self) -> Option<u8> {
        self.slice.first().copied()
    }
}

impl<E, I: Iterator<Item = Result<u8, E>>> Read for crate::IterLexer<E, I> {
    fn take_next(&mut self) -> Option<u8> {
        self.last.take().or_else(|| match self.bytes.next()? {
            Ok(b) => Some(b),
            Err(e) => {
                self.error = Some(e);
                None
            }
        })
    }

    fn peek_next(&mut self) -> Option<u8> {
        self.last.or_else(|| {
            self.last = self.take_next();
            self.last
        })
    }
}
