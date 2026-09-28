//! Small leaves whose subsystem isn't known yet, grouped by address. They
//! move to a named module once what they belong to is understood.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use n64mem::Mem;
use crate::recomp::{addu, call, div, enter, fpu, lb, lbu, ld, lh, lhu, li, lw, multu, reg::*, s32, sb, sh, sll, sllv, slt, sltu, sra, srav, subu, sw, RecompContext};

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

/// `func_8000803C(o, k)`: 1 if entry `k` of `o`'s table has room, else 0.
///
/// The table is at `[o + 0x40]`, 0x30-byte entries. `p = [entry + 8]`; if
/// `p` is 0 it returns 0. Otherwise `q = [p + 0x38]` and it returns
/// `[q] + [q + 4] < [p + 0x54]`, a 32-bit sum compared **unsigned** (the
/// words are sign-extended, which keeps their unsigned order).
///
/// Leaves `t6 = 0x30 * k`, `t7` = the entry, `v1 = p`, and when `p != 0`:
/// `t8`, `t9`, `t0` (the sum), `t1` (the limit) and `at` (the result).
///
/// Domain: canonical `o`, `[o + 0x40] + 0x30 * k + 8`, `p` and `q` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000803C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, g[A0], 0x40);
    // t6 = k * 0x30: ((k << 2) - k) << 4
    g[T6] = sll(g[A1], 2);
    g[T6] = subu(g[T6], g[A1]);
    g[T6] = sll(g[T6], 4);
    g[T7] = addu(g[V0], g[T6]);
    g[V1] = lw(m, g[T7], 8);
    if g[V1] == 0 {
        g[V0] = 0;
        return;
    }
    g[V0] = lw(m, g[V1], 0x38);
    g[T1] = lw(m, g[V1], 0x54);
    g[T8] = lw(m, g[V0], 0);
    g[T9] = lw(m, g[V0], 4);
    g[T0] = addu(g[T8], g[T9]);
    g[AT] = sltu(g[T0], g[T1]);
    // bnez at: v0 = 1, else v0 = 0; the same as v0 = at.
    g[V0] = g[AT];
}

/// `func_80008530`: returns at once.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80008530(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_80008540`: spills `a0` to `[sp]` and returns (see [`spill_a0`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80008540(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_a0(rdram, ctx)
}

/// An on/off switch kept as an "off" flag word: `switch(on)` with `on == 0`
/// sets the flag to 1, `on == 1` clears it, `on == -1` toggles it (flag =
/// flag == 0), anything else leaves it. Returns 1 if the flag is now 0.
/// `on` is compared as a full 64-bit register. Two instances,
/// [`func_80008630`] and [`func_80008694`], reached only through pointers.
///
/// Leaves `t6 = 1`, `a0 = flag`, `v1` = the flag's new value; `at` = 1 for
/// `on == 1`, -1 for other nonzero `on`, unchanged for 0; `t7` = the new
/// value only when toggling.
fn off_switch(rdram: *mut u8, ctx: *mut RecompContext, flag: u32) {
    // SAFETY: forwarded from the entry points below.
    let (mut mem, ctx) = unsafe { enter(rdram, ctx) };
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = 1;
    if g[A0] == 0 {
        sw(m, li(flag), 0, g[T6]);
    } else {
        g[AT] = 1;
        if g[A0] == g[AT] {
            sw(m, li(flag), 0, 0);
        } else {
            g[AT] = u64::MAX;
            if g[A0] == g[AT] {
                g[V1] = lw(m, li(flag), 0);
                g[T7] = sltu(g[V1], 1);
                sw(m, li(flag), 0, g[T7]);
            }
        }
    }
    g[A0] = li(flag);
    g[V1] = lw(m, g[A0], 0);
    g[V0] = sltu(g[V1], 1);
}

/// `func_80008630(on)`: [`off_switch`] on the flag at `0x8009A2C4`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80008630(rdram: *mut u8, ctx: *mut RecompContext) {
    off_switch(rdram, ctx, 0x8009_A2C4)
}

/// `func_80008694(on)`: [`off_switch`] on the flag at `0x8009A2C0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80008694(rdram: *mut u8, ctx: *mut RecompContext) {
    off_switch(rdram, ctx, 0x8009_A2C0)
}

/// `func_80008718(c)`: 1 if `0x8E <= c < 0x9E` or `c == 0x22`, else 0
/// (signed 64-bit compares). Called twice by `func_80008760`; the numbers
/// look like character codes (0x22 is `"`), which is a **guess**.
///
/// Leaves `at` = 1 when it returns 1 from the range test, else 0x22.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80008718(rdram: *mut u8, ctx: *mut RecompContext) {
    let (_mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0x8E);
    let below = g[AT] != 0;
    g[AT] = slt(g[A0], 0x9E);
    if !below && g[AT] != 0 {
        g[V0] = 1;
        return;
    }
    g[AT] = 0x22;
    g[V0] = u64::from(g[A0] == g[AT]);
}

/// `func_80008750(b)`: spills `a0` to `[sp]` and stores its low byte at
/// `0x8009A324`. Leaves `at = 0x800A0000`.
///
/// Domain: canonical `sp` with `[sp]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80008750(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    sw(m, g[SP], 0, g[A0]);
    sb(m, g[AT], -0x5CDC, g[A0]);
}

/// The eight halfword tables [`func_80008F6C`] indexes, by kind: the table's
/// length (the bound on the index), its offset from `0x800A0000` and the
/// temporary the index's byte offset goes through. They sit back to back
/// from `0x8009A6F0`, each 4-aligned.
const HANDLE_TABLES: [(u64, i32, usize); 8] = [
    (0x33, -0x5910, T7),
    (0x26, -0x58A8, T8),
    (0x39, -0x585C, T9),
    (5, -0x57E8, T0),
    (0x68, -0x57DC, T1),
    (0xA9, -0x570C, T2),
    (0x69, -0x55B8, T3),
    (0xA8, -0x54E4, T4),
];

/// `func_80008F6C(kind, a1, index)`: a 32-bit handle, `(kind << 24) | (a1
/// << 16) | table[kind][index] | 0x8000`, or -1 if an argument is out of
/// range. It looks like the handles [`func_80007CE4`] decodes (bit 15,
/// bytes 2 and 3), but that is a guess.
///
/// -1 when `index == -1` (full 64-bit compare), `kind >= 8` (unsigned), for
/// kinds 0 and 1 only `a1` outside `[0, 0x17)` (signed), or `index` outside
/// `(0, len)` (`> 0` signed, `< len` unsigned) for that kind's table in
/// [`HANDLE_TABLES`]. Entry 0 is never read. QUIRK: the entry is loaded
/// with `lh`, so a negative one sets every bit above 15 and wipes the kind
/// and `a1` fields. Kinds 2-7 put any `a1` in the handle, shifted and
/// truncated to 32 bits.
///
/// The kind picks its case through a jump table at `0x800A8200`. The C's
/// `default` (`switch_error`) can't be reached: `kind < 8` is checked just
/// before. Leaves `at` = the last range test (`-1` if `index == -1`), the
/// kind's temporary = `index << 1` once `index > 0`, and `t5`/`t6`/`t7`/`v1`
/// the handle's parts on success. On failure after the switch `t6` =
/// `0x800A8200 + 4 * kind`, the table entry's address: N64Recomp turns the
/// table's `lw` into an `addiu`. The hardware would have loaded the case
/// address, and the port follows the C.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80008F6C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = u64::MAX;
    if g[A2] == g[AT] {
        g[V0] = u64::MAX;
        return;
    }
    g[AT] = sltu(g[A0], 8);
    g[T6] = sll(g[A0], 2);
    if g[AT] == 0 {
        g[V0] = u64::MAX;
        return;
    }
    g[AT] = addu(li(0x800B_0000), g[T6]);
    let jr_addend = g[T6];
    g[T6] = addu(g[AT], (-0x7E00i64) as u64); // the table's lw, as N64Recomp emits it
    let kind = (jr_addend >> 2) as usize; // 0..=7
    let (len, table, t) = HANDLE_TABLES[kind];
    if kind < 2 {
        g[AT] = slt(g[A1], 0x17);
        if (g[A1] as i64) < 0 || g[AT] == 0 {
            g[V0] = u64::MAX;
            return;
        }
    }
    g[AT] = sltu(g[A2], len);
    if (g[A2] as i64) <= 0 {
        g[V0] = u64::MAX;
        return;
    }
    g[t] = sll(g[A2], 1);
    if g[AT] == 0 {
        g[V0] = u64::MAX;
        return;
    }
    g[V1] = lh(&mem, addu(li(0x800A_0000), g[t]), table);
    g[T5] = sll(g[A0], 24);
    g[T6] = g[V1] | g[T5];
    g[T7] = sll(g[A1], 16);
    g[V1] = g[T6] | g[T7];
    g[V0] = g[V1] | 0x8000;
}

/// The last three values pushed by [`func_80009278`] (halfwords), searched
/// by [`func_800092B0`]; `func_800092EC` uses both, a "recently seen" list.
pub const RECENT: u32 = 0x8009_ADF4;
/// The next slot of [`RECENT`] to write (a halfword).
pub const RECENT_NEXT: u32 = 0x8009_ADFC;

/// `func_80009278(v)`: `RECENT[next] = v` (halfword), then `next = (next +
/// 1) % 3`. QUIRK: `next` is read back unchecked and the remainder is
/// signed, so a slot index outside 0..3 writes outside the list (and a
/// negative one stays negative).
///
/// Leaves `v1 = RECENT_NEXT`, `v0` = the old `next` (sign-extended), `t6 =
/// 2 * next`, `at = 3`, `t7 = next + 1`, `t8` = the new `next`.
///
/// Domain: `RECENT + 2 * next` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80009278(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = li(RECENT_NEXT);
    g[V0] = lh(m, g[V1], 0);
    g[AT] = li(0x800A_0000);
    g[T6] = sll(g[V0], 1);
    g[AT] = addu(g[AT], g[T6]);
    sh(m, g[AT], -0x520C, g[A0]);
    g[AT] = 3;
    g[T7] = addu(g[V0], 1);
    let (_lo, hi) = div(g[T7], g[AT]); // `lo` is never read
    g[T8] = hi;
    sh(m, g[V1], 0, g[T8]);
}

/// `func_800092B0(v)`: 1 if `v` equals one of the three [`RECENT`]
/// halfwords, else 0. Each is sign-extended and compared with the whole
/// 64-bit `v`.
///
/// Leaves `v1` = the address after the match (or the list's end) and `t6` =
/// the last halfword compared.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800092B0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V1] = li(RECENT);
    g[V0] = li(RECENT + 6);
    g[T6] = lh(m, g[V1], 0);
    loop {
        g[V1] = addu(g[V1], 2);
        if g[A0] == g[T6] {
            g[V0] = 1;
            return;
        }
        if g[V1] == g[V0] {
            g[V0] = 0;
            return;
        }
        g[T6] = lh(m, g[V1], 0);
    }
}

/// Flag words indexed by [`func_80009524`], [`func_8000953C`] and
/// [`func_8000955C`] (test, set, clear). How many there are isn't known.
pub const FLAG_WORDS: u32 = 0x800D_2140;

/// `func_80009524(k, bits)`: `FLAG_WORDS[k] & bits`. QUIRK: `k` is unbounded.
///
/// Leaves `t6 = 4k` and `t7` = the word.
///
/// Domain: `FLAG_WORDS + 4k` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80009524(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 2);
    g[T7] = addu(li(0x800D_0000), g[T6]);
    g[T7] = lw(&mem, g[T7], 0x2140);
    g[V0] = g[T7] & g[A1];
}

/// `func_8000953C(k, bits)`: `FLAG_WORDS[k] |= bits`. QUIRK: `k` is unbounded.
///
/// Leaves `t7 = FLAG_WORDS`, `t6 = 4k`, `v0` = the word's address, `t8` =
/// the old word and `t9` the new one.
///
/// Domain: `FLAG_WORDS + 4k` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000953C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T7] = li(FLAG_WORDS);
    g[T6] = sll(g[A0], 2);
    g[V0] = addu(g[T6], g[T7]);
    g[T8] = lw(m, g[V0], 0);
    g[T9] = g[T8] | g[A1];
    sw(m, g[V0], 0, g[T9]);
}

/// `func_8000955C(k, bits)`: `FLAG_WORDS[k] &= !bits`. QUIRK: `k` is unbounded.
///
/// Leaves `t7 = FLAG_WORDS`, `t6 = 4k`, `v0` = the word's address, `t8` =
/// the old word, `t9 = !bits` (all 64 bits) and `t0` the new word.
///
/// Domain: `FLAG_WORDS + 4k` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000955C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T7] = li(FLAG_WORDS);
    g[T6] = sll(g[A0], 2);
    g[V0] = addu(g[T6], g[T7]);
    g[T8] = lw(m, g[V0], 0);
    g[T9] = !g[A1];
    g[T0] = g[T8] & g[T9];
    sw(m, g[V0], 0, g[T0]);
}

/// `func_8000A418(v)`: append `v` to the list of up to 64 words at
/// `0x800D3A90`, whose count is `[0x8009B774]` (signed); when full, nothing
/// happens. Reached only through pointers.
///
/// Leaves `v1 = 0x8009B774`, `v0` = the old count, `at` = 1 if it appended
/// (then `at = 0x800D0000 + 4 * count`, `t7` = the new count) else 0, and
/// `t6 = 4 * count`.
///
/// Domain: counts below 64 (negative ones too) index RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000A418(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = li(0x8009_B774);
    g[V0] = lw(m, g[V1], 0);
    g[AT] = slt(g[V0], 0x40);
    g[T6] = sll(g[V0], 2);
    if g[AT] != 0 {
        g[AT] = addu(li(0x800D_0000), g[T6]);
        sw(m, g[AT], 0x3A90, g[A0]);
        g[T7] = addu(g[V0], 1);
        sw(m, g[V1], 0, g[T7]);
    }
}

/// 32-byte records indexed by a signed 16-bit id: `+4`/`+6` halfwords
/// ([`func_8000AA78`]), `+0x14` flags with bit `0x20` an "on" bit
/// ([`func_8000A920`], [`func_8000AC34`], [`func_8000AC60`]), four bytes at
/// `+0x18` ([`func_8000AB24`], a colour by the look of it: **guess**) and a
/// pointer at `+0x1C` ([`func_8000ABD4`], [`func_8000AC0C`]). How many there
/// are isn't known. Ids -201, -103 and -104 name globals instead.
pub const RECORDS: u32 = 0x800D_2190;

/// `func_8000A920(id, on)`: turn something on or off, `on` = `a1 != 0`
/// (64-bit). `id` is `a0`'s low halfword, signed:
/// - -201: the word at `0x8009B778` = 1 or 0;
/// - -103, -104: the byte at `0x8009B77F` / `0x8009B783` = 0xFF or 0, the
///   fourth byte of the two globals [`func_8000AB24`] sets for those ids;
/// - `id >= 0`: set or clear bit `0x20` of `RECORDS[id]`'s flags;
/// - other negative ids: nothing.
///
/// Spills `a0` to `[sp]`; leaves `a0 = id`, `at` = the last constant
/// compared (or `0x800A0000` for a global, or `!0x20` when clearing a
/// record bit), and the temporaries each path uses: `t8`/`t9`/`t0` = 1 or
/// 0xFF when turning a global on; `t5 = 32 * id` and `v0` = the record,
/// with `t1`-`t4` (on) or `t6`-`t8` (off) for records.
///
/// Domain: canonical `sp` with `[sp]` in RDRAM; for `id >= 0`,
/// `RECORDS[id]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000A920(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0, g[A0]);
    g[T6] = sll(g[A0], 16);
    g[A0] = sra(g[T6], 16);
    g[AT] = (-0xC9i64) as u64;
    if g[A0] == g[AT] {
        g[AT] = li(0x800A_0000);
        let v = if g[A1] == 0 { 0 } else { g[T8] = 1; g[T8] };
        sw(m, g[AT], -0x4888, v);
        return;
    }
    g[AT] = (-0x67i64) as u64;
    if g[A0] == g[AT] {
        g[AT] = li(0x800A_0000);
        let v = if g[A1] == 0 { 0 } else { g[T9] = 0xFF; g[T9] };
        sb(m, g[AT], -0x4881, v);
        return;
    }
    g[AT] = (-0x68i64) as u64;
    if g[A0] == g[AT] {
        g[AT] = li(0x800A_0000);
        let v = if g[A1] == 0 { 0 } else { g[T0] = 0xFF; g[T0] };
        sb(m, g[AT], -0x487D, v);
        return;
    }
    if (g[A0] as i64) < 0 {
        return;
    }
    g[T5] = sll(g[A0], 5);
    if g[A1] != 0 {
        g[T2] = li(RECORDS);
        g[T1] = sll(g[A0], 5);
        g[V0] = addu(g[T1], g[T2]);
        g[T3] = lw(m, g[V0], 0x14);
        g[T4] = g[T3] | 0x20;
        sw(m, g[V0], 0x14, g[T4]);
    } else {
        g[T6] = li(RECORDS);
        g[V0] = addu(g[T5], g[T6]);
        g[T7] = lw(m, g[V0], 0x14);
        g[AT] = (-0x21i64) as u64;
        g[T8] = g[T7] & g[AT];
        sw(m, g[V0], 0x14, g[T8]);
    }
}

/// `func_8000AA78(id, x, y)`: for `id >= 0` (`a0`'s low halfword, signed),
/// store the low halfwords of `x` and `y` at `RECORDS[id] + 4` and `+ 6`.
/// Negative ids do nothing.
///
/// Spills `a0`-`a2` to `[sp]..[sp + 0xC]`; leaves `t6`-`t9`, `t0`, `t1`
/// (the shifted halfwords) and, for `id >= 0`, `t2`, `t3`, `v0` = the record.
///
/// Domain: canonical `sp` with its argument slots in RDRAM; for `id >= 0`,
/// `RECORDS[id]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AA78(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 16);
    g[T7] = sra(g[T6], 16);
    g[T8] = sll(g[A1], 16);
    g[T0] = sll(g[A2], 16);
    g[T1] = sra(g[T0], 16);
    g[T9] = sra(g[T8], 16);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[SP], 4, g[A1]);
    sw(m, g[SP], 8, g[A2]);
    if (g[T7] as i64) >= 0 {
        g[T3] = li(RECORDS);
        g[T2] = sll(g[T7], 5);
        g[V0] = addu(g[T2], g[T3]);
        sh(m, g[V0], 4, g[T9]);
        sh(m, g[V0], 6, g[T1]);
    }
}

/// `func_8000AB24(id, r, g, b, a)`: store four bytes (the low bytes of
/// `a1`-`a3` and of the fifth argument, the word at `[sp + 0x10]`) at
/// `0x8009B77C` for id -103, `0x8009B780` for id -104, or `RECORDS[id] +
/// 0x18` for `id >= 0` (`a0`'s low halfword, signed). Other negative ids do
/// nothing. [`func_8000A920`] turns the -103/-104 ones on and off through
/// their fourth byte.
///
/// Spills `a0`-`a3` to `[sp]..[sp + 0x10]`; leaves `a0 = id`, `a1`-`a3` and
/// `t8`, `t9`, `t0` = the low bytes, `at` = the last id compared, and the
/// temporaries of the path taken (`v0` = where the bytes went, `t1`/`t2`/
/// `t5` = the fourth byte, `t3`, `t4`).
///
/// Domain: canonical `sp` with its argument slots in RDRAM; for `id >= 0`,
/// `RECORDS[id]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AB24(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0, g[A0]);
    g[T6] = sll(g[A0], 16);
    g[A0] = sra(g[T6], 16);
    sw(m, g[SP], 4, g[A1]);
    g[T8] = g[A1] & 0xFF;
    sw(m, g[SP], 8, g[A2]);
    g[T9] = g[A2] & 0xFF;
    sw(m, g[SP], 0xC, g[A3]);
    g[T0] = g[A3] & 0xFF;
    g[AT] = (-0x67i64) as u64;
    g[A3] = g[T0];
    g[A2] = g[T9];
    g[A1] = g[T8];
    if g[A0] == g[AT] {
        g[V0] = li(0x8009_B77C);
        g[T1] = lbu(m, g[SP], 0x13);
        sb(m, g[V0], 0, g[T8]);
        sb(m, g[V0], 1, g[T9]);
        sb(m, g[V0], 2, g[T0]);
        sb(m, g[V0], 3, g[T1]);
        return;
    }
    g[AT] = (-0x68i64) as u64;
    g[V0] = li(0x800A_0000);
    if g[A0] == g[AT] {
        g[V0] = addu(g[V0], (-0x4880i64) as u64);
        g[T2] = lbu(m, g[SP], 0x13);
        sb(m, g[V0], 0, g[A1]);
        sb(m, g[V0], 1, g[A2]);
        sb(m, g[V0], 2, g[A3]);
        sb(m, g[V0], 3, g[T2]);
        return;
    }
    g[T4] = li(0x800D_0000);
    if (g[A0] as i64) >= 0 {
        g[T4] = addu(g[T4], 0x2190);
        g[T3] = sll(g[A0], 5);
        g[V0] = addu(g[T3], g[T4]);
        g[T5] = lbu(m, g[SP], 0x13);
        sb(m, g[V0], 0x18, g[A1]);
        sb(m, g[V0], 0x19, g[A2]);
        sb(m, g[V0], 0x1A, g[A3]);
        sb(m, g[V0], 0x1B, g[T5]);
    }
}

/// `func_8000ABD4(id)`: `p = RECORDS[id] + 0x1C` (the record's pointer);
/// returns `[p + 8]`, or 0 if `p` is null. QUIRK: unlike its setter
/// [`func_8000AC0C`], it doesn't check `id >= 0`, so a negative id reads
/// before the table.
///
/// Spills `a0` to `[sp]`; leaves `t6`-`t8` and `v1 = p`.
///
/// Domain: canonical `sp` with `[sp]` in RDRAM; `RECORDS[id]` and a nonzero
/// `p + 8` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000ABD4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 16);
    g[T7] = sra(g[T6], 16);
    g[T8] = sll(g[T7], 5);
    g[V1] = addu(li(0x800D_0000), g[T8]);
    g[V1] = lw(m, g[V1], 0x21AC);
    sw(m, g[SP], 0, g[A0]);
    g[V0] = if g[V1] == 0 { 0 } else { lw(m, g[V1], 8) };
}

/// `func_8000AC0C(id, p)`: for `id >= 0`, `RECORDS[id] + 0x1C = p` (see
/// [`func_8000ABD4`]); negative ids do nothing.
///
/// Spills `a0` to `[sp]`; leaves `t6`, `t7 = id` and, for `id >= 0`, `t8 =
/// 32 * id` and `at` = `0x800D0000 + t8`.
///
/// Domain: canonical `sp` with `[sp]` in RDRAM; for `id >= 0`,
/// `RECORDS[id]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AC0C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 16);
    g[T7] = sra(g[T6], 16);
    sw(m, g[SP], 0, g[A0]);
    if (g[T7] as i64) >= 0 {
        g[T8] = sll(g[T7], 5);
        g[AT] = addu(li(0x800D_0000), g[T8]);
        sw(m, g[AT], 0x21AC, g[A1]);
    }
}

/// `func_8000AC34(id, bits)`: `RECORDS[id]`'s flags `|= bits`. QUIRK: no
/// `id >= 0` check.
///
/// Spills `a0` to `[sp]`; leaves `t6`-`t9`, `v0` = the record, `t0` = the
/// old flags and `t1` the new.
///
/// Domain: canonical `sp` with `[sp]` in RDRAM; `RECORDS[id]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AC34(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 16);
    g[T7] = sra(g[T6], 16);
    g[T9] = li(RECORDS);
    g[T8] = sll(g[T7], 5);
    g[V0] = addu(g[T8], g[T9]);
    g[T0] = lw(m, g[V0], 0x14);
    sw(m, g[SP], 0, g[A0]);
    g[T1] = g[T0] | g[A1];
    sw(m, g[V0], 0x14, g[T1]);
}

/// `func_8000AC60(id, bits)`: `RECORDS[id]`'s flags `&= !bits`. QUIRK: no
/// `id >= 0` check.
///
/// Spills `a0` to `[sp]`; leaves `t6`-`t9`, `v0` = the record, `t0` = the
/// old flags, `t1 = !bits` (all 64 bits) and `t2` the new flags.
///
/// Domain: canonical `sp` with `[sp]` in RDRAM; `RECORDS[id]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AC60(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 16);
    g[T7] = sra(g[T6], 16);
    g[T9] = li(RECORDS);
    g[T8] = sll(g[T7], 5);
    g[V0] = addu(g[T8], g[T9]);
    g[T0] = lw(m, g[V0], 0x14);
    g[T1] = !g[A1];
    sw(m, g[SP], 0, g[A0]);
    g[T2] = g[T0] & g[T1];
    sw(m, g[V0], 0x14, g[T2]);
}

/// Pointer to an array of 0x7C-byte entries: `+0` flags (bit 0 marks the
/// selected one, [`func_8000B1B0`]), `+4`/`+6` halfwords and `+8` a word
/// ([`func_8000AEFC`]), `+0xC` halfword and `+0x10` word ([`func_8000B02C`]),
/// `+0x78` a word ([`func_8000B06C`]).
pub const ENTRIES: u32 = 0x8009_B790;
/// The selected entry's index ([`func_8000B1B0`]), -1 for none.
pub const SELECTED: u32 = 0x8009_B798;

/// `func_8000AEB4(k)`: the flags word of entry `k` of [`ENTRIES`], with `k`
/// the whole register's low word (not truncated to 16 bits like the
/// others). QUIRK: unbounded.
///
/// Leaves `t6` = the array, `t7 = 0x7C * k`, `t8` = the entry.
///
/// Domain: the entry in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AEB4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[T6] = lw(m, li(ENTRIES), 0);
    // t7 = k * 0x7C: ((k << 5) - k) << 2
    g[T7] = sll(g[A0], 5);
    g[T7] = subu(g[T7], g[A0]);
    g[T7] = sll(g[T7], 2);
    g[T8] = addu(g[T6], g[T7]);
    g[V0] = lw(m, g[T8], 0);
}

/// `func_8000AED4(k, bits)`: entry `k`'s flags `|= bits` (`k` as in
/// [`func_8000AEB4`]).
///
/// Leaves `t6` = the array, `t7 = 0x7C * k`, `v0` = the entry, `t8`/`t9` =
/// the old/new flags.
///
/// Domain: the entry in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AED4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = lw(m, li(ENTRIES), 0);
    g[T7] = sll(g[A0], 5);
    g[T7] = subu(g[T7], g[A0]);
    g[T7] = sll(g[T7], 2);
    g[V0] = addu(g[T6], g[T7]);
    g[T8] = lw(m, g[V0], 0);
    g[T9] = g[T8] | g[A1];
    sw(m, g[V0], 0, g[T9]);
}

