//! Vector maths (single precision), register-exact ports.
//!
//! Vectors are consecutive `f32`s in RDRAM. Float arithmetic follows
//! N64Recomp's C: IEEE single precision on the host, round to nearest, with
//! subnormals (the hardware flushes them: FCR31.FS is set at boot). NaN
//! operands are outside every port's domain here: the oracle asserts on them
//! (`NAN_CHECK`), and the hardware would trap (FCR31.EV). NOTES.md, "Floats".

// Ports keep N64Recomp's names (func_8001514C), capitals included.
#![allow(non_snake_case)]

use crate::recomp::{addu, enter, fpu, ld, li, lw, reg::*, sd, sll, sw, RecompContext};

/// `func_80001D34(plane, ray, out)` (ray_plane, **guess** at the name):
/// where the ray `o + t v` meets the plane `n . x = d`, returning `t` in
/// `f0`, or -1.0 for no hit. `plane` is `n` then `d` (4 floats); `ray` is
/// `o`, `v` and a length `L` (7 floats).
///
/// With `no = n2*o2 + (o0*n0 + o1*n1)` and `nv = n2*v2 + (v0*n0 + v1*n1)`
/// (sums in that order): -1.0 if `K1 <= nv <= K2` in double (the doubles
/// at `0x800A8128`/`0x800A8130`, -1e-4 and 1e-4 in the ROM). Otherwise `t =
/// (d - no) / nv`; -1.0 if `t < 0` or `L < t`. Otherwise `out = v * t`,
/// then `out += o`, component by component (stored, then read back and
/// added), and it returns `t`.
///
/// Saves `f20` (both halves) at `sp - 8` and restores it. Leaves `f16 =
/// nv`, `f20 = f64(nv)` until the restore, `at = 0xBF800000` past the
/// first compare, and `f2`..`f18` as the path left them (`f2 = t` from the
/// division on).
///
/// Domain: canonical pointers (16, 28 and 12 bytes); `n`, `o`, `v`, `d` and
/// every intermediate reaching another operation not NaN (`nv` included:
/// `cvt.d.s` is guarded); on the hit path `t` and each `v_i * t` not NaN.
/// The constants and `L` are only compared (any value). `out` must not
/// overlap `ray` for the statement above (it is re-read after the stores).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80001D34(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[SP] = addu(g[SP], (-0x10i64) as u64);
    sd(m, g[SP], 8, f[20].u64);
    // no = n . o and nv = n . v, interleaved.
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[12].set_u32l(lw(m, g[A0], 0) as u32);
    f[8].set_u32l(lw(m, g[A1], 4) as u32);
    f[14].set_u32l(lw(m, g[A0], 4) as u32);
    f[6].set_fl(f[4].fl() * f[12].fl());
    f[2].set_u32l(lw(m, g[A0], 8) as u32);
    f[18].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[10].set_fl(f[8].fl() * f[14].fl());
    f[8].set_u32l(lw(m, g[A1], 8) as u32);
    g[AT] = li(0x800B_0000);
    f[4].set_fl(f[6].fl() + f[10].fl());
    f[6].set_fl(f[2].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[10].set_fl(f[18].fl() * f[12].fl());
    f[0].set_fl(f[6].fl() + f[4].fl());
    f[6].set_fl(f[8].fl() * f[14].fl());
    f[8].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[4].set_fl(f[10].fl() + f[6].fl());
    f[10].set_fl(f[2].fl() * f[8].fl());
    f[6].u64 = ld(m, g[AT], -0x7ED8);
    g[AT] = li(0x800B_0000);
    f[16].set_fl(f[10].fl() + f[4].fl());
    f[20].set_d(f64::from(f[16].fl()));
    'done: {
        if f[6].d() <= f[20].d() {
            f[8].u64 = ld(m, g[AT], -0x7ED0);
            g[AT] = li(0xBF80_0000);
            if f[20].d() <= f[8].d() {
                // Parallel to the plane.
                f[0].set_u32l(g[AT] as u32);
                break 'done;
            }
        }
        f[10].set_u32l(lw(m, g[A0], 0xC) as u32);
        f[6].set_u32l(0);
        g[AT] = li(0xBF80_0000);
        f[4].set_fl(f[10].fl() - f[0].fl());
        f[2].set_fl(f[4].fl() / f[16].fl());
        if f[2].fl() < f[6].fl() {
            f[0].set_u32l(g[AT] as u32);
            break 'done;
        }
        f[8].set_u32l(lw(m, g[A1], 0x18) as u32);
        g[AT] = li(0xBF80_0000);
        if f[8].fl() < f[2].fl() {
            f[0].set_u32l(g[AT] as u32);
            break 'done;
        }
        // out = v * t, then out += o.
        f[10].set_fl(f[18].fl() * f[2].fl());
        f[0].set_u32l(f[2].u32l());
        sw(m, g[A2], 0, u64::from(f[10].u32l()));
        f[4].set_u32l(lw(m, g[A1], 0x10) as u32);
        f[6].set_fl(f[4].fl() * f[2].fl());
        f[4].set_u32l(lw(m, g[A2], 0) as u32);
        sw(m, g[A2], 4, u64::from(f[6].u32l()));
        f[8].set_u32l(lw(m, g[A1], 0x14) as u32);
        f[10].set_fl(f[8].fl() * f[2].fl());
        sw(m, g[A2], 8, u64::from(f[10].u32l()));
        f[6].set_u32l(lw(m, g[A1], 0) as u32);
        f[10].set_u32l(lw(m, g[A2], 4) as u32);
        f[8].set_fl(f[4].fl() + f[6].fl());
        sw(m, g[A2], 0, u64::from(f[8].u32l()));
        f[4].set_u32l(lw(m, g[A1], 4) as u32);
        f[8].set_u32l(lw(m, g[A2], 8) as u32);
        f[6].set_fl(f[10].fl() + f[4].fl());
        sw(m, g[A2], 4, u64::from(f[6].u32l()));
        f[10].set_u32l(lw(m, g[A1], 8) as u32);
        f[4].set_fl(f[8].fl() + f[10].fl());
        sw(m, g[A2], 8, u64::from(f[4].u32l()));
    }
    f[20].u64 = ld(m, g[SP], 8);
    g[SP] = addu(g[SP], 0x10);
}

