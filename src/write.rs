/// Writing input to bytes.
///
/// This is useful to capture a part of the input,
/// without allocating when the input is a slice.
pub trait Write {
    /// Type of bytes to write to.
    type Bytes: core::ops::Deref<Target = [u8]> + Default;

    /// Write input to `bytes` until `stop` yields true.
    ///
    /// The `stop` function gets the previously read bytes and the current byte.
    ///
    /// This function does not return a new [`Self::Bytes`] to avoid allocations.
    ///
    /// ~~~
    /// fn test<L: hifijson::Write>(lexer: &mut L) {
    ///     let mut bytes = L::Bytes::default();
    ///     lexer.write_until(&mut bytes, |_, c| c == b' ');
    ///     assert_eq!(&*bytes, b"Hello");
    ///     lexer.write_until(&mut bytes, |_, _| false);
    ///     assert_eq!(&*bytes, b" World");
    /// }
    /// let s = b"Hello World";
    /// test(&mut hifijson::SliceLexer::new(s));
    /// test(&mut hifijson::IterLexer::new(s.iter().copied().map(Ok::<_, ()>)));
    /// ~~~
    fn write_until(&mut self, bytes: &mut Self::Bytes, stop: impl FnMut(&[u8], u8) -> bool);

    /// Append input to `bytes` until `stop` yields true.
    ///
    /// This assumes that `bytes` is a suffix of the previously consumed input.
    fn append_until(&mut self, bytes: &mut Self::Bytes, stop: impl FnMut(&[u8], u8) -> bool);
}

impl<'a> Write for crate::SliceLexer<'a> {
    type Bytes = &'a [u8];

    fn append_until(&mut self, bytes: &mut Self::Bytes, mut stop: impl FnMut(&[u8], u8) -> bool) {
        // in my end is your beginning
        debug_assert_eq!(
            bytes.as_ptr() as usize + bytes.len(),
            self.slice.as_ptr() as usize
        );
        // rewind by prefix length
        self.slice = &self.whole[self.offset() - bytes.len()..];
        let mut iter = self.slice.iter().enumerate().skip(bytes.len());
        let pos = iter
            .find(|(i, c)| stop(&self.slice[..*i], **c))
            .map_or(self.slice.len(), |(i, _c)| i);
        *bytes = &self.slice[..pos];
        self.slice = &self.slice[pos..]
    }

    fn write_until(&mut self, bytes: &mut Self::Bytes, stop: impl FnMut(&[u8], u8) -> bool) {
        *bytes = &self.slice[..0];
        self.append_until(bytes, stop)
    }
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> Write for crate::IterLexer<E, I> {
    type Bytes = alloc::vec::Vec<u8>;

    fn write_until(&mut self, bytes: &mut Self::Bytes, stop: impl FnMut(&[u8], u8) -> bool) {
        bytes.clear();
        self.append_until(bytes, stop)
    }

    fn append_until(&mut self, bytes: &mut Self::Bytes, mut stop: impl FnMut(&[u8], u8) -> bool) {
        use crate::Read;
        while let Some(c) = self.peek_next() {
            if stop(bytes, c) {
                return;
            } else {
                self.take_next();
                bytes.push(c)
            }
        }
    }
}
