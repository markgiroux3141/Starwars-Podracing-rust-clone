//! Vector maths (single precision), register-exact ports.
//!
//! Vectors are consecutive `f32`s in RDRAM. Float arithmetic follows
//! N64Recomp's C: IEEE single precision on the host, round to nearest, with
//! subnormals (the hardware flushes them: FCR31.FS is set at boot). NaN
//! operands are outside every port's domain here: the oracle asserts on them
//! (`NAN_CHECK`), and the hardware would trap (FCR31.EV). NOTES.md, "Floats".

// Ports keep N64Recomp's names (func_8001514C), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use crate::recomp::{addu, call, enter, fpu, ld, li, lw, reg::*, sd, sll, slt, sltu, subu, sw, RecompContext};

/// `func_800005B4(p, a, b, c, u, v, w)` (point in triangle, **guess**: an
/// edge-cross test with `u`, `v`, `w` the edges): all vec3 pointers, the
/// last three on the stack. It returns 0 if `u x v` is zero (each component
/// equal to 0.0, so -0.0 counts). Otherwise, with `c1 = (a - p) x u`, `c2 =
/// (b - p) x v`, `c3 = (c - p) x w` ([`func_80015538`] on frame copies),
/// `|x|` the absolute value as `-x` only when `x < 0` (so -0.0 stays), `s(c)
/// = (|c.x| + |c.y|) + |c.z|` and `dom(c)` the axis of `c`'s largest
/// absolute component (if `|x| < |y|`: 2 if `|y| < |z|`, else 1; otherwise
/// 2 if `|x| < |z|`, else 0):
/// - if `K1 < s(c1)` (`K1` the float at `0x800A80F0`), with `i = dom(c1)`:
///   1 if `c1[i] < 0` (compared in double), `c2[i] <= 0` and `c3[i] <= 0`,
///   or if `c1[i]` is not below 0, `0 <= c2[i]` and `0 <= c3[i]`; else 0.
/// - else if `s(c2) < K2` (`0x800A80F4`): 1.
/// - else, with `i = dom(c2)`: `c3[i] <= 0` if `c2[i] < 0` (in double),
///   else `0 <= c3[i]`, as 1 or 0.
///
/// Frame (`sp - 0x78`): `ra` at `+0x14`; `a - p`, `b - p`, `c - p` at
/// `+0x6C`, `+0x60`, `+0x54` (component by component, each stored after
/// its subtraction), `u x v` then `c1` at `+0x48`, `c2` at `+0x3C`, `c3`
/// at `+0x30`. `p`, `a`, `b` are spilled to their home slots `sp + 0..8`.
/// Leaves `a0 = sp - 0x78 + 0x48` on the zero path and `sp - 0x78 + 0x30`
/// otherwise, `a1`, `a2` and the temporaries from the last cross product,
/// `v1` the axis and `t*`/`at` from the tests, and `f0`..`f18` as the path
/// used them (`f16 = 0.0`, `f14` a third component, `f12`/`f2` absolute
/// values, `f8`/`f18` `K1`/`K2`, `f10:f11` the double 0.0).
///
/// Domain: canonical pointers to 12 bytes in RDRAM, not overlapping the
/// frame; no NaN operand of a subtraction, a negation, a sum, a `cvt.d.s`,
/// or [`func_80015538`]'s arithmetic (the compares may see NaN).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800005B4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8000096C: {
        // The three differences to the frame, then u x v.
        g[SP] = addu(g[SP], (-0x78i64) as u64);
        sw(m, g[SP], 0x14, g[RA]);
        sw(m, g[SP], 0x78, g[A0]);
        sw(m, g[SP], 0x7C, g[A1]);
        sw(m, g[SP], 0x80, g[A2]);
        ctx.fpr[6].set_u32l(lw(m, g[A0], 0) as u32);
        ctx.fpr[4].set_u32l(lw(m, g[A1], 0) as u32);
        ctx.fpr[8].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
        sw(m, g[SP], 0x6C, u64::from(ctx.fpr[8].u32l()));
        ctx.fpr[18].set_u32l(lw(m, g[A0], 4) as u32);
        ctx.fpr[10].set_u32l(lw(m, g[A1], 4) as u32);
        ctx.fpr[4].set_fl(ctx.fpr[10].fl() - ctx.fpr[18].fl());
        sw(m, g[SP], 0x70, u64::from(ctx.fpr[4].u32l()));
        ctx.fpr[8].set_u32l(lw(m, g[A0], 8) as u32);
        ctx.fpr[6].set_u32l(lw(m, g[A1], 8) as u32);
        g[A1] = lw(m, g[SP], 0x88);
        ctx.fpr[10].set_fl(ctx.fpr[6].fl() - ctx.fpr[8].fl());
        sw(m, g[SP], 0x74, u64::from(ctx.fpr[10].u32l()));
        ctx.fpr[4].set_u32l(lw(m, g[A0], 0) as u32);
        ctx.fpr[18].set_u32l(lw(m, g[A2], 0) as u32);
        ctx.fpr[6].set_fl(ctx.fpr[18].fl() - ctx.fpr[4].fl());
        sw(m, g[SP], 0x60, u64::from(ctx.fpr[6].u32l()));
        ctx.fpr[10].set_u32l(lw(m, g[A0], 4) as u32);
        ctx.fpr[8].set_u32l(lw(m, g[A2], 4) as u32);
        ctx.fpr[18].set_fl(ctx.fpr[8].fl() - ctx.fpr[10].fl());
        sw(m, g[SP], 0x64, u64::from(ctx.fpr[18].u32l()));
        ctx.fpr[6].set_u32l(lw(m, g[A0], 8) as u32);
        ctx.fpr[4].set_u32l(lw(m, g[A2], 8) as u32);
        g[A2] = lw(m, g[SP], 0x8C);
        ctx.fpr[8].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
        sw(m, g[SP], 0x68, u64::from(ctx.fpr[8].u32l()));
        ctx.fpr[18].set_u32l(lw(m, g[A0], 0) as u32);
        ctx.fpr[10].set_u32l(lw(m, g[A3], 0) as u32);
        ctx.fpr[4].set_fl(ctx.fpr[10].fl() - ctx.fpr[18].fl());
        sw(m, g[SP], 0x54, u64::from(ctx.fpr[4].u32l()));
        ctx.fpr[8].set_u32l(lw(m, g[A0], 4) as u32);
        ctx.fpr[6].set_u32l(lw(m, g[A3], 4) as u32);
        ctx.fpr[10].set_fl(ctx.fpr[6].fl() - ctx.fpr[8].fl());
        sw(m, g[SP], 0x58, u64::from(ctx.fpr[10].u32l()));
        ctx.fpr[4].set_u32l(lw(m, g[A0], 8) as u32);
        ctx.fpr[18].set_u32l(lw(m, g[A3], 8) as u32);
        g[A0] = addu(g[SP], 0x48);
        ctx.fpr[6].set_fl(ctx.fpr[18].fl() - ctx.fpr[4].fl());
        sw(m, g[SP], 0x5C, u64::from(ctx.fpr[6].u32l()));
        call(imports::func_80015538, m, ctx);
        let g = &mut ctx.gpr;
        // u x v == 0: return 0 (each load in the previous compare's slot).
        ctx.fpr[0].set_u32l(lw(m, g[SP], 0x48) as u32);
        ctx.fpr[2].set_u32l(0);
        g[A0] = addu(g[SP], 0x48);
        g[A1] = addu(g[SP], 0x6C);
        let zero_x = ctx.fpr[2].fl() == ctx.fpr[0].fl();
        ctx.fpr[0].set_u32l(lw(m, g[SP], 0x4C) as u32);
        if zero_x {
            let zero_y = ctx.fpr[2].fl() == ctx.fpr[0].fl();
            ctx.fpr[14].set_u32l(lw(m, g[SP], 0x50) as u32);
            if zero_y && ctx.fpr[2].fl() == ctx.fpr[14].fl() {
                g[V0] = 0;
                break 'b_8000096C;
            }
        }
        // c1, c2, c3; then f12, f2, f0 = |c1| and f18 = s(c1).
        g[A2] = lw(m, g[SP], 0x88);
        call(imports::func_80015538, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = addu(g[SP], 0x3C);
        g[A1] = addu(g[SP], 0x60);
        g[A2] = lw(m, g[SP], 0x8C);
        call(imports::func_80015538, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = addu(g[SP], 0x30);
        g[A1] = addu(g[SP], 0x54);
        g[A2] = lw(m, g[SP], 0x90);
        call(imports::func_80015538, m, ctx);
        let g = &mut ctx.gpr;
        ctx.fpr[0].set_u32l(lw(m, g[SP], 0x48) as u32);
        ctx.fpr[16].set_u32l(0);
        g[A0] = addu(g[SP], 0x30);
        ctx.fpr[14].set_u32l(lw(m, g[SP], 0x50) as u32);
        if !(ctx.fpr[0].fl() < ctx.fpr[16].fl()) {
            ctx.fpr[12].set_u32l(ctx.fpr[0].u32l());
        } else {
            ctx.fpr[12].set_fl(-ctx.fpr[0].fl());
        }
        ctx.fpr[0].set_u32l(lw(m, g[SP], 0x4C) as u32);
        if !(ctx.fpr[0].fl() < ctx.fpr[16].fl()) {
            ctx.fpr[2].set_u32l(ctx.fpr[0].u32l());
        } else {
            ctx.fpr[2].set_fl(-ctx.fpr[0].fl());
        }
        let z_neg = ctx.fpr[14].fl() < ctx.fpr[16].fl();
        ctx.fpr[10].set_fl(ctx.fpr[12].fl() + ctx.fpr[2].fl());
        if !z_neg {
            ctx.fpr[0].set_u32l(ctx.fpr[14].u32l());
        } else {
            ctx.fpr[0].set_fl(-ctx.fpr[14].fl());
        }
        ctx.fpr[18].set_fl(ctx.fpr[10].fl() + ctx.fpr[0].fl());
        g[AT] = li(0x800B_0000);
        ctx.fpr[8].set_u32l(lw(m, g[AT], -0x7F10) as u32);
        ctx.fpr[14].set_u32l(lw(m, g[SP], 0x44) as u32);
        if !(ctx.fpr[8].fl() < ctx.fpr[18].fl()) {
            // c1 too small: f12, f2, f0 = |c2| and f8 = s(c2).
            ctx.fpr[0].set_u32l(lw(m, g[SP], 0x3C) as u32);
            if !(ctx.fpr[0].fl() < ctx.fpr[16].fl()) {
                ctx.fpr[12].set_u32l(ctx.fpr[0].u32l());
            } else {
                ctx.fpr[12].set_fl(-ctx.fpr[0].fl());
            }
            ctx.fpr[0].set_u32l(lw(m, g[SP], 0x40) as u32);
            if !(ctx.fpr[0].fl() < ctx.fpr[16].fl()) {
                ctx.fpr[2].set_u32l(ctx.fpr[0].u32l());
            } else {
                ctx.fpr[2].set_fl(-ctx.fpr[0].fl());
            }
            let z_neg = ctx.fpr[14].fl() < ctx.fpr[16].fl();
            g[AT] = li(0x800B_0000);
            ctx.fpr[10].set_fl(ctx.fpr[12].fl() + ctx.fpr[2].fl());
            if !z_neg {
                ctx.fpr[0].set_u32l(ctx.fpr[14].u32l());
            } else {
                ctx.fpr[0].set_fl(-ctx.fpr[14].fl());
            }
            ctx.fpr[8].set_fl(ctx.fpr[10].fl() + ctx.fpr[0].fl());
            ctx.fpr[18].set_u32l(lw(m, g[AT], -0x7F0C) as u32);
            if !(ctx.fpr[8].fl() < ctx.fpr[18].fl()) {
                // v1 = dom(c2); c2[i]'s sign picks c3[i]'s test.
                if !(ctx.fpr[12].fl() < ctx.fpr[2].fl()) {
                    let z = ctx.fpr[12].fl() < ctx.fpr[0].fl();
                    g[V1] = 0;
                    if z {
                        g[V1] = 2;
                    }
                } else if !(ctx.fpr[2].fl() < ctx.fpr[0].fl()) {
                    g[V1] = 1;
                } else {
                    g[V1] = 2;
                }
                g[V0] = sll(g[V1], 2);
                g[T4] = addu(g[SP], g[V0]);
                ctx.fpr[4].set_u32l(lw(m, g[T4], 0x3C) as u32);
                ctx.fpr[10].set_u32h(0); // f11
                ctx.fpr[10].set_u32l(0);
                ctx.fpr[6].set_d(f64::from(ctx.fpr[4].fl()));
                g[T5] = addu(g[A0], g[V0]);
                let neg = ctx.fpr[6].d() < ctx.fpr[10].d();
                g[T6] = addu(g[A0], g[V0]);
                if !neg {
                    ctx.fpr[18].set_u32l(lw(m, g[T6], 0) as u32);
                    g[V0] = u64::from(ctx.fpr[16].fl() <= ctx.fpr[18].fl());
                } else {
                    ctx.fpr[8].set_u32l(lw(m, g[T5], 0) as u32);
                    g[V0] = u64::from(ctx.fpr[8].fl() <= ctx.fpr[16].fl());
                }
            } else {
                g[V0] = 1;
            }
        } else {
            // v1 = dom(c1); c1[i]'s sign picks c2[i]'s and c3[i]'s tests.
            if !(ctx.fpr[12].fl() < ctx.fpr[2].fl()) {
                let z = ctx.fpr[12].fl() < ctx.fpr[0].fl();
                g[V1] = 0;
                if z {
                    g[V1] = 2;
                }
            } else if !(ctx.fpr[2].fl() < ctx.fpr[0].fl()) {
                g[V1] = 1;
            } else {
                g[V1] = 2;
            }
            g[V0] = sll(g[V1], 2);
            g[T9] = addu(g[SP], g[V0]);
            ctx.fpr[4].set_u32l(lw(m, g[T9], 0x48) as u32);
            ctx.fpr[10].set_u32h(0); // f11
            ctx.fpr[10].set_u32l(0);
            ctx.fpr[6].set_d(f64::from(ctx.fpr[4].fl()));
            g[T0] = addu(g[SP], g[V0]);
            let neg = ctx.fpr[6].d() < ctx.fpr[10].d();
            g[T2] = addu(g[SP], g[V0]);
            if !neg {
                ctx.fpr[4].set_u32l(lw(m, g[T2], 0x3C) as u32);
                g[T3] = addu(g[A0], g[V0]);
                if !(ctx.fpr[16].fl() <= ctx.fpr[4].fl()) {
                    g[V0] = 0;
                } else {
                    ctx.fpr[6].set_u32l(lw(m, g[T3], 0) as u32);
                    g[V0] = u64::from(ctx.fpr[16].fl() <= ctx.fpr[6].fl());
                }
            } else {
                ctx.fpr[8].set_u32l(lw(m, g[T0], 0x3C) as u32);
                g[T1] = addu(g[A0], g[V0]);
                if !(ctx.fpr[8].fl() <= ctx.fpr[16].fl()) {
                    g[V0] = 0;
                } else {
                    ctx.fpr[18].set_u32l(lw(m, g[T1], 0) as u32);
                    g[V0] = u64::from(ctx.fpr[18].fl() <= ctx.fpr[16].fl());
                }
            }
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x78);
}

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

/// `func_80015724(out, a, b)` (4x4 matrix product, row-major): `out = a *
/// b`, `out[i][j] = a[i][3] * b[3][j] + ((b[0][j] * a[i][0] + b[1][j] *
/// a[i][1]) + b[2][j] * a[i][2])` (sums in that order, nothing fused).
/// Both inputs are first copied to a 0x80-byte frame (`a` at `sp - 0x40`,
/// `b` at `sp - 0x80`) and the product is computed from the copies, so
/// `out` may alias either input. The compiler starts entry `[0][0]` inside
/// the copy of `b` and rotates its registers entry by entry; the port keeps
/// its instruction order (straight-line, like the C).
///
/// Leaves the frame holding the copies and `f4`..`f18` from the last
/// entries.
///
/// Domain: canonical pointers to 64 bytes; the entries, products and
/// partial sums not NaN (the entries of `out` may be).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015724(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    g[SP] = addu(g[SP], (-0x80i64) as u64);
    sw(m, g[SP], 0x40, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    sw(m, g[SP], 0x44, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 8) as u32);
    sw(m, g[SP], 0x48, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0xC) as u32);
    sw(m, g[SP], 0x4C, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A1], 0x10) as u32);
    sw(m, g[SP], 0x50, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0x14) as u32);
    sw(m, g[SP], 0x54, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x18) as u32);
    sw(m, g[SP], 0x58, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x1C) as u32);
    sw(m, g[SP], 0x5C, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x20) as u32);
    sw(m, g[SP], 0x60, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x24) as u32);
    sw(m, g[SP], 0x64, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A1], 0x28) as u32);
    sw(m, g[SP], 0x68, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0x2C) as u32);
    sw(m, g[SP], 0x6C, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x30) as u32);
    sw(m, g[SP], 0x70, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x34) as u32);
    sw(m, g[SP], 0x74, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x38) as u32);
    sw(m, g[SP], 0x78, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x3C) as u32);
    sw(m, g[SP], 0x7C, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A2], 0) as u32);
    sw(m, g[SP], 0, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A2], 4) as u32);
    sw(m, g[SP], 4, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A2], 8) as u32);
    sw(m, g[SP], 8, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A2], 0xC) as u32);
    sw(m, g[SP], 0xC, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A2], 0x10) as u32);
    sw(m, g[SP], 0x10, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A2], 0x14) as u32);
    sw(m, g[SP], 0x14, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A2], 0x18) as u32);
    sw(m, g[SP], 0x18, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A2], 0x1C) as u32);
    sw(m, g[SP], 0x1C, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A2], 0x20) as u32);
    sw(m, g[SP], 0x20, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A2], 0x24) as u32);
    sw(m, g[SP], 0x24, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A2], 0x28) as u32);
    sw(m, g[SP], 0x28, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A2], 0x2C) as u32);
    f[8].set_u32l(lw(m, g[SP], 0) as u32);
    sw(m, g[SP], 0x2C, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A2], 0x30) as u32);
    f[10].set_u32l(lw(m, g[SP], 0x40) as u32);
    sw(m, g[SP], 0x30, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A2], 0x34) as u32);
    f[16].set_fl(f[8].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[SP], 0x20) as u32);
    sw(m, g[SP], 0x34, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A2], 0x38) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x10) as u32);
    sw(m, g[SP], 0x38, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A2], 0x3C) as u32);
    f[4].set_u32l(lw(m, g[SP], 0x44) as u32);
    sw(m, g[SP], 0x3C, u64::from(f[6].u32l()));
    f[6].set_fl(f[18].fl() * f[4].fl());
    f[18].set_u32l(lw(m, g[SP], 0x48) as u32);
    f[4].set_fl(f[10].fl() * f[18].fl());
    f[10].set_u32l(lw(m, g[SP], 0x30) as u32);
    f[8].set_fl(f[16].fl() + f[6].fl());
    f[6].set_u32l(lw(m, g[SP], 0x4C) as u32);
    f[18].set_fl(f[6].fl() * f[10].fl());
    f[16].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[18].fl() + f[16].fl());
    sw(m, g[A0], 0, u64::from(f[8].u32l()));
    f[4].set_u32l(lw(m, g[SP], 4) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x40) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x14) as u32);
    f[16].set_u32l(lw(m, g[SP], 0x44) as u32);
    f[10].set_fl(f[4].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[SP], 0x24) as u32);
    f[8].set_fl(f[18].fl() * f[16].fl());
    f[18].set_u32l(lw(m, g[SP], 0x48) as u32);
    f[16].set_fl(f[6].fl() * f[18].fl());
    f[6].set_u32l(lw(m, g[SP], 0x34) as u32);
    f[4].set_fl(f[10].fl() + f[8].fl());
    f[8].set_u32l(lw(m, g[SP], 0x4C) as u32);
    f[18].set_fl(f[8].fl() * f[6].fl());
    f[10].set_fl(f[4].fl() + f[16].fl());
    f[4].set_fl(f[18].fl() + f[10].fl());
    sw(m, g[A0], 4, u64::from(f[4].u32l()));
    f[16].set_u32l(lw(m, g[SP], 8) as u32);
    f[8].set_u32l(lw(m, g[SP], 0x40) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x18) as u32);
    f[10].set_u32l(lw(m, g[SP], 0x44) as u32);
    f[6].set_fl(f[16].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[SP], 0x28) as u32);
    f[4].set_fl(f[18].fl() * f[10].fl());
    f[18].set_u32l(lw(m, g[SP], 0x48) as u32);
    f[10].set_fl(f[8].fl() * f[18].fl());
    f[8].set_u32l(lw(m, g[SP], 0x38) as u32);
    f[16].set_fl(f[6].fl() + f[4].fl());
    f[4].set_u32l(lw(m, g[SP], 0x4C) as u32);
    f[18].set_fl(f[4].fl() * f[8].fl());
    f[6].set_fl(f[16].fl() + f[10].fl());
    f[16].set_fl(f[18].fl() + f[6].fl());
    sw(m, g[A0], 8, u64::from(f[16].u32l()));
    f[10].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[4].set_u32l(lw(m, g[SP], 0x40) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x1C) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x44) as u32);
    f[8].set_fl(f[10].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[SP], 0x2C) as u32);
    f[16].set_fl(f[18].fl() * f[6].fl());
    f[18].set_u32l(lw(m, g[SP], 0x48) as u32);
    f[6].set_fl(f[4].fl() * f[18].fl());
    f[4].set_u32l(lw(m, g[SP], 0x3C) as u32);
    f[10].set_fl(f[8].fl() + f[16].fl());
    f[16].set_u32l(lw(m, g[SP], 0x4C) as u32);
    f[18].set_fl(f[16].fl() * f[4].fl());
    f[8].set_fl(f[10].fl() + f[6].fl());
    f[10].set_fl(f[18].fl() + f[8].fl());
    sw(m, g[A0], 0xC, u64::from(f[10].u32l()));
    f[6].set_u32l(lw(m, g[SP], 0) as u32);
    f[16].set_u32l(lw(m, g[SP], 0x50) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x10) as u32);
    f[8].set_u32l(lw(m, g[SP], 0x54) as u32);
    f[4].set_fl(f[6].fl() * f[16].fl());
    f[16].set_u32l(lw(m, g[SP], 0x20) as u32);
    f[10].set_fl(f[18].fl() * f[8].fl());
    f[18].set_u32l(lw(m, g[SP], 0x58) as u32);
    f[8].set_fl(f[16].fl() * f[18].fl());
    f[16].set_u32l(lw(m, g[SP], 0x30) as u32);
    f[6].set_fl(f[4].fl() + f[10].fl());
    f[10].set_u32l(lw(m, g[SP], 0x5C) as u32);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[4].set_fl(f[6].fl() + f[8].fl());
    f[6].set_fl(f[18].fl() + f[4].fl());
    sw(m, g[A0], 0x10, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[SP], 4) as u32);
    f[10].set_u32l(lw(m, g[SP], 0x50) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x14) as u32);
    f[4].set_u32l(lw(m, g[SP], 0x54) as u32);
    f[16].set_fl(f[8].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[SP], 0x24) as u32);
    f[6].set_fl(f[18].fl() * f[4].fl());
    f[18].set_u32l(lw(m, g[SP], 0x58) as u32);
    f[4].set_fl(f[10].fl() * f[18].fl());
    f[10].set_u32l(lw(m, g[SP], 0x34) as u32);
    f[8].set_fl(f[16].fl() + f[6].fl());
    f[6].set_u32l(lw(m, g[SP], 0x5C) as u32);
    f[18].set_fl(f[6].fl() * f[10].fl());
    f[16].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[18].fl() + f[16].fl());
    sw(m, g[A0], 0x14, u64::from(f[8].u32l()));
    f[4].set_u32l(lw(m, g[SP], 8) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x50) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x18) as u32);
    f[16].set_u32l(lw(m, g[SP], 0x54) as u32);
    f[10].set_fl(f[4].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[SP], 0x28) as u32);
    f[8].set_fl(f[18].fl() * f[16].fl());
    f[18].set_u32l(lw(m, g[SP], 0x58) as u32);
    f[16].set_fl(f[6].fl() * f[18].fl());
    f[6].set_u32l(lw(m, g[SP], 0x38) as u32);
    f[4].set_fl(f[10].fl() + f[8].fl());
    f[8].set_u32l(lw(m, g[SP], 0x5C) as u32);
    f[18].set_fl(f[8].fl() * f[6].fl());
    f[10].set_fl(f[4].fl() + f[16].fl());
    f[4].set_fl(f[18].fl() + f[10].fl());
    sw(m, g[A0], 0x18, u64::from(f[4].u32l()));
    f[16].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[8].set_u32l(lw(m, g[SP], 0x50) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x1C) as u32);
    f[10].set_u32l(lw(m, g[SP], 0x54) as u32);
    f[6].set_fl(f[16].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[SP], 0x2C) as u32);
    f[4].set_fl(f[18].fl() * f[10].fl());
    f[18].set_u32l(lw(m, g[SP], 0x58) as u32);
    f[10].set_fl(f[8].fl() * f[18].fl());
    f[8].set_u32l(lw(m, g[SP], 0x3C) as u32);
    f[16].set_fl(f[6].fl() + f[4].fl());
    f[4].set_u32l(lw(m, g[SP], 0x5C) as u32);
    f[18].set_fl(f[4].fl() * f[8].fl());
    f[6].set_fl(f[16].fl() + f[10].fl());
    f[16].set_fl(f[18].fl() + f[6].fl());
    sw(m, g[A0], 0x1C, u64::from(f[16].u32l()));
    f[10].set_u32l(lw(m, g[SP], 0) as u32);
    f[4].set_u32l(lw(m, g[SP], 0x60) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x10) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x64) as u32);
    f[8].set_fl(f[10].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[SP], 0x20) as u32);
    f[16].set_fl(f[18].fl() * f[6].fl());
    f[18].set_u32l(lw(m, g[SP], 0x68) as u32);
    f[6].set_fl(f[4].fl() * f[18].fl());
    f[4].set_u32l(lw(m, g[SP], 0x30) as u32);
    f[10].set_fl(f[8].fl() + f[16].fl());
    f[16].set_u32l(lw(m, g[SP], 0x6C) as u32);
    f[18].set_fl(f[16].fl() * f[4].fl());
    f[8].set_fl(f[10].fl() + f[6].fl());
    f[10].set_fl(f[18].fl() + f[8].fl());
    sw(m, g[A0], 0x20, u64::from(f[10].u32l()));
    f[6].set_u32l(lw(m, g[SP], 4) as u32);
    f[16].set_u32l(lw(m, g[SP], 0x60) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x14) as u32);
    f[8].set_u32l(lw(m, g[SP], 0x64) as u32);
    f[4].set_fl(f[6].fl() * f[16].fl());
    f[16].set_u32l(lw(m, g[SP], 0x24) as u32);
    f[10].set_fl(f[18].fl() * f[8].fl());
    f[18].set_u32l(lw(m, g[SP], 0x68) as u32);
    f[8].set_fl(f[16].fl() * f[18].fl());
    f[16].set_u32l(lw(m, g[SP], 0x34) as u32);
    f[6].set_fl(f[4].fl() + f[10].fl());
    f[10].set_u32l(lw(m, g[SP], 0x6C) as u32);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[4].set_fl(f[6].fl() + f[8].fl());
    f[6].set_fl(f[18].fl() + f[4].fl());
    sw(m, g[A0], 0x24, u64::from(f[6].u32l()));
    f[10].set_u32l(lw(m, g[SP], 0x60) as u32);
    f[8].set_u32l(lw(m, g[SP], 8) as u32);
    f[4].set_u32l(lw(m, g[SP], 0x64) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x18) as u32);
    f[16].set_fl(f[8].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[SP], 0x28) as u32);
    f[6].set_fl(f[18].fl() * f[4].fl());
    f[18].set_u32l(lw(m, g[SP], 0x68) as u32);
    f[4].set_fl(f[10].fl() * f[18].fl());
    f[10].set_u32l(lw(m, g[SP], 0x38) as u32);
    f[8].set_fl(f[16].fl() + f[6].fl());
    f[6].set_u32l(lw(m, g[SP], 0x6C) as u32);
    f[18].set_fl(f[6].fl() * f[10].fl());
    f[16].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[18].fl() + f[16].fl());
    sw(m, g[A0], 0x28, u64::from(f[8].u32l()));
    f[6].set_u32l(lw(m, g[SP], 0x60) as u32);
    f[4].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[16].set_u32l(lw(m, g[SP], 0x64) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x1C) as u32);
    f[10].set_fl(f[4].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[SP], 0x2C) as u32);
    f[8].set_fl(f[18].fl() * f[16].fl());
    f[18].set_u32l(lw(m, g[SP], 0x68) as u32);
    f[16].set_fl(f[6].fl() * f[18].fl());
    f[6].set_u32l(lw(m, g[SP], 0x3C) as u32);
    f[4].set_fl(f[10].fl() + f[8].fl());
    f[8].set_u32l(lw(m, g[SP], 0x6C) as u32);
    f[18].set_fl(f[8].fl() * f[6].fl());
    f[10].set_fl(f[4].fl() + f[16].fl());
    f[4].set_fl(f[18].fl() + f[10].fl());
    sw(m, g[A0], 0x2C, u64::from(f[4].u32l()));
    f[8].set_u32l(lw(m, g[SP], 0x70) as u32);
    f[16].set_u32l(lw(m, g[SP], 0) as u32);
    f[10].set_u32l(lw(m, g[SP], 0x74) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x10) as u32);
    f[6].set_fl(f[16].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[SP], 0x20) as u32);
    f[4].set_fl(f[18].fl() * f[10].fl());
    f[18].set_u32l(lw(m, g[SP], 0x78) as u32);
    f[10].set_fl(f[8].fl() * f[18].fl());
    f[8].set_u32l(lw(m, g[SP], 0x30) as u32);
    f[16].set_fl(f[6].fl() + f[4].fl());
    f[4].set_u32l(lw(m, g[SP], 0x7C) as u32);
    f[18].set_fl(f[4].fl() * f[8].fl());
    f[6].set_fl(f[16].fl() + f[10].fl());
    f[16].set_fl(f[18].fl() + f[6].fl());
    sw(m, g[A0], 0x30, u64::from(f[16].u32l()));
    f[4].set_u32l(lw(m, g[SP], 0x70) as u32);
    f[10].set_u32l(lw(m, g[SP], 4) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x74) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x14) as u32);
    f[8].set_fl(f[10].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[SP], 0x24) as u32);
    f[16].set_fl(f[18].fl() * f[6].fl());
    f[18].set_u32l(lw(m, g[SP], 0x78) as u32);
    f[10].set_fl(f[8].fl() + f[16].fl());
    f[6].set_fl(f[4].fl() * f[18].fl());
    f[4].set_u32l(lw(m, g[SP], 0x34) as u32);
    f[16].set_u32l(lw(m, g[SP], 0x7C) as u32);
    f[18].set_fl(f[16].fl() * f[4].fl());
    f[8].set_fl(f[10].fl() + f[6].fl());
    f[10].set_fl(f[18].fl() + f[8].fl());
    sw(m, g[A0], 0x34, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[SP], 0x70) as u32);
    f[6].set_u32l(lw(m, g[SP], 8) as u32);
    f[8].set_u32l(lw(m, g[SP], 0x74) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x18) as u32);
    f[4].set_fl(f[6].fl() * f[16].fl());
    f[16].set_u32l(lw(m, g[SP], 0x28) as u32);
    f[10].set_fl(f[18].fl() * f[8].fl());
    f[18].set_u32l(lw(m, g[SP], 0x78) as u32);
    f[8].set_fl(f[16].fl() * f[18].fl());
    f[16].set_u32l(lw(m, g[SP], 0x38) as u32);
    f[6].set_fl(f[4].fl() + f[10].fl());
    f[10].set_u32l(lw(m, g[SP], 0x7C) as u32);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[4].set_fl(f[6].fl() + f[8].fl());
    f[6].set_fl(f[18].fl() + f[4].fl());
    sw(m, g[A0], 0x38, u64::from(f[6].u32l()));
    f[10].set_u32l(lw(m, g[SP], 0x70) as u32);
    f[8].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[4].set_u32l(lw(m, g[SP], 0x74) as u32);
    f[18].set_u32l(lw(m, g[SP], 0x1C) as u32);
    f[16].set_fl(f[8].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[SP], 0x2C) as u32);
    f[6].set_fl(f[18].fl() * f[4].fl());
    f[18].set_u32l(lw(m, g[SP], 0x78) as u32);
    f[4].set_fl(f[10].fl() * f[18].fl());
    f[10].set_u32l(lw(m, g[SP], 0x3C) as u32);
    f[8].set_fl(f[16].fl() + f[6].fl());
    f[6].set_u32l(lw(m, g[SP], 0x7C) as u32);
    g[SP] = addu(g[SP], 0x80);
    f[18].set_fl(f[6].fl() * f[10].fl());
    f[16].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[18].fl() + f[16].fl());
    sw(m, g[A0], 0x3C, u64::from(f[8].u32l()));
}

