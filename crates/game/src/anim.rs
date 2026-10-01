//! Animation objects (**guess**; NOTES.md, "Depth-0 leaves ported in session
//! 8"): the table of object pointers at `0x800AF4C0` and the objects'
//! fields: `+0x100` flags (kind in the low 4 bits; bit 31 set hides an
//! object from the search), `+0x124` a target or id, float keys at
//! `[+0x11C]` (count `+0x104`), a time `+0x114`, a range `+0xF0..+0xF8`.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use crate::recomp::{addu, call, enter, fpu, ld, lh, lhu, li, lw, reg::*, s32, sd, sh, sll, slt, sltu, subu, sw, Fpr, RecompContext};
use n64mem::Mem;

/// Object table searched by [`func_80006D5C`] (300 words, cleared by
/// [`func_80005B80`]); `[0x8009A2A0]` is the number of entries in use.
pub const OBJECTS: u32 = 0x800A_F4C0;

/// `func_80005B80`: `[0x8009A2A0] = 0`, then zero the 300 words from
/// `0x800AF4C0` up to `0x800AF970`, four per iteration.
///
/// Leaves `at = 0x800A0000` and `v0 = v1 = 0x800AF970`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80005B80(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    g[V1] = li(0x800B_0000);
    g[V0] = li(0x800B_0000);
    sw(m, g[AT], -0x5D60, 0);
    g[V0] = addu(g[V0], (-0x690i64) as u64);
    g[V1] = addu(g[V1], (-0xB40i64) as u64);
    loop {
        g[V1] = addu(g[V1], 0x10);
        sw(m, g[V1], -0xC, 0);
        sw(m, g[V1], -0x8, 0);
        sw(m, g[V1], -0x4, 0);
        sw(m, g[V1], -0x10, 0); // bne delay slot
        if g[V1] == g[V0] {
            break;
        }
    }
}

/// `func_80005BB8(o)`: add `o` to [`OBJECTS`] and start it: `OBJECTS[n] =
/// o`, `n += 1` (`n = [0x8009A2A0]`, unbounded, as in [`func_80006D5C`]);
/// then `+0xF0 = 0.0`, `+0xF4 = +0x10C` if flag bit 5 is set, else `+0x108`
/// (copied as bits); `+0xDC = 0.0`; `+0xF8 = +0xFC = +0xF4` (re-read);
/// for kind 8 (`flags & 0xF`) with a nonzero `p = [o + 0x124]`, clear bit
/// 3 of the halfword `[p + 0xC]` and re-read the flags; finally set flag
/// bit 24 in the flags word it last read.
///
/// Leaves `v1 = 0x8009A2A0` (or `p`), `v0` = the flags, `at = 0x1000000`,
/// `t6`..`t9` and `t0` from the steps above (`t1`/`t2` the halfword for
/// kind 8), `t3` = the new flags, `f2 = 0`, `f0` = the start value, and
/// `f4` or `f6` the value copied.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80005BB8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V1] = li(0x8009_A2A0);
    g[V0] = lw(m, g[V1], 0);
    f[2].set_u32l(0);
    g[T6] = sll(g[V0], 2);
    g[AT] = addu(li(0x800B_0000), g[T6]);
    sw(m, g[AT], -0xB40, g[A0]);
    g[T7] = addu(g[V0], 1);
    sw(m, g[V1], 0, g[T7]);
    g[T8] = lw(m, g[A0], 0x100);
    g[AT] = 8;
    sw(m, g[A0], 0xF0, u64::from(f[2].u32l()));
    g[T9] = g[T8] & 0x20;
    let start = if g[T9] == 0 { (6, 0x108) } else { (4, 0x10C) };
    f[start.0].set_u32l(lw(m, g[A0], start.1) as u32);
    sw(m, g[A0], 0xF4, u64::from(f[start.0].u32l()));
    g[V0] = lw(m, g[A0], 0x100);
    f[0].set_u32l(lw(m, g[A0], 0xF4) as u32);
    sw(m, g[A0], 0xDC, u64::from(f[2].u32l()));
    g[T0] = g[V0] & 0xF;
    sw(m, g[A0], 0xF8, u64::from(f[0].u32l()));
    sw(m, g[A0], 0xFC, u64::from(f[0].u32l()));
    if g[T0] == g[AT] {
        g[V1] = lw(m, g[A0], 0x124);
        if g[V1] != 0 {
            g[T1] = lhu(m, g[V1], 0xC);
            g[T2] = g[T1] & 0xFFF7;
            sh(m, g[V1], 0xC, g[T2]);
            g[V0] = lw(m, g[A0], 0x100);
        }
    }
    g[AT] = li(0x0100_0000);
    g[T3] = g[V0] | g[AT];
    sw(m, g[A0], 0x100, g[T3]);
}

