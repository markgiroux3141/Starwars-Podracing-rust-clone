//! The spline walker and point helpers, over splines as `spline_load`
//! (`crate::loader::func_80030174`) leaves them in RDRAM (NOTES.md, "Splines"
//! and "Depth-0 leaves ported in session 7"; `assets::Spline` decodes the
//! same format from the ROM).

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use crate::recomp::{addu, div, enter, fpu, lh, li, lw, multu, reg::*, sll, slt, sra, srav, sw, RecompContext};

/// `func_8003A4A0(spline, i, out)`: the position (the float triple `+0x10`)
/// of point `i` (`84 * i`, 32-bit, unbounded) of the spline's points
/// (`[spline + 0xC]`, re-read before each word) copied to `out`.
///
/// Leaves `v0 = 84 i`, `t6`..`t1` = the points pointer and addresses,
/// `f4`/`f6`/`f8` = the words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003A4A0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = sll(g[A1], 2);
    g[V0] = addu(g[V0], g[A1]);
    g[T6] = lw(m, g[A0], 0xC);
    g[V0] = sll(g[V0], 2);
    g[V0] = addu(g[V0], g[A1]);
    g[V0] = sll(g[V0], 2);
    g[T7] = addu(g[T6], g[V0]);
    f[4].set_u32l(lw(m, g[T7], 0x10) as u32);
    sw(m, g[A2], 0, u64::from(f[4].u32l()));
    g[T8] = lw(m, g[A0], 0xC);
    g[T9] = addu(g[T8], g[V0]);
    f[6].set_u32l(lw(m, g[T9], 0x14) as u32);
    sw(m, g[A2], 4, u64::from(f[6].u32l()));
    g[T0] = lw(m, g[A0], 0xC);
    g[T1] = addu(g[T0], g[V0]);
    f[8].set_u32l(lw(m, g[T1], 0x18) as u32);
    sw(m, g[A2], 8, u64::from(f[8].u32l()));
}

/// `84 * i` (0x54, a spline point) as the code computes it into `t`: `((i
/// << 2) + i) << 2`, `+ i`, `<< 2`.
fn point_offset(g: &mut [u64; 32], i: usize, t: usize) {
    g[t] = sll(g[i], 2);
    g[t] = addu(g[t], g[i]);
    g[t] = sll(g[t], 2);
    g[t] = addu(g[t], g[i]);
    g[t] = sll(g[t], 2);
}

/// `func_8003A4E8(spline, i)`: the first of point `i`'s ten halfwords at
/// `+0x40` (usually the point's own index; NOTES.md, "Splines"), signed.
/// Leaves `t6` = the points, `t7 = 84 * i`, `t8` = the point.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003A4E8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    point_offset(g, A1, T7);
    g[T6] = lw(&mem, g[A0], 0xC);
    g[T8] = addu(g[T6], g[T7]);
    g[V0] = lh(&mem, g[T8], 0x40);
}

/// `func_8003A50C(spline, id, from)`: the first point `i` in `from..count`
/// (signed; `count = [spline + 4]`) whose halfword `+0x40` equals `id`
/// (full 64-bit compare with the sign-extended halfword), else -1. Leaves
/// `v1` = the last index tried, `a2` = its point (or `from` if none was
/// tried), `t6` = the points, `t7 = 84 * from` (`5 * from` if none), `t8`
/// = the last halfword, `at` = the last bound test.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003A50C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, g[A0], 4);
    g[T7] = sll(g[A2], 2);
    g[T7] = addu(g[T7], g[A2]);
    g[AT] = slt(g[A2], g[V0]);
    g[V1] = g[A2];
    if g[AT] != 0 {
        g[T6] = lw(m, g[A0], 0xC);
        g[T7] = sll(g[T7], 2);
        g[T7] = addu(g[T7], g[A2]);
        g[T7] = sll(g[T7], 2);
        g[A2] = addu(g[T6], g[T7]);
        loop {
            g[T8] = lh(m, g[A2], 0x40);
            if g[A1] == g[T8] {
                g[V0] = g[V1];
                return;
            }
            g[V1] = addu(g[V1], 1);
            g[AT] = slt(g[V1], g[V0]);
            g[A2] = addu(g[A2], 0x54);
            if g[AT] == 0 {
                break;
            }
        }
    }
    g[V0] = u64::MAX;
}