/// `func_80015C30(m, n)` (4x4 matrix product in place): `m = n * m`,
/// `m[i][j] = n[i][3] * m0[3][j] + ((m0[0][j] * n[i][0] + m0[1][j] *
/// n[i][1]) + m0[2][j] * n[i][2])` with `m0` = `m`'s copy in a 0x40-byte
/// frame (`sp - 0x40`, made first). `n` is read in place, so it must not
/// be `m`: it would see the rows already written. Straight-line, in the
/// C's order.
///
/// Leaves the frame holding the copy and `f4`..`f18` from the last
/// entries.
///
/// Domain: canonical pointers to 64 bytes, not overlapping; the entries,
/// products and partial sums not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80015C30(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A0], 0) as u32);
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    sw(m, g[SP], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 4) as u32);
    sw(m, g[SP], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A0], 8) as u32);
    sw(m, g[SP], 8, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0xC) as u32);
    sw(m, g[SP], 0xC, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A0], 0x10) as u32);
    sw(m, g[SP], 0x10, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x14) as u32);
    sw(m, g[SP], 0x14, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A0], 0x18) as u32);
    sw(m, g[SP], 0x18, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0x1C) as u32);
    sw(m, g[SP], 0x1C, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x20) as u32);
    sw(m, g[SP], 0x20, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0x24) as u32);
    sw(m, g[SP], 0x24, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A0], 0x28) as u32);
    sw(m, g[SP], 0x28, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x2C) as u32);
    f[16].set_u32l(lw(m, g[SP], 0) as u32);
    sw(m, g[SP], 0x2C, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A0], 0x30) as u32);
    sw(m, g[SP], 0x30, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0x34) as u32);
    sw(m, g[SP], 0x34, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x38) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x10) as u32);
    sw(m, g[SP], 0x38, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0x3C) as u32);
    sw(m, g[SP], 0x3C, u64::from(f[10].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0) as u32);
    f[8].set_u32l(lw(m, g[A1], 4) as u32);
    f[4].set_fl(f[16].fl() * f[18].fl());
    f[18].set_u32l(lw(m, g[SP], 0x20) as u32);
    f[10].set_fl(f[6].fl() * f[8].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[8].set_fl(f[18].fl() * f[6].fl());
    f[18].set_u32l(lw(m, g[SP], 0x30) as u32);
    f[16].set_fl(f[4].fl() + f[10].fl());
    f[10].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[6].set_fl(f[10].fl() * f[18].fl());
    f[4].set_fl(f[16].fl() + f[8].fl());
    f[16].set_fl(f[6].fl() + f[4].fl());
    sw(m, g[A0], 0, u64::from(f[16].u32l()));
    f[8].set_u32l(lw(m, g[SP], 4) as u32);
    f[10].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x14) as u32);
    f[4].set_u32l(lw(m, g[A1], 4) as u32);
    f[18].set_fl(f[8].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[SP], 0x24) as u32);
    f[16].set_fl(f[6].fl() * f[4].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[4].set_fl(f[10].fl() * f[6].fl());
    f[10].set_u32l(lw(m, g[SP], 0x34) as u32);
    f[8].set_fl(f[18].fl() + f[16].fl());
    f[16].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[6].set_fl(f[16].fl() * f[10].fl());
    f[18].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[6].fl() + f[18].fl());
    sw(m, g[A0], 4, u64::from(f[8].u32l()));
    f[4].set_u32l(lw(m, g[SP], 8) as u32);
    f[16].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x18) as u32);
    f[18].set_u32l(lw(m, g[A1], 4) as u32);
    f[10].set_fl(f[4].fl() * f[16].fl());
    f[16].set_u32l(lw(m, g[SP], 0x28) as u32);
    f[8].set_fl(f[6].fl() * f[18].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[18].set_fl(f[16].fl() * f[6].fl());
    f[16].set_u32l(lw(m, g[SP], 0x38) as u32);
    f[4].set_fl(f[10].fl() + f[8].fl());
    f[8].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[6].set_fl(f[8].fl() * f[16].fl());
    f[10].set_fl(f[4].fl() + f[18].fl());
    f[4].set_fl(f[6].fl() + f[10].fl());
    sw(m, g[A0], 8, u64::from(f[4].u32l()));
    f[18].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[8].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x1C) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[16].set_fl(f[18].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[SP], 0x2C) as u32);
    f[4].set_fl(f[6].fl() * f[10].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[10].set_fl(f[8].fl() * f[6].fl());
    f[8].set_u32l(lw(m, g[SP], 0x3C) as u32);
    f[18].set_fl(f[16].fl() + f[4].fl());
    f[4].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[6].set_fl(f[4].fl() * f[8].fl());
    f[16].set_fl(f[18].fl() + f[10].fl());
    f[18].set_fl(f[6].fl() + f[16].fl());
    sw(m, g[A0], 0xC, u64::from(f[18].u32l()));
    f[10].set_u32l(lw(m, g[SP], 0) as u32);
    f[4].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x10) as u32);
    f[16].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[8].set_fl(f[10].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[SP], 0x20) as u32);
    f[18].set_fl(f[6].fl() * f[16].fl());
    f[6].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[16].set_fl(f[4].fl() * f[6].fl());
    f[4].set_u32l(lw(m, g[SP], 0x30) as u32);
    f[10].set_fl(f[8].fl() + f[18].fl());
    f[18].set_u32l(lw(m, g[A1], 0x1C) as u32);
    f[6].set_fl(f[18].fl() * f[4].fl());
    f[8].set_fl(f[10].fl() + f[16].fl());
    f[10].set_fl(f[6].fl() + f[8].fl());
    sw(m, g[A0], 0x10, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[SP], 4) as u32);
    f[18].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x14) as u32);
    f[8].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[4].set_fl(f[16].fl() * f[18].fl());
    f[18].set_u32l(lw(m, g[SP], 0x24) as u32);
    f[10].set_fl(f[6].fl() * f[8].fl());
    f[6].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[8].set_fl(f[18].fl() * f[6].fl());
    f[18].set_u32l(lw(m, g[SP], 0x34) as u32);
    f[16].set_fl(f[4].fl() + f[10].fl());
    f[10].set_u32l(lw(m, g[A1], 0x1C) as u32);
    f[6].set_fl(f[10].fl() * f[18].fl());
    f[4].set_fl(f[16].fl() + f[8].fl());
    f[16].set_fl(f[6].fl() + f[4].fl());
    sw(m, g[A0], 0x14, u64::from(f[16].u32l()));
    f[8].set_u32l(lw(m, g[SP], 8) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x18) as u32);
    f[4].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[18].set_fl(f[8].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[SP], 0x28) as u32);
    f[16].set_fl(f[6].fl() * f[4].fl());
    f[6].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[4].set_fl(f[10].fl() * f[6].fl());
    f[10].set_u32l(lw(m, g[SP], 0x38) as u32);
    f[8].set_fl(f[18].fl() + f[16].fl());
    f[16].set_u32l(lw(m, g[A1], 0x1C) as u32);
    f[6].set_fl(f[16].fl() * f[10].fl());
    f[18].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[6].fl() + f[18].fl());
    sw(m, g[A0], 0x18, u64::from(f[8].u32l()));
    f[4].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[16].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x1C) as u32);
    f[18].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[10].set_fl(f[4].fl() * f[16].fl());
    f[16].set_u32l(lw(m, g[SP], 0x2C) as u32);
    f[8].set_fl(f[6].fl() * f[18].fl());
    f[6].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[18].set_fl(f[16].fl() * f[6].fl());
    f[16].set_u32l(lw(m, g[SP], 0x3C) as u32);
    f[4].set_fl(f[10].fl() + f[8].fl());
    f[8].set_u32l(lw(m, g[A1], 0x1C) as u32);
    f[6].set_fl(f[8].fl() * f[16].fl());
    f[10].set_fl(f[4].fl() + f[18].fl());
    f[4].set_fl(f[6].fl() + f[10].fl());
    sw(m, g[A0], 0x1C, u64::from(f[4].u32l()));
    f[18].set_u32l(lw(m, g[SP], 0) as u32);
    f[8].set_u32l(lw(m, g[A1], 0x20) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x10) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[16].set_fl(f[18].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[SP], 0x20) as u32);
    f[4].set_fl(f[6].fl() * f[10].fl());
    f[6].set_u32l(lw(m, g[A1], 0x28) as u32);
    f[10].set_fl(f[8].fl() * f[6].fl());
    f[8].set_u32l(lw(m, g[SP], 0x30) as u32);
    f[18].set_fl(f[16].fl() + f[4].fl());
    f[4].set_u32l(lw(m, g[A1], 0x2C) as u32);
    f[6].set_fl(f[4].fl() * f[8].fl());
    f[16].set_fl(f[18].fl() + f[10].fl());
    f[18].set_fl(f[6].fl() + f[16].fl());
    sw(m, g[A0], 0x20, u64::from(f[18].u32l()));
    f[10].set_u32l(lw(m, g[SP], 4) as u32);
    f[4].set_u32l(lw(m, g[A1], 0x20) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x14) as u32);
    f[16].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[8].set_fl(f[10].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[SP], 0x24) as u32);
    f[18].set_fl(f[6].fl() * f[16].fl());
    f[6].set_u32l(lw(m, g[A1], 0x28) as u32);
    f[16].set_fl(f[4].fl() * f[6].fl());
    f[4].set_u32l(lw(m, g[SP], 0x34) as u32);
    f[10].set_fl(f[8].fl() + f[18].fl());
    f[18].set_u32l(lw(m, g[A1], 0x2C) as u32);
    f[6].set_fl(f[18].fl() * f[4].fl());
    f[8].set_fl(f[10].fl() + f[16].fl());
    f[10].set_fl(f[6].fl() + f[8].fl());
    sw(m, g[A0], 0x24, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[SP], 8) as u32);
    f[18].set_u32l(lw(m, g[A1], 0x20) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x18) as u32);
    f[8].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[4].set_fl(f[16].fl() * f[18].fl());
    f[18].set_u32l(lw(m, g[SP], 0x28) as u32);
    f[10].set_fl(f[6].fl() * f[8].fl());
    f[6].set_u32l(lw(m, g[A1], 0x28) as u32);
    f[8].set_fl(f[18].fl() * f[6].fl());
    f[18].set_u32l(lw(m, g[SP], 0x38) as u32);
    f[16].set_fl(f[4].fl() + f[10].fl());
    f[10].set_u32l(lw(m, g[A1], 0x2C) as u32);
    f[6].set_fl(f[10].fl() * f[18].fl());
    f[4].set_fl(f[16].fl() + f[8].fl());
    f[16].set_fl(f[6].fl() + f[4].fl());
    sw(m, g[A0], 0x28, u64::from(f[16].u32l()));
    f[8].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x20) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x1C) as u32);
    f[4].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[18].set_fl(f[8].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[SP], 0x2C) as u32);
    f[16].set_fl(f[6].fl() * f[4].fl());
    f[6].set_u32l(lw(m, g[A1], 0x28) as u32);
    f[4].set_fl(f[10].fl() * f[6].fl());
    f[10].set_u32l(lw(m, g[SP], 0x3C) as u32);
    f[8].set_fl(f[18].fl() + f[16].fl());
    f[16].set_u32l(lw(m, g[A1], 0x2C) as u32);
    f[6].set_fl(f[16].fl() * f[10].fl());
    f[18].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[6].fl() + f[18].fl());
    sw(m, g[A0], 0x2C, u64::from(f[8].u32l()));
    f[16].set_u32l(lw(m, g[A1], 0x30) as u32);
    f[4].set_u32l(lw(m, g[SP], 0) as u32);
    f[18].set_u32l(lw(m, g[A1], 0x34) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x10) as u32);
    f[10].set_fl(f[4].fl() * f[16].fl());
    f[16].set_u32l(lw(m, g[SP], 0x20) as u32);
    f[8].set_fl(f[6].fl() * f[18].fl());
    f[6].set_u32l(lw(m, g[A1], 0x38) as u32);
    f[18].set_fl(f[16].fl() * f[6].fl());
    f[16].set_u32l(lw(m, g[SP], 0x30) as u32);
    f[4].set_fl(f[10].fl() + f[8].fl());
    f[8].set_u32l(lw(m, g[A1], 0x3C) as u32);
    f[6].set_fl(f[8].fl() * f[16].fl());
    f[10].set_fl(f[4].fl() + f[18].fl());
    f[4].set_fl(f[6].fl() + f[10].fl());
    sw(m, g[A0], 0x30, u64::from(f[4].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x30) as u32);
    f[18].set_u32l(lw(m, g[SP], 4) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x34) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x14) as u32);
    f[16].set_fl(f[18].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[SP], 0x24) as u32);
    f[4].set_fl(f[6].fl() * f[10].fl());
    f[6].set_u32l(lw(m, g[A1], 0x38) as u32);
    f[10].set_fl(f[8].fl() * f[6].fl());
    f[8].set_u32l(lw(m, g[SP], 0x34) as u32);
    f[18].set_fl(f[16].fl() + f[4].fl());
    f[4].set_u32l(lw(m, g[A1], 0x3C) as u32);
    f[6].set_fl(f[4].fl() * f[8].fl());
    f[16].set_fl(f[18].fl() + f[10].fl());
    f[18].set_fl(f[6].fl() + f[16].fl());
    sw(m, g[A0], 0x34, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x30) as u32);
    f[10].set_u32l(lw(m, g[SP], 8) as u32);
    f[16].set_u32l(lw(m, g[A1], 0x34) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x18) as u32);
    f[8].set_fl(f[10].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[SP], 0x28) as u32);
    f[18].set_fl(f[6].fl() * f[16].fl());
    f[6].set_u32l(lw(m, g[A1], 0x38) as u32);
    f[16].set_fl(f[4].fl() * f[6].fl());
    f[4].set_u32l(lw(m, g[SP], 0x38) as u32);
    f[10].set_fl(f[8].fl() + f[18].fl());
    f[18].set_u32l(lw(m, g[A1], 0x3C) as u32);
    f[6].set_fl(f[18].fl() * f[4].fl());
    f[8].set_fl(f[10].fl() + f[16].fl());
    f[10].set_fl(f[6].fl() + f[8].fl());
    sw(m, g[A0], 0x38, u64::from(f[10].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0x30) as u32);
    f[16].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[8].set_u32l(lw(m, g[A1], 0x34) as u32);
    f[6].set_u32l(lw(m, g[SP], 0x1C) as u32);
    f[4].set_fl(f[16].fl() * f[18].fl());
    f[18].set_u32l(lw(m, g[SP], 0x2C) as u32);
    f[10].set_fl(f[6].fl() * f[8].fl());
    f[16].set_fl(f[4].fl() + f[10].fl());
    f[6].set_u32l(lw(m, g[A1], 0x38) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x3C) as u32);
    f[8].set_fl(f[18].fl() * f[6].fl());
    f[18].set_u32l(lw(m, g[SP], 0x3C) as u32);
    g[SP] = addu(g[SP], 0x40);
    f[6].set_fl(f[10].fl() * f[18].fl());
    f[4].set_fl(f[16].fl() + f[8].fl());
    f[16].set_fl(f[6].fl() + f[4].fl());
    sw(m, g[A0], 0x3C, u64::from(f[16].u32l()));
}

