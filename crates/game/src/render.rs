//! Render state and display-list writers (NOTES.md, "Depth-0 leaves ported
//! in session 7"): appends to the display lists behind `[0x801217B0]` and
//! `[0x80112C90]`, the render modes at `0x800A3DA0` switched by tag, the
//! lights at `0x800A3DB0`, the framebuffer addresses, and the RSP task.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use n64mem::Mem;
use crate::recomp::{addu, enter, fpu, lbu, ld, lh, lhu, li, lw, multu, reg::*, s32, sb, sd, sh, sll, slt, sltu, sra, subu, sw, Fpr, RecompContext};

/// The display list pointer [`func_80014C98`] appends to.
pub const DL_HEAD: u32 = 0x8012_17B0;

/// `func_8000F5A0()` (depth probes, **guess**: flare and marker
/// occlusion): reads the depth buffer `Z = [0x80114528]` (u16 per pixel,
/// `W * H` of them for the s16 screen size at `0x80114470`, `multu`; `end =
/// Z + 2 W H`) at screen points held in several lists:
/// 1. For the two words `e = [0x8009B814 + 4k]` with `e >= 0` (signed),
///    with `x = [0x800D57A0 + 4k]`, `y = [0x800D57A8 + 4k]`: if `x < -500`,
///    `[0x800D57C0 + 4k] = 50`; otherwise it counts the 64 pixels of the 8 x
///    8 window at column `x - 4 + c`, row `y - 4 + r` (c, r = 0..7) that are
///    near an edge or not deep: counted if `c < 12 - x`, or `c >= W - 8 -
///    x`, or `r < 12 - y`, or `r >= H - 8 - y`; else if the address `p = Z +
///    2 ((y - 4 + r) W + x - 4 + c)` (32-bit) is below `Z`; else not if `p
///    >= end` (unsigned); else if `[p] < 0xFFDC`. The count goes to `[0x800D57C0
///    + 4k]`.
/// 2. For i = 0..19, two lists: `[0x800D5FD0 + 4i]` = -1000, then if `x =
///    [0x800D5AF8 + 4i] >= 0`, the depth at `(x, [0x800D5B48 + 4i])`; and
///    `[0x800D6020 + 4i]` from `[0x800D5B98 + 4i]`, `[0x800D5BE8 + 4i]` the
///    same way.
/// 3. For j = 0..39 whose byte `[0x800D5C38 + j] != 0`: `[0x800D60A0 + 4j]` =
///    -1000, then the depth at `([0x800D5E40 + 4j], [0x800D5EE0 + 4j])` if
///    that x is not negative.
/// 4. For j below the word `n = [0x8009B86C]` (signed): `[0x800D6070 + 4j]`
///    = -1000, then the depth at `([0x800D5958 + 4j], [0x800D5988 + 4j])`
///    if x is not negative.
///
/// "The depth at (x, y)" is the halfword `Z + 2 (y W + x)` (`multu`,
/// 32-bit), zero-extended. QUIRK: those reads aren't bounded (only `x >= 0`
/// is tested), and neither is `n` against its lists.
///
/// IDO unrolled the window's column loop by four, rotating the bounds and
/// temporaries through registers irregularly (the next column's left bound
/// is computed on every path out of a column); the port keeps it in the C's
/// order.
///
/// Saves `s0`..`s7`, `fp` at `sp - 0x64..-0x44` and restores them; spills
/// `H` at `sp - 0x3C`. Leaves `t4 = Z`, `a0 = -1000`, `a1` = the last
/// list's slot past its end, `v0`, `v1`, `a2`, `a3`, `t0`..`t3`, `t5`..`t9`,
/// `at` from the last steps.
///
/// Domain: the lists and outputs in RDRAM; every depth read in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000F5A0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A0] = li(0x8011_4470);
    g[SP] = addu(g[SP], (-0x68i64) as u64);
    sw(m, g[SP], 0x18, g[S5]);
    g[S5] = lh(m, g[A0], 0);
    g[T6] = lh(m, g[A0], 2);
    g[V0] = li(0x8011_0000);
    g[T4] = lw(m, g[V0], 0x4528);
    let (lo, _) = multu(g[S5], g[T6]);
    sw(m, g[SP], 0x20, g[S7]);
    sw(m, g[SP], 0x1C, g[S6]);
    sw(m, g[SP], 4, g[S0]);
    g[S7] = li(0x800A_0000);
    sw(m, g[SP], 0x24, g[FP]);
    sw(m, g[SP], 0x14, g[S4]);
    sw(m, g[SP], 0x10, g[S3]);
    sw(m, g[SP], 0xC, g[S2]);
    sw(m, g[SP], 8, g[S1]);
    g[T8] = lo;
    g[T9] = sll(g[T8], 1);
    g[S7] = addu(g[S7], (-0x47ECi64) as u64);
    g[S0] = 8;
    g[S6] = 0;
    g[T3] = 0xFFDC;
    g[T2] = 0xC;
    sw(m, g[SP], 0x2C, g[T6]);
    g[T1] = addu(g[T4], g[T9]);
    // 1. The two windows: s7 walks the entries, s6 = 4k; t4 = Z, t1 = end.
    loop {
        // L_8000F60C
        g[T6] = lw(m, g[S7], 0);
        g[A0] = li(0x800D_0000);
        g[S7] = addu(g[S7], 4);
        g[A0] = addu(g[A0], g[S6]);
        if (g[T6] as i64) >= 0 {
            g[A0] = lw(m, g[A0], 0x57A0);
            g[A2] = li(0x800D_0000);
            g[T7] = li(0x800D_0000);
            g[A2] = addu(g[A2], g[S6]);
            g[T7] = addu(g[T7], 0x57C0);
            g[AT] = slt(g[A0], (-0x1F4i64) as u64);
            g[A2] = lw(m, g[A2], 0x57A8);
            g[FP] = addu(g[S6], g[T7]);
            if g[AT] == 0 {
                // L_8000F650
                g[T6] = addu(g[A2], (-4i64) as u64);
                let (lo, _) = multu(g[T6], g[S5]);
                g[T9] = lw(m, g[SP], 0x2C);
                g[S4] = addu(g[S5], (-8i64) as u64);
                g[A3] = subu(g[S5], g[A0]);
                g[T0] = subu(g[T9], g[A2]);
                g[A3] = addu(g[A3], (-8i64) as u64);
                g[T0] = addu(g[T0], (-8i64) as u64);
                g[V1] = 0;
                g[A1] = 0;
                g[T7] = lo;
                g[T8] = addu(g[T7], g[A0]);
                g[T7] = 0xC;
                g[T9] = addu(g[T8], (-4i64) as u64);
                g[T5] = subu(g[T7], g[A0]);
                g[T6] = sll(g[T9], 1);
                g[T8] = sll(g[S4], 1);
                g[V0] = addu(g[T6], g[T4]);
                g[S4] = g[T8];
                g[S1] = addu(g[T5], (-3i64) as u64);
                g[S2] = addu(g[T5], (-2i64) as u64);
                g[S3] = addu(g[T5], u64::MAX);
                // a1 = r, a0 = c (four columns a pass), v0 = the pixel, v1
                // = the count; t5/s3/s2/s1 the left bounds of the four
                // columns, a3 (then a3 - 1 .. - 3) the right ones.
                loop {
                    // L_8000F6A8
                    g[A0] = 0;
                    loop {
                        'b_8000F70C: {
                            'b_8000F708: {
                                // L_8000F6AC
                                g[AT] = slt(g[A0], g[T5]);
                                g[T7] = addu(g[A3], u64::MAX);
                                if g[AT] == 0 {
                                    g[AT] = slt(g[A0], g[A3]);
                                    g[T9] = subu(g[T2], g[A2]);
                                    if g[AT] != 0 {
                                        g[AT] = slt(g[A1], g[T9]);
                                        let c0 = g[AT] == 0;
                                        g[AT] = slt(g[A1], g[T0]);
                                        if c0 && g[AT] != 0 {
                                            g[AT] = sltu(g[V0], g[T4]);
                                            // L_8000F6E4
                                            let c1 = g[AT] != 0;
                                            g[AT] = sltu(g[V0], g[T1]);
                                            if c1 {
                                                break 'b_8000F708;
                                            }
                                            if g[AT] == 0 {
                                                g[AT] = slt(g[A0], g[S3]);
                                                break 'b_8000F70C;
                                            }
                                            g[T6] = lhu(m, g[V0], 0);
                                            g[AT] = slt(g[T6], g[T3]);
                                            if g[AT] == 0 {
                                                g[AT] = slt(g[A0], g[S3]);
                                                break 'b_8000F70C;
                                            }
                                            g[V1] = addu(g[V1], 1);
                                            break 'b_8000F708;
                                        }
                                    }
                                }
                                // L_8000F6D8
                                g[V1] = addu(g[V1], 1);
                            }
                            // L_8000F708
                            g[AT] = slt(g[A0], g[S3]);
                        }
                        'b_8000F768: {
                            'b_8000F764: {
                                // L_8000F70C
                                g[V0] = addu(g[V0], 2);
                                if g[AT] == 0 {
                                    g[AT] = slt(g[A0], g[T7]);
                                    g[T8] = subu(g[T2], g[A2]);
                                    if g[AT] != 0 {
                                        g[AT] = slt(g[A1], g[T8]);
                                        let c2 = g[AT] == 0;
                                        g[AT] = slt(g[A1], g[T0]);
                                        if c2 && g[AT] != 0 {
                                            g[AT] = sltu(g[V0], g[T4]);
                                            // L_8000F740
                                            let c3 = g[AT] != 0;
                                            g[AT] = sltu(g[V0], g[T1]);
                                            if c3 {
                                                break 'b_8000F764;
                                            }
                                            if g[AT] == 0 {
                                                g[AT] = slt(g[A0], g[S2]);
                                                break 'b_8000F768;
                                            }
                                            g[T9] = lhu(m, g[V0], 0);
                                            g[AT] = slt(g[T9], g[T3]);
                                            if g[AT] == 0 {
                                                g[AT] = slt(g[A0], g[S2]);
                                                break 'b_8000F768;
                                            }
                                            g[V1] = addu(g[V1], 1);
                                            break 'b_8000F764;
                                        }
                                    }
                                }
                                // L_8000F734
                                g[V1] = addu(g[V1], 1);
                            }
                            // L_8000F764
                            g[AT] = slt(g[A0], g[S2]);
                        }
                        'b_8000F7C8: {
                            'b_8000F7C4: {
                                // L_8000F768
                                g[V0] = addu(g[V0], 2);
                                if g[AT] == 0 {
                                    g[T6] = addu(g[A3], (-2i64) as u64);
                                    g[AT] = slt(g[A0], g[T6]);
                                    g[T7] = subu(g[T2], g[A2]);
                                    if g[AT] != 0 {
                                        g[AT] = slt(g[A1], g[T7]);
                                        let c4 = g[AT] == 0;
                                        g[AT] = slt(g[A1], g[T0]);
                                        if c4 && g[AT] != 0 {
                                            g[AT] = sltu(g[V0], g[T4]);
                                            // L_8000F7A0
                                            let c5 = g[AT] != 0;
                                            g[AT] = sltu(g[V0], g[T1]);
                                            if c5 {
                                                break 'b_8000F7C4;
                                            }
                                            if g[AT] == 0 {
                                                g[AT] = slt(g[A0], g[S1]);
                                                break 'b_8000F7C8;
                                            }
                                            g[T8] = lhu(m, g[V0], 0);
                                            g[AT] = slt(g[T8], g[T3]);
                                            if g[AT] == 0 {
                                                g[AT] = slt(g[A0], g[S1]);
                                                break 'b_8000F7C8;
                                            }
                                            g[V1] = addu(g[V1], 1);
                                            break 'b_8000F7C4;
                                        }
                                    }
                                }
                                // L_8000F794
                                g[V1] = addu(g[V1], 1);
                            }
                            // L_8000F7C4
                            g[AT] = slt(g[A0], g[S1]);
                        }
                        'b_8000F828: {
                            'b_8000F824: {
                                // L_8000F7C8
                                g[V0] = addu(g[V0], 2);
                                if g[AT] == 0 {
                                    g[T9] = addu(g[A3], (-3i64) as u64);
                                    g[AT] = slt(g[A0], g[T9]);
                                    g[T6] = subu(g[T2], g[A2]);
                                    if g[AT] != 0 {
                                        g[AT] = slt(g[A1], g[T6]);
                                        let c6 = g[AT] == 0;
                                        g[AT] = slt(g[A1], g[T0]);
                                        if c6 && g[AT] != 0 {
                                            g[AT] = sltu(g[V0], g[T4]);
                                            // L_8000F800
                                            let c7 = g[AT] != 0;
                                            g[AT] = sltu(g[V0], g[T1]);
                                            if c7 {
                                                break 'b_8000F824;
                                            }
                                            if g[AT] == 0 {
                                                g[A0] = addu(g[A0], 4);
                                                break 'b_8000F828;
                                            }
                                            g[T7] = lhu(m, g[V0], 0);
                                            g[AT] = slt(g[T7], g[T3]);
                                            if g[AT] == 0 {
                                                g[A0] = addu(g[A0], 4);
                                                break 'b_8000F828;
                                            }
                                            g[V1] = addu(g[V1], 1);
                                            break 'b_8000F824;
                                        }
                                    }
                                }
                                // L_8000F7F4
                                g[V1] = addu(g[V1], 1);
                            }
                            // L_8000F824
                            g[A0] = addu(g[A0], 4);
                        }
                        // L_8000F828
                        g[V0] = addu(g[V0], 2);
                        if g[A0] == g[S0] {
                            break;
                        }
                    }
                    g[A1] = addu(g[A1], 1);
                    g[V0] = addu(g[V0], g[S4]);
                    if g[A1] == g[S0] {
                        break;
                    }
                }
                sw(m, g[FP], 0, g[V1]);
            } else {
                g[T8] = 0x32;
                sw(m, g[FP], 0, g[T8]);
            }
        }
        // L_8000F840
        g[T8] = li(0x8009_B81C);
        g[AT] = sltu(g[S7], g[T8]);
        g[S6] = addu(g[S6], 4);
        if g[AT] == 0 {
            break;
        }
    }
    // 2. Two lists of 20 points (a0 = -1000).
    g[A2] = li(0x800D_0000);
    g[A3] = li(0x800D_0000);
    g[T0] = li(0x800D_0000);
    g[T1] = li(0x800D_0000);
    g[T5] = li(0x800D_0000);
    g[T3] = li(0x800D_0000);
    g[T2] = li(0x800D_5B48);
    g[T3] = addu(g[T3], 0x5BE8);
    g[T5] = addu(g[T5], 0x5BE8);
    g[T1] = addu(g[T1], 0x5B98);
    g[T0] = addu(g[T0], 0x6020);
    g[A3] = addu(g[A3], 0x5AF8);
    g[A2] = addu(g[A2], 0x5FD0);
    g[V1] = 0;
    g[A0] = (-0x3E8i64) as u64;
    loop {
        // L_8000F894
        g[A1] = lw(m, g[A3], 0);
        g[A3] = addu(g[A3], 4);
        sw(m, g[A2], 0, g[A0]);
        g[T9] = addu(g[T2], g[V1]);
        if (g[A1] as i64) >= 0 {
            g[T6] = lw(m, g[T9], 0);
            let (lo, _) = multu(g[T6], g[S5]);
            g[T7] = lo;
            g[T8] = addu(g[A1], g[T7]);
            g[T9] = sll(g[T8], 1);
            g[V0] = addu(g[T9], g[T4]);
            g[T6] = lhu(m, g[V0], 0);
            sw(m, g[A2], 0, g[T6]);
        }
        // L_8000F8C8
        g[A1] = lw(m, g[T1], 0);
        g[A2] = addu(g[A2], 4);
        g[T1] = addu(g[T1], 4);
        sw(m, g[T0], 0, g[A0]);
        if (g[A1] as i64) >= 0 {
            g[T7] = addu(g[T3], g[V1]);
            g[T8] = lw(m, g[T7], 0);
            let (lo, _) = multu(g[T8], g[S5]);
            g[T9] = lo;
            g[T6] = addu(g[A1], g[T9]);
            g[T7] = sll(g[T6], 1);
            g[V0] = addu(g[T7], g[T4]);
            g[T8] = lhu(m, g[V0], 0);
            sw(m, g[T0], 0, g[T8]);
        }
        // L_8000F900
        g[AT] = sltu(g[T1], g[T5]);
        g[V1] = addu(g[V1], 4);
        g[T0] = addu(g[T0], 4);
        if g[AT] == 0 {
            break;
        }
    }
    // 3. 40 points, each where its byte is set.
    g[A2] = li(0x800D_0000);
    g[T3] = li(0x800D_0000);
    g[T2] = li(0x800D_0000);
    g[T1] = li(0x800D_60A0);
    g[T2] = addu(g[T2], 0x5E40);
    g[T3] = addu(g[T3], 0x5EE0);
    g[A2] = addu(g[A2], 0x5C38);
    g[A1] = 0;
    loop {
        // L_8000F934
        g[T9] = lbu(m, g[A2], 0);
        g[V1] = sll(g[A1], 2);
        g[T6] = addu(g[T2], g[V1]);
        if g[T9] == 0 {
            g[A1] = addu(g[A1], 1);
        } else {
            g[T0] = lw(m, g[T6], 0);
            g[A3] = addu(g[T1], g[V1]);
            sw(m, g[A3], 0, g[A0]);
            g[T7] = addu(g[T3], g[V1]);
            if (g[T0] as i64) >= 0 {
                g[T8] = lw(m, g[T7], 0);
                let (lo, _) = multu(g[T8], g[S5]);
                g[T9] = lo;
                g[T6] = addu(g[T0], g[T9]);
                g[T7] = sll(g[T6], 1);
                g[V0] = addu(g[T7], g[T4]);
                g[T8] = lhu(m, g[V0], 0);
                sw(m, g[A3], 0, g[T8]);
            }
            // L_8000F97C
            g[A1] = addu(g[A1], 1);
        }
        // L_8000F980
        g[AT] = slt(g[A1], 0x28);
        g[A2] = addu(g[A2], 1);
        if g[AT] == 0 {
            break;
        }
    }
    // 4. n points.
    g[V0] = li(0x800A_0000);
    g[V0] = lw(m, g[V0], -0x4794);
    g[A1] = li(0x800D_6070);
    g[V1] = 0;
    if (g[V0] as i64) > 0 {
        g[T9] = li(0x800D_0000);
        g[A2] = addu(g[T9], 0x5958);
        g[T6] = sll(g[V0], 2);
        g[T1] = li(0x800D_5988);
        g[T0] = addu(g[T6], g[A2]);
        loop {
            // L_8000F9BC
            g[A3] = lw(m, g[A2], 0);
            g[A2] = addu(g[A2], 4);
            g[AT] = sltu(g[A2], g[T0]);
            sw(m, g[A1], 0, g[A0]);
            if (g[A3] as i64) >= 0 {
                g[T7] = addu(g[T1], g[V1]);
                g[T8] = lw(m, g[T7], 0);
                let (lo, _) = multu(g[T8], g[S5]);
                g[T6] = lo;
                g[T9] = addu(g[A3], g[T6]);
                g[T7] = sll(g[T9], 1);
                g[V0] = addu(g[T7], g[T4]);
                g[T8] = lhu(m, g[V0], 0);
                sw(m, g[A1], 0, g[T8]);
            }
            // L_8000F9F4
            g[V1] = addu(g[V1], 4);
            g[A1] = addu(g[A1], 4);
            if g[AT] == 0 {
                break;
            }
        }
    }
    // L_8000FA00
    g[S0] = lw(m, g[SP], 4);
    g[S1] = lw(m, g[SP], 8);
    g[S2] = lw(m, g[SP], 0xC);
    g[S3] = lw(m, g[SP], 0x10);
    g[S4] = lw(m, g[SP], 0x14);
    g[S5] = lw(m, g[SP], 0x18);
    g[S6] = lw(m, g[SP], 0x1C);
    g[S7] = lw(m, g[SP], 0x20);
    g[FP] = lw(m, g[SP], 0x24);
    g[SP] = addu(g[SP], 0x68);
}

