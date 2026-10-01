//! Pure libultra functions (NOTES.md, "The OS boundary"): `sinf` and
//! `cosf` (single precision in and out, double precision inside, with the
//! constants read from their tables in the data segment; the game's own
//! maths, so ports never substitute `std`, SPEC §5.4), the 64-bit helpers
//! of `ll.c`, and `__osDisableInt`/`__osRestoreInt`, which reach CP0 Status
//! only through the runtime's hooks (NOTES.md, "CP0 Status"). Also parts of
//! libaudio: the heap, list links, and the event queue's flushes, which
//! mask interrupts through `osSetIntMask` (a contract double in the tests,
//! NOTES.md "Hardware doubles").

// Ports keep N64Recomp's names (func_8008A8C0), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use crate::recomp::{addu, call, ddiv, ddivu, dmultu, enter, fpu, ld, lh, li, lw, multu, reg::*, s32, sd, sh, sll, slt, sltu, sra, subu, sw, RecompContext};
use n64mem::Mem;

/// `func_80087CB0(x)`: `sqrtf`: `f0 = sqrt(f12)`, the single-precision
/// square root (`sqrt.s`; the host's is the same correctly rounded
/// operation). Domain: `x` not NaN (guarded).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80087CB0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (_mem, ctx) = enter(rdram, ctx);
    ctx.fpr[0].set_fl(ctx.fpr[12].fl().sqrt());
}

/// `func_80087FC0(file, line, heap, num, size)` (libultra's
/// `alHeapDBAlloc`; `size` the stack argument `sp + 0x10`): with the heap
/// record `{base, cur, len}` at `heap` and `n = (num * size + 15) & ~15`
/// (32-bit, the low word of `multu`), if `cur + n <= base + len` (unsigned)
/// returns `cur` in `v0` and advances `cur` by `n`; otherwise returns 0.
/// `file` and `line` (a debug build's caller) are only spilled to their
/// home slots `sp + 0`, `sp + 4`.
///
/// Leaves `t6 = size`, `t8 = base`, `t9 = len`, `a0 = cur`, `at` = the
/// compare, `t0 = base + len`, `t7 = n`, `t1 = t2 = cur + n`, `v1` = the
/// result.
///
/// Domain: canonical `heap` and `sp` with their words in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80087FC0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = lw(m, g[SP], 0x10);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[SP], 4, g[A1]);
    let (lo, _) = multu(g[A3], g[T6]);
    g[T9] = lw(m, g[A2], 8);
    g[T8] = lw(m, g[A2], 0);
    g[A0] = lw(m, g[A2], 4);
    g[AT] = (-0x10i64) as u64;
    g[T0] = addu(g[T8], g[T9]);
    g[V1] = 0;
    g[V0] = lo;
    g[V0] = addu(g[V0], 0xF);
    g[T7] = g[V0] & g[AT];
    g[T1] = addu(g[A0], g[T7]);
    g[AT] = sltu(g[T0], g[T1]);
    g[T2] = addu(g[A0], g[T7]);
    if g[AT] == 0 {
        g[V1] = g[A0];
        sw(m, g[A2], 4, g[T2]);
    }
    g[V0] = g[V1];
}

/// `func_80088020(ln)` (libultra's `alUnlink`): unlink the doubly linked
/// node `{next, prev}` at `ln`: `next->prev = ln->prev` if `next` is set,
/// then `prev->next = ln->next` if `prev` is set (each field re-read from
/// `ln` when it is stored). `ln` itself keeps its links.
///
/// Leaves `v0` = `prev` (re-read), `t6`, `t7` the words copied.
///
/// Domain: canonical pointers with the words in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80088020(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = lw(m, g[A0], 0);
    if g[V0] != 0 {
        g[T6] = lw(m, g[A0], 4);
        sw(m, g[V0], 4, g[T6]);
    }
    g[V0] = lw(m, g[A0], 4);
    if g[V0] != 0 {
        g[T7] = lw(m, g[A0], 0);
        sw(m, g[V0], 0, g[T7]);
    }
}

/// `func_80088050(ln, to)` (libultra's `alLink`): insert the node `ln`
/// after `to` in a doubly linked list of `{next, prev}`: `ln->prev = to`,
/// `ln->next = to->next`, `to->next->prev = ln` if `to->next` (re-read) is
/// set, then `to->next = ln`, in that order.
///
/// Leaves `t6` = the old `to->next`, `v0` = it re-read.
///
/// Domain: canonical pointers with the words in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80088050(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = lw(m, g[A1], 0);
    sw(m, g[A0], 4, g[A1]);
    sw(m, g[A0], 0, g[T6]);
    g[V0] = lw(m, g[A1], 0);
    if g[V0] != 0 {
        sw(m, g[V0], 4, g[A0]);
    }
    sw(m, g[A1], 0, g[A0]);
}

