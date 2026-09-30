//! The asset heap: a bump allocator with a stack of levels (NOTES.md, "Asset
//! heap").
//!
//! `heap_init` (`func_80030FF8`) sets the start to `0x8014D7E0` and the end
//! ([`HEAP_END`]) to `[0x80114538]` minus a video-mode-dependent reserve.
//! [`CURSORS`] holds one allocation cursor per level, zero-terminated. The
//! current level ([`LEVEL`]) selects the one in use. Loaders read the cursor,
//! place data there, and store the new end back.

// Ports keep N64Recomp's names (func_8002FAFC), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use crate::recomp::{addu, call, enter, li, lw, reg::*, s32, sll, sltu, subu, sw, RecompContext};

/// Current heap level (index into [`CURSORS`]).
pub const LEVEL: u32 = 0x800A_2868;
/// One cursor per level, 10 words. Unused levels are zero; `func_8002FC80`
/// relies on a zero after the last one in use.
pub const CURSORS: u32 = 0x800D_9DD8;
/// End of the heap (exclusive).
pub const HEAP_END: u32 = 0x800D_9DBC;
/// The value `heap_init` computes the heap end from, and `func_8002FC80`
/// compares the last cursor with.
pub const HEAP_TOP: u32 = 0x8011_4538;

fn at(base: u64, offset: i32) -> u32 {
    (base as u32).wrapping_add(offset as u32)
}

/// `func_8002FAFC` (heap_cursor): `v0 = CURSORS[LEVEL]`.
///
/// Domain: `CURSORS + 4 * level` inside RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002FAFC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    // lui t6, 0x800A / lw t6, 0x2868(t6)
    g[T6] = s32(mem.read_u32(LEVEL));
    // lui v0, 0x800E / sll t7, t6, 2 / addu v0, v0, t7 / (delay) lw v0, -0x6228(v0)
    g[T7] = sll(g[T6], 2);
    let v0 = addu(s32(0x800E_0000), g[T7]);
    g[V0] = s32(mem.read_u32(at(v0, -0x6228)));
}

/// `func_8002FB18(p)`: 1 if `p` lies below the current heap cursor
/// ([`func_8002FAFC`], unsigned compare), else 0.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`, `p` spilled to its slot `+0x18`.
/// Leaves `t6 = p` (sign-extended from its low word), `at = v0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002FB18(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x18, g[A0]);
    call(imports::func_8002FAFC, m, ctx);
    let g = &mut ctx.gpr;
    g[T6] = lw(m, g[SP], 0x18);
    g[RA] = lw(m, g[SP], 0x14);
    g[AT] = sltu(g[T6], g[V0]);
    g[V0] = 0;
    if g[AT] != 0 {
        g[V0] = 1;
    }
    g[SP] = addu(g[SP], 0x18);
}

/// `func_8002FC80` (heap_check): find the last nonzero cursor (walking from
/// slot 1 to the first zero), load it and compare it with `[HEAP_TOP]`. Both
/// outcomes return at once: an assert with its report compiled out. It
/// stores nothing; only the registers it leaves behind are observable.
///
/// QUIRK: the walk has no bound. If slots 1..=9 are all nonzero it carries on
/// into the texture cache at `0x800D9E00` until it meets a zero word.
///
/// `a0` is overwritten only if slot 1 is nonzero; otherwise the caller's
/// value survives.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002FC80(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    let lw = |base: u64, offset: i32| s32(mem.read_u32(at(base, offset)));

    g[A2] = addu(s32(0x800E_0000), (-0x6228i64) as u64); // CURSORS
    g[T6] = lw(g[A2], 4);
    g[V0] = lw(s32(0x8011_0000), 0x4538);
    // beqz t6 / (delay) addiu v1, zero, 1
    g[V1] = 1;
    if g[T6] != 0 {
        g[A0] = addu(s32(0x800E_0000), (-0x6224i64) as u64); // CURSORS + 4
        g[T7] = lw(g[A0], 4);
        loop {
            g[V1] = addu(g[V1], 1);
            g[A0] = addu(g[A0], 4);
            // bnel t7, zero / (delay, if taken) lw t7, 4(a0)
            if g[T7] == 0 {
                break;
            }
            g[T7] = lw(g[A0], 4);
        }
    }
    g[T8] = sll(g[V1], 2);
    g[T9] = addu(g[A2], g[T8]);
    g[T0] = lw(g[T9], -4);
    // sltu at, t0, v0 (on the full registers); both branches reach jr ra.
    g[AT] = u64::from(g[T0] < g[V0]);
}