/// `func_8000AEFC(id, x, w, y)`: entry `id` (low halfword, signed) of
/// [`ENTRIES`] gets `+6 = y`, `+4 = x` (halfwords) and `+8 = w`, in that
/// order, reloading the array pointer before each store.
///
/// Spills `a0`, `a1` and `a3` (not `a2`) to their argument slots; leaves
/// `v1 = ENTRIES`, `v0 = 0x7C * id`, `t6`/`t7` and `t2`-`t5` (the array and
/// entry, per store).
///
/// Domain: canonical `sp` with its argument slots in RDRAM; the entry in
/// RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AEFC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 16);
    g[V1] = li(ENTRIES);
    g[T7] = sra(g[T6], 16);
    g[T2] = lw(m, g[V1], 0);
    g[V0] = sll(g[T7], 5);
    g[V0] = subu(g[V0], g[T7]);
    g[V0] = sll(g[V0], 2);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[SP], 4, g[A1]);
    sw(m, g[SP], 0xC, g[A3]);
    g[T3] = addu(g[T2], g[V0]);
    sh(m, g[T3], 6, g[A3]);
    g[T4] = lw(m, g[V1], 0);
    g[T5] = addu(g[T4], g[V0]);
    sh(m, g[T5], 4, g[A1]);
    g[T6] = lw(m, g[V1], 0);
    g[T7] = addu(g[T6], g[V0]);
    sw(m, g[T7], 8, g[A2]);
}

/// `func_8000B02C(id, w, h)`: entry `id` (low halfword, signed) gets `+0xC
/// = h` (halfword) then `+0x10 = w`, reloading the array pointer between.
///
/// Spills `a0` and `a2`; leaves `v1 = ENTRIES`, `v0 = 0x7C * id`, `t6`,
/// `t7`, `t0`-`t3`.
///
/// Domain: canonical `sp` with its argument slots in RDRAM; the entry in
/// RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000B02C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 16);
    g[V1] = li(ENTRIES);
    g[T7] = sra(g[T6], 16);
    g[T0] = lw(m, g[V1], 0);
    g[V0] = sll(g[T7], 5);
    g[V0] = subu(g[V0], g[T7]);
    g[V0] = sll(g[V0], 2);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[SP], 8, g[A2]);
    g[T1] = addu(g[T0], g[V0]);
    sh(m, g[T1], 0xC, g[A2]);
    g[T2] = lw(m, g[V1], 0);
    g[T3] = addu(g[T2], g[V0]);
    sw(m, g[T3], 0x10, g[A1]);
}

/// `func_8000B06C(id, v)`: entry `id` (low halfword, signed) gets `+0x78 = v`.
///
/// Spills `a0`; leaves `t6`-`t9` and `t0` = the entry.
///
/// Domain: canonical `sp` with `[sp]` in RDRAM; the entry in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000B06C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 16);
    g[T7] = sra(g[T6], 16);
    g[T8] = lw(m, li(ENTRIES), 0);
    g[T9] = sll(g[T7], 5);
    g[T9] = subu(g[T9], g[T7]);
    g[T9] = sll(g[T9], 2);
    sw(m, g[SP], 0, g[A0]);
    g[T0] = addu(g[T8], g[T9]);
    sw(m, g[T0], 0x78, g[A1]);
}

/// `func_8000B098(id)`: bit 0 of entry `id`'s flags (low halfword, signed):
/// 1 if it is the selected entry.
///
/// Spills `a0`; leaves `t6`-`t9`, `t0` = the entry, `t1` = its flags and
/// `t2` = the bit.
///
/// Domain: canonical `sp` with `[sp]` in RDRAM; the entry in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000B098(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 16);
    g[T7] = sra(g[T6], 16);
    g[T8] = lw(m, li(ENTRIES), 0);
    g[T9] = sll(g[T7], 5);
    g[T9] = subu(g[T9], g[T7]);
    g[T9] = sll(g[T9], 2);
    sw(m, g[SP], 0, g[A0]);
    g[T0] = addu(g[T8], g[T9]);
    g[T1] = lw(m, g[T0], 0);
    g[T2] = g[T1] & 1;
    // v0 = 0; bnez t2: v0 = 1; the same as v0 = t2.
    g[V0] = g[T2];
}

/// `func_8000B1B0(id)`: select entry `id` (low halfword, signed; -1 for
/// none). If [`SELECTED`] isn't -1, clear bit 0 of that entry's flags. Then
/// `SELECTED = id`, and if `id` isn't -1: `[0x8009B794]` = the entry's
/// address, set bit 0 of its flags, and `[0x8009B79C] = 0`. The index
/// products are `multu` low words (the same as a signed product's).
///
/// Spills `a0`; leaves `a2 = SELECTED`, `a1 = -1`, `a3 = 0x7C`, `a0 = id`,
/// `v0` = the old selection (or, if `id != -1`, the array), and the
/// temporaries of the paths taken (`t6`, `t8`-`t9`, `t0`-`t7`, `v1`, `at`).
///
/// Domain: canonical `sp` with `[sp]` in RDRAM; the old and new entries in
/// RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000B1B0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A2] = li(SELECTED);
    g[V0] = lw(m, g[A2], 0);
    sw(m, g[SP], 0, g[A0]);
    g[T6] = sll(g[A0], 16);
    g[A1] = u64::MAX;
    g[A0] = sra(g[T6], 16);
    if g[A1] != g[V0] {
        // Deselect the old entry.
        g[A3] = 0x7C;
        let (lo, _) = multu(g[V0], g[A3]);
        g[T8] = lw(m, li(ENTRIES), 0);
        g[AT] = (-2i64) as u64;
        g[T9] = lo;
        g[V1] = addu(g[T8], g[T9]);
        g[T0] = lw(m, g[V1], 0);
        g[T1] = g[T0] & g[AT];
        sw(m, g[V1], 0, g[T1]);
    }
    g[A3] = 0x7C;
    sw(m, g[A2], 0, g[A0]);
    if g[A0] == g[A1] {
        return;
    }
    let (lo, _) = multu(g[A0], g[A3]);
    g[V0] = lw(m, li(ENTRIES), 0);
    g[AT] = li(0x800A_0000);
    g[T2] = lo;
    g[T3] = addu(g[T2], g[V0]);
    sw(m, g[AT], -0x486C, g[T3]);
    g[T4] = lw(m, g[A2], 0);
    g[AT] = li(0x800A_0000);
    let (lo, _) = multu(g[T4], g[A3]);
    g[T5] = lo;
    g[V1] = addu(g[V0], g[T5]);
    g[T6] = lw(m, g[V1], 0);
    g[T7] = g[T6] | 1;
    sw(m, g[V1], 0, g[T7]);
    sw(m, g[AT], -0x4864, 0);
}

/// `func_8000787C(x)`: if the flag `[0x8009A2B8]` (also tested by
/// [`func_80007A44`]) is set, `[0x8009A328] = trunc(x * 32000.0)`, with `x`
/// the float argument in `f12`. The scale suggests an audio volume
/// (**guess**). The conversion is `trunc.w.s`, N64Recomp's C cast: NaN or a
/// product outside `i32` stores `0x80000000` (host behaviour, NOTES.md,
/// "Floats"; the hardware would trap, FCR31.EV).
///
/// Leaves `t6` = the flag and `at = 0x46FA0000` (32000.0), and when the flag
/// is set `at = 0x800A0000`, `f4 = 32000.0`, `f6` = the product, `f8` and
/// `t8` = the stored word.
///
/// Domain: `x` not NaN (the oracle asserts on NaN operands).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000787C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = lw(m, li(0x800A_0000), -0x5D48);
    g[AT] = li(0x46FA_0000);
    if g[T6] == 0 {
        return;
    }
    f[4].set_u32l(g[AT] as u32);
    g[AT] = li(0x800A_0000);
    f[6].set_fl(f[12].fl() * f[4].fl());
    f[8].set_u32l(fpu::trunc_w_s(f[6].fl()));
    g[T8] = s32(f[8].u32l());
    sw(m, g[AT], -0x5CD8, g[T8]);
}

/// `func_8000C530`: returns at once.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000C530(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// The current id of the push/pop pair [`func_8000C5F0`] / [`func_8000C658`].
pub const CURRENT_ID: u32 = 0x8009_B800;
/// The current id's live word; saved to `SAVED_WORDS[id]` when it changes.
pub const CURRENT_WORD: u32 = 0x8009_B7DC;
/// One saved word per id. There is room for three before [`CURRENT_ID`]:
/// `SAVED_WORDS[3]` is `CURRENT_ID` itself.
pub const SAVED_WORDS: u32 = 0x8009_B7F4;
/// The stack of ids (words); `ID_STACK[depth]` is the current one.
pub const ID_STACK: u32 = 0x800D_5718;
/// The stack depth.
pub const ID_DEPTH: u32 = 0x800D_578C;

/// `func_8000C5F0(id)`: push `id` as the current id.
/// `SAVED_WORDS[current] = CURRENT_WORD`, `depth += 1`, `ID_STACK[depth] =
/// id`, then `CURRENT_ID = id` and `CURRENT_WORD = SAVED_WORDS[id]`.
/// QUIRK: nothing bounds the depth or the ids; ids from 3 up alias
/// `CURRENT_ID` and what follows it.
///
/// Leaves `v1 = CURRENT_ID`, `a1 = CURRENT_WORD`, `v0 = SAVED_WORDS`, `a2 =
/// ID_DEPTH`, `at`, `t0` = the old depth, `t1` the new, `t3`-`t9` the
/// scaled indices and words.
///
/// Domain: the ids and depth index RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000C5F0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = li(CURRENT_ID);
    g[T7] = lw(m, g[V1], 0);
    g[A1] = li(CURRENT_WORD);
    g[T6] = lw(m, g[A1], 0);
    g[V0] = li(SAVED_WORDS);
    g[T8] = sll(g[T7], 2);
    g[T9] = addu(g[V0], g[T8]);
    g[A2] = li(ID_DEPTH);
    sw(m, g[T9], 0, g[T6]);
    g[T0] = lw(m, g[A2], 0);
    g[T4] = sll(g[A0], 2);
    g[T1] = addu(g[T0], 1);
    g[T3] = sll(g[T1], 2);
    sw(m, g[A2], 0, g[T1]);
    g[AT] = addu(li(0x800D_0000), g[T3]);
    sw(m, g[AT], 0x5718, g[A0]);
    g[T5] = addu(g[V0], g[T4]);
    g[T7] = lw(m, g[T5], 0);
    sw(m, g[V1], 0, g[A0]);
    sw(m, g[A1], 0, g[T7]);
}

/// `func_8000C658()`: pop the current id. `SAVED_WORDS[current] =
/// CURRENT_WORD`; if `depth > 0` (signed) it decrements; then `CURRENT_ID =
/// ID_STACK[depth]` and `CURRENT_WORD = SAVED_WORDS[CURRENT_ID]`. QUIRK: at
/// depth 0 or below it doesn't decrement and reloads `ID_STACK[depth]`.
///
/// Leaves `a0 = CURRENT_ID`, `a1 = CURRENT_WORD`, `v1 = SAVED_WORDS`, `a2 =
/// ID_DEPTH`, `v0` = the depth now, `t0` = the old depth - 1, `t2` = the new
/// id, `t1`, `t4`-`t9`.
///
/// Domain: the ids and depth index RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000C658(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A0] = li(CURRENT_ID);
    g[T7] = lw(m, g[A0], 0);
    g[A1] = li(CURRENT_WORD);
    g[T6] = lw(m, g[A1], 0);
    g[V1] = li(SAVED_WORDS);
    g[T8] = sll(g[T7], 2);
    g[T9] = addu(g[V1], g[T8]);
    g[A2] = li(ID_DEPTH);
    sw(m, g[T9], 0, g[T6]);
    g[V0] = lw(m, g[A2], 0);
    g[T2] = li(0x800D_0000);
    g[T0] = addu(g[V0], u64::MAX);
    if (g[V0] as i64) > 0 {
        sw(m, g[A2], 0, g[T0]);
        g[V0] = g[T0];
    }
    g[T1] = sll(g[V0], 2);
    g[T2] = addu(g[T2], g[T1]);
    g[T2] = lw(m, g[T2], 0x5718);
    g[T4] = sll(g[T2], 2);
    g[T5] = addu(g[V1], g[T4]);
    g[T7] = lw(m, g[T5], 0);
    sw(m, g[A0], 0, g[T2]);
    sw(m, g[A1], 0, g[T7]);
}

/// `func_8000DA6C()`: returns `[0x8009B7E4]`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000DA6C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    ctx.gpr[V0] = lw(&mem, li(0x800A_0000), -0x481C);
}

/// `func_8000E9BC(p, b0, b1, b2, b3, b4, b5)`: if `p` and `q = [p + 0xC]`
/// are nonzero, store each of the six values as a byte at `q + 0x20` ..
/// `q + 0x25`, skipping negative ones. Each value is the low halfword of its
/// argument, signed: `a1`-`a3`, then the fifth to seventh arguments from
/// the stack (`[sp + 0x12]`, `[sp + 0x16]`, `[sp + 0x1A]`).
///
/// Spills `a1`-`a3` to their argument slots; leaves `t6`, `t8`, `t0`, `t7`
/// = the first value, `a2`, `a3` the next two, `v0 = q` if `p != 0`, and `v1`
/// = the last stack value read (all three when `q != 0`).
///
/// Domain: canonical `sp` with the argument slots in RDRAM; canonical `p`,
/// and `q` when `p != 0`, in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000E9BC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A1], 16);
    sw(m, g[SP], 8, g[A2]);
    g[T8] = sll(g[A2], 16);
    sw(m, g[SP], 0xC, g[A3]);
    g[T0] = sll(g[A3], 16);
    g[A3] = sra(g[T0], 16);
    g[A2] = sra(g[T8], 16);
    g[T7] = sra(g[T6], 16);
    sw(m, g[SP], 4, g[A1]);
    if g[A0] == 0 {
        return;
    }
    g[V0] = lw(m, g[A0], 0xC);
    if g[V0] == 0 {
        return;
    }
    if (g[T7] as i64) >= 0 {
        sb(m, g[V0], 0x20, g[T7]);
    }
    if (g[A2] as i64) >= 0 {
        sb(m, g[V0], 0x21, g[A2]);
    }
    if (g[A3] as i64) >= 0 {
        sb(m, g[V0], 0x22, g[A3]);
    }
    // Each lh is in the delay slot of the test before it, so it runs either way.
    g[V1] = lh(m, g[SP], 0x12);
    if (g[V1] as i64) >= 0 {
        sb(m, g[V0], 0x23, g[V1]);
    }
    g[V1] = lh(m, g[SP], 0x16);
    if (g[V1] as i64) >= 0 {
        sb(m, g[V0], 0x24, g[V1]);
    }
    g[V1] = lh(m, g[SP], 0x1A);
    if (g[V1] as i64) >= 0 {
        sb(m, g[V0], 0x25, g[V1]);
    }
}

/// `func_8000FCA4(k, b)`: the low byte of word `k` of the array at
/// `0x8009B824` (its byte at `+3`) = `b`; spills `a1`. QUIRK: `k` is unbounded.
///
/// Leaves `t7 = 4k`, `at = 0x800A0000 + 4k`.
///
/// Domain: canonical `sp` with `[sp + 4]` in RDRAM; the byte in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000FCA4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T7] = sll(g[A0], 2);
    g[AT] = addu(li(0x800A_0000), g[T7]);
    sw(m, g[SP], 4, g[A1]);
    sb(m, g[AT], -0x47D9, g[A1]);
}

/// `func_8000FE1C()`: set two blocks to -1: for `k` in 0..2, the word at
/// `0x8009B814 + 4k` and the eight words at `0x8009B82C + 0x20k`.
///
/// Leaves `v1 = 0x8009B81C`, `a0 = t0 = 0x8009B86C`, `a1 = a0`, `v0 = a3 =
/// 8`, `a2 = -1`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000FE1C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T0] = li(0x8009_B86C);
    g[A0] = li(0x8009_B82C);
    g[V1] = li(0x8009_B814);
    g[A3] = 8;
    g[A2] = u64::MAX;
    loop {
        sw(m, g[V1], 0, g[A2]);
        g[V0] = 0;
        g[A1] = g[A0];
        loop {
            g[V0] = addu(g[V0], 4);
            sw(m, g[A1], 0, g[A2]);
            sw(m, g[A1], 4, g[A2]);
            sw(m, g[A1], 8, g[A2]);
            sw(m, g[A1], 0xC, g[A2]);
            g[A1] = addu(g[A1], 0x10);
            if g[V0] == g[A3] {
                break;
            }
        }
        g[A0] = addu(g[A0], 0x20);
        g[V1] = addu(g[V1], 4);
        if g[A0] == g[T0] {
            break;
        }
    }
}

/// `func_8000FE78()`: the 20 words at `0x800D5AA8` = -9999, four per
/// iteration. Leaves `v1 = a0 = 0x800D5AF8`, `v0 = -9999`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000FE78(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A0] = li(0x800D_5AF8);
    g[V1] = li(0x800D_5AA8);
    g[V0] = (-0x270Fi64) as u64;
    loop {
        g[V1] = addu(g[V1], 0x10);
        sw(m, g[V1], -0xC, g[V0]);
        sw(m, g[V1], -8, g[V0]);
        sw(m, g[V1], -4, g[V0]);
        sw(m, g[V1], -0x10, g[V0]);
        if g[V1] == g[A0] {
            break;
        }
    }
}

/// `func_8000FEF0()`: the ten words at `0x800D5F80` and the ten at
/// `0x800D5FA8` = -1 (word `k` of the first, then of the second, per
/// iteration), then the 40 bytes at `0x800D5C38` = 0, four per iteration. The two word arrays are the ones
/// [`func_80010014`] sets and the bytes are [`func_8000FFF8`]'s.
///
/// Leaves `a0 = a1 = 0x800D5FD0`, `at = 0`, `v1 = v0 = 0x800D5C60`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000FEF0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A1] = li(0x800D_5FD0);
    g[A0] = li(0x800D_5FA8);
    g[V1] = li(0x800D_5F80);
    g[V0] = u64::MAX;
    loop {
        g[A0] = addu(g[A0], 4);
        g[AT] = sltu(g[A0], g[A1]);
        g[V1] = addu(g[V1], 4);
        sw(m, g[V1], -4, g[V0]);
        sw(m, g[A0], -4, g[V0]);
        if g[AT] == 0 {
            break;
        }
    }
    g[V0] = li(0x800D_5C60);
    g[V1] = li(0x800D_5C38);
    loop {
        g[V1] = addu(g[V1], 4);
        sb(m, g[V1], -3, 0);
        sb(m, g[V1], -2, 0);
        sb(m, g[V1], -1, 0);
        sb(m, g[V1], -4, 0);
        if g[V1] == g[V0] {
            break;
        }
    }
}

/// `func_8000FFF8(k)`: if `k < 40` (signed), the byte at `0x800D5C38 + k` =
/// 0. QUIRK: negative `k` writes before the array, and the compare is on
/// the whole 64-bit register while the address uses its low word, so e.g.
/// `i64::MIN` clears byte 0.
///
/// Leaves `at = 0x800D0000` (`+ k` when it stores).
///
/// Domain: `0x800D5C38 + k` in RDRAM for `k < 40`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000FFF8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0x28);
    let in_range = g[AT] != 0;
    g[AT] = li(0x800D_0000);
    if in_range {
        g[AT] = addu(g[AT], g[A0]);
        sb(&mut mem, g[AT], 0x5C38, 0);
    }
}

/// `func_80010014(k, a, b)`: if `k < 10` (signed), `[0x800D5F80 + 4k] = a`
/// and `[0x800D5FA8 + 4k] = b`. QUIRK: negative `k` writes before the
/// arrays; the compare is 64-bit, the address uses the low word.
///
/// Leaves `at` = 1 if `k < 10` (then `0x800D0000 + 4k`) else 0, `v0 = 4k`.
///
/// Domain: both words in RDRAM for `k < 10`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80010014(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0xA);
    g[V0] = sll(g[A0], 2);
    if g[AT] != 0 {
        g[AT] = addu(li(0x800D_0000), g[V0]);
        sw(m, g[AT], 0x5F80, g[A1]);
        g[AT] = addu(li(0x800D_0000), g[V0]);
        sw(m, g[AT], 0x5FA8, g[A2]);
    }
}

/// `func_80010040()`: `[0x8009B86C] = 0`. Leaves `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80010040(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    ctx.gpr[AT] = li(0x800A_0000);
    sw(&mut mem, ctx.gpr[AT], -0x4794, 0);
}

/// Eight (shift, base) pairs decoding 16-bit handles ([`func_8001004C`]).
pub const HANDLE_SEGMENTS: u32 = 0x8009_B888;

/// `func_8001004C(h)`: decode a handle: segment `s = (h & 0xE000) >> 13`,
/// index `i = (h & 0x1FFC) >> 2`; returns `base + (i << shift)` with
/// `(shift, base)` the words at `HANDLE_SEGMENTS + 8s`. The shift is `sllv`,
/// by `shift & 31`.
///
/// Leaves `t6`-`t9`, `a1` = the pair's address, `t2` = shift, `t4` = base,
/// `t0`, `t1 = i`, `t3 = i << shift`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8001004C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[T6] = g[A0] & 0xE000;
    g[T7] = sra(g[T6], 13);
    g[T9] = li(HANDLE_SEGMENTS);
    g[T8] = sll(g[T7], 3);
    g[A1] = addu(g[T8], g[T9]);
    g[T2] = lw(m, g[A1], 0);
    g[T0] = g[A0] & 0x1FFC;
    g[T4] = lw(m, g[A1], 4);
    g[T1] = sra(g[T0], 2);
    g[T3] = sllv(g[T1], g[T2]);
    g[V0] = addu(g[T4], g[T3]);
}

/// `func_80011778(r, g, b, a)`: the four bytes at `0x8009B874` = the low
/// bytes of `a0`-`a3`; spills them to their argument slots first. Leaves
/// `v0 = 0x8009B874`.
///
/// Domain: canonical `sp` with its argument slots in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011778(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x8009_B874);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[SP], 4, g[A1]);
    sw(m, g[SP], 8, g[A2]);
    sw(m, g[SP], 0xC, g[A3]);
    sb(m, g[V0], 0, g[A0]);
    sb(m, g[V0], 1, g[A1]);
    sb(m, g[V0], 2, g[A2]);
    sb(m, g[V0], 3, g[A3]);
}

/// `func_80011814()`: the byte at `0x8009B870` = 1. Leaves `t6 = 1`, `at =
/// 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011814(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = 1;
    g[AT] = li(0x800A_0000);
    sb(&mut mem, g[AT], -0x4790, g[T6]);
}

/// Words indexed by [`func_80011824`], reset with [`func_80011838`].
pub const WORD_TABLE_6140: u32 = 0x800D_6140;

/// `func_80011824(k, v)`: `WORD_TABLE_6140[k] = v`. QUIRK: `k` is unbounded.
/// Leaves `t6 = 4k`, `at = 0x800D0000 + 4k`.
///
/// Domain: the word in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011824(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 2);
    g[AT] = addu(li(0x800D_0000), g[T6]);
    sw(&mut mem, g[AT], 0x6140, g[A1]);
}

/// `func_80011838()`: the 80 words of [`WORD_TABLE_6140`] = -1 and the 80
/// bytes at `0x800D68C0` = 0, four of each per iteration, interleaved.
/// Leaves `a0 = a1 = 0x800D6910`, `v1 = 0x800D6280`, `v0 = -1`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011838(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A1] = li(0x800D_6910);
    g[A0] = li(0x800D_68C0);
    g[V1] = li(WORD_TABLE_6140);
    g[V0] = u64::MAX;
    loop {
        g[A0] = addu(g[A0], 4);
        sw(m, g[V1], 4, g[V0]);
        sb(m, g[A0], -3, 0);
        sw(m, g[V1], 8, g[V0]);
        sb(m, g[A0], -2, 0);
        sw(m, g[V1], 0xC, g[V0]);
        sb(m, g[A0], -1, 0);
        g[V1] = addu(g[V1], 0x10);
        sw(m, g[V1], -0x10, g[V0]);
        sb(m, g[A0], -4, 0);
        if g[A0] == g[A1] {
            break;
        }
    }
}

/// `func_80011918()`: `[0x8009B810] = 1`. Leaves `t6 = 1`, `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011918(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = 1;
    g[AT] = li(0x800A_0000);
    sw(&mut mem, g[AT], -0x47F0, g[T6]);
}

/// `func_80011928()`: `[0x8009B810] = 0`. Leaves `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011928(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    ctx.gpr[AT] = li(0x800A_0000);
    sw(&mut mem, ctx.gpr[AT], -0x47F0, 0);
}

/// Pointers selected by [`func_80011E54`], filled by [`func_80011DF0`].
pub const POINTER_TABLE_6940: u32 = 0x800D_6940;
/// How many of them there are (7 after [`func_80011DF0`]).
pub const POINTER_COUNT: u32 = 0x800A_1D88;
/// The selected one.
pub const POINTER_SELECTED: u32 = 0x800A_1D8C;

/// `func_80011DF0()`: `POINTER_COUNT = 7`, the seven words of
/// [`POINTER_TABLE_6940`] = `0x800A1BDC 0x800A1B78 0x800A1B14 0x800A1B78
/// 0x800A1C40 0x800A1BDC 0x800A1AB0`, and `POINTER_SELECTED = 0x800A1BDC`.
///
/// Leaves `t6 = 7`, `v0` = the table, `v1 = 0x800A1BDC`, `a0 = 0x800A1B78`,
/// `t7 = 0x800A1B14`, `t8 = 0x800A1C40`, `t9 = 0x800A1AB0`, `at =
/// 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011DF0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = 7;
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x1D88, g[T6]);
    g[V0] = li(POINTER_TABLE_6940);
    g[V1] = li(0x800A_1BDC);
    g[A0] = li(0x800A_1B78);
    g[T7] = li(0x800A_1B14);
    g[T8] = li(0x800A_1C40);
    g[T9] = li(0x800A_1AB0);
    sw(m, g[V0], 0, g[V1]);
    sw(m, g[V0], 4, g[A0]);
    sw(m, g[V0], 8, g[T7]);
    sw(m, g[V0], 0xC, g[A0]);
    sw(m, g[V0], 0x10, g[T8]);
    sw(m, g[V0], 0x14, g[V1]);
    sw(m, g[V0], 0x18, g[T9]);
    sw(m, g[AT], 0x1D8C, g[V1]);
}