/// `func_80005C54()` (animation reset, **guess**): zeroes the 30 words from
/// `0x800AF970` up to `0x800AF9E8` (the first two, then four per
/// iteration, all from `f0 = 0.0`), then clears the object table
/// ([`func_80005B80`]).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `f0 = 0.0` and
/// [`func_80005B80`]'s registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80005C54(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    ctx.fpr[0].set_u32l(0);
    g[AT] = li(0x800B_0000);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[V0] = li(0x800A_F978);
    g[V1] = li(0x800A_F9E8);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[AT], -0x690, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[AT], -0x68C, u64::from(ctx.fpr[0].u32l()));
    loop {
        g[V0] = addu(g[V0], 0x10);
        sw(m, g[V0], -0x10, u64::from(ctx.fpr[0].u32l()));
        sw(m, g[V0], -0xC, u64::from(ctx.fpr[0].u32l()));
        sw(m, g[V0], -8, u64::from(ctx.fpr[0].u32l()));
        sw(m, g[V0], -4, u64::from(ctx.fpr[0].u32l()));
        if g[V0] == g[V1] {
            break;
        }
    }
    call(imports::func_80005B80, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80005CAC(o, x, i)` with the float `x` in `a1`: where `x` lies
/// between keys `i` and `i + 1` of the float array `k = [o + 0x11C]`:
/// `(x - k[i]) / (k[i + 1] - k[i])` in `f0`. `i` is unbounded (32-bit
/// address arithmetic).
///
/// Leaves `t6 = k`, `t7 = 4i`, `v0 = &k[i]`, `f14 = x`, `f2`/`f12` = the
/// keys, `f4`/`f6` = the differences.
///
/// Domain: `x`, the keys and both differences not NaN (the quotient may be:
/// `0 / 0` for equal keys).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80005CAC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = lw(m, g[A0], 0x11C);
    g[T7] = sll(g[A2], 2);
    f[14].set_u32l(g[A1] as u32);
    g[V0] = addu(g[T6], g[T7]);
    f[2].set_u32l(lw(m, g[V0], 0) as u32);
    f[12].set_u32l(lw(m, g[V0], 4) as u32);
    f[4].set_fl(f[14].fl() - f[2].fl());
    f[6].set_fl(f[12].fl() - f[2].fl());
    f[0].set_fl(f[4].fl() / f[6].fl());
}

/// `func_80005CD4(out, o, x, i)` (a float track's value, **guess**): with
/// the float `x` in `a2`, keys `k = [o + 0x11C]` and values `v = [o +
/// 0x120]` (floats): `*out = v[i + 1]` if `k[i + 1] < x`; else `v[i]` if
/// `x <= k[i]`; else, with `t` = [`func_80005CAC`]`(o, x, i)`, `*out = t *
/// v[i + 1] + (1 - t) * v[i]` (the products in that order, `1.0` built in
/// a register). The copies are word moves.
///
/// Frame (`sp - 0x30`): `ra`, `s0` at `+0x1C`, `+0x18` (`s0 = o` meanwhile);
/// `out` spilled to its home slot `sp + 0`, `4i` to `+0x20` around the
/// call. Leaves `v1 = 4i`, `v0 = &k[i + 1] - 4` or `&v[i]`, `a2 = i` and
/// the keys and values in `f2`..`f18` as the path used them.
///
/// Domain: canonical pointers, `i` an index whose keys and values are in
/// RDRAM; no NaN operand of the interpolation (nor of [`func_80005CAC`]'s).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80005CD4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x30i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x18, g[S0]);
    sw(m, g[SP], 0x30, g[A0]);
    g[T6] = lw(m, g[A1], 0x11C);
    g[V1] = sll(g[A3], 2);
    ctx.fpr[12].set_u32l(g[A2] as u32);
    g[V0] = addu(g[T6], g[V1]);
    ctx.fpr[4].set_u32l(lw(m, g[V0], 4) as u32);
    g[A2] = g[A3];
    g[S0] = g[A1];
    if !(ctx.fpr[4].fl() < ctx.fpr[12].fl()) {
        ctx.fpr[8].set_u32l(lw(m, g[V0], 0) as u32);
        g[A0] = g[S0];
        if !(ctx.fpr[12].fl() <= ctx.fpr[8].fl()) {
            // Between the keys: t = f0.
            g[A1] = s32(ctx.fpr[12].u32l());
            sw(m, g[SP], 0x20, g[V1]);
            call(imports::func_80005CAC, m, ctx);
            let g = &mut ctx.gpr;
            g[V1] = lw(m, g[SP], 0x20);
            g[T3] = lw(m, g[S0], 0x120);
            g[AT] = li(0x3F80_0000);
            ctx.fpr[18].set_u32l(g[AT] as u32);
            g[V0] = addu(g[T3], g[V1]);
            ctx.fpr[12].set_u32l(lw(m, g[V0], 4) as u32);
            ctx.fpr[4].set_fl(ctx.fpr[18].fl() - ctx.fpr[0].fl());
            ctx.fpr[2].set_u32l(lw(m, g[V0], 0) as u32);
            ctx.fpr[16].set_fl(ctx.fpr[0].fl() * ctx.fpr[12].fl());
            g[T4] = lw(m, g[SP], 0x30);
            ctx.fpr[6].set_fl(ctx.fpr[4].fl() * ctx.fpr[2].fl());
            ctx.fpr[8].set_fl(ctx.fpr[16].fl() + ctx.fpr[6].fl());
            sw(m, g[T4], 0, u64::from(ctx.fpr[8].u32l()));
        } else {
            g[T0] = lw(m, g[S0], 0x120);
            g[T2] = lw(m, g[SP], 0x30);
            g[T1] = addu(g[T0], g[V1]);
            ctx.fpr[10].set_u32l(lw(m, g[T1], 0) as u32);
            sw(m, g[T2], 0, u64::from(ctx.fpr[10].u32l()));
        }
    } else {
        g[T7] = lw(m, g[A1], 0x120);
        g[T8] = addu(g[T7], g[V1]);
        ctx.fpr[6].set_u32l(lw(m, g[T8], 4) as u32);
        sw(m, g[A0], 0, u64::from(ctx.fpr[6].u32l()));
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x30);
}

/// `func_80005DA8(out, o, x, i)` (a vec3 track's value, **guess**): as
/// [`func_80005CD4`] with 12-byte values `v = [o + 0x120]`: `out = v[i + 1]`
/// if `k[i + 1] < x`; else `v[i]` if `x <= k[i]`; else with `t` from
/// [`func_80005CAC`], `out = t * v[i + 1]` (each component stored), then
/// [`func_800155EC`](crate::math::func_800155EC)`(out, out, 1 - t, A)`,
/// i.e. `out = A * (1 - t) + out`, with `A` = `v[i]` copied to the frame.
/// So each component is `v[i].c * (1 - t) + t * v[i + 1].c`. The copies are
/// word moves, the value pointer re-read for each word.
///
/// Frame (`sp - 0x48`): `ra`, `s2`, `s1`, `s0` at `+0x24..+0x18` (`s0 = o`,
/// `s1 = out`, `s2 = i` meanwhile), `v[i]` at `+0x3C`, `v[i + 1]` at
/// `+0x30`. Leaves `v0 = 12i` (or `4i`), the keys and values in
/// `f4`..`f18`, and the callees' registers.
///
/// Domain: as [`func_80005CD4`]'s; `out` disjoint from the values and the
/// frame.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80005DA8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x48i64) as u64);
    sw(m, g[SP], 0x24, g[RA]);
    sw(m, g[SP], 0x20, g[S2]);
    sw(m, g[SP], 0x1C, g[S1]);
    sw(m, g[SP], 0x18, g[S0]);
    g[T6] = lw(m, g[A1], 0x11C);
    g[T7] = sll(g[A3], 2);
    ctx.fpr[12].set_u32l(g[A2] as u32);
    g[V0] = addu(g[T6], g[T7]);
    ctx.fpr[4].set_u32l(lw(m, g[V0], 4) as u32);
    g[S0] = g[A1];
    g[S1] = g[A0];
    let after = ctx.fpr[4].fl() < ctx.fpr[12].fl();
    g[S2] = g[A3];
    if !after {
        ctx.fpr[16].set_u32l(lw(m, g[V0], 0) as u32);
        g[V0] = sll(g[S2], 2);
        g[A0] = g[S0];
        if !(ctx.fpr[12].fl() <= ctx.fpr[16].fl()) {
            // Between the keys: t = f0; v[i] and v[i + 1] to the frame.
            g[A1] = s32(ctx.fpr[12].u32l());
            g[A2] = g[S2];
            call(imports::func_80005CAC, m, ctx);
            let g = &mut ctx.gpr;
            g[T0] = lw(m, g[S0], 0x120);
            g[V0] = sll(g[S2], 2);
            g[V0] = subu(g[V0], g[S2]);
            g[V0] = sll(g[V0], 2);
            g[T1] = addu(g[T0], g[V0]);
            ctx.fpr[8].set_u32l(lw(m, g[T1], 0) as u32);
            g[AT] = li(0x3F80_0000);
            g[A0] = g[S1];
            sw(m, g[SP], 0x3C, u64::from(ctx.fpr[8].u32l()));
            g[T2] = lw(m, g[S0], 0x120);
            g[A1] = g[S1];
            g[A3] = addu(g[SP], 0x3C);
            g[T3] = addu(g[T2], g[V0]);
            ctx.fpr[10].set_u32l(lw(m, g[T3], 4) as u32);
            sw(m, g[SP], 0x40, u64::from(ctx.fpr[10].u32l()));
            g[T4] = lw(m, g[S0], 0x120);
            g[T5] = addu(g[T4], g[V0]);
            ctx.fpr[16].set_u32l(lw(m, g[T5], 8) as u32);
            sw(m, g[SP], 0x44, u64::from(ctx.fpr[16].u32l()));
            g[T6] = lw(m, g[S0], 0x120);
            g[T7] = addu(g[T6], g[V0]);
            ctx.fpr[18].set_u32l(lw(m, g[T7], 0xC) as u32);
            sw(m, g[SP], 0x30, u64::from(ctx.fpr[18].u32l()));
            g[T8] = lw(m, g[S0], 0x120);
            ctx.fpr[8].set_u32l(lw(m, g[SP], 0x30) as u32);
            g[T9] = addu(g[T8], g[V0]);
            ctx.fpr[4].set_u32l(lw(m, g[T9], 0x10) as u32);
            ctx.fpr[10].set_fl(ctx.fpr[8].fl() * ctx.fpr[0].fl());
            ctx.fpr[8].set_u32l(g[AT] as u32);
            sw(m, g[SP], 0x34, u64::from(ctx.fpr[4].u32l()));
            g[T0] = lw(m, g[S0], 0x120);
            g[T1] = addu(g[T0], g[V0]);
            ctx.fpr[6].set_u32l(lw(m, g[T1], 0x14) as u32);
            sw(m, g[SP], 0x38, u64::from(ctx.fpr[6].u32l()));
            // out = t * v[i + 1], then out = A * (1 - t) + out.
            sw(m, g[S1], 0, u64::from(ctx.fpr[10].u32l()));
            ctx.fpr[16].set_u32l(lw(m, g[SP], 0x34) as u32);
            ctx.fpr[10].set_fl(ctx.fpr[8].fl() - ctx.fpr[0].fl());
            ctx.fpr[18].set_fl(ctx.fpr[16].fl() * ctx.fpr[0].fl());
            g[A2] = s32(ctx.fpr[10].u32l());
            sw(m, g[S1], 4, u64::from(ctx.fpr[18].u32l()));
            ctx.fpr[4].set_u32l(lw(m, g[SP], 0x38) as u32);
            ctx.fpr[6].set_fl(ctx.fpr[4].fl() * ctx.fpr[0].fl());
            sw(m, g[S1], 8, u64::from(ctx.fpr[6].u32l()));
            call(imports::func_800155EC, m, ctx);
        } else {
            g[T4] = lw(m, g[S0], 0x120);
            g[V0] = subu(g[V0], g[S2]);
            g[V0] = sll(g[V0], 2);
            g[T5] = addu(g[T4], g[V0]);
            ctx.fpr[18].set_u32l(lw(m, g[T5], 0) as u32);
            sw(m, g[S1], 0, u64::from(ctx.fpr[18].u32l()));
            g[T6] = lw(m, g[S0], 0x120);
            g[T7] = addu(g[T6], g[V0]);
            ctx.fpr[4].set_u32l(lw(m, g[T7], 4) as u32);
            sw(m, g[S1], 4, u64::from(ctx.fpr[4].u32l()));
            g[T8] = lw(m, g[S0], 0x120);
            g[T9] = addu(g[T8], g[V0]);
            ctx.fpr[6].set_u32l(lw(m, g[T9], 8) as u32);
            sw(m, g[S1], 8, u64::from(ctx.fpr[6].u32l()));
        }
    } else {
        g[T8] = lw(m, g[A1], 0x120);
        g[V0] = sll(g[A3], 2);
        g[V0] = subu(g[V0], g[A3]);
        g[V0] = sll(g[V0], 2);
        g[T9] = addu(g[T8], g[V0]);
        ctx.fpr[6].set_u32l(lw(m, g[T9], 0xC) as u32);
        sw(m, g[A0], 0, u64::from(ctx.fpr[6].u32l()));
        g[T0] = lw(m, g[A1], 0x120);
        g[T1] = addu(g[T0], g[V0]);
        ctx.fpr[8].set_u32l(lw(m, g[T1], 0x10) as u32);
        sw(m, g[A0], 4, u64::from(ctx.fpr[8].u32l()));
        g[T2] = lw(m, g[A1], 0x120);
        g[T3] = addu(g[T2], g[V0]);
        ctx.fpr[10].set_u32l(lw(m, g[T3], 0x14) as u32);
        sw(m, g[A0], 8, u64::from(ctx.fpr[10].u32l()));
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x24);
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x48);
}