/// `func_80014D4C(x)` (arcsine in degrees, **guess** at the name): `asin(x)`
/// in degrees in `f0`, by the game's own series. With the floats `C0..C5`
/// at `0x800A8790` (0.999999, -0.999999, 0.7071068, -0.7071068, 0.001,
/// -0.001 in the ROM), `P1..P4` at `0x800A87A8` (1/6, 3/40, 15/336,
/// 0.047446) and the double `D` at `0x800A87B8` (pi):
///
/// - `C0 < x`: 90.0; `x < C1`: -90.0.
/// - Unless `C3 < x < C2`, it spills `x` to `[sp - 8]` and works on `y =
///   sqrt(1 - x*x)` (negated for `x < 0`) instead, flagging `v0 = 1`;
///   otherwise `y = x`, `v0 = 0`.
/// - `r = y` if `C5 < y < C4`, else `r = (((y^3*P1 + y) + y^5*P2) + y^7*P3)
///   + y^9*P4` with `y^3 = y*y^2`, `y^5 = y^3*y^2`, `y^7 = y^5*y^2`, `y^9 =
///   y^7*y^2` (all f32, not fused).
/// - `d = f32(f64(r * 180.0) / D)` (the multiply in f32, the division in
///   double, rounded to nearest).
/// - With `v0 = 1`: `-90 - d` if the spilled `x < 0`, else `90 - d`.
///
/// Leaves `at` = the last constant's upper half, and the FPRs of the path
/// (`f4`..`f18`, `f2` = the result before the move to `f0`).
///
/// Domain: `x` not NaN (every non-NaN `x` is fine with the ROM's
/// constants: the square root's operand stays positive).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80014D4C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x800B_0000);
    f[4].set_u32l(lw(m, g[AT], -0x7870) as u32);
    g[SP] = addu(g[SP], (-8i64) as u64);
    'done: {
        if f[4].fl() < f[12].fl() {
            g[AT] = li(0x42B4_0000);
            f[0].set_u32l(g[AT] as u32);
            break 'done;
        }
        f[6].set_u32l(lw(m, g[AT], -0x786C) as u32);
        if f[12].fl() < f[6].fl() {
            g[AT] = li(0xC2B4_0000);
            f[0].set_u32l(g[AT] as u32);
            break 'done;
        }
        'reduced: {
            f[8].set_u32l(lw(m, g[AT], -0x7868) as u32);
            if f[12].fl() < f[8].fl() {
                f[10].set_u32l(lw(m, g[AT], -0x7864) as u32);
                if f[10].fl() < f[12].fl() {
                    // Near 0: the series on x itself.
                    g[V0] = 0;
                    break 'reduced;
                }
            }
            // Near +-1: y = +-sqrt(1 - x*x).
            f[4].set_u32l(0);
            g[V0] = 1;
            sw(m, g[SP], 0, u64::from(f[12].u32l()));
            g[AT] = li(0x3F80_0000);
            if f[12].fl() < f[4].fl() {
                f[8].set_fl(f[12].fl() * f[12].fl());
                f[6].set_u32l(g[AT] as u32);
                f[0].set_fl(f[6].fl() - f[8].fl());
                f[0].set_fl(f[0].fl().sqrt());
                f[12].set_fl(-f[0].fl());
            } else {
                f[4].set_fl(f[12].fl() * f[12].fl());
                f[10].set_u32l(g[AT] as u32);
                f[0].set_fl(f[10].fl() - f[4].fl());
                f[12].set_fl(f[0].fl().sqrt());
            }
        }
        g[AT] = li(0x800B_0000);
        'series: {
            f[6].set_u32l(lw(m, g[AT], -0x7860) as u32);
            if f[12].fl() < f[6].fl() {
                f[8].set_u32l(lw(m, g[AT], -0x785C) as u32);
                if f[8].fl() < f[12].fl() {
                    f[2].set_u32l(f[12].u32l());
                    break 'series;
                }
            }
            f[0].set_fl(f[12].fl() * f[12].fl());
            f[10].set_u32l(lw(m, g[AT], -0x7858) as u32);
            f[8].set_u32l(lw(m, g[AT], -0x7854) as u32);
            f[14].set_fl(f[12].fl() * f[0].fl());
            f[16].set_fl(f[14].fl() * f[0].fl());
            f[18].set_fl(f[16].fl() * f[0].fl());
            f[4].set_fl(f[14].fl() * f[10].fl());
            f[6].set_fl(f[4].fl() + f[12].fl());
            f[10].set_fl(f[16].fl() * f[8].fl());
            f[8].set_u32l(lw(m, g[AT], -0x7850) as u32);
            f[4].set_fl(f[6].fl() + f[10].fl());
            f[6].set_fl(f[18].fl() * f[8].fl());
            f[10].set_fl(f[4].fl() + f[6].fl());
            f[8].set_fl(f[18].fl() * f[0].fl());
            f[4].set_u32l(lw(m, g[AT], -0x784C) as u32);
            f[6].set_fl(f[8].fl() * f[4].fl());
            f[2].set_fl(f[10].fl() + f[6].fl());
        }
        // Degrees: r * 180 / pi, the division in double.
        g[AT] = li(0x4334_0000);
        f[8].set_u32l(g[AT] as u32);
        g[AT] = li(0x800B_0000);
        f[6].u64 = ld(m, g[AT], -0x7848);
        f[4].set_fl(f[2].fl() * f[8].fl());
        f[10].set_d(f64::from(f[4].fl()));
        f[4].set_u32l(lw(m, g[SP], 0) as u32);
        f[8].set_d(f[10].d() / f[6].d());
        f[2].set_fl(fpu::cvt_s_d(f[8].d(), fpu::NEAREST));
        if g[V0] != 0 {
            f[10].set_u32l(0);
            g[AT] = li(0x42B4_0000);
            if f[4].fl() < f[10].fl() {
                g[AT] = li(0xC2B4_0000);
                f[6].set_u32l(g[AT] as u32);
                f[2].set_fl(f[6].fl() - f[2].fl());
            } else {
                f[8].set_u32l(g[AT] as u32);
                f[2].set_fl(f[8].fl() - f[2].fl());
            }
        }
        f[0].set_u32l(f[2].u32l());
    }
    g[SP] = addu(g[SP], 8);
}