/// `func_800125E4(tex, i)` (texture load, **guess** at the purpose): if
/// `i < [tex + 4]` (signed; `i` is spilled to its home slot `sp + 4` and
/// read back), appends to the display list at [`DL_HEAD`] the commands
/// loading image `[tex + 8 + 4i]` for the format `[tex]` (word pairs, each
/// command's words stored in the C's order):
/// - 1 or 3: `FD900000 img`, `F5900000 07080200`, `E6000000 0`,
///   `F3000000 077FF200`, `F5800800 00080200`, `F2000000 000FC1FC`,
///   `E200001C 00404240`;
/// - 2: `FD500000 img`, `F5500000 07080200`, `E6000000 0`, `F3000000
///   073FF200`, `F5400800 00080200`, `F2000000 000FC0FC`, `E200001C
///   00404240`;
/// - 0: as 2 up to `F5400800 00080200`, then `F5400800 01180200`,
///   `F5400800 02280200`, `F5400800 03380200`, `F2000000` with `000FC0FC`,
///   `010FC0FC`, `020FC0FC`, `030FC0FC`, and `E200001C 00404240`;
/// - anything else: nothing.
///
/// Saves `s0` at `sp - 4` and restores it on every path. Leaves `t7 =
/// [tex + 4]` and `at` from the tests, and on a format's path `v1` = the
/// head, `v0` = the last command's address, `t5 = 0xF2000000`, `t6`..`t9`
/// the last words.
///
/// Domain: canonical pointers; the list in RDRAM, not overlapping `tex`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800125E4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-8i64) as u64);
    sw(m, g[SP], 4, g[S0]);
    sw(m, g[SP], 0xC, g[A1]);
    g[T7] = lw(m, g[A0], 4);
    g[S0] = g[A0];
    g[AT] = slt(g[A1], g[T7]);
    if g[AT] == 0 {
        g[S0] = lw(m, g[SP], 4);
    } else {
        'b_800129AC: {
            // The format [tex]; v1 = the list head.
            g[V0] = lw(m, g[A0], 0);
            g[AT] = 1;
            g[V1] = li(0x8012_17B0);
            if g[V0] != g[AT] {
                g[AT] = 3;
                if g[V0] != g[AT] {
                    g[AT] = 2;
                    g[V1] = li(0x8012_0000);
                    if g[V0] != g[AT] {
                        g[V1] = li(0x8012_0000);
                        if g[V0] != 0 {
                            break 'b_800129AC;
                        }
                        // Format 0: four tiles.
                        g[V1] = addu(g[V1], 0x17B0);
                        g[V0] = lw(m, g[V1], 0);
                        g[T9] = li(0xFD50_0000);
                        g[T5] = li(0xF200_0000);
                        g[T8] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T8]);
                        sw(m, g[V0], 0, g[T9]);
                        g[T6] = lw(m, g[SP], 0xC);
                        g[T7] = sll(g[T6], 2);
                        g[T8] = addu(g[S0], g[T7]);
                        g[T9] = lw(m, g[T8], 8);
                        g[T8] = li(0x708_0200);
                        sw(m, g[V0], 4, g[T9]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T7] = li(0xF550_0000);
                        g[T6] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T6]);
                        sw(m, g[V0], 4, g[T8]);
                        sw(m, g[V0], 0, g[T7]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T6] = li(0xE600_0000);
                        g[T8] = li(0xF300_0000);
                        g[T9] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T9]);
                        sw(m, g[V0], 4, 0);
                        sw(m, g[V0], 0, g[T6]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T9] = li(0x73F_F200);
                        g[T7] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T7]);
                        sw(m, g[V0], 4, g[T9]);
                        sw(m, g[V0], 0, g[T8]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T8] = li(0x8_0000);
                        g[T7] = li(0xF540_0000);
                        g[T6] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T6]);
                        g[T7] = g[T7] | 0x800;
                        g[T8] = g[T8] | 0x200;
                        sw(m, g[V0], 4, g[T8]);
                        sw(m, g[V0], 0, g[T7]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T6] = li(0x228_0200);
                        g[T9] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T9]);
                        g[T9] = addu(g[T7], 0);
                        sw(m, g[V0], 0, g[T7]);
                        g[T7] = li(0x118_0200);
                        sw(m, g[V0], 4, g[T7]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T8] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T8]);
                        sw(m, g[V0], 4, g[T6]);
                        sw(m, g[V0], 0, g[T9]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T7] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T7]);
                        sw(m, g[V0], 0, g[T9]);
                        g[T9] = li(0x338_0200);
                        sw(m, g[V0], 4, g[T9]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T7] = li(0xF_C0FC);
                        g[T6] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T6]);
                        sw(m, g[V0], 4, g[T7]);
                        sw(m, g[V0], 0, g[T5]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T9] = li(0x10F_C0FC);
                        g[T8] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T8]);
                        sw(m, g[V0], 4, g[T9]);
                        sw(m, g[V0], 0, g[T5]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T7] = li(0x20F_C0FC);
                        g[T6] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T6]);
                        sw(m, g[V0], 4, g[T7]);
                        sw(m, g[V0], 0, g[T5]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T9] = li(0x30F_C0FC);
                        g[T8] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T8]);
                        sw(m, g[V0], 4, g[T9]);
                        sw(m, g[V0], 0, g[T5]);
                        g[V0] = lw(m, g[V1], 0);
                        g[T8] = li(0x40_0000);
                        g[T7] = li(0xE200_0000);
                        g[T6] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T6]);
                        g[T7] = g[T7] | 0x1C;
                        g[T8] = g[T8] | 0x4240;
                        sw(m, g[V0], 4, g[T8]);
                        sw(m, g[V0], 0, g[T7]);
                        break 'b_800129AC;
                    }
                    // Format 2.
                    g[V1] = addu(g[V1], 0x17B0);
                    g[V0] = lw(m, g[V1], 0);
                    g[T7] = li(0xFD50_0000);
                    g[T5] = li(0xF200_0000);
                    g[T6] = addu(g[V0], 8);
                    sw(m, g[V1], 0, g[T6]);
                    sw(m, g[V0], 0, g[T7]);
                    g[T8] = lw(m, g[SP], 0xC);
                    g[T9] = sll(g[T8], 2);
                    g[T6] = addu(g[S0], g[T9]);
                    g[T7] = lw(m, g[T6], 8);
                    g[T6] = li(0x708_0200);
                    sw(m, g[V0], 4, g[T7]);
                    g[V0] = lw(m, g[V1], 0);
                    g[T9] = li(0xF550_0000);
                    g[T8] = addu(g[V0], 8);
                    sw(m, g[V1], 0, g[T8]);
                    sw(m, g[V0], 4, g[T6]);
                    sw(m, g[V0], 0, g[T9]);
                    g[V0] = lw(m, g[V1], 0);
                    g[T8] = li(0xE600_0000);
                    g[T6] = li(0xF300_0000);
                    g[T7] = addu(g[V0], 8);
                    sw(m, g[V1], 0, g[T7]);
                    sw(m, g[V0], 4, 0);
                    sw(m, g[V0], 0, g[T8]);
                    g[V0] = lw(m, g[V1], 0);
                    g[T7] = li(0x73F_F200);
                    g[T9] = addu(g[V0], 8);
                    sw(m, g[V1], 0, g[T9]);
                    sw(m, g[V0], 4, g[T7]);
                    sw(m, g[V0], 0, g[T6]);
                    g[V0] = lw(m, g[V1], 0);
                    g[T6] = li(0x8_0000);
                    g[T9] = li(0xF540_0000);
                    g[T8] = addu(g[V0], 8);
                    sw(m, g[V1], 0, g[T8]);
                    g[T9] = g[T9] | 0x800;
                    g[T6] = g[T6] | 0x200;
                    sw(m, g[V0], 4, g[T6]);
                    sw(m, g[V0], 0, g[T9]);
                    g[V0] = lw(m, g[V1], 0);
                    g[T8] = li(0xF_C0FC);
                    g[T7] = addu(g[V0], 8);
                    sw(m, g[V1], 0, g[T7]);
                    sw(m, g[V0], 4, g[T8]);
                    sw(m, g[V0], 0, g[T5]);
                    g[V0] = lw(m, g[V1], 0);
                    g[T7] = li(0x40_0000);
                    g[T6] = li(0xE200_0000);
                    g[T9] = addu(g[V0], 8);
                    sw(m, g[V1], 0, g[T9]);
                    g[T6] = g[T6] | 0x1C;
                    g[T7] = g[T7] | 0x4240;
                    sw(m, g[V0], 4, g[T7]);
                    sw(m, g[V0], 0, g[T6]);
                    break 'b_800129AC;
                }
            }
            // Formats 1 and 3.
            g[V0] = lw(m, g[V1], 0);
            g[T9] = li(0xFD90_0000);
            g[T5] = li(0xF200_0000);
            g[T8] = addu(g[V0], 8);
            sw(m, g[V1], 0, g[T8]);
            sw(m, g[V0], 0, g[T9]);
            g[T6] = lw(m, g[SP], 0xC);
            g[T7] = sll(g[T6], 2);
            g[T8] = addu(g[S0], g[T7]);
            g[T9] = lw(m, g[T8], 8);
            g[T8] = li(0x708_0200);
            sw(m, g[V0], 4, g[T9]);
            g[V0] = lw(m, g[V1], 0);
            g[T7] = li(0xF590_0000);
            g[T6] = addu(g[V0], 8);
            sw(m, g[V1], 0, g[T6]);
            sw(m, g[V0], 4, g[T8]);
            sw(m, g[V0], 0, g[T7]);
            g[V0] = lw(m, g[V1], 0);
            g[T6] = li(0xE600_0000);
            g[T8] = li(0xF300_0000);
            g[T9] = addu(g[V0], 8);
            sw(m, g[V1], 0, g[T9]);
            sw(m, g[V0], 4, 0);
            sw(m, g[V0], 0, g[T6]);
            g[V0] = lw(m, g[V1], 0);
            g[T9] = li(0x77F_F200);
            g[T7] = addu(g[V0], 8);
            sw(m, g[V1], 0, g[T7]);
            sw(m, g[V0], 4, g[T9]);
            sw(m, g[V0], 0, g[T8]);
            g[V0] = lw(m, g[V1], 0);
            g[T8] = li(0x8_0000);
            g[T7] = li(0xF580_0000);
            g[T6] = addu(g[V0], 8);
            sw(m, g[V1], 0, g[T6]);
            g[T7] = g[T7] | 0x800;
            g[T8] = g[T8] | 0x200;
            sw(m, g[V0], 4, g[T8]);
            sw(m, g[V0], 0, g[T7]);
            g[V0] = lw(m, g[V1], 0);
            g[T6] = li(0xF_C1FC);
            g[T9] = addu(g[V0], 8);
            sw(m, g[V1], 0, g[T9]);
            sw(m, g[V0], 4, g[T6]);
            sw(m, g[V0], 0, g[T5]);
            g[V0] = lw(m, g[V1], 0);
            g[T9] = li(0x40_0000);
            g[T8] = li(0xE200_0000);
            g[T7] = addu(g[V0], 8);
            sw(m, g[V1], 0, g[T7]);
            g[T8] = g[T8] | 0x1C;
            g[T9] = g[T9] | 0x4240;
            sw(m, g[V0], 4, g[T9]);
            sw(m, g[V0], 0, g[T8]);
        }
        g[S0] = lw(m, g[SP], 4);
    }
    g[SP] = addu(g[SP], 8);
}

/// `func_80014C98()`: append `gDPPipeSync` (`0xE7000000`, `0`) at the
/// display list pointer [`DL_HEAD`] and advance it by 8 (the pointer is
/// stored before the command, then the low word, then the high).
///
/// Leaves `a0 = DL_HEAD`, `v1` = the old pointer, `t6` = the new, `t7 =
/// 0xE7000000`.
///
/// Domain: the pointer canonical, 8 bytes at it in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80014C98(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A0] = li(DL_HEAD);
    g[V1] = lw(m, g[A0], 0);
    g[T7] = li(0xE700_0000);
    g[T6] = addu(g[V1], 8);
    sw(m, g[A0], 0, g[T6]);
    sw(m, g[V1], 4, 0);
    sw(m, g[V1], 0, g[T7]);
}

