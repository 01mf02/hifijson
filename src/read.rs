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
