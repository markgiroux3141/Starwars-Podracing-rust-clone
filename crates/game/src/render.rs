//! Render state and display-list writers (NOTES.md, "Depth-0 leaves ported
//! in session 7"): appends to the display lists behind `[0x801217B0]` and
//! `[0x80112C90]`, the render modes at `0x800A3DA0` switched by tag, the
//! lights at `0x800A3DB0`, the framebuffer addresses, and the RSP task.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use n64mem::Mem;
use crate::imports;
use crate::recomp::{addu, call, enter, fpu, lbu, ld, lh, lhu, li, lw, multu, reg::*, s32, sb, sd, sh, sll, slt, sltu, sra, subu, sw, Fpr, RecompContext};

/// The display list pointer [`func_80014C98`] appends to.
pub const DL_HEAD: u32 = 0x8012_17B0;

/// The head of a second display list, the one the material and matrix
/// functions (e.g. [`func_8003609C`], [`func_80035BF0`]) append to.
pub const DL2_HEAD: u32 = 0x8011_2C90;

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

/// The box `func_80011F38` grows: s16 `ymin`, `ymax`, `xmin`, `xmax` from
/// here (`func_80013DC0` resets it before drawing text and queues it as a
/// screen rectangle after).
pub const TEXT_BOX: u32 = 0x800D_6914;

/// `func_80011F38(x, y, ox, oy, w, h, s, t, tile)` (glyph rectangle,
/// **guess**: `func_80013DC0`'s text drawing reaches it): every argument
/// is taken as s16, the last five being stack words read with `lh` from
/// their low halves. With `X = s16(x - ox)`, `Y = s16(y - oy)`, `X2 = s16(X
/// + w)`, `Y2 = s16(Y + h)` and the screen scales `ws = f32(W / 320.0)`,
/// `hs = f32(H / 240.0)` (`W`, `H` the s16 at `0x80114470`/`72`, divided in
/// double), each set to 1.0 if below it:
/// - the box at [`TEXT_BOX`]: if `xmax < xmin` (signed: empty), `xmin =
///   trunc(f32(X) * ws)`, `ymin = trunc(f32(Y) * hs)`, `xmax = trunc(f32(X2)
///   * ws)`, `ymax = trunc(f32(Y2) * hs)`. Otherwise each edge moves out to
///   the same product if it lies beyond (f32 compares: `xmin` if the
///   product is below `f32(xmin)`, `ymin` likewise, `xmax` and `ymax` if
///   above). `trunc` is the C cast; the stores keep the low halfword.
/// - appends `gSPTextureRectangle`'s three commands at [`DL_HEAD`], each
///   by reading the head, storing it advanced by 8, then the two words at
///   the old head: `E4000000 | XH << 12 | YH` and `(tile & 7) << 24 | XL
///   << 12 | YL`; `E1000000` and `s << 21 | ((t << 5) & 0xFFFF)`;
///   `F1000000` and `u(1024 / ws) << 16 | (u(1024 / hs) & 0xFFFF)`. Here
///   `XH = u(f32(s16(4 * X2)) * ws) & 0xFFF`, `YH` the same with `Y2` and
///   `hs`, `XL` and `YL` with `X` and `Y`, and `u` is IDO's float →
///   unsigned idiom ([`fpu::to_unsigned_s`]): truncation toward zero, with
///   negative results `0xFFFFFFFF`.
///
/// QUIRK: `4 * X2` etc. wrap at 16 bits (`sll 18`, `sra 16`), and a
/// negative scaled coordinate becomes `0xFFF` rather than 0. The idiom's
/// second path (for values from 2^31, which the products can't reach) is
/// dead under the oracle and kept as the C has it.
///
/// Spills `a0`..`a3` to their home slots `sp + 0..0xC`. Leaves `a0 = X`,
/// `a1 = Y`, `v0 = X2`, `v1 = Y2`, `a2`, `a3`, `t1` the three commands'
/// addresses (`t3` the head after the second), `t0 = 0x800D6918`, `t2` =
/// [`DL_HEAD`], `t9 = u(1024 / ws)`, `t7 = t9 << 16`, `t4 = u(1024 / hs)`,
/// `t5 = t4 & 0xFFFF`, `t6` the last word, `t8 = 0` (the saved FCR31), `at
/// = 0x4F000000`, `f0 = ws`, `f2 = hs`, `f12 = 1024.0` (with `f13` 1.0's
/// high word), and `f4`..`f18` from the path taken.
///
/// Domain: canonical pointers; the list in RDRAM, not overlapping the
/// head, the box, the screen size or the stack arguments. No operand can
/// be NaN: `ws`, `hs` lie in [1, 137) and the rest are s16 values.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011F38(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    let mut fcr31 = fpu::NEAREST;
    // f10 = W / 320.0 and f6 = H / 240.0 in double (f8 = 320.0, f4 = 240.0
    // and f12 = 1.0 as word pairs), then f0 = ws and f2 = hs; the
    // arguments spilled, and a0 = X, a1 = y, a2 = ox, a3 = oy as s16.
    g[V0] = li(0x8011_4470);
    g[T7] = lh(m, g[V0], 0);
    g[AT] = li(0x3FF0_0000);
    f[12].set_u32h(g[AT] as u32); // f13
    f[4].set_u32l(g[T7] as u32);
    g[AT] = li(0x4074_0000);
    f[8].set_u32h(g[AT] as u32); // f9
    f[6].set_d(f64::from(f[4].u32l() as i32));
    f[8].set_u32l(0);
    sw(m, g[SP], 4, g[A1]);
    g[T8] = sll(g[A1], 16);
    g[A1] = sra(g[T8], 16);
    f[10].set_d(f[6].d() / f[8].d());
    g[T8] = lh(m, g[V0], 2);
    g[AT] = li(0x406E_0000);
    f[4].set_u32h(g[AT] as u32); // f5
    f[16].set_u32l(g[T8] as u32);
    f[4].set_u32l(0);
    f[12].set_u32l(0);
    f[18].set_d(f64::from(f[16].u32l() as i32));
    sw(m, g[SP], 0, g[A0]);
    g[T6] = sll(g[A0], 16);
    sw(m, g[SP], 8, g[A2]);
    g[T3] = sll(g[A2], 16);
    g[A2] = sra(g[T3], 16);
    g[A0] = sra(g[T6], 16);
    g[A0] = subu(g[A0], g[A2]);
    sw(m, g[SP], 0xC, g[A3]);
    g[T5] = sll(g[A3], 16);
    g[T9] = sll(g[A0], 16);
    g[A3] = sra(g[T5], 16);
    g[AT] = li(0x3F80_0000);
    g[T0] = li(0x800D_6918);
    g[A0] = sra(g[T9], 16);
    g[V1] = li(0x800D_0000);
    f[6].set_d(f[18].d() / f[4].d());
    f[0].set_fl(fpu::cvt_s_d(f[10].d(), fcr31));
    f[8].set_d(f64::from(f[0].fl()));
    let below = f[8].d() < f[12].d();
    f[2].set_fl(fpu::cvt_s_d(f[6].d(), fcr31));
    if below {
        f[0].set_u32l(g[AT] as u32);
    }
    f[10].set_d(f64::from(f[2].fl()));
    g[AT] = li(0x3F80_0000);
    if f[10].d() < f[12].d() {
        f[2].set_u32l(g[AT] as u32);
    }
    // v1 = xmax, v0 = xmin, a1 = Y.
    g[V1] = lh(m, g[V1], 0x691A);
    g[V0] = lh(m, g[T0], 0);
    g[A1] = subu(g[A1], g[A3]);
    g[T4] = sll(g[A1], 16);
    g[AT] = slt(g[V1], g[V0]);
    g[A1] = sra(g[T4], 16);
    if g[AT] == 0 {
        // Grow: xmin, ymin (a2 = TEXT_BOX), then v0 = X2 for xmax, v1 = Y2
        // for ymax, each stored only if it moves out.
        f[8].set_u32l(g[A0] as u32);
        f[16].set_u32l(g[V0] as u32);
        f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fcr31));
        f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fcr31));
        f[12].set_fl(f[10].fl() * f[0].fl());
        if f[12].fl() < f[18].fl() {
            f[4].set_u32l(fpu::trunc_w_s(f[12].fl()));
            g[T7] = s32(f[4].u32l());
            sh(m, g[T0], 0, g[T7]);
        }
        f[6].set_u32l(g[A1] as u32);
        g[A2] = li(TEXT_BOX);
        g[T8] = lh(m, g[A2], 0);
        f[8].set_fl(fpu::cvt_s_w(f[6].u32l(), fcr31));
        f[10].set_u32l(g[T8] as u32);
        f[16].set_fl(fpu::cvt_s_w(f[10].u32l(), fcr31));
        f[12].set_fl(f[8].fl() * f[2].fl());
        f[8].set_u32l(g[V1] as u32);
        if f[12].fl() < f[16].fl() {
            f[18].set_u32l(fpu::trunc_w_s(f[12].fl()));
            g[T3] = s32(f[18].u32l());
            sh(m, g[A2], 0, g[T3]);
        }
        g[T4] = lh(m, g[SP], 0x12);
        f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fcr31));
        g[V0] = addu(g[A0], g[T4]);
        g[T5] = sll(g[V0], 16);
        g[V0] = sra(g[T5], 16);
        f[4].set_u32l(g[V0] as u32);
        f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fcr31));
        f[12].set_fl(f[6].fl() * f[0].fl());
        if f[10].fl() < f[12].fl() {
            f[16].set_u32l(fpu::trunc_w_s(f[12].fl()));
            g[AT] = li(0x800D_0000);
            g[T8] = s32(f[16].u32l());
            sh(m, g[AT], 0x691A, g[T8]);
        }
        g[T9] = lh(m, g[SP], 0x16);
        g[T5] = li(0x800D_0000);
        g[T5] = lh(m, g[T5], 0x6916);
        g[V1] = addu(g[A1], g[T9]);
        g[T3] = sll(g[V1], 16);
        g[V1] = sra(g[T3], 16);
        f[18].set_u32l(g[V1] as u32);
        f[6].set_u32l(g[T5] as u32);
        f[4].set_fl(fpu::cvt_s_w(f[18].u32l(), fcr31));
        f[8].set_fl(fpu::cvt_s_w(f[6].u32l(), fcr31));
        f[12].set_fl(f[4].fl() * f[2].fl());
        if f[8].fl() < f[12].fl() {
            f[10].set_u32l(fpu::trunc_w_s(f[12].fl()));
            g[AT] = li(0x800D_0000);
            g[T7] = s32(f[10].u32l());
            sh(m, g[AT], 0x6916, g[T7]);
        }
    } else {
        // Empty: all four edges set (v0 = X2, v1 = Y2).
        f[16].set_u32l(g[A0] as u32);
        f[8].set_u32l(g[A1] as u32);
        g[T3] = lh(m, g[SP], 0x12);
        f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fcr31));
        g[T8] = lh(m, g[SP], 0x16);
        g[V0] = addu(g[A0], g[T3]);
        g[T4] = sll(g[V0], 16);
        f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fcr31));
        f[4].set_fl(f[18].fl() * f[0].fl());
        g[V0] = sra(g[T4], 16);
        g[A2] = li(TEXT_BOX);
        g[V1] = addu(g[A1], g[T8]);
        g[AT] = li(0x800D_0000);
        f[16].set_fl(f[10].fl() * f[2].fl());
        f[6].set_u32l(fpu::trunc_w_s(f[4].fl()));
        f[4].set_u32l(g[V0] as u32);
        f[18].set_u32l(fpu::trunc_w_s(f[16].fl()));
        g[T7] = s32(f[6].u32l());
        f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fcr31));
        g[T9] = s32(f[18].u32l());
        sh(m, g[T0], 0, g[T7]);
        sh(m, g[A2], 0, g[T9]);
        g[T9] = sll(g[V1], 16);
        g[V1] = sra(g[T9], 16);
        f[16].set_u32l(g[V1] as u32);
        f[8].set_fl(f[6].fl() * f[0].fl());
        f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fcr31));
        f[10].set_u32l(fpu::trunc_w_s(f[8].fl()));
        f[4].set_fl(f[18].fl() * f[2].fl());
        g[T7] = s32(f[10].u32l());
        sh(m, g[AT], 0x691A, g[T7]);
        f[6].set_u32l(fpu::trunc_w_s(f[4].fl()));
        g[AT] = li(0x800D_0000);
        g[T5] = s32(f[6].u32l());
        sh(m, g[AT], 0x6916, g[T5]);
    }
    // G_TEXRECT: t6 = XH, t7 = YH, then t4 = XL and t6 = YL (a2 = the
    // command, t2 = DL_HEAD). No conversion sits between an idiom and its
    // restoring ctc1, so to_unsigned_s restores in place.
    g[T3] = sll(g[V0], 18);
    g[T4] = sra(g[T3], 16);
    f[16].set_u32l(g[T4] as u32);
    g[T6] = 1;
    f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fcr31));
    g[T2] = li(DL_HEAD);
    g[A2] = lw(m, g[T2], 0);
    g[AT] = li(0x4480_0000);
    f[12].set_u32l(g[AT] as u32);
    g[T8] = addu(g[A2], 8);
    f[4].set_fl(f[18].fl() * f[0].fl());
    sw(m, g[T2], 0, g[T8]);
    g[T4] = sll(g[V1], 18);
    fpu::to_unsigned_s(g, f, &mut fcr31, T5, T6, 6, 4);
    g[T5] = sra(g[T4], 16);
    f[8].set_u32l(g[T5] as u32);
    g[T7] = g[T6] & 0xFFF;
    g[T8] = sll(g[T7], 12);
    f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fcr31));
    g[T7] = 1;
    g[AT] = li(0xE400_0000);
    g[T9] = g[T8] | g[AT];
    f[16].set_fl(f[10].fl() * f[2].fl());
    fpu::to_unsigned_s(g, f, &mut fcr31, T6, T7, 18, 16);
    g[T8] = g[T7] & 0xFFF;
    g[T3] = g[T9] | g[T8];
    g[T9] = sll(g[A0], 18);
    g[T8] = sra(g[T9], 16);
    f[4].set_u32l(g[T8] as u32);
    sw(m, g[A2], 0, g[T3]);
    g[T4] = lh(m, g[SP], 0x22);
    f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fcr31));
    g[T5] = g[T4] & 7;
    g[T4] = 1;
    g[T6] = sll(g[T5], 24);
    f[8].set_fl(f[6].fl() * f[0].fl());
    fpu::to_unsigned_s(g, f, &mut fcr31, T3, T4, 10, 8);
    g[T3] = sll(g[A1], 18);
    g[T5] = g[T4] & 0xFFF;
    g[T4] = sra(g[T3], 16);
    f[16].set_u32l(g[T4] as u32);
    g[T7] = sll(g[T5], 12);
    g[T9] = g[T6] | g[T7];
    f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fcr31));
    g[T6] = 1;
    f[4].set_fl(f[18].fl() * f[2].fl());
    fpu::to_unsigned_s(g, f, &mut fcr31, T5, T6, 6, 4);
    g[T7] = g[T6] & 0xFFF;
    g[T8] = g[T9] | g[T7];
    // f8 = 1024 / ws; G_RDPHALF_1 (a3) with s and t, then G_RDPHALF_2 (t1).
    f[8].set_fl(f[12].fl() / f[0].fl());
    sw(m, g[A2], 4, g[T8]);
    g[A3] = lw(m, g[T2], 0);
    g[T4] = li(0xE100_0000);
    g[T5] = li(0xF100_0000);
    g[T3] = addu(g[A3], 8);
    sw(m, g[T2], 0, g[T3]);
    sw(m, g[A3], 0, g[T4]);
    g[T4] = lh(m, g[SP], 0x1E);
    g[T8] = lh(m, g[SP], 0x1A);
    g[T9] = sll(g[T4], 5);
    g[T7] = g[T9] & 0xFFFF;
    g[T9] = 1;
    g[T3] = sll(g[T8], 21);
    g[T8] = g[T3] | g[T7];
    sw(m, g[A3], 4, g[T8]);
    g[T1] = lw(m, g[T2], 0);
    g[T4] = addu(g[T1], 8);
    sw(m, g[T2], 0, g[T4]);
    sw(m, g[T1], 0, g[T5]);
    fpu::to_unsigned_s(g, f, &mut fcr31, T6, T9, 10, 8);
    g[T4] = 1;
    g[T7] = sll(g[T9], 16);
    f[16].set_fl(f[12].fl() / f[2].fl());
    fpu::to_unsigned_s(g, f, &mut fcr31, T8, T4, 18, 16);
    g[T5] = g[T4] & 0xFFFF;
    g[T6] = g[T7] | g[T5];
    sw(m, g[T1], 4, g[T6]);
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