/// `func_800160BC(out, m)` (inverse of a scaled rotation plus translation,
/// **guess** at the use: a camera or node transform): with `s_i` the
/// squared length of row `i` of `m` (`s_i = m[i][2]^2 + (m[i][0]^2 +
/// m[i][1]^2)`, rows 1 and 2 through a frame at `sp - 0x20`), `out[j][i] =
/// m[i][j] / s_i` for the 3x3 part (stored in the order `[0][1] [0][2]
/// [1][2] [1][0] [2][0] [2][1] [0][0] [1][1]`, then the zero column `+0xC,
/// +0x1C, +0x2C`, `1.0` at `+0x3C`, then `[2][2]`), and the translation row
/// `out[3][j] = -(out[2][j] * t.z + (t.x * out[0][j] + t.y * out[1][j]))`
/// with `t` = `m`'s row 3, reading `out` back. So `out` must not overlap
/// `m` for the statement (the C and the port agree either way).
///
/// Leaves `f0`, `f2`, `f12` = `t`, `f14 = 0.0`, `f4`..`f10` from the last
/// column, `at = 0x3F800000`.
///
/// Domain: canonical pointers to 64 bytes; the rows, `t`, every square,
/// sum and quotient, and every product and sum of the translation not NaN
/// (the negations are guarded too).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800160BC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[16].set_u32l(lw(m, g[A1], 0) as u32);
    f[18].set_u32l(lw(m, g[A1], 4) as u32);
    f[14].set_u32l(lw(m, g[A1], 8) as u32);
    f[4].set_fl(f[16].fl() * f[16].fl());
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    g[AT] = li(0x3F80_0000);
    f[6].set_fl(f[18].fl() * f[18].fl());
    f[8].set_fl(f[4].fl() + f[6].fl());
    f[10].set_fl(f[14].fl() * f[14].fl());
    f[4].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[14].set_u32l(0 as u32);
    sw(m, g[SP], 0x10, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[0].set_fl(f[10].fl() + f[8].fl());
    sw(m, g[SP], 0xC, u64::from(f[6].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[8].set_u32l(lw(m, g[SP], 0xC) as u32);
    sw(m, g[SP], 8, u64::from(f[10].u32l()));
    f[4].set_fl(f[8].fl() * f[8].fl());
    f[6].set_u32l(lw(m, g[SP], 8) as u32);
    f[10].set_fl(f[6].fl() * f[6].fl());
    f[6].set_fl(f[4].fl() + f[10].fl());
    f[4].set_u32l(lw(m, g[SP], 0x10) as u32);
    f[10].set_fl(f[4].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[A1], 0x28) as u32);
    sw(m, g[SP], 0x10, u64::from(f[4].u32l()));
    f[2].set_fl(f[10].fl() + f[6].fl());
    f[10].set_u32l(lw(m, g[A1], 0x20) as u32);
    sw(m, g[SP], 8, u64::from(f[10].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[4].set_u32l(lw(m, g[SP], 8) as u32);
    sw(m, g[SP], 4, u64::from(f[6].u32l()));
    f[10].set_fl(f[4].fl() * f[4].fl());
    f[6].set_u32l(lw(m, g[SP], 4) as u32);
    f[4].set_fl(f[6].fl() * f[6].fl());
    f[6].set_fl(f[10].fl() + f[4].fl());
    f[10].set_u32l(lw(m, g[SP], 0x10) as u32);
    f[4].set_fl(f[10].fl() * f[10].fl());
    f[10].set_fl(f[8].fl() / f[2].fl());
    f[12].set_fl(f[4].fl() + f[6].fl());
    sw(m, g[A0], 4, u64::from(f[10].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x20) as u32);
    f[6].set_fl(f[4].fl() / f[12].fl());
    sw(m, g[A0], 8, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[10].set_fl(f[8].fl() / f[12].fl());
    sw(m, g[A0], 0x18, u64::from(f[10].u32l()));
    f[4].set_u32l(lw(m, g[A1], 4) as u32);
    f[6].set_fl(f[4].fl() / f[0].fl());
    sw(m, g[A0], 0x10, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 8) as u32);
    f[10].set_fl(f[8].fl() / f[0].fl());
    sw(m, g[A0], 0x20, u64::from(f[10].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[6].set_fl(f[4].fl() / f[2].fl());
    sw(m, g[A0], 0x24, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_fl(f[8].fl() / f[0].fl());
    sw(m, g[A0], 0, u64::from(f[10].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[6].set_fl(f[4].fl() / f[2].fl());
    f[4].set_u32l(g[AT] as u32);
    sw(m, g[A0], 0x14, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x28) as u32);
    sw(m, g[A0], 0xC, u64::from(f[14].u32l()));
    sw(m, g[A0], 0x1C, u64::from(f[14].u32l()));
    f[10].set_fl(f[8].fl() / f[12].fl());
    sw(m, g[A0], 0x2C, u64::from(f[14].u32l()));
    sw(m, g[A0], 0x3C, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0) as u32);
    sw(m, g[A0], 0x28, u64::from(f[10].u32l()));
    f[0].set_u32l(lw(m, g[A1], 0x30) as u32);
    f[2].set_u32l(lw(m, g[A1], 0x34) as u32);
    f[10].set_u32l(lw(m, g[A0], 0x10) as u32);
    f[8].set_fl(f[0].fl() * f[6].fl());
    f[12].set_u32l(lw(m, g[A1], 0x38) as u32);
    f[4].set_fl(f[2].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A0], 0x20) as u32);
    f[6].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[10].fl() * f[12].fl());
    f[4].set_fl(f[8].fl() + f[6].fl());
    f[8].set_u32l(lw(m, g[A0], 4) as u32);
    f[10].set_fl(-f[4].fl());
    f[6].set_fl(f[0].fl() * f[8].fl());
    f[4].set_u32l(lw(m, g[A0], 0x14) as u32);
    sw(m, g[A0], 0x30, u64::from(f[10].u32l()));
    f[10].set_fl(f[2].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[A0], 0x24) as u32);
    f[8].set_fl(f[6].fl() + f[10].fl());
    f[6].set_fl(f[4].fl() * f[12].fl());
    f[10].set_fl(f[6].fl() + f[8].fl());
    f[6].set_u32l(lw(m, g[A0], 8) as u32);
    f[4].set_fl(-f[10].fl());
    f[8].set_fl(f[0].fl() * f[6].fl());
    f[10].set_u32l(lw(m, g[A0], 0x18) as u32);
    sw(m, g[A0], 0x34, u64::from(f[4].u32l()));
    f[4].set_fl(f[2].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A0], 0x28) as u32);
    g[SP] = addu(g[SP], 0x20);
    f[6].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[10].fl() * f[12].fl());
    f[4].set_fl(f[8].fl() + f[6].fl());
    f[10].set_fl(-f[4].fl());
    sw(m, g[A0], 0x38, u64::from(f[10].u32l()));
}