/// `func_80088110(heap, base, len)` (libultra's `alHeapInit`): the heap
/// record at `heap` becomes `{b, b, len, 0}` (`+0` base, `+4` cur, `+8`
/// len, `+0xC` count), `b` = `base` rounded up to 16 (`base + 16 - (base &
/// 15)` unless it is aligned). Stored base, len, count, then cur (re-read
/// from `+0`).
///
/// Leaves `v1 = 16`, `t6 = base & 15`, `v0 = 16 - t6`, `t7 = base + v0`,
/// `t8 = b`.
///
/// Domain: canonical `heap` with 16 bytes in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80088110(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = 0x10;
    g[T6] = g[A1] & 0xF;
    g[V0] = subu(g[V1], g[T6]);
    g[T7] = addu(g[A1], g[V0]);
    if g[V1] == g[V0] {
        sw(m, g[A0], 0, g[A1]);
    } else {
        sw(m, g[A0], 0, g[T7]);
    }
    g[T8] = lw(m, g[A0], 0);
    sw(m, g[A0], 8, g[A2]);
    sw(m, g[A0], 0xC, 0);
    sw(m, g[A0], 4, g[T8]);
}

/// `func_800883F0()`: returns at once (an empty libultra function; nothing
/// changes).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800883F0(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_800883F8(o, t)` (**guess**: microseconds to samples at the rate
/// `[o + 0x44]`): `v0 = trunc(f32(f64(f32(t) * f32(rate)) / [0x800ADD80] +
/// 0.5)) & ~15` with `t` and the rate converted from words (round to
/// nearest), the constant a double (1e6 in the ROM), the product widened
/// exactly and the sum narrowed to nearest; so the samples rounded to the
/// nearest whole number and then down to a multiple of 16.
///
/// Leaves `t6` = the rate, `at = -16`, `f4`/`f5` = the constant, `f6`/`f7`
/// = 0.5, `f8` = the quotient, `f10` = the sum, `f16` = the conversion,
/// `f18` = the product as a double, `f0` = the narrowed sum, `t8 = v0`.
///
/// Domain: canonical `o`; any `t` and rate (the product of two converted
/// words is finite).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800883F8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = lw(m, g[A0], 0x44);
    f[4].set_u32l(g[A1] as u32);
    g[AT] = li(0x800B_0000);
    f[8].set_u32l(g[T6] as u32);
    f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    f[4].u64 = ld(m, g[AT], -0x2280);
    g[AT] = li(0x3FE0_0000);
    f[6].set_u32h(g[AT] as u32);
    g[AT] = (-0x10i64) as u64;
    f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
    f[16].set_fl(f[6].fl() * f[10].fl());
    f[6].set_u32l(0);
    f[18].set_d(f64::from(f[16].fl()));
    f[8].set_d(f[18].d() / f[4].d());
    f[10].set_d(f[8].d() + f[6].d());
    f[0].set_fl(fpu::cvt_s_d(f[10].d(), fpu::NEAREST));
    f[16].set_u32l(fpu::trunc_w_s(f[0].fl()));
    g[V0] = s32(f[16].u32l());
    g[T8] = g[V0] & g[AT];
    g[V0] = g[T8];
}

/// `func_800884E8(p)` (**guess**: return a parameter record to the audio
/// free list): with `G = [0x800A6990]`, `[p] = [G + 0x2C]`, then `[G +
/// 0x2C] = p` (the reverse of [`func_80088500`]).
///
/// Leaves `v0 = G`, `t6` = the old head.
///
/// Domain: `G` and `p` canonical with the words in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800884E8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x800A_0000);
    g[V0] = lw(m, g[V0], 0x6990);
    g[T6] = lw(m, g[V0], 0x2C);
    sw(m, g[A0], 0, g[T6]);
    sw(m, g[V0], 0x2C, g[A0]);
}

/// `func_80088500()` (**guess**: take a parameter record from the audio
/// free list): with `G = [0x800A6990]` and the head `p = [G + 0x2C]`,
/// returns `p` in `v0` and, if it is set, unlinks it (`[G + 0x2C] = [p]`,
/// `[p] = 0`); 0 for an empty list.
///
/// Leaves `a0 = p`, `t6` = its successor, `v1 = p`.
///
/// Domain: `G` (and a set `p`) canonical with the words in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80088500(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x800A_0000);
    g[V0] = lw(m, g[V0], 0x6990);
    g[V1] = 0;
    g[A0] = lw(m, g[V0], 0x2C);
    if g[A0] != 0 {
        g[T6] = lw(m, g[A0], 0);
        g[V1] = g[A0];
        sw(m, g[V0], 0x2C, g[T6]);
        sw(m, g[A0], 0, 0);
    }
    g[V0] = g[V1];
}

/// `func_80088530()`: returns at once (an empty libultra function).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80088530(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_80088B00(o, v)`: `[o + 0x3C] = s16(v)` (the low halfword,
/// sign-extended to a word); `v` is spilled to its home slot `sp + 4`.
///
/// Leaves `t6 = v << 16`, `t7 = s16(v)`.
///
/// Domain: canonical `o` and `sp` with the words in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80088B00(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A1], 16);
    g[T7] = sra(g[T6], 16);
    sw(m, g[SP], 4, g[A1]);
    sw(m, g[A0], 0x3C, g[T7]);
}