/// `func_800141EC(c)` (set up glyph `c` for drawing, **guess**): appends
/// at [`DL_HEAD`] `FA000000` with the colour word from the bytes at
/// `0x800A1CCC` (`b0 << 24 | b1 << 16 | b2 << 8 | b3`), sets `[0x800D6938]
/// = 1`, appends the combine `FCFF97FF FF2DFEFF`; then, for the current
/// font `f = [0x800A1D8C]` by its format `[f]`: 0, the load prologue
/// `E3001001 0000C000`, `FD100000 800A1DD0`, `E8000000 0`, `F5000100
/// 07000000`, `E6000000 0`, `F0000000 073FC000`, `E7000000 0`; 2 (the
/// format re-read), `E3001001 00008000`, `FD100000 [f + 0x48]`, `E8000000
/// 0`, `F5000100 07000000`, `E6000000 0`, `F0000000 0703C000`, `E7000000
/// 0`, `E6000000 0`, the words stored in the C's order. Then the
/// character (the low byte of `c`, spilled to its home slot `sp + 0` and
/// read back as a byte at `sp + 3`) is uppercased (in that slot) if it is
/// lowercase and the font's last character `[f + 0x5B]` is below `'a'`;
/// with the glyph table `[f + 0x5C]`, first/last characters `[f + 0x5A]`,
/// `[f + 0x5B]` and `G` = the 16-byte glyph `c - first`: if the table is
/// set, `first <= c <= last` and `G`'s halfword `+8` isn't -1, the words
/// `0x800D691C..0x800D6930` = `G`'s signed halfwords `+6, +4, +0xC, +0xE,
/// +8, +0xA`, `[0x800D6934]` = `G`'s byte `+1` if the format is 0 (else 0),
/// and [`func_800125E4`]`(f, G's byte +0)` loads its texture.
///
/// Frame (`sp - 0x20`): `ra`, `s0` at `+0x1C`, `+0x18`, restored (`s0 =
/// f`). Leaves `v0`, `v1`, `a0`..`a3`, `t*` from the path and the callee's
/// registers.
///
/// Domain: the list, the font and its table in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800141EC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = li(0x8012_17B0);
    g[V0] = lw(m, g[V1], 0);
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    g[T6] = addu(g[V0], 8);
    sw(m, g[SP], 0x18, g[S0]);
    sw(m, g[SP], 0x20, g[A0]);
    g[A3] = li(0x800A_0000);
    sw(m, g[V1], 0, g[T6]);
    g[T7] = li(0xFA00_0000);
    g[A3] = addu(g[A3], 0x1CCC);
    sw(m, g[V0], 0, g[T7]);
    g[T7] = lbu(m, g[A3], 0);
    g[T9] = lbu(m, g[A3], 3);
    g[AT] = li(0x800D_0000);
    g[T8] = sll(g[T7], 24);
    g[T6] = g[T9] | g[T8];
    g[T9] = lbu(m, g[A3], 1);
    g[T4] = li(0x800A_1D8C);
    g[T8] = sll(g[T9], 16);
    g[T7] = g[T6] | g[T8];
    g[T6] = lbu(m, g[A3], 2);
    g[T8] = sll(g[T6], 8);
    g[T9] = g[T7] | g[T8];
    g[T6] = 1;
    sw(m, g[V0], 4, g[T9]);
    sw(m, g[AT], 0x6938, g[T6]);
    g[V0] = lw(m, g[V1], 0);
    g[T9] = li(0xFF2D_0000);
    g[T8] = li(0xFCFF_0000);
    g[T7] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T7]);
    g[T8] = g[T8] | 0x97FF;
    g[T9] = g[T9] | 0xFEFF;
    sw(m, g[V0], 4, g[T9]);
    sw(m, g[V0], 0, g[T8]);
    g[S0] = lw(m, g[T4], 0);
    g[T7] = li(0xE300_0000);
    g[AT] = 2;
    g[T3] = lw(m, g[S0], 0);
    if g[T3] == 0 {
        g[V0] = lw(m, g[V1], 0);
        g[T7] = g[T7] | 0x1001;
        g[T8] = 0xC000;
        g[T6] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T6]);
        sw(m, g[V0], 4, g[T8]);
        sw(m, g[V0], 0, g[T7]);
        g[V0] = lw(m, g[V1], 0);
        g[T7] = li(0x800A_1DD0);
        g[T9] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T9]);
        g[T6] = li(0xFD10_0000);
        sw(m, g[V0], 0, g[T6]);
        sw(m, g[V0], 4, g[T7]);
        g[V0] = lw(m, g[V1], 0);
        g[T9] = li(0xE800_0000);
        g[T7] = li(0xF500_0000);
        g[T8] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T8]);
        sw(m, g[V0], 4, 0);
        sw(m, g[V0], 0, g[T9]);
        g[V0] = lw(m, g[V1], 0);
        g[T8] = li(0x700_0000);
        g[T7] = g[T7] | 0x100;
        g[T6] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T6]);
        sw(m, g[V0], 4, g[T8]);
        sw(m, g[V0], 0, g[T7]);
        g[V0] = lw(m, g[V1], 0);
        g[T5] = li(0xE600_0000);
        g[T8] = li(0x73F_0000);
        g[T9] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T9]);
        sw(m, g[V0], 4, 0);
        sw(m, g[V0], 0, g[T5]);
        g[V0] = lw(m, g[V1], 0);
        g[T8] = g[T8] | 0xC000;
        g[T7] = li(0xF000_0000);
        g[T6] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T6]);
        sw(m, g[V0], 4, g[T8]);
        sw(m, g[V0], 0, g[T7]);
        g[V0] = lw(m, g[V1], 0);
        g[T6] = li(0xE700_0000);
        g[T9] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T9]);
        sw(m, g[V0], 4, 0);
        sw(m, g[V0], 0, g[T6]);
        g[S0] = lw(m, g[T4], 0);
        g[T3] = lw(m, g[S0], 0);
    }
    g[T5] = li(0xE600_0000);
    if g[T3] == g[AT] {
        g[V0] = lw(m, g[V1], 0);
        g[T8] = li(0xE300_1001);
        g[T7] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T7]);
        g[T9] = 0x8000;
        sw(m, g[V0], 4, g[T9]);
        sw(m, g[V0], 0, g[T8]);
        g[V0] = lw(m, g[V1], 0);
        g[T7] = li(0xFD10_0000);
        g[T6] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T6]);
        sw(m, g[V0], 0, g[T7]);
        g[T8] = lw(m, g[T4], 0);
        g[T7] = li(0xE800_0000);
        g[T9] = lw(m, g[T8], 0x48);
        sw(m, g[V0], 4, g[T9]);
        g[V0] = lw(m, g[V1], 0);
        g[T9] = li(0xF500_0100);
        g[T6] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T6]);
        sw(m, g[V0], 4, 0);
        sw(m, g[V0], 0, g[T7]);
        g[V0] = lw(m, g[V1], 0);
        g[T6] = li(0x700_0000);
        g[T8] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T8]);
        sw(m, g[V0], 4, g[T6]);
        sw(m, g[V0], 0, g[T9]);
        g[V0] = lw(m, g[V1], 0);
        g[T6] = li(0x703_C000);
        g[T7] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T7]);
        sw(m, g[V0], 4, 0);
        sw(m, g[V0], 0, g[T5]);
        g[V0] = lw(m, g[V1], 0);
        g[T9] = li(0xF000_0000);
        g[T8] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T8]);
        sw(m, g[V0], 4, g[T6]);
        sw(m, g[V0], 0, g[T9]);
        g[V0] = lw(m, g[V1], 0);
        g[T8] = li(0xE700_0000);
        g[T7] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T7]);
        sw(m, g[V0], 4, 0);
        sw(m, g[V0], 0, g[T8]);
        g[V0] = lw(m, g[V1], 0);
        g[T9] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T9]);
        sw(m, g[V0], 4, 0);
        sw(m, g[V0], 0, g[T5]);
        g[S0] = lw(m, g[T4], 0);
    }
    'b_80014480: {
        g[V1] = lbu(m, g[SP], 0x23);
        g[AT] = slt(g[V1], 0x61);
        let c0 = g[AT] == 0;
        g[AT] = slt(g[V1], 0x7B);
        if c0 {
            if g[AT] == 0 {
                g[A1] = lw(m, g[S0], 0x5C);
                break 'b_80014480;
            }
            g[T6] = lbu(m, g[S0], 0x5B);
            g[T7] = addu(g[V1], (-0x20i64) as u64);
            g[AT] = slt(g[T6], 0x61);
            if g[AT] == 0 {
                g[A1] = lw(m, g[S0], 0x5C);
                break 'b_80014480;
            }
            sb(m, g[SP], 0x23, g[T7]);
        }
        g[A1] = lw(m, g[S0], 0x5C);
    }
    g[V1] = lbu(m, g[SP], 0x23);
    if g[A1] == 0 {
        g[A1] = (-2i64) as u64;
    } else {
        g[A2] = lbu(m, g[S0], 0x5A);
        g[AT] = slt(g[V1], g[A2]);
        if g[AT] != 0 {
            g[A1] = (-2i64) as u64;
        } else {
            g[T8] = lbu(m, g[S0], 0x5B);
            g[A0] = subu(g[V1], g[A2]);
            g[T9] = sll(g[A0], 4);
            g[AT] = slt(g[T8], g[V1]);
            g[V0] = addu(g[A1], g[T9]);
            if g[AT] != 0 {
                g[A1] = (-2i64) as u64;
            } else {
                g[T6] = lh(m, g[V0], 8);
                g[AT] = u64::MAX;
                if g[T6] != g[AT] {
                    g[T7] = lh(m, g[V0], 6);
                    g[A1] = lbu(m, g[V0], 0);
                    g[AT] = li(0x800D_0000);
                    sw(m, g[AT], 0x691C, g[T7]);
                    g[T8] = lh(m, g[V0], 4);
                    g[AT] = li(0x800D_0000);
                    sw(m, g[AT], 0x6920, g[T8]);
                    g[T9] = lh(m, g[V0], 0xC);
                    g[AT] = li(0x800D_0000);
                    sw(m, g[AT], 0x6924, g[T9]);
                    g[T6] = lh(m, g[V0], 0xE);
                    g[AT] = li(0x800D_0000);
                    sw(m, g[AT], 0x6928, g[T6]);
                    g[T7] = lh(m, g[V0], 8);
                    g[AT] = li(0x800D_0000);
                    sw(m, g[AT], 0x692C, g[T7]);
                    g[T8] = lh(m, g[V0], 0xA);
                    g[AT] = li(0x800D_0000);
                    sw(m, g[AT], 0x6930, g[T8]);
                    g[T9] = lw(m, g[S0], 0);
                    g[AT] = li(0x800D_0000);
                    if g[T9] != 0 {
                        sw(m, g[AT], 0x6934, 0);
                    } else {
                        g[T6] = lbu(m, g[V0], 1);
                        g[AT] = li(0x800D_0000);
                        sw(m, g[AT], 0x6934, g[T6]);
                    }
                } else {
                    g[A1] = (-2i64) as u64;
                }
            }
        }
    }
    if (g[A1] as i64) < 0 {
        g[RA] = lw(m, g[SP], 0x1C);
    } else {
        g[A0] = g[S0];
        call(imports::func_800125E4, m, ctx);
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x1C);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x20);
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

