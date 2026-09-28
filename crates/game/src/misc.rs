//! Small leaves whose subsystem isn't known yet, grouped by address. They
//! move to a named module once what they belong to is understood.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use crate::recomp::{addu, enter, li, lw, reg::*, sw, RecompContext};

/// `func_80005AFC`: decrement `[0x8009A29C]` if it is positive (signed).
///
/// Leaves `v1 = 0x8009A29C`, `v0` = the old value and `t6` = old - 1 (the
/// `blez` delay slot computes it either way).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80005AFC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[V1] = li(0x8009_A29C);
    g[V0] = lw(&mem, g[V1], 0);
    g[T6] = addu(g[V0], u64::MAX); // blez delay slot: addiu t6, v0, -1
    if (g[V0] as i64) > 0 {
        sw(&mut mem, g[V1], 0, g[T6]);
    }
}

/// `func_80005B1C(which, value)`: `which == 3` stores `value` in
/// `[0x8009A290]`, `which == 5` in `[0x8009A28C]`; anything else does
/// nothing. `which` is compared as a full 64-bit register.
///
/// Leaves `at = 0x800A0000` (both `bne` delay slots load it).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80005B1C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = 3;
    let three = g[A0] == g[AT];
    g[AT] = li(0x800A_0000);
    if three {
        sw(&mut mem, g[AT], -0x5D70, g[A1]);
    }
    g[AT] = 5;
    let five = g[A0] == g[AT];
    g[AT] = li(0x800A_0000);
    if five {
        sw(&mut mem, g[AT], -0x5D74, g[A1]);
    }
}

/// `func_80005B44(which)`: the getter for [`func_80005B1C`]: `[0x8009A290]`
/// for 3, `[0x8009A28C]` for 5, otherwise -1 (full 64-bit compares).
///
/// Leaves `at` = 3 if `which` is 3, else 5.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80005B44(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = 3;
    g[V0] = li(0x800A_0000); // bne delay slot
    if g[A0] == g[AT] {
        g[V0] = lw(&mem, g[V0], -0x5D70); // jr delay slot
        return;
    }
    g[AT] = 5;
    g[V0] = u64::MAX; // bne delay slot: addiu v0, zero, -1
    if g[A0] == g[AT] {
        g[V0] = li(0x800A_0000);
        g[V0] = lw(&mem, g[V0], -0x5D74); // jr delay slot
    }
}

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

/// An empty function of one argument: it spills `a0` to its argument slot
/// `[sp]` and returns. Five in a row, `func_800066DC` to `func_800066FC`
/// (the ports below); presumably compiled-out debug hooks.
///
/// Domain: canonical `sp` with `[sp]` in RDRAM.
fn spill_a0(rdram: *mut u8, ctx: *mut RecompContext) {
    // SAFETY: forwarded from the entry points below.
    let (mut mem, ctx) = unsafe { enter(rdram, ctx) };
    sw(&mut mem, ctx.gpr[SP], 0, ctx.gpr[A0]);
}

/// `func_800066DC`: spills `a0` to `[sp]` and returns (see [`spill_a0`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800066DC(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_a0(rdram, ctx)
}

/// `func_800066E4`: spills `a0` to `[sp]` and returns (see [`spill_a0`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800066E4(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_a0(rdram, ctx)
}

/// `func_800066EC`: spills `a0` to `[sp]` and returns (see [`spill_a0`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800066EC(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_a0(rdram, ctx)
}

/// `func_800066F4`: spills `a0` to `[sp]` and returns (see [`spill_a0`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800066F4(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_a0(rdram, ctx)
}

/// `func_800066FC`: spills `a0` to `[sp]` and returns (see [`spill_a0`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800066FC(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_a0(rdram, ctx)
}