/// `func_8003A568(w, k)`: for the spline walker `w` ([`func_8003ABA0`]):
/// with no path bits (`+0x2C == 0`), the point index `w[+0x10 + 4k]`;
/// otherwise the halfword `+0x42 + 2 * j` of that point (of spline `[w]`),
/// where `j` = the path bits shifted right (arithmetic) by `k` (low 5
/// bits), or the bits themselves for `k == 0`.
///
/// QUIRK: `j` is the whole shifted bit field, not one bit, so it can reach
/// far past the point's ten halfwords. Domain: that halfword in RDRAM.
/// Leaves `t6 = t8 = 4k`, `t9 = w + 4k`, and on the bits path `v1` = the
/// point index, `a2 = j`, `t0` = the spline, `t1` = its points, `t2 = 84 *
/// index`, `t3` = the point, `t4 = 2j`, `t5` = the address; `t7 = t9`
/// otherwise.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003A568(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, g[A0], 0x2C);
    g[T8] = sll(g[A1], 2);
    g[T6] = sll(g[A1], 2);
    g[T9] = addu(g[A0], g[T8]);
    if g[V0] == 0 {
        g[T7] = addu(g[A0], g[T6]);
        g[V0] = lw(m, g[T7], 0x10);
        return;
    }
    g[V1] = lw(m, g[T9], 0x10);
    g[A2] = if g[A1] != 0 { srav(g[V0], g[A1]) } else { g[V0] };
    g[T0] = lw(m, g[A0], 0);
    g[T1] = lw(m, g[T0], 0xC);
    point_offset(g, V1, T2);
    g[T4] = sll(g[A2], 1);
    g[T3] = addu(g[T1], g[T2]);
    g[T5] = addu(g[T3], g[T4]);
    g[V0] = lh(m, g[T5], 0x42);
}

