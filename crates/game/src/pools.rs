//! The pool registry at `[0x800A2170]` (NOTES.md, "Depth-0 leaves ported in
//! session 7"): a 0-terminated list of pool descriptors (`+0` id, `+4` tag,
//! `+8` count, `+0xC` element size, `+0x10` elements, `+0x24` a callback)
//! and the functions that initialise, find, iterate, call and broadcast to
//! their elements.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use n64mem::Mem;
use crate::recomp::{addu, call, enter, lh, li, lw, multu, reg::*, sh, sll, slt, sltu, sra, subu, sw, Fpr, RecompContext};

/// The pool registry: `[0x800A2170]` points to a 0-terminated list of pool
/// descriptors, `+0` id, `+4` a tag word, `+8` count, `+0xC` element size,
/// `+0x10` the elements. [`func_8003F300`] .. [`func_8003FB78`] look pools up
/// by id, walking the list with `v0` (the slot) and `v1` (the descriptor).
pub const POOLS: u32 = 0x800A_2170;

/// `func_8003F300(id)`: initialise the elements of every pool with this id
/// (the search carries on past a match): element `k` gets `+0 = id` (the
/// descriptor's word), the halfword `+4 = k` and the halfword `+6` = the
/// descriptor's `+4`. The fields are re-read for every element.
///
/// QUIRK: `k` is kept as an s16, so a count above 32767 never ends the
/// loop. Domain: counts up to 32767, the elements in RDRAM. Leaves `a1 =
/// id`, `v0` = the list slot after the last, `v1 = 0`, `a0` = the last
/// descriptor tried, `a2` = the last count reached, and the loads in `t0`,
/// `t1`, `t3`, `t6`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003F300(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, li(0x800A_0000), 0x2170);
    g[A1] = g[A0];
    g[V1] = lw(m, g[V0], 0);
    while g[V1] != 0 {
        g[T6] = lw(m, g[V1], 0);
        g[A0] = g[V1];
        if g[A1] == g[T6] {
            g[T7] = lw(m, g[A0], 8);
            g[V1] = lw(m, g[V1], 0x10);
            g[A2] = 0;
            if (g[T7] as i64) > 0 {
                loop {
                    g[T8] = lw(m, g[A0], 0);
                    sh(m, g[V1], 4, g[A2]);
                    g[A2] = addu(g[A2], 1);
                    sw(m, g[V1], 0, g[T8]);
                    g[T9] = lw(m, g[A0], 4);
                    g[T1] = sll(g[A2], 16);
                    g[A2] = sra(g[T1], 16);
                    sh(m, g[V1], 6, g[T9]);
                    g[T3] = lw(m, g[A0], 8);
                    g[T0] = lw(m, g[A0], 0xC);
                    g[AT] = slt(g[A2], g[T3]);
                    g[V1] = addu(g[V1], g[T0]);
                    if g[AT] == 0 {
                        break;
                    }
                }
            }
        }
        g[V1] = lw(m, g[V0], 4);
        g[V0] = addu(g[V0], 4);
    }
}