/// [`func_8003594C`]'s rewrite of a combine word pair's bytes 0..6: the
/// register holding the replacement (`t3` = 6, `t4` = 4) and whether 8 and
/// 9 match too (else only 1 and 2).
const COMBINE_SLOTS: [(usize, bool); 7] = [(T3, true), (T4, true), (T4, true), (T3, true), (T3, false), (T3, false), (T4, false)];

/// `func_8003594C(mat, q)` (material render flags, **guess**): adjusts
/// the words `mat+0x18`/`+0x1C` and two 8-byte blocks at `mat+6` and
/// `mat+0xE` (combiner selectors, **guess**) for the current settings:
/// 1. If `[0x800A4740] == 0`: bits 4 then 5 of `+0x18`, then of `+0x1C`,
///    are cleared (each step stored).
/// 2. If bit 6 of the settings word `[0x800D697C]` is set and the halfword
///    `mat+4 == 2` and `+0x18`'s high half is `0xC800`: `+0x18` = its low
///    half, then `| 0x0C080000` (both stored); the settings are re-read.
/// 3. Unless bit 4 of the settings is clear and `q` is non-null with `[q +
///    8] != 0`: in each block, byte k is replaced when it is 1 or 2 (or 8
///    or 9, for k = 0..3): by 6 for k = 0, 3, 4, 5, 7, by 4 for k = 1, 2,
///    6. The block pointers go through the frame (`sp - 0x4C`, `- 0x48`)
///    and are re-read after each store.
/// 4. If `[0x800A3DA8] != 0`: bit 4 of `+0x18` and `+0x1C` cleared.
/// 5. If `[0x800A3D9C] != 0`: `+0x18 = [0x800A3DA0]`, `+0x1C =
///    [0x800A3DA4]`, and if the first is `0xF5504040`: bytes `+0x2A..0x2D` =
///    0x80, 0, 0xFF, 0x28 and bit 2 of the word `+0` set.
///
/// Leaves `a3 = q`, or past step 3 the last byte read; `v1 = -0x11`, `v0 =
/// 0x800A3DA0` past step 4, and the temporaries of the last steps.
///
/// Domain: canonical pointers (`q` may be null), `mat` not overlapping the
/// globals.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003594C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = li(0x800A_0000);
    g[T6] = lw(m, g[T6], 0x4740);
    g[SP] = addu(g[SP], (-0x58i64) as u64);
    g[A3] = g[A1];
    g[A2] = li(0x800D_0000);
    // 1.
    if g[T6] == 0 {
        g[T7] = lw(m, g[A0], 0x18);
        g[V1] = (-0x11i64) as u64;
        g[V0] = (-0x21i64) as u64;
        g[T8] = g[T7] & g[V1];
        g[T7] = lw(m, g[A0], 0x1C);
        sw(m, g[A0], 0x18, g[T8]);
        g[T6] = g[T8] & g[V0];
        g[T8] = g[T7] & g[V1];
        sw(m, g[A0], 0x18, g[T6]);
        sw(m, g[A0], 0x1C, g[T8]);
        g[T6] = g[T8] & g[V0];
        sw(m, g[A0], 0x1C, g[T6]);
    }
    // 2. a2 = the settings; t5 = the frame slots' end.
    g[A2] = lw(m, g[A2], 0x697C);
    g[V1] = (-0x11i64) as u64;
    g[T5] = addu(g[SP], 0x14);
    g[T7] = g[A2] & 0x40;
    g[T4] = 4;
    if g[T7] != 0 {
        g[T8] = lh(m, g[A0], 4);
        g[V0] = 2;
        if g[V0] == g[T8] {
            g[A1] = lw(m, g[A0], 0x18);
            g[AT] = li(0xFFFF_0000);
            g[T9] = g[A1] & g[AT];
            g[AT] = li(0xC800_0000);
            g[T6] = g[A1] & 0xFFFF;
            if g[T9] == g[AT] {
                g[AT] = li(0xC08_0000);
                sw(m, g[A0], 0x18, g[T6]);
                g[T8] = g[T6] | g[AT];
                sw(m, g[A0], 0x18, g[T8]);
                g[A2] = li(0x800D_0000);
                g[A2] = lw(m, g[A2], 0x697C);
            }
        }
    }
    g[T9] = g[A2] & 0x10;
    // 3.
    g[V0] = 2;
    let skip = g[T9] == 0 && g[A3] != 0 && {
        g[T6] = lw(m, g[A3], 8);
        g[T6] != 0
    };
    g[T7] = addu(g[A0], 6);
    if !skip {
        // a2 walks the frame's two block pointers, a1 = the block, a3 =
        // the byte; t0, v0, t1, t2 = 1, 2, 8, 9.
        g[T8] = addu(g[A0], 0xE);
        sw(m, g[SP], 0xC, g[T7]);
        sw(m, g[SP], 0x10, g[T8]);
        g[A2] = addu(g[SP], 0xC);
        g[T3] = 6;
        g[T2] = 9;
        g[T1] = 8;
        g[T0] = 1;
        g[A1] = lw(m, g[A2], 0);
        loop {
            g[A3] = lbu(m, g[A1], 0);
            for (k, &(to, wide)) in COMBINE_SLOTS.iter().enumerate() {
                let v = g[A3];
                if v == g[T0] || v == g[V0] || (wide && (v == g[T1] || v == g[T2])) {
                    sb(m, g[A1], k as i32, g[to]);
                    g[A1] = lw(m, g[A2], 0);
                }
                g[A3] = lbu(m, g[A1], k as i32 + 1);
            }
            g[A2] = addu(g[A2], 4);
            if g[A3] == g[T0] || g[A3] == g[V0] {
                sb(m, g[A1], 7, g[T3]);
            }
            if g[A2] == g[T5] {
                break;
            }
            g[A1] = lw(m, g[A2], 0);
        }
    }
    // 4.
    g[T9] = li(0x800A_0000);
    g[T9] = lw(m, g[T9], 0x3DA8);
    g[V0] = li(0x800A_3DA0);
    if g[T9] != 0 {
        g[T6] = lw(m, g[A0], 0x18);
        g[T8] = lw(m, g[A0], 0x1C);
        g[T7] = g[T6] & g[V1];
        g[T9] = g[T8] & g[V1];
        sw(m, g[A0], 0x18, g[T7]);
        sw(m, g[A0], 0x1C, g[T9]);
    }
    // 5.
    g[T6] = li(0x800A_0000);
    g[T6] = lw(m, g[T6], 0x3D9C);
    if g[T6] != 0 {
        g[T7] = lw(m, g[V0], 0);
        g[T8] = li(0x800A_0000);
        g[AT] = li(0xF550_0000);
        sw(m, g[A0], 0x18, g[T7]);
        g[T8] = lw(m, g[T8], 0x3DA4);
        g[AT] = g[AT] | 0x4040;
        g[T6] = 0x80;
        sw(m, g[A0], 0x1C, g[T8]);
        g[T9] = lw(m, g[V0], 0);
        g[T7] = 0xFF;
        g[T8] = 0x28;
        if g[T9] == g[AT] {
            g[T9] = lw(m, g[A0], 0);
            sb(m, g[A0], 0x2A, g[T6]);
            sb(m, g[A0], 0x2B, 0);
            g[T6] = g[T9] | 4;
            sb(m, g[A0], 0x2C, g[T7]);
            sb(m, g[A0], 0x2D, g[T8]);
            sw(m, g[A0], 0, g[T6]);
        }
    }
    g[SP] = addu(g[SP], 0x58);
}

