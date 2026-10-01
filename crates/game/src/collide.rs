//! Collision queries (**guess** at the subsystem): a sphere (centre
//! `0x800AE908`, radius `0x800AE8E0`) and a plane pair tested against
//! geometry in model space, the best hit kept in [`TRACKED`] (a squared
//! distance, a point and a direction). [`func_80004000`] sets a query up;
//! the others move it into the space of the matrix stack's top and back,
//! and test triangle edges. The depth-0 box rejects and the record step are
//! still in `misc` ([`crate::misc::func_80002BD4`],
//! [`crate::misc::func_8000097C`]).

// Ports keep N64Recomp's names (func_80000B00), capitals included.
#![allow(non_snake_case)]

use crate::imports;
pub use crate::misc::TRACKED;
use crate::recomp::{addu, call, enter, ld, lh, li, lw, reg::*, s32, sd, sh, sll, subu, sw, RecompContext};

/// The query's mode (s16): 3 when [`func_80004000`] set up a plane pair, 2
/// when the direction isn't transformed back ([`func_80003B44`]).
pub const MODE: u32 = 0x800A_E934;

/// `func_80000B00(p, a, b, c, q, r)` (nearest point of a triangle's edges,
/// **guess**): `q` and `r` are stack arguments. With `k1`, `k2`, `k3` the
/// points of the edges `a..b`, `b..c`, `c..a` closest to `p`
/// ([`func_80081A2C`](crate::math::func_80081A2C)) and `D(k) = dz*dz +
/// (dx*dx + dy*dy)` for `d = q - k`, it keeps `k1`, then `k2` or `k3` if
/// strictly nearer (`D < best`), and if `D <= T` (`T = [TRACKED]`) calls
/// `func_8000097C(_, q, k, r)` ([`crate::misc::func_8000097C`]) with `t =
/// D` in `f12`, which may record `t`, `k` and `r`.
///
/// Frame (`sp - 0x88`): `ra`, `s2`, `s1`, `s0` at `+0x2C..+0x20`, `f20`
/// (64-bit) at `+0x18`, restored; `p` spilled to its home slot `sp + 0`.
/// `b - a`, `c - b` and `a - c` go to `+0x74`, `+0x68`, `+0x5C` first
/// (dead: `k1`..`k3` overwrite them); the nearest point to `+0x50`, `q`'s
/// components (reloaded from `q` for each winner) to `+0x48` (x), `+0x44`
/// (y), `+0x4C` (z); `k.z` and `d.y` spills at `+0x30`, `+0x38`. Leaves
/// `a1 = q`, `a2` = the nearest point's frame slot, `a3 = r`, and the
/// callees' or the last candidate's registers.
///
/// Domain: canonical pointers to 12 bytes, not overlapping the frame; no NaN
/// operand of the differences, products and sums, nor of the callees'
/// arithmetic (their domains).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80000B00(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    // The three edge vectors (dead stores), then k1, k2, k3.
    g[SP] = addu(g[SP], (-0x88i64) as u64);
    sw(m, g[SP], 0x2C, g[RA]);
    sw(m, g[SP], 0x28, g[S2]);
    sw(m, g[SP], 0x24, g[S1]);
    sw(m, g[SP], 0x20, g[S0]);
    sd(m, g[SP], 0x18, ctx.fpr[20].u64);
    sw(m, g[SP], 0x88, g[A0]);
    ctx.fpr[4].set_u32l(lw(m, g[A1], 0) as u32);
    ctx.fpr[10].set_u32l(lw(m, g[A2], 0) as u32);
    g[S2] = g[A3];
    g[S0] = g[A2];
    ctx.fpr[8].set_fl(ctx.fpr[10].fl() - ctx.fpr[4].fl());
    g[S1] = g[A1];
    sw(m, g[SP], 0x74, u64::from(ctx.fpr[8].u32l()));
    ctx.fpr[10].set_u32l(lw(m, g[A1], 4) as u32);
    ctx.fpr[6].set_u32l(lw(m, g[A2], 4) as u32);
    ctx.fpr[4].set_fl(ctx.fpr[6].fl() - ctx.fpr[10].fl());
    sw(m, g[SP], 0x78, u64::from(ctx.fpr[4].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[A1], 8) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[A2], 8) as u32);
    ctx.fpr[10].set_fl(ctx.fpr[8].fl() - ctx.fpr[6].fl());
    sw(m, g[SP], 0x7C, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[4].set_u32l(lw(m, g[A3], 0) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[A2], 0) as u32);
    ctx.fpr[6].set_fl(ctx.fpr[4].fl() - ctx.fpr[8].fl());
    sw(m, g[SP], 0x68, u64::from(ctx.fpr[6].u32l()));
    ctx.fpr[10].set_u32l(lw(m, g[A3], 4) as u32);
    ctx.fpr[4].set_u32l(lw(m, g[A2], 4) as u32);
    ctx.fpr[8].set_fl(ctx.fpr[10].fl() - ctx.fpr[4].fl());
    sw(m, g[SP], 0x6C, u64::from(ctx.fpr[8].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[A3], 8) as u32);
    ctx.fpr[10].set_u32l(lw(m, g[A2], 8) as u32);
    ctx.fpr[4].set_fl(ctx.fpr[6].fl() - ctx.fpr[10].fl());
    sw(m, g[SP], 0x70, u64::from(ctx.fpr[4].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[A3], 0) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[A1], 0) as u32);
    ctx.fpr[10].set_fl(ctx.fpr[8].fl() - ctx.fpr[6].fl());
    sw(m, g[SP], 0x5C, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[8].set_u32l(lw(m, g[A3], 4) as u32);
    ctx.fpr[4].set_u32l(lw(m, g[A1], 4) as u32);
    ctx.fpr[6].set_fl(ctx.fpr[4].fl() - ctx.fpr[8].fl());
    sw(m, g[SP], 0x60, u64::from(ctx.fpr[6].u32l()));
    ctx.fpr[4].set_u32l(lw(m, g[A3], 8) as u32);
    ctx.fpr[10].set_u32l(lw(m, g[A1], 8) as u32);
    g[A3] = addu(g[SP], 0x74);
    ctx.fpr[8].set_fl(ctx.fpr[10].fl() - ctx.fpr[4].fl());
    sw(m, g[SP], 0x64, u64::from(ctx.fpr[8].u32l()));
    call(imports::func_80081A2C, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = lw(m, g[SP], 0x88);
    g[A1] = g[S0];
    g[A2] = g[S2];
    g[A3] = addu(g[SP], 0x68);
    call(imports::func_80081A2C, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = lw(m, g[SP], 0x88);
    g[A1] = g[S2];
    g[A2] = g[S1];
    g[A3] = addu(g[SP], 0x5C);
    call(imports::func_80081A2C, m, ctx);
    let g = &mut ctx.gpr;
    // f20 = D(k1), the nearest so far at +0x50; q's components to the frame.
    g[A1] = lw(m, g[SP], 0x98);
    ctx.fpr[10].set_u32l(lw(m, g[SP], 0x7C) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[SP], 0x74) as u32);
    ctx.fpr[6].set_u32l(lw(m, g[A1], 8) as u32);
    ctx.fpr[4].set_u32l(lw(m, g[A1], 0) as u32);
    g[AT] = li(0x800B_0000);
    ctx.fpr[0].set_fl(ctx.fpr[6].fl() - ctx.fpr[10].fl());
    ctx.fpr[6].set_u32l(lw(m, g[A1], 4) as u32);
    sw(m, g[SP], 0x30, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[2].set_fl(ctx.fpr[4].fl() - ctx.fpr[8].fl());
    ctx.fpr[4].set_u32l(lw(m, g[SP], 0x78) as u32);
    sw(m, g[SP], 0x50, u64::from(ctx.fpr[8].u32l()));
    g[A2] = addu(g[SP], 0x50);
    ctx.fpr[12].set_fl(ctx.fpr[6].fl() - ctx.fpr[4].fl());
    ctx.fpr[6].set_fl(ctx.fpr[2].fl() * ctx.fpr[2].fl());
    sw(m, g[SP], 0x54, u64::from(ctx.fpr[4].u32l()));
    ctx.fpr[4].set_u32l(lw(m, g[SP], 0x70) as u32);
    ctx.fpr[10].set_fl(ctx.fpr[12].fl() * ctx.fpr[12].fl());
    ctx.fpr[6].set_fl(ctx.fpr[6].fl() + ctx.fpr[10].fl());
    ctx.fpr[10].set_fl(ctx.fpr[0].fl() * ctx.fpr[0].fl());
    ctx.fpr[20].set_fl(ctx.fpr[10].fl() + ctx.fpr[6].fl());
    ctx.fpr[10].set_u32l(lw(m, g[SP], 0x30) as u32);
    sw(m, g[SP], 0x58, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[A1], 8) as u32);
    sw(m, g[SP], 0x4C, u64::from(ctx.fpr[6].u32l()));
    ctx.fpr[8].set_u32l(lw(m, g[SP], 0x4C) as u32);
    ctx.fpr[10].set_u32l(lw(m, g[A1], 0) as u32);
    ctx.fpr[16].set_fl(ctx.fpr[8].fl() - ctx.fpr[4].fl());
    sw(m, g[SP], 0x48, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[SP], 0x48) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[SP], 0x68) as u32);
    ctx.fpr[10].set_u32l(lw(m, g[A1], 4) as u32);
    ctx.fpr[18].set_fl(ctx.fpr[6].fl() - ctx.fpr[8].fl());
    sw(m, g[SP], 0x44, u64::from(ctx.fpr[10].u32l()));
    // f14 = D(k2), d.y spilled to +0x38 and squared from two reloads.
    ctx.fpr[6].set_u32l(lw(m, g[SP], 0x44) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[SP], 0x6C) as u32);
    ctx.fpr[10].set_fl(ctx.fpr[6].fl() - ctx.fpr[8].fl());
    sw(m, g[SP], 0x38, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[SP], 0x38) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[SP], 0x38) as u32);
    ctx.fpr[10].set_fl(ctx.fpr[6].fl() * ctx.fpr[8].fl());
    ctx.fpr[6].set_fl(ctx.fpr[18].fl() * ctx.fpr[18].fl());
    ctx.fpr[8].set_fl(ctx.fpr[6].fl() + ctx.fpr[10].fl());
    ctx.fpr[6].set_fl(ctx.fpr[16].fl() * ctx.fpr[16].fl());
    ctx.fpr[10].set_u32l(lw(m, g[SP], 0x68) as u32);
    ctx.fpr[14].set_fl(ctx.fpr[6].fl() + ctx.fpr[8].fl());
    if ctx.fpr[14].fl() < ctx.fpr[20].fl() {
        ctx.fpr[6].set_u32l(lw(m, g[SP], 0x6C) as u32);
        sw(m, g[SP], 0x50, u64::from(ctx.fpr[10].u32l()));
        sw(m, g[SP], 0x58, u64::from(ctx.fpr[4].u32l()));
        sw(m, g[SP], 0x54, u64::from(ctx.fpr[6].u32l()));
        ctx.fpr[8].set_u32l(lw(m, g[A1], 8) as u32);
        ctx.fpr[20].set_u32l(ctx.fpr[14].u32l());
        sw(m, g[SP], 0x4C, u64::from(ctx.fpr[8].u32l()));
        ctx.fpr[10].set_u32l(lw(m, g[A1], 0) as u32);
        sw(m, g[SP], 0x48, u64::from(ctx.fpr[10].u32l()));
        ctx.fpr[6].set_u32l(lw(m, g[A1], 4) as u32);
        sw(m, g[SP], 0x44, u64::from(ctx.fpr[6].u32l()));
    }
    // f14 = D(k3).
    ctx.fpr[4].set_u32l(lw(m, g[SP], 0x4C) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[SP], 0x64) as u32);
    ctx.fpr[10].set_u32l(lw(m, g[SP], 0x48) as u32);
    ctx.fpr[6].set_u32l(lw(m, g[SP], 0x5C) as u32);
    ctx.fpr[0].set_fl(ctx.fpr[4].fl() - ctx.fpr[8].fl());
    ctx.fpr[4].set_u32l(lw(m, g[SP], 0x44) as u32);
    sw(m, g[SP], 0x30, u64::from(ctx.fpr[8].u32l()));
    ctx.fpr[2].set_fl(ctx.fpr[10].fl() - ctx.fpr[6].fl());
    ctx.fpr[10].set_u32l(lw(m, g[SP], 0x60) as u32);
    ctx.fpr[12].set_fl(ctx.fpr[4].fl() - ctx.fpr[10].fl());
    ctx.fpr[4].set_fl(ctx.fpr[2].fl() * ctx.fpr[2].fl());
    ctx.fpr[8].set_fl(ctx.fpr[12].fl() * ctx.fpr[12].fl());
    ctx.fpr[4].set_fl(ctx.fpr[4].fl() + ctx.fpr[8].fl());
    ctx.fpr[8].set_fl(ctx.fpr[0].fl() * ctx.fpr[0].fl());
    ctx.fpr[14].set_fl(ctx.fpr[8].fl() + ctx.fpr[4].fl());
    if ctx.fpr[14].fl() < ctx.fpr[20].fl() {
        ctx.fpr[8].set_u32l(lw(m, g[SP], 0x30) as u32);
        sw(m, g[SP], 0x50, u64::from(ctx.fpr[6].u32l()));
        sw(m, g[SP], 0x54, u64::from(ctx.fpr[10].u32l()));
        ctx.fpr[20].set_u32l(ctx.fpr[14].u32l());
        sw(m, g[SP], 0x58, u64::from(ctx.fpr[8].u32l()));
    }
    // Offer the nearest to the record step if D <= T.
    ctx.fpr[4].set_u32l(lw(m, g[AT], -0x1750) as u32);
    g[A3] = lw(m, g[SP], 0x9C);
    if ctx.fpr[20].fl() <= ctx.fpr[4].fl() {
        ctx.fpr[12].set_u32l(ctx.fpr[20].u32l());
        call(imports::func_8000097C, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x2C);
    ctx.fpr[20].u64 = ld(m, g[SP], 0x18);
    g[S0] = lw(m, g[SP], 0x20);
    g[S1] = lw(m, g[SP], 0x24);
    g[S2] = lw(m, g[SP], 0x28);
    g[SP] = addu(g[SP], 0x88);
}