/// `func_80011E54(k)`: `POINTER_SELECTED = POINTER_TABLE_6940[k]`, with `k`
/// the low halfword of `a0`, signed, clamped: `k >= count` becomes the low
/// halfword of `count - 1`, then a negative `k` becomes 0 (so a count of 0
/// selects entry 0). Spills `a0`.
///
/// Leaves `v0` = the count, `a0 = k`, `t6`, `t8` (when clamping high), `at
/// = 0x800A0000`, `t0 = 4k`, `t1` = the entry.
///
/// Domain: canonical `sp` with `[sp]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011E54(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, li(POINTER_COUNT), 0);
    sw(m, g[SP], 0, g[A0]);
    g[T6] = sll(g[A0], 16);
    g[A0] = sra(g[T6], 16);
    g[AT] = slt(g[A0], g[V0]);
    g[T1] = li(0x800D_0000);
    if g[AT] == 0 {
        g[A0] = addu(g[V0], u64::MAX);
        g[T8] = sll(g[A0], 16);
        g[A0] = sra(g[T8], 16);
    }
    if (g[A0] as i64) < 0 {
        g[A0] = 0;
    }
    g[T0] = sll(g[A0], 2);
    g[T1] = addu(g[T1], g[T0]);
    g[T1] = lw(m, g[T1], 0x6940);
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x1D8C, g[T1]);
}

/// A pair of halfwords, set by [`func_80011ECC`] and read by
/// [`func_80011EE8`] (a position, by the look of it: **guess**).
pub const HALF_PAIR: u32 = 0x800A_1CD0;

/// `func_80011ECC(x, y)`: `HALF_PAIR = (x, y)` (low halfwords); spills
/// `a0`, `a1`. Leaves `v0 = HALF_PAIR`.
///
/// Domain: canonical `sp` with its argument slots in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011ECC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(HALF_PAIR);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[SP], 4, g[A1]);
    sh(m, g[V0], 0, g[A0]);
    sh(m, g[V0], 2, g[A1]);
}

/// `func_80011EE8(&x, &y)`: `*x = HALF_PAIR.0`, `*y = HALF_PAIR.1`
/// (halfwords; `y` is read after `*x` is written). Leaves `v0 = HALF_PAIR`,
/// `t6`, `t7` = the values sign-extended.
///
/// Domain: canonical `x`, `y` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011EE8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(HALF_PAIR);
    g[T6] = lh(m, g[V0], 0);
    sh(m, g[A0], 0, g[T6]);
    g[T7] = lh(m, g[V0], 2);
    sh(m, g[A1], 0, g[T7]);
}

/// `func_80011F04(r, g, b, a)`: the four bytes at `0x800A1CCC` = the low
/// bytes of `a0`-`a3`, then spills them, then `[0x800D6938] = 0`. Leaves
/// `v0 = 0x800A1CCC`, `at = 0x800D0000`.
///
/// Domain: canonical `sp` with its argument slots in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011F04(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x800A_1CCC);
    sb(m, g[V0], 0, g[A0]);
    sb(m, g[V0], 1, g[A1]);
    sb(m, g[V0], 2, g[A2]);
    sb(m, g[V0], 3, g[A3]);
    g[AT] = li(0x800D_0000);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[SP], 4, g[A1]);
    sw(m, g[SP], 8, g[A2]);
    sw(m, g[SP], 0xC, g[A3]);
    sw(m, g[AT], 0x6938, 0);
}

/// `func_80012B5C(c, o, &a, &b)`: look up `c` (the low byte of `a0`) in
/// `o`'s table of 16-byte records at `[o + 0x5C]`, which covers `c` from the
/// byte `[o + 0x5A]` up to the byte `[o + 0x5B]`. `*a` and `*b` (each only
/// if its pointer is nonzero) are first set to -1, then to the record's
/// halfwords at `+2` and `+0xE`, sign-extended.
///
/// QUIRK: those two values go through the stack (`[sp + 4]`, `[sp]` of an
/// 8-byte frame) and are copied out **whether or not the lookup hit**: on a
/// miss (no table, or `c` out of range) `*a` and `*b` get whatever those
/// stack words held, overwriting the -1. Spills `a0` to its slot.
///
/// Leaves `a0` = the byte (or the record, on a hit), `t6`-`t8` (`t7`/`t8` =
/// the words copied out), `v0` = the table, and on the way `t0`-`t5`, `t9`,
/// `at`.
///
/// Domain: canonical `sp` with `[sp - 8]..[sp + 4]` in RDRAM; canonical
/// `o`, `a`, `b`; the table and record in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80012B5C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-8i64) as u64);
    sw(m, g[SP], 8, g[A0]);
    g[T6] = g[A0] & 0xFF;
    g[A0] = g[T6];
    if g[A2] != 0 {
        g[T7] = u64::MAX;
        sw(m, g[A2], 0, g[T7]);
    }
    g[T8] = u64::MAX;
    if g[A3] != 0 {
        sw(m, g[A3], 0, g[T8]);
    }
    g[V0] = lw(m, g[A1], 0x5C);
    if g[V0] != 0 {
        g[T0] = lbu(m, g[A1], 0x5A);
        g[AT] = slt(g[A0], g[T0]);
        if g[AT] == 0 {
            g[T9] = lbu(m, g[A1], 0x5B);
            g[T1] = sll(g[A0], 4);
            g[T2] = addu(g[V0], g[T1]);
            g[AT] = slt(g[T9], g[A0]);
            g[T3] = sll(g[T0], 4);
            if g[AT] == 0 {
                // A hit: record = table + 16 * (c - first).
                g[T4] = subu(0, g[T3]);
                g[A0] = addu(g[T2], g[T4]);
                g[T5] = lh(m, g[A0], 2);
                sw(m, g[SP], 4, g[T5]);
                g[T6] = lh(m, g[A0], 0xE);
                sw(m, g[SP], 0, g[T6]);
            }
        }
    }
    // QUIRK: copied out on a miss too, from stack words never written.
    g[T7] = lw(m, g[SP], 4);
    if g[A2] != 0 {
        sw(m, g[A2], 0, g[T7]);
    }
    g[T8] = lw(m, g[SP], 0);
    if g[A3] != 0 {
        sw(m, g[A3], 0, g[T8]);
    }
    g[SP] = addu(g[SP], 8);
}

/// The display list pointer [`func_80014C98`] appends to.
pub const DL_HEAD: u32 = 0x8012_17B0;

/// `func_80014C98()`: append `gDPPipeSync` (`0xE7000000`, `0`) at the
/// display list pointer [`DL_HEAD`] and advance it by 8 (the pointer is
/// stored before the command, then the low word, then the high).
///
/// Leaves `a0 = DL_HEAD`, `v1` = the old pointer, `t6` = the new, `t7 =
/// 0xE7000000`.
///
/// Domain: the pointer canonical, 8 bytes at it in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80014C98(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A0] = li(DL_HEAD);
    g[V1] = lw(m, g[A0], 0);
    g[T7] = li(0xE700_0000);
    g[T6] = addu(g[V1], 8);
    sw(m, g[A0], 0, g[T6]);
    sw(m, g[V1], 4, 0);
    sw(m, g[V1], 0, g[T7]);
}

/// A struct field getter: `v0 = [a0 + off]`.
fn get_word(rdram: *mut u8, ctx: *mut RecompContext, off: i32) {
    // SAFETY: forwarded from the entry points below.
    let (mem, ctx) = unsafe { enter(rdram, ctx) };
    ctx.gpr[V0] = lw(&mem, ctx.gpr[A0], off);
}

/// A struct field getter: `v0 = (i16)[a0 + off]`.
fn get_half(rdram: *mut u8, ctx: *mut RecompContext, off: i32) {
    // SAFETY: forwarded from the entry points below.
    let (mem, ctx) = unsafe { enter(rdram, ctx) };
    ctx.gpr[V0] = lh(&mem, ctx.gpr[A0], off);
}

/// A struct field setter: `[a0 + off] = a1`.
fn set_word(rdram: *mut u8, ctx: *mut RecompContext, off: i32) {
    // SAFETY: forwarded from the entry points below.
    let (mut mem, ctx) = unsafe { enter(rdram, ctx) };
    sw(&mut mem, ctx.gpr[A0], off, ctx.gpr[A1]);
}

/// `func_80017D48(o, v)`: `[o + 0x1C] = v` ([`set_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017D48(rdram: *mut u8, ctx: *mut RecompContext) {
    set_word(rdram, ctx, 0x1C)
}

/// `func_80017D50(o)`: `[o + 0x1C]` ([`get_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017D50(rdram: *mut u8, ctx: *mut RecompContext) {
    get_word(rdram, ctx, 0x1C)
}

/// `func_80017DA4(o)`: `[o]` ([`get_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017DA4(rdram: *mut u8, ctx: *mut RecompContext) {
    get_word(rdram, ctx, 0)
}

/// `func_80017DAC(o)`: `[o + 0x14]` ([`get_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017DAC(rdram: *mut u8, ctx: *mut RecompContext) {
    get_word(rdram, ctx, 0x14)
}

/// `func_80017DB4(o, k)`: 0 if `o` is null, else word `k` of the array at
/// `[o + 0x18]`. QUIRK: `k` is unbounded. Leaves `t6` = the array, `t7 =
/// 4k`, `t8` = the element's address, when `o != 0`.
///
/// Domain: for nonzero `o`, canonical and the element in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017DB4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    if g[A0] == 0 {
        g[V0] = 0;
        return;
    }
    g[T6] = lw(m, g[A0], 0x18);
    g[T7] = sll(g[A1], 2);
    g[T8] = addu(g[T6], g[T7]);
    g[V0] = lw(m, g[T8], 0);
}

/// `func_80017DDC(o)`: `(i16)[o + 0x20]` ([`get_half`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017DDC(rdram: *mut u8, ctx: *mut RecompContext) {
    get_half(rdram, ctx, 0x20)
}

/// `func_80017DE4(o)`: `(i16)[o + 0x22]` ([`get_half`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017DE4(rdram: *mut u8, ctx: *mut RecompContext) {
    get_half(rdram, ctx, 0x22)
}

/// `func_80017DEC(o)`: `[o + 0x24]` ([`get_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017DEC(rdram: *mut u8, ctx: *mut RecompContext) {
    get_word(rdram, ctx, 0x24)
}

/// `func_80017DF4(o, zero, &a, &b)`: if `zero != 0` (64-bit), `*a = *b = 0`;
/// else `*a = [o + 0x2C]` and `*b = [o + 0x28]`, in that order. Leaves `t6`,
/// `t7` = the words copied, in the second case.
///
/// Domain: canonical `a`, `b` (and `o` when `zero == 0`) in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017DF4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    if g[A1] != 0 {
        sw(m, g[A2], 0, 0);
        sw(m, g[A3], 0, 0);
    } else {
        g[T6] = lw(m, g[A0], 0x2C);
        sw(m, g[A2], 0, g[T6]);
        g[T7] = lw(m, g[A0], 0x28);
        sw(m, g[A3], 0, g[T7]);
    }
}

/// `func_80017E54(o)`: `[o + 0x14]` ([`get_word`]; the same as
/// [`func_80017DAC`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017E54(rdram: *mut u8, ctx: *mut RecompContext) {
    get_word(rdram, ctx, 0x14)
}

/// `func_80017E5C(o, k)`: word `k` of the array at `[o + 0x18]`, without
/// [`func_80017DB4`]'s null check. QUIRK: `k` is unbounded. Leaves `t6` =
/// the array, `t7 = 4k`, `t8` = the element's address.
///
/// Domain: canonical `o`; the element in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017E5C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[T6] = lw(m, g[A0], 0x18);
    g[T7] = sll(g[A1], 2);
    g[T8] = addu(g[T6], g[T7]);
    g[V0] = lw(m, g[T8], 0);
}

/// `func_80017E70(o, which, v)`: `[o + 8] = v` if `which == 2` (64-bit
/// compare), else nothing. Leaves `at = 2`.
///
/// Domain: when `which == 2`, canonical `o` with `o + 8` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017E70(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = 2;
    if g[A1] == g[AT] {
        sw(&mut mem, g[A0], 8, g[A2]);
    }
}

/// `func_80017E88(o, which)`: -1 unless `o` is nonzero and `which == 1`
/// (64-bit); then, from the flags word `[o]`: bit 3 gives 1, bit 6 gives 2,
/// both 3, neither 0.
///
/// Leaves `v1` = the result (except for exactly bit 6, where it stays 0),
/// and when the flags are read `at = 1`, `v0`, `t6 = flags & 8`, `t7 =
/// flags & 0x40`.
///
/// Domain: canonical `o` with `[o]` in RDRAM when `which == 1`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017E88(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[V1] = u64::MAX;
    if g[A0] != 0 {
        g[AT] = 1;
        if g[A1] == g[AT] {
            g[V0] = lw(&mem, g[A0], 0);
            g[V1] = 0;
            g[T6] = g[V0] & 8;
            g[T7] = g[V0] & 0x40;
            if g[T6] != 0 {
                g[V1] = 1;
            }
            if g[T7] != 0 {
                if g[V1] == 0 {
                    g[V0] = 2;
                    return;
                }
                g[V1] = 3;
            }
        }
    }
    g[V0] = g[V1];
}

/// `func_80017EDC(o)`: `[o]` ([`get_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017EDC(rdram: *mut u8, ctx: *mut RecompContext) {
    get_word(rdram, ctx, 0)
}

/// `func_80017EE4(o)`: `[o + 4]` ([`get_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017EE4(rdram: *mut u8, ctx: *mut RecompContext) {
    get_word(rdram, ctx, 4)
}

/// `func_80017EEC(o, v)`: `[o + 4] = v` ([`set_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017EEC(rdram: *mut u8, ctx: *mut RecompContext) {
    set_word(rdram, ctx, 4)
}

/// `func_80017EF4(o)`: `[o]` ([`get_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017EF4(rdram: *mut u8, ctx: *mut RecompContext) {
    get_word(rdram, ctx, 0)
}

/// `func_80017EFC(o, bits)`: `[o] |= bits`. Leaves `t6`/`t7` = old/new.
///
/// Domain: canonical `o` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017EFC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = lw(&mem, g[A0], 0);
    g[T7] = g[T6] | g[A1];
    sw(&mut mem, g[A0], 0, g[T7]);
}

/// `func_80017F0C(o, bits)`: `[o] &= !bits`. Leaves `t6` = old, `t7 = !bits`
/// (all 64 bits), `t8` = new.
///
/// Domain: canonical `o` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017F0C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = lw(&mem, g[A0], 0);
    g[T7] = !g[A1];
    g[T8] = g[T6] & g[T7];
    sw(&mut mem, g[A0], 0, g[T8]);
}

/// `func_80017F20()`: returns 4.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017F20(_rdram: *mut u8, ctx: *mut RecompContext) {
    (*ctx).gpr[V0] = 4;
}

/// Four 0x170-byte records returned by [`func_80017F28`].
pub const RECORDS_170: u32 = 0x8012_0DF0;

/// `func_80017F28(k)`: `RECORDS_170 + 0x170 * k` for `0 <= k < 4` (signed
/// 64-bit), else 0. Leaves `at` = `k < 4`, and `t6` = `4k`, or on a hit
/// `0x170 * k`, with `t7 = RECORDS_170`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017F28(_rdram: *mut u8, ctx: *mut RecompContext) {
    let g = &mut (*ctx).gpr;
    g[AT] = slt(g[A0], 4);
    if (g[A0] as i64) >= 0 {
        g[T6] = sll(g[A0], 2);
        if g[AT] != 0 {
            // t6 = k * 0x170: ((4k - k) * 8 - k) * 16
            g[T6] = subu(g[T6], g[A0]);
            g[T6] = sll(g[T6], 3);
            g[T6] = subu(g[T6], g[A0]);
            g[T7] = li(RECORDS_170);
            g[T6] = sll(g[T6], 4);
            g[V0] = addu(g[T6], g[T7]);
            return;
        }
    }
    g[V0] = 0;
}

/// `func_80018114(o, v)`: `[o + 0x168] = v` ([`set_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80018114(rdram: *mut u8, ctx: *mut RecompContext) {
    set_word(rdram, ctx, 0x168)
}

/// `func_8001811C(o, k, v)`: store `v` in the field of `o` that `k` selects
/// (64-bit compares): 4 → `+0x15C`, 3 → `+0x164`, 6 → `+0x158`, 5 →
/// `+0x160`; anything else, nothing. [`func_80018164`] is the getter.
/// Leaves `at = 5`.
///
/// Domain: for a selecting `k`, canonical `o` with the field in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8001811C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    for (k, off) in [(4, 0x15C), (3, 0x164), (6, 0x158), (5, 0x160)] {
        g[AT] = k;
        if g[A1] == g[AT] {
            sw(m, g[A0], off, g[A2]);
        }
    }
}

/// `func_80018164(o, k)`: the field of `o` that `k` selects, as for
/// [`func_8001811C`], or -1. Leaves `at` = the constant that matched, else 5.
///
/// Domain: for a selecting `k`, canonical `o` with the field in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80018164(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    for (k, off) in [(4, 0x15C), (3, 0x164), (6, 0x158)] {
        g[AT] = k;
        if g[A1] == g[AT] {
            g[V0] = lw(&mem, g[A0], off);
            return;
        }
    }
    g[AT] = 5;
    g[V0] = if g[A1] == g[AT] { lw(&mem, g[A0], 0x160) } else { u64::MAX };
}

/// `func_800182FC(o, k)`: `[o + 8]` for `k == 0`, `[o + 4]` for `k == 2`
/// (64-bit), else 0. Leaves `at = 2`.
///
/// Domain: for `k` 0 or 2, canonical `o` with the field in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800182FC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = 2;
    g[V0] = if g[A1] == 0 {
        lw(&mem, g[A0], 8)
    } else if g[A1] == g[AT] {
        lw(&mem, g[A0], 4)
    } else {
        0
    };
}

/// `func_800183A8(o)`: `[o + 4]` ([`get_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800183A8(rdram: *mut u8, ctx: *mut RecompContext) {
    get_word(rdram, ctx, 4)
}

/// `func_800183B0(o, v)`: `[o + 4] = v` ([`set_word`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800183B0(rdram: *mut u8, ctx: *mut RecompContext) {
    set_word(rdram, ctx, 4)
}

/// `func_80018440`: returns at once.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80018440(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_80018448`: returns at once.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80018448(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_80018450`: an empty function of two arguments; spills `a0` and
/// `a1` to `[sp]` and `[sp + 4]`.
///
/// Domain: canonical `sp` with `[sp]..[sp + 8]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80018450(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &ctx.gpr;
    sw(&mut mem, g[SP], 0, g[A0]);
    sw(&mut mem, g[SP], 4, g[A1]);
}

/// `func_80018460`: returns at once.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80018460(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_80018470()`: `[0x800A21AC] = 5`. Leaves `t6 = 5`, `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80018470(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = 5;
    g[AT] = li(0x800A_0000);
    sw(&mut mem, g[AT], 0x21AC, g[T6]);
}

/// `func_8001F464()`: 1 if bit 1 of `[0x80113688]` is set, else 0. Leaves
/// `t6` = the word, `t7` = the bit.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8001F464(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = lw(&mem, li(0x8011_0000), 0x3688);
    g[T7] = g[T6] & 2;
    g[V0] = u64::from(g[T7] != 0);
}

/// `func_80024704(p)`: update the track selection (**guess**) at
/// `0x8011A240`: `+0x28` = circuits available, `+0x2C` = tracks unlocked in
/// the chosen circuit `c = (s8) [p + 0x5E]`, `+0x30` (the selected track)
/// clamped to at most `+0x2C - 1`, `+0x20 = (c > 0)` and `+0x24 = (c <
/// +0x28)` (signed).
///
/// The save data is `0x80113E60` if the byte `[p + 0x6C]` (signed) is
/// nonzero, else `0x80113680` (see [`func_8002DAD0`]). Circuits: 3, or 2 if
/// its byte `+0xB` (resp. `+0xF`) is 0. Tracks: the bits `0..n` set in its
/// byte `+8 + c` (resp. `+0xC + c`), `n = [0x800A21B4 + c]` (unsigned
/// byte); `c` and `n` are read again after each set bit.
///
/// Leaves `a2 = 0x8011A240`, `a3` = the save data, `v0` = tracks - 1, `v1 =
/// c`, `a1 = n`, `t2 = t8 = 1`, and the loop's loads in `t0`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80024704(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A2] = li(0x8011_A240);
    g[T6] = 3;
    sw(m, g[A2], 0x28, g[T6]);
    sw(m, g[A2], 0x2C, 0);
    g[T7] = lb(m, g[A0], 0x6C);
    g[A3] = li(0x8011_3680);
    // (flag byte, its temporary, the "2" register), then the bit loop's
    // (byte offset, pointer, byte, one, mask, test, count, count + 1).
    let (flag, tflag, two, bits, ptr, byte, one, mask, test, cnt, cnt1) = if g[T7] != 0 {
        g[A3] = li(0x8011_3E60);
        (0xB, T8, T9, 8, T0, T1, T2, T3, T4, T5, T6)
    } else {
        (0xF, T7, T8, 0xC, T9, T0, T2, T1, T3, T4, T5)
    };
    g[tflag] = lbu(m, g[A3], flag);
    g[two] = 2;
    if g[tflag] == 0 {
        sw(m, g[A2], 0x28, g[two]);
    }
    g[V1] = lb(m, g[A0], 0x5E);
    g[A1] = lbu(m, addu(li(0x800A_0000), g[V1]), 0x21B4);
    g[V0] = 0;
    g[ptr] = addu(g[A3], g[V1]);
    if (g[A1] as i64) > 0 {
        loop {
            g[byte] = lbu(m, g[ptr], bits);
            g[one] = 1;
            g[mask] = sllv(g[one], g[V0]);
            g[test] = g[byte] & g[mask];
            g[V0] = addu(g[V0], 1);
            if g[test] != 0 {
                g[cnt] = lw(m, g[A2], 0x2C);
                g[cnt1] = addu(g[cnt], 1);
                sw(m, g[A2], 0x2C, g[cnt1]);
                g[V1] = lb(m, g[A0], 0x5E);
                g[A1] = lbu(m, addu(li(0x800A_0000), g[V1]), 0x21B4);
            }
            g[AT] = slt(g[V0], g[A1]);
            if g[AT] == 0 {
                break;
            }
            g[ptr] = addu(g[A3], g[V1]);
        }
    }
    g[V0] = lw(m, g[A2], 0x2C);
    g[T6] = lw(m, g[A2], 0x30);
    g[T8] = 1;
    g[V0] = addu(g[V0], u64::MAX);
    g[AT] = slt(g[V0], g[T6]);
    g[T2] = 1;
    if g[AT] != 0 {
        sw(m, g[A2], 0x30, g[V0]);
    }
    sw(m, g[A2], 0x24, 0);
    sw(m, g[A2], 0x20, 0);
    g[V1] = lb(m, g[A0], 0x5E);
    if (g[V1] as i64) > 0 {
        sw(m, g[A2], 0x20, g[T8]);
        g[V1] = lb(m, g[A0], 0x5E);
    }
    g[T9] = lw(m, g[A2], 0x28);
    g[AT] = slt(g[V1], g[T9]);
    if g[AT] != 0 {
        sw(m, g[A2], 0x24, g[T2]);
    }
}

/// `func_800281F0(p)`: build the list of available racers (**guess**; 23
/// entries) at `0x800D6CD8`: the unlock mask `[0x80113E74 + 0x2C * (s8) [p
/// + 0x6F]]` (a word of the current profile record, [`func_80039914`]) ORed
/// with `0x22E01`, always available; each set bit `i < 23`, in order, gives
/// an 8-byte entry `{+0: i, +4: 0xFF, +5: 0}`, and the rest of the 23 get
/// `{-1, 0xFF, 0}`. The number available goes to `[0x8011A26C]` (the count
/// [`func_80047920`] reads). The fill is unrolled: `(23 - n) & 3` single
/// entries, then four at a time.
///
/// Leaves `v0` = the count, `a1 = 23`, `a2 = -1` (or `0x800D6CD8` if all 23
/// are available), `a3 = 0xFF`, `t9 = 1`, `at = 0x80120000`, and the
/// loops' registers in `a0`, `v1`, `t0`..`t8`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800281F0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = lb(m, g[A0], 0x6F);
    g[T7] = sll(g[T6], 2);
    g[T7] = subu(g[T7], g[T6]);
    g[T7] = sll(g[T7], 2);
    g[T7] = subu(g[T7], g[T6]);
    g[T7] = sll(g[T7], 2);
    g[V1] = lw(m, addu(li(0x8011_0000), g[T7]), 0x3E74);
    g[AT] = li(0x2_2E01);
    g[A2] = li(0x800D_6CD8);
    g[T8] = g[V1] | g[AT];
    g[V0] = 0;
    g[V1] = g[T8];
    g[A1] = 0;
    g[A3] = 0xFF;
    g[T9] = 1;
    loop {
        g[T1] = sllv(g[T9], g[A1]);
        g[T2] = g[T1] & g[V1];
        g[T3] = sll(g[V0], 3);
        if g[T2] != 0 {
            g[A0] = addu(g[A2], g[T3]);
            sw(m, g[A0], 0, g[A1]);
            sb(m, g[A0], 4, g[A3]);
            sb(m, g[A0], 5, 0);
            g[V0] = addu(g[V0], 1);
        }
        g[A1] = addu(g[A1], 1);
        g[AT] = slt(g[A1], 0x17);
        if g[AT] == 0 {
            break;
        }
        g[T9] = 1;
    }
    g[AT] = slt(g[V0], 0x17);
    g[A1] = g[V0];
    if g[AT] != 0 {
        // The unavailable rest: -1, 0xFF, 0.
        g[T0] = 0x17;
        g[A2] = subu(g[T0], g[V0]);
        g[T4] = g[A2] & 3;
        g[A0] = addu(g[T4], g[V0]);
        let mut done = false;
        if g[T4] != 0 {
            g[T6] = li(0x800D_6CD8);
            g[T5] = sll(g[A1], 3);
            g[V1] = addu(g[T5], g[T6]);
            g[A2] = u64::MAX;
            loop {
                g[A1] = addu(g[A1], 1);
                sw(m, g[V1], 0, g[A2]);
                sb(m, g[V1], 4, g[A3]);
                sb(m, g[V1], 5, 0);
                g[V1] = addu(g[V1], 8);
                if g[A0] == g[A1] {
                    break;
                }
            }
            if g[A1] == g[T0] {
                g[T8] = li(0x800D_0000);
                done = true;
            }
        }
        if !done {
            g[T8] = li(0x800D_6CD8);
            g[T7] = sll(g[A1], 3);
            g[A0] = li(0x800D_6D90);
            g[V1] = addu(g[T7], g[T8]);
            g[A2] = u64::MAX;
            loop {
                g[V1] = addu(g[V1], 0x20);
                for off in [-0x18, -0x10, -8, -0x20] {
                    sw(m, g[V1], off, g[A2]);
                    sb(m, g[V1], off + 4, g[A3]);
                    sb(m, g[V1], off + 5, 0);
                }
                if g[V1] == g[A0] {
                    break;
                }
            }
        }
    }
    g[AT] = li(0x8012_0000);
    sw(m, g[AT], -0x5D94, g[V0]);
}