/// `func_80035BF0(mat, force)` (a material's render state, **guess**):
/// appends to the list at [`DL2_HEAD`] the commands for the parts of `mat`
/// that differ from the state last sent (the copy at `0x80112DE4..`,
/// which this function reads but doesn't update), or all of them with
/// `force` nonzero:
/// 1. If `force` or the s16 `mat + 4` differs from `[0x80112DE4]`:
///    `E3000A01`, then `0` if that halfword is 1, else `0x100000`.
/// 2. If `force`, or the eight bytes `mat + 6` differ from `0x80112DE6` or
///    the eight at `mat + 0xE` from `0x80112DEE` ([`func_80081530`], the
///    second only if the first match): `gDPSetCombineLERP` from the 16
///    bytes `mat + 6..+0x16`, `(a0, b0, c0, d0, Aa0, Ab0, Ac0, Ad0, a1, b1,
///    c1, d1, Aa1, Ab1, Ac1, Ad1)`: `FC000000 | a0 << 20 | c0 << 15 | Aa0 <<
///    12 | Ac0 << 9 | a1 << 5 | c1` and `b0 << 28 | b1 << 24 | Aa1 << 21 |
///    Ac1 << 18 | d0 << 15 | Ab0 << 12 | Ad0 << 9 | d1 << 6 | Ab1 << 3 |
///    Ad1`, each field masked to its width (4 bits for `a`, `b`, 5 for
///    `c`, 3 for the others).
/// 3. If `force` or the words `mat + 0x18`, `+0x1C` differ from
///    `[0x80112DF8]`, `[0x80112DFC]` (the second tested only if the first
///    match): `E200001C`, `w` and `E2001E01`, `w & 3`, `w` the two words
///    OR'ed (re-read for each).
/// 4. With bit 0 of the word `[mat]`, if `force` or the six bytes `mat +
///    0x20` differ from `0x80112E00`: `FA000000 | m << 8 | l`, `r << 24 | g
///    << 16 | b << 8 | a` from those bytes `(m, l, r, g, b, a)`.
/// 5. With bit 1, the same for the four bytes `mat + 0x26` against
///    `0x80112E06`: `FB000000`, `rgba`.
/// 6. Without bit 2, the bytes `mat + 0x2A..+0x2E` first become the low
///    bytes of the four halfwords at `0x800A3D44` ([`func_80038DF8`]'s).
///    Then, if `force` or they differ from `0x80112E0A`: `F8000000`, `rgba`.
/// 7. With bit 3, the four bytes `mat + 0x2E` against `0x80112E0E`:
///    `F9000000`, `rgba`.
///
/// Each command is appended by reading the head, storing it advanced by 8,
/// then its words (in the order the code has them). The flags word is
/// re-read for each test.
///
/// Frame (`sp - 0x20`): `s0` (holding `mat`) at `+0x18` and `ra` at
/// `+0x1C`, both restored sign-extended; `force` spilled to its slot
/// `+0x24`. Leaves `a3 = DL2_HEAD`, `v0` = the last command's address (or
/// the last comparison's result), and the temporaries and callee
/// registers of the last steps.
///
/// Domain: canonical pointers, the list in RDRAM and not overlapping
/// `mat` or the copy.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80035BF0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_80035C7C: {
        g[SP] = addu(g[SP], (-0x20i64) as u64);
        sw(m, g[SP], 0x18, g[S0]);
        g[S0] = g[A0];
        sw(m, g[SP], 0x1C, g[RA]);
        sw(m, g[SP], 0x24, g[A1]);
        if g[A1] == 0 {
            g[T7] = li(0x8011_0000);
            g[T7] = lh(m, g[T7], 0x2DE4);
            g[T8] = lh(m, g[A0], 4);
            if g[T7] == g[T8] {
                g[T5] = lw(m, g[SP], 0x24);
                break 'b_80035C7C;
            }
        }
        g[T9] = lh(m, g[S0], 4);
        g[AT] = 1;
        g[A3] = li(DL2_HEAD);
        if g[T9] != g[AT] {
            g[V0] = lw(m, g[A3], 0);
            g[T3] = li(0xE300_0A01);
            g[T2] = addu(g[V0], 8);
            sw(m, g[A3], 0, g[T2]);
            g[T4] = li(0x10_0000);
            sw(m, g[V0], 4, g[T4]);
            sw(m, g[V0], 0, g[T3]);
        } else {
            g[A3] = li(DL2_HEAD);
            g[V0] = lw(m, g[A3], 0);
            g[T1] = li(0xE300_0A01);
            g[T0] = addu(g[V0], 8);
            sw(m, g[A3], 0, g[T0]);
            sw(m, g[V0], 4, 0);
            sw(m, g[V0], 0, g[T1]);
        }
        g[T5] = lw(m, g[SP], 0x24);
    }
    'b_80035DDC: {
        g[A3] = li(DL2_HEAD);
        g[A0] = addu(g[S0], 6);
        if g[T5] == 0 {
            g[A1] = li(0x8011_2DE6);
            g[A2] = 8;
            call(imports::func_80081530, m, ctx);
            let g = &mut ctx.gpr;
            g[A3] = li(DL2_HEAD);
            if g[V0] != 0 {
                g[A1] = li(0x8011_2DEE);
                g[A0] = addu(g[S0], 0xE);
                g[A2] = 8;
                call(imports::func_80081530, m, ctx);
                let g = &mut ctx.gpr;
                g[A3] = li(DL2_HEAD);
                if g[V0] != 0 {
                    break 'b_80035DDC;
                }
            }
        }
        let g = &mut ctx.gpr;
        g[V0] = lw(m, g[A3], 0);
        g[AT] = li(0xFF_FFFF);
        g[T6] = addu(g[V0], 8);
        sw(m, g[A3], 0, g[T6]);
        g[T9] = lbu(m, g[S0], 0xE);
        g[T7] = lbu(m, g[S0], 0x10);
        g[T3] = lbu(m, g[S0], 6);
        g[T0] = g[T9] & 0xF;
        g[T1] = sll(g[T0], 5);
        g[T8] = g[T7] & 0x1F;
        g[T2] = g[T8] | g[T1];
        g[T4] = g[T3] & 0xF;
        g[T7] = lbu(m, g[S0], 8);
        g[T5] = sll(g[T4], 20);
        g[T1] = lbu(m, g[S0], 0xA);
        g[T6] = g[T2] | g[T5];
        g[T5] = lbu(m, g[S0], 0xC);
        g[T9] = g[T7] & 0x1F;
        g[T0] = sll(g[T9], 15);
        g[T3] = g[T1] & 7;
        g[T4] = sll(g[T3], 12);
        g[T8] = g[T6] | g[T0];
        g[T7] = g[T5] & 7;
        g[T9] = sll(g[T7], 9);
        g[T2] = g[T8] | g[T4];
        g[T6] = g[T2] | g[T9];
        g[T0] = g[T6] & g[AT];
        g[AT] = li(0xFC00_0000);
        g[T1] = g[T0] | g[AT];
        sw(m, g[V0], 0, g[T1]);
        g[T4] = lbu(m, g[S0], 0xF);
        g[T3] = lbu(m, g[S0], 0x15);
        g[T9] = lbu(m, g[S0], 0x12);
        g[T5] = g[T4] & 0xF;
        g[T7] = sll(g[T5], 24);
        g[T8] = g[T3] & 7;
        g[T2] = g[T8] | g[T7];
        g[T6] = g[T9] & 7;
        g[T7] = lbu(m, g[S0], 0x11);
        g[T3] = lbu(m, g[S0], 0x14);
        g[T0] = sll(g[T6], 21);
        g[T1] = g[T2] | g[T0];
        g[T0] = lbu(m, g[S0], 0x13);
        g[T9] = g[T7] & 7;
        g[T4] = g[T3] & 7;
        g[T5] = sll(g[T4], 18);
        g[T7] = lbu(m, g[S0], 7);
        g[T8] = g[T1] | g[T5];
        g[T6] = sll(g[T9], 6);
        g[T3] = g[T0] & 7;
        g[T4] = sll(g[T3], 3);
        g[T2] = g[T8] | g[T6];
        g[T1] = g[T2] | g[T4];
        g[T6] = lbu(m, g[S0], 9);
        g[T9] = sll(g[T7], 28);
        g[T4] = lbu(m, g[S0], 0xB);
        g[T8] = g[T1] | g[T9];
        g[T9] = lbu(m, g[S0], 0xD);
        g[T0] = g[T6] & 7;
        g[T3] = sll(g[T0], 15);
        g[T5] = g[T4] & 7;
        g[T7] = sll(g[T5], 12);
        g[T2] = g[T8] | g[T3];
        g[T6] = g[T9] & 7;
        g[T0] = sll(g[T6], 9);
        g[T1] = g[T2] | g[T7];
        g[T8] = g[T1] | g[T0];
        sw(m, g[V0], 4, g[T8]);
    }
    let g = &mut ctx.gpr;
    'b_80035E68: {
        g[T3] = lw(m, g[SP], 0x24);
        g[T4] = li(0x8011_0000);
        if g[T3] != 0 {
            g[V0] = lw(m, g[A3], 0);
        } else {
            g[T4] = lw(m, g[T4], 0x2DF8);
            g[T5] = lw(m, g[S0], 0x18);
            g[T2] = li(0x8011_0000);
            if g[T4] != g[T5] {
                g[V0] = lw(m, g[A3], 0);
            } else {
                g[T2] = lw(m, g[T2], 0x2DFC);
                g[T7] = lw(m, g[S0], 0x1C);
                if g[T2] == g[T7] {
                    g[T6] = lw(m, g[S0], 0);
                    break 'b_80035E68;
                }
                g[V0] = lw(m, g[A3], 0);
            }
        }
        g[T6] = li(0xE200_001C);
        g[T9] = addu(g[V0], 8);
        sw(m, g[A3], 0, g[T9]);
        sw(m, g[V0], 0, g[T6]);
        g[T0] = lw(m, g[S0], 0x1C);
        g[T1] = lw(m, g[S0], 0x18);
        g[T4] = li(0xE200_1E01);
        g[T8] = g[T1] | g[T0];
        sw(m, g[V0], 4, g[T8]);
        g[V0] = lw(m, g[A3], 0);
        g[T3] = addu(g[V0], 8);
        sw(m, g[A3], 0, g[T3]);
        sw(m, g[V0], 0, g[T4]);
        g[T2] = lw(m, g[S0], 0x1C);
        g[T5] = lw(m, g[S0], 0x18);
        g[T7] = g[T5] | g[T2];
        g[T9] = g[T7] & 3;
        sw(m, g[V0], 4, g[T9]);
        g[T6] = lw(m, g[S0], 0);
    }
    g[T0] = lw(m, g[SP], 0x24);
    g[T1] = g[T6] & 1;
    if g[T1] == 0 {
        g[T4] = lw(m, g[S0], 0);
    } else {
        'b_80035EF0: {
            g[A0] = addu(g[S0], 0x20);
            if g[T0] == 0 {
                g[A1] = li(0x8011_2E00);
                g[A2] = 6;
                call(imports::func_80081530, m, ctx);
                let g = &mut ctx.gpr;
                g[A3] = li(DL2_HEAD);
                if g[V0] != 0 {
                    break 'b_80035EF0;
                }
            }
            let g = &mut ctx.gpr;
            g[V0] = lw(m, g[A3], 0);
            g[AT] = li(0xFA00_0000);
            g[T8] = addu(g[V0], 8);
            sw(m, g[A3], 0, g[T8]);
            g[T7] = lbu(m, g[S0], 0x20);
            g[T4] = lbu(m, g[S0], 0x21);
            g[T9] = sll(g[T7], 8);
            g[T5] = g[T4] | g[AT];
            g[T6] = g[T5] | g[T9];
            sw(m, g[V0], 0, g[T6]);
            g[T3] = lbu(m, g[S0], 0x22);
            g[T5] = lbu(m, g[S0], 0x23);
            g[T0] = lbu(m, g[S0], 0x25);
            g[T8] = lbu(m, g[S0], 0x24);
            g[T4] = sll(g[T3], 24);
            g[T9] = sll(g[T5], 16);
            g[T2] = g[T0] | g[T4];
            g[T6] = g[T2] | g[T9];
            g[T3] = sll(g[T8], 8);
            g[T0] = g[T6] | g[T3];
            sw(m, g[V0], 4, g[T0]);
        }
        let g = &mut ctx.gpr;
        g[T4] = lw(m, g[S0], 0);
    }
    let g = &mut ctx.gpr;
    g[T5] = lw(m, g[SP], 0x24);
    g[T7] = g[T4] & 2;
    if g[T7] == 0 {
        g[T0] = lw(m, g[S0], 0);
    } else {
        'b_80035F68: {
            g[A0] = addu(g[S0], 0x26);
            if g[T5] == 0 {
                g[A1] = li(0x8011_2E06);
                g[A2] = 4;
                call(imports::func_80081530, m, ctx);
                let g = &mut ctx.gpr;
                g[A3] = li(DL2_HEAD);
                if g[V0] != 0 {
                    break 'b_80035F68;
                }
            }
            let g = &mut ctx.gpr;
            g[V0] = lw(m, g[A3], 0);
            g[T9] = li(0xFB00_0000);
            g[T2] = addu(g[V0], 8);
            sw(m, g[A3], 0, g[T2]);
            sw(m, g[V0], 0, g[T9]);
            g[T3] = lbu(m, g[S0], 0x26);
            g[T5] = lbu(m, g[S0], 0x27);
            g[T8] = lbu(m, g[S0], 0x29);
            g[T6] = lbu(m, g[S0], 0x28);
            g[T0] = sll(g[T3], 24);
            g[T2] = sll(g[T5], 16);
            g[T4] = g[T8] | g[T0];
            g[T9] = g[T4] | g[T2];
            g[T3] = sll(g[T6], 8);
            g[T8] = g[T9] | g[T3];
            sw(m, g[V0], 4, g[T8]);
        }
        let g = &mut ctx.gpr;
        g[T0] = lw(m, g[S0], 0);
    }
    let g = &mut ctx.gpr;
    g[V0] = li(0x800A_3D44);
    g[T7] = g[T0] & 4;
    g[A0] = addu(g[S0], 0x2A);
    if g[T7] == 0 {
        g[T5] = lh(m, g[V0], 0);
        sb(m, g[S0], 0x2A, g[T5]);
        g[T4] = lh(m, g[V0], 2);
        sb(m, g[S0], 0x2B, g[T4]);
        g[T2] = lh(m, g[V0], 4);
        sb(m, g[S0], 0x2C, g[T2]);
        g[T1] = lh(m, g[V0], 6);
        sb(m, g[S0], 0x2D, g[T1]);
    }
    'b_80036008: {
        g[T6] = lw(m, g[SP], 0x24);
        g[A1] = li(0x8011_2E0A);
        if g[T6] != 0 {
            g[V0] = lw(m, g[A3], 0);
        } else {
            g[A2] = 4;
            call(imports::func_80081530, m, ctx);
            let g = &mut ctx.gpr;
            g[A3] = li(DL2_HEAD);
            if g[V0] != 0 {
                break 'b_80036008;
            }
            g[V0] = lw(m, g[A3], 0);
        }
        let g = &mut ctx.gpr;
        g[T3] = li(0xF800_0000);
        g[T9] = addu(g[V0], 8);
        sw(m, g[A3], 0, g[T9]);
        sw(m, g[V0], 0, g[T3]);
        g[T5] = lbu(m, g[S0], 0x2A);
        g[T6] = lbu(m, g[S0], 0x2B);
        g[T0] = lbu(m, g[S0], 0x2D);
        g[T7] = lbu(m, g[S0], 0x2C);
        g[T4] = sll(g[T5], 24);
        g[T9] = sll(g[T6], 16);
        g[T2] = g[T0] | g[T4];
        g[T3] = g[T2] | g[T9];
        g[T5] = sll(g[T7], 8);
        g[T0] = g[T3] | g[T5];
        sw(m, g[V0], 4, g[T0]);
    }
    let g = &mut ctx.gpr;
    g[T4] = lw(m, g[S0], 0);
    g[T6] = lw(m, g[SP], 0x24);
    g[T1] = g[T4] & 8;
    if g[T1] == 0 {
        g[RA] = lw(m, g[SP], 0x1C);
    } else {
        'b_80036080: {
            g[A0] = addu(g[S0], 0x2E);
            if g[T6] == 0 {
                g[A1] = li(0x8011_2E0E);
                g[A2] = 4;
                call(imports::func_80081530, m, ctx);
                let g = &mut ctx.gpr;
                g[A3] = li(DL2_HEAD);
                if g[V0] != 0 {
                    break 'b_80036080;
                }
            }
            let g = &mut ctx.gpr;
            g[V0] = lw(m, g[A3], 0);
            g[T9] = li(0xF900_0000);
            g[T2] = addu(g[V0], 8);
            sw(m, g[A3], 0, g[T2]);
            sw(m, g[V0], 0, g[T9]);
            g[T5] = lbu(m, g[S0], 0x2E);
            g[T6] = lbu(m, g[S0], 0x2F);
            g[T7] = lbu(m, g[S0], 0x31);
            g[T3] = lbu(m, g[S0], 0x30);
            g[T0] = sll(g[T5], 24);
            g[T2] = sll(g[T6], 16);
            g[T4] = g[T7] | g[T0];
            g[T9] = g[T4] | g[T2];
            g[T5] = sll(g[T3], 8);
            g[T7] = g[T9] | g[T5];
            sw(m, g[V0], 4, g[T7]);
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x1C);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_8003609C(mode, cur)` (render mode switches, **guess**): if `mode`
/// differs from the word `*cur`, for each two-bit field of `mode` (bits
/// 0-1, 2-3, 4-5, 6-7, in that order, `*cur` re-read for each) that
/// differs from `*cur`'s: the field is stored into `*cur` (cleared, stored,
/// then set, stored), and a `SetOtherMode_H` command appended to the list
/// at `[0x80112C90]`, by the field's value:
/// - bits 0-1: 0 `E3001001 0`, 1 `E3001001 8000`, 2 `E3001001 C000`;
/// - bits 2-3: 0 `E3000D01 0`, 4 `E3000D01 20000`, 8 `E3000D01 40000`;
/// - bits 4-5: 0 `E3001201 2000`, 0x10 `E3001201 0`, 0x20 `E3001201 3000`;
/// - bits 6-7: 0 `E3000F00 0`, 0x40 `E3000F00 10000`;
/// - the other value of each field (all bits set; 0x80 for bits 6-7 too)
///   appends nothing. Each command's second word is stored first.
///
/// Leaves `v1` = the list head's address (past bits 0-1), `a2`/`at` the
/// last field and mask, `v0`, `t0`..`t9` from the path taken.
///
/// Domain: canonical pointers; the list in RDRAM, not overlapping `cur`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003609C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, g[A1], 0);
    g[V1] = g[A0] & 3;
    g[T6] = g[V0] & 3;
    if g[A0] != g[V0] {
        // Bits 0-1.
        g[AT] = (-4i64) as u64;
        if g[V1] != g[T6] {
            g[T7] = g[V0] & g[AT];
            sw(m, g[A1], 0, g[T7]);
            g[T9] = g[T7] | g[V1];
            sw(m, g[A1], 0, g[T9]);
            if g[V1] != 0 {
                g[AT] = 1;
                if g[V1] != g[AT] {
                    g[AT] = 2;
                    let c0 = g[V1] == g[AT];
                    g[V1] = li(0x8011_0000);
                    if c0 {
                        g[V1] = addu(g[V1], 0x2C90);
                        g[V0] = lw(m, g[V1], 0);
                        g[T6] = li(0xE300_1001);
                        g[T5] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T5]);
                        g[T7] = 0xC000;
                        sw(m, g[V0], 4, g[T7]);
                        sw(m, g[V0], 0, g[T6]);
                    }
                } else {
                    g[V1] = li(0x8011_2C90);
                    g[V0] = lw(m, g[V1], 0);
                    g[T3] = li(0xE300_1001);
                    g[T2] = addu(g[V0], 8);
                    sw(m, g[V1], 0, g[T2]);
                    g[T4] = 0x8000;
                    sw(m, g[V0], 4, g[T4]);
                    sw(m, g[V0], 0, g[T3]);
                }
            } else {
                g[V1] = li(0x8011_2C90);
                g[V0] = lw(m, g[V1], 0);
                g[T1] = li(0xE300_1001);
                g[T0] = addu(g[V0], 8);
                sw(m, g[V1], 0, g[T0]);
                sw(m, g[V0], 4, 0);
                sw(m, g[V0], 0, g[T1]);
            }
        }
        // Bits 2-3; v1 = the list head from here on.
        g[V0] = lw(m, g[A1], 0);
        g[V1] = li(0x8011_0000);
        g[A2] = g[A0] & 0xC;
        g[T8] = g[V0] & 0xC;
        g[V1] = addu(g[V1], 0x2C90);
        if g[A2] != g[T8] {
            g[AT] = (-0xDi64) as u64;
            g[T9] = g[V0] & g[AT];
            sw(m, g[A1], 0, g[T9]);
            g[T1] = g[T9] | g[A2];
            sw(m, g[A1], 0, g[T1]);
            if g[A2] != 0 {
                g[AT] = 4;
                g[T5] = li(0xE300_0000);
                if g[A2] != g[AT] {
                    g[AT] = 8;
                    g[T8] = li(0xE300_0000);
                    if g[A2] == g[AT] {
                        g[V0] = lw(m, g[V1], 0);
                        g[T8] = g[T8] | 0xD01;
                        g[T9] = li(0x4_0000);
                        g[T7] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T7]);
                        sw(m, g[V0], 4, g[T9]);
                        sw(m, g[V0], 0, g[T8]);
                    }
                } else {
                    g[V0] = lw(m, g[V1], 0);
                    g[T5] = g[T5] | 0xD01;
                    g[T6] = li(0x2_0000);
                    g[T4] = addu(g[V0], 8);
                    sw(m, g[V1], 0, g[T4]);
                    sw(m, g[V0], 4, g[T6]);
                    sw(m, g[V0], 0, g[T5]);
                }
            } else {
                g[V0] = lw(m, g[V1], 0);
                g[T3] = li(0xE300_0D01);
                g[T2] = addu(g[V0], 8);
                sw(m, g[V1], 0, g[T2]);
                sw(m, g[V0], 4, 0);
                sw(m, g[V0], 0, g[T3]);
            }
        }
        // Bits 4-5.
        g[V0] = lw(m, g[A1], 0);
        g[A2] = g[A0] & 0x30;
        g[T0] = g[V0] & 0x30;
        g[AT] = (-0x31i64) as u64;
        if g[A2] != g[T0] {
            g[T1] = g[V0] & g[AT];
            sw(m, g[A1], 0, g[T1]);
            g[T3] = g[T1] | g[A2];
            sw(m, g[A1], 0, g[T3]);
            if g[A2] != 0 {
                g[AT] = 0x10;
                if g[A2] != g[AT] {
                    g[AT] = 0x20;
                    g[T0] = li(0xE300_0000);
                    if g[A2] == g[AT] {
                        g[V0] = lw(m, g[V1], 0);
                        g[T0] = g[T0] | 0x1201;
                        g[T1] = 0x3000;
                        g[T9] = addu(g[V0], 8);
                        sw(m, g[V1], 0, g[T9]);
                        sw(m, g[V0], 4, g[T1]);
                        sw(m, g[V0], 0, g[T0]);
                    }
                } else {
                    g[V0] = lw(m, g[V1], 0);
                    g[T8] = li(0xE300_1201);
                    g[T7] = addu(g[V0], 8);
                    sw(m, g[V1], 0, g[T7]);
                    sw(m, g[V0], 4, 0);
                    sw(m, g[V0], 0, g[T8]);
                }
            } else {
                g[V0] = lw(m, g[V1], 0);
                g[T5] = li(0xE300_1201);
                g[T4] = addu(g[V0], 8);
                sw(m, g[V1], 0, g[T4]);
                g[T6] = 0x2000;
                sw(m, g[V0], 4, g[T6]);
                sw(m, g[V0], 0, g[T5]);
            }
        }
        // Bits 6-7.
        g[V0] = lw(m, g[A1], 0);
        g[A2] = g[A0] & 0xC0;
        g[T2] = g[V0] & 0xC0;
        g[AT] = (-0xC1i64) as u64;
        if g[A2] != g[T2] {
            g[T3] = g[V0] & g[AT];
            sw(m, g[A1], 0, g[T3]);
            g[T5] = g[T3] | g[A2];
            sw(m, g[A1], 0, g[T5]);
            if g[A2] == 0 {
                g[V0] = lw(m, g[V1], 0);
                g[T7] = li(0xE300_0F00);
                g[T6] = addu(g[V0], 8);
                sw(m, g[V1], 0, g[T6]);
                sw(m, g[V0], 4, 0);
                sw(m, g[V0], 0, g[T7]);
                return;
            }
            g[AT] = 0x40;
            g[T9] = li(0xE300_0000);
            if g[A2] == g[AT] {
                g[V0] = lw(m, g[V1], 0);
                g[T9] = g[T9] | 0xF00;
                g[T0] = li(0x1_0000);
                g[T8] = addu(g[V0], 8);
                sw(m, g[V1], 0, g[T8]);
                sw(m, g[V0], 4, g[T0]);
                sw(m, g[V0], 0, g[T9]);
            }
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

/// `func_80036314(mat, s, t)` (a material's texture, **guess**): with `s`,
/// `t` taken as s16, appends to the list at [`DL2_HEAD`]:
/// 1. the render mode switches for the word `[mat]` against
///    `0x80112E14` ([`func_8003609C`]);
/// 2. `gSPTexture`: `D7000002 | (b[0xF] & 7) << 11 | (b[0xE] & 7) << 8`,
///    `h[0x14] << 16 | (h[0x16] & 0xFFFF)` (`b[k]`, `h[k]` the byte and s16
///    at `mat + k`);
/// 3. unless bit 8 of `[mat]` is set, the texture load, with `fmt =
///    (b[0xC] & 7) << 21` and `siz = 3 << 19` if `b[0xD] == 3`, else `2 <<
///    19`: `FD000000 | fmt | siz`, `[mat + 0x38]`; `F5000000 | fmt | siz`,
///    `0x07000000`; `E6000000`, `0`; `F3000000`, `0x07000000 | (min(h[0x1A],
///    0x7FF) & 0xFFF) << 12 | (h[0x18] & 0xFFF)` (the minimum signed);
/// 4. if `[mat + 0x3C]` is nonzero, its palette: `FD100000`, `[mat +
///    0x3C]`; `E8000000`, `0`; `F5000100`, `0x07000000`; `E6000000`, `0`;
///    `F0000000`, `0x073FC000` if `b[0xD] == 1`, else `0x0703C000`;
/// 5. for each tile `k = 0..6` whose pointer `e = [mat + 0x1C + 4k]` is
///    nonzero (`d[k]` its bytes, `e16[k]` its s16s): `F5000000 | (b[0xC] &
///    7) << 21 | (b[0xD] & 3) << 19 | d[2] << 9 | (e16[0] & 0x1FF)`, `k <<
///    24 | (d[3] & 3) << 18 | (d[5] & 0xF) << 14 | (d[7] & 0xF) << 10 |
///    ((d[3] & 0xF0) >> 4 & 3) << 8 | (d[4] & 0xF) << 4 | (d[6] & 0xF)`;
///    then `F2000000 | ((e16[8] + s) & 0xFFF) << 12 | ((e16[0xA] + t) &
///    0xFFF)`, `k << 24 | ((e16[0xC] + s) & 0xFFF) << 12 | ((e16[0xE] + t)
///    & 0xFFF)`.
///
/// The tile loop is skipped only for `mat == -0x1C` (the compiled null
/// test of `mat + 0x1C`). Each command is appended by reading the head,
/// storing it advanced by 8, then its words in the code's order.
///
/// Frame (`sp - 0x28`): `s0` (`mat`), `s1` (`s`), `s2` (`t`) at
/// `+0x18..+0x20` and `ra` at `+0x24`, all restored sign-extended; `s`,
/// `t` spilled to their slots `+0x2C`/`+0x30`. Leaves `t2 = DL2_HEAD`,
/// after the loop `t0 = 7` and `t1 = mat + 0x1C`, `t3 = 0xF2000000`, `t4 =
/// 7`, `t5 = 0xF5000000`, and the temporaries of the last tile and steps.
///
/// Domain: canonical pointers; the list in RDRAM and not overlapping
/// `mat`, its tiles or the mode copy.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80036314(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x28i64) as u64);
    sw(m, g[SP], 0x1C, g[S1]);
    g[S1] = sll(g[A1], 16);
    sw(m, g[SP], 0x20, g[S2]);
    g[S2] = sll(g[A2], 16);
    sw(m, g[SP], 0x18, g[S0]);
    sw(m, g[SP], 0x2C, g[A1]);
    g[S0] = g[A0];
    g[T7] = sra(g[S2], 16);
    g[T6] = sra(g[S1], 16);
    sw(m, g[SP], 0x24, g[RA]);
    sw(m, g[SP], 0x30, g[A2]);
    g[A1] = li(0x8011_0000);
    g[S1] = g[T6];
    g[S2] = g[T7];
    g[A1] = addu(g[A1], 0x2E14);
    g[A0] = lw(m, g[A0], 0);
    call(imports::func_8003609C, m, ctx);
    let g = &mut ctx.gpr;
    g[T2] = li(DL2_HEAD);
    g[V1] = lw(m, g[T2], 0);
    g[AT] = li(0xD700_0000);
    g[T5] = li(0xF500_0000);
    g[T8] = addu(g[V1], 8);
    sw(m, g[T2], 0, g[T8]);
    g[T9] = lbu(m, g[S0], 0xF);
    g[T4] = 7;
    g[T6] = g[T9] & 7;
    g[T9] = lbu(m, g[S0], 0xE);
    g[T7] = sll(g[T6], 11);
    g[T8] = g[T7] | g[AT];
    g[T6] = g[T9] & 7;
    g[T7] = sll(g[T6], 8);
    g[T9] = g[T8] | g[T7];
    g[T6] = g[T9] | 2;
    sw(m, g[V1], 0, g[T6]);
    g[T6] = lh(m, g[S0], 0x16);
    g[T7] = lh(m, g[S0], 0x14);
    g[T8] = g[T6] & 0xFFFF;
    g[T9] = sll(g[T7], 16);
    g[T7] = g[T9] | g[T8];
    sw(m, g[V1], 4, g[T7]);
    g[T6] = lw(m, g[S0], 0);
    g[T9] = g[T6] & 0x100;
    if g[T9] == 0 {
        g[T8] = lbu(m, g[S0], 0xD);
        g[AT] = 3;
        g[A3] = 2;
        g[V0] = 0x7FF;
        if g[T8] == g[AT] {
            g[A3] = 3;
        }
        g[V1] = lw(m, g[T2], 0);
        g[A0] = g[A3] & 3;
        g[T6] = sll(g[A0], 19);
        g[T7] = addu(g[V1], 8);
        sw(m, g[T2], 0, g[T7]);
        g[T9] = lbu(m, g[S0], 0xC);
        g[A0] = g[T6];
        g[AT] = li(0xFD00_0000);
        g[T8] = g[T9] & 7;
        g[T7] = sll(g[T8], 21);
        g[T6] = g[T7] | g[AT];
        g[T9] = g[T6] | g[A0];
        sw(m, g[V1], 0, g[T9]);
        g[T8] = lw(m, g[S0], 0x38);
        g[T5] = li(0xF500_0000);
        g[T3] = li(0x700_0000);
        sw(m, g[V1], 4, g[T8]);
        g[V1] = lw(m, g[T2], 0);
        g[T7] = addu(g[V1], 8);
        sw(m, g[T2], 0, g[T7]);
        g[T6] = lbu(m, g[S0], 0xC);
        sw(m, g[V1], 4, g[T3]);
        g[T9] = g[T6] & 7;
        g[T8] = sll(g[T9], 21);
        g[T7] = g[T8] | g[T5];
        g[T6] = g[T7] | g[A0];
        sw(m, g[V1], 0, g[T6]);
        g[V1] = lw(m, g[T2], 0);
        g[T8] = li(0xE600_0000);
        g[T6] = li(0xF300_0000);
        g[T9] = addu(g[V1], 8);
        sw(m, g[T2], 0, g[T9]);
        sw(m, g[V1], 4, 0);
        sw(m, g[V1], 0, g[T8]);
        g[T0] = lw(m, g[T2], 0);
        g[T7] = addu(g[T0], 8);
        sw(m, g[T2], 0, g[T7]);
        sw(m, g[T0], 0, g[T6]);
        g[T1] = lh(m, g[S0], 0x1A);
        g[AT] = slt(g[T1], 0x7FF);
        if g[AT] != 0 {
            g[V0] = g[T1];
        }
        g[T6] = lh(m, g[S0], 0x18);
        g[T9] = g[V0] & 0xFFF;
        g[T8] = sll(g[T9], 12);
        g[T7] = g[T8] | g[T3];
        g[T9] = g[T6] & 0xFFF;
        g[T8] = g[T7] | g[T9];
        sw(m, g[T0], 4, g[T8]);
    } else {
        g[T3] = li(0x700_0000);
    }
    g[T6] = lw(m, g[S0], 0x3C);
    g[T0] = 0;
    g[T1] = g[S0];
    if g[T6] == 0 {
        g[AT] = (-0x1Ci64) as u64;
    } else {
        g[V1] = lw(m, g[T2], 0);
        g[T9] = li(0xFD10_0000);
        g[AT] = 1;
        g[T7] = addu(g[V1], 8);
        sw(m, g[T2], 0, g[T7]);
        sw(m, g[V1], 0, g[T9]);
        g[T8] = lw(m, g[S0], 0x3C);
        g[T7] = li(0xE800_0000);
        sw(m, g[V1], 4, g[T8]);
        g[V1] = lw(m, g[T2], 0);
        g[T8] = li(0xF500_0100);
        g[T6] = addu(g[V1], 8);
        sw(m, g[T2], 0, g[T6]);
        sw(m, g[V1], 4, 0);
        sw(m, g[V1], 0, g[T7]);
        g[V1] = lw(m, g[T2], 0);
        g[T7] = li(0xE600_0000);
        g[T9] = addu(g[V1], 8);
        sw(m, g[T2], 0, g[T9]);
        sw(m, g[V1], 4, g[T3]);
        sw(m, g[V1], 0, g[T8]);
        g[V1] = lw(m, g[T2], 0);
        g[T8] = li(0xF000_0000);
        g[T6] = addu(g[V1], 8);
        sw(m, g[T2], 0, g[T6]);
        sw(m, g[V1], 4, 0);
        sw(m, g[V1], 0, g[T7]);
        g[T9] = lbu(m, g[S0], 0xD);
        g[T6] = li(0xF000_0000);
        if g[T9] != g[AT] {
            g[V1] = lw(m, g[T2], 0);
            g[T6] = li(0x703_C000);
            g[T9] = addu(g[V1], 8);
            sw(m, g[T2], 0, g[T9]);
            sw(m, g[V1], 4, g[T6]);
            sw(m, g[V1], 0, g[T8]);
        } else {
            g[V1] = lw(m, g[T2], 0);
            g[T7] = li(0x73F_C000);
            g[T8] = addu(g[V1], 8);
            sw(m, g[T2], 0, g[T8]);
            sw(m, g[V1], 4, g[T7]);
            sw(m, g[V1], 0, g[T6]);
        }
        g[AT] = (-0x1Ci64) as u64;
    }
    g[T3] = li(0xF200_0000);
    if g[S0] != g[AT] {
        loop {
            g[V0] = lw(m, g[T1], 0x1C);
            if g[V0] == 0 {
                g[T0] = addu(g[T0], 1);
            } else {
                g[V1] = lw(m, g[T2], 0);
                g[A1] = g[T0] & 7;
                g[T7] = addu(g[V1], 8);
                sw(m, g[T2], 0, g[T7]);
                g[T9] = lbu(m, g[S0], 0xC);
                g[T8] = g[T9] & 7;
                g[T9] = lbu(m, g[S0], 0xD);
                g[T6] = sll(g[T8], 21);
                g[T7] = g[T6] | g[T5];
                g[T8] = g[T9] & 3;
                g[T6] = sll(g[T8], 19);
                g[T9] = g[T7] | g[T6];
                g[T7] = lbu(m, g[V0], 2);
                g[T6] = sll(g[T7], 9);
                g[T7] = lh(m, g[V0], 0);
                g[T8] = g[T9] | g[T6];
                g[T9] = g[T7] & 0x1FF;
                g[T6] = g[T8] | g[T9];
                sw(m, g[V1], 0, g[T6]);
                g[A2] = lbu(m, g[V0], 3);
                g[T8] = lbu(m, g[V0], 5);
                g[T7] = sll(g[A1], 24);
                g[T9] = g[A2] & 3;
                g[T6] = sll(g[T9], 18);
                g[A1] = g[T7];
                g[T7] = g[T7] | g[T6];
                g[T9] = g[T8] & 0xF;
                g[T6] = sll(g[T9], 14);
                g[T9] = lbu(m, g[V0], 7);
                g[T8] = g[T7] | g[T6];
                g[T7] = g[T9] & 0xF;
                g[T6] = sll(g[T7], 10);
                g[T9] = g[T8] | g[T6];
                g[T7] = g[A2] & 0xF0;
                g[T8] = sra(g[T7], 4);
                g[T6] = g[T8] & 3;
                g[T7] = sll(g[T6], 8);
                g[T6] = lbu(m, g[V0], 4);
                g[T8] = g[T9] | g[T7];
                g[T9] = g[T6] & 0xF;
                g[T7] = sll(g[T9], 4);
                g[T9] = lbu(m, g[V0], 6);
                g[T6] = g[T8] | g[T7];
                g[T8] = g[T9] & 0xF;
                g[T7] = g[T6] | g[T8];
                sw(m, g[V1], 4, g[T7]);
                g[V1] = lw(m, g[T2], 0);
                g[T9] = addu(g[V1], 8);
                sw(m, g[T2], 0, g[T9]);
                g[T6] = lh(m, g[V0], 8);
                g[T8] = addu(g[T6], g[S1]);
                g[T7] = g[T8] & 0xFFF;
                g[T8] = lh(m, g[V0], 0xA);
                g[T9] = sll(g[T7], 12);
                g[T6] = g[T9] | g[T3];
                g[T7] = addu(g[T8], g[S2]);
                g[T9] = g[T7] & 0xFFF;
                g[T8] = g[T6] | g[T9];
                sw(m, g[V1], 0, g[T8]);
                g[T7] = lh(m, g[V0], 0xC);
                g[T6] = addu(g[T7], g[S1]);
                g[T9] = g[T6] & 0xFFF;
                g[T6] = lh(m, g[V0], 0xE);
                g[T8] = sll(g[T9], 12);
                g[T7] = g[A1] | g[T8];
                g[T9] = addu(g[T6], g[S2]);
                g[T8] = g[T9] & 0xFFF;
                g[T6] = g[T7] | g[T8];
                sw(m, g[V1], 4, g[T6]);
                g[T0] = addu(g[T0], 1);
            }
            g[T1] = addu(g[T1], 4);
            if g[T0] == g[T4] {
                break;
            }
        }
    }
    g[RA] = lw(m, g[SP], 0x24);
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x28);
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

