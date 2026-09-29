//! Controller input (**guess** at the subsystem, from the data's shape):
//! the four 6-byte pads at `0x800D74C0` (`OSContPad`: u16 buttons, s8
//! stick x, s8 stick y, u8 error) turned into the game's 0x18-byte
//! records at `0x800D74D8`.

// Ports keep N64Recomp's names (func_8002EA28), capitals included.
#![allow(non_snake_case)]

use crate::recomp::{addu, enter, fpu, lb, lhu, li, lw, multu, reg::*, s32, sb, sh, sllv, sltu, sw, RecompContext};

/// The four pad records `func_8002EA28` writes, 0x18 bytes each: `+0`/`+2`
/// the stick x/y (s16, clamped to ±100), `+4..+0x13` one byte per button
/// bit (bit k at `+4 + k`, 0 or 1), `+0x14` the button word.
pub const PAD_RECORDS: u32 = 0x800D_74D8;

/// `func_8002EA28()` (pads update, **guess**): fills [`PAD_RECORDS`] from
/// the pads at `0x800D74C0` and returns `v0 = PAD_RECORDS`.
///
/// 1. If bit `0x1000` of the settings word `[0x800D697C]` is set, record 0
///    is cleared: the word `+0x14`, the halfwords `+0`, `+2`, then the bytes
///    `+4..+0x13` (in the order `+5 +6 +7 +4`, `+9 +10 +11 +8`, ...).
/// 2. Unless `[0x800D7490] == -1` or the skip flag `[0x800A26D8] != 0`, for
///    each pad i = 0..3 in order, with `present[i]` the word at `0x800D7498
///    + 4i`:
///    - absent (0): halfwords `+0`, `+2` = 0 and bytes `+4..+0x13` = 0 in
///      the order above; `+0x14` keeps its old value;
///    - present: `b` = the pad's u16 buttons; the halfword `[0x800D74A8] = b`
///      (QUIRK: one halfword for all four, so the last present pad wins);
///      `+0` = the stick x byte (signed) clamped to -100..100 (converted to
///      float, compared with -100.0 and 100.0, truncated), `+2` the same for
///      y; `+0x14 = b`; then bytes `+4 + k` = bit k of `b`, k = 0..15 in
///      order.
/// 3. `[0x800A26D8] = 0`.
///
/// Saves `s0`/`s1` at `sp - 8`/`sp - 4` and restores them. Leaves `t2 =
/// v0 = PAD_RECORDS` and `at = 0x800A0000`. If the loop ran: `a0 =
/// 0x800D74A8`, `v1 = 4`, `a2 = t0 = PAD_RECORDS + 0x60`, `t1 =
/// PAD_RECORDS + 0x58`, `s0`/`s1` restored over 16/4, `f12 = -100.0`, `f14
/// = 100.0`, and `a1`, `a3`, `t3`..`t9`, `f0`..`f18` as the last pads'
/// paths left them. Otherwise `t8` = `[0x800D7490]`, `a0`/`v1`/`t9` as far
/// as the tests got, and `v1 = 0x800D74E8` from the clearing loop if it ran.
///
/// Domain: any memory contents; nothing reaches float arithmetic.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002EA28(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = li(0x800D_0000);
    g[T6] = lw(m, g[T6], 0x697C);
    g[SP] = addu(g[SP], (-0x10i64) as u64);
    sw(m, g[SP], 0xC, g[S1]);
    g[T7] = g[T6] & 0x1000;
    sw(m, g[SP], 8, g[S0]);
    if g[T7] != 0 {
        // 1. Clear record 0: v1 walks its bytes.
        g[T2] = li(0x800D_74D8);
        g[V1] = li(0x800D_0000);
        g[V0] = li(0x800D_0000);
        sw(m, g[T2], 0x14, 0);
        sh(m, g[T2], 0, 0);
        sh(m, g[T2], 2, 0);
        g[V0] = addu(g[V0], 0x74E8);
        g[V1] = addu(g[V1], 0x74D8);
        loop {
            g[V1] = addu(g[V1], 4);
            sb(m, g[V1], 1, 0);
            sb(m, g[V1], 2, 0);
            sb(m, g[V1], 3, 0);
            sb(m, g[V1], 0, 0);
            if g[V1] == g[V0] {
                break;
            }
        }
    }
    g[T8] = li(0x800D_0000);
    g[T8] = lw(m, g[T8], 0x7490);
    g[T2] = li(0x800D_0000);
    g[AT] = u64::MAX;
    g[T2] = addu(g[T2], 0x74D8);
    if g[T8] != g[AT] {
        g[T9] = li(0x800A_0000);
        g[T9] = lw(m, g[T9], 0x26D8);
        g[A0] = li(0x800D_7498);
        g[V1] = 0;
        if g[T9] == 0 {
            // 2. a0 = &present[i], v1 = i, t4 = the pads, t3 = the shared
            // buttons halfword, t0 = a2 = the record.
            g[AT] = li(0x42C8_0000);
            f[14].set_u32l(g[AT] as u32); // 100.0
            g[AT] = li(0xC2C8_0000);
            g[T6] = li(0x800D_0000);
            g[T0] = addu(g[T6], 0x74D8);
            g[T4] = li(0x800D_0000);
            g[T3] = li(0x800D_0000);
            f[12].set_u32l(g[AT] as u32); // -100.0
            g[T3] = addu(g[T3], 0x74A8);
            g[T4] = addu(g[T4], 0x74C0);
            g[A2] = g[T0];
            g[S1] = 4;
            g[S0] = 0x10;
            g[T5] = 6;
            loop {
                g[T7] = lw(m, g[A0], 0);
                g[A0] = addu(g[A0], 4);
                g[V0] = 0;
                g[T1] = g[T0];
                if g[T7] == 0 {
                    // Absent: t1 walks the button bytes, v0 counts them.
                    sh(m, g[A2], 0, 0);
                    sh(m, g[A2], 2, 0);
                    loop {
                        g[V0] = addu(g[V0], 4);
                        sb(m, g[T1], 5, 0);
                        sb(m, g[T1], 6, 0);
                        sb(m, g[T1], 7, 0);
                        g[T1] = addu(g[T1], 4);
                        sb(m, g[T1], 0, 0);
                        if g[V0] == g[S0] {
                            break;
                        }
                    }
                } else {
                    // Present: v0 = the pad, a1 = a3 = its buttons.
                    let (lo, _) = multu(g[V1], g[T5]);
                    g[T1] = g[T0];
                    g[T8] = lo;
                    g[V0] = addu(g[T4], g[T8]);
                    g[T9] = lb(m, g[V0], 2);
                    g[A1] = lhu(m, g[V0], 0);
                    f[4].set_u32l(g[T9] as u32);
                    sh(m, g[T3], 0, g[A1]);
                    g[A3] = g[A1];
                    // Stick x, clamped.
                    f[0].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
                    if !(f[12].fl() <= f[0].fl()) {
                        f[8].set_u32l(fpu::trunc_w_s(f[12].fl()));
                        g[T9] = s32(f[8].u32l());
                        sh(m, g[A2], 0, g[T9]);
                    } else {
                        if !(f[0].fl() <= f[14].fl()) {
                            f[2].set_u32l(f[14].u32l());
                        } else {
                            f[2].set_u32l(f[0].u32l());
                        }
                        f[6].set_u32l(fpu::trunc_w_s(f[2].fl()));
                        g[T7] = s32(f[6].u32l());
                        sh(m, g[A2], 0, g[T7]);
                    }
                    // Stick y, clamped.
                    g[T6] = lb(m, g[V0], 3);
                    g[V0] = 0;
                    f[10].set_u32l(g[T6] as u32);
                    f[0].set_fl(fpu::cvt_s_w(f[10].u32l(), fpu::NEAREST));
                    if !(f[12].fl() <= f[0].fl()) {
                        f[18].set_u32l(fpu::trunc_w_s(f[12].fl()));
                        g[T6] = s32(f[18].u32l());
                        sh(m, g[A2], 2, g[T6]);
                    } else {
                        if !(f[0].fl() <= f[14].fl()) {
                            f[2].set_u32l(f[14].u32l());
                        } else {
                            f[2].set_u32l(f[0].u32l());
                        }
                        f[16].set_u32l(fpu::trunc_w_s(f[2].fl()));
                        g[T8] = s32(f[16].u32l());
                        sh(m, g[A2], 2, g[T8]);
                    }
                    sw(m, g[A2], 0x14, g[A1]);
                    // One byte per button bit: v0 = k, t1 walks the bytes.
                    loop {
                        g[T7] = 1;
                        g[T8] = sllv(g[T7], g[V0]);
                        g[T9] = g[A3] & g[T8];
                        g[T8] = 1;
                        g[T7] = addu(g[V0], 1);
                        g[T6] = sltu(0, g[T9]);
                        g[T9] = sllv(g[T8], g[T7]);
                        sb(m, g[T1], 4, g[T6]);
                        g[T6] = g[A3] & g[T9];
                        g[T8] = sltu(0, g[T6]);
                        g[T9] = 1;
                        g[T7] = addu(g[V0], 2);
                        g[T6] = sllv(g[T9], g[T7]);
                        sb(m, g[T1], 5, g[T8]);
                        g[T8] = g[A3] & g[T6];
                        g[T9] = sltu(0, g[T8]);
                        g[T6] = 1;
                        g[T7] = addu(g[V0], 3);
                        g[T8] = sllv(g[T6], g[T7]);
                        sb(m, g[T1], 6, g[T9]);
                        g[T9] = g[A3] & g[T8];
                        g[T6] = sltu(0, g[T9]);
                        g[V0] = addu(g[V0], 4);
                        sb(m, g[T1], 7, g[T6]);
                        g[T1] = addu(g[T1], 4);
                        if g[V0] == g[S0] {
                            break;
                        }
                    }
                }
                g[V1] = addu(g[V1], 1);
                g[A2] = addu(g[A2], 0x18);
                g[T0] = addu(g[T0], 0x18);
                if g[V1] == g[S1] {
                    break;
                }
            }
        }
    }
    // 3. Clear the skip flag.
    g[AT] = li(0x800A_0000);
    g[S0] = lw(m, g[SP], 8);
    g[S1] = lw(m, g[SP], 0xC);
    sw(m, g[AT], 0x26D8, 0);
    g[SP] = addu(g[SP], 0x10);
    g[V0] = g[T2];
}
