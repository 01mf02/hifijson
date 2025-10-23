/// Writing input to bytes.
///
/// This is useful to capture a part of the input,
/// without allocating when the input is a slice.
pub trait Write {
    /// Type of bytes to write to.
    type Bytes: core::ops::Deref<Target = [u8]> + Default;

    /// Write input to `bytes` until `stop` yields true.
    ///
    /// This function does not return a new [`Self::Bytes`] to avoid allocations.
    ///
    /// ~~~
    /// fn test<L: hifijson::Write>(lexer: &mut L) {
    ///     let mut bytes = L::Bytes::default();
    ///     lexer.write_until(&mut bytes, |c| c == b' ');
    ///     assert_eq!(&*bytes, b"Hello");
    ///     lexer.write_until(&mut bytes, |_| false);
    ///     assert_eq!(&*bytes, b" World");
    /// }
    /// let s = b"Hello World";
    /// test(&mut hifijson::SliceLexer::new(s));
    /// test(&mut hifijson::IterLexer::new(s.iter().copied().map(Ok::<_, ()>)));
    /// ~~~
    fn write_until(&mut self, bytes: &mut Self::Bytes, stop: impl FnMut(u8) -> bool);
}

impl<'a> Write for crate::SliceLexer<'a> {
    type Bytes = &'a [u8];

    fn write_until(&mut self, bytes: &mut &'a [u8], mut stop: impl FnMut(u8) -> bool) {
        let pos = self.slice.iter().position(|c| stop(*c));
        let pos = pos.unwrap_or(self.slice.len());
        *bytes = &self.slice[..pos];
        self.slice = &self.slice[pos..]
    }
}

#[cfg(feature = "alloc")]
impl<E, I: Iterator<Item = Result<u8, E>>> Write for crate::IterLexer<E, I> {
    type Bytes = alloc::vec::Vec<u8>;

    fn write_until(&mut self, bytes: &mut Self::Bytes, mut stop: impl FnMut(u8) -> bool) {
        use crate::Read;
        bytes.clear();
        while let Some(c) = self.peek_next() {
            if stop(c) {
                return;
            } else {
                self.take_next();
                bytes.push(c)
            }
        }
    }
}