/// `func_8003ABA0(spline, dir, w)`: step the spline walker `w` one point,
/// forward if `(s16) dir == 1`, otherwise backward. `spline` is a loaded
/// spline header (`+0` the flag halfword F, `+0xC` the points; NOTES.md,
/// "Splines"). The walker holds point indices `+0x10` (current), `+0x14`,
/// `+0x18`, `+0x1C`, end flags `+0x20` (forward) and `+0x24` (backward),
/// a choice `+0x28` for forks, path bits `+0x2C`, and a float `+8`.
///
/// Forward: clear `+0x24`. Unless `+0x20` is already set, take point `p =
/// +0x14` (F != 0) or `+0x1C` (F == 0). With no successors: `+0x20 = 1`,
/// `+8 = 1.0`, the result -1. Otherwise successor `k = choice` if `choice
/// < count`, else `choice % count` (signed, truncated to s16 either way),
/// and the path bits become `(bits >> 1) | k` (F != 0) or `(bits >> 1) | (k
/// << 2)` (arithmetic shift). Then, if `+0x20` is (still) clear:
/// `+0x10 = +0x14`, and with F != 0 `+0x14` = the successor; with F == 0
/// `+0x14, +0x18, +0x1C = +0x18, +0x1C, successor`.
///
/// Backward: clear `+0x20`. Unless `+0x24` is already set, take point `p =
/// +0x10`. With no predecessors: `+0x24 = 1`, `+8 = 0.0`. Otherwise the
/// predecessor `choice < count ? pred[choice] : pred[choice % count]` (not
/// truncated), the path bits become `(bits << 1) & 1` (F != 0; QUIRK: that
/// is always 0) or `(bits << 1) & 7`, and gain bit 0 unless the
/// predecessor's first successor is the current point. Then, if `+0x24` is
/// clear: with F == 0, `+0x1C, +0x18 = +0x18, +0x14`; and `+0x14 = +0x10`,
/// `+0x10` = the predecessor.
///
/// `dir` is spilled to its slot `[sp + 4]`, and the chosen point (or -1)
/// is kept in the frame at `[sp - 8]`. QUIRK: when the end flag was already
/// set, `a1` is loaded from that frame word without it being written, an
/// uninitialised stack read (it only reaches `a1`). Point indices go
/// through `multu` by 0x54, so only their low 32 bits count.
///
/// Domain: a nonzero count before each `%` (so neither IDO divide check,
/// `do_break`, is reached: `choice % count` runs only when `choice >=
/// count`, which excludes `count == -1` with `choice = INT_MIN`), and the
/// points in RDRAM. Leaves `a3` = 1 or the choice, `v0`, `v1` = the point,
/// `t0 = 0x54` or `k`, `at`, the loads in `t1`..`t9`, and `f4 = 1.0` or `f6
/// = 0.0` at an end.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003ABA0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-8i64) as u64);
    g[T6] = sll(g[A1], 16);
    g[T7] = sra(g[T6], 16);
    sw(m, g[SP], 0xC, g[A1]);
    g[A3] = 1;
    g[A1] = g[T7];
    if g[T7] == g[A3] {
        // Forward.
        g[V0] = lw(m, g[A2], 0x20);
        sw(m, g[A2], 0x24, 0);
        if g[V0] == 0 {
            g[V0] = lh(m, g[A0], 0);
            g[AT] = li(0x3F80_0000); // 1.0
            let (p, pt, points) = if g[V0] != 0 { (T2, T3, T4) } else { (T8, T9, T1) };
            g[p] = lw(m, g[A2], if g[V0] != 0 { 0x14 } else { 0x1C });
            g[T0] = 0x54;
            g[points] = lw(m, g[A0], 0xC);
            g[pt] = multu(g[p], g[T0]).0;
            g[V1] = addu(g[pt], g[points]);
            g[A1] = lh(m, g[V1], 0);
            if g[A1] == 0 {
                ctx.fpr[4].set_u32l(g[AT] as u32);
                g[A1] = u64::MAX;
                sw(m, g[A2], 0x20, g[A3]);
                sw(m, g[A2], 8, u64::from(ctx.fpr[4].u32l()));
                sw(m, g[SP], 0, g[A1]);
                g[V0] = g[A3];
            } else {
                g[A3] = lw(m, g[A2], 0x28);
                g[AT] = slt(g[A3], g[A1]);
                if g[AT] == 0 {
                    g[T0] = div(g[A3], g[A1]).1;
                    g[T6] = sll(g[T0], 16);
                    g[T7] = sra(g[T6], 16);
                    g[T0] = g[T7];
                    if g[A1] == 0 {
                        imports::runtime::do_break(0x8003_AC7C);
                    }
                    g[AT] = u64::MAX;
                    let minus_one = g[A1] == g[AT];
                    g[AT] = li(0x8000_0000);
                    if minus_one && g[A3] == g[AT] {
                        imports::runtime::do_break(0x8003_AC94);
                    }
                } else {
                    g[T0] = sll(g[A3], 16);
                    g[T5] = sra(g[T0], 16);
                    g[T0] = g[T5];
                }
                g[T8] = sll(g[T0], 1);
                g[T9] = addu(g[V1], g[T8]);
                g[A1] = lh(m, g[T9], 4);
                if g[V0] != 0 {
                    g[T5] = lw(m, g[A2], 0x2C);
                    g[V0] = lw(m, g[A2], 0x20);
                    g[T6] = sra(g[T5], 1);
                    g[T7] = g[T6] | g[T0];
                    sw(m, g[A2], 0x2C, g[T7]);
                } else {
                    g[T1] = lw(m, g[A2], 0x2C);
                    g[T3] = sll(g[T0], 2);
                    g[V0] = lw(m, g[A2], 0x20);
                    g[T2] = sra(g[T1], 1);
                    g[T4] = g[T2] | g[T3];
                    sw(m, g[A2], 0x2C, g[T4]);
                }
                sw(m, g[SP], 0, g[A1]);
            }
        }
        g[A1] = lw(m, g[SP], 0); // QUIRK: never written if the end flag was set
        if g[V0] == 0 {
            g[T8] = lw(m, g[A2], 0x14);
            sw(m, g[A2], 0x10, g[T8]);
            g[T9] = lh(m, g[A0], 0);
            if g[T9] != 0 {
                sw(m, g[A2], 0x14, g[A1]);
            } else {
                g[T1] = lw(m, g[A2], 0x18);
                g[T2] = lw(m, g[A2], 0x1C);
                sw(m, g[A2], 0x1C, g[A1]);
                sw(m, g[A2], 0x14, g[T1]);
                sw(m, g[A2], 0x18, g[T2]);
            }
        }
    } else {
        // Backward.
        g[T3] = lw(m, g[A2], 0x24);
        sw(m, g[A2], 0x20, 0);
        if g[T3] == 0 {
            g[T4] = lw(m, g[A2], 0x10);
            g[T0] = 0x54;
            g[T6] = lw(m, g[A0], 0xC);
            g[T5] = multu(g[T4], g[T0]).0;
            g[V1] = addu(g[T5], g[T6]);
            g[V0] = lh(m, g[V1], 2);
            if g[V0] == 0 {
                ctx.fpr[6].set_u32l(0);
                g[A1] = u64::MAX;
                sw(m, g[A2], 0x24, g[A3]);
                sw(m, g[A2], 8, u64::from(ctx.fpr[6].u32l()));
                sw(m, g[SP], 0, g[A1]);
            } else {
                g[A3] = lw(m, g[A2], 0x28);
                g[AT] = slt(g[A3], g[V0]);
                if g[AT] == 0 {
                    g[T9] = div(g[A3], g[V0]).1;
                    g[T1] = sll(g[T9], 1);
                    g[T2] = addu(g[V1], g[T1]);
                    g[A1] = lh(m, g[T2], 8);
                    if g[V0] == 0 {
                        imports::runtime::do_break(0x8003_ADA4);
                    }
                    g[AT] = u64::MAX;
                    let minus_one = g[V0] == g[AT];
                    g[AT] = li(0x8000_0000);
                    if minus_one && g[A3] == g[AT] {
                        imports::runtime::do_break(0x8003_ADBC);
                    }
                } else {
                    g[T7] = sll(g[A3], 1);
                    g[T8] = addu(g[V1], g[T7]);
                    g[A1] = lh(m, g[T8], 8);
                }
                g[T3] = lh(m, g[A0], 0);
                if g[T3] != 0 {
                    g[T7] = lw(m, g[A2], 0x2C);
                    g[T8] = sll(g[T7], 1);
                    g[T9] = g[T8] & 1; // QUIRK: always 0
                    sw(m, g[A2], 0x2C, g[T9]);
                } else {
                    g[T4] = lw(m, g[A2], 0x2C);
                    g[T5] = sll(g[T4], 1);
                    g[T6] = g[T5] & 7;
                    sw(m, g[A2], 0x2C, g[T6]);
                }
                // Bit 0 unless we came from the predecessor's first successor.
                let lo = multu(g[A1], g[T0]).0;
                sw(m, g[SP], 0, g[A1]);
                g[T2] = lw(m, g[A0], 0xC);
                g[T1] = lw(m, g[A2], 0x10);
                g[T3] = lo;
                g[T4] = addu(g[T2], g[T3]);
                g[T5] = lh(m, g[T4], 4);
                if g[T1] != g[T5] {
                    g[T6] = lw(m, g[A2], 0x2C);
                    g[T7] = g[T6] | 1;
                    sw(m, g[A2], 0x2C, g[T7]);
                    sw(m, g[SP], 0, g[A1]);
                }
            }
        }
        g[T8] = lw(m, g[A2], 0x24);
        g[A1] = lw(m, g[SP], 0); // QUIRK: never written if the end flag was set
        if g[T8] == 0 {
            g[T9] = lh(m, g[A0], 0);
            if g[T9] == 0 {
                g[T2] = lw(m, g[A2], 0x18);
                g[T3] = lw(m, g[A2], 0x14);
                sw(m, g[A2], 0x1C, g[T2]);
                sw(m, g[A2], 0x18, g[T3]);
            }
            g[T4] = lw(m, g[A2], 0x10);
            sw(m, g[A2], 0x10, g[A1]);
            sw(m, g[A2], 0x14, g[T4]);
        }
    }
    g[SP] = addu(g[SP], 8);
}

