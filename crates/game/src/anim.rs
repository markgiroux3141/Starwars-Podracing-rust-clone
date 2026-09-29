//! Animation objects (**guess**; NOTES.md, "Depth-0 leaves ported in session
//! 8"): the table of object pointers at `0x800AF4C0` and the objects'
//! fields: `+0x100` flags (kind in the low 4 bits; bit 31 set hides an
//! object from the search), `+0x124` a target or id, float keys at
//! `[+0x11C]` (count `+0x104`), a time `+0x114`, a range `+0xF0..+0xF8`.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use crate::recomp::{addu, enter, lhu, li, lw, reg::*, sh, sll, sw, Fpr, RecompContext};
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