/// Append to the display list at `[0x80112C90]` (`dl` holds that address,
/// in the register the caller chose): `gSPMatrix(proj, projection | load)`
/// (`0xDA380003`) and `gSPForceMatrix(mvp)` (`G_MOVEMEM` `0xDC38000E`,
/// then `G_MOVEWORD` `0xDB0C0000`, `0x00010000`), advancing the pointer
/// before each command's words are written (the last command's second word
/// first). `proj` and `mvp` are loaded between the writes, from `[proj_base
/// + proj_off]` and `[mvp_base + mvp_off]`, into `t8` and `t1`.
///
/// Leaves `v1` = the last command's address, `t6`/`t9`/`t2` = the pointer
/// after each command, `t7`/`t0`/`t3`/`t4` = the command words.
fn projection_and_force_matrix(
    m: &mut Mem,
    g: &mut [u64; 32],
    dl: usize,
    (proj_base, proj_off): (usize, i32),
    (mvp_base, mvp_off): (usize, i32),
) {
    g[V1] = lw(m, g[dl], 0);
    g[T7] = li(0xDA38_0003);
    g[T6] = addu(g[V1], 8);
    sw(m, g[dl], 0, g[T6]);
    sw(m, g[V1], 0, g[T7]);
    g[T8] = lw(m, g[proj_base], proj_off);
    g[T0] = li(0xDC38_000E);
    sw(m, g[V1], 4, g[T8]);
    g[V1] = lw(m, g[dl], 0);
    g[T9] = addu(g[V1], 8);
    sw(m, g[dl], 0, g[T9]);
    sw(m, g[V1], 0, g[T0]);
    g[T1] = lw(m, g[mvp_base], mvp_off);
    g[T3] = li(0xDB0C_0000);
    g[T4] = li(0x1_0000);
    sw(m, g[V1], 4, g[T1]);
    g[V1] = lw(m, g[dl], 0);
    g[T2] = addu(g[V1], 8);
    sw(m, g[dl], 0, g[T2]);
    sw(m, g[V1], 4, g[T4]);
    sw(m, g[V1], 0, g[T3]);
}

/// `func_80034DA8()`: `[0x800A3FF8] = 0`, then append the projection and
/// forced matrices at `[0x801134D0]` and `[0x801134D4]` to the display list
/// ([`projection_and_force_matrix`]). Leaves `a2 = 0x80112C90`, `at =
/// 0x800A0000`, `t8`/`t1` = the two matrix addresses.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80034DA8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[A2] = li(0x8011_2C90);
    g[AT] = li(0x800A_0000);
    sw(&mut mem, g[AT], 0x3FF8, 0);
    // The matrices are read through t8/t1 = 0x80110000 + offset.
    g[T8] = li(0x8011_0000);
    g[T1] = li(0x8011_0000);
    projection_and_force_matrix(&mut mem, g, A2, (T8, 0x34D0), (T1, 0x34D4));
}

/// `func_8003527C(cam)`: append the projection and forced matrices at
/// `[cam + 0x34]` and `[cam + 0x38]` to the display list
/// ([`projection_and_force_matrix`]). Leaves `a3 = 0x80112C90`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003527C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[A3] = li(0x8011_2C90);
    projection_and_force_matrix(&mut mem, g, A3, (A0, 0x34), (A0, 0x38));
}

/// `func_800352E4()`: [`func_80034DA8`] without clearing `[0x800A3FF8]`.
/// Leaves `a2 = 0x80112C90`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800352E4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[A2] = li(0x8011_2C90);
    g[T8] = li(0x8011_0000);
    g[T1] = li(0x8011_0000);
    projection_and_force_matrix(&mut mem, g, A2, (T8, 0x34D0), (T1, 0x34D4));
}

/// One render-mode change tried by [`func_800356BC`]: if the first mode
/// word is `from`, set both words to `to`. The new words are built in the
/// temporaries `regs` (upper half, then `| lower`); the first one's upper
/// half is loaded even when `from` doesn't match (a delay slot).
struct ModeChange {
    from: u32,
    to: (u32, u32),
    regs: (usize, usize),
}

/// The four render modes (first word, second word) [`func_800356BC`] moves
/// between: two independent switches, "ZBZB" (z-buffering, **guess**) and
/// "AAEN" (anti-aliasing, **guess**).
const MODE_NONE: (u32, u32) = (0x0C08_4000, 0x0302_4000);

const MODE_ZB: (u32, u32) = (0x0044_2230, 0x0011_2230);

const MODE_AA: (u32, u32) = (0x0044_2048, 0x0011_2048);

const MODE_BOTH: (u32, u32) = (0x0044_2078, 0x0011_2078);

/// By switch (ZBZB, AAEN) and on (`a2 == 1`) or off: the two changes, in
/// the order the code tries them.
const MODE_CHANGES: [[[ModeChange; 2]; 2]; 2] = [
    [
        [
            ModeChange { from: MODE_ZB.0, to: MODE_NONE, regs: (T0, T1) },
            ModeChange { from: MODE_BOTH.0, to: MODE_AA, regs: (T2, T3) },
        ],
        [
            ModeChange { from: MODE_NONE.0, to: MODE_ZB, regs: (T6, T7) },
            ModeChange { from: MODE_AA.0, to: MODE_BOTH, regs: (T8, T9) },
        ],
    ],
    [
        [
            ModeChange { from: MODE_AA.0, to: MODE_NONE, regs: (T8, T9) },
            ModeChange { from: MODE_BOTH.0, to: MODE_ZB, regs: (T0, T1) },
        ],
        [
            ModeChange { from: MODE_NONE.0, to: MODE_AA, regs: (T4, T5) },
            ModeChange { from: MODE_ZB.0, to: MODE_BOTH, regs: (T6, T7) },
        ],
    ],
];

/// `func_800356BC(tag, _, a2)`: switch the render mode words at
/// `0x800A3DA0`/`0x800A3DA4`. Tag `"ZBZB"` or `"AAEN"` turns one of two
/// switches on (`a2 == 1`, full 64-bit compare) or off, moving between the
/// four modes of [`MODE_CHANGES`]. A mode that doesn't have the other
/// state is left alone (e.g. ZBZB on when already on). Tag `"Full"` sets
/// both words to `a2`. Any other tag does nothing. Spills `a1` to its slot
/// `[sp + 4]`.
///
/// Leaves `at` = the last constant compared (or `0x800A0000` after a
/// store), `v1 = 0x800A3DA0` (`0x800A0000` for an unknown tag), `v0` = the
/// old first word, and the changes' temporaries as [`ModeChange`] says.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800356BC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x5A42_5A42); // "ZBZB"
    sw(m, g[SP], 4, g[A1]);
    let switch = if g[A0] == g[AT] {
        g[AT] = 1;
        0
    } else {
        g[AT] = li(0x4141_454E); // "AAEN"
        let aaen = g[A0] == g[AT];
        g[AT] = 1;
        if !aaen {
            g[AT] = li(0x4675_6C6C); // "Full"
            g[V1] = li(0x800A_0000);
            if g[A0] == g[AT] {
                g[V1] = li(0x800A_3DA0);
                sw(m, g[V1], 0, g[A2]);
                g[AT] = li(0x800A_0000);
                sw(m, g[AT], 0x3DA4, g[A2]);
            }
            return;
        }
        1
    };
    let on = usize::from(g[A2] == g[AT]);
    g[V1] = li(0x800A_3DA0);
    g[V0] = lw(m, g[V1], 0);
    for c in &MODE_CHANGES[switch][on] {
        let (a, b) = c.regs;
        g[AT] = li(c.from);
        g[a] = li(c.to.0 & 0xFFFF_0000);
        if g[V0] == g[AT] {
            g[a] |= u64::from(c.to.0 & 0xFFFF);
            g[b] = li(c.to.1 & 0xFFFF_0000);
            sw(m, g[V1], 0, g[a]);
            g[b] |= u64::from(c.to.1 & 0xFFFF);
            g[AT] = li(0x800A_0000);
            sw(m, g[AT], 0x3DA4, g[b]);
            return;
        }
    }
}

/// The matrix [`func_80036A1C`] transforms boxes by (4x4, row-major, row
/// vectors), after the view copy at `0x80112E20`.
pub const CULL_MATRIX: u32 = 0x8011_2E60;

/// [`func_80036A1C`]'s side of a corner along x or y: with `w` in `f0` and
/// the coordinate in `f12`, `t4` = 1 above `|w|`, -1 below `-|w|`, else 0
/// (NaN coordinates give 0). The C negates `w` into `f8` when `w <= 0`,
/// into `f4` when `0 < w` and the coordinate isn't above it.
fn cull_side(g: &mut [u64; 32], f: &mut [Fpr; 32]) {
    if !(f[14].fl() < f[0].fl()) {
        f[8].set_fl(-f[0].fl());
        g[T4] = if f[8].fl() < f[12].fl() {
            1
        } else if f[12].fl() < f[0].fl() {
            u64::MAX
        } else {
            0
        };
    } else if f[0].fl() < f[12].fl() {
        g[T4] = 1;
    } else {
        f[4].set_fl(-f[0].fl());
        g[T4] = if f[12].fl() < f[4].fl() { u64::MAX } else { 0 };
    }
}

/// [`func_80036A1C`]'s per-axis update from the side in `t4`: `neg = -t4`;
/// a side (±1) clears the axis's all-inside flag and takes the axis
/// `state` from -2 (no corner yet) to that side, or to 0 if it held the
/// other side (the C sign-extends `t4` through `sll 16`/`sra 16` into
/// `tmp`); inside (0) sets the state to 0 and a nonzero flag to 1.
fn cull_update(g: &mut [u64; 32], state: usize, flag: usize, neg: usize, tmp: usize) {
    g[neg] = subu(0, g[T4]);
    if g[T4] != 0 {
        g[flag] = 0;
        if g[state] == g[neg] {
            g[state] = 0;
        } else if g[state] == g[S5] {
            g[state] = sll(g[T4], 16);
            g[tmp] = sra(g[state], 16);
            g[state] = g[tmp];
        }
    } else {
        g[state] = 0;
        if g[flag] != 0 {
            g[flag] = 1;
        }
    }
}

