//! RDRAM as N64Recomp's runtime lays it out (SPEC §5.3).
//!
//! The recompiled C addresses memory through the `MEM_*` macros in
//! N64Recomp's `include/recomp.h` (pinned at `ffb39cd`):
//!
//! - an address is a sign-extended 64-bit register value; its offset into the
//!   `rdram` buffer is `vaddr - 0xFFFF_FFFF_8000_0000`, i.e. the low 32 bits
//!   minus `0x8000_0000`;
//! - memory is stored as **host-endian 32-bit words**, so a word access is a
//!   plain native load/store;
//! - halfword accesses XOR the address with 2, byte accesses XOR it with 3,
//!   which is what makes them see big-endian ordering within each word;
//! - doublewords are two word accesses, high word at the lower address.
//!
//! Everything in Rust that touches emulated memory goes through [`Mem`], so if
//! this mapping ever turns out to be wrong it is fixed in one place.

use std::marker::PhantomData;

/// 8 MB: large enough for Expansion Pak mode (SPEC §2).
pub const RDRAM_SIZE: usize = 8 * 1024 * 1024;

/// Base of KSEG0, where the game's code and data live.
pub const KSEG0: u32 = 0x8000_0000;

/// Owned RDRAM buffer, 4-byte aligned so word accesses are aligned on the host.
#[derive(Clone, PartialEq, Eq)]
pub struct Rdram {
    words: Box<[u32]>,
}

impl Rdram {
    pub fn new() -> Self {
        Self { words: vec![0u32; RDRAM_SIZE / 4].into_boxed_slice() }
    }

    /// The whole buffer as N64 words: element `i` is the 32-bit word at
    /// `KSEG0 + 4 * i`, exactly as `read_u32` would return it. For bulk work
    /// in test harnesses (filling, comparing); ported code uses [`Mem`].
    pub fn as_words(&self) -> &[u32] {
        &self.words
    }

    /// Mutable form of [`Self::as_words`].
    pub fn as_words_mut(&mut self) -> &mut [u32] {
        &mut self.words
    }

    /// The pointer recompiled functions take as their `rdram` argument.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.words.as_mut_ptr().cast()
    }

    pub fn mem(&mut self) -> Mem<'_> {
        // SAFETY: the buffer is RDRAM_SIZE bytes, aligned, and borrowed mutably for 'a.
        unsafe { Mem::from_raw(self.as_mut_ptr(), RDRAM_SIZE) }
    }
}

impl Default for Rdram {
    fn default() -> Self {
        Self::new()
    }
}

/// A view of RDRAM with typed accessors. Addresses are N64 virtual addresses
/// (e.g. `0x8012_3456`); the view applies N64Recomp's offset and swizzling.
pub struct Mem<'a> {
    base: *mut u8,
    len: usize,
    _borrow: PhantomData<&'a mut [u8]>,
}

impl<'a> Mem<'a> {
    /// Wrap the `rdram` pointer passed to a recompiled-function-shaped
    /// `extern "C"` entry point.
    ///
    /// # Safety
    /// `base` must point to at least `len` writable bytes, 4-byte aligned,
    /// laid out as N64Recomp expects, and not otherwise accessed for `'a`
    /// except through this view.
    pub unsafe fn from_raw(base: *mut u8, len: usize) -> Self {
        debug_assert_eq!(base as usize % 4, 0, "rdram base must be word-aligned");
        Self { base, len, _borrow: PhantomData }
    }