/// `func_8003F714(id, tag)`: the first element, in any pool with this id,
/// whose halfword `+6` has bit 8 clear and whose halfword `+4` (signed)
/// equals `tag` (full 64-bit compare), else 0. `s0` holds `tag` and comes
/// back sign-extended from its low word.
///
/// Leaves `a2 = id`, `a0` = the last descriptor tried, `a1` = the last
/// element index (s16), `a3` = its count, `v1` = the last element or
/// slot, and the loads in `t0`, `t1`, `t6`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003F714(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, li(0x800A_0000), 0x2170);
    g[SP] = addu(g[SP], (-8i64) as u64);
    sw(m, g[SP], 4, g[S0]);
    g[V1] = lw(m, g[V0], 0);
    g[S0] = g[A1];
    g[A2] = g[A0];
    let found = 'search: {
        while g[V1] != 0 {
            g[T6] = lw(m, g[V1], 0);
            g[A0] = g[V1];
            if g[A2] == g[T6] {
                g[A3] = lw(m, g[A0], 8);
                g[V1] = lw(m, g[V1], 0x10);
                g[A1] = 0;
                if (g[A3] as i64) > 0 {
                    loop {
                        g[T7] = lh(m, g[V1], 6);
                        g[A1] = addu(g[A1], 1);
                        g[T1] = sll(g[A1], 16);
                        g[T8] = g[T7] & 0x100;
                        g[A1] = sra(g[T1], 16);
                        if g[T8] == 0 {
                            g[T9] = lh(m, g[V1], 4);
                            if g[S0] == g[T9] {
                                break 'search g[V1];
                            }
                        }
                        g[T0] = lw(m, g[A0], 0xC);
                        g[AT] = slt(g[A1], g[A3]);
                        g[V1] = addu(g[V1], g[T0]);
                        if g[AT] == 0 {
                            break;
                        }
                    }
                }
            }
            g[V1] = lw(m, g[V0], 4);
            g[V0] = addu(g[V0], 4);
        }
        0
    };
    g[V0] = found;
    g[S0] = lw(m, g[SP], 4);
    g[SP] = addu(g[SP], 8);
}

/// The first pool descriptor with this id, walking [`POOLS`] with `v0`
/// and `v1` (the id is in `id`). Returns false at the end of the list,
/// with `v1 = 0`.
fn find_pool(m: &Mem, g: &mut [u64; 32], id: usize, keep: Option<usize>) -> bool {
    g[V0] = lw(m, li(0x800A_0000), 0x2170);
    g[id] = g[A0];
    g[V1] = lw(m, g[V0], 0);
    while g[V1] != 0 {
        g[T6] = lw(m, g[V1], 0);
        if let Some(k) = keep {
            g[k] = g[V1];
        }
        if g[id] == g[T6] {
            return true;
        }
        g[V1] = lw(m, g[V0], 4);
        g[V0] = addu(g[V0], 4);
    }
    false
}

/// `func_8003F7B8(id)`: the count `+8` of the first pool with this id, or
/// 0. Leaves `a1 = id`, `v1` = the descriptor (or 0), `t6` = the last id
/// read.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003F7B8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[V0] = if find_pool(&mem, g, A1, None) { lw(&mem, g[V1], 8) } else { 0 };
}

/// `func_8003F800(id, i)`: start iterating the first pool with this id at
/// element `i`: clears the current pool `[0x800A4AA4]`, then if the pool
/// exists and `i < count` (signed), sets it and the index `[0x80118D10] =
/// i` and returns element `i` (`base + size * i`, low 32 bits of the
/// product), else 0. See [`func_8003F890`]. Leaves `a3 = 0x800A4AA4`, `a2 =
/// id`, `a0` = the descriptor, `v1` = the base, `t7` = the count, `at`,
/// `t8`, `t9` from the element.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003F800(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A3] = li(0x800A_4AA4);
    sw(m, g[A3], 0, 0);
    if !find_pool(m, g, A2, Some(A0)) {
        g[V0] = 0;
        return;
    }
    g[T7] = lw(m, g[A0], 8);
    g[V1] = lw(m, g[V1], 0x10);
    g[AT] = slt(g[A1], g[T7]);
    if g[AT] == 0 {
        g[V0] = 0;
        return;
    }
    sw(m, g[A3], 0, g[A0]);
    g[AT] = li(0x8012_0000);
    sw(m, g[AT], -0x72F0, g[A1]);
    g[T8] = lw(m, g[A0], 0xC);
    g[T9] = multu(g[T8], g[A1]).0;
    g[V0] = addu(g[V1], g[T9]);
}