/// `func_80088B70` (by shape a static libaudio helper: flush the events of
/// one key, **guess**: a voice): compiled with IDO's interprocedural
/// register allocation (-O3), it takes its arguments in callee-saved
/// registers, the event queue `q = s2` and the key `s3`, and uses `s0`,
/// `s1`, `s4` without saving them (its caller, `func_80088BFC`, expects
/// that). With interrupts masked (`osSetIntMask(1)`, the old mask kept in
/// `s4` and restored at the end by a second call), for each event item
/// `n` of the list from `[q + 8]` (the allocation list), `next = [n]` read
/// first: if the word `[n + 0x10]` equals `s3` (as a whole register), the
/// next item's delta `[next + 8] += [n + 8]` (if `next` is nonzero),
/// [`func_80088020`]`(n)` (`alUnlink`) and [`func_80088050`]`(n, q)`
/// (`alLink` onto the free list at `q`).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `s0 = 0`, `s1 = 0` (as it
/// came for an empty list), `s4` the old mask, `t6` the last key word,
/// `t7`..`t9` from the
/// last delta, `a0`, `a1` and the callees' registers (the double's).
///
/// Domain: [`crate::imports::func_80090500`]'s (a double); a finite list
/// in RDRAM, the callees' domains.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80088B70(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    g[A0] = 1;
    call(imports::func_80090500, m, ctx);
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[S2], 8);
    g[S4] = g[V0];
    loop {
        let g = &mut ctx.gpr;
        if g[S0] == 0 {
            break;
        }
        g[T6] = lw(m, g[S0], 0x10);
        g[S1] = lw(m, g[S0], 0);
        if g[S3] == g[T6] {
            if g[S1] != 0 {
                g[T7] = lw(m, g[S1], 8);
                g[T8] = lw(m, g[S0], 8);
                g[T9] = addu(g[T7], g[T8]);
                sw(m, g[S1], 8, g[T9]);
            }
            g[A0] = g[S0];
            call(imports::func_80088020, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = g[S0];
            g[A1] = g[S2];
            call(imports::func_80088050, m, ctx);
        }
        let g = &mut ctx.gpr;
        g[S0] = g[S1];
    }
    let g = &mut ctx.gpr;
    g[A0] = g[S4];
    call(imports::func_80090500, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80088BEC()`: returns at once (an empty libultra function).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80088BEC(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_80088BF4()`: returns at once (an empty libultra function).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80088BF4(_rdram: *mut u8, _ctx: *mut RecompContext) {}

/// `func_8008A8C0(x)` = `sinf`: for `xpt = (bits(x) >> 22) & 0x1FF`:
/// - `xpt < 230` (tiny): `x` itself.
/// - `xpt < 255`: `dx + (dx * xsq) * poly(xsq)` in double, `xsq = dx * dx`.
/// - `xpt < 310`: reduce by `n = round(dx * rpi)` (away from zero: `dn +/-
///   0.5`, truncated), `dx -= n * pihi`, `dx -= n * pilo`, the same
///   polynomial, negated if `n` is odd.
/// - otherwise: the word at `0x800AE110` (a quiet NaN) for NaN, else the
///   word at `0x800ADE60` (zero).
///
/// The polynomial is `((P4 xsq + P3) xsq + P2) xsq + P1` with `P1..P4` the
/// doubles at `0x800ADE28..0x800ADE40`; `rpi`, `pihi`, `pilo` at
/// `0x800ADE48..0x800ADE58`. `x` goes through its stack slot `[sp]` (the
/// word read back into `v0` and `f4`). The result's float is rounded to
/// nearest (`cvt.s.d`).
///
/// Domain: any `x`; NaN takes the branch without arithmetic. Leaves `v0` =
/// the bits (or `n`), `v1 = xpt` (or the table), `t6`, `t9` = `n & 1`,
/// `at`, and the double temporaries in `f2`..`f18` as the C leaves them.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008A8C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0, u64::from(ctx.fpr[12].u32l()));
    g[V0] = lw(m, g[SP], 0);
    ctx.fpr[4].set_u32l(lw(m, g[SP], 0) as u32);
    g[V1] = sra(g[V0], 22);
    g[T6] = g[V1] & 0x1FF;
    g[AT] = slt(g[T6], 0xFF);
    g[V1] = g[T6];
    if g[AT] != 0 {
        g[AT] = slt(g[T6], 0xE6);
        ctx.fpr[2].set_d(f64::from(ctx.fpr[4].fl()));
        if g[AT] != 0 {
            // Tiny: x itself.
            ctx.fpr[0].set_u32l(lw(m, g[SP], 0) as u32);
            return;
        }
        let f = &mut ctx.fpr;
        f[12].set_d(f[2].d() * f[2].d()); // xsq
        g[V1] = li(0x800A_DE20);
        f[6].u64 = ld(m, g[V1], 0x20);
        f[10].u64 = ld(m, g[V1], 0x18);
        f[4].u64 = ld(m, g[V1], 0x10);
        f[8].set_d(f[6].d() * f[12].d());
        f[16].set_d(f[8].d() + f[10].d());
        f[10].u64 = ld(m, g[V1], 8);
        f[18].set_d(f[16].d() * f[12].d());
        f[6].set_d(f[18].d() + f[4].d());
        f[8].set_d(f[6].d() * f[12].d());
        f[14].set_d(f[10].d() + f[8].d()); // poly
        f[16].set_d(f[2].d() * f[12].d());
        f[18].set_d(f[16].d() * f[14].d());
        f[4].set_d(f[18].d() + f[2].d());
        f[0].set_fl(fpu::cvt_s_d(f[4].d(), fpu::NEAREST));
        return;
    }
    g[AT] = slt(g[V1], 0x136);
    ctx.fpr[4].set_u32l(lw(m, g[SP], 0) as u32);
    if g[AT] == 0 {
        // NaN or huge: a constant. (`c.eq.s x, x` isn't NaN-checked.)
        let nan = ctx.fpr[4].fl() != ctx.fpr[4].fl();
        g[AT] = li(0x800B_0000);
        let at = if nan { -0x1EF0 } else { -0x21A0 };
        ctx.fpr[0].set_u32l(lw(m, g[AT], at) as u32);
        return;
    }
    // Reduce by n = round(x / pi).
    let f = &mut ctx.fpr;
    f[6].set_u32l(lw(m, g[SP], 0) as u32);
    g[AT] = li(0x800B_0000);
    f[10].u64 = ld(m, g[AT], -0x21B8); // rpi
    f[2].set_d(f64::from(f[6].fl())); // dx
    f[8].set_u32h(0); // f9
    f[0].set_d(f[2].d() * f[10].d()); // dn
    f[8].set_u32l(0);
    g[AT] = li(0x3FE0_0000); // 0.5, high word
    if f[8].d() <= f[0].d() {
        f[16].set_u32h(g[AT] as u32); // f17
        f[16].set_u32l(0);
        f[18].set_d(f[0].d() + f[16].d());
        f[4].set_u32l(fpu::trunc_w_d(f[18].d()));
        g[V0] = s32(f[4].u32l());
    } else {
        f[6].set_u32h(g[AT] as u32); // f7
        f[6].set_u32l(0);
        f[10].set_d(f[0].d() - f[6].d());
        f[8].set_u32l(fpu::trunc_w_d(f[10].d()));
        g[V0] = s32(f[8].u32l());
    }
    f[16].set_u32l(g[V0] as u32);
    g[AT] = li(0x800B_0000);
    f[18].u64 = ld(m, g[AT], -0x21B0); // pihi
    f[0].set_d(f64::from(f[16].u32l() as i32)); // dn = n
    f[6].u64 = ld(m, g[AT], -0x21A8); // pilo
    g[V1] = li(0x800A_DE20);
    f[4].set_d(f[0].d() * f[18].d());
    f[8].u64 = ld(m, g[V1], 0x20);
    f[18].u64 = ld(m, g[V1], 0x18);
    g[T9] = g[V0] & 1;
    f[10].set_d(f[0].d() * f[6].d());
    f[2].set_d(f[2].d() - f[4].d());
    f[2].set_d(f[2].d() - f[10].d());
    f[10].u64 = ld(m, g[V1], 0x10);
    f[12].set_d(f[2].d() * f[2].d()); // xsq
    f[16].set_d(f[8].d() * f[12].d());
    f[4].set_d(f[16].d() + f[18].d());
    f[18].u64 = ld(m, g[V1], 8);
    f[6].set_d(f[4].d() * f[12].d());
    f[8].set_d(f[6].d() + f[10].d());
    f[16].set_d(f[8].d() * f[12].d());
    f[14].set_d(f[18].d() + f[16].d()); // poly
    if g[T9] != 0 {
        f[8].set_d(f[2].d() * f[12].d());
        f[18].set_d(f[8].d() * f[14].d());
        f[16].set_d(f[18].d() + f[2].d());
        f[0].set_fl(fpu::cvt_s_d(f[16].d(), fpu::NEAREST));
        f[0].set_fl(-f[0].fl());
    } else {
        f[4].set_d(f[2].d() * f[12].d());
        f[6].set_d(f[4].d() * f[14].d());
        f[10].set_d(f[6].d() + f[2].d());
        f[0].set_fl(fpu::cvt_s_d(f[10].d(), fpu::NEAREST));
    }
}

/// `func_8008A750(x)` = `cosf`: for `xpt = (bits(x) >> 22) & 0x1FF < 310`:
/// `dx = |x|` (as `0 < x ? x : -x`, so -0.0 for 0), `dn = dx * rpi + 0.5`,
/// `n = round(dn)` (`dn +/- 0.5`, truncated), `dx -= (n - 0.5) * pihi`, `dx
/// -= (n - 0.5) * pilo`, then `dx + (dx * xsq) * poly(xsq)` like
/// [`func_8008A8C0`], negated if `n` is odd. Otherwise the word at
/// `0x800AE110` (a quiet NaN) for NaN, else the word at `0x800ADE10`
/// (zero).
///
/// Its own copy of the constants: `P1..P4` at `0x800ADDD8..0x800ADDF0`,
/// `rpi`, `pihi`, `pilo` at `0x800ADDF8..0x800ADE08`. Domain: any `x`.
/// Leaves `v0` = the bits (or `n`), `t6`/`t7` = `xpt`'s steps, `t0` = `n &
/// 1`, `at`, `v1` = the table, and the double temporaries in `f2`..`f18`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008A750(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0, u64::from(ctx.fpr[12].u32l()));
    g[V0] = lw(m, g[SP], 0);
    ctx.fpr[6].set_u32l(lw(m, g[SP], 0) as u32);
    ctx.fpr[10].set_u32l(lw(m, g[SP], 0) as u32);
    g[T6] = sra(g[V0], 22);
    g[T7] = g[T6] & 0x1FF;
    g[AT] = slt(g[T7], 0x136);
    if g[AT] == 0 {
        // NaN or huge: a constant. (`c.eq.s x, x` isn't NaN-checked.)
        let nan = ctx.fpr[10].fl() != ctx.fpr[10].fl();
        g[AT] = li(0x800B_0000);
        let at = if nan { -0x1EF0 } else { -0x21F0 };
        ctx.fpr[0].set_u32l(lw(m, g[AT], at) as u32);
        return;
    }
    let f = &mut ctx.fpr;
    f[4].set_u32l(0);
    g[AT] = li(0x3FE0_0000); // 0.5, high word
    f[18].set_u32h(g[AT] as u32); // f19
    let positive = f[4].fl() < f[6].fl();
    f[0].set_u32l(lw(m, g[SP], 0) as u32);
    g[AT] = li(0x800B_0000);
    if positive {
        f[0].set_u32l(f[6].u32l());
    } else {
        f[0].set_fl(-f[0].fl());
    }
    f[8].u64 = ld(m, g[AT], -0x2208); // rpi
    f[12].set_d(f64::from(f[0].fl())); // dx = |x|
    f[18].set_u32l(0); // f18 = 0.5
    f[10].set_d(f[12].d() * f[8].d());
    f[4].set_u32h(0); // f5
    f[4].set_u32l(0);
    f[14].set_d(f[10].d() + f[18].d()); // dn
    if f[4].d() <= f[14].d() {
        f[6].set_d(f[14].d() + f[18].d());
        f[8].set_u32l(fpu::trunc_w_d(f[6].d()));
        g[V0] = s32(f[8].u32l());
    } else {
        f[10].set_d(f[14].d() - f[18].d());
        f[4].set_u32l(fpu::trunc_w_d(f[10].d()));
        g[V0] = s32(f[4].u32l());
    }
    f[6].set_u32l(g[V0] as u32);
    g[AT] = li(0x800B_0000);
    f[10].u64 = ld(m, g[AT], -0x2200); // pihi
    f[8].set_d(f64::from(f[6].u32l() as i32));
    f[6].u64 = ld(m, g[AT], -0x21F8); // pilo
    g[V1] = li(0x800A_DDD0);
    f[0].set_d(f[8].d() - f[18].d()); // n - 0.5
    g[T0] = g[V0] & 1;
    f[4].set_d(f[0].d() * f[10].d());
    f[10].u64 = ld(m, g[V1], 0x20);
    f[8].set_d(f[0].d() * f[6].d());
    f[6].u64 = ld(m, g[V1], 0x18);
    f[2].set_d(f[12].d() - f[4].d());
    f[2].set_d(f[2].d() - f[8].d());
    f[14].set_d(f[2].d() * f[2].d()); // xsq
    f[4].set_d(f[10].d() * f[14].d());
    f[8].set_d(f[4].d() + f[6].d());
    f[4].u64 = ld(m, g[V1], 0x10);
    f[10].set_d(f[8].d() * f[14].d());
    f[6].set_d(f[10].d() + f[4].d());
    f[10].u64 = ld(m, g[V1], 8);
    f[8].set_d(f[6].d() * f[14].d());
    f[16].set_d(f[10].d() + f[8].d()); // poly
    if g[T0] != 0 {
        f[8].set_d(f[2].d() * f[14].d());
        f[4].set_d(f[8].d() * f[16].d());
        f[6].set_d(f[4].d() + f[2].d());
        f[0].set_fl(fpu::cvt_s_d(f[6].d(), fpu::NEAREST));
        f[0].set_fl(-f[0].fl());
    } else {
        f[4].set_d(f[2].d() * f[14].d());
        f[6].set_d(f[4].d() * f[16].d());
        f[10].set_d(f[6].d() + f[2].d());
        f[0].set_fl(fpu::cvt_s_d(f[10].d(), fpu::NEAREST));
    }
}