    /// The raw buffer, to pass as the `rdram` argument of another
    /// N64Recomp-shaped function. Taking `&mut self` means no access through
    /// this view can overlap the callee's.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.base
    }

    /// Size of the view in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Offset into the buffer for `vaddr`, exactly as `MEM_W` computes it.
    #[inline]
    fn offset(&self, vaddr: u32, size: usize) -> usize {
        let off = vaddr.wrapping_sub(KSEG0) as usize;
        assert!(
            off + size <= self.len,
            "address {vaddr:#010X} (+{size}) is outside the {} byte RDRAM view",
            self.len
        );
        off
    }

    #[inline]
    pub fn read_u32(&self, vaddr: u32) -> u32 {
        debug_assert_eq!(vaddr % 4, 0, "unaligned word read at {vaddr:#010X}");
        let off = self.offset(vaddr, 4);
        // SAFETY: bounds checked above; alignment follows from base and vaddr alignment.
        unsafe { self.base.add(off).cast::<u32>().read() }
    }

    #[inline]
    pub fn write_u32(&mut self, vaddr: u32, value: u32) {
        debug_assert_eq!(vaddr % 4, 0, "unaligned word write at {vaddr:#010X}");
        let off = self.offset(vaddr, 4);
        // SAFETY: as read_u32.
        unsafe { self.base.add(off).cast::<u32>().write(value) }
    }

    #[inline]
    pub fn read_u16(&self, vaddr: u32) -> u16 {
        debug_assert_eq!(vaddr % 2, 0, "unaligned halfword read at {vaddr:#010X}");
        let off = self.offset(vaddr ^ 2, 2);
        // SAFETY: bounds checked; (vaddr ^ 2) stays halfword-aligned.
        unsafe { self.base.add(off).cast::<u16>().read() }
    }

    #[inline]
    pub fn write_u16(&mut self, vaddr: u32, value: u16) {
        debug_assert_eq!(vaddr % 2, 0, "unaligned halfword write at {vaddr:#010X}");
        let off = self.offset(vaddr ^ 2, 2);
        // SAFETY: as read_u16.
        unsafe { self.base.add(off).cast::<u16>().write(value) }
    }

    #[inline]
    pub fn read_u8(&self, vaddr: u32) -> u8 {
        let off = self.offset(vaddr ^ 3, 1);
        // SAFETY: bounds checked.
        unsafe { self.base.add(off).read() }
    }

    #[inline]
    pub fn write_u8(&mut self, vaddr: u32, value: u8) {
        let off = self.offset(vaddr ^ 3, 1);
        // SAFETY: bounds checked.
        unsafe { self.base.add(off).write(value) }
    }

    /// `ld`: high word at `vaddr`, low word at `vaddr + 4`.
    #[inline]
    pub fn read_u64(&self, vaddr: u32) -> u64 {
        (u64::from(self.read_u32(vaddr)) << 32) | u64::from(self.read_u32(vaddr.wrapping_add(4)))
    }

    /// `sd`: high word at `vaddr`, low word at `vaddr + 4`.
    #[inline]
    pub fn write_u64(&mut self, vaddr: u32, value: u64) {
        self.write_u32(vaddr, (value >> 32) as u32);
        self.write_u32(vaddr.wrapping_add(4), value as u32);
    }

    // Signed and float views. MIPS distinguishes lb/lbu and lh/lhu, so callers
    // must pick deliberately (SPEC §5.4).

    #[inline]
    pub fn read_i8(&self, vaddr: u32) -> i8 {
        self.read_u8(vaddr) as i8
    }
    #[inline]
    pub fn read_i16(&self, vaddr: u32) -> i16 {
        self.read_u16(vaddr) as i16
    }
    #[inline]
    pub fn read_i32(&self, vaddr: u32) -> i32 {
        self.read_u32(vaddr) as i32
    }
    #[inline]
    pub fn read_f32(&self, vaddr: u32) -> f32 {
        f32::from_bits(self.read_u32(vaddr))
    }
    #[inline]
    pub fn write_f32(&mut self, vaddr: u32, value: f32) {
        self.write_u32(vaddr, value.to_bits())
    }
    #[inline]
    pub fn read_f64(&self, vaddr: u32) -> f64 {
        f64::from_bits(self.read_u64(vaddr))
    }
    #[inline]
    pub fn write_f64(&mut self, vaddr: u32, value: f64) {
        self.write_u64(vaddr, value.to_bits())
    }

    /// Copy big-endian bytes (e.g. straight from the z64 ROM) into RDRAM.
    pub fn write_bytes(&mut self, vaddr: u32, bytes: &[u8]) {
        for (i, &b) in bytes.iter().enumerate() {
            self.write_u8(vaddr.wrapping_add(i as u32), b);
        }
    }

    /// Read RDRAM back out as big-endian bytes, as the N64 would see them.
    pub fn read_bytes(&self, vaddr: u32, out: &mut [u8]) {
        for (i, b) in out.iter_mut().enumerate() {
            *b = self.read_u8(vaddr.wrapping_add(i as u32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subword_accesses_see_big_endian_order() {
        let mut r = Rdram::new();
        let mut m = r.mem();
        m.write_u32(0x8000_1000, 0x1122_3344);
        assert_eq!(m.read_u8(0x8000_1000), 0x11);
        assert_eq!(m.read_u8(0x8000_1003), 0x44);
        assert_eq!(m.read_u16(0x8000_1000), 0x1122);
        assert_eq!(m.read_u16(0x8000_1002), 0x3344);
    }

    #[test]
    fn bytes_roundtrip_as_big_endian() {
        let mut r = Rdram::new();
        let mut m = r.mem();
        m.write_bytes(0x8000_2001, &[1, 2, 3, 4, 5, 6, 7]);
        let mut out = [0u8; 7];
        m.read_bytes(0x8000_2001, &mut out);
        assert_eq!(out, [1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(m.read_u32(0x8000_2004), 0x0405_0607);
    }

    #[test]
    fn doubleword_is_high_word_first() {
        let mut r = Rdram::new();
        let mut m = r.mem();
        m.write_u64(0x8000_3000, 0x0102_0304_0506_0708);
        assert_eq!(m.read_u32(0x8000_3000), 0x0102_0304);
        assert_eq!(m.read_u32(0x8000_3004), 0x0506_0708);
        assert_eq!(m.read_u8(0x8000_3007), 0x08);
    }

    #[test]
    fn expansion_pak_range_is_addressable() {
        let mut r = Rdram::new();
        let mut m = r.mem();
        m.write_u32(0x807F_FFFC, 0xDEAD_BEEF);
        assert_eq!(m.read_u32(0x807F_FFFC), 0xDEAD_BEEF);
    }

    #[test]
    #[should_panic(expected = "outside")]
    fn out_of_range_panics() {
        let mut r = Rdram::new();
        r.mem().read_u32(0x8080_0000);
    }
}