/// `func_80014F54(y, x)` (atan2 in degrees, **guess** at the name) with
/// the floats in `f12`/`f14`: the angle of `(x, y)` in degrees in `f0`, by
/// the game's own series. With the floats `K0..K3` at `0x800A87C0` (1e-4,
/// then -1e-4 three times in the ROM), `Q1..Q4` at `0x800A87D0` (1/3, 1/5,
/// 1/7, 0.063235), the double `D` at `0x800A87E0` (pi) and `K8`, `K9` at
/// `0x800A87E8` (-1e-4 twice):
///
/// - `K1 <= x < K0`: `a = 90`. Else `K2 <= y < K0`: `a = 0`.
/// - Else with `ay = |y|`, `ax = |x|` (negated when `< 0`): `t = ay / ax`,
///   or `t = ax / ay` with `v0 = 1` if `ax < ay`. `y` and `x` are spilled
///   to `[sp]`/`[sp + 4]` (the caller's argument slots). `a = 0` if `K3 <= t
///   < K0`, else `a = f32(f64(p * 180.0) / D)` with `p = (((t - t^3*Q1) +
///   t^5*Q2) - t^7*Q3) + t^9*Q4` (powers by repeated `* t^2`, f32, not
///   fused; `y`, `x` reloaded from the slots). Then `a = 90 - a` if `v0`.
/// - Finally `a = 180 - a` if `x < K8`, then `a = -a` if `y < K9`.
///
/// Leaves `v0` as above (0 on the early paths), `f18` = `a` (also in `f0`),
/// `at` = the last constant's upper half, and the path's FPRs.
///
/// Domain: `x`, `y` not NaN, and not both infinite (`t` would be NaN and
/// reach the series).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80014F54(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    'quadrant: {
        g[AT] = li(0x800B_0000);
        f[18].set_u32l(lw(m, g[AT], -0x7840) as u32);
        // `y < K0`, computed in one of two delay slots.
        let y_small;
        if f[14].fl() < f[18].fl() {
            f[4].set_u32l(lw(m, g[AT], -0x783C) as u32);
            g[AT] = li(0x42B4_0000);
            if f[4].fl() <= f[14].fl() {
                f[18].set_u32l(g[AT] as u32);
                break 'quadrant;
            }
        }
        y_small = f[12].fl() < f[18].fl();
        g[AT] = li(0x800B_0000);
        if y_small {
            f[6].set_u32l(lw(m, g[AT], -0x7838) as u32);
            if f[6].fl() <= f[12].fl() {
                f[18].set_u32l(0);
                break 'quadrant;
            }
        }
        f[2].set_u32l(0);
        g[V0] = 0;
        if f[12].fl() < f[2].fl() {
            f[0].set_fl(-f[12].fl());
        } else {
            f[0].set_u32l(f[12].u32l());
        }
        if f[14].fl() < f[2].fl() {
            f[16].set_fl(-f[14].fl());
        } else {
            f[16].set_u32l(f[14].u32l());
        }
        if f[16].fl() < f[0].fl() {
            g[V0] = 1;
            f[2].set_fl(f[16].fl() / f[0].fl());
        } else {
            f[2].set_fl(f[0].fl() / f[16].fl());
        }
        'series: {
            let t_small = f[2].fl() < f[18].fl();
            sw(m, g[SP], 0, u64::from(f[12].u32l()));
            sw(m, g[SP], 4, u64::from(f[14].u32l()));
            if t_small {
                f[8].set_u32l(lw(m, g[AT], -0x7834) as u32);
                sw(m, g[SP], 0, u64::from(f[12].u32l()));
                sw(m, g[SP], 4, u64::from(f[14].u32l()));
                if f[8].fl() <= f[2].fl() {
                    f[18].set_u32l(0);
                    break 'series;
                }
            }
            f[0].set_fl(f[2].fl() * f[2].fl());
            f[10].set_u32l(lw(m, g[AT], -0x7830) as u32);
            f[8].set_u32l(lw(m, g[AT], -0x782C) as u32);
            f[12].set_fl(f[2].fl() * f[0].fl());
            f[14].set_fl(f[12].fl() * f[0].fl());
            f[16].set_fl(f[14].fl() * f[0].fl());
            f[4].set_fl(f[12].fl() * f[10].fl());
            f[12].set_u32l(lw(m, g[SP], 0) as u32);
            f[10].set_fl(f[14].fl() * f[8].fl());
            f[8].set_u32l(lw(m, g[AT], -0x7828) as u32);
            f[14].set_u32l(lw(m, g[SP], 4) as u32);
            f[6].set_fl(f[2].fl() - f[4].fl());
            f[4].set_fl(f[6].fl() + f[10].fl());
            f[6].set_fl(f[16].fl() * f[8].fl());
            f[10].set_fl(f[4].fl() - f[6].fl());
            f[8].set_fl(f[16].fl() * f[0].fl());
            f[4].set_u32l(lw(m, g[AT], -0x7824) as u32);
            g[AT] = li(0x4334_0000);
            f[6].set_fl(f[8].fl() * f[4].fl());
            f[4].set_u32l(g[AT] as u32);
            g[AT] = li(0x800B_0000);
            f[8].set_fl(f[10].fl() + f[6].fl());
            f[10].set_fl(f[8].fl() * f[4].fl());
            f[8].u64 = ld(m, g[AT], -0x7820);
            f[6].set_d(f64::from(f[10].fl()));
            f[4].set_d(f[6].d() / f[8].d());
            f[18].set_fl(fpu::cvt_s_d(f[4].d(), fpu::NEAREST));
        }
        g[AT] = li(0x42B4_0000);
        if g[V0] != 0 {
            f[10].set_u32l(g[AT] as u32);
            f[18].set_fl(f[10].fl() - f[18].fl());
        }
    }
    g[AT] = li(0x800B_0000);
    f[6].set_u32l(lw(m, g[AT], -0x7818) as u32);
    g[AT] = li(0x4334_0000);
    if f[14].fl() < f[6].fl() {
        f[8].set_u32l(g[AT] as u32);
        f[18].set_fl(f[8].fl() - f[18].fl());
    }
    g[AT] = li(0x800B_0000);
    f[4].set_u32l(lw(m, g[AT], -0x7814) as u32);
    if f[12].fl() < f[4].fl() {
        f[18].set_fl(-f[18].fl());
    }
    f[0].set_u32l(f[18].u32l());
}