/// `func_80006120(o)` (a vec3 track to its node's translation, **guess**):
/// `p` = [`func_80005DA8`]`(o, u, s)` with the time `u = [o + 0x114]` (a
/// float, passed in `a2`) and the segment `s = [o + 0x118]`. If bit 29 of
/// the flags `[o + 0x100]` is set, it blends in a second sample `q` =
/// [`func_80005DA8`]`(o, f32([o + 0xEC]), [o + 0xE8])` (the word converted
/// round to nearest) by the weight `w = [o + 0xE4]` (re-read for each use):
/// `p = p * w` component by component, then
/// [`func_800155EC`](crate::math::func_800155EC)`(p, p, 1 - w, q)`, i.e. `p
/// = q * (1 - w) + p`. Then, if the node `n = [o + 0x124]` is nonzero,
/// [`func_80017B7C`](crate::misc::func_80017B7C)`(n, p.x, p.y, p.z)` sets its
/// translation.
///
/// Frame (`sp - 0x40`): `ra` at `+0x1C`, `s0` (`= o`) at `+0x18`, restored;
/// `p` at `+0x34`, `q` at `+0x28`. Leaves `a0 = n`, `a1`, `a2` = `p.x`,
/// `p.y` (loaded whether or not `n` is 0; `a3 = p.z` only for a nonzero
/// `n`), `t6` the flags, `t7 = t6 << 2`; on the blend path `t8 = [o +
/// 0xEC]`, `at` = 1.0, `f16` = 1.0, `f4 = 1 - w`, `f6`..`f18` from the
/// products; and the callees' registers.
///
/// Domain: canonical `o`; the callees' domains (with `p` and `q`'s
/// components and `w` not NaN on the blend path, as operands of the
/// products and of `1 - w`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006120(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    sw(m, g[SP], 0x18, g[S0]);
    g[S0] = g[A0];
    sw(m, g[SP], 0x1C, g[RA]);
    g[A3] = lw(m, g[S0], 0x118);
    g[A2] = lw(m, g[S0], 0x114);
    g[A1] = g[S0];
    g[A0] = addu(g[SP], 0x34);
    call(imports::func_80005DA8, m, ctx);
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = lw(m, g[S0], 0x100);
    g[T7] = sll(g[T6], 2);
    if (g[T7] as i64) >= 0 {
        g[A0] = lw(m, g[S0], 0x124);
    } else {
        g[T8] = lw(m, g[S0], 0xEC);
        g[A0] = addu(g[SP], 0x28);
        g[A1] = g[S0];
        f[4].set_u32l(g[T8] as u32);
        g[A3] = lw(m, g[S0], 0xE8);
        f[4].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
        g[A2] = s32(f[4].u32l());
        call(imports::func_80005DA8, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        f[6].set_u32l(lw(m, g[SP], 0x34) as u32);
        f[8].set_u32l(lw(m, g[S0], 0xE4) as u32);
        f[16].set_u32l(lw(m, g[SP], 0x38) as u32);
        g[AT] = li(0x3F80_0000);
        f[10].set_fl(f[6].fl() * f[8].fl());
        f[6].set_u32l(lw(m, g[SP], 0x3C) as u32);
        g[A0] = addu(g[SP], 0x34);
        g[A1] = g[A0];
        g[A3] = addu(g[SP], 0x28);
        sw(m, g[SP], 0x34, u64::from(f[10].u32l()));
        f[18].set_u32l(lw(m, g[S0], 0xE4) as u32);
        f[4].set_fl(f[16].fl() * f[18].fl());
        f[16].set_u32l(g[AT] as u32);
        sw(m, g[SP], 0x38, u64::from(f[4].u32l()));
        f[8].set_u32l(lw(m, g[S0], 0xE4) as u32);
        f[10].set_fl(f[6].fl() * f[8].fl());
        sw(m, g[SP], 0x3C, u64::from(f[10].u32l()));
        f[18].set_u32l(lw(m, g[S0], 0xE4) as u32);
        f[4].set_fl(f[16].fl() - f[18].fl());
        g[A2] = s32(f[4].u32l());
        call(imports::func_800155EC, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = lw(m, g[S0], 0x124);
    }
    let g = &mut ctx.gpr;
    g[A1] = lw(m, g[SP], 0x34);
    g[A2] = lw(m, g[SP], 0x38);
    if g[A0] != 0 {
        g[A3] = lw(m, g[SP], 0x3C);
        call(imports::func_80017B7C, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x40);
}

/// `func_80006200(o)` (a vec3 track to its node's scale, **guess**): the
/// sample `p`, blended with a second one by bit 29 of `[o + 0x100]`,
/// exactly as in [`func_80006120`] (here at frame `+0xCC`, the second
/// sample at `+0xC0`). Then, if the node `n = [o + 0x124]` is nonzero, it
/// replaces the scale of `n`'s transform by `p`:
/// [`func_80017C18`](crate::misc::func_80017C18)`(n, M)` (the 3x4 to a 4x4
/// `M`), [`func_80081814`](crate::math::func_80081814)`(M, t, r, s)` (split
/// into translation, rotation and scale),
/// [`func_80081948`](crate::math::func_80081948)`(M, t, r, p)` (composed
/// again with `p` as the scale) and
/// [`func_80017BA8`](crate::misc::func_80017BA8)`(n, M)` (`n` re-read).
///
/// Frame (`sp - 0xD8`): `ra` at `+0x1C`, `s0` (`= o`) at `+0x18`, restored;
/// `s` at `+0x28`, `t` at `+0x34`, `r` at `+0x40`, `M` at `+0x80`, `p` at
/// `+0xCC`, `q` at `+0xC0`. Leaves `a0` = `n` (0 when it stops there) or
/// what the last callee left, `t6` the flags, `t7 = t6 << 2`, the blend
/// path's registers as in [`func_80006120`], and the callees' registers.
///
/// Domain: canonical `o`; the callees' domains (on the blend path `p`, `q`
/// and `w` not NaN as operands).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006200(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0xD8i64) as u64);
    sw(m, g[SP], 0x18, g[S0]);
    g[S0] = g[A0];
    sw(m, g[SP], 0x1C, g[RA]);
    g[A3] = lw(m, g[S0], 0x118);
    g[A2] = lw(m, g[S0], 0x114);
    g[A1] = g[S0];
    g[A0] = addu(g[SP], 0xCC);
    call(imports::func_80005DA8, m, ctx);
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = lw(m, g[S0], 0x100);
    g[T7] = sll(g[T6], 2);
    if (g[T7] as i64) >= 0 {
        g[A0] = lw(m, g[S0], 0x124);
    } else {
        g[T8] = lw(m, g[S0], 0xEC);
        g[A0] = addu(g[SP], 0xC0);
        g[A1] = g[S0];
        f[4].set_u32l(g[T8] as u32);
        g[A3] = lw(m, g[S0], 0xE8);
        f[4].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
        g[A2] = s32(f[4].u32l());
        call(imports::func_80005DA8, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        f[6].set_u32l(lw(m, g[SP], 0xCC) as u32);
        f[8].set_u32l(lw(m, g[S0], 0xE4) as u32);
        f[16].set_u32l(lw(m, g[SP], 0xD0) as u32);
        g[AT] = li(0x3F80_0000);
        f[10].set_fl(f[6].fl() * f[8].fl());
        f[6].set_u32l(lw(m, g[SP], 0xD4) as u32);
        g[A0] = addu(g[SP], 0xCC);
        g[A1] = g[A0];
        g[A3] = addu(g[SP], 0xC0);
        sw(m, g[SP], 0xCC, u64::from(f[10].u32l()));
        f[18].set_u32l(lw(m, g[S0], 0xE4) as u32);
        f[4].set_fl(f[16].fl() * f[18].fl());
        f[16].set_u32l(g[AT] as u32);
        sw(m, g[SP], 0xD0, u64::from(f[4].u32l()));
        f[8].set_u32l(lw(m, g[S0], 0xE4) as u32);
        f[10].set_fl(f[6].fl() * f[8].fl());
        sw(m, g[SP], 0xD4, u64::from(f[10].u32l()));
        f[18].set_u32l(lw(m, g[S0], 0xE4) as u32);
        f[4].set_fl(f[16].fl() - f[18].fl());
        g[A2] = s32(f[4].u32l());
        call(imports::func_800155EC, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = lw(m, g[S0], 0x124);
    }
    let g = &mut ctx.gpr;
    if g[A0] != 0 {
        g[A1] = addu(g[SP], 0x80);
        call(imports::func_80017C18, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = addu(g[SP], 0x80);
        g[A1] = addu(g[SP], 0x34);
        g[A2] = addu(g[SP], 0x40);
        g[A3] = addu(g[SP], 0x28);
        call(imports::func_80081814, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = addu(g[SP], 0x80);
        g[A1] = addu(g[SP], 0x34);
        g[A2] = addu(g[SP], 0x40);
        g[A3] = addu(g[SP], 0xCC);
        call(imports::func_80081948, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = lw(m, g[S0], 0x124);
        g[A1] = addu(g[SP], 0x80);
        call(imports::func_80017BA8, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0xD8);
}

/// `func_8000646C(o)` (a step track to its targets, **guess**): with the
/// segment `s = [o + 0x118]`, keys `k = [o + 0x11C]`, time `u = [o +
/// 0x114]` and words `v = [o + 0x120]`: `w = v[s + 1]` if `k[s + 1] < u`,
/// else `v[s]`; then for each entry `e` of the 0-terminated word list `[o +
/// 0x124]` (if that is nonzero), [`func_80017E70`](crate::misc::func_80017E70)`(e,
/// 2, w)`, which sets `[e + 8] = w`.
///
/// Frame (`sp - 0x28`): `ra`, `s2`, `s1`, `s0` at `+0x24..+0x18`, restored
/// (`s2 = w`, `s0` walking the list, `s1` the entry). Leaves `v0` = the
/// list, `t3` its first word, `t6`..`t9` the addresses used, and the
/// callee's registers.
///
/// Domain: canonical pointers; the keys, the words and the list in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000646C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x28i64) as u64);
    sw(m, g[SP], 0x24, g[RA]);
    sw(m, g[SP], 0x20, g[S2]);
    sw(m, g[SP], 0x1C, g[S1]);
    sw(m, g[SP], 0x18, g[S0]);
    g[V0] = lw(m, g[A0], 0x118);
    g[T7] = lw(m, g[A0], 0x11C);
    ctx.fpr[6].set_u32l(lw(m, g[A0], 0x114) as u32);
    g[T6] = sll(g[V0], 2);
    g[T8] = addu(g[T7], g[T6]);
    ctx.fpr[4].set_u32l(lw(m, g[T8], 4) as u32);
    g[V0] = g[T6];
    if !(ctx.fpr[4].fl() < ctx.fpr[6].fl()) {
        g[T1] = lw(m, g[A0], 0x120);
        g[T2] = addu(g[T1], g[V0]);
        g[S2] = lw(m, g[T2], 0);
    } else {
        g[T9] = lw(m, g[A0], 0x120);
        g[T0] = addu(g[T9], g[T6]);
        g[S2] = lw(m, g[T0], 4);
    }
    g[V0] = lw(m, g[A0], 0x124);
    if g[V0] != 0 {
        g[T3] = lw(m, g[V0], 0);
        g[S0] = g[V0];
        g[S1] = g[T3];
        if g[T3] != 0 {
            g[A0] = g[S1];
            loop {
                let g = &mut ctx.gpr;
                g[A1] = 2;
                g[A2] = g[S2];
                call(imports::func_80017E70, m, ctx);
                let g = &mut ctx.gpr;
                g[S1] = lw(m, g[S0], 4);
                g[S0] = addu(g[S0], 4);
                if g[S1] == 0 {
                    break;
                }
                g[A0] = g[S1];
            }
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x24);
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x28);
}

/// `func_8000651C(o, k)` (a float track to a node's halfword, **guess**: a
/// texture scroll or frame): `s` = [`func_80005CD4`]`(o, [o + 0x114], [o +
/// 0x118])` (as in [`func_80006120`], a float sample). If bit 29 of `[o +
/// 0x100]` is set it blends in a second sample, but through the vec3
/// sampler [`func_80005DA8`]`(o, f32([o + 0xEC]), [o + 0xE8])`, whose 12
/// bytes at frame `+0x24` overwrite `s` at `+0x28` with their `y`: so `s =
/// (1 - w) * q.x + w * q.y` (`w = [o + 0xE4]`, re-read; `w * q.y` stored
/// and re-read, then the sum in that order, not fused). QUIRK: the vec3
/// sampler reads `[o + 0x120]` as 12-byte values.
///
/// Then with the node `n = [o + 0x124]` and `t = [n + 8]`, both nonzero (or
/// nothing happens), and `j = 6` if `k` is nonzero, else `j = 4`: the
/// halfword `h[n + j] = trunc(f32(L) * s)` with `L = h[t + j]` (signed, the
/// product's operands in that order; `trunc` is the C cast), then wrapped
/// into `[0, L]`: while `L < v` (signed, `v = h[n + j]` re-read and `L`
/// re-read each step), `h[n + j] = v - L`; then while `v < 0`, `h[n + j] =
/// v + L`. Each new value is stored as a halfword and read back signed, so
/// the steps wrap at 16 bits, and the loops end for any `L` (for `L = 0`,
/// `v` is 0).
///
/// Frame (`sp - 0x38`): `ra` at `+0x14`; `o` spilled to its home slot
/// `+0x38` and re-read after each call, `k` to `+0x3C`. Leaves `a1` = `o`,
/// then `t` past the node test; `v0` the last value read (or `n`, 0 when
/// it stops there), `v1 = n`, `t9 = k`, `a0` the last `L` read and `at`
/// the last compare; the conversion's FPRs (`f16`/`f18`, `f4`, `f6`/`f8`,
/// `f10`) and step registers (`t6`/`t8`/`t9` or `t0`/`t2`..`t5`) as the
/// path left them; on the blend path `t8 = [o + 0xEC]`, `at` = 1.0, `f4 =
/// 1 - w`, `f16` the sum; and the callees' registers.
///
/// Domain: canonical `o`; the callees' domains; on the blend path `q.x`,
/// `q.y`, `w` and the product not NaN; for a nonzero `n` and `t`, `s` not
/// NaN (`f32(L) * s` is guarded), `n` and `t` canonical.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000651C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x38i64) as u64);
    sw(m, g[SP], 0x3C, g[A1]);
    g[A1] = g[A0];
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x38, g[A0]);
    g[A3] = lw(m, g[A1], 0x118);
    g[A2] = lw(m, g[A1], 0x114);
    sw(m, g[SP], 0x38, g[A1]);
    g[A0] = addu(g[SP], 0x28);
    call(imports::func_80005CD4, m, ctx);
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[A1] = lw(m, g[SP], 0x38);
    g[T6] = lw(m, g[A1], 0x100);
    g[T7] = sll(g[T6], 2);
    if (g[T7] as i64) >= 0 {
        g[V0] = lw(m, g[A1], 0x124);
    } else {
        g[T8] = lw(m, g[A1], 0xEC);
        g[A3] = lw(m, g[A1], 0xE8);
        sw(m, g[SP], 0x38, g[A1]);
        f[4].set_u32l(g[T8] as u32);
        g[A0] = addu(g[SP], 0x24);
        f[4].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
        g[A2] = s32(f[4].u32l());
        call(imports::func_80005DA8, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        g[A1] = lw(m, g[SP], 0x38);
        f[8].set_u32l(lw(m, g[SP], 0x28) as u32);
        g[AT] = li(0x3F80_0000);
        f[6].set_u32l(lw(m, g[A1], 0xE4) as u32);
        f[16].set_u32l(g[AT] as u32);
        f[10].set_fl(f[6].fl() * f[8].fl());
        f[6].set_u32l(lw(m, g[SP], 0x24) as u32);
        sw(m, g[SP], 0x28, u64::from(f[10].u32l()));
        f[18].set_u32l(lw(m, g[A1], 0xE4) as u32);
        f[10].set_u32l(lw(m, g[SP], 0x28) as u32);
        f[4].set_fl(f[16].fl() - f[18].fl());
        f[8].set_fl(f[4].fl() * f[6].fl());
        f[16].set_fl(f[8].fl() + f[10].fl());
        sw(m, g[SP], 0x28, u64::from(f[16].u32l()));
        g[V0] = lw(m, g[A1], 0x124);
    }
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    if g[V0] != 0 {
        g[A1] = lw(m, g[V0], 8);
        g[V1] = g[V0];
        g[T9] = lw(m, g[SP], 0x3C);
        if g[A1] != 0 {
            if g[T9] != 0 {
                g[T6] = lh(m, g[A1], 6);
                f[4].set_u32l(lw(m, g[SP], 0x28) as u32);
                f[16].set_u32l(g[T6] as u32);
                f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fpu::NEAREST));
                f[6].set_fl(f[18].fl() * f[4].fl());
                f[8].set_u32l(fpu::trunc_w_s(f[6].fl()));
                g[T8] = s32(f[8].u32l());
                sh(m, g[V1], 6, g[T8]);
                g[V0] = lh(m, g[V1], 6);
                g[A0] = lh(m, g[A1], 6);
                g[AT] = slt(g[A0], g[V0]);
                g[T9] = subu(g[V0], g[A0]);
                while g[AT] != 0 {
                    sh(m, g[V1], 6, g[T9]);
                    g[V0] = lh(m, g[V1], 6);
                    g[A0] = lh(m, g[A1], 6);
                    g[AT] = slt(g[A0], g[V0]);
                    if g[AT] != 0 {
                        g[T9] = subu(g[V0], g[A0]);
                    }
                }
                if (g[V0] as i64) < 0 {
                    g[T0] = lh(m, g[A1], 6);
                    loop {
                        g[T1] = addu(g[V0], g[T0]);
                        sh(m, g[V1], 6, g[T1]);
                        g[V0] = lh(m, g[V1], 6);
                        if (g[V0] as i64) >= 0 {
                            break;
                        }
                        g[T0] = lh(m, g[A1], 6);
                    }
                }
            } else {
                g[T0] = lh(m, g[A1], 4);
                f[6].set_u32l(lw(m, g[SP], 0x28) as u32);
                f[18].set_u32l(g[T0] as u32);
                f[4].set_fl(fpu::cvt_s_w(f[18].u32l(), fpu::NEAREST));
                f[8].set_fl(f[4].fl() * f[6].fl());
                f[10].set_u32l(fpu::trunc_w_s(f[8].fl()));
                g[T2] = s32(f[10].u32l());
                sh(m, g[V0], 4, g[T2]);
                g[V0] = lh(m, g[V0], 4);
                g[A0] = lh(m, g[A1], 4);
                g[AT] = slt(g[A0], g[V0]);
                g[T3] = subu(g[V0], g[A0]);
                while g[AT] != 0 {
                    sh(m, g[V1], 4, g[T3]);
                    g[V0] = lh(m, g[V1], 4);
                    g[A0] = lh(m, g[A1], 4);
                    g[AT] = slt(g[A0], g[V0]);
                    if g[AT] != 0 {
                        g[T3] = subu(g[V0], g[A0]);
                    }
                }
                if (g[V0] as i64) < 0 {
                    g[T4] = lh(m, g[A1], 4);
                    loop {
                        g[T5] = addu(g[V0], g[T4]);
                        sh(m, g[V1], 4, g[T5]);
                        g[V0] = lh(m, g[V1], 4);
                        if (g[V0] as i64) >= 0 {
                            break;
                        }
                        g[T4] = lh(m, g[A1], 4);
                    }
                }
            }
        }
    }
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x38);
}

/// `func_80006704(o)`: the key segment holding the time `u = [o + 0x114]`
/// among the `n = [o + 0x104]` float keys at `k = [o + 0x11C]`: `n - 2` if
/// `k[n - 1] < u`; else 0 if `u < k[0]`; else the largest `i <= n - 2` with
/// `!(u < k[i])`, scanning down from `n - 2`. Only compares, so a NaN `u` or
/// key just makes the compares false (a NaN `u` gives `n - 2`).
///
/// The scan stops at `i = 0` at the latest (`u < k[0]` is false by then),
/// so it is bounded for `n >= 2`. QUIRK: for `n < 2` it starts below
/// `k[0]` and runs down through memory until some word compares `<= u`.
///
/// Leaves `v1 = n`, `f0 = u`, `t6 = 4n`, `t7 = &k[n]`, `f4 = k[n - 1]`, and
/// past the first test `f6 = k[0]`, `a0` = the result, `t8 = 4(n - 2)`,
/// `a1 = &k[result]`, `f8 = k[n - 2]`, `f10` = the key that stopped the
/// scan.
///
/// Domain: `n >= 2` and the keys in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006704(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V1] = lw(m, g[A0], 0x104);
    g[V0] = lw(m, g[A0], 0x11C);
    f[0].set_u32l(lw(m, g[A0], 0x114) as u32);
    g[T6] = sll(g[V1], 2);
    g[T7] = addu(g[V0], g[T6]);
    f[4].set_u32l(lw(m, g[T7], -4) as u32);
    if f[4].fl() < f[0].fl() {
        g[V0] = addu(g[V1], (-2i64) as u64);
        return;
    }
    f[6].set_u32l(lw(m, g[V0], 0) as u32);
    g[A0] = addu(g[V1], (-2i64) as u64);
    g[T8] = sll(g[A0], 2);
    g[A1] = addu(g[V0], g[T8]);
    if f[0].fl() < f[6].fl() {
        g[V0] = 0;
        return;
    }
    f[8].set_u32l(lw(m, g[A1], 0) as u32);
    if f[0].fl() < f[8].fl() {
        loop {
            f[10].set_u32l(lw(m, g[A1], -4) as u32);
            g[A0] = addu(g[A0], u64::MAX);
            g[A1] = addu(g[A1], (-4i64) as u64);
            if !(f[0].fl() < f[10].fl()) {
                break;
            }
        }
    }
    g[V0] = g[A0];
}

/// `func_80006D5C(id, kind)`: find an object in [`OBJECTS`] by id and kind.
///
/// `id == 0` returns 0 at once. Otherwise it visits the first `[0x8009A2A0]`
/// (signed) entries and returns the first nonzero `o` whose flags word
/// `[o + 0x100]` has bit 31 clear, `flags & 0xF == kind` and `[o + 0x124] ==
/// id`, else 0. Both equalities compare full 64-bit registers (the loaded
/// words sign-extended), so a `kind` or `id` with a nonstandard upper half
/// never matches. QUIRK: nothing bounds the count by the table's 300 entries.
///
/// `s0` holds `kind` and is restored from its low word; the loop's
/// temporaries (`v1` count, `a1` last entry, `a2` next slot, `a3`, `t0`,
/// `t6`-`t8`, `at`) are left as the original leaves them.
///
/// Domain: canonical `sp` with its 8-byte frame in RDRAM; nonzero entries
/// canonical pointers to objects in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006D5C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-8i64) as u64);
    sw(m, g[SP], 4, g[S0]);
    g[S0] = g[A1]; // bnez delay slot
    'found: {
        if g[A0] == 0 {
            g[V0] = 0;
            break 'found;
        }
        g[V1] = lw(m, li(0x800A_0000), -0x5D60);
        g[A2] = li(OBJECTS);
        g[V0] = 0; // blez delay slot
        if (g[V1] as i64) > 0 {
            g[T0] = li(0x8000_0000);
            loop {
                g[A1] = lw(m, g[A2], 0);
                g[V0] = addu(g[V0], 1);
                g[AT] = u64::from((g[V0] as i64) < (g[V1] as i64));
                if g[A1] != 0 {
                    g[A3] = lw(m, g[A1], 0x100);
                    g[T6] = g[A3] & g[T0];
                    g[T7] = g[A3] & 0xF; // bnez delay slot
                    if g[T6] == 0 && g[S0] == g[T7] {
                        g[T8] = lw(m, g[A1], 0x124);
                        if g[A0] == g[T8] {
                            g[V0] = g[A1]; // b delay slot
                            break 'found;
                        }
                    }
                }
                // L_80006DD0: bnez at / (delay) addiu a2, a2, 4
                g[A2] = addu(g[A2], 4);
                if g[AT] == 0 {
                    break;
                }
            }
        }
        // L_80006DD8
        g[V0] = 0;
    }
    // L_80006DDC
    g[S0] = lw(m, g[SP], 4);
    g[SP] = addu(g[SP], 8);
}