/// `func_80036A1C(box, all)` (box culling, **guess** at the purpose):
/// classifies the axis-aligned box `box` (min x, y, z at `+0..+8`, max at
/// `+0xC..+0x14`) against the clip volume of [`CULL_MATRIX`] `M`. Returns 0
/// (culled), 1 (visible), or with `all != 0`, 2 when every corner is
/// inside.
///
/// Returns 0 at once if `max < min` on x, then y, then z (compared in that
/// order; NaN compares false). Otherwise it stores the 24 products
/// `M[r][c] * v` (v each of the box's min and max coordinates, r < 3 its
/// axis, c < 4) in the frame, then for each corner k = 0..7 (x = max if bit 2, y bit 1,
/// z bit 0, else min) forms `w = M[3][3] + ((M[0][3] x + M[1][3] y) + M[2][3]
/// z)` and, the same way from its column, the coordinates X, Y, Z (each
/// `M[3][c] + ((.. x + .. y) + .. z)`). Per axis there is a state (-2 at
/// the start) and an all-inside flag (-1 if `all`, else 0):
/// - x and y: the side is 1 if the coordinate is above `|w|`, -1 below
///   `-|w|`, else 0 ([`cull_side`]);
/// - z: -1 if `w <= 0` (Z isn't computed), else 1 if `w < Z`, else 0;
/// - the state and flag update as in [`cull_update`]; an axis whose state
///   and flag are both 0 is skipped from then on (neither can change).
///
/// Every product is stored to the frame and reloaded before it is added,
/// so none can fuse into an add.
///
/// It returns 2 if all three flags are nonzero, else 1 if all three states
/// are 0, else 0. So a box is culled when all its corners are on one side
/// of a plane (only then does an axis state stay ±1), the near "plane"
/// being `w <= 0` and the far one `Z > w`.
///
/// Saves `s0`..`s5` at `sp - 0xA8..-0x94` and restores them; the products
/// are at `sp - 0x64..-0x08` (w, z, y, x tables of 6 each: min then max),
/// and `sp - 0x84` is a spill slot for `M[2][1]`, `M[0][2]`, `M[1][2]`,
/// `M[2][2]`, `M[0][3]` in turn. On the early returns it leaves `f0`/`f4`
/// (`f6`/`f8`, `f10`) the pair compared last. Otherwise `t0 = at = 8`,
/// `t5 = CULL_MATRIX`, `f2 = M[3][3]`, `f14 = 0.0`, `a1`/`a2`/`a3` the
/// states and `v1`/`a0` the y/z flags (as the path left them; `v0` is the
/// result), `t1`..`t3` = 12 × the last corner's bits, and `t4`, `t6`..`t9`,
/// `f0`, `f4`..`f12` from the last corner's path.
///
/// Domain: `box` a canonical pointer to 24 bytes. The coordinates compared
/// before an early return may be anything; past the tests, the box and
/// `M`'s entries, products and sums not NaN where they are computed, and
/// `w` not NaN wherever it is negated or reaches Z.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80036A1C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[SP] = addu(g[SP], (-0xB0i64) as u64);
    sw(m, g[SP], 0x1C, g[S5]);
    sw(m, g[SP], 0x18, g[S4]);
    sw(m, g[SP], 0x14, g[S3]);
    sw(m, g[SP], 0x10, g[S2]);
    sw(m, g[SP], 0xC, g[S1]);
    sw(m, g[SP], 8, g[S0]);
    'done: {
        // max < min on an axis: culled.
        f[4].set_u32l(lw(m, g[A0], 0xC) as u32);
        f[0].set_u32l(lw(m, g[A0], 0) as u32);
        if f[4].fl() < f[0].fl() {
            g[V0] = 0;
            break 'done;
        }
        f[6].set_u32l(lw(m, g[A0], 0x10) as u32);
        f[8].set_u32l(lw(m, g[A0], 4) as u32);
        if f[6].fl() < f[8].fl() {
            g[V0] = 0;
            break 'done;
        }
        f[10].set_u32l(lw(m, g[A0], 0x14) as u32);
        f[4].set_u32l(lw(m, g[A0], 8) as u32);
        g[T5] = li(0x8011_2E60);
        let flipped = f[10].fl() < f[4].fl();
        g[A2] = (-2i64) as u64;
        g[A3] = (-2i64) as u64;
        g[T0] = 0;
        g[S5] = (-2i64) as u64;
        if flipped {
            g[V0] = 0;
            break 'done;
        }
        // The products: x table at sp + 0x94 (column 0), y at + 0x7C, z at
        // + 0x64, w at + 0x4C; each min x, y, z then max x, y, z.
        f[2].set_u32l(lw(m, g[T5], 0) as u32);
        f[12].set_u32l(lw(m, g[T5], 0x10) as u32);
        f[14].set_u32l(lw(m, g[T5], 0x20) as u32);
        f[6].set_fl(f[2].fl() * f[0].fl());
        f[16].set_u32l(lw(m, g[T5], 4) as u32);
        f[18].set_u32l(lw(m, g[T5], 0x14) as u32);
        f[0].set_u32l(lw(m, g[T5], 0x1C) as u32);
        g[V0] = 0;
        g[V1] = 0;
        g[S4] = addu(g[SP], 0x64);
        sw(m, g[SP], 0x94, u64::from(f[6].u32l()));
        f[8].set_u32l(lw(m, g[A0], 0xC) as u32);
        g[S3] = addu(g[SP], 0x7C);
        g[S2] = addu(g[SP], 0x94);
        f[10].set_fl(f[2].fl() * f[8].fl());
        f[2].set_u32l(lw(m, g[T5], 0x2C) as u32);
        g[S1] = 0xC;
        g[S0] = addu(g[SP], 0x4C);
        sw(m, g[SP], 0xA0, u64::from(f[10].u32l()));
        f[4].set_u32l(lw(m, g[A0], 4) as u32);
        f[6].set_fl(f[12].fl() * f[4].fl());
        sw(m, g[SP], 0x98, u64::from(f[6].u32l()));
        f[8].set_u32l(lw(m, g[A0], 0x10) as u32);
        f[10].set_fl(f[12].fl() * f[8].fl());
        sw(m, g[SP], 0xA4, u64::from(f[10].u32l()));
        f[4].set_u32l(lw(m, g[A0], 8) as u32);
        f[6].set_fl(f[14].fl() * f[4].fl());
        sw(m, g[SP], 0x9C, u64::from(f[6].u32l()));
        f[8].set_u32l(lw(m, g[A0], 0x14) as u32);
        f[10].set_fl(f[14].fl() * f[8].fl());
        f[14].set_u32l(0);
        sw(m, g[SP], 0xA8, u64::from(f[10].u32l()));
        f[4].set_u32l(lw(m, g[A0], 0) as u32);
        f[6].set_fl(f[16].fl() * f[4].fl());
        sw(m, g[SP], 0x7C, u64::from(f[6].u32l()));
        f[8].set_u32l(lw(m, g[A0], 0xC) as u32);
        f[10].set_fl(f[16].fl() * f[8].fl());
        sw(m, g[SP], 0x88, u64::from(f[10].u32l()));
        f[4].set_u32l(lw(m, g[A0], 4) as u32);
        f[6].set_fl(f[18].fl() * f[4].fl());
        f[4].set_u32l(lw(m, g[T5], 0x24) as u32);
        sw(m, g[SP], 0x80, u64::from(f[6].u32l()));
        f[8].set_u32l(lw(m, g[A0], 0x10) as u32);
        sw(m, g[SP], 0x2C, u64::from(f[4].u32l()));
        f[6].set_u32l(lw(m, g[SP], 0x2C) as u32);
        f[10].set_fl(f[18].fl() * f[8].fl());
        sw(m, g[SP], 0x8C, u64::from(f[10].u32l()));
        f[8].set_u32l(lw(m, g[A0], 8) as u32);
        f[10].set_fl(f[6].fl() * f[8].fl());
        sw(m, g[SP], 0x84, u64::from(f[10].u32l()));
        f[4].set_u32l(lw(m, g[A0], 0x14) as u32);
        f[10].set_u32l(lw(m, g[T5], 8) as u32);
        f[8].set_fl(f[6].fl() * f[4].fl());
        sw(m, g[SP], 0x2C, u64::from(f[10].u32l()));
        f[6].set_u32l(lw(m, g[SP], 0x2C) as u32);
        sw(m, g[SP], 0x90, u64::from(f[8].u32l()));
        f[4].set_u32l(lw(m, g[A0], 0) as u32);
        f[8].set_fl(f[6].fl() * f[4].fl());
        sw(m, g[SP], 0x64, u64::from(f[8].u32l()));
        f[10].set_u32l(lw(m, g[A0], 0xC) as u32);
        f[8].set_u32l(lw(m, g[T5], 0x18) as u32);
        f[4].set_fl(f[6].fl() * f[10].fl());
        sw(m, g[SP], 0x2C, u64::from(f[8].u32l()));
        f[6].set_u32l(lw(m, g[SP], 0x2C) as u32);
        sw(m, g[SP], 0x70, u64::from(f[4].u32l()));
        f[10].set_u32l(lw(m, g[A0], 4) as u32);
        f[4].set_fl(f[6].fl() * f[10].fl());
        sw(m, g[SP], 0x68, u64::from(f[4].u32l()));
        f[8].set_u32l(lw(m, g[A0], 0x10) as u32);
        f[4].set_u32l(lw(m, g[T5], 0x28) as u32);
        f[10].set_fl(f[6].fl() * f[8].fl());
        sw(m, g[SP], 0x2C, u64::from(f[4].u32l()));
        f[6].set_u32l(lw(m, g[SP], 0x2C) as u32);
        sw(m, g[SP], 0x74, u64::from(f[10].u32l()));
        f[8].set_u32l(lw(m, g[A0], 8) as u32);
        f[10].set_fl(f[6].fl() * f[8].fl());
        sw(m, g[SP], 0x6C, u64::from(f[10].u32l()));
        f[4].set_u32l(lw(m, g[A0], 0x14) as u32);
        f[10].set_u32l(lw(m, g[T5], 0xC) as u32);
        f[8].set_fl(f[6].fl() * f[4].fl());
        sw(m, g[SP], 0x2C, u64::from(f[10].u32l()));
        f[6].set_u32l(lw(m, g[SP], 0x2C) as u32);
        sw(m, g[SP], 0x78, u64::from(f[8].u32l()));
        f[4].set_u32l(lw(m, g[A0], 0) as u32);
        f[8].set_fl(f[6].fl() * f[4].fl());
        sw(m, g[SP], 0x4C, u64::from(f[8].u32l()));
        f[10].set_u32l(lw(m, g[A0], 0xC) as u32);
        f[4].set_fl(f[6].fl() * f[10].fl());
        sw(m, g[SP], 0x58, u64::from(f[4].u32l()));
        f[8].set_u32l(lw(m, g[A0], 4) as u32);
        f[6].set_fl(f[0].fl() * f[8].fl());
        sw(m, g[SP], 0x50, u64::from(f[6].u32l()));
        f[10].set_u32l(lw(m, g[A0], 0x10) as u32);
        f[4].set_fl(f[0].fl() * f[10].fl());
        sw(m, g[SP], 0x5C, u64::from(f[4].u32l()));
        f[8].set_u32l(lw(m, g[A0], 8) as u32);
        f[6].set_fl(f[2].fl() * f[8].fl());
        sw(m, g[SP], 0x54, u64::from(f[6].u32l()));
        f[10].set_u32l(lw(m, g[A0], 0x14) as u32);
        g[A0] = 0;
        f[4].set_fl(f[2].fl() * f[10].fl());
        sw(m, g[SP], 0x60, u64::from(f[4].u32l()));
        // The all-inside flags: v0 (x), v1 (y), a0 (z).
        if g[A1] != 0 {
            g[V0] = u64::MAX;
            g[V1] = u64::MAX;
            g[A0] = u64::MAX;
        }
        // The states: a1 (x), a2 (y), a3 (z); t0 = the corner.
        g[A1] = (-2i64) as u64;
        f[2].set_u32l(lw(m, g[T5], 0x3C) as u32);
        g[T6] = g[T0] & 4;
        loop {
            // t1, t2, t3 = the table offsets (12 × bit) for x, y, z.
            g[T7] = sra(g[T6], 2);
            let (lo, _) = multu(g[T7], g[S1]);
            g[T8] = g[T0] & 2;
            g[T9] = sra(g[T8], 1);
            g[T6] = g[T0] & 1;
            g[T0] = addu(g[T0], 1);
            g[AT] = 8;
            g[T1] = lo;
            g[T7] = addu(g[S0], g[T1]);
            f[8].set_u32l(lw(m, g[T7], 0) as u32);
            let (lo, _) = multu(g[T9], g[S1]);
            g[T2] = lo;
            g[T8] = addu(g[S0], g[T2]);
            f[6].set_u32l(lw(m, g[T8], 4) as u32);
            let (lo, _) = multu(g[T6], g[S1]);
            g[T6] = addu(g[S2], g[T1]);
            f[10].set_fl(f[8].fl() + f[6].fl());
            g[T7] = addu(g[S2], g[T2]);
            g[T3] = lo;
            g[T9] = addu(g[S0], g[T3]);
            f[4].set_u32l(lw(m, g[T9], 8) as u32);
            f[8].set_fl(f[10].fl() + f[4].fl());
            f[0].set_fl(f[2].fl() + f[8].fl()); // w
            // x
            if g[V0] != 0 || g[A1] != 0 {
                f[6].set_u32l(lw(m, g[T6], 0) as u32);
                f[10].set_u32l(lw(m, g[T7], 4) as u32);
                g[T8] = addu(g[S2], g[T3]);
                f[8].set_u32l(lw(m, g[T8], 8) as u32);
                f[4].set_fl(f[6].fl() + f[10].fl());
                f[10].set_u32l(lw(m, g[T5], 0x30) as u32);
                f[6].set_fl(f[4].fl() + f[8].fl());
                f[12].set_fl(f[10].fl() + f[6].fl());
                cull_side(g, f);
                cull_update(g, A1, V0, T9, T6);
            }
            // y
            g[T7] = addu(g[S3], g[T1]);
            g[T8] = addu(g[S3], g[T2]);
            if g[V1] != 0 || g[A2] != 0 {
                f[6].set_u32l(lw(m, g[T8], 4) as u32);
                f[10].set_u32l(lw(m, g[T7], 0) as u32);
                g[T9] = addu(g[S3], g[T3]);
                f[8].set_u32l(lw(m, g[T9], 8) as u32);
                f[4].set_fl(f[10].fl() + f[6].fl());
                f[6].set_u32l(lw(m, g[T5], 0x34) as u32);
                f[10].set_fl(f[4].fl() + f[8].fl());
                f[12].set_fl(f[6].fl() + f[10].fl());
                cull_side(g, f);
                cull_update(g, A2, V1, T6, T7);
            }
            // z: -1 behind (w <= 0), 1 beyond w.
            if g[A0] != 0 || g[A3] != 0 {
                g[T4] = u64::MAX;
                g[T8] = addu(g[S4], g[T1]);
                g[T9] = addu(g[S4], g[T2]);
                if f[14].fl() < f[0].fl() {
                    f[6].set_u32l(lw(m, g[T8], 0) as u32);
                    f[10].set_u32l(lw(m, g[T9], 4) as u32);
                    g[T6] = addu(g[S4], g[T3]);
                    f[8].set_u32l(lw(m, g[T6], 8) as u32);
                    f[4].set_fl(f[6].fl() + f[10].fl());
                    f[10].set_u32l(lw(m, g[T5], 0x38) as u32);
                    f[6].set_fl(f[4].fl() + f[8].fl());
                    f[12].set_fl(f[10].fl() + f[6].fl());
                    g[T4] = if f[0].fl() < f[12].fl() { 1 } else { 0 };
                }
                cull_update(g, A3, A0, T7, T8);
            }
            if g[T0] == g[AT] {
                break;
            }
            g[T6] = g[T0] & 4;
        }
        g[V0] = if g[V0] != 0 && g[V1] != 0 && g[A0] != 0 {
            2
        } else if g[A1] == 0 && g[A2] == 0 && g[A3] == 0 {
            1
        } else {
            0
        };
    }
    g[S0] = lw(m, g[SP], 8);
    g[S1] = lw(m, g[SP], 0xC);
    g[S2] = lw(m, g[SP], 0x10);
    g[S3] = lw(m, g[SP], 0x14);
    g[S4] = lw(m, g[SP], 0x18);
    g[S5] = lw(m, g[SP], 0x1C);
    g[SP] = addu(g[SP], 0xB0);
}

/// Write one light at `[base]` as F3DEX2's `Light` wants it: the colour
/// from the low bytes of the three halfwords at `rgb` (bytes 1, 3, 5) into
/// `col` (`+0..+2`) and `colc` (`+4..+6`), through `v0`. `dir`, if given,
/// is three halfwords, negated and stored as bytes at `+0x10..+0x12`
/// through the temporary pairs `temps` (load, negation).
fn write_light(m: &mut Mem, g: &mut [u64; 32], base: usize, offs: [i32; 3], rgb: usize) {
    for (k, off) in offs.into_iter().enumerate() {
        g[V0] = lbu(m, g[rgb], 1 + 2 * k as i32);
        sb(m, g[base], off, g[V0]);
        sb(m, g[base], off + 4, g[V0]);
    }
}

fn write_direction(m: &mut Mem, g: &mut [u64; 32], base: usize, at: i32, dir: usize, temps: [(usize, usize); 3]) {
    for (k, (t, n)) in temps.into_iter().enumerate() {
        g[t] = lh(m, g[dir], 2 * k as i32);
        g[n] = subu(0, g[t]);
        sb(m, g[base], at + k as i32, g[n]);
    }
}

/// `func_80038E58(ambient, diffuse, dir)`: fill the `Lights1` at
/// `0x800A3DB0`: the ambient colour (`+0`, `+4`) and the light colour (`+8`,
/// `+0xC`) from the low bytes of each argument's three halfwords
/// ([`write_light`]), and the direction (`+0x10`) as the negated low bytes
/// of `dir`'s halfwords. Leaves `v1 = 0x800A3DB0`, `v0` = the last colour
/// byte, `t6`..`t1` = the direction halfwords and their negations.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038E58(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = li(0x800A_3DB0);
    write_light(m, g, V1, [0, 1, 2], A0);
    write_light(m, g, V1, [8, 9, 0xA], A1);
    write_direction(m, g, V1, 0x10, A2, [(T6, T7), (T8, T9), (T0, T1)]);
}

/// The twelve 0x28-byte `Lights2` slots at `0x800A3DC8` (ambient, two
/// lights) that [`func_80038ED0`], [`func_80038F68`] and [`func_80038FE8`]
/// fill, with a word per slot at `0x800A3FA8`: 1 for one light, 2 for two.
pub const LIGHT_SLOTS: u32 = 0x800A_3DC8;

/// The slot's address as the code computes it: `((i << 2) + i) << 3`.
fn light_slot(g: &mut [u64; 32], t: usize, base: usize, dst: usize) {
    g[t] = addu(g[t], g[A0]);
    g[base] = li(LIGHT_SLOTS);
    g[t] = sll(g[t], 3);
    g[dst] = addu(g[t], g[base]);
}

/// `func_80038ED0(i, ambient, diffuse, dir)`: for `0 <= i < 12` (signed),
/// fill light slot `i`'s ambient and first light like [`func_80038E58`];
/// anything else does nothing. Leaves `at` = the bound test, `t6` =
/// `4 * i` (then `40 * i`), `t7 = LIGHT_SLOTS`, `v1` = the slot, `v0`,
/// `t8`..`t3` as the writes left them.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038ED0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0xC);
    if (g[A0] as i64) < 0 {
        return;
    }
    g[T6] = sll(g[A0], 2);
    if g[AT] == 0 {
        return;
    }
    light_slot(g, T6, T7, V1);
    write_light(m, g, V1, [0, 1, 2], A1);
    write_light(m, g, V1, [8, 9, 0xA], A2);
    write_direction(m, g, V1, 0x10, A3, [(T8, T9), (T0, T1), (T2, T3)]);
}

/// `func_80038F68(i)`: for `0 <= i < 12`, copy the `Lights1` at `0x800A3DB0`
/// (24 bytes) into light slot `i` and set its word at `0x800A3FA8` to 1 (one
/// light). Leaves the copy's words in `at`, `t0`, `t3`, `t8 = 0x800A3DB0`,
/// `t1 = 0x800A3DB8`, `v0` = the slot, `t4 = 1`, `t5 = 4 * i`, `at =
/// 0x800A0000 + 4 * i`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038F68(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0xC);
    if (g[A0] as i64) < 0 {
        return;
    }
    g[T6] = sll(g[A0], 2);
    if g[AT] == 0 {
        return;
    }
    g[T8] = li(0x800A_3DB0);
    g[AT] = lw(m, g[T8], 0);
    light_slot(g, T6, T7, V0);
    sw(m, g[V0], 0, g[AT]);
    g[T0] = lw(m, g[T8], 4);
    g[T1] = li(0x800A_3DB8);
    sw(m, g[V0], 4, g[T0]);
    g[AT] = lw(m, g[T1], 0);
    g[T5] = sll(g[A0], 2);
    g[T4] = 1;
    sw(m, g[V0], 8, g[AT]);
    g[T3] = lw(m, g[T1], 4);
    sw(m, g[V0], 0xC, g[T3]);
    g[AT] = lw(m, g[T1], 8);
    sw(m, g[V0], 0x10, g[AT]);
    g[T3] = lw(m, g[T1], 0xC);
    g[AT] = addu(li(0x800A_0000), g[T5]);
    sw(m, g[V0], 0x14, g[T3]);
    sw(m, g[AT], 0x3FA8, g[T4]);
}