/// `func_80029298(x)`: set the horizontal position (**guess**) of the forty
/// 32-byte records from `0x800A4C00` by the kind halfword at `+0x18`
/// (signed): kinds -1 and 4 set `[+8] = x - 145.0`, kind 0 `[+8] = x -
/// 60.0`, kind 1 `[+8] = [+0x14] = x - 157.0`, kind 2 `[+8] = x - 157.0`.
/// Kind 3 and the rest leave the record alone.
///
/// The kind + 1 picks the case through a jump table at `0x800A9DF0`. The
/// `sltiu 6` before it bounds it, so the C's `default` can't be reached.
/// Domain: `x` not NaN (the oracle's `NAN_CHECK`), unless no record has an
/// adding kind. Leaves `f2`/`f14`/`f16` = -60/-145/-157, `v0 = v1 =
/// 0x800A5100`, and from the last record: `t6` = its kind, `at`/`t7` = the
/// range test and `(kind + 1) << 2` if out of range, else `0x800B0000 + 4 *
/// (kind + 1)` and the table entry's address (the table's `lw` as
/// N64Recomp emits it). `f0`/`f4`/`f6`/`f8` keep the last sum each case
/// computed.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80029298(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0xC31D_0000); // -157.0
    ctx.fpr[16].set_u32l(g[AT] as u32);
    g[AT] = li(0xC311_0000); // -145.0
    ctx.fpr[14].set_u32l(g[AT] as u32);
    g[AT] = li(0xC270_0000); // -60.0
    ctx.fpr[2].set_u32l(g[AT] as u32);
    g[V0] = li(0x800A_5100);
    g[V1] = li(0x800A_4C00);
    g[T6] = lh(m, g[V1], 0x18);
    loop {
        g[T7] = addu(g[T6], 1);
        g[AT] = sltu(g[T7], 6);
        g[T7] = sll(g[T7], 2);
        if g[AT] != 0 {
            g[AT] = addu(li(0x800B_0000), g[T7]);
            let case = g[T7] >> 2; // 0..=5
            g[T7] = addu(g[AT], (-0x6210i64) as u64); // the table's lw, as N64Recomp emits it
            let x = ctx.fpr[12].fl();
            match case {
                0 | 5 => {
                    ctx.fpr[6].set_fl(ctx.fpr[14].fl() + x);
                    sw(m, g[V1], 8, u64::from(ctx.fpr[6].u32l()));
                }
                1 => {
                    ctx.fpr[4].set_fl(ctx.fpr[2].fl() + x);
                    sw(m, g[V1], 8, u64::from(ctx.fpr[4].u32l()));
                }
                2 => {
                    ctx.fpr[0].set_fl(ctx.fpr[16].fl() + x);
                    sw(m, g[V1], 8, u64::from(ctx.fpr[0].u32l()));
                    sw(m, g[V1], 0x14, u64::from(ctx.fpr[0].u32l()));
                }
                3 => {
                    ctx.fpr[8].set_fl(ctx.fpr[16].fl() + x);
                    sw(m, g[V1], 8, u64::from(ctx.fpr[8].u32l()));
                }
                _ => {} // 4: kind 3
            }
        }
        g[V1] = addu(g[V1], 0x20);
        if g[V1] == g[V0] {
            break;
        }
        g[T6] = lh(m, g[V1], 0x18);
    }
}

/// The track names [`func_8002D598`] returns, by track: strings in the data
/// segment, back to back and 4-aligned, each starting with `~~`.
pub const TRACK_NAMES: [u32; 25] = [
    0x800A_98E4, 0x800A_9904, 0x800A_991C, 0x800A_9930, 0x800A_9940, 0x800A_9958, 0x800A_9970, 0x800A_9984, 0x800A_9994,
    0x800A_99A8, 0x800A_99BC, 0x800A_99D0, 0x800A_99D8, 0x800A_99E8, 0x800A_99FC, 0x800A_9A14, 0x800A_9A20, 0x800A_9A38,
    0x800A_9A4C, 0x800A_9A60, 0x800A_9A6C, 0x800A_9A7C, 0x800A_9A8C, 0x800A_9A9C, 0x800A_9AA8,
];

/// `func_8002D598(track)` = `track_name`: the address of the track's name
/// in [`TRACK_NAMES`] (`track < 25`, unsigned 64-bit), else 0.
///
/// The case comes from a jump table at `0x800A9F24`. The C's `default`
/// can't be reached. Leaves `v1 = 0x800B0000` for tracks 0-23 but the name
/// for track 24 (its case ends differently), and 0 out of range. In range,
/// `at = 0x800B0000 + 4 * track` and `t6` = the table entry's address
/// (N64Recomp's `addiu` for the table's `lw`); out of range, `at = 0` and
/// `t6` is untouched.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002D598(rdram: *mut u8, ctx: *mut RecompContext) {
    let (_mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = sltu(g[A0], 0x19);
    g[V1] = 0;
    if g[AT] != 0 {
        g[T6] = sll(g[A0], 2);
        g[AT] = addu(li(0x800B_0000), g[T6]);
        let track = (g[T6] >> 2) as usize; // 0..=24
        g[T6] = addu(g[AT], (-0x60DCi64) as u64);
        g[V1] = li(0x800B_0000);
        if track < 24 {
            g[V0] = li(TRACK_NAMES[track]);
            return;
        }
        g[V1] = li(TRACK_NAMES[24]);
    }
    g[V0] = g[V1];
}

/// `func_8002D968(a, b)`: if bit 14 of `[0x8009B7D8]` is set, 1 when the
/// first three bytes at `a` and `b` are equal, else 0. It stops at the first
/// byte that differs. With the bit clear, 0 and no reads.
///
/// Leaves `t6`/`t7` = the word and its bit, the bytes compared in
/// `t8`/`t9`, `t0`/`t1`, `t2`/`t3`, and `v1 = v0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002D968(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = lw(&mem, li(0x800A_0000), -0x4828);
    g[T7] = g[T6] & 0x4000;
    if g[T7] == 0 {
        g[V0] = 0;
        return;
    }
    for (k, (x, y)) in [(T8, T9), (T0, T1), (T2, T3)].into_iter().enumerate() {
        g[x] = lbu(&mem, g[A0], k as i32);
        g[y] = lbu(&mem, g[A1], k as i32);
        g[V1] = sltu(g[x] ^ g[y], 1);
        g[V0] = g[V1];
        if g[V1] == 0 {
            return;
        }
    }
}

/// `func_8002D9D0(a, b)`: 4 if `(s8) a < 3` and `(u8) b < 6`, else 3.
/// Spills both arguments to their slots `[sp]`, `[sp + 4]`.
///
/// Leaves `t6 = a << 24`, `t7 = (s8) a`, `t8 = b & 0xFF` and `at` = the
/// last test made.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002D9D0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 24);
    g[T7] = sra(g[T6], 24);
    g[AT] = slt(g[T7], 3);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[SP], 4, g[A1]);
    g[T8] = g[A1] & 0xFF;
    g[V0] = 3;
    if g[AT] != 0 {
        g[AT] = slt(g[T8], 6);
        if g[AT] != 0 {
            g[V0] = 4;
        }
    }
}

/// `func_8002DAD0(p, i, bit)`: whether bit `(u8) bit` (mod 32) of the byte
/// `table[(s8) i]` is set, 1 or 0. The table is at `0x80113E68` if the byte
/// `[p + 0x6C]` (signed) is nonzero, else at `0x8011368C`. Spills `i` and
/// `bit` to their slots `[sp + 4]`, `[sp + 8]`.
///
/// The C reads the `0x80113E68` byte first either way; only the loads'
/// values are observable, so the port reads the one it keeps. Leaves `a2 =
/// t8 = (u8) bit`, `t6 = i << 24`, `t7 = (s8) i`, `t9` = the flag byte, `v1`
/// = the table byte, `t0 = 1`, `t1` = the mask.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002DAD0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    sw(m, g[SP], 4, g[A1]);
    sw(m, g[SP], 8, g[A2]);
    g[T9] = lb(m, g[A0], 0x6C);
    g[T6] = sll(g[A1], 24);
    g[T7] = sra(g[T6], 24);
    g[T8] = g[A2] & 0xFF;
    g[A2] = g[T8];
    let table = if g[T9] != 0 { 0x3E68 } else { 0x368C };
    g[V1] = lbu(m, addu(li(0x8011_0000), g[T7]), table);
    g[T0] = 1;
    g[T1] = sllv(g[T0], g[A2]);
    g[V0] = g[V1] & g[T1];
    g[T2] = sltu(0, g[V0]);
    g[V0] = g[T2];
}

/// `func_8002DC7C(p)`: 1 or 0. If the byte `[p + 0x6C]` (signed) is
/// nonzero: whether the halfwords at `0x80113E6C`, `0x80113E6E`,
/// `0x80113E70` are all `0x3FFF` and the one at `0x80113E72` is `0xFF`
/// (signed loads, so `0xBFFF` etc. don't match). Otherwise: bit 5 of
/// `[0x80113688]`.
///
/// Leaves `v1 = 0x80113E60` and `t7 = 0x80110000` (or the word), then the
/// halfwords read in `t9`, `t0`, `t1`, `t2` (up to the first mismatch), `at
/// = 0xFF` if the last was read, `t8` = the bit.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002DC7C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[T6] = lb(m, g[A0], 0x6C);
    g[V1] = li(0x8011_3E60);
    g[T7] = li(0x8011_0000);
    if g[T6] == 0 {
        g[T7] = lw(m, g[T7], 0x3688);
        g[T8] = g[T7] & 0x20;
        g[V0] = u64::from(g[T8] != 0);
        return;
    }
    g[V0] = 0x3FFF;
    for (r, off) in [(T9, 0xC), (T0, 0xE), (T1, 0x10)] {
        g[r] = lh(m, g[V1], off);
        if g[V0] != g[r] {
            g[V0] = 0;
            return;
        }
    }
    g[T2] = lh(m, g[V1], 0x12);
    g[AT] = 0xFF;
    g[V0] = u64::from(g[T2] == g[AT]);
}

/// `func_8002DFB0(a, b)`: an empty function that spills both arguments to
/// their slots `[sp]`, `[sp + 4]`. `func_8002E034` calls it with `(0x3F,
/// 0)` and `(0x40, 0)`, presumably a compiled-out debug hook.
///
/// Domain: canonical `sp` with its slots in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002DFB0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    sw(&mut mem, g[SP], 0, g[A0]);
    sw(&mut mem, g[SP], 4, g[A1]);
}

/// `func_8002E028()`: `[0x800A268C] = 0`. Leaves `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002E028(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    sw(&mut mem, g[AT], 0x268C, 0);
}

/// `func_8002E0A8()`: `[0x800A2690] = 0`, the flag `func_8002E034` loops
/// on (NOTES.md, "Asset heap"). Leaves `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002E0A8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    sw(&mut mem, g[AT], 0x2690, 0);
}

/// `func_8002E8D4(i)`: the word `[0x800D7498 + 4 * i]`. Unbounded; the
/// index is shifted as a 32-bit value and the address wraps at 32 bits.
/// Domain: the address in RDRAM. Leaves `t6 = i << 2`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002E8D4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 2);
    g[V0] = lw(&mem, addu(li(0x800D_0000), g[T6]), 0x7498);
}

/// `func_8002F054()`: the word `[0x800A26F4]`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F054(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[V0] = lw(&mem, li(0x800A_0000), 0x26F4);
}

/// `func_8002F1CC()`: `[0x800A26F8] = 0` and the byte `[0x800A26FC] = 1`.
/// Leaves `at = 0x800A0000`, `t6 = 1`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F1CC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x26F8, 0);
    g[T6] = 1;
    sb(m, g[AT], 0x26FC, g[T6]);
}

/// `func_8002F1E4`: empty (`jr ra`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F1E4(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_8002F440()`: fill `[0x800AE8B0, 0x80149C60)` with `0xDDEEDDEE`.
/// That range starts where boot's BSS clear starts (NOTES.md, "Code
/// segment"); a debug fill pattern (**guess**). The bounds are constants,
/// so its `sltu` guard always passes.
///
/// Leaves `a0 = 0xDDEEDDEE`, `v0 = v1 = 0x80149C60`, `t6 = 0x800AE8B1` and
/// `at = 0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F440(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = li(0x8014_9C60);
    g[T6] = li(0x800A_E8B1);
    g[V0] = li(0x800A_E8B0);
    g[AT] = sltu(g[V1], g[T6]);
    if g[AT] == 0 {
        g[A0] = li(0xDDEE_DDEE);
        loop {
            g[V0] = addu(g[V0], 4);
            g[AT] = sltu(g[V0], g[V1]);
            sw(m, g[V0], -4, g[A0]);
            if g[AT] == 0 {
                break;
            }
        }
    }
}

/// `func_8002F480()`: fill from `0x8014D7E0`, where heap_init puts the
/// heap's start (NOTES.md, "Asset heap"), up to the end of RDRAM (`osMemSize
/// | 0x80000000`) with `0xBBCCBBCC`. `osMemSize` (`[0x80000318]`) is read
/// again before each word after the first. Nothing is written unless the
/// end is above the start (unsigned compare of the sign-extended values). A
/// word is written wherever the fill starts below the end, so an unaligned
/// end gets the last word in full.
///
/// Domain: the end within the RDRAM buffer (`osMemSize` at most 8 MB). Leaves
/// `a1 = 0x80000318`, `a0 = 0x80000000`, `v1 = 0xBBCC0000` (`0xBBCCBBCC` if
/// anything was written), `v0` = the start, or the address past the last
/// word, `t6`/`t7` = `osMemSize` and the end, `t8`/`t9` the same from the
/// last re-read, and `at = 0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F480(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A1] = li(0x8000_0318);
    g[T6] = lw(m, g[A1], 0);
    g[A0] = li(0x8000_0000);
    g[V0] = li(0x8014_D7E0);
    g[T7] = g[A0] | g[T6];
    g[AT] = sltu(g[V0], g[T7]);
    g[V1] = li(0xBBCC_0000);
    if g[AT] == 0 {
        return;
    }
    g[V1] |= 0xBBCC;
    sw(m, g[V0], 0, g[V1]);
    loop {
        g[T8] = lw(m, g[A1], 0);
        g[V0] = addu(g[V0], 4);
        g[T9] = g[A0] | g[T8];
        g[AT] = sltu(g[V0], g[T9]);
        if g[AT] == 0 {
            break;
        }
        sw(m, g[V0], 0, g[V1]);
    }
}

/// `func_8002F740`: a bare epilogue, `ra = [sp + 0x1C]` (sign-extended) and
/// `sp += 0x20`, then return. Nothing calls or references it; probably dead
/// code left after a function that doesn't return (**guess**).
///
/// Domain: canonical `sp` with `[sp + 0x1C]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F740(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(&mem, g[SP], 0x1C);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_8002F9E0(a0, a1, a2, a3)`: an empty function with an 8-byte frame
/// that spills all four arguments to the caller's slots `[sp]..[sp +
/// 0xC]`. `sp` comes back sign-extended from its low word (two `addiu`s).
///
/// Domain: canonical `sp` with its slots in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F9E0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-8i64) as u64);
    for (k, r) in [A0, A1, A2, A3].into_iter().enumerate() {
        sw(m, g[SP], 8 + 4 * k as i32, g[r]);
    }
    g[SP] = addu(g[SP], 8);
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

/// `func_8002FE80(a0, a1, a2, a3)`: an empty function that spills all four
/// arguments to their slots `[sp]..[sp + 0xC]`.
///
/// Domain: canonical `sp` with its slots in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002FE80(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    for (k, r) in [A0, A1, A2, A3].into_iter().enumerate() {
        sw(m, g[SP], 4 * k as i32, g[r]);
    }
}

/// `func_8002FE94(n)`: `n` rounded up to a power of two, at least 16, for
/// `1 <= n <= 2^30`. It takes the highest set bit `b` of `n` among bits
/// 30..0 and returns `b`, or `2b` if `b < n` (signed), then at least 16.
///
/// QUIRKs: for `2^30 < n < 2^31` the doubled bit is `0x80000000`,
/// sign-extended, so negative, and the result is 16. For negative `n`, `b
/// < n` never holds, so it returns the highest set bit among bits 30..0
/// (at least 16), e.g. `2^30` for -5.
///
/// Leaves `a1` = 31 minus the bits tested, `v0 = v1`, `t6` = `b / 2` (0
/// if no bit was found), `t7 = b` (0 when `b` is bit 0) and `at` = the
/// minimum test.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002FE94(rdram: *mut u8, ctx: *mut RecompContext) {
    let (_mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[V1] = li(0x4000_0000);
    g[A1] = 0x1F;
    g[V0] = g[V1] & g[A0];
    loop {
        g[T6] = sra(g[V1], 1);
        g[V1] = g[T6];
        g[A1] = addu(g[A1], u64::MAX);
        if g[V0] != 0 || g[A1] == 0 {
            break;
        }
        g[V0] = g[V1] & g[A0];
    }
    g[T7] = sll(g[V1], 1);
    g[AT] = slt(g[T7], g[A0]);
    g[V1] = g[T7];
    if g[AT] != 0 {
        g[V1] = sll(g[T7], 1);
    }
    g[AT] = slt(g[V1], 0x10);
    if g[AT] != 0 {
        g[V1] = 0x10;
    }
    g[V0] = g[V1];
}

/// `func_80030A7C(p, q)`: merge the word list `q` into the model-style
/// header `p` (tags as in model headers; **guess** at the purpose).
/// 1. Node list: for each word of `p` before its -1 terminator, a nonzero
///    word at the same index of `q` replaces it (nothing if `p` starts
///    with -1).
/// 2. After the terminator, `p` may have `"Data"` (tag, count `n`, `n`
///    words) and then `"Anim"` (tag, words up to a 0); both are skipped.
/// 3. If `p` then has `"AltN"`, and `q` has `"AltN"` right after its own
///    node list (`q` is not skipped past any Data or Anim), each nonzero
///    pointer in `p`'s AltN list gets `q`'s word at the same index stored
///    through it, until `p`'s 0.
///
/// Domain: the lists terminated, and the pointers in RDRAM and aligned.
/// Leaves `a2 = -1`, `a1 = "AltN"`, `v0`/`v1` at the last words read of
/// `p`/`q`, `a0` the last `p` word, `at` the last tag compared, and the
/// loads in `t0`, `t1`, `t6`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80030A7C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    // 1. Node lists.
    g[T6] = lw(m, g[A0], 0);
    g[A2] = u64::MAX;
    g[V0] = g[A0];
    g[V1] = g[A1];
    if g[A2] != g[T6] {
        g[A0] = lw(m, g[V1], 0);
        loop {
            if g[A0] != 0 {
                sw(m, g[V0], 0, g[A0]);
            }
            g[T7] = lw(m, g[V0], 4);
            g[V0] = addu(g[V0], 4);
            g[V1] = addu(g[V1], 4);
            if g[A2] == g[T7] {
                break;
            }
            g[A0] = lw(m, g[V1], 0);
        }
    }
    // 2. Skip "Data" and "Anim".
    g[T8] = lw(m, g[V0], 4);
    g[AT] = li(0x4461_7461); // "Data"
    g[V0] = addu(g[V0], 4);
    g[V1] = addu(g[V1], 4);
    if g[T8] == g[AT] {
        g[A0] = lw(m, g[V0], 4);
        g[V0] = addu(g[V0], 8);
        if (g[A0] as i64) <= 0 {
            g[A0] = addu(g[A0], u64::MAX);
        } else {
            loop {
                g[A0] = addu(g[A0], u64::MAX);
                g[V0] = addu(g[V0], 4);
                if (g[A0] as i64) <= 0 {
                    break;
                }
            }
        }
    }
    g[A0] = lw(m, g[V0], 0);
    g[AT] = li(0x416E_696D); // "Anim"
    g[A1] = li(0x416C_0000);
    if g[A0] == g[AT] {
        // The C also tests the tag against 0 here, which can't succeed.
        g[T9] = lw(m, g[V0], 4);
        loop {
            g[V0] = addu(g[V0], 4);
            if g[T9] == 0 {
                break;
            }
            g[T9] = lw(m, g[V0], 4);
        }
        g[V0] = addu(g[V0], 4);
        g[A0] = lw(m, g[V0], 0);
    }
    // 3. "AltN" in both.
    g[A1] |= 0x744E; // "AltN"
    if g[A1] != g[A0] {
        return;
    }
    g[T0] = lw(m, g[V1], 0);
    if g[A1] != g[T0] {
        return;
    }
    g[A0] = lw(m, g[V0], 4);
    g[V0] = addu(g[V0], 4);
    g[V1] = addu(g[V1], 4);
    if g[A0] == 0 {
        return;
    }
    g[T1] = lw(m, g[V1], 0);
    loop {
        g[V0] = addu(g[V0], 4);
        g[V1] = addu(g[V1], 4);
        sw(m, g[A0], 0, g[T1]);
        g[A0] = lw(m, g[V0], 0);
        if g[A0] == 0 {
            break;
        }
        g[T1] = lw(m, g[V1], 0);
    }
}

/// `func_80030B68(&a, &b, &c)`: model_load's statistics (NOTES.md, "Texture
/// and model loaders"): `*a = [0x800D9DC8]` (model bytes), `*c =
/// [0x800D9DCC]` (texture bytes), `*b = [0x800D9DD0]` (always 0). Leaves
/// `t6`/`t7`/`t8` = the three words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80030B68(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = lw(m, li(0x800E_0000), -0x6238);
    sw(m, g[A0], 0, g[T6]);
    g[T7] = lw(m, li(0x800E_0000), -0x6234);
    sw(m, g[A2], 0, g[T7]);
    g[T8] = lw(m, li(0x800E_0000), -0x6230);
    sw(m, g[A1], 0, g[T8]);
}

/// `func_800313D8(p, b, n)`: `memset`: store the byte `b` at `p..p + n`,
/// return `p`. `n == 0` (all 64 bits) stores nothing; otherwise the count
/// runs on its low word.
///
/// Domain: `n` canonical and the bytes in RDRAM (a negative `n` would run
/// for 2^32 bytes). Leaves `v1 = 0`, `a2 = -1` (or `n - 1` if `n == 0`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800313D8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = sltu(g[A2], 1) ^ 1;
    g[V0] = g[A0];
    g[A2] = addu(g[A2], u64::MAX);
    while g[V1] != 0 {
        g[V1] = sltu(g[A2], 1) ^ 1;
        sb(m, g[V0], 0, g[A1]);
        g[A2] = addu(g[A2], u64::MAX);
        g[V0] = addu(g[V0], 1);
    }
    g[V0] = g[A0];
}

/// The four 28-byte records at `0x800DB8A0` that [`func_800314C0`] ..
/// [`func_80031640`] use, by index (sound channels? **guess**).
pub const CHANNELS: u32 = 0x800D_B8A0;

/// `rec = CHANNELS + 28 * i` as the code computes it: `t = ((i << 3) - i)
/// << 2`, 32-bit.
fn channel_offset(g: &mut [u64; 32], i: usize, t: usize) {
    g[t] = sll(g[i], 3);
    g[t] = subu(g[t], g[i]);
    g[t] = sll(g[t], 2);
}

/// `func_800314C0(i)`: the word `[CHANNELS + 28 * i]`. Unbounded. Leaves
/// `t6 = 28 * i`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800314C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    channel_offset(g, A0, T6);
    g[V0] = lw(&mem, addu(li(0x800E_0000), g[T6]), -0x4760);
}

/// `func_800314DC(i, value, a2, a3)`: set channel `i`'s halfwords `+4` and
/// `+0x16` to `(u16) value`, and `+8` to -1 if `a2 == 0`, else 1 if `a3
/// == 0`, else 2 (full 64-bit tests). `+6` becomes 0x8000 unless it was
/// nonzero, `+4` already equalled the value and `a2 != 0`. Spills `value`
/// to its slot `[sp + 4]`.
///
/// Leaves `a1 = v1 = (u16) value = t6`, `v0` = the record, `t7 = 28 * i`,
/// `t8 = CHANNELS`, `t9` = the old `+6`, `t0` = the old `+4` if `+6` was
/// nonzero, `t1 = 0x8000`, and `t4 = -1`, or `t3 = 1` (and `t2 = 2`) as
/// `+8` chose.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800314DC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    channel_offset(g, A0, T7);
    g[T8] = li(0x800D_B8A0);
    g[V0] = addu(g[T7], g[T8]);
    g[T9] = lhu(m, g[V0], 6);
    sw(m, g[SP], 4, g[A1]);
    g[T6] = g[A1] & 0xFFFF;
    g[A1] = g[T6];
    let mut keep = false;
    if g[T9] != 0 {
        g[T0] = lhu(m, g[V0], 4);
        g[V1] = g[T6];
        keep = g[T6] == g[T0] && g[A2] != 0;
    }
    g[T1] = 0x8000;
    if !keep {
        sh(m, g[V0], 6, g[T1]);
    }
    g[V1] = g[A1];
    sh(m, g[V0], 4, g[A1]);
    sh(m, g[V0], 0x16, g[V1]);
    if g[A2] == 0 {
        g[T4] = u64::MAX;
        sh(m, g[V0], 8, g[T4]);
    } else if g[A3] == 0 {
        g[T3] = 1;
        sh(m, g[V0], 8, g[T3]);
    } else {
        g[T3] = 1;
        g[T2] = 2;
        sh(m, g[V0], 8, g[T2]);
    }
}

