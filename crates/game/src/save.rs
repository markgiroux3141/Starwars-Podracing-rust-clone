//! Save data and what the game derives from it (**guess** at the meaning;
//! NOTES.md, "Depth-0 leaves ported in session 7"): the 0x3F0-byte block at
//! `0x80113680` and its 0x2C-byte profile records, the racer list and track
//! selection built from them, and the CRC-32 table (probably for its
//! checksum).

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use n64mem::Mem;
use crate::imports;
use crate::recomp::{addu, call, enter, lb, lbu, li, lw, multu, reg::*, sb, sh, sll, sllv, slt, srl, subu, sw, RecompContext};

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
/// nonzero, else `0x80113680` (see [`func_8002DAD0`](crate::misc::func_8002DAD0)). Circuits: 3, or 2 if
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

/// `func_80024874(r)` (count the unlocked tracks before the current one,
/// **guess**): `[0x8011A270] = 0`, then for `i` from 0 while `i < N[c]`
/// (`c` = the signed byte `r + 0x5E`, re-read each step, `N` the bytes at
/// `0x800A21B4`): stop if the signed byte `r + 0x5D` equals the word
/// `T[c][i]` (`T` = 28-byte rows at `0x800A22E8`); else if
/// [`func_8002DAD0`]`(r, c, i & 0xFF)` (the unlock bit), `[0x8011A270] +=
/// 1`.
///
/// Frame (`sp - 0x30`): `ra`, `s5`..`s0` at `+0x2C..+0x14`, restored (`s3 =
/// 0x8011A240`, `s5 = 0x800A21B4`, `s4 = 0x800A22E8`, `s1 = r`, `s0 = i`,
/// `s2 = 4i`). Leaves `a1 = c`, `t*`, `at` and the callee's registers.
///
/// Domain: the tables' entries used in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80024874(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_80024934: {
        'b_80024930: {
            g[SP] = addu(g[SP], (-0x30i64) as u64);
            sw(m, g[SP], 0x20, g[S3]);
            g[S3] = li(0x8011_A240);
            sw(m, g[SP], 0x2C, g[RA]);
            sw(m, g[SP], 0x28, g[S5]);
            sw(m, g[SP], 0x24, g[S4]);
            sw(m, g[SP], 0x1C, g[S2]);
            sw(m, g[SP], 0x18, g[S1]);
            sw(m, g[SP], 0x14, g[S0]);
            sw(m, g[S3], 0x30, 0);
            g[A1] = lb(m, g[A0], 0x5E);
            g[S5] = li(0x800A_21B4);
            g[T6] = addu(g[S5], g[A1]);
            g[T7] = lbu(m, g[T6], 0);
            g[S1] = g[A0];
            g[S0] = 0;
            g[S2] = 0;
            if (g[T7] as i64) > 0 {
                g[S4] = li(0x800A_22E8);
                g[T9] = sll(g[A1], 3);
                loop {
                    let g = &mut ctx.gpr;
                    g[T9] = subu(g[T9], g[A1]);
                    g[T9] = sll(g[T9], 2);
                    g[T0] = addu(g[S4], g[T9]);
                    g[T1] = addu(g[T0], g[S2]);
                    g[T2] = lw(m, g[T1], 0);
                    g[T8] = lb(m, g[S1], 0x5D);
                    g[A0] = g[S1];
                    if g[T8] == g[T2] {
                        break;
                    }
                    g[A2] = g[S0] & 0xFF;
                    call(imports::func_8002DAD0, m, ctx);
                    let g = &mut ctx.gpr;
                    if g[V0] == 0 {
                        g[A1] = lb(m, g[S1], 0x5E);
                    } else {
                        g[T3] = lw(m, g[S3], 0x30);
                        g[T4] = addu(g[T3], 1);
                        sw(m, g[S3], 0x30, g[T4]);
                        g[A1] = lb(m, g[S1], 0x5E);
                    }
                    g[S0] = addu(g[S0], 1);
                    g[S2] = addu(g[S2], 4);
                    g[T5] = addu(g[S5], g[A1]);
                    g[T6] = lbu(m, g[T5], 0);
                    g[AT] = slt(g[S0], g[T6]);
                    if g[AT] == 0 {
                        break 'b_80024930;
                    }
                    g[T9] = sll(g[A1], 3);
                }
                let g = &mut ctx.gpr;
                g[RA] = lw(m, g[SP], 0x2C);
                break 'b_80024934;
            }
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x2C);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[S2] = lw(m, g[SP], 0x1C);
    g[S3] = lw(m, g[SP], 0x20);
    g[S4] = lw(m, g[SP], 0x24);
    g[S5] = lw(m, g[SP], 0x28);
    g[SP] = addu(g[SP], 0x30);
}