/// `func_80038FE8(i, on, colour, dir)`: for `0 <= i < 12`, light slot `i`'s
/// second light. With `on == 0` (64-bit) the slot's word at `0x800A3FA8`
/// becomes 1 (one light); otherwise 2, and the second light (`+0x18`) gets
/// `colour` and the negated `dir` like [`func_80038E58`].
///
/// QUIRK: the red byte goes to `+0x19`, where green overwrites it, and
/// `+0x18` is never written (`colc` at `+0x1C` gets all three right).
/// Leaves `at` = the bound test or `0x800A0000 + 4 * i`, `t6 = 1` or `t8 =
/// 2`, and with a light `v1` = the slot, `v0`, `t0`..`t7` as the writes
/// left them.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038FE8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0xC);
    if (g[A0] as i64) < 0 || g[AT] == 0 {
        return;
    }
    g[T8] = 2;
    if g[A1] == 0 {
        g[T7] = sll(g[A0], 2);
        g[AT] = addu(li(0x800A_0000), g[T7]);
        g[T6] = 1;
        sw(m, g[AT], 0x3FA8, g[T6]);
        return;
    }
    g[T9] = sll(g[A0], 2);
    g[AT] = addu(li(0x800A_0000), g[T9]);
    sw(m, g[AT], 0x3FA8, g[T8]);
    g[T0] = sll(g[A0], 2);
    g[V0] = lbu(m, g[A2], 1);
    light_slot(g, T0, T1, V1);
    sb(m, g[V1], 0x19, g[V0]); // QUIRK: +0x18 in a correct light
    sb(m, g[V1], 0x1C, g[V0]);
    g[V0] = lbu(m, g[A2], 3);
    sb(m, g[V1], 0x19, g[V0]);
    sb(m, g[V1], 0x1D, g[V0]);
    g[V0] = lbu(m, g[A2], 5);
    sb(m, g[V1], 0x1A, g[V0]);
    sb(m, g[V1], 0x1E, g[V0]);
    write_direction(m, g, V1, 0x20, A3, [(T2, T3), (T4, T5), (T6, T7)]);
}

/// `func_80039A30()` = `framebuffers_init`: the three framebuffer addresses
/// at `0x80114530..0x80114538` (NOTES.md, "Asset heap"): with `osMemSize`
/// (`[0x80000318]`) at least 8 MB (unsigned, of the sign-extended word) the
/// width is 640, 4 bytes a pixel and an extra `x = 2 * 4 * 640`, otherwise
/// 320, 2 bytes and `x = 0`; `fb[k - 1] = (osMemSize | 0x80000000) - k *
/// (w * 240 * bpp + x) + x` for `k = 1..3`. Each is stored without the
/// final `+ x`, read back and stored again with it.
///
/// The compiler repeats the size test before every choice, so `at` always
/// ends as it. Leaves `v0 = t3 = 3`, `v1` = osMemSize, `t1 = 0x8011453C`,
/// `t2` = the end of RDRAM, `t4 = 0x800000`, `t5 = 0xF0`, `a0`/`a1` = bpp
/// and width, `a2`/`a3` = 640 and 4 with 8 MB, `t0` = `x / 2`, and the
/// arithmetic in `t6`..`t9`. `s0` is saved and restored (sign-extended).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80039A30(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = lw(m, li(0x8000_0000), 0x318);
    g[SP] = addu(g[SP], (-8i64) as u64);
    sw(m, g[SP], 4, g[S0]);
    g[AT] = li(0x8000_0000);
    g[T1] = li(0x8011_4530);
    g[S0] = 3;
    g[V0] = 0;
    g[T5] = 0xF0;
    g[T4] = li(0x80_0000);
    g[T2] = g[V1] | g[AT];
    g[AT] = sltu(g[V1], g[T4]);
    let big = g[AT] == 0;
    // x / 2 = 4 * 640 (8 MB only), through a3 and a2.
    let half_extra = |g: &mut [u64; 32]| {
        g[A2] = 0x280;
        g[A3] = 4;
        g[T0] = multu(g[A3], g[A2]).0;
    };
    loop {
        g[A0] = if big { 4 } else { 2 };
        g[A1] = if big { 0x280 } else { 0x140 };
        g[T0] = 0;
        if big {
            half_extra(g);
        }
        g[T7] = multu(g[A1], g[T5]).0;
        g[T6] = sll(g[T0], 1);
        g[T3] = addu(g[V0], 1);
        g[V0] = g[T3];
        g[T0] = 0;
        g[T8] = multu(g[T7], g[A0]).0;
        g[T9] = addu(g[T6], g[T8]);
        g[T7] = multu(g[T9], g[T3]).0;
        g[T6] = subu(g[T2], g[T7]);
        sw(m, g[T1], 0, g[T6]);
        if big {
            half_extra(g);
        }
        g[T8] = lw(m, g[T1], 0);
        g[T9] = sll(g[T0], 1);
        g[T1] = addu(g[T1], 4);
        g[T7] = addu(g[T8], g[T9]);
        sw(m, g[T1], -4, g[T7]);
        if g[T3] == g[S0] {
            break;
        }
    }
    g[S0] = lw(m, g[SP], 4);
    g[SP] = addu(g[SP], 8);
}