/// `func_8001514C(out, a, b)` (vec2_add): `out = b + a`, two floats. Each
/// component is loaded, added and stored before the next, so `out` may alias
/// `a` or `b`. (It adds `b`'s component to `a`'s in that operand order,
/// which only matters for NaN payloads.)
///
/// Leaves `f4`, `f6`, `f8` (x) and `f16`, `f10`, `f18` (y): `b`'s and `a`'s
/// components and the sums.
///
/// Domain: canonical `out`, `a`, `b` with 8 bytes each in RDRAM; no NaN
/// components.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8001514C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A2], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[8].set_fl(f[4].fl() + f[6].fl());
    sw(m, g[A0], 0, u64::from(f[8].u32l()));
    f[16].set_u32l(lw(m, g[A1], 4) as u32);
    f[10].set_u32l(lw(m, g[A2], 4) as u32);
    f[18].set_fl(f[10].fl() + f[16].fl());
    sw(m, g[A0], 4, u64::from(f[18].u32l()));
}

/// `func_80015170(out, s, v)` (vec2_scale): `out = v * s`, with the float
/// `s` passed in `a1` (the o32 convention for a float after a pointer).
/// Component by component, so `out` may alias `v`.
///
/// Leaves `f12 = s`, `f4`/`f6` (x: `v`'s component and the product) and
/// `f8`/`f10` (y).
///
/// Domain: canonical `out`, `v` with 8 bytes each in RDRAM; `s` and the
/// components not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015170(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A1] as u32);
    f[4].set_u32l(lw(m, g[A2], 0) as u32);
    f[6].set_fl(f[4].fl() * f[12].fl());
    sw(m, g[A0], 0, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A2], 4) as u32);
    f[10].set_fl(f[8].fl() * f[12].fl());
    sw(m, g[A0], 4, u64::from(f[10].u32l()));
}

/// `func_80015190(out, p, s, q)` (vec2 multiply-add): `out = q * s + p`
/// with the float `s` in `a2`, component by component (`(q.x * s) + p.x`,
/// not fused), each stored before the next is loaded, so `out` may alias
/// `p` or `q`.
///
/// Leaves `f12 = s`, `f4`/`f16` = `q`'s components, `f8`/`f4` = `p`'s
/// (the second overwrites the first), `f6`/`f18` the products, `f10`/`f6`
/// the sums.
///
/// Domain: canonical pointers to 8 bytes; `s`, the components and the
/// products not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015190(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A2] as u32);
    f[4].set_u32l(lw(m, g[A3], 0) as u32);
    f[8].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_fl(f[4].fl() * f[12].fl());
    f[10].set_fl(f[6].fl() + f[8].fl());
    sw(m, g[A0], 0, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A3], 4) as u32);
    f[4].set_u32l(lw(m, g[A1], 4) as u32);
    f[18].set_fl(f[16].fl() * f[12].fl());
    f[6].set_fl(f[18].fl() + f[4].fl());
    sw(m, g[A0], 4, u64::from(f[6].u32l()));
}