/// `func_80001E80(plane, a, b, c, ray)` (the query's ray against a
/// triangle, **guess**): `plane` is the triangle's `n` then `d` (4 floats),
/// `a`, `b`, `c` its corners, `ray` (on the stack) is `o`, `v` and a length
/// (7 floats, as [`func_80001D34`](crate::math::func_80001D34) takes them).
/// With `e = n.z*v.z + (v.x*n.x + v.y*n.y)` (in that order, not fused): if
/// `0 < e` it needs the word `[0x8009A278]` nonzero (a back face, `back =
/// 1`), otherwise `[0x8009A274]` (`back = 0`); if that word is 0 nothing
/// happens. Then `t` = [`func_80001D34`](crate::math::func_80001D34)`(plane,
/// ray, h)` (`h` a frame point); nothing more unless `0 <= t` and `t < T`
/// (`T = [TRACKED]`). With the edges `b - a`, `c - b`, `a - c`, if
/// [`func_800005B4`](crate::math::func_800005B4)`(h, a, b, c, ..)` says `h`
/// is inside: `[0x800AEC7C] = 1`, `[TRACKED] = t`, `TRACKED + 8 = h`,
/// `[0x800AE8D8] = [0x800AE938]`, and `TRACKED + 0x18 = n`, or `-n` for a
/// back face (each component loaded just before its store).
///
/// Frame (`sp - 0x78`): `ra`, `s2`, `s1`, `s0` at `+0x34..+0x28`
/// (`s0 = plane`, `s1 = b`, `s2 = a`), restored sign-extended; `back` at
/// `+0x38` around both calls, `c` spilled to its home slot `+0x84` and
/// re-read, `t` at `+0x40` around the second call; `h` at `+0x48`, the
/// edges at `+0x6C`, `+0x60`, `+0x54` (each component stored after its
/// subtraction), and their addresses as [`func_800005B4`]'s stack
/// arguments at `+0x10..+0x18`. Leaves `t6 = ray`, `t7`/`t8` `0x800A0000`
/// or the flag word, `v1 = back`, `a0 = plane`, `a1 = ray`, `a2 = h`, `f8
/// = 0.0`, `f0 = e` when it stops at the flags; after the first call `f2 =
/// t`, `f18 = 0.0`, `at = 0x800B0000`, `f6 = T` past the first compare;
/// the edge temporaries, `t9`, `t0`, `t1` and the second callee's
/// registers after it; on a hit `t2 = 1`, `t3` the copied word, `v0 =
/// 0x800AE8C8` and `f4`..`f18` from the copies.
///
/// Domain: canonical pointers (16, 12, 12, 12 and 28 bytes) not overlapping
/// the frame or [`TRACKED`]; no NaN operand of the products and sums of `e`,
/// of the edge differences or of a negation (back faces); the callees'
/// domains. `T`, `t` and `e` are only compared.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80001E80(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    'done: {
        g[SP] = addu(g[SP], (-0x78i64) as u64);
        g[T6] = lw(m, g[SP], 0x88);
        sw(m, g[SP], 0x34, g[RA]);
        sw(m, g[SP], 0x30, g[S2]);
        sw(m, g[SP], 0x2C, g[S1]);
        sw(m, g[SP], 0x28, g[S0]);
        // e = n.z*v.z + (v.x*n.x + v.y*n.y)
        f[6].set_u32l(lw(m, g[A0], 0) as u32);
        f[4].set_u32l(lw(m, g[T6], 0xC) as u32);
        f[16].set_u32l(lw(m, g[A0], 4) as u32);
        f[10].set_u32l(lw(m, g[T6], 0x10) as u32);
        f[8].set_fl(f[4].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A0], 8) as u32);
        g[S0] = g[A0];
        f[18].set_fl(f[10].fl() * f[16].fl());
        f[10].set_u32l(lw(m, g[T6], 0x14) as u32);
        g[S1] = g[A2];
        g[S2] = g[A1];
        f[16].set_fl(f[6].fl() * f[10].fl());
        g[V1] = 0;
        g[T7] = li(0x800A_0000);
        g[T8] = li(0x800A_0000);
        g[A1] = lw(m, g[SP], 0x88);
        f[4].set_fl(f[8].fl() + f[18].fl());
        f[8].set_u32l(0);
        g[A2] = addu(g[SP], 0x48);
        g[A0] = g[S0];
        f[0].set_fl(f[16].fl() + f[4].fl());
        if f[8].fl() < f[0].fl() {
            g[T7] = lw(m, g[T7], -0x5D88);
            if g[T7] == 0 {
                break 'done;
            }
            g[V1] = 1;
        } else {
            g[T8] = lw(m, g[T8], -0x5D8C);
            if g[T8] == 0 {
                break 'done;
            }
        }
        sw(m, g[SP], 0x38, g[V1]);
        sw(m, g[SP], 0x84, g[A3]);
        call(imports::func_80001D34, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        f[18].set_u32l(0);
        g[V1] = lw(m, g[SP], 0x38);
        g[A3] = lw(m, g[SP], 0x84);
        f[2].set_u32l(f[0].u32l());
        g[AT] = li(0x800B_0000);
        if f[0].fl() < f[18].fl() {
            break 'done;
        }
        f[6].set_u32l(lw(m, g[AT], -0x1750) as u32);
        if !(f[0].fl() < f[6].fl()) {
            break 'done;
        }
        // The edges b - a, c - b, a - c.
        f[10].set_u32l(lw(m, g[S1], 0) as u32);
        f[16].set_u32l(lw(m, g[S2], 0) as u32);
        g[T9] = addu(g[SP], 0x6C);
        g[T0] = addu(g[SP], 0x60);
        f[4].set_fl(f[10].fl() - f[16].fl());
        g[T1] = addu(g[SP], 0x54);
        g[A0] = addu(g[SP], 0x48);
        g[A1] = g[S2];
        sw(m, g[SP], 0x6C, u64::from(f[4].u32l()));
        f[18].set_u32l(lw(m, g[S2], 4) as u32);
        f[8].set_u32l(lw(m, g[S1], 4) as u32);
        g[A2] = g[S1];
        f[6].set_fl(f[8].fl() - f[18].fl());
        sw(m, g[SP], 0x70, u64::from(f[6].u32l()));
        f[16].set_u32l(lw(m, g[S2], 8) as u32);
        f[10].set_u32l(lw(m, g[S1], 8) as u32);
        f[4].set_fl(f[10].fl() - f[16].fl());
        sw(m, g[SP], 0x74, u64::from(f[4].u32l()));
        f[8].set_u32l(lw(m, g[A3], 0) as u32);
        f[18].set_u32l(lw(m, g[S1], 0) as u32);
        f[6].set_fl(f[8].fl() - f[18].fl());
        sw(m, g[SP], 0x60, u64::from(f[6].u32l()));
        f[10].set_u32l(lw(m, g[A3], 4) as u32);
        f[16].set_u32l(lw(m, g[S1], 4) as u32);
        f[4].set_fl(f[10].fl() - f[16].fl());
        sw(m, g[SP], 0x64, u64::from(f[4].u32l()));
        f[8].set_u32l(lw(m, g[A3], 8) as u32);
        f[18].set_u32l(lw(m, g[S1], 8) as u32);
        f[6].set_fl(f[8].fl() - f[18].fl());
        sw(m, g[SP], 0x68, u64::from(f[6].u32l()));
        f[16].set_u32l(lw(m, g[A3], 0) as u32);
        f[10].set_u32l(lw(m, g[S2], 0) as u32);
        f[4].set_fl(f[10].fl() - f[16].fl());
        sw(m, g[SP], 0x54, u64::from(f[4].u32l()));
        f[18].set_u32l(lw(m, g[A3], 4) as u32);
        f[8].set_u32l(lw(m, g[S2], 4) as u32);
        f[6].set_fl(f[8].fl() - f[18].fl());
        sw(m, g[SP], 0x58, u64::from(f[6].u32l()));
        f[16].set_u32l(lw(m, g[A3], 8) as u32);
        f[10].set_u32l(lw(m, g[S2], 8) as u32);
        sw(m, g[SP], 0x40, u64::from(f[2].u32l()));
        sw(m, g[SP], 0x38, g[V1]);
        f[4].set_fl(f[10].fl() - f[16].fl());
        sw(m, g[SP], 0x18, g[T1]);
        sw(m, g[SP], 0x14, g[T0]);
        sw(m, g[SP], 0x10, g[T9]);
        sw(m, g[SP], 0x5C, u64::from(f[4].u32l()));
        call(imports::func_800005B4, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        g[V1] = lw(m, g[SP], 0x38);
        f[2].set_u32l(lw(m, g[SP], 0x40) as u32);
        if g[V0] == 0 {
            break 'done;
        }
        // Record the hit.
        g[T2] = 1;
        g[AT] = li(0x800B_0000);
        sw(m, g[AT], -0x1384, g[T2]);
        g[V0] = li(0x800B_0000);
        g[AT] = li(0x800B_0000);
        f[8].set_u32l(lw(m, g[SP], 0x48) as u32);
        f[18].set_u32l(lw(m, g[SP], 0x4C) as u32);
        f[6].set_u32l(lw(m, g[SP], 0x50) as u32);
        g[V0] = addu(g[V0], (-0x1748i64) as u64);
        sw(m, g[AT], -0x1750, u64::from(f[2].u32l()));
        g[T3] = li(0x800B_0000);
        sw(m, g[V0], 0, u64::from(f[8].u32l()));
        sw(m, g[V0], 4, u64::from(f[18].u32l()));
        sw(m, g[V0], 8, u64::from(f[6].u32l()));
        g[T3] = lw(m, g[T3], -0x16C8);
        g[AT] = li(0x800B_0000);
        sw(m, g[AT], -0x1728, g[T3]);
        f[10].set_u32l(lw(m, g[S0], 0) as u32);
        g[V0] = li(0x800A_E8C8);
        if g[V1] == 0 {
            sw(m, g[V0], 0, u64::from(f[10].u32l()));
            f[16].set_u32l(lw(m, g[S0], 4) as u32);
            sw(m, g[V0], 4, u64::from(f[16].u32l()));
            f[4].set_u32l(lw(m, g[S0], 8) as u32);
            sw(m, g[V0], 8, u64::from(f[4].u32l()));
        } else {
            f[16].set_fl(-f[10].fl());
            sw(m, g[V0], 0, u64::from(f[16].u32l()));
            f[4].set_u32l(lw(m, g[S0], 4) as u32);
            f[8].set_fl(-f[4].fl());
            sw(m, g[V0], 4, u64::from(f[8].u32l()));
            f[18].set_u32l(lw(m, g[S0], 8) as u32);
            f[6].set_fl(-f[18].fl());
            sw(m, g[V0], 8, u64::from(f[6].u32l()));
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x34);
    g[S0] = lw(m, g[SP], 0x28);
    g[S1] = lw(m, g[SP], 0x2C);
    g[S2] = lw(m, g[SP], 0x30);
    g[SP] = addu(g[SP], 0x78);
}

/// `func_800038E8(flags, pout, p, nout, n)` (query into model space,
/// **guess**): `n` is a stack argument, all are vec3 pointers. With `M` the
/// matrix stack's top ([`func_800059A8`](crate::matrix::func_800059A8),
/// frame copy), `T` its translation row, the plane offsets `d1 =
/// [0x800AE96C]`, `d2 = [0x800AE970]` and results `[0x800AE954]`,
/// `[0x800AE958]`, the plane parts done only when [`MODE`] is 3:
/// - flags bit 0 clear: `pout = p`; `nout = n`, results `d1`, `d2`.
/// - bit 0 set, bit 1 clear: `pout = p - T`; `nout = n`, results `d1 - e`
///   and `d2 - e` with `e = T.z*nout.z + (nout.x*T.x + nout.y*T.y)`
///   (computed twice from `nout` read back).
/// - bits 0 and 1 set: with `I = M^-1`
///   ([`func_800160BC`](crate::math::func_800160BC)) and `U` its
///   translation row: `pout = p * I` as a point
///   ([`func_80016CAC`](crate::math::func_80016CAC)); `nout = n * I`'s 3x3
///   part ([`func_80016BF4`](crate::math::func_80016BF4)), results `e + d1`
///   and `e + d2` with `e = U.z*nout.z + (nout.x*U.x + nout.y*U.y)`
///   (twice, as above).
///
/// Copies and differences go component by component (load, store).
/// Frame (`sp - 0xA8`): `ra`, `s2`, `s1`, `s0` at `+0x24..+0x18`, `M` at
/// `+0x68`, `I` at `+0x28`; `flags` spilled to its home slot `sp + 0`.
/// Leaves `a1 = n` once loaded, `a0`, `a2` the last call's arguments,
/// `t*`/`at` from the tests and `f4`..`f18` from the path.
///
/// Domain: canonical pointers to 12 bytes outside the frame, `pout` and
/// `nout` each disjoint from the inputs or equal to them; no NaN operand in
/// the arithmetic here or in the callees'.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800038E8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_80003B30: {
        g[SP] = addu(g[SP], (-0xA8i64) as u64);
        sw(m, g[SP], 0x20, g[S2]);
        sw(m, g[SP], 0x1C, g[S1]);
        sw(m, g[SP], 0x18, g[S0]);
        g[T7] = g[A0] & 1;
        g[S0] = g[A3];
        g[S1] = g[A1];
        g[S2] = g[A2];
        sw(m, g[SP], 0x24, g[RA]);
        sw(m, g[SP], 0xA8, g[A0]);
        if g[T7] == 0 {
            // World space: copies.
            ctx.fpr[4].set_u32l(lw(m, g[S2], 0) as u32);
            g[T2] = li(0x800B_0000);
            g[AT] = 3;
            sw(m, g[S1], 0, u64::from(ctx.fpr[4].u32l()));
            ctx.fpr[8].set_u32l(lw(m, g[S2], 4) as u32);
            sw(m, g[S1], 4, u64::from(ctx.fpr[8].u32l()));
            ctx.fpr[16].set_u32l(lw(m, g[S2], 8) as u32);
            sw(m, g[S1], 8, u64::from(ctx.fpr[16].u32l()));
            g[T2] = lh(m, g[T2], -0x16CC);
            g[A1] = lw(m, g[SP], 0xB8);
            if g[T2] != g[AT] {
                g[RA] = lw(m, g[SP], 0x24);
                break 'b_80003B30;
            }
            ctx.fpr[10].set_u32l(lw(m, g[A1], 0) as u32);
            g[AT] = li(0x800B_0000);
            sw(m, g[S0], 0, u64::from(ctx.fpr[10].u32l()));
            ctx.fpr[6].set_u32l(lw(m, g[A1], 4) as u32);
            sw(m, g[S0], 4, u64::from(ctx.fpr[6].u32l()));
            ctx.fpr[18].set_u32l(lw(m, g[A1], 8) as u32);
            sw(m, g[S0], 8, u64::from(ctx.fpr[18].u32l()));
            ctx.fpr[4].set_u32l(lw(m, g[AT], -0x1694) as u32);
            g[AT] = li(0x800B_0000);
            sw(m, g[AT], -0x16AC, u64::from(ctx.fpr[4].u32l()));
            g[AT] = li(0x800B_0000);
            ctx.fpr[8].set_u32l(lw(m, g[AT], -0x1690) as u32);
            g[AT] = li(0x800B_0000);
            sw(m, g[AT], -0x16A8, u64::from(ctx.fpr[8].u32l()));
        } else {
            g[A0] = addu(g[SP], 0x68);
            call(imports::func_800059A8, m, ctx);
            let g = &mut ctx.gpr;
            g[T8] = lw(m, g[SP], 0xA8);
            g[A0] = addu(g[SP], 0x28);
            ctx.fpr[16].set_u32l(lw(m, g[SP], 0x98) as u32);
            g[T9] = g[T8] & 2;
            if g[T9] == 0 {
                // Translation only: pout = p - T, offsets less nout . T.
                ctx.fpr[4].set_u32l(lw(m, g[S2], 0) as u32);
                g[T1] = li(0x800B_0000);
                g[AT] = 3;
                ctx.fpr[6].set_fl(ctx.fpr[4].fl() - ctx.fpr[16].fl());
                sw(m, g[S1], 0, u64::from(ctx.fpr[6].u32l()));
                ctx.fpr[8].set_u32l(lw(m, g[SP], 0x9C) as u32);
                ctx.fpr[18].set_u32l(lw(m, g[S2], 4) as u32);
                ctx.fpr[10].set_fl(ctx.fpr[18].fl() - ctx.fpr[8].fl());
                sw(m, g[S1], 4, u64::from(ctx.fpr[10].u32l()));
                ctx.fpr[16].set_u32l(lw(m, g[SP], 0xA0) as u32);
                ctx.fpr[4].set_u32l(lw(m, g[S2], 8) as u32);
                ctx.fpr[6].set_fl(ctx.fpr[4].fl() - ctx.fpr[16].fl());
                sw(m, g[S1], 8, u64::from(ctx.fpr[6].u32l()));
                g[T1] = lh(m, g[T1], -0x16CC);
                g[A1] = lw(m, g[SP], 0xB8);
                if g[T1] != g[AT] {
                    g[RA] = lw(m, g[SP], 0x24);
                    break 'b_80003B30;
                }
                ctx.fpr[18].set_u32l(lw(m, g[A1], 0) as u32);
                g[AT] = li(0x800B_0000);
                sw(m, g[S0], 0, u64::from(ctx.fpr[18].u32l()));
                ctx.fpr[8].set_u32l(lw(m, g[A1], 4) as u32);
                ctx.fpr[4].set_u32l(lw(m, g[S0], 0) as u32);
                sw(m, g[S0], 4, u64::from(ctx.fpr[8].u32l()));
                ctx.fpr[10].set_u32l(lw(m, g[A1], 8) as u32);
                ctx.fpr[18].set_u32l(lw(m, g[S0], 4) as u32);
                sw(m, g[S0], 8, u64::from(ctx.fpr[10].u32l()));
                ctx.fpr[16].set_u32l(lw(m, g[SP], 0x98) as u32);
                ctx.fpr[8].set_u32l(lw(m, g[SP], 0x9C) as u32);
                ctx.fpr[6].set_fl(ctx.fpr[4].fl() * ctx.fpr[16].fl());
                ctx.fpr[10].set_fl(ctx.fpr[18].fl() * ctx.fpr[8].fl());
                ctx.fpr[18].set_u32l(lw(m, g[SP], 0xA0) as u32);
                ctx.fpr[4].set_fl(ctx.fpr[6].fl() + ctx.fpr[10].fl());
                ctx.fpr[6].set_u32l(lw(m, g[S0], 8) as u32);
                ctx.fpr[10].set_fl(ctx.fpr[18].fl() * ctx.fpr[6].fl());
                ctx.fpr[6].set_fl(ctx.fpr[10].fl() + ctx.fpr[4].fl());
                ctx.fpr[10].set_u32l(lw(m, g[AT], -0x1694) as u32);
                g[AT] = li(0x800B_0000);
                ctx.fpr[4].set_fl(ctx.fpr[10].fl() - ctx.fpr[6].fl());
                sw(m, g[AT], -0x16AC, u64::from(ctx.fpr[4].u32l()));
                ctx.fpr[10].set_u32l(lw(m, g[S0], 0) as u32);
                ctx.fpr[4].set_u32l(lw(m, g[S0], 4) as u32);
                g[AT] = li(0x800B_0000);
                ctx.fpr[6].set_fl(ctx.fpr[10].fl() * ctx.fpr[16].fl());
                ctx.fpr[10].set_fl(ctx.fpr[4].fl() * ctx.fpr[8].fl());
                ctx.fpr[4].set_u32l(lw(m, g[S0], 8) as u32);
                ctx.fpr[8].set_fl(ctx.fpr[18].fl() * ctx.fpr[4].fl());
                ctx.fpr[16].set_fl(ctx.fpr[6].fl() + ctx.fpr[10].fl());
                ctx.fpr[10].set_u32l(lw(m, g[AT], -0x1690) as u32);
                g[AT] = li(0x800B_0000);
                ctx.fpr[6].set_fl(ctx.fpr[8].fl() + ctx.fpr[16].fl());
                ctx.fpr[18].set_fl(ctx.fpr[10].fl() - ctx.fpr[6].fl());
                sw(m, g[AT], -0x16A8, u64::from(ctx.fpr[18].u32l()));
            } else {
                // Full inverse: pout = p * I, nout = n * I (3x3), offsets
                // plus nout . U.
                g[A1] = addu(g[SP], 0x68);
                call(imports::func_800160BC, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = g[S1];
                g[A1] = g[S2];
                g[A2] = addu(g[SP], 0x28);
                call(imports::func_80016CAC, m, ctx);
                let g = &mut ctx.gpr;
                g[T0] = li(0x800B_0000);
                g[T0] = lh(m, g[T0], -0x16CC);
                g[AT] = 3;
                g[A1] = lw(m, g[SP], 0xB8);
                g[A0] = g[S0];
                if g[T0] == g[AT] {
                    g[A2] = addu(g[SP], 0x28);
                    call(imports::func_80016BF4, m, ctx);
                    let g = &mut ctx.gpr;
                    ctx.fpr[4].set_u32l(lw(m, g[S0], 0) as u32);
                    ctx.fpr[6].set_u32l(lw(m, g[SP], 0x58) as u32);
                    ctx.fpr[10].set_u32l(lw(m, g[S0], 4) as u32);
                    ctx.fpr[16].set_u32l(lw(m, g[SP], 0x5C) as u32);
                    ctx.fpr[8].set_fl(ctx.fpr[4].fl() * ctx.fpr[6].fl());
                    g[AT] = li(0x800B_0000);
                    ctx.fpr[18].set_fl(ctx.fpr[10].fl() * ctx.fpr[16].fl());
                    ctx.fpr[10].set_u32l(lw(m, g[SP], 0x60) as u32);
                    ctx.fpr[4].set_fl(ctx.fpr[8].fl() + ctx.fpr[18].fl());
                    ctx.fpr[8].set_u32l(lw(m, g[S0], 8) as u32);
                    ctx.fpr[18].set_fl(ctx.fpr[10].fl() * ctx.fpr[8].fl());
                    ctx.fpr[8].set_fl(ctx.fpr[18].fl() + ctx.fpr[4].fl());
                    ctx.fpr[18].set_u32l(lw(m, g[AT], -0x1694) as u32);
                    g[AT] = li(0x800B_0000);
                    ctx.fpr[4].set_fl(ctx.fpr[8].fl() + ctx.fpr[18].fl());
                    sw(m, g[AT], -0x16AC, u64::from(ctx.fpr[4].u32l()));
                    ctx.fpr[8].set_u32l(lw(m, g[S0], 0) as u32);
                    ctx.fpr[4].set_u32l(lw(m, g[S0], 4) as u32);
                    g[AT] = li(0x800B_0000);
                    ctx.fpr[18].set_fl(ctx.fpr[8].fl() * ctx.fpr[6].fl());
                    ctx.fpr[8].set_fl(ctx.fpr[4].fl() * ctx.fpr[16].fl());
                    ctx.fpr[4].set_u32l(lw(m, g[S0], 8) as u32);
                    ctx.fpr[16].set_fl(ctx.fpr[10].fl() * ctx.fpr[4].fl());
                    ctx.fpr[6].set_fl(ctx.fpr[18].fl() + ctx.fpr[8].fl());
                    ctx.fpr[8].set_u32l(lw(m, g[AT], -0x1690) as u32);
                    g[AT] = li(0x800B_0000);
                    ctx.fpr[18].set_fl(ctx.fpr[16].fl() + ctx.fpr[6].fl());
                    ctx.fpr[10].set_fl(ctx.fpr[18].fl() + ctx.fpr[8].fl());
                    sw(m, g[AT], -0x16A8, u64::from(ctx.fpr[10].u32l()));
                }
            }
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x24);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0xA8);
}