/// `func_8003F890()`: the next element of the iteration [`func_8003F800`]
/// started: with a current pool, `i = [0x80118D10] + 1` is stored, and if
/// `i < count` (signed) element `i` is returned; otherwise the current
/// pool is cleared and 0 returned. With none, 0. Leaves `a1 = 0x800A4AA4`,
/// `v1` = the pool, `a0 = i`, `t6` = the old index, `t8` = the count, `at`,
/// and `t0`, `t9`, `t1` from the element.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003F890(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A1] = li(0x800A_4AA4);
    g[V1] = lw(m, g[A1], 0);
    g[V0] = li(0x8011_8D10);
    if g[V1] == 0 {
        g[V0] = 0;
        return;
    }
    g[T6] = lw(m, g[V0], 0);
    g[A0] = addu(g[T6], 1);
    sw(m, g[V0], 0, g[A0]);
    g[T8] = lw(m, g[V1], 8);
    g[V0] = 0;
    g[AT] = slt(g[A0], g[T8]);
    if g[AT] == 0 {
        sw(m, g[A1], 0, 0);
        return;
    }
    g[T0] = lw(m, g[V1], 0xC);
    g[T9] = lw(m, g[V1], 0x10);
    g[T1] = multu(g[T0], g[A0]).0;
    g[V0] = addu(g[T9], g[T1]);
}