/// `func_8003B250(w, p)`: start the spline walker `w` ([`func_8003ABA0`]) at
/// point `p`: all four indices `+0x10..+0x1C = p`, then `+0x14` = `p`'s
/// first successor if it has any. With the spline flag (`[spline]`, `spline
/// = [w]`) clear, `+0x18` and `+0x1C` follow the first successors two and
/// three steps on, as far as they exist. The points are re-read from
/// `[spline + 0xC]` at each step; the multiplies keep the low 32 bits.
///
/// Leaves `a2 = 0x54`, `v0` = the spline, `v1` = the points or the second
/// point, `a1` = `p` or the third point, and the loads in `t0`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003B250(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A2] = 0x54;
    g[T6] = multu(g[A1], g[A2]).0;
    g[V0] = lw(m, g[A0], 0);
    for off in [0x10, 0x14, 0x18, 0x1C] {
        sw(m, g[A0], off, g[A1]);
    }
    g[V1] = lw(m, g[V0], 0xC);
    g[T7] = addu(g[V1], g[T6]);
    g[T8] = lh(m, g[T7], 0);
    if g[T8] == 0 {
        return;
    }
    g[T0] = multu(g[A1], g[A2]).0;
    g[T1] = addu(g[V1], g[T0]);
    g[T2] = lh(m, g[T1], 4);
    sw(m, g[A0], 0x14, g[T2]);
    g[T3] = lh(m, g[V0], 0);
    if g[T3] != 0 {
        return;
    }
    g[T6] = multu(g[T2], g[A2]).0;
    g[T4] = lw(m, g[V0], 0xC);
    g[V1] = addu(g[T4], g[T6]);
    g[T7] = lh(m, g[V1], 0);
    if g[T7] == 0 {
        return;
    }
    g[T8] = lh(m, g[V1], 4);
    g[T1] = multu(g[T8], g[A2]).0;
    sw(m, g[A0], 0x18, g[T8]);
    g[T9] = lw(m, g[V0], 0xC);
    g[A1] = addu(g[T9], g[T1]);
    g[T2] = lh(m, g[A1], 0);
    if g[T2] != 0 {
        g[T3] = lh(m, g[A1], 4);
        sw(m, g[A0], 0x1C, g[T3]);
    }
}