/// The frame shared by [`func_80031560`], [`func_800315D8`] and
/// [`func_80031640`]: `i == -1` (full 64-bit compare) calls the function
/// itself for channels 0..3 (through its C symbol), otherwise `one(i)`
/// updates channel `i`. `ra`, `s0` and `s1` are saved in a 0x20-byte frame
/// and come back as their low words, sign-extended, so the loop leaves only
/// `a0 = 3`, `at = -1` and the last call's leftovers.
unsafe fn each_channel(
    rdram: *mut u8,
    ctx: *mut RecompContext,
    this: crate::recomp::RecompFn,
    one: fn(&mut Mem, &mut [u64; 32]),
) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    g[AT] = u64::MAX;
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x18, g[S1]);
    sw(m, g[SP], 0x14, g[S0]);
    if g[A0] == g[AT] {
        g[S0] = 0;
        g[S1] = 4;
        loop {
            ctx.gpr[A0] = ctx.gpr[S0];
            call(this, m, ctx);
            let g = &mut ctx.gpr;
            g[S0] = addu(g[S0], 1);
            if g[S0] == g[S1] {
                break;
            }
        }
    } else {
        one(m, g);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_80031560(i)`: start channel `i` (**guess**): `[rec + 0xC] = 1`
/// and the halfword `[rec + 6] = 0x8000`; `i == -1` does channels 0..3
/// ([`each_channel`]). Leaves `t6 = 28 * i`, `t7 = CHANNELS`, `v0` = the
/// record, `t8 = 1`, `t9 = 0x8000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80031560(rdram: *mut u8, ctx: *mut RecompContext) {
    each_channel(rdram, ctx, imports::func_80031560, |m, g| {
        channel_offset(g, A0, T6);
        g[T7] = li(0x800D_B8A0);
        g[V0] = addu(g[T6], g[T7]);
        g[T8] = 1;
        g[T9] = 0x8000;
        sw(m, g[V0], 0xC, g[T8]);
        sh(m, g[V0], 6, g[T9]);
    });
}

/// `func_800315D8(i)`: `[rec + 0xC] = 0` for channel `i`, or channels
/// 0..3 if `i == -1` ([`each_channel`]). Leaves `t6 = 28 * i`, `at =
/// 0x800E0000 + 28 * i`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800315D8(rdram: *mut u8, ctx: *mut RecompContext) {
    each_channel(rdram, ctx, imports::func_800315D8, |m, g| {
        channel_offset(g, A0, T6);
        g[AT] = addu(li(0x800E_0000), g[T6]);
        sw(m, g[AT], -0x4754, 0);
    });
}

/// `func_80031640(i)`: the halfword `[rec + 8] = 0` for channel `i`, or
/// channels 0..3 if `i == -1` ([`each_channel`]). Leaves `t6 = 28 * i`,
/// `at = 0x800E0000 + 28 * i`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80031640(rdram: *mut u8, ctx: *mut RecompContext) {
    each_channel(rdram, ctx, imports::func_80031640, |m, g| {
        channel_offset(g, A0, T6);
        g[AT] = addu(li(0x800E_0000), g[T6]);
        sh(m, g[AT], -0x4758, 0);
    });
}

/// `func_80031BEC(i)`: `[0x800DB910 + 4 * i] = 1`, a word per channel
/// right after [`CHANNELS`] (unbounded). Leaves `t7 = 4 * i`, `at =
/// 0x800E0000 + 4 * i`, `t6 = 1`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80031BEC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T7] = sll(g[A0], 2);
    g[AT] = addu(li(0x800E_0000), g[T7]);
    g[T6] = 1;
    sw(&mut mem, g[AT], -0x46F0, g[T6]);
}

/// `func_80031F80(p)`: the destination of [`func_80031FA4`]'s animation,
/// `[[0x800A2DD4] + 4] = [p + 8]`. Leaves `t7` = the object, `t6` = the
/// word.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80031F80(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T7] = lw(m, li(0x800A_0000), 0x2DD4);
    g[T6] = lw(m, g[A0], 8);
    sw(m, g[T7], 4, g[T6]);
}

/// `func_80031F94()`: clear the destination of [`func_80031FA4`]'s
/// animation, `[[0x800A2DD4] + 4] = 0`, which stops its copying. Leaves
/// `t6` = the object.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80031F94(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = lw(&mem, li(0x800A_0000), 0x2DD4);
    sw(&mut mem, g[T6], 4, 0);
}

/// `func_80031FA4()`: step the cycling animation of the object `o =
/// [0x800A2DD4]`, probably palette colour cycling (**guess**). The timer
/// `[o + 0x14]` (f32) loses the double at `0x80120BF0` (computed in double,
/// rounded to single). While it is negative the period `[o + 0x10]` is
/// added back, counting the additions. Then, if the destination `[o + 4]`
/// is nonzero, the count positive and `n = (s16) [o + 0x18]` positive, for
/// each `i < n`: `phase[i] = (phase[i] + count) % (s16) [o + 0xC]` (signed
/// remainder, stored as a byte) and `dest[map[i]] = src[phase[i]]`. The
/// phases are bytes at `[o + 0x20]`, the map bytes at `[o + 0x1C]`, `src`
/// and `dest` halfwords at `[o + 8]` and `[o + 4]`.
///
/// `o` is read again from `0x800A2DD4` after every store, and `n` after
/// every entry. QUIRK: a period that doesn't bring the timer back (zero,
/// negative, or too small to change it) never ends the loop.
///
/// Domain: the timer, the step and the period not NaN (`NAN_CHECK`), the
/// loop ending, a nonzero modulus, and the arrays in RDRAM. A zero modulus
/// faults the host's divide in the C before its `break 7`, and the port's
/// `div` asserts. `break 6` (modulus -1 with `phase + count = 0x80000000`)
/// would need about 2^31 additions of the period, and after about 2^25
/// adding the period no longer changes the timer, so the loop never ends.
/// So neither `do_break` is reached, but the port keeps both, like the C.
///
/// Leaves `a2 = 0x800A2DD4`, `v1 = o`, `v0` = the count, `f2 = 0.0`, the
/// timer arithmetic in `f4`..`f18`, and from the copy loop `a0 = n` and
/// the temporaries in `a1`, `at`, `t0`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80031FA4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A2] = li(0x800A_2DD4);
    g[V1] = lw(m, g[A2], 0);
    g[AT] = li(0x8012_0000);
    ctx.fpr[8].u64 = ld(m, g[AT], 0xBF0);
    ctx.fpr[4].set_u32l(lw(m, g[V1], 0x14) as u32);
    ctx.fpr[2].set_u32l(0);
    g[V0] = 0;
    ctx.fpr[6].set_d(f64::from(ctx.fpr[4].fl()));
    ctx.fpr[10].set_d(ctx.fpr[6].d() - ctx.fpr[8].d());
    ctx.fpr[16].set_fl(fpu::cvt_s_d(ctx.fpr[10].d(), fpu::NEAREST));
    sw(m, g[V1], 0x14, u64::from(ctx.fpr[16].u32l()));
    // Add periods back until the timer isn't negative, counting them.
    g[V1] = lw(m, g[A2], 0);
    ctx.fpr[0].set_u32l(lw(m, g[V1], 0x14) as u32);
    while ctx.fpr[0].fl() < ctx.fpr[2].fl() {
        ctx.fpr[18].set_u32l(lw(m, g[V1], 0x10) as u32);
        g[V0] = addu(g[V0], 1);
        ctx.fpr[4].set_fl(ctx.fpr[0].fl() + ctx.fpr[18].fl());
        sw(m, g[V1], 0x14, u64::from(ctx.fpr[4].u32l()));
        g[V1] = lw(m, g[A2], 0);
        ctx.fpr[0].set_u32l(lw(m, g[V1], 0x14) as u32);
    }
    g[T6] = lw(m, g[V1], 4);
    if g[T6] == 0 || (g[V0] as i64) <= 0 {
        return;
    }
    g[T7] = lh(m, g[V1], 0x18);
    g[A0] = 0;
    if (g[T7] as i64) <= 0 {
        return;
    }
    g[T8] = lw(m, g[V1], 0x20);
    loop {
        // phase[i] = (phase[i] + count) % modulus
        g[T1] = lh(m, g[V1], 0xC);
        g[A1] = addu(g[T8], g[A0]);
        g[T9] = lbu(m, g[A1], 0);
        g[T0] = addu(g[T9], g[V0]);
        let (_, hi) = div(g[T0], g[T1]);
        g[T2] = hi;
        sb(m, g[A1], 0, g[T2]);
        g[V1] = lw(m, g[A2], 0);
        // IDO's divide checks (see the domain above).
        if g[T1] == 0 {
            imports::runtime::do_break(0x8003_2064);
        }
        g[AT] = u64::MAX;
        let minus_one = g[T1] == g[AT];
        g[AT] = li(0x8000_0000);
        if minus_one && g[T0] == g[AT] {
            imports::runtime::do_break(0x8003_207C);
        }
        // dest[map[i]] = src[phase[i]]
        g[T4] = lw(m, g[V1], 0x20);
        g[T1] = lw(m, g[V1], 0x1C);
        g[T3] = lw(m, g[V1], 8);
        g[T5] = addu(g[T4], g[A0]);
        g[T6] = lbu(m, g[T5], 0);
        g[T2] = addu(g[T1], g[A0]);
        g[T4] = lbu(m, g[T2], 0);
        g[T7] = sll(g[T6], 1);
        g[T0] = lw(m, g[V1], 4);
        g[T8] = addu(g[T3], g[T7]);
        g[T9] = lh(m, g[T8], 0);
        g[T5] = sll(g[T4], 1);
        g[T6] = addu(g[T0], g[T5]);
        sh(m, g[T6], 0, g[T9]);
        g[V1] = lw(m, g[A2], 0);
        g[A0] = addu(g[A0], 1);
        g[T3] = lh(m, g[V1], 0x18);
        g[AT] = slt(g[A0], g[T3]);
        if g[AT] == 0 {
            break;
        }
        g[T8] = lw(m, g[V1], 0x20);
    }
}

/// `func_80033B14(p)`: 1 if `p != 0`, `o = *p != 0` and bit 29 of the
/// object's flags `[o + 0x100]` is set (NOTES.md, the object table), else
/// 0. Leaves `v0`, and `t6`/`t7` = the flags and `flags << 2` when read.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033B14(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    if g[A0] == 0 {
        g[V0] = 0;
        return;
    }
    g[V0] = lw(&mem, g[A0], 0);
    if g[V0] == 0 {
        return;
    }
    g[T6] = lw(&mem, g[V0], 0x100);
    g[T7] = sll(g[T6], 2);
    g[V0] = u64::from((g[T7] as i64) < 0);
}

/// `func_80033DC4`: empty (`jr ra`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033DC4(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// A ring of `count` entries of `1 << shift` bytes at `base`, its last
/// index at `index`: advance it (signed; wrapping to 0 at `count`, with
/// the stored index going through `count` first) and return the entry. So
/// the first entry handed out after a reset to 0 is entry 1.
fn ring_next(m: &mut Mem, g: &mut [u64; 32], index: u32, base: u32, count: u64, shift: u32) {
    g[V0] = li(index);
    g[T6] = lw(m, g[V0], 0);
    g[T9] = li(base);
    g[V1] = addu(g[T6], 1);
    g[AT] = slt(g[V1], count);
    sw(m, g[V0], 0, g[V1]);
    if g[AT] == 0 {
        sw(m, g[V0], 0, 0);
        g[V1] = 0;
    }
    g[T8] = sll(g[V1], shift);
    g[V0] = addu(g[T8], g[T9]);
}

/// `func_80033DD0()`: the next of 256 32-byte entries at `0x800E0C50`,
/// index at `0x800A3CC0` ([`ring_next`]). QUIRK: a negative stored index
/// isn't reset (signed test) and gives an entry below the ring. Leaves `v1`
/// = the index, `t6` = the old one, `t8` = the offset, `t9` = the base,
/// `at` = the test.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033DD0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    ring_next(&mut mem, &mut ctx.gpr, 0x800A_3CC0, 0x800E_0C50, 0x100, 5);
}

/// `func_80033E08()`: the next of 3072 64-byte entries at
/// `0x800E2C50`, index at `0x800A3CC4` ([`ring_next`]; the same QUIRK and
/// leftovers as [`func_80033DD0`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033E08(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    ring_next(&mut mem, &mut ctx.gpr, 0x800A_3CC4, 0x800E_2C50, 0xC00, 6);
}

/// `func_800344C8()`: `[0x800A3FF4] = 1`, then decrement `[0x800A3FF0]` if
/// it is positive (signed). Leaves `t6 = 1`, `at = 0x800A0000`, `v1 =
/// 0x800A3FF0`, `v0` = the old count, `t7` = old - 1.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800344C8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = 1;
    g[AT] = li(0x800A_0000);
    g[V1] = li(0x800A_3FF0);
    sw(m, g[AT], 0x3FF4, g[T6]);
    g[V0] = lw(m, g[V1], 0);
    g[T7] = addu(g[V0], u64::MAX);
    if (g[V0] as i64) > 0 {
        sw(m, g[V1], 0, g[T7]);
    }
}

/// Append to the display list at `[0x80112C90]` (`dl` holds that address,
/// in the register the caller chose): `gSPMatrix(proj, projection | load)`
/// (`0xDA380003`) and `gSPForceMatrix(mvp)` (`G_MOVEMEM` `0xDC38000E`,
/// then `G_MOVEWORD` `0xDB0C0000`, `0x00010000`), advancing the pointer
/// before each command's words are written (the last command's second word
/// first). `proj` and `mvp` are loaded between the writes, from `[proj_base
/// + proj_off]` and `[mvp_base + mvp_off]`, into `t8` and `t1`.
///
/// Leaves `v1` = the last command's address, `t6`/`t9`/`t2` = the pointer
/// after each command, `t7`/`t0`/`t3`/`t4` = the command words.
fn projection_and_force_matrix(
    m: &mut Mem,
    g: &mut [u64; 32],
    dl: usize,
    (proj_base, proj_off): (usize, i32),
    (mvp_base, mvp_off): (usize, i32),
) {
    g[V1] = lw(m, g[dl], 0);
    g[T7] = li(0xDA38_0003);
    g[T6] = addu(g[V1], 8);
    sw(m, g[dl], 0, g[T6]);
    sw(m, g[V1], 0, g[T7]);
    g[T8] = lw(m, g[proj_base], proj_off);
    g[T0] = li(0xDC38_000E);
    sw(m, g[V1], 4, g[T8]);
    g[V1] = lw(m, g[dl], 0);
    g[T9] = addu(g[V1], 8);
    sw(m, g[dl], 0, g[T9]);
    sw(m, g[V1], 0, g[T0]);
    g[T1] = lw(m, g[mvp_base], mvp_off);
    g[T3] = li(0xDB0C_0000);
    g[T4] = li(0x1_0000);
    sw(m, g[V1], 4, g[T1]);
    g[V1] = lw(m, g[dl], 0);
    g[T2] = addu(g[V1], 8);
    sw(m, g[dl], 0, g[T2]);
    sw(m, g[V1], 4, g[T4]);
    sw(m, g[V1], 0, g[T3]);
}

/// `func_80034DA8()`: `[0x800A3FF8] = 0`, then append the projection and
/// forced matrices at `[0x801134D0]` and `[0x801134D4]` to the display list
/// ([`projection_and_force_matrix`]). Leaves `a2 = 0x80112C90`, `at =
/// 0x800A0000`, `t8`/`t1` = the two matrix addresses.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80034DA8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[A2] = li(0x8011_2C90);
    g[AT] = li(0x800A_0000);
    sw(&mut mem, g[AT], 0x3FF8, 0);
    // The matrices are read through t8/t1 = 0x80110000 + offset.
    g[T8] = li(0x8011_0000);
    g[T1] = li(0x8011_0000);
    projection_and_force_matrix(&mut mem, g, A2, (T8, 0x34D0), (T1, 0x34D4));
}

/// `func_8003527C(cam)`: append the projection and forced matrices at
/// `[cam + 0x34]` and `[cam + 0x38]` to the display list
/// ([`projection_and_force_matrix`]). Leaves `a3 = 0x80112C90`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003527C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[A3] = li(0x8011_2C90);
    projection_and_force_matrix(&mut mem, g, A3, (A0, 0x34), (A0, 0x38));
}

/// `func_800352E4()`: [`func_80034DA8`] without clearing `[0x800A3FF8]`.
/// Leaves `a2 = 0x80112C90`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800352E4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[A2] = li(0x8011_2C90);
    g[T8] = li(0x8011_0000);
    g[T1] = li(0x8011_0000);
    projection_and_force_matrix(&mut mem, g, A2, (T8, 0x34D0), (T1, 0x34D4));
}

/// `func_80035698(v)`: `[0x800A3D9C] = 1` if `v == 1` (full 64-bit
/// compare), else 0. Leaves `v0 = 1`, `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80035698(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[V0] = 1;
    g[AT] = li(0x800A_0000);
    let v = if g[A0] == g[V0] { g[V0] } else { 0 };
    sw(&mut mem, g[AT], 0x3D9C, v);
}

/// One render-mode change tried by [`func_800356BC`]: if the first mode
/// word is `from`, set both words to `to`. The new words are built in the
/// temporaries `regs` (upper half, then `| lower`); the first one's upper
/// half is loaded even when `from` doesn't match (a delay slot).
struct ModeChange {
    from: u32,
    to: (u32, u32),
    regs: (usize, usize),
}

/// The four render modes (first word, second word) [`func_800356BC`] moves
/// between: two independent switches, "ZBZB" (z-buffering, **guess**) and
/// "AAEN" (anti-aliasing, **guess**).
const MODE_NONE: (u32, u32) = (0x0C08_4000, 0x0302_4000);
const MODE_ZB: (u32, u32) = (0x0044_2230, 0x0011_2230);
const MODE_AA: (u32, u32) = (0x0044_2048, 0x0011_2048);
const MODE_BOTH: (u32, u32) = (0x0044_2078, 0x0011_2078);

/// By switch (ZBZB, AAEN) and on (`a2 == 1`) or off: the two changes, in
/// the order the code tries them.
const MODE_CHANGES: [[[ModeChange; 2]; 2]; 2] = [
    [
        [
            ModeChange { from: MODE_ZB.0, to: MODE_NONE, regs: (T0, T1) },
            ModeChange { from: MODE_BOTH.0, to: MODE_AA, regs: (T2, T3) },
        ],
        [
            ModeChange { from: MODE_NONE.0, to: MODE_ZB, regs: (T6, T7) },
            ModeChange { from: MODE_AA.0, to: MODE_BOTH, regs: (T8, T9) },
        ],
    ],
    [
        [
            ModeChange { from: MODE_AA.0, to: MODE_NONE, regs: (T8, T9) },
            ModeChange { from: MODE_BOTH.0, to: MODE_ZB, regs: (T0, T1) },
        ],
        [
            ModeChange { from: MODE_NONE.0, to: MODE_AA, regs: (T4, T5) },
            ModeChange { from: MODE_ZB.0, to: MODE_BOTH, regs: (T6, T7) },
        ],
    ],
];

/// `func_800356BC(tag, _, a2)`: switch the render mode words at
/// `0x800A3DA0`/`0x800A3DA4`. Tag `"ZBZB"` or `"AAEN"` turns one of two
/// switches on (`a2 == 1`, full 64-bit compare) or off, moving between the
/// four modes of [`MODE_CHANGES`]. A mode that doesn't have the other
/// state is left alone (e.g. ZBZB on when already on). Tag `"Full"` sets
/// both words to `a2`. Any other tag does nothing. Spills `a1` to its slot
/// `[sp + 4]`.
///
/// Leaves `at` = the last constant compared (or `0x800A0000` after a
/// store), `v1 = 0x800A3DA0` (`0x800A0000` for an unknown tag), `v0` = the
/// old first word, and the changes' temporaries as [`ModeChange`] says.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800356BC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x5A42_5A42); // "ZBZB"
    sw(m, g[SP], 4, g[A1]);
    let switch = if g[A0] == g[AT] {
        g[AT] = 1;
        0
    } else {
        g[AT] = li(0x4141_454E); // "AAEN"
        let aaen = g[A0] == g[AT];
        g[AT] = 1;
        if !aaen {
            g[AT] = li(0x4675_6C6C); // "Full"
            g[V1] = li(0x800A_0000);
            if g[A0] == g[AT] {
                g[V1] = li(0x800A_3DA0);
                sw(m, g[V1], 0, g[A2]);
                g[AT] = li(0x800A_0000);
                sw(m, g[AT], 0x3DA4, g[A2]);
            }
            return;
        }
        1
    };
    let on = usize::from(g[A2] == g[AT]);
    g[V1] = li(0x800A_3DA0);
    g[V0] = lw(m, g[V1], 0);
    for c in &MODE_CHANGES[switch][on] {
        let (a, b) = c.regs;
        g[AT] = li(c.from);
        g[a] = li(c.to.0 & 0xFFFF_0000);
        if g[V0] == g[AT] {
            g[a] |= u64::from(c.to.0 & 0xFFFF);
            g[b] = li(c.to.1 & 0xFFFF_0000);
            sw(m, g[V1], 0, g[a]);
            g[b] |= u64::from(c.to.1 & 0xFFFF);
            g[AT] = li(0x800A_0000);
            sw(m, g[AT], 0x3DA4, g[b]);
            return;
        }
    }
}

/// `func_800358A0(buttons)`: adjust an input bit mask by the settings word
/// `[0x800D697C]` and two flags (**guess**: controller options). With
/// setting bit 6, clear bit 16; with bit 5, clear bit 17. Then if
/// `[0x800A4744] == 0`, clear bits 9 and 10; otherwise, if `[0x800A3D60] !=
/// 0` and exactly one of bits 9 and 10 is set, swap them. Returns the
/// mask (bits above 31 kept).
///
/// Leaves `a0` = the mask after the first two steps (or after the swap
/// from 10 to 9), `t0`/`t3` = the flags, `t1 = a0 & !0x400`, `t6`/`t8` =
/// the setting bits, and `at`, `t4`/`t5`/`t7`/`t8` as the path left them.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800358A0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, li(0x800D_0000), 0x697C);
    g[AT] = li(0xFFFE_0000);
    g[T0] = li(0x800A_0000);
    g[T6] = g[V0] & 0x40;
    g[T8] = g[V0] & 0x20;
    if g[T6] != 0 {
        g[AT] |= 0xFFFF;
        g[T7] = g[A0] & g[AT];
        g[A0] = g[T7];
    }
    g[AT] = li(0xFFFD_0000);
    if g[T8] != 0 {
        g[AT] |= 0xFFFF;
        g[T9] = g[A0] & g[AT];
        g[A0] = g[T9];
    }
    g[T0] = lw(m, g[T0], 0x4744);
    g[AT] = (-0x401i64) as u64;
    g[T1] = g[A0] & g[AT];
    g[T3] = li(0x800A_0000);
    if g[T0] == 0 {
        g[AT] = (-0x201i64) as u64;
        g[V0] = g[T1] & g[AT];
        return;
    }
    g[T3] = lw(m, g[T3], 0x3D60);
    g[V0] = g[A0] & 0x200;
    if g[T3] != 0 {
        g[T7] = g[A0] & 0x400;
        if g[V0] == 0 {
            if g[T7] != 0 {
                // Bit 10 only: move it to bit 9.
                g[AT] = (-0x401i64) as u64;
                g[T8] = g[A0] & g[AT];
                g[A0] = g[T8] | 0x200;
            }
        } else {
            g[T4] = g[A0] & 0x400;
            g[T5] = g[A0] | 0x400;
            if g[T4] == 0 {
                // Bit 9 only: move it to bit 10.
                g[AT] = (-0x201i64) as u64;
                g[V0] = g[T5] & g[AT];
                return;
            }
        }
    }
    g[V0] = g[A0];
}

/// `func_80036094(a0)`: an empty function that spills `a0` to its slot
/// `[sp]`. Domain: canonical `sp` with `[sp]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80036094(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    sw(&mut mem, ctx.gpr[SP], 0, ctx.gpr[A0]);
}

/// `func_80036F7C()`: `[0x800A3D30] = 0x800DB930` and `[0x800A3D38] = 0`.
/// Leaves `t6 = 0x800DB930`, `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80036F7C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = li(0x800D_B930);
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x3D30, g[T6]);
    sw(m, g[AT], 0x3D38, 0);
}

/// `func_80037BF0(a0)`: an empty function that spills `a0` to its slot
/// `[sp]`. Domain: canonical `sp` with `[sp]` in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80037BF0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    sw(&mut mem, ctx.gpr[SP], 0, ctx.gpr[A0]);
}

/// `func_80038DBC(off)`: bit 6 of the settings word `[0x800D697C]` (the
/// one [`func_800358A0`] reads): set if `off == 0` (full 64-bit test),
/// cleared otherwise. Leaves `v0 = 0x800D6960`, the old word in `t6` or
/// `t8`, the new one in `t7` or `t9`, and `at = !0x40` when clearing.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038DBC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x800D_6960);
    if g[A0] == 0 {
        g[T8] = lw(m, g[V0], 0x1C);
        g[T9] = g[T8] | 0x40;
        sw(m, g[V0], 0x1C, g[T9]);
    } else {
        g[T6] = lw(m, g[V0], 0x1C);
        g[AT] = (-0x41i64) as u64;
        g[T7] = g[T6] & g[AT];
        sw(m, g[V0], 0x1C, g[T7]);
    }
}

/// `func_80038DF8(a, b, c, d, e, f)`: store each argument that isn't
/// negative (signed 64-bit test; `e`, `f` from the stack slots `[sp +
/// 0x10]`, `[sp + 0x14]`, signed words) as a halfword: `a` at `0x800A3D4C`,
/// `b` at `0x800A3D50`, and `c`..`f` at `0x800A3D44`..`0x800A3D4A`.
/// Leaves `at = 0x800A0000`, `v1 = 0x800A3D44`, `v0 = f`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038DF8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    if (g[A0] as i64) >= 0 {
        sh(m, g[AT], 0x3D4C, g[A0]);
    }
    if (g[A1] as i64) >= 0 {
        sh(m, g[AT], 0x3D50, g[A1]);
    }
    g[V1] = li(0x800A_3D44);
    if (g[A2] as i64) >= 0 {
        sh(m, g[V1], 0, g[A2]);
    }
    if (g[A3] as i64) >= 0 {
        sh(m, g[V1], 2, g[A3]);
    }
    g[V0] = lw(m, g[SP], 0x10);
    if (g[V0] as i64) >= 0 {
        sh(m, g[V1], 4, g[V0]);
    }
    g[V0] = lw(m, g[SP], 0x14);
    if (g[V0] as i64) >= 0 {
        sh(m, g[V1], 6, g[V0]);
    }
}

/// Write one light at `[base]` as F3DEX2's `Light` wants it: the colour
/// from the low bytes of the three halfwords at `rgb` (bytes 1, 3, 5) into
/// `col` (`+0..+2`) and `colc` (`+4..+6`), through `v0`. `dir`, if given,
/// is three halfwords, negated and stored as bytes at `+0x10..+0x12`
/// through the temporary pairs `temps` (load, negation).
fn write_light(m: &mut Mem, g: &mut [u64; 32], base: usize, offs: [i32; 3], rgb: usize) {
    for (k, off) in offs.into_iter().enumerate() {
        g[V0] = lbu(m, g[rgb], 1 + 2 * k as i32);
        sb(m, g[base], off, g[V0]);
        sb(m, g[base], off + 4, g[V0]);
    }
}

fn write_direction(m: &mut Mem, g: &mut [u64; 32], base: usize, at: i32, dir: usize, temps: [(usize, usize); 3]) {
    for (k, (t, n)) in temps.into_iter().enumerate() {
        g[t] = lh(m, g[dir], 2 * k as i32);
        g[n] = subu(0, g[t]);
        sb(m, g[base], at + k as i32, g[n]);
    }
}