/// `func_8003B324(dl, x0, x1, y0, y1, sa, sb, ta, tb, sn, cs)` (screen
/// quad, **guess** at the purpose): appends 14 F3DEX2 commands (0x70 bytes)
/// at `*dl` drawing the rectangle `x0..x1` by `y0..y1` (screen pixels,
/// integers), rotated about its centre by the angle whose sine and cosine
/// are `sn` and `cs` (floats, the last two stack arguments), with texture
/// coordinates `sa`, `sb`, `ta`, `tb`, and advances `*dl`. The commands:
/// `gSPVertex(0x800A4920, 4, 0)` (`0x01004008`); `G_MODIFYVTX` Z = 0 of
/// vertices 0..3 (`0x021C0000 | 2k`); XY of vertices 0..3 (`0x02180000 |
/// 2k`, `X << 16 | (Y & 0xFFFF)`); ST (`0x02140000 | 2k`, `s << 16 | (t &
/// 0xFFFF)`: vertex 0 (sa, ta), 1 (sb, ta), 2 (sb, tb), 3 (sa, tb)); and
/// `G_TRI2` `0x06000402, 0x00000604` (triangles 0 2 1 and 0 3 2). The
/// words are stored in that order, but the TRI2's second word first.
///
/// Vertices 0..3 are the corners (x0, y0), (x1, y0), (x1, y1), (x0, y1).
/// The centre is `cx = f32(x0 + x1) * 0.5`, `cy = f32(y0 + y1) * 0.5` (the
/// sums 32-bit), and for a corner `dx = f32(x) - cx`, `dy = f32(y) - cy`.
/// With the screen size `W`, `H` (s16 at `0x80114470`/`72`), `ws = W /
/// 320`, `hs = H / 240` and `r = hs / ws`: if `K < r - 1` or `K < -(r - 1)`
/// (`K` the float at `0x800AAB10`, 0.05 in the ROM),
/// - `X = trunc((((dx / ws) * cs + cx / ws) + (dy / hs) * sn) * ws + 0.5)`,
///   `Y = trunc((((dy / hs) * cs + cy / hs) + (dx / ws) * -sn) * hs + 0.5)`;
///
/// otherwise
/// - `X = trunc(((dx * cs + cx) + dy * sn) + 0.5)`, `Y = trunc(((dy * cs +
///   cy) + dx * -sn) + 0.5)`,
///
/// each in that order of operations (`-sn` negated first, `trunc` the C
/// cast: `0x80000000` out of range). The C computes shared terms once
/// (`dx / ws` of x0 serves vertices 0 and 3), spilling some to the frame.
///
/// Frame (`sp - 0x100`): `f20`..`f26` saved at `+8..+0x20` and restored;
/// spills at `+0x28..+0x58` and `+0xBC..+0xD0`; `dl` goes to `sp + 0`
/// (the caller's `a0` slot). Leaves `v0` = the new end, `t7 = dl`, `t0`,
/// `a0`, `a3`, `t3`, `t1` the addresses of the five commands before
/// it, `t4`/`t5` those of the XY of vertices 1 and 2, `v1 = tb`, `a1 =
/// sa << 16`, `t2 = ta & 0xFFFF`, `t8` vertex 3's ST word, `t9`/`t6` the
/// TRI2's words, and `at`, `f0`..`f18` from the path taken.
///
/// Domain: canonical `dl` and list pointers; no NaN operands: the screen
/// size, `K` (only compared), `sn`, `cs` and every intermediate (e.g. `W`
/// or `H` of 0 gives an infinite or NaN `r`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003B324(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[SP] = addu(g[SP], (-0x100i64) as u64);
    // f20 = cx, f22 = cy; the list starts with gSPVertex and four Z = 0.
    g[T6] = lw(m, g[SP], 0x110);
    g[T9] = addu(g[A2], g[A1]);
    f[4].set_u32l(g[T9] as u32);
    sd(m, g[SP], 0x20, f[26].u64);
    sd(m, g[SP], 0x18, f[24].u64);
    sd(m, g[SP], 0x10, f[22].u64);
    sd(m, g[SP], 8, f[20].u64);
    sw(m, g[SP], 0x100, g[A0]);
    g[V0] = lw(m, g[A0], 0);
    g[T7] = addu(g[T6], g[A3]);
    f[10].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    f[8].set_u32l(g[T7] as u32);
    g[AT] = li(0x3F00_0000);
    g[T9] = li(0x100_0000);
    g[V1] = g[V0];
    f[6].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
    f[14].set_u32l(g[AT] as u32);
    g[T9] = g[T9] | 0x4008;
    g[T6] = li(0x800A_4920);
    sw(m, g[V1], 0, g[T9]);
    g[V0] = addu(g[V0], 8);
    sw(m, g[V1], 4, g[T6]);
    g[T0] = g[V0];
    f[20].set_fl(f[10].fl() * f[14].fl());
    g[V0] = addu(g[V0], 8);
    g[T7] = li(0x21C_0000);
    g[T1] = g[V0];
    g[T8] = li(0x21C_0000);
    sw(m, g[T0], 0, g[T7]);
    sw(m, g[T0], 4, 0);
    g[T8] = g[T8] | 2;
    f[22].set_fl(f[6].fl() * f[14].fl());
    sw(m, g[T1], 0, g[T8]);
    g[V0] = addu(g[V0], 8);
    g[AT] = li(0x800B_0000);
    f[14].set_u32l(lw(m, g[AT], -0x54F0) as u32);
    g[V1] = g[V0];
    sw(m, g[T1], 4, 0);
    g[T9] = li(0x21C_0000);
    f[4].set_u32l(g[A1] as u32);
    g[T9] = g[T9] | 4;
    g[V0] = addu(g[V0], 8);
    f[8].set_u32l(g[A3] as u32);
    sw(m, g[V1], 0, g[T9]);
    sw(m, g[V1], 4, 0);
    g[A0] = g[V0];
    g[T6] = li(0x21C_0000);
    g[A1] = li(0x8011_0000);
    g[T6] = g[T6] | 6;
    f[10].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    g[A1] = addu(g[A1], 0x4470);
    // f24 = x0 - cx, f26 = y0 - cy; f12 = ws = W / 320, f2 = hs = H / 240,
    // f8 = r = hs / ws.
    sw(m, g[A0], 0, g[T6]);
    sw(m, g[A0], 4, 0);
    g[T7] = lh(m, g[A1], 2);
    f[6].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
    g[T8] = lh(m, g[A1], 0);
    f[4].set_u32l(g[T7] as u32);
    g[AT] = li(0x4370_0000);
    f[8].set_u32l(g[AT] as u32);
    f[24].set_fl(f[10].fl() - f[20].fl());
    g[AT] = li(0x43A0_0000);
    g[V0] = addu(g[V0], 8);
    f[10].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    g[T4] = addu(g[V0], 8);
    g[T3] = g[V0];
    g[T5] = addu(g[T4], 8);
    g[T7] = li(0x218_0000);
    f[26].set_fl(f[6].fl() - f[22].fl());
    f[6].set_u32l(g[T8] as u32);
    g[T8] = li(0x218_0000);
    f[2].set_fl(f[10].fl() / f[8].fl());
    f[10].set_u32l(g[AT] as u32);
    g[AT] = li(0x3F80_0000);
    g[T8] = g[T8] | 2;
    f[4].set_fl(fpu::cvt_s_w(f[6].u32l(), fpu::NEAREST));
    f[6].set_u32l(g[AT] as u32);
    g[V0] = addu(g[T5], 8);
    f[12].set_fl(f[4].fl() / f[10].fl());
    f[8].set_fl(f[2].fl() / f[12].fl());
    f[0].set_fl(f[8].fl() - f[6].fl());
    // |r - 1| > K (r = hs / ws): correct for the aspect ratio.
    let correct = if f[14].fl() < f[0].fl() {
        f[10].set_fl(f[20].fl() / f[12].fl());
        true
    } else {
        f[4].set_fl(-f[0].fl());
        let c = f[14].fl() < f[4].fl();
        f[14].set_u32l(lw(m, g[SP], 0x128) as u32);
        if c {
            f[10].set_fl(f[20].fl() / f[12].fl());
        }
        c
    };
    if correct {
        // Rotate in units of (ws, hs), then scale back.
        f[6].set_u32l(g[A2] as u32);
        g[T9] = lw(m, g[SP], 0x110);
        f[14].set_u32l(lw(m, g[SP], 0x128) as u32);
        f[4].set_fl(fpu::cvt_s_w(f[6].u32l(), fpu::NEAREST));
        g[AT] = li(0x3F00_0000);
        f[0].set_u32l(g[AT] as u32);
        f[8].set_fl(f[22].fl() / f[2].fl());
        sw(m, g[SP], 0xD0, u64::from(f[10].u32l()));
        f[10].set_fl(f[4].fl() - f[20].fl());
        f[4].set_u32l(g[T9] as u32);
        f[6].set_fl(f[26].fl() / f[2].fl());
        sw(m, g[SP], 0xCC, u64::from(f[8].u32l()));
        f[8].set_fl(f[10].fl() / f[12].fl());
        sw(m, g[SP], 0xC8, u64::from(f[6].u32l()));
        f[10].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
        f[6].set_fl(f[10].fl() - f[22].fl());
        sw(m, g[SP], 0xBC, u64::from(f[8].u32l()));
        sw(m, g[SP], 0x28, u64::from(f[8].u32l()));
        f[18].set_fl(f[24].fl() / f[12].fl());
        f[8].set_u32l(lw(m, g[SP], 0xC8) as u32);
        f[24].set_u32l(lw(m, g[SP], 0x28) as u32);
        sw(m, g[SP], 0x34, u64::from(f[8].u32l()));
        f[4].set_fl(f[6].fl() / f[2].fl());
        f[6].set_u32l(lw(m, g[SP], 0xD0) as u32);
        f[10].set_fl(f[18].fl() * f[14].fl());
        sw(m, g[SP], 0x30, u64::from(f[6].u32l()));
        f[10].set_fl(f[10].fl() + f[6].fl());
        sw(m, g[SP], 0xC4, u64::from(f[4].u32l()));
        sw(m, g[SP], 0x2C, u64::from(f[4].u32l()));
        f[4].set_u32l(lw(m, g[SP], 0x124) as u32);
        sw(m, g[SP], 0x54, u64::from(f[10].u32l()));
        f[26].set_u32l(lw(m, g[SP], 0x2C) as u32);
        f[6].set_fl(f[8].fl() * f[4].fl());
        f[8].set_fl(f[10].fl() + f[6].fl());
        sw(m, g[SP], 0x50, u64::from(f[6].u32l()));
        sw(m, g[SP], 0x38, u64::from(f[6].u32l()));
        f[8].set_fl(f[8].fl() * f[12].fl());
        f[8].set_fl(f[8].fl() + f[0].fl());
        f[8].set_u32l(fpu::trunc_w_s(f[8].fl()));
        g[A1] = s32(f[8].u32l()); // vertex 0 x
        f[8].set_u32l(lw(m, g[SP], 0x34) as u32);
        sw(m, g[SP], 0x34, u64::from(f[4].u32l()));
        f[4].set_u32l(lw(m, g[SP], 0xCC) as u32);
        f[8].set_fl(f[8].fl() * f[14].fl());
        f[8].set_fl(f[8].fl() + f[4].fl());
        sw(m, g[SP], 0x4C, u64::from(f[8].u32l()));
        f[8].set_u32l(lw(m, g[SP], 0x34) as u32);
        sw(m, g[SP], 0x34, u64::from(f[10].u32l()));
        f[16].set_fl(-f[8].fl());
        f[10].set_fl(f[18].fl() * f[16].fl());
        sw(m, g[SP], 0x44, u64::from(f[10].u32l()));
        f[6].set_u32l(lw(m, g[SP], 0x44) as u32);
        f[10].set_u32l(lw(m, g[SP], 0x4C) as u32);
        f[10].set_fl(f[10].fl() + f[6].fl());
        f[6].set_fl(f[10].fl() * f[2].fl());
        f[10].set_fl(f[6].fl() + f[0].fl());
        f[6].set_u32l(fpu::trunc_w_s(f[10].fl()));
        f[10].set_fl(f[24].fl() * f[14].fl());
        g[A3] = s32(f[6].u32l()); // vertex 0 y
        f[6].set_u32l(lw(m, g[SP], 0x30) as u32);
        f[18].set_fl(f[10].fl() + f[6].fl());
        f[10].set_u32l(lw(m, g[SP], 0x38) as u32);
        f[6].set_fl(f[18].fl() + f[10].fl());
        f[10].set_fl(f[6].fl() * f[12].fl());
        f[6].set_fl(f[10].fl() + f[0].fl());
        f[10].set_u32l(fpu::trunc_w_s(f[6].fl()));
        f[6].set_fl(f[24].fl() * f[16].fl());
        g[T0] = s32(f[10].u32l()); // vertex 1 x
        f[10].set_u32l(lw(m, g[SP], 0x4C) as u32);
        sw(m, g[SP], 0x58, u64::from(f[6].u32l()));
        f[6].set_u32l(lw(m, g[SP], 0x58) as u32);
        f[10].set_fl(f[10].fl() + f[6].fl());
        f[6].set_fl(f[10].fl() * f[2].fl());
        f[10].set_fl(f[6].fl() + f[0].fl());
        f[20].set_fl(f[26].fl() * f[8].fl());
        f[6].set_u32l(fpu::trunc_w_s(f[10].fl()));
        f[10].set_fl(f[18].fl() + f[20].fl());
        g[T1] = s32(f[6].u32l()); // vertex 1 y
        f[6].set_fl(f[10].fl() * f[12].fl());
        f[8].set_fl(f[6].fl() + f[0].fl());
        f[6].set_fl(f[26].fl() * f[14].fl());
        f[10].set_u32l(fpu::trunc_w_s(f[8].fl()));
        f[8].set_u32l(lw(m, g[SP], 0x58) as u32);
        f[22].set_fl(f[6].fl() + f[4].fl());
        g[A2] = s32(f[10].u32l()); // vertex 2 x
        f[10].set_fl(f[22].fl() + f[8].fl());
        f[6].set_fl(f[10].fl() * f[2].fl());
        f[10].set_u32l(lw(m, g[SP], 0x34) as u32);
        f[4].set_fl(f[6].fl() + f[0].fl());
        f[6].set_fl(f[10].fl() + f[20].fl());
        f[8].set_u32l(fpu::trunc_w_s(f[4].fl()));
        f[4].set_fl(f[6].fl() * f[12].fl());
        f[6].set_u32l(lw(m, g[SP], 0x44) as u32);
        g[T2] = s32(f[8].u32l()); // vertex 2 y
        f[8].set_fl(f[4].fl() + f[0].fl());
        f[4].set_fl(f[22].fl() + f[6].fl());
        f[10].set_u32l(fpu::trunc_w_s(f[8].fl()));
        f[8].set_fl(f[4].fl() * f[2].fl());
        g[V1] = s32(f[10].u32l()); // vertex 3 x
        f[10].set_fl(f[8].fl() + f[0].fl());
        f[6].set_u32l(fpu::trunc_w_s(f[10].fl()));
        g[A0] = s32(f[6].u32l()); // vertex 3 y
    } else {
        // Rotate in pixels: f14 = cs, f16 = -sn.
        f[4].set_fl(f[24].fl() * f[14].fl());
        f[10].set_u32l(lw(m, g[SP], 0x124) as u32);
        g[AT] = li(0x3F00_0000);
        f[0].set_u32l(g[AT] as u32);
        f[12].set_fl(f[26].fl() * f[10].fl());
        sw(m, g[SP], 0x34, u64::from(f[10].u32l()));
        f[16].set_fl(-f[10].fl());
        g[T6] = lw(m, g[SP], 0x110);
        f[8].set_fl(f[4].fl() + f[20].fl());
        f[6].set_fl(f[8].fl() + f[12].fl());
        sw(m, g[SP], 0x4C, u64::from(f[8].u32l()));
        sw(m, g[SP], 0x38, u64::from(f[8].u32l()));
        f[4].set_fl(f[6].fl() + f[0].fl());
        f[6].set_u32l(fpu::trunc_w_s(f[4].fl()));
        f[4].set_fl(f[26].fl() * f[14].fl());
        g[A1] = s32(f[6].u32l()); // vertex 0 x
        f[6].set_fl(f[24].fl() * f[16].fl());
        f[18].set_fl(f[4].fl() + f[22].fl());
        sw(m, g[SP], 0x48, u64::from(f[6].u32l()));
        f[4].set_u32l(lw(m, g[SP], 0x48) as u32);
        f[6].set_fl(f[18].fl() + f[4].fl());
        f[6].set_fl(f[6].fl() + f[0].fl());
        f[6].set_u32l(fpu::trunc_w_s(f[6].fl()));
        g[A3] = s32(f[6].u32l()); // vertex 0 y
        f[6].set_u32l(g[A2] as u32);
        f[6].set_fl(fpu::cvt_s_w(f[6].u32l(), fpu::NEAREST));
        f[2].set_fl(f[6].fl() - f[20].fl());
        f[6].set_fl(f[2].fl() * f[14].fl());
        f[6].set_fl(f[6].fl() + f[20].fl());
        f[8].set_fl(f[6].fl() + f[12].fl());
        sw(m, g[SP], 0x54, u64::from(f[6].u32l()));
        f[8].set_fl(f[8].fl() + f[0].fl());
        f[8].set_u32l(fpu::trunc_w_s(f[8].fl()));
        g[T0] = s32(f[8].u32l()); // vertex 1 x
        f[8].set_fl(f[2].fl() * f[16].fl());
        sw(m, g[SP], 0x50, u64::from(f[8].u32l()));
        f[8].set_u32l(lw(m, g[SP], 0x50) as u32);
        f[10].set_fl(f[18].fl() + f[8].fl());
        f[10].set_fl(f[10].fl() + f[0].fl());
        f[10].set_u32l(fpu::trunc_w_s(f[10].fl()));
        g[T1] = s32(f[10].u32l()); // vertex 1 y
        f[10].set_u32l(g[T6] as u32);
        f[10].set_fl(fpu::cvt_s_w(f[10].u32l(), fpu::NEAREST));
        f[2].set_fl(f[10].fl() - f[22].fl());
        f[10].set_u32l(lw(m, g[SP], 0x34) as u32);
        f[12].set_fl(f[2].fl() * f[10].fl());
        f[10].set_fl(f[6].fl() + f[12].fl());
        f[6].set_fl(f[10].fl() + f[0].fl());
        f[10].set_u32l(fpu::trunc_w_s(f[6].fl()));
        f[6].set_fl(f[2].fl() * f[14].fl());
        g[A2] = s32(f[10].u32l()); // vertex 2 x
        f[16].set_fl(f[6].fl() + f[22].fl());
        f[10].set_fl(f[16].fl() + f[8].fl());
        f[6].set_fl(f[10].fl() + f[0].fl());
        f[10].set_u32l(lw(m, g[SP], 0x38) as u32);
        f[8].set_u32l(fpu::trunc_w_s(f[6].fl()));
        f[6].set_fl(f[10].fl() + f[12].fl());
        g[T2] = s32(f[8].u32l()); // vertex 2 y
        f[8].set_fl(f[6].fl() + f[0].fl());
        f[6].set_fl(f[16].fl() + f[4].fl());
        f[10].set_u32l(fpu::trunc_w_s(f[8].fl()));
        f[8].set_fl(f[6].fl() + f[0].fl());
        g[V1] = s32(f[10].u32l()); // vertex 3 x
        f[10].set_u32l(fpu::trunc_w_s(f[8].fl()));
        g[A0] = s32(f[10].u32l()); // vertex 3 y
    }
    sw(m, g[T3], 0, g[T7]);
    // XY of vertices 0..3, then ST, then the two triangles.
    g[T9] = sll(g[A1], 16);
    g[T6] = g[A3] & 0xFFFF;
    g[T7] = g[T9] | g[T6];
    sw(m, g[T3], 4, g[T7]);
    sw(m, g[T4], 0, g[T8]);
    g[T7] = g[T1] & 0xFFFF;
    g[T6] = sll(g[T0], 16);
    g[T8] = g[T6] | g[T7];
    sw(m, g[T4], 4, g[T8]);
    g[T9] = li(0x218_0004);
    sw(m, g[T5], 0, g[T9]);
    g[T8] = g[T2] & 0xFFFF;
    g[T7] = sll(g[A2], 16);
    g[T9] = g[T7] | g[T8];
    sw(m, g[T5], 4, g[T9]);
    g[T6] = li(0x218_0006);
    sw(m, g[V0], 0, g[T6]);
    g[T9] = g[A0] & 0xFFFF;
    g[T8] = sll(g[V1], 16);
    g[T6] = g[T8] | g[T9];
    sw(m, g[V0], 4, g[T6]);
    g[T1] = addu(g[V0], 8);
    g[T7] = li(0x214_0000);
    sw(m, g[T1], 0, g[T7]);
    g[T8] = lw(m, g[SP], 0x114);
    g[T2] = lw(m, g[SP], 0x11C);
    g[T3] = addu(g[T1], 8);
    g[A1] = sll(g[T8], 16);
    g[T6] = g[T2] & 0xFFFF;
    g[T7] = g[A1] | g[T6];
    g[T8] = li(0x214_0000);
    sw(m, g[T1], 4, g[T7]);
    g[T8] = g[T8] | 2;
    sw(m, g[T3], 0, g[T8]);
    g[T9] = lw(m, g[SP], 0x118);
    g[T2] = g[T6];
    g[T8] = li(0x214_0000);
    g[T6] = sll(g[T9], 16);
    g[T7] = g[T6] | g[T2];
    sw(m, g[T3], 4, g[T7]);
    g[A3] = addu(g[T3], 8);
    g[T8] = g[T8] | 4;
    sw(m, g[A3], 0, g[T8]);
    g[V1] = lw(m, g[SP], 0x120);
    g[A0] = addu(g[A3], 8);
    g[T7] = li(0x214_0000);
    g[T9] = g[V1] & 0xFFFF;
    g[T6] = g[T6] | g[T9];
    sw(m, g[A3], 4, g[T6]);
    g[T8] = g[A1] | g[T9];
    g[T7] = g[T7] | 6;
    sw(m, g[A0], 0, g[T7]);
    sw(m, g[A0], 4, g[T8]);
    g[T0] = addu(g[A0], 8);
    g[T9] = li(0x600_0402);
    g[T6] = 0x604;
    sw(m, g[T0], 4, g[T6]);
    sw(m, g[T0], 0, g[T9]);
    // *dl = the end of the 14 commands.
    g[T7] = lw(m, g[SP], 0x100);
    g[V0] = addu(g[T0], 8);
    sw(m, g[T7], 0, g[V0]);
    f[26].u64 = ld(m, g[SP], 0x20);
    f[24].u64 = ld(m, g[SP], 0x18);
    f[22].u64 = ld(m, g[SP], 0x10);
    f[20].u64 = ld(m, g[SP], 8);
    g[SP] = addu(g[SP], 0x100);
}

/// Append a command to the display list whose pointer is at `[a1]`: load it
/// into `v1`, store `v1 + 8` (through `next`) back, then store `words`
/// (offset, register) in the compiler's order.
fn dl_append(m: &mut Mem, g: &mut [u64; 32], next: usize, words: [(i32, usize); 2]) {
    g[V1] = lw(m, g[A1], 0);
    g[next] = addu(g[V1], 8);
    sw(m, g[A1], 0, g[next]);
    for (off, r) in words {
        sw(m, g[V1], off, g[r]);
    }
}

/// `func_8003D370()`: append render state to the display list at
/// `[0x801217B0]`: `gSPTexture` (`0xD7000000`, `0x80008000`: scale 0x8000,
/// off), `gDPSetCombine` (`0xFCFFFFFF`, `0xFFFE793C`) and a
/// `G_SETOTHERMODE_L` (`0xE2001D00`, 0). Then, by the flags word
/// `[0x800A4960]`: bit 0 adds `gDPSetRenderMode` (`0xE200001C`,
/// `0x0F0A4000`), and bit 2 (the word read again) adds `0xE2001E01`, 0.
///
/// Leaves `a1 = 0x801217B0`, `v1` = the last command, `v0` = the flags,
/// `t4`/`t8` = the tested bits, and the words and pointers in `t0`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003D370(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A1] = li(0x8012_17B0);
    g[T8] = li(0x8000_8000);
    g[T7] = li(0xD700_0000);
    dl_append(m, g, T6, [(0, T7), (4, T8)]);
    g[T0] = li(0xFCFF_FFFF);
    g[T1] = li(0xFFFE_793C);
    dl_append(m, g, T9, [(0, T0), (4, T1)]);
    g[T3] = li(0xE200_1D00);
    g[V1] = lw(m, g[A1], 0);
    g[T2] = addu(g[V1], 8);
    sw(m, g[A1], 0, g[T2]);
    sw(m, g[V1], 4, 0);
    sw(m, g[V1], 0, g[T3]);
    g[V0] = lw(m, li(0x800A_0000), 0x4960);
    g[T6] = li(0xE200_001C);
    g[T4] = g[V0] & 1;
    g[T7] = li(0x0F0A_0000);
    if g[T4] != 0 {
        g[T7] |= 0x4000;
        dl_append(m, g, T5, [(4, T7), (0, T6)]);
        g[V0] = lw(m, li(0x800A_0000), 0x4960);
    }
    g[T8] = g[V0] & 4;
    if g[T8] != 0 {
        g[T0] = li(0xE200_1E01);
        g[V1] = lw(m, g[A1], 0);
        g[T9] = addu(g[V1], 8);
        sw(m, g[A1], 0, g[T9]);
        sw(m, g[V1], 4, 0);
        sw(m, g[V1], 0, g[T0]);
    }
}

/// `func_80084C30(type)`: fill the `OSTask` at `[0x801488C0]` (read again
/// before every field): `ucode_boot` (`+8`) = rspboot at `0x80097FF0`,
/// `ucode_boot_size` (`+0xC`) = `0xD0` (NOTES.md, "Code segment"),
/// `output_buff` (`+0x28`) and its size (`+0x2C`) from `[0x800DB894]` and
/// `[0x800DB898]`; if `(s16) type == 5`, `ucode` (`+0x10`) = the F3DEX2
/// text at `0x800980C0` and `ucode_data` (`+0x18`) = `0x800AE1D0`; and
/// `data_ptr` (`+0x30`) = `[0x801217B4]`. Spills `type` to its slot
/// `[sp]`.
///
/// Leaves `v0 = 0x801488C0`, `v1 = 0x80097FF0`, `at = 5`, `t0` = the data
/// pointer, and the pointers and values in `t1`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80084C30(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x8014_88C0);
    g[T8] = lw(m, g[V0], 0);
    g[V1] = li(0x8009_7FF0);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[T8], 8, g[V1]);
    g[T1] = lw(m, g[V0], 0);
    g[T9] = li(0x8009_80C0);
    g[T0] = subu(g[T9], g[V1]);
    sw(m, g[T1], 0xC, g[T0]);
    g[T2] = lw(m, li(0x800E_0000), -0x476C);
    g[T3] = lw(m, g[V0], 0);
    g[T6] = sll(g[A0], 16);
    sw(m, g[T3], 0x28, g[T2]);
    g[T5] = lw(m, g[V0], 0);
    g[T4] = lw(m, li(0x800E_0000), -0x4768);
    g[T7] = sra(g[T6], 16);
    g[AT] = 5;
    sw(m, g[T5], 0x2C, g[T4]);
    if g[T7] == g[AT] {
        g[T7] = lw(m, g[V0], 0);
        g[T6] = li(0x8009_80C0);
        sw(m, g[T7], 0x10, g[T6]);
        g[T9] = lw(m, g[V0], 0);
        g[T8] = li(0x800A_E1D0);
        sw(m, g[T9], 0x18, g[T8]);
    }
    g[T0] = lw(m, li(0x8012_0000), 0x17B4);
    g[T1] = lw(m, g[V0], 0);
    sw(m, g[T1], 0x30, g[T0]);
}

