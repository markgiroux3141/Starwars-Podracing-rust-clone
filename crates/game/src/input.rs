//! Controller input (**guess** at the subsystem, from the data's shape):
//! the four 6-byte pads at `0x800D74C0` (`OSContPad`: u16 buttons, s8
//! stick x, s8 stick y, u8 error) turned into the game's 0x18-byte
//! records at `0x800D74D8`.

// Ports keep N64Recomp's names (func_8002EA28), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use crate::recomp::{addu, call, enter, fpu, lb, lbu, ld, lh, lhu, li, lw, multu, reg::*, s32, sb, sd, sh, sllv, sltu, sw, RecompContext};

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

/// The button words `func_8002ECA0` derives, four each (one per pad), from
/// here: `+0` the held word, `+0x10` newly pressed, `+0x20` newly released,
/// `+0x30` the stick x and `+0x40` the stick y (floats).
pub const BUTTON_WORDS: u32 = 0x800D_76F0;

/// `func_8002ECA0()` (button words, **guess**): updates [`PAD_RECORDS`]
/// ([`func_8002EA28`]), then for each pad i = 0..3 builds a word `w` from
/// its record `r` and stores it with its changes at [`BUTTON_WORDS`] + 4i:
/// - bits 0..13: whether the bytes `r + 0x13, 0x12, 7, 6, 5, 4, 9, 8,
///   0x11, 0x10, 0xF, 0xE, 0xD, 0xC` (in that order) are nonzero;
/// - with `X = f32(s16 [r]) / 100.0` and `Y = f32(s16 [r + 2]) / 100.0`,
///   `x`, `y` their doubles, and the doubles `A` = `[0x800A9FA8]`, `B` =
///   `[0x800A9FA0]`, `C` = `[0x800A9F98]`, `D` = `[0x800A9F90]` (0.3, -0.3,
///   -0.2, 0.2 in the ROM): bit 14 `A < y`, 15 `y < B`, 16 `x < B`, 17 `A <
///   x`, 18 `C < x < D`, 19 `C < y < D`, 20 `B <= x <= C`, 21 `D <= x <=
///   A`, 22 `D <= y <= A`, 23 `B <= y <= C`.
///
/// With `old` the word at `+0`: `+0x10 = (old ^ w) & w`, `+0x20 = (old ^ w)
/// & old`, `+0 = w`, `+0x30 = X`, `+0x40 = Y`, stored in that order.
///
/// Frame (`sp - 0x30`): `ra` at `+0x2C`, `f20`/`f22`/`f24` (whole 64-bit
/// registers) at `+0x10`/`+0x18`/`+0x20`, all restored; `ra` is a
/// temporary (`0x80000`) in between. Leaves, from the last pad: `v0 = w`,
/// `v1 = old`, `a0 = old ^ w`, `t6` the released bits, `t7 =
/// 0x800D7740`, `t8`, `t9` and `at` as its tests left them, `a1 =
/// PAD_RECORDS + 0x60`, `a2`, `t0`, `t1`, `t2`, `a3` just past their
/// arrays, `t3`..`t5` = `0x10000`..`0x40000`, `f0 = x`, `f2 = y`, `f4`,
/// `f6`, `f8`, `f10` the halfwords and their floats, `f12`..`f18` = `A`,
/// `B`, `C`, `D`, and the callee's other registers.
///
/// Domain: any memory contents (nothing reaches arithmetic but the
/// divisions of converted halfwords by 100).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002ECA0(rdram: *mut u8, ctx: *mut RecompContext) {
    // Bits 1..13: (register loaded, record offset, register holding v0 |
    // bit, bit), in the C's order and registers.
    const BUTTONS: [(usize, i32, usize, u64); 13] = [
        (T7, 0x12, T8, 0x2),
        (T9, 0x7, T6, 0x4),
        (T7, 0x6, T8, 0x8),
        (T9, 0x5, T6, 0x10),
        (T7, 0x4, T8, 0x20),
        (T9, 0x9, T6, 0x40),
        (T7, 0x8, T8, 0x80),
        (T9, 0x11, T6, 0x100),
        (T7, 0x10, T8, 0x200),
        (T9, 0xF, T6, 0x400),
        (T7, 0xE, T8, 0x800),
        (T9, 0xD, T6, 0x1000),
        (T7, 0xC, T8, 0x2000),
    ];
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x30i64) as u64);
    sw(m, g[SP], 0x2C, g[RA]);
    sd(m, g[SP], 0x20, ctx.fpr[24].u64);
    sd(m, g[SP], 0x18, ctx.fpr[22].u64);
    sd(m, g[SP], 0x10, ctx.fpr[20].u64);
    call(imports::func_8002EA28, m, ctx);
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x42C8_0000);
    f[24].set_u32l(g[AT] as u32);
    g[AT] = li(0x800B_0000);
    f[18].u64 = ld(m, g[AT], -0x6070);
    f[16].u64 = ld(m, g[AT], -0x6068);
    f[14].u64 = ld(m, g[AT], -0x6060);
    f[12].u64 = ld(m, g[AT], -0x6058);
    g[A1] = li(PAD_RECORDS);
    g[A2] = li(BUTTON_WORDS);
    g[T0] = li(BUTTON_WORDS + 0x10);
    g[T1] = li(BUTTON_WORDS + 0x20);
    g[T2] = li(BUTTON_WORDS + 0x30);
    g[A3] = li(BUTTON_WORDS + 0x40);
    g[RA] = li(0x8_0000);
    g[T5] = li(0x4_0000);
    g[T4] = li(0x2_0000);
    g[T3] = li(0x1_0000);
    loop {
        g[T6] = lbu(m, g[A1], 0x13);
        g[V0] = 0;
        if g[T6] != 0 {
            g[V0] = 1;
        }
        for &(r, off, t, bit) in &BUTTONS {
            g[r] = lbu(m, g[A1], off);
            g[t] = g[V0] | bit;
            if g[r] != 0 {
                g[V0] = g[t];
            }
        }
        g[T9] = lh(m, g[A1], 2);
        g[T6] = g[V0] | 0x4000;
        f[4].set_u32l(g[T9] as u32);
        f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
        f[22].set_fl(f[6].fl() / f[24].fl());
        f[2].set_d(f64::from(f[22].fl()));
        let (a, b, c, d, y) = (f[12].d(), f[14].d(), f[16].d(), f[18].d(), f[2].d());
        if a < y {
            g[V0] = g[T6];
        }
        g[T7] = g[V0] | 0x8000;
        if y < b {
            g[V0] = g[T7];
        }
        g[T8] = lh(m, g[A1], 0);
        g[A1] = addu(g[A1], 0x18);
        f[8].set_u32l(g[T8] as u32);
        f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
        f[20].set_fl(f[10].fl() / f[24].fl());
        f[0].set_d(f64::from(f[20].fl()));
        let x = f[0].d();
        if x < b {
            g[V0] |= g[T3];
        }
        if a < x {
            g[V0] |= g[T4];
        }
        if c < x && x < d {
            g[V0] |= g[T5];
        }
        if c < y && y < d {
            g[V0] |= g[RA];
        }
        if x <= c {
            g[AT] = li(0x10_0000);
            g[T9] = g[V0] | g[AT];
            if b <= x {
                g[V0] = g[T9];
            }
        }
        if d <= x {
            g[AT] = li(0x20_0000);
            g[T6] = g[V0] | g[AT];
            if x <= a {
                g[V0] = g[T6];
            }
        }
        if d <= y {
            g[AT] = li(0x40_0000);
            g[T7] = g[V0] | g[AT];
            if y <= a {
                g[V0] = g[T7];
            }
        }
        g[T7] = li(BUTTON_WORDS + 0x50);
        if y <= c {
            g[AT] = li(0x80_0000);
            g[T8] = g[V0] | g[AT];
            if b <= y {
                g[V0] = g[T8];
            }
        }
        // Store: pressed, released, held, x, y.
        g[V1] = lw(m, g[A2], 0);
        g[A3] = addu(g[A3], 4);
        g[A2] = addu(g[A2], 4);
        g[A0] = g[V1] ^ g[V0];
        g[T9] = g[A0] & g[V0];
        g[T6] = g[A0] & g[V1];
        g[T0] = addu(g[T0], 4);
        g[T1] = addu(g[T1], 4);
        g[T2] = addu(g[T2], 4);
        sw(m, g[T0], -4, g[T9]);
        sw(m, g[T1], -4, g[T6]);
        sw(m, g[A2], -4, g[V0]);
        sw(m, g[T2], -4, u64::from(f[20].u32l()));
        sw(m, g[A3], -4, u64::from(f[22].u32l()));
        if g[A3] == g[T7] {
            break;
        }
    }
    g[RA] = lw(m, g[SP], 0x2C);
    f[20].u64 = ld(m, g[SP], 0x10);
    f[22].u64 = ld(m, g[SP], 0x18);
    f[24].u64 = ld(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x30);
}