/// `func_80003B44(flags)` (the query's result back to world space,
/// **guess**): if bit 0 is set, with `M` the matrix stack's top
/// ([`func_800059A8`](crate::matrix::func_800059A8), frame copy at `sp -
/// 0x60 + 0x20`) and `P = TRACKED + 8`: bit 1 clear, `P = T + P`
/// componentwise (`T` = `M`'s translation row, the sum in that order; x is
/// stored after y's sum is computed); bit 1 set, `P = P * M` as a point
/// ([`func_80016CAC`](crate::math::func_80016CAC)) and, unless [`MODE`] is
/// 2, `N = N * M`'s 3x3 part ([`func_80016BF4`](crate::math::func_80016BF4),
/// `N = TRACKED + 0x18`).
///
/// Frame (`sp - 0x60`): `ra` at `+0x14`; `flags` spilled to its home slot
/// `sp + 0` around the first call. Leaves `a1 = flags` (or the callees'),
/// `a0 = P` or `N`, `t7`, `t8`, `at` from the tests, and `f4`..`f18` from
/// the sums.
///
/// Domain: no NaN operand of the sums or the callees' arithmetic.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80003B44(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x60i64) as u64);
    g[T6] = g[A0] & 1;
    sw(m, g[SP], 0x14, g[RA]);
    g[A1] = g[A0];
    if g[T6] != 0 {
        g[A0] = addu(g[SP], 0x20);
        sw(m, g[SP], 0x60, g[A1]);
        call(imports::func_800059A8, m, ctx);
        let g = &mut ctx.gpr;
        g[A1] = lw(m, g[SP], 0x60);
        g[A0] = li(0x800A_E8B8);
        g[T7] = g[A1] & 2;
        ctx.fpr[4].set_u32l(lw(m, g[SP], 0x50) as u32);
        if g[T7] == 0 {
            ctx.fpr[6].set_u32l(lw(m, g[A0], 0) as u32);
            ctx.fpr[10].set_u32l(lw(m, g[SP], 0x54) as u32);
            ctx.fpr[16].set_u32l(lw(m, g[A0], 4) as u32);
            ctx.fpr[8].set_fl(ctx.fpr[4].fl() + ctx.fpr[6].fl());
            ctx.fpr[6].set_u32l(lw(m, g[A0], 8) as u32);
            ctx.fpr[4].set_u32l(lw(m, g[SP], 0x58) as u32);
            ctx.fpr[18].set_fl(ctx.fpr[10].fl() + ctx.fpr[16].fl());
            sw(m, g[A0], 0, u64::from(ctx.fpr[8].u32l()));
            ctx.fpr[8].set_fl(ctx.fpr[4].fl() + ctx.fpr[6].fl());
            sw(m, g[A0], 4, u64::from(ctx.fpr[18].u32l()));
            sw(m, g[A0], 8, u64::from(ctx.fpr[8].u32l()));
        } else {
            g[A1] = g[A0];
            g[A2] = addu(g[SP], 0x20);
            call(imports::func_80016CAC, m, ctx);
            let g = &mut ctx.gpr;
            g[T8] = li(0x800B_0000);
            g[T8] = lh(m, g[T8], -0x16CC);
            g[AT] = 2;
            g[A0] = li(0x800A_E8C8);
            if g[T8] != g[AT] {
                g[A1] = g[A0];
                g[A2] = addu(g[SP], 0x20);
                call(imports::func_80016BF4, m, ctx);
            }
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x60);
}