/// `func_80085AB4()` (screen brightness, **guess**): histograms the
/// framebuffer `[0x800A68B0]`, `W * H` RGBA5551 halfwords (the s16 screen
/// size at `0x80114470`, multiplied with `multu`; nothing if the product
/// isn't positive), into 32 word counters each at `0x801488C8` (bits 1..5,
/// blue) and `0x80148948` (bits 11..15, red), zeroed first. Returns `f0 =
/// sum` of `(f32(red[k] >> 2) / 19200) * f32(k - 1)` for k = 2..15, added
/// in k order to 0.0 (the counters shifted right as signed words).
///
/// The loop over k is IDO's: two bins, then four per pass with the
/// conversions interleaved, then four more; the port keeps it as it is.
///
/// Leaves `a1` = the red counters' `+0x40`, `v0 = 16`, `v1 = 16`, `f12 =
/// 19200.0`, `t2`/`t3` the counter bases, `a0 = W * H`, and the last
/// pixel's and bins' registers (`a2`, `a3`, `t0`..`t9`, `f2`..`f18`).
///
/// Domain: the framebuffer and counters in RDRAM, not overlapping; any
/// pixel values.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80085AB4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = li(0x800A_0000);
    g[A0] = li(0x8015_0000);
    g[A1] = li(0x8015_0000);
    g[V1] = li(0x8015_0000);
    g[V0] = lw(m, g[V0], 0x68B0);
    g[V1] = addu(g[V1], (-0x7638i64) as u64);
    g[A1] = addu(g[A1], (-0x76B8i64) as u64);
    g[A0] = addu(g[A0], (-0x7738i64) as u64);
    loop {
        g[A1] = addu(g[A1], 4);
        g[AT] = sltu(g[A1], g[V1]);
        g[A0] = addu(g[A0], 4);
        sw(m, g[A0], -4, 0);
        sw(m, g[A1], -4, 0);
        if g[AT] == 0 {
            break;
        }
    }
    g[A1] = li(0x8011_4470);
    g[T6] = lh(m, g[A1], 0);
    g[T7] = lh(m, g[A1], 2);
    g[T3] = li(0x8015_0000);
    g[T2] = li(0x8015_0000);
    let (lo, _) = multu(g[T6], g[T7]);
    g[T2] = addu(g[T2], (-0x7738i64) as u64);
    g[T3] = addu(g[T3], (-0x76B8i64) as u64);
    g[V1] = 0;
    g[A0] = lo;
    if (g[A0] as i64) > 0 {
        loop {
            g[A2] = lhu(m, g[V0], 0);
            g[V1] = addu(g[V1], 1);
            g[AT] = slt(g[V1], g[A0]);
            g[A1] = sra(g[A2], 1);
            g[A3] = sra(g[A2], 11);
            g[T8] = g[A1] & 0x1F;
            g[T9] = g[A3] & 0x1F;
            g[T4] = sll(g[T8], 2);
            g[T7] = sll(g[T9], 2);
            g[T0] = addu(g[T2], g[T4]);
            g[T1] = addu(g[T3], g[T7]);
            g[T5] = lw(m, g[T0], 0);
            g[T8] = lw(m, g[T1], 0);
            g[V0] = addu(g[V0], 2);
            g[T6] = addu(g[T5], 1);
            g[T9] = addu(g[T8], 1);
            sw(m, g[T0], 0, g[T6]);
            sw(m, g[T1], 0, g[T9]);
            if g[AT] == 0 {
                break;
            }
        }
    }
    g[T4] = li(0x8015_0000);
    g[T4] = lw(m, g[T4], -0x76B0);
    g[AT] = li(0x4696_0000);
    g[T7] = li(0x8015_0000);
    g[T5] = sra(g[T4], 2);
    f[4].set_u32l(g[T5] as u32);
    g[T7] = lw(m, g[T7], -0x76AC);
    f[12].set_u32l(g[AT] as u32);
    f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    g[T8] = sra(g[T7], 2);
    f[4].set_u32l(g[T8] as u32);
    g[T6] = 1;
    f[10].set_u32l(g[T6] as u32);
    g[T9] = 2;
    f[8].set_fl(f[6].fl() / f[12].fl());
    f[2].set_u32l(0 as u32);
    g[A1] = li(0x8014_8958);
    f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    g[V1] = 4;
    g[V0] = 0x10;
    f[16].set_fl(fpu::cvt_s_w(f[10].u32l(), fpu::NEAREST));
    f[10].set_fl(f[6].fl() / f[12].fl());
    f[18].set_fl(f[8].fl() * f[16].fl());
    f[8].set_u32l(g[T9] as u32);
    f[16].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
    f[0].set_fl(f[2].fl() + f[18].fl());
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[0].set_fl(f[0].fl() + f[18].fl());
    g[T4] = lw(m, g[A1], 0);
    g[V0] = addu(g[V0], (-4i64) as u64);
    g[T7] = lw(m, g[A1], 4);
    g[T5] = sra(g[T4], 2);
    f[16].set_u32l(g[T5] as u32);
    f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fpu::NEAREST));
    if g[V1] != g[V0] {
        loop {
            f[16].set_fl(f[18].fl() / f[12].fl());
            g[T9] = lw(m, g[A1], 8);
            f[14].set_u32l(g[V1] as u32);
            g[T6] = addu(g[V1], u64::MAX);
            f[10].set_u32l(g[T6] as u32);
            g[T6] = lw(m, g[A1], 0xC);
            g[T8] = sra(g[T7], 2);
            f[10].set_fl(fpu::cvt_s_w(f[10].u32l(), fpu::NEAREST));
            f[8].set_u32l(g[T8] as u32);
            g[T4] = sra(g[T9], 2);
            f[6].set_u32l(g[T4] as u32);
            g[T7] = sra(g[T6], 2);
            f[4].set_u32l(g[T7] as u32);
            g[T4] = lw(m, g[A1], 0x10);
            g[T5] = addu(g[V1], 1);
            f[2].set_u32l(g[T5] as u32);
            g[T8] = addu(g[V1], 2);
            f[18].set_u32l(g[T8] as u32);
            g[T5] = sra(g[T4], 2);
            g[V1] = addu(g[V1], 4);
            g[A1] = addu(g[A1], 0x10);
            g[T7] = lw(m, g[A1], 4);
            f[8].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
            f[10].set_fl(f[16].fl() * f[10].fl());
            f[16].set_u32l(g[T5] as u32);
            f[8].set_fl(f[8].fl() / f[12].fl());
            f[6].set_fl(fpu::cvt_s_w(f[6].u32l(), fpu::NEAREST));
            f[4].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
            f[6].set_fl(f[6].fl() / f[12].fl());
            f[14].set_fl(fpu::cvt_s_w(f[14].u32l(), fpu::NEAREST));
            f[2].set_fl(fpu::cvt_s_w(f[2].u32l(), fpu::NEAREST));
            f[4].set_fl(f[4].fl() / f[12].fl());
            f[14].set_fl(f[8].fl() * f[14].fl());
            f[0].set_fl(f[0].fl() + f[10].fl());
            f[2].set_fl(f[6].fl() * f[2].fl());
            f[6].set_fl(fpu::cvt_s_w(f[18].u32l(), fpu::NEAREST));
            f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fpu::NEAREST));
            f[6].set_fl(f[4].fl() * f[6].fl());
            f[0].set_fl(f[0].fl() + f[14].fl());
            f[0].set_fl(f[0].fl() + f[2].fl());
            f[0].set_fl(f[0].fl() + f[6].fl());
            if g[V1] == g[V0] {
                break;
            }
        }
    }
    g[T9] = lw(m, g[A1], 8);
    g[T8] = sra(g[T7], 2);
    f[8].set_u32l(g[T8] as u32);
    g[T4] = sra(g[T9], 2);
    f[6].set_u32l(g[T4] as u32);
    f[8].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
    g[T6] = addu(g[V1], u64::MAX);
    f[10].set_u32l(g[T6] as u32);
    g[T6] = lw(m, g[A1], 0xC);
    f[14].set_u32l(g[V1] as u32);
    f[16].set_fl(f[18].fl() / f[12].fl());
    g[T7] = sra(g[T6], 2);
    f[4].set_u32l(g[T7] as u32);
    g[T5] = addu(g[V1], 1);
    f[6].set_fl(fpu::cvt_s_w(f[6].u32l(), fpu::NEAREST));
    f[2].set_u32l(g[T5] as u32);
    g[T8] = addu(g[V1], 2);
    f[18].set_u32l(g[T8] as u32);
    g[V0] = addu(g[V0], 4);
    g[V1] = addu(g[V1], 4);
    g[A1] = addu(g[A1], 0x10);
    f[8].set_fl(f[8].fl() / f[12].fl());
    f[4].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    f[10].set_fl(fpu::cvt_s_w(f[10].u32l(), fpu::NEAREST));
    f[6].set_fl(f[6].fl() / f[12].fl());
    f[14].set_fl(fpu::cvt_s_w(f[14].u32l(), fpu::NEAREST));
    f[10].set_fl(f[16].fl() * f[10].fl());
    f[2].set_fl(fpu::cvt_s_w(f[2].u32l(), fpu::NEAREST));
    f[4].set_fl(f[4].fl() / f[12].fl());
    f[14].set_fl(f[8].fl() * f[14].fl());
    f[0].set_fl(f[0].fl() + f[10].fl());
    f[2].set_fl(f[6].fl() * f[2].fl());
    f[6].set_fl(fpu::cvt_s_w(f[18].u32l(), fpu::NEAREST));
    f[0].set_fl(f[0].fl() + f[14].fl());
    f[6].set_fl(f[4].fl() * f[6].fl());
    f[0].set_fl(f[0].fl() + f[2].fl());
    f[0].set_fl(f[0].fl() + f[6].fl());
}

/// `func_80086178(vp)` (viewport from a rectangle, **guess**): with the
/// words `x0, y0, x1, y1 = [vp + 0x20..0x2C]`, `sx = f64(W) / 320`, `sy =
/// f64(H) / 240` (the s16 screen size at `0x80114470`): `X0 = trunc(f64(f32
/// (x0)) * sx)`, `Y0` likewise with `sy`, `X1`, `Y1`; then the halfwords
/// `vp + 0x10 = 2 (X1 - X0) + 8`, `+0x18 = 2 (X0 + X1)`, `+0x12 = 2 (Y1 -
/// Y0) + 8`, `+0x1A = 2 (Y0 + Y1)` (32-bit, stored low halves). If the
/// word `[0x8009B7E8] != 0`: `+0x10 = 0x500`, `+0x12 = 0x3C0`, and `+0x18
/// = 0` if bit 0 of `[0x800D5710]` else `0x500`, `+0x1A = 0` if bit 1 else
/// `0x3C0`. Finally `+0x14 = 0x92`, `+0x1C = 0x36C`.
///
/// Conversions to int are the C's (`trunc.w.d`: `0x80000000` out of
/// range).
///
/// Leaves `v0`, `v1`, `a1`, `a2`, `t0`..`t9` from the path taken (`v0 =
/// 0x500`, `v1 = 0x3C0`, `t1 = 0x92`, `t2 = 0x36C`, `a1 = 0x800D5710`),
/// `a3 = 0x80114470`, and the doubles and conversions in `f0`..`f18`.
///
/// Domain: `vp` canonical; any values.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80086178(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[A3] = li(0x8011_4470);
    g[T6] = lh(m, g[A3], 0);
    g[AT] = li(0x4074_0000);
    f[8].set_u32h(g[AT] as u32); // f9
    f[4].set_u32l(g[T6] as u32);
    f[8].set_u32l(0 as u32);
    g[T7] = lw(m, g[A0], 0x20);
    f[6].set_d(f64::from(f[4].u32l() as i32));
    g[T9] = lh(m, g[A3], 2);
    f[10].set_u32l(g[T7] as u32);
    g[AT] = li(0x406E_0000);
    g[T0] = lw(m, g[A0], 0x24);
    f[0].set_d(f[6].d() / f[8].d());
    f[8].set_u32l(g[T9] as u32);
    g[T2] = lw(m, g[A0], 0x28);
    g[T4] = lw(m, g[A0], 0x2C);
    f[16].set_fl(fpu::cvt_s_w(f[10].u32l(), fpu::NEAREST));
    f[16].set_u32h(g[AT] as u32); // f17
    f[10].set_d(f64::from(f[8].u32l() as i32));
    f[18].set_d(f64::from(f[16].fl()));
    f[16].set_u32l(0 as u32);
    f[2].set_d(f[10].d() / f[16].d());
    f[4].set_d(f[18].d() * f[0].d());
    f[18].set_u32l(g[T0] as u32);
    f[16].set_u32l(g[T2] as u32);
    f[6].set_u32l(fpu::trunc_w_d(f[4].d()));
    f[4].set_fl(fpu::cvt_s_w(f[18].u32l(), fpu::NEAREST));
    g[V0] = s32(f[6].u32l());
    f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fpu::NEAREST));
    f[6].set_d(f64::from(f[4].fl()));
    f[4].set_d(f64::from(f[18].fl()));
    f[8].set_d(f[6].d() * f[2].d());
    f[10].set_u32l(fpu::trunc_w_d(f[8].d()));
    f[6].set_d(f[4].d() * f[0].d());
    g[V1] = s32(f[10].u32l());
    f[10].set_u32l(g[T4] as u32);
    f[16].set_fl(fpu::cvt_s_w(f[10].u32l(), fpu::NEAREST));
    f[8].set_u32l(fpu::trunc_w_d(f[6].d()));
    f[18].set_d(f64::from(f[16].fl()));
    g[A1] = s32(f[8].u32l());
    f[4].set_d(f[18].d() * f[2].d());
    g[T6] = subu(g[A1], g[V0]);
    g[T7] = sll(g[T6], 1);
    g[T2] = addu(g[V0], g[A1]);
    g[T8] = addu(g[T7], 8);
    g[T3] = sll(g[T2], 1);
    sh(m, g[A0], 0x10, g[T8]);
    f[6].set_u32l(fpu::trunc_w_d(f[4].d()));
    sh(m, g[A0], 0x18, g[T3]);
    g[T6] = li(0x800A_0000);
    g[A1] = li(0x800D_0000);
    g[A2] = s32(f[6].u32l());
    g[A1] = addu(g[A1], 0x5710);
    g[V0] = 0x500;
    g[T9] = subu(g[A2], g[V1]);
    g[T0] = sll(g[T9], 1);
    g[T4] = addu(g[V1], g[A2]);
    g[T1] = addu(g[T0], 8);
    g[T5] = sll(g[T4], 1);
    sh(m, g[A0], 0x12, g[T1]);
    sh(m, g[A0], 0x1A, g[T5]);
    g[T6] = lw(m, g[T6], -0x4818);
    g[V1] = 0x3C0;
    g[T1] = 0x92;
    g[T2] = 0x36C;
    if g[T6] != 0 {
        sh(m, g[A0], 0x10, g[V0]);
        sh(m, g[A0], 0x12, g[V1]);
        g[T7] = lw(m, g[A1], 0);
        g[T8] = g[T7] & 1;
        if g[T8] == 0 {
            sh(m, g[A0], 0x18, g[V0]);
        } else {
            sh(m, g[A0], 0x18, 0);
        }
        g[T9] = lw(m, g[A1], 0);
        g[T0] = g[T9] & 2;
        if g[T0] == 0 {
            sh(m, g[A0], 0x1A, g[V1]);
        } else {
            sh(m, g[A0], 0x1A, 0);
        }
    }
    sh(m, g[A0], 0x14, g[T1]);
    sh(m, g[A0], 0x1C, g[T2]);
}