/// `func_80006DE8(o, a, b)` with the floats `a`, `b` in `a1`/`a2`: set the
/// range `+0xF0 = a`, `+0xF4 = b`, `+0xF8 = b - a`, each clamped below at
/// 0.0 first (`x < 0` becomes `+0.0`, so `-0.0` stays). QUIRK: `b < a`
/// hangs (`b .`, N64Recomp's `pause_self`), before anything is stored.
///
/// Leaves `f14`/`f12` = the clamped `a`/`b`, `f0 = 0.0`, `f4 = b - a`.
///
/// Domain: `a`, `b` not NaN (the subtraction is guarded).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006DE8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[14].set_u32l(g[A1] as u32);
    f[12].set_u32l(g[A2] as u32);
    if f[12].fl() < f[14].fl() {
        imports::runtime::pause_self(m.as_mut_ptr());
    }
    f[0].set_u32l(0);
    if f[14].fl() < f[0].fl() {
        f[14].set_u32l(f[0].u32l());
    }
    if f[12].fl() < f[0].fl() {
        f[12].set_u32l(f[0].u32l());
    }
    f[4].set_fl(f[12].fl() - f[14].fl());
    sw(m, g[A0], 0xF0, u64::from(f[14].u32l()));
    sw(m, g[A0], 0xF4, u64::from(f[12].u32l()));
    sw(m, g[A0], 0xF8, u64::from(f[4].u32l()));
}