/// The argument handling of libultra's `ll.c` helpers ([`func_8008AAE0`]
/// .. [`func_8008AD74`]): the two 64-bit arguments arrive as register
/// pairs, `a0:a1` and `a2:a3` (high word first), are spilled to their slots
/// `[sp]..[sp + 0xC]` and read back as doublewords, `t6` = the first and
/// `t7` = the second (`t7` loaded first).
///
/// Domain (all of them): canonical `sp` with the slots in RDRAM.
fn ll_args(m: &mut Mem, g: &mut [u64; 32], base: i32) {
    for (k, r) in [A0, A1, A2, A3].into_iter().enumerate() {
        sw(m, g[SP], base + 4 * k as i32, g[r]);
    }
    g[T7] = ld(m, g[SP], base + 8);
    g[T6] = ld(m, g[SP], base);
}

/// The 64-bit result in `v0` returned as the pair `v0:v1` (high, low),
/// each sign-extended.
fn ll_result(g: &mut [u64; 32]) {
    g[V1] = g[V0] << 32;
    g[V1] = ((g[V1] as i64) >> 32) as u64;
    g[V0] = ((g[V0] as i64) >> 32) as u64;
}

/// IDO's check after a 64-bit divide: `break 7` for a zero divisor (never
/// reached: the host's divide faults first in the C, and [`ddivu`]
/// asserts). With `signed`, also `break 6` for `INT64_MIN / -1`, which
/// recomp.h's `DDIV` computes without a fault, so the C reaches it and the
/// runtime traps. Leaves `at = 1` (or `1 << 63` if the divisor is -1).
unsafe fn ll_divide_checks(g: &mut [u64; 32], zero: u32, overflow: Option<u32>) {
    if g[T7] == 0 {
        imports::runtime::do_break(zero);
    }
    let Some(overflow) = overflow else { return };
    g[AT] = u64::MAX;
    let minus_one = g[T7] == g[AT];
    g[AT] = 1;
    if minus_one {
        g[AT] <<= 63;
        if g[T6] == g[AT] {
            imports::runtime::do_break(overflow);
        }
    }
}