/// `func_80004000(c, r, n, d1, d2)` (query setup, **guess**: a sphere of
/// radius `r` at `c` against the planes `n . x = e + d1` and `e + d2`): `r`
/// and `d1` are floats in `a1`, `a3`, `d2` on the stack. It stores
/// `[0x800AE8E0] = r`, `[0x800AE8DC] = r * r`, `c` to `0x800AE908`, `n` to
/// `0x800AE948` and again to `0x800AE960` (each component loaded, then
/// stored), then with `e = n.z*c.z + (c.x*n.x + c.y*n.y)` (both re-read):
/// `[0x800AE954] = e + d1`, `[0x800AE958] = e + d2`, both copied (read
/// back) to `0x800AE96C` and `0x800AE970`; `[0x800AE8D8] = 0`, `[TRACKED]
/// = (r * r) * K` (`r * r` read back, `K` the float at `0x800A8138`),
/// [`MODE`] = 3, the callbacks `0x80003348` and `0x80002FFC` at
/// `0x800AE93C`/`40`, `[0x800AEC78] = [0x800AEC7C] = 0`; then it resets the
/// matrix stack ([`func_8000550C`](crate::matrix::func_8000550C)).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `t6 = 3`, `t7`/`t8` the
/// callbacks, `a1`, `a3`, `v1`, `t0` addresses it used, `f12 = r`, `f14 =
/// d1`, `f0 = e`, and the matrix reset's registers.
///
/// Domain: canonical pointers to 12 bytes, not overlapping what it writes;
/// no NaN operand of the products and sums.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80004000(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    ctx.fpr[12].set_u32l(g[A1] as u32);
    ctx.fpr[14].set_u32l(g[A3] as u32);
    ctx.fpr[4].set_fl(ctx.fpr[12].fl() * ctx.fpr[12].fl());
    g[AT] = li(0x800B_0000);
    g[A3] = li(0x800A_E8DC);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[AT], -0x1720, u64::from(ctx.fpr[12].u32l()));
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[A3], 0, u64::from(ctx.fpr[4].u32l()));
    // c, then n twice.
    ctx.fpr[6].set_u32l(lw(m, g[A0], 0) as u32);
    g[V0] = li(0x800A_E908);
    sw(m, g[V0], 0, u64::from(ctx.fpr[6].u32l()));
    ctx.fpr[8].set_u32l(lw(m, g[A0], 4) as u32);
    g[V1] = li(0x800A_E948);
    sw(m, g[V0], 4, u64::from(ctx.fpr[8].u32l()));
    ctx.fpr[10].set_u32l(lw(m, g[A0], 8) as u32);
    g[A1] = li(0x800A_E960);
    sw(m, g[V0], 8, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[16].set_u32l(lw(m, g[A2], 0) as u32);
    g[T0] = li(0x800A_E954);
    sw(m, g[V1], 0, u64::from(ctx.fpr[16].u32l()));
    ctx.fpr[18].set_u32l(lw(m, g[A2], 4) as u32);
    g[V0] = li(0x800A_E958);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 4, u64::from(ctx.fpr[18].u32l()));
    ctx.fpr[4].set_u32l(lw(m, g[A2], 8) as u32);
    g[T6] = 3;
    g[T7] = li(0x8000_3348);
    sw(m, g[V1], 8, u64::from(ctx.fpr[4].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[A2], 0) as u32);
    g[T8] = li(0x8000_2FFC);
    sw(m, g[A1], 0, u64::from(ctx.fpr[6].u32l()));
    ctx.fpr[8].set_u32l(lw(m, g[A2], 4) as u32);
    sw(m, g[A1], 4, u64::from(ctx.fpr[8].u32l()));
    ctx.fpr[10].set_u32l(lw(m, g[A2], 8) as u32);
    sw(m, g[A1], 8, u64::from(ctx.fpr[10].u32l()));
    // e, the two offsets and their copies.
    ctx.fpr[18].set_u32l(lw(m, g[A2], 0) as u32);
    ctx.fpr[16].set_u32l(lw(m, g[A0], 0) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[A2], 4) as u32);
    ctx.fpr[6].set_u32l(lw(m, g[A0], 4) as u32);
    ctx.fpr[4].set_fl(ctx.fpr[16].fl() * ctx.fpr[18].fl());
    ctx.fpr[18].set_u32l(lw(m, g[A2], 8) as u32);
    ctx.fpr[10].set_fl(ctx.fpr[6].fl() * ctx.fpr[8].fl());
    ctx.fpr[6].set_u32l(lw(m, g[A0], 8) as u32);
    ctx.fpr[8].set_fl(ctx.fpr[18].fl() * ctx.fpr[6].fl());
    ctx.fpr[16].set_fl(ctx.fpr[4].fl() + ctx.fpr[10].fl());
    ctx.fpr[10].set_u32l(lw(m, g[SP], 0x28) as u32);
    ctx.fpr[0].set_fl(ctx.fpr[8].fl() + ctx.fpr[16].fl());
    ctx.fpr[4].set_fl(ctx.fpr[0].fl() + ctx.fpr[14].fl());
    ctx.fpr[18].set_fl(ctx.fpr[0].fl() + ctx.fpr[10].fl());
    sw(m, g[T0], 0, u64::from(ctx.fpr[4].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[T0], 0) as u32);
    sw(m, g[V0], 0, u64::from(ctx.fpr[18].u32l()));
    sw(m, g[AT], -0x1694, u64::from(ctx.fpr[6].u32l()));
    ctx.fpr[8].set_u32l(lw(m, g[V0], 0) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[AT], -0x1690, u64::from(ctx.fpr[8].u32l()));
    // The rest of the query state.
    g[AT] = li(0x800B_0000);
    sw(m, g[AT], -0x1728, 0);
    g[AT] = li(0x800B_0000);
    ctx.fpr[4].set_u32l(lw(m, g[AT], -0x7EC8) as u32);
    ctx.fpr[16].set_u32l(lw(m, g[A3], 0) as u32);
    g[AT] = li(0x800B_0000);
    ctx.fpr[10].set_fl(ctx.fpr[16].fl() * ctx.fpr[4].fl());
    sw(m, g[AT], -0x1750, u64::from(ctx.fpr[10].u32l()));
    g[AT] = li(0x800B_0000);
    sh(m, g[AT], -0x16CC, g[T6]);
    g[AT] = li(0x800B_0000);
    sw(m, g[AT], -0x16C4, g[T7]);
    g[AT] = li(0x800B_0000);
    sw(m, g[AT], -0x16C0, g[T8]);
    g[AT] = li(0x800B_0000);
    sw(m, g[AT], -0x1388, 0);
    g[AT] = li(0x800B_0000);
    sw(m, g[AT], -0x1384, 0);
    call(imports::func_8000550C, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}