/// `func_80006E50(o, bits)`: `[o + 0x100] |= bits` (an object's flags).
///
/// Domain: canonical `o` with `o + 0x100` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006E50(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = lw(&mem, g[A0], 0x100);
    g[T7] = g[T6] | g[A1];
    sw(&mut mem, g[A0], 0x100, g[T7]);
}

/// `func_80006E60(o, bits)`: `[o + 0x100] &= !bits`.
///
/// Domain: canonical `o` with `o + 0x100` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006E60(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = lw(&mem, g[A0], 0x100);
    g[T7] = !g[A1]; // nor t7, a1, zero: all 64 bits
    g[T8] = g[T6] & g[T7];
    sw(&mut mem, g[A0], 0x100, g[T8]);
}

/// `func_80006E74(o, u)` (set a track's time, **guess**): with the float
/// `u` in `a1`, `[o + 0x114] = u`, `[o + 0x118]` = the key segment of `u`
/// ([`func_80006704`]), and bit 24 of the flags `[o + 0x100]` set (read
/// after the call).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`; `o` spilled to its home slot `sp
/// + 0`. Leaves `f12 = u`, `at = 0x01000000`, `t6`/`t7` the flags before
/// and after, and [`func_80006704`]'s registers.
///
/// Domain: [`func_80006704`]'s (at least two keys).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006E74(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    ctx.fpr[12].set_u32l(g[A1] as u32);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[A0], 0x114, u64::from(ctx.fpr[12].u32l()));
    sw(m, g[SP], 0x18, g[A0]);
    call(imports::func_80006704, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = lw(m, g[SP], 0x18);
    g[AT] = li(0x0100_0000);
    g[T6] = lw(m, g[A0], 0x100);
    sw(m, g[A0], 0x118, g[V0]);
    g[T7] = g[T6] | g[AT];
    sw(m, g[A0], 0x100, g[T7]);
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80006EB4(o, x)`: `[o + 0x110] = x` (a float passed in `a1`, a
/// rate, **guess**). Leaves `f12 = x`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006EB4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    ctx.fpr[12].set_u32l(ctx.gpr[A1] as u32);
    sw(&mut mem, ctx.gpr[A0], 0x110, u64::from(ctx.fpr[12].u32l()));
}

/// `func_80006EC0(o, u, w)` (start a blend to a new time, **guess**): with
/// the floats `u`, `w` in `a1`, `a2`: bit 29 of the flags `[o + 0x100]` set,
/// `[o + 0x114] = u`, `[o + 0xE0] = w`, `[o + 0xE4] = 0.0`, `[o + 0xE8]` =
/// the old segment `[o + 0x118]`, `[o + 0xEC] = trunc(old time)` (the C
/// cast of the old `[o + 0x114]`: an integer, `0x80000000` out of range),
/// then `[o + 0x118]` = the new segment ([`func_80006704`]).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`; `o` spilled to its home slot `sp
/// + 0` for the call. Leaves `f6` = the old time, `f8`/`t0` its
/// truncation, `f12 = w`, `f14 = u`, `f4 = 0.0`, `t6`/`t7` the flags, `t8`
/// the old segment, `at = 0x20000000`, and [`func_80006704`]'s registers.
///
/// Domain: [`func_80006704`]'s.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006EC0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    ctx.fpr[6].set_u32l(lw(m, g[A0], 0x114) as u32);
    g[T6] = lw(m, g[A0], 0x100);
    ctx.fpr[14].set_u32l(g[A1] as u32);
    ctx.fpr[8].set_u32l(fpu::trunc_w_s(ctx.fpr[6].fl()));
    ctx.fpr[12].set_u32l(g[A2] as u32);
    ctx.fpr[4].set_u32l(0);
    g[T8] = lw(m, g[A0], 0x118);
    g[T0] = s32(ctx.fpr[8].u32l());
    g[AT] = li(0x2000_0000);
    g[T7] = g[T6] | g[AT];
    sw(m, g[A0], 0x100, g[T7]);
    sw(m, g[A0], 0x114, u64::from(ctx.fpr[14].u32l()));
    sw(m, g[A0], 0xE0, u64::from(ctx.fpr[12].u32l()));
    sw(m, g[A0], 0xE4, u64::from(ctx.fpr[4].u32l()));
    sw(m, g[A0], 0xE8, g[T8]);
    sw(m, g[A0], 0xEC, g[T0]);
    sw(m, g[SP], 0x18, g[A0]);
    call(imports::func_80006704, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = lw(m, g[SP], 0x18);
    sw(m, g[A0], 0x118, g[V0]);
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80006F28(o, x)`: `[o + 0xDC] = x` (a float passed in `a1`; the
/// field [`func_80005BB8`] zeroes). Leaves `f12 = x`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006F28(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    ctx.fpr[12].set_u32l(ctx.gpr[A1] as u32);
    sw(&mut mem, ctx.gpr[A0], 0xDC, u64::from(ctx.fpr[12].u32l()));
}

/// `func_80030964(p)` (start a block's animations, **guess**): `p` points
/// at words ending in -1. After the -1, a `"Data"` chunk (the tag, a count
/// `n`, `n` words; nothing skipped for `n <= 0`) is skipped if present;
/// then, if an `"Anim"` chunk follows, its zero-terminated list of object
/// pointers is walked: [`func_80005BB8`]`(o)` for each (starting it),
/// with `lo` the unsigned minimum of the entries (each re-read after its
/// call). Returns the list's address, or 0 without an `"Anim"` chunk.
///
/// Then, with `lo != -1` (the list had an entry below `0xFFFFFFFF`): `d =
/// [0x800D9DC4] - lo` is stored at `0x800D9DD0` and `[0x800D9DC8] -= d`;
/// otherwise `[0x800D9DD0] = 0` (heap bookkeeping, **guess**).
///
/// Frame (`sp - 0x28`): `s0` at `+0x14`, `s1` (holding `lo`) at `+0x18`,
/// `ra` at `+0x1C`, all restored sign-extended; the result at `+0x20`,
/// read back into `v0`. Leaves `at = -1`, `v1 = 0x800D9DD0`, with `lo`:
/// `t1`, `t3` the old words, `t2 = d`, `t5` the new `[0x800D9DC8]`; `t6`
/// .. `t9` from the scans, `t0` = the first entry, and the callee's
/// registers.
///
/// Domain: the words from `p` to the end of the list in RDRAM, and each
/// entry an object [`func_80005BB8`] can start.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80030964(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x28i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x18, g[S1]);
    sw(m, g[SP], 0x14, g[S0]);
    g[T6] = lw(m, g[A0], 0);
    g[V1] = u64::MAX;
    g[V0] = g[A0];
    g[S1] = u64::MAX;
    if g[V1] != g[T6] {
        g[T7] = lw(m, g[V0], 4);
        loop {
            g[V0] = addu(g[V0], 4);
            if g[V1] == g[T7] {
                break;
            }
            g[T7] = lw(m, g[V0], 4);
        }
    }
    g[T8] = lw(m, g[V0], 4);
    g[AT] = li(0x4461_7461);
    g[V0] = addu(g[V0], 4);
    if g[T8] == g[AT] {
        g[V1] = lw(m, g[V0], 4);
        g[V0] = addu(g[V0], 8);
        if (g[V1] as i64) <= 0 {
            g[V1] = addu(g[V1], u64::MAX);
        } else {
            loop {
                g[V1] = addu(g[V1], u64::MAX);
                g[V0] = addu(g[V0], 4);
                if (g[V1] as i64) <= 0 {
                    break;
                }
            }
        }
    }
    // An "Anim" chunk: start each object, s1 = the lowest entry.
    g[T9] = lw(m, g[V0], 0);
    g[AT] = li(0x416E_696D);
    g[S0] = addu(g[V0], 4);
    if g[T9] != g[AT] {
        sw(m, g[SP], 0x20, 0);
    } else {
        sw(m, g[SP], 0x20, g[S0]);
        g[T0] = lw(m, g[S0], 0);
        g[A0] = g[T0];
        if g[T0] != 0 {
            loop {
                call(imports::func_80005BB8, m, ctx);
                let g = &mut ctx.gpr;
                g[V0] = lw(m, g[S0], 0);
                g[AT] = sltu(g[V0], g[S1]);
                if g[AT] != 0 {
                    g[S1] = g[V0];
                }
                g[A0] = lw(m, g[S0], 4);
                g[S0] = addu(g[S0], 4);
                if g[A0] == 0 {
                    break;
                }
            }
        }
    }
    let g = &mut ctx.gpr;
    g[AT] = u64::MAX;
    g[V0] = li(0x800E_0000);
    if g[S1] != g[AT] {
        g[T1] = li(0x800E_0000);
        g[T1] = lw(m, g[T1], -0x623C);
        g[V0] = addu(g[V0], (-0x6238i64) as u64);
        g[T3] = lw(m, g[V0], 0);
        g[V1] = li(0x800E_0000);
        g[T2] = subu(g[T1], g[S1]);
        g[V1] = addu(g[V1], (-0x6230i64) as u64);
        g[T5] = subu(g[T3], g[T2]);
        sw(m, g[V1], 0, g[T2]);
        sw(m, g[V0], 0, g[T5]);
    } else {
        g[V1] = li(0x800D_9DD0);
        sw(m, g[V1], 0, 0);
    }
    g[RA] = lw(m, g[SP], 0x1C);
    g[V0] = lw(m, g[SP], 0x20);
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x28);
}