/// `func_800151C0(v)` (vec2_length): returns `sqrt(y*y + x*x)` in `f0`, the
/// squares added in that order. `sqrt.s` is IEEE (correctly rounded), which
/// Rust's `f32::sqrt` also is; no game table is involved.
///
/// Leaves `f2 = y`, `f12 = x`, `f4 = y*y`, `f6 = x*x`.
///
/// Domain: canonical `v` with 8 bytes in RDRAM; no NaN components.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800151C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &ctx.gpr;
    let f = &mut ctx.fpr;
    f[2].set_u32l(lw(&mem, g[A0], 4) as u32);
    f[12].set_u32l(lw(&mem, g[A0], 0) as u32);
    f[4].set_fl(f[2].fl() * f[2].fl());
    f[6].set_fl(f[12].fl() * f[12].fl());
    f[0].set_fl(f[4].fl() + f[6].fl());
    f[0].set_fl(f[0].fl().sqrt());
}

/// `func_8001523C(a, b)` (vec2_dist_sq): returns `(a.x - b.x)^2 + (a.y -
/// b.y)^2` in `f0`.
///
/// Leaves `f4`, `f6`, `f8`, `f10` (the components), `f2`/`f12` (the
/// differences) and `f16`/`f18` (their squares).
///
/// Domain: canonical `a`, `b` with 8 bytes each in RDRAM; no NaN components.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8001523C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(&mem, g[A0], 0) as u32);
    f[6].set_u32l(lw(&mem, g[A1], 0) as u32);
    f[8].set_u32l(lw(&mem, g[A0], 4) as u32);
    f[10].set_u32l(lw(&mem, g[A1], 4) as u32);
    f[2].set_fl(f[4].fl() - f[6].fl());
    f[12].set_fl(f[8].fl() - f[10].fl());
    f[16].set_fl(f[2].fl() * f[2].fl());
    f[18].set_fl(f[12].fl() * f[12].fl());
    f[0].set_fl(f[16].fl() + f[18].fl());
}

/// `func_80015268(out, x, y, z)` (vec3 set): `out = (x, y, z)` with the
/// floats in `a1`, `a2`, `a3`; `z` goes through its spill slot `[sp +
/// 0xC]`. Bits are copied, NaNs included.
///
/// Leaves `f12 = x`, `f14 = y`, `f4 = z`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015268(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A1] as u32);
    f[14].set_u32l(g[A2] as u32);
    sw(m, g[SP], 0xC, g[A3]);
    sw(m, g[A0], 0, u64::from(f[12].u32l()));
    sw(m, g[A0], 4, u64::from(f[14].u32l()));
    f[4].set_u32l(lw(m, g[SP], 0xC) as u32);
    sw(m, g[A0], 8, u64::from(f[4].u32l()));
}

/// `func_80015288(out, v)` (vec3 copy): `out = v`, word by word (each
/// loaded just before its store). Leaves `f4`, `f6`, `f8` = the words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015288(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    sw(m, g[A0], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    sw(m, g[A0], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 8) as u32);
    sw(m, g[A0], 8, u64::from(f[8].u32l()));
}

/// `func_800152A4(out, v)` (vec3 negate): `out = -v`, component by
/// component. `neg.s` flips the sign bit (and is NaN-checked in the C).
///
/// Leaves `f4`/`f8`/`f16` = `v`'s components, `f6`/`f10`/`f18` = the
/// negations.
///
/// Domain: canonical pointers to 12 bytes; no NaN components.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800152A4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_fl(-f[4].fl());
    sw(m, g[A0], 0, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 4) as u32);
    f[10].set_fl(-f[8].fl());
    sw(m, g[A0], 4, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A1], 8) as u32);
    f[18].set_fl(-f[16].fl());
    sw(m, g[A0], 8, u64::from(f[18].u32l()));
}

/// `func_800152CC(a, b)` (vec3 equal): 1 if `a.x == b.x`, `a.y == b.y` and
/// `a.z == b.z` (IEEE equality: `0.0 == -0.0`, NaN never equal), else 0;
/// it stops loading at the first unequal component. Compares only, so any
/// bits are in the domain.
///
/// Leaves `f4`/`f6`, `f8`/`f10`, `f16`/`f18` = the components it loaded.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800152CC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = 0;
    for (off, a, b) in [(0, 4, 6), (4, 8, 10), (8, 16, 18)] {
        f[a].set_u32l(lw(m, g[A0], off) as u32);
        f[b].set_u32l(lw(m, g[A1], off) as u32);
        if f[a].fl() != f[b].fl() {
            return;
        }
    }
    g[V0] = 1;
}

/// `func_80015328(out, a, b)` (vec3 add): `out = b + a`, component by
/// component, each stored before the next is loaded (so `out` may alias
/// either input).
///
/// Leaves `f4`/`f10`/`f4` = `b`'s components, `f6`/`f16`/`f6` = `a`'s, and
/// `f8`/`f18`/`f8` the sums (later ones overwrite earlier ones).
///
/// Domain: canonical pointers to 12 bytes; no NaN components.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015328(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A2], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[8].set_fl(f[4].fl() + f[6].fl());
    sw(m, g[A0], 0, u64::from(f[8].u32l()));
    f[16].set_u32l(lw(m, g[A1], 4) as u32);
    f[10].set_u32l(lw(m, g[A2], 4) as u32);
    f[18].set_fl(f[10].fl() + f[16].fl());
    sw(m, g[A0], 4, u64::from(f[18].u32l()));
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[4].set_u32l(lw(m, g[A2], 8) as u32);
    f[8].set_fl(f[4].fl() + f[6].fl());
    sw(m, g[A0], 8, u64::from(f[8].u32l()));
}

