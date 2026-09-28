//! Asset loading: the "Comp"/"Wolf" LZSS decompressor (NOTES.md, "Asset
//! compression").
//!
//! [`func_80011940`] is the N64Recomp-shaped port, exact down to the RDRAM
//! access order and the registers it leaves behind. [`lzss`] is the same
//! algorithm over byte slices, for tools that read assets straight from the
//! ROM; the differential tests check it against the recompiled C too.

use crate::recomp::{addu, enter, reg::*, s32, sll, RecompContext};

/// `func_80011940(src, dst)`: decompress the LZSS stream at `a0` to `a1` and
/// return the end of the output in `v0`.
///
/// The 4 KB ring buffer is the memory just below the input (`a0 - 0x1000`)
/// and is never initialised. The write position starts at 1. Flag bits are
/// consumed LSB first: 1 = literal byte, 0 = two-byte reference
/// `((b0 & 0xF) << 8 | b1, (b0 >> 4) + 2)`, and offset 0 ends the stream.
///
/// Every output byte is read (from the input or the window), written to
/// `dst`, then written to the window, in that order, as the original does.
/// So overlapping output, window and input behave exactly as on hardware.
///
/// Domain: `a0` and `a1` are canonical (sign-extended 32-bit) pointers, since
/// they are dereferenced before any 32-bit add; the input, window, output and
/// the 16 bytes below `sp` lie in RDRAM; and the output does not overwrite
/// input not yet read in a way that stops the stream terminating. Outside
/// it the original faults or runs away, and this port panics in `n64mem`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011940(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    let rd = |mem: &n64mem::Mem, a: u64| u64::from(mem.read_u8(a as u32));
    let off = |r: u64, o: u32| (r as u32).wrapping_add(o);

    let (mut at, mut a0, mut a1) = (g[AT], g[A0], g[A1]);
    let (mut t1, mut t2, mut t3, mut t4, mut t5) = (g[T1], g[T2], g[T3], g[T4], g[T5]);
    // t6-t9 are always written before they are read.
    let (mut t6, mut t7, mut t8, mut t9);
    let (mut s1, mut s2) = (g[S1], g[S2]);

    // Prologue: s0-s2 are saved with sw (low words).
    let sp = addu(g[SP], (-0x10i64) as u64);
    mem.write_u32(off(sp, 4), g[S0] as u32);
    let mut s0 = a1; // or $s0, $a1, $zero: a full 64-bit move
    mem.write_u32(off(sp, 0xC), s2 as u32);
    mem.write_u32(off(sp, 8), s1 as u32);

    let v0 = addu(a0, (-0x1000i64) as u64); // window base
    let mut v1: u64 = 1; // window write position
    let mut a2: u64 = 0; // set by the terminator
    let mut a3: u64;
    let mut t0 = rd(&mem, a0);
    loop {
        // L_80011964: next flag byte
        a0 = addu(a0, 1);
        a3 = 0;
        t6 = 1;
        loop {
            // L_80011970
            t7 = sll(t6, (a3 & 31) as u32);
            t8 = t0 & t7;
            t9 = addu(v0, v1); // branch delay slot, taken or not
            if t8 != 0 {
                // Literal.
                a1 = rd(&mem, a0);
                v1 = addu(v1, 1);
                t6 = v1 & 0xFFF;
                mem.write_u8(s0 as u32, a1 as u8);
                a0 = addu(a0, 1);
                s0 = addu(s0, 1);
                v1 = t6;
                mem.write_u8(t9 as u32, a1 as u8);
            } else {
                // Reference.
                t1 = rd(&mem, a0);
                t2 = rd(&mem, u64::from(off(a0, 1)));
                a0 = addu(a0, 2);
                t7 = t1 & 0xF;
                t8 = sll(t7, 8);
                t9 = s32(((t1 as i64) >> 4) as u32);
                t2 = addu(t2, t8);
                t1 = t9;
                if t2 == 0 {
                    // Offset 0: end of stream, straight out of the bit loop.
                    a2 = 1;
                    break;
                }
                t1 = addu(t1, 1);
                t3 = 0;
                // bltz $t1: never taken (t1 is 1..16), kept for fidelity.
                if (t1 as i64) >= 0 {
                    // Copy t1 + 1 bytes: first (t1 + 1) & 3 singly, then four at a time.
                    a1 = addu(t1, 1);
                    t6 = a1 & 3;
                    t5 = t6;
                    let mut unrolled = true;
                    if t6 != 0 {
                        t4 = addu(t2, 0);
                        loop {
                            t7 = t4 & 0xFFF;
                            t8 = addu(t7, v0);
                            a1 = rd(&mem, t8);
                            t9 = addu(v0, v1);
                            v1 = addu(v1, 1);
                            t6 = v1 & 0xFFF;
                            t3 = addu(t3, 1);
                            mem.write_u8(s0 as u32, a1 as u8);
                            v1 = t6;
                            t4 = addu(t4, 1);
                            s0 = addu(s0, 1);
                            mem.write_u8(t9 as u32, a1 as u8);
                            if t5 == t3 {
                                break;
                            }
                        }
                        t7 = addu(t1, 1);
                        unrolled = t7 != t3;
                    }
                    // L_80011A2C (also the delay slot of the beq above)
                    t4 = addu(t2, t3);
                    if unrolled {
                        t5 = addu(t4, 1);
                        s1 = addu(t4, 2);
                        s2 = addu(t4, 3);
                        loop {
                            // L_80011A3C
                            t8 = t4 & 0xFFF;
                            t9 = addu(t8, v0);
                            a1 = rd(&mem, t9);
                            t8 = t5 & 0xFFF;
                            t6 = addu(v0, v1);
                            mem.write_u8(s0 as u32, a1 as u8);
                            t9 = addu(t8, v0);
                            mem.write_u8(t6 as u32, a1 as u8);

                            a1 = rd(&mem, t9);
                            v1 = addu(v1, 1);
                            t7 = v1 & 0xFFF;
                            t6 = addu(v0, t7);
                            t8 = s1 & 0xFFF;
                            mem.write_u8(off(s0, 1), a1 as u8);
                            t9 = addu(t8, v0);
                            mem.write_u8(t6 as u32, a1 as u8);

                            a1 = rd(&mem, t9);
                            v1 = addu(t7, 1);
                            t7 = v1 & 0xFFF;
                            t6 = addu(v0, t7);
                            t8 = s2 & 0xFFF;
                            mem.write_u8(off(s0, 2), a1 as u8);
                            t9 = addu(t8, v0);
                            v1 = addu(t7, 1);
                            mem.write_u8(t6 as u32, a1 as u8);

                            a1 = rd(&mem, t9);
                            t7 = v1 & 0xFFF;
                            t6 = addu(v0, t7);
                            v1 = addu(t7, 1);
                            t7 = v1 & 0xFFF;
                            t8 = addu(t1, 1);
                            t3 = addu(t3, 4);
                            mem.write_u8(off(s0, 3), a1 as u8);
                            v1 = t7;
                            s2 = addu(s2, 4);
                            s1 = addu(s1, 4);
                            t5 = addu(t5, 4);
                            t4 = addu(t4, 4);
                            s0 = addu(s0, 4);
                            mem.write_u8(t6 as u32, a1 as u8);
                            if t8 == t3 {
                                break;
                            }
                        }
                    }
                }
            }
            // L_80011AE0: next flag bit (a3 is kept as a sign-extended halfword)
            a3 = addu(a3, 1);
            t9 = sll(a3, 16);
            a3 = s32(((t9 as i64) >> 16) as u32);
            at = u64::from((a3 as i64) < 8);
            if at == 0 {
                break;
            }
            t6 = 1; // bnel delay slot: only when taken
        }
        // L_80011AF8
        if a2 != 0 {
            break;
        }
        t0 = rd(&mem, a0); // beql delay slot: only when taken
    }

    // Epilogue: v0 = end of output; s0-s2 come back sign-extended from the
    // stack (the s1/s2 values the copy loop left are discarded).
    g[V0] = s0;
    g[S0] = s32(mem.read_u32(off(sp, 4)));
    g[S1] = s32(mem.read_u32(off(sp, 8)));
    g[S2] = s32(mem.read_u32(off(sp, 0xC)));
    g[SP] = addu(sp, 0x10);

    g[AT] = at;
    g[V1] = v1;
    g[A0] = a0;
    g[A1] = a1;
    g[A2] = a2;
    g[A3] = a3;
    g[T0] = t0;
    g[T1] = t1;
    g[T2] = t2;
    g[T3] = t3;
    g[T4] = t4;
    g[T5] = t5;
    g[T6] = t6;
    g[T7] = t7;
    g[T8] = t8;
    g[T9] = t9;
}