/// `func_80033928(list, x)` with the float `x` in `a1`: for each object of
/// the zero-terminated list (nothing if `list` or its first entry is
/// null), [`func_80006EB4`]`(o, x)` (`[o + 0x110] = x`).
///
/// Frame (`sp - 0x28`): `f20` (all 64 bits; `x` in between) at `+0x10`,
/// `s0` at `+0x1C`, `s1` at `+0x20`, `ra` at `+0x24`, all restored.
/// Leaves `a1 = x` (sign-extended), `a0` = the last object, `t6` = the
/// first entry, and the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033928(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_80033978: {
        g[SP] = addu(g[SP], (-0x28i64) as u64);
        sd(m, g[SP], 0x10, ctx.fpr[20].u64);
        ctx.fpr[20].set_u32l(g[A1] as u32);
        sw(m, g[SP], 0x24, g[RA]);
        sw(m, g[SP], 0x20, g[S1]);
        sw(m, g[SP], 0x1C, g[S0]);
        if g[A0] != 0 {
            g[T6] = lw(m, g[A0], 0);
            g[S0] = g[A0];
            if g[T6] == 0 {
                g[RA] = lw(m, g[SP], 0x24);
                break 'b_80033978;
            }
            g[S1] = lw(m, g[A0], 0);
            g[A1] = s32(ctx.fpr[20].u32l());
            loop {
                let g = &mut ctx.gpr;
                g[A0] = g[S1];
                call(imports::func_80006EB4, m, ctx);
                let g = &mut ctx.gpr;
                g[S1] = lw(m, g[S0], 4);
                g[S0] = addu(g[S0], 4);
                if g[S1] == 0 {
                    break;
                }
                g[A1] = s32(ctx.fpr[20].u32l());
            }
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x24);
    }
    let g = &mut ctx.gpr;
    ctx.fpr[20].u64 = ld(m, g[SP], 0x10);
    g[S0] = lw(m, g[SP], 0x1C);
    g[S1] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x28);
}