/// `func_80087814(x0, y0, x1, y1)` (queue a rectangle, **guess**): the
/// arguments are s16 (spilled to their home slots `sp + 0..0xC` first).
/// If the count `n = [0x800A6978]` (s16) is below 31: the rectangle grows
/// by one on each side (as s16), `x0`, `y0` are clamped to at least 0 and
/// `x1`, `y1` to at most `W - 1`, `H - 1` (the s16 screen size at
/// `0x80114470`); if `x0 < x1` and `y0 < y1`, `n + 1` is stored and entry
/// `n` of the halfword quads at `0x80148B60` becomes `trunc(f64(x0) * sx),
/// trunc(f64(y0) * sy), trunc(f64(x1) * sx), trunc(f64(y1) * sy)` with `sx
/// = f64(W) / 320`, `sy = f64(H) / 240`.
///
/// Leaves `t2 = 0x800A6978`, `v0 = n`, `a0`..`a3` the adjusted rectangle,
/// and the path's `at`, `t0`..`t9`, `v1`, `f0`..`f18`.
///
/// Domain: any values.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80087814(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T2] = li(0x800A_6978);
    g[V0] = lh(m, g[T2], 0);
    sw(m, g[SP], 0, g[A0]);
    g[T6] = sll(g[A0], 16);
    g[A0] = sra(g[T6], 16);
    g[T6] = sll(g[A3], 16);
    sw(m, g[SP], 4, g[A1]);
    g[T8] = sll(g[A1], 16);
    sw(m, g[SP], 8, g[A2]);
    g[T4] = sll(g[A2], 16);
    sw(m, g[SP], 0xC, g[A3]);
    g[AT] = slt(g[V0], 0x1F);
    g[A3] = sra(g[T6], 16);
    g[A2] = sra(g[T4], 16);
    g[A1] = sra(g[T8], 16);
    if g[AT] != 0 {
        g[A0] = addu(g[A0], u64::MAX);
        g[T8] = sll(g[A0], 16);
        g[A0] = sra(g[T8], 16);
        g[A1] = addu(g[A1], u64::MAX);
        g[A2] = addu(g[A2], 1);
        g[A3] = addu(g[A3], 1);
        g[T4] = sll(g[A1], 16);
        g[T6] = sll(g[A2], 16);
        g[T8] = sll(g[A3], 16);
        g[A1] = sra(g[T4], 16);
        g[A2] = sra(g[T6], 16);
        g[A3] = sra(g[T8], 16);
        if (g[A0] as i64) < 0 {
            g[A0] = 0;
        }
        g[T3] = li(0x8011_0000);
        if (g[A1] as i64) < 0 {
            g[A1] = 0;
        }
        g[T3] = addu(g[T3], 0x4470);
        g[V1] = lh(m, g[T3], 0);
        g[T0] = addu(g[V1], u64::MAX);
        g[AT] = slt(g[T0], g[A2]);
        if g[AT] == 0 {
            g[T0] = lh(m, g[T3], 2);
        } else {
            g[A2] = sll(g[T0], 16);
            g[T4] = sra(g[A2], 16);
            g[A2] = g[T4];
            g[T0] = lh(m, g[T3], 2);
        }
        g[T1] = addu(g[T0], u64::MAX);
        g[AT] = slt(g[T1], g[A3]);
        if g[AT] == 0 {
            g[AT] = slt(g[A0], g[A2]);
        } else {
            g[A3] = sll(g[T1], 16);
            g[T5] = sra(g[A3], 16);
            g[A3] = g[T5];
            g[AT] = slt(g[A0], g[A2]);
        }
        let c0 = g[AT] != 0;
        g[AT] = slt(g[A1], g[A3]);
        if c0 && g[AT] != 0 {
            f[4].set_u32l(g[V1] as u32);
            g[AT] = li(0x4074_0000);
            f[8].set_u32h(g[AT] as u32); // f9
            f[6].set_d(f64::from(f[4].u32l() as i32));
            f[8].set_u32l(0 as u32);
            f[10].set_u32l(g[A0] as u32);
            g[AT] = li(0x406E_0000);
            f[10].set_u32h(g[AT] as u32); // f11
            f[0].set_d(f[6].d() / f[8].d());
            f[6].set_u32l(g[T0] as u32);
            g[T7] = li(0x8014_8B60);
            f[8].set_d(f64::from(f[6].u32l() as i32));
            g[T6] = sll(g[V0], 3);
            g[T1] = addu(g[T6], g[T7]);
            g[T4] = addu(g[V0], 1);
            f[16].set_d(f64::from(f[10].u32l() as i32));
            f[10].set_u32l(0 as u32);
            sh(m, g[T2], 0, g[T4]);
            f[2].set_d(f[8].d() / f[10].d());
            f[18].set_d(f[16].d() * f[0].d());
            f[16].set_u32l(g[A1] as u32);
            f[8].set_u32l(g[A2] as u32);
            f[10].set_d(f64::from(f[8].u32l() as i32));
            f[4].set_u32l(fpu::trunc_w_d(f[18].d()));
            f[18].set_d(f64::from(f[16].u32l() as i32));
            g[T9] = s32(f[4].u32l());
            sh(m, g[T1], 0, g[T9]);
            f[4].set_d(f[18].d() * f[2].d());
            f[6].set_u32l(fpu::trunc_w_d(f[4].d()));
            f[4].set_u32l(g[A3] as u32);
            f[16].set_d(f[10].d() * f[0].d());
            g[T5] = s32(f[6].u32l());
            sh(m, g[T1], 2, g[T5]);
            f[6].set_d(f64::from(f[4].u32l() as i32));
            f[18].set_u32l(fpu::trunc_w_d(f[16].d()));
            f[8].set_d(f[6].d() * f[2].d());
            g[T7] = s32(f[18].u32l());
            sh(m, g[T1], 4, g[T7]);
            f[10].set_u32l(fpu::trunc_w_d(f[8].d()));
            g[T9] = s32(f[10].u32l());
            sh(m, g[T1], 6, g[T9]);
        }
    }
}

/// `func_800879B8()` (draw the queued rectangles, **guess**): appends to
/// the display list `[0x801217B0]` a `gDPPipeSync` (`0xE7000000, 0`),
/// `0xE3000A01, 0`, `0xE200001C, 0x0F5A4240`, then a `G_FILLRECT` for each
/// of the `n = [0x800A6978]` entries of [`func_80087814`]'s queue (`0xF6 <<
/// 24 | ((x1 + 1) & 0x3FF) << 14 | ((y1 + 1) & 0x3FF) << 2` and `((x0 - 1)
/// & 0x3FF) << 14 | ((y0 - 1) & 0x3FF) << 2`, the halfwords signed and
/// re-read), then four more for a border: with the words `L, T, R, B =
/// [0x80120E10..1C]` scaled to the screen (`trunc(f32(v) * (W / 320.0))`,
/// or `H / 240` for T and B, in f32), rectangles of width 4 around each
/// edge; then another `gDPPipeSync`, and `n = 0`. The count is compared
/// unsigned (`sltu`) against the entries done.
///
/// Saves `s0` at `sp - 0x4C` and restores it; spills two words at `sp -
/// 0x44`/`- 0x48` and two list addresses at `sp - 0x30`/`- 0x34`.
///
/// Leaves `v1 = 0x801217B0`, `a0 = 0x800A6978`, `a1 = 0xE7000000`, and the
/// last commands' registers.
///
/// Domain: the list pointer and queue in RDRAM; any values.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800879B8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V1] = li(0x8012_17B0);
    g[V0] = lw(m, g[V1], 0);
    g[SP] = addu(g[SP], (-0x50i64) as u64);
    sw(m, g[SP], 4, g[S0]);
    g[T6] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T6]);
    g[A1] = li(0xE700_0000);
    sw(m, g[V0], 0, g[A1]);
    sw(m, g[V0], 4, 0);
    g[V0] = lw(m, g[V1], 0);
    g[T8] = li(0xE300_0A01);
    g[T7] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T7]);
    sw(m, g[V0], 4, 0);
    sw(m, g[V0], 0, g[T8]);
    g[V0] = lw(m, g[V1], 0);
    g[T7] = li(0xF5A_0000);
    g[T6] = li(0xE200_0000);
    g[T9] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T9]);
    g[A0] = li(0x800A_0000);
    g[T6] = g[T6] | 0x1C;
    g[T7] = g[T7] | 0x4240;
    g[A0] = addu(g[A0], 0x6978);
    sw(m, g[V0], 4, g[T7]);
    sw(m, g[V0], 0, g[T6]);
    g[T8] = lh(m, g[A0], 0);
    g[A3] = li(0x8014_8B60);
    g[T1] = 0;
    if g[T8] != 0 {
        g[S0] = li(0xF600_0000);
        g[T6] = lh(m, g[A3], 6);
        loop {
            g[V0] = lw(m, g[V1], 0);
            g[T1] = addu(g[T1], 1);
            g[T7] = addu(g[T6], 1);
            g[T8] = g[T7] & 0x3FF;
            g[T9] = addu(g[V0], 8);
            g[T7] = lh(m, g[A3], 4);
            sw(m, g[V1], 0, g[T9]);
            g[T9] = sll(g[T8], 2);
            g[T6] = g[T9] | g[S0];
            g[T8] = addu(g[T7], 1);
            g[T9] = g[T8] & 0x3FF;
            g[T7] = sll(g[T9], 14);
            g[T8] = g[T6] | g[T7];
            sw(m, g[V0], 0, g[T8]);
            g[T9] = lh(m, g[A3], 2);
            g[A3] = addu(g[A3], 8);
            g[T6] = addu(g[T9], u64::MAX);
            g[T9] = lh(m, g[A3], -8);
            g[T7] = g[T6] & 0x3FF;
            g[T8] = sll(g[T7], 2);
            g[T6] = addu(g[T9], u64::MAX);
            g[T7] = g[T6] & 0x3FF;
            g[T9] = sll(g[T7], 14);
            g[T6] = g[T8] | g[T9];
            sw(m, g[V0], 4, g[T6]);
            g[T7] = lh(m, g[A0], 0);
            g[AT] = sltu(g[T1], g[T7]);
            if g[AT] == 0 {
                break;
            }
            g[T6] = lh(m, g[A3], 6);
        }
    }
    g[T8] = li(0x8011_0000);
    g[T8] = lh(m, g[T8], 0x4470);
    g[AT] = li(0x43A0_0000);
    g[T9] = li(0x8012_0000);
    f[4].set_u32l(g[T8] as u32);
    g[T9] = lw(m, g[T9], 0xE10);
    f[8].set_u32l(g[AT] as u32);
    f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    f[10].set_u32l(g[T9] as u32);
    g[T9] = li(0x8011_0000);
    g[T9] = lh(m, g[T9], 0x4472);
    g[AT] = li(0x4370_0000);
    f[16].set_fl(fpu::cvt_s_w(f[10].u32l(), fpu::NEAREST));
    f[10].set_u32l(g[AT] as u32);
    g[T6] = li(0x8012_0000);
    g[T6] = lw(m, g[T6], 0xE14);
    g[V0] = lw(m, g[V1], 0);
    f[0].set_fl(f[6].fl() / f[8].fl());
    f[6].set_u32l(g[T9] as u32);
    g[S0] = li(0xF600_0000);
    f[8].set_fl(fpu::cvt_s_w(f[6].u32l(), fpu::NEAREST));
    f[2].set_fl(f[8].fl() / f[10].fl());
    f[18].set_fl(f[16].fl() * f[0].fl());
    f[16].set_u32l(g[T6] as u32);
    g[T6] = li(0x8012_0000);
    g[T6] = lw(m, g[T6], 0xE18);
    f[8].set_u32l(g[T6] as u32);
    g[T6] = li(0x8012_0000);
    g[T6] = lw(m, g[T6], 0xE1C);
    f[4].set_u32l(fpu::trunc_w_s(f[18].fl()));
    f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fpu::NEAREST));
    g[A2] = s32(f[4].u32l());
    g[T4] = g[A2] & 0x3FF;
    f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
    f[4].set_fl(f[18].fl() * f[2].fl());
    f[6].set_u32l(fpu::trunc_w_s(f[4].fl()));
    f[4].set_u32l(g[T6] as u32);
    f[16].set_fl(f[10].fl() * f[0].fl());
    g[T6] = addu(g[V0], 8);
    g[A3] = s32(f[6].u32l());
    sw(m, g[V1], 0, g[T6]);
    f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    f[18].set_u32l(fpu::trunc_w_s(f[16].fl()));
    f[8].set_fl(f[6].fl() * f[2].fl());
    g[T0] = s32(f[18].u32l());
    g[T7] = g[T0] & 0x3FF;
    f[10].set_u32l(fpu::trunc_w_s(f[8].fl()));
    g[T8] = sll(g[T7], 14);
    g[T3] = g[T8] | g[S0];
    g[T1] = s32(f[10].u32l());
    g[T9] = addu(g[T1], 2);
    g[T6] = g[T9] & 0x3FF;
    g[T7] = sll(g[T6], 2);
    g[T8] = g[T3] | g[T7];
    g[T6] = addu(g[T1], (-2i64) as u64);
    g[T7] = g[T6] & 0x3FF;
    g[T9] = sll(g[T4], 14);
    sw(m, g[V0], 0, g[T8]);
    g[T8] = sll(g[T7], 2);
    g[T4] = g[T9];
    g[T9] = g[T9] | g[T8];
    sw(m, g[V0], 4, g[T9]);
    g[V0] = lw(m, g[V1], 0);
    g[T9] = addu(g[A2], 2);
    g[T7] = g[T1] & 0x3FF;
    g[T6] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T6]);
    g[T6] = g[T9] & 0x3FF;
    g[T8] = sll(g[T7], 2);
    g[T7] = sll(g[T6], 14);
    g[T9] = g[T7] | g[S0];
    g[T6] = g[T9] | g[T8];
    sw(m, g[SP], 0xC, g[T8]);
    g[T7] = g[A3] & 0x3FF;
    g[T9] = sll(g[T7], 2);
    g[T8] = addu(g[A2], (-2i64) as u64);
    sw(m, g[V0], 0, g[T6]);
    g[T6] = g[T8] & 0x3FF;
    g[T7] = sll(g[T6], 14);
    g[T8] = g[T7] | g[T9];
    sw(m, g[SP], 8, g[T9]);
    sw(m, g[V0], 4, g[T8]);
    g[V0] = lw(m, g[V1], 0);
    g[T7] = addu(g[T0], 2);
    g[T9] = g[T7] & 0x3FF;
    g[T7] = lw(m, g[SP], 0xC);
    g[T6] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T6]);
    g[T8] = sll(g[T9], 14);
    g[T6] = g[T8] | g[S0];
    g[T9] = g[T6] | g[T7];
    g[T6] = addu(g[T0], (-2i64) as u64);
    sw(m, g[SP], 0x20, g[V0]);
    g[T7] = g[T6] & 0x3FF;
    sw(m, g[V0], 0, g[T9]);
    g[T6] = lw(m, g[SP], 8);
    g[T9] = sll(g[T7], 14);
    g[T7] = g[T9] | g[T6];
    sw(m, g[V0], 4, g[T7]);
    g[V0] = lw(m, g[V1], 0);
    g[T6] = addu(g[A3], 2);
    g[T7] = g[T6] & 0x3FF;
    g[T9] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T9]);
    g[T8] = sll(g[T7], 2);
    g[T9] = g[T3] | g[T8];
    g[T7] = addu(g[A3], (-2i64) as u64);
    sw(m, g[SP], 0x1C, g[V0]);
    g[T8] = g[T7] & 0x3FF;
    sw(m, g[V0], 0, g[T9]);
    g[T9] = sll(g[T8], 2);
    g[T7] = g[T4] | g[T9];
    sw(m, g[V0], 4, g[T7]);
    g[V0] = lw(m, g[V1], 0);
    g[T8] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T8]);
    sw(m, g[V0], 4, 0);
    sw(m, g[V0], 0, g[A1]);
    g[S0] = lw(m, g[SP], 4);
    sh(m, g[A0], 0, 0);
    g[SP] = addu(g[SP], 0x50);
}
