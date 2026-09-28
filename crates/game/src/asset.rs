//! Asset loading: the "Comp"/"Wolf" LZSS decompressor (NOTES.md, "Asset
//! compression").
//!
//! [`func_80011940`] is the N64Recomp-shaped port, exact down to the RDRAM
//! access order and the registers it leaves behind. The same algorithm over
//! byte slices, for tools that read assets straight from the ROM, is
//! `assets::lzss`; the differential tests check it against the recompiled C
//! too.

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