/// `func_8001535C(out, a, b)` (vec3 subtract): `out = a - b`, component by
/// component, each stored before the next is loaded.
///
/// Leaves `f4`/`f10`/`f4` = `a`'s components, `f6`/`f16`/`f6` = `b`'s, and
/// `f8`/`f18`/`f8` the differences.
///
/// Domain: canonical pointers to 12 bytes; no NaN components.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8001535C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[A2], 0) as u32);
    f[8].set_fl(f[4].fl() - f[6].fl());
    sw(m, g[A0], 0, u64::from(f[8].u32l()));
    f[16].set_u32l(lw(m, g[A2], 4) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[18].set_fl(f[10].fl() - f[16].fl());
    sw(m, g[A0], 4, u64::from(f[18].u32l()));
    f[6].set_u32l(lw(m, g[A2], 8) as u32);
    f[4].set_u32l(lw(m, g[A1], 8) as u32);
    f[8].set_fl(f[4].fl() - f[6].fl());
    sw(m, g[A0], 8, u64::from(f[8].u32l()));
}

/// `func_80015390(a, b)` (vec3 dot): `b.z * a.z + (a.x * b.x + a.y * b.y)`
/// in `f0`, summed in that order.
///
/// Leaves `f4`, `f10` = `a.x`, `a.z`; `f6` = `b.z`; `f8`, `f18`, `f16` =
/// the products; `f4` = the partial sum.
///
/// Domain: canonical pointers to 12 bytes; no NaN components, products or
/// partial sum (the result may be NaN).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015390(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A0], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_u32l(lw(m, g[A0], 4) as u32);
    f[16].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[10].set_u32l(lw(m, g[A0], 8) as u32);
    f[16].set_fl(f[6].fl() * f[10].fl());
    f[4].set_fl(f[8].fl() + f[18].fl());
    f[0].set_fl(f[16].fl() + f[4].fl());
}

/// `func_800153C0(v)` (vec3 length): `sqrt(z*z + (x*x + y*y))` in `f0`.
/// `sqrt.s` is IEEE (correctly rounded), as is Rust's `f32::sqrt`.
///
/// Leaves `f12`, `f14`, `f2` = the components, `f4`, `f6`, `f10` = their
/// squares, `f8` = the partial sum.
///
/// Domain: canonical `v` with 12 bytes; no NaN components or partial sum.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800153C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(lw(m, g[A0], 0) as u32);
    f[14].set_u32l(lw(m, g[A0], 4) as u32);
    f[2].set_u32l(lw(m, g[A0], 8) as u32);
    f[4].set_fl(f[12].fl() * f[12].fl());
    f[6].set_fl(f[14].fl() * f[14].fl());
    f[8].set_fl(f[4].fl() + f[6].fl());
    f[10].set_fl(f[2].fl() * f[2].fl());
    f[0].set_fl(f[10].fl() + f[8].fl());
    f[0].set_fl(f[0].fl().sqrt());
}

/// `func_800153EC(a, b)` (vec3 squared distance): with `d = a - b`,
/// `(d.x*d.x + d.y*d.y) + d.z*d.z` in `f0`.
///
/// Leaves `f2`, `f12`, `f14` = `d`; `f4`, `f6`, `f10` = the squares; `f8`
/// = the partial sum; `f16`/`f18` = the z components.
///
/// Domain: canonical pointers to 12 bytes; no NaN components, differences
/// or partial sum.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800153EC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A0], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[8].set_u32l(lw(m, g[A0], 4) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[2].set_fl(f[4].fl() - f[6].fl());
    f[16].set_u32l(lw(m, g[A0], 8) as u32);
    f[18].set_u32l(lw(m, g[A1], 8) as u32);
    f[12].set_fl(f[8].fl() - f[10].fl());
    f[4].set_fl(f[2].fl() * f[2].fl());
    f[14].set_fl(f[16].fl() - f[18].fl());
    f[6].set_fl(f[12].fl() * f[12].fl());
    f[8].set_fl(f[4].fl() + f[6].fl());
    f[10].set_fl(f[14].fl() * f[14].fl());
    f[0].set_fl(f[8].fl() + f[10].fl());
}

/// `func_80015428(a, b)` (vec2 distance): with `d = b - a` spilled to a
/// frame (`sp - 8`), `sqrt(d.y*d.y + d.x*d.x)` in `f0`.
///
/// Leaves `f8`, `f4` = `d.x`, `d.y` (reloaded from the frame), `f6`,
/// `f10` = their squares, and `f16`, `f10`/`f18` from the loads.
///
/// Domain: canonical pointers to 8 bytes; no NaN components, differences
/// or sum of squares' operands.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015428(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[A0], 0) as u32);
    g[SP] = addu(g[SP], (-8i64) as u64);
    f[8].set_fl(f[4].fl() - f[6].fl());
    sw(m, g[SP], 0, u64::from(f[8].u32l()));
    f[16].set_u32l(lw(m, g[A0], 4) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_u32l(lw(m, g[SP], 0) as u32);
    f[18].set_fl(f[10].fl() - f[16].fl());
    sw(m, g[SP], 4, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[SP], 4) as u32);
    g[SP] = addu(g[SP], 8);
    f[6].set_fl(f[4].fl() * f[4].fl());
    f[10].set_fl(f[8].fl() * f[8].fl());
    f[0].set_fl(f[6].fl() + f[10].fl());
    f[0].set_fl(f[0].fl().sqrt());
}