/// `func_80038E58(ambient, diffuse, dir)`: fill the `Lights1` at
/// `0x800A3DB0`: the ambient colour (`+0`, `+4`) and the light colour (`+8`,
/// `+0xC`) from the low bytes of each argument's three halfwords
/// ([`write_light`]), and the direction (`+0x10`) as the negated low bytes
/// of `dir`'s halfwords. Leaves `v1 = 0x800A3DB0`, `v0` = the last colour
/// byte, `t6`..`t1` = the direction halfwords and their negations.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038E58(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = li(0x800A_3DB0);
    write_light(m, g, V1, [0, 1, 2], A0);
    write_light(m, g, V1, [8, 9, 0xA], A1);
    write_direction(m, g, V1, 0x10, A2, [(T6, T7), (T8, T9), (T0, T1)]);
}

/// The twelve 0x28-byte `Lights2` slots at `0x800A3DC8` (ambient, two
/// lights) that [`func_80038ED0`], [`func_80038F68`] and [`func_80038FE8`]
/// fill, with a word per slot at `0x800A3FA8`: 1 for one light, 2 for two.
pub const LIGHT_SLOTS: u32 = 0x800A_3DC8;

/// The slot's address as the code computes it: `((i << 2) + i) << 3`.
fn light_slot(g: &mut [u64; 32], t: usize, base: usize, dst: usize) {
    g[t] = addu(g[t], g[A0]);
    g[base] = li(LIGHT_SLOTS);
    g[t] = sll(g[t], 3);
    g[dst] = addu(g[t], g[base]);
}

/// `func_80038ED0(i, ambient, diffuse, dir)`: for `0 <= i < 12` (signed),
/// fill light slot `i`'s ambient and first light like [`func_80038E58`];
/// anything else does nothing. Leaves `at` = the bound test, `t6` =
/// `4 * i` (then `40 * i`), `t7 = LIGHT_SLOTS`, `v1` = the slot, `v0`,
/// `t8`..`t3` as the writes left them.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038ED0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0xC);
    if (g[A0] as i64) < 0 {
        return;
    }
    g[T6] = sll(g[A0], 2);
    if g[AT] == 0 {
        return;
    }
    light_slot(g, T6, T7, V1);
    write_light(m, g, V1, [0, 1, 2], A1);
    write_light(m, g, V1, [8, 9, 0xA], A2);
    write_direction(m, g, V1, 0x10, A3, [(T8, T9), (T0, T1), (T2, T3)]);
}

/// `func_80038F68(i)`: for `0 <= i < 12`, copy the `Lights1` at `0x800A3DB0`
/// (24 bytes) into light slot `i` and set its word at `0x800A3FA8` to 1 (one
/// light). Leaves the copy's words in `at`, `t0`, `t3`, `t8 = 0x800A3DB0`,
/// `t1 = 0x800A3DB8`, `v0` = the slot, `t4 = 1`, `t5 = 4 * i`, `at =
/// 0x800A0000 + 4 * i`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038F68(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0xC);
    if (g[A0] as i64) < 0 {
        return;
    }
    g[T6] = sll(g[A0], 2);
    if g[AT] == 0 {
        return;
    }
    g[T8] = li(0x800A_3DB0);
    g[AT] = lw(m, g[T8], 0);
    light_slot(g, T6, T7, V0);
    sw(m, g[V0], 0, g[AT]);
    g[T0] = lw(m, g[T8], 4);
    g[T1] = li(0x800A_3DB8);
    sw(m, g[V0], 4, g[T0]);
    g[AT] = lw(m, g[T1], 0);
    g[T5] = sll(g[A0], 2);
    g[T4] = 1;
    sw(m, g[V0], 8, g[AT]);
    g[T3] = lw(m, g[T1], 4);
    sw(m, g[V0], 0xC, g[T3]);
    g[AT] = lw(m, g[T1], 8);
    sw(m, g[V0], 0x10, g[AT]);
    g[T3] = lw(m, g[T1], 0xC);
    g[AT] = addu(li(0x800A_0000), g[T5]);
    sw(m, g[V0], 0x14, g[T3]);
    sw(m, g[AT], 0x3FA8, g[T4]);
}

/// `func_80038FE8(i, on, colour, dir)`: for `0 <= i < 12`, light slot `i`'s
/// second light. With `on == 0` (64-bit) the slot's word at `0x800A3FA8`
/// becomes 1 (one light); otherwise 2, and the second light (`+0x18`) gets
/// `colour` and the negated `dir` like [`func_80038E58`].
///
/// QUIRK: the red byte goes to `+0x19`, where green overwrites it, and
/// `+0x18` is never written (`colc` at `+0x1C` gets all three right).
/// Leaves `at` = the bound test or `0x800A0000 + 4 * i`, `t6 = 1` or `t8 =
/// 2`, and with a light `v1` = the slot, `v0`, `t0`..`t7` as the writes
/// left them.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038FE8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0xC);
    if (g[A0] as i64) < 0 || g[AT] == 0 {
        return;
    }
    g[T8] = 2;
    if g[A1] == 0 {
        g[T7] = sll(g[A0], 2);
        g[AT] = addu(li(0x800A_0000), g[T7]);
        g[T6] = 1;
        sw(m, g[AT], 0x3FA8, g[T6]);
        return;
    }
    g[T9] = sll(g[A0], 2);
    g[AT] = addu(li(0x800A_0000), g[T9]);
    sw(m, g[AT], 0x3FA8, g[T8]);
    g[T0] = sll(g[A0], 2);
    g[V0] = lbu(m, g[A2], 1);
    light_slot(g, T0, T1, V1);
    sb(m, g[V1], 0x19, g[V0]); // QUIRK: +0x18 in a correct light
    sb(m, g[V1], 0x1C, g[V0]);
    g[V0] = lbu(m, g[A2], 3);
    sb(m, g[V1], 0x19, g[V0]);
    sb(m, g[V1], 0x1D, g[V0]);
    g[V0] = lbu(m, g[A2], 5);
    sb(m, g[V1], 0x1A, g[V0]);
    sb(m, g[V1], 0x1E, g[V0]);
    write_direction(m, g, V1, 0x20, A3, [(T2, T3), (T4, T5), (T6, T7)]);
}

/// `func_80039090(a0, a1, a2, a3)`: an empty function that spills all four
/// arguments to their slots `[sp]..[sp + 0xC]`. Domain: canonical `sp` with
/// its slots in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80039090(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    for (k, r) in [A0, A1, A2, A3].into_iter().enumerate() {
        sw(&mut mem, g[SP], 4 * k as i32, g[r]);
    }
}

/// `func_800390A4`: empty (`jr ra`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800390A4(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_800390AC`: empty (`jr ra`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800390AC(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_800390C0()` = `crc32_table_init`: fill the 256 words at
/// `0x80114070` with the table of the MSB-first CRC-32 (polynomial
/// `0x04C11DB7`): entry `v` is `v << 24` shifted left eight times, XORing
/// the polynomial after each shift out of bit 31. The compiler unrolled the
/// eight steps into two passes of four, each step with its own temporaries.
///
/// Leaves, from the last entry (255): `v0 = 0x100 = t0`, `a1 = 0x80114470`,
/// `a2 = 0x80000000`, `a3` = the polynomial, `a0 = 0`, `v1` = the entry,
/// and the steps' temporaries `t1`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800390C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A1] = li(0x8011_4070);
    g[A3] = li(0x04C1_1DB7);
    g[V0] = 0;
    g[T0] = 0x100;
    g[A2] = li(0x8000_0000);
    loop {
        g[V1] = sll(g[V0], 24);
        g[A0] = 8;
        g[T6] = g[V1] & g[A2];
        loop {
            g[A0] = addu(g[A0], (-4i64) as u64);
            // Step 1: test t6, shift in t7 (bit set) or t8.
            if g[T6] == 0 {
                g[T8] = sll(g[V1], 1);
                g[V1] = g[T8];
            } else {
                g[T7] = sll(g[V1], 1);
                g[V1] = g[T7] ^ g[A3];
            }
            // Steps 2-4: test, the plain shift (always made), the XOR shift.
            for (test, plain, xor) in [(T9, T2, T1), (T3, T5, T4), (T6, T8, T7)] {
                g[test] = g[V1] & g[A2];
                g[plain] = sll(g[V1], 1);
                if g[test] == 0 {
                    g[V1] = g[plain];
                } else {
                    g[xor] = sll(g[V1], 1);
                    g[V1] = g[xor] ^ g[A3];
                }
            }
            if g[A0] == 0 {
                break;
            }
            g[T6] = g[V1] & g[A2];
        }
        g[V0] = addu(g[V0], 1);
        g[A1] = addu(g[A1], 4);
        sw(m, g[A1], -4, g[V1]);
        if g[V0] == g[T0] {
            break;
        }
    }
}

/// Copy `words` words (a multiple of 3, plus `tail` = 0 or 2 more) from
/// `[src]` to `[dst]`, three per iteration as the compiler unrolled it,
/// through `at` (the tail's second word through `tail_reg`). Leaves `src`
/// and `dst` advanced past the triples, `end` = the end of the triples.
fn copy_words(m: &mut Mem, g: &mut [u64; 32], (src, dst, end): (usize, usize, usize), tail: Option<usize>) {
    loop {
        g[AT] = lw(m, g[src], 0);
        g[src] = addu(g[src], 0xC);
        g[dst] = addu(g[dst], 0xC);
        sw(m, g[dst], -0xC, g[AT]);
        g[AT] = lw(m, g[src], -8);
        sw(m, g[dst], -8, g[AT]);
        g[AT] = lw(m, g[src], -4);
        sw(m, g[dst], -4, g[AT]);
        if g[src] == g[end] {
            break;
        }
    }
    if let Some(t) = tail {
        g[AT] = lw(m, g[src], 0);
        sw(m, g[dst], 0, g[AT]);
        g[t] = lw(m, g[src], 4);
        sw(m, g[dst], 4, g[t]);
    }
}

/// `func_8003960C()`: copy the 0x3F0-byte block at `0x80113680` (the save
/// data, **guess**; its flags at `+8` are read by [`func_8001F464`] and
/// [`func_8002DC7C`]) to `0x80113A70`. Leaves `t7 = t0 = 0x80113A70`, `t6
/// = 0x80113E60`, `at` = the last word.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003960C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T7] = li(0x8011_3680);
    g[T6] = li(0x8011_3A70);
    g[T0] = addu(g[T7], 0x3F0);
    copy_words(&mut mem, g, (T7, T6, T0), None);
}

/// `func_80039914(a, b)`: copy the 0x2C-byte record `b` of the save data
/// (`0x80113694 + 0x2C * b`) to record `a` at `0x80113E60` (the current
/// profiles, **guess**). The multiplies keep only the low 32 bits.
/// Leaves `v0 = 0x2C`, `t6`/`t9` = the products, `t1 = 0x80113680`, `t7 =
/// 0x80113E60`, `t0 = 0x2C * b + 0x14`, `t2`/`t8` past the triples, `t5` =
/// the last word, `at` the one before.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80039914(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[V0] = 0x2C;
    g[T1] = li(0x8011_3680);
    g[T7] = li(0x8011_3E60);
    g[T6] = multu(g[A0], g[V0]).0;
    g[T8] = addu(g[T6], g[T7]);
    g[T9] = multu(g[A1], g[V0]).0;
    g[T0] = addu(g[T9], 0x14);
    g[T2] = addu(g[T0], g[T1]);
    g[T5] = addu(g[T2], 0x24);
    copy_words(&mut mem, g, (T2, T8, T5), Some(T5));
}

/// `func_80039984(a, b)`: the reverse of [`func_80039914`], record `b` at
/// `0x80113E60` to record `a` of the save data at `0x80113694`. Leaves `v0 =
/// 0x2C`, `t6`/`t0` = the products, `t1 = 0x80113E60`, `t8 =
/// 0x80113680`, `t7 = 0x2C * a + 0x14`, `t2`/`t9` past the triples, `t5` =
/// the last word, `at` the one before.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80039984(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[V0] = 0x2C;
    g[T1] = li(0x8011_3E60);
    g[T8] = li(0x8011_3680);
    g[T6] = multu(g[A0], g[V0]).0;
    g[T7] = addu(g[T6], 0x14);
    g[T9] = addu(g[T7], g[T8]);
    g[T0] = multu(g[A1], g[V0]).0;
    g[T2] = addu(g[T0], g[T1]);
    g[T5] = addu(g[T2], 0x24);
    copy_words(&mut mem, g, (T2, T9, T5), Some(T5));
}

/// `func_80039A30()` = `framebuffers_init`: the three framebuffer addresses
/// at `0x80114530..0x80114538` (NOTES.md, "Asset heap"): with `osMemSize`
/// (`[0x80000318]`) at least 8 MB (unsigned, of the sign-extended word) the
/// width is 640, 4 bytes a pixel and an extra `x = 2 * 4 * 640`, otherwise
/// 320, 2 bytes and `x = 0`; `fb[k - 1] = (osMemSize | 0x80000000) - k *
/// (w * 240 * bpp + x) + x` for `k = 1..3`. Each is stored without the
/// final `+ x`, read back and stored again with it.
///
/// The compiler repeats the size test before every choice, so `at` always
/// ends as it. Leaves `v0 = t3 = 3`, `v1` = osMemSize, `t1 = 0x8011453C`,
/// `t2` = the end of RDRAM, `t4 = 0x800000`, `t5 = 0xF0`, `a0`/`a1` = bpp
/// and width, `a2`/`a3` = 640 and 4 with 8 MB, `t0` = `x / 2`, and the
/// arithmetic in `t6`..`t9`. `s0` is saved and restored (sign-extended).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80039A30(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = lw(m, li(0x8000_0000), 0x318);
    g[SP] = addu(g[SP], (-8i64) as u64);
    sw(m, g[SP], 4, g[S0]);
    g[AT] = li(0x8000_0000);
    g[T1] = li(0x8011_4530);
    g[S0] = 3;
    g[V0] = 0;
    g[T5] = 0xF0;
    g[T4] = li(0x80_0000);
    g[T2] = g[V1] | g[AT];
    g[AT] = sltu(g[V1], g[T4]);
    let big = g[AT] == 0;
    // x / 2 = 4 * 640 (8 MB only), through a3 and a2.
    let half_extra = |g: &mut [u64; 32]| {
        g[A2] = 0x280;
        g[A3] = 4;
        g[T0] = multu(g[A3], g[A2]).0;
    };
    loop {
        g[A0] = if big { 4 } else { 2 };
        g[A1] = if big { 0x280 } else { 0x140 };
        g[T0] = 0;
        if big {
            half_extra(g);
        }
        g[T7] = multu(g[A1], g[T5]).0;
        g[T6] = sll(g[T0], 1);
        g[T3] = addu(g[V0], 1);
        g[V0] = g[T3];
        g[T0] = 0;
        g[T8] = multu(g[T7], g[A0]).0;
        g[T9] = addu(g[T6], g[T8]);
        g[T7] = multu(g[T9], g[T3]).0;
        g[T6] = subu(g[T2], g[T7]);
        sw(m, g[T1], 0, g[T6]);
        if big {
            half_extra(g);
        }
        g[T8] = lw(m, g[T1], 0);
        g[T9] = sll(g[T0], 1);
        g[T1] = addu(g[T1], 4);
        g[T7] = addu(g[T8], g[T9]);
        sw(m, g[T1], -4, g[T7]);
        if g[T3] == g[S0] {
            break;
        }
    }
    g[S0] = lw(m, g[SP], 4);
    g[SP] = addu(g[SP], 8);
}

/// `func_80039CD8(restore)`: save or restore three framebuffer-related
/// words (**guess**) in the block at `0x80114488`. `restore == 0` (64-bit)
/// saves: `+0x1C = [0x8011453C]`, `+0x2C = [0x80114504]`, `+0x40 =
/// [0x80114518]`. Otherwise, if `+0x1C` is nonzero, `[0x8011453C] = +0x1C`
/// and `+0x1C = 0`. Leaves `v0` = the saved word (restoring) or
/// `0x801144D8`, `v1 = 0x80114488`, `at = 0x80110000` (restoring), and
/// `t6`..`t8` the words saved.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80039CD8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    if g[A0] == 0 {
        g[V0] = li(0x8011_44D8);
        g[T6] = lw(m, li(0x8011_0000), 0x453C);
        g[T7] = lw(m, g[V0], 0x2C);
        g[T8] = lw(m, g[V0], 0x40);
        g[V1] = li(0x8011_4488);
        sw(m, g[V1], 0x1C, g[T6]);
        sw(m, g[V1], 0x2C, g[T7]);
        sw(m, g[V1], 0x40, g[T8]);
    } else {
        g[V1] = li(0x8011_4488);
        g[V0] = lw(m, g[V1], 0x1C);
        g[AT] = li(0x8011_0000);
        if g[V0] != 0 {
            sw(m, g[AT], 0x453C, g[V0]);
            sw(m, g[V1], 0x1C, 0);
        }
    }
}

/// `84 * i` (0x54, a spline point) as the code computes it into `t`: `((i
/// << 2) + i) << 2`, `+ i`, `<< 2`.
fn point_offset(g: &mut [u64; 32], i: usize, t: usize) {
    g[t] = sll(g[i], 2);
    g[t] = addu(g[t], g[i]);
    g[t] = sll(g[t], 2);
    g[t] = addu(g[t], g[i]);
    g[t] = sll(g[t], 2);
}

/// `func_8003A4E8(spline, i)`: the first of point `i`'s ten halfwords at
/// `+0x40` (usually the point's own index; NOTES.md, "Splines"), signed.
/// Leaves `t6` = the points, `t7 = 84 * i`, `t8` = the point.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003A4E8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    point_offset(g, A1, T7);
    g[T6] = lw(&mem, g[A0], 0xC);
    g[T8] = addu(g[T6], g[T7]);
    g[V0] = lh(&mem, g[T8], 0x40);
}

/// `func_8003A50C(spline, id, from)`: the first point `i` in `from..count`
/// (signed; `count = [spline + 4]`) whose halfword `+0x40` equals `id`
/// (full 64-bit compare with the sign-extended halfword), else -1. Leaves
/// `v1` = the last index tried, `a2` = its point (or `from` if none was
/// tried), `t6` = the points, `t7 = 84 * from` (`5 * from` if none), `t8`
/// = the last halfword, `at` = the last bound test.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003A50C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, g[A0], 4);
    g[T7] = sll(g[A2], 2);
    g[T7] = addu(g[T7], g[A2]);
    g[AT] = slt(g[A2], g[V0]);
    g[V1] = g[A2];
    if g[AT] != 0 {
        g[T6] = lw(m, g[A0], 0xC);
        g[T7] = sll(g[T7], 2);
        g[T7] = addu(g[T7], g[A2]);
        g[T7] = sll(g[T7], 2);
        g[A2] = addu(g[T6], g[T7]);
        loop {
            g[T8] = lh(m, g[A2], 0x40);
            if g[A1] == g[T8] {
                g[V0] = g[V1];
                return;
            }
            g[V1] = addu(g[V1], 1);
            g[AT] = slt(g[V1], g[V0]);
            g[A2] = addu(g[A2], 0x54);
            if g[AT] == 0 {
                break;
            }
        }
    }
    g[V0] = u64::MAX;
}

/// `func_8003A568(w, k)`: for the spline walker `w` ([`func_8003ABA0`]):
/// with no path bits (`+0x2C == 0`), the point index `w[+0x10 + 4k]`;
/// otherwise the halfword `+0x42 + 2 * j` of that point (of spline `[w]`),
/// where `j` = the path bits shifted right (arithmetic) by `k` (low 5
/// bits), or the bits themselves for `k == 0`.
///
/// QUIRK: `j` is the whole shifted bit field, not one bit, so it can reach
/// far past the point's ten halfwords. Domain: that halfword in RDRAM.
/// Leaves `t6 = t8 = 4k`, `t9 = w + 4k`, and on the bits path `v1` = the
/// point index, `a2 = j`, `t0` = the spline, `t1` = its points, `t2 = 84 *
/// index`, `t3` = the point, `t4 = 2j`, `t5` = the address; `t7 = t9`
/// otherwise.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003A568(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, g[A0], 0x2C);
    g[T8] = sll(g[A1], 2);
    g[T6] = sll(g[A1], 2);
    g[T9] = addu(g[A0], g[T8]);
    if g[V0] == 0 {
        g[T7] = addu(g[A0], g[T6]);
        g[V0] = lw(m, g[T7], 0x10);
        return;
    }
    g[V1] = lw(m, g[T9], 0x10);
    g[A2] = if g[A1] != 0 { srav(g[V0], g[A1]) } else { g[V0] };
    g[T0] = lw(m, g[A0], 0);
    g[T1] = lw(m, g[T0], 0xC);
    point_offset(g, V1, T2);
    g[T4] = sll(g[A2], 1);
    g[T3] = addu(g[T1], g[T2]);
    g[T5] = addu(g[T3], g[T4]);
    g[V0] = lh(m, g[T5], 0x42);
}

/// `func_8003ABA0(spline, dir, w)`: step the spline walker `w` one point,
/// forward if `(s16) dir == 1`, otherwise backward. `spline` is a loaded
/// spline header (`+0` the flag halfword F, `+0xC` the points; NOTES.md,
/// "Splines"). The walker holds point indices `+0x10` (current), `+0x14`,
/// `+0x18`, `+0x1C`, end flags `+0x20` (forward) and `+0x24` (backward),
/// a choice `+0x28` for forks, path bits `+0x2C`, and a float `+8`.
///
/// Forward: clear `+0x24`. Unless `+0x20` is already set, take point `p =
/// +0x14` (F != 0) or `+0x1C` (F == 0). With no successors: `+0x20 = 1`,
/// `+8 = 1.0`, the result -1. Otherwise successor `k = choice` if `choice
/// < count`, else `choice % count` (signed, truncated to s16 either way),
/// and the path bits become `(bits >> 1) | k` (F != 0) or `(bits >> 1) | (k
/// << 2)` (arithmetic shift). Then, if `+0x20` is (still) clear:
/// `+0x10 = +0x14`, and with F != 0 `+0x14` = the successor; with F == 0
/// `+0x14, +0x18, +0x1C = +0x18, +0x1C, successor`.
///
/// Backward: clear `+0x20`. Unless `+0x24` is already set, take point `p =
/// +0x10`. With no predecessors: `+0x24 = 1`, `+8 = 0.0`. Otherwise the
/// predecessor `choice < count ? pred[choice] : pred[choice % count]` (not
/// truncated), the path bits become `(bits << 1) & 1` (F != 0; QUIRK: that
/// is always 0) or `(bits << 1) & 7`, and gain bit 0 unless the
/// predecessor's first successor is the current point. Then, if `+0x24` is
/// clear: with F == 0, `+0x1C, +0x18 = +0x18, +0x14`; and `+0x14 = +0x10`,
/// `+0x10` = the predecessor.
///
/// `dir` is spilled to its slot `[sp + 4]`, and the chosen point (or -1)
/// is kept in the frame at `[sp - 8]`. QUIRK: when the end flag was already
/// set, `a1` is loaded from that frame word without it being written, an
/// uninitialised stack read (it only reaches `a1`). Point indices go
/// through `multu` by 0x54, so only their low 32 bits count.
///
/// Domain: a nonzero count before each `%` (so neither IDO divide check,
/// `do_break`, is reached: `choice % count` runs only when `choice >=
/// count`, which excludes `count == -1` with `choice = INT_MIN`), and the
/// points in RDRAM. Leaves `a3` = 1 or the choice, `v0`, `v1` = the point,
/// `t0 = 0x54` or `k`, `at`, the loads in `t1`..`t9`, and `f4 = 1.0` or `f6
/// = 0.0` at an end.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003ABA0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-8i64) as u64);
    g[T6] = sll(g[A1], 16);
    g[T7] = sra(g[T6], 16);
    sw(m, g[SP], 0xC, g[A1]);
    g[A3] = 1;
    g[A1] = g[T7];
    if g[T7] == g[A3] {
        // Forward.
        g[V0] = lw(m, g[A2], 0x20);
        sw(m, g[A2], 0x24, 0);
        if g[V0] == 0 {
            g[V0] = lh(m, g[A0], 0);
            g[AT] = li(0x3F80_0000); // 1.0
            let (p, pt, points) = if g[V0] != 0 { (T2, T3, T4) } else { (T8, T9, T1) };
            g[p] = lw(m, g[A2], if g[V0] != 0 { 0x14 } else { 0x1C });
            g[T0] = 0x54;
            g[points] = lw(m, g[A0], 0xC);
            g[pt] = multu(g[p], g[T0]).0;
            g[V1] = addu(g[pt], g[points]);
            g[A1] = lh(m, g[V1], 0);
            if g[A1] == 0 {
                ctx.fpr[4].set_u32l(g[AT] as u32);
                g[A1] = u64::MAX;
                sw(m, g[A2], 0x20, g[A3]);
                sw(m, g[A2], 8, u64::from(ctx.fpr[4].u32l()));
                sw(m, g[SP], 0, g[A1]);
                g[V0] = g[A3];
            } else {
                g[A3] = lw(m, g[A2], 0x28);
                g[AT] = slt(g[A3], g[A1]);
                if g[AT] == 0 {
                    g[T0] = div(g[A3], g[A1]).1;
                    g[T6] = sll(g[T0], 16);
                    g[T7] = sra(g[T6], 16);
                    g[T0] = g[T7];
                    if g[A1] == 0 {
                        imports::runtime::do_break(0x8003_AC7C);
                    }
                    g[AT] = u64::MAX;
                    let minus_one = g[A1] == g[AT];
                    g[AT] = li(0x8000_0000);
                    if minus_one && g[A3] == g[AT] {
                        imports::runtime::do_break(0x8003_AC94);
                    }
                } else {
                    g[T0] = sll(g[A3], 16);
                    g[T5] = sra(g[T0], 16);
                    g[T0] = g[T5];
                }
                g[T8] = sll(g[T0], 1);
                g[T9] = addu(g[V1], g[T8]);
                g[A1] = lh(m, g[T9], 4);
                if g[V0] != 0 {
                    g[T5] = lw(m, g[A2], 0x2C);
                    g[V0] = lw(m, g[A2], 0x20);
                    g[T6] = sra(g[T5], 1);
                    g[T7] = g[T6] | g[T0];
                    sw(m, g[A2], 0x2C, g[T7]);
                } else {
                    g[T1] = lw(m, g[A2], 0x2C);
                    g[T3] = sll(g[T0], 2);
                    g[V0] = lw(m, g[A2], 0x20);
                    g[T2] = sra(g[T1], 1);
                    g[T4] = g[T2] | g[T3];
                    sw(m, g[A2], 0x2C, g[T4]);
                }
                sw(m, g[SP], 0, g[A1]);
            }
        }
        g[A1] = lw(m, g[SP], 0); // QUIRK: never written if the end flag was set
        if g[V0] == 0 {
            g[T8] = lw(m, g[A2], 0x14);
            sw(m, g[A2], 0x10, g[T8]);
            g[T9] = lh(m, g[A0], 0);
            if g[T9] != 0 {
                sw(m, g[A2], 0x14, g[A1]);
            } else {
                g[T1] = lw(m, g[A2], 0x18);
                g[T2] = lw(m, g[A2], 0x1C);
                sw(m, g[A2], 0x1C, g[A1]);
                sw(m, g[A2], 0x14, g[T1]);
                sw(m, g[A2], 0x18, g[T2]);
            }
        }
    } else {
        // Backward.
        g[T3] = lw(m, g[A2], 0x24);
        sw(m, g[A2], 0x20, 0);
        if g[T3] == 0 {
            g[T4] = lw(m, g[A2], 0x10);
            g[T0] = 0x54;
            g[T6] = lw(m, g[A0], 0xC);
            g[T5] = multu(g[T4], g[T0]).0;
            g[V1] = addu(g[T5], g[T6]);
            g[V0] = lh(m, g[V1], 2);
            if g[V0] == 0 {
                ctx.fpr[6].set_u32l(0);
                g[A1] = u64::MAX;
                sw(m, g[A2], 0x24, g[A3]);
                sw(m, g[A2], 8, u64::from(ctx.fpr[6].u32l()));
                sw(m, g[SP], 0, g[A1]);
            } else {
                g[A3] = lw(m, g[A2], 0x28);
                g[AT] = slt(g[A3], g[V0]);
                if g[AT] == 0 {
                    g[T9] = div(g[A3], g[V0]).1;
                    g[T1] = sll(g[T9], 1);
                    g[T2] = addu(g[V1], g[T1]);
                    g[A1] = lh(m, g[T2], 8);
                    if g[V0] == 0 {
                        imports::runtime::do_break(0x8003_ADA4);
                    }
                    g[AT] = u64::MAX;
                    let minus_one = g[V0] == g[AT];
                    g[AT] = li(0x8000_0000);
                    if minus_one && g[A3] == g[AT] {
                        imports::runtime::do_break(0x8003_ADBC);
                    }
                } else {
                    g[T7] = sll(g[A3], 1);
                    g[T8] = addu(g[V1], g[T7]);
                    g[A1] = lh(m, g[T8], 8);
                }
                g[T3] = lh(m, g[A0], 0);
                if g[T3] != 0 {
                    g[T7] = lw(m, g[A2], 0x2C);
                    g[T8] = sll(g[T7], 1);
                    g[T9] = g[T8] & 1; // QUIRK: always 0
                    sw(m, g[A2], 0x2C, g[T9]);
                } else {
                    g[T4] = lw(m, g[A2], 0x2C);
                    g[T5] = sll(g[T4], 1);
                    g[T6] = g[T5] & 7;
                    sw(m, g[A2], 0x2C, g[T6]);
                }
                // Bit 0 unless we came from the predecessor's first successor.
                let lo = multu(g[A1], g[T0]).0;
                sw(m, g[SP], 0, g[A1]);
                g[T2] = lw(m, g[A0], 0xC);
                g[T1] = lw(m, g[A2], 0x10);
                g[T3] = lo;
                g[T4] = addu(g[T2], g[T3]);
                g[T5] = lh(m, g[T4], 4);
                if g[T1] != g[T5] {
                    g[T6] = lw(m, g[A2], 0x2C);
                    g[T7] = g[T6] | 1;
                    sw(m, g[A2], 0x2C, g[T7]);
                    sw(m, g[SP], 0, g[A1]);
                }
            }
        }
        g[T8] = lw(m, g[A2], 0x24);
        g[A1] = lw(m, g[SP], 0); // QUIRK: never written if the end flag was set
        if g[T8] == 0 {
            g[T9] = lh(m, g[A0], 0);
            if g[T9] == 0 {
                g[T2] = lw(m, g[A2], 0x18);
                g[T3] = lw(m, g[A2], 0x14);
                sw(m, g[A2], 0x1C, g[T2]);
                sw(m, g[A2], 0x18, g[T3]);
            }
            g[T4] = lw(m, g[A2], 0x10);
            sw(m, g[A2], 0x10, g[A1]);
            sw(m, g[A2], 0x14, g[T4]);
        }
    }
    g[SP] = addu(g[SP], 8);
}