/// `func_80016260(lu, a, indx)` (ludcmp, **guess** at the name: the shape
/// of Numerical Recipes' Crout LU decomposition with implicit partial
/// pivoting, for n = 3): copies the 3x3 matrix `a` to `lu` and decomposes
/// it in place, writing the pivot row of each column to `indx` (3 words).
/// Returns 1, or 0 for a zero pivot. Rows are 16 bytes apart (the upper
/// 3x3 of a 4x4 matrix); the fourth float of each row is not touched. No
/// parity is computed. Only caller: `func_80016A20` (a 3x3 inverse).
///
/// With `|x|` = `-x` if `x < 0`, else `x`, and every sum subtracting its
/// products in increasing `k`:
/// 1. **Scaling**, row by row: for j = 0..2, `lu[i][j] = a[i][j]`; then
///    `t = |a[i][j]|` (re-read from `a`); if `big < t` (big = 0 at the row's
///    start), `big = t` and `vv[i] = 1 / t`. `vv` is the frame
///    (`sp - 0x28 + 4i`). QUIRK: a row with no nonzero element leaves `vv[i]`
///    as the stale stack word, which step 2 then multiplies. The machine
///    code's singular test (`t == 0`: return 0) sits inside the update,
///    where `t > big >= 0`, so it can never fire; the port leaves it out.
/// 2. For each column j = 0..2:
///    - for i < j: `lu[i][j] = lu[i][j] - lu[i][0] * lu[0][j] - ...` (k < i);
///    - `big = 0`; for i = j..2: `sum = lu[i][j] - lu[i][k] * lu[k][j]...`
///      (k < j), stored; `dum = |sum| * vv[i]`; if `big <= dum`, `big = dum`
///      and `imax = i`. `imax` carries over from column to column. QUIRK: at
///      j = 0 it is the stale frame word `sp - 0x10`, kept if no `dum` passes
///      (all NaN, e.g. `0 * inf` from a subnormal row maximum, or a stale
///      negative `vv`), and then used as a row index, even outside 0..2;
///    - if `imax != j`, rows j and imax swap (all three words, as bits) and
///      `vv[imax] = vv[j]`;
///    - `indx[j] = imax`; if `lu[j][j] == 0` (either sign), return 0;
///    - if j != 2: `r = 1 / lu[j][j]` and `lu[i][j] = lu[i][j] * r` for i > j.
/// 3. `imax` goes back to its frame word and it returns 1.
///
/// IDO unrolled every `k` loop by four after a remainder loop; with n = 3
/// no loop has more than two terms, so only the remainder loops run and
/// the port leaves the unrolled parts out, as it does the remainder loop's
/// repeat above the diagonal (`i <= 1` there: one term at most).
/// `debug_assert`s mark where the C would branch to them.
///
/// Frame (`sp - 0x88`): `f20` saved at `+8`, `s0`..`s5` at `+0x10..+0x24`,
/// `vv` at `+0x60`, `imax` at `+0x78`, all restored or left as written.
/// Leaves `f20` restored, `f16`/`f18` = 0.0, `f0` the last pivot (or `big`
/// if the scaling ended it), `f2`..`f14` and `t0`..`t9`, `v1`, `a0`..`a3`,
/// `at` from the last steps taken (a zero pivot returns early with them as
/// they were).
///
/// Domain: canonical pointers to three 12-byte rows each, and `indx`; `lu`
/// either equal to `a` or disjoint from it. Every value reaching an
/// arithmetic operation not NaN: the elements, the stale `vv` words of zero
/// rows, the sums and pivots (the absolute values, compares and swaps take
/// anything). A stale `imax` must address memory: rows `lu + 16 * imax` and
/// frame word `vv[imax]`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80016260(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[SP] = addu(g[SP], (-0x88i64) as u64);
    sw(m, g[SP], 0x14, g[S1]);
    sd(m, g[SP], 8, f[20].u64);
    g[AT] = li(0x3F80_0000);
    g[S1] = g[A2];
    sw(m, g[SP], 0x10, g[S0]);
    f[20].set_u32l(g[AT] as u32); // 1.0
    f[18].set_u32l(0);
    f[16].set_u32l(0);
    g[S0] = g[A0];
    sw(m, g[SP], 0x24, g[S5]);
    sw(m, g[SP], 0x20, g[S4]);
    sw(m, g[SP], 0x1C, g[S3]);
    sw(m, g[SP], 0x18, g[S2]);
    g[A2] = 3; // n
    g[T2] = addu(g[SP], 0x60); // vv
    // 1. Copy and scale: a3 = i, t0 = &lu[i][0], v0 = &a[i][0].
    g[A3] = 0;
    g[T0] = g[A0];
    g[V0] = g[A1];
    loop {
        f[0].set_u32l(f[16].u32l()); // big = 0
        g[A1] = 0; // 4j
        g[V1] = g[T0];
        g[T1] = g[V0];
        loop {
            f[4].set_u32l(lw(m, g[T1], 0) as u32);
            g[A1] = addu(g[A1], 4);
            g[AT] = slt(g[A1], 0xC);
            sw(m, g[V1], 0, u64::from(f[4].u32l()));
            f[2].set_u32l(lw(m, g[T1], 0) as u32);
            g[V1] = addu(g[V1], 4);
            if f[2].fl() < f[18].fl() {
                f[12].set_fl(-f[2].fl());
            } else {
                f[12].set_u32l(f[2].u32l());
            }
            if f[0].fl() < f[12].fl() {
                f[0].set_u32l(f[12].u32l());
                f[6].set_fl(f[20].fl() / f[12].fl());
                g[T6] = sll(g[A3], 2);
                g[T7] = addu(g[T2], g[T6]);
                sw(m, g[T7], 0, u64::from(f[6].u32l()));
            }
            g[T1] = addu(g[T1], 4);
            if g[AT] == 0 {
                break;
            }
        }
        g[A3] = addu(g[A3], 1);
        g[T0] = addu(g[T0], 0x10);
        g[V0] = addu(g[V0], 0x10);
        if g[A3] == g[A2] {
            break;
        }
    }
    'done: {
        // 2. Column by column: a0 = j, a1 = 4j, t2 = &lu[j][0], t3 = &indx[j],
        // t4 = &lu[j][j], t1 = imax.
        g[A0] = 0;
        g[A1] = 0;
        g[T2] = g[S0];
        g[T3] = g[S1];
        g[T4] = g[S0];
        g[T5] = 2; // n - 1
        g[T1] = lw(m, g[SP], 0x78); // QUIRK: stale at j = 0
        loop {
            g[A3] = 0; // i
            f[0].set_u32l(f[18].u32l()); // big = 0
            // Above the diagonal (i < j): t0 = &lu[i][0], v1 = &lu[i][j].
            if (g[A0] as i64) > 0 {
                g[T0] = g[S0];
                g[V1] = addu(g[S0], g[A1]);
                loop {
                    f[2].set_u32l(lw(m, g[V1], 0) as u32); // sum
                    g[V0] = 0; // k
                    if (g[A3] as i64) > 0 {
                        // s2 = &lu[i][k], s3 = &lu[k][j], s1 = the remainder count.
                        g[S4] = g[A3] & 3;
                        g[S1] = g[S4];
                        g[T9] = sll(0, 4);
                        g[T6] = addu(g[S0], g[T9]);
                        g[T8] = sll(0, 2);
                        g[S2] = addu(g[T0], g[T8]);
                        g[S3] = addu(g[T6], g[A1]);
                        g[V0] = addu(g[V0], 1);
                        f[14].set_u32l(lw(m, g[S2], 0) as u32);
                        f[12].set_u32l(lw(m, g[S3], 0) as u32);
                        // i <= 1: the C's loop over further terms never runs.
                        debug_assert_eq!(g[S1], g[V0], "i < j <= 2: one term");
                        f[10].set_fl(f[14].fl() * f[12].fl());
                        g[S2] = addu(g[S2], 4);
                        g[S3] = addu(g[S3], 0x10);
                        f[2].set_fl(f[2].fl() - f[10].fl());
                        debug_assert_eq!(g[V0], g[A3], "unrolled part (i >= 4)");
                        g[T8] = sll(g[V0], 4);
                    }
                    g[A3] = addu(g[A3], 1);
                    g[T0] = addu(g[T0], 0x10);
                    g[V1] = addu(g[V1], 0x10);
                    sw(m, g[V1], -0x10, u64::from(f[2].u32l()));
                    if g[A3] == g[A0] {
                        break;
                    }
                }
            }
            // On and below (i = j..2): t0 = &lu[i][0], s1 = &vv[i], v1 = &lu[i][j].
            g[AT] = slt(g[A0], 3);
            g[A3] = g[A0];
            debug_assert!(g[AT] != 0);
            g[T6] = sll(g[A3], 4);
            g[T0] = addu(g[S0], g[T6]);
            g[T7] = sll(g[A3], 2);
            g[T8] = addu(g[SP], 0x60);
            g[S1] = addu(g[T7], g[T8]);
            g[V1] = addu(g[T0], g[A1]);
            loop {
                f[2].set_u32l(lw(m, g[V1], 0) as u32); // sum
                g[V0] = 0; // k
                if (g[A0] as i64) > 0 {
                    // s2 = &lu[i][k], s3 = &lu[k][j], s4 = the remainder count.
                    g[S5] = g[A0] & 3;
                    g[S4] = g[S5];
                    g[T6] = sll(0, 4);
                    g[T7] = addu(g[S0], g[T6]);
                    g[T9] = sll(0, 2);
                    g[S2] = addu(g[T0], g[T9]);
                    g[S3] = addu(g[T7], g[A1]);
                    g[V0] = addu(g[V0], 1);
                    f[14].set_u32l(lw(m, g[S2], 0) as u32);
                    f[12].set_u32l(lw(m, g[S3], 0) as u32);
                    while g[S4] != g[V0] {
                        f[10].set_fl(f[14].fl() * f[12].fl());
                        f[14].set_u32l(lw(m, g[S2], 4) as u32);
                        f[12].set_u32l(lw(m, g[S3], 0x10) as u32);
                        g[V0] = addu(g[V0], 1);
                        g[S2] = addu(g[S2], 4);
                        g[S3] = addu(g[S3], 0x10);
                        f[2].set_fl(f[2].fl() - f[10].fl());
                    }
                    f[10].set_fl(f[14].fl() * f[12].fl());
                    g[S2] = addu(g[S2], 4);
                    g[S3] = addu(g[S3], 0x10);
                    f[2].set_fl(f[2].fl() - f[10].fl());
                    debug_assert_eq!(g[V0], g[A0], "unrolled part (j >= 4)");
                    g[T9] = sll(g[V0], 4);
                }
                g[T0] = addu(g[T0], 0x10);
                g[V1] = addu(g[V1], 0x10);
                sw(m, g[V1], -0x10, u64::from(f[2].u32l()));
                if f[2].fl() < f[18].fl() {
                    f[14].set_fl(-f[2].fl());
                } else {
                    f[14].set_u32l(f[2].u32l());
                }
                f[4].set_u32l(lw(m, g[S1], 0) as u32);
                f[12].set_fl(f[14].fl() * f[4].fl()); // dum
                if f[0].fl() <= f[12].fl() {
                    f[0].set_u32l(f[12].u32l());
                    g[T1] = g[A3];
                }
                g[A3] = addu(g[A3], 1);
                g[S1] = addu(g[S1], 4);
                if g[A3] == g[A2] {
                    break;
                }
            }
            // Swap rows j and imax: t0 = &vv[imax], v1 = &lu[imax][k], a3 = &lu[j][k].
            g[V0] = 0;
            if g[A0] != g[T1] {
                g[T9] = addu(g[SP], 0x60);
                g[T7] = sll(g[T1], 4);
                g[T8] = sll(g[T1], 2);
                g[T0] = addu(g[T8], g[T9]);
                g[V1] = addu(g[S0], g[T7]);
                g[S1] = addu(g[A1], g[T9]);
                g[A3] = g[T2];
                loop {
                    f[6].set_u32l(lw(m, g[A3], 0) as u32);
                    f[12].set_u32l(lw(m, g[V1], 0) as u32);
                    g[V0] = addu(g[V0], 1);
                    sw(m, g[V1], 0, u64::from(f[6].u32l()));
                    g[V1] = addu(g[V1], 4);
                    g[A3] = addu(g[A3], 4);
                    sw(m, g[A3], -4, u64::from(f[12].u32l()));
                    if g[V0] == g[A2] {
                        break;
                    }
                }
                f[8].set_u32l(lw(m, g[S1], 0) as u32);
                sw(m, g[T0], 0, u64::from(f[8].u32l()));
            }
            sw(m, g[T3], 0, g[T1]); // indx[j] = imax
            f[0].set_u32l(lw(m, g[T4], 0) as u32); // the pivot
            g[V0] = addu(g[A0], 1);
            g[A3] = g[V0];
            g[AT] = slt(g[V0], 3);
            if f[16].fl() == f[0].fl() {
                g[V0] = 0;
                break 'done;
            }
            if g[A0] == g[T5] {
                g[A0] = addu(g[A0], 1);
            } else {
                // Divide the column below by the pivot: a3 = i, v1 = &lu[i][j],
                // t0 = the remainder loop's end (n).
                f[12].set_fl(f[20].fl() / f[0].fl());
                debug_assert!(g[AT] != 0);
                g[S1] = subu(g[A2], g[V0]);
                g[T6] = g[S1] & 3;
                g[T0] = addu(g[T6], g[V0]);
                debug_assert!(g[T6] != 0);
                g[T7] = sll(g[V0], 4);
                g[T8] = addu(g[S0], g[T7]);
                g[V1] = addu(g[T8], g[A1]);
                f[10].set_u32l(lw(m, g[V1], 0) as u32);
                g[A3] = addu(g[A3], 1);
                f[14].set_fl(f[10].fl() * f[12].fl());
                while g[T0] != g[A3] {
                    f[10].set_u32l(lw(m, g[V1], 0x10) as u32);
                    g[A3] = addu(g[A3], 1);
                    sw(m, g[V1], 0, u64::from(f[14].u32l()));
                    f[14].set_fl(f[10].fl() * f[12].fl());
                    g[V1] = addu(g[V1], 0x10);
                }
                sw(m, g[V1], 0, u64::from(f[14].u32l()));
                g[V1] = addu(g[V1], 0x10);
                debug_assert_eq!(g[A3], g[A2], "unrolled part");
                g[T9] = sll(g[A3], 4);
                g[A0] = addu(g[A0], 1);
            }
            g[A1] = addu(g[A1], 4);
            g[T2] = addu(g[T2], 0x10);
            g[T3] = addu(g[T3], 4);
            g[T4] = addu(g[T4], 0x14);
            if g[A0] == g[A2] {
                break;
            }
        }
        sw(m, g[SP], 0x78, g[T1]);
        g[V0] = 1;
    }
    f[20].u64 = ld(m, g[SP], 8);
    g[S0] = lw(m, g[SP], 0x10);
    g[S1] = lw(m, g[SP], 0x14);
    g[S2] = lw(m, g[SP], 0x18);
    g[S3] = lw(m, g[SP], 0x1C);
    g[S4] = lw(m, g[SP], 0x20);
    g[S5] = lw(m, g[SP], 0x24);
    g[SP] = addu(g[SP], 0x88);
}

