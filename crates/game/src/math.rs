//! Vector maths (single precision), register-exact ports.
//!
//! Vectors are consecutive `f32`s in RDRAM. Float arithmetic follows
//! N64Recomp's C: IEEE single precision on the host, round to nearest, with
//! subnormals (the hardware flushes them: FCR31.FS is set at boot). NaN
//! operands are outside every port's domain here: the oracle asserts on them
//! (`NAN_CHECK`), and the hardware would trap (FCR31.EV). NOTES.md, "Floats".

// Ports keep N64Recomp's names (func_8001514C), capitals included.
#![allow(non_snake_case)]

use crate::recomp::{addu, enter, ld, li, lw, reg::*, sd, sw, RecompContext};

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