/// `func_8003B250(w, p)`: start the spline walker `w` ([`func_8003ABA0`]) at
/// point `p`: all four indices `+0x10..+0x1C = p`, then `+0x14` = `p`'s
/// first successor if it has any. With the spline flag (`[spline]`, `spline
/// = [w]`) clear, `+0x18` and `+0x1C` follow the first successors two and
/// three steps on, as far as they exist. The points are re-read from
/// `[spline + 0xC]` at each step; the multiplies keep the low 32 bits.
///
/// Leaves `a2 = 0x54`, `v0` = the spline, `v1` = the points or the second
/// point, `a1` = `p` or the third point, and the loads in `t0`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003B250(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A2] = 0x54;
    g[T6] = multu(g[A1], g[A2]).0;
    g[V0] = lw(m, g[A0], 0);
    for off in [0x10, 0x14, 0x18, 0x1C] {
        sw(m, g[A0], off, g[A1]);
    }
    g[V1] = lw(m, g[V0], 0xC);
    g[T7] = addu(g[V1], g[T6]);
    g[T8] = lh(m, g[T7], 0);
    if g[T8] == 0 {
        return;
    }
    g[T0] = multu(g[A1], g[A2]).0;
    g[T1] = addu(g[V1], g[T0]);
    g[T2] = lh(m, g[T1], 4);
    sw(m, g[A0], 0x14, g[T2]);
    g[T3] = lh(m, g[V0], 0);
    if g[T3] != 0 {
        return;
    }
    g[T6] = multu(g[T2], g[A2]).0;
    g[T4] = lw(m, g[V0], 0xC);
    g[V1] = addu(g[T4], g[T6]);
    g[T7] = lh(m, g[V1], 0);
    if g[T7] == 0 {
        return;
    }
    g[T8] = lh(m, g[V1], 4);
    g[T1] = multu(g[T8], g[A2]).0;
    sw(m, g[A0], 0x18, g[T8]);
    g[T9] = lw(m, g[V0], 0xC);
    g[A1] = addu(g[T9], g[T1]);
    g[T2] = lh(m, g[A1], 0);
    if g[T2] != 0 {
        g[T3] = lh(m, g[A1], 4);
        sw(m, g[A0], 0x1C, g[T3]);
    }
}

/// `func_8003B300(a, b, c, d)`: `[0x80114548] = a`, `[0x8011454C] = c`,
/// `[0x80114540] = b`, `[0x80114544] = d`, in that order. Leaves `at =
/// 0x80110000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003B300(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x8011_0000);
    for (off, r) in [(0x4548, A0), (0x454C, A2), (0x4540, A1), (0x4544, A3)] {
        sw(&mut mem, g[AT], off, g[r]);
    }
}

/// Append a command to the display list whose pointer is at `[a1]`: load it
/// into `v1`, store `v1 + 8` (through `next`) back, then store `words`
/// (offset, register) in the compiler's order.
fn dl_append(m: &mut Mem, g: &mut [u64; 32], next: usize, words: [(i32, usize); 2]) {
    g[V1] = lw(m, g[A1], 0);
    g[next] = addu(g[V1], 8);
    sw(m, g[A1], 0, g[next]);
    for (off, r) in words {
        sw(m, g[V1], off, g[r]);
    }
}

/// `func_8003D370()`: append render state to the display list at
/// `[0x801217B0]`: `gSPTexture` (`0xD7000000`, `0x80008000`: scale 0x8000,
/// off), `gDPSetCombine` (`0xFCFFFFFF`, `0xFFFE793C`) and a
/// `G_SETOTHERMODE_L` (`0xE2001D00`, 0). Then, by the flags word
/// `[0x800A4960]`: bit 0 adds `gDPSetRenderMode` (`0xE200001C`,
/// `0x0F0A4000`), and bit 2 (the word read again) adds `0xE2001E01`, 0.
///
/// Leaves `a1 = 0x801217B0`, `v1` = the last command, `v0` = the flags,
/// `t4`/`t8` = the tested bits, and the words and pointers in `t0`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003D370(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A1] = li(0x8012_17B0);
    g[T8] = li(0x8000_8000);
    g[T7] = li(0xD700_0000);
    dl_append(m, g, T6, [(0, T7), (4, T8)]);
    g[T0] = li(0xFCFF_FFFF);
    g[T1] = li(0xFFFE_793C);
    dl_append(m, g, T9, [(0, T0), (4, T1)]);
    g[T3] = li(0xE200_1D00);
    g[V1] = lw(m, g[A1], 0);
    g[T2] = addu(g[V1], 8);
    sw(m, g[A1], 0, g[T2]);
    sw(m, g[V1], 4, 0);
    sw(m, g[V1], 0, g[T3]);
    g[V0] = lw(m, li(0x800A_0000), 0x4960);
    g[T6] = li(0xE200_001C);
    g[T4] = g[V0] & 1;
    g[T7] = li(0x0F0A_0000);
    if g[T4] != 0 {
        g[T7] |= 0x4000;
        dl_append(m, g, T5, [(4, T7), (0, T6)]);
        g[V0] = lw(m, li(0x800A_0000), 0x4960);
    }
    g[T8] = g[V0] & 4;
    if g[T8] != 0 {
        g[T0] = li(0xE200_1E01);
        g[V1] = lw(m, g[A1], 0);
        g[T9] = addu(g[V1], 8);
        sw(m, g[A1], 0, g[T9]);
        sw(m, g[V1], 4, 0);
        sw(m, g[V1], 0, g[T0]);
    }
}

/// `func_8003D488(v)`: `[0x800A48D4] = v & 0xFFFF`, spilling `v` to its
/// slot `[sp]`. Leaves `t7 = v & 0xFFFF`, `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003D488(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T7] = g[A0] & 0xFFFF;
    g[AT] = li(0x800A_0000);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[AT], 0x48D4, g[T7]);
}

/// `func_8003E1D0()`: `[0x800A4984] = [0x800A4970] = [0x800A4978] = 0`
/// (the first is [`func_8003E54C`]'s count). Leaves `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003E1D0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    for off in [0x4984, 0x4970, 0x4978] {
        sw(&mut mem, g[AT], off, 0);
    }
}

/// `func_8003E54C(b, x, y)`: append to a list of up to 190 entries: `n =
/// [0x800A4984]`; if `n < 190` (signed), the halfwords `x`, `y` at
/// `0x80118958 + 4n`, the byte `b` at `0x80118C50 + n`, and `n + 1`. Spills
/// `b` to its slot `[sp]`. Leaves `a3 = 0x800A4984`, `v0 = n`, `t6 = b &
/// 0xFF`, `t7 = 4n`, `at` = the bound test or `0x80120000 + n`, and `t8`,
/// `v1`, `t9` from the append.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003E54C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A3] = li(0x800A_4984);
    g[V0] = lw(m, g[A3], 0);
    sw(m, g[SP], 0, g[A0]);
    g[T6] = g[A0] & 0xFF;
    g[AT] = slt(g[V0], 0xBE);
    g[T7] = sll(g[V0], 2);
    if g[AT] == 0 {
        return;
    }
    g[T8] = li(0x8011_8958);
    g[V1] = addu(g[T7], g[T8]);
    sh(m, g[V1], 0, g[A1]);
    sh(m, g[V1], 2, g[A2]);
    g[AT] = addu(li(0x8012_0000), g[V0]);
    sb(m, g[AT], -0x73B0, g[T6]);
    g[T9] = addu(g[V0], 1);
    sw(m, g[A3], 0, g[T9]);
}

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

/// `func_80045634(obj, old, new)`: if all three are nonzero, replace every
/// `old` in the word list at `[obj + 0x18]` (`[obj + 0x14]` entries, signed,
/// the count re-read after each replacement) with `new`. Leaves `v0` = the
/// entries visited, `a3 = 4 * v0`, `v1` = the count, `t0` = the last entry's
/// address, `t6`/`t7` = the list and the last entry, `at = 0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80045634(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    if g[A0] == 0 || g[A1] == 0 || g[A2] == 0 {
        return;
    }
    g[V1] = lw(m, g[A0], 0x14);
    g[V0] = 0;
    g[A3] = 0;
    if (g[V1] as i64) <= 0 {
        return;
    }
    loop {
        g[T6] = lw(m, g[A0], 0x18);
        g[V0] = addu(g[V0], 1);
        g[T0] = addu(g[T6], g[A3]);
        g[T7] = lw(m, g[T0], 0);
        if g[A1] == g[T7] {
            sw(m, g[T0], 0, g[A2]);
            g[V1] = lw(m, g[A0], 0x14);
        }
        g[AT] = slt(g[V0], g[V1]);
        g[A3] = addu(g[A3], 4);
        if g[AT] == 0 {
            break;
        }
    }
}

/// The 16-word table at `0x8011A508` that [`func_80047920`] and
/// [`func_80051994`] use (players? **guess**); `[entry + 0x238]` points to
/// each one's object pointer.
pub const PLAYER_TABLE: u32 = 0x8011_A508;

/// `func_80047920()`: for each `i < [0x8011A26C]` (signed, the count re-read
/// after each live entry), the object `o = *[PLAYER_TABLE + 4i + 0x238]`, if
/// nonzero, gets the halfword `+0xE` = 0 if `i == [0x800A4BE8]` else 1, and
/// bit 2 set in `+0x10`. Leaves `v0` = the count reached, `a1` = the table
/// position, `a2 = 0x800A4BE8`, `a3 = 1`, `v1` = the count, `a0` = the last
/// object, and the loads in `t0`, `t1`, `t6`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80047920(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = lw(m, li(0x8012_0000), -0x5D94);
    g[A1] = li(PLAYER_TABLE);
    g[V0] = 0;
    if (g[V1] as i64) <= 0 {
        return;
    }
    g[A2] = li(0x800A_4BE8);
    g[A3] = 1;
    loop {
        g[T6] = lw(m, g[A1], 0x238);
        g[A0] = lw(m, g[T6], 0);
        if g[A0] != 0 {
            g[T7] = lw(m, g[A2], 0);
            g[V1] = li(0x8012_0000);
            if g[V0] != g[T7] {
                g[T0] = lw(m, g[A0], 0x10);
                sh(m, g[A0], 0xE, g[A3]);
                g[T1] = g[T0] | 4;
                sw(m, g[A0], 0x10, g[T1]);
            } else {
                g[T8] = lw(m, g[A0], 0x10);
                sh(m, g[A0], 0xE, 0);
                g[T9] = g[T8] | 4;
                sw(m, g[A0], 0x10, g[T9]);
            }
            g[V1] = lw(m, g[V1], -0x5D94);
        }
        g[V0] = addu(g[V0], 1);
        g[AT] = slt(g[V0], g[V1]);
        g[A1] = addu(g[A1], 4);
        if g[AT] == 0 {
            break;
        }
    }
}

/// `func_8004DFEC()`: for `i < 4`, `[0x800A4B94 + 4i] = -1` and
/// `[0x800A4BA4 + 4i] = 0` (see [`func_8004E488`]). Leaves `v0 = 4` (an
/// s16 count), `v1 = 12`, `a0 = 0x800A4B94`, `a2 = 0x800A4BA4`, `a1 = -1`,
/// `t6`/`t7` = the last addresses, `t8 = 4 << 16`, `at = 0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8004DFEC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A2] = li(0x800A_4BA4);
    g[A0] = li(0x800A_4B94);
    g[V0] = 0;
    g[A1] = u64::MAX;
    loop {
        g[V1] = sll(g[V0], 2);
        g[V0] = addu(g[V0], 1);
        g[T8] = sll(g[V0], 16);
        g[V0] = sra(g[T8], 16);
        g[T6] = addu(g[A0], g[V1]);
        g[AT] = slt(g[V0], 4);
        sw(m, g[T6], 0, g[A1]);
        g[T7] = addu(g[A2], g[V1]);
        sw(m, g[T7], 0, 0);
        if g[AT] == 0 {
            break;
        }
    }
}

/// `func_8004E488(i, on, mask)`: with `on == 0` (64-bit), clear `mask` in
/// `[0x800A4B94 + 4i]`. Otherwise set it there, and if none of `mask`'s
/// bits were set before, also OR `mask` into `[0x800A4BA4 + 4i]` (the bits
/// newly turned on, **guess**). Unbounded `i`. Leaves `t2 = 4i`, `v1` = the
/// word's address, and the loads and results in `a0`, `a1`, `v0`,
/// `t0`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8004E488(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T2] = sll(g[A0], 2);
    if g[A1] == 0 {
        g[T3] = li(0x800A_4B94);
        g[V1] = addu(g[T2], g[T3]);
        g[T4] = lw(m, g[V1], 0);
        g[T5] = !g[A2];
        g[T6] = g[T4] & g[T5];
        sw(m, g[V1], 0, g[T6]);
        return;
    }
    g[T6] = li(0x800A_4B94);
    g[V0] = sll(g[A0], 2);
    g[V1] = addu(g[V0], g[T6]);
    g[A1] = lw(m, g[V1], 0);
    g[T8] = li(0x800A_4BA4);
    g[T7] = g[A1] & g[A2];
    g[T1] = g[A1] | g[A2];
    if g[T7] == 0 {
        g[A0] = addu(g[V0], g[T8]);
        g[T9] = lw(m, g[A0], 0);
        g[T0] = g[T9] | g[A2];
        sw(m, g[A0], 0, g[T0]);
    }
    sw(m, g[V1], 0, g[T1]);
}

/// `func_8004F6E8(p)`: set the four words from `0x800A4B7C` to -1, then
/// word `i` to `i` for `i < n = (s8) [p + 0x70]`, in a loop the compiler
/// unrolled by four after `n & 3` single steps.
///
/// QUIRK: `n` isn't bounded by the four words, so up to 127 words from
/// `0x800A4B7C` are written. Leaves `a0 = v1 = n` (0 if `n <= 0`), `a1` =
/// past the last word written, `v0 = n`, `a2 = a3 = n & 3`, `t6`..`t9`,
/// `t0`..`t2` from the loops.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8004F6E8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = lb(m, g[A0], 0x70);
    g[A0] = li(0x800A_4B8C);
    g[A1] = li(0x800A_4B7C);
    g[V1] = u64::MAX;
    loop {
        g[A1] = addu(g[A1], 4);
        g[AT] = sltu(g[A1], g[A0]);
        sw(m, g[A1], -4, g[V1]);
        if g[AT] == 0 {
            break;
        }
    }
    g[A0] = 0;
    g[V1] = 0;
    if (g[V0] as i64) <= 0 {
        return;
    }
    g[A3] = g[V0] & 3;
    g[A2] = g[A3];
    if g[A3] != 0 {
        g[T7] = li(0x800A_4B7C);
        g[T6] = 0;
        g[A1] = addu(g[T6], g[T7]);
        loop {
            sw(m, g[A1], 0, g[V1]);
            g[V1] = addu(g[V1], 1);
            g[A0] = addu(g[A0], 1);
            g[A1] = addu(g[A1], 4);
            if g[A2] == g[V1] {
                break;
            }
        }
        if g[V1] == g[V0] {
            g[T9] = li(0x800A_0000);
            return;
        }
    }
    g[T9] = li(0x800A_4B7C);
    g[T8] = sll(g[A0], 2);
    g[A1] = addu(g[T8], g[T9]);
    loop {
        g[T0] = addu(g[V1], 1);
        g[T1] = addu(g[V1], 2);
        g[T2] = addu(g[V1], 3);
        sw(m, g[A1], 0, g[V1]);
        g[V1] = addu(g[V1], 4);
        sw(m, g[A1], 0xC, g[T2]);
        sw(m, g[A1], 8, g[T1]);
        sw(m, g[A1], 4, g[T0]);
        g[A1] = addu(g[A1], 0x10);
        if g[V1] == g[V0] {
            break;
        }
    }
}

/// `"AAII"`, the tag at `+4` of a valid 0x88-byte slot at `0x80118F90`
/// (save slots, **guess**).
const SLOT_TAG: u32 = 0x4141_4949;

/// `func_8004FE30(p)`: choose an index among `n = (s8) [p + 0x71]`: the
/// last `i < n` whose byte `(s8) [p + 0x72 + i]` equals the key `(s8)
/// [0x800A21C2 + 12 * (s8) [p + 0x5D]]`. With `[p + 0x64] != 0` that's
/// the answer, or 0 if none. With `[p + 0x64] == 0` a match also needs
/// slot `i` (`0x80118F90 + 0x88 * i`) tagged [`SLOT_TAG`]; with no such
/// match, the first tagged slot below `n`, else -1.
///
/// Leaves `v1 = v0`, `a1 = n`, `a2 = -1`, and the scans' registers (`a0`,
/// `a3`, `t0`..`t9`, `at`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8004FE30(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[T6] = lw(m, g[A0], 0x64);
    g[V1] = u64::MAX;
    // The key: (s8) [0x800A21C2 + 12 * (s8) [p + 0x5D]], through `idx`/`t`.
    let key = |g: &mut [u64; 32], idx: usize, t: usize| {
        g[idx] = lb(m, g[A0], 0x5D);
        g[t] = sll(g[idx], 2);
        g[t] = subu(g[t], g[idx]);
        g[t] = sll(g[t], 2);
    };
    if g[T6] != 0 {
        g[A1] = lb(m, g[A0], 0x71);
        g[V0] = 0;
        if (g[A1] as i64) > 0 {
            key(g, T7, T8);
            g[A3] = li(0x800A_0000);
            g[A2] = g[A0];
            g[A3] = addu(g[A3], g[T8]);
            g[A3] = lb(m, g[A3], 0x21C2);
            loop {
                g[T9] = lb(m, g[A2], 0x72);
                if g[T9] == g[A3] {
                    g[V1] = g[V0];
                }
                g[V0] = addu(g[V0], 1);
                g[AT] = slt(g[V0], g[A1]);
                g[A2] = addu(g[A2], 1);
                if g[AT] == 0 {
                    break;
                }
            }
        }
        g[A2] = u64::MAX;
        g[V0] = if g[V1] == g[A2] { 0 } else { g[V1] };
        return;
    }
    g[A1] = lb(m, g[A0], 0x71);
    g[V0] = 0;
    g[A2] = g[A0];
    g[A3] = li(0x800A_0000);
    if (g[A1] as i64) > 0 {
        key(g, T2, T3);
        g[A0] = li(0x8011_8F90);
        g[T0] = li(SLOT_TAG);
        g[A3] = addu(g[A3], g[T3]);
        g[A3] = lb(m, g[A3], 0x21C2);
        g[T1] = 0x88;
        loop {
            g[T4] = lb(m, g[A2], 0x72);
            if g[T4] == g[A3] {
                g[T5] = multu(g[V0], g[T1]).0;
                g[T6] = addu(g[A0], g[T5]);
                g[T7] = lw(m, g[T6], 4);
                if g[T0] == g[T7] {
                    g[V1] = g[V0];
                }
            }
            g[V0] = addu(g[V0], 1);
            g[AT] = slt(g[V0], g[A1]);
            g[A2] = addu(g[A2], 1);
            if g[AT] == 0 {
                break;
            }
        }
    }
    g[A2] = u64::MAX;
    g[T0] = li(SLOT_TAG);
    if g[V1] == g[A2] {
        // No tagged match: the first tagged slot.
        g[V0] = 0;
        g[T8] = 0;
        if (g[A1] as i64) > 0 {
            g[T9] = li(0x8011_8F90);
            g[A0] = addu(g[T8], g[T9]);
            g[T2] = lw(m, g[A0], 4);
            loop {
                if g[T0] == g[T2] {
                    g[V1] = g[V0];
                } else {
                    g[V0] = addu(g[V0], 1);
                    g[A0] = addu(g[A0], 0x88);
                }
                g[AT] = slt(g[V0], g[A1]);
                if g[V1] != g[A2] || g[AT] == 0 {
                    break;
                }
                g[T2] = lw(m, g[A0], 4);
            }
        }
    }
    g[V0] = g[V1];
}

/// `func_8004FF7C()`: the four words `0x800A4B6C..0x800A4B78` = -1 (stored
/// `6C`, `78`, `74`, `70`). Leaves `t6`..`t9 = -1`, `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8004FF7C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    for (r, off) in [(T6, 0x4B6C), (T9, 0x4B78), (T8, 0x4B74), (T7, 0x4B70)] {
        g[r] = u64::MAX;
        sw(m, g[AT], off, g[r]);
    }
}

/// `func_80050208(p)`: initialise a record (the one [`func_8004FE30`] and
/// [`func_8004F6E8`] read): `+0x64 = 0`, `+0x68 = -1`, the bytes `+0x6C =
/// 1`, `+0x6D..+0x6F = 0`, `+0x70 = 1`, `+0x71 = 12`, `+0x8E = 3`, `+0x8F
/// = +0x90 = 2`, and `+0x72 + i = i` for `i < 23`. Leaves `a2 = 1`, `a3 =
/// 2`, `t6 = -1`, `t7 = 12`, `t8 = 3`, `a0 = v0 = 23`, `v1 = p + 23`,
/// `t9`/`t0`/`t1` = 20..22.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80050208(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A2] = 1;
    g[A3] = 2;
    g[T6] = u64::MAX;
    g[T7] = 0xC;
    g[T8] = 3;
    sw(m, g[A0], 0x64, 0);
    sw(m, g[A0], 0x68, g[T6]);
    for (off, v) in [
        (0x6C, g[A2]),
        (0x6D, 0),
        (0x6E, 0),
        (0x6F, 0),
        (0x70, g[A2]),
        (0x71, g[T7]),
        (0x8E, g[T8]),
        (0x8F, g[A3]),
        (0x90, g[A3]),
        (0x74, g[A3]),
        (0x73, g[A2]),
        (0x72, 0),
    ] {
        sb(m, g[A0], off, v);
    }
    g[V1] = addu(g[A0], 3);
    g[A0] = 0x17;
    g[V0] = 3;
    loop {
        g[T9] = addu(g[V0], 1);
        g[T0] = addu(g[V0], 2);
        g[T1] = addu(g[V0], 3);
        sb(m, g[V1], 0x72, g[V0]);
        g[V0] = addu(g[V0], 4);
        sb(m, g[V1], 0x75, g[T1]);
        sb(m, g[V1], 0x74, g[T0]);
        sb(m, g[V1], 0x73, g[T9]);
        g[V1] = addu(g[V1], 4);
        if g[V0] == g[A0] {
            break;
        }
    }
}

