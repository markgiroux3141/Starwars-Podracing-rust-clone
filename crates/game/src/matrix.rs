//! The matrix stack: 33 row-major 4x4 float matrices at `0x800AEC80`, the
//! top selected by the depth word at `0x8009A29C` (NOTES.md, "Depth-0 leaves
//! ported in session 8"). Reset, push (multiply into the top), get and pop.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]

use crate::recomp::{addu, enter, li, lw, reg::*, sll, slt, sw, RecompContext};

/// The stack of 4x4 float matrices (64 bytes each, row-major): 32 levels
/// above the base, so 33 matrices from `0x800AEC80`.
pub const MATRIX_STACK: u32 = 0x800A_EC80;

/// The index of the top matrix of [`MATRIX_STACK`] (a word).
pub const MATRIX_DEPTH: u32 = 0x8009_A29C;

/// `func_8000550C()` (matrix stack reset): depth 0 and the base matrix =
/// identity. Stores the ones (`+0x3C`, `+0x28`, `+0x14`, `+0`) before the
/// zeros.
///
/// Leaves `f0 = 1.0`, `f2 = 0.0`, `v0 = MATRIX_STACK`, `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000550C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x3F80_0000);
    f[0].set_u32l(g[AT] as u32);
    g[V0] = li(MATRIX_STACK);
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], -0x5D64, 0);
    f[2].set_u32l(0);
    for off in [0x3C, 0x28, 0x14, 0] {
        sw(m, g[V0], off, u64::from(f[0].u32l()));
    }
    for off in [4, 8, 0xC, 0x10, 0x18, 0x1C, 0x20, 0x24, 0x2C, 0x30, 0x34, 0x38] {
        sw(m, g[V0], off, u64::from(f[2].u32l()));
    }
}

/// The FPRs one entry of [`func_8000556C`] uses, `[a, b, d, e, f]` (the
/// third load register is always `f10`): they rotate with period 5.
const PUSH_REGS: [[usize; 5]; 5] = [[4, 6, 16, 8, 18], [16, 18, 8, 6, 4], [8, 4, 6, 18, 16], [6, 16, 18, 4, 8], [18, 8, 4, 16, 6]];

/// `func_8000556C(M)` (matrix stack push, **guess** at the use: multiply
/// into the current transform): if the depth is below 32 (signed), depth
/// += 1 (stored first) and the new top = `M * old top`: entry `(i, j)` =
/// `M[i][3]*T[3][j] + ((T[0][j]*M[i][0] + T[1][j]*M[i][1]) + T[2][j]*M[i][2])`,
/// sums in that order, entries in row-major order, each stored before the
/// next is computed (so an `M` overlapping the new top changes later
/// entries). At depth 32 or more it does nothing but load.
///
/// Leaves `a3 = MATRIX_DEPTH`, `v0` = the old depth, `a2 = M`, `t6` =
/// depth + 1, `at` = the bound test, `t9 = MATRIX_STACK` (or `0x800B0000`
/// when full); on a push `t8 = 64 * (depth + 1)`, `a0` = the new top, `a1`
/// = the old, and the FPRs of the last entry (`f4`..`f18`).
///
/// Domain: canonical `M` with 64 bytes; on a push, a depth whose two
/// levels lie in RDRAM (the signed bound lets negative depths through), and
/// the entries of `M` and the top, and every product and partial sum, not
/// NaN. (The last row
/// loads `M` before `T`; loads have no effects, so the order is not
/// reproduced.)
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000556C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[A3] = li(MATRIX_DEPTH);
    g[V0] = lw(m, g[A3], 0);
    g[A2] = g[A0];
    g[T9] = li(0x800B_0000);
    g[AT] = slt(g[V0], 0x20);
    g[T6] = addu(g[V0], 1);
    if g[AT] == 0 {
        return;
    }
    g[T8] = sll(g[T6], 6);
    g[T9] = li(MATRIX_STACK);
    sw(m, g[A3], 0, g[T6]);
    g[A0] = addu(g[T8], g[T9]);
    g[A1] = addu(g[A0], (-0x40i64) as u64);
    for k in 0..16 {
        let (i, j) = ((k / 4) as i32, (k % 4) as i32);
        let [a, b, d, e, fr] = PUSH_REGS[k % 5];
        let t = |l: i32| 4 * j + 0x10 * l; // T[l][j] at a1
        let mi = |l: i32| 0x10 * i + 4 * l; // M[i][l] at a2
        f[a].set_u32l(lw(m, g[A1], t(0)) as u32);
        f[b].set_u32l(lw(m, g[A2], mi(0)) as u32);
        f[10].set_u32l(lw(m, g[A1], t(1)) as u32);
        f[d].set_u32l(lw(m, g[A2], mi(1)) as u32);
        f[e].set_fl(f[a].fl() * f[b].fl());
        f[b].set_u32l(lw(m, g[A1], t(2)) as u32);
        f[fr].set_fl(f[10].fl() * f[d].fl());
        f[10].set_u32l(lw(m, g[A2], mi(2)) as u32);
        f[d].set_fl(f[b].fl() * f[10].fl());
        f[b].set_u32l(lw(m, g[A1], t(3)) as u32);
        f[a].set_fl(f[e].fl() + f[fr].fl());
        f[fr].set_u32l(lw(m, g[A2], mi(3)) as u32);
        f[10].set_fl(f[fr].fl() * f[b].fl());
        f[e].set_fl(f[a].fl() + f[d].fl());
        f[a].set_fl(f[10].fl() + f[e].fl());
        sw(m, g[A0], 4 * k as i32, u64::from(f[a].u32l()));
    }
}

/// The temporaries [`func_800059A8`] cycles through, three per word.
const COPY_TEMPS: [usize; 10] = [T6, T7, T8, T9, T0, T1, T2, T3, T4, T5];

/// The FPRs it cycles through, one per word.
const COPY_FPRS: [usize; 6] = [4, 6, 8, 10, 16, 18];

/// `func_800059A8(out)` (matrix stack get): copy the top matrix to `out`,
/// word by word. QUIRK: the depth is re-read before every word, so an `out`
/// overlapping [`MATRIX_DEPTH`] (or a depth outside 0..=32) changes which
/// matrix later words come from.
///
/// Leaves `v1 = MATRIX_DEPTH`, `v0 = MATRIX_STACK`, and in `t0`..`t9` and
/// `f4`..`f18` what the last words used (word `k` uses three temporaries
/// from `t6` on, cycling `t6..t9, t0..t5`, and one FPR cycling `f4 f6 f8
/// f10 f16 f18`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800059A8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V1] = li(MATRIX_DEPTH);
    g[V0] = li(MATRIX_STACK);
    for k in 0..16 {
        let (depth, off, at) = (COPY_TEMPS[3 * k % 10], COPY_TEMPS[(3 * k + 1) % 10], COPY_TEMPS[(3 * k + 2) % 10]);
        let fr = COPY_FPRS[k % 6];
        g[depth] = lw(m, g[V1], 0);
        g[off] = sll(g[depth], 6);
        g[at] = addu(g[V0], g[off]);
        f[fr].set_u32l(lw(m, g[at], 4 * k as i32) as u32);
        sw(m, g[A0], 4 * k as i32, u64::from(f[fr].u32l()));
    }
}

/// `func_80005AFC` (matrix stack pop): decrement [`MATRIX_DEPTH`] if it is
/// positive (signed).
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