/// `func_800167E4(lu, indx, b)` (lubksb, **guess** at the name: Numerical
/// Recipes' LU back-substitution, n = 3): solves `LU x = P b` in place,
/// with `lu` and `indx` as `func_80016260` leaves them (rows 16 bytes
/// apart). Only caller: `func_80016A20`, once per column of the inverse.
///
/// Forward, with `ii` = -1 at the start: for i = 0..2, `ip = indx[i]`,
/// `sum = b[ip]`, `b[ip] = b[i]` (as bits; `b[i]` read first); if `ii >= 0`,
/// `sum = sum - lu[i][ii] * b[ii] - ... - lu[i][i-1] * b[i-1]`; otherwise if
/// `sum != 0` (so not ±0), `ii = i`; then `b[i] = sum`. Backward: for
/// i = 2..0, `sum = b[i] - lu[i][i+1] * b[i+1] - ...` (j < 3) and
/// `b[i] = sum / lu[i][i]`. Subtractions go in increasing index order.
///
/// As in `func_80016260`, IDO unrolled the loops by four after a remainder
/// loop that does all of the (at most two) terms; the unrolled parts, and
/// the forward loop's skip for `ii > i - 1` (impossible once `ii` is set),
/// are left out. No return value: `v0` is left at `lu - 0x10`.
///
/// Saves `s0` at `sp - 4` and restores it. Leaves `f2` = 0.0, `f0` the
/// last sum, `f8` the last quotient (`b[0]`), `f6` = `lu[0][0]`, `f14`,
/// `f16`, `f18` the last product and factors, `v1` = -1, `a3` = `lu - 0x14`,
/// `t0` = `b - 4`, and `a0`, `a1`, `t1`..`t9`, `at` from the last steps.
///
/// Domain: canonical pointers (`lu` three 12-byte rows, `indx` and `b` 12
/// bytes); `indx` entries addressing memory (`b + 4 * ip`, sign-extended
/// words). Values reaching arithmetic not NaN (the forward loop's zero test
/// takes anything).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800167E4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[SP] = addu(g[SP], (-8i64) as u64);
    sw(m, g[SP], 4, g[S0]);
    f[2].set_u32l(0);
    g[S0] = g[A0];
    // Forward: v0 = ii, v1 = i, a3 = &indx[i], t0 = &b[i].
    g[V0] = u64::MAX;
    g[V1] = 0;
    g[A3] = g[A1];
    g[T0] = g[A2];
    loop {
        g[A0] = lw(m, g[A3], 0); // ip
        f[4].set_u32l(lw(m, g[T0], 0) as u32);
        g[T6] = sll(g[A0], 2);
        g[A1] = addu(g[A2], g[T6]);
        f[0].set_u32l(lw(m, g[A1], 0) as u32); // sum
        sw(m, g[A1], 0, u64::from(f[4].u32l()));
        if (g[V0] as i64) < 0 {
            if f[0].fl() != f[2].fl() {
                g[V0] = g[V1];
            }
        } else {
            // t2 = &lu[i][j], t3 = &b[j + 1], t4 = &b[i - 1].
            g[A1] = addu(g[V1], u64::MAX);
            g[AT] = slt(g[A1], g[V0]);
            g[A0] = g[V0];
            debug_assert!(g[AT] == 0, "ii > i - 1");
            g[T1] = sll(g[A0], 2);
            g[T7] = sll(g[V1], 4);
            g[T8] = addu(g[S0], g[T7]);
            g[T9] = sll(g[A1], 2);
            g[T4] = addu(g[T9], g[A2]);
            g[T2] = addu(g[T8], g[T1]);
            g[T3] = addu(g[A2], g[T1]);
            g[T3] = addu(g[T3], 4);
            g[AT] = sltu(g[T4], g[T3]);
            f[18].set_u32l(lw(m, g[T2], 0) as u32);
            f[16].set_u32l(lw(m, g[T3], -4) as u32);
            while g[AT] == 0 {
                f[14].set_fl(f[18].fl() * f[16].fl());
                f[18].set_u32l(lw(m, g[T2], 4) as u32);
                f[16].set_u32l(lw(m, g[T3], 0) as u32);
                g[T3] = addu(g[T3], 4);
                g[AT] = sltu(g[T4], g[T3]);
                g[T2] = addu(g[T2], 4);
                f[0].set_fl(f[0].fl() - f[14].fl());
            }
            f[14].set_fl(f[18].fl() * f[16].fl());
            g[T2] = addu(g[T2], 4);
            f[0].set_fl(f[0].fl() - f[14].fl());
        }
        g[V1] = addu(g[V1], 1);
        g[AT] = slt(g[V1], 3);
        g[A3] = addu(g[A3], 4);
        g[T0] = addu(g[T0], 4);
        sw(m, g[T0], -4, u64::from(f[0].u32l()));
        if g[AT] == 0 {
            break;
        }
    }
    // Backward: v1 = i, v0 = &lu[i][0], a3 = &lu[i][i], t0 = &b[i],
    // s0 = n.
    g[V0] = addu(g[S0], 0x20);
    g[A3] = addu(g[V0], 8);
    g[S0] = 3;
    g[V1] = 2;
    g[T0] = addu(g[A2], 8);
    loop {
        g[A1] = addu(g[V1], 1);
        g[AT] = slt(g[A1], 3);
        f[0].set_u32l(lw(m, g[T0], 0) as u32); // sum
        g[A0] = g[A1]; // j
        if g[AT] != 0 {
            // t2 = &lu[i][j], t3 = &b[j], t4 = the remainder loop's end (n).
            g[T5] = subu(g[S0], g[A1]);
            g[T6] = g[T5] & 3;
            g[T4] = addu(g[T6], g[A1]);
            debug_assert!(g[T6] != 0);
            g[T1] = sll(g[A1], 2);
            g[T2] = addu(g[V0], g[T1]);
            g[T3] = addu(g[A2], g[T1]);
            g[A0] = addu(g[A0], 1);
            f[18].set_u32l(lw(m, g[T2], 0) as u32);
            f[16].set_u32l(lw(m, g[T3], 0) as u32);
            while g[T4] != g[A0] {
                f[14].set_fl(f[18].fl() * f[16].fl());
                f[18].set_u32l(lw(m, g[T2], 4) as u32);
                f[16].set_u32l(lw(m, g[T3], 4) as u32);
                g[A0] = addu(g[A0], 1);
                g[T2] = addu(g[T2], 4);
                g[T3] = addu(g[T3], 4);
                f[0].set_fl(f[0].fl() - f[14].fl());
            }
            f[14].set_fl(f[18].fl() * f[16].fl());
            g[T2] = addu(g[T2], 4);
            g[T3] = addu(g[T3], 4);
            f[0].set_fl(f[0].fl() - f[14].fl());
            debug_assert_eq!(g[A0], g[S0], "unrolled part");
            g[T1] = sll(g[A0], 2);
        }
        f[6].set_u32l(lw(m, g[A3], 0) as u32);
        g[V1] = addu(g[V1], u64::MAX);
        g[T0] = addu(g[T0], (-4i64) as u64);
        f[8].set_fl(f[0].fl() / f[6].fl());
        g[V0] = addu(g[V0], (-0x10i64) as u64);
        g[A3] = addu(g[A3], (-0x14i64) as u64);
        sw(m, g[T0], 4, u64::from(f[8].u32l()));
        if (g[V1] as i64) < 0 {
            break;
        }
    }
    g[S0] = lw(m, g[SP], 4);
    g[SP] = addu(g[SP], 8);
}