/// `func_80051994(a, b)`: swap the words `a` and `b` of [`PLAYER_TABLE`]
/// (unbounded). Leaves `a3` = the table, `t6`/`t7 = 4a`/`4b`, `v1`/`a2` =
/// the two addresses, `v0`/`t8` = the two old words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80051994(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A3] = li(PLAYER_TABLE);
    g[T7] = sll(g[A1], 2);
    g[T6] = sll(g[A0], 2);
    g[A2] = addu(g[A3], g[T7]);
    g[T8] = lw(m, g[A2], 0);
    g[V1] = addu(g[A3], g[T6]);
    g[V0] = lw(m, g[V1], 0);
    sw(m, g[V1], 0, g[T8]);
    sw(m, g[A2], 0, g[V0]);
}

/// The four words at `0x8011B1BC` that [`func_80051FF4`] and
/// [`func_800520C8`] read, in `t6`..`t9`.
const FOUR: [(usize, i32); 4] = [(T6, -0x4E44), (T7, -0x4E40), (T8, -0x4E3C), (T9, -0x4E38)];

/// `func_80051FF4()`: the index of the first zero among the four words at
/// `0x8011B1BC`, or 4. Leaves the words read in `t6`..`t9` and the next
/// register of the four `= 0x80120000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80051FF4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = li(0x8012_0000);
    for (k, &(r, off)) in FOUR.iter().enumerate() {
        g[r] = lw(&mem, g[r], off);
        if let Some(&(next, _)) = FOUR.get(k + 1) {
            g[next] = li(0x8012_0000);
        }
        if g[r] == 0 {
            g[V0] = k as u64;
            return;
        }
    }
    g[V0] = 4;
}

/// `func_800520C8(x)`: the index of the first of the four words at
/// `0x8011B1BC` equal to `x` (full 64-bit compare with the sign-extended
/// word), or -1. Leaves the same registers as [`func_80051FF4`].
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800520C8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = li(0x8012_0000);
    for (k, &(r, off)) in FOUR.iter().enumerate() {
        g[r] = lw(&mem, g[r], off);
        if let Some(&(next, _)) = FOUR.get(k + 1) {
            g[next] = li(0x8012_0000);
        }
        if g[A0] == g[r] {
            g[V0] = k as u64;
            return;
        }
    }
    g[V0] = u64::MAX;
}

/// `func_8005D310(a, b)`: `[0x800A59FC] = a`, `[0x800A5A00] = b`. Leaves `at
/// = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8005D310(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    sw(&mut mem, g[AT], 0x59FC, g[A0]);
    sw(&mut mem, g[AT], 0x5A00, g[A1]);
}

/// `func_8005EEFC()`: the word `[0x8011AC8C]`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8005EEFC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    ctx.gpr[V0] = lw(&mem, li(0x8012_0000), -0x5374);
}

/// `func_8005F31C(p, v)`: if `p != 0`, `[p + 0x18] = v` and `[p + 0x14]`
/// counts one more. Leaves `t6`/`t7` = the old and new count.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8005F31C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    if g[A0] == 0 {
        return;
    }
    g[T6] = lw(m, g[A0], 0x14);
    sw(m, g[A0], 0x18, g[A1]);
    g[T7] = addu(g[T6], 1);
    sw(m, g[A0], 0x14, g[T7]);
}

/// An empty function of one argument: it spills `a0` to its slot `[sp]`.
/// Domain: canonical `sp` with `[sp]` in RDRAM.
unsafe fn spill_one(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    sw(&mut mem, ctx.gpr[SP], 0, ctx.gpr[A0]);
}

/// `func_80062C78(a0)`: empty, spills `a0` ([`spill_one`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80062C78(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_one(rdram, ctx);
}

/// Where [`func_80063344`] gets each type's pair (indexed by type - 1).
#[derive(Clone, Copy)]
enum PairSource {
    /// Halfword pairs at `table + 4 * i`, each converted and multiplied by
    /// the float at `0x800B0000 + scale`. The case's seven temporaries run
    /// through [`T_CYCLE`] from `first`.
    Table { table: u32, scale: i32, first: usize },
    /// Two float words at `0x800B0000 + at`, through `fa` and `fb`.
    Words { at: i32, fa: usize, fb: usize },
    Zero,
}

/// The temporaries in the order the compiler cycles through them.
const T_CYCLE: [usize; 10] = [T0, T1, T2, T3, T4, T5, T6, T7, T8, T9];

const PAIRS: [PairSource; 12] = [
    PairSource::Table { table: 0x800A_3090, scale: -0x2C14, first: 8 },
    PairSource::Table { table: 0x800A_3104, scale: -0x2C10, first: 5 },
    PairSource::Table { table: 0x800A_313C, scale: -0x2C0C, first: 2 },
    PairSource::Words { at: -0x2C08, fa: 4, fb: 6 },
    PairSource::Words { at: -0x2C00, fa: 8, fb: 10 },
    PairSource::Zero,
    PairSource::Words { at: -0x2BF8, fa: 16, fb: 18 },
    PairSource::Table { table: 0x800A_31B0, scale: -0x2BE4, first: 0 },
    PairSource::Table { table: 0x800A_31C4, scale: -0x2BE0, first: 7 },
    PairSource::Table { table: 0x800A_317C, scale: -0x2BF0, first: 9 },
    PairSource::Table { table: 0x800A_319C, scale: -0x2BE8, first: 3 },
    PairSource::Table { table: 0x800A_318C, scale: -0x2BEC, first: 6 },
];

/// `func_80063344(obj, &x, &y)`: store a pair of floats chosen by the
/// object's type `[obj + 8]` (1-12) and index `i = [obj + 0x88]`
/// ([`PAIRS`] by type - 1). Types 1-3 and 8-12 scale a halfword pair from
/// their table, `x = (f32) tbl[i].0 * scale` and `y = (f32) tbl[i].1 *
/// scale`. Types 4, 5 and 7 copy two constant words. Type 6 and any type
/// outside 1-12 store 0.0 in both. With `i == -1`, types 1, 2, 6 and 3
/// return first and store nothing.
///
/// QUIRKs: the index is read again after `x` is stored, so an `x` that
/// aliases `[obj + 0x88]` changes `y`'s entry. With `i == -1`, the table
/// types 8-12 read `tbl[-1]`, the word before their table.
///
/// The type - 1 picks the case through a jump table at `0x800AD3BC`
/// (bounded by `sltiu 12`; the C's `default` can't be reached). Domain:
/// `obj`, `x`, `y` word-aligned and the entries in RDRAM, the scale floats
/// not NaN (`NAN_CHECK`), and in a table case an `x` aliasing `[obj +
/// 0x88]` only if the stored bits make an index that stays in RDRAM.
/// Leaves `v0 = i`, `at` and `t6`/`t7` from the type test (`t7` the table
/// entry's address, from N64Recomp's `addiu` for the table's `lw`), `v1` =
/// the table, the case's temporaries, `f0` = the scale (or 0.0) and
/// `f4`..`f18` as the case left them.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80063344(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, g[A0], 0x88);
    g[AT] = u64::MAX;
    if g[V0] == g[AT] {
        g[V1] = lw(m, g[A0], 8);
        // Types 1, 2, 6 and 3 return; each beq's delay slot loads the next.
        g[AT] = 1;
        for next in [2, 6, 3] {
            let hit = g[V1] == g[AT];
            g[AT] = next;
            if hit {
                return;
            }
        }
        if g[V1] == g[AT] {
            return;
        }
    }
    g[T6] = lw(m, g[A0], 8);
    g[T7] = addu(g[T6], u64::MAX);
    g[AT] = sltu(g[T7], 12);
    g[T7] = sll(g[T7], 2);
    let source = if g[AT] == 0 {
        PairSource::Zero
    } else {
        g[AT] = addu(li(0x800B_0000), g[T7]);
        let case = (g[T7] >> 2) as usize; // 0..=11
        g[T7] = addu(g[AT], (-0x2C44i64) as u64); // the table's lw, as N64Recomp emits it
        PAIRS[case]
    };
    match source {
        PairSource::Zero => {
            ctx.fpr[0].set_u32l(0);
            sw(m, g[A1], 0, 0);
            sw(m, g[A2], 0, 0);
        }
        PairSource::Words { at, fa, fb } => {
            g[AT] = li(0x800B_0000);
            ctx.fpr[fa].set_u32l(lw(m, g[AT], at) as u32);
            sw(m, g[A1], 0, u64::from(ctx.fpr[fa].u32l()));
            ctx.fpr[fb].set_u32l(lw(m, g[AT], at + 4) as u32);
            sw(m, g[A2], 0, u64::from(ctx.fpr[fb].u32l()));
        }
        PairSource::Table { table, scale, first } => {
            let t = |k: usize| T_CYCLE[(first + k) % T_CYCLE.len()];
            g[V1] = li(table);
            g[t(0)] = sll(g[V0], 2);
            g[t(1)] = addu(g[V1], g[t(0)]);
            g[t(2)] = lh(m, g[t(1)], 0);
            g[AT] = li(0x800B_0000);
            ctx.fpr[0].set_u32l(lw(m, g[AT], scale) as u32);
            ctx.fpr[4].set_u32l(g[t(2)] as u32);
            ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
            ctx.fpr[8].set_fl(ctx.fpr[6].fl() * ctx.fpr[0].fl());
            sw(m, g[A1], 0, u64::from(ctx.fpr[8].u32l()));
            g[t(3)] = lw(m, g[A0], 0x88); // QUIRK: read again after the store
            g[t(4)] = sll(g[t(3)], 2);
            g[t(5)] = addu(g[V1], g[t(4)]);
            g[t(6)] = lh(m, g[t(5)], 2);
            ctx.fpr[10].set_u32l(g[t(6)] as u32);
            ctx.fpr[16].set_fl(fpu::cvt_s_w(ctx.fpr[10].u32l(), fpu::NEAREST));
            ctx.fpr[18].set_fl(ctx.fpr[16].fl() * ctx.fpr[0].fl());
            sw(m, g[A2], 0, u64::from(ctx.fpr[18].u32l()));
        }
    }
}

/// `func_8006506C(a0)`: empty, spills `a0` ([`spill_one`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8006506C(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_one(rdram, ctx);
}

/// `func_80065804(v)`: `[0x8011C840] = v`. Leaves `at = 0x80120000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80065804(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x8012_0000);
    sw(&mut mem, g[AT], -0x37C0, g[A0]);
}

/// `func_80065CB0(p, v)`: if `p != 0`, `[p + 0xF0] = v`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80065CB0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    if g[A0] != 0 {
        sw(&mut mem, g[A0], 0xF0, g[A1]);
    }
}

/// `func_8006C6D0(p, i)`: if bit 13 of `[0x800D76F0 + 4i]` is set, set bit
/// 12 of `[p + 0x60]` (stored only if it was clear). Leaves `t6 = 4i`, `t7`
/// = the word, `t8` = its bit, and `v0`/`t9`/`t0` = the flags, their bit
/// and the flags with it when read.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8006C6D0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A1], 2);
    g[T7] = lw(m, addu(li(0x800D_0000), g[T6]), 0x76F0);
    g[T8] = g[T7] & 0x2000;
    if g[T8] == 0 {
        return;
    }
    g[V0] = lw(m, g[A0], 0x60);
    g[T9] = g[V0] & 0x1000;
    g[T0] = g[V0] | 0x1000;
    if g[T9] == 0 {
        sw(m, g[A0], 0x60, g[T0]);
    }
}

/// `func_8006C708(p)`: a 2-bit summary of the six words `w0..w5` at `p +
/// 0x2A0`: bit 0 if any of `w0..w2` has bit 2 or 4 set, bit 1 if any of
/// `w3..w5` has. The compiler unrolled a loop over the six with its index
/// folded in (`a1 = 8`), so the tests `8 < 12`, `8 < 8`, `8 < 4` and `8 <
/// 0` pick the bit for the last four.
///
/// Leaves `v0 = v1` = the result, `a0 = p`, `a1 = 8`, `a2 = p + 8`, `a3 =
/// w2`, `at` = the last `slt` (0), and each word's two bit tests and the
/// candidate results in `t0`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8006C708(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    // Bits 2 and 4 of `w` into ta and tb; whether either is set.
    let hit = |g: &mut [u64; 32], w: usize, ta: usize, tb: usize| {
        g[ta] = g[w] & 4;
        g[tb] = g[w] & 0x10;
        g[ta] != 0 || g[tb] != 0
    };
    g[A1] = lw(m, g[A0], 0x2A0);
    g[V1] = 0;
    g[V0] = g[A0];
    if hit(g, A1, T6, T7) {
        g[V1] = 1;
    }
    g[A1] = lw(m, g[V0], 0x2A4);
    g[V0] = 2;
    g[T0] = g[V1] | 1;
    if hit(g, A1, T8, T9) {
        g[V1] = g[T0];
    }
    g[A1] = sll(g[V0], 2);
    g[A2] = addu(g[A0], g[A1]);
    g[A3] = lw(m, g[A2], 0x2A0);
    // w2..w4: `at = a1 < bound` chooses bit 0 (set) or bit 1.
    let mut word = A3;
    for (k, (bound, ta, tb, t_hi, t_lo)) in [(0xC, T1, T2, T4, T3), (8, T5, T6, T8, T7), (4, T9, T0, T2, T1)].into_iter().enumerate() {
        g[AT] = slt(g[A1], bound);
        if hit(g, word, ta, tb) {
            g[t_hi] = g[V1] | 2;
            if g[AT] == 0 {
                g[V1] = g[t_hi];
            } else {
                g[t_lo] = g[V1] | 1;
                g[V1] = g[t_lo];
            }
        }
        g[V0] = lw(m, g[A2], 0x2A4 + 4 * k as i32);
        word = V0;
    }
    // w5: `a1 < 0` would choose bit 0.
    if hit(g, V0, T3, T4) {
        g[T6] = g[V1] | 2;
        if (g[A1] as i64) < 0 {
            g[V0] = g[V1] | 1;
            return;
        }
        g[V1] = g[T6];
    }
    g[V0] = g[V1];
}

/// `func_8006E008(a0)`: whether the word at `[sp - 8]` is 2, spilling `a0`
/// to its slot `[sp]`. QUIRK: `[sp - 8]` is the first word of the
/// function's own 8-byte frame, which nothing writes: an uninitialised
/// stack read. Leaves `t6` = that word, `at = 2`, `v1 = v0`, and `sp` back
/// (sign-extended from its low word).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8006E008(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-8i64) as u64);
    g[T6] = lw(m, g[SP], 0); // QUIRK: never written
    sw(m, g[SP], 8, g[A0]);
    g[AT] = 2;
    g[SP] = addu(g[SP], 8);
    g[V1] = u64::from(g[T6] == g[AT]);
    g[V0] = g[V1];
}

/// `func_8006FED0(p)`: the flags word `[p + 0x60]`. It tests bit 7 and
/// branches, but both paths return at once (compiled-out code, **guess**).
/// Leaves `t6` = bit 7.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8006FED0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[V0] = lw(&mem, g[A0], 0x60);
    g[T6] = g[V0] & 0x80;
}

/// `func_800735B4`: empty (`jr ra`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800735B4(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_80073708(a0)`: spills `a0` to its slot `[sp]` and returns 0.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80073708(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_one(rdram, ctx);
    (*ctx).gpr[V0] = 0;
}

/// `func_8007AFE0(a0)`: empty, spills `a0` ([`spill_one`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8007AFE0(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_one(rdram, ctx);
}

/// `func_8007B41C(v)`: `[0x8011C8E0] = v`. Leaves `at = 0x80120000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8007B41C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x8012_0000);
    sw(&mut mem, g[AT], -0x3720, g[A0]);
}

/// `func_8007BA9C(x)`: the 0x58-byte record at `0x8011CB20` that belongs
/// to `x` in the 50-entry list at `0x8011CA58`: `i` = the first entry equal
/// to `x` or zero (or 50 if neither), and the record `i` if entry `i` is
/// nonzero, else 0.
///
/// QUIRK: when the search runs off the end (`i = 50`), "entry 50" is the
/// first word of record 0, right after the list, and if that is nonzero
/// the result is record 50, past the records. Leaves `v1 = i`, `a1 =
/// 0x8011CA58`, `t2 = 0x8011CB20`, `t0` = entry `i`, `t1` = `0x58 * i` or
/// `3 * i`, `t8`/`t9` = its offset and address, and `at`, `t6`, `t7` from
/// the search.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8007BA9C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, li(0x8012_0000), -0x35A8);
    g[T2] = li(0x8011_CB20);
    g[V1] = 0;
    if g[V0] != 0 {
        g[A1] = li(0x8012_0000);
        if g[A0] != g[V0] {
            g[A1] = li(0x8011_CA58);
            g[V1] = 1;
            loop {
                g[AT] = slt(g[V1], 0x32);
                g[T6] = sll(g[V1], 2);
                if g[AT] == 0 {
                    break;
                }
                g[T7] = addu(g[A1], g[T6]);
                g[V0] = lw(m, g[T7], 0);
                if g[V0] == 0 || g[A0] == g[V0] {
                    break;
                }
                g[V1] = addu(g[V1], 1);
            }
        }
    }
    g[A1] = li(0x8011_CA58);
    g[T8] = sll(g[V1], 2);
    g[T9] = addu(g[A1], g[T8]);
    g[T0] = lw(m, g[T9], 0);
    g[T1] = sll(g[V1], 2);
    g[T1] = subu(g[T1], g[V1]);
    g[V0] = 0;
    if g[T0] != 0 {
        g[T1] = sll(g[T1], 2);
        g[T1] = subu(g[T1], g[V1]);
        g[T1] = sll(g[T1], 3);
        g[V0] = addu(g[T1], g[T2]);
    }
}

/// `func_8007C3BC(a0)`: empty, spills `a0` ([`spill_one`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8007C3BC(rdram: *mut u8, ctx: *mut RecompContext) {
    spill_one(rdram, ctx);
}

/// A stub of two arguments: spill `a0` to `[sp]`, `*a1 = 0`, return 0.
unsafe fn clear_out(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    sw(&mut mem, g[SP], 0, g[A0]);
    sw(&mut mem, g[A1], 0, 0);
    g[V0] = 0;
}

/// `func_8007F22C(a0, &out)`: `out = 0`, returns 0 ([`clear_out`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8007F22C(rdram: *mut u8, ctx: *mut RecompContext) {
    clear_out(rdram, ctx);
}

/// `func_8007F23C(a0, &out)`: `out = 0`, returns 0 ([`clear_out`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8007F23C(rdram: *mut u8, ctx: *mut RecompContext) {
    clear_out(rdram, ctx);
}

/// `func_800811CC()`: `[0x800A6758] = 1`. Leaves `t6 = 1`, `at =
/// 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800811CC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[T6] = 1;
    g[AT] = li(0x800A_0000);
    sw(&mut mem, g[AT], 0x6758, g[T6]);
}

/// `func_80081260()`: `[0x800A675C] = 0`. Leaves `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80081260(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    sw(&mut mem, g[AT], 0x675C, 0);
}

/// `func_80081530(a, b, n)`: 1 if the first `n` bytes at `a` and `b` are
/// equal (or `n <= 0`, signed), else 0. The compiler compares `n & 3`
/// single bytes, then four at a time; it stops at the first difference.
///
/// Leaves `v1`/`a3` at the bytes compared last, `t0 = t1 = n & 3`, and the
/// last bytes loaded in `t2`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80081530(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V0] = 0;
    if (g[A2] as i64) <= 0 {
        g[V0] = 1;
        return;
    }
    g[T1] = g[A2] & 3;
    g[T0] = g[T1];
    let mut done = false;
    if g[T1] != 0 {
        g[V1] = addu(g[A0], 0);
        g[A3] = addu(g[A1], 0);
        loop {
            g[T6] = lbu(m, g[V1], 0);
            g[T7] = lbu(m, g[A3], 0);
            g[V0] = addu(g[V0], 1);
            g[V1] = addu(g[V1], 1);
            if g[T6] != g[T7] {
                g[V0] = 0;
                return;
            }
            g[A3] = addu(g[A3], 1);
            if g[T0] == g[V0] {
                break;
            }
        }
        if g[V0] == g[A2] {
            g[V1] = addu(g[A0], g[V0]);
            done = true;
        }
    }
    if !done {
        g[V1] = addu(g[A0], g[V0]);
        g[A3] = addu(g[A1], g[V0]);
        'quads: loop {
            for (k, (x, y)) in [(T8, T9), (T2, T3), (T4, T5), (T6, T7)].into_iter().enumerate() {
                g[x] = lbu(m, g[V1], k as i32);
                g[y] = lbu(m, g[A3], k as i32);
                if k == 0 {
                    g[V0] = addu(g[V0], 4);
                }
                if k == 3 {
                    g[V1] = addu(g[V1], 4);
                }
                if g[x] != g[y] {
                    g[V0] = 0;
                    return;
                }
            }
            g[A3] = addu(g[A3], 4);
            if g[V0] == g[A2] {
                break 'quads;
            }
        }
    }
    g[V0] = 1;
}

/// `func_800815FC(a, b)` = `strcmp` (-1, 0 or 1): compare the NUL-terminated
/// byte strings (unsigned bytes) to the first difference or end; a string
/// that ends first is less. Leaves `a0`/`a1` at the last bytes compared,
/// `v1` the last byte of `b`, `at` the last test, `t6` = `b`'s byte when
/// `a` ended.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800815FC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[V0] = lbu(m, g[A0], 0);
    if g[V0] != 0 {
        g[V1] = lbu(m, g[A1], 0);
        loop {
            g[AT] = slt(g[V0], g[V1]);
            if g[V1] == 0 {
                g[V0] = 1;
                return;
            }
            if g[AT] != 0 {
                g[V0] = u64::MAX;
                return;
            }
            g[AT] = slt(g[V1], g[V0]);
            if g[AT] != 0 {
                g[V0] = 1;
                return;
            }
            g[V0] = lbu(m, g[A0], 1);
            g[A0] = addu(g[A0], 1);
            g[A1] = addu(g[A1], 1);
            if g[V0] == 0 {
                break;
            }
            g[V1] = lbu(m, g[A1], 0);
        }
    }
    // `a` ended: equal if `b` did too.
    g[T6] = lbu(m, g[A1], 0);
    g[V0] = if g[T6] == 0 { 0 } else { u64::MAX };
}

/// `func_800834DC(a0, a1, a2, a3)`: an empty function that spills all four
/// arguments to their slots `[sp]..[sp + 0xC]`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800834DC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    for (k, r) in [A0, A1, A2, A3].into_iter().enumerate() {
        sw(&mut mem, g[SP], 4 * k as i32, g[r]);
    }
}

/// `func_80084C30(type)`: fill the `OSTask` at `[0x801488C0]` (read again
/// before every field): `ucode_boot` (`+8`) = rspboot at `0x80097FF0`,
/// `ucode_boot_size` (`+0xC`) = `0xD0` (NOTES.md, "Code segment"),
/// `output_buff` (`+0x28`) and its size (`+0x2C`) from `[0x800DB894]` and
/// `[0x800DB898]`; if `(s16) type == 5`, `ucode` (`+0x10`) = the F3DEX2
/// text at `0x800980C0` and `ucode_data` (`+0x18`) = `0x800AE1D0`; and
/// `data_ptr` (`+0x30`) = `[0x801217B4]`. Spills `type` to its slot
/// `[sp]`.
///
/// Leaves `v0 = 0x801488C0`, `v1 = 0x80097FF0`, `at = 5`, `t0` = the data
/// pointer, and the pointers and values in `t1`..`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80084C30(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x8014_88C0);
    g[T8] = lw(m, g[V0], 0);
    g[V1] = li(0x8009_7FF0);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[T8], 8, g[V1]);
    g[T1] = lw(m, g[V0], 0);
    g[T9] = li(0x8009_80C0);
    g[T0] = subu(g[T9], g[V1]);
    sw(m, g[T1], 0xC, g[T0]);
    g[T2] = lw(m, li(0x800E_0000), -0x476C);
    g[T3] = lw(m, g[V0], 0);
    g[T6] = sll(g[A0], 16);
    sw(m, g[T3], 0x28, g[T2]);
    g[T5] = lw(m, g[V0], 0);
    g[T4] = lw(m, li(0x800E_0000), -0x4768);
    g[T7] = sra(g[T6], 16);
    g[AT] = 5;
    sw(m, g[T5], 0x2C, g[T4]);
    if g[T7] == g[AT] {
        g[T7] = lw(m, g[V0], 0);
        g[T6] = li(0x8009_80C0);
        sw(m, g[T7], 0x10, g[T6]);
        g[T9] = lw(m, g[V0], 0);
        g[T8] = li(0x800A_E1D0);
        sw(m, g[T9], 0x18, g[T8]);
    }
    g[T0] = lw(m, li(0x8012_0000), 0x17B4);
    g[T1] = lw(m, g[V0], 0);
    sw(m, g[T1], 0x30, g[T0]);
}

/// `func_8008635C(i, v)`: record `i` of the 0x170-byte records at
/// `0x80120DF0`: bit 0 of `+0` set if `v >= 0` (signed 64-bit), cleared
/// otherwise, and `+4 = v`. Leaves `v0` = the record, `t0` or `t6` = its
/// offset, `t1`/`t7` = the base, the flags in `t2`/`t3` or `t8`/`t9`, `at
/// = -2` when clearing.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008635C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    // ((((i << 2) - i) << 3) - i) << 4 = 0x170 * i, through `t`.
    let record = |g: &mut [u64; 32], t: usize, base: usize| {
        g[t] = subu(g[t], g[A0]);
        g[t] = sll(g[t], 3);
        g[t] = subu(g[t], g[A0]);
        g[base] = li(0x8012_0DF0);
        g[t] = sll(g[t], 4);
        g[V0] = addu(g[t], g[base]);
    };
    g[T0] = sll(g[A0], 2);
    if (g[A1] as i64) >= 0 {
        record(g, T0, T1);
        g[T2] = lw(m, g[V0], 0);
        g[T3] = g[T2] | 1;
        sw(m, g[V0], 0, g[T3]);
    } else {
        g[T6] = sll(g[A0], 2);
        record(g, T6, T7);
        g[T8] = lw(m, g[V0], 0);
        g[AT] = (-2i64) as u64;
        g[T9] = g[T8] & g[AT];
        sw(m, g[V0], 0, g[T9]);
    }
    sw(m, g[V0], 4, g[A1]);
}

/// `func_80086CC8(a, b, c)`: the halfwords `a`, `b`, `c` at `0x801488B8`.
/// Leaves `v0 = 0x801488B8`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80086CC8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    g[V0] = li(0x8014_88B8);
    for (k, r) in [A0, A1, A2].into_iter().enumerate() {
        sh(&mut mem, g[V0], 2 * k as i32, g[r]);
    }
}