/// `func_8003398C(list, t, x, y, mode, w, z)` (start a set of animation
/// objects, **guess**): `t`, `x`, `y` are floats in `a1`..`a3`; `mode` (a
/// word), `w` and `z` (floats) are on the stack. For each object `o` of the
/// zero-terminated word list at `list` (nothing if `list` or its first word
/// is 0; each call re-reads `o` from the list):
/// - [`func_80006DE8`]`(o, x, y)`;
/// - `mode` 0: the flags `|= 0x04000000` ([`func_80006E50`]), `&=
///   !0x02000000` ([`func_80006E60`]); otherwise `|= 0x02000010`, `&=
///   !0x04000000`, and bit `0x40` set with [`func_80006F28`]`(o, z)` if `0 <
///   z`, else cleared;
/// - if `0 <= t`: [`func_80006E74`]`(o, t)` if not `0 < w`, else
///   [`func_80006EC0`]`(o, t, w)`.
///
/// Frame (`sp - 0x60`): `ra`, `s4`..`s0` at `+0x5C..+0x48`, `f30`, `f28`,
/// `f26`, `f24`, `f22`, `f20` (64-bit) at `+0x40`, `+0x38`, `+0x30`,
/// `+0x28`, `+0x20`, `+0x18`, restored (`s0` the list cursor, `s1 =
/// 0x04000000`, `s2 = mode`, `s3 = 0x02000010`, `s4 = 0x02000000`, `f20` =
/// 0.0, `f22 = t`, `f24 = z`, `f26 = w`, `f28 = x`, `f30 = y` meanwhile);
/// `list` spilled to its home slot `+0x60`. Leaves `a0` = the list's 0, `a1`,
/// `a2` the last call's, `t6` the first word, and the callees' registers.
///
/// Domain: the callees'; the list and its objects in RDRAM. `t`, `w` and
/// `z` are only compared here.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003398C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[SP] = addu(g[SP], (-0x60i64) as u64);
    sd(m, g[SP], 0x40, f[30].u64);
    sd(m, g[SP], 0x38, f[28].u64);
    sd(m, g[SP], 0x20, f[22].u64);
    f[22].set_u32l(g[A1] as u32);
    f[28].set_u32l(g[A2] as u32);
    f[30].set_u32l(g[A3] as u32);
    sw(m, g[SP], 0x5C, g[RA]);
    sw(m, g[SP], 0x58, g[S4]);
    sw(m, g[SP], 0x54, g[S3]);
    sw(m, g[SP], 0x50, g[S2]);
    sw(m, g[SP], 0x4C, g[S1]);
    sw(m, g[SP], 0x48, g[S0]);
    sd(m, g[SP], 0x30, f[26].u64);
    sd(m, g[SP], 0x28, f[24].u64);
    sd(m, g[SP], 0x18, f[20].u64);
    sw(m, g[SP], 0x60, g[A0]);
    if g[A0] != 0 {
        g[T6] = lw(m, g[A0], 0);
        g[S0] = g[A0];
        f[26].set_u32l(lw(m, g[SP], 0x74) as u32);
        f[24].set_u32l(lw(m, g[SP], 0x78) as u32);
        if g[T6] != 0 {
            g[S3] = li(0x200_0000);
            f[20].set_u32l(0);
            g[S3] |= 0x10;
            g[A0] = lw(m, g[A0], 0);
            g[S4] = li(0x200_0000);
            g[S2] = lw(m, g[SP], 0x70);
            g[S1] = li(0x400_0000);
            g[A1] = s32(f[28].u32l());
            loop {
                let g = &mut ctx.gpr;
                g[A2] = s32(ctx.fpr[30].u32l());
                call(imports::func_80006DE8, m, ctx);
                let g = &mut ctx.gpr;
                g[A1] = g[S1];
                if g[S2] == 0 {
                    g[A0] = lw(m, g[S0], 0);
                    call(imports::func_80006E50, m, ctx);
                    let g = &mut ctx.gpr;
                    g[A0] = lw(m, g[S0], 0);
                    g[A1] = g[S4];
                    call(imports::func_80006E60, m, ctx);
                } else {
                    g[A0] = lw(m, g[S0], 0);
                    g[A1] = g[S3];
                    call(imports::func_80006E50, m, ctx);
                    let g = &mut ctx.gpr;
                    g[A0] = lw(m, g[S0], 0);
                    g[A1] = g[S1];
                    call(imports::func_80006E60, m, ctx);
                    let g = &mut ctx.gpr;
                    let f = &mut ctx.fpr;
                    let shown = f[20].fl() < f[24].fl();
                    g[A1] = 0x40;
                    if !shown {
                        g[A0] = lw(m, g[S0], 0);
                        call(imports::func_80006E60, m, ctx);
                    } else {
                        g[A0] = lw(m, g[S0], 0);
                        g[A1] = 0x40;
                        call(imports::func_80006E50, m, ctx);
                        let g = &mut ctx.gpr;
                        g[A1] = s32(ctx.fpr[24].u32l());
                        g[A0] = lw(m, g[S0], 0);
                        call(imports::func_80006F28, m, ctx);
                    }
                }
                let g = &mut ctx.gpr;
                let f = &mut ctx.fpr;
                if f[20].fl() <= f[22].fl() {
                    if !(f[20].fl() < f[26].fl()) {
                        g[A1] = s32(f[22].u32l());
                        g[A0] = lw(m, g[S0], 0);
                        call(imports::func_80006E74, m, ctx);
                    } else {
                        g[A1] = s32(f[22].u32l());
                        g[A2] = s32(f[26].u32l());
                        g[A0] = lw(m, g[S0], 0);
                        call(imports::func_80006EC0, m, ctx);
                    }
                }
                let g = &mut ctx.gpr;
                g[A0] = lw(m, g[S0], 4);
                g[S0] = addu(g[S0], 4);
                if g[A0] == 0 {
                    break;
                }
                g[A1] = s32(ctx.fpr[28].u32l());
            }
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x5C);
    ctx.fpr[20].u64 = ld(m, g[SP], 0x18);
    ctx.fpr[22].u64 = ld(m, g[SP], 0x20);
    ctx.fpr[24].u64 = ld(m, g[SP], 0x28);
    ctx.fpr[26].u64 = ld(m, g[SP], 0x30);
    ctx.fpr[28].u64 = ld(m, g[SP], 0x38);
    ctx.fpr[30].u64 = ld(m, g[SP], 0x40);
    g[S0] = lw(m, g[SP], 0x48);
    g[S1] = lw(m, g[SP], 0x4C);
    g[S2] = lw(m, g[SP], 0x50);
    g[S3] = lw(m, g[SP], 0x54);
    g[S4] = lw(m, g[SP], 0x58);
    g[SP] = addu(g[SP], 0x60);
}

