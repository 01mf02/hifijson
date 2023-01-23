/// Low-level input operations.
pub trait Read {
    /// Return `true` if the given byte sequence is a prefix of the input.
    fn strip_prefix<const N: usize>(&mut self, s: [u8; N]) -> bool;

    fn skip_until(&mut self, stop: impl FnMut(u8) -> bool);

    /// Ignore input until `stop` yields true.
    fn skip_next_until(&mut self, stop: impl FnMut(u8) -> bool);

    /// Read a byte, do not put it into buffer.
    fn read(&mut self) -> Option<u8>;

    /// Read a byte and put it into buffer.
    fn read_next(&mut self);

    /// Peek at the byte from the buffer.
    fn peek_next(&self) -> Option<&u8>;

    /// Take the byte from the buffer.
    fn take_next(&mut self) -> Option<u8>;
}

impl<'a> Read for crate::SliceLexer<'a> {
    fn strip_prefix<const N: usize>(&mut self, s: [u8; N]) -> bool {
        if let Some(rest) = self.slice.strip_prefix(&s) {
            self.slice = rest;
            true
        } else {
            false
        }
    }

    fn skip_until(&mut self, stop: impl FnMut(u8) -> bool) {
        use crate::Write;
        self.write_until(&mut &[][..], stop)
    }

    fn skip_next_until(&mut self, stop: impl FnMut(u8) -> bool) {
        self.skip_until(stop)
    }

    fn read(&mut self) -> Option<u8> {
        let (head, rest) = self.slice.split_first()?;
        self.slice = rest;
        Some(*head)
    }

    fn read_next(&mut self) {
        self.slice = &self.slice[1..]
    }

    fn peek_next(&self) -> Option<&u8> {
        self.slice.first()
    }

    fn take_next(&mut self) -> Option<u8> {
        self.read()
    }
}

impl<E, I: Iterator<Item = Result<u8, E>>> Read for crate::IterLexer<E, I> {
    fn strip_prefix<const N: usize>(&mut self, s: [u8; N]) -> bool {
        for c1 in s {
            match self.read() {
                Some(c2) if c1 == c2 => continue,
                Some(_) | None => return false,
            }
        }
        true
    }

    fn skip_until(&mut self, mut stop: impl FnMut(u8) -> bool) {
        for c in self.bytes.by_ref() {
            match c {
                Ok(c) if !stop(c) => continue,
                Ok(c) => self.last = Some(c),
                Err(e) => {
                    self.last = Some(0);
                    self.error = Some(e);
                }
            }
            return;
        }
        self.last = None
    }

    fn skip_next_until(&mut self, mut stop: impl FnMut(u8) -> bool) {
        match self.last {
            Some(last) if stop(last) => (),
            _ => self.skip_until(stop),
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

    fn read_next(&mut self) {
        self.skip_until(|_| true)
    }

    fn take_next(&mut self) -> Option<u8> {
        self.last.take()
    }

    fn peek_next(&self) -> Option<&u8> {
        self.last.as_ref()
    }
}
