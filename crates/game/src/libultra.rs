//! libultra's `sinf` and `cosf` (NOTES.md, "The OS boundary"): single
//! precision in and out, double precision inside, with the polynomial and
//! the reduction constants read from their tables in the data segment. The
//! game's own maths routines: ports never substitute `std` (SPEC §5.4).

// Ports keep N64Recomp's names (func_8008A8C0), capitals included.
#![allow(non_snake_case)]

use crate::recomp::{enter, fpu, ld, li, lw, reg::*, s32, sra, slt, sw, RecompContext};

/// `func_8008A8C0(x)` = `sinf`: for `xpt = (bits(x) >> 22) & 0x1FF`:
/// - `xpt < 230` (tiny): `x` itself.
/// - `xpt < 255`: `dx + (dx * xsq) * poly(xsq)` in double, `xsq = dx * dx`.
/// - `xpt < 310`: reduce by `n = round(dx * rpi)` (away from zero: `dn +/-
///   0.5`, truncated), `dx -= n * pihi`, `dx -= n * pilo`, the same
///   polynomial, negated if `n` is odd.
/// - otherwise: the word at `0x800AE110` (a quiet NaN) for NaN, else the
///   word at `0x800ADE60` (zero).
///
/// The polynomial is `((P4 xsq + P3) xsq + P2) xsq + P1` with `P1..P4` the
/// doubles at `0x800ADE28..0x800ADE40`; `rpi`, `pihi`, `pilo` at
/// `0x800ADE48..0x800ADE58`. `x` goes through its stack slot `[sp]` (the
/// word read back into `v0` and `f4`). The result's float is rounded to
/// nearest (`cvt.s.d`).
///
/// Domain: any `x`; NaN takes the branch without arithmetic. Leaves `v0` =
/// the bits (or `n`), `v1 = xpt` (or the table), `t6`, `t9` = `n & 1`,
/// `at`, and the double temporaries in `f2`..`f18` as the C leaves them.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008A8C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0, u64::from(ctx.fpr[12].u32l()));
    g[V0] = lw(m, g[SP], 0);
    ctx.fpr[4].set_u32l(lw(m, g[SP], 0) as u32);
    g[V1] = sra(g[V0], 22);
    g[T6] = g[V1] & 0x1FF;
    g[AT] = slt(g[T6], 0xFF);
    g[V1] = g[T6];
    if g[AT] != 0 {
        g[AT] = slt(g[T6], 0xE6);
        ctx.fpr[2].set_d(f64::from(ctx.fpr[4].fl()));
        if g[AT] != 0 {
            // Tiny: x itself.
            ctx.fpr[0].set_u32l(lw(m, g[SP], 0) as u32);
            return;
        }
        let f = &mut ctx.fpr;
        f[12].set_d(f[2].d() * f[2].d()); // xsq
        g[V1] = li(0x800A_DE20);
        f[6].u64 = ld(m, g[V1], 0x20);
        f[10].u64 = ld(m, g[V1], 0x18);
        f[4].u64 = ld(m, g[V1], 0x10);
        f[8].set_d(f[6].d() * f[12].d());
        f[16].set_d(f[8].d() + f[10].d());
        f[10].u64 = ld(m, g[V1], 8);
        f[18].set_d(f[16].d() * f[12].d());
        f[6].set_d(f[18].d() + f[4].d());
        f[8].set_d(f[6].d() * f[12].d());
        f[14].set_d(f[10].d() + f[8].d()); // poly
        f[16].set_d(f[2].d() * f[12].d());
        f[18].set_d(f[16].d() * f[14].d());
        f[4].set_d(f[18].d() + f[2].d());
        f[0].set_fl(fpu::cvt_s_d(f[4].d(), fpu::NEAREST));
        return;
    }
    g[AT] = slt(g[V1], 0x136);
    ctx.fpr[4].set_u32l(lw(m, g[SP], 0) as u32);
    if g[AT] == 0 {
        // NaN or huge: a constant. (`c.eq.s x, x` isn't NaN-checked.)
        let nan = ctx.fpr[4].fl() != ctx.fpr[4].fl();
        g[AT] = li(0x800B_0000);
        let at = if nan { -0x1EF0 } else { -0x21A0 };
        ctx.fpr[0].set_u32l(lw(m, g[AT], at) as u32);
        return;
    }
    // Reduce by n = round(x / pi).
    let f = &mut ctx.fpr;
    f[6].set_u32l(lw(m, g[SP], 0) as u32);
    g[AT] = li(0x800B_0000);
    f[10].u64 = ld(m, g[AT], -0x21B8); // rpi
    f[2].set_d(f64::from(f[6].fl())); // dx
    f[8].set_u32h(0); // f9
    f[0].set_d(f[2].d() * f[10].d()); // dn
    f[8].set_u32l(0);
    g[AT] = li(0x3FE0_0000); // 0.5, high word
    if f[8].d() <= f[0].d() {
        f[16].set_u32h(g[AT] as u32); // f17
        f[16].set_u32l(0);
        f[18].set_d(f[0].d() + f[16].d());
        f[4].set_u32l(fpu::trunc_w_d(f[18].d()));
        g[V0] = s32(f[4].u32l());
    } else {
        f[6].set_u32h(g[AT] as u32); // f7
        f[6].set_u32l(0);
        f[10].set_d(f[0].d() - f[6].d());
        f[8].set_u32l(fpu::trunc_w_d(f[10].d()));
        g[V0] = s32(f[8].u32l());
    }
    f[16].set_u32l(g[V0] as u32);
    g[AT] = li(0x800B_0000);
    f[18].u64 = ld(m, g[AT], -0x21B0); // pihi
    f[0].set_d(f64::from(f[16].u32l() as i32)); // dn = n
    f[6].u64 = ld(m, g[AT], -0x21A8); // pilo
    g[V1] = li(0x800A_DE20);
    f[4].set_d(f[0].d() * f[18].d());
    f[8].u64 = ld(m, g[V1], 0x20);
    f[18].u64 = ld(m, g[V1], 0x18);
    g[T9] = g[V0] & 1;
    f[10].set_d(f[0].d() * f[6].d());
    f[2].set_d(f[2].d() - f[4].d());
    f[2].set_d(f[2].d() - f[10].d());
    f[10].u64 = ld(m, g[V1], 0x10);
    f[12].set_d(f[2].d() * f[2].d()); // xsq
    f[16].set_d(f[8].d() * f[12].d());
    f[4].set_d(f[16].d() + f[18].d());
    f[18].u64 = ld(m, g[V1], 8);
    f[6].set_d(f[4].d() * f[12].d());
    f[8].set_d(f[6].d() + f[10].d());
    f[16].set_d(f[8].d() * f[12].d());
    f[14].set_d(f[18].d() + f[16].d()); // poly
    if g[T9] != 0 {
        f[8].set_d(f[2].d() * f[12].d());
        f[18].set_d(f[8].d() * f[14].d());
        f[16].set_d(f[18].d() + f[2].d());
        f[0].set_fl(fpu::cvt_s_d(f[16].d(), fpu::NEAREST));
        f[0].set_fl(-f[0].fl());
    } else {
        f[4].set_d(f[2].d() * f[12].d());
        f[6].set_d(f[4].d() * f[14].d());
        f[10].set_d(f[6].d() + f[2].d());
        f[0].set_fl(fpu::cvt_s_d(f[10].d(), fpu::NEAREST));
    }
}