/// `func_8003B860(dl, x0, x1, y0, y1, px, py, sa, sb, ta, tb)` (a screen
/// quad with one corner moved, **guess**): like [`func_8003B324`] without
/// the rotation: appends `gSPVertex(0x800A4920, 4, 0)`, `G_MODIFYVTX` Z = 0
/// for vertices 0..3, XY for each (`x << 16 | (y & 0xFFFF)`), ST
/// (`0x02140000 | 2k`: vertex 0 `(sa, ta)`, 1 `(sb, ta)`, 2 `(sb, tb)`, 3
/// `(sa, tb)`) and `G_TRI2 06000402 00000604`, 14 commands, and advances
/// `*dl`. The corners are (x0, y0), (x1, y0), (x1, y1), (x0, y1), except
/// that the one whose quadrant of `(x0, y0)` holds `(px, py)` becomes `(px,
/// py)`: vertex 0 if `px < x0` and `py < y0`, 1 if not `px < x0` and `py <
/// y0`, 2 if neither, 3 if `px < x0` and not `py < y0` (signed compares;
/// the arguments after `y0` are on the stack). `dl` is spilled to its home
/// slot `sp + 0`.
///
/// Leaves `v0` = the new end, `t7 = dl`, `t0` = the TRI2's address, `a3`,
/// `a0`, `t2` three of the commands before it, `a1 = sa << 16`, `v1 = tb`,
/// and `t4`..`t9` the last words.
///
/// Domain: canonical pointers; the list in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003B860(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8003B91C: {
        sw(m, g[SP], 0, g[A0]);
        g[V0] = lw(m, g[A0], 0);
        g[T7] = li(0x100_0000);
        g[T8] = li(0x800A_0000);
        g[V1] = g[V0];
        g[V0] = addu(g[V0], 8);
        g[T8] = addu(g[T8], 0x4920);
        g[T7] = g[T7] | 0x4008;
        g[T0] = g[V0];
        sw(m, g[V1], 0, g[T7]);
        sw(m, g[V1], 4, g[T8]);
        g[V0] = addu(g[V0], 8);
        g[T9] = li(0x21C_0000);
        g[T1] = g[V0];
        sw(m, g[T0], 0, g[T9]);
        sw(m, g[T0], 4, 0);
        g[V0] = addu(g[V0], 8);
        g[T4] = li(0x21C_0002);
        g[T2] = g[V0];
        sw(m, g[T1], 0, g[T4]);
        sw(m, g[T1], 4, 0);
        g[T5] = li(0x21C_0004);
        g[V0] = addu(g[V0], 8);
        g[T6] = li(0x21C_0000);
        sw(m, g[T2], 0, g[T5]);
        sw(m, g[T2], 4, 0);
        g[T6] = g[T6] | 6;
        g[T3] = g[V0];
        sw(m, g[T3], 0, g[T6]);
        g[T0] = lw(m, g[SP], 0x14);
        g[V0] = addu(g[V0], 8);
        sw(m, g[T3], 4, 0);
        g[AT] = slt(g[T0], g[A1]);
        g[T2] = g[V0];
        if g[AT] == 0 {
            g[T1] = lw(m, g[SP], 0x18);
        } else {
            g[T1] = lw(m, g[SP], 0x18);
            g[V1] = g[T0];
            g[AT] = slt(g[T1], g[A3]);
            if g[AT] != 0 {
                g[A0] = g[T1];
                break 'b_8003B91C;
            }
            g[T1] = lw(m, g[SP], 0x18);
        }
        g[V1] = g[A1];
        g[A0] = g[A3];
    }
    'b_8003B95C: {
        g[T9] = sll(g[V1], 16);
        g[T4] = g[A0] & 0xFFFF;
        g[T5] = g[T9] | g[T4];
        g[T7] = li(0x218_0000);
        g[AT] = slt(g[T0], g[A1]);
        sw(m, g[T2], 0, g[T7]);
        sw(m, g[T2], 4, g[T5]);
        g[V0] = addu(g[V0], 8);
        if g[AT] == 0 {
            g[AT] = slt(g[T1], g[A3]);
            g[V1] = g[T0];
            if g[AT] != 0 {
                g[A0] = g[T1];
                break 'b_8003B95C;
            }
        }
        g[V1] = g[A2];
        g[A0] = g[A3];
    }
    'b_8003B9A4: {
        g[T2] = g[V0];
        g[T6] = li(0x218_0000);
        g[T8] = sll(g[V1], 16);
        g[T9] = g[A0] & 0xFFFF;
        g[T4] = g[T8] | g[T9];
        g[T6] = g[T6] | 2;
        g[AT] = slt(g[T0], g[A1]);
        sw(m, g[T2], 0, g[T6]);
        sw(m, g[T2], 4, g[T4]);
        g[V0] = addu(g[V0], 8);
        if g[AT] == 0 {
            g[AT] = slt(g[T1], g[A3]);
            g[V1] = g[T0];
            if g[AT] == 0 {
                g[A0] = g[T1];
                break 'b_8003B9A4;
            }
        }
        g[V1] = g[A2];
        g[A0] = lw(m, g[SP], 0x10);
    }
    'b_8003B9EC: {
        g[A2] = g[V0];
        g[T5] = li(0x218_0000);
        g[T7] = sll(g[V1], 16);
        g[T8] = g[A0] & 0xFFFF;
        g[T9] = g[T7] | g[T8];
        g[T5] = g[T5] | 4;
        g[AT] = slt(g[T0], g[A1]);
        sw(m, g[A2], 0, g[T5]);
        sw(m, g[A2], 4, g[T9]);
        g[V0] = addu(g[V0], 8);
        if g[AT] != 0 {
            g[AT] = slt(g[T1], g[A3]);
            g[V1] = g[T0];
            if g[AT] == 0 {
                g[A0] = g[T1];
                break 'b_8003B9EC;
            }
        }
        g[V1] = g[A1];
        g[A0] = lw(m, g[SP], 0x10);
    }
    g[A3] = g[V0];
    g[T4] = li(0x218_0000);
    g[T6] = sll(g[V1], 16);
    g[T7] = g[A0] & 0xFFFF;
    g[T8] = g[T6] | g[T7];
    g[T4] = g[T4] | 6;
    sw(m, g[A3], 0, g[T4]);
    sw(m, g[A3], 4, g[T8]);
    g[T9] = li(0x214_0000);
    sw(m, g[V0], 8, g[T9]);
    g[T1] = lw(m, g[SP], 0x24);
    g[T4] = lw(m, g[SP], 0x1C);
    g[V0] = addu(g[V0], 8);
    g[T6] = g[T1] & 0xFFFF;
    g[A1] = sll(g[T4], 16);
    g[T7] = g[A1] | g[T6];
    g[T8] = li(0x214_0000);
    sw(m, g[V0], 4, g[T7]);
    g[T2] = addu(g[V0], 8);
    g[T8] = g[T8] | 2;
    sw(m, g[T2], 0, g[T8]);
    g[T9] = lw(m, g[SP], 0x20);
    g[A0] = addu(g[T2], 8);
    g[A3] = addu(g[A0], 8);
    g[T4] = sll(g[T9], 16);
    g[T5] = g[T4] | g[T6];
    g[T6] = li(0x214_0000);
    sw(m, g[T2], 4, g[T5]);
    g[T6] = g[T6] | 4;
    sw(m, g[A0], 0, g[T6]);
    g[V1] = lw(m, g[SP], 0x28);
    g[T9] = li(0x214_0006);
    g[T7] = g[V1] & 0xFFFF;
    g[T8] = g[T4] | g[T7];
    sw(m, g[A0], 4, g[T8]);
    g[T4] = g[A1] | g[T7];
    sw(m, g[A3], 4, g[T4]);
    sw(m, g[A3], 0, g[T9]);
    g[T0] = addu(g[A3], 8);
    g[T5] = li(0x600_0402);
    g[T6] = 0x604;
    sw(m, g[T0], 4, g[T6]);
    sw(m, g[T0], 0, g[T5]);
    g[T7] = lw(m, g[SP], 0);
    g[V0] = addu(g[T0], 8);
    sw(m, g[T7], 0, g[V0]);
}