/// `func_80004160(p, r, s, h, u, N, e, k)` (the query's hit applied to a
/// sphere, **guess**: pushing the centre `p` out along the hit): `r` and `h`
/// are floats in `a1`, `a3`; `u` (a float), `N`, `e`, `k` (vec3 pointers)
/// are on the stack; `p` and `s` are vec3 pointers. With `T = [TRACKED]`
/// (the best squared distance), `R2 = [0x800AE8DC]` (`r * r` from
/// [`func_80004000`]), the hit point `K = TRACKED + 8` and direction `TRACKED
/// + 0x18`:
/// - If not `T < R2`: returns `v0 = 0`, nothing else changes but the frame.
/// - Otherwise `N` = the direction (word by word), `D` = `p - K` normalised
///   ([`func_800154D0`](crate::math::func_800154D0)), `Y = (D x s) x s`
///   ([`func_80015538`](crate::math::func_80015538) twice) and `l = |Y|`
///   ([`func_800153C0`](crate::math::func_800153C0)).
/// - If `l < L0` (`L0` the float at `0x800A813C`, 0.01 in the ROM): with
///   `dd = s.z*D.z + (D.x*s.x + D.y*s.y)`, `D = N * (-u - sqrt(T))` if not
///   `dd < 0`, else `D = N * -(h - sqrt(T))`
///   ([`func_800155C0`](crate::math::func_800155C0)); then `p += D` and `e =
///   -N`, component by component.
/// - Otherwise `Y = Y * (1 / l)`, `A = Y * sqrt(R2 - h*h)`, `B = s * h + A`
///   ([`func_800155EC`](crate::math::func_800155EC)), `C = Y * sqrt(R2 -
///   u*u)`, `E = s * u + C`, `F = B x D`, `G = E x D` and `fg = G.z*F.z +
///   (F.x*G.x + F.y*G.y)`:
///   - if not `fg < 0`: with `sn = N.z*s.z + (s.x*N.x + s.y*N.y)`, `P = E +
///     p` if `0 < sn`, else `B + p` (component by component); `e = E`, then
///     normalised; then `p = N * (kd - td) + p` with `kd = (K.x*N.x +
///     K.y*N.y) + K.z*N.z` and `td = N.z*P.z + (P.x*N.x + P.y*N.y)`.
///   - else `D = D * (r - sqrt(T))`, then `p += D` and `e = -N`.
/// - Then `k = K` (word by word), `[0x8009A280] = [0x800AE8D8]` if that word
///   is nonzero, and `v0 = 1`.
///
/// Sums and differences are in the order written, nothing fused; `sqrt`
/// is IEEE. `T`, `R2`, `l`, `dd`, `fg` and `sn` are only compared, but
/// `T` and the radicands go through `sqrt` on the paths that use them.
///
/// Frame (`sp - 0xC0`): `ra`, `s2`, `s1`, `s0` at `+0x24..+0x18` (`s0 = p`,
/// `s1 = N`, `s2 = s`), restored sign-extended; `r` and `h` spilled to
/// their home slots `+0xC4`, `+0xCC`. `D` at `+0xB4`, `X` at `+0x6C`, `Y`
/// at `+0x90`, `A` at `+0x84`, `B` at `+0x58`, `C` at `+0x78`, `E` at
/// `+0x4C`, `F` at `+0x40`, `G` at `+0x34`, `P` at `+0xA8`. Past the hit
/// test it leaves `v1 = [0x800AE8D8]`, `a0 = 0x800AE8B8`, `at =
/// 0x800A0000`, `f4`, `f10`, `f6` = `K`'s words, and the path's
/// temporaries and callees' registers otherwise (`f0` 0.0 or a square
/// root, `f2`, `f12`, `f14` as the path left them). The early return
/// leaves `at = 0x800B0000`, `f4 = T`, `f6 = R2`.
///
/// Domain: canonical pointers to 12 bytes not overlapping the frame or the
/// query's state; the callees' domains; no NaN operand of the differences,
/// products, sums, negations or `sqrt`s (so `T >= 0` where `sqrt(T)` is
/// taken, and `R2 - h*h`, `R2 - u*u >= 0` on the `l >= L0` path).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80004160(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x800B_0000);
    f[4].set_u32l(lw(m, g[AT], -0x1750) as u32);
    g[AT] = li(0x800B_0000);
    f[6].set_u32l(lw(m, g[AT], -0x1724) as u32);
    g[SP] = addu(g[SP], (-0xC0i64) as u64);
    sw(m, g[SP], 0x20, g[S2]);
    let hit = f[4].fl() < f[6].fl();
    sw(m, g[SP], 0x18, g[S0]);
    g[S0] = g[A0];
    g[S2] = g[A2];
    sw(m, g[SP], 0x24, g[RA]);
    sw(m, g[SP], 0x1C, g[S1]);
    sw(m, g[SP], 0xC4, g[A1]);
    sw(m, g[SP], 0xCC, g[A3]);
    if !hit {
        g[V0] = 0;
        g[RA] = lw(m, g[SP], 0x24);
        g[S0] = lw(m, g[SP], 0x18);
        g[S1] = lw(m, g[SP], 0x1C);
        g[S2] = lw(m, g[SP], 0x20);
        g[SP] = addu(g[SP], 0xC0);
        return;
    }
    // N = the direction; D = p - K, normalised.
    g[V0] = li(0x800A_E8C8);
    g[S1] = lw(m, g[SP], 0xD4);
    f[8].set_u32l(lw(m, g[V0], 0) as u32);
    g[V1] = li(0x800A_E8B8);
    sw(m, g[S1], 0, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[V0], 4) as u32);
    sw(m, g[S1], 4, u64::from(f[10].u32l()));
    f[18].set_u32l(lw(m, g[V0], 8) as u32);
    sw(m, g[S1], 8, u64::from(f[18].u32l()));
    f[6].set_u32l(lw(m, g[V1], 0) as u32);
    f[4].set_u32l(lw(m, g[A0], 0) as u32);
    f[18].set_u32l(lw(m, g[V1], 4) as u32);
    f[8].set_fl(f[4].fl() - f[6].fl());
    sw(m, g[SP], 0xB4, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 4) as u32);
    f[8].set_u32l(lw(m, g[V1], 8) as u32);
    f[4].set_fl(f[10].fl() - f[18].fl());
    sw(m, g[SP], 0xB8, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 8) as u32);
    g[A0] = addu(g[SP], 0xB4);
    f[10].set_fl(f[6].fl() - f[8].fl());
    sw(m, g[SP], 0xBC, u64::from(f[10].u32l()));
    call(imports::func_800154D0, m, ctx);
    // Y = (D x s) x s and its length.
    let g = &mut ctx.gpr;
    g[A0] = addu(g[SP], 0x6C);
    g[A1] = addu(g[SP], 0xB4);
    g[A2] = g[S2];
    call(imports::func_80015538, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = addu(g[SP], 0x90);
    g[A1] = addu(g[SP], 0x6C);
    g[A2] = g[S2];
    call(imports::func_80015538, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = addu(g[SP], 0x90);
    call(imports::func_800153C0, m, ctx);
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x800B_0000);
    f[18].set_u32l(lw(m, g[AT], -0x7EC4) as u32);
    f[2].set_u32l(f[0].u32l());
    f[4].set_u32l(lw(m, g[SP], 0xB4) as u32);
    let parallel = f[0].fl() < f[18].fl();
    g[AT] = li(0x3F80_0000);
    if !parallel {
        f[18].set_u32l(g[AT] as u32);
        // Y /= l; A = Y sqrt(R2 - h h), B = s h + A; C = Y sqrt(R2 - u u),
        // E = s u + C.
        g[A0] = addu(g[SP], 0x90);
        g[A2] = g[A0];
        f[8].set_fl(f[18].fl() / f[2].fl());
        g[A1] = s32(f[8].u32l());
        call(imports::func_800155C0, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        f[2].set_u32l(lw(m, g[SP], 0xCC) as u32);
        g[AT] = li(0x800B_0000);
        f[6].set_u32l(lw(m, g[AT], -0x1724) as u32);
        f[4].set_fl(f[2].fl() * f[2].fl());
        g[A0] = addu(g[SP], 0x84);
        g[A2] = addu(g[SP], 0x90);
        f[0].set_fl(f[6].fl() - f[4].fl());
        f[0].set_fl(f[0].fl().sqrt());
        g[A1] = s32(f[0].u32l());
        call(imports::func_800155C0, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = addu(g[SP], 0x58);
        g[A1] = addu(g[SP], 0x84);
        g[A2] = lw(m, g[SP], 0xCC);
        g[A3] = g[S2];
        call(imports::func_800155EC, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        f[2].set_u32l(lw(m, g[SP], 0xD0) as u32);
        g[AT] = li(0x800B_0000);
        f[10].set_u32l(lw(m, g[AT], -0x1724) as u32);
        f[18].set_fl(f[2].fl() * f[2].fl());
        g[A0] = addu(g[SP], 0x78);
        g[A2] = addu(g[SP], 0x90);
        f[0].set_fl(f[10].fl() - f[18].fl());
        f[0].set_fl(f[0].fl().sqrt());
        g[A1] = s32(f[0].u32l());
        call(imports::func_800155C0, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = addu(g[SP], 0x4C);
        g[A1] = addu(g[SP], 0x78);
        g[A2] = lw(m, g[SP], 0xD0);
        g[A3] = g[S2];
        call(imports::func_800155EC, m, ctx);
        // F = B x D, G = E x D, fg = G.z*F.z + (F.x*G.x + F.y*G.y).
        let g = &mut ctx.gpr;
        g[A0] = addu(g[SP], 0x40);
        g[A1] = addu(g[SP], 0x58);
        g[A2] = addu(g[SP], 0xB4);
        call(imports::func_80015538, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = addu(g[SP], 0x34);
        g[A1] = addu(g[SP], 0x4C);
        g[A2] = addu(g[SP], 0xB4);
        call(imports::func_80015538, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        f[8].set_u32l(lw(m, g[SP], 0x40) as u32);
        f[6].set_u32l(lw(m, g[SP], 0x34) as u32);
        f[10].set_u32l(lw(m, g[SP], 0x44) as u32);
        f[18].set_u32l(lw(m, g[SP], 0x38) as u32);
        f[4].set_fl(f[8].fl() * f[6].fl());
        f[0].set_u32l(0);
        g[AT] = li(0x800B_0000);
        f[8].set_fl(f[10].fl() * f[18].fl());
        f[18].set_u32l(lw(m, g[SP], 0x48) as u32);
        f[10].set_u32l(lw(m, g[SP], 0x3C) as u32);
        f[6].set_fl(f[4].fl() + f[8].fl());
        f[4].set_fl(f[10].fl() * f[18].fl());
        f[8].set_fl(f[4].fl() + f[6].fl());
        if !(f[8].fl() < f[0].fl()) {
            // sn = N.z*s.z + (s.x*N.x + s.y*N.y) picks P = E + p or B + p.
            f[4].set_u32l(lw(m, g[S2], 0) as u32);
            f[6].set_u32l(lw(m, g[S1], 0) as u32);
            f[10].set_u32l(lw(m, g[S2], 4) as u32);
            f[18].set_u32l(lw(m, g[S1], 4) as u32);
            f[8].set_fl(f[4].fl() * f[6].fl());
            f[4].set_fl(f[10].fl() * f[18].fl());
            f[18].set_u32l(lw(m, g[S2], 8) as u32);
            f[10].set_u32l(lw(m, g[S1], 8) as u32);
            f[6].set_fl(f[8].fl() + f[4].fl());
            f[8].set_fl(f[10].fl() * f[18].fl());
            f[10].set_u32l(lw(m, g[SP], 0x4C) as u32);
            f[4].set_fl(f[8].fl() + f[6].fl());
            f[6].set_u32l(lw(m, g[SP], 0x58) as u32);
            if !(f[0].fl() < f[4].fl()) {
                f[4].set_u32l(lw(m, g[S0], 0) as u32);
                f[18].set_u32l(lw(m, g[SP], 0x5C) as u32);
                g[A0] = lw(m, g[SP], 0xD8);
                f[10].set_fl(f[6].fl() + f[4].fl());
                f[4].set_u32l(lw(m, g[SP], 0x60) as u32);
                sw(m, g[SP], 0xA8, u64::from(f[10].u32l()));
                f[8].set_u32l(lw(m, g[S0], 4) as u32);
                f[6].set_fl(f[18].fl() + f[8].fl());
                f[8].set_u32l(lw(m, g[SP], 0x4C) as u32);
                sw(m, g[SP], 0xAC, u64::from(f[6].u32l()));
                f[10].set_u32l(lw(m, g[S0], 8) as u32);
                f[18].set_fl(f[4].fl() + f[10].fl());
                sw(m, g[SP], 0xB0, u64::from(f[18].u32l()));
                sw(m, g[A0], 0, u64::from(f[8].u32l()));
                f[6].set_u32l(lw(m, g[SP], 0x50) as u32);
                sw(m, g[A0], 4, u64::from(f[6].u32l()));
                f[4].set_u32l(lw(m, g[SP], 0x54) as u32);
                sw(m, g[A0], 8, u64::from(f[4].u32l()));
            } else {
                f[18].set_u32l(lw(m, g[S0], 0) as u32);
                f[6].set_u32l(lw(m, g[SP], 0x50) as u32);
                g[A0] = lw(m, g[SP], 0xD8);
                f[8].set_fl(f[10].fl() + f[18].fl());
                sw(m, g[SP], 0xA8, u64::from(f[8].u32l()));
                f[4].set_u32l(lw(m, g[S0], 4) as u32);
                f[8].set_u32l(lw(m, g[SP], 0x54) as u32);
                f[18].set_fl(f[6].fl() + f[4].fl());
                sw(m, g[SP], 0xAC, u64::from(f[18].u32l()));
                f[6].set_u32l(lw(m, g[S0], 8) as u32);
                f[4].set_fl(f[8].fl() + f[6].fl());
                sw(m, g[SP], 0xB0, u64::from(f[4].u32l()));
                sw(m, g[A0], 0, u64::from(f[10].u32l()));
                f[18].set_u32l(lw(m, g[SP], 0x50) as u32);
                sw(m, g[A0], 4, u64::from(f[18].u32l()));
                f[8].set_u32l(lw(m, g[SP], 0x54) as u32);
                sw(m, g[A0], 8, u64::from(f[8].u32l()));
            }
            // e normalised; p = N (kd - td) + p.
            call(imports::func_800154D0, m, ctx);
            let g = &mut ctx.gpr;
            let f = &mut ctx.fpr;
            g[V0] = li(0x800A_E8B8);
            f[10].set_u32l(lw(m, g[V0], 0) as u32);
            f[2].set_u32l(lw(m, g[S1], 0) as u32);
            f[8].set_u32l(lw(m, g[V0], 4) as u32);
            f[12].set_u32l(lw(m, g[S1], 4) as u32);
            f[18].set_fl(f[10].fl() * f[2].fl());
            f[10].set_u32l(lw(m, g[V0], 8) as u32);
            f[14].set_u32l(lw(m, g[S1], 8) as u32);
            f[6].set_fl(f[8].fl() * f[12].fl());
            g[A0] = g[S0];
            g[A1] = g[S0];
            f[8].set_fl(f[10].fl() * f[14].fl());
            g[A3] = g[S1];
            f[4].set_fl(f[18].fl() + f[6].fl());
            f[6].set_u32l(lw(m, g[SP], 0xA8) as u32);
            f[18].set_fl(f[4].fl() + f[8].fl());
            f[10].set_fl(f[6].fl() * f[2].fl());
            f[4].set_u32l(lw(m, g[SP], 0xAC) as u32);
            f[8].set_fl(f[4].fl() * f[12].fl());
            f[4].set_u32l(lw(m, g[SP], 0xB0) as u32);
            f[6].set_fl(f[10].fl() + f[8].fl());
            f[10].set_fl(f[14].fl() * f[4].fl());
            f[8].set_fl(f[10].fl() + f[6].fl());
            f[16].set_fl(f[18].fl() - f[8].fl());
            g[A2] = s32(f[16].u32l());
            call(imports::func_800155EC, m, ctx);
        } else {
            // D *= r - sqrt(T); p += D; e = -N.
            f[0].set_u32l(lw(m, g[AT], -0x1750) as u32);
            f[10].set_u32l(lw(m, g[SP], 0xC4) as u32);
            g[A2] = addu(g[SP], 0xB4);
            f[0].set_fl(f[0].fl().sqrt());
            g[A0] = g[A2];
            f[18].set_fl(f[10].fl() - f[0].fl());
            g[A1] = s32(f[18].u32l());
            call(imports::func_800155C0, m, ctx);
            let g = &mut ctx.gpr;
            let f = &mut ctx.fpr;
            f[4].set_u32l(lw(m, g[S0], 0) as u32);
            f[6].set_u32l(lw(m, g[SP], 0xB4) as u32);
            g[A0] = lw(m, g[SP], 0xD8);
            f[10].set_u32l(lw(m, g[S0], 4) as u32);
            f[8].set_fl(f[4].fl() + f[6].fl());
            f[6].set_u32l(lw(m, g[S0], 8) as u32);
            sw(m, g[S0], 0, u64::from(f[8].u32l()));
            f[18].set_u32l(lw(m, g[SP], 0xB8) as u32);
            f[4].set_fl(f[10].fl() + f[18].fl());
            sw(m, g[S0], 4, u64::from(f[4].u32l()));
            f[8].set_u32l(lw(m, g[SP], 0xBC) as u32);
            f[10].set_fl(f[6].fl() + f[8].fl());
            sw(m, g[S0], 8, u64::from(f[10].u32l()));
            f[18].set_u32l(lw(m, g[S1], 0) as u32);
            f[4].set_fl(-f[18].fl());
            sw(m, g[A0], 0, u64::from(f[4].u32l()));
            f[6].set_u32l(lw(m, g[S1], 4) as u32);
            f[8].set_fl(-f[6].fl());
            sw(m, g[A0], 4, u64::from(f[8].u32l()));
            f[10].set_u32l(lw(m, g[S1], 8) as u32);
            f[18].set_fl(-f[10].fl());
            sw(m, g[A0], 8, u64::from(f[18].u32l()));
        }
    } else {
        // Y too short: dd = s.z*D.z + (D.x*s.x + D.y*s.y) picks the push.
        f[6].set_u32l(lw(m, g[S2], 0) as u32);
        f[10].set_u32l(lw(m, g[SP], 0xB8) as u32);
        f[18].set_u32l(lw(m, g[S2], 4) as u32);
        f[8].set_fl(f[4].fl() * f[6].fl());
        f[0].set_u32l(0);
        g[AT] = li(0x800B_0000);
        f[4].set_fl(f[10].fl() * f[18].fl());
        f[18].set_u32l(lw(m, g[SP], 0xBC) as u32);
        f[10].set_u32l(lw(m, g[S2], 8) as u32);
        f[6].set_fl(f[8].fl() + f[4].fl());
        f[8].set_fl(f[10].fl() * f[18].fl());
        f[4].set_fl(f[8].fl() + f[6].fl());
        f[6].set_u32l(lw(m, g[SP], 0xD0) as u32);
        if !(f[4].fl() < f[0].fl()) {
            f[0].set_u32l(lw(m, g[AT], -0x1750) as u32);
            f[4].set_fl(-f[6].fl());
            g[A0] = addu(g[SP], 0xB4);
            f[0].set_fl(f[0].fl().sqrt());
            g[A2] = g[S1];
            f[10].set_fl(f[4].fl() - f[0].fl());
            g[A1] = s32(f[10].u32l());
            call(imports::func_800155C0, m, ctx);
        } else {
            g[AT] = li(0x800B_0000);
            f[0].set_u32l(lw(m, g[AT], -0x1750) as u32);
            f[10].set_u32l(lw(m, g[SP], 0xCC) as u32);
            g[A0] = addu(g[SP], 0xB4);
            f[0].set_fl(f[0].fl().sqrt());
            g[A2] = g[S1];
            f[18].set_fl(f[10].fl() - f[0].fl());
            f[8].set_fl(-f[18].fl());
            g[A1] = s32(f[8].u32l());
            call(imports::func_800155C0, m, ctx);
        }
        // p += D; e = -N.
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        f[18].set_u32l(lw(m, g[S0], 0) as u32);
        f[8].set_u32l(lw(m, g[SP], 0xB4) as u32);
        g[A0] = lw(m, g[SP], 0xD8);
        f[4].set_u32l(lw(m, g[S0], 4) as u32);
        f[6].set_fl(f[18].fl() + f[8].fl());
        f[8].set_u32l(lw(m, g[S0], 8) as u32);
        sw(m, g[S0], 0, u64::from(f[6].u32l()));
        f[10].set_u32l(lw(m, g[SP], 0xB8) as u32);
        f[18].set_fl(f[4].fl() + f[10].fl());
        sw(m, g[S0], 4, u64::from(f[18].u32l()));
        f[6].set_u32l(lw(m, g[SP], 0xBC) as u32);
        f[4].set_fl(f[8].fl() + f[6].fl());
        sw(m, g[S0], 8, u64::from(f[4].u32l()));
        f[10].set_u32l(lw(m, g[S1], 0) as u32);
        f[18].set_fl(-f[10].fl());
        sw(m, g[A0], 0, u64::from(f[18].u32l()));
        f[8].set_u32l(lw(m, g[S1], 4) as u32);
        f[6].set_fl(-f[8].fl());
        sw(m, g[A0], 4, u64::from(f[6].u32l()));
        f[4].set_u32l(lw(m, g[S1], 8) as u32);
        f[10].set_fl(-f[4].fl());
        sw(m, g[A0], 8, u64::from(f[10].u32l()));
    }
    // k = K; [0x8009A280] = [0x800AE8D8] if nonzero.
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[A0] = li(0x800A_E8B8);
    g[V0] = lw(m, g[SP], 0xDC);
    f[4].set_u32l(lw(m, g[A0], 0) as u32);
    g[V1] = li(0x800B_0000);
    g[AT] = li(0x800A_0000);
    sw(m, g[V0], 0, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[A0], 4) as u32);
    sw(m, g[V0], 4, u64::from(f[10].u32l()));
    f[6].set_u32l(lw(m, g[A0], 8) as u32);
    sw(m, g[V0], 8, u64::from(f[6].u32l()));
    g[V1] = lw(m, g[V1], -0x1728);
    if g[V1] != 0 {
        sw(m, g[AT], -0x5D80, g[V1]);
    }
    g[V0] = 1;
    g[RA] = lw(m, g[SP], 0x24);
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0xC0);
}

/// `func_80004704(flags, n, out, in)` (records into model space, **guess**:
/// a point and a direction per 28-byte record, word 6 untouched): for `n >
/// 0` (a signed 64-bit test; nothing otherwise), record by record:
/// - flags bit 0 clear: words 0..5 copied;
/// - bit 0 set, bit 1 clear: with `T` the translation row of the matrix
///   stack's top `M` ([`func_800059A8`](crate::matrix::func_800059A8),
///   frame copy), `out.p = in.p - T` (T's components reloaded each time) and
///   words 3..5 copied;
/// - bits 0 and 1 set: with `I = M^-1`
///   ([`func_800160BC`](crate::math::func_800160BC)), `out.p = in.p * I` as
///   a point ([`func_80016CAC`](crate::math::func_80016CAC)) and `out.v =
///   in.v * I`'s 3x3 part ([`func_80016BF4`](crate::math::func_80016BF4)).
///
/// The first two go through the records in pairs (an odd `n`'s first record
/// on its own first), each word loaded, then stored; the loops end when the
/// input pointer reaches `in + 28 n` (32-bit arithmetic). So `out == in`
/// works, and so do the callees' frame copies.
///
/// Frame (`sp - 0xD8`): `ra`, `s6`..`s0` at `+0x34..+0x18`, `M` at `+0x98`,
/// `I` at `+0x58`; with bit 0 set `out` and `in` are spilled to their home
/// slots `sp + 8`, `sp + 0xC` around the calls. Leaves `s0`..`s6`
/// restored, `v0`, `v1`, `t8`/`t9` the loop bounds and offsets, and
/// `f4`..`f18` the last record's words (or the callees' registers).
///
/// Domain: canonical `n`, `out` and `in` in RDRAM with `out` equal to `in`
/// or disjoint from it and the frame; no NaN operand of the subtractions or
/// the callees' arithmetic.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80004704(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_800049D8: {
        'b_800049D4: {
            g[SP] = addu(g[SP], (-0xD8i64) as u64);
            sw(m, g[SP], 0x30, g[S6]);
            sw(m, g[SP], 0x18, g[S0]);
            g[T6] = g[A0] & 1;
            g[S0] = g[A0];
            g[S6] = g[A1];
            sw(m, g[SP], 0x34, g[RA]);
            sw(m, g[SP], 0x2C, g[S5]);
            sw(m, g[SP], 0x28, g[S4]);
            sw(m, g[SP], 0x24, g[S3]);
            sw(m, g[SP], 0x20, g[S2]);
            sw(m, g[SP], 0x1C, g[S1]);
            if g[T6] == 0 {
                // L_800048FC
                g[S4] = 0;
                if (g[S6] as i64) > 0 {
                    g[V0] = g[S6] & 1;
                    g[T9] = sll(g[S6], 3);
                    if g[V0] != 0 {
                        ctx.fpr[4].set_u32l(lw(m, g[A3], 0) as u32);
                        g[S4] = 1;
                        sw(m, g[A2], 0, u64::from(ctx.fpr[4].u32l()));
                        ctx.fpr[6].set_u32l(lw(m, g[A3], 4) as u32);
                        sw(m, g[A2], 4, u64::from(ctx.fpr[6].u32l()));
                        ctx.fpr[8].set_u32l(lw(m, g[A3], 8) as u32);
                        sw(m, g[A2], 8, u64::from(ctx.fpr[8].u32l()));
                        ctx.fpr[10].set_u32l(lw(m, g[A3], 0xC) as u32);
                        sw(m, g[A2], 0xC, u64::from(ctx.fpr[10].u32l()));
                        ctx.fpr[16].set_u32l(lw(m, g[A3], 0x10) as u32);
                        sw(m, g[A2], 0x10, u64::from(ctx.fpr[16].u32l()));
                        ctx.fpr[18].set_u32l(lw(m, g[A3], 0x14) as u32);
                        sw(m, g[A2], 0x14, u64::from(ctx.fpr[18].u32l()));
                        if g[S4] == g[S6] {
                            break 'b_800049D4;
                        }
                    }
                    // L_80004948
                    g[V1] = sll(g[S4], 3);
                    g[V1] = subu(g[V1], g[S4]);
                    g[V1] = sll(g[V1], 2);
                    g[T9] = subu(g[T9], g[S6]);
                    g[T9] = sll(g[T9], 2);
                    g[V0] = addu(g[T9], g[A3]);
                    g[S1] = addu(g[A2], g[V1]);
                    g[S0] = addu(g[A3], g[V1]);
                    loop {
                        // L_80004968
                        ctx.fpr[4].set_u32l(lw(m, g[S0], 0) as u32);
                        g[S0] = addu(g[S0], 0x38);
                        g[S1] = addu(g[S1], 0x38);
                        sw(m, g[S1], -0x38, u64::from(ctx.fpr[4].u32l()));
                        ctx.fpr[6].set_u32l(lw(m, g[S0], -0x34) as u32);
                        sw(m, g[S1], -0x34, u64::from(ctx.fpr[6].u32l()));
                        ctx.fpr[8].set_u32l(lw(m, g[S0], -0x30) as u32);
                        sw(m, g[S1], -0x30, u64::from(ctx.fpr[8].u32l()));
                        ctx.fpr[10].set_u32l(lw(m, g[S0], -0x2C) as u32);
                        sw(m, g[S1], -0x2C, u64::from(ctx.fpr[10].u32l()));
                        ctx.fpr[16].set_u32l(lw(m, g[S0], -0x28) as u32);
                        sw(m, g[S1], -0x28, u64::from(ctx.fpr[16].u32l()));
                        ctx.fpr[18].set_u32l(lw(m, g[S0], -0x24) as u32);
                        sw(m, g[S1], -0x24, u64::from(ctx.fpr[18].u32l()));
                        ctx.fpr[4].set_u32l(lw(m, g[S0], -0x1C) as u32);
                        sw(m, g[S1], -0x1C, u64::from(ctx.fpr[4].u32l()));
                        ctx.fpr[6].set_u32l(lw(m, g[S0], -0x18) as u32);
                        sw(m, g[S1], -0x18, u64::from(ctx.fpr[6].u32l()));
                        ctx.fpr[8].set_u32l(lw(m, g[S0], -0x14) as u32);
                        sw(m, g[S1], -0x14, u64::from(ctx.fpr[8].u32l()));
                        ctx.fpr[10].set_u32l(lw(m, g[S0], -0x10) as u32);
                        sw(m, g[S1], -0x10, u64::from(ctx.fpr[10].u32l()));
                        ctx.fpr[16].set_u32l(lw(m, g[S0], -0xC) as u32);
                        sw(m, g[S1], -0xC, u64::from(ctx.fpr[16].u32l()));
                        ctx.fpr[18].set_u32l(lw(m, g[S0], -8) as u32);
                        sw(m, g[S1], -8, u64::from(ctx.fpr[18].u32l()));
                        if g[S0] == g[V0] {
                            break;
                        }
                    }
                }
            } else {
                g[S1] = addu(g[SP], 0x98);
                g[A0] = g[S1];
                sw(m, g[SP], 0xE0, g[A2]);
                sw(m, g[SP], 0xE4, g[A3]);
                call(imports::func_800059A8, m, ctx);
                let g = &mut ctx.gpr;
                g[T7] = g[S0] & 2;
                g[A2] = lw(m, g[SP], 0xE0);
                g[A3] = lw(m, g[SP], 0xE4);
                if g[T7] == 0 {
                    // L_800047D4
                    g[S4] = 0;
                    if (g[S6] as i64) > 0 {
                        g[V0] = g[S6] & 1;
                        ctx.fpr[6].set_u32l(lw(m, g[SP], 0xC8) as u32);
                        if g[V0] != 0 {
                            ctx.fpr[4].set_u32l(lw(m, g[A3], 0) as u32);
                            g[S4] = 1;
                            ctx.fpr[8].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
                            sw(m, g[A2], 0, u64::from(ctx.fpr[8].u32l()));
                            ctx.fpr[16].set_u32l(lw(m, g[SP], 0xCC) as u32);
                            ctx.fpr[10].set_u32l(lw(m, g[A3], 4) as u32);
                            ctx.fpr[18].set_fl(ctx.fpr[10].fl() - ctx.fpr[16].fl());
                            sw(m, g[A2], 4, u64::from(ctx.fpr[18].u32l()));
                            ctx.fpr[6].set_u32l(lw(m, g[SP], 0xD0) as u32);
                            ctx.fpr[4].set_u32l(lw(m, g[A3], 8) as u32);
                            ctx.fpr[8].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
                            sw(m, g[A2], 8, u64::from(ctx.fpr[8].u32l()));
                            ctx.fpr[10].set_u32l(lw(m, g[A3], 0xC) as u32);
                            sw(m, g[A2], 0xC, u64::from(ctx.fpr[10].u32l()));
                            ctx.fpr[16].set_u32l(lw(m, g[A3], 0x10) as u32);
                            sw(m, g[A2], 0x10, u64::from(ctx.fpr[16].u32l()));
                            ctx.fpr[18].set_u32l(lw(m, g[A3], 0x14) as u32);
                            sw(m, g[A2], 0x14, u64::from(ctx.fpr[18].u32l()));
                            if g[S4] == g[S6] {
                                break 'b_800049D4;
                            }
                        }
                        // L_80004834
                        g[V1] = sll(g[S4], 3);
                        g[V1] = subu(g[V1], g[S4]);
                        g[T8] = sll(g[S6], 3);
                        g[T8] = subu(g[T8], g[S6]);
                        g[V1] = sll(g[V1], 2);
                        g[T8] = sll(g[T8], 2);
                        g[V0] = addu(g[T8], g[A3]);
                        g[S1] = addu(g[A2], g[V1]);
                        g[S0] = addu(g[A3], g[V1]);
                        loop {
                            // L_80004858
                            ctx.fpr[4].set_u32l(lw(m, g[S0], 0) as u32);
                            ctx.fpr[6].set_u32l(lw(m, g[SP], 0xC8) as u32);
                            g[S0] = addu(g[S0], 0x38);
                            g[S1] = addu(g[S1], 0x38);
                            ctx.fpr[8].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
                            sw(m, g[S1], -0x38, u64::from(ctx.fpr[8].u32l()));
                            ctx.fpr[16].set_u32l(lw(m, g[SP], 0xCC) as u32);
                            ctx.fpr[10].set_u32l(lw(m, g[S0], -0x34) as u32);
                            ctx.fpr[18].set_fl(ctx.fpr[10].fl() - ctx.fpr[16].fl());
                            sw(m, g[S1], -0x34, u64::from(ctx.fpr[18].u32l()));
                            ctx.fpr[6].set_u32l(lw(m, g[SP], 0xD0) as u32);
                            ctx.fpr[4].set_u32l(lw(m, g[S0], -0x30) as u32);
                            ctx.fpr[8].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
                            sw(m, g[S1], -0x30, u64::from(ctx.fpr[8].u32l()));
                            ctx.fpr[10].set_u32l(lw(m, g[S0], -0x2C) as u32);
                            sw(m, g[S1], -0x2C, u64::from(ctx.fpr[10].u32l()));
                            ctx.fpr[16].set_u32l(lw(m, g[S0], -0x28) as u32);
                            sw(m, g[S1], -0x28, u64::from(ctx.fpr[16].u32l()));
                            ctx.fpr[18].set_u32l(lw(m, g[S0], -0x24) as u32);
                            sw(m, g[S1], -0x24, u64::from(ctx.fpr[18].u32l()));
                            ctx.fpr[6].set_u32l(lw(m, g[SP], 0xC8) as u32);
                            ctx.fpr[4].set_u32l(lw(m, g[S0], -0x1C) as u32);
                            ctx.fpr[8].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
                            sw(m, g[S1], -0x1C, u64::from(ctx.fpr[8].u32l()));
                            ctx.fpr[16].set_u32l(lw(m, g[SP], 0xCC) as u32);
                            ctx.fpr[10].set_u32l(lw(m, g[S0], -0x18) as u32);
                            ctx.fpr[18].set_fl(ctx.fpr[10].fl() - ctx.fpr[16].fl());
                            sw(m, g[S1], -0x18, u64::from(ctx.fpr[18].u32l()));
                            ctx.fpr[6].set_u32l(lw(m, g[SP], 0xD0) as u32);
                            ctx.fpr[4].set_u32l(lw(m, g[S0], -0x14) as u32);
                            ctx.fpr[8].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
                            sw(m, g[S1], -0x14, u64::from(ctx.fpr[8].u32l()));
                            ctx.fpr[10].set_u32l(lw(m, g[S0], -0x10) as u32);
                            sw(m, g[S1], -0x10, u64::from(ctx.fpr[10].u32l()));
                            ctx.fpr[16].set_u32l(lw(m, g[S0], -0xC) as u32);
                            sw(m, g[S1], -0xC, u64::from(ctx.fpr[16].u32l()));
                            ctx.fpr[18].set_u32l(lw(m, g[S0], -8) as u32);
                            sw(m, g[S1], -8, u64::from(ctx.fpr[18].u32l()));
                            if g[S0] == g[V0] {
                                break;
                            }
                        }
                        g[RA] = lw(m, g[SP], 0x34);
                        break 'b_800049D8;
                    }
                } else {
                    g[S5] = addu(g[SP], 0x58);
                    g[A0] = g[S5];
                    g[A1] = g[S1];
                    sw(m, g[SP], 0xE0, g[A2]);
                    sw(m, g[SP], 0xE4, g[A3]);
                    call(imports::func_800160BC, m, ctx);
                    let g = &mut ctx.gpr;
                    g[A2] = lw(m, g[SP], 0xE0);
                    g[A3] = lw(m, g[SP], 0xE4);
                    g[S4] = 0;
                    if (g[S6] as i64) > 0 {
                        g[S1] = g[A2];
                        g[S0] = g[A3];
                        g[S2] = addu(g[A2], 0xC);
                        g[S3] = addu(g[A3], 0xC);
                        loop {
                            let g = &mut ctx.gpr;
                            // L_80004794
                            g[A0] = g[S1];
                            g[A1] = g[S0];
                            g[A2] = g[S5];
                            call(imports::func_80016CAC, m, ctx);
                            let g = &mut ctx.gpr;
                            g[A0] = g[S2];
                            g[A1] = g[S3];
                            g[A2] = g[S5];
                            call(imports::func_80016BF4, m, ctx);
                            let g = &mut ctx.gpr;
                            g[S4] = addu(g[S4], 1);
                            g[S1] = addu(g[S1], 0x1C);
                            g[S0] = addu(g[S0], 0x1C);
                            g[S2] = addu(g[S2], 0x1C);
                            g[S3] = addu(g[S3], 0x1C);
                            if g[S4] == g[S6] {
                                break;
                            }
                        }
                        let g = &mut ctx.gpr;
                        g[RA] = lw(m, g[SP], 0x34);
                        break 'b_800049D8;
                    }
                }
            }
        }
        let g = &mut ctx.gpr;
        // L_800049D4
        g[RA] = lw(m, g[SP], 0x34);
    }
    let g = &mut ctx.gpr;
    // L_800049D8
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[S3] = lw(m, g[SP], 0x24);
    g[S4] = lw(m, g[SP], 0x28);
    g[S5] = lw(m, g[SP], 0x2C);
    g[S6] = lw(m, g[SP], 0x30);
    g[SP] = addu(g[SP], 0xD8);
}