/// `func_80016BF4(out, v, m)` (vec3 times the 3x3 part of a 4x4 matrix,
/// row vector): `out[j] = v.z * m[2][j] + (m[0][j] * v.x + m[1][j] * v.y)`,
/// all three computed into a frame (`sp - 0x10 + 4..`) before `out` is
/// written, so `out` may alias `v`.
///
/// Leaves `f8` = `out.x`, `f6`/`f18` = `out.y`/`out.z` reloaded, and
/// `f4`..`f16` from the last component.
///
/// Domain: canonical pointers (12 and 64 bytes); the inputs, products and
/// partial sums not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80016BF4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A2], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_u32l(lw(m, g[A2], 0x10) as u32);
    f[16].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    g[SP] = addu(g[SP], (-0x10i64) as u64);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[10].set_u32l(lw(m, g[A2], 0x20) as u32);
    f[16].set_fl(f[6].fl() * f[10].fl());
    f[4].set_fl(f[8].fl() + f[18].fl());
    f[8].set_fl(f[16].fl() + f[4].fl());
    sw(m, g[SP], 4, u64::from(f[8].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[18].set_u32l(lw(m, g[A2], 4) as u32);
    f[4].set_u32l(lw(m, g[A1], 4) as u32);
    f[16].set_u32l(lw(m, g[A2], 0x14) as u32);
    f[10].set_fl(f[18].fl() * f[6].fl());
    f[18].set_fl(f[16].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[A2], 0x24) as u32);
    f[16].set_u32l(lw(m, g[A1], 8) as u32);
    f[6].set_fl(f[10].fl() + f[18].fl());
    f[10].set_fl(f[16].fl() * f[4].fl());
    f[18].set_fl(f[10].fl() + f[6].fl());
    sw(m, g[SP], 8, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[16].set_u32l(lw(m, g[A2], 8) as u32);
    f[18].set_u32l(lw(m, g[A1], 4) as u32);
    f[6].set_u32l(lw(m, g[A2], 0x18) as u32);
    f[10].set_fl(f[16].fl() * f[4].fl());
    f[16].set_fl(f[6].fl() * f[18].fl());
    f[18].set_u32l(lw(m, g[A2], 0x28) as u32);
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[4].set_fl(f[10].fl() + f[16].fl());
    f[10].set_fl(f[6].fl() * f[18].fl());
    f[16].set_fl(f[10].fl() + f[4].fl());
    sw(m, g[SP], 0xC, u64::from(f[16].u32l()));
    sw(m, g[A0], 0, u64::from(f[8].u32l()));
    f[6].set_u32l(lw(m, g[SP], 8) as u32);
    sw(m, g[A0], 4, u64::from(f[6].u32l()));
    f[18].set_u32l(lw(m, g[SP], 0xC) as u32);
    g[SP] = addu(g[SP], 0x10);
    sw(m, g[A0], 8, u64::from(f[18].u32l()));
}

/// `func_80016CAC(out, v, m)` (vec3 point times a 4x4 matrix, row vector):
/// `out[j] = m[3][j] + ((m[0][j] * v.x + m[1][j] * v.y) + m[2][j] * v.z)`,
/// computed into a frame before `out` is written (so `out` may alias `v`).
///
/// Leaves `f6` = `out.x`, `f18`/`f16` = `out.y`/`out.z` reloaded, and
/// `f4`..`f10` from the last component.
///
/// Domain: as for [`func_80016BF4`].
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80016CAC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A2], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_u32l(lw(m, g[A2], 0x10) as u32);
    f[16].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A2], 0x20) as u32);
    g[SP] = addu(g[SP], (-0x10i64) as u64);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[10].set_u32l(lw(m, g[A1], 8) as u32);
    f[16].set_fl(f[6].fl() * f[10].fl());
    f[4].set_fl(f[8].fl() + f[18].fl());
    f[18].set_u32l(lw(m, g[A2], 0x30) as u32);
    f[8].set_fl(f[4].fl() + f[16].fl());
    f[6].set_fl(f[18].fl() + f[8].fl());
    sw(m, g[SP], 4, u64::from(f[6].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_u32l(lw(m, g[A2], 4) as u32);
    f[8].set_u32l(lw(m, g[A1], 4) as u32);
    f[18].set_u32l(lw(m, g[A2], 0x14) as u32);
    f[16].set_fl(f[10].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[A2], 0x24) as u32);
    f[6].set_fl(f[18].fl() * f[8].fl());
    f[18].set_u32l(lw(m, g[A1], 8) as u32);
    f[8].set_fl(f[4].fl() * f[18].fl());
    f[10].set_fl(f[16].fl() + f[6].fl());
    f[6].set_u32l(lw(m, g[A2], 0x34) as u32);
    f[16].set_fl(f[10].fl() + f[8].fl());
    f[4].set_fl(f[6].fl() + f[16].fl());
    sw(m, g[SP], 8, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0) as u32);
    f[18].set_u32l(lw(m, g[A2], 8) as u32);
    f[16].set_u32l(lw(m, g[A1], 4) as u32);
    f[6].set_u32l(lw(m, g[A2], 0x18) as u32);
    f[8].set_fl(f[18].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A2], 0x28) as u32);
    f[4].set_fl(f[6].fl() * f[16].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[16].set_fl(f[10].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[SP], 4) as u32);
    f[18].set_fl(f[8].fl() + f[4].fl());
    f[4].set_u32l(lw(m, g[A2], 0x38) as u32);
    f[8].set_fl(f[18].fl() + f[16].fl());
    f[10].set_fl(f[4].fl() + f[8].fl());
    sw(m, g[SP], 0xC, u64::from(f[10].u32l()));
    sw(m, g[A0], 0, u64::from(f[6].u32l()));
    f[18].set_u32l(lw(m, g[SP], 8) as u32);
    sw(m, g[A0], 4, u64::from(f[18].u32l()));
    f[16].set_u32l(lw(m, g[SP], 0xC) as u32);
    g[SP] = addu(g[SP], 0x10);
    sw(m, g[A0], 8, u64::from(f[16].u32l()));
}

/// `func_80016D78(out, x, y, z, w)` (vec4 set): `out = (x, y, z, w)` with
/// the floats in `a1`, `a2`, `a3` (through its spill `[sp + 0xC]`) and the
/// stack argument at `sp + 0x10`. Bits are copied.
///
/// Leaves `f12 = x`, `f14 = y`, `f4 = z`, `f6 = w`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80016D78(rdram: *mut u8, ctx: *mut RecompContext) {
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
    f[6].set_u32l(lw(m, g[SP], 0x10) as u32);
    sw(m, g[A0], 0xC, u64::from(f[6].u32l()));
}

/// `func_80016DA0(out, s, v)` (vec4 scale): `out = v * s`, with the float
/// `s` in `a1`, component by component (so `out` may alias `v`).
///
/// Leaves `f12 = s`, `f4`/`f6` = the last component and product, and
/// `f8`..`f18` from the others.
///
/// Domain: canonical pointers to 16 bytes; `s` and the components not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80016DA0(rdram: *mut u8, ctx: *mut RecompContext) {
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
    f[4].set_u32l(lw(m, g[A2], 0xC) as u32);
    f[6].set_fl(f[4].fl() * f[12].fl());
    sw(m, g[A0], 0xC, u64::from(f[6].u32l()));
}

/// `func_80016DD8(out, v, m)` (vec4 times a 4x4 matrix, row vector):
/// `out[j] = v.w * m[3][j] + ((m[0][j] * v.x + m[1][j] * v.y) + m[2][j] *
/// v.z)`, all four computed into a frame (`sp - 0x10`) before `out` is
/// written (so `out` may alias `v`).
///
/// Leaves `f4` = `out.x`, `f10`/`f18`/`f6` = the others reloaded, and
/// `f8`, `f16` from the last component.
///
/// Domain: canonical pointers (16 and 64 bytes); the inputs, products and
/// partial sums not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80016DD8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A2], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_u32l(lw(m, g[A2], 0x10) as u32);
    f[16].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A2], 0x20) as u32);
    g[SP] = addu(g[SP], (-0x10i64) as u64);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[10].set_u32l(lw(m, g[A1], 8) as u32);
    f[16].set_fl(f[6].fl() * f[10].fl());
    f[6].set_u32l(lw(m, g[A2], 0x30) as u32);
    f[4].set_fl(f[8].fl() + f[18].fl());
    f[18].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[10].set_fl(f[18].fl() * f[6].fl());
    f[8].set_fl(f[4].fl() + f[16].fl());
    f[4].set_fl(f[10].fl() + f[8].fl());
    sw(m, g[SP], 0, u64::from(f[4].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0) as u32);
    f[16].set_u32l(lw(m, g[A2], 4) as u32);
    f[8].set_u32l(lw(m, g[A1], 4) as u32);
    f[10].set_u32l(lw(m, g[A2], 0x14) as u32);
    f[6].set_fl(f[16].fl() * f[18].fl());
    f[16].set_fl(f[10].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[A1], 8) as u32);
    f[10].set_u32l(lw(m, g[A2], 0x24) as u32);
    f[18].set_fl(f[6].fl() + f[16].fl());
    f[6].set_fl(f[10].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[A2], 0x34) as u32);
    f[10].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[16].set_fl(f[18].fl() + f[6].fl());
    f[18].set_fl(f[10].fl() * f[8].fl());
    f[6].set_fl(f[18].fl() + f[16].fl());
    sw(m, g[SP], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_u32l(lw(m, g[A2], 8) as u32);
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    f[16].set_u32l(lw(m, g[A2], 0x18) as u32);
    f[18].set_fl(f[10].fl() * f[8].fl());
    f[10].set_fl(f[16].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[16].set_u32l(lw(m, g[A2], 0x28) as u32);
    f[8].set_fl(f[18].fl() + f[10].fl());
    f[18].set_fl(f[16].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A2], 0x38) as u32);
    f[16].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[10].set_fl(f[8].fl() + f[18].fl());
    f[8].set_fl(f[16].fl() * f[6].fl());
    f[18].set_fl(f[8].fl() + f[10].fl());
    sw(m, g[SP], 8, u64::from(f[18].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[16].set_u32l(lw(m, g[A2], 0xC) as u32);
    f[18].set_u32l(lw(m, g[A1], 4) as u32);
    f[10].set_u32l(lw(m, g[A2], 0x1C) as u32);
    f[8].set_fl(f[16].fl() * f[6].fl());
    f[16].set_fl(f[10].fl() * f[18].fl());
    f[18].set_u32l(lw(m, g[A1], 8) as u32);
    f[10].set_u32l(lw(m, g[A2], 0x2C) as u32);
    f[6].set_fl(f[8].fl() + f[16].fl());
    f[8].set_fl(f[10].fl() * f[18].fl());
    f[18].set_u32l(lw(m, g[A2], 0x3C) as u32);
    f[10].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[16].set_fl(f[6].fl() + f[8].fl());
    f[6].set_fl(f[10].fl() * f[18].fl());
    f[8].set_fl(f[6].fl() + f[16].fl());
    sw(m, g[SP], 0xC, u64::from(f[8].u32l()));
    sw(m, g[A0], 0, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[SP], 4) as u32);
    sw(m, g[A0], 4, u64::from(f[10].u32l()));
    f[18].set_u32l(lw(m, g[SP], 8) as u32);
    sw(m, g[A0], 8, u64::from(f[18].u32l()));
    f[6].set_u32l(lw(m, g[SP], 0xC) as u32);
    g[SP] = addu(g[SP], 0x10);
    sw(m, g[A0], 0xC, u64::from(f[6].u32l()));
}