/// `func_8008AAE0(a, n)` = `__ull_rshift`: `a >> (n & 63)`, logical.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008AAE0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    ll_args(&mut mem, g, 0);
    g[V0] = g[T6] >> (g[T7] & 63);
    ll_result(g);
}

/// `func_8008AB0C(a, b)` = `__ull_rem`: `a % b`, unsigned. Domain: `b !=
/// 0` (see [`ll_divide_checks`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008AB0C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    ll_args(&mut mem, g, 0);
    let (_, hi) = ddivu(g[T6], g[T7]);
    ll_divide_checks(g, 0x8008_AB30, None);
    g[V0] = hi;
    ll_result(g);
}

/// `func_8008AB48(a, b)` = `__ull_div`: `a / b`, unsigned. Domain: `b !=
/// 0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008AB48(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    ll_args(&mut mem, g, 0);
    let (lo, _) = ddivu(g[T6], g[T7]);
    ll_divide_checks(g, 0x8008_AB6C, None);
    g[V0] = lo;
    ll_result(g);
}

/// `func_8008AB84(a, n)` = `__ll_lshift`: `a << (n & 63)`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008AB84(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    ll_args(&mut mem, g, 0);
    g[V0] = g[T6] << (g[T7] & 63);
    ll_result(g);
}

