/// Low-level input operations.
pub trait Read {
    type Bytes: core::ops::Deref<Target = [u8]> + Default;

    /// Ignore input until `stop` yields true.
    fn skip_until(&mut self, stop: impl FnMut(u8) -> bool);
    /// Read input to `bytes` until `stop` yields true.
    fn read_until(&mut self, bytes: &mut Self::Bytes, stop: impl FnMut(u8) -> bool);

    /// Look at the next byte.
    fn peek_byte(&self) -> Option<&u8>;
    /// Consume the next byte.
    fn read_byte(&mut self) -> Option<u8>;

    /// Return `true` if the given byte sequence is a prefix of the input.
    fn strip_prefix<const N: usize>(&mut self, s: [u8; N]) -> bool;
}

impl<'a> Read for crate::SliceLexer<'a> {
    type Bytes = &'a [u8];

    fn strip_prefix<const N: usize>(&mut self, s: [u8; N]) -> bool {
        if let Some(rest) = self.slice.strip_prefix(&s) {
            self.slice = rest;
            true
        } else {
            false
        }
    }

    fn peek_byte(&self) -> Option<&u8> {
        self.slice.first()
    }

    fn read_byte(&mut self) -> Option<u8> {
        let (head, rest) = self.slice.split_first()?;
        self.slice = rest;
        Some(*head)
    }

    fn skip_until(&mut self, stop: impl FnMut(u8) -> bool) {
        self.read_until(&mut &[][..], stop)
    }

    fn read_until(&mut self, bytes: &mut &'a [u8], mut stop: impl FnMut(u8) -> bool) {
        let pos = self.slice.iter().position(|c| stop(*c));
        let pos = pos.unwrap_or(self.slice.len());
        *bytes = &self.slice[..pos];
        self.slice = &self.slice[pos..]
    }
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> Read for crate::IterLexer<E, I> {
    type Bytes = alloc::vec::Vec<u8>;

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
        match self.last {
            Some(last) if stop(last) => return,
            _ => self.last = None,
        }

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
    }

    fn read_until(&mut self, bytes: &mut Self::Bytes, mut stop: impl FnMut(u8) -> bool) {
        while let Some(c) = self.read() {
            if stop(c) {
                self.last = Some(c);
                return;
            } else {
                bytes.push(c)
            }
        }
    }

    fn read_byte(&mut self) -> Option<u8> {
        self.last.take()
    }

    fn peek_byte(&self) -> Option<&u8> {
        self.last.as_ref()
    }
}