/// `func_80017520(m, x, y, z)` (4x4 scale matrix): `m` = diag(`x`, `y`, `z`,
/// 1) with the floats in `a1`, `a2`, `a3` (`z` through its spill `[sp +
/// 0xC]`), row-major. Stores zeros at `+4..+0x24` (off the diagonal), `x`,
/// `y`, the zeros at `+0x2C..+0x38`, `z`, then 1.0 at `+0x3C`. Bits are
/// copied, NaNs included.
///
/// Leaves `f0 = 0.0`, `f12 = x`, `f14 = y`, `f4 = z`, `f6 = 1.0`, `at =
/// 0x3F800000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017520(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[0].set_u32l(0 as u32);
    sw(m, g[SP], 0xC, g[A3]);
    f[12].set_u32l(g[A1] as u32);
    f[14].set_u32l(g[A2] as u32);
    sw(m, g[A0], 4, u64::from(f[0].u32l()));
    sw(m, g[A0], 8, u64::from(f[0].u32l()));
    sw(m, g[A0], 0xC, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x10, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x18, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x1C, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x20, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x24, u64::from(f[0].u32l()));
    sw(m, g[A0], 0, u64::from(f[12].u32l()));
    sw(m, g[A0], 0x14, u64::from(f[14].u32l()));
    f[4].set_u32l(lw(m, g[SP], 0xC) as u32);
    g[AT] = li(0x3F80_0000);
    f[6].set_u32l(g[AT] as u32);
    sw(m, g[A0], 0x2C, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x30, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x34, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x38, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x28, u64::from(f[4].u32l()));
    sw(m, g[A0], 0x3C, u64::from(f[6].u32l()));
}

/// `func_80017580(m, x, y, z)` (4x4 translation matrix): `m` = identity
/// with the translation row `+0x30..+0x38 = (x, y, z)` (floats in `a1`,
/// `a2`, `a3`, `z` through its spill). Zeros first, then the diagonal ones,
/// `x`, `y`, 1.0 at `+0x3C` and `z` last.
///
/// Leaves `f2 = 0.0`, `f0 = 1.0`, `f12 = x`, `f14 = y`, `f4 = z`, `at =
/// 0x3F800000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017580(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    sw(m, g[SP], 0xC, g[A3]);
    f[2].set_u32l(0 as u32);
    g[AT] = li(0x3F80_0000);
    f[0].set_u32l(g[AT] as u32);
    f[12].set_u32l(g[A1] as u32);
    f[14].set_u32l(g[A2] as u32);
    sw(m, g[A0], 4, u64::from(f[2].u32l()));
    sw(m, g[A0], 8, u64::from(f[2].u32l()));
    sw(m, g[A0], 0xC, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x10, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x18, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x1C, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x20, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x24, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x2C, u64::from(f[2].u32l()));
    sw(m, g[A0], 0, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x14, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x28, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x30, u64::from(f[12].u32l()));
    sw(m, g[A0], 0x34, u64::from(f[14].u32l()));
    f[4].set_u32l(lw(m, g[SP], 0xC) as u32);
    sw(m, g[A0], 0x3C, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x38, u64::from(f[4].u32l()));
}

/// `func_80017874(m)` (4x4 identity): the twelve off-diagonal words = 0.0,
/// then the four diagonal ones = 1.0.
///
/// Leaves `f2 = 0.0`, `f0 = 1.0`, `at = 0x3F800000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017874(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[2].set_u32l(0 as u32);
    g[AT] = li(0x3F80_0000);
    f[0].set_u32l(g[AT] as u32);
    sw(m, g[A0], 4, u64::from(f[2].u32l()));
    sw(m, g[A0], 8, u64::from(f[2].u32l()));
    sw(m, g[A0], 0xC, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x10, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x18, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x1C, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x20, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x24, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x2C, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x30, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x34, u64::from(f[2].u32l()));
    sw(m, g[A0], 0x38, u64::from(f[2].u32l()));
    sw(m, g[A0], 0, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x14, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x28, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x3C, u64::from(f[0].u32l()));
}

/// `func_80017918(out, sx, sy, sz, m)` (row-scaled 4x4 matrix): `out` = `m`
/// (the fifth argument, `sp + 0x10`, loaded into `a1`) with row 0 times
/// `sx`, row 1 times `sy`, row 2 times `sz` (floats in `a1`..`a3`; `sz`
/// re-read from its spill `[sp + 0xC]` for each element), row 3 copied.
/// Element by element, each stored before the next is loaded.
///
/// Leaves `a1 = m`, `f12 = sx`, `f14 = sy`, and `f4`..`f18` from the last
/// elements.
///
/// Domain: canonical pointers to 64 bytes; the scales and the first three
/// rows not NaN (row 3 is copied bit for bit).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017918(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A1] as u32);
    g[A1] = lw(m, g[SP], 0x10);
    sw(m, g[SP], 0xC, g[A3]);
    f[14].set_u32l(g[A2] as u32);
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_fl(f[4].fl() * f[12].fl());
    sw(m, g[A0], 0, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 4) as u32);
    f[10].set_fl(f[8].fl() * f[12].fl());
    sw(m, g[A0], 4, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A1], 8) as u32);
    f[18].set_fl(f[16].fl() * f[12].fl());
    sw(m, g[A0], 8, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[6].set_fl(f[4].fl() * f[12].fl());
    sw(m, g[A0], 0xC, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[10].set_fl(f[8].fl() * f[14].fl());
    sw(m, g[A0], 0x10, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[18].set_fl(f[16].fl() * f[14].fl());
    sw(m, g[A0], 0x14, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[6].set_fl(f[4].fl() * f[14].fl());
    sw(m, g[A0], 0x18, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x1C) as u32);
    f[10].set_fl(f[8].fl() * f[14].fl());
    sw(m, g[A0], 0x1C, u64::from(f[10].u32l()));
    f[18].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[16].set_u32l(lw(m, g[A1], 0x20) as u32);
    f[4].set_fl(f[16].fl() * f[18].fl());
    sw(m, g[A0], 0x20, u64::from(f[4].u32l()));
    f[8].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[6].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[10].set_fl(f[6].fl() * f[8].fl());
    sw(m, g[A0], 0x24, u64::from(f[10].u32l()));
    f[18].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[16].set_u32l(lw(m, g[A1], 0x28) as u32);
    f[4].set_fl(f[16].fl() * f[18].fl());
    sw(m, g[A0], 0x28, u64::from(f[4].u32l()));
    f[8].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[6].set_u32l(lw(m, g[A1], 0x2C) as u32);
    f[10].set_fl(f[6].fl() * f[8].fl());
    sw(m, g[A0], 0x2C, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A1], 0x30) as u32);
    sw(m, g[A0], 0x30, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0x34) as u32);
    sw(m, g[A0], 0x34, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x38) as u32);
    sw(m, g[A0], 0x38, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x3C) as u32);
    sw(m, g[A0], 0x3C, u64::from(f[6].u32l()));
}

/// `func_800811C0(x)`: stores the double register pair `f12`/`f13` (all
/// 64 bits, as `sdc1`) at `0x800A6750`. Leaves `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800811C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    sd(m, g[AT], 0x6750, ctx.fpr[12].u64);
}

/// `func_80081680(x, n, scale, max)` (a shaped response, **guess**): all
/// floats but the int `n` (`x` in `f12`, `scale` in `a2`, `max` in `a3`,
/// spilled to its home slot `sp + 0xC` and read back). With `a = x` if `0
/// < x`, else `-x` (so `+0` counts as negative): `p = a` multiplied by `a`
/// another `n - 1` times (none for `n < 2`), then by `scale`, clamped to
/// at most `max` (a `<` compare); returns `f0 = p`, negated when `x` was
/// not positive.
///
/// Leaves `f12 = a`, `f14 = scale`, `f2 = p`, `f4 = 0.0`, `f6 = max`, `v0`
/// = 1 for a negated result, else 0, `at = (n < 2)` (or 1 after the loop),
/// `a1` = 1 after the loop (else `n`).
///
/// Domain: no NaN operands: `x` (negated when not positive), and the
/// products (e.g. `0 * inf`, whose NaN the negation would meet); `max`
/// is only compared.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80081680(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(0);
    f[14].set_u32l(g[A2] as u32);
    sw(m, g[SP], 0xC, g[A3]);
    let positive = f[4].fl() < f[12].fl();
    g[V0] = 1;
    g[AT] = slt(g[A1], 2);
    if positive {
        g[V0] = 0;
    } else {
        f[12].set_fl(-f[12].fl());
    }
    f[2].set_u32l(f[12].u32l());
    if g[AT] == 0 {
        g[A1] = addu(g[A1], u64::MAX);
        loop {
            f[2].set_fl(f[2].fl() * f[12].fl());
            g[AT] = slt(g[A1], 2);
            if g[AT] != 0 {
                break;
            }
            g[A1] = addu(g[A1], u64::MAX);
        }
    }
    f[2].set_fl(f[2].fl() * f[14].fl());
    f[6].set_u32l(lw(m, g[SP], 0xC) as u32);
    if f[6].fl() < f[2].fl() {
        f[2].set_u32l(f[6].u32l());
    }
    if g[V0] == 0 {
        f[0].set_u32l(f[2].u32l());
    } else {
        f[0].set_fl(-f[2].fl());
    }
}

/// `func_80081700(a, b)`: `f0 = 1 - r / (r + a)` with `r = b / K` (floats
/// `a`, `b` in `f12`, `f14`; `K` the float at `0x800ADCB0`).
///
/// Leaves `at = 0x3F800000`, `f2 = r`, `f4 = K`, `f6 = 1.0`, `f8 = r + a`,
/// `f10` the quotient.
///
/// Domain: no NaN operands (`a`, `b`, `K`, and the intermediates).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80081700(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x800B_0000);
    f[4].set_u32l(lw(m, g[AT], -0x2350) as u32);
    g[AT] = li(0x3F80_0000);
    f[6].set_u32l(g[AT] as u32);
    f[2].set_fl(f[14].fl() / f[4].fl());
    f[8].set_fl(f[2].fl() + f[12].fl());
    f[10].set_fl(f[2].fl() / f[8].fl());
    f[0].set_fl(f[6].fl() - f[10].fl());
}

/// `func_80081730(out, v, m)` (point times a 4x4 matrix, row vector, w =
/// 1): `out[j] = m[3][j] + ((m[0][j] * v.x + m[1][j] * v.y) + m[2][j] *
/// v.z)` for j = 0..3 (four outputs), each stored before the next is
/// computed, with `v` re-read for each (so `out == v` feeds the first
/// results into the later ones).
///
/// Leaves `f4`..`f18` from the last component.
///
/// Domain: canonical pointers (16, 12 and 64 bytes); `out` equal to `v` or
/// disjoint from it, and disjoint from `m`; no NaN operands.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80081730(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    // out[0]
    f[4].set_u32l(lw(m, g[A2], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_u32l(lw(m, g[A2], 0x10) as u32);
    f[16].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A2], 0x20) as u32);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[10].set_u32l(lw(m, g[A1], 8) as u32);
    f[16].set_fl(f[6].fl() * f[10].fl());
    f[4].set_fl(f[8].fl() + f[18].fl());
    f[18].set_u32l(lw(m, g[A2], 0x30) as u32);
    f[8].set_fl(f[4].fl() + f[16].fl());
    f[6].set_fl(f[18].fl() + f[8].fl());
    sw(m, g[A0], 0, u64::from(f[6].u32l()));
    // out[1]
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_u32l(lw(m, g[A2], 4) as u32);
    f[8].set_u32l(lw(m, g[A1], 4) as u32);
    f[18].set_u32l(lw(m, g[A2], 0x14) as u32);
    f[16].set_fl(f[10].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[A2], 0x24) as u32);
    f[6].set_fl(f[18].fl() * f[8].fl());
    f[18].set_u32l(lw(m, g[A1], 8) as u32);
    f[8].set_fl(f[4].fl() * f[18].fl());
    f[10].set_fl(f[16].fl() + f[6].fl());
    f[6].set_u32l(lw(m, g[A2], 0x34) as u32);
    f[16].set_fl(f[10].fl() + f[8].fl());
    f[4].set_fl(f[6].fl() + f[16].fl());
    sw(m, g[A0], 4, u64::from(f[4].u32l()));
    // out[2]
    f[10].set_u32l(lw(m, g[A1], 0) as u32);
    f[18].set_u32l(lw(m, g[A2], 8) as u32);
    f[16].set_u32l(lw(m, g[A1], 4) as u32);
    f[6].set_u32l(lw(m, g[A2], 0x18) as u32);
    f[8].set_fl(f[18].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A2], 0x28) as u32);
    f[4].set_fl(f[6].fl() * f[16].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[16].set_fl(f[10].fl() * f[6].fl());
    f[18].set_fl(f[8].fl() + f[4].fl());
    f[4].set_u32l(lw(m, g[A2], 0x38) as u32);
    f[8].set_fl(f[18].fl() + f[16].fl());
    f[10].set_fl(f[4].fl() + f[8].fl());
    sw(m, g[A0], 8, u64::from(f[10].u32l()));
    // out[3]
    f[18].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[A2], 0xC) as u32);
    f[8].set_u32l(lw(m, g[A1], 4) as u32);
    f[4].set_u32l(lw(m, g[A2], 0x1C) as u32);
    f[16].set_fl(f[6].fl() * f[18].fl());
    f[18].set_u32l(lw(m, g[A2], 0x2C) as u32);
    f[10].set_fl(f[4].fl() * f[8].fl());
    f[4].set_u32l(lw(m, g[A1], 8) as u32);
    f[8].set_fl(f[18].fl() * f[4].fl());
    f[6].set_fl(f[16].fl() + f[10].fl());
    f[10].set_u32l(lw(m, g[A2], 0x3C) as u32);
    f[16].set_fl(f[6].fl() + f[8].fl());
    f[18].set_fl(f[10].fl() + f[16].fl());
    sw(m, g[A0], 0xC, u64::from(f[18].u32l()));
}

/// `func_800819A4(n, p, out)` (projection onto a plane): with the plane
/// `n` (`n.xyz`, then `d` at `+0xC`) and `t = d - (n.z * p.z + (p.x * n.x
/// + p.y * n.y))`: `out = n.xyz * t` (component by component, each
/// stored), then `out += p`, each read back from `out` (so `p = n t + p`
/// for a unit `n`: `p`'s foot on the plane).
///
/// Leaves `f2 = n.x`, `f0` = the dot, `f12 = t`, and `f4`..`f18` the last
/// loads and sums.
///
/// Domain: canonical pointers; `out` disjoint from `n`, equal to `p` or
/// disjoint from it (it is re-read after the stores); no NaN operands.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800819A4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    // f12 = t.
    f[4].set_u32l(lw(m, g[A0], 8) as u32);
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[16].set_u32l(lw(m, g[A0], 4) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[2].set_u32l(lw(m, g[A0], 0) as u32);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[16].set_u32l(lw(m, g[A0], 0xC) as u32);
    f[6].set_fl(f[4].fl() * f[2].fl());
    f[10].set_fl(f[6].fl() + f[18].fl());
    f[0].set_fl(f[8].fl() + f[10].fl());
    f[12].set_fl(f[16].fl() - f[0].fl());
    // out = n t, then out += p.
    f[4].set_fl(f[2].fl() * f[12].fl());
    sw(m, g[A2], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 4) as u32);
    f[16].set_u32l(lw(m, g[A2], 0) as u32);
    f[18].set_fl(f[6].fl() * f[12].fl());
    sw(m, g[A2], 4, u64::from(f[18].u32l()));
    f[8].set_u32l(lw(m, g[A0], 8) as u32);
    f[18].set_u32l(lw(m, g[A2], 4) as u32);
    f[10].set_fl(f[8].fl() * f[12].fl());
    sw(m, g[A2], 8, u64::from(f[10].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_fl(f[16].fl() + f[4].fl());
    f[16].set_u32l(lw(m, g[A2], 8) as u32);
    sw(m, g[A2], 0, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 4) as u32);
    f[10].set_fl(f[18].fl() + f[8].fl());
    sw(m, g[A2], 4, u64::from(f[10].u32l()));
    f[4].set_u32l(lw(m, g[A1], 8) as u32);
    f[6].set_fl(f[16].fl() + f[4].fl());
    sw(m, g[A2], 8, u64::from(f[6].u32l()));
}