/// `func_8008ABB0(a, b)` = `__ll_rem`: `a % b` computed **unsigned**, as
/// libultra's `unsigned long long % long long` does (QUIRK of the source,
/// not of the port: `-7 % 2` is not -1). Domain: `b != 0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008ABB0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    ll_args(&mut mem, g, 0);
    let (_, hi) = ddivu(g[T6], g[T7]);
    ll_divide_checks(g, 0x8008_ABD4, None);
    g[V0] = hi;
    ll_result(g);
}

/// `func_8008ABEC(a, b)` = `__ll_div`: `a / b`, signed, truncating. Domain:
/// `b != 0`; `INT64_MIN / -1` reaches `do_break` ([`ll_divide_checks`]).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008ABEC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    ll_args(&mut mem, g, 0);
    let (lo, _) = ddiv(g[T6], g[T7]);
    ll_divide_checks(g, 0x8008_AC14, Some(0x8008_AC30));
    g[V0] = lo;
    ll_result(g);
}

/// `func_8008AC48(a, b)` = `__ll_mul`: the low 64 bits of `a * b`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008AC48(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    ll_args(&mut mem, g, 0);
    let (lo, _) = dmultu(g[T6], g[T7]);
    g[V0] = lo;
    ll_result(g);
}

/// `func_8008AC78(&quot, &rem, a, d)` = `__ull_divremi`: `*quot = a / d`
/// and `*rem = a % d`, unsigned 64-bit, with `a` in `a2:a3` and `d` the
/// fifth argument's low halfword (`[sp + 0x12]`). QUIRK: the halfword is
/// loaded with `lh`, so a divisor with bit 15 set becomes a huge 64-bit
/// one. The dividend and divisor are read again for the remainder.
/// Domain: `d != 0`. Leaves `t6`..`t9`, `t0`..`t5` as the C.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008AC78(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T7] = lh(m, g[SP], 0x12);
    sw(m, g[SP], 8, g[A2]);
    sw(m, g[SP], 0xC, g[A3]);
    g[T6] = ld(m, g[SP], 8);
    g[T8] = g[T7];
    g[T9] = g[T8];
    let (lo, _) = ddivu(g[T6], g[T9]);
    if g[T9] == 0 {
        imports::runtime::do_break(0x8008_AC9C);
    }
    g[T0] = lo;
    sd(m, g[A0], 0, g[T0]);
    g[T2] = lh(m, g[SP], 0x12);
    g[T1] = ld(m, g[SP], 8);
    g[T3] = g[T2];
    g[T4] = g[T3];
    let (_, hi) = ddivu(g[T1], g[T4]);
    if g[T4] == 0 {
        imports::runtime::do_break(0x8008_ACC4);
    }
    g[T5] = hi;
    sd(m, g[A1], 0, g[T5]);
}