/// The same LZSS over byte slices, for reading assets from the ROM.
pub mod lzss {
    /// Ring buffer size and initial write position (see [`super::func_80011940`]).
    pub const WINDOW: usize = 0x1000;
    const START: usize = 1;

    #[derive(Debug, PartialEq, Eq)]
    pub enum Error {
        /// The stream ended before its terminator.
        Truncated { at: usize },
        /// A reference read a window byte nothing had written yet. On the N64
        /// that is whatever RDRAM held below the input. No real asset does it
        /// (NOTES.md), so this refuses rather than guess.
        UnwrittenWindow { at: usize },
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Error::Truncated { at } => write!(f, "LZSS stream truncated at input byte {at:#x}"),
                Error::UnwrittenWindow { at } => {
                    write!(f, "LZSS reference at input byte {at:#x} reads the uninitialised window")
                }
            }
        }
    }

    impl std::error::Error for Error {}

    /// A decompressed stream and how many input bytes it used, terminator included.
    #[derive(Debug)]
    pub struct Decompressed {
        pub data: Vec<u8>,
        pub consumed: usize,
    }

    /// Decompress an LZSS stream (the bytes after the 12-byte "Comp" header).
    pub fn decompress(input: &[u8]) -> Result<Decompressed, Error> {
        let mut window = [0u8; WINDOW];
        let mut written = [false; WINDOW];
        let mut pos = START;
        let mut out = Vec::new();
        let mut i = 0;
        let byte = |i: usize| input.get(i).copied().ok_or(Error::Truncated { at: i });
        loop {
            let flags = byte(i)?;
            i += 1;
            for bit in 0..8 {
                if flags >> bit & 1 != 0 {
                    let b = byte(i)?;
                    i += 1;
                    out.push(b);
                    window[pos] = b;
                    written[pos] = true;
                    pos = (pos + 1) % WINDOW;
                } else {
                    let (b0, b1) = (byte(i)?, byte(i + 1)?);
                    let at = i;
                    i += 2;
                    let offset = usize::from(b0 & 0xF) << 8 | usize::from(b1);
                    if offset == 0 {
                        return Ok(Decompressed { data: out, consumed: i });
                    }
                    for k in 0..usize::from(b0 >> 4) + 2 {
                        let src = (offset + k) % WINDOW;
                        if !written[src] {
                            return Err(Error::UnwrittenWindow { at });
                        }
                        let b = window[src];
                        out.push(b);
                        window[pos] = b;
                        written[pos] = true;
                        pos = (pos + 1) % WINDOW;
                    }
                }
            }
        }
    }

    /// A "Comp" block: the 12-byte header and its stream.
    #[derive(Debug, PartialEq, Eq)]
    pub struct Comp<'a> {
        /// Bytes 4..8. "Wolf" in every block of the USA ROM.
        pub tag: [u8; 4],
        /// Declared decompressed size.
        pub size: u32,
        pub stream: &'a [u8],
    }

    /// Recognise a "Comp" block, as the model loader does (first word only).
    pub fn parse_comp(block: &[u8]) -> Option<Comp<'_>> {
        if block.len() < 12 || &block[..4] != b"Comp" {
            return None;
        }
        Some(Comp {
            tag: block[4..8].try_into().unwrap(),
            size: u32::from_be_bytes(block[8..12].try_into().unwrap()),
            stream: &block[12..],
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn literals_references_and_terminator() {
            // flags 0b0000_0111: three literals, then a reference to offset 1
            // (the first literal) of length 5 that overlaps itself, then the end.
            let s = [0x07, b'a', b'b', b'c', 0x30, 0x01, 0x00, 0x00];
            let d = decompress(&s).unwrap();
            assert_eq!(d.data, b"abcabcab");
            assert_eq!(d.consumed, s.len());
        }

        #[test]
        fn errors() {
            assert_eq!(decompress(&[0x01]).unwrap_err(), Error::Truncated { at: 1 });
            assert_eq!(decompress(&[0x00, 0x00, 0x05]).unwrap_err(), Error::UnwrittenWindow { at: 1 });
            assert!(parse_comp(b"Comp").is_none());
            let c = parse_comp(b"CompWolf\x00\x00\x01\x00xyz").unwrap();
            assert_eq!((c.tag, c.size, c.stream), (*b"Wolf", 0x100, &b"xyz"[..]));
        }
    }
}