/// `func_80081A2C(p, a, b, out)` (closest point on a segment): with `d = b
/// - a` (spilled to `sp - 0x10..-0x8`), `pd = p.z d.z + (p.x d.x + p.y
/// d.y)`, `ad = d.z a.z + (a.x d.x + a.y d.y)` and `dd = d.z d.z + (d.x d.x
/// + d.y d.y)`:
/// - if `f64(dd) <= E` (the double at `0x800ADCC0`): `out = a`;
/// - else with `u = pd - ad` and `t = u / dd` (spilled, read back): `out =
///   a` if `t <= 0`, `b` if `1 <= t`, else `out = d * (u / dd)` (the
///   quotient recomputed) stored, then `out += a` read back.
///
/// Copies are word moves (`a.x` from its register, loaded before the
/// test). The frame (`sp - 0x38`) also holds `d.x` and `d.z` at `+0`, `+4`
/// (then `d.z * a.z` at `+0`), and `t` at `+0xC`.
///
/// Leaves `f0 = a.x` (or the quotient), `f2 = pd`, `f12 = ad`, `f14 =
/// f16 = dd`, `f8 = f64(dd)` and `f10 = E` on the first test's way, and
/// `f4`..`f18` from the path taken.
///
/// Domain: canonical pointers; `out` disjoint from `a`, `b`, `p` (it is
/// re-read); no NaN operands (`dd` included: it is widened).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80081A2C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    // d = b - a at sp + 0x28..
    f[6].set_u32l(lw(m, g[A2], 0) as u32);
    f[8].set_u32l(lw(m, g[A1], 0) as u32);
    g[SP] = addu(g[SP], (-0x38i64) as u64);
    g[AT] = li(0x800B_0000);
    f[4].set_fl(f[6].fl() - f[8].fl());
    sw(m, g[SP], 0x28, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    f[10].set_u32l(lw(m, g[A2], 4) as u32);
    f[8].set_fl(f[10].fl() - f[6].fl());
    sw(m, g[SP], 0x2C, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A1], 8) as u32);
    f[4].set_u32l(lw(m, g[A2], 8) as u32);
    f[6].set_fl(f[4].fl() - f[10].fl());
    f[4].set_u32l(lw(m, g[SP], 0x28) as u32);
    sw(m, g[SP], 0x30, u64::from(f[6].u32l()));
    // f2 = pd, f12 = ad, f16 = dd.
    f[8].set_u32l(lw(m, g[A0], 0) as u32);
    f[6].set_u32l(lw(m, g[A0], 4) as u32);
    sw(m, g[SP], 0, u64::from(f[4].u32l()));
    f[10].set_fl(f[8].fl() * f[4].fl());
    f[8].set_u32l(lw(m, g[SP], 0x2C) as u32);
    f[4].set_u32l(lw(m, g[A0], 8) as u32);
    f[0].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_fl(f[6].fl() * f[8].fl());
    f[10].set_fl(f[10].fl() + f[6].fl());
    f[6].set_u32l(lw(m, g[SP], 0x30) as u32);
    f[4].set_fl(f[6].fl() * f[4].fl());
    f[2].set_fl(f[4].fl() + f[10].fl());
    f[4].set_u32l(lw(m, g[A1], 8) as u32);
    f[10].set_fl(f[6].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[A1], 4) as u32);
    sw(m, g[SP], 4, u64::from(f[6].u32l()));
    f[6].set_u32l(lw(m, g[SP], 0) as u32);
    f[4].set_fl(f[4].fl() * f[8].fl());
    sw(m, g[SP], 0, u64::from(f[10].u32l()));
    f[10].set_fl(f[0].fl() * f[6].fl());
    f[10].set_fl(f[10].fl() + f[4].fl());
    f[4].set_u32l(lw(m, g[SP], 0) as u32);
    f[12].set_fl(f[4].fl() + f[10].fl());
    f[4].set_fl(f[6].fl() * f[6].fl());
    f[10].set_fl(f[8].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[SP], 4) as u32);
    f[6].set_fl(f[4].fl() + f[10].fl());
    f[4].set_fl(f[8].fl() * f[8].fl());
    f[10].u64 = ld(m, g[AT], -0x2340);
    f[16].set_fl(f[4].fl() + f[6].fl());
    f[8].set_d(f64::from(f[16].fl()));
    f[14].set_u32l(f[16].u32l());
    'done: {
        if !(f[8].d() <= f[10].d()) {
            // t = (pd - ad) / dd, spilled at sp + 0xC and read back.
            f[18].set_fl(f[2].fl() - f[12].fl());
            f[8].set_u32l(0);
            g[AT] = li(0x3F80_0000);
            f[10].set_fl(f[18].fl() / f[16].fl());
            sw(m, g[SP], 0xC, u64::from(f[10].u32l()));
            f[4].set_u32l(lw(m, g[SP], 0xC) as u32);
            let nonpositive = f[4].fl() <= f[8].fl();
            f[8].set_u32l(lw(m, g[SP], 0xC) as u32);
            if !nonpositive {
                f[4].set_u32l(g[AT] as u32);
                if f[4].fl() <= f[8].fl() {
                    // Past b.
                    f[6].set_u32l(lw(m, g[A2], 0) as u32);
                    sw(m, g[A3], 0, u64::from(f[6].u32l()));
                    f[10].set_u32l(lw(m, g[A2], 4) as u32);
                    sw(m, g[A3], 4, u64::from(f[10].u32l()));
                    f[8].set_u32l(lw(m, g[A2], 8) as u32);
                    sw(m, g[A3], 8, u64::from(f[8].u32l()));
                    break 'done;
                }
                // Between: out = d * t, then out += a.
                f[0].set_fl(f[18].fl() / f[14].fl());
                f[4].set_u32l(lw(m, g[SP], 0x28) as u32);
                f[6].set_fl(f[4].fl() * f[0].fl());
                sw(m, g[A3], 0, u64::from(f[6].u32l()));
                f[10].set_u32l(lw(m, g[SP], 0x2C) as u32);
                f[8].set_fl(f[10].fl() * f[0].fl());
                sw(m, g[A3], 4, u64::from(f[8].u32l()));
                f[4].set_u32l(lw(m, g[SP], 0x30) as u32);
                f[8].set_u32l(lw(m, g[A3], 0) as u32);
                f[6].set_fl(f[4].fl() * f[0].fl());
                sw(m, g[A3], 8, u64::from(f[6].u32l()));
                f[10].set_u32l(lw(m, g[A1], 0) as u32);
                f[4].set_fl(f[10].fl() + f[8].fl());
                f[10].set_u32l(lw(m, g[A3], 4) as u32);
                sw(m, g[A3], 0, u64::from(f[4].u32l()));
                f[6].set_u32l(lw(m, g[A1], 4) as u32);
                f[8].set_fl(f[6].fl() + f[10].fl());
                f[6].set_u32l(lw(m, g[A3], 8) as u32);
                sw(m, g[A3], 4, u64::from(f[8].u32l()));
                f[4].set_u32l(lw(m, g[A1], 8) as u32);
                f[10].set_fl(f[4].fl() + f[6].fl());
                sw(m, g[A3], 8, u64::from(f[10].u32l()));
                break 'done;
            }
            // Before a.
            sw(m, g[A3], 0, u64::from(f[0].u32l()));
            f[6].set_u32l(lw(m, g[A1], 4) as u32);
            sw(m, g[A3], 4, u64::from(f[6].u32l()));
            f[10].set_u32l(lw(m, g[A1], 8) as u32);
            sw(m, g[A3], 8, u64::from(f[10].u32l()));
            break 'done;
        }
        // A degenerate segment.
        sw(m, g[A3], 0, u64::from(f[0].u32l()));
        f[4].set_u32l(lw(m, g[A1], 4) as u32);
        sw(m, g[A3], 4, u64::from(f[4].u32l()));
        f[6].set_u32l(lw(m, g[A1], 8) as u32);
        sw(m, g[A3], 8, u64::from(f[6].u32l()));
    }
    g[SP] = addu(g[SP], 0x38);
}

/// `func_800823A4(n, p)`: the plane's value at `p`, `f0 = d - (n.z p.z +
/// (p.x n.x + p.y n.y))` (`n.xyz`, then `d` at `+0xC`).
///
/// Leaves `f2` = the dot, `f4` the partial sum, `f8 = d`, and the products
/// and loads in `f6`..`f18`.
///
/// Domain: canonical pointers; no NaN operands.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800823A4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[A0], 0) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[16].set_u32l(lw(m, g[A0], 4) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A0], 8) as u32);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[10].set_u32l(lw(m, g[A1], 8) as u32);
    f[16].set_fl(f[6].fl() * f[10].fl());
    f[4].set_fl(f[8].fl() + f[18].fl());
    f[8].set_u32l(lw(m, g[A0], 0xC) as u32);
    f[2].set_fl(f[16].fl() + f[4].fl());
    f[0].set_fl(f[8].fl() - f[2].fl());
}

/// `func_800823DC(n, p)`: the plane's height over `p`'s x, y: `f0 = p.z +
/// (d - (n.z p.z + (p.x n.x + p.y n.y))) / n.z`.
///
/// Leaves `f12 = n.z`, `f14 = p.z`, `f18 = d`, `f2` the dot, `f8` the
/// difference, `f16` the quotient, `f4`..`f10` the products and sums.
///
/// Domain: canonical pointers; no NaN operands (`n.z = 0` gives an
/// infinity, or a NaN result from `0 / 0`, which is added: outside).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800823DC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[A0], 0) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[18].set_u32l(lw(m, g[A0], 4) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    f[12].set_u32l(lw(m, g[A0], 8) as u32);
    f[14].set_u32l(lw(m, g[A1], 8) as u32);
    f[4].set_fl(f[10].fl() * f[18].fl());
    f[18].set_u32l(lw(m, g[A0], 0xC) as u32);
    f[10].set_fl(f[12].fl() * f[14].fl());
    f[6].set_fl(f[8].fl() + f[4].fl());
    f[2].set_fl(f[10].fl() + f[6].fl());
    f[8].set_fl(f[18].fl() - f[2].fl());
    f[16].set_fl(f[8].fl() / f[12].fl());
    f[0].set_fl(f[14].fl() + f[16].fl());
}

/// `func_8008241C(a, b)`: the 2D cross product `f0 = a.x b.y - b.x a.y`.
///
/// Leaves `f4 = a.x`, `f6 = b.y`, `f10 = b.x`, `f16 = a.y`, `f8`/`f18` the
/// products.
///
/// Domain: canonical pointers; no NaN operands.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008241C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A0], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    f[10].set_u32l(lw(m, g[A1], 0) as u32);
    f[16].set_u32l(lw(m, g[A0], 4) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[0].set_fl(f[8].fl() - f[18].fl());
}

/// `func_8008681C(out, m)` (inverse of a rotation plus translation, with
/// y and z swapped, **guess**): 4x4 row-major. `out`'s 3x3 is `m`'s
/// transposed with its second row negated into the third column and its
/// third row into the second: `out[i][0] = m[0][i]`, `out[i][2] =
/// -m[1][i]`, `out[i][1] = m[2][i]`; then `out[3][j] = -(out[2][j] t.z +
/// (t.x out[0][j] + t.y out[1][j]))` with `t = m[3]` (reading `out` back),
/// and the fourth column copied from `m`. Stores in the C's order.
///
/// Leaves `f4`..`f18` from the last steps.
///
/// Domain: canonical pointers, `out` disjoint from `m`; no NaN operands
/// (the negated entries included).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008681C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    sw(m, g[A0], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    sw(m, g[A0], 0x10, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 8) as u32);
    sw(m, g[A0], 0x20, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[16].set_fl(-f[10].fl());
    sw(m, g[A0], 8, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[4].set_fl(-f[18].fl());
    sw(m, g[A0], 0x18, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[8].set_fl(-f[6].fl());
    f[6].set_u32l(lw(m, g[A0], 0) as u32);
    sw(m, g[A0], 0x28, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x20) as u32);
    sw(m, g[A0], 4, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A1], 0x24) as u32);
    sw(m, g[A0], 0x14, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0x28) as u32);
    f[16].set_u32l(lw(m, g[A0], 0x10) as u32);
    sw(m, g[A0], 0x24, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x30) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x34) as u32);
    f[8].set_fl(f[4].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A0], 0x20) as u32);
    f[18].set_fl(f[10].fl() * f[16].fl());
    f[10].set_u32l(lw(m, g[A1], 0x38) as u32);
    f[16].set_fl(f[6].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A0], 4) as u32);
    f[4].set_fl(f[8].fl() + f[18].fl());
    f[8].set_fl(f[16].fl() + f[4].fl());
    f[18].set_fl(-f[8].fl());
    f[8].set_u32l(lw(m, g[A0], 0x14) as u32);
    sw(m, g[A0], 0x30, u64::from(f[18].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x30) as u32);
    f[4].set_u32l(lw(m, g[A1], 0x34) as u32);
    f[16].set_fl(f[6].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A0], 0x24) as u32);
    f[18].set_fl(f[4].fl() * f[8].fl());
    f[4].set_u32l(lw(m, g[A1], 0x38) as u32);
    f[8].set_fl(f[10].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[A0], 8) as u32);
    f[6].set_fl(f[16].fl() + f[18].fl());
    f[16].set_fl(f[8].fl() + f[6].fl());
    f[18].set_fl(-f[16].fl());
    f[16].set_u32l(lw(m, g[A0], 0x18) as u32);
    sw(m, g[A0], 0x34, u64::from(f[18].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x30) as u32);
    f[6].set_u32l(lw(m, g[A1], 0x34) as u32);
    f[8].set_fl(f[10].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[A0], 0x28) as u32);
    f[18].set_fl(f[6].fl() * f[16].fl());
    f[6].set_u32l(lw(m, g[A1], 0x38) as u32);
    f[16].set_fl(f[4].fl() * f[6].fl());
    f[10].set_fl(f[8].fl() + f[18].fl());
    f[8].set_fl(f[16].fl() + f[10].fl());
    f[18].set_fl(-f[8].fl());
    sw(m, g[A0], 0x38, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0xC) as u32);
    sw(m, g[A0], 0xC, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x1C) as u32);
    sw(m, g[A0], 0x1C, u64::from(f[6].u32l()));
    f[16].set_u32l(lw(m, g[A1], 0x2C) as u32);
    sw(m, g[A0], 0x2C, u64::from(f[16].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x3C) as u32);
    sw(m, g[A0], 0x3C, u64::from(f[10].u32l()));
}
