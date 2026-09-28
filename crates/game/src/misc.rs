//! Small leaves whose subsystem isn't known yet, grouped by address. They
//! move to a named module once what they belong to is understood.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use crate::recomp::{addu, div, enter, fpu, lbu, lh, li, lw, multu, reg::*, s32, sb, sh, sll, sllv, slt, sltu, sra, subu, sw, RecompContext};

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
