pub trait Write {
    type Bytes: core::ops::Deref<Target = [u8]> + Default;

    /// Write input to `bytes` until `stop` yields true.
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
        while let Some(c) = self.read() {
            if stop(c) {
                self.last = Some(c);
                return;
            } else {
                bytes.push(c)
            }
        }
        self.last = None
    }
}