/// `func_8003D110()` (render state reset, **guess**): zeroes the words
/// `[0x800A4960]`, `[0x80114548]`, `[0x8011454C]`, sets `[0x80114540] =
/// 320`, `[0x80114544] = 240`, and appends to the list at [`DL_HEAD`]:
/// `E7000000 0` (pipe sync), `E3000A01 0`, `D7000002 80008000` (texture,
/// stored first word first), `E2001E01 0`, `E3000C00 0`, `E3001201 0`,
/// `E3001402 00000C00`, `E3000D01 0`, `E3000F00 0`, `E3001001 0`,
/// `E200001C 0F0A7008`, `DC38000E 800A3C80` (a `G_MOVEMEM` from
/// `0x800A3C80`), `DB0C0000 00010000`; then `E3001801 000000C0` if bit 11
/// of the settings word `[0x800D697C]` is set, else `E3001801` with the
/// sign-extended halfword `[0x8011447C]`; then `E3001A01 00000030`; and
/// finally `[0x800A48D0] = 0`. Each command's second word is stored first
/// unless noted.
///
/// Leaves `at = 0x800A0000`, `v1` = [`DL_HEAD`]'s address, `v0` = the last
/// command's address, `t6`..`t9` its words and the flag test.
///
/// Domain: the list in RDRAM, not overlapping the globals.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003D110(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x4960, 0);
    g[AT] = li(0x8011_0000);
    sw(m, g[AT], 0x4548, 0);
    g[AT] = li(0x8011_0000);
    sw(m, g[AT], 0x454C, 0);
    g[AT] = li(0x8011_0000);
    g[T6] = 0x140;
    sw(m, g[AT], 0x4540, g[T6]);
    g[V1] = li(0x8012_0000);
    g[AT] = li(0x8011_0000);
    g[T7] = 0xF0;
    g[V1] = addu(g[V1], 0x17B0);
    sw(m, g[AT], 0x4544, g[T7]);
    g[V0] = lw(m, g[V1], 0);
    g[T9] = li(0xE700_0000);
    g[T7] = li(0xE300_0000);
    g[T8] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T8]);
    sw(m, g[V0], 4, 0);
    sw(m, g[V0], 0, g[T9]);
    g[V0] = lw(m, g[V1], 0);
    g[T7] = g[T7] | 0xA01;
    g[T9] = li(0xD700_0000);
    g[T6] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T6]);
    sw(m, g[V0], 4, 0);
    sw(m, g[V0], 0, g[T7]);
    g[V0] = lw(m, g[V1], 0);
    g[T6] = li(0x8000_8000);
    g[T8] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T8]);
    g[T9] = g[T9] | 2;
    sw(m, g[V0], 0, g[T9]);
    sw(m, g[V0], 4, g[T6]);
    g[V0] = lw(m, g[V1], 0);
    g[T8] = li(0xE200_1E01);
    g[T7] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T7]);
    sw(m, g[V0], 4, 0);
    sw(m, g[V0], 0, g[T8]);
    g[V0] = lw(m, g[V1], 0);
    g[T6] = li(0xE300_0C00);
    g[T9] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T9]);
    sw(m, g[V0], 4, 0);
    sw(m, g[V0], 0, g[T6]);
    g[V0] = lw(m, g[V1], 0);
    g[T8] = li(0xE300_1201);
    g[T7] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T7]);
    sw(m, g[V0], 4, 0);
    sw(m, g[V0], 0, g[T8]);
    g[V0] = lw(m, g[V1], 0);
    g[T6] = li(0xE300_1402);
    g[T9] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T9]);
    g[T7] = 0xC00;
    sw(m, g[V0], 4, g[T7]);
    sw(m, g[V0], 0, g[T6]);
    g[V0] = lw(m, g[V1], 0);
    g[T9] = li(0xE300_0D01);
    g[T8] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T8]);
    sw(m, g[V0], 4, 0);
    sw(m, g[V0], 0, g[T9]);
    g[V0] = lw(m, g[V1], 0);
    g[T7] = li(0xE300_0F00);
    g[T6] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T6]);
    sw(m, g[V0], 4, 0);
    sw(m, g[V0], 0, g[T7]);
    g[V0] = lw(m, g[V1], 0);
    g[T9] = li(0xE300_1001);
    g[T8] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T8]);
    sw(m, g[V0], 4, 0);
    sw(m, g[V0], 0, g[T9]);
    g[V0] = lw(m, g[V1], 0);
    g[T8] = li(0xF0A_0000);
    g[T7] = li(0xE200_0000);
    g[T6] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T6]);
    g[T7] = g[T7] | 0x1C;
    g[T8] = g[T8] | 0x7008;
    sw(m, g[V0], 4, g[T8]);
    sw(m, g[V0], 0, g[T7]);
    g[V0] = lw(m, g[V1], 0);
    g[T7] = li(0x800A_0000);
    g[T6] = li(0xDC38_0000);
    g[T9] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T9]);
    g[T6] = g[T6] | 0xE;
    g[T7] = addu(g[T7], 0x3C80);
    sw(m, g[V0], 4, g[T7]);
    sw(m, g[V0], 0, g[T6]);
    g[V0] = lw(m, g[V1], 0);
    g[T6] = li(0x1_0000);
    g[T9] = li(0xDB0C_0000);
    g[T8] = addu(g[V0], 8);
    sw(m, g[V1], 0, g[T8]);
    g[T7] = li(0x800D_0000);
    sw(m, g[V0], 4, g[T6]);
    sw(m, g[V0], 0, g[T9]);
    g[T7] = lw(m, g[T7], 0x697C);
    g[T6] = li(0xE300_0000);
    g[T9] = 0xC0;
    g[T8] = g[T7] & 0x800;
    g[AT] = li(0x800A_0000);
    if g[T8] != 0 {
        g[V0] = lw(m, g[V1], 0);
        g[T8] = li(0xE300_1801);
        g[T7] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T7]);
        sw(m, g[V0], 4, g[T9]);
        sw(m, g[V0], 0, g[T8]);
        g[V0] = lw(m, g[V1], 0);
        g[T7] = li(0xE300_1A01);
        g[T6] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T6]);
        g[T8] = 0x30;
        sw(m, g[V0], 4, g[T8]);
        sw(m, g[V0], 0, g[T7]);
    } else {
        g[V0] = lw(m, g[V1], 0);
        g[T6] = g[T6] | 0x1801;
        g[T7] = li(0x8011_0000);
        g[T9] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T9]);
        sw(m, g[V0], 0, g[T6]);
        g[T7] = lh(m, g[T7], 0x447C);
        g[T9] = li(0xE300_1A01);
        sw(m, g[V0], 4, g[T7]);
        g[V0] = lw(m, g[V1], 0);
        g[T6] = 0x30;
        g[T8] = addu(g[V0], 8);
        sw(m, g[V1], 0, g[T8]);
        sw(m, g[V0], 4, g[T6]);
        sw(m, g[V0], 0, g[T9]);
    }
    sw(m, g[AT], 0x48D0, 0);
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