/// `func_8002FAC4` (heap_set_cursor): `CURSORS[LEVEL] = a0`, then
/// `func_8002FC80(1)`.
///
/// Domain: `CURSORS + 4 * level` and the 0x18-byte frame below `sp` inside
/// RDRAM, and [`func_8002FC80`]'s.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002FAC4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = s32(mem.read_u32(LEVEL));
    g[AT] = s32(0x800E_0000);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[T7] = sll(g[T6], 2);
    g[AT] = addu(g[AT], g[T7]);
    mem.write_u32(at(g[SP], 0x14), g[RA] as u32);
    mem.write_u32(at(g[AT], -0x6228), g[A0] as u32);
    // jal func_8002FC80 / (delay) addiu a0, zero, 1
    g[A0] = 1;
    call(imports::func_8002FC80, &mut mem, ctx);
    let g = &mut ctx.gpr;
    g[RA] = s32(mem.read_u32(at(g[SP], 0x14)));
    g[SP] = addu(g[SP], 0x18);
}

/// `func_8002FC58` (heap_free): `v0 = [HEAP_END] - func_8002FAFC()`, the
/// bytes left above the current cursor (32-bit, may be negative).
///
/// Domain: the 0x18-byte frame below `sp` inside RDRAM, and
/// [`func_8002FAFC`]'s.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002FC58(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    mem.write_u32(at(g[SP], 0x14), g[RA] as u32);
    call(imports::func_8002FAFC, &mut mem, ctx);
    let g = &mut ctx.gpr;
    g[RA] = s32(mem.read_u32(at(g[SP], 0x14)));
    g[T6] = s32(mem.read_u32(HEAP_END));
    g[SP] = addu(g[SP], 0x18);
    g[V0] = subu(g[T6], g[V0]);
}

/// `func_8002FB4C(p)`: the heap level whose allocations hold `p`. It scans
/// levels `l` from `level - 1` down to 1 (level = `[0x800A2868]`, signed)
/// and returns `l + 1` for the first with `p >= cursors[l]` (the cursors at
/// `0x800D9DD8`; unsigned 64-bit compare with the sign-extended cursor),
/// or 1 if none has. A level of 1 or less returns the level itself.
///
/// Leaves `v1 = v0 - 1`, `t6 = (level - 1) << 2`, `t7 = 0x800D9DD8`, and
/// from the scan (if any) `t8` = the last cursor read and `at` = its test.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002FB4C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V1] = lw(m, li(0x800A_0000), 0x2868);
    g[T7] = li(0x800D_9DD8);
    g[V1] = addu(g[V1], u64::MAX);
    g[T6] = sll(g[V1], 2);
    if (g[V1] as i64) > 0 {
        g[V0] = addu(g[T6], g[T7]);
        loop {
            g[T8] = lw(m, g[V0], 0);
            g[AT] = sltu(g[A0], g[T8]);
            if g[AT] == 0 {
                break;
            }
            g[V1] = addu(g[V1], u64::MAX);
            g[V0] = addu(g[V0], (-4i64) as u64);
            if (g[V1] as i64) <= 0 {
                break;
            }
        }
    }
    g[V0] = addu(g[V1], 1);
}
