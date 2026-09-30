//! The four 28-byte records at `0x800DB8A0` (channels, **guess**; NOTES.md,
//! "Depth-0 leaves ported in session 7") and the word per channel after them
//! at `0x800DB910`. The setters take -1 for "all four".

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use n64mem::Mem;
use crate::recomp::{addu, call, enter, lh, lhu, li, lw, reg::*, sh, sll, slt, subu, sw, RecompContext};

/// The four 28-byte records at `0x800DB8A0` that [`func_800314C0`] ..
/// [`func_80031640`] use, by index (sound channels? **guess**).
pub const CHANNELS: u32 = 0x800D_B8A0;

/// The entry each channel plays, a word per channel (-1: none):
/// [`func_800319F4`] starts an entry only if it ranks at least as high.
pub const CURRENT: u32 = 0x800A_290C;

/// The 13 entries [`func_800319F4`] starts, 12 bytes each: a halfword
/// value, then the two words [`func_800314DC`] takes as `a2`/`a3`.
pub const ENTRIES: u32 = 0x800A_2870;

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

/// `func_800319F4(i, k)` (start entry `k` on channel `i` if it ranks high
/// enough, **guess**): with `cur` = [`CURRENT`]`[i]` and the entry `e` =
/// [`ENTRIES`] `+ 12k`: nothing if `k < cur` (signed); for `k == cur`,
/// nothing if the word `[e + 4]` is nonzero or the channel isn't stopped
/// (its halfword `+8` nonzero). Otherwise `CURRENT[i] = k` and
/// [`func_800314DC`]`(i, (u16) [e], [e + 4], [e + 8])`.
///
/// QUIRK: `k >= 13` (signed) hangs (`b .`, N64Recomp's `pause_self`) before
/// anything else: an assert's idle loop.
///
/// Frame (`sp - 0x20`): `s0` (holding `k`) at `+0x18` and `ra` at `+0x1C`,
/// both restored sign-extended. Leaves `at` = the test `k < cur`, `v1` =
/// `CURRENT[i]`'s address, `t6 = 4i`, `t7 = CURRENT`, and `v0 = cur` with
/// `t8 = 4k` and, for `k == cur`, `t8 = 12k`, `t9 = [e + 4]`, `t0 = 28i`,
/// `t1` the halfword, `t2 = 4k`; or, when it starts, `t2 = 12k`, `t3 =
/// ENTRIES`, `v0 = e`, the arguments and the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800319F4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x18, g[S0]);
    g[AT] = slt(g[A1], 0xD);
    g[S0] = g[A1];
    sw(m, g[SP], 0x1C, g[RA]);
    if g[AT] == 0 {
        imports::runtime::pause_self(m.as_mut_ptr());
    }
    g[T7] = li(CURRENT);
    g[T6] = sll(g[A0], 2);
    g[V1] = addu(g[T6], g[T7]);
    g[V0] = lw(m, g[V1], 0);
    g[AT] = slt(g[S0], g[V0]);
    if g[AT] != 0 {
        g[RA] = lw(m, g[SP], 0x1C);
    } else {
        'b_80031A9C: {
            g[T8] = sll(g[S0], 2);
            if g[S0] == g[V0] {
                g[T8] = subu(g[T8], g[S0]);
                g[T8] = sll(g[T8], 2);
                g[T9] = li(0x800A_0000);
                g[T9] = addu(g[T9], g[T8]);
                g[T9] = lw(m, g[T9], 0x2874);
                g[T0] = sll(g[A0], 3);
                g[T0] = subu(g[T0], g[A0]);
                g[T0] = sll(g[T0], 2);
                if g[T9] != 0 {
                    break 'b_80031A9C;
                }
                g[T1] = li(0x800E_0000);
                g[T1] = addu(g[T1], g[T0]);
                g[T1] = lh(m, g[T1], -0x4758);
                if g[T1] != 0 {
                    g[T2] = sll(g[S0], 2);
                    break 'b_80031A9C;
                }
            }
            g[T2] = sll(g[S0], 2);
            g[T2] = subu(g[T2], g[S0]);
            g[T3] = li(ENTRIES);
            g[T2] = sll(g[T2], 2);
            g[V0] = addu(g[T2], g[T3]);
            sw(m, g[V1], 0, g[S0]);
            g[A1] = lhu(m, g[V0], 0);
            g[A2] = lw(m, g[V0], 4);
            g[A3] = lw(m, g[V0], 8);
            call(imports::func_800314DC, m, ctx);
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x1C);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_80031AB0(i, k, value)`: [`func_800319F4`] with `(u16) value` in
/// place of the entry's halfword: the same tests on [`CURRENT`]`[i]`, the
/// entry `ENTRIES + 12k` and the channel, then `CURRENT[i] = k` and
/// [`func_800314DC`]`(i, (u16) value, [e + 4], [e + 8])`. `value` is
/// spilled to its slot `[sp + 0x28]` and read back as a halfword.
///
/// QUIRK: `k >= 13` (signed) hangs (`b .`, `pause_self`) first, as in
/// [`func_800319F4`].
///
/// Frame (`sp - 0x20`) and leftovers as for [`func_800319F4`].
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80031AB0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x18, g[S0]);
    g[AT] = slt(g[A1], 0xD);
    g[S0] = g[A1];
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x28, g[A2]);
    if g[AT] == 0 {
        imports::runtime::pause_self(m.as_mut_ptr());
    }
    g[T7] = li(CURRENT);
    g[T6] = sll(g[A0], 2);
    g[V1] = addu(g[T6], g[T7]);
    g[V0] = lw(m, g[V1], 0);
    g[AT] = slt(g[S0], g[V0]);
    if g[AT] != 0 {
        g[RA] = lw(m, g[SP], 0x1C);
    } else {
        'b_80031B5C: {
            g[T8] = sll(g[S0], 2);
            if g[S0] == g[V0] {
                g[T8] = subu(g[T8], g[S0]);
                g[T8] = sll(g[T8], 2);
                g[T9] = li(0x800A_0000);
                g[T9] = addu(g[T9], g[T8]);
                g[T9] = lw(m, g[T9], 0x2874);
                g[T0] = sll(g[A0], 3);
                g[T0] = subu(g[T0], g[A0]);
                g[T0] = sll(g[T0], 2);
                if g[T9] != 0 {
                    break 'b_80031B5C;
                }
                g[T1] = li(0x800E_0000);
                g[T1] = addu(g[T1], g[T0]);
                g[T1] = lh(m, g[T1], -0x4758);
                if g[T1] != 0 {
                    g[T2] = sll(g[S0], 2);
                    break 'b_80031B5C;
                }
            }
            g[T2] = sll(g[S0], 2);
            g[T2] = subu(g[T2], g[S0]);
            g[T3] = li(ENTRIES);
            g[T2] = sll(g[T2], 2);
            g[V0] = addu(g[T2], g[T3]);
            sw(m, g[V1], 0, g[S0]);
            g[A2] = lw(m, g[V0], 4);
            g[A3] = lw(m, g[V0], 8);
            g[A1] = lhu(m, g[SP], 0x2A);
            call(imports::func_800314DC, m, ctx);
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x1C);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_80031BBC(i)` (stop channel `i`, **guess**): its entry at
/// [`CURRENT`] `+ 4i` = -1, then [`func_80031640`]`(i)`.
///
/// QUIRK: for `i == -1` the callee clears all four channels, but the store
/// goes to `CURRENT - 4`, the third word of entry 12 of [`ENTRIES`], and no
/// channel's entry is reset.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `t7 = 4i` and the
/// callee's registers (its `t6` and `at`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80031BBC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[T7] = sll(g[A0], 2);
    g[AT] = li(0x800A_0000);
    sw(m, g[SP], 0x14, g[RA]);
    g[AT] = addu(g[AT], g[T7]);
    g[T6] = u64::MAX;
    sw(m, g[AT], 0x290C, g[T6]);
    call(imports::func_80031640, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
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