/// `func_8003F99C(elem, arg)`: call the callback `[pool + 0x24]` of the
/// first pool whose id equals the element's `[elem + 0]`, as `cb(elem,
/// arg)`, unless `elem` is 0, no pool matches, the callback is 0, or bit 8
/// of the element's halfword `+6` is set. The callback is reached through
/// `LOOKUP_FUNC` (`imports::runtime::get_function`), so it may be any
/// function. `arg` is spilled to its slot and read back into `a1` (and
/// `a2`); `ra` is saved in a 0x18-byte frame.
///
/// Leaves (without a call) `a3 = elem`, `v0`/`v1` = the search's slot and
/// descriptor or the callback, `a1` = the element's id or `arg`, `t6`..`t8`
/// from the tests; with a call, the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003F99C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x1C, g[A1]);
    g[A3] = g[A0];
    'done: {
        if g[A0] == 0 {
            break 'done;
        }
        g[V0] = lw(m, li(0x800A_0000), 0x2170);
        g[V1] = lw(m, g[V0], 0);
        if g[V1] == 0 {
            break 'done;
        }
        g[A1] = lw(m, g[A0], 0);
        loop {
            g[T6] = lw(m, g[V1], 0);
            if g[A1] == g[T6] {
                break;
            }
            g[V1] = lw(m, g[V0], 4);
            g[V0] = addu(g[V0], 4);
            if g[V1] == 0 {
                break 'done;
            }
        }
        g[V0] = lw(m, g[V1], 0x24);
        if g[V0] == 0 {
            break 'done;
        }
        g[T7] = lh(m, g[A3], 6);
        g[A2] = lw(m, g[SP], 0x1C);
        g[T8] = g[T7] & 0x100;
        g[A1] = g[A2];
        if g[T8] != 0 {
            break 'done;
        }
        g[A0] = g[A3];
        call(imports::runtime::get_function(g[V0] as i32).expect("LOOKUP_FUNC"), m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_8003FA24(id, arg)`: send `arg` to the elements of the pools with
/// this id, or of every pool if `id` is `"All!"` (`0x416C6C21`): for each
/// pool, its callback `[pool + 0x24]` (if nonzero, through `LOOKUP_FUNC`)
/// is called as `cb(elem, arg, arg)` on each element whose halfword `+6`
/// has bit 8 clear, the count and element size re-read after each. A
/// callback returning 2 stops everything. With a single id, the first
/// matching pool is the last visited.
///
/// `s0`..`s7`, `fp` and `ra` are saved in a 0x40-byte frame and come back
/// sign-extended from their low words. Leaves `v0` = the last callback's
/// result or list word, `a0`/`a1`/`a2` = the last element and `arg`, and
/// the loop's loads in `t0`, `t1`, `t6`..`t9`, `at`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003FA24(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    sw(m, g[SP], 0x30, g[S6]);
    g[S6] = lw(m, li(0x800A_0000), 0x2170);
    for (off, r) in [(0x3C, RA), (0x38, FP), (0x34, S7), (0x2C, S5), (0x28, S4), (0x24, S3), (0x20, S2), (0x1C, S1), (0x18, S0)] {
        sw(m, g[SP], off, g[r]);
    }
    g[V0] = lw(m, g[S6], 0);
    g[S3] = g[A1];
    g[S7] = g[A0];
    g[FP] = li(0x416C_0000);
    'done: {
        if g[V0] == 0 {
            break 'done;
        }
        g[FP] |= 0x6C21; // "All!"
        g[S5] = 2;
        loop {
            let g = &mut ctx.gpr;
            g[T6] = lw(m, g[V0], 0);
            g[S2] = g[V0];
            if g[S7] == g[T6] || g[S7] == g[FP] {
                g[V0] = lw(m, g[S2], 0x24);
                if g[V0] != 0 {
                    g[T7] = lw(m, g[S2], 8);
                    g[S4] = g[V0];
                    g[S0] = lw(m, g[S2], 0x10);
                    g[S1] = 0;
                    if (g[T7] as i64) > 0 {
                        loop {
                            let g = &mut ctx.gpr;
                            g[T8] = lh(m, g[S0], 6);
                            g[A0] = g[S0];
                            g[A1] = g[S3];
                            g[T9] = g[T8] & 0x100;
                            if g[T9] == 0 {
                                g[A2] = g[S3];
                                call(imports::runtime::get_function(g[S4] as i32).expect("LOOKUP_FUNC"), m, ctx);
                                if ctx.gpr[V0] == ctx.gpr[S5] {
                                    break 'done;
                                }
                            }
                            let g = &mut ctx.gpr;
                            g[T1] = lw(m, g[S2], 8);
                            g[T0] = lw(m, g[S2], 0xC);
                            g[S1] = addu(g[S1], 1);
                            g[AT] = slt(g[S1], g[T1]);
                            g[S0] = addu(g[S0], g[T0]);
                            if g[AT] == 0 {
                                break;
                            }
                        }
                    }
                }
                if ctx.gpr[S7] != ctx.gpr[FP] {
                    break 'done;
                }
            }
            let g = &mut ctx.gpr;
            g[V0] = lw(m, g[S6], 4);
            g[S6] = addu(g[S6], 4);
            if g[V0] == 0 {
                break;
            }
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x3C);
    for (off, r) in [(0x18, S0), (0x1C, S1), (0x20, S2), (0x24, S3), (0x28, S4), (0x2C, S5), (0x30, S6), (0x34, S7), (0x38, FP)] {
        g[r] = lw(m, g[SP], off);
    }
    g[SP] = addu(g[SP], 0x40);
}

/// `func_8003FB78(id, count, base)`: set the first pool with this id to
/// `count` elements at `base` (`+8`, `+0x10`) and return `count * size`
/// (low 32 bits), or 0 if there is none. Leaves `a3 = id`, `v1` = the
/// descriptor, `t6`/`t7` = its id and size.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003FB78(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    if !find_pool(m, g, A3, None) {
        g[V0] = 0;
        return;
    }
    g[T7] = lw(m, g[V1], 0xC);
    sw(m, g[V1], 0x10, g[A2]);
    sw(m, g[V1], 8, g[A1]);
    g[V0] = multu(g[T7], g[A1]).0;
}

/// `func_8003FDCC(id, pos, max, skip, cap, dist, delta, found)` (nearest
/// elements, **guess** at the purpose): collects the elements of the
/// enabled pools with this id nearest to `pos`, sorted by squared distance,
/// and returns how many it kept (at most `cap`). `id` may be `"All!"`
/// (`0x416C6C21`) for every pool; `max` is a float (passed in `a2`); the
/// last four are stack arguments: `cap` a count, `dist` floats, `delta`
/// 12-byte vectors, `found` element pointers.
///
/// For each descriptor `d` of [`POOLS`] in order, if `d+0 == id` or `id`
/// is `"All!"`, and bit 0 of `d+4` is set, and the count `d+8 > 0`: for
/// each element `e` (from `d+0x10`, the count and stride `d+0xC` re-read
/// per element), unless bit 8 of the halfword `e+6` is set or `e == skip`:
/// `dx, dy, dz = e+0x50.. - pos` (floats), `q = dz*dz + (dx*dx + dy*dy)`.
/// If `q < max`: `k` = the first index below the count `n` so far with
/// `!(dist[k] < q)`, or `n`. If `k < cap` (signed): the last slot is `n`
/// (and `n` grows) while `n < cap`, else `cap - 1`; entries `last - 1`
/// down to `k` move up one (for each, `found`, then `dist`, then `delta`'s
/// z, y, x: words, as bits); then `dist[k] = q`, `found[k] = e`, `delta[k]
/// = (dx, dy, dz)`, reloaded from the frame.
///
/// Uses `ra` as the id's register. Saves `s0`..`s7`, `fp`, `ra` at `sp -
/// 0x48..-0x24`, spills `dx, dy, dz` at `sp - 0x1C..-0x14`, and restores
/// the saved registers. Leaves `v0 = t1 = n`, `f12 = max`, and `a0`..`a3`,
/// `t0`, `t2`..`t9`, `at`, `f0`..`f18` as the last steps left them (`t2
/// = dist` once the list isn't empty).
///
/// Domain: canonical pointers; the registry's descriptors and elements in
/// RDRAM; the arrays disjoint from each other, from the elements and from
/// the frame, with room for `cap` entries. No NaN operands: `pos` and the positions of elements that are
/// measured (`max` and the stored distances are only compared).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003FDCC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[SP] = addu(g[SP], (-0x50i64) as u64);
    sw(m, g[SP], 0x28, g[FP]);
    g[FP] = li(0x800A_0000);
    g[FP] = lw(m, g[FP], 0x2170);
    sw(m, g[SP], 0x2C, g[RA]);
    sw(m, g[SP], 0x24, g[S7]);
    sw(m, g[SP], 0x20, g[S6]);
    sw(m, g[SP], 0x1C, g[S5]);
    sw(m, g[SP], 0x18, g[S4]);
    sw(m, g[SP], 0x14, g[S3]);
    sw(m, g[SP], 0x10, g[S2]);
    sw(m, g[SP], 0xC, g[S1]);
    sw(m, g[SP], 8, g[S0]);
    // fp = the list slot, v0 = the descriptor, ra = id, s3 = pos, f12 =
    // max, s7 = skip, t1 = n.
    g[V0] = lw(m, g[FP], 0);
    f[12].set_u32l(g[A2] as u32);
    g[S3] = g[A1];
    g[S7] = g[A3];
    g[RA] = g[A0];
    g[T1] = 0;
    if g[V0] != 0 {
        // s1 = cap, t2 = dist, s2 = delta, s5 = found.
        g[S6] = 0xC;
        g[S5] = lw(m, g[SP], 0x6C);
        g[S2] = lw(m, g[SP], 0x68);
        g[S1] = lw(m, g[SP], 0x60);
        g[T2] = lw(m, g[SP], 0x64);
        g[T6] = lw(m, g[V0], 0);
        loop {
            // s4 = the descriptor.
            g[AT] = li(0x416C_0000);
            g[S4] = g[V0];
            g[AT] = g[AT] | 0x6C21; // "All!"
            if g[RA] != g[T6] && g[RA] != g[AT] {
                g[V0] = lw(m, g[FP], 4);
            } else {
                g[T7] = lw(m, g[S4], 4);
                g[T8] = g[T7] & 1;
                if g[T8] == 0 {
                    g[V0] = lw(m, g[FP], 4);
                } else {
                    // s0 = the index, t3 = the element.
                    g[T9] = lw(m, g[S4], 8);
                    g[T3] = lw(m, g[S4], 0x10);
                    g[S0] = 0;
                    if (g[T9] as i64) > 0 {
                        loop {
                            g[T6] = lh(m, g[T3], 6);
                            g[T7] = g[T6] & 0x100;
                            if g[T7] != 0 || g[T3] == g[S7] {
                                g[T8] = lw(m, g[S4], 8);
                            } else {
                                nearest_insert(m, g, f);
                            }
                            g[T7] = lw(m, g[S4], 0xC);
                            g[S0] = addu(g[S0], 1);
                            g[AT] = slt(g[S0], g[T8]);
                            g[T3] = addu(g[T3], g[T7]);
                            if g[AT] == 0 {
                                break;
                            }
                        }
                    }
                    g[V0] = lw(m, g[FP], 4);
                }
            }
            g[FP] = addu(g[FP], 4);
            if g[V0] == 0 {
                break;
            }
            g[T6] = lw(m, g[V0], 0);
        }
    }
    g[RA] = lw(m, g[SP], 0x2C);
    g[S0] = lw(m, g[SP], 8);
    g[S1] = lw(m, g[SP], 0xC);
    g[S2] = lw(m, g[SP], 0x10);
    g[S3] = lw(m, g[SP], 0x14);
    g[S4] = lw(m, g[SP], 0x18);
    g[S5] = lw(m, g[SP], 0x1C);
    g[S6] = lw(m, g[SP], 0x20);
    g[S7] = lw(m, g[SP], 0x24);
    g[FP] = lw(m, g[SP], 0x28);
    g[SP] = addu(g[SP], 0x50);
    g[V0] = g[T1];
}