/// `func_80015470(a, b)` (vec3 distance): with `d = b - a` spilled to a
/// frame (`sp - 0x10 + 4..`), `sqrt(d.z*d.z + (d.x*d.x + d.y*d.y))` in
/// `f0`.
///
/// Leaves `f10`, `f18`, `f8` = the components of `d`, `f16`, `f4`, `f10` =
/// their squares, `f6` = the partial sum.
///
/// Domain: canonical pointers to 12 bytes; no NaN components, differences
/// or partial sum.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015470(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[A0], 0) as u32);
    g[SP] = addu(g[SP], (-0x10i64) as u64);
    f[8].set_fl(f[4].fl() - f[6].fl());
    sw(m, g[SP], 4, u64::from(f[8].u32l()));
    f[16].set_u32l(lw(m, g[A0], 4) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[18].set_fl(f[10].fl() - f[16].fl());
    f[10].set_u32l(lw(m, g[SP], 4) as u32);
    sw(m, g[SP], 8, u64::from(f[18].u32l()));
    f[6].set_u32l(lw(m, g[A0], 8) as u32);
    f[4].set_u32l(lw(m, g[A1], 8) as u32);
    f[16].set_fl(f[10].fl() * f[10].fl());
    f[18].set_u32l(lw(m, g[SP], 8) as u32);
    f[8].set_fl(f[4].fl() - f[6].fl());
    f[4].set_fl(f[18].fl() * f[18].fl());
    sw(m, g[SP], 0xC, u64::from(f[8].u32l()));
    f[8].set_u32l(lw(m, g[SP], 0xC) as u32);
    g[SP] = addu(g[SP], 0x10);
    f[10].set_fl(f[8].fl() * f[8].fl());
    f[6].set_fl(f[16].fl() + f[4].fl());
    f[0].set_fl(f[10].fl() + f[6].fl());
    f[0].set_fl(f[0].fl().sqrt());
}

/// `func_80015538(out, a, b)` (vec3 cross): `out = a x b`, i.e. `(a.y*b.z
/// - b.y*a.z, a.z*b.x - b.z*a.x, a.x*b.y - b.x*a.y)` with each operand pair
/// in that order. All three are computed (into a frame at `sp - 0x10`)
/// before `out` is written, so `out` may alias `a` or `b`.
///
/// Leaves `f4` = the first component, `f6`/`f10` = the other two reloaded,
/// and `f8`, `f16`, `f18` from the last products.
///
/// Domain: canonical pointers to 12 bytes; no NaN components or products.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015538(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 4) as u32);
    f[6].set_u32l(lw(m, g[A2], 8) as u32);
    f[10].set_u32l(lw(m, g[A2], 4) as u32);
    f[16].set_u32l(lw(m, g[A1], 8) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    g[SP] = addu(g[SP], (-0x10i64) as u64);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[4].set_fl(f[8].fl() - f[18].fl());
    sw(m, g[SP], 4, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[A2], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[18].set_u32l(lw(m, g[A1], 0) as u32);
    f[8].set_u32l(lw(m, g[A2], 8) as u32);
    f[16].set_fl(f[6].fl() * f[10].fl());
    f[6].set_fl(f[8].fl() * f[18].fl());
    f[10].set_fl(f[16].fl() - f[6].fl());
    sw(m, g[SP], 8, u64::from(f[10].u32l()));
    f[18].set_u32l(lw(m, g[A2], 4) as u32);
    f[8].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[6].set_u32l(lw(m, g[A2], 0) as u32);
    f[16].set_fl(f[8].fl() * f[18].fl());
    f[8].set_fl(f[6].fl() * f[10].fl());
    f[18].set_fl(f[16].fl() - f[8].fl());
    sw(m, g[SP], 0xC, u64::from(f[18].u32l()));
    sw(m, g[A0], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[SP], 8) as u32);
    sw(m, g[A0], 4, u64::from(f[6].u32l()));
    f[10].set_u32l(lw(m, g[SP], 0xC) as u32);
    g[SP] = addu(g[SP], 0x10);
    sw(m, g[A0], 8, u64::from(f[10].u32l()));
}

/// `func_800155C0(out, s, v)` (vec3 scale): `out = v * s`, with the float
/// `s` in `a1`, component by component (so `out` may alias `v`).
///
/// Leaves `f12 = s`, `f4`/`f8`/`f16` = `v`'s components, `f6`/`f10`/`f18`
/// = the products.
///
/// Domain: canonical pointers to 12 bytes; `s` and the components not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800155C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A1] as u32);
    f[4].set_u32l(lw(m, g[A2], 0) as u32);
    f[6].set_fl(f[4].fl() * f[12].fl());
    sw(m, g[A0], 0, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A2], 4) as u32);
    f[10].set_fl(f[8].fl() * f[12].fl());
    sw(m, g[A0], 4, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A2], 8) as u32);
    f[18].set_fl(f[16].fl() * f[12].fl());
    sw(m, g[A0], 8, u64::from(f[18].u32l()));
}