/// `func_8008ACD8(a, b)` = `__ll_mod`: the signed remainder moved to the
/// divisor's sign (`rem + b` when `rem` and `b` have opposite signs), so
/// `-7 mod 2 = 1`. It works in an 8-byte frame, the remainder stored and
/// re-read there, and returns it from the frame's two words. Domain: `b !=
/// 0`; `INT64_MIN mod -1` reaches `do_break`.
///
/// Leaves `t6`/`t7` = the arguments, `t8` = the remainder, `t9`..`t3` as
/// the path read them, `at` from the checks, `sp` back sign-extended.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008ACD8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-8i64) as u64);
    ll_args(m, g, 8);
    let (_, hi) = ddiv(g[T6], g[T7]);
    ll_divide_checks(g, 0x8008_AD04, Some(0x8008_AD20));
    g[T8] = hi;
    sd(m, g[SP], 0, g[T8]);
    let adjust = if (g[T8] as i64) < 0 && (g[T7] as i64) > 0 {
        true
    } else {
        g[T9] = ld(m, g[SP], 0);
        if (g[T9] as i64) > 0 {
            g[T0] = ld(m, g[SP], 0x10);
            (g[T0] as i64) < 0
        } else {
            false
        }
    };
    if adjust {
        g[T1] = ld(m, g[SP], 0);
        g[T2] = ld(m, g[SP], 0x10);
        g[T3] = g[T1].wrapping_add(g[T2]);
        sd(m, g[SP], 0, g[T3]);
    }
    g[V0] = lw(m, g[SP], 0);
    g[V1] = lw(m, g[SP], 4);
    g[SP] = addu(g[SP], 8);
}

/// `func_8008AD74(a, n)` = `__ll_rshift`: `a >> (n & 63)`, arithmetic.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008AD74(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    ll_args(&mut mem, g, 0);
    g[V0] = ((g[T6] as i64) >> (g[T7] & 63)) as u64;
    ll_result(g);
}

/// `__osDisableInt` (`func_8008CA80`): clear the interrupt-enable bit (IE,
/// bit 0) of CP0 Status and return its old value (0 or 1) in `v0`. Status
/// is read and written through the runtime
/// ([`imports::runtime::cop0_status_read`] and `cop0_status_write`, as the
/// C does; NOTES.md, "CP0 Status").
///
/// Leaves `t0` = the old Status (sign-extended), `at = -2`, `t1 = t0 & -2`
/// (the value written).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008CA80(rdram: *mut u8, ctx: *mut RecompContext) {
    let (_mem, ctx) = enter(rdram, ctx);
    let status = imports::runtime::cop0_status_read(ctx);
    let g = &mut ctx.gpr;
    g[T0] = status;
    g[AT] = (-2i64) as u64;
    g[T1] = g[T0] & g[AT];
    let disabled = g[T1];
    imports::runtime::cop0_status_write(ctx, disabled);
    let g = &mut ctx.gpr;
    g[V0] = g[T0] & 1;
}

/// `__osRestoreInt` (`func_8008CAA0`): `Status |= a0` (the 64-bit OR of the
/// sign-extended Status and `a0`, of which the runtime keeps the low word),
/// so passing [`func_8008CA80`]'s result puts IE back as it was.
///
/// Leaves `t0` = the value written.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008CAA0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (_mem, ctx) = enter(rdram, ctx);
    let status = imports::runtime::cop0_status_read(ctx);
    let g = &mut ctx.gpr;
    g[T0] = status | g[A0];
    let value = g[T0];
    imports::runtime::cop0_status_write(ctx, value);
}

/// `func_8008FB20(q, type)` (by shape libaudio's `alEvtqFlushType`): with
/// interrupts masked (`osSetIntMask(1)`, the old mask saved in the frame
/// and restored at the end), for each event item `n` of the allocation
/// list from `[q + 8]`, `next = [n]` read first: if the halfword `h[n +
/// 0xC]` (the event type, signed) equals `type` (the low halfword of `a1`,
/// sign-extended), the next item's delta `[next + 8] += [n + 8]` (if `next`
/// is nonzero), [`func_80088020`]`(n)` (`alUnlink`) and
/// [`func_80088050`]`(n, q)` (`alLink` onto the free list at `q`).
///
/// Frame (`sp - 0x40`): `ra`, `s3`, `s2`, `s1`, `s0` at `+0x24..+0x14`,
/// restored sign-extended (`s2 = q`, `s3 = type`, `s0`/`s1` the walk); `a1`
/// spilled to its home slot `+0x44`, the old mask to `+0x2C`. Leaves `t6 =
/// type`, `t7` the last type read, `t8`..`t0` from the last delta, `a0` the
/// mask, and the callees' registers.
///
/// Domain: [`crate::imports::func_80090500`]'s (a double); a finite list
/// in RDRAM, the callees' domains.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008FB20(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    sw(m, g[SP], 0x20, g[S3]);
    g[S3] = sll(g[A1], 16);
    sw(m, g[SP], 0x1C, g[S2]);
    g[S2] = g[A0];
    g[T6] = sra(g[S3], 16);
    sw(m, g[SP], 0x24, g[RA]);
    g[S3] = g[T6];
    sw(m, g[SP], 0x18, g[S1]);
    sw(m, g[SP], 0x14, g[S0]);
    sw(m, g[SP], 0x44, g[A1]);
    g[A0] = 1;
    call(imports::func_80090500, m, ctx);
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0x2C, g[V0]);
    g[S0] = lw(m, g[S2], 8);
    loop {
        let g = &mut ctx.gpr;
        if g[S0] == 0 {
            break;
        }
        g[T7] = lh(m, g[S0], 0xC);
        g[S1] = lw(m, g[S0], 0);
        if g[S3] == g[T7] {
            if g[S1] != 0 {
                g[T8] = lw(m, g[S1], 8);
                g[T9] = lw(m, g[S0], 8);
                g[T0] = addu(g[T8], g[T9]);
                sw(m, g[S1], 8, g[T0]);
            }
            g[A0] = g[S0];
            call(imports::func_80088020, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = g[S0];
            g[A1] = g[S2];
            call(imports::func_80088050, m, ctx);
        }
        let g = &mut ctx.gpr;
        g[S0] = g[S1];
    }
    let g = &mut ctx.gpr;
    g[A0] = lw(m, g[SP], 0x2C);
    call(imports::func_80090500, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x24);
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[S2] = lw(m, g[SP], 0x1C);
    g[S3] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x40);
}