/// [`func_8003FDCC`]'s measure and sorted insert of the element in `t3`
/// (registers as there); ends with `t8` = the pool's count, re-read.
fn nearest_insert(m: &mut Mem, g: &mut [u64; 32], f: &mut [Fpr; 32]) {
    // q = dz*dz + (dx*dx + dy*dy), dx..dz spilled at sp + 0x34.. and
    // reloaded.
    f[4].set_u32l(lw(m, g[T3], 0x50) as u32);
    f[6].set_u32l(lw(m, g[S3], 0) as u32);
    f[8].set_fl(f[4].fl() - f[6].fl());
    sw(m, g[SP], 0x34, u64::from(f[8].u32l()));
    f[16].set_u32l(lw(m, g[S3], 4) as u32);
    f[10].set_u32l(lw(m, g[T3], 0x54) as u32);
    f[18].set_fl(f[10].fl() - f[16].fl());
    f[10].set_u32l(lw(m, g[SP], 0x34) as u32);
    sw(m, g[SP], 0x38, u64::from(f[18].u32l()));
    f[6].set_u32l(lw(m, g[S3], 8) as u32);
    f[4].set_u32l(lw(m, g[T3], 0x58) as u32);
    f[16].set_fl(f[10].fl() * f[10].fl());
    f[18].set_u32l(lw(m, g[SP], 0x38) as u32);
    f[8].set_fl(f[4].fl() - f[6].fl());
    f[4].set_fl(f[18].fl() * f[18].fl());
    sw(m, g[SP], 0x3C, u64::from(f[8].u32l()));
    f[8].set_u32l(lw(m, g[SP], 0x3C) as u32);
    f[10].set_fl(f[8].fl() * f[8].fl());
    f[6].set_fl(f[16].fl() + f[4].fl());
    f[2].set_fl(f[10].fl() + f[6].fl());
    f[0].set_u32l(f[2].u32l());
    if f[2].fl() < f[12].fl() {
        // a1 = k: the first index whose distance isn't below q.
        g[A1] = 0;
        'k: {
            if (g[T1] as i64) > 0 {
                f[18].set_u32l(lw(m, g[T2], 0) as u32);
                if !(f[18].fl() < f[2].fl()) {
                    g[AT] = slt(g[A1], g[S1]);
                    break 'k;
                }
                g[A1] = addu(g[A1], 1);
                loop {
                    g[AT] = slt(g[A1], g[T1]);
                    g[T8] = sll(g[A1], 2);
                    if g[AT] == 0 {
                        break;
                    }
                    g[T9] = addu(g[T2], g[T8]);
                    f[16].set_u32l(lw(m, g[T9], 0) as u32);
                    if !(f[16].fl() < f[0].fl()) {
                        break;
                    }
                    g[A1] = addu(g[A1], 1);
                }
            }
            g[AT] = slt(g[A1], g[S1]);
        }
        g[T4] = sll(g[A1], 2);
        if g[AT] != 0 {
            // a3 = the last slot, t5 = &dist[k].
            g[AT] = slt(g[T1], g[S1]);
            g[T5] = addu(g[T2], g[T4]);
            if g[AT] == 0 {
                g[A3] = addu(g[S1], u64::MAX);
            } else {
                g[A3] = g[T1];
                g[T1] = addu(g[T1], 1);
            }
            g[AT] = slt(g[A1], g[A3]);
            g[T0] = sll(g[A3], 2);
            if g[AT] != 0 {
                // Move entries k..last-1 up, from the top: v1 = &found[j],
                // a0 = &dist[j], v0 = &delta[j - 1], a2 = &delta[k].
                g[T6] = sll(g[A3], 2);
                g[T7] = sll(g[A1], 2);
                g[T7] = subu(g[T7], g[A1]);
                g[T6] = subu(g[T6], g[A3]);
                g[T6] = sll(g[T6], 2);
                g[T7] = sll(g[T7], 2);
                g[A2] = addu(g[T7], g[S2]);
                g[V0] = addu(g[S2], g[T6]);
                g[V1] = addu(g[S5], g[T0]);
                g[A0] = addu(g[T2], g[T0]);
                loop {
                    g[T8] = lw(m, g[V1], -4);
                    g[V0] = addu(g[V0], (-0xCi64) as u64);
                    g[AT] = sltu(g[A2], g[V0]);
                    sw(m, g[V1], 0, g[T8]);
                    f[4].set_u32l(lw(m, g[A0], -4) as u32);
                    g[V1] = addu(g[V1], (-4i64) as u64);
                    g[A0] = addu(g[A0], (-4i64) as u64);
                    sw(m, g[A0], 4, u64::from(f[4].u32l()));
                    f[6].set_u32l(lw(m, g[V0], 8) as u32);
                    f[10].set_u32l(lw(m, g[V0], 4) as u32);
                    f[8].set_u32l(lw(m, g[V0], 0) as u32);
                    sw(m, g[V0], 0x14, u64::from(f[6].u32l()));
                    sw(m, g[V0], 0x10, u64::from(f[10].u32l()));
                    sw(m, g[V0], 0xC, u64::from(f[8].u32l()));
                    if g[AT] == 0 {
                        break;
                    }
                }
            }
            // The new entry.
            let (lo, _) = multu(g[A1], g[S6]);
            sw(m, g[T5], 0, u64::from(f[0].u32l()));
            g[T9] = addu(g[S5], g[T4]);
            sw(m, g[T9], 0, g[T3]);
            f[18].set_u32l(lw(m, g[SP], 0x34) as u32);
            g[T6] = lo;
            g[V0] = addu(g[S2], g[T6]);
            sw(m, g[V0], 0, u64::from(f[18].u32l()));
            f[16].set_u32l(lw(m, g[SP], 0x38) as u32);
            sw(m, g[V0], 4, u64::from(f[16].u32l()));
            f[4].set_u32l(lw(m, g[SP], 0x3C) as u32);
            sw(m, g[V0], 8, u64::from(f[4].u32l()));
        }
    }
    g[T8] = lw(m, g[S4], 8);
}