/// `func_8003D444()` (reset the render state and the screen rectangle,
/// **guess**): [`func_8003D110`], then
/// [`func_8003B300`](crate::misc::func_8003B300)`(0, W - 1, 0, H - 1)` with
/// `W`, `H` the s16 at `0x80114470`/`72` (the screen size, **guess**).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `v0 = 0x80114470` and the
/// callees' registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003D444(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    call(imports::func_8003D110, m, ctx);
    let g = &mut ctx.gpr;
    g[V0] = li(0x8011_4470);
    g[A1] = lh(m, g[V0], 0);
    g[A3] = lh(m, g[V0], 2);
    g[A0] = 0;
    g[A2] = 0;
    g[A1] = addu(g[A1], u64::MAX);
    g[A3] = addu(g[A3], u64::MAX);
    call(imports::func_8003B300, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
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

/// `func_80086A20(i, ambient, diffuse, dir)` (set a light from floats):
/// `ambient`, `diffuse` and `dir` point to float triples. With `l` =
/// [`func_800153C0`]`(dir)`, the direction is `v = dir * (120 / l)` (into
/// the frame at `sp + 0x24`), or `(0, 0, -1)` if `l < [0x800ADD6C]` (0.01
/// in the ROM). The three triples are truncated to halfwords (`ambient` at
/// `sp + 0x40`, `diffuse` at `+0x38`, `v` at `+0x30`, the conversions
/// interleaved as the C has them) and passed to [`func_80038ED0`]`(i, ...)`,
/// or to [`func_80038E58`] (the default `Lights1`) when `i == -1` (64-bit).
///
/// Frame (`sp - 0x48`): `ra` at `+0x14`; the arguments spilled to
/// `+0x48`..`+0x54` and re-read sign-extended. Leaves `at = -1`, `t7`,
/// `t9`, `t1`, `t3`, `t5` from the halfword stores, the conversions in
/// `f4`..`f18`, `f0`, `f2` (the scale) or `f6` = -1.0, and the callee's
/// registers.
///
/// Domain: canonical pointers; `dir`'s components not NaN, nor the scale
/// (the products' operands).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80086A20(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x48i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x48, g[A0]);
    g[A0] = g[A3];
    sw(m, g[SP], 0x4C, g[A1]);
    sw(m, g[SP], 0x50, g[A2]);
    sw(m, g[SP], 0x54, g[A3]);
    call(imports::func_800153C0, m, ctx);
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x800B_0000);
    f[4].set_u32l(lw(m, g[AT], -0x2294) as u32);
    g[A0] = lw(m, g[SP], 0x48);
    g[A1] = lw(m, g[SP], 0x4C);
    let short = f[0].fl() < f[4].fl();
    g[A2] = lw(m, g[SP], 0x50);
    g[A3] = lw(m, g[SP], 0x54);
    g[AT] = li(0x42F0_0000);
    if !short {
        f[8].set_u32l(g[AT] as u32);
        f[10].set_u32l(lw(m, g[A3], 0) as u32);
        f[2].set_fl(f[8].fl() / f[0].fl());
        f[16].set_fl(f[10].fl() * f[2].fl());
        sw(m, g[SP], 0x24, u64::from(f[16].u32l()));
        f[18].set_u32l(lw(m, g[A3], 4) as u32);
        f[4].set_fl(f[18].fl() * f[2].fl());
        sw(m, g[SP], 0x28, u64::from(f[4].u32l()));
        f[6].set_u32l(lw(m, g[A3], 8) as u32);
        f[8].set_fl(f[6].fl() * f[2].fl());
        sw(m, g[SP], 0x2C, u64::from(f[8].u32l()));
    } else {
        f[0].set_u32l(0);
        g[AT] = li(0xBF80_0000);
        f[6].set_u32l(g[AT] as u32);
        sw(m, g[SP], 0x24, u64::from(f[0].u32l()));
        sw(m, g[SP], 0x28, u64::from(f[0].u32l()));
        sw(m, g[SP], 0x2C, u64::from(f[6].u32l()));
    }
    f[10].set_u32l(lw(m, g[A1], 0) as u32);
    g[AT] = u64::MAX;
    f[16].set_u32l(fpu::trunc_w_s(f[10].fl()));
    g[T7] = s32(f[16].u32l());
    sh(m, g[SP], 0x40, g[T7]);
    f[18].set_u32l(lw(m, g[A1], 4) as u32);
    f[4].set_u32l(fpu::trunc_w_s(f[18].fl()));
    g[T9] = s32(f[4].u32l());
    sh(m, g[SP], 0x42, g[T9]);
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    g[A1] = addu(g[SP], 0x40);
    f[8].set_u32l(fpu::trunc_w_s(f[6].fl()));
    g[T1] = s32(f[8].u32l());
    sh(m, g[SP], 0x44, g[T1]);
    f[10].set_u32l(lw(m, g[A2], 0) as u32);
    f[16].set_u32l(fpu::trunc_w_s(f[10].fl()));
    f[10].set_u32l(lw(m, g[SP], 0x24) as u32);
    g[T3] = s32(f[16].u32l());
    f[16].set_u32l(fpu::trunc_w_s(f[10].fl()));
    sh(m, g[SP], 0x38, g[T3]);
    f[18].set_u32l(lw(m, g[A2], 4) as u32);
    g[T9] = s32(f[16].u32l());
    f[4].set_u32l(fpu::trunc_w_s(f[18].fl()));
    f[18].set_u32l(lw(m, g[SP], 0x28) as u32);
    g[T5] = s32(f[4].u32l());
    f[4].set_u32l(fpu::trunc_w_s(f[18].fl()));
    sh(m, g[SP], 0x3A, g[T5]);
    f[6].set_u32l(lw(m, g[A2], 8) as u32);
    g[T1] = s32(f[4].u32l());
    g[A2] = addu(g[SP], 0x38);
    f[8].set_u32l(fpu::trunc_w_s(f[6].fl()));
    f[6].set_u32l(lw(m, g[SP], 0x2C) as u32);
    sh(m, g[SP], 0x30, g[T9]);
    sh(m, g[SP], 0x32, g[T1]);
    g[T7] = s32(f[8].u32l());
    f[8].set_u32l(fpu::trunc_w_s(f[6].fl()));
    sh(m, g[SP], 0x3C, g[T7]);
    g[T3] = s32(f[8].u32l());
    sh(m, g[SP], 0x34, g[T3]);
    if g[A0] != g[AT] {
        g[A3] = addu(g[SP], 0x30);
        call(imports::func_80038ED0, m, ctx);
    } else {
        g[A0] = addu(g[SP], 0x40);
        g[A1] = addu(g[SP], 0x38);
        g[A2] = addu(g[SP], 0x30);
        call(imports::func_80038E58, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x48);
}