/// `func_800155EC(out, p, s, q)` (vec3 multiply-add): `out = q * s + p`
/// with the float `s` in `a2`, component by component (`(q.x * s) + p.x`,
/// not fused), each stored before the next is loaded.
///
/// Leaves `f12 = s`, and `f4`..`f18` from the three components.
///
/// Domain: canonical pointers to 12 bytes; `s`, the components and the
/// products not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800155EC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A2] as u32);
    f[4].set_u32l(lw(m, g[A3], 0) as u32);
    f[8].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_fl(f[4].fl() * f[12].fl());
    f[10].set_fl(f[6].fl() + f[8].fl());
    sw(m, g[A0], 0, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A3], 4) as u32);
    f[4].set_u32l(lw(m, g[A1], 4) as u32);
    f[18].set_fl(f[16].fl() * f[12].fl());
    f[6].set_fl(f[18].fl() + f[4].fl());
    sw(m, g[A0], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A3], 8) as u32);
    f[16].set_u32l(lw(m, g[A1], 8) as u32);
    f[10].set_fl(f[8].fl() * f[12].fl());
    f[18].set_fl(f[10].fl() + f[16].fl());
    sw(m, g[A0], 8, u64::from(f[18].u32l()));
}

/// `func_80015630(out, u, q, t, p)` (vec3 linear combination): `out = p *
/// t + q * u`, with the floats `u` in `a1` and `t` in `a3`, and `p` the
/// fifth argument (stack, `sp + 0x10`, loaded into `a1`); component by
/// component, products in that order, each stored before the next is
/// loaded.
///
/// Leaves `a1 = p`, `f14 = u`, `f12 = t`, and `f4`..`f18` from the three
/// components.
///
/// Domain: canonical pointers to 12 bytes; `t`, `u`, the components and the
/// products not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015630(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[14].set_u32l(g[A1] as u32);
    g[A1] = lw(m, g[SP], 0x10);
    f[12].set_u32l(g[A3] as u32);
    f[8].set_u32l(lw(m, g[A2], 0) as u32);
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_fl(f[4].fl() * f[12].fl());
    f[10].set_fl(f[8].fl() * f[14].fl());
    f[16].set_fl(f[6].fl() + f[10].fl());
    sw(m, g[A0], 0, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_u32l(lw(m, g[A2], 4) as u32);
    f[4].set_fl(f[18].fl() * f[12].fl());
    f[6].set_fl(f[8].fl() * f[14].fl());
    f[10].set_fl(f[4].fl() + f[6].fl());
    sw(m, g[A0], 4, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A1], 8) as u32);
    f[8].set_u32l(lw(m, g[A2], 8) as u32);
    f[18].set_fl(f[16].fl() * f[12].fl());
    f[4].set_fl(f[8].fl() * f[14].fl());
    f[6].set_fl(f[18].fl() + f[4].fl());
    sw(m, g[A0], 8, u64::from(f[6].u32l()));
}

/// `func_80015694(m, k, v)`: row `k` of a matrix of 16-byte rows gets the
/// float triple `v`: `m + 16k` (32-bit address arithmetic, unbounded) =
/// `v`, word by word. Returns the row's address.
///
/// Leaves `t6 = 16k`, `f4`, `f6`, `f8` = the words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015694(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A2], 0) as u32);
    g[T6] = sll(g[A1], 4);
    g[V0] = addu(g[A0], g[T6]);
    sw(m, g[V0], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A2], 4) as u32);
    sw(m, g[V0], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A2], 8) as u32);
    sw(m, g[V0], 8, u64::from(f[8].u32l()));
}

/// `func_800156B8(m, k, out)`: the reverse of [`func_80015694`]: `out` =
/// the float triple at row `k` (`m + 16k`), word by word. Returns the row's
/// address.
///
/// Leaves `t6 = 16k`, `f4`, `f6`, `f8` = the words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800156B8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = sll(g[A1], 4);
    g[V0] = addu(g[A0], g[T6]);
    f[4].set_u32l(lw(m, g[V0], 0) as u32);
    sw(m, g[A2], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[V0], 4) as u32);
    sw(m, g[A2], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[V0], 8) as u32);
    sw(m, g[A2], 8, u64::from(f[8].u32l()));
}

/// `func_800156DC(out, m)` (4x4 matrix copy): copy the 16 words, a row per
/// iteration, each word loaded just before its store (so overlapping
/// copies propagate).
///
/// Leaves `v0 = a0 = 4`, `v1 = out + 64`, `a2 = m + 64`, `f4`..`f10` = the
/// last row.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800156DC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V1] = g[A0];
    g[A0] = 4;
    g[V0] = 0;
    g[A2] = g[A1];
    loop {
        // L_800156EC
        f[4].set_u32l(lw(m, g[A2], 0) as u32);
        g[V0] = addu(g[V0], 1);
        g[V1] = addu(g[V1], 0x10);
        sw(m, g[V1], -0x10, u64::from(f[4].u32l()));
        f[6].set_u32l(lw(m, g[A2], 4) as u32);
        g[A2] = addu(g[A2], 0x10);
        sw(m, g[V1], -0xC, u64::from(f[6].u32l()));
        f[8].set_u32l(lw(m, g[A2], -8) as u32);
        sw(m, g[V1], -8, u64::from(f[8].u32l()));
        f[10].set_u32l(lw(m, g[A2], -4) as u32);
        sw(m, g[V1], -4, u64::from(f[10].u32l()));
        if g[V0] == g[A0] {
            break;
        }
    }
}