/// `func_800736AC(list)`: 1 if any object in the 0-terminated pointer list
/// is not running (bit 28 of `[o + 0x100]` clear) or has reached its end
/// (`[o + 0x108] <= [o + 0x114]`, floats: the end against the time,
/// **guess**), else 0 (an empty list included). The first object found
/// stops the walk.
///
/// Leaves `v1 = 0x10000000`, `a0` = the slot of the object that stopped
/// it (or of the terminator), `t6`/`t7` its flags and bit, and `f4`/`f6`
/// its floats if they were compared.
///
/// Domain: canonical pointers; any float values (only compared).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800736AC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, g[A0], 0);
    g[V1] = li(0x1000_0000);
    g[V0] = any_done(m, g, &mut ctx.fpr, A0, V0, V1, (T6, T7));
}

/// The walk of [`func_800736AC`] and [`func_8007B7BC`]: `slot` holds the
/// list slot, `obj` the first object, `mask` bit 28; the flags go through
/// `t`. Returns the result (the C leaves `obj` as is on a 1).
fn any_done(m: &Mem, g: &mut [u64; 32], f: &mut [Fpr; 32], slot: usize, obj: usize, mask: usize, t: (usize, usize)) -> u64 {
    if g[obj] == 0 {
        return 0;
    }
    g[t.0] = lw(m, g[obj], 0x100);
    loop {
        g[t.1] = g[t.0] & g[mask];
        if g[t.1] == 0 {
            return 1;
        }
        f[4].set_u32l(lw(m, g[obj], 0x114) as u32);
        f[6].set_u32l(lw(m, g[obj], 0x108) as u32);
        if f[6].fl() <= f[4].fl() {
            return 1;
        }
        g[obj] = lw(m, g[slot], 4);
        g[slot] = addu(g[slot], 4);
        if g[obj] == 0 {
            return 0;
        }
        g[t.0] = lw(m, g[obj], 0x100);
    }
}

/// `func_8007B7BC(i)`: [`func_800736AC`] over the list `[0x8011C8F0 + 4i]`
/// (the index 32-bit).
///
/// Leaves `t6 = 4i`, `a0 = 0x10000000`, `v0` = the slot that stopped the
/// walk, `v1` = that object (0 at the end), `t7`/`t8` its flags and bit,
/// `f4`/`f6` its floats if compared.
///
/// Domain: `[0x8011C8F0 + 4i]` in RDRAM and a canonical list pointer.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8007B7BC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 2);
    g[V0] = li(0x8012_0000);
    g[V0] = addu(g[V0], g[T6]);
    g[V0] = lw(m, g[V0], -0x3710);
    g[A0] = li(0x1000_0000);
    g[V1] = lw(m, g[V0], 0);
    g[V0] = any_done(m, g, &mut ctx.fpr, V0, V1, A0, (T7, T8));
}