/// `func_80086B8C(i, on, colour, dir)` (set a light's second light from
/// floats): with `on != 0` (64-bit), `colour` and the direction (as in
/// [`func_80086A20`]: `dir * (120 / l)`, or `(0, 0, -1)` if `l <
/// [0x800ADD70]`, 0.01 in the ROM) are truncated to halfwords at `sp +
/// 0x38` and `sp + 0x30`. Then, in either case, [`func_80038FE8`]`(i, on,
/// sp + 0x38, sp + 0x30)` (which reads the halfwords only for `on != 0`).
///
/// Frame (`sp - 0x40`): `ra` at `+0x14`, `i` spilled to `+0x40` and re-read
/// sign-extended; with `on`, `on`, `colour`, `dir` spilled to `+0x44`..
/// `+0x4C` and re-read. Leaves, with `on`, `at` = the bits of 120.0 (or of
/// -1.0 for a short `dir`), `t7`, `t3`, `t9`, `t5`, `t1` from the stores and
/// the conversions in `f4`..`f18`; and the callees' registers.
///
/// Domain: canonical pointers; with `on`, `dir`'s components and the scale
/// not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80086B8C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x40, g[A0]);
    if g[A1] != 0 {
        g[A0] = g[A3];
        sw(m, g[SP], 0x44, g[A1]);
        sw(m, g[SP], 0x48, g[A2]);
        sw(m, g[SP], 0x4C, g[A3]);
        call(imports::func_800153C0, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        g[AT] = li(0x800B_0000);
        f[4].set_u32l(lw(m, g[AT], -0x2290) as u32);
        g[A1] = lw(m, g[SP], 0x44);
        g[A2] = lw(m, g[SP], 0x48);
        let short = f[0].fl() < f[4].fl();
        g[A3] = lw(m, g[SP], 0x4C);
        g[AT] = li(0x42F0_0000);
        if !short {
            f[8].set_u32l(g[AT] as u32);
            f[10].set_u32l(lw(m, g[A3], 0) as u32);
            f[2].set_fl(f[8].fl() / f[0].fl());
            f[16].set_fl(f[10].fl() * f[2].fl());
            sw(m, g[SP], 0x24, u64::from(f[16].u32l()));
            f[18].set_u32l(lw(m, g[A3], 4) as u32);
            f[4].set_fl(f[18].fl() * f[2].fl());
            sw(m, g[SP], 0x28, u64::from(f[4].u32l()));
            f[6].set_u32l(lw(m, g[A3], 8) as u32);
            f[8].set_fl(f[6].fl() * f[2].fl());
            sw(m, g[SP], 0x2C, u64::from(f[8].u32l()));
        } else {
            f[0].set_u32l(0);
            g[AT] = li(0xBF80_0000);
            f[6].set_u32l(g[AT] as u32);
            sw(m, g[SP], 0x24, u64::from(f[0].u32l()));
            sw(m, g[SP], 0x28, u64::from(f[0].u32l()));
            sw(m, g[SP], 0x2C, u64::from(f[6].u32l()));
        }
        f[10].set_u32l(lw(m, g[A2], 0) as u32);
        f[16].set_u32l(fpu::trunc_w_s(f[10].fl()));
        f[10].set_u32l(lw(m, g[SP], 0x24) as u32);
        g[T7] = s32(f[16].u32l());
        f[16].set_u32l(fpu::trunc_w_s(f[10].fl()));
        sh(m, g[SP], 0x38, g[T7]);
        f[18].set_u32l(lw(m, g[A2], 4) as u32);
        g[T3] = s32(f[16].u32l());
        f[4].set_u32l(fpu::trunc_w_s(f[18].fl()));
        f[18].set_u32l(lw(m, g[SP], 0x28) as u32);
        g[T9] = s32(f[4].u32l());
        f[4].set_u32l(fpu::trunc_w_s(f[18].fl()));
        sh(m, g[SP], 0x3A, g[T9]);
        f[6].set_u32l(lw(m, g[A2], 8) as u32);
        g[T5] = s32(f[4].u32l());
        sh(m, g[SP], 0x30, g[T3]);
        f[8].set_u32l(fpu::trunc_w_s(f[6].fl()));
        f[6].set_u32l(lw(m, g[SP], 0x2C) as u32);
        sh(m, g[SP], 0x32, g[T5]);
        g[T1] = s32(f[8].u32l());
        f[8].set_u32l(fpu::trunc_w_s(f[6].fl()));
        sh(m, g[SP], 0x3C, g[T1]);
        g[T7] = s32(f[8].u32l());
        sh(m, g[SP], 0x34, g[T7]);
    }
    let g = &mut ctx.gpr;
    g[A0] = lw(m, g[SP], 0x40);
    g[A2] = addu(g[SP], 0x38);
    g[A3] = addu(g[SP], 0x30);
    call(imports::func_80038FE8, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x40);
}

/// `func_80086CA0(i)`: [`func_80038F68`]`(i)` (copy the default `Lights1`
/// into slot `i`) unless `i == -1` (64-bit).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `at = -1` and the callee's
/// registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80086CA0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[AT] = u64::MAX;
    sw(m, g[SP], 0x14, g[RA]);
    if g[A0] != g[AT] {
        call(imports::func_80038F68, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
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
