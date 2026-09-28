//! Render state and display-list writers (NOTES.md, "Depth-0 leaves ported
//! in session 7"): appends to the display lists behind `[0x801217B0]` and
//! `[0x80112C90]`, the render modes at `0x800A3DA0` switched by tag, the
//! lights at `0x800A3DB0`, the framebuffer addresses, and the RSP task.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use n64mem::Mem;
use crate::recomp::{addu, enter, lbu, lh, li, lw, multu, reg::*, sb, sll, slt, sltu, sra, subu, sw, RecompContext};

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