/// `func_80028070()` (reset every profile record, **guess**):
/// [`func_80029A3C`]`(0, i)` for `i` in `0..12` (the working copies), then
/// `(1, i)` for `i` in `0..4` (the saved ones).
///
/// Frame (`sp - 0x20`): `ra`, `s1`, `s0` at `+0x1C`, `+0x18`, `+0x14`,
/// restored; `a0` spilled to its home slot `sp + 0`. Leaves the callee's
/// registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80028070(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x14, g[S0]);
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x18, g[S1]);
    sw(m, g[SP], 0x20, g[A0]);
    g[S0] = 0;
    g[A0] = 0;
    loop {
        let g = &mut ctx.gpr;
        g[A1] = g[S0];
        call(imports::func_80029A3C, m, ctx);
        let g = &mut ctx.gpr;
        g[S0] = addu(g[S0], 1);
        g[AT] = slt(g[S0], 0xC);
        if g[AT] == 0 {
            break;
        }
        g[A0] = 0;
    }
    let g = &mut ctx.gpr;
    g[S0] = 0;
    g[S1] = 4;
    g[A0] = 1;
    loop {
        let g = &mut ctx.gpr;
        g[A1] = g[S0];
        call(imports::func_80029A3C, m, ctx);
        let g = &mut ctx.gpr;
        g[S0] = addu(g[S0], 1);
        if g[S0] == g[S1] {
            break;
        }
        g[A0] = 1;
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_800281F0(p)`: build the list of available racers (**guess**; 23
/// entries) at `0x800D6CD8`: the unlock mask `[0x80113E74 + 0x2C * (s8) [p
/// + 0x6F]]` (a word of the current profile record, [`func_80039914`]) ORed
/// with `0x22E01`, always available; each set bit `i < 23`, in order, gives
/// an 8-byte entry `{+0: i, +4: 0xFF, +5: 0}`, and the rest of the 23 get
/// `{-1, 0xFF, 0}`. The number available goes to `[0x8011A26C]` (the count
/// [`func_80047920`](crate::misc::func_80047920) reads). The fill is unrolled: `(23 - n) & 3` single
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

/// `func_80029A3C(which, i)` (profile reset, **guess**): resets the
/// 0x2C-byte profile record `i`, the working copy at `0x80113E60 + 44i`
/// for `which == 0` or the saved one at `0x80113694 + 44i` (in the save
/// block at `0x80113680`) for `which == 1`; any other `which` does
/// nothing. The record gets: bytes `+3`, `+4`, `+6` = 0; `+5` = 0xFF (for
/// the saved one: the low byte of `i`); the word `+0x18` = 400; byte `+0x1C`
/// = 1; the word `+0x14` = 0x22E01 (the six always-unlocked racers,
/// NOTES.md "Racers and tracks"); `+7` = 0xFF; the halfwords `+0xC..+0x12`
/// = 0; bytes `+8..+0xA` = 1, `+0xB` = 0; `+0x1D..+0x23` = 0 and
/// `+0x24..+0x2A` = 0xFF (in pairs); `+0..+2` = 0; and for the saved one
/// also `+0x2B` = 0. Stores in the C's order; `i` is 32-bit (`44i` as
/// shifts and subtractions).
///
/// Leaves `a3 = 1`, `v0` = the record base as the C addresses it (`- 0x14`
/// for the saved one), `v1` = the last loop's count, `a0 = 3`, `a1`, `a2`
/// past the loops, and `t0`..`t9` the constants and addresses.
///
/// Domain: `i` with the record in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80029A3C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A3] = 1;
    if g[A0] == 0 {
        // The working copy: v0 = a0 = 0x80113E60 + 44i.
        g[T6] = sll(g[A1], 2);
        g[T6] = subu(g[T6], g[A1]);
        g[T6] = sll(g[T6], 2);
        g[T6] = subu(g[T6], g[A1]);
        g[T7] = li(0x8011_3E60);
        g[T6] = sll(g[T6], 2);
        g[V0] = addu(g[T6], g[T7]);
        g[T2] = sll(g[A1], 2);
        g[T2] = subu(g[T2], g[A1]);
        g[T2] = sll(g[T2], 2);
        g[T2] = subu(g[T2], g[A1]);
        g[T3] = li(0x8011_0000);
        g[T9] = li(0x2_0000);
        g[T3] = addu(g[T3], 0x3E60);
        g[T2] = sll(g[T2], 2);
        g[T0] = 0xFF;
        g[T8] = 0x190;
        g[T9] = g[T9] | 0x2E01;
        g[T1] = u64::MAX;
        g[A0] = addu(g[T2], g[T3]);
        sb(m, g[V0], 3, 0);
        sb(m, g[V0], 4, 0);
        sb(m, g[V0], 5, g[T0]);
        sw(m, g[V0], 0x18, g[T8]);
        sb(m, g[V0], 0x1C, g[A3]);
        sb(m, g[V0], 6, 0);
        sw(m, g[V0], 0x14, g[T9]);
        sb(m, g[V0], 7, g[T1]);
        g[A2] = g[A0];
        g[V1] = 0;
        loop {
            g[V1] = addu(g[V1], 1);
            g[AT] = slt(g[V1], 4);
            sh(m, g[A2], 0xC, 0);
            g[A2] = addu(g[A2], 2);
            if g[AT] == 0 {
                break;
            }
        }
        sb(m, g[V0], 8, g[A3]);
        sb(m, g[V0], 9, g[A3]);
        sb(m, g[V0], 0xA, g[A3]);
        sb(m, g[V0], 0xB, 0);
        g[V1] = 0;
        g[A1] = g[A0];
        loop {
            g[V1] = addu(g[V1], 1);
            g[AT] = slt(g[V1], 7);
            g[A1] = addu(g[A1], 1);
            sb(m, g[A1], 0x1C, 0);
            sb(m, g[A1], 0x23, g[T0]);
            if g[AT] == 0 {
                break;
            }
        }
        g[A1] = g[A0];
        g[A0] = 3;
        g[V1] = 0;
        loop {
            g[V1] = addu(g[V1], 1);
            sb(m, g[A1], 0, 0);
            g[A1] = addu(g[A1], 1);
            if g[V1] == g[A0] {
                break;
            }
        }
    } else {
        g[AT] = 1;
        g[A3] = 1;
        if g[A0] == g[AT] {
            // The saved copy: v0 = a0 = 0x80113680 + 44i, offsets + 0x14.
            g[T4] = sll(g[A1], 2);
            g[T4] = subu(g[T4], g[A1]);
            g[T4] = sll(g[T4], 2);
            g[T4] = subu(g[T4], g[A1]);
            g[T5] = li(0x8011_3680);
            g[T4] = sll(g[T4], 2);
            g[V0] = addu(g[T4], g[T5]);
            g[T9] = sll(g[A1], 2);
            g[T9] = subu(g[T9], g[A1]);
            g[T9] = sll(g[T9], 2);
            g[T9] = subu(g[T9], g[A1]);
            g[T1] = li(0x8011_0000);
            g[T7] = li(0x2_0000);
            g[T1] = addu(g[T1], 0x3680);
            g[T9] = sll(g[T9], 2);
            g[T6] = 0x190;
            g[T7] = g[T7] | 0x2E01;
            g[T8] = u64::MAX;
            g[A0] = addu(g[T9], g[T1]);
            sb(m, g[V0], 0x17, 0);
            sb(m, g[V0], 0x18, 0);
            sb(m, g[V0], 0x19, g[A1]);
            sw(m, g[V0], 0x2C, g[T6]);
            sb(m, g[V0], 0x30, g[A3]);
            sb(m, g[V0], 0x1A, 0);
            sw(m, g[V0], 0x28, g[T7]);
            sb(m, g[V0], 0x1B, g[T8]);
            g[A2] = g[A0];
            g[V1] = 0;
            loop {
                g[V1] = addu(g[V1], 1);
                g[AT] = slt(g[V1], 4);
                sh(m, g[A2], 0x20, 0);
                g[A2] = addu(g[A2], 2);
                if g[AT] == 0 {
                    break;
                }
            }
            sb(m, g[V0], 0x1C, g[A3]);
            sb(m, g[V0], 0x1D, g[A3]);
            sb(m, g[V0], 0x1E, g[A3]);
            sb(m, g[V0], 0x1F, 0);
            g[V1] = 0;
            g[A1] = g[A0];
            g[T0] = 0xFF;
            loop {
                g[V1] = addu(g[V1], 1);
                g[AT] = slt(g[V1], 7);
                g[A1] = addu(g[A1], 1);
                sb(m, g[A1], 0x30, 0);
                sb(m, g[A1], 0x37, g[T0]);
                if g[AT] == 0 {
                    break;
                }
            }
            g[A1] = g[A0];
            g[A0] = 3;
            g[V1] = 0;
            loop {
                g[V1] = addu(g[V1], 1);
                sb(m, g[A1], 0x14, 0);
                g[A1] = addu(g[A1], 1);
                if g[V1] == g[A0] {
                    break;
                }
            }
            sb(m, g[V0], 0x3F, 0);
        }
    }
}

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

/// `func_80039178(p, n)` (crc32, MSB first): builds the table at
/// `0x80114070` first if its entry 1 is 0 ([`func_800390C0`]); then `c =
/// -1` and, for each byte `b` of `p[0..n]` (none for `n <= 0`, signed),
/// `c = T[b ^ (c >> 24)] ^ (c << 8)`; returns `!c`.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`, `p` spilled to its slot `+0x18`,
/// `n` to `+0x1C` when the table is built. Leaves `a2` = the table, `a1 =
/// 0` (or `n` if `n <= 0`), `v1 = c`, from the last byte `t7` (the byte),
/// `t8 = c >> 24`, `t9`, `t0` (the entry's offset), `t1`, `t2` (the entry),
/// `t3 = c << 8`, and the callee's registers if it ran.
///
/// Domain: the table's entry 1 and the bytes in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80039178(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A2] = li(0x8011_4070);
    g[T6] = lw(m, g[A2], 4);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x18, g[A0]);
    if g[T6] == 0 {
        sw(m, g[SP], 0x1C, g[A1]);
        call(imports::func_800390C0, m, ctx);
        let g = &mut ctx.gpr;
        g[A2] = li(0x8011_4070);
        g[A1] = lw(m, g[SP], 0x1C);
    }
    let g = &mut ctx.gpr;
    g[V1] = u64::MAX;
    g[V0] = lw(m, g[SP], 0x18);
    if (g[A1] as i64) > 0 {
        loop {
            g[T7] = lbu(m, g[V0], 0);
            g[T8] = srl(g[V1], 24);
            g[A1] = addu(g[A1], u64::MAX);
            g[T9] = g[T7] ^ g[T8];
            g[T0] = sll(g[T9], 2);
            g[T1] = addu(g[A2], g[T0]);
            g[T2] = lw(m, g[T1], 0);
            g[T3] = sll(g[V1], 8);
            g[V0] = addu(g[V0], 1);
            g[V1] = g[T2] ^ g[T3];
            if (g[A1] as i64) <= 0 {
                break;
            }
        }
    }
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
    g[V0] = !g[V1];
}

/// `func_8003931C(p)` (a save block's checksum, **guess**): `v0` =
/// [`func_80039178`]`(p + 4, 0x3EC)`, the crc32 of the 0x3EC bytes after
/// the first word.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves the callee's registers.
///
/// Domain: the callee's.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003931C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    g[A0] = addu(g[A0], 4);
    g[A1] = 0x3EC;
    call(imports::func_80039178, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_8003960C()`: copy the 0x3F0-byte block at `0x80113680` (the save
/// data, **guess**; its flags at `+8` are read by [`func_8001F464`] and
/// [`func_8002DC7C`](crate::misc::func_8002DC7C)) to `0x80113A70`. Leaves `t7 = t0 = 0x80113A70`, `t6
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