/// `func_8008FBCC(q)` (by shape libaudio's `alEvtqFlush`): with interrupts
/// masked (`osSetIntMask(1)`, the old mask saved in the frame and restored
/// at the end), every event item `n` of the allocation list from `[q + 8]`
/// (`next = [n]` read first) goes back to the free list:
/// [`func_80088020`]`(n)` (`alUnlink`), [`func_80088050`]`(n, q)`
/// (`alLink`). The deltas are left as they are.
///
/// Frame (`sp - 0x38`): `ra`, `s2`, `s1`, `s0` at `+0x24..+0x18`, restored
/// sign-extended (`s2 = q`, `s0`/`s1` the walk); the old mask at `+0x2C`.
/// Leaves `a0` the mask and the callees' registers.
///
/// Domain: [`crate::imports::func_80090500`]'s (a double); a finite list
/// in RDRAM, the callees' domains.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008FBCC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x38i64) as u64);
    sw(m, g[SP], 0x20, g[S2]);
    g[S2] = g[A0];
    sw(m, g[SP], 0x24, g[RA]);
    sw(m, g[SP], 0x1C, g[S1]);
    sw(m, g[SP], 0x18, g[S0]);
    g[A0] = 1;
    call(imports::func_80090500, m, ctx);
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0x2C, g[V0]);
    g[S0] = lw(m, g[S2], 8);
    loop {
        let g = &mut ctx.gpr;
        if g[S0] == 0 {
            break;
        }
        g[S1] = lw(m, g[S0], 0);
        g[A0] = g[S0];
        call(imports::func_80088020, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = g[S0];
        g[A1] = g[S2];
        call(imports::func_80088050, m, ctx);
        let g = &mut ctx.gpr;
        g[S0] = g[S1];
    }
    let g = &mut ctx.gpr;
    g[A0] = lw(m, g[SP], 0x2C);
    call(imports::func_80090500, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x24);
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x38);
}

/// `func_8008FE60(s, p)` (by shape libaudio's `alSynAddPlayer`): with
/// interrupts masked (`osSetIntMask(1)`, restored after), the player `p` is
/// pushed on the synthesizer's list: `[p + 0x10] = [s + 0x20]` (its samples
/// left from the current samples), `[p] = [s]`, `[s] = p`, in that order.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`; `s` and `p` spilled to their home
/// slots `+0x18`, `+0x1C` and re-read sign-extended. Leaves `a2 = s`, `a1 =
/// p`, `a0` the old mask, `t6`, `t7` the words copied, and the double's
/// registers.
///
/// Domain: [`crate::imports::func_80090500`]'s (a double); `s` and `p` in
/// RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8008FE60(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    g[A2] = g[A0];
    sw(m, g[SP], 0x18, g[A2]);
    g[A0] = 1;
    sw(m, g[SP], 0x1C, g[A1]);
    call(imports::func_80090500, m, ctx);
    let g = &mut ctx.gpr;
    g[A2] = lw(m, g[SP], 0x18);
    g[A1] = lw(m, g[SP], 0x1C);
    g[A0] = g[V0];
    g[T6] = lw(m, g[A2], 0x20);
    sw(m, g[A1], 0x10, g[T6]);
    g[T7] = lw(m, g[A2], 0);
    sw(m, g[A1], 0, g[T7]);
    sw(m, g[A2], 0, g[A1]);
    call(imports::func_80090500, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80095AA0(e, a, b, c)` (**guess**: initialise a 0x14-byte event or
/// list record): `[e] = 0`, `[e + 4] = a`, `[e + 8] = b`, the halfwords
/// `[e + 0xC] = [e + 0xE] = 0`, `[e + 0x10] = c` (low words), in that
/// order.
///
/// Domain: canonical `e` with 0x14 bytes in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80095AA0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    sw(m, g[A0], 0, 0);
    sw(m, g[A0], 4, g[A1]);
    sw(m, g[A0], 8, g[A2]);
    sh(m, g[A0], 0xC, 0);
    sh(m, g[A0], 0xE, 0);
    sw(m, g[A0], 0x10, g[A3]);
}