/// `func_8007EE98(w, v)` (walker placement, **guess** at the name): puts
/// the spline walker `w` (NOTES.md, "Depth-0 leaves ported in session 7")
/// at segment `v / 10` (signed `div`), with the remainder as the fraction
/// `w+8 = (f32(v) - f32(v / 10) * 10) / 10`. With `s = [w]`, `n = [s + 4]`
/// the point count, `pts = [s + 0xC]` (0x54 bytes each) and `F = (s16)
/// [s + 0]`:
/// - a segment `k = v / 10 < n` is a point: `w+0x10 = k`, `w+0x14 = pts[k]
///   .next[0]` (the halfword `+4`), and if `F != 1`, `w+0x18 = pts[w+0x14]
///   .next[0]`, `w+0x1C = pts[w+0x18].next[0]`; `w+0x2C = 0`, then the
///   fraction;
/// - otherwise it looks for `k` among the points' extra segment ids (the
///   halfwords `+0x42 + 2j`, j = 0..7, point by point): at the first match
///   (point `a`, slot `j`), `w+0x2C = j`, `w+0x10 = a`, the fraction, then
///   the path through the forks: `w+0x14 = pts[a].next[j & 1]`, and if `F
///   != 1`, `w+0x18 = pts[w+0x14].next[(j >> 1) & 1]`, `w+0x1C =
///   pts[w+0x18].next[(j >> 2) & 1]`. Nothing is written without a match.
///
/// The search stops by setting both indices to 99999; QUIRK: with `n`
/// above 100000 it would carry on from point 100000. Indices are signed
/// halfwords used as they are (not bounded).
///
/// Leaves `v0 = k`, `v1 = s`, `a1 = v`, and (the paths differ) the loop
/// registers `a2`, `a3`, `t0`..`t9`, `f0 = 10.0` and the conversions in
/// `f4`..`f18`.
///
/// Domain: canonical pointers; the points and the ones they name in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8007EE98(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = 0xA;
    let (lo, _) = div(g[A1], g[AT]);
    g[V1] = lw(m, g[A0], 0);
    g[V0] = lo;
    g[A2] = lw(m, g[V1], 4);
    g[AT] = slt(g[V0], g[A2]);
    if g[AT] != 0 {
        // A point: its successors, 0x54-byte points (a2).
        g[A2] = 0x54;
        let (lo, _) = multu(g[V0], g[A2]);
        sw(m, g[A0], 0x10, g[V0]);
        g[T4] = lw(m, g[V1], 0xC);
        g[T0] = 1;
        g[T8] = lo;
        g[T6] = addu(g[T4], g[T8]);
        g[T2] = lh(m, g[T6], 4);
        sw(m, g[A0], 0x14, g[T2]);
        g[T9] = lh(m, g[V1], 0);
        if g[T0] != g[T9] {
            let (lo, _) = multu(g[T2], g[A2]);
            g[T3] = lw(m, g[V1], 0xC);
            g[T7] = lo;
            g[T4] = addu(g[T3], g[T7]);
            g[T8] = lh(m, g[T4], 4);
            let (lo, _) = multu(g[T8], g[A2]);
            sw(m, g[A0], 0x18, g[T8]);
            g[T6] = lw(m, g[V1], 0xC);
            g[T9] = lo;
            g[T5] = addu(g[T6], g[T9]);
            g[T3] = lh(m, g[T5], 4);
            sw(m, g[A0], 0x1C, g[T3]);
        }
        f[6].set_u32l(g[V0] as u32);
        g[AT] = li(0x4120_0000); // 10.0
        f[0].set_u32l(g[AT] as u32);
        f[16].set_fl(fpu::cvt_s_w(f[6].u32l(), fpu::NEAREST));
        f[8].set_u32l(g[A1] as u32);
        sw(m, g[A0], 0x2C, 0);
        f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
        f[18].set_fl(f[16].fl() * f[0].fl());
        f[4].set_fl(f[10].fl() - f[18].fl());
        f[8].set_fl(f[4].fl() / f[0].fl());
        sw(m, g[A0], 8, u64::from(f[8].u32l()));
        return;
    }
    // A fork's segment: a3 = the point, a2 = the slot, t1 = 99999.
    g[A3] = 0;
    if (g[A2] as i64) <= 0 {
        return;
    }
    g[AT] = li(0x4120_0000);
    g[T1] = li(0x1_0000);
    f[0].set_u32l(g[AT] as u32);
    g[T1] = g[T1] | 0x869F;
    g[T0] = 1;
    g[A2] = 0;
    loop {
        loop {
            // t2 = &pts[a3] + 2 a2 (a3 * 0x54 as ((a3*5)*4 + a3)*4).
            g[T7] = sll(g[A3], 2);
            g[T7] = addu(g[T7], g[A3]);
            g[T6] = lw(m, g[V1], 0xC);
            g[T7] = sll(g[T7], 2);
            g[T7] = addu(g[T7], g[A3]);
            g[T7] = sll(g[T7], 2);
            g[T9] = sll(g[A2], 1);
            g[T8] = addu(g[T6], g[T7]);
            g[T2] = addu(g[T8], g[T9]);
            g[T3] = lh(m, g[T2], 0x42);
            if g[V0] != g[T3] {
                g[A2] = addu(g[A2], 1);
            } else {
                f[8].set_u32l(g[V0] as u32);
                f[4].set_u32l(g[A1] as u32);
                g[T5] = sll(g[A3], 2);
                f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
                sw(m, g[A0], 0x2C, g[A2]);
                sw(m, g[A0], 0x10, g[A3]);
                g[T5] = addu(g[T5], g[A3]);
                g[T5] = sll(g[T5], 2);
                f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
                f[16].set_fl(f[10].fl() * f[0].fl());
                g[T5] = addu(g[T5], g[A3]);
                g[T5] = sll(g[T5], 2);
                g[T7] = g[A2] & 1;
                g[T8] = sll(g[T7], 1);
                g[A3] = g[T1];
                f[18].set_fl(f[6].fl() - f[16].fl());
                f[4].set_fl(f[18].fl() / f[0].fl());
                sw(m, g[A0], 8, u64::from(f[4].u32l()));
                g[T4] = lw(m, g[V1], 0xC);
                g[T6] = addu(g[T4], g[T5]);
                g[T9] = addu(g[T6], g[T8]);
                g[T2] = lh(m, g[T9], 4);
                g[T8] = sra(g[A2], 1);
                g[T9] = g[T8] & 1;
                sw(m, g[A0], 0x14, g[T2]);
                g[T3] = lh(m, g[V1], 0);
                g[T7] = sll(g[T2], 2);
                g[T7] = addu(g[T7], g[T2]);
                g[T7] = sll(g[T7], 2);
                if g[T0] != g[T3] {
                    g[T4] = lw(m, g[V1], 0xC);
                    g[T7] = addu(g[T7], g[T2]);
                    g[T7] = sll(g[T7], 2);
                    g[T2] = sll(g[T9], 1);
                    g[T6] = addu(g[T4], g[T7]);
                    g[T3] = addu(g[T6], g[T2]);
                    g[T5] = lh(m, g[T3], 4);
                    g[T6] = sra(g[A2], 2);
                    g[T2] = g[T6] & 1;
                    g[T8] = sll(g[T5], 2);
                    sw(m, g[A0], 0x18, g[T5]);
                    g[T8] = addu(g[T8], g[T5]);
                    g[T4] = lw(m, g[V1], 0xC);
                    g[T8] = sll(g[T8], 2);
                    g[T8] = addu(g[T8], g[T5]);
                    g[T8] = sll(g[T8], 2);
                    g[T3] = sll(g[T2], 1);
                    g[T9] = addu(g[T4], g[T8]);
                    g[T5] = addu(g[T9], g[T3]);
                    g[T7] = lh(m, g[T5], 4);
                    sw(m, g[A0], 0x1C, g[T7]);
                }
                // Stop both loops.
                g[A2] = g[T1];
                g[A2] = addu(g[A2], 1);
            }
            g[AT] = slt(g[A2], 8);
            if g[AT] == 0 {
                break;
            }
        }
        g[A2] = lw(m, g[V1], 4);
        g[A3] = addu(g[A3], 1);
        g[AT] = slt(g[A3], g[A2]);
        if g[AT] == 0 {
            return;
        }
        g[A2] = 0;
    }
}
