//! Small leaves whose subsystem isn't known yet, grouped by address. They
//! move to a named module once what they belong to is understood.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use crate::recomp::{addu, enter, li, lw, reg::*, s32, sll, sw, RecompContext};

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

/// Object table searched by [`func_80006D5C`] (300 words, cleared by
/// [`func_80005B80`]); `[0x8009A2A0]` is the number of entries in use.
pub const OBJECTS: u32 = 0x800A_F4C0;

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

/// `func_80006F34`: spills `a0` to `[sp]` and returns (see [`spill_a0`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006F34(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_a0(rdram, ctx)
}

/// `func_80006F3C`: an empty function of two arguments returning 0. It
/// spills `a0` and `a1` to `[sp]` and `[sp + 4]`.
///
/// Domain: canonical `sp` with `[sp]..[sp + 8]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006F3C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    sw(&mut mem, g[SP], 0, g[A0]);
    sw(&mut mem, g[SP], 4, g[A1]);
    g[V0] = 0;
}

/// `func_80006FD4`: returns at once.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006FD4(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_80006FDC`: returns at once.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006FDC(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_8000758C`: returns at once.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000758C(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_80007710(&state)` (audio_dma_new): `*state = 0x800AFAC0` and return
/// `0x80007594`, the audio DMA callback (NOTES.md, "ROM asset loading path").
/// The shape of the libultra audio library's `ALDMANew`.
///
/// Domain: canonical `state` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80007710(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = li(0x800A_FAC0);
    g[V0] = li(0x8000_7594);
    sw(&mut mem, g[A0], 0, g[T6]);
}

/// `func_80007A44`: if `[0x8009A2B8]` is nonzero, zero the word at `+0x18` of
/// each of the eight 0x20-byte records at `0x800D2038`, four per iteration.
///
/// Leaves `v0 = 0x800D0000` and `v1 = 0x800D2038` if the flag is 0, else
/// `v0 = v1 = 0x800D2138`; `t6` = the flag.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80007A44(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = lw(m, li(0x800A_0000), -0x5D48);
    g[V1] = li(0x800D_2038);
    g[V0] = li(0x800D_0000); // beqz delay slot
    if g[T6] == 0 {
        return;
    }
    g[V0] = addu(g[V0], 0x2138);
    loop {
        g[V1] = addu(g[V1], 0x80);
        sw(m, g[V1], -0x48, 0);
        sw(m, g[V1], -0x28, 0);
        sw(m, g[V1], -0x8, 0);
        sw(m, g[V1], -0x68, 0); // bne delay slot
        if g[V1] == g[V0] {
            break;
        }
    }
}

/// `func_80007CE4(handle)`: look a handle up in the slot tables that
/// `func_80007E80` walks.
///
/// Bit 15 clear: returns `[0x800AFA54]`. Otherwise `k = (handle >> 24) &
/// 0xFF`: for `k` 0 or 1 it returns `A[(handle >> 16) & 0xFF]` with `A` the
/// 23 words at `0x8009A32C`, else `B[k]` with `B` the 8 words at
/// `0x8009A388`. QUIRK: nothing bounds the indices (up to 255), so both
/// lookups can read past their tables.
///
/// The shifts are N64Recomp's `sra` on the full register: for a non-canonical
/// `handle`, bits of the upper half reach the low word (on hardware the result
/// is undefined; real callers pass sign-extended words).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80007CE4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    let sra = |v: u64, s: u32| s32(((v as i64) >> s) as u32);
    g[T6] = g[A0] & 0x8000;
    g[V1] = sra(g[A0], 24); // bnez delay slot
    if g[T6] == 0 {
        g[V0] = lw(&mem, li(0x800B_0000), -0x5AC);
        return;
    }
    g[T7] = g[V1] & 0xFF;
    g[V1] = g[T7]; // beqz delay slot
    if g[T7] != 0 {
        g[AT] = 1;
        g[T1] = sll(g[V1], 2); // bne delay slot
        if g[T7] != g[AT] {
            g[V0] = addu(li(0x800A_0000), g[T1]);
            g[V0] = lw(&mem, g[V0], -0x5C78);
            return;
        }
    }
    // L_80007D14
    g[T8] = sra(g[A0], 16);
    g[T9] = g[T8] & 0xFF;
    g[T0] = sll(g[T9], 2);
    g[V0] = addu(li(0x800A_0000), g[T0]);
    g[V0] = lw(&mem, g[V0], -0x5CD4);
}