/// `func_8008A750(x)` = `cosf`: for `xpt = (bits(x) >> 22) & 0x1FF < 310`:
/// `dx = |x|` (as `0 < x ? x : -x`, so -0.0 for 0), `dn = dx * rpi + 0.5`,
/// `n = round(dn)` (`dn +/- 0.5`, truncated), `dx -= (n - 0.5) * pihi`, `dx
/// -= (n - 0.5) * pilo`, then `dx + (dx * xsq) * poly(xsq)` like
/// [`func_8008A8C0`], negated if `n` is odd. Otherwise the word at
/// `0x800AE110` (a quiet NaN) for NaN, else the word at `0x800ADE10`
/// (zero).
///
/// Its own copy of the constants: `P1..P4` at `0x800ADDD8..0x800ADDF0`,
/// `rpi`, `pihi`, `pilo` at `0x800ADDF8..0x800ADE08`. Domain: any `x`.
/// Leaves `v0` = the bits (or `n`), `t6`/`t7` = `xpt`'s steps, `t0` = `n &
/// 1`, `at`, `v1` = the table, and the double temporaries in `f2`..`f18`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008A750(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0, u64::from(ctx.fpr[12].u32l()));
    g[V0] = lw(m, g[SP], 0);
    ctx.fpr[6].set_u32l(lw(m, g[SP], 0) as u32);
    ctx.fpr[10].set_u32l(lw(m, g[SP], 0) as u32);
    g[T6] = sra(g[V0], 22);
    g[T7] = g[T6] & 0x1FF;
    g[AT] = slt(g[T7], 0x136);
    if g[AT] == 0 {
        // NaN or huge: a constant. (`c.eq.s x, x` isn't NaN-checked.)
        let nan = ctx.fpr[10].fl() != ctx.fpr[10].fl();
        g[AT] = li(0x800B_0000);
        let at = if nan { -0x1EF0 } else { -0x21F0 };
        ctx.fpr[0].set_u32l(lw(m, g[AT], at) as u32);
        return;
    }
    let f = &mut ctx.fpr;
    f[4].set_u32l(0);
    g[AT] = li(0x3FE0_0000); // 0.5, high word
    f[18].set_u32h(g[AT] as u32); // f19
    let positive = f[4].fl() < f[6].fl();
    f[0].set_u32l(lw(m, g[SP], 0) as u32);
    g[AT] = li(0x800B_0000);
    if positive {
        f[0].set_u32l(f[6].u32l());
    } else {
        f[0].set_fl(-f[0].fl());
    }
    f[8].u64 = ld(m, g[AT], -0x2208); // rpi
    f[12].set_d(f64::from(f[0].fl())); // dx = |x|
    f[18].set_u32l(0); // f18 = 0.5
    f[10].set_d(f[12].d() * f[8].d());
    f[4].set_u32h(0); // f5
    f[4].set_u32l(0);
    f[14].set_d(f[10].d() + f[18].d()); // dn
    if f[4].d() <= f[14].d() {
        f[6].set_d(f[14].d() + f[18].d());
        f[8].set_u32l(fpu::trunc_w_d(f[6].d()));
        g[V0] = s32(f[8].u32l());
    } else {
        f[10].set_d(f[14].d() - f[18].d());
        f[4].set_u32l(fpu::trunc_w_d(f[10].d()));
        g[V0] = s32(f[4].u32l());
    }
    f[6].set_u32l(g[V0] as u32);
    g[AT] = li(0x800B_0000);
    f[10].u64 = ld(m, g[AT], -0x2200); // pihi
    f[8].set_d(f64::from(f[6].u32l() as i32));
    f[6].u64 = ld(m, g[AT], -0x21F8); // pilo
    g[V1] = li(0x800A_DDD0);
    f[0].set_d(f[8].d() - f[18].d()); // n - 0.5
    g[T0] = g[V0] & 1;
    f[4].set_d(f[0].d() * f[10].d());
    f[10].u64 = ld(m, g[V1], 0x20);
    f[8].set_d(f[0].d() * f[6].d());
    f[6].u64 = ld(m, g[V1], 0x18);
    f[2].set_d(f[12].d() - f[4].d());
    f[2].set_d(f[2].d() - f[8].d());
    f[14].set_d(f[2].d() * f[2].d()); // xsq
    f[4].set_d(f[10].d() * f[14].d());
    f[8].set_d(f[4].d() + f[6].d());
    f[4].u64 = ld(m, g[V1], 0x10);
    f[10].set_d(f[8].d() * f[14].d());
    f[6].set_d(f[10].d() + f[4].d());
    f[10].u64 = ld(m, g[V1], 8);
    f[8].set_d(f[6].d() * f[14].d());
    f[16].set_d(f[10].d() + f[8].d()); // poly
    if g[T0] != 0 {
        f[8].set_d(f[2].d() * f[14].d());
        f[4].set_d(f[8].d() * f[16].d());
        f[6].set_d(f[4].d() + f[2].d());
        f[0].set_fl(fpu::cvt_s_d(f[6].d(), fpu::NEAREST));
        f[0].set_fl(-f[0].fl());
    } else {
        f[4].set_d(f[2].d() * f[14].d());
        f[6].set_d(f[4].d() * f[16].d());
        f[10].set_d(f[6].d() + f[2].d());
        f[0].set_fl(fpu::cvt_s_d(f[10].d(), fpu::NEAREST));
    }
}
