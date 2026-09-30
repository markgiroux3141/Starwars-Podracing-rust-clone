//! Small leaves whose subsystem isn't known yet, grouped by address. They
//! move to a named module once what they belong to is understood.

// Ports keep N64Recomp's names (func_80005AFC), capitals included.
#![allow(non_snake_case)]
use crate::imports;
use crate::render::DL2_HEAD;
use n64mem::Mem;
use crate::recomp::{addu, call, div, divu, enter, fpu, lb, lbu, ld, lh, lhu, li, lw, multu, reg::*, s32, sb, sd, sh, sll, sllv, slt, sltu, sra, srav, subu, sw, swl, swr, RecompContext};

/// The state [`func_8000097C`] records: a time (f32) at `0x800AE8B0`, then
/// two float triples at `0x800AE8B8` (a position, **guess**) and
/// `0x800AE8C8` (a direction, **guess**).
pub const TRACKED: u32 = 0x800A_E8B0;

/// [`func_8000097C`]'s record step: `[TRACKED] = t` (`f12`), the triple at
/// `a2` to `TRACKED + 8`, the one at `a3` to `TRACKED + 0x18` (each loaded
/// just before its store), then `[0x800AE8D8] = [0x800AE938]` and
/// `[0x800AEC7C] = 1`. `temps` are the two GPRs the path uses for the last
/// two words (`t8`/`t9` or `t6`/`t7`).
fn record(m: &mut Mem, g: &mut [u64; 32], f: &mut [crate::recomp::Fpr; 32], (word, one): (usize, usize)) {
    sw(m, g[V0], 0, u64::from(f[12].u32l()));
    f[8].set_u32l(lw(m, g[A2], 0) as u32);
    g[A0] = li(0x800A_E8B8);
    sw(m, g[A0], 0, u64::from(f[8].u32l()));
    f[4].set_u32l(lw(m, g[A2], 4) as u32);
    g[V1] = li(0x800A_E8C8);
    sw(m, g[A0], 4, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[A2], 8) as u32);
    sw(m, g[A0], 8, u64::from(f[10].u32l()));
    f[6].set_u32l(lw(m, g[A3], 0) as u32);
    sw(m, g[V1], 0, u64::from(f[6].u32l()));
    f[16].set_u32l(lw(m, g[A3], 4) as u32);
    sw(m, g[V1], 4, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A3], 8) as u32);
    sw(m, g[V1], 8, u64::from(f[18].u32l()));
    g[word] = lw(m, li(0x800B_0000), -0x16C8);
    g[AT] = li(0x800B_0000);
    sw(m, g[AT], -0x1728, g[word]);
    g[one] = 1;
    sw(m, g[AT], -0x1384, g[one]);
}

/// `func_8000097C(_, p, q, r)` with the float `t` in `f12`: record `t`, `q`
/// and `r` in [`TRACKED`] ([`record`]) unless `T - E < t` (`T = [TRACKED]`,
/// `E = [0x800A80F8]`, 0.01 in the ROM) and the stored direction `S =
/// [TRACKED + 0x18]` fails the test below. With `d = p - q` (stored to the
/// stack at `sp - 0xC..`), it records anyway when `S2*d2 + (d0*S0 + d1*S1)
/// < (d0*r0 + d1*r1) + d2*r2`, i.e. when `d` points further along `r` than
/// along `S`. The sums are in that order; nothing is fused.
///
/// Leaves `v0 = TRACKED`, `f6 = E`, `f8 = T - E` on the first path. The dot
/// test leaves `v1 = TRACKED + 0x18` and `f4`..`f18` as it used them; the
/// record step leaves `a0`, `v1`, `at = 0x800B0000`, `f4`..`f18` = the
/// words it copied and its two temporaries.
///
/// Domain: canonical pointers, 12 bytes each; `T` and `E` not NaN. On the
/// dot-test path `p`, `q`, `S`, `r` not NaN and no NaN intermediate (`inf -
/// inf`, `0 * inf`) reaching another operation; the two dot products may be
/// NaN (only compared). `t` may be anything (only compared and stored).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000097C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = li(TRACKED);
    g[AT] = li(0x800B_0000);
    f[6].set_u32l(lw(m, g[AT], -0x7F08) as u32);
    f[4].set_u32l(lw(m, g[V0], 0) as u32);
    g[SP] = addu(g[SP], (-0x10i64) as u64);
    f[8].set_fl(f[4].fl() - f[6].fl());
    if !(f[8].fl() < f[12].fl()) {
        record(m, g, f, (T8, T9));
    } else {
        // d = p - q, spilled to the frame; dot1 = d . S, dot2 = d . r.
        f[10].set_u32l(lw(m, g[A1], 0) as u32);
        f[16].set_u32l(lw(m, g[A2], 0) as u32);
        g[V1] = li(0x800A_E8C8);
        f[18].set_fl(f[10].fl() - f[16].fl());
        sw(m, g[SP], 4, u64::from(f[18].u32l()));
        f[6].set_u32l(lw(m, g[A2], 4) as u32);
        f[4].set_u32l(lw(m, g[A1], 4) as u32);
        f[8].set_fl(f[4].fl() - f[6].fl());
        f[6].set_u32l(lw(m, g[V1], 0) as u32);
        f[4].set_u32l(lw(m, g[SP], 4) as u32);
        sw(m, g[SP], 8, u64::from(f[8].u32l()));
        f[16].set_u32l(lw(m, g[A2], 8) as u32);
        f[10].set_u32l(lw(m, g[A1], 8) as u32);
        f[8].set_fl(f[4].fl() * f[6].fl());
        f[18].set_fl(f[10].fl() - f[16].fl());
        f[16].set_u32l(lw(m, g[V1], 4) as u32);
        f[10].set_u32l(lw(m, g[SP], 8) as u32);
        sw(m, g[SP], 0xC, u64::from(f[18].u32l()));
        f[18].set_fl(f[10].fl() * f[16].fl());
        f[16].set_u32l(lw(m, g[V1], 8) as u32);
        f[6].set_fl(f[8].fl() + f[18].fl());
        f[8].set_u32l(lw(m, g[SP], 0xC) as u32);
        f[18].set_fl(f[16].fl() * f[8].fl());
        f[16].set_fl(f[18].fl() + f[6].fl());
        f[18].set_u32l(lw(m, g[A3], 0) as u32);
        f[6].set_fl(f[4].fl() * f[18].fl());
        f[4].set_u32l(lw(m, g[A3], 4) as u32);
        f[18].set_fl(f[10].fl() * f[4].fl());
        f[4].set_u32l(lw(m, g[A3], 8) as u32);
        f[10].set_fl(f[6].fl() + f[18].fl());
        f[6].set_fl(f[8].fl() * f[4].fl());
        f[18].set_fl(f[10].fl() + f[6].fl());
        if f[16].fl() < f[18].fl() {
            record(m, g, f, (T6, T7));
        }
    }
    g[SP] = addu(g[SP], 0x10);
}

/// `func_80002BD4(a, b, c, d)`: 0 if the four points (float triples) all
/// lie beyond the same face of the box of half-size `h = [0x800AE8E0]`
/// around `C = [0x800AE908]` (a trivial reject, **guess**: of a quad
/// against a view or collision box), else 1. With `r_k = p_k - C` (all
/// twelve computed first and spilled to the frame at `sp - 0x30..`), it
/// tests in order: all `r_k.x < -h`, all `h < r_k.x`, then y, then z; each
/// test stops at the first point that fails it. Only strict compares.
///
/// Leaves `f0 = -h`, `f2 = h`, `f12`..`f16` = `C` or the last components
/// reloaded, and `f4`..`f18` as the tests that ran left them.
///
/// Domain: canonical pointers to 12 bytes each; the points, `C` and `h` not
/// NaN, and no `r_k` NaN (`inf - inf`). NaN compares would be false.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80002BD4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    let mut c1cs;
    'b_80002E28: {
        'b_80002E20: {
            'b_80002CCC: {
                g[V0] = li(0x800A_E908);
                f[12].set_u32l(lw(m, g[V0], 0) as u32);
                f[4].set_u32l(lw(m, g[A0], 0) as u32);
                g[SP] = addu(g[SP], (-0x30i64) as u64);
                f[14].set_u32l(lw(m, g[V0], 4) as u32);
                f[6].set_fl(f[4].fl() - f[12].fl());
                f[16].set_u32l(lw(m, g[V0], 8) as u32);
                g[AT] = li(0x800B_0000);
                f[2].set_u32l(lw(m, g[AT], -0x1720) as u32);
                sw(m, g[SP], 0x24, u64::from(f[6].u32l()));
                f[8].set_u32l(lw(m, g[A0], 4) as u32);
                f[0].set_fl(-f[2].fl());
                f[10].set_fl(f[8].fl() - f[14].fl());
                sw(m, g[SP], 0x28, u64::from(f[10].u32l()));
                f[18].set_u32l(lw(m, g[A0], 8) as u32);
                f[4].set_fl(f[18].fl() - f[16].fl());
                sw(m, g[SP], 0x2C, u64::from(f[4].u32l()));
                f[6].set_u32l(lw(m, g[A1], 0) as u32);
                f[8].set_fl(f[6].fl() - f[12].fl());
                sw(m, g[SP], 0x18, u64::from(f[8].u32l()));
                f[10].set_u32l(lw(m, g[A1], 4) as u32);
                f[18].set_fl(f[10].fl() - f[14].fl());
                sw(m, g[SP], 0x1C, u64::from(f[18].u32l()));
                f[4].set_u32l(lw(m, g[A1], 8) as u32);
                f[6].set_fl(f[4].fl() - f[16].fl());
                sw(m, g[SP], 0x20, u64::from(f[6].u32l()));
                f[8].set_u32l(lw(m, g[A2], 0) as u32);
                f[10].set_fl(f[8].fl() - f[12].fl());
                sw(m, g[SP], 0xC, u64::from(f[10].u32l()));
                f[18].set_u32l(lw(m, g[A2], 4) as u32);
                f[4].set_fl(f[18].fl() - f[14].fl());
                sw(m, g[SP], 0x10, u64::from(f[4].u32l()));
                f[6].set_u32l(lw(m, g[A2], 8) as u32);
                f[8].set_fl(f[6].fl() - f[16].fl());
                sw(m, g[SP], 0x14, u64::from(f[8].u32l()));
                f[10].set_u32l(lw(m, g[A3], 0) as u32);
                f[18].set_fl(f[10].fl() - f[12].fl());
                sw(m, g[SP], 0, u64::from(f[18].u32l()));
                f[4].set_u32l(lw(m, g[A3], 4) as u32);
                f[18].set_u32l(lw(m, g[SP], 0x24) as u32);
                f[6].set_fl(f[4].fl() - f[14].fl());
                f[4].set_u32l(lw(m, g[SP], 0x18) as u32);
                c1cs = f[18].fl() < f[0].fl();
                sw(m, g[SP], 4, u64::from(f[6].u32l()));
                f[8].set_u32l(lw(m, g[A3], 8) as u32);
                f[10].set_fl(f[8].fl() - f[16].fl());
                sw(m, g[SP], 8, u64::from(f[10].u32l()));
                if c1cs {
                    c1cs = f[4].fl() < f[0].fl();
                    f[6].set_u32l(lw(m, g[SP], 0xC) as u32);
                    if !c1cs {
                        f[10].set_u32l(lw(m, g[SP], 0x24) as u32);
                        break 'b_80002CCC;
                    }
                    c1cs = f[6].fl() < f[0].fl();
                    f[8].set_u32l(lw(m, g[SP], 0) as u32);
                    if !c1cs {
                        f[10].set_u32l(lw(m, g[SP], 0x24) as u32);
                        break 'b_80002CCC;
                    }
                    c1cs = f[8].fl() < f[0].fl();
                    if c1cs {
                        break 'b_80002E20;
                    }
                }
                // L_80002CC8
                f[10].set_u32l(lw(m, g[SP], 0x24) as u32);
            }
            // L_80002CCC
            f[18].set_u32l(lw(m, g[SP], 0x18) as u32);
            f[12].set_u32l(lw(m, g[SP], 0x28) as u32);
            c1cs = f[2].fl() < f[10].fl();
            if !c1cs {
                c1cs = f[12].fl() < f[0].fl();
            } else {
                c1cs = f[2].fl() < f[18].fl();
                f[4].set_u32l(lw(m, g[SP], 0xC) as u32);
                if !c1cs {
                    c1cs = f[12].fl() < f[0].fl();
                } else {
                    c1cs = f[2].fl() < f[4].fl();
                    f[6].set_u32l(lw(m, g[SP], 0) as u32);
                    if !c1cs {
                        c1cs = f[12].fl() < f[0].fl();
                    } else {
                        c1cs = f[2].fl() < f[6].fl();
                        if c1cs {
                            break 'b_80002E20;
                        }
                        c1cs = f[12].fl() < f[0].fl();
                    }
                }
            }
            // L_80002D18
            f[8].set_u32l(lw(m, g[SP], 0x1C) as u32);
            if !c1cs {
                c1cs = f[2].fl() < f[12].fl();
            } else {
                c1cs = f[8].fl() < f[0].fl();
                f[10].set_u32l(lw(m, g[SP], 0x10) as u32);
                if !c1cs {
                    c1cs = f[2].fl() < f[12].fl();
                } else {
                    c1cs = f[10].fl() < f[0].fl();
                    f[18].set_u32l(lw(m, g[SP], 4) as u32);
                    if !c1cs {
                        c1cs = f[2].fl() < f[12].fl();
                    } else {
                        c1cs = f[18].fl() < f[0].fl();
                        if c1cs {
                            break 'b_80002E20;
                        }
                        c1cs = f[2].fl() < f[12].fl();
                    }
                }
            }
            // L_80002D58
            f[12].set_u32l(lw(m, g[SP], 0x2C) as u32);
            f[4].set_u32l(lw(m, g[SP], 0x1C) as u32);
            if !c1cs {
                c1cs = f[12].fl() < f[0].fl();
            } else {
                c1cs = f[2].fl() < f[4].fl();
                f[6].set_u32l(lw(m, g[SP], 0x10) as u32);
                if !c1cs {
                    c1cs = f[12].fl() < f[0].fl();
                } else {
                    c1cs = f[2].fl() < f[6].fl();
                    f[8].set_u32l(lw(m, g[SP], 4) as u32);
                    if !c1cs {
                        c1cs = f[12].fl() < f[0].fl();
                    } else {
                        c1cs = f[2].fl() < f[8].fl();
                        if c1cs {
                            break 'b_80002E20;
                        }
                        c1cs = f[12].fl() < f[0].fl();
                    }
                }
            }
            // L_80002D9C
            f[10].set_u32l(lw(m, g[SP], 0x20) as u32);
            if !c1cs {
                c1cs = f[2].fl() < f[12].fl();
            } else {
                c1cs = f[10].fl() < f[0].fl();
                f[18].set_u32l(lw(m, g[SP], 0x14) as u32);
                if !c1cs {
                    c1cs = f[2].fl() < f[12].fl();
                } else {
                    c1cs = f[18].fl() < f[0].fl();
                    f[4].set_u32l(lw(m, g[SP], 8) as u32);
                    if !c1cs {
                        c1cs = f[2].fl() < f[12].fl();
                    } else {
                        c1cs = f[4].fl() < f[0].fl();
                        if c1cs {
                            break 'b_80002E20;
                        }
                        c1cs = f[2].fl() < f[12].fl();
                    }
                }
            }
            // L_80002DDC
            f[6].set_u32l(lw(m, g[SP], 0x20) as u32);
            g[V0] = 1;
            if !c1cs {
                break 'b_80002E28;
            }
            c1cs = f[2].fl() < f[6].fl();
            f[8].set_u32l(lw(m, g[SP], 0x14) as u32);
            if !c1cs {
                break 'b_80002E28;
            }
            c1cs = f[2].fl() < f[8].fl();
            f[10].set_u32l(lw(m, g[SP], 8) as u32);
            if !c1cs {
                break 'b_80002E28;
            }
            c1cs = f[2].fl() < f[10].fl();
            if !c1cs {
                break 'b_80002E28;
            }
        }
        // L_80002E1C
        g[V0] = 0;
    }
    // L_80002E24
    g[SP] = addu(g[SP], 0x30);
}

/// `func_80002E2C(a, b, c)`: [`func_80002BD4`] for three points: 0 if all
/// three lie beyond the same face of the box around `C = [0x800AE908]` with
/// half-size `h = [0x800AE8E0]`, else 1. The nine differences go to the
/// frame at `sp - 0x28..`, then the same six tests in the same order.
///
/// Leaves `f0 = -h`, `f2 = h`, and `f4`..`f18` as the tests left them.
///
/// Domain: as for [`func_80002BD4`].
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80002E2C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    let mut c1cs;
    'b_80002FF8: {
        'b_80002FF0: {
            'b_80002EF4: {
                g[V0] = li(0x800A_E908);
                f[12].set_u32l(lw(m, g[V0], 0) as u32);
                f[4].set_u32l(lw(m, g[A0], 0) as u32);
                g[SP] = addu(g[SP], (-0x28i64) as u64);
                f[14].set_u32l(lw(m, g[V0], 4) as u32);
                f[6].set_fl(f[4].fl() - f[12].fl());
                f[16].set_u32l(lw(m, g[V0], 8) as u32);
                g[AT] = li(0x800B_0000);
                f[2].set_u32l(lw(m, g[AT], -0x1720) as u32);
                sw(m, g[SP], 0x1C, u64::from(f[6].u32l()));
                f[8].set_u32l(lw(m, g[A0], 4) as u32);
                f[0].set_fl(-f[2].fl());
                f[10].set_fl(f[8].fl() - f[14].fl());
                sw(m, g[SP], 0x20, u64::from(f[10].u32l()));
                f[18].set_u32l(lw(m, g[A0], 8) as u32);
                f[4].set_fl(f[18].fl() - f[16].fl());
                sw(m, g[SP], 0x24, u64::from(f[4].u32l()));
                f[6].set_u32l(lw(m, g[A1], 0) as u32);
                f[8].set_fl(f[6].fl() - f[12].fl());
                sw(m, g[SP], 0x10, u64::from(f[8].u32l()));
                f[10].set_u32l(lw(m, g[A1], 4) as u32);
                f[18].set_fl(f[10].fl() - f[14].fl());
                sw(m, g[SP], 0x14, u64::from(f[18].u32l()));
                f[4].set_u32l(lw(m, g[A1], 8) as u32);
                f[6].set_fl(f[4].fl() - f[16].fl());
                sw(m, g[SP], 0x18, u64::from(f[6].u32l()));
                f[8].set_u32l(lw(m, g[A2], 0) as u32);
                f[10].set_fl(f[8].fl() - f[12].fl());
                sw(m, g[SP], 4, u64::from(f[10].u32l()));
                f[18].set_u32l(lw(m, g[A2], 4) as u32);
                f[10].set_u32l(lw(m, g[SP], 0x1C) as u32);
                f[4].set_fl(f[18].fl() - f[14].fl());
                f[18].set_u32l(lw(m, g[SP], 0x10) as u32);
                c1cs = f[10].fl() < f[0].fl();
                sw(m, g[SP], 8, u64::from(f[4].u32l()));
                f[6].set_u32l(lw(m, g[A2], 8) as u32);
                f[8].set_fl(f[6].fl() - f[16].fl());
                f[6].set_u32l(lw(m, g[SP], 0x1C) as u32);
                sw(m, g[SP], 0xC, u64::from(f[8].u32l()));
                if c1cs {
                    c1cs = f[18].fl() < f[0].fl();
                    f[4].set_u32l(lw(m, g[SP], 4) as u32);
                    if !c1cs {
                        c1cs = f[2].fl() < f[6].fl();
                        break 'b_80002EF4;
                    }
                    c1cs = f[4].fl() < f[0].fl();
                    if c1cs {
                        break 'b_80002FF0;
                    }
                }
                // L_80002EF0
                c1cs = f[2].fl() < f[6].fl();
            }
            // L_80002EF4
            f[8].set_u32l(lw(m, g[SP], 0x10) as u32);
            f[12].set_u32l(lw(m, g[SP], 0x20) as u32);
            if !c1cs {
                c1cs = f[12].fl() < f[0].fl();
            } else {
                c1cs = f[2].fl() < f[8].fl();
                f[10].set_u32l(lw(m, g[SP], 4) as u32);
                if !c1cs {
                    c1cs = f[12].fl() < f[0].fl();
                } else {
                    c1cs = f[2].fl() < f[10].fl();
                    if c1cs {
                        break 'b_80002FF0;
                    }
                    c1cs = f[12].fl() < f[0].fl();
                }
            }
            // L_80002F28
            f[18].set_u32l(lw(m, g[SP], 0x14) as u32);
            if !c1cs {
                c1cs = f[2].fl() < f[12].fl();
            } else {
                c1cs = f[18].fl() < f[0].fl();
                f[4].set_u32l(lw(m, g[SP], 8) as u32);
                if !c1cs {
                    c1cs = f[2].fl() < f[12].fl();
                } else {
                    c1cs = f[4].fl() < f[0].fl();
                    if c1cs {
                        break 'b_80002FF0;
                    }
                    c1cs = f[2].fl() < f[12].fl();
                }
            }
            // L_80002F58
            f[12].set_u32l(lw(m, g[SP], 0x24) as u32);
            f[6].set_u32l(lw(m, g[SP], 0x14) as u32);
            if !c1cs {
                c1cs = f[12].fl() < f[0].fl();
            } else {
                c1cs = f[2].fl() < f[6].fl();
                f[8].set_u32l(lw(m, g[SP], 8) as u32);
                if !c1cs {
                    c1cs = f[12].fl() < f[0].fl();
                } else {
                    c1cs = f[2].fl() < f[8].fl();
                    if c1cs {
                        break 'b_80002FF0;
                    }
                    c1cs = f[12].fl() < f[0].fl();
                }
            }
            // L_80002F8C
            f[10].set_u32l(lw(m, g[SP], 0x18) as u32);
            if !c1cs {
                c1cs = f[2].fl() < f[12].fl();
            } else {
                c1cs = f[10].fl() < f[0].fl();
                f[18].set_u32l(lw(m, g[SP], 0xC) as u32);
                if !c1cs {
                    c1cs = f[2].fl() < f[12].fl();
                } else {
                    c1cs = f[18].fl() < f[0].fl();
                    if c1cs {
                        break 'b_80002FF0;
                    }
                    c1cs = f[2].fl() < f[12].fl();
                }
            }
            // L_80002FBC
            f[4].set_u32l(lw(m, g[SP], 0x18) as u32);
            g[V0] = 1;
            if !c1cs {
                break 'b_80002FF8;
            }
            c1cs = f[2].fl() < f[4].fl();
            f[6].set_u32l(lw(m, g[SP], 0xC) as u32);
            if !c1cs {
                break 'b_80002FF8;
            }
            c1cs = f[2].fl() < f[6].fl();
            if !c1cs {
                break 'b_80002FF8;
            }
        }
        // L_80002FEC
        g[V0] = 0;
    }
    // L_80002FF4
    g[SP] = addu(g[SP], 0x28);
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

/// `func_80006F4C`: spills `f12` (a float argument) to its slot `[sp]` and
/// returns.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80006F4C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    sw(&mut mem, ctx.gpr[SP], 0, u64::from(ctx.fpr[12].u32l()));
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

/// `func_80007F5C(handle)` (a handle's length, **guess**: a sound's
/// duration): with `o` = [`func_80007CE4`]`(handle)`, `f0 = 0.0` if `o` is
/// 0, else `f0 = f32(n) * K` with `n = [[[[o + 0xC] + 4j + 0x10] + 8] +
/// 4]` (a signed word, `j = handle & 0x7FFF`) and `K` the float at
/// `0x800A81C8`.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`; `handle` spilled to its home slot
/// `sp + 0` around the call. Leaves `a0 = j`, `t6 = j`, and on the lookup
/// path `t7`..`t1` the pointers, `v1 = n`, `f4 = n`, `f6 = f32(n)`, `f8 =
/// K`; [`func_80007CE4`]'s registers otherwise.
///
/// Domain: [`func_80007CE4`]'s; for a nonzero `o`, the chain's words in
/// RDRAM and `K` not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80007F5C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x18, g[A0]);
    call(imports::func_80007CE4, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = lw(m, g[SP], 0x18);
    g[RA] = lw(m, g[SP], 0x14);
    g[T6] = g[A0] & 0x7FFF;
    g[A0] = g[T6];
    if g[V0] != 0 {
        g[T7] = lw(m, g[V0], 0xC);
        g[T8] = sll(g[A0], 2);
        g[AT] = li(0x800B_0000);
        g[T9] = addu(g[T7], g[T8]);
        g[T0] = lw(m, g[T9], 0x10);
        ctx.fpr[8].set_u32l(lw(m, g[AT], -0x7E38) as u32);
        g[T1] = lw(m, g[T0], 8);
        g[V1] = lw(m, g[T1], 4);
        ctx.fpr[4].set_u32l(g[V1] as u32);
        ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
        ctx.fpr[0].set_fl(ctx.fpr[6].fl() * ctx.fpr[8].fl());
    } else {
        ctx.fpr[0].set_u32l(0);
    }
    g[SP] = addu(g[SP], 0x18);
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

/// `func_800086F8()`: [`func_80007A44`] through a frame (`sp - 0x18`, `ra`
/// at `+0x14`); leaves its registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800086F8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    call(imports::func_80007A44, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
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

/// `func_80008760(id, prio, pitch, vol, pan, keep)` (request a sound,
/// **guess**: the eight 0x20-byte slots at `0x800D2038` that
/// [`func_80007A44`] and `func_80007E80` walk): `pitch` and `vol` are
/// floats in `a2`, `a3`; `pan` (the low halfword of the fifth argument) and
/// `keep` are on the stack; `prio` is `a1`'s low halfword, signed. With `f
/// = 1.0`, or `1.0 - B / 255.0` if `keep` (`B` the byte at `0x8009B77F`),
/// `S` the byte at `0x80113686` if [`func_80008718`]`(id)` (called twice)
/// says the id is special, else at `0x80113685`, and `v = (vol * (S /
/// 255.0)) * f`: nothing happens if `v == 0.0`. Otherwise, with `v' = v *
/// K` (`K` the float at `0x800A81E0`) and `prio` = 8 for special ids: the
/// pan becomes 0x40 unless bit 0 of `[0x80113688]` is set (stored into the
/// caller's argument slot `sp + 0x12`, QUIRK), and for `id >= 0` (signed
/// 64-bit):
/// - with `keep` nonzero, the first slot `r` (in order) with `[r + 8] != 0`
///   and `[r + 4] == id` is refreshed: unless `v' < f32([r + 0x18])` and
///   `[r + 0xC]` equals the frame counter `[0x80120BE8]` (then nothing
///   changes), `[r + 0x14] = pitch * 2`, `h[r + 0x1C] = pan`, `[r + 0x10] =
///   prio`, `[r + 0x18] = trunc(v')`, `[r + 0xC]` = the frame counter.
/// - otherwise the first slot with `[r] == -1`, or failing that the first
///   slot with the lowest priority `[r + 0x10]` below `prio` (signed), gets
///   `[r + 4] = id`, `[r] = -2`, `[r + 8] = keep`, `[r + 0x10] = prio`,
///   `[r + 0x14] = pitch * 2`, `h[r + 0x1C] = pan`, `[r + 0xC]` = the frame
///   counter, `[r + 0x18] = trunc(v')`; if no slot qualifies, nothing.
///
/// The byte-to-float conversions keep IDO's unsigned fix-up (`+ 2^32` for
/// a negative word), dead after an `lbu`. `trunc` is the C cast.
///
/// Frame (`sp - 0x38`): `ra`, `s1`, `s0` at `+0x1C..+0x14`, restored; `f`
/// spilled at `+0x24` around the calls, `pitch`, `vol` in their home slots
/// `sp + 8`, `sp + 0xC`, `prio` (`a1`) in `sp + 4`. Leaves `t1` = the pan,
/// `t2 = keep` (for `id >= 0`), `v0`, `v1`, `a0`..`a3` from the searches,
/// and `f0`..`f18` as the path used them.
///
/// Domain: `vol` and `K` not NaN, and no NaN intermediate (the products
/// are guarded; `pitch * 2` too).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80008760(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x38i64) as u64);
    g[T7] = lw(m, g[SP], 0x4C);
    sw(m, g[SP], 0x18, g[S1]);
    g[S1] = sll(g[A1], 16);
    g[T6] = sra(g[S1], 16);
    sw(m, g[SP], 0x14, g[S0]);
    ctx.fpr[14].set_u32l(g[A2] as u32);
    ctx.fpr[12].set_u32l(g[A3] as u32);
    g[S0] = g[A0];
    g[S1] = g[T6];
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x3C, g[A1]);
    if g[T7] == 0 {
        g[AT] = li(0x3F80_0000);
        ctx.fpr[0].set_u32l(g[AT] as u32);
    } else {
        g[T8] = li(0x800A_0000);
        g[T8] = lbu(m, g[T8], -0x4881);
        g[AT] = li(0x4F80_0000);
        ctx.fpr[4].set_u32l(g[T8] as u32);
        ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
        if (g[T8] as i64) < 0 {
            ctx.fpr[8].set_u32l(g[AT] as u32);
            ctx.fpr[6].set_fl(ctx.fpr[6].fl() + ctx.fpr[8].fl());
        }
        g[AT] = li(0x437F_0000);
        ctx.fpr[10].set_u32l(g[AT] as u32);
        g[AT] = li(0x3F80_0000);
        ctx.fpr[18].set_u32l(g[AT] as u32);
        ctx.fpr[16].set_fl(ctx.fpr[6].fl() / ctx.fpr[10].fl());
        ctx.fpr[0].set_fl(ctx.fpr[18].fl() - ctx.fpr[16].fl());
    }
    g[A0] = g[S0];
    sw(m, g[SP], 0x24, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[SP], 0x44, u64::from(ctx.fpr[12].u32l()));
    sw(m, g[SP], 0x40, u64::from(ctx.fpr[14].u32l()));
    call(imports::func_80008718, m, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x437F_0000);
    ctx.fpr[2].set_u32l(g[AT] as u32);
    ctx.fpr[0].set_u32l(lw(m, g[SP], 0x24) as u32);
    ctx.fpr[12].set_u32l(lw(m, g[SP], 0x44) as u32);
    ctx.fpr[14].set_u32l(lw(m, g[SP], 0x40) as u32);
    if g[V0] == 0 {
        g[T3] = li(0x8011_0000);
        g[T3] = lbu(m, g[T3], 0x3685);
        g[AT] = li(0x4F80_0000);
        ctx.fpr[18].set_u32l(g[T3] as u32);
        ctx.fpr[16].set_fl(fpu::cvt_s_w(ctx.fpr[18].u32l(), fpu::NEAREST));
        if (g[T3] as i64) < 0 {
            ctx.fpr[4].set_u32l(g[AT] as u32);
            ctx.fpr[16].set_fl(ctx.fpr[16].fl() + ctx.fpr[4].fl());
        }
        ctx.fpr[6].set_fl(ctx.fpr[16].fl() / ctx.fpr[2].fl());
        ctx.fpr[12].set_fl(ctx.fpr[12].fl() * ctx.fpr[6].fl());
        g[A0] = g[S0];
    } else {
        g[T9] = li(0x8011_0000);
        g[T9] = lbu(m, g[T9], 0x3686);
        g[AT] = li(0x4F80_0000);
        ctx.fpr[4].set_u32l(g[T9] as u32);
        ctx.fpr[8].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
        if (g[T9] as i64) < 0 {
            ctx.fpr[6].set_u32l(g[AT] as u32);
            ctx.fpr[8].set_fl(ctx.fpr[8].fl() + ctx.fpr[6].fl());
        }
        ctx.fpr[10].set_fl(ctx.fpr[8].fl() / ctx.fpr[2].fl());
        ctx.fpr[12].set_fl(ctx.fpr[12].fl() * ctx.fpr[10].fl());
        g[A0] = g[S0];
    }
    sw(m, g[SP], 0x24, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[SP], 0x44, u64::from(ctx.fpr[12].u32l()));
    sw(m, g[SP], 0x40, u64::from(ctx.fpr[14].u32l()));
    call(imports::func_80008718, m, ctx);
    let g = &mut ctx.gpr;
    ctx.fpr[0].set_u32l(lw(m, g[SP], 0x24) as u32);
    ctx.fpr[12].set_u32l(lw(m, g[SP], 0x44) as u32);
    ctx.fpr[14].set_u32l(lw(m, g[SP], 0x40) as u32);
    if g[V0] != 0 {
        g[S1] = 8;
    }
    'b_80008B04: {
        ctx.fpr[12].set_fl(ctx.fpr[12].fl() * ctx.fpr[0].fl());
        ctx.fpr[8].set_u32l(0);
        g[AT] = li(0x4000_0000);
        g[T4] = li(0x8011_0000);
        if ctx.fpr[12].fl() == ctx.fpr[8].fl() {
            g[RA] = lw(m, g[SP], 0x1C);
        } else {
            ctx.fpr[10].set_u32l(g[AT] as u32);
            g[AT] = li(0x800B_0000);
            ctx.fpr[18].set_u32l(lw(m, g[AT], -0x7E20) as u32);
            g[T4] = lw(m, g[T4], 0x3688);
            ctx.fpr[14].set_fl(ctx.fpr[14].fl() * ctx.fpr[10].fl());
            g[T1] = 0x40;
            g[T5] = g[T4] & 1;
            ctx.fpr[12].set_fl(ctx.fpr[12].fl() * ctx.fpr[18].fl());
            if g[T5] == 0 {
                sh(m, g[SP], 0x4A, g[T1]);
            }
            'b_80008B00: {
                g[T1] = lh(m, g[SP], 0x4A);
                if (g[S0] as i64) >= 0 {
                    g[T2] = lw(m, g[SP], 0x4C);
                    g[V0] = li(0x800D_2038);
                    g[V1] = li(0x800D_0000);
                    if g[T2] != 0 {
                        g[V1] = addu(g[V1], 0x2138);
                        g[T6] = lw(m, g[V0], 8);
                        loop {
                            if g[T6] == 0 {
                                g[T7] = lw(m, g[V0], 0x28);
                            } else {
                                g[T7] = lw(m, g[V0], 4);
                                if g[S0] == g[T7] {
                                    g[T8] = lw(m, g[V0], 0x18);
                                    g[T9] = li(0x8012_0000);
                                    ctx.fpr[4].set_u32l(g[T8] as u32);
                                    ctx.fpr[16].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
                                    if ctx.fpr[16].fl() <= ctx.fpr[12].fl() {
                                        ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[12].fl()));
                                    } else {
                                        g[T9] = lw(m, g[T9], 0xBE8);
                                        g[T3] = lw(m, g[V0], 0xC);
                                        if g[T9] == g[T3] {
                                            g[RA] = lw(m, g[SP], 0x1C);
                                            break 'b_80008B04;
                                        }
                                        ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[12].fl()));
                                    }
                                    sw(m, g[V0], 0x14, u64::from(ctx.fpr[14].u32l()));
                                    sh(m, g[V0], 0x1C, g[T1]);
                                    sw(m, g[V0], 0x10, g[S1]);
                                    g[T5] = s32(ctx.fpr[6].u32l());
                                    g[T6] = li(0x8012_0000);
                                    sw(m, g[V0], 0x18, g[T5]);
                                    g[T6] = lw(m, g[T6], 0xBE8);
                                    sw(m, g[V0], 0xC, g[T6]);
                                    break 'b_80008B00;
                                }
                                g[T7] = lw(m, g[V0], 0x28);
                            }
                            if g[T7] == 0 {
                                g[V0] = addu(g[V0], 0x40);
                            } else {
                                g[T8] = lw(m, g[V0], 0x24);
                                if g[S0] == g[T8] {
                                    g[T9] = lw(m, g[V0], 0x38);
                                    g[T3] = li(0x8012_0000);
                                    ctx.fpr[8].set_u32l(g[T9] as u32);
                                    ctx.fpr[10].set_fl(fpu::cvt_s_w(ctx.fpr[8].u32l(), fpu::NEAREST));
                                    if ctx.fpr[10].fl() <= ctx.fpr[12].fl() {
                                        ctx.fpr[18].set_u32l(fpu::trunc_w_s(ctx.fpr[12].fl()));
                                    } else {
                                        g[T3] = lw(m, g[T3], 0xBE8);
                                        g[T4] = lw(m, g[V0], 0x2C);
                                        if g[T3] == g[T4] {
                                            g[RA] = lw(m, g[SP], 0x1C);
                                            break 'b_80008B04;
                                        }
                                        ctx.fpr[18].set_u32l(fpu::trunc_w_s(ctx.fpr[12].fl()));
                                    }
                                    sw(m, g[V0], 0x34, u64::from(ctx.fpr[14].u32l()));
                                    sh(m, g[V0], 0x3C, g[T1]);
                                    sw(m, g[V0], 0x30, g[S1]);
                                    g[T6] = s32(ctx.fpr[18].u32l());
                                    g[T7] = li(0x8012_0000);
                                    sw(m, g[V0], 0x38, g[T6]);
                                    g[T7] = lw(m, g[T7], 0xBE8);
                                    sw(m, g[V0], 0x2C, g[T7]);
                                    break 'b_80008B00;
                                }
                                g[V0] = addu(g[V0], 0x40);
                            }
                            if g[V0] == g[V1] {
                                break;
                            }
                            g[T6] = lw(m, g[V0], 8);
                        }
                    }
                    g[V0] = li(0x800D_0000);
                    g[A2] = u64::MAX;
                    g[V0] = addu(g[V0], 0x2038);
                    g[V1] = 0;
                    g[A3] = u64::MAX;
                    loop {
                        g[T8] = lw(m, g[V0], 0);
                        if g[A3] != g[T8] {
                            g[V1] = addu(g[V1], 1);
                        } else if g[A2] != g[A3] {
                            g[V1] = addu(g[V1], 1);
                        } else {
                            g[A2] = g[V1];
                            g[V1] = addu(g[V1], 1);
                        }
                        g[AT] = slt(g[V1], 8);
                        g[V0] = addu(g[V0], 0x20);
                        if g[AT] == 0 {
                            break;
                        }
                    }
                    g[A0] = g[S1];
                    if g[A2] == g[A3] {
                        g[V0] = li(0x800D_2038);
                        g[V1] = 0;
                        g[T0] = 8;
                        loop {
                            g[A1] = lw(m, g[V0], 0x10);
                            g[AT] = slt(g[A1], g[A0]);
                            if g[AT] == 0 {
                                g[A1] = lw(m, g[V0], 0x30);
                            } else {
                                g[A0] = g[A1];
                                g[A2] = g[V1];
                                g[A1] = lw(m, g[V0], 0x30);
                            }
                            g[AT] = slt(g[A1], g[A0]);
                            if g[AT] == 0 {
                                g[A1] = lw(m, g[V0], 0x50);
                            } else {
                                g[A0] = g[A1];
                                g[A2] = addu(g[V1], 1);
                                g[A1] = lw(m, g[V0], 0x50);
                            }
                            g[AT] = slt(g[A1], g[A0]);
                            if g[AT] == 0 {
                                g[A1] = lw(m, g[V0], 0x70);
                            } else {
                                g[A0] = g[A1];
                                g[A2] = addu(g[V1], 2);
                                g[A1] = lw(m, g[V0], 0x70);
                            }
                            g[AT] = slt(g[A1], g[A0]);
                            if g[AT] == 0 {
                                g[V1] = addu(g[V1], 4);
                            } else {
                                g[A0] = g[A1];
                                g[A2] = addu(g[V1], 3);
                                g[V1] = addu(g[V1], 4);
                            }
                            g[V0] = addu(g[V0], 0x80);
                            if g[V1] == g[T0] {
                                break;
                            }
                        }
                    }
                    g[T9] = sll(g[A2], 5);
                    if g[A2] != g[A3] {
                        g[T3] = li(0x800D_2038);
                        g[V0] = addu(g[T9], g[T3]);
                        ctx.fpr[4].set_u32l(fpu::trunc_w_s(ctx.fpr[12].fl()));
                        g[T4] = (-2i64) as u64;
                        sw(m, g[V0], 4, g[S0]);
                        sw(m, g[V0], 0, g[T4]);
                        sw(m, g[V0], 8, g[T2]);
                        g[T5] = li(0x8012_0000);
                        g[T5] = lw(m, g[T5], 0xBE8);
                        g[T7] = s32(ctx.fpr[4].u32l());
                        sw(m, g[V0], 0x10, g[S1]);
                        sw(m, g[V0], 0x14, u64::from(ctx.fpr[14].u32l()));
                        sh(m, g[V0], 0x1C, g[T1]);
                        sw(m, g[V0], 0xC, g[T5]);
                        sw(m, g[V0], 0x18, g[T7]);
                    }
                }
            }
            g[RA] = lw(m, g[SP], 0x1C);
        }
    }
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x38);
}

/// `func_80008F58(x, y)` with the floats in `f12`/`f14`: `[0x8009AD08] =
/// x`, `[0x8009AD0C] = y`. Leaves `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80008F58(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], -0x52F8, u64::from(ctx.fpr[12].u32l()));
    sw(m, g[AT], -0x52F4, u64::from(ctx.fpr[14].u32l()));
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

/// `func_80009134(k, i)`: whether a float is positive (`0.0 < x`, so false
/// for NaN and zeros): `x = [0x8009AD30 + 4i]` for `k` 0 or 1, else `x =
/// [0x8009AD10 + 4k]` (full 64-bit compares of `k`; both indices
/// unbounded, 32-bit address arithmetic).
///
/// Leaves `at` = the address or 1, `t6 = 4i` or `t7 = 4k`, `f4`/`f6` or
/// `f8`/`f10` = 0.0 and `x`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80009134(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = 1;
    if g[A0] != 0 {
        g[T7] = sll(g[A0], 2);
        if g[A0] != g[AT] {
            g[AT] = addu(li(0x800A_0000), g[T7]);
            f[10].set_u32l(lw(m, g[AT], -0x52F0) as u32);
            f[8].set_u32l(0);
            g[V0] = u64::from(f[8].fl() < f[10].fl());
            return;
        }
    }
    g[T6] = sll(g[A1], 2);
    g[AT] = addu(li(0x800A_0000), g[T6]);
    f[6].set_u32l(lw(m, g[AT], -0x52D0) as u32);
    f[4].set_u32l(0);
    g[V0] = u64::from(f[4].fl() < f[6].fl());
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

/// `func_80009744(mode, k)` (set a mode's parameters, **guess**): by
/// `mode` (64-bit compares), with `[0x8009AEE0] = mode` in every known
/// case:
/// - 0: `[0x8009AEEC] = -1`;
/// - 1: `[0x8009AEEC]` = the s16 `T[k % 5]` (`T` at `0x8009AEFC`, `k`'s
///   low word unsigned), then 0x8F instead if the first `"Test"` pool's
///   count ([`func_8003F7B8`](crate::pools::func_8003F7B8)) is at most 0;
/// - 2: `[0x8009AEE8]` = the float at `0x800A8220`;
/// - 3: `[0x8009AEE8] = -2.0`;
/// - anything else: nothing.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `a2 = mode`, `at` from
/// the tests, `t7`..`t9` (mode 1), `t0 = 0x8F` and the callee's
/// registers (mode 1), `f4` or `f6` (modes 2, 3).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80009744(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_80009814: {
        g[SP] = addu(g[SP], (-0x18i64) as u64);
        sw(m, g[SP], 0x14, g[RA]);
        g[A2] = g[A0];
        if g[A0] == 0 {
            g[AT] = li(0x800A_0000);
            sw(m, g[AT], -0x5120, g[A2]);
            g[AT] = li(0x800A_0000);
            g[T6] = u64::MAX;
            sw(m, g[AT], -0x5114, g[T6]);
        } else {
            g[AT] = 1;
            g[T9] = li(0x800A_0000);
            if g[A0] == g[AT] {
                g[AT] = 5;
                let (_, hi) = divu(g[A1], g[AT]);
                g[T7] = hi;
                g[T8] = sll(g[T7], 1);
                g[T9] = addu(g[T9], g[T8]);
                g[T9] = lh(m, g[T9], -0x5104);
                g[AT] = li(0x800A_0000);
                g[A0] = li(0x5465_0000);
                sw(m, g[AT], -0x5114, g[T9]);
                g[AT] = li(0x800A_0000);
                sw(m, g[AT], -0x5120, g[A2]);
                g[A0] = g[A0] | 0x7374;
                call(imports::func_8003F7B8, m, ctx);
                let g = &mut ctx.gpr;
                g[T0] = 0x8F;
                if (g[V0] as i64) <= 0 {
                    g[AT] = li(0x800A_0000);
                    sw(m, g[AT], -0x5114, g[T0]);
                }
            } else {
                g[AT] = 2;
                let c0 = g[A0] == g[AT];
                g[AT] = 3;
                if c0 {
                    g[AT] = li(0x800A_0000);
                    sw(m, g[AT], -0x5120, g[A2]);
                    g[AT] = li(0x800B_0000);
                    ctx.fpr[4].set_u32l(lw(m, g[AT], -0x7DE0) as u32);
                    g[AT] = li(0x800A_0000);
                    sw(m, g[AT], -0x5118, u64::from(ctx.fpr[4].u32l()));
                } else {
                    if g[A0] != g[AT] {
                        g[RA] = lw(m, g[SP], 0x14);
                        break 'b_80009814;
                    }
                    g[AT] = li(0x800A_0000);
                    sw(m, g[AT], -0x5120, g[A2]);
                    g[AT] = li(0xC000_0000);
                    ctx.fpr[6].set_u32l(g[AT] as u32);
                    g[AT] = li(0x800A_0000);
                    sw(m, g[AT], -0x5118, u64::from(ctx.fpr[6].u32l()));
                }
            }
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x14);
    }
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80009B8C(k)` (**guess**: pick a parameter by `k`): unless the
/// first `"Test"` pool has a positive count
/// ([`func_8003F7B8`](crate::pools::func_8003F7B8)) and
/// [`func_80051FF4`] returns 0, it sets
/// `[0x8009AF24] = 0.0`, `[0x8009AEF0]` = the s16 `T[k % 12]` (`T` at
/// `0x8009AF08`, `k`'s low word unsigned) and `[0x8009AF20]` = the float at
/// `0x800A822C`.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`; `k` spilled to its home slot `sp
/// + 0` around the calls. Leaves `v1 = k`, `t6`..`t8`, `f4 =
/// 0.0`, `f6` and the callees' registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80009B8C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_80009BFC: {
        g[SP] = addu(g[SP], (-0x18i64) as u64);
        sw(m, g[SP], 0x18, g[A0]);
        sw(m, g[SP], 0x14, g[RA]);
        g[A0] = li(0x5465_7374);
        call(imports::func_8003F7B8, m, ctx);
        let g = &mut ctx.gpr;
        g[V1] = lw(m, g[SP], 0x18);
        if (g[V0] as i64) > 0 {
            sw(m, g[SP], 0x18, g[V1]);
            call(imports::func_80051FF4, m, ctx);
            let g = &mut ctx.gpr;
            g[V1] = lw(m, g[SP], 0x18);
            if g[V0] == 0 {
                break 'b_80009BFC;
            }
        }
        let g = &mut ctx.gpr;
        ctx.fpr[4].set_u32l(0);
        g[AT] = li(0x800A_0000);
        g[T8] = li(0x800A_0000);
        sw(m, g[AT], -0x50DC, u64::from(ctx.fpr[4].u32l()));
        g[AT] = 0xC;
        let (_, hi) = divu(g[V1], g[AT]);
        g[T6] = hi;
        g[T7] = sll(g[T6], 1);
        g[T8] = addu(g[T8], g[T7]);
        g[T8] = lh(m, g[T8], -0x50F8);
        g[AT] = li(0x800A_0000);
        sw(m, g[AT], -0x5110, g[T8]);
        g[AT] = li(0x800B_0000);
        ctx.fpr[6].set_u32l(lw(m, g[AT], -0x7DD4) as u32);
        g[AT] = li(0x800A_0000);
        sw(m, g[AT], -0x50E0, u64::from(ctx.fpr[6].u32l()));
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80009F6C()`: [`func_80007A44`], then `[0x8009AEF0] =
/// [0x8009ADE8] = -1`.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `v0 = -1`, `at =
/// 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80009F6C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    call(imports::func_80007A44, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[V0] = u64::MAX;
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], -0x5110, g[V0]);
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], -0x5218, g[V0]);
    g[SP] = addu(g[SP], 0x18);
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

/// `func_8000A44C(id, p)`: initialise record `id` of [`RECORDS`] (`a0`'s
/// low halfword, signed; spilled to `[sp]` first) if `id < 200`: raise the
/// count `[0x8009B770]` to `id + 1` unless `id < count` (signed), then
/// halfwords `+0`, `+2` = 0, flags `+0x14` = 1, the four bytes `+0x18..` =
/// 0xFF, `+0x1C = p`, floats `+8 = +0xC = 1.0`, `+0x10 = 0.0`. QUIRK:
/// negative ids pass the bound and write below the records.
///
/// Leaves `a0` = the id (sign-extended), `t6 = a0 << 16`, `t7` = the id,
/// `at` = the bound test (or `0x3F800000` past it), and past it `v0` = the
/// record, `v1 = 0xFF`, `t8` = the old count, `t0 = 32 * id`, `t1 =
/// RECORDS`, `t2 = 1`, `f0 = 1.0`, `f4 = 0.0` (`t9 = id + 1` if the count
/// was raised).
///
/// Domain: `id >= -26764`, so the record is in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000A44C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = sll(g[A0], 16);
    g[T7] = sra(g[T6], 16);
    sw(m, g[SP], 0, g[A0]);
    g[AT] = slt(g[T7], 0xC8);
    g[A0] = g[T7];
    if g[AT] == 0 {
        return;
    }
    g[V0] = li(0x8009_B770);
    g[T8] = lw(m, g[V0], 0);
    g[V1] = 0xFF;
    g[T0] = sll(g[A0], 5);
    g[AT] = slt(g[T7], g[T8]);
    if g[AT] == 0 {
        g[T9] = addu(g[T7], 1);
        sw(m, g[V0], 0, g[T9]);
    }
    g[T1] = li(RECORDS);
    g[V0] = addu(g[T0], g[T1]);
    g[AT] = li(0x3F80_0000);
    f[0].set_u32l(g[AT] as u32);
    f[4].set_u32l(0);
    g[T2] = 1;
    sh(m, g[V0], 0, 0);
    sh(m, g[V0], 2, 0);
    sw(m, g[V0], 0x14, g[T2]);
    for off in 0x18..0x1C {
        sb(m, g[V0], off, g[V1]);
    }
    sw(m, g[V0], 0x1C, g[A1]);
    sw(m, g[V0], 8, u64::from(f[0].u32l()));
    sw(m, g[V0], 0xC, u64::from(f[0].u32l()));
    sw(m, g[V0], 0x10, u64::from(f[4].u32l()));
}

/// 32-byte records indexed by a signed 16-bit id: `+4`/`+6` halfwords
/// ([`func_8000AA78`]), `+0x14` flags with bit `0x20` an "on" bit
/// ([`func_8000A920`], [`func_8000AC34`], [`func_8000AC60`]), four bytes at
/// `+0x18` ([`func_8000AB24`], a colour by the look of it: **guess**) and a
/// pointer at `+0x1C` ([`func_8000ABD4`], [`func_8000AC0C`]). How many there
/// are isn't known. Ids -201, -103 and -104 name globals instead.
pub const RECORDS: u32 = 0x800D_2190;

/// `func_8000A4D8()` (reset the records, **guess**): for `i` from 0 while
/// `i < [0x8009B770]` (signed, the count re-read after each call),
/// [`func_8000A44C`]`(i, 0)` with `i` as a sign-extended halfword; then
/// `[0x8009B770] = 0`.
///
/// Frame (`sp - 0x20`): `ra`, `s1`, `s0` at `+0x1C..+0x14`, restored (`s1
/// = 0x8009B770`, `s0 = i`). Leaves `t6`..`t8`, `at` from the loop and
/// the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000A4D8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x18, g[S1]);
    g[S1] = li(0x8009_B770);
    g[T6] = lw(m, g[S1], 0);
    sw(m, g[SP], 0x14, g[S0]);
    sw(m, g[SP], 0x1C, g[RA]);
    g[S0] = 0;
    if (g[T6] as i64) > 0 {
        g[A0] = sll(g[S0], 16);
        loop {
            let g = &mut ctx.gpr;
            g[T7] = sra(g[A0], 16);
            g[A0] = g[T7];
            g[A1] = 0;
            call(imports::func_8000A44C, m, ctx);
            let g = &mut ctx.gpr;
            g[T8] = lw(m, g[S1], 0);
            g[S0] = addu(g[S0], 1);
            g[AT] = slt(g[S0], g[T8]);
            if g[AT] == 0 {
                break;
            }
            g[A0] = sll(g[S0], 16);
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    sw(m, g[S1], 0, 0);
    g[S1] = lw(m, g[SP], 0x18);
    g[S0] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x20);
}

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

/// `func_8000AA04(id, x, y)`: set record `id`'s halfwords `+0 = x`, `+2 =
/// y` for `id >= 0`, or for `id == -201` the floats `[0x8009B784] = x`,
/// `[0x8009B788] = y` (converted). All three are the low halfwords of the
/// arguments, signed; the full arguments are spilled to `[sp..sp + 0xC)`
/// first. No bound above (QUIRK, as for the other record setters).
///
/// Leaves `a0`/`a1`/`a2` = the halfwords (sign-extended), `t6`/`t8`/`t0` =
/// the arguments `<< 16`, `at = -201` (or `0x800A0000` for -201), and
/// `t2 = 32 * id`, `t3 = RECORDS`, `v0` = the record, or `f4`/`f8` = the
/// halfwords and `f6`/`f10` = the floats.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AA04(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    sw(m, g[SP], 0, g[A0]);
    g[T6] = sll(g[A0], 16);
    g[A0] = sra(g[T6], 16);
    sw(m, g[SP], 4, g[A1]);
    g[T8] = sll(g[A1], 16);
    sw(m, g[SP], 8, g[A2]);
    g[T0] = sll(g[A2], 16);
    g[AT] = (-0xC9i64) as u64;
    g[A2] = sra(g[T0], 16);
    g[A1] = sra(g[T8], 16);
    if g[A0] == g[AT] {
        f[4].set_u32l(g[A1] as u32);
        f[8].set_u32l(g[A2] as u32);
        g[AT] = li(0x800A_0000);
        f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
        f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
        sw(m, g[AT], -0x487C, u64::from(f[6].u32l()));
        sw(m, g[AT], -0x4878, u64::from(f[10].u32l()));
        return;
    }
    g[T2] = sll(g[A0], 5);
    if (g[A0] as i64) >= 0 {
        g[T3] = li(RECORDS);
        g[V0] = addu(g[T2], g[T3]);
        sh(m, g[V0], 0, g[A1]);
        sh(m, g[V0], 2, g[A2]);
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

/// `func_8000AAC0(id, x, y)` with the floats in `a1`/`a2`: for `id >= 0`
/// (`a0`'s low halfword, signed; `a0` spilled to `[sp]`), record `id`'s
/// floats `+8 = x`, `+0xC = y`. No bound above.
///
/// Leaves `t6 = a0 << 16`, `t7` = the id, `f12`/`f14` = the floats, and
/// for `id >= 0` `t8 = 32 * id`, `t9 = RECORDS`, `v0` = the record.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AAC0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = sll(g[A0], 16);
    g[T7] = sra(g[T6], 16);
    f[12].set_u32l(g[A1] as u32);
    f[14].set_u32l(g[A2] as u32);
    sw(m, g[SP], 0, g[A0]);
    if (g[T7] as i64) >= 0 {
        g[T9] = li(RECORDS);
        g[T8] = sll(g[T7], 5);
        g[V0] = addu(g[T8], g[T9]);
        sw(m, g[V0], 8, u64::from(f[12].u32l()));
        sw(m, g[V0], 0xC, u64::from(f[14].u32l()));
    }
}

/// `func_8000AAF8(id, x)` with the float in `a1`: for `id >= 0` (`a0`'s low
/// halfword, signed; `a0` spilled to `[sp]`), record `id`'s float `+0x10 =
/// x`. No bound above.
///
/// Leaves `t6 = a0 << 16`, `t7` = the id, `f12 = x`, and for `id >= 0` `t8
/// = 32 * id`, `at = 0x800D0000 + t8`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AAF8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = sll(g[A0], 16);
    g[T7] = sra(g[T6], 16);
    f[12].set_u32l(g[A1] as u32);
    sw(m, g[SP], 0, g[A0]);
    if (g[T7] as i64) >= 0 {
        g[T8] = sll(g[T7], 5);
        g[AT] = addu(li(0x800D_0000), g[T8]);
        sw(m, g[AT], 0x21A0, u64::from(f[12].u32l()));
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

/// `func_8000AC90(v)`: [`func_8003D488`]`(v &
/// 0xFFFF)`, which sets `[0x800A48D4]`.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`; `v` spilled to its home slot `sp
/// + 0`. Leaves `t6 = v & 0xFFFF` and the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AC90(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x18, g[A0]);
    g[T6] = g[A0] & 0xFFFF;
    g[A0] = g[T6];
    call(imports::func_8003D488, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_8000ACC0()` (reset the entries, **guess**): for each of the 32
/// entries of 0x7C bytes at `[0x8009B790]` (the base re-read before each
/// store): `+0x14..+0x50` = the identity matrix, `+8 = 0`, halfword `+4 =
/// 0`, `+0x6C..+0x74` = 0.0. Then `[0x8009B794]` = the base, the selection
/// `[0x8009B798] = 0`, and the entry at `0x800D4B20`: bit 0 of `+0` cleared,
/// halfwords `+4 = +6 = +0x80 = 0`, `+0x20 = 0.0`, `+0x84 = 0`, `+0x28..` the
/// identity, `+0x68..+0x70 = 0.0`, `+0x24 = 10.0`; then
/// [`func_8000AED4`]`(1, 4)`, entry 1's flags `|= 4`.
///
/// The loop index is a sign-extended halfword, compared (signed) with 32
/// after each entry; `lo = 0x7C * i` (`multu`). Frame (`sp - 0x18`): `ra`
/// at `+0x14`. Leaves `v1 = 0x8009B790`, `f0 = f12 = 0.0`, `f2 = 1.0`, `f4
/// = 10.0`, and the callee's registers.
///
/// Domain: the entries and the base in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000ACC0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x3F80_0000);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[V1] = li(0x800A_0000);
    ctx.fpr[2].set_u32l(g[AT] as u32);
    ctx.fpr[12].set_u32l(0);
    ctx.fpr[0].set_u32l(0);
    sw(m, g[SP], 0x14, g[RA]);
    g[V1] = addu(g[V1], (-0x4870i64) as u64);
    g[A0] = 0;
    g[A1] = 0x7C;
    loop {
        let (lo, _) = multu(g[A0], g[A1]);
        g[T6] = lw(m, g[V1], 0);
        g[A0] = addu(g[A0], 1);
        g[V0] = lo;
        g[T7] = addu(g[T6], g[V0]);
        sw(m, g[T7], 0x14, u64::from(ctx.fpr[2].u32l()));
        g[T8] = lw(m, g[V1], 0);
        g[T9] = addu(g[T8], g[V0]);
        sw(m, g[T9], 0x18, u64::from(ctx.fpr[0].u32l()));
        g[T0] = lw(m, g[V1], 0);
        g[T1] = addu(g[T0], g[V0]);
        sw(m, g[T1], 0x1C, u64::from(ctx.fpr[0].u32l()));
        g[T2] = lw(m, g[V1], 0);
        g[T3] = addu(g[T2], g[V0]);
        sw(m, g[T3], 0x20, u64::from(ctx.fpr[0].u32l()));
        g[T4] = lw(m, g[V1], 0);
        g[T5] = addu(g[T4], g[V0]);
        sw(m, g[T5], 0x24, u64::from(ctx.fpr[0].u32l()));
        g[T6] = lw(m, g[V1], 0);
        g[T7] = addu(g[T6], g[V0]);
        sw(m, g[T7], 0x28, u64::from(ctx.fpr[2].u32l()));
        g[T8] = lw(m, g[V1], 0);
        g[T9] = addu(g[T8], g[V0]);
        sw(m, g[T9], 0x2C, u64::from(ctx.fpr[0].u32l()));
        g[T0] = lw(m, g[V1], 0);
        g[T1] = addu(g[T0], g[V0]);
        sw(m, g[T1], 0x30, u64::from(ctx.fpr[0].u32l()));
        g[T2] = lw(m, g[V1], 0);
        g[T3] = addu(g[T2], g[V0]);
        sw(m, g[T3], 0x34, u64::from(ctx.fpr[0].u32l()));
        g[T4] = lw(m, g[V1], 0);
        g[T5] = addu(g[T4], g[V0]);
        sw(m, g[T5], 0x38, u64::from(ctx.fpr[0].u32l()));
        g[T6] = lw(m, g[V1], 0);
        g[T7] = addu(g[T6], g[V0]);
        sw(m, g[T7], 0x3C, u64::from(ctx.fpr[2].u32l()));
        g[T8] = lw(m, g[V1], 0);
        g[T9] = addu(g[T8], g[V0]);
        sw(m, g[T9], 0x40, u64::from(ctx.fpr[0].u32l()));
        g[T0] = lw(m, g[V1], 0);
        g[T1] = addu(g[T0], g[V0]);
        sw(m, g[T1], 0x44, u64::from(ctx.fpr[0].u32l()));
        g[T2] = lw(m, g[V1], 0);
        g[T3] = addu(g[T2], g[V0]);
        sw(m, g[T3], 0x48, u64::from(ctx.fpr[0].u32l()));
        g[T4] = lw(m, g[V1], 0);
        g[T5] = addu(g[T4], g[V0]);
        sw(m, g[T5], 0x4C, u64::from(ctx.fpr[0].u32l()));
        g[T6] = lw(m, g[V1], 0);
        g[T7] = addu(g[T6], g[V0]);
        sw(m, g[T7], 0x50, u64::from(ctx.fpr[2].u32l()));
        g[T8] = lw(m, g[V1], 0);
        g[T9] = addu(g[T8], g[V0]);
        sw(m, g[T9], 8, 0);
        g[T0] = lw(m, g[V1], 0);
        g[T8] = sll(g[A0], 16);
        g[A0] = sra(g[T8], 16);
        g[T1] = addu(g[T0], g[V0]);
        sh(m, g[T1], 4, 0);
        g[T2] = lw(m, g[V1], 0);
        g[AT] = slt(g[A0], 0x20);
        g[T3] = addu(g[T2], g[V0]);
        sw(m, g[T3], 0x6C, u64::from(ctx.fpr[12].u32l()));
        g[T4] = lw(m, g[V1], 0);
        g[T5] = addu(g[T4], g[V0]);
        sw(m, g[T5], 0x70, u64::from(ctx.fpr[12].u32l()));
        g[T6] = lw(m, g[V1], 0);
        g[T7] = addu(g[T6], g[V0]);
        sw(m, g[T7], 0x74, u64::from(ctx.fpr[12].u32l()));
        if g[AT] == 0 {
            break;
        }
    }
    g[T0] = lw(m, g[V1], 0);
    g[AT] = li(0x800A_0000);
    g[V0] = li(0x800D_0000);
    sw(m, g[AT], -0x486C, g[T0]);
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], -0x4868, 0);
    g[V0] = addu(g[V0], 0x4B20);
    g[T1] = lw(m, g[V0], 0);
    g[AT] = (-2i64) as u64;
    sh(m, g[V0], 4, 0);
    g[T2] = g[T1] & g[AT];
    g[AT] = li(0x4120_0000);
    ctx.fpr[4].set_u32l(g[AT] as u32);
    sw(m, g[V0], 0, g[T2]);
    sh(m, g[V0], 6, 0);
    sw(m, g[V0], 0x20, u64::from(ctx.fpr[12].u32l()));
    sh(m, g[V0], 0x80, 0);
    sw(m, g[V0], 0x84, 0);
    sw(m, g[V0], 0x28, u64::from(ctx.fpr[2].u32l()));
    sw(m, g[V0], 0x2C, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x30, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x34, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x38, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x3C, u64::from(ctx.fpr[2].u32l()));
    sw(m, g[V0], 0x40, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x44, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x48, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x4C, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x50, u64::from(ctx.fpr[2].u32l()));
    sw(m, g[V0], 0x54, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x58, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x5C, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x60, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x64, u64::from(ctx.fpr[2].u32l()));
    sw(m, g[V0], 0x68, u64::from(ctx.fpr[12].u32l()));
    sw(m, g[V0], 0x6C, u64::from(ctx.fpr[12].u32l()));
    sw(m, g[V0], 0x70, u64::from(ctx.fpr[12].u32l()));
    g[A0] = 1;
    g[A1] = 4;
    sw(m, g[V0], 0x24, u64::from(ctx.fpr[4].u32l()));
    call(imports::func_8000AED4, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

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

/// Where one float field of [`entry_floats`] comes from.
#[derive(Clone, Copy)]
enum FloatArg {
    /// An argument register (`mtc1`).
    Reg(usize),
    /// A stack argument slot (`lwc1`).
    Stack(i32),
}

/// The shared body of [`func_8000AF4C`] and [`func_8000AFD4`]: store float
/// arguments into entry `id` of [`ENTRIES`] (`a0`'s low halfword, signed;
/// `0x7C * id`, no bounds). `a0` goes to `[sp]` and `a3` to `[sp + 0xC]`
/// first. The entry array pointer is re-read before every field (QUIRK:
/// a store over [`ENTRIES`] redirects the rest). Field `k` uses the
/// temporaries `TEMPS[2k]`, `TEMPS[2k + 1]` for the pointer and address.
fn entry_floats(m: &mut Mem, g: &mut [u64; 32], f: &mut [crate::recomp::Fpr; 32], fields: &[(usize, FloatArg, i32)]) {
    const TEMPS: [usize; 12] = [T8, T9, T0, T1, T2, T3, T4, T5, T6, T7, T8, T9];
    g[T6] = sll(g[A0], 16);
    g[V1] = li(ENTRIES);
    g[T7] = sra(g[T6], 16);
    // The first field's pointer is read before the spills.
    g[TEMPS[0]] = lw(m, g[V1], 0);
    g[V0] = sll(g[T7], 5);
    g[V0] = subu(g[V0], g[T7]);
    g[V0] = sll(g[V0], 2);
    sw(m, g[SP], 0, g[A0]);
    sw(m, g[SP], 0xC, g[A3]);
    for (k, &(fr, src, off)) in fields.iter().enumerate() {
        let (base, at) = (TEMPS[2 * k], TEMPS[2 * k + 1]);
        if k > 0 {
            g[base] = lw(m, g[V1], 0);
        }
        let bits = match src {
            FloatArg::Reg(r) => g[r] as u32,
            FloatArg::Stack(s) => lw(m, g[SP], s) as u32,
        };
        f[fr].set_u32l(bits);
        g[at] = addu(g[base], g[V0]);
        sw(m, g[at], off, u64::from(f[fr].u32l()));
    }
}

/// `func_8000AF4C(id, a, b, c, d, e, f)`: entry `id` of [`ENTRIES`] gets
/// the six floats at `+0x54..+0x6C` (`a`, `b` in `a1`/`a2`, `c` in `a3`,
/// through its spill, `d`..`f` from the stack arguments at `sp + 0x10..`),
/// through [`entry_floats`].
///
/// Leaves `v1 = ENTRIES`, `v0 = 0x7C * id`, `t0`..`t9` = the pointer and
/// field addresses, `f12`, `f14`, `f4`..`f10` = the floats.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AF4C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    use FloatArg::{Reg, Stack};
    entry_floats(&mut mem, &mut ctx.gpr, &mut ctx.fpr, &[
        (12, Reg(A1), 0x54), (14, Reg(A2), 0x58), (4, Stack(0xC), 0x5C),
        (6, Stack(0x10), 0x60), (8, Stack(0x14), 0x64), (10, Stack(0x18), 0x68),
    ]);
}

/// `func_8000AFD4(id, a, b, c)`: entry `id` of [`ENTRIES`] gets the floats
/// `+0x6C = a`, `+0x70 = b` (`a1`/`a2`) and `+0x74 = c` (`a3`, through its
/// spill), through [`entry_floats`].
///
/// Leaves `v1 = ENTRIES`, `v0 = 0x7C * id`, `t6 = a0 << 16`, `t7` = the id,
/// `t8`..`t3` = the pointer and field addresses, `f12`, `f14`, `f4`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000AFD4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    use FloatArg::{Reg, Stack};
    entry_floats(&mut mem, &mut ctx.gpr, &mut ctx.fpr, &[(12, Reg(A1), 0x6C), (14, Reg(A2), 0x70), (4, Stack(0xC), 0x74)]);
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

/// `func_8000B0E0(id, o)` (select entry `id` for `o`, **guess**: entries
/// behind `[0x8009B790]`, 0x7C bytes, flags at `+0`): if `o` is nonzero:
/// if `o`'s current entry `e = [o + 4]` ([`func_80017EE4`]) is not -1, bit
/// 0 of entry `e`'s flags is cleared (`e` read again); then if `id` (`a0`'s
/// low halfword, signed) is not -1, `[o + 4] = id` ([`func_80017EEC`]),
/// bit 0 of the selected entry `[0x8009B798]`'s flags is set and
/// `[0x8009B79C] = 0`; if `id` is -1, the selection becomes -1.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`; `id` and `o` spilled to their home
/// slots `sp + 0`, `sp + 4`, with `id`'s halfword stored again at `sp + 2`
/// around the calls. Leaves `a2` = the id, `at`, `t6`..`t5` and the
/// callees' registers.
///
/// Domain: the entries touched in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000B0E0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[A2] = sll(g[A0], 16);
    g[T6] = sra(g[A2], 16);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x18, g[A0]);
    if g[A1] != 0 {
        g[A0] = g[A1];
        sw(m, g[SP], 0x1C, g[A1]);
        sh(m, g[SP], 0x1A, g[T6]);
        call(imports::func_80017EE4, m, ctx);
        let g = &mut ctx.gpr;
        g[AT] = u64::MAX;
        g[A2] = lh(m, g[SP], 0x1A);
        if g[V0] != g[AT] {
            g[A0] = lw(m, g[SP], 0x1C);
            sh(m, g[SP], 0x1A, g[A2]);
            call(imports::func_80017EE4, m, ctx);
            let g = &mut ctx.gpr;
            g[T7] = li(0x800A_0000);
            g[T7] = lw(m, g[T7], -0x4870);
            g[T8] = sll(g[V0], 5);
            g[T8] = subu(g[T8], g[V0]);
            g[T8] = sll(g[T8], 2);
            g[V1] = addu(g[T7], g[T8]);
            g[T9] = lw(m, g[V1], 0);
            g[AT] = (-2i64) as u64;
            g[A2] = lh(m, g[SP], 0x1A);
            g[T0] = g[T9] & g[AT];
            sw(m, g[V1], 0, g[T0]);
        }
        let g = &mut ctx.gpr;
        g[AT] = u64::MAX;
        g[A0] = lw(m, g[SP], 0x1C);
        if g[A2] != g[AT] {
            g[A1] = g[A2];
            call(imports::func_80017EEC, m, ctx);
            let g = &mut ctx.gpr;
            g[T2] = li(0x800A_0000);
            g[T2] = lw(m, g[T2], -0x4868);
            g[T1] = li(0x800A_0000);
            g[T1] = lw(m, g[T1], -0x4870);
            g[T3] = sll(g[T2], 5);
            g[T3] = subu(g[T3], g[T2]);
            g[T3] = sll(g[T3], 2);
            g[V0] = addu(g[T1], g[T3]);
            g[T4] = lw(m, g[V0], 0);
            g[AT] = li(0x800A_0000);
            g[T5] = g[T4] | 1;
            sw(m, g[V0], 0, g[T5]);
            sw(m, g[AT], -0x4864, 0);
        } else {
            g[AT] = li(0x800A_0000);
            sw(m, g[AT], -0x4868, g[A2]);
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
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

/// `func_800078B4()`: if the flag `[0x8009A2B8]` is set (**guess**: audio
/// running), scale the word `+0x18` of each of the eight 0x20-byte records
/// at `0x800D2038` by the float `K = [0x800A81C4]`: for records whose `+4`
/// isn't `0x4E`, `+0x18 = trunc(f32(+0x18) * K)` (a C cast: out of range
/// or NaN gives `0x80000000`); then for every record, `+0x18 = 0` if it is
/// negative. Each step re-reads the word.
///
/// Leaves `v1 = 0x800D2038` and `at = 0x800B0000`, `t6` = the flag; with
/// the flag set, `v1 = a0 = 0x800D2138`, `v0 = 0x4E`, `f0 = K`, `f2 = 0`,
/// and the temporaries of the last two records (`t7 t8 t0 t1` for even
/// records, `t2 t3 t5 t6` for odd ones; `f4`..`f18`).
///
/// Domain: `K` not NaN (the product is guarded).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800078B4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = lw(m, li(0x800A_0000), -0x5D48);
    g[V1] = li(0x800D_2038);
    g[AT] = li(0x800B_0000);
    if g[T6] == 0 {
        return;
    }
    g[A0] = li(0x800D_2138);
    f[2].set_u32l(0);
    f[0].set_u32l(lw(m, g[AT], -0x7E3C) as u32);
    g[V0] = 0x4E;
    loop {
        for r in 0..4 {
            let off = 0x20 * r;
            let [tag, old, new, word] = if r % 2 == 0 { [T7, T8, T0, T1] } else { [T2, T3, T5, T6] };
            g[tag] = lw(m, g[V1], off + 4);
            if g[V0] != g[tag] {
                g[old] = lw(m, g[V1], off + 0x18);
                f[4].set_u32l(g[old] as u32);
                f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
                f[8].set_fl(f[6].fl() * f[0].fl());
                f[10].set_u32l(fpu::trunc_w_s(f[8].fl()));
                g[new] = s32(f[10].u32l());
                sw(m, g[V1], off + 0x18, g[new]);
            }
            g[word] = lw(m, g[V1], off + 0x18);
            f[16].set_u32l(g[word] as u32);
            f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fpu::NEAREST));
            if f[18].fl() < f[2].fl() {
                sw(m, g[V1], off + 0x18, 0);
            }
        }
        g[V1] = addu(g[V1], 0x80);
        if g[V1] == g[A0] {
            break;
        }
    }
}

/// `func_8000B254(mode, n, m)` (a node's transform as a 4x4 matrix,
/// **guess**): mode 3, [`func_80017C98`]`(n, m)`; mode 2,
/// [`func_80017C18`]`(n, m)`; otherwise [`func_80017C98`]`(n, m)` if the
/// node's type word `[n]` ([`func_80017DA4`]) is `0xD065`, else identity
/// ([`func_80017874`](crate::math::func_80017874)`(m)`). QUIRK: for other modes it tests the type
/// twice, and the second test's [`func_80017C18`] branch can't be taken.
/// All compares are 64-bit.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`; `n`, `m` spilled to their home
/// slots `sp + 4`, `sp + 8` around the type tests. Leaves `a3 = n`, `at`
/// and the callees' registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000B254(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[AT] = 3;
    sw(m, g[SP], 0x14, g[RA]);
    g[A3] = g[A1];
    if g[A0] != g[AT] {
        g[AT] = 2;
        g[A1] = g[A2];
        if g[A0] != g[AT] {
            g[A0] = g[A3];
            sw(m, g[SP], 0x20, g[A2]);
            sw(m, g[SP], 0x1C, g[A3]);
            call(imports::func_80017DA4, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = 0xD065;
            g[A2] = lw(m, g[SP], 0x20);
            g[A3] = lw(m, g[SP], 0x1C);
            if g[V0] != g[AT] {
                g[A0] = g[A3];
                sw(m, g[SP], 0x20, g[A2]);
                sw(m, g[SP], 0x1C, g[A3]);
                call(imports::func_80017DA4, m, ctx);
                let g = &mut ctx.gpr;
                g[AT] = 0xD065;
                g[A2] = lw(m, g[SP], 0x20);
                g[A3] = lw(m, g[SP], 0x1C);
                if g[V0] != g[AT] {
                    g[A0] = g[A2];
                    call(imports::func_80017874, m, ctx);
                    let g = &mut ctx.gpr;
                    g[RA] = lw(m, g[SP], 0x14);
                } else {
                    g[A0] = g[A3];
                    g[A1] = g[A2];
                    call(imports::func_80017C18, m, ctx);
                    let g = &mut ctx.gpr;
                    g[RA] = lw(m, g[SP], 0x14);
                }
            } else {
                g[A0] = g[A3];
                g[A1] = g[A2];
                call(imports::func_80017C98, m, ctx);
                let g = &mut ctx.gpr;
                g[RA] = lw(m, g[SP], 0x14);
            }
        } else {
            g[A0] = g[A3];
            call(imports::func_80017C18, m, ctx);
            let g = &mut ctx.gpr;
            g[RA] = lw(m, g[SP], 0x14);
        }
    } else {
        g[A0] = g[A1];
        g[A1] = g[A2];
        call(imports::func_80017C98, m, ctx);
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x14);
    }
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], 0x18);
}

/// `func_8000B98C(out, p)` (the nearer selected entry's matrix, **guess**):
/// for the records `r1`, `r2` of [`func_80017F28`] (1 and 2), `d_k` is -1.0,
/// or, if bit 0 of `[r_k]` is set, the squared distance `dz*dz + (dx*dx +
/// dy*dy)` from `p` to the position `+0x44..+0x4C` of entry `e_k = [r_k +
/// 4]` behind [`ENTRIES`] (`d = p - pos`). Then `out` = the matrix `+0x14`
/// of entry `e_2` ([`func_800156DC`](crate::math::func_800156DC)) if `0 <=
/// d_2` and `!(d_1 < d_2)`; else that of `e_1` if `0 <= d_1`; else the
/// identity ([`func_80017874`](crate::math::func_80017874)). QUIRK: with only
/// `r2` selected, `d_1 = -1 < d_2`, so the result is the identity, not
/// `e_2`'s matrix.
///
/// Frame (`sp - 0x40`): `ra`, `s0` at `+0x1C`, `+0x18` (`s0 = p`
/// meanwhile), `d_1`, `d_2` at `+0x24`, `+0x20`, the records at `+0x2C`,
/// `+0x28`, `[r_1]` at `+0x3C`, `[r_2]` at `+0x38` (if `r_1` is selected),
/// `e_1`, `e_2` at `+0x34`, `+0x30` (only for selected records); `out` spilled to its home
/// slot `sp + 0`. Leaves `a2`, `a3 = 0x7C` and the callees' registers.
///
/// Domain: the records' entries in RDRAM; no NaN operand of the distances.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000B98C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    g[AT] = li(0xBF80_0000);
    ctx.fpr[0].set_u32l(g[AT] as u32);
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x18, g[S0]);
    sw(m, g[SP], 0x40, g[A0]);
    g[S0] = g[A1];
    g[A0] = 1;
    sw(m, g[SP], 0x24, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[SP], 0x20, u64::from(ctx.fpr[0].u32l()));
    call(imports::func_80017F28, m, ctx);
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0x2C, g[V0]);
    g[A0] = 2;
    call(imports::func_80017F28, m, ctx);
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0x28, g[V0]);
    g[A0] = lw(m, g[SP], 0x2C);
    call(imports::func_80017EF4, m, ctx);
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0x3C, g[V0]);
    g[A0] = lw(m, g[SP], 0x28);
    call(imports::func_80017EF4, m, ctx);
    let g = &mut ctx.gpr;
    g[T6] = lw(m, g[SP], 0x3C);
    g[A1] = g[V0];
    g[A0] = lw(m, g[SP], 0x2C);
    g[T7] = g[T6] & 1;
    if g[T7] != 0 {
        sw(m, g[SP], 0x38, g[V0]);
        call(imports::func_80017EE4, m, ctx);
        let g = &mut ctx.gpr;
        g[A3] = 0x7C;
        let (lo, _) = multu(g[V0], g[A3]);
        g[A2] = li(0x8009_B790);
        g[T8] = lw(m, g[A2], 0);
        sw(m, g[SP], 0x34, g[V0]);
        ctx.fpr[8].set_u32l(lw(m, g[S0], 0) as u32);
        ctx.fpr[16].set_u32l(lw(m, g[S0], 4) as u32);
        ctx.fpr[4].set_u32l(lw(m, g[S0], 8) as u32);
        g[A1] = lw(m, g[SP], 0x38);
        g[T9] = lo;
        g[V1] = addu(g[T8], g[T9]);
        ctx.fpr[10].set_u32l(lw(m, g[V1], 0x44) as u32);
        ctx.fpr[6].set_u32l(lw(m, g[V1], 0x4C) as u32);
        ctx.fpr[18].set_u32l(lw(m, g[V1], 0x48) as u32);
        ctx.fpr[2].set_fl(ctx.fpr[8].fl() - ctx.fpr[10].fl());
        ctx.fpr[0].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
        ctx.fpr[4].set_fl(ctx.fpr[2].fl() * ctx.fpr[2].fl());
        ctx.fpr[12].set_fl(ctx.fpr[16].fl() - ctx.fpr[18].fl());
        ctx.fpr[6].set_fl(ctx.fpr[12].fl() * ctx.fpr[12].fl());
        ctx.fpr[8].set_fl(ctx.fpr[4].fl() + ctx.fpr[6].fl());
        ctx.fpr[10].set_fl(ctx.fpr[0].fl() * ctx.fpr[0].fl());
        ctx.fpr[16].set_fl(ctx.fpr[10].fl() + ctx.fpr[8].fl());
        sw(m, g[SP], 0x24, u64::from(ctx.fpr[16].u32l()));
    }
    let g = &mut ctx.gpr;
    g[A2] = li(0x800A_0000);
    g[T0] = g[A1] & 1;
    g[A2] = addu(g[A2], (-0x4870i64) as u64);
    g[A3] = 0x7C;
    if g[T0] != 0 {
        g[A0] = lw(m, g[SP], 0x28);
        call(imports::func_80017EE4, m, ctx);
        let g = &mut ctx.gpr;
        g[A3] = 0x7C;
        let (lo, _) = multu(g[V0], g[A3]);
        g[A2] = li(0x8009_B790);
        g[T1] = lw(m, g[A2], 0);
        sw(m, g[SP], 0x30, g[V0]);
        ctx.fpr[6].set_u32l(lw(m, g[S0], 0) as u32);
        ctx.fpr[8].set_u32l(lw(m, g[S0], 4) as u32);
        ctx.fpr[18].set_u32l(lw(m, g[S0], 8) as u32);
        g[T2] = lo;
        g[V1] = addu(g[T1], g[T2]);
        ctx.fpr[10].set_u32l(lw(m, g[V1], 0x44) as u32);
        ctx.fpr[4].set_u32l(lw(m, g[V1], 0x4C) as u32);
        ctx.fpr[16].set_u32l(lw(m, g[V1], 0x48) as u32);
        ctx.fpr[2].set_fl(ctx.fpr[6].fl() - ctx.fpr[10].fl());
        ctx.fpr[0].set_fl(ctx.fpr[18].fl() - ctx.fpr[4].fl());
        ctx.fpr[18].set_fl(ctx.fpr[2].fl() * ctx.fpr[2].fl());
        ctx.fpr[12].set_fl(ctx.fpr[8].fl() - ctx.fpr[16].fl());
        ctx.fpr[4].set_fl(ctx.fpr[12].fl() * ctx.fpr[12].fl());
        ctx.fpr[6].set_fl(ctx.fpr[18].fl() + ctx.fpr[4].fl());
        ctx.fpr[10].set_fl(ctx.fpr[0].fl() * ctx.fpr[0].fl());
        ctx.fpr[14].set_fl(ctx.fpr[10].fl() + ctx.fpr[6].fl());
        sw(m, g[SP], 0x20, u64::from(ctx.fpr[14].u32l()));
    }
    let g = &mut ctx.gpr;
    'b_8000BB68: {
        ctx.fpr[0].set_u32l(0);
        ctx.fpr[14].set_u32l(lw(m, g[SP], 0x20) as u32);
        ctx.fpr[8].set_u32l(lw(m, g[SP], 0x24) as u32);
        ctx.fpr[16].set_u32l(lw(m, g[SP], 0x24) as u32);
        // f14 = d_2, f8 = f16 = d_1: e_2 if 0 <= d_2 and not d_1 < d_2,
        // else e_1 if 0 <= d_1 (both paths test that last), else identity.
        if !(ctx.fpr[14].fl() < ctx.fpr[0].fl()) {
            let nearer = ctx.fpr[8].fl() < ctx.fpr[14].fl();
            g[T7] = lw(m, g[SP], 0x30);
            if !nearer {
                let (lo, _) = multu(g[T7], g[A3]);
                g[T6] = lw(m, g[A2], 0);
                g[A0] = lw(m, g[SP], 0x40);
                g[T8] = lo;
                g[A1] = addu(g[T6], g[T8]);
                g[A1] = addu(g[A1], 0x14);
                call(imports::func_800156DC, m, ctx);
                let g = &mut ctx.gpr;
                g[RA] = lw(m, g[SP], 0x1C);
                break 'b_8000BB68;
            }
        }
        let g = &mut ctx.gpr;
        g[T4] = lw(m, g[SP], 0x34);
        if !(ctx.fpr[16].fl() < ctx.fpr[0].fl()) {
            let (lo, _) = multu(g[T4], g[A3]);
            g[T3] = lw(m, g[A2], 0);
            g[A0] = lw(m, g[SP], 0x40);
            g[T5] = lo;
            g[A1] = addu(g[T3], g[T5]);
            g[A1] = addu(g[A1], 0x14);
            call(imports::func_800156DC, m, ctx);
            let g = &mut ctx.gpr;
            g[RA] = lw(m, g[SP], 0x1C);
        } else {
            g[A0] = lw(m, g[SP], 0x40);
            call(imports::func_80017874, m, ctx);
            let g = &mut ctx.gpr;
            g[RA] = lw(m, g[SP], 0x1C);
        }
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x40);
}

/// `func_8000BB78(o)` (is `o` selected, **guess**): 1 if one of the
/// [`func_80017F20`] (4) records `r` of [`func_80017F28`] is nonzero, has
/// bit 0 of `[r]` set and `[r + 4] == o` (64-bit compare), checked in
/// order; else 0.
///
/// Frame (`sp - 0x28`): `ra`, `s3`..`s0` at `+0x24..+0x14`, restored (`s3 =
/// o`, `s2` the count, `s1` the index, `s0` the record meanwhile). Leaves
/// `t6`, `at` and the callees' registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000BB78(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8000BBF4: {
        g[SP] = addu(g[SP], (-0x28i64) as u64);
        sw(m, g[SP], 0x24, g[RA]);
        sw(m, g[SP], 0x20, g[S3]);
        g[S3] = g[A0];
        sw(m, g[SP], 0x1C, g[S2]);
        sw(m, g[SP], 0x18, g[S1]);
        sw(m, g[SP], 0x14, g[S0]);
        call(imports::func_80017F20, m, ctx);
        let g = &mut ctx.gpr;
        g[S2] = g[V0];
        g[S1] = 0;
        if (g[V0] as i64) > 0 {
            loop {
                let g = &mut ctx.gpr;
                g[A0] = g[S1];
                call(imports::func_80017F28, m, ctx);
                let g = &mut ctx.gpr;
                g[S0] = g[V0];
                if g[V0] == 0 {
                    g[S1] = addu(g[S1], 1);
                } else {
                    g[A0] = g[V0];
                    call(imports::func_80017EF4, m, ctx);
                    let g = &mut ctx.gpr;
                    g[T6] = g[V0] & 1;
                    if g[T6] == 0 {
                        g[S1] = addu(g[S1], 1);
                    } else {
                        g[A0] = g[S0];
                        call(imports::func_80017EE4, m, ctx);
                        let g = &mut ctx.gpr;
                        if g[V0] == g[S3] {
                            g[V0] = 1;
                            break 'b_8000BBF4;
                        }
                        g[S1] = addu(g[S1], 1);
                    }
                }
                let g = &mut ctx.gpr;
                g[AT] = slt(g[S1], g[S2]);
                if g[AT] == 0 {
                    break;
                }
            }
        }
        let g = &mut ctx.gpr;
        g[V0] = 0;
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x24);
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[S2] = lw(m, g[SP], 0x1C);
    g[S3] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x28);
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

/// `func_8000C6C8(p, x, y, lo, hi)` with the floats `x`, `y` in `a1`/`a2`,
/// `lo` in `a3` and `hi` on the stack (`sp + 0x10`): `*p += x * y` (not
/// fused), then `*p = lo` if `*p < lo`, then `*p = hi` if `hi < *p`, each
/// test re-reading `*p`. So `hi` wins if `hi < lo`; a NaN sum is stored and
/// stays (both compares false). `a3` is spilled to `[sp + 0xC]` first.
///
/// Leaves `f12 = x`, `f14 = y`, `f4` = the old value, `f6 = x * y`, `f8` =
/// the sum, `f10 = lo`, `f0` = `*p` before the last test, `f2 = hi`.
///
/// Domain: `x`, `y`, the old `*p` and the product not NaN; `lo`, `hi` any.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000C6C8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A1] as u32);
    f[14].set_u32l(g[A2] as u32);
    sw(m, g[SP], 0xC, g[A3]);
    f[4].set_u32l(lw(m, g[A0], 0) as u32);
    f[6].set_fl(f[12].fl() * f[14].fl());
    f[8].set_fl(f[4].fl() + f[6].fl());
    sw(m, g[A0], 0, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[0].set_u32l(lw(m, g[A0], 0) as u32);
    if f[0].fl() < f[10].fl() {
        sw(m, g[A0], 0, u64::from(f[10].u32l()));
        f[0].set_u32l(lw(m, g[A0], 0) as u32);
    }
    f[2].set_u32l(lw(m, g[SP], 0x10) as u32);
    if f[2].fl() < f[0].fl() {
        sw(m, g[A0], 0, u64::from(f[2].u32l()));
    }
}

/// `func_8000C724(p, x, y, lo, hi)`: the integer version of
/// [`func_8000C6C8`]: `v = trunc(f32(*p) + x * y)` (a C cast: `0x80000000`
/// out of range), stored; if `v < lo` (64-bit signed, `lo` = the whole
/// `a3`), `*p = lo` and `v = lo`; then if `hi < v` (`hi` = the word at `sp +
/// 0x10`, sign-extended), `*p = hi`. Returns `v` (before the `hi` clamp).
///
/// Leaves `t6` = the old `*p`, `f12 = x`, `f14 = y`, `f4`/`f6` = it as an
/// int and a float, `f8 = x * y`, `f10` = the sum, `f16` = the truncation,
/// `v1 = hi`, `at` = the last test.
///
/// Domain: `x`, `y` and the product not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000C724(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = lw(m, g[A0], 0);
    f[12].set_u32l(g[A1] as u32);
    f[14].set_u32l(g[A2] as u32);
    f[4].set_u32l(g[T6] as u32);
    f[8].set_fl(f[12].fl() * f[14].fl());
    f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    f[10].set_fl(f[6].fl() + f[8].fl());
    f[16].set_u32l(fpu::trunc_w_s(f[10].fl()));
    g[V0] = s32(f[16].u32l());
    g[AT] = slt(g[V0], g[A3]);
    sw(m, g[A0], 0, g[V0]);
    if g[AT] != 0 {
        sw(m, g[A0], 0, g[A3]);
        g[V0] = g[A3];
    }
    g[V1] = lw(m, g[SP], 0x10);
    g[AT] = slt(g[V1], g[V0]);
    if g[AT] != 0 {
        sw(m, g[A0], 0, g[V1]);
    }
}

/// `func_8000CC1C(k, x)` (edit tuning value `k` of the selected "Test"
/// element, **guess**: a debug menu): with the float `x` in `a1`, `[0x8009B804]
/// = 1` and `e` = [`func_8003F714`](crate::pools::func_8003F714)`("Test",
/// T)` for the selected tag `T = [0x8009B7E0]`; nothing more if `e` is 0 or
/// `k >= 17` (unsigned). Otherwise by `k`:
/// - 1..15: [`func_8000C6C8`]`(e + o, x, y, lo, hi)` (`*p = clamp(*p + x *
///   y)`), then the result copied to `[[e + 0x1E70] + o']`, with `(o, y,
///   lo, hi, o')` = 1 `(0x6C, 0.01, K0, 1.0, 0x1C)`, 2 `(0x70, 1, 10,
///   1000, 0x20)`, 3 `(0x74, 1, 10, 1000, 0x24)`, 4 `(0x78, 0.01, 0.02, 10,
///   0x28)`, 5 `(0x7C, 1, 100, 2000, 0x2C)`, 6 `(0x80, 0.5, 2, 1000, 0x30)`,
///   7 `(0x84, 0.5, 2, 1000, 0x34)`, 8 `(0x88, 1, 10, 1000, 0x38)`, 9
///   `(0x8C, K1, K1, 30, 0x3C)`, 10 `(0x90, K2, K2, 20, 0x40)`, 11 `(0x94,
///   K3, 3, 30, 0x44)`, 12 `(0x98, 0.01, 0, 1, 0x48)`, 13 `(0x9C, K4, 1, 100,
///   0x4C)`, 14 `(0xA0, 0.01, 0, 1, 0x50)`, 15 `(0xA8, K5, K5, 20, 0x54)`;
///   `K0`..`K5` are the floats at `0x800A85F0`, `F4`, `F8`, `FC`,
///   `0x800A8600`, `04`.
/// - 16: `[e + 0x108] = s * s` with `s = sqrt([e + 0x108])` clamped through
///   the frame word `+0x2C` by `(x, 1, 10, 500)`.
/// - 0: step the selection: `T = trunc(f32(T) + d)` with `d = 1.0` if `0 <
///   x`, else -1.0; done if an element with tag `T` exists and bit 8 of its
///   halfword `+6` is clear. Otherwise, stepping up, `T = 0`; stepping down,
///   `T` goes up by one while the next tag is such an element and then back
///   by one (QUIRK: past the first element it wraps to the top of the
///   contiguous run above `T`, as the loop is written).
///
/// Frame (`sp - 0x38`): `ra`, `s1`, `s0` at `+0x24`, `+0x20`, `+0x1C`,
/// restored (`s0 = 0x8009B7E0`, `s1 = e`); `x` (or `d`) spilled at `+0x3C`
/// around the calls, `hi` at `+0x10` for [`func_8000C6C8`], `s` at `+0x2C`
/// (case 16); `k` in its home slot `sp + 0`. Leaves `f0`..`f18` and the
/// callees' registers as the case used them.
///
/// Domain: `k < 17` a case with its element's fields in RDRAM; no NaN
/// operand of the arithmetic (the callees' included); for case 0 a pool
/// whose selectable tags are finite in number.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000CC1C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8000D120: {
        'b_8000D11C: {
            g[SP] = addu(g[SP], (-0x38i64) as u64);
            sw(m, g[SP], 0x1C, g[S0]);
            ctx.fpr[12].set_u32l(g[A1] as u32);
            g[S0] = li(0x800A_0000);
            sw(m, g[SP], 0x38, g[A0]);
            g[T6] = 1;
            g[AT] = li(0x800A_0000);
            g[S0] = addu(g[S0], (-0x4820i64) as u64);
            sw(m, g[SP], 0x24, g[RA]);
            sw(m, g[AT], -0x47FC, g[T6]);
            g[A0] = li(0x5465_0000);
            sw(m, g[SP], 0x20, g[S1]);
            g[A0] = g[A0] | 0x7374;
            g[A1] = lw(m, g[S0], 0);
            sw(m, g[SP], 0x3C, u64::from(ctx.fpr[12].u32l()));
            call(imports::func_8003F714, m, ctx);
            let g = &mut ctx.gpr;
            ctx.fpr[12].set_u32l(lw(m, g[SP], 0x3C) as u32);
            g[S1] = g[V0];
            if g[V0] != 0 {
                g[T7] = lw(m, g[SP], 0x38);
                g[AT] = sltu(g[T7], 0x11);
                g[T7] = sll(g[T7], 2);
                if g[AT] != 0 {
                    g[AT] = li(0x800B_0000);
                    let jr_addend_8000CC84 = g[T7];
                    g[AT] = addu(g[AT], g[T7]);
                    g[T7] = addu(g[AT], (-0x7A54i64) as u64);
                    match jr_addend_8000CC84 >> 2 {
                        0 => {}
                        1 => {
                            g[AT] = li(0x800B_0000);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            ctx.fpr[0].set_u32l(lw(m, g[AT], -0x7A10) as u32);
                            g[AT] = li(0x3F80_0000);
                            ctx.fpr[16].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = li(0x3C23_0000);
                            g[A3] = s32(ctx.fpr[0].u32l());
                            g[A2] = g[A2] | 0xD70A;
                            g[A0] = addu(g[S1], 0x6C);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[16].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[18].set_u32l(lw(m, g[S1], 0x6C) as u32);
                            g[T9] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T9], 0x1C, u64::from(ctx.fpr[18].u32l()));
                            break 'b_8000D11C;
                        }
                        2 => {
                            g[A0] = addu(g[S1], 0x70);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            g[AT] = li(0x447A_0000);
                            ctx.fpr[4].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = li(0x3F80_0000);
                            g[A3] = li(0x4120_0000);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[4].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[6].set_u32l(lw(m, g[S1], 0x70) as u32);
                            g[T0] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T0], 0x20, u64::from(ctx.fpr[6].u32l()));
                            break 'b_8000D11C;
                        }
                        3 => {
                            g[A0] = addu(g[S1], 0x74);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            g[AT] = li(0x447A_0000);
                            ctx.fpr[8].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = li(0x3F80_0000);
                            g[A3] = li(0x4120_0000);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[8].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[10].set_u32l(lw(m, g[S1], 0x74) as u32);
                            g[T1] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T1], 0x24, u64::from(ctx.fpr[10].u32l()));
                            break 'b_8000D11C;
                        }
                        4 => {
                            g[A0] = addu(g[S1], 0x78);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            g[AT] = li(0x4120_0000);
                            ctx.fpr[16].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = li(0x3C23_0000);
                            g[A3] = li(0x3CA3_D70A);
                            g[A2] = g[A2] | 0xD70A;
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[16].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[18].set_u32l(lw(m, g[S1], 0x78) as u32);
                            g[T2] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T2], 0x28, u64::from(ctx.fpr[18].u32l()));
                            break 'b_8000D11C;
                        }
                        5 => {
                            g[A0] = addu(g[S1], 0x7C);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            g[AT] = li(0x44FA_0000);
                            ctx.fpr[4].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = li(0x3F80_0000);
                            g[A3] = li(0x42C8_0000);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[4].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[6].set_u32l(lw(m, g[S1], 0x7C) as u32);
                            g[T3] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T3], 0x2C, u64::from(ctx.fpr[6].u32l()));
                            break 'b_8000D11C;
                        }
                        6 => {
                            g[A0] = addu(g[S1], 0x80);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            g[AT] = li(0x447A_0000);
                            ctx.fpr[8].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = li(0x3F00_0000);
                            g[A3] = li(0x4000_0000);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[8].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[10].set_u32l(lw(m, g[S1], 0x80) as u32);
                            g[T4] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T4], 0x30, u64::from(ctx.fpr[10].u32l()));
                            break 'b_8000D11C;
                        }
                        7 => {
                            g[A0] = addu(g[S1], 0x84);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            g[AT] = li(0x447A_0000);
                            ctx.fpr[16].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = li(0x3F00_0000);
                            g[A3] = li(0x4000_0000);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[16].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[18].set_u32l(lw(m, g[S1], 0x84) as u32);
                            g[T5] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T5], 0x34, u64::from(ctx.fpr[18].u32l()));
                            break 'b_8000D11C;
                        }
                        8 => {
                            g[A0] = addu(g[S1], 0x88);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            g[AT] = li(0x447A_0000);
                            ctx.fpr[4].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = li(0x3F80_0000);
                            g[A3] = li(0x4120_0000);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[4].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[6].set_u32l(lw(m, g[S1], 0x88) as u32);
                            g[T6] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T6], 0x38, u64::from(ctx.fpr[6].u32l()));
                            break 'b_8000D11C;
                        }
                        9 => {
                            g[AT] = li(0x800B_0000);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            ctx.fpr[0].set_u32l(lw(m, g[AT], -0x7A0C) as u32);
                            g[AT] = li(0x41F0_0000);
                            ctx.fpr[8].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = s32(ctx.fpr[0].u32l());
                            g[A3] = s32(ctx.fpr[0].u32l());
                            g[A0] = addu(g[S1], 0x8C);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[8].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[10].set_u32l(lw(m, g[S1], 0x8C) as u32);
                            g[T7] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T7], 0x3C, u64::from(ctx.fpr[10].u32l()));
                            break 'b_8000D11C;
                        }
                        10 => {
                            g[AT] = li(0x800B_0000);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            ctx.fpr[0].set_u32l(lw(m, g[AT], -0x7A08) as u32);
                            g[AT] = li(0x41A0_0000);
                            ctx.fpr[16].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = s32(ctx.fpr[0].u32l());
                            g[A3] = s32(ctx.fpr[0].u32l());
                            g[A0] = addu(g[S1], 0x90);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[16].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[18].set_u32l(lw(m, g[S1], 0x90) as u32);
                            g[T8] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T8], 0x40, u64::from(ctx.fpr[18].u32l()));
                            break 'b_8000D11C;
                        }
                        11 => {
                            g[AT] = li(0x800B_0000);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            ctx.fpr[0].set_u32l(lw(m, g[AT], -0x7A04) as u32);
                            g[AT] = li(0x41F0_0000);
                            ctx.fpr[4].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = s32(ctx.fpr[0].u32l());
                            g[A0] = addu(g[S1], 0x94);
                            g[A3] = li(0x4040_0000);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[4].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[6].set_u32l(lw(m, g[S1], 0x94) as u32);
                            g[T9] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T9], 0x44, u64::from(ctx.fpr[6].u32l()));
                            break 'b_8000D11C;
                        }
                        12 => {
                            g[A0] = addu(g[S1], 0x98);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            g[AT] = li(0x3F80_0000);
                            ctx.fpr[8].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = li(0x3C23_D70A);
                            g[A3] = 0;
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[8].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[10].set_u32l(lw(m, g[S1], 0x98) as u32);
                            g[T0] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T0], 0x48, u64::from(ctx.fpr[10].u32l()));
                            break 'b_8000D11C;
                        }
                        13 => {
                            g[AT] = li(0x800B_0000);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            ctx.fpr[0].set_u32l(lw(m, g[AT], -0x7A00) as u32);
                            g[AT] = li(0x42C8_0000);
                            ctx.fpr[16].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = s32(ctx.fpr[0].u32l());
                            g[A0] = addu(g[S1], 0x9C);
                            g[A3] = li(0x3F80_0000);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[16].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[18].set_u32l(lw(m, g[S1], 0x9C) as u32);
                            g[T1] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T1], 0x4C, u64::from(ctx.fpr[18].u32l()));
                            break 'b_8000D11C;
                        }
                        14 => {
                            g[A0] = addu(g[S1], 0xA0);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            g[AT] = li(0x3F80_0000);
                            ctx.fpr[4].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = li(0x3C23_D70A);
                            g[A3] = 0;
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[4].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[6].set_u32l(lw(m, g[S1], 0xA0) as u32);
                            g[T2] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T2], 0x50, u64::from(ctx.fpr[6].u32l()));
                            break 'b_8000D11C;
                        }
                        15 => {
                            g[AT] = li(0x800B_0000);
                            if g[V0] == 0 {
                                break 'b_8000D11C;
                            }
                            ctx.fpr[0].set_u32l(lw(m, g[AT], -0x79FC) as u32);
                            g[AT] = li(0x41A0_0000);
                            ctx.fpr[8].set_u32l(g[AT] as u32);
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A2] = s32(ctx.fpr[0].u32l());
                            g[A3] = s32(ctx.fpr[0].u32l());
                            g[A0] = addu(g[S1], 0xA8);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[8].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[10].set_u32l(lw(m, g[S1], 0xA8) as u32);
                            g[T3] = lw(m, g[S1], 0x1E70);
                            sw(m, g[T3], 0x54, u64::from(ctx.fpr[10].u32l()));
                            break 'b_8000D11C;
                        }
                        16 => {
                            if g[V0] == 0 {
                                g[RA] = lw(m, g[SP], 0x24);
                                break 'b_8000D120;
                            }
                            ctx.fpr[0].set_u32l(lw(m, g[S1], 0x108) as u32);
                            g[AT] = li(0x43FA_0000);
                            ctx.fpr[16].set_u32l(g[AT] as u32);
                            ctx.fpr[0].set_fl(ctx.fpr[0].fl().sqrt());
                            g[A1] = s32(ctx.fpr[12].u32l());
                            g[A0] = addu(g[SP], 0x2C);
                            g[A2] = li(0x3F80_0000);
                            g[A3] = li(0x4120_0000);
                            sw(m, g[SP], 0x10, u64::from(ctx.fpr[16].u32l()));
                            sw(m, g[SP], 0x2C, u64::from(ctx.fpr[0].u32l()));
                            call(imports::func_8000C6C8, m, ctx);
                            let g = &mut ctx.gpr;
                            ctx.fpr[18].set_u32l(lw(m, g[SP], 0x2C) as u32);
                            ctx.fpr[4].set_fl(ctx.fpr[18].fl() * ctx.fpr[18].fl());
                            sw(m, g[S1], 0x108, u64::from(ctx.fpr[4].u32l()));
                            break 'b_8000D11C;
                        }
                        _ => {
                            imports::runtime::switch_error(c"func_8000CC1C".as_ptr(), 0x8000_CC84, 0x800A_85AC);
                        }
                    }
                    let g = &mut ctx.gpr;
                    ctx.fpr[0].set_u32l(0);
                    g[AT] = li(0xBF80_0000);
                    if !(ctx.fpr[0].fl() < ctx.fpr[12].fl()) {
                        ctx.fpr[12].set_u32l(g[AT] as u32);
                        g[T8] = lw(m, g[S0], 0);
                    } else {
                        g[AT] = li(0x3F80_0000);
                        ctx.fpr[12].set_u32l(g[AT] as u32);
                        g[T8] = lw(m, g[S0], 0);
                    }
                    g[A0] = li(0x5465_7374);
                    ctx.fpr[4].set_u32l(g[T8] as u32);
                    sw(m, g[SP], 0x3C, u64::from(ctx.fpr[12].u32l()));
                    ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
                    ctx.fpr[8].set_fl(ctx.fpr[6].fl() + ctx.fpr[12].fl());
                    ctx.fpr[10].set_u32l(fpu::trunc_w_s(ctx.fpr[8].fl()));
                    g[A1] = s32(ctx.fpr[10].u32l());
                    sw(m, g[S0], 0, g[A1]);
                    call(imports::func_8003F714, m, ctx);
                    let g = &mut ctx.gpr;
                    ctx.fpr[0].set_u32l(0);
                    ctx.fpr[12].set_u32l(lw(m, g[SP], 0x3C) as u32);
                    if g[V0] != 0 {
                        g[T1] = lh(m, g[V0], 6);
                        g[T2] = g[T1] & 0x100;
                        if g[T2] == 0 {
                            g[RA] = lw(m, g[SP], 0x24);
                            break 'b_8000D120;
                        }
                    }
                    if ctx.fpr[12].fl() < ctx.fpr[0].fl() {
                        // The loop's entry test, repeated by IDO: never taken.
                        if !(ctx.fpr[12].fl() < ctx.fpr[0].fl()) {
                            g[RA] = lw(m, g[SP], 0x24);
                            break 'b_8000D120;
                        }
                        g[T3] = lw(m, g[S0], 0);
                        loop {
                            let g = &mut ctx.gpr;
                            'b_8000CD74: {
                                g[A0] = li(0x5465_7374);
                                g[A1] = addu(g[T3], 1);
                                sw(m, g[S0], 0, g[A1]);
                                sw(m, g[SP], 0x3C, u64::from(ctx.fpr[12].u32l()));
                                call(imports::func_8003F714, m, ctx);
                                let g = &mut ctx.gpr;
                                ctx.fpr[0].set_u32l(0);
                                ctx.fpr[12].set_u32l(lw(m, g[SP], 0x3C) as u32);
                                if g[V0] != 0 {
                                    g[T5] = lh(m, g[V0], 6);
                                    g[T6] = g[T5] & 0x100;
                                    if g[T6] == 0 {
                                        break 'b_8000CD74;
                                    }
                                }
                                g[T7] = lw(m, g[S0], 0);
                                ctx.fpr[12].set_u32l(ctx.fpr[0].u32l());
                                g[T8] = addu(g[T7], u64::MAX);
                                sw(m, g[S0], 0, g[T8]);
                            }
                            let g = &mut ctx.gpr;
                            // Both ways test d < 0 (d = 0.0 after a step back).
                            if !(ctx.fpr[12].fl() < ctx.fpr[0].fl()) {
                                break;
                            }
                            g[T3] = lw(m, g[S0], 0);
                        }
                        let g = &mut ctx.gpr;
                        g[RA] = lw(m, g[SP], 0x24);
                        break 'b_8000D120;
                    }
                    let g = &mut ctx.gpr;
                    sw(m, g[S0], 0, 0);
                }
            }
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x24);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x1C);
    g[S1] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x38);
}

/// `func_8000D5EC(k, x)` (edit setting `k`, **guess**: the same debug
/// menu): with the float `x` in `a1` and the menu flags `F = [0x8009B7D8]`,
/// by `k` (unsigned; nothing for `k >= 9` or 8):
/// - 0: [`func_8000C724`]`(0x8009B7D0, x, 1.0, 0, 6)`, an integer clamped
///   to 0..6;
/// - 1: if `F & 4`, `[0x800A52D4] = ([0x800A52D4] == 0)`; 6: the same with
///   `F & 0x10` and `[0x800A52D0]`;
/// - 2..5, if `F & 8`: [`func_8000C6C8`]`(p, x, y, lo, hi)` with `(p, y,
///   lo, hi)` = `(0x800A5B64, 0.001, 0.2, 2.0)`, `(0x800A5B68, 0.5, 2.0,
///   200.0)`, `(0x800A5B54, 1.0, 20.0, 1000.0)`, `(0x800A5B58, 1.0, 20.0,
///   500.0)`;
/// - 7: if `F & 0x20`, bit 14 of `[0x800D697C]` toggled.
///
/// Frame (`sp - 0x20`): `ra` at `+0x1C`, `hi` at `+0x10` for the callees.
/// Leaves `f12 = x`, `at`, `t*`, `v0`/`v1` from the case and the callees'
/// registers.
///
/// Domain: no NaN operand of the callees' arithmetic.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000D5EC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8000D7D0: {
        'b_8000D7CC: {
            g[SP] = addu(g[SP], (-0x20i64) as u64);
            ctx.fpr[12].set_u32l(g[A1] as u32);
            g[AT] = sltu(g[A0], 9);
            sw(m, g[SP], 0x1C, g[RA]);
            if g[AT] != 0 {
                g[T6] = sll(g[A0], 2);
                g[AT] = li(0x800B_0000);
                let jr_addend_8000D610 = g[T6];
                g[AT] = addu(g[AT], g[T6]);
                g[T6] = addu(g[AT], (-0x79C4i64) as u64);
                match jr_addend_8000D610 >> 2 {
                    0 => {}
                    1 => {
                        g[T8] = li(0x800A_0000);
                        g[T8] = lw(m, g[T8], -0x4828);
                        g[V1] = li(0x800A_52D4);
                        g[T9] = g[T8] & 4;
                        if g[T9] == 0 {
                            g[RA] = lw(m, g[SP], 0x1C);
                            break 'b_8000D7D0;
                        }
                        g[V0] = lw(m, g[V1], 0);
                        g[T0] = sltu(g[V0], 1);
                        sw(m, g[V1], 0, g[T0]);
                        break 'b_8000D7CC;
                    }
                    2 => {
                        g[T1] = li(0x800A_0000);
                        g[T1] = lw(m, g[T1], -0x4828);
                        g[T2] = g[T1] & 8;
                        g[AT] = li(0x4000_0000);
                        if g[T2] == 0 {
                            break 'b_8000D7CC;
                        }
                        ctx.fpr[4].set_u32l(g[AT] as u32);
                        g[A0] = li(0x800A_0000);
                        g[A1] = s32(ctx.fpr[12].u32l());
                        g[A2] = li(0x3A83_0000);
                        g[A3] = li(0x3E4C_CCCD);
                        g[A2] = g[A2] | 0x126F;
                        g[A0] = addu(g[A0], 0x5B64);
                        sw(m, g[SP], 0x10, u64::from(ctx.fpr[4].u32l()));
                        call(imports::func_8000C6C8, m, ctx);
                        let g = &mut ctx.gpr;
                        g[RA] = lw(m, g[SP], 0x1C);
                        break 'b_8000D7D0;
                    }
                    3 => {
                        g[T3] = li(0x800A_0000);
                        g[T3] = lw(m, g[T3], -0x4828);
                        g[A0] = li(0x800A_5B68);
                        g[T4] = g[T3] & 8;
                        g[A2] = li(0x3F00_0000);
                        if g[T4] == 0 {
                            break 'b_8000D7CC;
                        }
                        g[AT] = li(0x4348_0000);
                        ctx.fpr[6].set_u32l(g[AT] as u32);
                        g[A1] = s32(ctx.fpr[12].u32l());
                        g[A3] = li(0x4000_0000);
                        sw(m, g[SP], 0x10, u64::from(ctx.fpr[6].u32l()));
                        call(imports::func_8000C6C8, m, ctx);
                        let g = &mut ctx.gpr;
                        g[RA] = lw(m, g[SP], 0x1C);
                        break 'b_8000D7D0;
                    }
                    4 => {
                        g[T5] = li(0x800A_0000);
                        g[T5] = lw(m, g[T5], -0x4828);
                        g[A0] = li(0x800A_5B54);
                        g[T6] = g[T5] & 8;
                        g[A2] = li(0x3F80_0000);
                        if g[T6] == 0 {
                            break 'b_8000D7CC;
                        }
                        g[AT] = li(0x447A_0000);
                        ctx.fpr[8].set_u32l(g[AT] as u32);
                        g[A1] = s32(ctx.fpr[12].u32l());
                        g[A3] = li(0x41A0_0000);
                        sw(m, g[SP], 0x10, u64::from(ctx.fpr[8].u32l()));
                        call(imports::func_8000C6C8, m, ctx);
                        let g = &mut ctx.gpr;
                        g[RA] = lw(m, g[SP], 0x1C);
                        break 'b_8000D7D0;
                    }
                    5 => {
                        g[T7] = li(0x800A_0000);
                        g[T7] = lw(m, g[T7], -0x4828);
                        g[A0] = li(0x800A_5B58);
                        g[T8] = g[T7] & 8;
                        g[A2] = li(0x3F80_0000);
                        if g[T8] == 0 {
                            break 'b_8000D7CC;
                        }
                        g[AT] = li(0x43FA_0000);
                        ctx.fpr[10].set_u32l(g[AT] as u32);
                        g[A1] = s32(ctx.fpr[12].u32l());
                        g[A3] = li(0x41A0_0000);
                        sw(m, g[SP], 0x10, u64::from(ctx.fpr[10].u32l()));
                        call(imports::func_8000C6C8, m, ctx);
                        let g = &mut ctx.gpr;
                        g[RA] = lw(m, g[SP], 0x1C);
                        break 'b_8000D7D0;
                    }
                    6 => {
                        g[T9] = li(0x800A_0000);
                        g[T9] = lw(m, g[T9], -0x4828);
                        g[V1] = li(0x800A_52D0);
                        g[T0] = g[T9] & 0x10;
                        if g[T0] == 0 {
                            g[RA] = lw(m, g[SP], 0x1C);
                            break 'b_8000D7D0;
                        }
                        g[V0] = lw(m, g[V1], 0);
                        g[T1] = sltu(g[V0], 1);
                        sw(m, g[V1], 0, g[T1]);
                        break 'b_8000D7CC;
                    }
                    7 => {
                        g[T2] = li(0x800A_0000);
                        g[T2] = lw(m, g[T2], -0x4828);
                        g[T3] = g[T2] & 0x20;
                        g[V1] = li(0x800D_0000);
                        if g[T3] == 0 {
                            break 'b_8000D7CC;
                        }
                        g[V1] = addu(g[V1], 0x6960);
                        g[V0] = lw(m, g[V1], 0x1C);
                        g[AT] = (-0x4001i64) as u64;
                        g[T4] = g[V0] & 0x4000;
                        g[T6] = g[V0] | 0x4000;
                        if g[T4] == 0 {
                            sw(m, g[V1], 0x1C, g[T6]);
                            break 'b_8000D7CC;
                        }
                        g[T5] = g[V0] & g[AT];
                        sw(m, g[V1], 0x1C, g[T5]);
                        break 'b_8000D7CC;
                    }
                    8 => {
                        break 'b_8000D7CC;
                    }
                    _ => {
                        imports::runtime::switch_error(c"func_8000D5EC".as_ptr(), 0x8000_D610, 0x800A_863C);
                    }
                }
                let g = &mut ctx.gpr;
                g[A0] = li(0x800A_0000);
                g[A1] = s32(ctx.fpr[12].u32l());
                g[T7] = 6;
                sw(m, g[SP], 0x10, g[T7]);
                g[A0] = addu(g[A0], (-0x4830i64) as u64);
                g[A2] = li(0x3F80_0000);
                g[A3] = 0;
                call(imports::func_8000C724, m, ctx);
                let g = &mut ctx.gpr;
                g[RA] = lw(m, g[SP], 0x1C);
                break 'b_8000D7D0;
            }
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x1C);
    }
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], 0x20);
}

/// `func_8000D7DC(k)`: if `k == 8` (64-bit) and bit 1 of `[0x8009B7D8]` is
/// set, [`func_8000C5F0`]`(1)` (push id 1).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `at = 8`, `t6`/`t7` and
/// the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000D7DC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8000D80C: {
        g[SP] = addu(g[SP], (-0x18i64) as u64);
        g[AT] = 8;
        sw(m, g[SP], 0x14, g[RA]);
        if g[A0] == g[AT] {
            g[T6] = li(0x800A_0000);
            g[T6] = lw(m, g[T6], -0x4828);
            g[T7] = g[T6] & 2;
            if g[T7] == 0 {
                g[RA] = lw(m, g[SP], 0x14);
                break 'b_8000D80C;
            }
            g[A0] = 1;
            call(imports::func_8000C5F0, m, ctx);
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x14);
    }
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], 0x18);
}

/// `func_8000D9A8()` (reset the id stack, **guess** at the use): if
/// `SAVED_WORDS[0]` is -2, [`CURRENT_ID`] = -1 (then a loop that only counts
/// to 3); [`CURRENT_WORD`] = `SAVED_WORDS[0]` (re-read), `[0x8009B7E4] = 0`,
/// [`ID_DEPTH`] = 0; with `e` = element 0 of the first "Jdge" pool
/// ([`func_8003F800`](crate::pools::func_8003F800)): if bit 12 of its
/// halfword `+6` is set, `CURRENT_ID = 0`, else `CURRENT_ID = 2` and
/// `CURRENT_WORD = 0`; finally `ID_STACK[ID_DEPTH] = CURRENT_ID`.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `v0` = [`CURRENT_ID`]'s
/// address, `t0` its word, `t1`, `t2`, `at` the slot's address, `t7`, `t8`
/// the flags and the callee's registers.
///
/// Domain: a "Jdge" pool with at least one element (QUIRK: `e` isn't tested
/// for 0).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000D9A8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x800A_0000);
    g[V0] = lw(m, g[V0], -0x480C);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[AT] = (-2i64) as u64;
    sw(m, g[SP], 0x14, g[RA]);
    if g[V0] == g[AT] {
        g[T6] = u64::MAX;
        g[AT] = li(0x800A_0000);
        sw(m, g[AT], -0x4800, g[T6]);
        g[V0] = 0;
        g[V1] = 3;
        g[V0] = addu(g[V0], 1);
        loop {
            if g[V0] == g[V1] {
                break;
            }
            g[V0] = addu(g[V0], 1);
        }
        g[V0] = li(0x800A_0000);
        g[V0] = lw(m, g[V0], -0x480C);
    }
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], -0x4824, g[V0]);
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], -0x481C, 0);
    g[AT] = li(0x800D_0000);
    g[A0] = li(0x4A64_0000);
    sw(m, g[AT], 0x578C, 0);
    g[A0] = g[A0] | 0x6765;
    g[A1] = 0;
    call(imports::func_8003F800, m, ctx);
    let g = &mut ctx.gpr;
    g[T7] = lh(m, g[V0], 6);
    g[V0] = li(0x8009_B800);
    g[T8] = g[T7] & 0x1000;
    g[T1] = li(0x800D_0000);
    if g[T8] == 0 {
        g[T9] = 2;
        sw(m, g[V0], 0, g[T9]);
        g[AT] = li(0x800A_0000);
        sw(m, g[AT], -0x4824, 0);
    } else {
        g[V0] = li(0x8009_B800);
        sw(m, g[V0], 0, 0);
    }
    g[T1] = lw(m, g[T1], 0x578C);
    g[RA] = lw(m, g[SP], 0x14);
    g[T0] = lw(m, g[V0], 0);
    g[AT] = li(0x800D_0000);
    g[T2] = sll(g[T1], 2);
    g[AT] = addu(g[AT], g[T2]);
    g[SP] = addu(g[SP], 0x18);
    sw(m, g[AT], 0x5718, g[T0]);
}

/// `func_8000DA6C()`: returns `[0x8009B7E4]`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000DA6C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    ctx.gpr[V0] = lw(&mem, li(0x800A_0000), -0x481C);
}

/// `func_8000E680(id, x, y)` (a record's position from screen pixels,
/// **guess**): [`func_8000AA04`]`(id, trunc(f32(x) / f32(W) * 320.0),
/// trunc(f32(y) / f32(H) * 240.0))` with `W`, `H` the s16 screen size at
/// `0x80114470`/`72`. Every argument is taken as s16 (`y` read back from
/// its home slot), and so are the truncations (the C cast, low halfword).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`; `id`, `x`, `y` spilled to their
/// home slots `sp + 0..8`. Leaves `f0`, `f2` the scaled values, `f4`, `f6`
/// their truncations, `t*`/`at` from the conversions and the callee's
/// registers.
///
/// Domain: no NaN operand: not `0 / 0` (a zero coordinate on a zero screen
/// size).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000E680(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x8011_4470);
    g[T9] = lh(m, g[V0], 0);
    g[A3] = sll(g[A1], 16);
    g[T8] = sra(g[A3], 16);
    ctx.fpr[4].set_u32l(g[T8] as u32);
    ctx.fpr[8].set_u32l(g[T9] as u32);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
    sw(m, g[SP], 0x20, g[A2]);
    g[T0] = lh(m, g[SP], 0x22);
    g[T1] = lh(m, g[V0], 2);
    g[AT] = li(0x43A0_0000);
    ctx.fpr[10].set_fl(fpu::cvt_s_w(ctx.fpr[8].u32l(), fpu::NEAREST));
    ctx.fpr[4].set_u32l(g[T0] as u32);
    ctx.fpr[18].set_u32l(g[AT] as u32);
    g[AT] = li(0x4370_0000);
    sw(m, g[SP], 0x1C, g[A1]);
    ctx.fpr[8].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
    sw(m, g[SP], 0x18, g[A0]);
    g[T6] = sll(g[A0], 16);
    g[A0] = sra(g[T6], 16);
    sw(m, g[SP], 0x14, g[RA]);
    ctx.fpr[16].set_fl(ctx.fpr[6].fl() / ctx.fpr[10].fl());
    ctx.fpr[6].set_u32l(g[T1] as u32);
    ctx.fpr[10].set_fl(fpu::cvt_s_w(ctx.fpr[6].u32l(), fpu::NEAREST));
    ctx.fpr[0].set_fl(ctx.fpr[16].fl() * ctx.fpr[18].fl());
    ctx.fpr[16].set_fl(ctx.fpr[8].fl() / ctx.fpr[10].fl());
    ctx.fpr[18].set_u32l(g[AT] as u32);
    ctx.fpr[4].set_u32l(fpu::trunc_w_s(ctx.fpr[0].fl()));
    g[A1] = s32(ctx.fpr[4].u32l());
    g[T3] = sll(g[A1], 16);
    g[A1] = sra(g[T3], 16);
    ctx.fpr[2].set_fl(ctx.fpr[16].fl() * ctx.fpr[18].fl());
    ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[2].fl()));
    g[A2] = s32(ctx.fpr[6].u32l());
    g[T6] = sll(g[A2], 16);
    g[A2] = sra(g[T6], 16);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_8000E738(id, x, y)`: as [`func_8000E680`] with
/// [`func_8000AA78`] (the record's `+4`/`+6` halfwords).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000E738(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x8011_4470);
    g[T9] = lh(m, g[V0], 0);
    g[A3] = sll(g[A1], 16);
    g[T8] = sra(g[A3], 16);
    ctx.fpr[4].set_u32l(g[T8] as u32);
    ctx.fpr[8].set_u32l(g[T9] as u32);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
    sw(m, g[SP], 0x20, g[A2]);
    g[T0] = lh(m, g[SP], 0x22);
    g[T1] = lh(m, g[V0], 2);
    g[AT] = li(0x43A0_0000);
    ctx.fpr[10].set_fl(fpu::cvt_s_w(ctx.fpr[8].u32l(), fpu::NEAREST));
    ctx.fpr[4].set_u32l(g[T0] as u32);
    ctx.fpr[18].set_u32l(g[AT] as u32);
    g[AT] = li(0x4370_0000);
    sw(m, g[SP], 0x1C, g[A1]);
    ctx.fpr[8].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
    sw(m, g[SP], 0x18, g[A0]);
    g[T6] = sll(g[A0], 16);
    g[A0] = sra(g[T6], 16);
    sw(m, g[SP], 0x14, g[RA]);
    ctx.fpr[16].set_fl(ctx.fpr[6].fl() / ctx.fpr[10].fl());
    ctx.fpr[6].set_u32l(g[T1] as u32);
    ctx.fpr[10].set_fl(fpu::cvt_s_w(ctx.fpr[6].u32l(), fpu::NEAREST));
    ctx.fpr[0].set_fl(ctx.fpr[16].fl() * ctx.fpr[18].fl());
    ctx.fpr[16].set_fl(ctx.fpr[8].fl() / ctx.fpr[10].fl());
    ctx.fpr[18].set_u32l(g[AT] as u32);
    ctx.fpr[4].set_u32l(fpu::trunc_w_s(ctx.fpr[0].fl()));
    g[A1] = s32(ctx.fpr[4].u32l());
    g[T3] = sll(g[A1], 16);
    g[A1] = sra(g[T3], 16);
    ctx.fpr[2].set_fl(ctx.fpr[16].fl() * ctx.fpr[18].fl());
    ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[2].fl()));
    g[A2] = s32(ctx.fpr[6].u32l());
    g[T6] = sll(g[A2], 16);
    g[A2] = sra(g[T6], 16);
    call(imports::func_8000AA78, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_8000E8C4(n)` (a node's first nonzero child word, **guess**): 0 for
/// a null `n`. For a node of type `[n] == 0x3064` ([`func_80017DA4`]): the
/// first nonzero word `[c]` of its children `c = [[n + 0x18] + 4i]`, for `i
/// < [n + 0x14]` (signed, re-read each step), else 0. For other types with
/// bit 14 set: the first nonzero `func_8000E8C4(c)` over the children (the
/// count [`func_80017DAC`] re-read each step), else 0; other types give 0.
/// The recursive calls go through the C symbol like any callee.
///
/// Frame (`sp - 0x28`): `ra`, `s2`, `s1`, `s0` at `+0x24..+0x18`, restored
/// (`s2 = n`, `s0` the index, `s1 = 4i`). Leaves `v1`, `a0`, `a1`, `t6`..`t9`
/// from the scan and the callees' registers.
///
/// Domain: the node tree in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000E8C4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8000E9A8: {
        'b_8000E9A4: {
            'b_8000E9A0: {
                g[SP] = addu(g[SP], (-0x28i64) as u64);
                sw(m, g[SP], 0x20, g[S2]);
                g[S2] = g[A0];
                sw(m, g[SP], 0x24, g[RA]);
                sw(m, g[SP], 0x1C, g[S1]);
                sw(m, g[SP], 0x18, g[S0]);
                if g[A0] != 0 {
                    call(imports::func_80017DA4, m, ctx);
                    let g = &mut ctx.gpr;
                    g[AT] = 0x3064;
                    if g[V0] == g[AT] {
                        g[T6] = lw(m, g[S2], 0x14);
                        g[A1] = g[S2];
                        g[S0] = 0;
                        if (g[T6] as i64) <= 0 {
                            g[V0] = 0;
                            break 'b_8000E9A4;
                        }
                        g[A0] = lw(m, g[S2], 0x18);
                        g[V0] = lw(m, g[A0], 0);
                        loop {
                            g[V1] = lw(m, g[V0], 0);
                            if g[V1] != 0 {
                                break;
                            }
                            g[T7] = lw(m, g[A1], 0x14);
                            g[S0] = addu(g[S0], 1);
                            g[A0] = addu(g[A0], 4);
                            g[AT] = slt(g[S0], g[T7]);
                            if g[AT] == 0 {
                                g[V0] = 0;
                                break 'b_8000E9A4;
                            }
                            g[V0] = lw(m, g[A0], 0);
                        }
                        g[V0] = g[V1];
                        break 'b_8000E9A4;
                    }
                    g[A0] = g[S2];
                    call(imports::func_80017DA4, m, ctx);
                    let g = &mut ctx.gpr;
                    g[T8] = g[V0] & 0x4000;
                    g[S0] = 0;
                    if g[T8] != 0 {
                        g[A0] = g[S2];
                        call(imports::func_80017DAC, m, ctx);
                        let g = &mut ctx.gpr;
                        g[S1] = sll(g[S0], 2);
                        if (g[V0] as i64) > 0 {
                            g[T9] = lw(m, g[S2], 0x18);
                            loop {
                                let g = &mut ctx.gpr;
                                g[T0] = addu(g[T9], g[S1]);
                                g[A0] = lw(m, g[T0], 0);
                                call(imports::func_8000E8C4, m, ctx);
                                let g = &mut ctx.gpr;
                                g[S0] = addu(g[S0], 1);
                                if g[V0] != 0 {
                                    break;
                                }
                                g[S1] = addu(g[S1], 4);
                                g[A0] = g[S2];
                                call(imports::func_80017DAC, m, ctx);
                                let g = &mut ctx.gpr;
                                g[AT] = slt(g[S0], g[V0]);
                                if g[AT] == 0 {
                                    break 'b_8000E9A0;
                                }
                                g[T9] = lw(m, g[S2], 0x18);
                            }
                            let g = &mut ctx.gpr;
                            g[RA] = lw(m, g[SP], 0x24);
                            break 'b_8000E9A8;
                        }
                    }
                }
            }
            let g = &mut ctx.gpr;
            g[V0] = 0;
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x24);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x28);
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

/// `func_8000EA4C(n, b0, b1, b2, b3, b4, b5)` (set six bytes through a
/// node tree, **guess**): the values are the low halfwords of `a1`..`a3`
/// and the fifth to seventh arguments, signed. For a non-null `n` of type
/// `0x3064` ([`func_80017DA4`]): [`func_8000E9BC`]`([c], b0, .., b5)` for
/// each child `c = [[n + 0x18] + 4i]`, `i < [n + 0x14]` (re-read each
/// step); for other types with bit 14 set, `func_8000EA4C(c, b0, .., b5)`
/// for each child (the count [`func_80017DAC`] re-read each step);
/// otherwise nothing.
///
/// Frame (`sp - 0x50`): `ra`, `fp`, `s7`..`s0` at `+0x4C..+0x28`, restored;
/// `b0`..`b2` spilled to their home slots `sp + 4..0xC`; `b3`..`b5` stored
/// as outgoing stack arguments at `+0x10..+0x18` for each call. Leaves the
/// callees' registers.
///
/// Domain: the node tree and the byte targets in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000EA4C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8000EBBC: {
        g[SP] = addu(g[SP], (-0x50i64) as u64);
        sw(m, g[SP], 0x48, g[FP]);
        sw(m, g[SP], 0x44, g[S7]);
        g[S7] = sll(g[A1], 16);
        g[FP] = sll(g[A2], 16);
        g[T7] = sra(g[FP], 16);
        g[T6] = sra(g[S7], 16);
        sw(m, g[SP], 0x3C, g[S5]);
        g[S5] = g[A0];
        g[S7] = g[T6];
        g[FP] = g[T7];
        sw(m, g[SP], 0x4C, g[RA]);
        sw(m, g[SP], 0x40, g[S6]);
        sw(m, g[SP], 0x38, g[S4]);
        sw(m, g[SP], 0x34, g[S3]);
        sw(m, g[SP], 0x30, g[S2]);
        sw(m, g[SP], 0x2C, g[S1]);
        sw(m, g[SP], 0x28, g[S0]);
        sw(m, g[SP], 0x54, g[A1]);
        sw(m, g[SP], 0x58, g[A2]);
        sw(m, g[SP], 0x5C, g[A3]);
        if g[A0] != 0 {
            call(imports::func_80017DA4, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = 0x3064;
            if g[V0] != g[AT] {
                g[A0] = g[S5];
                call(imports::func_80017DA4, m, ctx);
                let g = &mut ctx.gpr;
                g[T4] = g[V0] & 0x4000;
                g[S1] = 0;
                if g[T4] != 0 {
                    g[A0] = g[S5];
                    call(imports::func_80017DAC, m, ctx);
                    let g = &mut ctx.gpr;
                    g[S0] = sll(g[S1], 2);
                    if (g[V0] as i64) > 0 {
                        g[S4] = lh(m, g[SP], 0x6A);
                        g[S3] = lh(m, g[SP], 0x66);
                        g[S2] = lh(m, g[SP], 0x62);
                        g[T5] = lw(m, g[S5], 0x18);
                        loop {
                            let g = &mut ctx.gpr;
                            g[A1] = sll(g[S7], 16);
                            g[A2] = sll(g[FP], 16);
                            g[T6] = addu(g[T5], g[S0]);
                            g[A0] = lw(m, g[T6], 0);
                            g[T8] = sra(g[A2], 16);
                            g[T7] = sra(g[A1], 16);
                            g[A1] = g[T7];
                            g[A2] = g[T8];
                            sw(m, g[SP], 0x18, g[S4]);
                            sw(m, g[SP], 0x14, g[S3]);
                            sw(m, g[SP], 0x10, g[S2]);
                            g[A3] = lh(m, g[SP], 0x5E);
                            call(imports::func_8000EA4C, m, ctx);
                            let g = &mut ctx.gpr;
                            g[S1] = addu(g[S1], 1);
                            g[S0] = addu(g[S0], 4);
                            g[A0] = g[S5];
                            call(imports::func_80017DAC, m, ctx);
                            let g = &mut ctx.gpr;
                            g[AT] = slt(g[S1], g[V0]);
                            if g[AT] == 0 {
                                break;
                            }
                            g[T5] = lw(m, g[S5], 0x18);
                        }
                    }
                }
            } else {
                g[T8] = lw(m, g[S5], 0x14);
                g[S6] = g[S5];
                g[S1] = 0;
                g[S0] = 0;
                if (g[T8] as i64) > 0 {
                    g[S4] = lh(m, g[SP], 0x6A);
                    g[S3] = lh(m, g[SP], 0x66);
                    g[S2] = lh(m, g[SP], 0x62);
                    g[T9] = lw(m, g[S5], 0x18);
                    loop {
                        let g = &mut ctx.gpr;
                        g[A1] = sll(g[S7], 16);
                        g[A2] = sll(g[FP], 16);
                        g[T0] = addu(g[T9], g[S0]);
                        g[V0] = lw(m, g[T0], 0);
                        g[T2] = sra(g[A2], 16);
                        g[T1] = sra(g[A1], 16);
                        g[A0] = lw(m, g[V0], 0);
                        sw(m, g[SP], 0x18, g[S4]);
                        sw(m, g[SP], 0x14, g[S3]);
                        sw(m, g[SP], 0x10, g[S2]);
                        g[A1] = g[T1];
                        g[A2] = g[T2];
                        g[A3] = lh(m, g[SP], 0x5E);
                        call(imports::func_8000E9BC, m, ctx);
                        let g = &mut ctx.gpr;
                        g[T3] = lw(m, g[S6], 0x14);
                        g[S1] = addu(g[S1], 1);
                        g[S0] = addu(g[S0], 4);
                        g[AT] = slt(g[S1], g[T3]);
                        if g[AT] == 0 {
                            break;
                        }
                        g[T9] = lw(m, g[S5], 0x18);
                    }
                    let g = &mut ctx.gpr;
                    g[RA] = lw(m, g[SP], 0x4C);
                    break 'b_8000EBBC;
                }
            }
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x4C);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x28);
    g[S1] = lw(m, g[SP], 0x2C);
    g[S2] = lw(m, g[SP], 0x30);
    g[S3] = lw(m, g[SP], 0x34);
    g[S4] = lw(m, g[SP], 0x38);
    g[S5] = lw(m, g[SP], 0x3C);
    g[S6] = lw(m, g[SP], 0x40);
    g[S7] = lw(m, g[SP], 0x44);
    g[FP] = lw(m, g[SP], 0x48);
    g[SP] = addu(g[SP], 0x50);
}

/// `func_8000EBE8(vp, p, sx, sy, zw, w, local)` (project a point to the
/// screen, **guess**): `zw`, `w` and `local` are stack arguments. With the
/// matrix `M` at `0x80112E20` (copied to the frame), the viewport's
/// halfwords `vp + 0x10`, `+0x12` halved and `+0x18`, `+0x1A` quartered
/// (s16, rounded toward zero) as `hw`, `hh`, `cx`, `cy`, `ox = f32(cx) -
/// f32(hw) * 0.5` and `oy = f32(cy) - f32(hh) * 0.5`: `*sx = *sy = -1000.0`;
/// `(x, y, z, w')` = `p * M` ([`func_80081730`](crate::math::func_80081730),
/// `w = 1`), or `(p - C) * M` with `C` the float triple at `0x800A3FDC`
/// unless `local` is nonzero; `x = -x` if bit 14 of `[0x800D697C]` is set
/// (mirror). Then if `K < f64(w')` (`K` the double at `0x800A8680`): `X =
/// (x / w' + 1.0) * (f32(hw) * 0.5) + ox`, `Y = (1.0 - y / w') * (f32(hh) *
/// 0.5) + oy`, `*zw = z / w'`, `*w = w'`, and `*sx = X`, `*sy = Y` if `ox -
/// 8 < X < (f32(hw) + ox) + 8` and `oy - 8 < Y < (f32(hh) + oy) + 8`.
///
/// Frame (`sp - 0xA8`): `ra` at `+0x14`; `f32(hh)`, `f32(hw)` at `+0x20`,
/// `+0x24`, `oy`, `ox` at `+0x28`, `+0x2C`, their halves' products at
/// `+0x30`, `+0x34`, `p - C` at `+0x48`, the result at `+0x54`, `M` at
/// `+0x68`; `p`, `sx`, `sy` spilled to their home slots `sp + 4..0xC`.
/// Leaves `f0`..`f18` from the path and the callee's registers.
///
/// Domain: no NaN operand of the arithmetic (the callee's included).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000EBE8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0xA8i64) as u64);
    g[V0] = li(0x8011_2E20);
    ctx.fpr[4].set_u32l(lw(m, g[V0], 0) as u32);
    ctx.fpr[6].set_u32l(lw(m, g[V0], 4) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[V0], 8) as u32);
    ctx.fpr[10].set_u32l(lw(m, g[V0], 0xC) as u32);
    sw(m, g[SP], 0x68, u64::from(ctx.fpr[4].u32l()));
    sw(m, g[SP], 0x6C, u64::from(ctx.fpr[6].u32l()));
    sw(m, g[SP], 0x70, u64::from(ctx.fpr[8].u32l()));
    sw(m, g[SP], 0x74, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[10].set_u32l(lw(m, g[V0], 0x1C) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[V0], 0x18) as u32);
    ctx.fpr[6].set_u32l(lw(m, g[V0], 0x14) as u32);
    ctx.fpr[4].set_u32l(lw(m, g[V0], 0x10) as u32);
    sw(m, g[SP], 0x84, u64::from(ctx.fpr[10].u32l()));
    sw(m, g[SP], 0x80, u64::from(ctx.fpr[8].u32l()));
    sw(m, g[SP], 0x7C, u64::from(ctx.fpr[6].u32l()));
    sw(m, g[SP], 0x78, u64::from(ctx.fpr[4].u32l()));
    ctx.fpr[4].set_u32l(lw(m, g[V0], 0x20) as u32);
    ctx.fpr[6].set_u32l(lw(m, g[V0], 0x24) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[V0], 0x28) as u32);
    ctx.fpr[10].set_u32l(lw(m, g[V0], 0x2C) as u32);
    sw(m, g[SP], 0x88, u64::from(ctx.fpr[4].u32l()));
    sw(m, g[SP], 0x8C, u64::from(ctx.fpr[6].u32l()));
    sw(m, g[SP], 0x90, u64::from(ctx.fpr[8].u32l()));
    sw(m, g[SP], 0x94, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[10].set_u32l(lw(m, g[V0], 0x3C) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[V0], 0x38) as u32);
    ctx.fpr[6].set_u32l(lw(m, g[V0], 0x34) as u32);
    ctx.fpr[4].set_u32l(lw(m, g[V0], 0x30) as u32);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0xAC, g[A1]);
    sw(m, g[SP], 0xB0, g[A2]);
    sw(m, g[SP], 0xB4, g[A3]);
    sw(m, g[SP], 0xA4, u64::from(ctx.fpr[10].u32l()));
    sw(m, g[SP], 0xA0, u64::from(ctx.fpr[8].u32l()));
    sw(m, g[SP], 0x9C, u64::from(ctx.fpr[6].u32l()));
    sw(m, g[SP], 0x98, u64::from(ctx.fpr[4].u32l()));
    g[V1] = lh(m, g[A0], 0x10);
    g[AT] = li(0x3F00_0000);
    ctx.fpr[12].set_u32l(g[AT] as u32);
    g[AT] = li(0xC47A_0000);
    ctx.fpr[18].set_u32l(g[AT] as u32);
    g[T0] = lh(m, g[A0], 0x12);
    g[T6] = sra(g[V1], 1);
    if (g[V1] as i64) < 0 {
        g[AT] = addu(g[V1], 1);
        g[T6] = sra(g[AT], 1);
    }
    g[V1] = g[T6];
    ctx.fpr[4].set_u32l(g[V1] as u32);
    g[T7] = sra(g[T0], 1);
    if (g[T0] as i64) < 0 {
        g[AT] = addu(g[T0], 1);
        g[T7] = sra(g[AT], 1);
    }
    g[T0] = g[T7];
    ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
    ctx.fpr[10].set_u32l(g[T0] as u32);
    g[T3] = lw(m, g[SP], 0xB0);
    g[V0] = li(0x800A_0000);
    g[A2] = addu(g[SP], 0x68);
    ctx.fpr[4].set_fl(fpu::cvt_s_w(ctx.fpr[10].u32l(), fpu::NEAREST));
    sw(m, g[SP], 0x24, u64::from(ctx.fpr[6].u32l()));
    ctx.fpr[8].set_u32l(lw(m, g[SP], 0x24) as u32);
    ctx.fpr[0].set_fl(ctx.fpr[8].fl() * ctx.fpr[12].fl());
    sw(m, g[SP], 0x20, u64::from(ctx.fpr[4].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[SP], 0x20) as u32);
    ctx.fpr[2].set_fl(ctx.fpr[6].fl() * ctx.fpr[12].fl());
    sw(m, g[SP], 0x34, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[SP], 0x30, u64::from(ctx.fpr[2].u32l()));
    g[T8] = lh(m, g[A0], 0x18);
    g[T1] = lh(m, g[A0], 0x1A);
    sw(m, g[T3], 0, u64::from(ctx.fpr[18].u32l()));
    g[T9] = sra(g[T8], 2);
    if (g[T8] as i64) < 0 {
        g[AT] = addu(g[T8], 3);
        g[T9] = sra(g[AT], 2);
    }
    ctx.fpr[8].set_u32l(g[T9] as u32);
    g[T4] = lw(m, g[SP], 0xB4);
    g[A0] = addu(g[SP], 0x54);
    ctx.fpr[10].set_fl(fpu::cvt_s_w(ctx.fpr[8].u32l(), fpu::NEAREST));
    sw(m, g[T4], 0, u64::from(ctx.fpr[18].u32l()));
    g[T5] = lw(m, g[SP], 0xC0);
    g[T6] = lw(m, g[SP], 0xAC);
    g[A1] = lw(m, g[SP], 0xAC);
    ctx.fpr[14].set_fl(ctx.fpr[10].fl() - ctx.fpr[0].fl());
    g[T2] = sra(g[T1], 2);
    if (g[T1] as i64) < 0 {
        g[AT] = addu(g[T1], 3);
        g[T2] = sra(g[AT], 2);
    }
    ctx.fpr[4].set_u32l(g[T2] as u32);
    ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
    ctx.fpr[16].set_fl(ctx.fpr[6].fl() - ctx.fpr[2].fl());
    if g[T5] != 0 {
        sw(m, g[SP], 0x2C, u64::from(ctx.fpr[14].u32l()));
        sw(m, g[SP], 0x28, u64::from(ctx.fpr[16].u32l()));
        call(imports::func_80081730, m, ctx);
        let g = &mut ctx.gpr;
        ctx.fpr[14].set_u32l(lw(m, g[SP], 0x2C) as u32);
        ctx.fpr[16].set_u32l(lw(m, g[SP], 0x28) as u32);
    } else {
        g[V0] = addu(g[V0], 0x3FDC);
        ctx.fpr[10].set_u32l(lw(m, g[V0], 0) as u32);
        ctx.fpr[8].set_u32l(lw(m, g[T6], 0) as u32);
        g[A0] = addu(g[SP], 0x54);
        g[A1] = addu(g[SP], 0x48);
        ctx.fpr[4].set_fl(ctx.fpr[8].fl() - ctx.fpr[10].fl());
        ctx.fpr[8].set_u32l(lw(m, g[V0], 4) as u32);
        g[A2] = addu(g[SP], 0x68);
        sw(m, g[SP], 0x48, u64::from(ctx.fpr[4].u32l()));
        ctx.fpr[6].set_u32l(lw(m, g[T6], 4) as u32);
        ctx.fpr[10].set_fl(ctx.fpr[6].fl() - ctx.fpr[8].fl());
        ctx.fpr[6].set_u32l(lw(m, g[V0], 8) as u32);
        sw(m, g[SP], 0x4C, u64::from(ctx.fpr[10].u32l()));
        ctx.fpr[4].set_u32l(lw(m, g[T6], 8) as u32);
        sw(m, g[SP], 0x28, u64::from(ctx.fpr[16].u32l()));
        sw(m, g[SP], 0x2C, u64::from(ctx.fpr[14].u32l()));
        ctx.fpr[8].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
        sw(m, g[SP], 0x50, u64::from(ctx.fpr[8].u32l()));
        call(imports::func_80081730, m, ctx);
        let g = &mut ctx.gpr;
        ctx.fpr[14].set_u32l(lw(m, g[SP], 0x2C) as u32);
        ctx.fpr[16].set_u32l(lw(m, g[SP], 0x28) as u32);
    }
    let g = &mut ctx.gpr;
    g[T7] = li(0x800D_0000);
    g[T7] = lw(m, g[T7], 0x697C);
    ctx.fpr[10].set_u32l(lw(m, g[SP], 0x54) as u32);
    g[T8] = g[T7] & 0x4000;
    if g[T8] == 0 {
        ctx.fpr[8].set_u32l(lw(m, g[SP], 0x60) as u32);
    } else {
        ctx.fpr[4].set_fl(-ctx.fpr[10].fl());
        sw(m, g[SP], 0x54, u64::from(ctx.fpr[4].u32l()));
        ctx.fpr[8].set_u32l(lw(m, g[SP], 0x60) as u32);
    }
    'b_8000EED4: {
        g[AT] = li(0x800B_0000);
        ctx.fpr[6].u64 = ld(m, g[AT], -0x7980);
        ctx.fpr[10].set_d(f64::from(ctx.fpr[8].fl()));
        ctx.fpr[4].set_u32l(lw(m, g[SP], 0x54) as u32);
        if !(ctx.fpr[6].d() < ctx.fpr[10].d()) {
            g[RA] = lw(m, g[SP], 0x14);
        } else {
            ctx.fpr[6].set_fl(ctx.fpr[4].fl() / ctx.fpr[8].fl());
            g[AT] = li(0x4100_0000);
            ctx.fpr[12].set_u32l(g[AT] as u32);
            g[AT] = li(0x3F80_0000);
            ctx.fpr[18].set_u32l(g[AT] as u32);
            ctx.fpr[4].set_u32l(lw(m, g[SP], 0x34) as u32);
            g[T9] = lw(m, g[SP], 0xB8);
            ctx.fpr[10].set_fl(ctx.fpr[6].fl() + ctx.fpr[18].fl());
            ctx.fpr[6].set_fl(ctx.fpr[10].fl() * ctx.fpr[4].fl());
            ctx.fpr[10].set_u32l(lw(m, g[SP], 0x58) as u32);
            ctx.fpr[4].set_fl(ctx.fpr[10].fl() / ctx.fpr[8].fl());
            ctx.fpr[10].set_u32l(lw(m, g[SP], 0x30) as u32);
            ctx.fpr[2].set_fl(ctx.fpr[6].fl() + ctx.fpr[14].fl());
            ctx.fpr[6].set_fl(ctx.fpr[18].fl() - ctx.fpr[4].fl());
            ctx.fpr[4].set_fl(ctx.fpr[6].fl() * ctx.fpr[10].fl());
            ctx.fpr[6].set_u32l(lw(m, g[SP], 0x5C) as u32);
            ctx.fpr[10].set_fl(ctx.fpr[6].fl() / ctx.fpr[8].fl());
            ctx.fpr[0].set_fl(ctx.fpr[4].fl() + ctx.fpr[16].fl());
            ctx.fpr[4].set_fl(ctx.fpr[14].fl() - ctx.fpr[12].fl());
            let right_of_left = ctx.fpr[4].fl() < ctx.fpr[2].fl();
            sw(m, g[T9], 0, u64::from(ctx.fpr[10].u32l()));
            g[T1] = lw(m, g[SP], 0xBC);
            sw(m, g[T1], 0, u64::from(ctx.fpr[8].u32l()));
            if right_of_left {
                ctx.fpr[6].set_u32l(lw(m, g[SP], 0x24) as u32);
                ctx.fpr[10].set_fl(ctx.fpr[6].fl() + ctx.fpr[14].fl());
                ctx.fpr[8].set_fl(ctx.fpr[10].fl() + ctx.fpr[12].fl());
                if !(ctx.fpr[2].fl() < ctx.fpr[8].fl()) {
                    g[RA] = lw(m, g[SP], 0x14);
                    break 'b_8000EED4;
                }
                ctx.fpr[4].set_fl(ctx.fpr[16].fl() - ctx.fpr[12].fl());
                ctx.fpr[6].set_u32l(lw(m, g[SP], 0x20) as u32);
                if !(ctx.fpr[4].fl() < ctx.fpr[0].fl()) {
                    g[RA] = lw(m, g[SP], 0x14);
                    break 'b_8000EED4;
                }
                ctx.fpr[10].set_fl(ctx.fpr[6].fl() + ctx.fpr[16].fl());
                g[T2] = lw(m, g[SP], 0xB0);
                ctx.fpr[8].set_fl(ctx.fpr[10].fl() + ctx.fpr[12].fl());
                if !(ctx.fpr[0].fl() < ctx.fpr[8].fl()) {
                    g[RA] = lw(m, g[SP], 0x14);
                    break 'b_8000EED4;
                }
                sw(m, g[T2], 0, u64::from(ctx.fpr[2].u32l()));
                g[T3] = lw(m, g[SP], 0xB4);
                sw(m, g[T3], 0, u64::from(ctx.fpr[0].u32l()));
            }
            g[RA] = lw(m, g[SP], 0x14);
        }
    }
    g[SP] = addu(g[SP], 0xA8);
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

/// `func_8000FCBC(i, w, v, x, r, g, b, a)`: for `0 <= i < 2` (64-bit
/// signed), set slot `i` of the pair of blocks [`func_8000FE1C`] resets:
/// `[0x8009B814 + 4i] = w`; the float triple at `0x800D57D0 + 12i` = `*v`
/// (each loaded before its store); the words `0x800D57B0`, `0x800D57A0`,
/// `0x800D5790` `+ 4i` = -1000 (in that order); the float `[0x8009B81C +
/// 4i] = x` (`a3`); then the four bytes at `0x8009B824 + 4i` = the low bytes
/// of the four stack arguments (`sp + 0x10..0x20`), all loaded before the
/// stores.
///
/// Leaves `f12 = x`, `at` = the bound test, `t0 = -1000` for `i >= 0`; on a
/// store `v0 = 4i`, `t6 = 12i`, `t7 = 0x800D57D0`, `v1` = the triple,
/// `f4`/`f6`/`f8` its words, `t8 = 0x8009B824`, `a3` = the bytes' address,
/// `t9 t1 t2 t3` the bytes, `at = 0x800A0000 + 4i`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000FCBC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A3] as u32);
    g[AT] = slt(g[A0], 2);
    if (g[A0] as i64) < 0 {
        return;
    }
    g[T0] = (-0x3E8i64) as u64;
    if g[AT] == 0 {
        return;
    }
    g[V0] = sll(g[A0], 2);
    g[AT] = addu(li(0x800A_0000), g[V0]);
    sw(m, g[AT], -0x47EC, g[A1]);
    g[T6] = sll(g[A0], 2);
    f[4].set_u32l(lw(m, g[A2], 0) as u32);
    g[T6] = subu(g[T6], g[A0]);
    g[T7] = li(0x800D_57D0);
    g[T6] = sll(g[T6], 2);
    g[V1] = addu(g[T6], g[T7]);
    sw(m, g[V1], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A2], 4) as u32);
    g[AT] = addu(li(0x800D_0000), g[V0]);
    sw(m, g[V1], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A2], 8) as u32);
    g[T8] = li(0x8009_B824);
    sw(m, g[V1], 8, u64::from(f[8].u32l()));
    sw(m, g[AT], 0x57B0, g[T0]);
    sw(m, g[AT], 0x57A0, g[T0]);
    sw(m, g[AT], 0x5790, g[T0]);
    g[AT] = addu(li(0x800A_0000), g[V0]);
    sw(m, g[AT], -0x47E4, u64::from(f[12].u32l()));
    g[A3] = addu(g[V0], g[T8]);
    let bytes = [T9, T1, T2, T3];
    for (k, r) in bytes.into_iter().enumerate() {
        g[r] = lbu(m, g[SP], 0x13 + 4 * k as i32);
    }
    for (k, r) in bytes.into_iter().enumerate() {
        sb(m, g[A3], k as i32, g[r]);
    }
}

/// `func_8000FD74(i, j, w, x, y, r, g, b)`: for `0 <= i < 2` and `0 <= j <
/// 8` (64-bit signed), set entry `(i, j)` of the 2x8 tables [`func_8000FE1C`]
/// resets: `[0x8009B82C + 32i + 4j] = w`, the float `[0x800D57E8 + 32i +
/// 4j] = y` (the stack argument at `sp + 0x10`), the float `[0x800D5828 +
/// 32i + 4j] = x` (`a3`), and the three bytes at `0x800D5868 + 24i + 3j` =
/// the low bytes of the stack arguments at `sp + 0x14..0x20`.
///
/// Leaves `f12 = x`, `at` = the last bound test (or an address), `v0 = 4j`
/// once `j >= 0`, and on a store `f4 = y`, `t0`..`t9` the offsets and
/// bytes, `v1` = the bytes' address.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000FD74(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A3] as u32);
    g[AT] = slt(g[A0], 2);
    if (g[A0] as i64) < 0 || g[AT] == 0 {
        return;
    }
    g[AT] = slt(g[A1], 8);
    if (g[A1] as i64) < 0 {
        return;
    }
    g[V0] = sll(g[A1], 2);
    if g[AT] == 0 {
        return;
    }
    g[T6] = sll(g[A0], 5);
    g[T7] = addu(g[T6], g[V0]);
    g[AT] = addu(li(0x800A_0000), g[T7]);
    f[4].set_u32l(lw(m, g[SP], 0x10) as u32);
    sw(m, g[AT], -0x47D4, g[A2]);
    g[T8] = sll(g[A0], 5);
    g[T9] = addu(g[T8], g[V0]);
    g[AT] = addu(li(0x800D_0000), g[T9]);
    g[T2] = sll(g[A0], 2);
    g[T0] = sll(g[A0], 5);
    g[T2] = subu(g[T2], g[A0]);
    g[T3] = sll(g[A1], 2);
    sw(m, g[AT], 0x57E8, u64::from(f[4].u32l()));
    g[T1] = addu(g[T0], g[V0]);
    g[T3] = subu(g[T3], g[A1]);
    g[T2] = sll(g[T2], 3);
    g[T5] = li(0x800D_5868);
    g[AT] = addu(li(0x800D_0000), g[T1]);
    g[T4] = addu(g[T2], g[T3]);
    g[T6] = lbu(m, g[SP], 0x17);
    g[T7] = lbu(m, g[SP], 0x1B);
    g[T8] = lbu(m, g[SP], 0x1F);
    sw(m, g[AT], 0x5828, u64::from(f[12].u32l()));
    g[V1] = addu(g[T4], g[T5]);
    sb(m, g[V1], 0, g[T6]);
    sb(m, g[V1], 1, g[T7]);
    sb(m, g[V1], 2, g[T8]);
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

/// `func_8000FEAC(k, v, w)`: `[0x800D5AA8 + 4k] = w` (the words
/// [`func_8000FE78`] resets to -9999), then the float triple at
/// `0x800D59B8 + 12k` = `*v`. Unbounded (32-bit address arithmetic).
///
/// Leaves `t6 = 4k`, `at = 0x800D0000 + 4k`, `t7 = 12k`, `t8 =
/// 0x800D59B8`, `v0` = the triple, `f4`/`f6`/`f8` its words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000FEAC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = sll(g[A0], 2);
    g[AT] = addu(li(0x800D_0000), g[T6]);
    sw(m, g[AT], 0x5AA8, g[A2]);
    g[T7] = sll(g[A0], 2);
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    g[T7] = subu(g[T7], g[A0]);
    g[T8] = li(0x800D_59B8);
    g[T7] = sll(g[T7], 2);
    g[V0] = addu(g[T7], g[T8]);
    sw(m, g[V0], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    sw(m, g[V0], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 8) as u32);
    sw(m, g[V0], 8, u64::from(f[8].u32l()));
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

/// The float-triple copy [`func_8000FF54`] and [`func_8000FFB8`] share:
/// `v0` = triple `k` of the 40 at `0x800D5C60`, then `*v` copied to it
/// word by word. `after_y` runs between the second word's load and store
/// (the compiler put [`func_8000FF54`]'s extra setup there).
fn copy_triple(m: &mut Mem, g: &mut [u64; 32], f: &mut [crate::recomp::Fpr; 32], after_y: impl FnOnce(&mut [u64; 32])) {
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    g[T6] = subu(g[T6], g[A0]);
    g[T7] = li(0x800D_5C60);
    g[T6] = sll(g[T6], 2);
    g[V0] = addu(g[T6], g[T7]);
    sw(m, g[V0], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    after_y(g);
    sw(m, g[V0], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 8) as u32);
    sw(m, g[V0], 8, u64::from(f[8].u32l()));
}

/// `func_8000FF54(k, v)`: if `k < 40` (64-bit signed), the float triple at
/// `0x800D5C60 + 12k` = `*v`, `[0x800D60A0 + 4k] = -1000`, and the flag
/// byte `[0x800D5C38 + k] = 1` ([`func_8000FFF8`]'s bytes). QUIRK: negative
/// `k` passes and writes before the arrays (as in [`func_8000FFF8`]).
///
/// Leaves `at` = the bound test (or `0x800D0000 + k` past it), `t6 = 4k`
/// then `12k`, and past it `t7 = 0x800D5C60`, `v0` = the triple, `f4`..`f8`
/// its words, `t9 = 4k`, `t8 = -1000`, `t0 = 1`.
///
/// Domain: the three stores in RDRAM for `k < 40`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000FF54(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0x28);
    g[T6] = sll(g[A0], 2);
    if g[AT] == 0 {
        return;
    }
    copy_triple(m, g, &mut ctx.fpr, |g| {
        g[T9] = sll(g[A0], 2);
        g[AT] = li(0x800D_0000);
    });
    g[AT] = addu(g[AT], g[T9]);
    g[T8] = (-0x3E8i64) as u64;
    sw(m, g[AT], 0x60A0, g[T8]);
    g[AT] = addu(li(0x800D_0000), g[A0]);
    g[T0] = 1;
    sb(m, g[AT], 0x5C38, g[T0]);
}

/// `func_8000FFB8(k, v)`: if `k < 40` (64-bit signed), the float triple at
/// `0x800D5C60 + 12k` = `*v` ([`func_8000FF54`] without the marker and the
/// flag). QUIRK: negative `k` passes.
///
/// Leaves `at` = the bound test, `t6 = 4k` then `12k`, and past it `t7 =
/// 0x800D5C60`, `v0` = the triple, `f4`..`f8` its words.
///
/// Domain: the triple in RDRAM for `k < 40`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8000FFB8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = slt(g[A0], 0x28);
    g[T6] = sll(g[A0], 2);
    if g[AT] == 0 {
        return;
    }
    copy_triple(m, g, &mut ctx.fpr, |_| {});
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

/// `func_800116E8()` (turn off the 80 listed records, **guess**): for `i`
/// in `0..0x50`, if the word `v = [0x800D6140 + 4i]` is not -1,
/// [`func_8000A920`]`(v, 0)` (its low halfword as the id); then the byte
/// `[0x800D68C0 + i] = 0`.
///
/// Frame (`sp - 0x28`): `ra`, `s3`, `s2`, `s1`, `s0` at `+0x24`, `+0x20`,
/// `+0x1C`, `+0x18`, `+0x14`, restored. Leaves `v0` = the last word, `a0`,
/// `a1 = 0`, `t6` and the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800116E8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x28i64) as u64);
    sw(m, g[SP], 0x20, g[S3]);
    sw(m, g[SP], 0x18, g[S1]);
    sw(m, g[SP], 0x14, g[S0]);
    sw(m, g[SP], 0x1C, g[S2]);
    g[S0] = li(0x800D_0000);
    g[S1] = li(0x800D_0000);
    g[S3] = li(0x800D_0000);
    sw(m, g[SP], 0x24, g[RA]);
    g[S3] = addu(g[S3], 0x6910);
    g[S1] = addu(g[S1], 0x6140);
    g[S0] = addu(g[S0], 0x68C0);
    g[S2] = u64::MAX;
    loop {
        let g = &mut ctx.gpr;
        g[V0] = lw(m, g[S1], 0);
        g[A1] = 0;
        g[A0] = sll(g[V0], 16);
        if g[S2] != g[V0] {
            g[T6] = sra(g[A0], 16);
            g[A0] = g[T6];
            call(imports::func_8000A920, m, ctx);
        }
        let g = &mut ctx.gpr;
        g[S0] = addu(g[S0], 1);
        g[S1] = addu(g[S1], 4);
        sb(m, g[S0], -1, 0);
        if g[S0] == g[S3] {
            break;
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x24);
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[S2] = lw(m, g[SP], 0x1C);
    g[S3] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x28);
}

/// `func_80011764(x, y)` with the floats in `f12`/`f14`: `[0x8009B878] =
/// x`, `[0x8009B87C] = y`. Leaves `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011764(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    ctx.gpr[AT] = li(0x800A_0000);
    sw(&mut mem, ctx.gpr[AT], -0x4788, u64::from(ctx.fpr[12].u32l()));
    sw(&mut mem, ctx.gpr[AT], -0x4784, u64::from(ctx.fpr[14].u32l()));
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

/// `func_800117E4(x)` with the float in `f12`: `[0x8009B880] = x`. Leaves
/// `at = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800117E4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    ctx.gpr[AT] = li(0x800A_0000);
    sw(&mut mem, ctx.gpr[AT], -0x4780, u64::from(ctx.fpr[12].u32l()));
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

/// `func_800118F8()`: [`func_8000F5A0`](crate::render::func_8000F5A0)
/// through a frame (`sp - 0x18`, `ra` at `+0x14`); leaves its registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800118F8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    call(imports::func_8000F5A0, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
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

/// `func_80011EA4(k)`: [`func_80011E54`]`(k)` with `k` as a sign-extended
/// halfword (select pointer `k`, clamped), through a frame (`sp - 0x18`,
/// `ra` at `+0x14`, `k` spilled to its home slot `sp + 0`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80011EA4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x18, g[A0]);
    g[T6] = sll(g[A0], 16);
    g[A0] = sra(g[T6], 16);
    call(imports::func_80011E54, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

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

/// `func_800129B8(s, k)`: [`func_800129E4`]`(s, [0x800D6940 + 4k])`, the
/// width of text `s` in font `k` (`k`'s low word, unbounded), through a
/// frame (`sp - 0x18`, `ra` at `+0x14`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800129B8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A1], 2);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[A1] = li(0x800D_0000);
    sw(m, g[SP], 0x14, g[RA]);
    g[A1] = addu(g[A1], g[T6]);
    g[A1] = lw(m, g[A1], 0x6940);
    call(imports::func_800129E4, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_800129E4(s, font)` (text width, **guess**: of one line): the sum
/// of the glyph advances (the signed halfword `+2` of 16-byte glyph
/// records, 32-bit wrapping) of the characters of `s`, up to a NUL or
/// `"~n"` (both end it without a glyph). `"~~"` is a literal `~`; any
/// other `"~X"` adds nothing. The font: `+0x5A` first and `+0x5B` last
/// character (bytes), `+0x5C` the glyph table (or 0), `+0x60` an extended
/// table (or 0).
///
/// Per character `c` (unsigned): a lowercase `c` becomes uppercase if the
/// font's last character is below `'a'`. If `c >= 0x97` and the extended
/// table is set, the byte `k = [0x800A1C86 + c]` (unless 0xFF) selects the
/// pair at `0x800A1CD8 + 2k`: if its second byte is 0xFF, the glyph is
/// extended glyph `pair[0]` and `c` becomes 0; otherwise `c` becomes that
/// byte. Then, if the glyph table is set and `first <= c <= last`, the glyph
/// is `table[c - first]` instead. QUIRK: so with `first == 0` an extended
/// glyph is replaced by glyph 0. QUIRK: a `~` just before the NUL takes the
/// NUL as its escape, so the scan runs on past the string (unbounded, like
/// the string itself).
///
/// Saves and restores `s0`..`s3` (sign-extended) in a 0x18-byte frame.
/// Leaves `v0 = a3` = the width, `v1 = 1`, `a0 = '~'`, `t0 = 'n'`, `a2 =
/// 0xFF`, `t1 = 0x800A1D1C`, `t2 = 0x800A1CD8`, and `t3`..`t9`, `at` as the
/// last character left them.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800129E4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[V0] = g[A0];
    sw(m, g[SP], 0x14, g[S3]);
    sw(m, g[SP], 0x10, g[S2]);
    sw(m, g[SP], 0xC, g[S1]);
    sw(m, g[SP], 8, g[S0]);
    g[A3] = 0;
    g[V1] = 0;
    g[T1] = li(0x800A_1D1C);
    g[T2] = li(0x800A_1CD8);
    g[A0] = 0x7E;
    g[T0] = 0x6E;
    g[A2] = 0xFF;
    loop {
        // t3 = the character to look up, t5 = nonzero if it has a glyph.
        g[T3] = lbu(m, g[V0], 0);
        g[T5] = g[T3];
        if g[T3] == 0 {
            g[V1] = 1;
        }
        if g[A0] == g[T5] {
            g[S2] = lbu(m, g[V0], 1);
            g[V0] = addu(g[V0], 1);
            if g[T0] == g[S2] {
                g[V1] = 1;
            } else {
                g[T3] = 0;
                if g[A0] == g[S2] {
                    g[T3] = 0x7E;
                    g[T5] = 0x7E;
                } else {
                    g[T5] = 0;
                }
            }
        }
        if g[T5] != 0 {
            g[S0] = 0;
            if g[V1] == 0 {
                g[AT] = slt(g[T5], 0x61);
                g[S3] = lw(m, g[A1], 0x5C);
                g[T4] = g[T3] & 0xFF;
                if g[AT] == 0 {
                    g[AT] = slt(g[T5], 0x7B);
                    if g[AT] != 0 {
                        g[T6] = lbu(m, g[A1], 0x5B);
                        g[AT] = slt(g[T6], 0x61);
                        if g[AT] != 0 {
                            // No lowercase in the font: fold to uppercase.
                            g[T4] = addu(g[T5], (-0x20i64) as u64);
                            g[T7] = g[T4] & 0xFF;
                            g[T4] = g[T7];
                        }
                    }
                }
                g[AT] = slt(g[T4], 0x97);
                g[T3] = g[T4];
                if g[AT] == 0 {
                    // High characters: remapped through the two tables.
                    g[T5] = lw(m, g[A1], 0x60);
                    g[T8] = addu(g[T1], g[T3]);
                    if g[T5] != 0 {
                        g[S1] = lbu(m, g[T8], -0x96);
                        g[T9] = sll(g[S1], 1);
                        if g[A2] != g[S1] {
                            g[S2] = addu(g[T2], g[T9]);
                            g[T4] = lbu(m, g[S2], 1);
                            g[T3] = lbu(m, g[S2], 0);
                            g[T6] = sll(g[T3], 4);
                            if g[A2] == g[T4] {
                                g[S0] = addu(g[T5], g[T6]);
                                g[T4] = 0;
                            }
                        }
                    }
                }
                if g[S3] != 0 {
                    g[T5] = lbu(m, g[A1], 0x5A);
                    g[AT] = slt(g[T4], g[T5]);
                    if g[AT] == 0 {
                        g[T7] = lbu(m, g[A1], 0x5B);
                        g[T8] = subu(g[T4], g[T5]);
                        g[T9] = sll(g[T8], 4);
                        g[AT] = slt(g[T7], g[T4]);
                        if g[AT] == 0 {
                            g[S0] = addu(g[S3], g[T9]);
                        }
                    }
                }
                if g[S0] != 0 {
                    g[T6] = lh(m, g[S0], 2);
                    g[A3] = addu(g[A3], g[T6]);
                }
            }
        }
        g[V0] = addu(g[V0], 1);
        if g[V1] != 0 {
            break;
        }
    }
    g[S0] = lw(m, g[SP], 8);
    g[S1] = lw(m, g[SP], 0xC);
    g[S2] = lw(m, g[SP], 0x10);
    g[S3] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
    g[V0] = g[A3];
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

/// `func_80017B7C(n, x, y, z)`: set a node's translation `+0x40..+0x48 =
/// (x, y, z)` (floats in `a1`, `a2`, `a3`, `z` through its spill `[sp +
/// 0xC]`) and mark it changed: the halfword flags `+0xC |= 3` (read before
/// the stores, written before `z`).
///
/// Leaves `f12 = x`, `f14 = y`, `f4 = z`, `t6`/`t7` = the old/new flags.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017B7C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A1] as u32);
    f[14].set_u32l(g[A2] as u32);
    sw(m, g[SP], 0xC, g[A3]);
    g[T6] = lhu(m, g[A0], 0xC);
    sw(m, g[A0], 0x40, u64::from(f[12].u32l()));
    sw(m, g[A0], 0x44, u64::from(f[14].u32l()));
    f[4].set_u32l(lw(m, g[SP], 0xC) as u32);
    g[T7] = g[T6] | 3;
    sh(m, g[A0], 0xC, g[T7]);
    sw(m, g[A0], 0x48, u64::from(f[4].u32l()));
}

/// `func_80017BA8(n, m)`: set a node's 3x4 transform `+0x1C..+0x48` from
/// the 4x4 matrix `m`: its rows' first three columns, row by row (`m + 0`,
/// `+0x10`, `+0x20`, `+0x30`), word by word; the flags halfword `+0xC |= 3`
/// (read first, written before the last word).
///
/// Leaves `t6`/`t7` = the old/new flags and `f4`..`f18` = the last words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017BA8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    g[T6] = lhu(m, g[A0], 0xC);
    sw(m, g[A0], 0x1C, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    g[T7] = g[T6] | 3;
    sw(m, g[A0], 0x20, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 8) as u32);
    sw(m, g[A0], 0x24, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x10) as u32);
    sw(m, g[A0], 0x28, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A1], 0x14) as u32);
    sw(m, g[A0], 0x2C, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0x18) as u32);
    sw(m, g[A0], 0x30, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x20) as u32);
    sw(m, g[A0], 0x34, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x24) as u32);
    sw(m, g[A0], 0x38, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x28) as u32);
    sw(m, g[A0], 0x3C, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x30) as u32);
    sw(m, g[A0], 0x40, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A1], 0x34) as u32);
    sw(m, g[A0], 0x44, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0x38) as u32);
    sh(m, g[A0], 0xC, g[T7]);
    sw(m, g[A0], 0x48, u64::from(f[18].u32l()));
}

/// `func_80017C18(n, m)`: the reverse of [`func_80017BA8`]: the 4x4 matrix
/// `m` = a node's 3x4 transform `+0x1C..+0x48` as its rows' first three
/// columns, with the fourth column (0, 0, 0, 1). Word by word; the zeros
/// and the 1.0 are interleaved with the copies.
///
/// Leaves `f0 = 0.0`, `at = 0x3F800000`, `f4` = 1.0 (after its last use as
/// a copy), and `f6`..`f18` = the last words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017C18(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A0], 0x1C) as u32);
    f[0].set_u32l(0 as u32);
    g[AT] = li(0x3F80_0000);
    sw(m, g[A1], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0x20) as u32);
    sw(m, g[A1], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x24) as u32);
    sw(m, g[A1], 0xC, u64::from(f[0].u32l()));
    sw(m, g[A1], 8, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0x28) as u32);
    sw(m, g[A1], 0x10, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A0], 0x2C) as u32);
    sw(m, g[A1], 0x14, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x30) as u32);
    sw(m, g[A1], 0x1C, u64::from(f[0].u32l()));
    sw(m, g[A1], 0x18, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A0], 0x34) as u32);
    sw(m, g[A1], 0x20, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0x38) as u32);
    f[4].set_u32l(g[AT] as u32);
    sw(m, g[A1], 0x24, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x3C) as u32);
    sw(m, g[A1], 0x2C, u64::from(f[0].u32l()));
    sw(m, g[A1], 0x28, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0x40) as u32);
    sw(m, g[A1], 0x30, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A0], 0x44) as u32);
    sw(m, g[A1], 0x34, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x48) as u32);
    sw(m, g[A1], 0x3C, u64::from(f[4].u32l()));
    sw(m, g[A1], 0x38, u64::from(f[18].u32l()));
}

/// `func_80017C98(n, m)`: the same code as [`func_80017C18`], instruction
/// for instruction (a second copy in the ROM).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017C98(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A0], 0x1C) as u32);
    f[0].set_u32l(0 as u32);
    g[AT] = li(0x3F80_0000);
    sw(m, g[A1], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0x20) as u32);
    sw(m, g[A1], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x24) as u32);
    sw(m, g[A1], 0xC, u64::from(f[0].u32l()));
    sw(m, g[A1], 8, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0x28) as u32);
    sw(m, g[A1], 0x10, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A0], 0x2C) as u32);
    sw(m, g[A1], 0x14, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x30) as u32);
    sw(m, g[A1], 0x1C, u64::from(f[0].u32l()));
    sw(m, g[A1], 0x18, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A0], 0x34) as u32);
    sw(m, g[A1], 0x20, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0x38) as u32);
    f[4].set_u32l(g[AT] as u32);
    sw(m, g[A1], 0x24, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x3C) as u32);
    sw(m, g[A1], 0x2C, u64::from(f[0].u32l()));
    sw(m, g[A1], 0x28, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0x40) as u32);
    sw(m, g[A1], 0x30, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A0], 0x44) as u32);
    sw(m, g[A1], 0x34, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x48) as u32);
    sw(m, g[A1], 0x3C, u64::from(f[4].u32l()));
    sw(m, g[A1], 0x38, u64::from(f[18].u32l()));
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

/// `func_80017D58(n, k, x)`: for `0 <= k < 8` (64-bit signed), the float
/// `[n + 0x1C + 4k] = x` (passed in `a2`).
///
/// Leaves `f12 = x`, `at` = the bound test, and for `k < 8` `t6 = 4k`
/// (and `t7 = n + 4k` when it stores).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017D58(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A2] as u32);
    g[AT] = slt(g[A1], 8);
    if g[AT] != 0 {
        g[T6] = sll(g[A1], 2);
        if (g[A1] as i64) >= 0 {
            g[T7] = addu(g[A0], g[T6]);
            sw(m, g[T7], 0x1C, u64::from(f[12].u32l()));
        }
    }
    // L_80017D78
}

/// `func_80017D80(n, k)`: for `0 <= k < 8` (64-bit signed), `f0 = [n + 0x1C
/// + 4k]`; otherwise `f0` is left as it was (the caller's value comes back).
///
/// Leaves `at` = the bound test, and for `k < 8` `t6 = 4k` (and `t7`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017D80(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = slt(g[A1], 8);
    if g[AT] != 0 {
        g[T6] = sll(g[A1], 2);
        if (g[A1] as i64) >= 0 {
            g[T7] = addu(g[A0], g[T6]);
            f[0].set_u32l(lw(m, g[T7], 0x1C) as u32);
        }
    }
    // L_80017D9C
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

/// `func_80017E20(n, out)`: copy the six floats at `n + 8..n + 0x20` to
/// `out`, word by word. Leaves `f4`..`f18` = the words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017E20(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A0], 8) as u32);
    sw(m, g[A1], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0xC) as u32);
    sw(m, g[A1], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x10) as u32);
    sw(m, g[A1], 8, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0x14) as u32);
    sw(m, g[A1], 0xC, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A0], 0x18) as u32);
    sw(m, g[A1], 0x10, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x1C) as u32);
    sw(m, g[A1], 0x14, u64::from(f[18].u32l()));
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

/// `func_80017FD0(o, m)` (a node's world matrix, **guess**): copies the 16
/// words of `m` to `o + 0xB0` (each loaded, then stored), then `o + 0x70 =
/// (o + 0x30) * (o + 0xB0)` ([`func_80015724`](crate::math::func_80015724)).
///
/// Frame (`sp - 0x20`): `ra`, `s0` at `+0x1C`, `+0x18`, restored. Leaves
/// `a3 = o` and the callee's registers.
///
/// Domain: canonical `o`, `m` in RDRAM, `m` disjoint from `o + 0xB0..+0xF0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80017FD0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x18, g[S0]);
    ctx.fpr[4].set_u32l(lw(m, g[A1], 0) as u32);
    g[A3] = g[A0];
    g[A2] = addu(g[A3], 0xB0);
    sw(m, g[A0], 0xB0, u64::from(ctx.fpr[4].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[A1], 4) as u32);
    g[A0] = addu(g[A0], 0x70);
    sw(m, g[A0], 0x44, u64::from(ctx.fpr[6].u32l()));
    ctx.fpr[8].set_u32l(lw(m, g[A1], 8) as u32);
    sw(m, g[A0], 0x48, u64::from(ctx.fpr[8].u32l()));
    ctx.fpr[10].set_u32l(lw(m, g[A1], 0xC) as u32);
    sw(m, g[A0], 0x4C, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[16].set_u32l(lw(m, g[A1], 0x10) as u32);
    sw(m, g[A0], 0x50, u64::from(ctx.fpr[16].u32l()));
    ctx.fpr[18].set_u32l(lw(m, g[A1], 0x14) as u32);
    sw(m, g[A0], 0x54, u64::from(ctx.fpr[18].u32l()));
    ctx.fpr[4].set_u32l(lw(m, g[A1], 0x18) as u32);
    sw(m, g[A0], 0x58, u64::from(ctx.fpr[4].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[A1], 0x1C) as u32);
    sw(m, g[A0], 0x5C, u64::from(ctx.fpr[6].u32l()));
    ctx.fpr[8].set_u32l(lw(m, g[A1], 0x20) as u32);
    sw(m, g[A0], 0x60, u64::from(ctx.fpr[8].u32l()));
    ctx.fpr[10].set_u32l(lw(m, g[A1], 0x24) as u32);
    sw(m, g[A0], 0x64, u64::from(ctx.fpr[10].u32l()));
    ctx.fpr[16].set_u32l(lw(m, g[A1], 0x28) as u32);
    sw(m, g[A0], 0x68, u64::from(ctx.fpr[16].u32l()));
    ctx.fpr[18].set_u32l(lw(m, g[A1], 0x2C) as u32);
    sw(m, g[A0], 0x6C, u64::from(ctx.fpr[18].u32l()));
    ctx.fpr[4].set_u32l(lw(m, g[A1], 0x30) as u32);
    sw(m, g[A0], 0x70, u64::from(ctx.fpr[4].u32l()));
    ctx.fpr[6].set_u32l(lw(m, g[A1], 0x34) as u32);
    sw(m, g[A0], 0x74, u64::from(ctx.fpr[6].u32l()));
    ctx.fpr[8].set_u32l(lw(m, g[A1], 0x38) as u32);
    sw(m, g[A0], 0x78, u64::from(ctx.fpr[8].u32l()));
    ctx.fpr[10].set_u32l(lw(m, g[A1], 0x3C) as u32);
    g[A1] = addu(g[A3], 0x30);
    sw(m, g[A0], 0x7C, u64::from(ctx.fpr[10].u32l()));
    call(imports::func_80015724, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_80018084(o, k, x)`: store the float `x` (in `a2`) at `+0x148` for
/// `k == 2`, `+0x14C` for 3, `+0x150` for 5 (full 64-bit compares);
/// anything else stores nothing.
///
/// Leaves `f12 = x`, `at` = 3 for `k == 2`, else 5.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80018084(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A2] as u32);
    g[AT] = 2;
    let c0 = g[A1] == g[AT];
    g[AT] = 3;
    if c0 {
        // L_800180AC
        sw(m, g[A0], 0x148, u64::from(f[12].u32l()));
    } else {
        let c1 = g[A1] == g[AT];
        g[AT] = 5;
        if c1 {
            // L_800180B4
            sw(m, g[A0], 0x14C, u64::from(f[12].u32l()));
        } else if g[A1] == g[AT] {
            sw(m, g[A0], 0x150, u64::from(f[12].u32l()));
            // L_800180C0
        }
    }
}

/// `func_800180C8(o, k)`: the getter for [`func_80018084`]: `f0` = the
/// float at `+0x148` (`k == 2`), `+0x14C` (3), `+0x150` (5), else -1.0.
///
/// Leaves `at` = 3 for `k == 2`, 5 for 3 or 5, else `0xBF800000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800180C8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = 2;
    let c0 = g[A1] == g[AT];
    g[AT] = 3;
    if c0 {
        // L_800180EC
        f[0].set_u32l(lw(m, g[A0], 0x148) as u32);
    } else {
        let c1 = g[A1] == g[AT];
        g[AT] = 5;
        if c1 {
            // L_800180F4
            f[0].set_u32l(lw(m, g[A0], 0x14C) as u32);
        } else if g[A1] == g[AT] {
            // L_800180FC
            f[0].set_u32l(lw(m, g[A0], 0x150) as u32);
        } else {
            g[AT] = li(0xBF80_0000);
            // L_80018104
            f[0].set_u32l(g[AT] as u32);
        }
    }
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

/// `func_800181BC(n, which, v, flags, op)` (update a node's flag word,
/// **guess**): `op` is a stack argument. 0 for a null `n` or `which`
/// other than 0 and 2. The word is `n + 8` for `which == 0`, `n + 4` for
/// `which == 2`; its address is returned in `v0` unless `flags & 0x20`,
/// when `v0` is what the last call returned (the type word, or the child
/// count: QUIRK, nothing preserves it). If `flags & 0x10`: `op` 2
/// ORs `v` in, 3 ANDs it, 1 stores it (other ops nothing). If `flags &
/// 0x20` and the node's type ([`func_80017DA4`]) has bit 14, it calls
/// itself on each child ([`func_80017DB4`], the count [`func_80017DAC`]
/// re-read each step) with `(which, v, flags & 0x10, op)`. QUIRK: the
/// children get `flags & 0x10` only, so the walk stops one level down.
///
/// Frame (`sp - 0x38`): `ra`, `s5`..`s0` at `+0x34..+0x1C`, restored; `op`
/// passed on at `+0x10`. Leaves `t0 = flags & 0x20` and the callees'
/// registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800181BC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_800182D8: {
        g[SP] = addu(g[SP], (-0x38i64) as u64);
        sw(m, g[SP], 0x30, g[S5]);
        sw(m, g[SP], 0x2C, g[S4]);
        sw(m, g[SP], 0x20, g[S1]);
        g[S1] = g[A0];
        g[S4] = g[A2];
        g[S5] = g[A1];
        sw(m, g[SP], 0x34, g[RA]);
        sw(m, g[SP], 0x28, g[S3]);
        sw(m, g[SP], 0x24, g[S2]);
        sw(m, g[SP], 0x1C, g[S0]);
        if g[A0] != 0 {
            g[S2] = g[A3] & 0x10;
            if g[S5] != 0 {
                g[AT] = 2;
                if g[S5] != g[AT] {
                    g[V0] = 0;
                    break 'b_800182D8;
                }
                g[V0] = addu(g[S1], 4);
            } else {
                g[V0] = addu(g[S1], 8);
            }
            g[T0] = g[A3] & 0x20;
            if g[S2] != 0 {
                g[S3] = lw(m, g[SP], 0x48);
                g[AT] = 2;
                if g[S3] != g[AT] {
                    g[AT] = 3;
                    if g[S3] != g[AT] {
                        g[AT] = 1;
                        if g[S3] == g[AT] {
                            sw(m, g[V0], 0, g[S4]);
                        }
                    } else {
                        g[T8] = lw(m, g[V0], 0);
                        g[T9] = g[T8] & g[S4];
                        sw(m, g[V0], 0, g[T9]);
                    }
                } else {
                    g[T6] = lw(m, g[V0], 0);
                    g[T7] = g[T6] | g[S4];
                    sw(m, g[V0], 0, g[T7]);
                }
            }
            g[S3] = lw(m, g[SP], 0x48);
            if g[T0] != 0 {
                g[A0] = g[S1];
                call(imports::func_80017DA4, m, ctx);
                let g = &mut ctx.gpr;
                g[T1] = g[V0] & 0x4000;
                g[S0] = 0;
                if g[T1] != 0 {
                    g[A0] = g[S1];
                    call(imports::func_80017DAC, m, ctx);
                    let g = &mut ctx.gpr;
                    g[A0] = g[S1];
                    if (g[V0] as i64) > 0 {
                        loop {
                            let g = &mut ctx.gpr;
                            g[A1] = g[S0];
                            call(imports::func_80017DB4, m, ctx);
                            let g = &mut ctx.gpr;
                            g[A0] = g[V0];
                            g[A1] = g[S5];
                            g[A2] = g[S4];
                            g[A3] = g[S2];
                            sw(m, g[SP], 0x10, g[S3]);
                            call(imports::func_800181BC, m, ctx);
                            let g = &mut ctx.gpr;
                            g[S0] = addu(g[S0], 1);
                            g[A0] = g[S1];
                            call(imports::func_80017DAC, m, ctx);
                            let g = &mut ctx.gpr;
                            g[AT] = slt(g[S0], g[V0]);
                            if g[AT] == 0 {
                                break;
                            }
                            g[A0] = g[S1];
                        }
                    }
                }
            }
        } else {
            g[V0] = 0;
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x34);
    g[S0] = lw(m, g[SP], 0x1C);
    g[S1] = lw(m, g[SP], 0x20);
    g[S2] = lw(m, g[SP], 0x24);
    g[S3] = lw(m, g[SP], 0x28);
    g[S4] = lw(m, g[SP], 0x2C);
    g[S5] = lw(m, g[SP], 0x30);
    g[SP] = addu(g[SP], 0x38);
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

/// `func_80018324(n, type)`: initialise a node header (**guess**: a model
/// node): `+0 = type`, `+4 = +8 = -1`, halfwords `+0xC = +0xE = 0`, `+0x10 =
/// 0`; if `type & 0x4000`, also `+0x14 = +0x18 = 0`, and for `type ==
/// 0xD065` (64-bit compare) the 3x4 transform `+0x1C..+0x48` = identity
/// (zeros first, then 1.0 at `+0x1C`, `+0x2C`, `+0x3C`) and `+0x4C..+0x54`
/// = 0.
///
/// Leaves `v0 = -1`, `t6 = type & 0x4000`, `at = 0xD065` (or `0x3F800000`
/// after the identity), `f0 = 0.0`, `f2 = 1.0`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80018324(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = u64::MAX;
    g[T6] = g[A1] & 0x4000;
    sw(m, g[A0], 0, g[A1]);
    sw(m, g[A0], 4, g[V0]);
    sw(m, g[A0], 8, g[V0]);
    sh(m, g[A0], 0xC, 0);
    sh(m, g[A0], 0xE, 0);
    sw(m, g[A0], 0x10, 0);
    if g[T6] != 0 {
        g[AT] = 0xD065;
        sw(m, g[A0], 0x14, 0);
        sw(m, g[A0], 0x18, 0);
        if g[A1] == g[AT] {
            f[0].set_u32l(0 as u32);
            g[AT] = li(0x3F80_0000);
            f[2].set_u32l(g[AT] as u32);
            sw(m, g[A0], 0x4C, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x50, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x54, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x20, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x24, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x28, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x30, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x34, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x38, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x40, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x44, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x48, u64::from(f[0].u32l()));
            sw(m, g[A0], 0x1C, u64::from(f[2].u32l()));
            sw(m, g[A0], 0x2C, u64::from(f[2].u32l()));
            sw(m, g[A0], 0x3C, u64::from(f[2].u32l()));
        }
    }
    // L_800183A0
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

/// `func_800183C0()`: reset the block at `0x800D6960` (the settings word
/// [`func_80038DBC`] and [`func_800358A0`] use is its `+0x1C`): the settings
/// word goes 0, 1, then 9; `+0x18 = 0`, halfwords `+0x20 = 0`, `+0x22 = 2`,
/// `+0x2C = 0`, `+0x30 = 1`, halfwords `+0x34 = 30`, `+0x36 = 0`, `+0x38 =
/// 0`, the six floats `+0..+0x14 = 0.0`, then `+0x28 = +0x24 = 1.0`.
///
/// Leaves `v0 = 0x800D6960`, `f0 = 0.0`, `f2 = 1.0`, `at = 0x3F800000`,
/// `t8 = 1`, `t6 = 2`, `t0 = 9`, `t1 = 1`, `t2 = 30`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800183C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = li(0x800D_6960);
    f[0].set_u32l(0 as u32);
    g[AT] = li(0x3F80_0000);
    f[2].set_u32l(g[AT] as u32);
    g[T8] = 1;
    sw(m, g[V0], 0x1C, 0);
    g[T6] = 2;
    sw(m, g[V0], 0x1C, g[T8]);
    g[T0] = g[T8] | 8;
    g[T1] = 1;
    g[T2] = 0x1E;
    sw(m, g[V0], 0x18, 0);
    sh(m, g[V0], 0x20, 0);
    sh(m, g[V0], 0x22, g[T6]);
    sw(m, g[V0], 0x1C, g[T0]);
    sw(m, g[V0], 0x2C, 0);
    sw(m, g[V0], 0x30, g[T1]);
    sh(m, g[V0], 0x34, g[T2]);
    sh(m, g[V0], 0x36, 0);
    sw(m, g[V0], 0x38, 0);
    sw(m, g[V0], 0, u64::from(f[0].u32l()));
    sw(m, g[V0], 4, u64::from(f[0].u32l()));
    sw(m, g[V0], 8, u64::from(f[0].u32l()));
    sw(m, g[V0], 0xC, u64::from(f[0].u32l()));
    sw(m, g[V0], 0x10, u64::from(f[0].u32l()));
    sw(m, g[V0], 0x14, u64::from(f[0].u32l()));
    sw(m, g[V0], 0x28, u64::from(f[2].u32l()));
    sw(m, g[V0], 0x24, u64::from(f[2].u32l()));
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

/// `func_800290A4()` (fade-in step, **guess**: it lowers the alpha of the
/// colour global -103 from 255 to 0, then turns it off). With the float
/// `F` = `[0x800A2604]`, the s16 `H` = `[0x800A2608]`, the word `N` =
/// `[0x800A260C]`, the frame time `dt` = `[0x80120BF8]` and the rate `k` =
/// `[0x800A9DEC]` (850.0 in the ROM):
/// 1. If `F == 255.0` or `N > 0` (`N` read only if `F != 255.0`):
///    [`func_8000AB24`]`(-103, 0, 0, 0, 255)`, then `N` = `N - 1` (re-read)
///    is stored; if it is still positive, return 0.
/// 2. `F` is re-read. If `F <= 0`: `H = s16(H - 1)` is stored, the result
///    will be 1 if that is `<= 0`, and `F = 0.0`. Otherwise `F = F - k * dt`
///    (the result so far 0).
/// 3. If `F < 0`: `F = 0.0` and `H = 3`.
/// 4. `[0x800A2604] = F` and [`func_8000AB24`]`(-103, 0, 0, 0, u(F))`, `u`
///    being IDO's float → unsigned idiom ([`fpu::to_unsigned_s`]).
/// 5. If the result is 1: [`func_8000A920`]`(-103, 0)`, `[0x800A4BD8] = 1`,
///    `[0x800A4BDC] = 0`, `N = 3`, `[0x800A2604] = 255.0`. Returns the
///    result.
///
/// The idiom's second path (for `F` from 2^31) is dead under the oracle
/// (the flag test reads 0) and kept as the C has it.
///
/// Frame (`sp - 0x28`): `ra` at `+0x1C`, the result spilled to `+0x24`
/// around each call, the fifth argument at `+0x10`. Leaves `a0 = -103`
/// (as the callees leave it), `a1` = 0, `v1` = `N - 1` after an early
/// return, else 3; `t0 =
/// u(F)`, `t9 = 0` (the saved FCR31), `t6 = 255` if step 1 ran, `t7`/`t8`
/// from step 2's decrement, `t1 = 1` and `f2 = 255.0` after step 5, else
/// `f2 = F`; `f0 = dt`, `f12 = 0.0`, `f4 = 255.0` and `f6`/`f8`/`f10` from
/// the arithmetic and the conversion; and the callees' registers.
///
/// Domain: `F` not NaN, and `k`, `dt` not NaN when `F > 0` (NaN operands of
/// the subtraction).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800290A4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let mut fcr31 = fpu::NEAREST;
    'b_80029288: {
        'b_80029118: {
            g[AT] = li(0x800A_0000);
            ctx.fpr[2].set_u32l(lw(m, g[AT], 0x2604) as u32);
            g[AT] = li(0x437F_0000);
            ctx.fpr[4].set_u32l(g[AT] as u32);
            g[SP] = addu(g[SP], (-0x28i64) as u64);
            sw(m, g[SP], 0x1C, g[RA]);
            let full = ctx.fpr[2].fl() == ctx.fpr[4].fl();
            g[V0] = 0;
            g[V1] = li(0x800A_0000);
            g[A0] = (-0x67i64) as u64;
            g[A1] = 0;
            if !full {
                g[V1] = lw(m, g[V1], 0x260C);
                if (g[V1] as i64) <= 0 {
                    g[T6] = 0xFF;
                    break 'b_80029118;
                }
            }
            // Step 1: colour -103 = (0, 0, 0, 255), count N down.
            g[T6] = 0xFF;
            sw(m, g[SP], 0x10, g[T6]);
            g[A2] = 0;
            g[A3] = 0;
            sw(m, g[SP], 0x24, g[V0]);
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[V1] = li(0x800A_0000);
            g[V1] = lw(m, g[V1], 0x260C);
            g[AT] = li(0x800A_0000);
            g[V0] = lw(m, g[SP], 0x24);
            g[V1] = addu(g[V1], u64::MAX);
            sw(m, g[AT], 0x260C, g[V1]);
            if (g[V1] as i64) > 0 {
                g[V0] = 0;
                break 'b_80029288;
            }
        }
        // Step 2: F steps down, or H counts down once F is 0.
        let g = &mut ctx.gpr;
        g[AT] = li(0x800A_0000);
        ctx.fpr[2].set_u32l(lw(m, g[AT], 0x2604) as u32);
        ctx.fpr[12].set_u32l(0);
        g[AT] = li(0x8012_0000);
        ctx.fpr[0].set_u32l(lw(m, g[AT], 0xBF8) as u32);
        let empty = ctx.fpr[2].fl() <= ctx.fpr[12].fl();
        g[V1] = li(0x800A_0000);
        g[AT] = li(0x800B_0000);
        g[T0] = 1;
        if !empty {
            ctx.fpr[6].set_u32l(lw(m, g[AT], -0x6214) as u32);
            ctx.fpr[8].set_fl(ctx.fpr[6].fl() * ctx.fpr[0].fl());
            ctx.fpr[2].set_fl(ctx.fpr[2].fl() - ctx.fpr[8].fl());
        } else {
            g[V1] = lh(m, g[V1], 0x2608);
            g[AT] = li(0x800A_0000);
            g[V1] = addu(g[V1], u64::MAX);
            g[T7] = sll(g[V1], 16);
            g[T8] = sra(g[T7], 16);
            sh(m, g[AT], 0x2608, g[T8]);
            if (g[T8] as i64) <= 0 {
                g[V0] = 1;
            }
            ctx.fpr[2].set_u32l(ctx.fpr[12].u32l());
        }
        // Step 3.
        g[V1] = 3;
        g[AT] = li(0x800A_0000);
        if ctx.fpr[2].fl() < ctx.fpr[12].fl() {
            ctx.fpr[2].set_u32l(ctx.fpr[12].u32l());
            sh(m, g[AT], 0x2608, g[V1]);
        }
        // Step 4. Both arms above end in the idiom's save (cfc1 t9), just
        // before its ctc1 t0 (t0 = 1); the restoring ctc1 t9 follows it
        // directly, so to_unsigned_s restores in place.
        g[A0] = (-0x67i64) as u64;
        g[A1] = 0;
        g[A2] = 0;
        g[A3] = 0;
        fpu::to_unsigned_s(g, &mut ctx.fpr, &mut fcr31, T9, T0, 10, 2);
        g[AT] = li(0x800A_0000);
        sw(m, g[AT], 0x2604, u64::from(ctx.fpr[2].u32l()));
        sw(m, g[SP], 0x10, g[T0]);
        sw(m, g[SP], 0x24, g[V0]);
        call(imports::func_8000AB24, m, ctx);
        let g = &mut ctx.gpr;
        // Step 5.
        g[V0] = lw(m, g[SP], 0x24);
        g[A0] = (-0x67i64) as u64;
        if g[V0] == 0 {
            g[V0] = 0;
        } else {
            g[A1] = 0;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x437F_0000);
            ctx.fpr[2].set_u32l(g[AT] as u32);
            g[AT] = li(0x800A_0000);
            g[T1] = 1;
            sw(m, g[AT], 0x4BD8, g[T1]);
            sw(m, g[AT], 0x4BDC, 0);
            g[V1] = 3;
            sw(m, g[AT], 0x260C, g[V1]);
            g[V0] = 1;
            sw(m, g[AT], 0x2604, u64::from(ctx.fpr[2].u32l()));
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[SP] = addu(g[SP], 0x28);
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

/// `func_80029C24()`: two scaled counts and a flag (**guess** at the
/// meaning). With `i = [0x8011A270]`, the 56-byte record `r = 0x801198A8 +
/// 56i`, the 16-byte entries `E` at `0x800A2DE0`, `e = E[(s8) r[0]]`, `v =
/// (u8) e[3]`, `e2 = E[(s8) [0x8011A050 + 56v]]`, and `K = [0x800A9E08]`:
/// `A = trunc(f32((u8) [0x80113E84 + v] * e2.word4) * K) + 1` goes to
/// `[0x800D6CC8]`, `B = trunc(f32((u8) r[1] * e.word4) * K) + 1` to
/// `[0x800D6CCC]` (the products are `multu` low words, converted as signed
/// ints; the truncations are C casts, `0x80000000` out of range), and the
/// halfword `[0x800D6CC4]` = 1 if `[0x80113E78] + A < B` (signed), else 0.
/// All indices unbounded.
///
/// Leaves `lo`/`hi` from the second `multu`, and the addresses, bytes and
/// words above in `a0`..`a3`, `v0`, `v1`, `t0`..`t9`, `f0`..`f18`.
///
/// Domain: `K` not NaN (the products are guarded); the records and entries
/// in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80029C24(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = li(0x8012_0000);
    g[T6] = lw(m, g[T6], -0x5D90);
    g[T8] = li(0x8011_98A8);
    g[T7] = sll(g[T6], 3);
    g[T7] = subu(g[T7], g[T6]);
    g[T7] = sll(g[T7], 3);
    g[V1] = addu(g[T7], g[T8]);
    g[T9] = lb(m, g[V1], 0);
    g[A1] = li(0x800A_2DE0);
    g[T1] = sll(g[T9], 4);
    g[A0] = addu(g[A1], g[T1]);
    g[V0] = lbu(m, g[A0], 3);
    g[T5] = li(0x8012_0000);
    g[A3] = li(0x8011_0000);
    g[T4] = sll(g[V0], 3);
    g[T4] = subu(g[T4], g[V0]);
    g[T4] = sll(g[T4], 3);
    g[T5] = addu(g[T5], g[T4]);
    g[T5] = lb(m, g[T5], -0x5FB0);
    g[A3] = addu(g[A3], 0x3E60);
    g[T2] = addu(g[A3], g[V0]);
    g[T6] = sll(g[T5], 4);
    g[T7] = addu(g[A1], g[T6]);
    g[T8] = lw(m, g[T7], 4);
    g[T3] = lbu(m, g[T2], 0x24);
    g[T6] = lw(m, g[A0], 4);
    g[T5] = lbu(m, g[V1], 1);
    let (lo, _) = multu(g[T3], g[T8]);
    g[AT] = li(0x800B_0000);
    f[0].set_u32l(lw(m, g[AT], -0x61F8) as u32);
    g[T1] = lw(m, g[A3], 0x18);
    g[A2] = li(0x800D_6CC8);
    g[T0] = li(0x800D_6CCC);
    g[T9] = lo;
    f[4].set_u32l(g[T9] as u32);
    let (lo, _) = multu(g[T5], g[T6]);
    f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
    g[T6] = 1;
    f[8].set_fl(f[6].fl() * f[0].fl());
    g[T7] = lo;
    f[16].set_u32l(g[T7] as u32);
    f[18].set_fl(fpu::cvt_s_w(f[16].u32l(), fpu::NEAREST));
    f[10].set_u32l(fpu::trunc_w_s(f[8].fl()));
    f[4].set_fl(f[18].fl() * f[0].fl());
    g[T2] = s32(f[10].u32l());
    g[T4] = addu(g[T2], 1);
    f[6].set_u32l(fpu::trunc_w_s(f[4].fl()));
    sw(m, g[A2], 0, g[T4]);
    g[T4] = addu(g[T1], g[T4]);
    g[T8] = s32(f[6].u32l());
    g[T9] = addu(g[T8], 1);
    g[AT] = slt(g[T4], g[T9]);
    sw(m, g[T0], 0, g[T9]);
    if g[AT] == 0 {
        // L_80029D28
        g[AT] = li(0x800D_0000);
        sh(m, g[AT], 0x6CC4, 0);
    } else {
        g[AT] = li(0x800D_0000);
        sh(m, g[AT], 0x6CC4, g[T6]);
    }
}

/// The track names [`func_8002D598`] returns, by track: strings in the data
/// segment, back to back and 4-aligned, each starting with `~~`.
pub const TRACK_NAMES: [u32; 25] = [
    0x800A_98E4, 0x800A_9904, 0x800A_991C, 0x800A_9930, 0x800A_9940, 0x800A_9958, 0x800A_9970, 0x800A_9984, 0x800A_9994,
    0x800A_99A8, 0x800A_99BC, 0x800A_99D0, 0x800A_99D8, 0x800A_99E8, 0x800A_99FC, 0x800A_9A14, 0x800A_9A20, 0x800A_9A38,
    0x800A_9A4C, 0x800A_9A60, 0x800A_9A6C, 0x800A_9A7C, 0x800A_9A8C, 0x800A_9A9C, 0x800A_9AA8,
];

/// `func_8002C780(x, y, h)` (a HUD panel with two bars, **guess**): moves
/// two levels and lays out the records 0xAF, 0xAE, 0xB2, 0xB1, 0xB4 (and
/// 0xB3, 0xB0 when enabled) around `(x, y)`. With `T = 0x8011A240`, `s` =
/// the float `[0x800A9ED8]` (838.2 in the ROM) and `dt` = `[0x80120BF8]`:
/// 1. Level 1 at `0x800A2654`: `L = L + d * dt` with `d = s` if `[T +
///    0x20] != 0`, else `s * -1.0`; `L = 254.0` if `254.0 < L`; stored;
///    then 0.0 is stored over it if `L < 0`. Level 2 at `0x800A2658` the
///    same with `[T + 0x24]`.
/// 2. For each record, [`func_8000A920`]`(id, 1)`, [`func_8000AA04`]`(id,
///    X, Y)` and [`func_8000AB24`]`(id, r, g, b, a)`, positions s16-wrapped:
///    - 0xAF at `(x - 0x1B, y - 0xB)`, colour `A3 BE 11 FE`;
///    - 0xAE at `(x - 0x14, y - 7)`, colour `32 FF FF u(L1)`;
///    - 0xB2 at `(x - 0x1B, y + h + 0xF)`, `A3 BE 11 FE`;
///    - 0xB1 at `(x - 0x14, y + h + 0x17)`, `32 FF FF u(L2)`;
///    - 0xB4 at `(x - 0x11, y + 0x12)`, `A3 BE 11 FE`, then
///      [`func_8000AAC0`]`(0xB4, 1.0, f32(h) * [0x800A9EDC])`;
///    - if `[T + 0x24] != 0` and bit `0x8000` of `[0x800A4B94]`: 0xB3 at
///      `(x - 0x20, y + h - 0x13)`, `32 FF FF FE`;
///    - if `[T + 0x20] != 0` and bit `0x4000` of `[0x800A4B94]`: 0xB0 at
///      `(x - 0x20, y - 0x11)`, `32 FF FF FE`.
///
///    `u(L)` is IDO's float → unsigned idiom ([`fpu::to_unsigned_s`]) of
///    the level re-read from memory. Its second path (from 2^31, beyond
///    254.0) is dead and kept as the C has it.
///
/// Frame (`sp - 0x38`): `ra` at `+0x1C`, the arguments spilled to their
/// slots `+0x38..+0x40`, the fifth argument at `+0x10`, and `s16(x -
/// 0x1B)` at `+0x2C` (`s16(x - 0x20)` if 0xB0 is drawn), `s16(x - 0x14)` at
/// `+0x28`, `y + h` at `+0x24`. Leaves `v0 = y + h` and the callees'
/// registers; `t7 = 0` / `t6 = 0` (the saved FCR31s), `t2`, `t3` the last
/// flag loads (`t4`/`t5`/`t6` with them), `f18 = s`, `f0 = s`, `f2` = the
/// second level as re-read, `f12 = dt`, `f14 = 254.0`, `f16 = 0.0`, `f4`,
/// `f6`, `f8`, `f10` from the last conversions and products.
///
/// Domain: `s`, `dt`, both levels and `[0x800A9EDC]` not NaN (operands of
/// the arithmetic).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002C780(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let mut fcr31 = fpu::NEAREST;
    g[V0] = li(0x8011_A240);
    g[T6] = lw(m, g[V0], 0x20);
    g[SP] = addu(g[SP], (-0x38i64) as u64);
    g[AT] = li(0x800B_0000);
    ctx.fpr[18].set_u32l(lw(m, g[AT], -0x6128) as u32);
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x38, g[A0]);
    sw(m, g[SP], 0x3C, g[A1]);
    sw(m, g[SP], 0x40, g[A2]);
    // Level 1.
    ctx.fpr[0].set_u32l(ctx.fpr[18].u32l());
    if g[T6] == 0 {
        g[AT] = li(0xBF80_0000);
        ctx.fpr[4].set_u32l(g[AT] as u32);
        ctx.fpr[0].set_fl(ctx.fpr[18].fl() * ctx.fpr[4].fl());
    }
    g[AT] = li(0x800A_0000);
    ctx.fpr[2].set_u32l(lw(m, g[AT], 0x2654) as u32);
    g[AT] = li(0x437E_0000);
    ctx.fpr[14].set_u32l(g[AT] as u32);
    g[AT] = li(0x8012_0000);
    ctx.fpr[12].set_u32l(lw(m, g[AT], 0xBF8) as u32);
    ctx.fpr[16].set_u32l(0);
    g[A0] = 0xAF;
    ctx.fpr[6].set_fl(ctx.fpr[0].fl() * ctx.fpr[12].fl());
    ctx.fpr[0].set_u32l(ctx.fpr[18].u32l());
    ctx.fpr[2].set_fl(ctx.fpr[2].fl() + ctx.fpr[6].fl());
    if ctx.fpr[14].fl() < ctx.fpr[2].fl() {
        ctx.fpr[2].set_u32l(ctx.fpr[14].u32l());
    }
    let below = ctx.fpr[2].fl() < ctx.fpr[16].fl();
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x2654, u64::from(ctx.fpr[2].u32l()));
    if below {
        sw(m, g[AT], 0x2654, u64::from(ctx.fpr[16].u32l()));
    }
    // Level 2.
    g[T7] = lw(m, g[V0], 0x24);
    g[AT] = li(0xBF80_0000);
    if g[T7] == 0 {
        ctx.fpr[8].set_u32l(g[AT] as u32);
        ctx.fpr[0].set_fl(ctx.fpr[18].fl() * ctx.fpr[8].fl());
    }
    ctx.fpr[10].set_fl(ctx.fpr[0].fl() * ctx.fpr[12].fl());
    g[AT] = li(0x800A_0000);
    ctx.fpr[2].set_u32l(lw(m, g[AT], 0x2658) as u32);
    ctx.fpr[2].set_fl(ctx.fpr[2].fl() + ctx.fpr[10].fl());
    if ctx.fpr[14].fl() < ctx.fpr[2].fl() {
        ctx.fpr[2].set_u32l(ctx.fpr[14].u32l());
    }
    let below = ctx.fpr[2].fl() < ctx.fpr[16].fl();
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x2658, u64::from(ctx.fpr[2].u32l()));
    if below {
        sw(m, g[AT], 0x2658, u64::from(ctx.fpr[16].u32l()));
    }
    // 0xAF.
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A1] = lw(m, g[SP], 0x38);
    g[A2] = lw(m, g[SP], 0x3C);
    g[A0] = 0xAF;
    g[A1] = addu(g[A1], (-0x1Bi64) as u64);
    g[T8] = sll(g[A1], 16);
    g[A2] = addu(g[A2], (-0xBi64) as u64);
    g[T0] = sll(g[A2], 16);
    g[A1] = sra(g[T8], 16);
    sw(m, g[SP], 0x2C, g[A1]);
    g[A2] = sra(g[T0], 16);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[T2] = 0xFE;
    sw(m, g[SP], 0x10, g[T2]);
    g[A0] = 0xAF;
    g[A1] = 0xA3;
    g[A2] = 0xBE;
    g[A3] = 0x11;
    call(imports::func_8000AB24, m, ctx);
    // 0xAE, alpha u(level 1). No conversion or arithmetic sits between the
    // idiom and its restoring ctc1 t7, so to_unsigned_s restores in place.
    let g = &mut ctx.gpr;
    g[A0] = 0xAE;
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A1] = lw(m, g[SP], 0x38);
    g[A2] = lw(m, g[SP], 0x3C);
    g[A0] = 0xAE;
    g[A1] = addu(g[A1], (-0x14i64) as u64);
    g[T3] = sll(g[A1], 16);
    g[A2] = addu(g[A2], (-7i64) as u64);
    g[T5] = sll(g[A2], 16);
    g[A1] = sra(g[T3], 16);
    sw(m, g[SP], 0x28, g[A1]);
    g[A2] = sra(g[T5], 16);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    g[T8] = 1;
    ctx.fpr[2].set_u32l(lw(m, g[AT], 0x2654) as u32);
    g[A0] = 0xAE;
    g[A1] = 0x32;
    g[A2] = 0xFF;
    g[A3] = 0xFF;
    fpu::to_unsigned_s(g, &mut ctx.fpr, &mut fcr31, T7, T8, 4, 2);
    sw(m, g[SP], 0x10, g[T8]);
    call(imports::func_8000AB24, m, ctx);
    // 0xB2.
    let g = &mut ctx.gpr;
    g[A0] = 0xB2;
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[T9] = lw(m, g[SP], 0x3C);
    g[T0] = lw(m, g[SP], 0x40);
    g[A0] = 0xB2;
    g[A1] = lh(m, g[SP], 0x2E);
    g[V0] = addu(g[T9], g[T0]);
    g[A2] = addu(g[V0], 0xF);
    g[T1] = sll(g[A2], 16);
    g[A2] = sra(g[T1], 16);
    sw(m, g[SP], 0x24, g[V0]);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[T3] = 0xFE;
    sw(m, g[SP], 0x10, g[T3]);
    g[A0] = 0xB2;
    g[A1] = 0xA3;
    g[A2] = 0xBE;
    g[A3] = 0x11;
    call(imports::func_8000AB24, m, ctx);
    // 0xB1, alpha u(level 2); restored in place as for 0xAE.
    let g = &mut ctx.gpr;
    g[A0] = 0xB1;
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A2] = lw(m, g[SP], 0x24);
    g[A0] = 0xB1;
    g[A1] = lh(m, g[SP], 0x2A);
    g[A2] = addu(g[A2], 0x17);
    g[T4] = sll(g[A2], 16);
    g[A2] = sra(g[T4], 16);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    g[T7] = 1;
    ctx.fpr[2].set_u32l(lw(m, g[AT], 0x2658) as u32);
    g[A0] = 0xB1;
    g[A1] = 0x32;
    g[A2] = 0xFF;
    g[A3] = 0xFF;
    fpu::to_unsigned_s(g, &mut ctx.fpr, &mut fcr31, T6, T7, 6, 2);
    sw(m, g[SP], 0x10, g[T7]);
    call(imports::func_8000AB24, m, ctx);
    // 0xB4, and its scale.
    let g = &mut ctx.gpr;
    g[A0] = 0xB4;
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A1] = lw(m, g[SP], 0x38);
    g[A2] = lw(m, g[SP], 0x3C);
    g[A0] = 0xB4;
    g[A1] = addu(g[A1], (-0x11i64) as u64);
    g[A2] = addu(g[A2], 0x12);
    g[T0] = sll(g[A2], 16);
    g[T8] = sll(g[A1], 16);
    g[A1] = sra(g[T8], 16);
    g[A2] = sra(g[T0], 16);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[T2] = 0xFE;
    sw(m, g[SP], 0x10, g[T2]);
    g[A0] = 0xB4;
    g[A1] = 0xA3;
    g[A2] = 0xBE;
    g[A3] = 0x11;
    call(imports::func_8000AB24, m, ctx);
    let g = &mut ctx.gpr;
    g[T3] = lw(m, g[SP], 0x40);
    g[AT] = li(0x800B_0000);
    ctx.fpr[4].set_u32l(lw(m, g[AT], -0x6124) as u32);
    ctx.fpr[8].set_u32l(g[T3] as u32);
    g[A0] = 0xB4;
    g[A1] = li(0x3F80_0000);
    ctx.fpr[10].set_fl(fpu::cvt_s_w(ctx.fpr[8].u32l(), fcr31));
    ctx.fpr[6].set_fl(ctx.fpr[10].fl() * ctx.fpr[4].fl());
    g[A2] = s32(ctx.fpr[6].u32l());
    call(imports::func_8000AAC0, m, ctx);
    // 0xB3 if enabled.
    let g = &mut ctx.gpr;
    g[T4] = li(0x8012_0000);
    g[T4] = lw(m, g[T4], -0x5D9C);
    g[T5] = li(0x800A_0000);
    if g[T4] != 0 {
        g[T5] = lw(m, g[T5], 0x4B94);
        g[A0] = 0xB3;
        g[T6] = g[T5] & 0x8000;
        if g[T6] != 0 {
            g[A1] = 1;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[SP], 0x38);
            g[A2] = lw(m, g[SP], 0x24);
            g[A0] = 0xB3;
            g[A1] = addu(g[A1], (-0x20i64) as u64);
            g[A2] = addu(g[A2], (-0x13i64) as u64);
            g[T9] = sll(g[A2], 16);
            g[T7] = sll(g[A1], 16);
            g[A1] = sra(g[T7], 16);
            g[A2] = sra(g[T9], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[T1] = 0xFE;
            sw(m, g[SP], 0x10, g[T1]);
            g[A0] = 0xB3;
            g[A1] = 0x32;
            g[A2] = 0xFF;
            g[A3] = 0xFF;
            call(imports::func_8000AB24, m, ctx);
        }
    }
    // 0xB0 if enabled.
    let g = &mut ctx.gpr;
    g[T2] = li(0x8012_0000);
    g[T2] = lw(m, g[T2], -0x5DA0);
    g[T3] = li(0x800A_0000);
    if g[T2] != 0 {
        g[T3] = lw(m, g[T3], 0x4B94);
        g[T5] = lw(m, g[SP], 0x38);
        g[T4] = g[T3] & 0x4000;
        g[T6] = addu(g[T5], (-0x20i64) as u64);
        if g[T4] != 0 {
            g[T7] = sll(g[T6], 16);
            g[T8] = sra(g[T7], 16);
            sw(m, g[SP], 0x2C, g[T8]);
            g[A0] = 0xB0;
            g[A1] = 1;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[A2] = lw(m, g[SP], 0x3C);
            g[A0] = 0xB0;
            g[A1] = lh(m, g[SP], 0x2E);
            g[A2] = addu(g[A2], (-0x11i64) as u64);
            g[T9] = sll(g[A2], 16);
            g[A2] = sra(g[T9], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[T1] = 0xFE;
            sw(m, g[SP], 0x10, g[T1]);
            g[A0] = 0xB0;
            g[A1] = 0x32;
            g[A2] = 0xFF;
            g[A3] = 0xFF;
            call(imports::func_8000AB24, m, ctx);
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[SP] = addu(g[SP], 0x38);
}

/// `func_8002CC28(x, y, h)` (a second HUD panel, **guess**; `x` and `h`
/// are spilled but unused): like [`func_8002C780`] with its own levels and
/// records, at fixed horizontal positions. With `T = 0x8011A240`, `s` =
/// the float `[0x800A9EE0]` (838.2 in the ROM) and `dt` = `[0x80120BF8]`:
/// 1. Level 3 at `0x800A265C`: `L = L + d * dt` with `d = s` if `[T +
///    0x18] != 0`, else `s * -1.0`; `L = 254.0` if `254.0 < L`; stored;
///    then 0.0 is stored over it if `L < 0`. Level 4 at `0x800A2660` the
///    same with `[T + 0x1C]`.
/// 2. For each record, [`func_8000A920`]`(id, 1)`, [`func_8000AA04`]`(id,
///    X, Y)` and [`func_8000AB24`]`(id, r, g, b, a)`, `Y` s16-wrapped:
///    - 0xAB at `(0x13, y - 0xE)`, colour `A3 BE 11 FE`;
///    - 0xAA at `(0x16, y - 7)`, colour `32 FF FF u(L3)`;
///    - 0xA8 at `(0x109, y - 0xE)`, `A3 BE 11 FE`;
///    - 0xA7 at `(0x110, y - 7)`, `32 FF FF u(L4)`;
///    - 0xAD at `(0x30, y - 4)`, `A3 BE 11 FE`, then
///      [`func_8000AAC0`]`(0xAD, 221.0 * [0x800A9EE4], 1.0)`;
///    - if `[T + 0x14] != 0`: 0xA9 at `(0xE6, y - 0x13)`, `32 FF FF FE`;
///    - if `[T + 0x10] != 0`: 0xAC at `(0xC, y - 0x13)`, `32 FF FF FE`.
///
///    `u(L)` is IDO's float → unsigned idiom ([`fpu::to_unsigned_s`]) of
///    the level re-read from memory. Its second path (from 2^31, beyond
///    254.0) is dead and kept as the C has it.
///
/// Frame (`sp - 0x30`): `ra` at `+0x1C`, the arguments spilled to their
/// slots `+0x30..+0x38`, the fifth argument at `+0x10`, `s16(y - 0xE)` at
/// `+0x24` (`s16(y - 0x13)` if 0xAC is drawn) and `s16(y - 7)` at `+0x20`.
/// Leaves the callees' registers; `t3 = 0` / `t6` (the saved FCR31s, `t6`
/// then the last flag), `t7 = y` (sign-extended), `t8 = y - 0x13`, `t2`
/// the other flag, `f18 = s`, `f0 = s`, `f2` = level 4 as re-read, `f12 =
/// dt`, `f14 = 254.0`, `f16 = 0.0`, `f4`, `f6`, `f8`, `f10` from the last
/// conversions and products.
///
/// Domain: `s`, `dt`, both levels and `[0x800A9EE4]` not NaN (operands of
/// the arithmetic).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002CC28(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let mut fcr31 = fpu::NEAREST;
    g[V0] = li(0x8011_A240);
    g[T6] = lw(m, g[V0], 0x18);
    g[SP] = addu(g[SP], (-0x30i64) as u64);
    g[AT] = li(0x800B_0000);
    ctx.fpr[18].set_u32l(lw(m, g[AT], -0x6120) as u32);
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x30, g[A0]);
    sw(m, g[SP], 0x34, g[A1]);
    sw(m, g[SP], 0x38, g[A2]);
    // Level 3.
    ctx.fpr[0].set_u32l(ctx.fpr[18].u32l());
    if g[T6] == 0 {
        g[AT] = li(0xBF80_0000);
        ctx.fpr[4].set_u32l(g[AT] as u32);
        ctx.fpr[0].set_fl(ctx.fpr[18].fl() * ctx.fpr[4].fl());
    }
    g[AT] = li(0x800A_0000);
    ctx.fpr[2].set_u32l(lw(m, g[AT], 0x265C) as u32);
    g[AT] = li(0x437E_0000);
    ctx.fpr[14].set_u32l(g[AT] as u32);
    g[AT] = li(0x8012_0000);
    ctx.fpr[12].set_u32l(lw(m, g[AT], 0xBF8) as u32);
    ctx.fpr[16].set_u32l(0);
    g[A0] = 0xAB;
    ctx.fpr[6].set_fl(ctx.fpr[0].fl() * ctx.fpr[12].fl());
    ctx.fpr[0].set_u32l(ctx.fpr[18].u32l());
    ctx.fpr[2].set_fl(ctx.fpr[2].fl() + ctx.fpr[6].fl());
    if ctx.fpr[14].fl() < ctx.fpr[2].fl() {
        ctx.fpr[2].set_u32l(ctx.fpr[14].u32l());
    }
    let below = ctx.fpr[2].fl() < ctx.fpr[16].fl();
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x265C, u64::from(ctx.fpr[2].u32l()));
    if below {
        sw(m, g[AT], 0x265C, u64::from(ctx.fpr[16].u32l()));
    }
    // Level 4.
    g[T7] = lw(m, g[V0], 0x1C);
    g[AT] = li(0xBF80_0000);
    if g[T7] == 0 {
        ctx.fpr[8].set_u32l(g[AT] as u32);
        ctx.fpr[0].set_fl(ctx.fpr[18].fl() * ctx.fpr[8].fl());
    }
    ctx.fpr[10].set_fl(ctx.fpr[0].fl() * ctx.fpr[12].fl());
    g[AT] = li(0x800A_0000);
    ctx.fpr[2].set_u32l(lw(m, g[AT], 0x2660) as u32);
    ctx.fpr[2].set_fl(ctx.fpr[2].fl() + ctx.fpr[10].fl());
    if ctx.fpr[14].fl() < ctx.fpr[2].fl() {
        ctx.fpr[2].set_u32l(ctx.fpr[14].u32l());
    }
    let below = ctx.fpr[2].fl() < ctx.fpr[16].fl();
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x2660, u64::from(ctx.fpr[2].u32l()));
    if below {
        sw(m, g[AT], 0x2660, u64::from(ctx.fpr[16].u32l()));
    }
    // 0xAB.
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A2] = lw(m, g[SP], 0x34);
    g[A0] = 0xAB;
    g[A1] = 0x13;
    g[A2] = addu(g[A2], (-0xEi64) as u64);
    g[T8] = sll(g[A2], 16);
    g[A2] = sra(g[T8], 16);
    sw(m, g[SP], 0x24, g[A2]);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[T0] = 0xFE;
    sw(m, g[SP], 0x10, g[T0]);
    g[A0] = 0xAB;
    g[A1] = 0xA3;
    g[A2] = 0xBE;
    g[A3] = 0x11;
    call(imports::func_8000AB24, m, ctx);
    // 0xAA, alpha u(level 3). No conversion or arithmetic sits between the
    // idiom and its restoring ctc1 t3, so to_unsigned_s restores in place.
    let g = &mut ctx.gpr;
    g[A0] = 0xAA;
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A2] = lw(m, g[SP], 0x34);
    g[A0] = 0xAA;
    g[A1] = 0x16;
    g[A2] = addu(g[A2], (-7i64) as u64);
    g[T1] = sll(g[A2], 16);
    g[A2] = sra(g[T1], 16);
    sw(m, g[SP], 0x20, g[A2]);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    g[T4] = 1;
    ctx.fpr[2].set_u32l(lw(m, g[AT], 0x265C) as u32);
    g[A0] = 0xAA;
    g[A1] = 0x32;
    g[A2] = 0xFF;
    g[A3] = 0xFF;
    fpu::to_unsigned_s(g, &mut ctx.fpr, &mut fcr31, T3, T4, 4, 2);
    sw(m, g[SP], 0x10, g[T4]);
    call(imports::func_8000AB24, m, ctx);
    // 0xA8.
    let g = &mut ctx.gpr;
    g[A0] = 0xA8;
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = 0xA8;
    g[A1] = 0x109;
    g[A2] = lh(m, g[SP], 0x26);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[T5] = 0xFE;
    sw(m, g[SP], 0x10, g[T5]);
    g[A0] = 0xA8;
    g[A1] = 0xA3;
    g[A2] = 0xBE;
    g[A3] = 0x11;
    call(imports::func_8000AB24, m, ctx);
    // 0xA7, alpha u(level 4); restored in place as for 0xAA.
    let g = &mut ctx.gpr;
    g[A0] = 0xA7;
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = 0xA7;
    g[A1] = 0x110;
    g[A2] = lh(m, g[SP], 0x22);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    g[T7] = 1;
    ctx.fpr[2].set_u32l(lw(m, g[AT], 0x2660) as u32);
    g[A0] = 0xA7;
    g[A1] = 0x32;
    g[A2] = 0xFF;
    g[A3] = 0xFF;
    fpu::to_unsigned_s(g, &mut ctx.fpr, &mut fcr31, T6, T7, 6, 2);
    sw(m, g[SP], 0x10, g[T7]);
    call(imports::func_8000AB24, m, ctx);
    // 0xAD, and its scale.
    let g = &mut ctx.gpr;
    g[A0] = 0xAD;
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A2] = lw(m, g[SP], 0x34);
    g[A0] = 0xAD;
    g[A1] = 0x30;
    g[A2] = addu(g[A2], (-4i64) as u64);
    g[T8] = sll(g[A2], 16);
    g[A2] = sra(g[T8], 16);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[T0] = 0xFE;
    sw(m, g[SP], 0x10, g[T0]);
    g[A0] = 0xAD;
    g[A1] = 0xA3;
    g[A2] = 0xBE;
    g[A3] = 0x11;
    call(imports::func_8000AB24, m, ctx);
    let g = &mut ctx.gpr;
    g[T1] = 0xDD;
    ctx.fpr[8].set_u32l(g[T1] as u32);
    g[AT] = li(0x800B_0000);
    ctx.fpr[4].set_u32l(lw(m, g[AT], -0x611C) as u32);
    ctx.fpr[10].set_fl(fpu::cvt_s_w(ctx.fpr[8].u32l(), fcr31));
    g[A0] = 0xAD;
    g[A2] = li(0x3F80_0000);
    ctx.fpr[6].set_fl(ctx.fpr[10].fl() * ctx.fpr[4].fl());
    g[A1] = s32(ctx.fpr[6].u32l());
    call(imports::func_8000AAC0, m, ctx);
    // 0xA9 if enabled.
    let g = &mut ctx.gpr;
    g[T2] = li(0x8012_0000);
    g[T2] = lw(m, g[T2], -0x5DAC);
    g[A0] = 0xA9;
    if g[T2] != 0 {
        g[A1] = 1;
        call(imports::func_8000A920, m, ctx);
        let g = &mut ctx.gpr;
        g[A2] = lw(m, g[SP], 0x34);
        g[A0] = 0xA9;
        g[A1] = 0xE6;
        g[A2] = addu(g[A2], (-0x13i64) as u64);
        g[T3] = sll(g[A2], 16);
        g[A2] = sra(g[T3], 16);
        call(imports::func_8000AA04, m, ctx);
        let g = &mut ctx.gpr;
        g[T5] = 0xFE;
        sw(m, g[SP], 0x10, g[T5]);
        g[A0] = 0xA9;
        g[A1] = 0x32;
        g[A2] = 0xFF;
        g[A3] = 0xFF;
        call(imports::func_8000AB24, m, ctx);
    }
    // 0xAC if enabled.
    let g = &mut ctx.gpr;
    g[T6] = li(0x8012_0000);
    g[T6] = lw(m, g[T6], -0x5DB0);
    g[T7] = lw(m, g[SP], 0x34);
    g[T8] = addu(g[T7], (-0x13i64) as u64);
    if g[T6] != 0 {
        g[T9] = sll(g[T8], 16);
        g[T0] = sra(g[T9], 16);
        sw(m, g[SP], 0x24, g[T0]);
        g[A0] = 0xAC;
        g[A1] = 1;
        call(imports::func_8000A920, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = 0xAC;
        g[A1] = 0xC;
        g[A2] = lh(m, g[SP], 0x26);
        call(imports::func_8000AA04, m, ctx);
        let g = &mut ctx.gpr;
        g[T1] = 0xFE;
        sw(m, g[SP], 0x10, g[T1]);
        g[A0] = 0xAC;
        g[A1] = 0x32;
        g[A2] = 0xFF;
        g[A3] = 0xFF;
        call(imports::func_8000AB24, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[SP] = addu(g[SP], 0x30);
}

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

/// `func_8002DA0C(i, j)`: with `k = (s8) i`, `c = (u8) j`, `r` = the s16
/// at `0x80113E6C + 2k` shifted right (arithmetic) by `2c` (mod 32), and
/// `q = r % 4` (C remainder: negative for negative `r`): 1 if
/// [`func_8002D9D0`]`(k, c)` is 3 and `q == 0`; otherwise, for `k < 3`,
/// whether bit `c + 1` (mod 32) of the byte at `0x80113E68 + k` is clear
/// (1 or 0); otherwise 0.
///
/// Frame (`sp - 0x28`): `ra` at `+0x14`, `c` at `+0x1C`, `q`'s halfword at
/// `+0x26`; `i` and `j` spilled to their slots `+0x28`/`+0x2C`, and the
/// byte `k` at `+0x2B` (the low byte `i`'s spill already holds). Calls the
/// callee with `a0 = k`, `a1 = a2 = c`. Leaves `a0 = k`, `a1 = a2 = c`,
/// `at = 3`, `v0 = v1` = the result, `t0` = the s16, `t1 = 2c`, `t2 = r`,
/// `t3 = q`, `t9 = 2k` and the callee's `t6`..`t8`; the bit test leaves
/// `t5` = the byte, `t6 = c + 1`, `t7 = 1`, `t8` the mask, `t9`, and `t5 =
/// 0x80110000` if `k >= 3`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002DA0C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = sll(g[A0], 24);
    g[T7] = sra(g[T6], 24);
    g[T9] = sll(g[T7], 1);
    g[T0] = li(0x8011_0000);
    g[T0] = addu(g[T0], g[T9]);
    g[T0] = lh(m, g[T0], 0x3E6C);
    g[T8] = g[A1] & 0xFF;
    g[SP] = addu(g[SP], (-0x28i64) as u64);
    g[T1] = sll(g[T8], 1);
    g[T2] = srav(g[T0], g[T1]);
    sw(m, g[SP], 0x28, g[A0]);
    g[A0] = g[T7];
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x2C, g[A1]);
    g[A2] = g[T8];
    g[A1] = g[T8];
    g[T3] = g[T2] & 3;
    if (g[T2] as i64) < 0 && g[T3] != 0 {
        g[T3] = addu(g[T3], (-4i64) as u64);
    }
    'b_8002DAC0: {
        sh(m, g[SP], 0x26, g[T3]);
        sw(m, g[SP], 0x1C, g[A2]);
        sb(m, g[SP], 0x2B, g[A0]);
        call(imports::func_8002D9D0, m, ctx);
        let g = &mut ctx.gpr;
        g[AT] = 3;
        g[A0] = lb(m, g[SP], 0x2B);
        g[A2] = lw(m, g[SP], 0x1C);
        if g[V0] == g[AT] {
            g[V0] = lh(m, g[SP], 0x26);
            g[V1] = sltu(g[V0], 1);
            if g[V1] != 0 {
                g[RA] = lw(m, g[SP], 0x14);
                break 'b_8002DAC0;
            }
        }
        g[V1] = slt(g[A0], 3);
        g[T5] = li(0x8011_0000);
        if g[V1] != 0 {
            g[T5] = addu(g[T5], g[A0]);
            g[T5] = lbu(m, g[T5], 0x3E68);
            g[T6] = addu(g[A2], 1);
            g[T7] = 1;
            g[T8] = sllv(g[T7], g[T6]);
            g[V1] = g[T5] & g[T8];
            g[T9] = sltu(0, g[V1]);
            g[V1] = sltu(g[T9], 1);
        }
        g[RA] = lw(m, g[SP], 0x14);
    }
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], 0x28);
    g[V0] = g[V1];
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

/// `func_8002DB20(p, n)`: the index `i` of the `n`-th (from 0) entry whose
/// bit is set, testing `i = 0, 1, ...` with
/// [`func_8002DAD0`]`(p, k, i & 0xFF)` while `i` is below the count, the
/// byte at `0x800A21B4 + k` (`slt`: signed against an unsigned byte), with
/// `k = (s8) [p + 0x5E]`; -1 if there is no such entry. `k` and the count
/// are re-read after each call, so a callee changing them is followed.
/// The match counter starts at -1 and is compared after each increment, so
/// `n < 0` never matches (it would need 2^32 calls).
///
/// `s0`..`s4` and `ra` are saved in a 0x30-byte frame (`+0x18`..`+0x2C`)
/// and come back sign-extended from their low words. Leaves `a3` = the
/// last `k`, the last call's arguments in `a0`..`a2` and its registers,
/// and after a loop that ran to its end `t9` = the count's address, `t0`
/// the count and `at = 0`. With a zero count, no call: `t6`, `t7` = the
/// count's address and byte.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002DB20(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8002DBB8: {
        g[SP] = addu(g[SP], (-0x30i64) as u64);
        sw(m, g[SP], 0x2C, g[RA]);
        sw(m, g[SP], 0x28, g[S4]);
        sw(m, g[SP], 0x24, g[S3]);
        sw(m, g[SP], 0x20, g[S2]);
        sw(m, g[SP], 0x1C, g[S1]);
        sw(m, g[SP], 0x18, g[S0]);
        g[A3] = lb(m, g[A0], 0x5E);
        g[S4] = li(0x800A_21B4);
        g[T6] = addu(g[S4], g[A3]);
        g[T7] = lbu(m, g[T6], 0);
        g[S2] = g[A0];
        g[S3] = g[A1];
        g[S1] = u64::MAX;
        g[S0] = 0;
        if (g[T7] as i64) > 0 {
            g[A1] = sll(g[A3], 24);
            loop {
                let g = &mut ctx.gpr;
                g[T8] = sra(g[A1], 24);
                g[A1] = g[T8];
                g[A0] = g[S2];
                g[A2] = g[S0] & 0xFF;
                call(imports::func_8002DAD0, m, ctx);
                let g = &mut ctx.gpr;
                if g[V0] == 0 {
                    g[A3] = lb(m, g[S2], 0x5E);
                } else {
                    g[S1] = addu(g[S1], 1);
                    if g[S1] == g[S3] {
                        g[V0] = g[S0];
                        break 'b_8002DBB8;
                    }
                    g[A3] = lb(m, g[S2], 0x5E);
                }
                g[S0] = addu(g[S0], 1);
                g[T9] = addu(g[S4], g[A3]);
                g[T0] = lbu(m, g[T9], 0);
                g[AT] = slt(g[S0], g[T0]);
                if g[AT] == 0 {
                    break;
                }
                g[A1] = sll(g[A3], 24);
            }
        }
        let g = &mut ctx.gpr;
        g[V0] = u64::MAX;
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x2C);
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[S3] = lw(m, g[SP], 0x24);
    g[S4] = lw(m, g[SP], 0x28);
    g[SP] = addu(g[SP], 0x30);
}

/// `func_8002DBD8(o)`: with `e = 0x800A4C00 + 32 * [o + 0x34]` (the word
/// re-read from `o` before each test), 0 if the vec3 at `e + 0xC` equals
/// the one at `0x80118E50`, the vec3 at `e + 0xC` equals the one at
/// `0x80118ED0`, and the vec3 at `e` equals the one at `0x80118E10`
/// ([`func_800152CC`], IEEE equality); else 1, stopping at the first test
/// that fails.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`, `o` spilled to its slot `+0x18`.
/// Leaves `a0 = v0` = the result, `a1` = the last vec3 compared against,
/// the callee's registers, and the address computation of the last test in
/// `t7`/`t8`/`t9`/`t0`, `t1`/`t2`/`t3`/`t4`/`t5` or `t6`/`t7`/`t8`/`t9`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002DBD8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x18, g[A0]);
    g[T7] = lw(m, g[A0], 0x34);
    g[T0] = li(0x800A_4C00);
    g[T8] = sll(g[T7], 5);
    g[T9] = addu(g[T8], 0xC);
    g[A1] = li(0x8011_8E50);
    g[A0] = addu(g[T9], g[T0]);
    call(imports::func_800152CC, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = sltu(g[V0], 1);
    g[T1] = lw(m, g[SP], 0x18);
    if g[A0] == 0 {
        g[T2] = lw(m, g[T1], 0x34);
        g[T5] = li(0x800A_4C00);
        g[T3] = sll(g[T2], 5);
        g[T4] = addu(g[T3], 0xC);
        g[A1] = li(0x8011_8ED0);
        g[A0] = addu(g[T4], g[T5]);
        call(imports::func_800152CC, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = sltu(g[V0], 1);
        g[T6] = lw(m, g[SP], 0x18);
        if g[A0] == 0 {
            g[T7] = lw(m, g[T6], 0x34);
            g[T9] = li(0x800A_4C00);
            g[A1] = li(0x8012_0000);
            g[T8] = sll(g[T7], 5);
            g[A0] = addu(g[T8], g[T9]);
            g[A1] = addu(g[A1], (-0x71F0i64) as u64);
            call(imports::func_800152CC, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = sltu(g[V0], 1);
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
    g[V0] = g[A0];
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

/// `func_8002DCF4()`: 1 if [`func_8002D968`] holds for all four pairs
/// `(0x80113694, 0x800A9ABC)`, `(0x801136C0, 0x800A9AC0)`, `(0x801136EC,
/// 0x800A9AC4)`, `(0x80113718, 0x800A9AC8)`, tested in that order and
/// stopping at the first that fails; else 0.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `a1` = the last pair's
/// second address, `a0` the first (or `0x80110000` after a failed test
/// other than the last) and the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002DCF4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8002DD6C: {
        g[SP] = addu(g[SP], (-0x18i64) as u64);
        sw(m, g[SP], 0x14, g[RA]);
        g[A0] = li(0x8011_0000);
        g[A1] = li(0x800A_9ABC);
        g[A0] = addu(g[A0], 0x3694);
        call(imports::func_8002D968, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = li(0x8011_0000);
        if g[V0] != 0 {
            g[A1] = li(0x800A_9AC0);
            g[A0] = addu(g[A0], 0x36C0);
            call(imports::func_8002D968, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = li(0x8011_0000);
            if g[V0] != 0 {
                g[A1] = li(0x800A_9AC4);
                g[A0] = addu(g[A0], 0x36EC);
                call(imports::func_8002D968, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = li(0x8011_0000);
                if g[V0] != 0 {
                    g[A1] = li(0x800A_9AC8);
                    g[A0] = addu(g[A0], 0x3718);
                    call(imports::func_8002D968, m, ctx);
                    let g = &mut ctx.gpr;
                    if g[V0] == 0 {
                        g[V0] = 0;
                        break 'b_8002DD6C;
                    }
                    g[V0] = 1;
                    break 'b_8002DD6C;
                }
            }
        }
        let g = &mut ctx.gpr;
        g[V0] = 0;
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
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

/// `func_8002F000()`: `[0x800A26F4] = 0` (the word [`func_8002F054`]
/// returns), `[0x800A26F8] = 1`, and the float `[0x800D7740] = 0.0`.
/// Leaves `t6 = 1`, `f4 = 0.0`, `at = 0x800D0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F000(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x26F4, 0);
    g[AT] = li(0x800A_0000);
    g[T6] = 1;
    f[4].set_u32l(0 as u32);
    sw(m, g[AT], 0x26F8, g[T6]);
    g[AT] = li(0x800D_0000);
    sw(m, g[AT], 0x7740, u64::from(f[4].u32l()));
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

/// `func_8002F060()`: `f0 = [0x800D7740]` (the float [`func_8002F000`]
/// zeroes). Leaves `at = 0x800D0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F060(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x800D_0000);
    f[0].set_u32l(lw(m, g[AT], 0x7740) as u32);
}

/// `func_8002F0EC()` (pause broadcast, **guess** from the tag): if the word
/// `[0x800A26F8]` is 0, sets `[0x800A26F4] = 0` and sends the message
/// `{ "Paws" (0x50617773), -1, `[`func_8000DA6C`]`() }`, three words in
/// its frame, to every pool ([`func_8003FA24`]`("All!", &msg)`).
///
/// Frame (`sp - 0x28`): `ra` at `+0x14`, the message at `+0x18..+0x24`.
/// Leaves `t6` = the word, `at = 0x800A0000`, and when sending, `t7`/`t8`
/// = the first two message words, `a0 = "All!"`, `a1` = the message and
/// the callees' registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F0EC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = li(0x800A_0000);
    g[T6] = lw(m, g[T6], 0x26F8);
    g[SP] = addu(g[SP], (-0x28i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    g[AT] = li(0x800A_0000);
    if g[T6] == 0 {
        g[T7] = li(0x5061_7773);
        g[T8] = u64::MAX;
        sw(m, g[AT], 0x26F4, 0);
        sw(m, g[SP], 0x18, g[T7]);
        sw(m, g[SP], 0x1C, g[T8]);
        call(imports::func_8000DA6C, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = li(0x416C_0000);
        sw(m, g[SP], 0x20, g[V0]);
        g[A0] = g[A0] | 0x6C21;
        g[A1] = addu(g[SP], 0x18);
        call(imports::func_8003FA24, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x28);
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

/// `func_8002F9C0`: a bare epilogue like [`func_8002F740`]: `ra = [sp +
/// 0x34]`, `f20`/`f22` = the doubles at `sp + 0x18`/`0x20`, `s0`..`s2` =
/// `[sp + 0x28..0x34)` (words sign-extended), `sp += 0x38`, return. Nothing
/// calls it (**guess**: the tail of a function that doesn't return).
///
/// Domain: canonical `sp` with the frame in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002F9C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[RA] = lw(m, g[SP], 0x34);
    f[20].u64 = ld(m, g[SP], 0x18);
    f[22].u64 = ld(m, g[SP], 0x20);
    g[S0] = lw(m, g[SP], 0x28);
    g[S1] = lw(m, g[SP], 0x2C);
    g[S2] = lw(m, g[SP], 0x30);
    g[SP] = addu(g[SP], 0x38);
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

/// `func_8002FDF8()` (a reset, **guess**): clears the block at
/// `0x800D6960`: the word `+0x18 = 0`, the halfword `+0x20 = 2`, then the
/// six floats `+0..+0x14 = 0.0`; sets colour -103 to `(0, 0, 0, 255)`
/// ([`func_8000AB24`]) and calls [`func_80038DF8`]`(0x3E4, 0x3E8, 0xFF,
/// 0xFF, 0xFF, 0xFF)`, the last two on the stack.
///
/// Frame (`sp - 0x20`): `ra` at `+0x1C`, the stack arguments at
/// `+0x10`/`+0x14`. Leaves `f0 = 0.0`, `t6 = 2`, `t7 = t8 = t9 = 0xFF` and
/// the callees' registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002FDF8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(0x800D_0000);
    ctx.fpr[0].set_u32l(0);
    g[V0] = addu(g[V0], 0x6960);
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    g[T6] = 2;
    g[T7] = 0xFF;
    sw(m, g[V0], 0x18, 0);
    sh(m, g[V0], 0x20, g[T6]);
    sw(m, g[SP], 0x10, g[T7]);
    g[A0] = (-0x67i64) as u64;
    g[A1] = 0;
    g[A2] = 0;
    g[A3] = 0;
    sw(m, g[V0], 0, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 4, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 8, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0xC, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x10, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 0x14, u64::from(ctx.fpr[0].u32l()));
    call(imports::func_8000AB24, m, ctx);
    let g = &mut ctx.gpr;
    g[T8] = 0xFF;
    g[T9] = 0xFF;
    sw(m, g[SP], 0x14, g[T9]);
    sw(m, g[SP], 0x10, g[T8]);
    g[A0] = 0x3E4;
    g[A1] = 0x3E8;
    g[A2] = 0xFF;
    g[A3] = 0xFF;
    call(imports::func_80038DF8, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[SP] = addu(g[SP], 0x20);
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

/// `func_80030FA0()`: colour -103 = `(0, 0, 0, 255)` ([`func_8000AB24`]),
/// then [`func_80038DF8`]`(0x3E4, 0x3E8, 0xFF, 0xFF, 0xFF, 0xFF)`, the
/// last two on the stack: the second half of
/// [`func_8002FDF8`](crate::misc::func_8002FDF8).
///
/// Frame (`sp - 0x20`): `ra` at `+0x1C`, the stack arguments at
/// `+0x10`/`+0x14`. Leaves `t6 = t7 = t8 = 0xFF` and the callees'
/// registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80030FA0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    g[T6] = 0xFF;
    sw(m, g[SP], 0x10, g[T6]);
    g[A0] = (-0x67i64) as u64;
    g[A1] = 0;
    g[A2] = 0;
    g[A3] = 0;
    call(imports::func_8000AB24, m, ctx);
    let g = &mut ctx.gpr;
    g[T7] = 0xFF;
    g[T8] = 0xFF;
    sw(m, g[SP], 0x14, g[T8]);
    sw(m, g[SP], 0x10, g[T7]);
    g[A0] = 0x3E4;
    g[A1] = 0x3E8;
    g[A2] = 0xFF;
    g[A3] = 0xFF;
    call(imports::func_80038DF8, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[SP] = addu(g[SP], 0x20);
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

/// `func_800320E0(out, s)`: seven normalised values (**guess**: a pod's
/// stats for display, from the fields [`func_800321F0`] updates) with `K1
/// = [0x800AA3A0]`, `K2 = [0x800AA3A4]`, `K3 = [0x800AA3A8]`: `out[0] =
/// s[0] * 1.0`, `out[1] = s[1] / 1000`, `out[2] = 1 - sqrt(s[3] * 1.0) /
/// K1`, `out[3] = (s[4] - 450) / 200`, `out[4] = 8 / sqrt(s[5] * 0.5) - K2`,
/// `out[5] = s[9] / 20`, `out[6] = s[11]` (copied), stored in that order
/// (`s[k]` = the float at `s + 4k`). Then each `out[k]` is clamped:
/// `v < K3` stores `K3` (and re-reads), then `1 < v` stores 1.0; the loop
/// counts with a byte (`andi 0xFF`) up to 7.
///
/// Leaves `v0 = 7`, `v1 = out + 24`, `t6 = 24`, `t7 = 7`, `at = 0`, `f2 =
/// 1.0`, `f12 = K3`, `f0` = `out[6]` as last compared, and `f4`..`f18` from
/// the steps.
///
/// Domain: canonical pointers; `s[0], s[1], s[3], s[4], s[5], s[9]` and the
/// constants not NaN, `s[3]`, `s[5]` not negative (a NaN square root would
/// reach a division), and no NaN quotient or difference (`0/0`, `inf/inf`,
/// `inf - inf`); `s[11]` any (copied, then only compared).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800320E0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    let mut c1cs;
    g[AT] = li(0x3F80_0000);
    f[2].set_u32l(g[AT] as u32);
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    g[AT] = li(0x447A_0000);
    f[10].set_u32l(g[AT] as u32);
    f[6].set_fl(f[4].fl() * f[2].fl());
    g[AT] = li(0x800B_0000);
    g[V0] = 0;
    sw(m, g[A0], 0, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 4) as u32);
    f[16].set_fl(f[8].fl() / f[10].fl());
    sw(m, g[A0], 4, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[4].set_u32l(lw(m, g[AT], -0x5C60) as u32);
    g[AT] = li(0x43E1_0000);
    f[0].set_fl(f[18].fl() * f[2].fl());
    f[16].set_u32l(g[AT] as u32);
    g[AT] = li(0x4348_0000);
    f[0].set_fl(f[0].fl().sqrt());
    f[6].set_fl(f[0].fl() / f[4].fl());
    f[4].set_u32l(g[AT] as u32);
    g[AT] = li(0x3F00_0000);
    f[8].set_fl(f[2].fl() - f[6].fl());
    sw(m, g[A0], 8, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[18].set_fl(f[10].fl() - f[16].fl());
    f[10].set_u32l(g[AT] as u32);
    g[AT] = li(0x4100_0000);
    f[16].set_u32l(g[AT] as u32);
    f[6].set_fl(f[18].fl() / f[4].fl());
    g[AT] = li(0x800B_0000);
    sw(m, g[A0], 0xC, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[4].set_u32l(lw(m, g[AT], -0x5C5C) as u32);
    g[AT] = li(0x41A0_0000);
    f[0].set_fl(f[8].fl() * f[10].fl());
    f[10].set_u32l(g[AT] as u32);
    g[AT] = li(0x800B_0000);
    f[0].set_fl(f[0].fl().sqrt());
    f[18].set_fl(f[16].fl() / f[0].fl());
    f[6].set_fl(f[18].fl() - f[4].fl());
    sw(m, g[A0], 0x10, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[16].set_fl(f[8].fl() / f[10].fl());
    sw(m, g[A0], 0x14, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A1], 0x2C) as u32);
    sw(m, g[A0], 0x18, u64::from(f[18].u32l()));
    f[12].set_u32l(lw(m, g[AT], -0x5C58) as u32);
    loop {
        // L_800321A0
        g[T6] = sll(g[V0], 2);
        g[V1] = addu(g[A0], g[T6]);
        f[0].set_u32l(lw(m, g[V1], 0) as u32);
        g[V0] = addu(g[V0], 1);
        g[T7] = g[V0] & 0xFF;
        c1cs = f[0].fl() < f[12].fl();
        g[AT] = slt(g[T7], 7);
        if !c1cs {
            c1cs = f[2].fl() < f[0].fl();
        } else {
            sw(m, g[V1], 0, u64::from(f[12].u32l()));
            f[0].set_u32l(lw(m, g[V1], 0) as u32);
            c1cs = f[2].fl() < f[0].fl();
        }
        // L_800321D0
        if c1cs {
            sw(m, g[V1], 0, u64::from(f[2].u32l()));
        }
        // L_800321E0
        g[V0] = g[T7];
        if g[AT] == 0 {
            break;
        }
    }
}

/// Where [`func_800321F0`] gets a constant: an immediate (`lui at, hi;
/// mtc1 at`), a float in the data segment (`lwc1 [at + off]` with `at =
/// 0x800B0000`), or zero (`mtc1 zero`, `at` untouched).
#[derive(Clone, Copy)]
enum Konst {
    Imm(u32),
    Data(i32),
    Zero,
}

impl Konst {
    /// Load it into `f[r]` as the code does, `at` included.
    fn load(self, m: &Mem, g: &mut [u64; 32], f: &mut [crate::recomp::Fpr; 32], r: usize) {
        match self {
            Konst::Imm(v) => {
                g[AT] = li(v);
                f[r].set_u32l(v);
            }
            Konst::Data(off) => {
                g[AT] = li(0x800B_0000);
                f[r].set_u32l(lw(m, g[AT], off) as u32);
            }
            Konst::Zero => f[r].set_u32l(0),
        }
    }
}

/// One level's update in [`func_800321F0`], with the registers it uses.
#[derive(Clone, Copy)]
enum Step {
    /// `field += c * a3`: registers `[c, field, product, sum]`.
    Add(Konst, [usize; 4]),
    /// `field *= b + a * (1 - a3)` (`f2` holds 1.0): registers `[1 - a3, a,
    /// b, product, field, factor, result]`, used in that order.
    Mul(Konst, Konst, [usize; 7]),
}

/// One stat of [`func_800321F0`] (**guess**: a pod upgrade category).
struct Stat {
    /// The field's offset from `a0`.
    off: i32,
    /// The register level 1 reads `a3` into (later levels use `f16`).
    a3: usize,
    /// The bound registers: `hi` is tested first.
    hi: usize,
    lo: usize,
    /// The bounds as level 1 loads them (inside its block), and as the glue
    /// after level 1 loads them for levels 2..5.
    bounds1: &'static [(usize, Konst)],
    bounds: &'static [(usize, Konst)],
    /// Whether `f16` is re-read from the spill before levels 3, 4 and 5.
    reload: bool,
    /// Whether levels 3..5 set `at` for their constant before testing the
    /// level (the fallthrough case 0), rather than inside the block.
    at_first: bool,
    levels: [Step; 5],
}

const fn add(k: Konst, r: [usize; 4]) -> Step {
    Step::Add(k, r)
}

const fn mul(a: i32, b: i32, r: [usize; 7]) -> Step {
    Step::Mul(Konst::Data(a), Konst::Data(b), r)
}

use Konst::{Data, Imm, Zero};

/// The seven stats by `a1`, from the code (constants in the ROM: see the
/// function's doc).
const STATS: [Stat; 7] = [
    Stat {
        off: 0, a3: 16, hi: 2, lo: 12,
        bounds1: &[(2, Imm(0x3F80_0000)), (12, Data(-0x5C34))],
        bounds: &[(2, Imm(0x3F80_0000)), (12, Data(-0x5C30))],
        reload: false, at_first: true,
        levels: [
            add(Data(-0x5C38), [6, 4, 8, 10]), add(Data(-0x5C2C), [6, 18, 4, 8]), add(Data(-0x5C28), [6, 10, 18, 4]),
            add(Data(-0x5C24), [6, 8, 10, 18]), add(Imm(0x3E80_0000), [6, 4, 8, 10]),
        ],
    },
    Stat {
        off: 4, a3: 6, hi: 12, lo: 2,
        bounds1: &[(12, Imm(0x447A_0000)), (2, Imm(0x4248_0000))],
        bounds: &[(2, Imm(0x4248_0000)), (12, Imm(0x447A_0000))],
        reload: true, at_first: false,
        levels: [
            add(Imm(0x42E8_0000), [18, 8, 4, 10]), add(Imm(0x4368_0000), [6, 18, 8, 4]), add(Imm(0x43AE_0000), [6, 10, 18, 8]),
            add(Imm(0x43E8_0000), [6, 4, 10, 18]), add(Data(-0x5C20), [6, 8, 4, 10]),
        ],
    },
    Stat {
        off: 0xC, a3: 16, hi: 12, lo: 14,
        bounds1: &[(2, Imm(0x3F80_0000)), (12, Imm(0x40A0_0000)), (14, Data(-0x5C14))],
        bounds: &[(2, Imm(0x3F80_0000)), (12, Imm(0x40A0_0000)), (14, Data(-0x5C10))],
        reload: false, at_first: false,
        levels: [
            mul(-0x5C1C, -0x5C18, [6, 18, 4, 8, 18, 10, 6]), mul(-0x5C0C, -0x5C08, [8, 4, 10, 18, 4, 6, 8]),
            mul(-0x5C04, -0x5C00, [18, 10, 6, 4, 10, 8, 18]), mul(-0x5BFC, -0x5BF8, [4, 6, 8, 10, 6, 18, 4]),
            mul(-0x5BF4, -0x5BF0, [10, 8, 18, 6, 8, 4, 10]),
        ],
    },
    Stat {
        off: 0x10, a3: 6, hi: 2, lo: 12,
        bounds1: &[(2, Data(-0x5BEC)), (12, Imm(0x43E1_0000))],
        bounds: &[(2, Data(-0x5BE8)), (12, Imm(0x43E1_0000))],
        reload: true, at_first: false,
        levels: [
            add(Imm(0x4220_0000), [18, 4, 8, 10]), add(Imm(0x42A0_0000), [6, 18, 4, 8]), add(Imm(0x42F0_0000), [6, 10, 18, 4]),
            add(Imm(0x4320_0000), [6, 8, 10, 18]), add(Imm(0x4348_0000), [6, 4, 8, 10]),
        ],
    },
    Stat {
        off: 0x14, a3: 6, hi: 12, lo: 2,
        bounds1: &[(2, Imm(0x3F80_0000)), (12, Imm(0x447A_0000))],
        bounds: &[(2, Imm(0x3F80_0000)), (12, Imm(0x447A_0000))],
        reload: true, at_first: false,
        levels: [
            mul(-0x5BE4, -0x5BE0, [4, 18, 10, 8, 18, 6, 4]), mul(-0x5BDC, -0x5BD8, [8, 10, 6, 18, 10, 4, 8]),
            mul(-0x5BD4, -0x5BD0, [18, 6, 4, 10, 6, 8, 18]), mul(-0x5BCC, -0x5BC8, [10, 4, 8, 6, 4, 18, 10]),
            mul(-0x5BC4, -0x5BC0, [6, 8, 18, 4, 8, 10, 6]),
        ],
    },
    Stat {
        off: 0x24, a3: 4, hi: 12, lo: 2,
        bounds1: &[(12, Imm(0x41A0_0000)), (2, Imm(0x3F80_0000))],
        bounds: &[(2, Imm(0x3F80_0000)), (12, Imm(0x41A0_0000))],
        reload: true, at_first: false,
        levels: [
            add(Data(-0x5BBC), [18, 10, 8, 6]), add(Data(-0x5BB8), [4, 18, 10, 8]), add(Data(-0x5BB4), [4, 6, 18, 10]),
            add(Data(-0x5BB0), [4, 8, 6, 18]), add(Imm(0x4100_0000), [4, 10, 8, 6]),
        ],
    },
    Stat {
        off: 0x2C, a3: 4, hi: 2, lo: 12,
        bounds1: &[(2, Imm(0x3F80_0000)), (12, Zero)],
        bounds: &[(2, Imm(0x3F80_0000)), (12, Zero)],
        reload: true, at_first: false,
        levels: [
            add(Data(-0x5BAC), [18, 8, 10, 6]), add(Data(-0x5BA8), [4, 18, 8, 10]), add(Data(-0x5BA4), [4, 6, 18, 8]),
            add(Data(-0x5BA0), [4, 10, 6, 18]), add(Data(-0x5B9C), [4, 8, 10, 6]),
        ],
    },
];

/// Run one level's step on the stat's field, then clamp it: `v = field`
/// re-read; `hi < v` stores `hi` (and re-reads); then `v < lo` stores `lo`.
fn level_step(m: &mut Mem, g: &mut [u64; 32], f: &mut [crate::recomp::Fpr; 32], st: &Stat, step: Step, a3: usize) {
    let off = st.off;
    match step {
        Step::Add(k, [c, fld, p, s]) => {
            k.load(m, g, f, c);
            f[fld].set_u32l(lw(m, g[A0], off) as u32);
            f[p].set_fl(f[c].fl() * f[a3].fl());
            f[s].set_fl(f[fld].fl() + f[p].fl());
            sw(m, g[A0], off, u64::from(f[s].u32l()));
        }
        Step::Mul(a, b, [t, ra, rb, p, fld, s, r]) => {
            f[t].set_fl(f[2].fl() - f[a3].fl());
            a.load(m, g, f, ra);
            b.load(m, g, f, rb);
            f[p].set_fl(f[ra].fl() * f[t].fl());
            f[fld].set_u32l(lw(m, g[A0], off) as u32);
            f[s].set_fl(f[rb].fl() + f[p].fl());
            f[r].set_fl(f[fld].fl() * f[s].fl());
            sw(m, g[A0], off, u64::from(f[r].u32l()));
        }
    }
    f[0].set_u32l(lw(m, g[A0], off) as u32);
    if f[st.hi].fl() < f[0].fl() {
        sw(m, g[A0], off, u64::from(f[st.hi].u32l()));
        f[0].set_u32l(lw(m, g[A0], off) as u32);
    }
    if f[0].fl() < f[st.lo].fl() {
        sw(m, g[A0], off, u64::from(f[st.lo].u32l()));
    }
}

/// `func_800321F0(p, stat, level, x)` with the float `x` in `a3`: update
/// one of seven float fields of `p` by `level` 1..5 (**guess**: a pod's
/// stats by upgrade category, level and part condition `x`), then clamp it
/// to `[lo, hi]`. `stat >= 7` (unsigned) or a level outside 1..5 does
/// nothing but spill `a3` to `[sp + 0xC]` (done first in every case).
///
/// By `stat` (constants from the data segment at `0x800AA3C8..`, values
/// in the ROM):
///
/// | stat | field | update | per-level constants | bounds `[lo, hi]` |
/// |---|---|---|---|---|
/// | 0 | `+0` | `+= c * x` | 0.05, 0.1, 0.15, 0.2, 0.25 | `[0.01, 1]` |
/// | 1 | `+4` | `+= c * x` | 116, 232, 348, 464, 578 | `[50, 1000]` |
/// | 2 | `+0xC` | `*= b + a * (1 - x)` | (a, b) pairs, e.g. (0.14, 0.86) | `[0.1, 5]` |
/// | 3 | `+0x10` | `+= c * x` | 40, 80, 120, 160, 200 | `[450, 650]` |
/// | 4 | `+0x14` | `*= b + a * (1 - x)` | (a, b) pairs | `[1, 1000]` |
/// | 5 | `+0x24` | `+= c * x` | 1.6, 3.2, 4.8, 6.4, 8 | `[1, 20]` |
/// | 6 | `+0x2C` | `+= c * x` | 0.1 .. 0.45 | `[0, 1]` |
///
/// The clamp stores the result, re-reads it, stores `hi` if `hi < v`
/// (re-reading again), then `lo` if `v < lo`; so `hi` loses to `lo` when
/// `hi < lo`, and NaN results stay. Nothing is fused.
///
/// Registers follow the code block by block ([`STATS`]): the jump table
/// leaves `t6` = the table entry's address (N64Recomp's `lw` becomes an
/// `addiu`, NOTES.md "Translator"), each case its bound, constant and
/// temporary FPRs, `f16` (or the case's level-1 register) = `x`, and `at` =
/// the next level number, or the last constant's upper half after a level
/// ran (`0x3E800000` in case 0 when no level 5 ran).
///
/// Domain: canonical `p` with the field in RDRAM; for the level that runs,
/// `x`, the field and every intermediate not NaN (the results may be NaN
/// only if the last operation makes one, e.g. `0 * inf`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800321F0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = sltu(g[A1], 7);
    sw(m, g[SP], 0xC, g[A3]);
    if g[AT] == 0 {
        return;
    }
    g[T6] = sll(g[A1], 2);
    g[AT] = addu(li(0x800B_0000), g[T6]);
    g[T6] = addu(g[AT], (-0x5C54i64) as u64);
    let st = &STATS[g[A1] as usize];
    // Level 1, with its own bounds, then the bounds for levels 2..5.
    g[AT] = 1;
    f[st.a3].set_u32l(lw(m, g[SP], 0xC) as u32);
    if g[A2] == g[AT] {
        for &(r, k) in st.bounds1 {
            k.load(m, g, f, r);
        }
        level_step(m, g, f, st, st.levels[0], st.a3);
    }
    for &(r, k) in st.bounds {
        k.load(m, g, f, r);
    }
    g[AT] = 2;
    f[16].set_u32l(lw(m, g[SP], 0xC) as u32);
    if g[A2] == g[AT] {
        level_step(m, g, f, st, st.levels[1], 16);
    }
    for level in 3..=5 {
        g[AT] = level;
        if st.reload {
            f[16].set_u32l(lw(m, g[SP], 0xC) as u32);
        }
        let step = st.levels[level as usize - 1];
        let hit = g[A2] == g[AT];
        if st.at_first {
            // Case 0 sets `at` for the constant before testing the level.
            g[AT] = li(match step {
                Step::Add(Imm(v), _) => v,
                _ => 0x800B_0000,
            });
        }
        if hit {
            level_step(m, g, f, st, step, 16);
            if level == 5 {
                return;
            }
        } else if level == 5 {
            return;
        }
    }
}

/// The frame time (a double) that [`func_80033328`] and [`func_800334F4`]
/// scale their steps by (the animation step [`func_80031FA4`] subtracts it
/// too).
pub const FRAME_TIME: u32 = 0x8012_0BF0;

/// `func_80032F2C(dst, src, levels, conds)` (a pod's stats from its parts,
/// **guess**): copies the 0x3C bytes at `src` to `dst` (word by word,
/// three per step), then for each stat `i = 0..6` in order,
/// [`func_800321F0`]`(dst, i, levels[i], f32(conds[i]) / 255.0)` with
/// `levels`, `conds` bytes (unsigned) and the float in `a3`.
///
/// The conversion's unsigned correction (`+ 2^32` for a negative word) is
/// dead, the bytes being positive, and kept as the C has it.
///
/// Frame (`sp - 0x38`): `f20` (all 64 bits) at `+0x18`, `s0`..`s4` at
/// `+0x20`..`+0x30`, `ra` at `+0x34`, all restored (`f20 = 255.0` in
/// between). Leaves `t8 = t9 = src + 0x3C`, `t0 = dst + 0x3C`, and from
/// the last stat `t1` = the byte, `f4`/`f6` = it and its float, `f0 = x`,
/// `a0 = dst`, `a1 = 6`, `a2` = the level, `a3 = x`, and the callee's
/// registers.
///
/// Domain: the callee's, for each stat.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80032F2C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x38i64) as u64);
    sw(m, g[SP], 0x2C, g[S3]);
    g[S3] = g[A0];
    sw(m, g[SP], 0x34, g[RA]);
    sw(m, g[SP], 0x30, g[S4]);
    sw(m, g[SP], 0x28, g[S2]);
    sw(m, g[SP], 0x24, g[S1]);
    sw(m, g[SP], 0x20, g[S0]);
    sd(m, g[SP], 0x18, ctx.fpr[20].u64);
    g[T9] = g[A1];
    g[T0] = g[A0];
    g[T8] = addu(g[A1], 0x3C);
    loop {
        g[AT] = lw(m, g[T9], 0);
        g[T9] = addu(g[T9], 0xC);
        g[T0] = addu(g[T0], 0xC);
        sw(m, g[T0], -0xC, g[AT]);
        g[AT] = lw(m, g[T9], -8);
        sw(m, g[T0], -8, g[AT]);
        g[AT] = lw(m, g[T9], -4);
        sw(m, g[T0], -4, g[AT]);
        if g[T9] == g[T8] {
            break;
        }
    }
    g[AT] = li(0x437F_0000);
    ctx.fpr[20].set_u32l(g[AT] as u32);
    g[S0] = 0;
    g[S1] = g[A2];
    g[S2] = g[A3];
    g[S4] = 7;
    loop {
        let g = &mut ctx.gpr;
        g[T1] = lbu(m, g[S2], 0);
        g[A2] = lbu(m, g[S1], 0);
        g[AT] = li(0x4F80_0000);
        ctx.fpr[4].set_u32l(g[T1] as u32);
        ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
        if (g[T1] as i64) < 0 {
            ctx.fpr[8].set_u32l(g[AT] as u32);
            ctx.fpr[6].set_fl(ctx.fpr[6].fl() + ctx.fpr[8].fl());
        }
        ctx.fpr[0].set_fl(ctx.fpr[6].fl() / ctx.fpr[20].fl());
        g[A0] = g[S3];
        g[A1] = g[S0];
        g[A3] = s32(ctx.fpr[0].u32l());
        call(imports::func_800321F0, m, ctx);
        let g = &mut ctx.gpr;
        g[S0] = addu(g[S0], 1);
        g[S1] = addu(g[S1], 1);
        g[S2] = addu(g[S2], 1);
        if g[S0] == g[S4] {
            break;
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x34);
    ctx.fpr[20].u64 = ld(m, g[SP], 0x18);
    g[S0] = lw(m, g[SP], 0x20);
    g[S1] = lw(m, g[SP], 0x24);
    g[S2] = lw(m, g[SP], 0x28);
    g[S3] = lw(m, g[SP], 0x2C);
    g[S4] = lw(m, g[SP], 0x30);
    g[SP] = addu(g[SP], 0x38);
}

/// `func_80033328(x, angle, target, rate, bias, add)` with the floats
/// `target`, `rate` in `a2`/`a3` and `bias`, `add` on the stack (`sp +
/// 0x10`/`0x14`), `dt` = the double at [`FRAME_TIME`] (**guess**: a turn
/// rate eased toward a target, then integrated into a heading):
///
/// 1. If `target < *x`: `rate *= 5` if `*x > 0`; `*x = f32(f64(*x) - dt *
///    f64(rate))`; `*x = target` if now below it. Otherwise: `rate *= 5`
///    if `*x < 0`; `*x = f32(f64(*x) + dt * f64(rate))`; `*x = target` if
///    now above it. (Double arithmetic, rounded to nearest; each store is
///    re-read.)
/// 2. A positive `bias` clamps `*x` to `>= 0`, a negative one to `<= 0`.
/// 3. `*angle = f32(f64(*angle) + f64((*x + bias) + add) * dt)`, then 360
///    is subtracted if it is above 180, then added if it is below -180
///    (each once, in that order, re-reading).
///
/// Leaves `v0 = FRAME_TIME`, `f0 = *x`, `f2 = bias`, `f12` = the angle,
/// `f14` = the rate used, `f16 = 0.0`, `at` = `0x43B40000`, and the path's
/// FPRs (`f4`..`f18`, doubles included).
///
/// Domain: the floats, `dt` and every intermediate reaching an operation
/// not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033328(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A2] as u32);
    f[0].set_u32l(lw(m, g[A0], 0) as u32);
    f[14].set_u32l(g[A3] as u32);
    g[V0] = li(FRAME_TIME);
    f[16].set_u32l(0);
    g[AT] = li(0x40A0_0000);
    if f[12].fl() < f[0].fl() {
        // Above the target: step down, clamp at it.
        if f[16].fl() < f[0].fl() {
            f[4].set_u32l(g[AT] as u32);
            f[14].set_fl(f[14].fl() * f[4].fl());
        }
        f[6].u64 = ld(m, g[V0], 0);
        f[8].set_d(f64::from(f[14].fl()));
        f[18].set_d(f64::from(f[0].fl()));
        f[10].set_d(f[6].d() * f[8].d());
        f[4].set_d(f[18].d() - f[10].d());
        f[6].set_fl(fpu::cvt_s_d(f[4].d(), fpu::NEAREST));
        sw(m, g[A0], 0, u64::from(f[6].u32l()));
        f[0].set_u32l(lw(m, g[A0], 0) as u32);
        if f[0].fl() < f[12].fl() {
            sw(m, g[A0], 0, u64::from(f[12].u32l()));
            f[0].set_u32l(lw(m, g[A0], 0) as u32);
        }
    } else {
        // At or below: step up, clamp at it.
        if f[0].fl() < f[16].fl() {
            f[8].set_u32l(g[AT] as u32);
            f[14].set_fl(f[14].fl() * f[8].fl());
        }
        f[18].u64 = ld(m, g[V0], 0);
        f[10].set_d(f64::from(f[14].fl()));
        f[6].set_d(f64::from(f[0].fl()));
        f[4].set_d(f[18].d() * f[10].d());
        f[8].set_d(f[6].d() + f[4].d());
        f[18].set_fl(fpu::cvt_s_d(f[8].d(), fpu::NEAREST));
        sw(m, g[A0], 0, u64::from(f[18].u32l()));
        f[0].set_u32l(lw(m, g[A0], 0) as u32);
        if f[12].fl() < f[0].fl() {
            sw(m, g[A0], 0, u64::from(f[12].u32l()));
            f[0].set_u32l(lw(m, g[A0], 0) as u32);
        }
    }
    // The bias's sign bounds *x on one side of 0.
    f[2].set_u32l(lw(m, g[SP], 0x10) as u32);
    if f[16].fl() < f[2].fl() && f[0].fl() < f[16].fl() {
        sw(m, g[A0], 0, u64::from(f[16].u32l()));
        f[0].set_u32l(lw(m, g[A0], 0) as u32);
    }
    if f[2].fl() < f[16].fl() && f[16].fl() < f[0].fl() {
        sw(m, g[A0], 0, u64::from(f[16].u32l()));
        f[0].set_u32l(lw(m, g[A0], 0) as u32);
    }
    f[10].set_fl(f[0].fl() + f[2].fl());
    // Integrate the angle, then wrap it once.
    f[6].set_u32l(lw(m, g[SP], 0x14) as u32);
    f[18].u64 = ld(m, g[V0], 0);
    g[AT] = li(0x4334_0000);
    f[4].set_fl(f[10].fl() + f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[8].set_d(f64::from(f[4].fl()));
    f[4].set_d(f64::from(f[6].fl()));
    f[10].set_d(f[8].d() * f[18].d());
    f[6].set_u32l(g[AT] as u32);
    f[8].set_d(f[4].d() + f[10].d());
    f[18].set_fl(fpu::cvt_s_d(f[8].d(), fpu::NEAREST));
    sw(m, g[A1], 0, u64::from(f[18].u32l()));
    f[12].set_u32l(lw(m, g[A1], 0) as u32);
    let above = f[6].fl() < f[12].fl();
    g[AT] = li(0x43B4_0000);
    if above {
        f[4].set_u32l(g[AT] as u32);
        f[10].set_fl(f[12].fl() - f[4].fl());
        sw(m, g[A1], 0, u64::from(f[10].u32l()));
        f[12].set_u32l(lw(m, g[A1], 0) as u32);
    }
    g[AT] = li(0xC334_0000);
    f[8].set_u32l(g[AT] as u32);
    let below = f[12].fl() < f[8].fl();
    g[AT] = li(0x43B4_0000);
    if below {
        f[18].set_u32l(g[AT] as u32);
        f[6].set_fl(f[12].fl() + f[18].fl());
        sw(m, g[A1], 0, u64::from(f[6].u32l()));
    }
}

/// `func_800334F4(x, a, _, b, s)` with the floats `a`, `b` in `a1`/`a3`
/// and `s` on the stack (`a2` is only spilled to `[sp + 8]`): ease `*x`
/// toward `t = -(a / b) * s` clamped to `[-80, 80]` (80 checked first):
/// `*x = f32(f64(*x) + (f64(t - *x) * 5.0) * dt)` with `dt` = the double at
/// [`FRAME_TIME`] (double arithmetic, rounded to nearest).
///
/// Leaves `f12 = -80.0`, `f14 = b`, `f18 = 80.0`, `f2 = t`, `f10 = t -
/// *x`, `f0` = the new value, `f4`..`f8` the doubles, `at = 0x80120000`.
///
/// Domain: the floats and `dt` not NaN, and no NaN intermediate (`a / b`
/// for `0/0` or `inf/inf`, `0 * inf`, ...).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800334F4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A1] as u32);
    f[14].set_u32l(g[A3] as u32);
    f[8].set_u32l(lw(m, g[SP], 0x10) as u32);
    g[AT] = li(0x42A0_0000);
    f[4].set_fl(f[12].fl() / f[14].fl());
    f[18].set_u32l(g[AT] as u32);
    g[AT] = li(0xC2A0_0000);
    sw(m, g[SP], 8, g[A2]);
    f[12].set_u32l(g[AT] as u32);
    f[0].set_u32l(lw(m, g[A0], 0) as u32);
    f[6].set_fl(-f[4].fl());
    f[2].set_fl(f[6].fl() * f[8].fl());
    if f[18].fl() < f[2].fl() {
        f[2].set_u32l(f[18].u32l());
    }
    if f[2].fl() < f[12].fl() {
        f[2].set_u32l(f[12].u32l());
    }
    f[10].set_fl(f[2].fl() - f[0].fl());
    // 5.0 as a double: the high word by `lui`/`mtc1` into f7, the low word 0.
    g[AT] = li(0x4014_0000);
    f[6].set_u32h(g[AT] as u32);
    f[6].set_u32l(0);
    f[4].set_d(f64::from(f[10].fl()));
    g[AT] = li(0x8012_0000);
    f[8].set_d(f[4].d() * f[6].d());
    f[10].u64 = ld(m, g[AT], 0xBF0);
    f[6].set_d(f64::from(f[0].fl()));
    f[4].set_d(f[8].d() * f[10].d());
    f[8].set_d(f[6].d() + f[4].d());
    f[0].set_fl(fpu::cvt_s_d(f[8].d(), fpu::NEAREST));
    sw(m, g[A0], 0, u64::from(f[0].u32l()));
}

/// `func_80033590(pair, out)` (a point on a node, **guess**), `pair` two
/// node pointers `a`, `b` at `+0`/`+4`: `out = (0, 0, 0)`
/// ([`func_80015268`]) if `pair` or `a` is null. Otherwise, with `A` =
/// `a`'s transform as a 4x4 ([`func_80017C18`]), `out = A[3]` (the
/// translation row, copied as bits), and if `b` is nonzero, with `B` =
/// `b`'s, `out = A[1] * B[3][1] + out` ([`func_800155EC`]): moved along
/// `a`'s second axis by `b`'s y.
///
/// Frame (`sp - 0xA8`): `s0` (holding `out`) at `+0x18`, `ra` at `+0x1C`,
/// both restored sign-extended; `b` at `+0x20`, `B` at `+0x28`, `A` at
/// `+0x68`. Leaves `f4`/`f6`/`f8` = `A[3]` and the callees' registers; for
/// a null `pair` or `a`, `f0 = 0.0` and `a0 = out`, `a1`..`a3` = 0 (with
/// `a2`/`a3` the pair's words if `pair` isn't null).
///
/// Domain: with `b`, the callee's (`A[1]`, `B[3][1]`, `A[3]` and the
/// products not NaN).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033590(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0xA8i64) as u64);
    sw(m, g[SP], 0x18, g[S0]);
    g[S0] = g[A1];
    sw(m, g[SP], 0x1C, g[RA]);
    if g[A0] != 0 {
        g[A2] = lw(m, g[A0], 0);
        g[A3] = lw(m, g[A0], 4);
        g[A1] = addu(g[SP], 0x68);
        g[A0] = g[A2];
        if g[A2] != 0 {
            sw(m, g[SP], 0x20, g[A3]);
            call(imports::func_80017C18, m, ctx);
            let g = &mut ctx.gpr;
            ctx.fpr[4].set_u32l(lw(m, g[SP], 0x98) as u32);
            g[A3] = lw(m, g[SP], 0x20);
            sw(m, g[S0], 0, u64::from(ctx.fpr[4].u32l()));
            ctx.fpr[6].set_u32l(lw(m, g[SP], 0x9C) as u32);
            g[A0] = g[A3];
            sw(m, g[S0], 4, u64::from(ctx.fpr[6].u32l()));
            ctx.fpr[8].set_u32l(lw(m, g[SP], 0xA0) as u32);
            sw(m, g[S0], 8, u64::from(ctx.fpr[8].u32l()));
            if g[A3] != 0 {
                g[A1] = addu(g[SP], 0x28);
                call(imports::func_80017C18, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = g[S0];
                g[A1] = g[S0];
                g[A2] = lw(m, g[SP], 0x5C);
                g[A3] = addu(g[SP], 0x78);
                call(imports::func_800155EC, m, ctx);
            }
            let g = &mut ctx.gpr;
            g[RA] = lw(m, g[SP], 0x1C);
        } else {
            ctx.fpr[0].set_u32l(0);
            g[A0] = g[S0];
            g[A1] = s32(ctx.fpr[0].u32l());
            g[A2] = s32(ctx.fpr[0].u32l());
            g[A3] = s32(ctx.fpr[0].u32l());
            call(imports::func_80015268, m, ctx);
            let g = &mut ctx.gpr;
            g[RA] = lw(m, g[SP], 0x1C);
        }
    } else {
        ctx.fpr[0].set_u32l(0);
        g[A0] = g[A1];
        g[A1] = s32(ctx.fpr[0].u32l());
        g[A2] = s32(ctx.fpr[0].u32l());
        g[A3] = s32(ctx.fpr[0].u32l());
        call(imports::func_80015268, m, ctx);
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x1C);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0xA8);
}

/// `func_8003365C(pair, q, v, lim)` with the float `lim` in `a3` (keep
/// node `a` level with node `b`, **guess**), `pair` two node pointers `a`,
/// `b` at `+0`/`+4`: nothing if `pair`, `q`, `[q]`, `a` or `b` is null.
/// Otherwise, with `A`, `B` the nodes' transforms as 4x4s
/// ([`func_80017C18`]): `d = B[3] - v` ([`func_8001535C`]), then `v =
/// B[3]` (bits); if bit 30 of the word `[[q] + 0x100]` is set, or `d.y <
/// -lim`, or `lim < d.y`: `A[3] = A[1] * -d.y + A[3]` ([`func_800155EC`])
/// and `A` goes back into `a` ([`func_80017BA8`], which also sets its flag
/// bits 0 and 1).
///
/// Frame (`sp - 0xB8`): `ra` at `+0x14`; `q`, `v` and `lim` spilled to
/// their slots `+0xBC`..`+0xC4`; `b` at `+0x30`, `a` at `+0x34`; `B[3]`
/// copied to `+0x18`, `d` at `+0x24`, `B` at `+0x38`, `A` at `+0x78`.
/// Leaves, after the tests: `v0 = v`, `f10`/`f16`/`f18` = `B[3]`, `f2 =
/// lim`, `f0 = d.y`, `f4 = -lim` unless the bit is set, `t9 = q`, `t0 =
/// [q]`, `t1` the flags, `t2 = t1 << 1`, and `f6 = -d.y` and the callees'
/// registers when `a` moves. For the null cases, `t8 = [q]`, `a3 = a`, `a2
/// = b` as far as it got.
///
/// Domain: `B[3]` and `v` not NaN; `lim` not NaN unless the bit is set;
/// when `a` moves, `d.y`, `A[1]`, `A[3]` and the products not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003365C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_80033774: {
        g[SP] = addu(g[SP], (-0xB8i64) as u64);
        sw(m, g[SP], 0x14, g[RA]);
        sw(m, g[SP], 0xBC, g[A1]);
        sw(m, g[SP], 0xC0, g[A2]);
        sw(m, g[SP], 0xC4, g[A3]);
        if g[A0] != 0 {
            if g[A1] == 0 {
                g[RA] = lw(m, g[SP], 0x14);
                break 'b_80033774;
            }
            g[T8] = lw(m, g[A1], 0);
            if g[T8] == 0 {
                g[RA] = lw(m, g[SP], 0x14);
                break 'b_80033774;
            }
            g[A3] = lw(m, g[A0], 0);
            g[A2] = lw(m, g[A0], 4);
            if g[A3] == 0 {
                g[RA] = lw(m, g[SP], 0x14);
                break 'b_80033774;
            }
            g[A0] = g[A3];
            if g[A2] != 0 {
                g[A1] = addu(g[SP], 0x78);
                sw(m, g[SP], 0x30, g[A2]);
                sw(m, g[SP], 0x34, g[A3]);
                call(imports::func_80017C18, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = lw(m, g[SP], 0x30);
                g[A1] = addu(g[SP], 0x38);
                call(imports::func_80017C18, m, ctx);
                let g = &mut ctx.gpr;
                ctx.fpr[4].set_u32l(lw(m, g[SP], 0x68) as u32);
                ctx.fpr[6].set_u32l(lw(m, g[SP], 0x6C) as u32);
                ctx.fpr[8].set_u32l(lw(m, g[SP], 0x70) as u32);
                g[A0] = addu(g[SP], 0x24);
                g[A1] = addu(g[SP], 0x18);
                g[A2] = lw(m, g[SP], 0xC0);
                sw(m, g[SP], 0x18, u64::from(ctx.fpr[4].u32l()));
                sw(m, g[SP], 0x1C, u64::from(ctx.fpr[6].u32l()));
                sw(m, g[SP], 0x20, u64::from(ctx.fpr[8].u32l()));
                call(imports::func_8001535C, m, ctx);
                let g = &mut ctx.gpr;
                g[V0] = lw(m, g[SP], 0xC0);
                ctx.fpr[10].set_u32l(lw(m, g[SP], 0x18) as u32);
                sw(m, g[V0], 0, u64::from(ctx.fpr[10].u32l()));
                ctx.fpr[16].set_u32l(lw(m, g[SP], 0x1C) as u32);
                sw(m, g[V0], 4, u64::from(ctx.fpr[16].u32l()));
                ctx.fpr[18].set_u32l(lw(m, g[SP], 0x20) as u32);
                sw(m, g[V0], 8, u64::from(ctx.fpr[18].u32l()));
                g[T9] = lw(m, g[SP], 0xBC);
                ctx.fpr[2].set_u32l(lw(m, g[SP], 0xC4) as u32);
                g[T0] = lw(m, g[T9], 0);
                g[T1] = lw(m, g[T0], 0x100);
                g[T2] = sll(g[T1], 1);
                // d.y against lim unless flag bit 30 is set (every path
                // loads f0 = d.y, some twice).
                ctx.fpr[0].set_u32l(lw(m, g[SP], 0x28) as u32);
                if (g[T2] as i64) >= 0 {
                    ctx.fpr[4].set_fl(-ctx.fpr[2].fl());
                    if !(ctx.fpr[0].fl() < ctx.fpr[4].fl()) && !(ctx.fpr[2].fl() < ctx.fpr[0].fl()) {
                        g[RA] = lw(m, g[SP], 0x14);
                        break 'b_80033774;
                    }
                }
                g[A0] = addu(g[SP], 0xA8);
                g[A1] = g[A0];
                ctx.fpr[6].set_fl(-ctx.fpr[0].fl());
                g[A3] = addu(g[SP], 0x88);
                g[A2] = s32(ctx.fpr[6].u32l());
                call(imports::func_800155EC, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = lw(m, g[SP], 0x34);
                g[A1] = addu(g[SP], 0x78);
                call(imports::func_80017BA8, m, ctx);
            }
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x14);
    }
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], 0xB8);
}

/// `func_80033878(pair, v)` (place node `a` at `v`, **guess**; the inverse
/// of [`func_80033590`]), `pair` two node pointers `a`, `b` at `+0`/`+4`:
/// nothing if `pair` or `a` is null. Otherwise `M` = `a`'s transform as a
/// 4x4 ([`func_80017C18`], copied by [`func_800156DC`]) with its
/// translation row `M[3] = v` ([`func_80015288`]); if `b` is nonzero, with
/// `B` = `b`'s transform, `M[3] = M[1] * -B[3][1] + M[3]`
/// ([`func_800155EC`]); then `M` goes back into `a` ([`func_80017BA8`],
/// which also sets its flag bits 0 and 1).
///
/// Frame (`sp - 0xE8`): `s0` (holding `a`) at `+0x18` and `ra` at
/// `+0x1C`, both restored sign-extended; `v` spilled to its slot `+0xEC`;
/// `b` at `+0x20` (once `pair` is read), `B` at `+0x28`, the 4x4 at
/// `+0x68`, `M` at `+0xA8`. Leaves `t6 = b`, with `b` `f4 = B[3][1]` and
/// `f6 = -f4`, and the callees' registers.
///
/// Domain: with `b`, `B[3][1]` not NaN (negated), and `M[1]`, `v` and the
/// products not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033878(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_80033918: {
        g[SP] = addu(g[SP], (-0xE8i64) as u64);
        sw(m, g[SP], 0x1C, g[RA]);
        sw(m, g[SP], 0x18, g[S0]);
        sw(m, g[SP], 0xEC, g[A1]);
        if g[A0] != 0 {
            g[S0] = lw(m, g[A0], 0);
            g[T6] = lw(m, g[A0], 4);
            g[A1] = addu(g[SP], 0x68);
            sw(m, g[SP], 0x20, g[T6]);
            if g[S0] != 0 {
                g[A0] = g[S0];
                call(imports::func_80017C18, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = addu(g[SP], 0xA8);
                g[A1] = addu(g[SP], 0x68);
                call(imports::func_800156DC, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = addu(g[SP], 0xD8);
                g[A1] = lw(m, g[SP], 0xEC);
                call(imports::func_80015288, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = lw(m, g[SP], 0x20);
                g[A1] = addu(g[SP], 0xA8);
                if g[A0] == 0 {
                    g[A0] = g[S0];
                    call(imports::func_80017BA8, m, ctx);
                    let g = &mut ctx.gpr;
                    g[RA] = lw(m, g[SP], 0x1C);
                    break 'b_80033918;
                }
                let g = &mut ctx.gpr;
                g[A1] = addu(g[SP], 0x28);
                call(imports::func_80017C18, m, ctx);
                let g = &mut ctx.gpr;
                ctx.fpr[4].set_u32l(lw(m, g[SP], 0x5C) as u32);
                g[A0] = addu(g[SP], 0xD8);
                g[A1] = g[A0];
                ctx.fpr[6].set_fl(-ctx.fpr[4].fl());
                g[A3] = addu(g[SP], 0xB8);
                g[A2] = s32(ctx.fpr[6].u32l());
                call(imports::func_800155EC, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = g[S0];
                g[A1] = addu(g[SP], 0xA8);
                call(imports::func_80017BA8, m, ctx);
            }
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x1C);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0xE8);
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

/// `func_80033B5C(pp)`: the float `+0x110` of the object `*pp` (the rate
/// [`func_80006EB4`](crate::anim::func_80006EB4) sets), or 0.0 if `pp` or
/// `*pp` is null. Leaves `v0 = *pp` (when `pp` isn't null).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033B5C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    if g[A0] != 0 {
        g[V0] = lw(m, g[A0], 0);
        // L_80033B74
        if g[V0] != 0 {
            f[0].set_u32l(lw(m, g[V0], 0x110) as u32);
            // L_80033B8C
        } else {
            f[0].set_u32l(0 as u32);
        }
    } else {
        f[0].set_u32l(0 as u32);
    }
}

/// `func_80033B94(pp)`: as [`func_80033B5C`] for the float `+0x114` (the
/// time [`func_80006704`](crate::anim::func_80006704) looks up).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033B94(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    if g[A0] != 0 {
        g[V0] = lw(m, g[A0], 0);
        // L_80033BAC
        if g[V0] != 0 {
            f[0].set_u32l(lw(m, g[V0], 0x114) as u32);
            // L_80033BC4
        } else {
            f[0].set_u32l(0 as u32);
        }
    } else {
        f[0].set_u32l(0 as u32);
    }
}

/// `func_80033BCC(p, q)`, a float in `f0` (a node's heading in degrees,
/// **guess**): 0.0 unless `q`, `[q]`, `p`, `[p + 4]` and `n = [p + 8]` are
/// all nonzero; then, with `N` = `n`'s transform as a 4x4
/// ([`func_80017C18`]), [`func_80014F54`]`(-N[1][0], N[1][1])`, the game's
/// atan2 in degrees (`y` in `f12`, `x` in `f14`).
///
/// Frame (`sp - 0x60`): `ra` at `+0x14`, `N` at `+0x18`. Leaves `f12`,
/// `f14` = the arguments and the callees' registers; for 0.0, `t6`, `t7`
/// and `a2` as far as the tests got.
///
/// Domain: `N[1][0]` not NaN (negated), and the callee's.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033BCC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x60i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    if g[A1] != 0 {
        g[T6] = lw(m, g[A1], 0);
        if g[T6] != 0 {
            if g[A0] != 0 {
                g[T7] = lw(m, g[A0], 4);
                if g[T7] != 0 {
                    g[A2] = lw(m, g[A0], 8);
                    g[A0] = g[A2];
                    if g[A2] != 0 {
                        g[A1] = addu(g[SP], 0x18);
                        call(imports::func_80017C18, m, ctx);
                        let g = &mut ctx.gpr;
                        ctx.fpr[12].set_u32l(lw(m, g[SP], 0x28) as u32);
                        ctx.fpr[14].set_u32l(lw(m, g[SP], 0x2C) as u32);
                        ctx.fpr[12].set_fl(-ctx.fpr[12].fl());
                        call(imports::func_80014F54, m, ctx);
                        let g = &mut ctx.gpr;
                        g[RA] = lw(m, g[SP], 0x14);
                    } else {
                        ctx.fpr[0].set_u32l(0);
                        g[RA] = lw(m, g[SP], 0x14);
                    }
                } else {
                    ctx.fpr[0].set_u32l(0);
                    g[RA] = lw(m, g[SP], 0x14);
                }
            } else {
                ctx.fpr[0].set_u32l(0);
                g[RA] = lw(m, g[SP], 0x14);
            }
        } else {
            ctx.fpr[0].set_u32l(0);
            g[RA] = lw(m, g[SP], 0x14);
        }
    } else {
        ctx.fpr[0].set_u32l(0);
        g[RA] = lw(m, g[SP], 0x14);
    }
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], 0x60);
}

/// `func_80033C70()`: fill fixed entries of two tables (**guess**: menu or
/// HUD layout): words at `0x800A31E0 + 0x1A4..` = 0x11B, 0x11A, 0x11E, the
/// addresses `0x800AA640`/`0x800AA648` at `+0x1B4`/`+0x1B8`, 0x120 at
/// `+0x1D0`; then floats at `0x800A5CA0 + 0x36C..0x3A4` copied from the
/// data segment at `0x800AAA90..` (and `+0x3A4 = 0.0`), in the code's
/// order.
///
/// Leaves `v0 = 0x800A31E0`, `v1 = 0x800A5CA0`, `t6`..`t1` = the words and
/// addresses, `at = 0x800B0000`, `f4`..`f18` = the floats.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033C70(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = li(0x800A_31E0);
    g[T9] = li(0x800B_0000);
    g[T0] = li(0x800B_0000);
    g[T6] = 0x11B;
    g[T7] = 0x11A;
    g[T8] = 0x11E;
    g[T9] = addu(g[T9], (-0x59C0i64) as u64);
    g[T0] = addu(g[T0], (-0x59B8i64) as u64);
    g[T1] = 0x120;
    sw(m, g[V0], 0x1A4, g[T6]);
    sw(m, g[V0], 0x1A8, g[T7]);
    sw(m, g[V0], 0x1AC, g[T8]);
    sw(m, g[V0], 0x1B4, g[T9]);
    sw(m, g[V0], 0x1B8, g[T0]);
    sw(m, g[V0], 0x1D0, g[T1]);
    g[AT] = li(0x800B_0000);
    f[4].set_u32l(lw(m, g[AT], -0x5570) as u32);
    g[V1] = li(0x800A_5CA0);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x36C, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[AT], -0x556C) as u32);
    g[AT] = li(0x800B_0000);
    f[10].set_u32l(0 as u32);
    sw(m, g[V1], 0x39C, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[AT], -0x5568) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x3A4, u64::from(f[10].u32l()));
    sw(m, g[V1], 0x3A0, u64::from(f[8].u32l()));
    f[16].set_u32l(lw(m, g[AT], -0x5564) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x384, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[AT], -0x5560) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x388, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[AT], -0x555C) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x38C, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[AT], -0x5558) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x390, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[AT], -0x5554) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x394, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[AT], -0x5550) as u32);
    sw(m, g[V1], 0x398, u64::from(f[10].u32l()));
}

/// `func_80033D30()`: as [`func_80033C70`] for other entries: words at
/// `0x800A31E0 + 0x47C..` = 0x11D, 0x11C, 0x11F, the addresses `0x800AA650`
/// and `0x800AA654` at `+0x48C`/`+0x490`, 0x121 at `+0x4A8`; floats at
/// `0x800A5CA0 + 0x978..0x98C` from `0x800AAAB4..`.
///
/// Leaves `v0`, `v1`, `t6`..`t1`, `at`, `f4`..`f18` likewise.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033D30(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = li(0x800A_31E0);
    g[T9] = li(0x800B_0000);
    g[T0] = li(0x800B_0000);
    g[T6] = 0x11D;
    g[T7] = 0x11C;
    g[T8] = 0x11F;
    g[T9] = addu(g[T9], (-0x59B0i64) as u64);
    g[T0] = addu(g[T0], (-0x59ACi64) as u64);
    g[T1] = 0x121;
    sw(m, g[V0], 0x47C, g[T6]);
    sw(m, g[V0], 0x480, g[T7]);
    sw(m, g[V0], 0x484, g[T8]);
    sw(m, g[V0], 0x48C, g[T9]);
    sw(m, g[V0], 0x490, g[T0]);
    sw(m, g[V0], 0x4A8, g[T1]);
    g[AT] = li(0x800B_0000);
    f[4].set_u32l(lw(m, g[AT], -0x554C) as u32);
    g[V1] = li(0x800A_5CA0);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x984, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[AT], -0x5548) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x988, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[AT], -0x5544) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x98C, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[AT], -0x5540) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x978, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[AT], -0x553C) as u32);
    g[AT] = li(0x800B_0000);
    sw(m, g[V1], 0x97C, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[AT], -0x5538) as u32);
    sw(m, g[V1], 0x980, u64::from(f[18].u32l()));
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

/// `func_80033E40(m, v)`: copy the float triple `v` to `0x800A3FDC` and the
/// 4x4 matrix `m` (16 words) to `0x80112E20` (**guess**: the current view),
/// word by word.
///
/// Leaves `v0 = 0x800A3FDC`, `v1 = 0x80112E20`, `f4`..`f18` = the last
/// words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033E40(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A1], 0) as u32);
    g[V0] = li(0x800A_3FDC);
    sw(m, g[V0], 0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    g[V1] = li(0x8011_2E20);
    sw(m, g[V0], 4, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 8) as u32);
    sw(m, g[V0], 8, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0) as u32);
    sw(m, g[V1], 0, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A0], 4) as u32);
    sw(m, g[V1], 4, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A0], 8) as u32);
    sw(m, g[V1], 8, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A0], 0xC) as u32);
    sw(m, g[V1], 0xC, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0x10) as u32);
    sw(m, g[V1], 0x10, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x14) as u32);
    sw(m, g[V1], 0x14, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0x18) as u32);
    sw(m, g[V1], 0x18, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A0], 0x1C) as u32);
    sw(m, g[V1], 0x1C, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x20) as u32);
    sw(m, g[V1], 0x20, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A0], 0x24) as u32);
    sw(m, g[V1], 0x24, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0x28) as u32);
    sw(m, g[V1], 0x28, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x2C) as u32);
    sw(m, g[V1], 0x2C, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A0], 0x30) as u32);
    sw(m, g[V1], 0x30, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[A0], 0x34) as u32);
    sw(m, g[V1], 0x34, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x38) as u32);
    sw(m, g[V1], 0x38, u64::from(f[18].u32l()));
    f[4].set_u32l(lw(m, g[A0], 0x3C) as u32);
    sw(m, g[V1], 0x3C, u64::from(f[4].u32l()));
}

/// `func_80033EEC(m)`: push a 3x4 matrix (12 words) onto the stack of 48-byte
/// entries at `0x80112EA0` (**guess**; its pop is [`func_800344C8`]): set
/// the flag `[0x800A3FF4] = 1`; if the depth `[0x800A3FF0] < 32` (signed),
/// depth += 1 and entry `depth` = `m`, word by word. Entry 0 is never
/// written.
///
/// Leaves `a1 = 0x800A3FF0`, `t6 = 1`, `v0` = the old depth, `t0 =
/// 0x80112EA0`, `at` = the bound test, `t7` = depth + 1, and on a push `t9 =
/// 48 (depth + 1)`, `v1` = the entry, `f4`..`f18` = the last words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033EEC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[A1] = li(0x800A_0000);
    g[T6] = 1;
    g[AT] = li(0x800A_0000);
    g[A1] = addu(g[A1], 0x3FF0);
    sw(m, g[AT], 0x3FF4, g[T6]);
    g[V0] = lw(m, g[A1], 0);
    g[T0] = li(0x8011_2EA0);
    g[AT] = slt(g[V0], 0x20);
    g[T7] = addu(g[V0], 1);
    if g[AT] != 0 {
        sw(m, g[A1], 0, g[T7]);
        g[T9] = sll(g[T7], 2);
        f[4].set_u32l(lw(m, g[A0], 0) as u32);
        g[T9] = subu(g[T9], g[T7]);
        g[T9] = sll(g[T9], 4);
        g[V1] = addu(g[T9], g[T0]);
        sw(m, g[V1], 0, u64::from(f[4].u32l()));
        f[6].set_u32l(lw(m, g[A0], 4) as u32);
        sw(m, g[V1], 4, u64::from(f[6].u32l()));
        f[8].set_u32l(lw(m, g[A0], 8) as u32);
        sw(m, g[V1], 8, u64::from(f[8].u32l()));
        f[10].set_u32l(lw(m, g[A0], 0xC) as u32);
        sw(m, g[V1], 0xC, u64::from(f[10].u32l()));
        f[16].set_u32l(lw(m, g[A0], 0x10) as u32);
        sw(m, g[V1], 0x10, u64::from(f[16].u32l()));
        f[18].set_u32l(lw(m, g[A0], 0x14) as u32);
        sw(m, g[V1], 0x14, u64::from(f[18].u32l()));
        f[4].set_u32l(lw(m, g[A0], 0x18) as u32);
        sw(m, g[V1], 0x18, u64::from(f[4].u32l()));
        f[6].set_u32l(lw(m, g[A0], 0x1C) as u32);
        sw(m, g[V1], 0x1C, u64::from(f[6].u32l()));
        f[8].set_u32l(lw(m, g[A0], 0x20) as u32);
        sw(m, g[V1], 0x20, u64::from(f[8].u32l()));
        f[10].set_u32l(lw(m, g[A0], 0x24) as u32);
        sw(m, g[V1], 0x24, u64::from(f[10].u32l()));
        f[16].set_u32l(lw(m, g[A0], 0x28) as u32);
        sw(m, g[V1], 0x28, u64::from(f[16].u32l()));
        f[18].set_u32l(lw(m, g[A0], 0x2C) as u32);
        sw(m, g[V1], 0x2C, u64::from(f[18].u32l()));
    }
    // L_80033F8C
}

/// `func_80033F94(m)`: push `m * top` onto the stack of 3x4 matrices at
/// `0x80112EA0` ([`func_80033EEC`] pushes `m` itself): set the flag
/// `[0x800A3FF4] = 1`; if the depth `[0x800A3FF0] < 32` (signed), depth +=
/// 1 and entry `depth` = `m * p`, `p` the entry below. Matrices are 4 rows
/// of 3 floats (rows 0..2 the 3x3 part, row 3 the translation), and for
/// each column c:
/// - `out[r][c] = m[r][2] * p[2][c] + (p[0][c] * m[r][0] + p[1][c] * m[r][1])`
///   for r < 3;
/// - `out[3][c] = p[3][c] + ((p[0][c] * m[3][0] + p[1][c] * m[3][1]) +
///   p[2][c] * m[3][2])`.
///
/// Elements are computed and stored in row order, each from fresh loads.
///
/// Leaves `a3 = 0x800A3FF0`, `t6 = 1`, `v0` = the old depth, `a2 = m`, `at`
/// = the bound test, `t7` = depth + 1, and `t0 = 0x80110000`; on a push `t0
/// = 0x80112EA0`, `t9 = 48 (depth + 1)`, `a0` = the new entry, `a1` = the
/// one below, and `f4`..`f18` from the last element.
///
/// Domain: `m` a canonical pointer to 48 bytes, not overlapping the new
/// entry (it is re-read after stores); a depth whose entries lie in RDRAM
/// (negative ones pass the signed test). On a push, the elements, products
/// and partial sums not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80033F94(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[A3] = li(0x800A_0000);
    g[T6] = 1;
    g[AT] = li(0x800A_0000);
    g[A3] = addu(g[A3], 0x3FF0);
    sw(m, g[AT], 0x3FF4, g[T6]);
    g[V0] = lw(m, g[A3], 0);
    g[A2] = g[A0];
    g[T0] = li(0x8011_0000);
    g[AT] = slt(g[V0], 0x20);
    g[T7] = addu(g[V0], 1);
    if g[AT] != 0 {
        // a0 = the new entry, a1 = the old top, a2 = m.
        g[T9] = sll(g[T7], 2);
        g[T9] = subu(g[T9], g[T7]);
        g[T9] = sll(g[T9], 4);
        g[T0] = addu(g[T0], 0x2EA0);
        sw(m, g[A3], 0, g[T7]);
        g[A0] = addu(g[T9], g[T0]);
        g[A1] = addu(g[A0], (-0x30i64) as u64);
        f[4].set_u32l(lw(m, g[A1], 0) as u32);
        f[6].set_u32l(lw(m, g[A2], 0) as u32);
        f[10].set_u32l(lw(m, g[A1], 0xC) as u32);
        f[16].set_u32l(lw(m, g[A2], 4) as u32);
        f[8].set_fl(f[4].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A2], 8) as u32);
        f[18].set_fl(f[10].fl() * f[16].fl());
        f[10].set_u32l(lw(m, g[A1], 0x18) as u32);
        f[16].set_fl(f[6].fl() * f[10].fl());
        f[4].set_fl(f[8].fl() + f[18].fl());
        f[8].set_fl(f[16].fl() + f[4].fl());
        sw(m, g[A0], 0, u64::from(f[8].u32l())); // out[0][0]
        f[18].set_u32l(lw(m, g[A1], 4) as u32);
        f[6].set_u32l(lw(m, g[A2], 0) as u32);
        f[16].set_u32l(lw(m, g[A1], 0x10) as u32);
        f[4].set_u32l(lw(m, g[A2], 4) as u32);
        f[10].set_fl(f[18].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A2], 8) as u32);
        f[8].set_fl(f[16].fl() * f[4].fl());
        f[16].set_u32l(lw(m, g[A1], 0x1C) as u32);
        f[4].set_fl(f[6].fl() * f[16].fl());
        f[18].set_fl(f[10].fl() + f[8].fl());
        f[10].set_fl(f[4].fl() + f[18].fl());
        sw(m, g[A0], 4, u64::from(f[10].u32l())); // out[0][1]
        f[8].set_u32l(lw(m, g[A1], 8) as u32);
        f[6].set_u32l(lw(m, g[A2], 0) as u32);
        f[4].set_u32l(lw(m, g[A1], 0x14) as u32);
        f[18].set_u32l(lw(m, g[A2], 4) as u32);
        f[16].set_fl(f[8].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A2], 8) as u32);
        f[10].set_fl(f[4].fl() * f[18].fl());
        f[4].set_u32l(lw(m, g[A1], 0x20) as u32);
        f[18].set_fl(f[6].fl() * f[4].fl());
        f[8].set_fl(f[16].fl() + f[10].fl());
        f[16].set_fl(f[18].fl() + f[8].fl());
        sw(m, g[A0], 8, u64::from(f[16].u32l())); // out[0][2]
        f[10].set_u32l(lw(m, g[A1], 0) as u32);
        f[6].set_u32l(lw(m, g[A2], 0xC) as u32);
        f[18].set_u32l(lw(m, g[A1], 0xC) as u32);
        f[8].set_u32l(lw(m, g[A2], 0x10) as u32);
        f[4].set_fl(f[10].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A2], 0x14) as u32);
        f[16].set_fl(f[18].fl() * f[8].fl());
        f[18].set_u32l(lw(m, g[A1], 0x18) as u32);
        f[8].set_fl(f[6].fl() * f[18].fl());
        f[10].set_fl(f[4].fl() + f[16].fl());
        f[4].set_fl(f[8].fl() + f[10].fl());
        sw(m, g[A0], 0xC, u64::from(f[4].u32l())); // out[1][0]
        f[16].set_u32l(lw(m, g[A1], 4) as u32);
        f[6].set_u32l(lw(m, g[A2], 0xC) as u32);
        f[8].set_u32l(lw(m, g[A1], 0x10) as u32);
        f[10].set_u32l(lw(m, g[A2], 0x10) as u32);
        f[18].set_fl(f[16].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A2], 0x14) as u32);
        f[4].set_fl(f[8].fl() * f[10].fl());
        f[8].set_u32l(lw(m, g[A1], 0x1C) as u32);
        f[10].set_fl(f[6].fl() * f[8].fl());
        f[16].set_fl(f[18].fl() + f[4].fl());
        f[18].set_fl(f[10].fl() + f[16].fl());
        sw(m, g[A0], 0x10, u64::from(f[18].u32l())); // out[1][1]
        f[4].set_u32l(lw(m, g[A1], 8) as u32);
        f[6].set_u32l(lw(m, g[A2], 0xC) as u32);
        f[10].set_u32l(lw(m, g[A1], 0x14) as u32);
        f[16].set_u32l(lw(m, g[A2], 0x10) as u32);
        f[8].set_fl(f[4].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A2], 0x14) as u32);
        f[18].set_fl(f[10].fl() * f[16].fl());
        f[10].set_u32l(lw(m, g[A1], 0x20) as u32);
        f[16].set_fl(f[6].fl() * f[10].fl());
        f[4].set_fl(f[8].fl() + f[18].fl());
        f[8].set_fl(f[16].fl() + f[4].fl());
        sw(m, g[A0], 0x14, u64::from(f[8].u32l())); // out[1][2]
        f[18].set_u32l(lw(m, g[A1], 0) as u32);
        f[6].set_u32l(lw(m, g[A2], 0x18) as u32);
        f[16].set_u32l(lw(m, g[A1], 0xC) as u32);
        f[4].set_u32l(lw(m, g[A2], 0x1C) as u32);
        f[10].set_fl(f[18].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A2], 0x20) as u32);
        f[8].set_fl(f[16].fl() * f[4].fl());
        f[16].set_u32l(lw(m, g[A1], 0x18) as u32);
        f[4].set_fl(f[6].fl() * f[16].fl());
        f[18].set_fl(f[10].fl() + f[8].fl());
        f[10].set_fl(f[4].fl() + f[18].fl());
        sw(m, g[A0], 0x18, u64::from(f[10].u32l())); // out[2][0]
        f[8].set_u32l(lw(m, g[A1], 4) as u32);
        f[6].set_u32l(lw(m, g[A2], 0x18) as u32);
        f[4].set_u32l(lw(m, g[A1], 0x10) as u32);
        f[18].set_u32l(lw(m, g[A2], 0x1C) as u32);
        f[16].set_fl(f[8].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A2], 0x20) as u32);
        f[10].set_fl(f[4].fl() * f[18].fl());
        f[4].set_u32l(lw(m, g[A1], 0x1C) as u32);
        f[18].set_fl(f[6].fl() * f[4].fl());
        f[8].set_fl(f[16].fl() + f[10].fl());
        f[16].set_fl(f[18].fl() + f[8].fl());
        sw(m, g[A0], 0x1C, u64::from(f[16].u32l())); // out[2][1]
        f[10].set_u32l(lw(m, g[A1], 8) as u32);
        f[6].set_u32l(lw(m, g[A2], 0x18) as u32);
        f[18].set_u32l(lw(m, g[A1], 0x14) as u32);
        f[8].set_u32l(lw(m, g[A2], 0x1C) as u32);
        f[4].set_fl(f[10].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A2], 0x20) as u32);
        f[16].set_fl(f[18].fl() * f[8].fl());
        f[18].set_u32l(lw(m, g[A1], 0x20) as u32);
        f[8].set_fl(f[6].fl() * f[18].fl());
        f[10].set_fl(f[4].fl() + f[16].fl());
        f[4].set_fl(f[8].fl() + f[10].fl());
        sw(m, g[A0], 0x20, u64::from(f[4].u32l())); // out[2][2]
        f[6].set_u32l(lw(m, g[A2], 0x24) as u32);
        f[16].set_u32l(lw(m, g[A1], 0) as u32);
        f[10].set_u32l(lw(m, g[A2], 0x28) as u32);
        f[8].set_u32l(lw(m, g[A1], 0xC) as u32);
        f[18].set_fl(f[16].fl() * f[6].fl());
        f[6].set_u32l(lw(m, g[A1], 0x18) as u32);
        f[4].set_fl(f[8].fl() * f[10].fl());
        f[8].set_u32l(lw(m, g[A2], 0x2C) as u32);
        f[10].set_fl(f[6].fl() * f[8].fl());
        f[16].set_fl(f[18].fl() + f[4].fl());
        f[4].set_u32l(lw(m, g[A1], 0x24) as u32);
        f[18].set_fl(f[16].fl() + f[10].fl());
        f[6].set_fl(f[4].fl() + f[18].fl());
        sw(m, g[A0], 0x24, u64::from(f[6].u32l())); // out[3][0]
        f[16].set_u32l(lw(m, g[A2], 0x24) as u32);
        f[8].set_u32l(lw(m, g[A1], 4) as u32);
        f[18].set_u32l(lw(m, g[A2], 0x28) as u32);
        f[4].set_u32l(lw(m, g[A1], 0x10) as u32);
        f[10].set_fl(f[8].fl() * f[16].fl());
        f[16].set_u32l(lw(m, g[A1], 0x1C) as u32);
        f[6].set_fl(f[4].fl() * f[18].fl());
        f[4].set_u32l(lw(m, g[A2], 0x2C) as u32);
        f[18].set_fl(f[16].fl() * f[4].fl());
        f[8].set_fl(f[10].fl() + f[6].fl());
        f[6].set_u32l(lw(m, g[A1], 0x28) as u32);
        f[10].set_fl(f[8].fl() + f[18].fl());
        f[16].set_fl(f[6].fl() + f[10].fl());
        sw(m, g[A0], 0x28, u64::from(f[16].u32l())); // out[3][1]
        f[8].set_u32l(lw(m, g[A2], 0x24) as u32);
        f[4].set_u32l(lw(m, g[A1], 8) as u32);
        f[10].set_u32l(lw(m, g[A2], 0x28) as u32);
        f[6].set_u32l(lw(m, g[A1], 0x14) as u32);
        f[18].set_fl(f[4].fl() * f[8].fl());
        f[8].set_u32l(lw(m, g[A1], 0x20) as u32);
        f[16].set_fl(f[6].fl() * f[10].fl());
        f[6].set_u32l(lw(m, g[A2], 0x2C) as u32);
        f[10].set_fl(f[8].fl() * f[6].fl());
        f[4].set_fl(f[18].fl() + f[16].fl());
        f[16].set_u32l(lw(m, g[A1], 0x2C) as u32);
        f[18].set_fl(f[4].fl() + f[10].fl());
        f[8].set_fl(f[16].fl() + f[18].fl());
        sw(m, g[A0], 0x2C, u64::from(f[8].u32l())); // out[3][2]
    }
}

/// `func_8003423C(out)`: copy the top 3x4 matrix of the stack at
/// `0x80112EA0` ([`func_80033EEC`]) to `out`, word by word (12 words).
/// QUIRK: the depth `[0x800A3FF0]` is re-read (and multiplied by 48 with
/// `multu`) before every word, so an `out` over it changes the source of
/// later words (as in [`func_800059A8`](crate::matrix::func_800059A8)).
///
/// Leaves `v1 = 0x800A3FF0`, `a1 = 48`, `v0 = 0x80112EA0`, `lo`/`hi` from
/// the last `multu`, and in `t0`..`t9`, `f4`..`f18` what the last words
/// used.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003423C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V1] = li(0x800A_3FF0);
    g[T6] = lw(m, g[V1], 0);
    g[A1] = 0x30;
    g[V0] = li(0x8011_0000);
    let (lo, _) = multu(g[T6], g[A1]);
    g[V0] = addu(g[V0], 0x2EA0);
    g[T7] = lo;
    g[T8] = addu(g[V0], g[T7]);
    f[4].set_u32l(lw(m, g[T8], 0) as u32);
    sw(m, g[A0], 0, u64::from(f[4].u32l()));
    g[T9] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T9], g[A1]);
    g[T0] = lo;
    g[T1] = addu(g[V0], g[T0]);
    f[6].set_u32l(lw(m, g[T1], 4) as u32);
    sw(m, g[A0], 4, u64::from(f[6].u32l()));
    g[T2] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T2], g[A1]);
    g[T3] = lo;
    g[T4] = addu(g[V0], g[T3]);
    f[8].set_u32l(lw(m, g[T4], 8) as u32);
    sw(m, g[A0], 8, u64::from(f[8].u32l()));
    g[T5] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T5], g[A1]);
    g[T6] = lo;
    g[T7] = addu(g[V0], g[T6]);
    f[10].set_u32l(lw(m, g[T7], 0xC) as u32);
    sw(m, g[A0], 0xC, u64::from(f[10].u32l()));
    g[T8] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T8], g[A1]);
    g[T9] = lo;
    g[T0] = addu(g[V0], g[T9]);
    f[16].set_u32l(lw(m, g[T0], 0x10) as u32);
    sw(m, g[A0], 0x10, u64::from(f[16].u32l()));
    g[T1] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T1], g[A1]);
    g[T2] = lo;
    g[T3] = addu(g[V0], g[T2]);
    f[18].set_u32l(lw(m, g[T3], 0x14) as u32);
    sw(m, g[A0], 0x14, u64::from(f[18].u32l()));
    g[T4] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T4], g[A1]);
    g[T5] = lo;
    g[T6] = addu(g[V0], g[T5]);
    f[4].set_u32l(lw(m, g[T6], 0x18) as u32);
    sw(m, g[A0], 0x18, u64::from(f[4].u32l()));
    g[T7] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T7], g[A1]);
    g[T8] = lo;
    g[T9] = addu(g[V0], g[T8]);
    f[6].set_u32l(lw(m, g[T9], 0x1C) as u32);
    sw(m, g[A0], 0x1C, u64::from(f[6].u32l()));
    g[T0] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T0], g[A1]);
    g[T1] = lo;
    g[T2] = addu(g[V0], g[T1]);
    f[8].set_u32l(lw(m, g[T2], 0x20) as u32);
    sw(m, g[A0], 0x20, u64::from(f[8].u32l()));
    g[T3] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T3], g[A1]);
    g[T4] = lo;
    g[T5] = addu(g[V0], g[T4]);
    f[10].set_u32l(lw(m, g[T5], 0x24) as u32);
    sw(m, g[A0], 0x24, u64::from(f[10].u32l()));
    g[T6] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T6], g[A1]);
    g[T7] = lo;
    g[T8] = addu(g[V0], g[T7]);
    f[16].set_u32l(lw(m, g[T8], 0x28) as u32);
    sw(m, g[A0], 0x28, u64::from(f[16].u32l()));
    g[T9] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T9], g[A1]);
    g[T0] = lo;
    g[T1] = addu(g[V0], g[T0]);
    f[18].set_u32l(lw(m, g[T1], 0x2C) as u32);
    sw(m, g[A0], 0x2C, u64::from(f[18].u32l()));
}

/// `func_80034374(out)`: the top 3x4 matrix of that stack as a 4x4
/// matrix: row `i` of `out` = its row `i`, the fourth column (0, 0, 0,
/// 1.0). Word by word, re-reading the depth as [`func_8003423C`] does; the
/// fourth column is written last.
///
/// Leaves `v1`, `a1`, `v0`, `lo`/`hi`, `f0 = 0.0`, `at = 0x3F800000` and
/// the last words' temporaries as [`func_8003423C`] does.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80034374(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V1] = li(0x800A_3FF0);
    g[T6] = lw(m, g[V1], 0);
    g[A1] = 0x30;
    g[V0] = li(0x8011_0000);
    let (lo, _) = multu(g[T6], g[A1]);
    g[V0] = addu(g[V0], 0x2EA0);
    f[0].set_u32l(0 as u32);
    g[AT] = li(0x3F80_0000);
    g[T7] = lo;
    g[T8] = addu(g[V0], g[T7]);
    f[4].set_u32l(lw(m, g[T8], 0) as u32);
    sw(m, g[A0], 0, u64::from(f[4].u32l()));
    g[T9] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T9], g[A1]);
    g[T0] = lo;
    g[T1] = addu(g[V0], g[T0]);
    f[6].set_u32l(lw(m, g[T1], 4) as u32);
    sw(m, g[A0], 4, u64::from(f[6].u32l()));
    g[T2] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T2], g[A1]);
    g[T3] = lo;
    g[T4] = addu(g[V0], g[T3]);
    f[8].set_u32l(lw(m, g[T4], 8) as u32);
    sw(m, g[A0], 8, u64::from(f[8].u32l()));
    g[T5] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T5], g[A1]);
    g[T6] = lo;
    g[T7] = addu(g[V0], g[T6]);
    f[10].set_u32l(lw(m, g[T7], 0xC) as u32);
    sw(m, g[A0], 0x10, u64::from(f[10].u32l()));
    g[T8] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T8], g[A1]);
    g[T9] = lo;
    g[T0] = addu(g[V0], g[T9]);
    f[16].set_u32l(lw(m, g[T0], 0x10) as u32);
    sw(m, g[A0], 0x14, u64::from(f[16].u32l()));
    g[T1] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T1], g[A1]);
    g[T2] = lo;
    g[T3] = addu(g[V0], g[T2]);
    f[18].set_u32l(lw(m, g[T3], 0x14) as u32);
    sw(m, g[A0], 0x18, u64::from(f[18].u32l()));
    g[T4] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T4], g[A1]);
    g[T5] = lo;
    g[T6] = addu(g[V0], g[T5]);
    f[4].set_u32l(lw(m, g[T6], 0x18) as u32);
    sw(m, g[A0], 0x20, u64::from(f[4].u32l()));
    g[T7] = lw(m, g[V1], 0);
    f[4].set_u32l(g[AT] as u32);
    let (lo, _) = multu(g[T7], g[A1]);
    g[T8] = lo;
    g[T9] = addu(g[V0], g[T8]);
    f[6].set_u32l(lw(m, g[T9], 0x1C) as u32);
    sw(m, g[A0], 0x24, u64::from(f[6].u32l()));
    g[T0] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T0], g[A1]);
    g[T1] = lo;
    g[T2] = addu(g[V0], g[T1]);
    f[8].set_u32l(lw(m, g[T2], 0x20) as u32);
    sw(m, g[A0], 0x28, u64::from(f[8].u32l()));
    g[T3] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T3], g[A1]);
    g[T4] = lo;
    g[T5] = addu(g[V0], g[T4]);
    f[10].set_u32l(lw(m, g[T5], 0x24) as u32);
    sw(m, g[A0], 0x30, u64::from(f[10].u32l()));
    g[T6] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T6], g[A1]);
    g[T7] = lo;
    g[T8] = addu(g[V0], g[T7]);
    f[16].set_u32l(lw(m, g[T8], 0x28) as u32);
    sw(m, g[A0], 0x34, u64::from(f[16].u32l()));
    g[T9] = lw(m, g[V1], 0);
    let (lo, _) = multu(g[T9], g[A1]);
    g[T0] = lo;
    g[T1] = addu(g[V0], g[T0]);
    f[18].set_u32l(lw(m, g[T1], 0x2C) as u32);
    sw(m, g[A0], 0xC, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x1C, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x2C, u64::from(f[0].u32l()));
    sw(m, g[A0], 0x3C, u64::from(f[4].u32l()));
    sw(m, g[A0], 0x38, u64::from(f[18].u32l()));
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

/// `func_800344F4(out, m)` (float 4x4 to the RSP's fixed-point `Mtx`, like
/// libultra's `guMtxF2L` with clamps): for each entry `v = m[i][j]`
/// (row-major, 64 bytes), with `v` negated in column 0 when bit 14 of the
/// settings word `[0x800D697C]` is set (**guess**: mirror mode):
/// - `0 < v`: 32000 if `32000 < v`, else 0 if `v < E` (`E =
///   [0x800AAAD4]`, 1e-6 in the ROM);
/// - otherwise: -32000 if `v < -32000`, else 0 if `E' < v` (`E' =
///   [0x800AAAD0]`, -1e-6), so -0.0 becomes +0.0;
///
/// then `w = trunc(v * 65536)` (a C cast) goes to the frame word `sp -
/// 0x38 + 0x20`, and its two signed halfwords to the integer half (`out +
/// 2k`) and the fraction half (`out + 0x20 + 2k`), `k = 4i + j`. Saves and
/// restores `f20`/`f22` in the frame.
///
/// Leaves `v0 = out + 0x20`, `v1 = out + 0x40`, `a0 = a2 = 4`, `a3` = `m`'s
/// last row, `t0 = 0x800D6960`, `t1`/`t2` = the frame halves, `f12 =
/// 32000`, `f14 = 65536`, `f16 = E`, `f18 = f2 = 0`, `f0` = the last value,
/// `f4`/`f6` its product and truncation.
///
/// Domain: the entries not NaN (the product is guarded).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800344F4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    let mut c1cs;
    g[SP] = addu(g[SP], (-0x38i64) as u64);
    sd(m, g[SP], 0x10, f[22].u64);
    g[AT] = li(0x800B_0000);
    f[22].set_u32l(lw(m, g[AT], -0x5530) as u32);
    sd(m, g[SP], 8, f[20].u64);
    g[AT] = li(0xC6FA_0000);
    f[20].set_u32l(g[AT] as u32);
    g[AT] = li(0x800B_0000);
    f[16].set_u32l(lw(m, g[AT], -0x552C) as u32);
    g[AT] = li(0x4780_0000);
    f[14].set_u32l(g[AT] as u32);
    g[AT] = li(0x46FA_0000);
    g[T0] = li(0x800D_0000);
    f[12].set_u32l(g[AT] as u32);
    f[18].set_u32l(0 as u32);
    f[2].set_u32l(0 as u32);
    g[V0] = g[A0];
    g[V1] = addu(g[A0], 0x20);
    g[T0] = addu(g[T0], 0x6960);
    g[T2] = addu(g[SP], 0x22);
    g[T1] = addu(g[SP], 0x20);
    g[A2] = 0;
    g[T6] = sll(g[A2], 4);
    loop {
        // L_80034550
        g[A3] = addu(g[A1], g[T6]);
        g[A0] = 0;
        loop {
            // L_80034558
            g[T7] = sll(g[A0], 2);
            g[T8] = addu(g[A3], g[T7]);
            f[0].set_u32l(lw(m, g[T8], 0) as u32);
            if g[A0] == 0 {
                g[T9] = lw(m, g[T0], 0x1C);
                g[T3] = g[T9] & 0x4000;
                if g[T3] != 0 {
                    f[0].set_fl(-f[0].fl());
                }
            }
            // L_8003457C
            c1cs = f[2].fl() < f[0].fl();
            // L_80034580
            if !c1cs {
                c1cs = f[0].fl() < f[20].fl();
                // L_800345C0
                if !c1cs {
                    c1cs = f[22].fl() < f[0].fl();
                    // L_800345D8
                    if c1cs {
                        f[0].set_u32l(f[18].u32l());
                    }
                } else {
                    f[0].set_u32l(f[20].u32l());
                }
            } else {
                c1cs = f[12].fl() < f[0].fl();
                if !c1cs {
                    c1cs = f[0].fl() < f[16].fl();
                    // L_800345A8
                    if c1cs {
                        f[0].set_u32l(f[18].u32l());
                    }
                } else {
                    f[0].set_u32l(f[12].u32l());
                }
            }
            // L_800345E8
            f[4].set_fl(f[0].fl() * f[14].fl());
            g[A0] = addu(g[A0], 1);
            g[T8] = sll(g[A0], 16);
            g[A0] = sra(g[T8], 16);
            g[AT] = slt(g[A0], 4);
            g[V0] = addu(g[V0], 2);
            g[V1] = addu(g[V1], 2);
            f[6].set_u32l(fpu::trunc_w_s(f[4].fl()));
            g[T5] = s32(f[6].u32l());
            sw(m, g[SP], 0x20, g[T5]);
            g[T6] = lh(m, g[T1], 0);
            sh(m, g[V0], -2, g[T6]);
            g[T7] = lh(m, g[T2], 0);
            sh(m, g[V1], -2, g[T7]);
            if g[AT] == 0 {
                break;
            }
        }
        g[A2] = addu(g[A2], 1);
        g[T3] = sll(g[A2], 16);
        g[A2] = sra(g[T3], 16);
        g[AT] = slt(g[A2], 4);
        if g[AT] == 0 {
            break;
        }
        g[T6] = sll(g[A2], 4);
    }
    f[20].u64 = ld(m, g[SP], 8);
    f[22].u64 = ld(m, g[SP], 0x10);
    g[SP] = addu(g[SP], 0x38);
}

/// `func_80034650(out, m)`: [`func_800344F4`] for a 4x3 matrix (four rows
/// of three floats, 48 bytes, as on the stack at `0x80112EA0`): column 3
/// is not read but written as 0, or 1.0 (`0x10000`) in row 3. The
/// thresholds are `[0x800AAADC]` and `[0x800AAAD8]`.
///
/// Leaves `v0 = out + 0x20`, `v1 = out + 0x40`, `a0 = a2 = 4`, `a3 =
/// 0x800D6960`, `t2 = 0x10000`, and the rest as [`func_800344F4`] does.
///
/// Domain: the entries not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80034650(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    let mut c1cs;
    g[SP] = addu(g[SP], (-0x38i64) as u64);
    sd(m, g[SP], 0x10, f[22].u64);
    g[AT] = li(0x800B_0000);
    f[22].set_u32l(lw(m, g[AT], -0x5528) as u32);
    sd(m, g[SP], 8, f[20].u64);
    g[AT] = li(0xC6FA_0000);
    f[20].set_u32l(g[AT] as u32);
    g[AT] = li(0x800B_0000);
    f[16].set_u32l(lw(m, g[AT], -0x5524) as u32);
    g[AT] = li(0x4780_0000);
    f[14].set_u32l(g[AT] as u32);
    g[AT] = li(0x46FA_0000);
    g[A3] = li(0x800D_0000);
    f[12].set_u32l(g[AT] as u32);
    f[18].set_u32l(0 as u32);
    f[2].set_u32l(0 as u32);
    g[V0] = g[A0];
    g[V1] = addu(g[A0], 0x20);
    g[A3] = addu(g[A3], 0x6960);
    g[T1] = addu(g[SP], 0x22);
    g[T0] = addu(g[SP], 0x20);
    g[A2] = 0;
    g[T2] = li(0x1_0000);
    g[A0] = 0;
    loop {
        // L_800346B0
        g[AT] = slt(g[A0], 3);
        g[T6] = sll(g[A2], 2);
        if g[AT] == 0 {
            // L_8003476C
            g[AT] = slt(g[A2], 3);
            if g[AT] == 0 {
                sw(m, g[SP], 0x20, g[T2]);
            } else {
                sw(m, g[SP], 0x20, 0);
            }
        } else {
            g[T6] = subu(g[T6], g[A2]);
            g[T6] = sll(g[T6], 2);
            g[T7] = addu(g[A1], g[T6]);
            g[T8] = sll(g[A0], 2);
            g[T9] = addu(g[T7], g[T8]);
            f[0].set_u32l(lw(m, g[T9], 0) as u32);
            if g[A0] == 0 {
                g[T3] = lw(m, g[A3], 0x1C);
                g[T4] = g[T3] & 0x4000;
                if g[T4] != 0 {
                    f[0].set_fl(-f[0].fl());
                }
            }
            // L_800346EC
            c1cs = f[2].fl() < f[0].fl();
            // L_800346F0
            if !c1cs {
                c1cs = f[0].fl() < f[20].fl();
                // L_80034730
                if !c1cs {
                    c1cs = f[22].fl() < f[0].fl();
                    // L_80034748
                    if c1cs {
                        f[0].set_u32l(f[18].u32l());
                    }
                } else {
                    f[0].set_u32l(f[20].u32l());
                }
            } else {
                c1cs = f[12].fl() < f[0].fl();
                if !c1cs {
                    c1cs = f[0].fl() < f[16].fl();
                    // L_80034718
                    if c1cs {
                        f[0].set_u32l(f[18].u32l());
                    }
                } else {
                    f[0].set_u32l(f[12].u32l());
                }
            }
            // L_80034758
            f[4].set_fl(f[0].fl() * f[14].fl());
            f[6].set_u32l(fpu::trunc_w_s(f[4].fl()));
            g[T6] = s32(f[6].u32l());
            sw(m, g[SP], 0x20, g[T6]);
        }
        // L_80034784
        g[T7] = lh(m, g[T0], 0);
        g[A0] = addu(g[A0], 1);
        g[T9] = sll(g[A0], 16);
        sh(m, g[V0], 0, g[T7]);
        g[T8] = lh(m, g[T1], 0);
        g[A0] = sra(g[T9], 16);
        g[AT] = slt(g[A0], 4);
        g[V0] = addu(g[V0], 2);
        g[V1] = addu(g[V1], 2);
        sh(m, g[V1], -2, g[T8]);
        if g[AT] == 0 {
            g[A2] = addu(g[A2], 1);
            g[T4] = sll(g[A2], 16);
            g[A2] = sra(g[T4], 16);
            g[AT] = slt(g[A2], 4);
            if g[AT] == 0 {
                f[20].u64 = ld(m, g[SP], 8);
                f[22].u64 = ld(m, g[SP], 0x10);
                g[SP] = addu(g[SP], 0x38);
                return;
            }
            g[A0] = 0;
        }
    }
}

/// `func_800347D8(out, m)`: copy a 4x4 matrix, row by row, with the row and
/// column counters kept as sign-extended halfwords (`sll 16; sra 16`).
///
/// Leaves `a3 = m`, `v0 = v1 = 4`, `a1` = the last row of `out`, `t0` =
/// the last row of `m`, `a2 = 12`, `f4` = the last word, and the counter
/// temporaries.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800347D8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[A3] = g[A1];
    g[V0] = 0;
    g[T6] = sll(g[V0], 4);
    loop {
        // L_800347E4
        g[T7] = sll(g[V0], 4);
        g[T0] = addu(g[A3], g[T7]);
        g[A1] = addu(g[A0], g[T6]);
        g[V1] = 0;
        loop {
            // L_800347F4
            g[A2] = sll(g[V1], 2);
            g[V1] = addu(g[V1], 1);
            g[T1] = sll(g[V1], 16);
            g[T8] = addu(g[T0], g[A2]);
            f[4].set_u32l(lw(m, g[T8], 0) as u32);
            g[V1] = sra(g[T1], 16);
            g[AT] = slt(g[V1], 4);
            g[T9] = addu(g[A1], g[A2]);
            sw(m, g[T9], 0, u64::from(f[4].u32l()));
            if g[AT] == 0 {
                break;
            }
        }
        g[V0] = addu(g[V0], 1);
        g[T3] = sll(g[V0], 16);
        g[V0] = sra(g[T3], 16);
        g[AT] = slt(g[V0], 4);
        if g[AT] == 0 {
            break;
        }
        g[T6] = sll(g[V0], 4);
    }
}

/// A stack of 4x3 float matrices (four rows of three, 48 bytes each), as
/// [`func_80034650`] converts them; the depth word is [`MTX43_DEPTH`].
pub const MTX43_STACK: u32 = 0x8011_2EA0;

/// The depth of [`MTX43_STACK`] (a word).
pub const MTX43_DEPTH: u32 = 0x800A_3FF0;

/// `func_8003483C()` (reset the 4x3 stack, **guess**): the depth
/// [`MTX43_DEPTH`] `= 0`, `[0x800A3FF4] = 1`, `[0x800A3FF8] = 0`; then the
/// matrix at the depth in [`MTX43_STACK`] (the depth re-read for each row)
/// = identity, row by row through [`func_80015268`]: `(1, 0, 0)`, `(0, 1,
/// 0)`, `(0, 0, 1)`, `(0, 0, 0)`.
///
/// Frame (`sp - 0x20`): `f20` (all 64 bits; 0.0 in between) at `+0x10`,
/// restored, and `ra` at `+0x1C`. Leaves `v0 = MTX43_DEPTH`, the last
/// row's address arithmetic in `t8`, `t9`, `t0`, `t1`, and the callee's
/// registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003483C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V0] = li(MTX43_DEPTH);
    sw(m, g[V0], 0, 0);
    g[T6] = 1;
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x3FF4, g[T6]);
    g[AT] = li(0x800A_0000);
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[AT], 0x3FF8, 0);
    g[T7] = lw(m, g[V0], 0);
    sd(m, g[SP], 0x10, ctx.fpr[20].u64);
    ctx.fpr[20].set_u32l(0);
    g[T8] = sll(g[T7], 2);
    g[T9] = li(0x8011_0000);
    g[T8] = subu(g[T8], g[T7]);
    sw(m, g[SP], 0x1C, g[RA]);
    g[T8] = sll(g[T8], 4);
    g[T9] = addu(g[T9], 0x2EA0);
    g[A2] = s32(ctx.fpr[20].u32l());
    g[A3] = s32(ctx.fpr[20].u32l());
    g[A0] = addu(g[T8], g[T9]);
    g[A1] = li(0x3F80_0000);
    call(imports::func_80015268, m, ctx);
    let g = &mut ctx.gpr;
    g[T0] = li(0x800A_0000);
    g[T0] = lw(m, g[T0], 0x3FF0);
    g[T3] = li(0x8011_2EA0);
    g[T1] = sll(g[T0], 2);
    g[T1] = subu(g[T1], g[T0]);
    g[T1] = sll(g[T1], 4);
    g[T2] = addu(g[T1], 0xC);
    g[A1] = s32(ctx.fpr[20].u32l());
    g[A3] = s32(ctx.fpr[20].u32l());
    g[A0] = addu(g[T2], g[T3]);
    g[A2] = li(0x3F80_0000);
    call(imports::func_80015268, m, ctx);
    let g = &mut ctx.gpr;
    g[T4] = li(0x800A_0000);
    g[T4] = lw(m, g[T4], 0x3FF0);
    g[T7] = li(0x8011_2EA0);
    g[T5] = sll(g[T4], 2);
    g[T5] = subu(g[T5], g[T4]);
    g[T5] = sll(g[T5], 4);
    g[T6] = addu(g[T5], 0x18);
    g[A1] = s32(ctx.fpr[20].u32l());
    g[A2] = s32(ctx.fpr[20].u32l());
    g[A0] = addu(g[T6], g[T7]);
    g[A3] = li(0x3F80_0000);
    call(imports::func_80015268, m, ctx);
    let g = &mut ctx.gpr;
    g[T8] = li(0x800A_0000);
    g[T8] = lw(m, g[T8], 0x3FF0);
    g[T1] = li(0x8011_2EA0);
    g[T9] = sll(g[T8], 2);
    g[T9] = subu(g[T9], g[T8]);
    g[T9] = sll(g[T9], 4);
    g[T0] = addu(g[T9], 0x24);
    g[A1] = s32(ctx.fpr[20].u32l());
    g[A2] = s32(ctx.fpr[20].u32l());
    g[A3] = s32(ctx.fpr[20].u32l());
    g[A0] = addu(g[T0], g[T1]);
    call(imports::func_80015268, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    ctx.fpr[20].u64 = ld(m, g[SP], 0x10);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_80034948()` (the stack's matrices to the RSP, **guess**): with `p`
/// = the 4x3 matrix at the depth in [`MTX43_STACK`] (at the depth itself,
/// unlike [`func_80034E20`]): `[0x801134D0]` = the next ring entry
/// ([`func_80033E08`]), filled from `p` ([`func_80034650`]); then
/// `[0x801134D4]` = the next, filled from the 4x4 at `0x80112E60`
/// ([`func_800344F4`]).
///
/// Frame (`sp - 0x20`): `ra` at `+0x14`, `p` at `+0x1C`. Leaves `v1 =
/// 0x801134D4`, `a1 = 0x80112E60`, `a0` = the second entry, and the
/// callees' registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80034948(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = li(0x800A_0000);
    g[T6] = lw(m, g[T6], 0x3FF0);
    g[T8] = li(0x8011_0000);
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    g[T7] = sll(g[T6], 2);
    g[T7] = subu(g[T7], g[T6]);
    g[T7] = sll(g[T7], 4);
    g[T8] = addu(g[T8], 0x2EA0);
    sw(m, g[SP], 0x14, g[RA]);
    g[T9] = addu(g[T7], g[T8]);
    sw(m, g[SP], 0x1C, g[T9]);
    call(imports::func_80033E08, m, ctx);
    let g = &mut ctx.gpr;
    g[V1] = li(0x8011_34D0);
    sw(m, g[V1], 0, g[V0]);
    g[A0] = g[V0];
    g[A1] = lw(m, g[SP], 0x1C);
    call(imports::func_80034650, m, ctx);
    call(imports::func_80033E08, m, ctx);
    let g = &mut ctx.gpr;
    g[V1] = li(0x8011_34D4);
    g[A1] = li(0x8011_0000);
    sw(m, g[V1], 0, g[V0]);
    g[A1] = addu(g[A1], 0x2E60);
    g[A0] = g[V0];
    call(imports::func_800344F4, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_80034E20()` (load the model matrix, **guess**): with `p` = the 4x3
/// matrix one below the depth in [`MTX43_STACK`] (`MTX43_STACK + 48 *
/// (depth - 1)`), `V` = the 4x4 at `0x80112E20` and, if the word
/// `[0x800A3FEC]` is nonzero, `o` = the vec3 at `0x800A3FDC`:
/// 1. With `o`: `p`'s row 3 `-= o`, component by component (stored).
/// 2. `P = p × V` into the frame, `p` taken as a 4x4 with column 3 `(0, 0,
///    0, 1)`: `P[r][c] = (p[r][0] * V[0][c] + p[r][1] * V[1][c]) + p[r][2] *
///    V[2][c]`, with `V[3][c]` added last in row 3. Not fused; row by row.
/// 3. With `o`: `p`'s row 3 `+= o`. QUIRK: `(t - o) + o` rounds, so the
///    translation can change.
/// 4. `gSPMatrix`: the next ring entry ([`func_80033E08`]) filled from `p`
///    ([`func_80034650`]) is appended to the list at
///    [`DL2_HEAD`](crate::render::DL2_HEAD) as `DA380003`, entry; then
///    `gSPForceMatrix`: another entry filled from
///    `P` ([`func_800344F4`]), appended as `DC38000E`, entry, and
///    `DB0C0000`, `0x10000`.
///
/// Frame (`sp - 0xD0`): `f20`..`f30` (all 64 bits) at `+0x10`..`+0x38`,
/// restored; `ra` at `+0x44`; the entry at `+0x88`, `P` at `+0x8C`, `p` at
/// `+0xCC`. Leaves `a2` = the list head's address, `v0` = the last command's address,
/// `t3`..`t8` from the appends, the callees' registers, and before them
/// `v1` = the flag (re-read with `o`).
///
/// Domain: the entries of `p` and `V`, `o`, and every sum not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80034E20(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = li(0x800A_0000);
    g[T6] = lw(m, g[T6], 0x3FF0);
    g[SP] = addu(g[SP], (-0xD0i64) as u64);
    g[V1] = li(0x800A_0000);
    g[T7] = sll(g[T6], 2);
    g[V1] = lw(m, g[V1], 0x3FEC);
    g[T7] = subu(g[T7], g[T6]);
    g[T7] = sll(g[T7], 4);
    g[T9] = li(MTX43_STACK);
    g[T8] = addu(g[T7], (-0x30i64) as u64);
    sw(m, g[SP], 0x44, g[RA]);
    sd(m, g[SP], 0x38, ctx.fpr[30].u64);
    sd(m, g[SP], 0x30, ctx.fpr[28].u64);
    sd(m, g[SP], 0x28, ctx.fpr[26].u64);
    sd(m, g[SP], 0x20, ctx.fpr[24].u64);
    sd(m, g[SP], 0x18, ctx.fpr[22].u64);
    sd(m, g[SP], 0x10, ctx.fpr[20].u64);
    let f = &mut ctx.fpr;
    g[A1] = addu(g[T8], g[T9]);
    if g[V1] != 0 {
        g[A0] = li(0x800A_3FDC);
        f[6].set_u32l(lw(m, g[A0], 0) as u32);
        f[4].set_u32l(lw(m, g[A1], 0x24) as u32);
        f[10].set_u32l(lw(m, g[A1], 0x28) as u32);
        g[V1] = li(0x800A_0000);
        f[8].set_fl(f[4].fl() - f[6].fl());
        sw(m, g[A1], 0x24, u64::from(f[8].u32l()));
        f[4].set_u32l(lw(m, g[A0], 4) as u32);
        f[8].set_u32l(lw(m, g[A1], 0x2C) as u32);
        f[6].set_fl(f[10].fl() - f[4].fl());
        sw(m, g[A1], 0x28, u64::from(f[6].u32l()));
        f[10].set_u32l(lw(m, g[A0], 8) as u32);
        f[4].set_fl(f[8].fl() - f[10].fl());
        sw(m, g[A1], 0x2C, u64::from(f[4].u32l()));
        g[V1] = lw(m, g[V1], 0x3FEC);
    }
    g[V0] = li(0x8011_2E20);
    f[2].set_u32l(lw(m, g[V0], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[12].set_u32l(lw(m, g[V0], 0x10) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_fl(f[2].fl() * f[6].fl());
    f[0].set_u32l(lw(m, g[V0], 0x20) as u32);
    f[16].set_u32l(lw(m, g[V0], 4) as u32);
    f[4].set_fl(f[12].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A1], 8) as u32);
    f[18].set_u32l(lw(m, g[V0], 0x14) as u32);
    f[14].set_u32l(lw(m, g[V0], 0x24) as u32);
    f[22].set_u32l(lw(m, g[V0], 8) as u32);
    f[24].set_u32l(lw(m, g[V0], 0x18) as u32);
    f[20].set_u32l(lw(m, g[V0], 0x28) as u32);
    f[6].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[10].fl() * f[0].fl());
    f[28].set_u32l(lw(m, g[V0], 0xC) as u32);
    f[30].set_u32l(lw(m, g[V0], 0x1C) as u32);
    f[26].set_u32l(lw(m, g[V0], 0x2C) as u32);
    g[A0] = li(0x800A_3FDC);
    f[4].set_fl(f[8].fl() + f[6].fl());
    sw(m, g[SP], 0x8C, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_fl(f[16].fl() * f[10].fl());
    f[4].set_fl(f[18].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[10].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[6].fl() * f[14].fl());
    f[4].set_fl(f[8].fl() + f[10].fl());
    sw(m, g[SP], 0x90, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0) as u32);
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_fl(f[22].fl() * f[6].fl());
    f[4].set_fl(f[24].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A1], 8) as u32);
    f[6].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[10].fl() * f[20].fl());
    f[4].set_fl(f[8].fl() + f[6].fl());
    sw(m, g[SP], 0x94, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0) as u32);
    f[6].set_u32l(lw(m, g[A1], 4) as u32);
    f[8].set_fl(f[28].fl() * f[10].fl());
    f[4].set_fl(f[30].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 8) as u32);
    f[10].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[6].fl() * f[26].fl());
    f[4].set_fl(f[8].fl() + f[10].fl());
    sw(m, g[SP], 0x98, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[8].set_fl(f[2].fl() * f[6].fl());
    f[4].set_fl(f[12].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[6].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[10].fl() * f[0].fl());
    f[4].set_fl(f[8].fl() + f[6].fl());
    sw(m, g[SP], 0x9C, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[6].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[8].set_fl(f[16].fl() * f[10].fl());
    f[4].set_fl(f[18].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[10].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[6].fl() * f[14].fl());
    f[4].set_fl(f[8].fl() + f[10].fl());
    sw(m, g[SP], 0xA0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[8].set_fl(f[22].fl() * f[6].fl());
    f[4].set_fl(f[24].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[6].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[10].fl() * f[20].fl());
    f[4].set_fl(f[8].fl() + f[6].fl());
    sw(m, g[SP], 0xA4, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0xC) as u32);
    f[6].set_u32l(lw(m, g[A1], 0x10) as u32);
    f[8].set_fl(f[28].fl() * f[10].fl());
    f[4].set_fl(f[30].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 0x14) as u32);
    f[10].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[6].fl() * f[26].fl());
    f[4].set_fl(f[8].fl() + f[10].fl());
    sw(m, g[SP], 0xA8, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x1C) as u32);
    f[8].set_fl(f[2].fl() * f[6].fl());
    f[4].set_fl(f[12].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A1], 0x20) as u32);
    f[6].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[10].fl() * f[0].fl());
    f[4].set_fl(f[8].fl() + f[6].fl());
    sw(m, g[SP], 0xAC, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[6].set_u32l(lw(m, g[A1], 0x1C) as u32);
    f[8].set_fl(f[16].fl() * f[10].fl());
    f[4].set_fl(f[18].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 0x20) as u32);
    f[10].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[6].fl() * f[14].fl());
    f[4].set_fl(f[8].fl() + f[10].fl());
    sw(m, g[SP], 0xB0, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x1C) as u32);
    f[8].set_fl(f[22].fl() * f[6].fl());
    f[4].set_fl(f[24].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A1], 0x20) as u32);
    f[6].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[10].fl() * f[20].fl());
    f[4].set_fl(f[8].fl() + f[6].fl());
    sw(m, g[SP], 0xB4, u64::from(f[4].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x18) as u32);
    f[6].set_u32l(lw(m, g[A1], 0x1C) as u32);
    f[8].set_fl(f[28].fl() * f[10].fl());
    f[4].set_fl(f[30].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 0x20) as u32);
    f[10].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[6].fl() * f[26].fl());
    f[4].set_fl(f[8].fl() + f[10].fl());
    sw(m, g[SP], 0xB8, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[10].set_u32l(lw(m, g[A1], 0x28) as u32);
    f[8].set_fl(f[2].fl() * f[6].fl());
    f[4].set_fl(f[12].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[A1], 0x2C) as u32);
    f[6].set_fl(f[8].fl() + f[4].fl());
    f[8].set_fl(f[0].fl() * f[10].fl());
    f[10].set_u32l(lw(m, g[V0], 0x30) as u32);
    f[4].set_fl(f[6].fl() + f[8].fl());
    f[6].set_fl(f[10].fl() + f[4].fl());
    sw(m, g[SP], 0xBC, u64::from(f[6].u32l()));
    f[8].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[4].set_u32l(lw(m, g[A1], 0x28) as u32);
    f[10].set_fl(f[16].fl() * f[8].fl());
    f[6].set_fl(f[18].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[A1], 0x2C) as u32);
    f[8].set_fl(f[10].fl() + f[6].fl());
    f[10].set_fl(f[14].fl() * f[4].fl());
    f[4].set_u32l(lw(m, g[V0], 0x34) as u32);
    f[6].set_fl(f[8].fl() + f[10].fl());
    f[8].set_fl(f[4].fl() + f[6].fl());
    sw(m, g[SP], 0xC0, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[6].set_u32l(lw(m, g[A1], 0x28) as u32);
    f[4].set_fl(f[22].fl() * f[10].fl());
    f[8].set_fl(f[24].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[A1], 0x2C) as u32);
    f[10].set_fl(f[4].fl() + f[8].fl());
    f[4].set_fl(f[20].fl() * f[6].fl());
    f[6].set_u32l(lw(m, g[V0], 0x38) as u32);
    f[8].set_fl(f[10].fl() + f[4].fl());
    f[10].set_fl(f[6].fl() + f[8].fl());
    sw(m, g[SP], 0xC4, u64::from(f[10].u32l()));
    f[4].set_u32l(lw(m, g[A1], 0x24) as u32);
    f[8].set_u32l(lw(m, g[A1], 0x28) as u32);
    f[6].set_fl(f[28].fl() * f[4].fl());
    f[10].set_fl(f[30].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[A1], 0x2C) as u32);
    f[4].set_fl(f[6].fl() + f[10].fl());
    f[6].set_fl(f[26].fl() * f[8].fl());
    f[8].set_u32l(lw(m, g[V0], 0x3C) as u32);
    f[10].set_fl(f[4].fl() + f[6].fl());
    f[4].set_fl(f[8].fl() + f[10].fl());
    sw(m, g[SP], 0xC8, u64::from(f[4].u32l()));
    if g[V1] != 0 {
        f[6].set_u32l(lw(m, g[A0], 0) as u32);
        f[8].set_u32l(lw(m, g[A1], 0x24) as u32);
        f[10].set_fl(f[6].fl() + f[8].fl());
        f[6].set_u32l(lw(m, g[A1], 0x28) as u32);
        sw(m, g[A1], 0x24, u64::from(f[10].u32l()));
        f[4].set_u32l(lw(m, g[A0], 4) as u32);
        f[8].set_fl(f[4].fl() + f[6].fl());
        f[4].set_u32l(lw(m, g[A1], 0x2C) as u32);
        sw(m, g[A1], 0x28, u64::from(f[8].u32l()));
        f[10].set_u32l(lw(m, g[A0], 8) as u32);
        f[6].set_fl(f[10].fl() + f[4].fl());
        sw(m, g[A1], 0x2C, u64::from(f[6].u32l()));
    }
    sw(m, g[SP], 0xCC, g[A1]);
    call(imports::func_80033E08, m, ctx);
    let g = &mut ctx.gpr;
    g[A1] = lw(m, g[SP], 0xCC);
    sw(m, g[SP], 0x88, g[V0]);
    g[A0] = g[V0];
    call(imports::func_80034650, m, ctx);
    let g = &mut ctx.gpr;
    g[A2] = li(DL2_HEAD);
    g[V0] = lw(m, g[A2], 0);
    g[T1] = li(0xDA38_0003);
    g[T0] = addu(g[V0], 8);
    sw(m, g[A2], 0, g[T0]);
    sw(m, g[V0], 0, g[T1]);
    g[T2] = lw(m, g[SP], 0x88);
    sw(m, g[V0], 4, g[T2]);
    call(imports::func_80033E08, m, ctx);
    let g = &mut ctx.gpr;
    sw(m, g[SP], 0x88, g[V0]);
    g[A0] = g[V0];
    g[A1] = addu(g[SP], 0x8C);
    call(imports::func_800344F4, m, ctx);
    let g = &mut ctx.gpr;
    g[A2] = li(DL2_HEAD);
    g[V0] = lw(m, g[A2], 0);
    g[T4] = li(0xDC38_000E);
    g[T3] = addu(g[V0], 8);
    sw(m, g[A2], 0, g[T3]);
    sw(m, g[V0], 0, g[T4]);
    g[T5] = lw(m, g[SP], 0x88);
    g[T7] = li(0xDB0C_0000);
    g[T8] = li(0x1_0000);
    sw(m, g[V0], 4, g[T5]);
    g[V0] = lw(m, g[A2], 0);
    g[T6] = addu(g[V0], 8);
    sw(m, g[A2], 0, g[T6]);
    sw(m, g[V0], 4, g[T8]);
    sw(m, g[V0], 0, g[T7]);
    g[RA] = lw(m, g[SP], 0x44);
    ctx.fpr[30].u64 = ld(m, g[SP], 0x38);
    ctx.fpr[28].u64 = ld(m, g[SP], 0x30);
    ctx.fpr[26].u64 = ld(m, g[SP], 0x28);
    ctx.fpr[24].u64 = ld(m, g[SP], 0x20);
    ctx.fpr[22].u64 = ld(m, g[SP], 0x18);
    ctx.fpr[20].u64 = ld(m, g[SP], 0x10);
    g[SP] = addu(g[SP], 0xD0);
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

/// `func_80037BF8(t, r, p)` (**guess**: rotate a transform about a pivot):
/// for each column `k` of the 3x3 matrix `r` (rows of three floats), the
/// float `t[9 + k]` (the translation row of a 4x3 transform `t`) becomes
/// `(((t[9+k] + (-p.x) * r[0][k]) + (-p.y) * r[1][k]) + (-p.z) * r[2][k])
/// + p[k]`, each partial sum stored and re-read (so `t` may overlap `p` or
/// `r` only as the code allows). Nothing is fused.
///
/// Leaves `a0 = v0 = 3`, `v1 = t + 12`, `a3 = r + 12`, `t0 = p + 12`, and
/// `f4`..`f18` from the last column.
///
/// Domain: canonical pointers; the floats, products and partial sums not
/// NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80037BF8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V1] = g[A0];
    g[A0] = 3;
    g[V0] = 0;
    g[A3] = g[A1];
    g[T0] = g[A2];
    loop {
        // L_80037C0C
        f[4].set_u32l(lw(m, g[A2], 0) as u32);
        f[8].set_u32l(lw(m, g[A3], 0) as u32);
        f[16].set_u32l(lw(m, g[V1], 0x24) as u32);
        f[6].set_fl(-f[4].fl());
        g[V0] = addu(g[V0], 1);
        f[10].set_fl(f[6].fl() * f[8].fl());
        g[V1] = addu(g[V1], 4);
        g[A3] = addu(g[A3], 4);
        g[T0] = addu(g[T0], 4);
        f[18].set_fl(f[16].fl() + f[10].fl());
        sw(m, g[V1], 0x20, u64::from(f[18].u32l()));
        f[4].set_u32l(lw(m, g[A2], 4) as u32);
        f[8].set_u32l(lw(m, g[A3], 8) as u32);
        f[10].set_u32l(lw(m, g[V1], 0x20) as u32);
        f[6].set_fl(-f[4].fl());
        f[16].set_fl(f[6].fl() * f[8].fl());
        f[18].set_fl(f[10].fl() + f[16].fl());
        sw(m, g[V1], 0x20, u64::from(f[18].u32l()));
        f[4].set_u32l(lw(m, g[A2], 8) as u32);
        f[8].set_u32l(lw(m, g[A3], 0x14) as u32);
        f[16].set_u32l(lw(m, g[V1], 0x20) as u32);
        f[6].set_fl(-f[4].fl());
        f[10].set_fl(f[6].fl() * f[8].fl());
        f[18].set_fl(f[16].fl() + f[10].fl());
        sw(m, g[V1], 0x20, u64::from(f[18].u32l()));
        f[4].set_u32l(lw(m, g[V1], 0x20) as u32);
        f[6].set_u32l(lw(m, g[T0], -4) as u32);
        f[8].set_fl(f[4].fl() + f[6].fl());
        sw(m, g[V1], 0x20, u64::from(f[8].u32l()));
        if g[V0] == g[A0] {
            break;
        }
    }
}

/// `func_80038294(o)` (pick a level by thresholds, **guess**): with `n =
/// [o + 0x14]` ([`func_80017DAC`]) and the floats `k[j] = [o + 0x1C +
/// 4j]`: -1 if `n <= 0`; 0 if the word `[0x800A3FE8]` is 0; -1 if `10.0 <
/// k[0]`. Otherwise `c` = how many keys from `k[1]` on (at most 7) come
/// before the first that is -1.0 or not below 10.0 (`k[j] == -1.0` tested
/// first, then `k[j] < 10.0`, so NaN stops too); the result is `c` if `c <
/// n` (signed), else -1.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`, `o` spilled to its slot `+0x18`.
/// Leaves `a0 = o`; past the count test `t6` = the flag and `at`; past the
/// flag test `f14 = f2 = 10.0` and `f4 = k[0]`; for the key tests `f12 =
/// -1.0`, `f0` = the last key read, `v1 = c`, and `t7`, `t8` from the
/// loop.
///
/// Domain: any memory contents (compares only).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038294(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x18, g[A0]);
    call(imports::func_80017DAC, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = lw(m, g[SP], 0x18);
    if (g[V0] as i64) > 0 {
        g[T6] = li(0x800A_0000);
        g[T6] = lw(m, g[T6], 0x3FE8);
        g[AT] = li(0x4120_0000);
        if g[T6] == 0 {
            g[V0] = 0;
        } else {
            ctx.fpr[14].set_u32l(g[AT] as u32);
            ctx.fpr[4].set_u32l(lw(m, g[A0], 0x1C) as u32);
            g[AT] = li(0xBF80_0000);
            ctx.fpr[2].set_u32l(ctx.fpr[14].u32l());
            if !(ctx.fpr[14].fl() < ctx.fpr[4].fl()) {
                // c: the keys from k[1] before the first that is -1.0 or
                // not below 10.0, at most 7. Every exit ends in v1 -= 1.
                ctx.fpr[12].set_u32l(g[AT] as u32);
                ctx.fpr[0].set_u32l(lw(m, g[A0], 0x20) as u32);
                g[V1] = 1;
                if ctx.fpr[12].fl() != ctx.fpr[0].fl() && ctx.fpr[0].fl() < ctx.fpr[14].fl() {
                    g[V1] = addu(g[V1], 1);
                    loop {
                        g[AT] = slt(g[V1], 8);
                        g[T7] = sll(g[V1], 2);
                        if g[AT] == 0 {
                            break;
                        }
                        g[T8] = addu(g[A0], g[T7]);
                        ctx.fpr[0].set_u32l(lw(m, g[T8], 0x1C) as u32);
                        if ctx.fpr[12].fl() == ctx.fpr[0].fl() || !(ctx.fpr[0].fl() < ctx.fpr[2].fl()) {
                            break;
                        }
                        g[V1] = addu(g[V1], 1);
                    }
                }
                g[V1] = addu(g[V1], u64::MAX);
                g[AT] = slt(g[V1], g[V0]);
                if g[AT] != 0 {
                    g[V0] = g[V1];
                } else {
                    g[V0] = u64::MAX;
                }
            } else {
                g[V0] = u64::MAX;
            }
        }
    } else {
        g[V0] = u64::MAX;
    }
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80038D5C(pp)` (append a matrix, **guess**: the projection): with
/// `dl = *pp`, `mtx` = the next ring entry ([`func_80033E08`]) filled from
/// the 4x4 at `0x80112E20` ([`func_800344F4`]): `dl[0] = 0xDA380007`,
/// `dl[1] = mtx` (`gSPMatrix`), `*pp = dl + 8`.
///
/// Frame (`sp - 0x20`): `ra` at `+0x14`, `dl` at `+0x18`, `mtx` at
/// `+0x1C`, `pp` spilled to its slot `+0x20`. Leaves `v0 = dl`, `v1 = dl +
/// 8`, `t7 = 0xDA380007`, `t8 = mtx`, `t9 = pp`, and the callees'
/// registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80038D5C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x20, g[A0]);
    g[V1] = lw(m, g[A0], 0);
    sw(m, g[SP], 0x18, g[V1]);
    call(imports::func_80033E08, m, ctx);
    let g = &mut ctx.gpr;
    g[A1] = li(0x8011_0000);
    sw(m, g[SP], 0x1C, g[V0]);
    g[A1] = addu(g[A1], 0x2E20);
    g[A0] = g[V0];
    call(imports::func_800344F4, m, ctx);
    let g = &mut ctx.gpr;
    g[V0] = lw(m, g[SP], 0x18);
    g[T7] = li(0xDA38_0007);
    sw(m, g[V0], 0, g[T7]);
    g[T8] = lw(m, g[SP], 0x1C);
    g[V1] = addu(g[V0], 8);
    sw(m, g[V0], 4, g[T8]);
    g[T9] = lw(m, g[SP], 0x20);
    sw(m, g[T9], 0, g[V1]);
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x20);
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

/// `func_8003D49C(x, y)` (**guess**: a screen position as fractions): `*x
/// = f32(f64(h0) / 320.0)` and `*y = f32(f64(h1) / 240.0)` with `h0`, `h1`
/// the signed halfwords at `0x80114470`/`0x80114472` (divisions in double,
/// rounded to nearest).
///
/// Leaves `v0 = 0x80114470`, `t6`/`t7` = the halfwords, `at = 0x406E0000`,
/// and the doubles in `f4`..`f18`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003D49C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = li(0x8011_4470);
    g[T6] = lh(m, g[V0], 0);
    g[AT] = li(0x4074_0000);
    f[8].set_u32h(g[AT] as u32); // f9
    f[4].set_u32l(g[T6] as u32);
    f[8].set_u32l(0 as u32);
    g[AT] = li(0x406E_0000);
    f[6].set_d(f64::from(f[4].u32l() as i32));
    f[10].set_d(f[6].d() / f[8].d());
    f[6].set_u32l(0 as u32);
    f[6].set_u32h(g[AT] as u32); // f7
    f[16].set_fl(fpu::cvt_s_d(f[10].d(), fpu::NEAREST));
    sw(m, g[A0], 0, u64::from(f[16].u32l()));
    g[T7] = lh(m, g[V0], 2);
    f[18].set_u32l(g[T7] as u32);
    f[4].set_d(f64::from(f[18].u32l() as i32));
    f[8].set_d(f[4].d() / f[6].d());
    f[10].set_fl(fpu::cvt_s_d(f[8].d(), fpu::NEAREST));
    sw(m, g[A1], 0, u64::from(f[10].u32l()));
}

/// `func_8003E0A0(o, sx, sy)` with the floats `sx`, `sy` in `a1`/`a2`
/// (**guess**: scroll a texture by a speed): if `o` isn't null, set bit 15
/// of `[o]`; then with `t = [o + 8]` (if not null), for the halfword pair
/// `u = o + 4`, `w = t + 4` and then `o + 6`, `t + 6`: `*u = trunc(f32(*u)
/// + s * f32(*w))` (halfwords signed, a C cast truncated to 16 bits), then
/// `*u -= *w` if `*w < *u`, then `*u += *w` if `*u < 0` (each re-read).
///
/// Leaves `f12 = sx`, `f14 = sy`, `v0 = t`, `v1` = the last value, `a1` =
/// the last size, and the temporaries of the second axis.
///
/// Domain: `sx`, `sy` and the products not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003E0A0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A1] as u32);
    f[14].set_u32l(g[A2] as u32);
    if g[A0] != 0 {
        g[T6] = lw(m, g[A0], 0);
        g[V0] = lw(m, g[A0], 8);
        g[T7] = g[T6] | 0x8000;
        sw(m, g[A0], 0, g[T7]);
        if g[V0] != 0 {
            g[T9] = lh(m, g[V0], 4);
            g[T8] = lh(m, g[A0], 4);
            f[8].set_u32l(g[T9] as u32);
            f[4].set_u32l(g[T8] as u32);
            f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
            f[6].set_fl(fpu::cvt_s_w(f[4].u32l(), fpu::NEAREST));
            f[16].set_fl(f[12].fl() * f[10].fl());
            f[18].set_fl(f[6].fl() + f[16].fl());
            f[4].set_u32l(fpu::trunc_w_s(f[18].fl()));
            g[T1] = s32(f[4].u32l());
            sh(m, g[A0], 4, g[T1]);
            g[V1] = lh(m, g[A0], 4);
            g[A1] = lh(m, g[V0], 4);
            g[AT] = slt(g[A1], g[V1]);
            g[T2] = subu(g[V1], g[A1]);
            if g[AT] != 0 {
                sh(m, g[A0], 4, g[T2]);
                g[V1] = lh(m, g[A0], 4);
            }
            // L_8003E110
            if (g[V1] as i64) >= 0 {
                g[T6] = lh(m, g[V0], 6);
            } else {
                g[T3] = lh(m, g[V0], 4);
                g[T4] = addu(g[V1], g[T3]);
                sh(m, g[A0], 4, g[T4]);
                g[T6] = lh(m, g[V0], 6);
            }
            // L_8003E128
            g[T5] = lh(m, g[A0], 6);
            f[6].set_u32l(g[T6] as u32);
            f[8].set_u32l(g[T5] as u32);
            f[16].set_fl(fpu::cvt_s_w(f[6].u32l(), fpu::NEAREST));
            f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
            f[18].set_fl(f[14].fl() * f[16].fl());
            f[4].set_fl(f[10].fl() + f[18].fl());
            f[8].set_u32l(fpu::trunc_w_s(f[4].fl()));
            g[T8] = s32(f[8].u32l());
            sh(m, g[A0], 6, g[T8]);
            g[V1] = lh(m, g[A0], 6);
            g[A1] = lh(m, g[V0], 6);
            g[AT] = slt(g[A1], g[V1]);
            g[T9] = subu(g[V1], g[A1]);
            if g[AT] != 0 {
                sh(m, g[A0], 6, g[T9]);
                g[V1] = lh(m, g[A0], 6);
            }
            // L_8003E170
            if (g[V1] as i64) < 0 {
                g[T0] = lh(m, g[V0], 6);
                g[T1] = addu(g[V1], g[T0]);
                sh(m, g[A0], 6, g[T1]);
            }
        }
    }
    // L_8003E184
}

/// `func_8003E18C(p, _, sx, sy)` with the floats in `a2`/`a3`:
/// [`func_8003E0A0`]`([p], sx, sy)`. The second argument is spilled to its
/// slot and not used.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`, `p` and the second argument
/// spilled to their slots `+0x18`/`+0x1C`. Leaves `f12 = sx`, `f14 = sy`
/// (from the moves) and the callee's registers.
///
/// Domain: the callee's.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003E18C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    ctx.fpr[12].set_u32l(g[A2] as u32);
    ctx.fpr[14].set_u32l(g[A3] as u32);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x1C, g[A1]);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x18, g[A0]);
    g[A1] = s32(ctx.fpr[12].u32l());
    g[A2] = s32(ctx.fpr[14].u32l());
    g[A0] = lw(m, g[A0], 0);
    call(imports::func_8003E0A0, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
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

/// `func_8004110C(o, v)`: copy the floats `+0x20..+0x5C` to `+0x224..+0x260`
/// and `+0x108..+0x144` to `+0x264..+0x2A0` (16 words each, loaded and
/// stored six at a time in the compiler's order), and `[o + 0x7C] = v`
/// (**guess**: saving a pose).
///
/// Leaves `f4`..`f18` = the last six words.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8004110C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A0], 0x20) as u32);
    f[6].set_u32l(lw(m, g[A0], 0x24) as u32);
    f[8].set_u32l(lw(m, g[A0], 0x28) as u32);
    sw(m, g[A0], 0x224, u64::from(f[4].u32l()));
    sw(m, g[A0], 0x228, u64::from(f[6].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0x3C) as u32);
    f[4].set_u32l(lw(m, g[A0], 0x38) as u32);
    f[10].set_u32l(lw(m, g[A0], 0x2C) as u32);
    f[16].set_u32l(lw(m, g[A0], 0x30) as u32);
    f[18].set_u32l(lw(m, g[A0], 0x34) as u32);
    sw(m, g[A0], 0x22C, u64::from(f[8].u32l()));
    sw(m, g[A0], 0x240, u64::from(f[6].u32l()));
    sw(m, g[A0], 0x23C, u64::from(f[4].u32l()));
    sw(m, g[A0], 0x230, u64::from(f[10].u32l()));
    sw(m, g[A0], 0x234, u64::from(f[16].u32l()));
    sw(m, g[A0], 0x238, u64::from(f[18].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x4C) as u32);
    f[16].set_u32l(lw(m, g[A0], 0x48) as u32);
    f[10].set_u32l(lw(m, g[A0], 0x44) as u32);
    f[4].set_u32l(lw(m, g[A0], 0x50) as u32);
    f[6].set_u32l(lw(m, g[A0], 0x54) as u32);
    f[8].set_u32l(lw(m, g[A0], 0x40) as u32);
    sw(m, g[A0], 0x250, u64::from(f[18].u32l()));
    sw(m, g[A0], 0x24C, u64::from(f[16].u32l()));
    sw(m, g[A0], 0x248, u64::from(f[10].u32l()));
    sw(m, g[A0], 0x254, u64::from(f[4].u32l()));
    sw(m, g[A0], 0x258, u64::from(f[6].u32l()));
    sw(m, g[A0], 0x244, u64::from(f[8].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x58) as u32);
    f[10].set_u32l(lw(m, g[A0], 0x5C) as u32);
    f[6].set_u32l(lw(m, g[A0], 0x114) as u32);
    f[4].set_u32l(lw(m, g[A0], 0x110) as u32);
    f[16].set_u32l(lw(m, g[A0], 0x108) as u32);
    f[18].set_u32l(lw(m, g[A0], 0x10C) as u32);
    sw(m, g[A0], 0x25C, u64::from(f[8].u32l()));
    sw(m, g[A0], 0x260, u64::from(f[10].u32l()));
    sw(m, g[A0], 0x270, u64::from(f[6].u32l()));
    sw(m, g[A0], 0x26C, u64::from(f[4].u32l()));
    sw(m, g[A0], 0x264, u64::from(f[16].u32l()));
    sw(m, g[A0], 0x268, u64::from(f[18].u32l()));
    f[18].set_u32l(lw(m, g[A0], 0x124) as u32);
    f[16].set_u32l(lw(m, g[A0], 0x120) as u32);
    f[4].set_u32l(lw(m, g[A0], 0x128) as u32);
    f[6].set_u32l(lw(m, g[A0], 0x12C) as u32);
    f[10].set_u32l(lw(m, g[A0], 0x11C) as u32);
    f[8].set_u32l(lw(m, g[A0], 0x118) as u32);
    sw(m, g[A0], 0x280, u64::from(f[18].u32l()));
    sw(m, g[A0], 0x27C, u64::from(f[16].u32l()));
    sw(m, g[A0], 0x284, u64::from(f[4].u32l()));
    sw(m, g[A0], 0x288, u64::from(f[6].u32l()));
    sw(m, g[A0], 0x278, u64::from(f[10].u32l()));
    sw(m, g[A0], 0x274, u64::from(f[8].u32l()));
    f[8].set_u32l(lw(m, g[A0], 0x130) as u32);
    f[10].set_u32l(lw(m, g[A0], 0x134) as u32);
    f[6].set_u32l(lw(m, g[A0], 0x144) as u32);
    f[4].set_u32l(lw(m, g[A0], 0x140) as u32);
    f[16].set_u32l(lw(m, g[A0], 0x138) as u32);
    f[18].set_u32l(lw(m, g[A0], 0x13C) as u32);
    sw(m, g[A0], 0x7C, g[A1]);
    sw(m, g[A0], 0x28C, u64::from(f[8].u32l()));
    sw(m, g[A0], 0x290, u64::from(f[10].u32l()));
    sw(m, g[A0], 0x2A0, u64::from(f[6].u32l()));
    sw(m, g[A0], 0x29C, u64::from(f[4].u32l()));
    sw(m, g[A0], 0x294, u64::from(f[16].u32l()));
    sw(m, g[A0], 0x298, u64::from(f[18].u32l()));
}

/// `func_80041214(o)`: with `v = [o + 0x80]`, [`func_8004110C`]`(o, v)` if
/// `v` is 1 or 2, else `[o + 0x7C] = v` (which the callee also stores).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `a1 = v`, `at = 1`, and
/// the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80041214(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8004124C: {
        g[SP] = addu(g[SP], (-0x18i64) as u64);
        sw(m, g[SP], 0x14, g[RA]);
        g[A1] = lw(m, g[A0], 0x80);
        g[AT] = 2;
        let c0 = g[A1] != g[AT];
        g[AT] = 1;
        if c0 && g[A1] != g[AT] {
            sw(m, g[A0], 0x7C, g[A1]);
            g[RA] = lw(m, g[SP], 0x14);
            break 'b_8004124C;
        }
        call(imports::func_8004110C, m, ctx);
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x14);
    }
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], 0x18);
}

/// `func_800454A8(o, list)` (**guess**: a race's setup from its objects):
/// `m` = the largest float `[v + 0x108]` over the objects `v` of the
/// zero-terminated `list`, from 0.0 up (`m < x` replaces, so NaN never
/// does; 0.0 for a null or empty list); `[o + 0xB68] = m`, `[o + 0xC] =
/// 0.0`; if `[o + 0x50]` is nonzero, `[o + 0x58] = (f32([[o + 0xC4] + 4])
/// - 1.0) / m`. Then [`func_8000AEFC`]`(5, 1, o + 0xB28, 0)` and colour
/// -103 = `(0, 0, 0, 255)` ([`func_8000AB24`]).
///
/// The loop also tests `a2`, which is 0 there: dead, kept as the C has it.
///
/// Frame (`sp - 0x28`): `s0` (`o`) at `+0x20` and `ra` at `+0x24`,
/// restored sign-extended; the fifth argument at `+0x10`. Leaves `f12 =
/// 0.0`, `f2 = m`, `f0` = the last key, `t7` = the word `[o + 0x50]`, with
/// it `t8`, `t9`, `f4`..`f16` from the division, and the callees'
/// registers.
///
/// Domain: any values (the maximum is never NaN; the division may give
/// infinities or NaN).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800454A8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    ctx.fpr[12].set_u32l(0);
    g[SP] = addu(g[SP], (-0x28i64) as u64);
    sw(m, g[SP], 0x20, g[S0]);
    g[S0] = g[A0];
    sw(m, g[SP], 0x24, g[RA]);
    g[A2] = 0;
    ctx.fpr[2].set_u32l(ctx.fpr[12].u32l());
    // m = the largest [v + 0x108] from 0.0 (the a2 test is dead: a2 = 0).
    if g[A1] != 0 {
        g[T6] = lw(m, g[A1], 0);
        g[A0] = g[A1];
        if g[T6] != 0 {
            g[V1] = lw(m, g[A1], 0);
            ctx.fpr[0].set_u32l(lw(m, g[V1], 0x108) as u32);
            loop {
                if ctx.fpr[2].fl() < ctx.fpr[0].fl() {
                    ctx.fpr[2].set_u32l(ctx.fpr[0].u32l());
                }
                g[V1] = lw(m, g[A0], 4);
                g[A0] = addu(g[A0], 4);
                if g[V1] == 0 || g[A2] != 0 {
                    break;
                }
                ctx.fpr[0].set_u32l(lw(m, g[V1], 0x108) as u32);
            }
        }
    }
    g[T7] = lw(m, g[S0], 0x50);
    sw(m, g[S0], 0xB68, u64::from(ctx.fpr[2].u32l()));
    sw(m, g[S0], 0xC, u64::from(ctx.fpr[12].u32l()));
    g[A0] = 5;
    if g[T7] != 0 {
        g[T8] = lw(m, g[S0], 0xC4);
        g[AT] = li(0x3F80_0000);
        ctx.fpr[8].set_u32l(g[AT] as u32);
        g[T9] = lw(m, g[T8], 4);
        ctx.fpr[4].set_u32l(g[T9] as u32);
        ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fpu::NEAREST));
        ctx.fpr[10].set_fl(ctx.fpr[6].fl() - ctx.fpr[8].fl());
        ctx.fpr[16].set_fl(ctx.fpr[10].fl() / ctx.fpr[2].fl());
        sw(m, g[S0], 0x58, u64::from(ctx.fpr[16].u32l()));
    }
    g[A1] = 1;
    g[A2] = addu(g[S0], 0xB28);
    g[A3] = 0;
    call(imports::func_8000AEFC, m, ctx);
    let g = &mut ctx.gpr;
    g[T0] = 0xFF;
    sw(m, g[SP], 0x10, g[T0]);
    g[A0] = (-0x67i64) as u64;
    g[A1] = 0;
    g[A2] = 0;
    g[A3] = 0;
    call(imports::func_8000AB24, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x24);
    g[S0] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x28);
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

/// `func_80045DA0(o)` (a node group, **guess**: 12 parts): `g = o +
/// 0x1B30` gets a node header of type 0x5064 ([`func_80018324`]), `[g +
/// 0x14] = 12`, `[g + 0x18] = o + 0x16E0` (its child list), `[o + 0xBC4] =
/// g`; then for `i = 0..5`, `j = 0..1`, the child `n = o + 0x1710 + 0xB0i
/// + 0x58j` gets a header of type 0xD065 (identity transform), `[n + 0x14]
/// = [n + 0x18] = 0`, and the list entry `[o + 0x16E0 + 8i + 4j] = n`.
///
/// Frame (`sp - 0x40`): `s0`..`s7`, `fp` and `ra` at `+0x18..+0x3C`, all
/// restored sign-extended. Leaves `a0` = the last child, `a1 = 0xD065`,
/// `t6 = 12`, `t7 = o + 0x16E0`, and the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80045DA0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    sw(m, g[SP], 0x1C, g[S1]);
    sw(m, g[SP], 0x18, g[S0]);
    g[S0] = g[A0];
    sw(m, g[SP], 0x3C, g[RA]);
    g[S1] = addu(g[A0], 0x1B30);
    sw(m, g[SP], 0x38, g[FP]);
    sw(m, g[SP], 0x34, g[S7]);
    sw(m, g[SP], 0x30, g[S6]);
    sw(m, g[SP], 0x2C, g[S5]);
    sw(m, g[SP], 0x28, g[S4]);
    sw(m, g[SP], 0x24, g[S3]);
    sw(m, g[SP], 0x20, g[S2]);
    g[A0] = g[S1];
    g[A1] = 0x5064;
    call(imports::func_80018324, m, ctx);
    let g = &mut ctx.gpr;
    g[T6] = 0xC;
    g[T7] = addu(g[S0], 0x16E0);
    sw(m, g[S1], 0x14, g[T6]);
    sw(m, g[S1], 0x18, g[T7]);
    sw(m, g[S0], 0xBC4, g[S1]);
    g[S5] = g[S0];
    g[S6] = 0;
    g[S7] = g[S0];
    g[FP] = 0xC;
    g[S4] = 0xB0;
    loop {
        let g = &mut ctx.gpr;
        g[S2] = 0;
        g[S0] = addu(g[S5], 0x1710);
        g[S3] = g[S7];
        loop {
            let g = &mut ctx.gpr;
            g[S1] = g[S0];
            g[A0] = g[S0];
            g[A1] = 0xD065;
            call(imports::func_80018324, m, ctx);
            let g = &mut ctx.gpr;
            g[S2] = addu(g[S2], 0x58);
            sw(m, g[S1], 0x14, 0);
            sw(m, g[S1], 0x18, 0);
            g[S0] = addu(g[S0], 0x58);
            g[S3] = addu(g[S3], 4);
            sw(m, g[S3], 0x16DC, g[S1]);
            if g[S2] == g[S4] {
                break;
            }
        }
        let g = &mut ctx.gpr;
        g[S6] = addu(g[S6], 2);
        g[S5] = addu(g[S5], 0xB0);
        g[S7] = addu(g[S7], 8);
        if g[S6] == g[FP] {
            break;
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x3C);
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[S3] = lw(m, g[SP], 0x24);
    g[S4] = lw(m, g[SP], 0x28);
    g[S5] = lw(m, g[SP], 0x2C);
    g[S6] = lw(m, g[SP], 0x30);
    g[S7] = lw(m, g[SP], 0x34);
    g[FP] = lw(m, g[SP], 0x38);
    g[SP] = addu(g[SP], 0x40);
}

/// `func_80045E80(o)` (a node group, **guess**: 18 parts): as
/// [`func_80045DA0`] with `g = o + 0x16C4`, `[g + 0x14] = 18`, the list at
/// `o + 0x104C`, `[o + 0xBB8] = g`, and the children `n = o + 0x1094 +
/// 0x108i + 0x58j` for `i = 0..5`, `j = 0..2`, listed at `o + 0x104C + 12i
/// + 4j`.
///
/// Frame (`sp - 0x40`): `s0`..`s7`, `fp` and `ra` at `+0x18..+0x3C`, all
/// restored sign-extended. Leaves `a0` = the last child, `a1 = 0xD065`,
/// `t6 = 18`, `t7 = o + 0x104C`, and the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80045E80(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    sw(m, g[SP], 0x1C, g[S1]);
    sw(m, g[SP], 0x18, g[S0]);
    g[S0] = g[A0];
    sw(m, g[SP], 0x3C, g[RA]);
    g[S1] = addu(g[A0], 0x16C4);
    sw(m, g[SP], 0x38, g[FP]);
    sw(m, g[SP], 0x34, g[S7]);
    sw(m, g[SP], 0x30, g[S6]);
    sw(m, g[SP], 0x2C, g[S5]);
    sw(m, g[SP], 0x28, g[S4]);
    sw(m, g[SP], 0x24, g[S3]);
    sw(m, g[SP], 0x20, g[S2]);
    g[A0] = g[S1];
    g[A1] = 0x5064;
    call(imports::func_80018324, m, ctx);
    let g = &mut ctx.gpr;
    g[T6] = 0x12;
    g[T7] = addu(g[S0], 0x104C);
    sw(m, g[S1], 0x14, g[T6]);
    sw(m, g[S1], 0x18, g[T7]);
    sw(m, g[S0], 0xBB8, g[S1]);
    g[S5] = g[S0];
    g[S6] = 0;
    g[S7] = g[S0];
    g[FP] = 0x12;
    g[S4] = 0x108;
    loop {
        let g = &mut ctx.gpr;
        g[S2] = 0;
        g[S0] = addu(g[S5], 0x1094);
        g[S3] = g[S7];
        loop {
            let g = &mut ctx.gpr;
            g[S1] = g[S0];
            g[A0] = g[S0];
            g[A1] = 0xD065;
            call(imports::func_80018324, m, ctx);
            let g = &mut ctx.gpr;
            g[S2] = addu(g[S2], 0x58);
            sw(m, g[S1], 0x14, 0);
            sw(m, g[S1], 0x18, 0);
            g[S0] = addu(g[S0], 0x58);
            g[S3] = addu(g[S3], 4);
            sw(m, g[S3], 0x1048, g[S1]);
            if g[S2] == g[S4] {
                break;
            }
        }
        let g = &mut ctx.gpr;
        g[S6] = addu(g[S6], 3);
        g[S5] = addu(g[S5], 0x108);
        g[S7] = addu(g[S7], 0xC);
        if g[S6] == g[FP] {
            break;
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x3C);
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[S3] = lw(m, g[SP], 0x24);
    g[S4] = lw(m, g[SP], 0x28);
    g[S5] = lw(m, g[SP], 0x2C);
    g[S6] = lw(m, g[SP], 0x30);
    g[S7] = lw(m, g[SP], 0x34);
    g[FP] = lw(m, g[SP], 0x38);
    g[SP] = addu(g[SP], 0x40);
}

/// `func_80046764()` (clear the scene lists, **guess**): zeroes the words
/// `0x8011A2A8..0x8011A504` (the root's child list, [`func_80046974`]) and
/// `0x8011A508..0x8011A764` (151 each: three singles each, then four per
/// step of both), then clears the object table ([`func_80005B80`]).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `at = 0x80120000`, `v0 =
/// a0 = 0x8011A504`, `v1 = 0x8011A764` and the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80046764(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x8012_0000);
    sw(m, g[AT], -0x5AF8, 0);
    g[AT] = li(0x8012_0000);
    sw(m, g[AT], -0x5D58, 0);
    g[AT] = li(0x8012_0000);
    sw(m, g[AT], -0x5AF4, 0);
    g[AT] = li(0x8012_0000);
    sw(m, g[AT], -0x5D54, 0);
    g[AT] = li(0x8012_0000);
    sw(m, g[AT], -0x5AF0, 0);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[AT] = li(0x8012_0000);
    g[V1] = li(0x8012_0000);
    g[V0] = li(0x8012_0000);
    g[A0] = li(0x8012_0000);
    sw(m, g[SP], 0x14, g[RA]);
    g[A0] = addu(g[A0], (-0x5AFCi64) as u64);
    g[V0] = addu(g[V0], (-0x5D4Ci64) as u64);
    g[V1] = addu(g[V1], (-0x5AECi64) as u64);
    sw(m, g[AT], -0x5D50, 0);
    loop {
        g[V0] = addu(g[V0], 0x10);
        sw(m, g[V1], 4, 0);
        sw(m, g[V0], -0xC, 0);
        sw(m, g[V1], 8, 0);
        sw(m, g[V0], -8, 0);
        sw(m, g[V1], 0xC, 0);
        sw(m, g[V0], -4, 0);
        g[V1] = addu(g[V1], 0x10);
        sw(m, g[V1], -0x10, 0);
        sw(m, g[V0], -0x10, 0);
        if g[V0] == g[A0] {
            break;
        }
    }
    call(imports::func_80005B80, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80046870()` (unlist the second table's objects, **guess**): for
/// each slot `0x8011A51C..0x8011A764` (146 words, in order) holding a
/// pointer `p`: every word of the root's list `0x8011A2A8..0x8011A504`
/// (151 words, in order) equal to `[p]` becomes 0 (`[p]` read again after
/// each clear), then the slot becomes 0. Then clears the object table
/// ([`func_80005B80`]).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `a2 = a3 = 0x8011A764`,
/// `a1 = 0x8011A504`, `a0` = the last slot's word, `t6`..`t8` =
/// `0x80120000` or the three singles, `v0`, `v1`, `t9`..`t2` from the last
/// list scan, `at`, and the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80046870(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    g[A2] = li(0x8012_0000);
    g[A3] = li(0x8012_0000);
    g[A1] = li(0x8012_0000);
    sw(m, g[SP], 0x14, g[RA]);
    g[A1] = addu(g[A1], (-0x5AFCi64) as u64);
    g[A3] = addu(g[A3], (-0x589Ci64) as u64);
    g[A2] = addu(g[A2], (-0x5AE4i64) as u64);
    g[A0] = lw(m, g[A2], 0);
    loop {
        g[T6] = li(0x8012_0000);
        g[T7] = li(0x8012_0000);
        g[T8] = li(0x8012_0000);
        if g[A0] != 0 {
            g[V1] = lw(m, g[A0], 0);
            g[T6] = lw(m, g[T6], -0x5D58);
            g[V0] = li(0x8011_A2B4);
            g[AT] = li(0x8012_0000);
            if g[T6] == g[V1] {
                sw(m, g[AT], -0x5D58, 0);
                g[V1] = lw(m, g[A0], 0);
            }
            g[T7] = lw(m, g[T7], -0x5D54);
            g[AT] = li(0x8012_0000);
            if g[T7] == g[V1] {
                sw(m, g[AT], -0x5D54, 0);
                g[V1] = lw(m, g[A0], 0);
            }
            g[T8] = lw(m, g[T8], -0x5D50);
            g[AT] = li(0x8012_0000);
            if g[T8] == g[V1] {
                sw(m, g[AT], -0x5D50, 0);
            }
            g[V1] = lw(m, g[A0], 0);
            loop {
                g[T9] = lw(m, g[V0], 0);
                if g[T9] != g[V1] {
                    g[T0] = lw(m, g[V0], 4);
                } else {
                    sw(m, g[V0], 0, 0);
                    g[V1] = lw(m, g[A0], 0);
                    g[T0] = lw(m, g[V0], 4);
                }
                if g[T0] != g[V1] {
                    g[T1] = lw(m, g[V0], 8);
                } else {
                    sw(m, g[V0], 4, 0);
                    g[V1] = lw(m, g[A0], 0);
                    g[T1] = lw(m, g[V0], 8);
                }
                if g[T1] != g[V1] {
                    g[T2] = lw(m, g[V0], 0xC);
                } else {
                    sw(m, g[V0], 8, 0);
                    g[V1] = lw(m, g[A0], 0);
                    g[T2] = lw(m, g[V0], 0xC);
                }
                if g[T2] != g[V1] {
                    g[V0] = addu(g[V0], 0x10);
                } else {
                    sw(m, g[V0], 0xC, 0);
                    g[V0] = addu(g[V0], 0x10);
                }
                if g[V0] == g[A1] {
                    break;
                }
                g[V1] = lw(m, g[A0], 0);
            }
            sw(m, g[A2], 0, 0);
        }
        g[A2] = addu(g[A2], 4);
        if g[A2] == g[A3] {
            break;
        }
        g[A0] = lw(m, g[A2], 0);
    }
    call(imports::func_80005B80, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80046974()` (the scene root, **guess**): the node at `0x8011A288`
/// gets a header of type 0x5064 ([`func_80018324`]), then `[0x8011A29C] =
/// 0x97` (151 children) and `[0x8011A2A0] = 0x8011A2A8` (the list
/// [`func_80046764`] clears).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `t6 = 0x97`, `t7 =
/// 0x8011A2A8`, `at = 0x80120000` and the callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80046974(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    g[A0] = li(0x8011_A288);
    g[A1] = 0x5064;
    call(imports::func_80018324, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[T6] = 0x97;
    g[AT] = li(0x8012_0000);
    sw(m, g[AT], -0x5D64, g[T6]);
    g[T7] = li(0x8011_A2A8);
    g[AT] = li(0x8012_0000);
    sw(m, g[AT], -0x5D60, g[T7]);
    g[SP] = addu(g[SP], 0x18);
}

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

/// `func_80047A78(o, x, y, f, _, u, v)` (a bar of two records, **guess**:
/// a gauge filled to `f`), `f` a float in `a3`, `u`, `v` floats on the
/// stack at `[sp + 0x14]`/`[sp + 0x18]`: with `(A, B)` = `(0.375, 3.75)`
/// if `[o + 8] == 8`, else `((u - 2.0) * 0.125, v * 0.125)` and then `x
/// += 3`, `y += 1`; and `c = 1.0 - f`: two records with the ids `k + 0x8D`
/// (s16) from the counter `k = [0x800A4BB8]`, which each takes and
/// increments:
/// 1. [`func_8000A920`]`(id, 1)`, [`func_8000AA04`]`(id, s16(x), s16(y))`,
///    [`func_8000AAC0`]`(id, A, c * B)`, colour `(0, 0, 0, 255)`
///    ([`func_8000AB24`]);
/// 2. `(id, 1)`, `(id, s16(x), s16(trunc(f32(y) + c * v)))` (a C cast),
///    `(id, A, B * f)`, colour `(u(c * 255.0), u(255.0 * f), 0, 255)` with
///    `u` IDO's float to unsigned idiom ([`fpu::to_unsigned_s`]; its second
///    path, from 2^31, is dead under the oracle), the low bytes kept.
///
/// Frame (`sp - 0x40`): `s0` (the id) at `+0x20` and `ra` at `+0x24`,
/// restored sign-extended; `x`, `y` and `f` spilled to their slots
/// `+0x44`..`+0x4C` (`x`, `y` after the adjustment), `c` at `+0x2C`,
/// `s16(x)` at `+0x30`, `B` at `+0x34`, `A` at `+0x38`, the fifth argument
/// at `+0x10`. Leaves `a1`, `a2` = the colour bytes, `t7 = t9 = 0` (the
/// saved FCR31s), `t1 = 0xFF`, `f0 = 255.0`, the products' FPRs and the
/// callee's registers.
///
/// Domain: `f`, `v` and, unless `[o + 8] == 8`, `u` not NaN, and the
/// products and sums not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80047A78(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let mut fcr31 = fpu::NEAREST;
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    sw(m, g[SP], 0x24, g[RA]);
    sw(m, g[SP], 0x20, g[S0]);
    sw(m, g[SP], 0x44, g[A1]);
    sw(m, g[SP], 0x4C, g[A3]);
    g[T6] = lw(m, g[A0], 8);
    g[AT] = 8;
    g[A3] = g[A1];
    g[V1] = li(0x800A_0000);
    if g[T6] != g[AT] {
        g[AT] = li(0x3E00_0000);
        ctx.fpr[2].set_u32l(g[AT] as u32);
        g[AT] = li(0x4000_0000);
        ctx.fpr[0].set_u32l(lw(m, g[SP], 0x54) as u32);
        ctx.fpr[8].set_u32l(g[AT] as u32);
        ctx.fpr[16].set_u32l(lw(m, g[SP], 0x58) as u32);
        g[A3] = addu(g[A3], 3);
        ctx.fpr[0].set_fl(ctx.fpr[0].fl() - ctx.fpr[8].fl());
        g[A2] = addu(g[A2], 1);
        ctx.fpr[10].set_fl(ctx.fpr[0].fl() * ctx.fpr[2].fl());
        ctx.fpr[18].set_fl(ctx.fpr[16].fl() * ctx.fpr[2].fl());
        sw(m, g[SP], 0x38, u64::from(ctx.fpr[10].u32l()));
        sw(m, g[SP], 0x34, u64::from(ctx.fpr[18].u32l()));
    } else {
        g[AT] = li(0x3EC0_0000);
        ctx.fpr[4].set_u32l(g[AT] as u32);
        g[AT] = li(0x4070_0000);
        ctx.fpr[6].set_u32l(g[AT] as u32);
        sw(m, g[SP], 0x38, u64::from(ctx.fpr[4].u32l()));
        sw(m, g[SP], 0x34, u64::from(ctx.fpr[6].u32l()));
    }
    g[V1] = addu(g[V1], 0x4BB8);
    g[V0] = lw(m, g[V1], 0);
    g[A1] = 1;
    sw(m, g[SP], 0x48, g[A2]);
    g[S0] = addu(g[V0], 0x8D);
    g[T7] = sll(g[S0], 16);
    g[S0] = sra(g[T7], 16);
    g[A0] = sll(g[S0], 16);
    g[T0] = sra(g[A0], 16);
    g[T9] = addu(g[V0], 1);
    sw(m, g[V1], 0, g[T9]);
    g[A0] = g[T0];
    sw(m, g[SP], 0x44, g[A3]);
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A3] = lw(m, g[SP], 0x44);
    g[A0] = sll(g[S0], 16);
    g[T1] = sra(g[A0], 16);
    g[A1] = sll(g[A3], 16);
    g[T2] = sra(g[A1], 16);
    g[A1] = g[T2];
    sw(m, g[SP], 0x30, g[T2]);
    g[A0] = g[T1];
    g[A2] = lh(m, g[SP], 0x4A);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[AT] = li(0x3F80_0000);
    ctx.fpr[4].set_u32l(g[AT] as u32);
    ctx.fpr[6].set_u32l(lw(m, g[SP], 0x4C) as u32);
    ctx.fpr[8].set_u32l(lw(m, g[SP], 0x34) as u32);
    g[A0] = sll(g[S0], 16);
    ctx.fpr[0].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
    g[T3] = sra(g[A0], 16);
    g[A0] = g[T3];
    g[A1] = lw(m, g[SP], 0x38);
    ctx.fpr[10].set_fl(ctx.fpr[0].fl() * ctx.fpr[8].fl());
    sw(m, g[SP], 0x2C, u64::from(ctx.fpr[0].u32l()));
    g[A2] = s32(ctx.fpr[10].u32l());
    call(imports::func_8000AAC0, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = sll(g[S0], 16);
    g[T4] = sra(g[A0], 16);
    g[T5] = 0xFF;
    sw(m, g[SP], 0x10, g[T5]);
    g[A0] = g[T4];
    g[A1] = 0;
    g[A2] = 0;
    g[A3] = 0;
    call(imports::func_8000AB24, m, ctx);
    let g = &mut ctx.gpr;
    g[V1] = li(0x800A_4BB8);
    g[V0] = lw(m, g[V1], 0);
    g[A1] = 1;
    g[S0] = addu(g[V0], 0x8D);
    g[T6] = sll(g[S0], 16);
    g[S0] = sra(g[T6], 16);
    g[A0] = sll(g[S0], 16);
    g[T9] = sra(g[A0], 16);
    g[T8] = addu(g[V0], 1);
    sw(m, g[V1], 0, g[T8]);
    g[A0] = g[T9];
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[T1] = lw(m, g[SP], 0x48);
    ctx.fpr[16].set_u32l(lw(m, g[SP], 0x2C) as u32);
    ctx.fpr[18].set_u32l(lw(m, g[SP], 0x58) as u32);
    ctx.fpr[6].set_u32l(g[T1] as u32);
    g[A0] = sll(g[S0], 16);
    ctx.fpr[4].set_fl(ctx.fpr[16].fl() * ctx.fpr[18].fl());
    g[T0] = sra(g[A0], 16);
    g[A0] = g[T0];
    g[A1] = lh(m, g[SP], 0x32);
    ctx.fpr[8].set_fl(fpu::cvt_s_w(ctx.fpr[6].u32l(), fcr31));
    ctx.fpr[10].set_fl(ctx.fpr[8].fl() + ctx.fpr[4].fl());
    ctx.fpr[16].set_u32l(fpu::trunc_w_s(ctx.fpr[10].fl()));
    g[A2] = s32(ctx.fpr[16].u32l());
    g[T3] = sll(g[A2], 16);
    g[A2] = sra(g[T3], 16);
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    ctx.fpr[18].set_u32l(lw(m, g[SP], 0x34) as u32);
    ctx.fpr[6].set_u32l(lw(m, g[SP], 0x4C) as u32);
    g[A0] = sll(g[S0], 16);
    g[T5] = sra(g[A0], 16);
    ctx.fpr[8].set_fl(ctx.fpr[18].fl() * ctx.fpr[6].fl());
    g[A0] = g[T5];
    g[A1] = lw(m, g[SP], 0x38);
    g[A2] = s32(ctx.fpr[8].u32l());
    call(imports::func_8000AAC0, m, ctx);
    let g = &mut ctx.gpr;
    // The colour: u(c * 255.0) (save t7, tmp a1), then u(255.0 * f) (save
    // t9, tmp a2). Each idiom's restoring ctc1 follows it directly, and
    // the second product comes after the first restore, so to_unsigned_s
    // restores in place.
    g[AT] = li(0x437F_0000);
    ctx.fpr[0].set_u32l(g[AT] as u32);
    ctx.fpr[4].set_u32l(lw(m, g[SP], 0x2C) as u32);
    g[A1] = 1;
    g[A0] = sll(g[S0], 16);
    ctx.fpr[10].set_fl(ctx.fpr[4].fl() * ctx.fpr[0].fl());
    g[T6] = sra(g[A0], 16);
    g[A0] = g[T6];
    g[AT] = li(0x4F00_0000);
    g[A3] = 0;
    g[T1] = 0xFF;
    fpu::to_unsigned_s(g, &mut ctx.fpr, &mut fcr31, T7, A1, 16, 10);
    ctx.fpr[18].set_u32l(lw(m, g[SP], 0x4C) as u32);
    g[A2] = 1;
    g[T8] = g[A1] & 0xFF;
    ctx.fpr[6].set_fl(ctx.fpr[0].fl() * ctx.fpr[18].fl());
    g[A1] = g[T8];
    g[AT] = li(0x4F00_0000);
    fpu::to_unsigned_s(g, &mut ctx.fpr, &mut fcr31, T9, A2, 8, 6);
    g[T0] = g[A2] & 0xFF;
    g[A2] = g[T0];
    sw(m, g[SP], 0x10, g[T1]);
    call(imports::func_8000AB24, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x24);
    g[S0] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x40);
}

/// `func_8004BAC8(o, full)` (the track-select grid, **guess**: 4 circuits
/// of 7 tracks): records `0x7F..0x9E` are initialised with the image
/// `[o + 0xC8]` ([`func_8000A44C`]). Then, if `full` is nonzero, for each
/// circuit `c = 0..3` and track `k = 0..6`, with `id = s16(0x60 + 7c +
/// k)` and `q = s16((h >> 2k) % 4)` (`h` the s16 at `0x80113E6C + 2c`, the
/// shift mod 32, a C remainder):
/// 1. record `id`: [`func_8000A44C`]`(id, [o + 0xC0])`,
///    [`func_8000AC34`]`(id, 0x8000)`, colour (alpha 0xFE) `32 FF FF`,
///    `44 FF 3E`, `A3 BE 11` or `9D 59 20` by `c`, then `80 80 80` if
///    [`func_8002DAD0`]`(o, c, k)` is 0 (locked, **guess**);
/// 2. if the byte `[o + 0x6C]` is nonzero and `q` is 1, 2 or 3: record `id`
///    is initialised again with the image `[o + 0xBC]`, `[o + 0xB8]` or
///    `[o + 0xB4]` (QUIRK: which resets the flag and colour just set);
/// 3. record `id + 0x1C`: [`func_8000A44C`]`(.., [o + 0xC4])`,
///    [`func_8000AC34`]`(.., 0x8000)`, colour `A3 BE 11 FE`.
///
/// Frame (`sp - 0x78`): `s0`..`s7`, `fp` and `ra` at `+0x20..+0x44`, all
/// restored sign-extended; the fifth argument at `+0x10`, `c` (as an s8
/// word) at `+0x4C`, `7c` at `+0x58`, `0x80113E60 + 2c` at `+0x60`. Leaves
/// the last callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8004BAC8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x78i64) as u64);
    sw(m, g[SP], 0x28, g[S2]);
    sw(m, g[SP], 0x24, g[S1]);
    sw(m, g[SP], 0x20, g[S0]);
    g[S1] = g[A1];
    g[S2] = g[A0];
    sw(m, g[SP], 0x44, g[RA]);
    sw(m, g[SP], 0x40, g[FP]);
    sw(m, g[SP], 0x3C, g[S7]);
    sw(m, g[SP], 0x38, g[S6]);
    sw(m, g[SP], 0x34, g[S5]);
    sw(m, g[SP], 0x30, g[S4]);
    sw(m, g[SP], 0x2C, g[S3]);
    g[S0] = 0x7F;
    g[A0] = sll(g[S0], 16);
    loop {
        let g = &mut ctx.gpr;
        g[T6] = sra(g[A0], 16);
        g[A0] = g[T6];
        g[A1] = lw(m, g[S2], 0xC8);
        call(imports::func_8000A44C, m, ctx);
        let g = &mut ctx.gpr;
        g[S0] = addu(g[S0], 1);
        g[T7] = sll(g[S0], 16);
        g[S0] = sra(g[T7], 16);
        g[AT] = slt(g[S0], 0x9F);
        if g[AT] == 0 {
            break;
        }
        g[A0] = sll(g[S0], 16);
    }
    let g = &mut ctx.gpr;
    g[S4] = 0;
    if g[S1] != 0 {
        g[V0] = li(0x8011_3E60);
        sw(m, g[SP], 0x60, g[V0]);
        sw(m, g[SP], 0x58, 0);
        g[FP] = 2;
        g[S7] = 1;
        loop {
            let g = &mut ctx.gpr;
            g[V1] = lw(m, g[SP], 0x58);
            g[T9] = sll(g[S4], 24);
            g[T0] = sra(g[T9], 24);
            sw(m, g[SP], 0x4C, g[T0]);
            g[S3] = 0;
            g[S5] = 0;
            g[S6] = addu(g[V1], 0x60);
            loop {
                let g = &mut ctx.gpr;
                g[V0] = lw(m, g[SP], 0x60);
                g[S1] = sll(g[S6], 16);
                g[T5] = sra(g[S1], 16);
                g[T1] = lh(m, g[V0], 0xC);
                g[A0] = sll(g[T5], 16);
                g[T6] = sra(g[A0], 16);
                g[S0] = srav(g[T1], g[S5]);
                g[V1] = lw(m, g[SP], 0x58);
                g[T2] = g[S0] & 3;
                if (g[S0] as i64) < 0 && g[T2] != 0 {
                    g[T2] = addu(g[T2], (-4i64) as u64);
                }
                g[T3] = sll(g[T2], 16);
                g[S0] = sra(g[T3], 16);
                g[A0] = g[T6];
                g[S1] = g[T5];
                g[A1] = lw(m, g[S2], 0xC0);
                call(imports::func_8000A44C, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = sll(g[S1], 16);
                g[T7] = sra(g[A0], 16);
                g[A0] = g[T7];
                g[A1] = 0x8000;
                call(imports::func_8000AC34, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = sll(g[S1], 16);
                if g[S4] == 0 {
                    g[T8] = sra(g[A0], 16);
                    g[T9] = 0xFE;
                    sw(m, g[SP], 0x10, g[T9]);
                    g[A0] = g[T8];
                    g[A1] = 0x32;
                    g[A2] = 0xFF;
                    g[A3] = 0xFF;
                    call(imports::func_8000AB24, m, ctx);
                    let g = &mut ctx.gpr;
                    g[A0] = g[S2];
                } else {
                    g[A0] = sll(g[S1], 16);
                    if g[S4] == g[S7] {
                        g[T0] = sra(g[A0], 16);
                        g[T1] = 0xFE;
                        sw(m, g[SP], 0x10, g[T1]);
                        g[A0] = g[T0];
                        g[A1] = 0x44;
                        g[A2] = 0xFF;
                        g[A3] = 0x3E;
                        call(imports::func_8000AB24, m, ctx);
                        let g = &mut ctx.gpr;
                        g[A0] = g[S2];
                    } else {
                        g[A0] = sll(g[S1], 16);
                        if g[S4] == g[FP] {
                            g[T2] = sra(g[A0], 16);
                            g[T3] = 0xFE;
                            sw(m, g[SP], 0x10, g[T3]);
                            g[A0] = g[T2];
                            g[A1] = 0xA3;
                            g[A2] = 0xBE;
                            g[A3] = 0x11;
                            call(imports::func_8000AB24, m, ctx);
                            let g = &mut ctx.gpr;
                            g[A0] = g[S2];
                        } else {
                            g[AT] = 3;
                            g[A0] = sll(g[S1], 16);
                            if g[S4] == g[AT] {
                                g[T4] = sra(g[A0], 16);
                                g[T5] = 0xFE;
                                sw(m, g[SP], 0x10, g[T5]);
                                g[A0] = g[T4];
                                g[A1] = 0x9D;
                                g[A2] = 0x59;
                                g[A3] = 0x20;
                                call(imports::func_8000AB24, m, ctx);
                                let g = &mut ctx.gpr;
                                g[A0] = g[S2];
                            } else {
                                g[A0] = g[S2];
                            }
                        }
                    }
                }
                let g = &mut ctx.gpr;
                g[A1] = lb(m, g[SP], 0x4F);
                g[A2] = g[S3] & 0xFF;
                call(imports::func_8002DAD0, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = sll(g[S1], 16);
                if g[V0] == 0 {
                    g[T6] = sra(g[A0], 16);
                    g[T7] = 0xFE;
                    sw(m, g[SP], 0x10, g[T7]);
                    g[A0] = g[T6];
                    g[A1] = 0x80;
                    g[A2] = 0x80;
                    g[A3] = 0x80;
                    call(imports::func_8000AB24, m, ctx);
                }
                let g = &mut ctx.gpr;
                g[T8] = lb(m, g[S2], 0x6C);
                if g[T8] == 0 {
                    g[S0] = addu(g[S1], 0x1C);
                } else if g[S0] == 0 {
                    g[S0] = addu(g[S1], 0x1C);
                } else {
                    g[A0] = sll(g[S1], 16);
                    if g[S0] == g[S7] {
                        g[T9] = sra(g[A0], 16);
                        g[A0] = g[T9];
                        g[A1] = lw(m, g[S2], 0xBC);
                        call(imports::func_8000A44C, m, ctx);
                        let g = &mut ctx.gpr;
                        g[S0] = addu(g[S1], 0x1C);
                    } else {
                        g[A0] = sll(g[S1], 16);
                        if g[S0] == g[FP] {
                            g[T0] = sra(g[A0], 16);
                            g[A0] = g[T0];
                            g[A1] = lw(m, g[S2], 0xB8);
                            call(imports::func_8000A44C, m, ctx);
                            let g = &mut ctx.gpr;
                            g[S0] = addu(g[S1], 0x1C);
                        } else {
                            g[AT] = 3;
                            g[A0] = sll(g[S1], 16);
                            if g[S0] == g[AT] {
                                g[T1] = sra(g[A0], 16);
                                g[A0] = g[T1];
                                g[A1] = lw(m, g[S2], 0xB4);
                                call(imports::func_8000A44C, m, ctx);
                                let g = &mut ctx.gpr;
                                g[S0] = addu(g[S1], 0x1C);
                            } else {
                                g[S0] = addu(g[S1], 0x1C);
                            }
                        }
                    }
                }
                let g = &mut ctx.gpr;
                g[T2] = sll(g[S0], 16);
                g[S0] = sra(g[T2], 16);
                g[A0] = sll(g[S0], 16);
                g[T4] = sra(g[A0], 16);
                g[A0] = g[T4];
                g[A1] = lw(m, g[S2], 0xC4);
                call(imports::func_8000A44C, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = sll(g[S0], 16);
                g[T5] = sra(g[A0], 16);
                g[A0] = g[T5];
                g[A1] = 0x8000;
                call(imports::func_8000AC34, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = sll(g[S0], 16);
                g[T6] = sra(g[A0], 16);
                g[T7] = 0xFE;
                sw(m, g[SP], 0x10, g[T7]);
                g[A0] = g[T6];
                g[A1] = 0xA3;
                g[A2] = 0xBE;
                g[A3] = 0x11;
                call(imports::func_8000AB24, m, ctx);
                let g = &mut ctx.gpr;
                g[S3] = addu(g[S3], 1);
                g[AT] = 7;
                g[S5] = addu(g[S5], 2);
                g[S6] = addu(g[S6], 1);
                if g[S3] == g[AT] {
                    break;
                }
            }
            let g = &mut ctx.gpr;
            g[T8] = lw(m, g[SP], 0x60);
            g[T0] = lw(m, g[SP], 0x58);
            g[S4] = addu(g[S4], 1);
            g[AT] = 4;
            g[T9] = addu(g[T8], 2);
            g[T1] = addu(g[T0], 7);
            sw(m, g[SP], 0x58, g[T1]);
            sw(m, g[SP], 0x60, g[T9]);
            if g[S4] == g[AT] {
                break;
            }
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x44);
    g[S0] = lw(m, g[SP], 0x20);
    g[S1] = lw(m, g[SP], 0x24);
    g[S2] = lw(m, g[SP], 0x28);
    g[S3] = lw(m, g[SP], 0x2C);
    g[S4] = lw(m, g[SP], 0x30);
    g[S5] = lw(m, g[SP], 0x34);
    g[S6] = lw(m, g[SP], 0x38);
    g[S7] = lw(m, g[SP], 0x3C);
    g[FP] = lw(m, g[SP], 0x40);
    g[SP] = addu(g[SP], 0x78);
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

/// `func_8004E0A0(i)` (menu input for player `i`, **guess**): the
/// newly-set word `N = [0x800A4BA4 + 4i] = 0`; if `[0x800A4BD8]` is
/// nonzero:
/// 1. for each bit `b = 1 << k`, `k = 0..23`, [`func_8004E488`]`(i, H &
///    b, b)` with `H` the held word `[0x800D76F0 + 4i]` (re-read each
///    time), which sets or clears `b` in `S = [0x800A4B94 + 4i]` and
///    collects the newly set bits in `N`;
/// 2. with `w = S` (read once), the direction `dx = [0x800A5208 + 4i]` =
///    0 if bit 18 of `w` (x centred), else -1 with bit 16 (left), else 1
///    with bit 17 (right), else unchanged; `dy = [0x800A5218 + 4i]` the same
///    with bits 19, 14 (up), 15 (down);
/// 3. with `N` re-read and `dx`, `dy` re-read: bit 20 of `N` and `dx ==
///    -1` sets bit 16 of `S` (as `w | 0x10000`), bit 21 and `dx == 1` bit
///    17, bit 22 and `dy == -1` bit 14, bit 23 and `dy == 1` bit 15 (each
///    `S` re-read).
///
/// Frame (`sp - 0x30`): `s0` (`i`), `s1`, `s2` (`4i`) at `+0x18..+0x20`
/// and `ra` at `+0x24`, restored sign-extended; `N`'s address at `+0x28`.
/// Leaves `v0 = w`, `v1` = `S`'s address, `a0 = N`, `a1 = -1`, `a2 = 1`,
/// `a3 = 0x800A5208`, `t0 = 0x800A5218`, the tests' temporaries, and the
/// callee's registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8004E0A0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    'b_8004E474: {
        g[SP] = addu(g[SP], (-0x30i64) as u64);
        sw(m, g[SP], 0x20, g[S2]);
        g[T6] = li(0x800A_4BA4);
        g[S2] = sll(g[A0], 2);
        g[V0] = addu(g[S2], g[T6]);
        sw(m, g[V0], 0, 0);
        g[T7] = li(0x800A_0000);
        g[T7] = lw(m, g[T7], 0x4BD8);
        sw(m, g[SP], 0x18, g[S0]);
        g[S0] = g[A0];
        sw(m, g[SP], 0x24, g[RA]);
        sw(m, g[SP], 0x1C, g[S1]);
        if g[T7] != 0 {
            g[T8] = li(0x800D_76F0);
            g[S1] = addu(g[S2], g[T8]);
            g[A1] = lw(m, g[S1], 0);
            g[A2] = 1;
            sw(m, g[SP], 0x28, g[V0]);
            g[T9] = g[A1] & 1;
            g[A1] = g[T9];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 2;
            g[T1] = g[A1] & 2;
            g[A1] = g[T1];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 4;
            g[T2] = g[A1] & 4;
            g[A1] = g[T2];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 8;
            g[T3] = g[A1] & 8;
            g[A1] = g[T3];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x10;
            g[T4] = g[A1] & 0x10;
            g[A1] = g[T4];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x20;
            g[T5] = g[A1] & 0x20;
            g[A1] = g[T5];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x40;
            g[T6] = g[A1] & 0x40;
            g[A1] = g[T6];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x80;
            g[T7] = g[A1] & 0x80;
            g[A1] = g[T7];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x100;
            g[T8] = g[A1] & 0x100;
            g[A1] = g[T8];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x200;
            g[T9] = g[A1] & 0x200;
            g[A1] = g[T9];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x400;
            g[T1] = g[A1] & 0x400;
            g[A1] = g[T1];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x800;
            g[T2] = g[A1] & 0x800;
            g[A1] = g[T2];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x1000;
            g[T3] = g[A1] & 0x1000;
            g[A1] = g[T3];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x2000;
            g[T4] = g[A1] & 0x2000;
            g[A1] = g[T4];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x4000;
            g[T5] = g[A1] & 0x4000;
            g[A1] = g[T5];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[S1], 0);
            g[A0] = g[S0];
            g[A2] = 0x8000;
            g[T6] = g[A1] & 0x8000;
            g[A1] = g[T6];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[T7] = lw(m, g[S1], 0);
            g[A2] = li(0x1_0000);
            g[A0] = g[S0];
            g[A1] = g[T7] & g[A2];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[T8] = lw(m, g[S1], 0);
            g[A2] = li(0x2_0000);
            g[A0] = g[S0];
            g[A1] = g[T8] & g[A2];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[T9] = lw(m, g[S1], 0);
            g[A2] = li(0x4_0000);
            g[A0] = g[S0];
            g[A1] = g[T9] & g[A2];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[T1] = lw(m, g[S1], 0);
            g[A2] = li(0x8_0000);
            g[A0] = g[S0];
            g[A1] = g[T1] & g[A2];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[T2] = lw(m, g[S1], 0);
            g[A2] = li(0x10_0000);
            g[A0] = g[S0];
            g[A1] = g[T2] & g[A2];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[T3] = lw(m, g[S1], 0);
            g[A2] = li(0x20_0000);
            g[A0] = g[S0];
            g[A1] = g[T3] & g[A2];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[T4] = lw(m, g[S1], 0);
            g[A2] = li(0x40_0000);
            g[A0] = g[S0];
            g[A1] = g[T4] & g[A2];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[T5] = lw(m, g[S1], 0);
            g[A2] = li(0x80_0000);
            g[A0] = g[S0];
            g[A1] = g[T5] & g[A2];
            call(imports::func_8004E488, m, ctx);
            let g = &mut ctx.gpr;
            g[T6] = li(0x800A_4B94);
            g[V1] = addu(g[S2], g[T6]);
            g[V0] = lw(m, g[V1], 0);
            g[A3] = li(0x800A_5208);
            g[T7] = sll(g[V0], 13);
            g[T8] = addu(g[A3], g[S2]);
            if (g[T7] as i64) >= 0 {
                g[T9] = sll(g[V0], 15);
                g[A3] = li(0x800A_0000);
                if (g[T9] as i64) >= 0 {
                    g[T2] = sll(g[V0], 14);
                    g[A3] = li(0x800A_0000);
                    if (g[T2] as i64) < 0 {
                        g[A3] = addu(g[A3], 0x5208);
                        g[T3] = addu(g[A3], g[S2]);
                        g[A2] = 1;
                        sw(m, g[T3], 0, g[A2]);
                    }
                } else {
                    g[A3] = addu(g[A3], 0x5208);
                    g[T1] = addu(g[A3], g[S2]);
                    g[A1] = u64::MAX;
                    sw(m, g[T1], 0, g[A1]);
                }
            } else {
                sw(m, g[T8], 0, 0);
            }
            g[A3] = li(0x800A_0000);
            g[T4] = sll(g[V0], 12);
            g[A3] = addu(g[A3], 0x5208);
            g[A1] = u64::MAX;
            g[A2] = 1;
            if (g[T4] as i64) >= 0 {
                g[T6] = g[V0] & 0x4000;
                g[T0] = li(0x800A_0000);
                if g[T6] == 0 {
                    g[T8] = g[V0] & 0x8000;
                    g[T0] = li(0x800A_0000);
                    if g[T8] != 0 {
                        g[T0] = addu(g[T0], 0x5218);
                        g[T9] = addu(g[T0], g[S2]);
                        sw(m, g[T9], 0, g[A2]);
                    }
                } else {
                    g[T0] = addu(g[T0], 0x5218);
                    g[T7] = addu(g[T0], g[S2]);
                    sw(m, g[T7], 0, g[A1]);
                }
            } else {
                g[T0] = li(0x800A_5218);
                g[T5] = addu(g[T0], g[S2]);
                sw(m, g[T5], 0, 0);
            }
            g[T1] = lw(m, g[SP], 0x28);
            g[T0] = li(0x800A_5218);
            g[A0] = lw(m, g[T1], 0);
            g[T2] = sll(g[A0], 11);
            g[T6] = sll(g[A0], 10);
            if (g[T2] as i64) < 0 {
                g[T3] = addu(g[A3], g[S2]);
                g[T4] = lw(m, g[T3], 0);
                g[AT] = li(0x1_0000);
                g[T5] = g[V0] | g[AT];
                if g[A1] == g[T4] {
                    sw(m, g[V1], 0, g[T5]);
                }
            }
            g[T2] = sll(g[A0], 9);
            if (g[T6] as i64) < 0 {
                g[T7] = addu(g[A3], g[S2]);
                g[T8] = lw(m, g[T7], 0);
                if g[A2] == g[T8] {
                    g[T9] = lw(m, g[V1], 0);
                    g[AT] = li(0x2_0000);
                    g[T1] = g[T9] | g[AT];
                    sw(m, g[V1], 0, g[T1]);
                }
            }
            g[T7] = sll(g[A0], 8);
            if (g[T2] as i64) < 0 {
                g[T3] = addu(g[T0], g[S2]);
                g[T4] = lw(m, g[T3], 0);
                if g[A1] == g[T4] {
                    g[T5] = lw(m, g[V1], 0);
                    g[T6] = g[T5] | 0x4000;
                    sw(m, g[V1], 0, g[T6]);
                }
            }
            g[T8] = addu(g[T0], g[S2]);
            if (g[T7] as i64) < 0 {
                g[T9] = lw(m, g[T8], 0);
                if g[A2] != g[T9] {
                    g[RA] = lw(m, g[SP], 0x24);
                    break 'b_8004E474;
                }
                g[T1] = lw(m, g[V1], 0);
                g[T2] = g[T1] | 0x8000;
                sw(m, g[V1], 0, g[T2]);
            }
        }
        let g = &mut ctx.gpr;
        g[RA] = lw(m, g[SP], 0x24);
    }
    let g = &mut ctx.gpr;
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[SP] = addu(g[SP], 0x30);
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

/// `func_8004F254(o)` (name the players' cameras, **guess**): for the
/// 0x88-byte records `r` at `0x80118F90`, while the count `n` = the byte
/// `[o + 0x71]` (signed, re-read after each message) is above the index:
/// each record whose word `+4` is `"Locl"` or whose flags `+8` have bit 5
/// set gets the message `{"NAsn", j + 1, [r + 0x84]}` (in the frame at
/// `+0x5C`), sent ([`func_8003F99C`]) to the `"cMan"` element with tag `j`
/// ([`func_8003F714`]), `j` counting the messages from 0.
///
/// Frame (`sp - 0x80`): `s0`..`s7`, `fp` and `ra` at `+0x18..+0x3C`, all
/// restored sign-extended; `o` spilled to its slot `+0x80`. Leaves `v0 =
/// n`, `at` = the last test, and the loop's and callees' registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8004F254(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x80i64) as u64);
    sw(m, g[SP], 0x3C, g[RA]);
    sw(m, g[SP], 0x38, g[FP]);
    sw(m, g[SP], 0x34, g[S7]);
    sw(m, g[SP], 0x30, g[S6]);
    sw(m, g[SP], 0x2C, g[S5]);
    sw(m, g[SP], 0x28, g[S4]);
    sw(m, g[SP], 0x24, g[S3]);
    sw(m, g[SP], 0x20, g[S2]);
    sw(m, g[SP], 0x1C, g[S1]);
    sw(m, g[SP], 0x18, g[S0]);
    sw(m, g[SP], 0x80, g[A0]);
    g[V0] = lb(m, g[A0], 0x71);
    g[S1] = 0;
    g[S2] = 1;
    g[S3] = 0;
    if (g[V0] as i64) > 0 {
        g[S0] = li(0x8012_0000);
        g[FP] = li(0x800A_0000);
        g[S6] = li(0x634D_0000);
        g[S5] = li(0x4E41_0000);
        g[S4] = li(0x4C6F_636C);
        g[S5] = g[S5] | 0x736E;
        g[S6] = g[S6] | 0x616E;
        g[FP] = addu(g[FP], 0x4BE0);
        g[S0] = addu(g[S0], (-0x7070i64) as u64);
        g[S7] = addu(g[SP], 0x5C);
        loop {
            let g = &mut ctx.gpr;
            'b_8004F31C: {
                g[T7] = lw(m, g[S0], 4);
                g[A0] = g[S6];
                g[A1] = g[S1];
                if g[S4] == g[T7] {
                    g[T0] = lw(m, g[S0], 0x84);
                } else {
                    g[T8] = lw(m, g[S0], 8);
                    g[T9] = g[T8] & 0x20;
                    if g[T9] == 0 {
                        g[S3] = addu(g[S3], 1);
                        break 'b_8004F31C;
                    }
                    g[T0] = lw(m, g[S0], 0x84);
                }
                sw(m, g[SP], 0x5C, g[S5]);
                sw(m, g[SP], 0x60, g[S2]);
                sw(m, g[SP], 0x64, g[T0]);
                call(imports::func_8003F714, m, ctx);
                let g = &mut ctx.gpr;
                g[A0] = g[V0];
                g[A1] = g[S7];
                call(imports::func_8003F99C, m, ctx);
                let g = &mut ctx.gpr;
                g[T1] = lw(m, g[SP], 0x80);
                g[S1] = addu(g[S1], 1);
                g[S2] = addu(g[S2], 1);
                g[V0] = lb(m, g[T1], 0x71);
                g[S3] = addu(g[S3], 1);
            }
            let g = &mut ctx.gpr;
            g[AT] = slt(g[S3], g[V0]);
            g[S0] = addu(g[S0], 0x88);
            if g[AT] == 0 {
                break;
            }
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x3C);
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[S3] = lw(m, g[SP], 0x24);
    g[S4] = lw(m, g[SP], 0x28);
    g[S5] = lw(m, g[SP], 0x2C);
    g[S6] = lw(m, g[SP], 0x30);
    g[S7] = lw(m, g[SP], 0x34);
    g[FP] = lw(m, g[SP], 0x38);
    g[SP] = addu(g[SP], 0x80);
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

/// `func_8005058C(p, q, k, keep, first)` (**guess**: start a camera
/// transition, stepped by [`func_8005065C`]). The 4x4 matrices at
/// `0x80118D60`, `DA0`, `DE0`, `E20`, `E60` and `EA0` have their translation
/// row `t(M)` at `+0x30`. Unless the fifth argument `first` is nonzero, the
/// two current matrices are saved: `E60 = E20`, `DA0 = D60`
/// ([`func_800156DC`]). Then the targets `t(DE0) = p` (at `0x80118E10`) and
/// `t(EA0) = q` ([`func_80015288`]), and the halfword `[0x800A4BC0] = k`. If
/// `(i16)k == 3` and `keep` is nonzero, `t(DE0) = (t(D60) - t(E20)) + q`
/// through a frame vec3 at `sp + 0x1C` ([`func_8001535C`],
/// [`func_80015328`]).
///
/// Frame (`sp - 0x28`): `ra` at `+0x14`, the four arguments spilled to their
/// slots `+0x28..+0x34`. Leaves `t6 = first`, `t7 = k`, `t8 = (i16)k`, `t9 =
/// keep` (both sign-extended words), `v0 = 0x800A4BC0`, `at = 3`, `a0`..`a2`
/// from the last call made, and the callees' registers.
///
/// Domain: canonical `p`, `q`; with the sum taken, no NaN among the three
/// vec3s' components or the differences (`inf - inf`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8005058C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x28i64) as u64);
    g[T6] = lw(m, g[SP], 0x38);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x28, g[A0]);
    sw(m, g[SP], 0x2C, g[A1]);
    sw(m, g[SP], 0x30, g[A2]);
    sw(m, g[SP], 0x34, g[A3]);
    if g[T6] == 0 {
        g[A0] = li(0x8011_8E60);
        g[A1] = li(0x8011_8E20);
        call(imports::func_800156DC, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = li(0x8011_8DA0);
        g[A1] = li(0x8011_8D60);
        call(imports::func_800156DC, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[A0] = li(0x8011_8E10);
    g[A1] = lw(m, g[SP], 0x28);
    call(imports::func_80015288, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = li(0x8011_8ED0);
    g[A1] = lw(m, g[SP], 0x2C);
    call(imports::func_80015288, m, ctx);
    let g = &mut ctx.gpr;
    g[T7] = lw(m, g[SP], 0x30);
    g[V0] = li(0x800A_4BC0);
    sh(m, g[V0], 0, g[T7]);
    g[T8] = lh(m, g[V0], 0);
    g[AT] = 3;
    g[T9] = lw(m, g[SP], 0x34);
    if g[T8] == g[AT] {
        g[A0] = addu(g[SP], 0x1C);
        if g[T9] != 0 {
            g[A1] = li(0x8011_8D90);
            g[A2] = li(0x8011_8E50);
            call(imports::func_8001535C, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = li(0x8011_8E10);
            g[A1] = li(0x8011_8ED0);
            g[A2] = addu(g[SP], 0x1C);
            call(imports::func_80015328, m, ctx);
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x28);
}

/// `func_8005065C(o)` (**guess**: step the camera transition that
/// [`func_8005058C`] starts). The 4x4 matrices at `0x80118D60`, `DA0`, `DE0`,
/// `E20`, `E60` and `EA0` have their translation row `t(M)` at `+0x30`; the
/// state at `0x8011AC24` is `D = (dx, dy, dz)`, a second delta `E` at `+0xC`,
/// `t` at `+0x18` and `T` at `+0x1C`.
///
/// 1. `t(D60)` and `t(E20)` are copied to frame vec3s at `sp + 0x44`/`+0x38`
///    ([`func_80015288`]); nothing reads them.
/// 2. If the word `S = [0x800A4BD4]` is nonzero (a new transition): `D =
///    t(EA0) - t(E60)`, `E = t(DE0) - t(DA0)`, `t = 0.0`, `T = 0.5`; `T` = the
///    float at `0x800AB350` (0.3 in the ROM) instead if `-500 < dx < 500`,
///    `-500 < dy < 500` (`dy` re-read) and `[o + 0x38] == 1`; `dx` is stored
///    a second time when `dx < 500`. Then `S = 0`.
/// 3. If `t < T` (re-read): `t += [0x80120BF8]`, then `t = T` if `T < t`; `s
///    = t / T`; `t(E20) = (t(EA0) - t(E60)) * s + t(E60)` component by
///    component (the differences taken again, not `D`), and unless the
///    halfword `[0x800A4BC0]` is 3, `t(D60) = (t(DE0) - t(DA0)) * s +
///    t(DA0)`.
/// 4. Else (also for NaN): the halfword `[0x800A4BC0] = 5`, then 0 if `[o +
///    8] == 8`; `S = 1`; if the word `[0x800A4BD0]` is nonzero it becomes 0,
///    `[0x800A4BC4]` becomes `1` if it was 0 and else 0, and if that is 1 with
///    `[o + 0x38] == 1` and `[o + 0x34] != 3`, the halfword `[0x800A219C] =
///    1`. Then `E60 = E20`, `DA0 = D60` ([`func_800156DC`]), and
///    `[0x800A4BC4] = 0` unless `[0x800A2198] == -1` or the halfword
///    `[0x800A219C]` is nonzero.
///
/// Frame (`sp - 0x68`): `ra` at `+0x14`, `o` spilled to its slot `+0x68`, the
/// two copies, and in step 3 the `y` and `z` differences of `E20`'s at
/// `+0x24`/`+0x1C`. Leaves `a2 = o`, `v1 = 0x80118DE0`, `a3 = 0x80118DA0`
/// (`0x80120000` after step 4), `a1` = 1 or the last matrix read, `f14 = s`
/// and the differences, products and sums in `f0`..`f18`, and the callees'
/// registers.
///
/// Domain: canonical `o`; no NaN operand: the translations read, `t`, `T`
/// and `[0x80120BF8]` when step 3 runs, and no NaN from `inf - inf`, `0 / 0`
/// or `0 * inf` on the way.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8005065C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x68i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    g[A2] = g[A0];
    g[A1] = li(0x8011_8D90);
    sw(m, g[SP], 0x68, g[A2]);
    g[A0] = addu(g[SP], 0x44);
    call(imports::func_80015288, m, ctx);
    let g = &mut ctx.gpr;
    g[A1] = li(0x8011_8E50);
    g[A0] = addu(g[SP], 0x38);
    call(imports::func_80015288, m, ctx);
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[A0] = li(0x800A_4BD4);
    g[T6] = lw(m, g[A0], 0);
    g[A2] = lw(m, g[SP], 0x68);
    g[V0] = li(0x8011_8EA0);
    if g[T6] != 0 {
        // A new transition: D, E, t = 0 and T.
        g[AT] = li(0x43FA_0000);
        f[2].set_u32l(g[AT] as u32);
        g[AT] = li(0x8012_0000);
        f[6].set_u32l(lw(m, g[AT], -0x7170) as u32);
        f[4].set_u32l(lw(m, g[V0], 0x30) as u32);
        f[10].set_u32l(lw(m, g[AT], -0x716C) as u32);
        f[8].set_u32l(lw(m, g[V0], 0x34) as u32);
        f[0].set_fl(f[4].fl() - f[6].fl());
        g[AT] = li(0x8012_0000);
        f[4].set_fl(f[8].fl() - f[10].fl());
        g[A3] = li(0x8011_8DA0);
        g[V1] = li(0x8011_8DE0);
        sw(m, g[AT], -0x53D8, u64::from(f[4].u32l()));
        g[AT] = li(0x8012_0000);
        f[8].set_u32l(lw(m, g[AT], -0x7168) as u32);
        f[6].set_u32l(lw(m, g[V0], 0x38) as u32);
        g[AT] = li(0x8012_0000);
        let below = f[0].fl() < f[2].fl();
        f[10].set_fl(f[6].fl() - f[8].fl());
        sw(m, g[AT], -0x53D4, u64::from(f[10].u32l()));
        f[6].set_u32l(lw(m, g[A3], 0x30) as u32);
        f[4].set_u32l(lw(m, g[V1], 0x30) as u32);
        g[AT] = li(0x8012_0000);
        f[8].set_fl(f[4].fl() - f[6].fl());
        sw(m, g[AT], -0x53D0, u64::from(f[8].u32l()));
        f[4].set_u32l(lw(m, g[A3], 0x34) as u32);
        f[10].set_u32l(lw(m, g[V1], 0x34) as u32);
        g[AT] = li(0x8012_0000);
        f[6].set_fl(f[10].fl() - f[4].fl());
        sw(m, g[AT], -0x53CC, u64::from(f[6].u32l()));
        f[10].set_u32l(lw(m, g[A3], 0x38) as u32);
        f[8].set_u32l(lw(m, g[V1], 0x38) as u32);
        f[6].set_u32l(0);
        g[AT] = li(0x8012_0000);
        f[4].set_fl(f[8].fl() - f[10].fl());
        sw(m, g[AT], -0x53C8, u64::from(f[4].u32l()));
        g[AT] = li(0x8012_0000);
        sw(m, g[AT], -0x53C4, u64::from(f[6].u32l()));
        g[AT] = li(0x3F00_0000);
        f[8].set_u32l(g[AT] as u32);
        g[AT] = li(0x8012_0000);
        sw(m, g[AT], -0x53C0, u64::from(f[8].u32l()));
        g[AT] = li(0x8012_0000);
        sw(m, g[AT], -0x53DC, u64::from(f[0].u32l()));
        if below {
            g[AT] = li(0xC3FA_0000);
            f[12].set_u32l(g[AT] as u32);
            g[AT] = li(0x8012_0000);
            sw(m, g[AT], -0x53DC, u64::from(f[0].u32l()));
            let near = f[12].fl() < f[0].fl();
            g[AT] = li(0x8012_0000);
            if near {
                f[10].set_u32l(lw(m, g[AT], -0x53D8) as u32);
                if f[10].fl() < f[2].fl() && f[12].fl() < f[10].fl() {
                    g[T7] = lw(m, g[A2], 0x38);
                    g[A1] = 1;
                    g[AT] = li(0x800B_0000);
                    if g[A1] == g[T7] {
                        f[4].set_u32l(lw(m, g[AT], -0x4CB0) as u32);
                        g[AT] = li(0x8012_0000);
                        sw(m, g[AT], -0x53C0, u64::from(f[4].u32l()));
                    }
                }
            }
        }
        sw(m, g[A0], 0, 0);
    }
    g[AT] = li(0x8012_0000);
    f[6].set_u32l(lw(m, g[AT], -0x53C4) as u32);
    g[AT] = li(0x8012_0000);
    f[8].set_u32l(lw(m, g[AT], -0x53C0) as u32);
    let running = f[6].fl() < f[8].fl();
    g[A3] = li(0x8011_8DA0);
    g[V1] = li(0x8011_8DE0);
    g[A1] = 1;
    g[V0] = li(0x800A_0000);
    if running {
        // t += dt, clamped to T; E20 (and D60) move by s = t / T.
        g[AT] = li(0x8012_0000);
        f[10].set_u32l(lw(m, g[AT], 0xBF8) as u32);
        g[AT] = li(0x8012_0000);
        f[4].set_fl(f[6].fl() + f[10].fl());
        sw(m, g[AT], -0x53C4, u64::from(f[4].u32l()));
        g[AT] = li(0x8012_0000);
        f[2].set_u32l(lw(m, g[AT], -0x7170) as u32);
        g[AT] = li(0x8012_0000);
        f[6].set_u32l(lw(m, g[AT], -0x7130) as u32);
        g[AT] = li(0x8012_0000);
        f[12].set_u32l(lw(m, g[AT], -0x716C) as u32);
        g[AT] = li(0x8012_0000);
        f[10].set_u32l(lw(m, g[AT], -0x712C) as u32);
        g[AT] = li(0x8012_0000);
        f[18].set_fl(f[6].fl() - f[2].fl());
        f[16].set_u32l(lw(m, g[AT], -0x7168) as u32);
        g[AT] = li(0x8012_0000);
        f[6].set_fl(f[10].fl() - f[12].fl());
        f[10].set_u32l(lw(m, g[AT], -0x7128) as u32);
        g[AT] = li(0x8012_0000);
        let over = f[8].fl() < f[4].fl();
        sw(m, g[SP], 0x24, u64::from(f[6].u32l()));
        f[6].set_fl(f[10].fl() - f[16].fl());
        sw(m, g[SP], 0x1C, u64::from(f[6].u32l()));
        if over {
            sw(m, g[AT], -0x53C4, u64::from(f[8].u32l()));
        }
        g[AT] = li(0x8012_0000);
        f[10].set_u32l(lw(m, g[AT], -0x53C4) as u32);
        g[AT] = li(0x8012_0000);
        f[6].set_u32l(lw(m, g[AT], -0x53C0) as u32);
        g[A1] = li(0x8011_8E20);
        f[14].set_fl(f[10].fl() / f[6].fl());
        f[10].set_u32l(lw(m, g[SP], 0x24) as u32);
        g[V0] = li(0x800A_4BC0);
        g[T8] = lh(m, g[V0], 0);
        g[AT] = 3;
        f[4].set_fl(f[18].fl() * f[14].fl());
        f[8].set_fl(f[4].fl() + f[2].fl());
        f[6].set_fl(f[10].fl() * f[14].fl());
        sw(m, g[A1], 0x30, u64::from(f[8].u32l()));
        f[8].set_u32l(lw(m, g[SP], 0x1C) as u32);
        f[10].set_fl(f[8].fl() * f[14].fl());
        f[4].set_fl(f[6].fl() + f[12].fl());
        sw(m, g[A1], 0x34, u64::from(f[4].u32l()));
        f[6].set_fl(f[10].fl() + f[16].fl());
        sw(m, g[A1], 0x38, u64::from(f[6].u32l()));
        if g[T8] != g[AT] {
            f[0].set_u32l(lw(m, g[A3], 0x30) as u32);
            f[4].set_u32l(lw(m, g[V1], 0x30) as u32);
            f[2].set_u32l(lw(m, g[A3], 0x34) as u32);
            f[12].set_u32l(lw(m, g[A3], 0x38) as u32);
            f[8].set_fl(f[4].fl() - f[0].fl());
            f[4].set_u32l(lw(m, g[V1], 0x34) as u32);
            g[A1] = li(0x8011_8D60);
            f[10].set_fl(f[8].fl() * f[14].fl());
            f[8].set_fl(f[4].fl() - f[2].fl());
            f[4].set_u32l(lw(m, g[V1], 0x38) as u32);
            f[6].set_fl(f[10].fl() + f[0].fl());
            f[10].set_fl(f[8].fl() * f[14].fl());
            f[8].set_fl(f[4].fl() - f[12].fl());
            sw(m, g[A1], 0x30, u64::from(f[6].u32l()));
            f[6].set_fl(f[10].fl() + f[2].fl());
            f[10].set_fl(f[8].fl() * f[14].fl());
            sw(m, g[A1], 0x34, u64::from(f[6].u32l()));
            f[6].set_fl(f[10].fl() + f[12].fl());
            sw(m, g[A1], 0x38, u64::from(f[6].u32l()));
        }
    } else {
        // Done: the mode halfword, S = 1, the toggles, save the matrices.
        g[V0] = li(0x800A_4BC0);
        g[T9] = 5;
        sh(m, g[V0], 0, g[T9]);
        g[T1] = lw(m, g[A2], 8);
        g[AT] = 8;
        g[V1] = li(0x800A_4BD0);
        if g[T1] == g[AT] {
            sh(m, g[V0], 0, 0);
        }
        g[T2] = lw(m, g[V1], 0);
        sw(m, g[A0], 0, g[A1]);
        g[A0] = li(0x8011_8E60);
        if g[T2] != 0 {
            g[T0] = li(0x800A_4BC4);
            g[V0] = lw(m, g[T0], 0);
            sw(m, g[V1], 0, 0);
            g[T3] = sltu(g[V0], 1);
            sw(m, g[T0], 0, g[T3]);
            g[T4] = lw(m, g[A2], 0x38);
            if g[A1] == g[T4] {
                g[T5] = lw(m, g[A2], 0x34);
                g[AT] = 3;
                if g[T5] != g[AT] {
                    g[AT] = li(0x800A_0000);
                    if g[T3] != 0 {
                        sh(m, g[AT], 0x219C, g[A1]);
                    }
                }
            }
        }
        g[A1] = li(0x8011_8E20);
        call(imports::func_800156DC, m, ctx);
        let g = &mut ctx.gpr;
        g[A3] = li(0x8012_0000);
        g[A0] = li(0x8011_8DA0);
        g[A1] = li(0x8011_8D60);
        call(imports::func_800156DC, m, ctx);
        let g = &mut ctx.gpr;
        g[T7] = lw(m, li(0x800A_0000), 0x2198);
        g[AT] = u64::MAX;
        g[T8] = li(0x800A_0000);
        if g[T7] != g[AT] {
            g[T8] = lh(m, g[T8], 0x219C);
            g[AT] = li(0x800A_0000);
            if g[T8] == 0 {
                sw(m, g[AT], 0x4BC4, 0);
            }
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x68);
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

/// `func_80052134(p)` (**guess**: a progress figure with a fraction that
/// wraps): with `o = [p + 0x84]`, `a = [o + 0xE8]`, `b = [o + 0xE0]`
/// (floats) and the word `n = [p + 0x78]`: `d = a - b`, negated if `a <
/// b`; if `0.5 < d`, `d = 1 - d` (the distance on a circle of
/// circumference 1). Returns `f0 = r = (f32(n) + a) - d`, or 0.0 if `r < 0`.
///
/// Leaves `v0 = o`, `t6 = n`, `at = 0x3F800000`, `f2 = f0`, `f4 = 0.5`,
/// `f8` = `n`'s bits, `f10 = f32(n)`, `f12 = d`, `f14 = 0.0`, `f16 = f32(n)
/// + a`, and `f6 = 1.0` if `d` was folded.
///
/// Domain: `p`, `o` canonical; no NaN operands: `a`, `b` not NaN, and not
/// the same infinity (their difference would reach the last subtraction).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80052134(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = lw(m, g[A0], 0x84);
    g[AT] = li(0x3F00_0000);
    f[4].set_u32l(g[AT] as u32);
    f[0].set_u32l(lw(m, g[V0], 0xE8) as u32);
    f[2].set_u32l(lw(m, g[V0], 0xE0) as u32);
    g[AT] = li(0x3F80_0000);
    // f12 = d = |a - b|, folded above 0.5.
    let below = f[0].fl() < f[2].fl();
    f[12].set_fl(f[0].fl() - f[2].fl());
    if below {
        f[12].set_fl(-f[12].fl());
    }
    if f[4].fl() < f[12].fl() {
        f[6].set_u32l(g[AT] as u32);
        f[12].set_fl(f[6].fl() - f[12].fl());
    }
    g[T6] = lw(m, g[A0], 0x78);
    f[14].set_u32l(0);
    f[8].set_u32l(g[T6] as u32);
    f[10].set_fl(fpu::cvt_s_w(f[8].u32l(), fpu::NEAREST));
    f[16].set_fl(f[10].fl() + f[0].fl());
    f[2].set_fl(f[16].fl() - f[12].fl());
    if f[2].fl() < f[14].fl() {
        f[2].set_u32l(f[14].u32l());
    }
    f[0].set_u32l(f[2].u32l());
}

/// `func_800521C0(p)` (**guess**: a racer's progress figure): if bit 1 of
/// `[p + 8]` is set, `f0 = K - [p + 0x74]` with `K` the float at
/// `0x800ACE38` (10000.0 in the ROM); else [`func_80052134`]`(p)`.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`. Leaves `t6 = [p + 8]`, `t7` its bit
/// 1, `at = 0x800B0000`, and with the bit set `f4 = K`, `f6 = [p + 0x74]`;
/// else the callee's registers.
///
/// Domain: canonical `p`; with the bit set, neither `K` nor `[p + 0x74]` NaN;
/// else [`func_80052134`]'s.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800521C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    g[T6] = lw(m, g[A0], 8);
    g[AT] = li(0x800B_0000);
    g[T7] = g[T6] & 2;
    if g[T7] != 0 {
        let f = &mut ctx.fpr;
        f[4].set_u32l(lw(m, g[AT], -0x31C8) as u32);
        f[6].set_u32l(lw(m, g[A0], 0x74) as u32);
        f[0].set_fl(f[4].fl() - f[6].fl());
    } else {
        call(imports::func_80052134, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80053220(mask)` (**guess**: menu buttons pressed by whoever
/// controls the menu): the pressed words are `P = 0x800D7700` (the pads'
/// `+0x10` words, [`crate::input::BUTTON_WORDS`]), the selected pad `[0x800A59A8]`.
/// Returns 0 if `[0x800A5998]` is nonzero and `[0x800A59A0]` is 0. Else, if
/// `[0x800A52BC] < 2` (signed), `P[0] & mask`; else if [`func_8002F054`]
/// returns nonzero, `P[sel] & mask`; else the first of pads 0 and 1 with
/// `P[k] & mask` nonzero becomes the selection and 1 is returned, or 0 (the
/// callee's `v0`) if neither.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`, `mask` spilled to its slot `+0x18`
/// around the call (and re-read from there, sign-extended). Leaves `t6` =
/// `[0x800A5998]`, `t7 = [0x800A59A0]` if that was read, else
/// `0x800A0000` (1 if pad 1 was tested); `v1 = 0x800D7700` once `[0x800A52BC]`
/// is read; the words and masks of the path in `t8`, `t0`..`t6`, `at`, and
/// the callee's registers.
///
/// Domain: `[0x800A59A8]` indexes RDRAM (unchecked).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80053220(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = lw(m, li(0x800A_0000), 0x5998);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    g[T7] = li(0x800A_0000);
    'done: {
        if g[T6] != 0 {
            g[T7] = lw(m, g[T7], 0x59A0);
            if g[T7] == 0 {
                g[V0] = 0;
                break 'done;
            }
        }
        g[T8] = lw(m, li(0x800A_0000), 0x52BC);
        g[V1] = li(0x800D_7700);
        g[AT] = slt(g[T8], 2);
        if g[AT] != 0 {
            g[T8] = lw(m, g[V1], 0);
            g[V0] = g[T8] & g[A0];
            break 'done;
        }
        sw(m, g[SP], 0x18, g[A0]);
        call(imports::func_8002F054, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = lw(m, g[SP], 0x18);
        if g[V0] != 0 {
            g[T9] = lw(m, li(0x800A_0000), 0x59A8);
            g[V1] = li(0x800D_7700);
            g[T0] = sll(g[T9], 2);
            g[T1] = addu(g[V1], g[T0]);
            g[T2] = lw(m, g[T1], 0);
            g[V0] = g[T2] & g[A0];
            break 'done;
        }
        // No selection: take the first of pads 0 and 1 that pressed it.
        g[V1] = li(0x800D_7700);
        g[T3] = lw(m, g[V1], 0);
        g[AT] = li(0x800A_0000);
        g[T4] = g[T3] & g[A0];
        if g[T4] != 0 {
            sw(m, g[AT], 0x59A8, 0);
            g[V0] = 1;
            break 'done;
        }
        g[T5] = lw(m, g[V1], 4);
        g[T7] = 1;
        g[AT] = li(0x800A_0000);
        g[T6] = g[T5] & g[A0];
        if g[T6] != 0 {
            sw(m, g[AT], 0x59A8, g[T7]);
            g[V0] = 1;
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// The registers each of [`func_80055AEC`]'s four unrolled pixel copies
/// uses (IDO allocated each copy its own): `r` the red, `c`/`n` the integers
/// 492 and -242, `fc`/`fnr` their FPRs, `fc2`/`fn2` the converted floats,
/// `b`/`b4` the blue and blue * 4, `d` red minus that, `one` the opaque
/// pixel, `prod`/`sum` the threshold's product and sum, `thr` the threshold
/// (`fnr` also receives its truncation).
struct AlphaRegs {
    r: usize,
    c: usize,
    n: usize,
    b: usize,
    b4: usize,
    d: usize,
    one: usize,
    thr: usize,
    fc: usize,
    fnr: usize,
    fc2: usize,
    fn2: usize,
    prod: usize,
    sum: usize,
}

const ALPHA_COPIES: [AlphaRegs; 4] = [
    AlphaRegs { r: T7, c: T3, n: T2, b: T8, b4: T9, d: T1, one: T6, thr: T5, fc: 8, fnr: 4, fc2: 10, fn2: 6, prod: 16, sum: 18 },
    AlphaRegs { r: T8, c: T1, n: T4, b: T9, b4: T2, d: T3, one: T7, thr: T6, fc: 6, fnr: 8, fc2: 16, fn2: 10, prod: 18, sum: 4 },
    AlphaRegs { r: T9, c: T3, n: T5, b: T2, b4: T4, d: T1, one: T8, thr: T7, fc: 10, fnr: 6, fc2: 18, fn2: 16, prod: 4, sum: 8 },
    AlphaRegs { r: T2, c: T1, n: T6, b: T4, b4: T5, d: T3, one: T9, thr: T8, fc: 16, fnr: 10, fc2: 4, fn2: 18, prod: 8, sum: 6 },
];

/// `func_80055AEC(id, level)` (**guess**: a texture's alpha from a level):
/// `level` (the float in `a1`) is clamped to `[0, 1]` (`level < 0` gives 0,
/// `1 < level` gives 1, so NaN stays), then the pixel data `p` of record
/// `(i16)id`'s texture ([`func_8000ABD4`]); if `p` is nonzero, each of the 256
/// RGBA5551 halfwords at `p` gets its alpha bit set if its red is nonzero
/// and `R - B < trunc(-242.0 + level * 492.0)` (`R = (px & 0xF800) >> 8`, `B
/// = (px & 0x3E) << 2`, both 8-bit scaled), and cleared otherwise. The
/// threshold is recomputed for every pixel, as the C does. The loop is
/// unrolled by four ([`ALPHA_COPIES`]).
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`, `id` spilled to its slot `+0x18`,
/// the clamped level to `+0x1C` (and re-read). Leaves `t6 = (i16)id`, `f0` =
/// 0.0 or 1.0 from the clamp, `f12` = the level; with `p` nonzero `a1 = p +
/// 512`, `a2 = -2`, `a3 = t0 = 0x100`, `v1` = the last pixel, `a0` its red
/// bits, and each copy's registers from the last pixel it saw with red; else
/// `a1 = 0` and the callee's registers.
///
/// Domain: `p` in RDRAM; `level` not NaN if any pixel has red.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80055AEC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(g[A1] as u32);
    f[0].set_u32l(0);
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x18, g[A0]);
    let below = f[12].fl() < f[0].fl();
    g[A1] = g[A0];
    g[A0] = sll(g[A1], 16);
    sw(m, g[SP], 0x14, g[RA]);
    g[T6] = sra(g[A0], 16);
    if below {
        f[12].set_u32l(f[0].u32l());
    } else {
        g[AT] = li(0x3F80_0000);
        f[0].set_u32l(g[AT] as u32);
        if f[0].fl() < f[12].fl() {
            f[12].set_u32l(f[0].u32l());
        }
    }
    g[A0] = g[T6];
    sw(m, g[SP], 0x1C, u64::from(f[12].u32l()));
    call(imports::func_8000ABD4, m, ctx);
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[12].set_u32l(lw(m, g[SP], 0x1C) as u32);
    g[A1] = g[V0];
    if g[A1] != 0 {
        g[A3] = 0;
        g[T0] = 0x100;
        g[A2] = (-2i64) as u64;
        loop {
            for (k, r) in ALPHA_COPIES.iter().enumerate() {
                // The first copy reads at a1 and counts; the others step a1 by
                // 2 first (the C reads at a1 + 2, then steps).
                if k == 0 {
                    g[V1] = lhu(m, g[A1], 0);
                    g[A3] = addu(g[A3], 4);
                } else {
                    g[V1] = lhu(m, g[A1], 2);
                    g[A1] = addu(g[A1], 2);
                }
                g[r.c] = 0x1EC;
                g[A0] = g[V1] & 0xF800;
                g[r.r] = sra(g[A0], 8);
                if (g[r.r] as i64) > 0 {
                    f[r.fc].set_u32l(g[r.c] as u32);
                    g[r.n] = (-0xF2i64) as u64;
                    f[r.fnr].set_u32l(g[r.n] as u32);
                    f[r.fc2].set_fl(fpu::cvt_s_w(f[r.fc].u32l(), fpu::NEAREST));
                    g[r.b] = g[V1] & 0x3E;
                    g[r.b4] = sll(g[r.b], 2);
                    g[r.d] = subu(g[r.r], g[r.b4]);
                    g[r.one] = g[V1] | 1;
                    f[r.fn2].set_fl(fpu::cvt_s_w(f[r.fnr].u32l(), fpu::NEAREST));
                    f[r.prod].set_fl(f[12].fl() * f[r.fc2].fl());
                    f[r.sum].set_fl(f[r.fn2].fl() + f[r.prod].fl());
                    f[r.fnr].set_u32l(fpu::trunc_w_s(f[r.sum].fl()));
                    g[r.thr] = s32(f[r.fnr].u32l());
                    g[AT] = slt(g[r.d], g[r.thr]);
                    if g[AT] != 0 {
                        sh(m, g[A1], 0, g[r.one]);
                        continue;
                    }
                }
                g[r.r] = g[V1] & g[A2];
                sh(m, g[A1], 0, g[r.r]);
            }
            g[A1] = addu(g[A1], 2);
            if g[A3] == g[T0] {
                break;
            }
        }
    }
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80056464()` (**guess**: a fading HUD panel): `v` = [`func_8002F060`]
/// (the float `[0x800D7740]`). If `v <= 0.0`, record 26 is turned off
/// ([`func_8000A920`]); else (also for NaN) it is turned on and laid out:
/// `y = 90.0 - (1.0 - v) * 80.0`, position `(160, trunc(y))`
/// ([`func_8000AA04`]), scale `(12.5 + 20.0, 3.90625)` ([`func_8000AAC0`]),
/// colour `(u(0 * 0.5), u(110 * 0.5), u(143 * 0.5), u(254.0 * v))`
/// ([`func_8000AB24`]; the first three computed in double, masked to a
/// byte, the fourth passed whole), then the screen rectangle `(95, trunc(y -
/// 30))..(220, trunc(y + 30))` ([`func_80087814`]). Positions are the
/// truncations (`trunc.w.s`) sign-extended from 16 bits; `u` is IDO's float
/// to unsigned idiom (in double through [`fpu::to_unsigned_d`], the fourth
/// through [`fpu::to_unsigned_s`]; each restores FCR31 directly after).
///
/// Frame (`sp - 0x50`): `ra` at `+0x1C`, `v` at `+0x30`, `y` at `+0x24`, the
/// fifth argument at `+0x10`. Leaves the constants, conversions and sums of
/// the rectangle in `f0`..`f18`, `t0`/`t3`/`t4`/`t7` from the shifts, `at =
/// 0x42700000`, and the callees' registers; the hidden path only the
/// callees' and `f4 = 0.0`.
///
/// Domain: `v` not NaN (it reaches `1.0 - v`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80056464(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let mut fcr31 = fpu::NEAREST;
    g[SP] = addu(g[SP], (-0x50i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    call(imports::func_8002F060, m, ctx);
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(0);
    sw(m, g[SP], 0x30, u64::from(f[0].u32l()));
    g[A1] = 0;
    let hidden = f[0].fl() <= f[4].fl();
    g[A0] = 0x1A;
    if hidden {
        call(imports::func_8000A920, m, ctx);
    } else {
        g[A1] = 1;
        call(imports::func_8000A920, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        // The position: (160, trunc(90 - (1 - v) * 80)).
        g[AT] = li(0x4320_0000);
        f[6].set_u32l(g[AT] as u32);
        g[AT] = li(0x3F80_0000);
        f[10].set_u32l(g[AT] as u32);
        f[16].set_u32l(lw(m, g[SP], 0x30) as u32);
        g[AT] = li(0x42A0_0000);
        f[4].set_u32l(g[AT] as u32);
        f[18].set_fl(f[10].fl() - f[16].fl());
        g[AT] = li(0x42B4_0000);
        g[A0] = 0x1A;
        f[8].set_u32l(fpu::trunc_w_s(f[6].fl()));
        f[6].set_fl(f[18].fl() * f[4].fl());
        g[A1] = s32(f[8].u32l());
        f[8].set_u32l(g[AT] as u32);
        g[T7] = sll(g[A1], 16);
        f[0].set_fl(f[8].fl() - f[6].fl());
        g[A1] = sra(g[T7], 16);
        f[10].set_u32l(fpu::trunc_w_s(f[0].fl()));
        sw(m, g[SP], 0x24, u64::from(f[0].u32l()));
        g[A2] = s32(f[10].u32l());
        g[T0] = sll(g[A2], 16);
        g[A2] = sra(g[T0], 16);
        call(imports::func_8000AA04, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        g[AT] = li(0x4148_0000);
        f[16].set_u32l(g[AT] as u32);
        g[AT] = li(0x41A0_0000);
        f[18].set_u32l(g[AT] as u32);
        g[A0] = 0x1A;
        g[A2] = li(0x407A_0000);
        f[4].set_fl(f[16].fl() + f[18].fl());
        g[A1] = s32(f[4].u32l());
        call(imports::func_8000AAC0, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        // The colour: three bytes from doubles times 0.5 (f0:f1), then the
        // alpha 254 * v. Each idiom's restoring ctc1 comes first after it,
        // so the helpers restore in place.
        f[8].set_u32l(0);
        g[AT] = li(0x3FE0_0000);
        f[0].set_u32h(g[AT] as u32); // f1
        f[6].set_d(f64::from(f[8].u32l() as i32));
        f[0].set_u32l(0);
        g[A1] = 1;
        g[AT] = li(0x41E0_0000);
        g[A0] = 0x1A;
        f[10].set_d(f[6].d() * f[0].d());
        g[T4] = 0x6E;
        g[T7] = 0x8F;
        g[T0] = 0xFE;
        fpu::to_unsigned_d(g, f, &mut fcr31, T2, A1, 16, 10);
        f[18].set_u32l(g[T4] as u32);
        g[A2] = 1;
        g[T3] = g[A1] & 0xFF;
        f[4].set_d(f64::from(f[18].u32l() as i32));
        g[A1] = g[T3];
        g[AT] = li(0x41E0_0000);
        f[8].set_d(f[4].d() * f[0].d());
        fpu::to_unsigned_d(g, f, &mut fcr31, T5, A2, 6, 8);
        f[10].set_u32l(g[T7] as u32);
        g[A3] = 1;
        g[T6] = g[A2] & 0xFF;
        f[16].set_d(f64::from(f[10].u32l() as i32));
        g[A2] = g[T6];
        g[AT] = li(0x41E0_0000);
        f[18].set_d(f[16].d() * f[0].d());
        fpu::to_unsigned_d(g, f, &mut fcr31, T8, A3, 4, 18);
        f[8].set_u32l(g[T0] as u32);
        f[10].set_u32l(lw(m, g[SP], 0x30) as u32);
        g[T2] = 1;
        f[6].set_fl(fpu::cvt_s_w(f[8].u32l(), fcr31));
        g[T9] = g[A3] & 0xFF;
        g[A3] = g[T9];
        // (to_unsigned_s sets at = 0x4F000000 again, as the C's delay slot
        // would in its other shape.)
        g[AT] = li(0x4F00_0000);
        f[16].set_fl(f[6].fl() * f[10].fl());
        fpu::to_unsigned_s(g, f, &mut fcr31, T1, T2, 18, 16);
        sw(m, g[SP], 0x10, g[T2]);
        call(imports::func_8000AB24, m, ctx);
        let g = &mut ctx.gpr;
        let f = &mut ctx.fpr;
        // The rectangle (95, trunc(y - 30))..(220, trunc(y + 30)).
        g[AT] = li(0x41F0_0000);
        f[0].set_u32l(g[AT] as u32);
        g[AT] = li(0x4320_0000);
        f[12].set_u32l(g[AT] as u32);
        g[AT] = li(0x4282_0000);
        f[4].set_u32l(g[AT] as u32);
        f[2].set_u32l(lw(m, g[SP], 0x24) as u32);
        g[AT] = li(0x4270_0000);
        f[8].set_fl(f[12].fl() - f[4].fl());
        f[18].set_u32l(g[AT] as u32);
        f[10].set_fl(f[2].fl() - f[0].fl());
        f[6].set_u32l(fpu::trunc_w_s(f[8].fl()));
        f[4].set_fl(f[12].fl() + f[18].fl());
        g[A0] = s32(f[6].u32l());
        f[6].set_fl(f[2].fl() + f[0].fl());
        g[T4] = sll(g[A0], 16);
        g[A0] = sra(g[T4], 16);
        f[16].set_u32l(fpu::trunc_w_s(f[10].fl()));
        f[10].set_u32l(fpu::trunc_w_s(f[6].fl()));
        g[A1] = s32(f[16].u32l());
        f[8].set_u32l(fpu::trunc_w_s(f[4].fl()));
        g[A3] = s32(f[10].u32l());
        g[T7] = sll(g[A1], 16);
        g[A1] = sra(g[T7], 16);
        g[A2] = s32(f[8].u32l());
        g[T3] = sll(g[A3], 16);
        g[A3] = sra(g[T3], 16);
        g[T0] = sll(g[A2], 16);
        g[A2] = sra(g[T0], 16);
        call(imports::func_80087814, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[SP] = addu(g[SP], 0x50);
}

/// `func_80056844(flags)` (**guess**: lay out the race HUD): nothing if
/// [`func_80051FF4`] returns 2 or more. Else it turns records on or off
/// ([`func_8000A920`]) and sets their positions ([`func_8000AA04`]),
/// scales ([`func_8000AAC0`]) and colours ([`func_8000AB24`]; `G = (0x59,
/// 0x8C, 0x36)`, `B = (0, 0x6E, 0x8F)`), in this order:
///
/// - `flags & 1`: 6 at `(23, 220)` and 5 at `(23, 20)`, both scaled `(270,
///   2)`, colour `G` alpha `0x40`; 13, 0, 11, 4, 12 and 1 off.
/// - else: 5 at `(0, 21)` scaled `(320, 2)`, `G` alpha `0x40`; 6 off; 13 at
///   `(0, 23)` scaled `(80, 1)`, `B`; 0 at `(18, 34)`, 11 at `(66, 35)`
///   scaled `(21.5, 1)`, 4 at `(109, 35)`, 12 at `(211, 35)` scaled `(21.5,
///   1)` and 1 at `(254, 34)`, all `G`.
/// - then 3 at `(229, 180)`, 2 at `(243, 180)` scaled `(6, 1)` and 10 at
///   `(267, 180)`, all `B`.
/// - `flags & 4` clear: with `flags & 2`, `h = min(2 (1 - v), 1) * (1 -
///   L)` (`v` = [`func_8002F060`], `L` = the float `[0x800A59B0]`; the
///   minimum as `if 1.0 < h`), else `h = 0`; `k = 90 h`, `y = 164 - k`; 7 at
///   `(275, trunc(y))` scaled `(1, k / 2)` and 9 at `(275, trunc(y - 4))`,
///   both `G`; 8 off.
/// - `flags & 4` set: 7 at `(22, 21)` and 8 at `(trunc(X), 21)`, both
///   scaled `(1, 99.5)`, and 9 at `(275, 160)`, all `G`; `X` is the float at
///   `0x800ACEDC` (283.0 in the ROM).
/// - last, 25 on if `0 < L` (else off), at `(trunc(X2), 60)` with `X2` the
///   float at `0x800ACEE0` (232.75), scaled `(15.625, 3.90625)`, colour `(0,
///   55, 71, u(254 L))` as in [`func_80056464`] (three double idioms through
///   [`fpu::to_unsigned_d`], the alpha through [`fpu::to_unsigned_s`], each
///   restored directly after, the alpha passed whole).
///
/// Colour alphas other than `0x40` are `0xFE`. Positions are truncations
/// (`trunc.w.s`) sign-extended from 16 bits, the constant ones computed at
/// run time from float constants and sums, as the C does; several go
/// through frame words (`sp + 0x28`, `+0x24`, `+0xA8`, `+0xAC`) and are
/// re-read as halfwords from there.
///
/// Frame (`sp - 0x1B8`): `ra` at `+0x1C`, `flags` spilled to its slot
/// `+0x1B8`, the fifth argument at `+0x10`, and the positions, `h` (`+0xA4`),
/// `k` (`+0x28`), `y` (`+0x20`) and `L` (`+0x2C`) as the C leaves them.
/// Leaves the constants, conversions and sums of the last layout in
/// `f0`..`f18`, the shifts' temporaries, `at = 0x4F000000` and the callees'
/// registers (or only [`func_80051FF4`]'s and `at`, `t6` if it returned 2 or
/// more).
///
/// Domain: `L` not NaN once [`func_80051FF4`] returns less than 2; with
/// `flags & 2` set and `flags & 4` clear, `v` not NaN either.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80056844(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let mut fcr31 = fpu::NEAREST;
    g[SP] = addu(g[SP], (-0x1B8i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x1B8, g[A0]);
    call(imports::func_80051FF4, m, ctx);
    let g = &mut ctx.gpr;
    g[AT] = slt(g[V0], 2);
    g[T6] = lw(m, g[SP], 0x1B8);
    if g[AT] != 0 {
        g[T7] = g[T6] & 1;
        g[A0] = 5;
        if g[T7] != 0 {
            g[A0] = 6;
            g[A1] = 1;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x41B8_0000);
            ctx.fpr[6].set_u32l(g[AT] as u32);
            g[AT] = li(0x435C_0000);
            ctx.fpr[10].set_u32l(g[AT] as u32);
            ctx.fpr[8].set_u32l(fpu::trunc_w_s(ctx.fpr[6].fl()));
            g[A0] = 6;
            ctx.fpr[18].set_u32l(fpu::trunc_w_s(ctx.fpr[10].fl()));
            g[A1] = s32(ctx.fpr[8].u32l());
            g[A2] = s32(ctx.fpr[18].u32l());
            g[T9] = sll(g[A1], 16);
            g[A1] = sra(g[T9], 16);
            g[T2] = sll(g[A2], 16);
            g[A2] = sra(g[T2], 16);
            sw(m, g[SP], 0x28, g[A1]);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 6;
            g[A1] = li(0x4387_0000);
            g[A2] = li(0x4000_0000);
            call(imports::func_8000AAC0, m, ctx);
            let g = &mut ctx.gpr;
            g[T4] = 0x40;
            sw(m, g[SP], 0x10, g[T4]);
            g[A0] = 6;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 5;
            g[A1] = 1;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x41A0_0000);
            ctx.fpr[4].set_u32l(g[AT] as u32);
            g[A0] = 5;
            g[A1] = lh(m, g[SP], 0x2A);
            ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[4].fl()));
            g[A2] = s32(ctx.fpr[6].u32l());
            g[T6] = sll(g[A2], 16);
            g[A2] = sra(g[T6], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 5;
            g[A1] = li(0x4387_0000);
            g[A2] = li(0x4000_0000);
            call(imports::func_8000AAC0, m, ctx);
            let g = &mut ctx.gpr;
            g[T8] = 0x40;
            sw(m, g[SP], 0x10, g[T8]);
            g[A0] = 5;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 0xD;
            g[A1] = 0;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 0;
            g[A1] = 0;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 0xB;
            g[A1] = 0;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 4;
            g[A1] = 0;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 0xC;
            g[A1] = 0;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 1;
            g[A1] = 0;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 3;
        } else {
            g[A1] = 1;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x41A0_0000);
            ctx.fpr[8].set_u32l(g[AT] as u32);
            g[AT] = li(0x3F80_0000);
            ctx.fpr[10].set_u32l(g[AT] as u32);
            ctx.fpr[4].set_u32l(0);
            g[A0] = 5;
            ctx.fpr[18].set_fl(ctx.fpr[8].fl() + ctx.fpr[10].fl());
            ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[4].fl()));
            ctx.fpr[4].set_u32l(fpu::trunc_w_s(ctx.fpr[18].fl()));
            g[A1] = s32(ctx.fpr[6].u32l());
            g[A2] = s32(ctx.fpr[4].u32l());
            g[T9] = sll(g[A1], 16);
            g[A1] = sra(g[T9], 16);
            g[T2] = sll(g[A2], 16);
            g[A2] = sra(g[T2], 16);
            sw(m, g[SP], 0x28, g[A1]);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 5;
            g[A1] = li(0x43A0_0000);
            g[A2] = li(0x4000_0000);
            call(imports::func_8000AAC0, m, ctx);
            let g = &mut ctx.gpr;
            g[T4] = 0x40;
            sw(m, g[SP], 0x10, g[T4]);
            g[A0] = 5;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 6;
            g[A1] = 0;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 0xD;
            g[A1] = 1;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x41A0_0000);
            ctx.fpr[6].set_u32l(g[AT] as u32);
            g[AT] = li(0x4040_0000);
            ctx.fpr[8].set_u32l(g[AT] as u32);
            g[A0] = 0xD;
            g[A1] = lh(m, g[SP], 0x2A);
            ctx.fpr[10].set_fl(ctx.fpr[6].fl() + ctx.fpr[8].fl());
            ctx.fpr[18].set_u32l(fpu::trunc_w_s(ctx.fpr[10].fl()));
            g[A2] = s32(ctx.fpr[18].u32l());
            g[T6] = sll(g[A2], 16);
            g[A2] = sra(g[T6], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 0xD;
            g[A1] = li(0x42A0_0000);
            g[A2] = li(0x3F80_0000);
            call(imports::func_8000AAC0, m, ctx);
            let g = &mut ctx.gpr;
            g[T8] = 0xFE;
            sw(m, g[SP], 0x10, g[T8]);
            g[A0] = 0xD;
            g[A1] = 0;
            g[A2] = 0x6E;
            g[A3] = 0x8F;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x41A0_0000);
            ctx.fpr[4].set_u32l(g[AT] as u32);
            g[AT] = li(0x4160_0000);
            ctx.fpr[6].set_u32l(g[AT] as u32);
            g[A0] = 0;
            g[A1] = 1;
            ctx.fpr[8].set_fl(ctx.fpr[4].fl() + ctx.fpr[6].fl());
            sw(m, g[SP], 0xA8, u64::from(ctx.fpr[8].u32l()));
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x4190_0000);
            ctx.fpr[10].set_u32l(g[AT] as u32);
            ctx.fpr[4].set_u32l(lw(m, g[SP], 0xA8) as u32);
            g[A0] = 0;
            ctx.fpr[18].set_u32l(fpu::trunc_w_s(ctx.fpr[10].fl()));
            ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[4].fl()));
            g[A1] = s32(ctx.fpr[18].u32l());
            g[A2] = s32(ctx.fpr[6].u32l());
            g[T0] = sll(g[A1], 16);
            g[A1] = sra(g[T0], 16);
            g[T3] = sll(g[A2], 16);
            g[A2] = sra(g[T3], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[T5] = 0xFE;
            sw(m, g[SP], 0x10, g[T5]);
            g[A0] = 0;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x4190_0000);
            ctx.fpr[8].set_u32l(g[AT] as u32);
            g[AT] = li(0x4240_0000);
            ctx.fpr[10].set_u32l(g[AT] as u32);
            g[AT] = li(0x3F80_0000);
            ctx.fpr[6].set_u32l(g[AT] as u32);
            ctx.fpr[4].set_u32l(lw(m, g[SP], 0xA8) as u32);
            ctx.fpr[18].set_fl(ctx.fpr[8].fl() + ctx.fpr[10].fl());
            g[A0] = 0xB;
            g[A1] = 1;
            ctx.fpr[8].set_fl(ctx.fpr[4].fl() + ctx.fpr[6].fl());
            sw(m, g[SP], 0xAC, u64::from(ctx.fpr[18].u32l()));
            sw(m, g[SP], 0xA8, u64::from(ctx.fpr[8].u32l()));
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            ctx.fpr[4].set_u32l(lw(m, g[SP], 0xA8) as u32);
            ctx.fpr[10].set_u32l(lw(m, g[SP], 0xAC) as u32);
            g[A0] = 0xB;
            ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[4].fl()));
            ctx.fpr[18].set_u32l(fpu::trunc_w_s(ctx.fpr[10].fl()));
            g[A2] = s32(ctx.fpr[6].u32l());
            g[A1] = s32(ctx.fpr[18].u32l());
            g[T0] = sll(g[A2], 16);
            g[A2] = sra(g[T0], 16);
            g[T7] = sll(g[A1], 16);
            g[A1] = sra(g[T7], 16);
            sw(m, g[SP], 0x28, g[A2]);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 0xB;
            g[A1] = li(0x41AC_0000);
            g[A2] = li(0x3F80_0000);
            call(imports::func_8000AAC0, m, ctx);
            let g = &mut ctx.gpr;
            g[T2] = 0xFE;
            sw(m, g[SP], 0x10, g[T2]);
            g[A0] = 0xB;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 4;
            g[A1] = 1;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x42DA_0000);
            ctx.fpr[8].set_u32l(g[AT] as u32);
            g[A0] = 4;
            g[A2] = lh(m, g[SP], 0x2A);
            ctx.fpr[10].set_u32l(fpu::trunc_w_s(ctx.fpr[8].fl()));
            g[A1] = s32(ctx.fpr[10].u32l());
            g[T4] = sll(g[A1], 16);
            g[A1] = sra(g[T4], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[T6] = 0xFE;
            sw(m, g[SP], 0x10, g[T6]);
            g[A0] = 4;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x42DA_0000);
            ctx.fpr[18].set_u32l(g[AT] as u32);
            g[AT] = li(0x42CC_0000);
            ctx.fpr[4].set_u32l(g[AT] as u32);
            g[A0] = 0xC;
            g[A1] = 1;
            ctx.fpr[6].set_fl(ctx.fpr[18].fl() + ctx.fpr[4].fl());
            sw(m, g[SP], 0xAC, u64::from(ctx.fpr[6].u32l()));
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            ctx.fpr[8].set_u32l(lw(m, g[SP], 0xAC) as u32);
            g[A0] = 0xC;
            g[A2] = lh(m, g[SP], 0x2A);
            ctx.fpr[10].set_u32l(fpu::trunc_w_s(ctx.fpr[8].fl()));
            g[A1] = s32(ctx.fpr[10].u32l());
            g[T8] = sll(g[A1], 16);
            g[A1] = sra(g[T8], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 0xC;
            g[A1] = li(0x41AC_0000);
            g[A2] = li(0x3F80_0000);
            call(imports::func_8000AAC0, m, ctx);
            let g = &mut ctx.gpr;
            g[T0] = 0xFE;
            sw(m, g[SP], 0x10, g[T0]);
            g[A0] = 0xC;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x3F80_0000);
            ctx.fpr[4].set_u32l(g[AT] as u32);
            ctx.fpr[18].set_u32l(lw(m, g[SP], 0xA8) as u32);
            g[A0] = 1;
            g[A1] = 1;
            ctx.fpr[6].set_fl(ctx.fpr[18].fl() - ctx.fpr[4].fl());
            sw(m, g[SP], 0xA8, u64::from(ctx.fpr[6].u32l()));
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x437E_0000);
            ctx.fpr[8].set_u32l(g[AT] as u32);
            ctx.fpr[18].set_u32l(lw(m, g[SP], 0xA8) as u32);
            g[A0] = 1;
            ctx.fpr[10].set_u32l(fpu::trunc_w_s(ctx.fpr[8].fl()));
            ctx.fpr[4].set_u32l(fpu::trunc_w_s(ctx.fpr[18].fl()));
            g[A1] = s32(ctx.fpr[10].u32l());
            g[A2] = s32(ctx.fpr[4].u32l());
            g[T2] = sll(g[A1], 16);
            g[A1] = sra(g[T2], 16);
            g[T5] = sll(g[A2], 16);
            g[A2] = sra(g[T5], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[T7] = 0xFE;
            sw(m, g[SP], 0x10, g[T7]);
            g[A0] = 1;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 3;
        }
        let g = &mut ctx.gpr;
        g[A1] = 1;
        call(imports::func_8000A920, m, ctx);
        let g = &mut ctx.gpr;
        g[T3] = 0xB4;
        ctx.fpr[18].set_u32l(g[T3] as u32);
        g[T9] = 0xE5;
        ctx.fpr[8].set_u32l(g[T9] as u32);
        ctx.fpr[4].set_fl(fpu::cvt_s_w(ctx.fpr[18].u32l(), fcr31));
        g[A0] = 3;
        ctx.fpr[0].set_fl(fpu::cvt_s_w(ctx.fpr[8].u32l(), fcr31));
        ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[4].fl()));
        sw(m, g[SP], 0x24, u64::from(ctx.fpr[0].u32l()));
        ctx.fpr[10].set_u32l(fpu::trunc_w_s(ctx.fpr[0].fl()));
        g[A2] = s32(ctx.fpr[6].u32l());
        g[A1] = s32(ctx.fpr[10].u32l());
        g[T5] = sll(g[A2], 16);
        g[A2] = sra(g[T5], 16);
        g[T1] = sll(g[A1], 16);
        g[A1] = sra(g[T1], 16);
        sw(m, g[SP], 0x28, g[A2]);
        call(imports::func_8000AA04, m, ctx);
        let g = &mut ctx.gpr;
        g[T7] = 0xFE;
        sw(m, g[SP], 0x10, g[T7]);
        g[A0] = 3;
        g[A1] = 0;
        g[A2] = 0x6E;
        g[A3] = 0x8F;
        call(imports::func_8000AB24, m, ctx);
        let g = &mut ctx.gpr;
        g[AT] = li(0x4160_0000);
        ctx.fpr[10].set_u32l(g[AT] as u32);
        ctx.fpr[8].set_u32l(lw(m, g[SP], 0x24) as u32);
        g[A0] = 2;
        g[A1] = 1;
        ctx.fpr[18].set_fl(ctx.fpr[8].fl() + ctx.fpr[10].fl());
        sw(m, g[SP], 0xAC, u64::from(ctx.fpr[18].u32l()));
        call(imports::func_8000A920, m, ctx);
        let g = &mut ctx.gpr;
        ctx.fpr[4].set_u32l(lw(m, g[SP], 0xAC) as u32);
        g[A0] = 2;
        g[A2] = lh(m, g[SP], 0x2A);
        ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[4].fl()));
        g[A1] = s32(ctx.fpr[6].u32l());
        g[T9] = sll(g[A1], 16);
        g[A1] = sra(g[T9], 16);
        call(imports::func_8000AA04, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = 2;
        g[A1] = li(0x40C0_0000);
        g[A2] = li(0x3F80_0000);
        call(imports::func_8000AAC0, m, ctx);
        let g = &mut ctx.gpr;
        g[T1] = 0xFE;
        sw(m, g[SP], 0x10, g[T1]);
        g[A0] = 2;
        g[A1] = 0;
        g[A2] = 0x6E;
        g[A3] = 0x8F;
        call(imports::func_8000AB24, m, ctx);
        let g = &mut ctx.gpr;
        g[AT] = li(0x41C0_0000);
        ctx.fpr[10].set_u32l(g[AT] as u32);
        ctx.fpr[8].set_u32l(lw(m, g[SP], 0xAC) as u32);
        g[A0] = 0xA;
        g[A1] = 1;
        ctx.fpr[18].set_fl(ctx.fpr[8].fl() + ctx.fpr[10].fl());
        sw(m, g[SP], 0xAC, u64::from(ctx.fpr[18].u32l()));
        call(imports::func_8000A920, m, ctx);
        let g = &mut ctx.gpr;
        ctx.fpr[4].set_u32l(lw(m, g[SP], 0xAC) as u32);
        g[A0] = 0xA;
        g[A2] = lh(m, g[SP], 0x2A);
        ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[4].fl()));
        g[A1] = s32(ctx.fpr[6].u32l());
        g[T3] = sll(g[A1], 16);
        g[A1] = sra(g[T3], 16);
        call(imports::func_8000AA04, m, ctx);
        let g = &mut ctx.gpr;
        g[T5] = 0xFE;
        sw(m, g[SP], 0x10, g[T5]);
        g[A0] = 0xA;
        g[A1] = 0;
        g[A2] = 0x6E;
        g[A3] = 0x8F;
        call(imports::func_8000AB24, m, ctx);
        let g = &mut ctx.gpr;
        g[T6] = lw(m, g[SP], 0x1B8);
        g[T8] = lw(m, g[SP], 0x1B8);
        g[T7] = g[T6] & 4;
        g[T9] = g[T8] & 2;
        if g[T7] == 0 {
            if g[T9] == 0 {
                ctx.fpr[14].set_u32l(0);
                g[A0] = 7;
            } else {
                call(imports::func_8002F060, m, ctx);
                let g = &mut ctx.gpr;
                g[AT] = li(0x3F80_0000);
                ctx.fpr[16].set_u32l(g[AT] as u32);
                g[AT] = li(0x800A_0000);
                ctx.fpr[2].set_fl(ctx.fpr[16].fl() - ctx.fpr[0].fl());
                ctx.fpr[14].set_fl(ctx.fpr[2].fl() + ctx.fpr[2].fl());
                if ctx.fpr[16].fl() < ctx.fpr[14].fl() {
                    ctx.fpr[14].set_u32l(ctx.fpr[16].u32l());
                }
                ctx.fpr[18].set_u32l(lw(m, g[AT], 0x59B0) as u32);
                ctx.fpr[4].set_fl(ctx.fpr[16].fl() - ctx.fpr[18].fl());
                ctx.fpr[14].set_fl(ctx.fpr[14].fl() * ctx.fpr[4].fl());
                g[A0] = 7;
            }
            let g = &mut ctx.gpr;
            g[A1] = 1;
            sw(m, g[SP], 0xA4, u64::from(ctx.fpr[14].u32l()));
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[T0] = 0x113;
            g[AT] = li(0x42B4_0000);
            ctx.fpr[14].set_u32l(lw(m, g[SP], 0xA4) as u32);
            ctx.fpr[18].set_u32l(g[AT] as u32);
            ctx.fpr[6].set_u32l(g[T0] as u32);
            g[T4] = 0xA4;
            ctx.fpr[4].set_u32l(g[T4] as u32);
            ctx.fpr[0].set_fl(ctx.fpr[18].fl() * ctx.fpr[14].fl());
            g[A0] = 7;
            ctx.fpr[8].set_fl(fpu::cvt_s_w(ctx.fpr[6].u32l(), fcr31));
            sw(m, g[SP], 0x28, u64::from(ctx.fpr[0].u32l()));
            ctx.fpr[6].set_fl(fpu::cvt_s_w(ctx.fpr[4].u32l(), fcr31));
            ctx.fpr[10].set_u32l(fpu::trunc_w_s(ctx.fpr[8].fl()));
            ctx.fpr[2].set_fl(ctx.fpr[6].fl() - ctx.fpr[0].fl());
            g[A1] = s32(ctx.fpr[10].u32l());
            ctx.fpr[8].set_u32l(fpu::trunc_w_s(ctx.fpr[2].fl()));
            g[T2] = sll(g[A1], 16);
            g[A1] = sra(g[T2], 16);
            sw(m, g[SP], 0x24, g[A1]);
            g[A2] = s32(ctx.fpr[8].u32l());
            sw(m, g[SP], 0x20, u64::from(ctx.fpr[2].u32l()));
            g[T6] = sll(g[A2], 16);
            g[A2] = sra(g[T6], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x4000_0000);
            ctx.fpr[0].set_u32l(lw(m, g[SP], 0x28) as u32);
            ctx.fpr[10].set_u32l(g[AT] as u32);
            g[A0] = 7;
            g[A1] = li(0x3F80_0000);
            ctx.fpr[18].set_fl(ctx.fpr[0].fl() / ctx.fpr[10].fl());
            g[A2] = s32(ctx.fpr[18].u32l());
            call(imports::func_8000AAC0, m, ctx);
            let g = &mut ctx.gpr;
            g[T8] = 0xFE;
            sw(m, g[SP], 0x10, g[T8]);
            g[A0] = 7;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x4080_0000);
            ctx.fpr[6].set_u32l(g[AT] as u32);
            ctx.fpr[4].set_u32l(lw(m, g[SP], 0x20) as u32);
            g[A0] = 9;
            g[A1] = 1;
            ctx.fpr[8].set_fl(ctx.fpr[4].fl() - ctx.fpr[6].fl());
            sw(m, g[SP], 0xA8, u64::from(ctx.fpr[8].u32l()));
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            ctx.fpr[10].set_u32l(lw(m, g[SP], 0xA8) as u32);
            g[A0] = 9;
            g[A1] = lh(m, g[SP], 0x26);
            ctx.fpr[18].set_u32l(fpu::trunc_w_s(ctx.fpr[10].fl()));
            g[A2] = s32(ctx.fpr[18].u32l());
            g[T0] = sll(g[A2], 16);
            g[A2] = sra(g[T0], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[T2] = 0xFE;
            sw(m, g[SP], 0x10, g[T2]);
            g[A0] = 9;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 8;
            g[A1] = 0;
            call(imports::func_8000A920, m, ctx);
        } else {
            g[A0] = 7;
            g[A1] = 1;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x41B0_0000);
            ctx.fpr[8].set_u32l(g[AT] as u32);
            g[AT] = li(0x41A8_0000);
            ctx.fpr[18].set_u32l(g[AT] as u32);
            ctx.fpr[10].set_u32l(fpu::trunc_w_s(ctx.fpr[8].fl()));
            g[A0] = 7;
            ctx.fpr[4].set_u32l(fpu::trunc_w_s(ctx.fpr[18].fl()));
            g[A1] = s32(ctx.fpr[10].u32l());
            g[A2] = s32(ctx.fpr[4].u32l());
            g[T9] = sll(g[A1], 16);
            g[A1] = sra(g[T9], 16);
            g[T2] = sll(g[A2], 16);
            g[A2] = sra(g[T2], 16);
            sw(m, g[SP], 0x28, g[A2]);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 7;
            g[A1] = li(0x3F80_0000);
            g[A2] = li(0x42C7_0000);
            call(imports::func_8000AAC0, m, ctx);
            let g = &mut ctx.gpr;
            g[T4] = 0xFE;
            sw(m, g[SP], 0x10, g[T4]);
            g[A0] = 7;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 8;
            g[A1] = 1;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[AT] = li(0x800B_0000);
            ctx.fpr[6].set_u32l(lw(m, g[AT], -0x3124) as u32);
            g[A0] = 8;
            g[A2] = lh(m, g[SP], 0x2A);
            ctx.fpr[8].set_u32l(fpu::trunc_w_s(ctx.fpr[6].fl()));
            g[A1] = s32(ctx.fpr[8].u32l());
            g[T6] = sll(g[A1], 16);
            g[A1] = sra(g[T6], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 8;
            g[A1] = li(0x3F80_0000);
            g[A2] = li(0x42C7_0000);
            call(imports::func_8000AAC0, m, ctx);
            let g = &mut ctx.gpr;
            g[T8] = 0xFE;
            sw(m, g[SP], 0x10, g[T8]);
            g[A0] = 8;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
            let g = &mut ctx.gpr;
            g[A0] = 9;
            g[A1] = 1;
            call(imports::func_8000A920, m, ctx);
            let g = &mut ctx.gpr;
            g[T9] = 0x113;
            g[T3] = 0xA0;
            ctx.fpr[6].set_u32l(g[T3] as u32);
            ctx.fpr[10].set_u32l(g[T9] as u32);
            g[A0] = 9;
            ctx.fpr[8].set_fl(fpu::cvt_s_w(ctx.fpr[6].u32l(), fcr31));
            ctx.fpr[18].set_fl(fpu::cvt_s_w(ctx.fpr[10].u32l(), fcr31));
            ctx.fpr[10].set_u32l(fpu::trunc_w_s(ctx.fpr[8].fl()));
            ctx.fpr[4].set_u32l(fpu::trunc_w_s(ctx.fpr[18].fl()));
            g[A2] = s32(ctx.fpr[10].u32l());
            g[A1] = s32(ctx.fpr[4].u32l());
            g[T5] = sll(g[A2], 16);
            g[A2] = sra(g[T5], 16);
            g[T1] = sll(g[A1], 16);
            g[A1] = sra(g[T1], 16);
            call(imports::func_8000AA04, m, ctx);
            let g = &mut ctx.gpr;
            g[T7] = 0xFE;
            sw(m, g[SP], 0x10, g[T7]);
            g[A0] = 9;
            g[A1] = 0x59;
            g[A2] = 0x8C;
            g[A3] = 0x36;
            call(imports::func_8000AB24, m, ctx);
        }
        let g = &mut ctx.gpr;
        g[AT] = li(0x800A_0000);
        ctx.fpr[4].set_u32l(lw(m, g[AT], 0x59B0) as u32);
        ctx.fpr[6].set_u32l(0);
        g[A1] = 1;
        g[A0] = 0x19;
        let on = ctx.fpr[6].fl() < ctx.fpr[4].fl();
        sw(m, g[SP], 0x2C, u64::from(ctx.fpr[4].u32l()));
        if !on {
            g[A1] = 0;
        }
        call(imports::func_8000A920, m, ctx);
        let g = &mut ctx.gpr;
        g[AT] = li(0x800B_0000);
        ctx.fpr[8].set_u32l(lw(m, g[AT], -0x3120) as u32);
        g[AT] = li(0x4273_0000);
        ctx.fpr[18].set_u32l(g[AT] as u32);
        ctx.fpr[10].set_u32l(fpu::trunc_w_s(ctx.fpr[8].fl()));
        g[A0] = 0x19;
        ctx.fpr[6].set_u32l(fpu::trunc_w_s(ctx.fpr[18].fl()));
        g[A1] = s32(ctx.fpr[10].u32l());
        g[A2] = s32(ctx.fpr[6].u32l());
        g[T4] = sll(g[A1], 16);
        g[A1] = sra(g[T4], 16);
        g[T7] = sll(g[A2], 16);
        g[A2] = sra(g[T7], 16);
        call(imports::func_8000AA04, m, ctx);
        let g = &mut ctx.gpr;
        g[A0] = 0x19;
        g[A1] = li(0x417A_0000);
        g[A2] = li(0x407A_0000);
        call(imports::func_8000AAC0, m, ctx);
        let g = &mut ctx.gpr;
        // The colour: three bytes from doubles times 0.5 (f0:f1), then the
        // alpha 254 L. Each idiom's restoring ctc1 comes first after it, so
        // the helpers restore in place.
        ctx.fpr[4].set_u32l(0);
        g[AT] = li(0x3FE0_0000);
        ctx.fpr[0].set_u32h(g[AT] as u32); // f1
        ctx.fpr[8].set_d(f64::from(ctx.fpr[4].u32l() as i32));
        ctx.fpr[0].set_u32l(0);
        g[A1] = 1;
        g[AT] = li(0x41E0_0000);
        g[A0] = 0x19;
        ctx.fpr[10].set_d(ctx.fpr[8].d() * ctx.fpr[0].d());
        g[T1] = 0x6E;
        g[T4] = 0x8F;
        g[T7] = 0xFE;
        fpu::to_unsigned_d(g, &mut ctx.fpr, &mut fcr31, T9, A1, 18, 10);
        ctx.fpr[6].set_u32l(g[T1] as u32);
        g[A2] = 1;
        g[T0] = g[A1] & 0xFF;
        ctx.fpr[4].set_d(f64::from(ctx.fpr[6].u32l() as i32));
        g[A1] = g[T0];
        g[AT] = li(0x41E0_0000);
        ctx.fpr[8].set_d(ctx.fpr[4].d() * ctx.fpr[0].d());
        fpu::to_unsigned_d(g, &mut ctx.fpr, &mut fcr31, T2, A2, 10, 8);
        ctx.fpr[18].set_u32l(g[T4] as u32);
        g[A3] = 1;
        g[T3] = g[A2] & 0xFF;
        ctx.fpr[6].set_d(f64::from(ctx.fpr[18].u32l() as i32));
        g[A2] = g[T3];
        g[AT] = li(0x41E0_0000);
        ctx.fpr[4].set_d(ctx.fpr[6].d() * ctx.fpr[0].d());
        fpu::to_unsigned_d(g, &mut ctx.fpr, &mut fcr31, T5, A3, 8, 4);
        ctx.fpr[10].set_u32l(g[T7] as u32);
        ctx.fpr[6].set_u32l(lw(m, g[SP], 0x2C) as u32);
        g[T9] = 1;
        ctx.fpr[18].set_fl(fpu::cvt_s_w(ctx.fpr[10].u32l(), fcr31));
        g[T6] = g[A3] & 0xFF;
        g[A3] = g[T6];
        // (to_unsigned_s sets at = 0x4F000000 again.)
        g[AT] = li(0x4F00_0000);
        ctx.fpr[4].set_fl(ctx.fpr[18].fl() * ctx.fpr[6].fl());
        fpu::to_unsigned_s(g, &mut ctx.fpr, &mut fcr31, T8, T9, 8, 4);
        sw(m, g[SP], 0x10, g[T9]);
        call(imports::func_8000AB24, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[SP] = addu(g[SP], 0x1B8);
}

/// `func_80057ED4()` (**guess**: a HUD bar): record 23 on
/// ([`func_8000A920`]), at `(0, 118)` ([`func_8000AA04`]), scaled `(320.0,
/// 4.0)` ([`func_8000AAC0`]), coloured `(0, 0, 0, 255)` ([`func_8000AB24`]),
/// then the screen rectangle `(20, 117)..(300, 123)` ([`func_80087814`]).
///
/// Frame (`sp - 0x20`): `ra` at `+0x1C`, the fifth argument at `+0x10`.
/// Leaves `t6 = 0xFF` and the callees' registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80057ED4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    g[A0] = 0x17;
    g[A1] = 1;
    call(imports::func_8000A920, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = 0x17;
    g[A1] = 0;
    g[A2] = 0x76;
    call(imports::func_8000AA04, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = 0x17;
    g[A1] = li(0x43A0_0000);
    g[A2] = li(0x4080_0000);
    call(imports::func_8000AAC0, m, ctx);
    let g = &mut ctx.gpr;
    g[T6] = 0xFF;
    sw(m, g[SP], 0x10, g[T6]);
    g[A0] = 0x17;
    g[A1] = 0;
    g[A2] = 0;
    g[A3] = 0;
    call(imports::func_8000AB24, m, ctx);
    let g = &mut ctx.gpr;
    g[A0] = 0x14;
    g[A1] = 0x75;
    g[A2] = 0x12C;
    g[A3] = 0x7B;
    call(imports::func_80087814, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_80057F48(k)` (**guess**: hide a player's HUD set): turns off
/// ([`func_8000A920`]`(id, 0)`) records 35..40, 41 and 42 if
/// [`func_80051FF4`] returns 2 (the first two of the four words at
/// `0x8011B1BC` set, the third not) and `k` differs from the second word
/// `[0x8011B1C0]` (a full 64-bit compare with the sign-extended word); else
/// records 27..32, 33 and 34.
///
/// The three copies of the loop differ only in the temporary: `t7` (27.., `v0
/// == 2`), `t9` (35..) or `t1` (27.., `v0 != 2`).
///
/// Frame (`sp - 0x20`): `s0` at `+0x14`, `s1` at `+0x18`, `ra` at `+0x1C`,
/// restored sign-extended. Leaves `a0` = the last id, `a1 = 0`, `at = 2`,
/// the temporary `= (first + 5) << 16`, `t6 = [0x8011B1C0]` if read, and the
/// callees' registers.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80057F48(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x20i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x14, g[S0]);
    g[S0] = g[A0];
    sw(m, g[SP], 0x18, g[S1]);
    call(imports::func_80051FF4, m, ctx);
    let g = &mut ctx.gpr;
    g[AT] = 2;
    g[S1] = 6;
    let (first, tmp, last) = if g[V0] != g[AT] {
        g[S0] = 0;
        (0x1B, T1, [0x21, 0x22])
    } else {
        g[T6] = lw(m, li(0x8012_0000), -0x4E40);
        g[S1] = 6;
        let other = g[S0] != g[T6];
        g[S0] = 0;
        if other {
            (0x23, T9, [0x29, 0x2A])
        } else {
            g[S1] = 6;
            (0x1B, T7, [0x21, 0x22])
        }
    };
    g[A0] = addu(g[S0], first);
    loop {
        let g = &mut ctx.gpr;
        g[tmp] = sll(g[A0], 16);
        g[A0] = sra(g[tmp], 16);
        g[A1] = 0;
        call(imports::func_8000A920, m, ctx);
        let g = &mut ctx.gpr;
        g[S0] = addu(g[S0], 1);
        if g[S0] == g[S1] {
            break;
        }
        g[A0] = addu(g[S0], first);
    }
    for id in last {
        let g = &mut ctx.gpr;
        g[A0] = id;
        g[A1] = 0;
        call(imports::func_8000A920, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x20);
}

/// `func_8005B2D0(r, o)` (**guess**: whether an object counts as active):
/// returns `v0 = 0` early if `[r + 0x1AC] == 1`, `[r + 0x1C0] == 3`, the
/// float `x = [o + 0x50]` lies strictly between the floats at `0x800ACFA0`
/// and `0x800ACFA4` (-6081.0 and -5086.0 in the ROM), `y = [o + 0x54]`
/// strictly between those at `0x800ACFA8` and `0x800ACFAC` (-2801.0 and
/// -1182.0), `[o + 0x140]` is nonzero, and its [`func_800183A8`] value `p` is
/// nonzero with bit 3 of the halfword `[p]` set. Otherwise it returns 0 if
/// bit 25 of `[o + 0x64]` is set, else 1 if the halfword `[o + 0x10C]` is at
/// least 5 (signed) or `f64([o + 0x1A0]) < 60.0`, else 0.
///
/// Frame (`sp - 0x18`): `ra` at `+0x14`, `o` spilled to its slot `+0x1C`
/// around the call (and re-read sign-extended). Leaves `t6`, `t7`, `at`, the
/// bounds and coordinates compared in `f0`..`f10`, `t8`/`t9` from `[p]`,
/// `t0` = `[o + 0x64]`, `t1 = t0 << 6`, `t2` = the halfword, `f16`/`f18` the
/// float and its double, `f4` = 60.0 as a double, and the callee's
/// registers.
///
/// Domain: canonical `r`, `o`; `[o + 0x1A0]` not NaN when it is converted.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8005B2D0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    g[T6] = lw(m, g[A0], 0x1AC);
    g[AT] = 1;
    'done: {
        // The early 0: mode 1/3, (x, y) in the box, and p's bit 3.
        'test: {
            if g[T6] != g[AT] {
                break 'test;
            }
            g[T7] = lw(m, g[A0], 0x1C0);
            g[AT] = 3;
            let mode3 = g[T7] == g[AT];
            g[AT] = li(0x800B_0000);
            if !mode3 {
                break 'test;
            }
            let f = &mut ctx.fpr;
            f[4].set_u32l(lw(m, g[AT], -0x3060) as u32);
            f[0].set_u32l(lw(m, g[A1], 0x50) as u32);
            g[AT] = li(0x800B_0000);
            if !(f[4].fl() < f[0].fl()) {
                break 'test;
            }
            f[6].set_u32l(lw(m, g[AT], -0x305C) as u32);
            g[AT] = li(0x800B_0000);
            if !(f[0].fl() < f[6].fl()) {
                break 'test;
            }
            f[0].set_u32l(lw(m, g[A1], 0x54) as u32);
            f[8].set_u32l(lw(m, g[AT], -0x3058) as u32);
            g[AT] = li(0x800B_0000);
            if !(f[8].fl() < f[0].fl()) {
                break 'test;
            }
            f[10].set_u32l(lw(m, g[AT], -0x3054) as u32);
            if !(f[0].fl() < f[10].fl()) {
                break 'test;
            }
            g[A0] = lw(m, g[A1], 0x140);
            if g[A0] == 0 {
                break 'test;
            }
            sw(m, g[SP], 0x1C, g[A1]);
            call(imports::func_800183A8, m, ctx);
            let g = &mut ctx.gpr;
            g[A1] = lw(m, g[SP], 0x1C);
            if g[V0] != 0 {
                g[T8] = lh(m, g[V0], 0);
                g[T9] = g[T8] & 8;
                if g[T9] != 0 {
                    g[V0] = 0;
                    break 'done;
                }
            }
        }
        let g = &mut ctx.gpr;
        g[T0] = lw(m, g[A1], 0x64);
        g[V0] = 0;
        g[T1] = sll(g[T0], 6);
        if (g[T1] as i64) < 0 {
            break 'done;
        }
        g[T2] = lh(m, g[A1], 0x10C);
        g[AT] = slt(g[T2], 5);
        if g[AT] != 0 {
            let f = &mut ctx.fpr;
            f[16].set_u32l(lw(m, g[A1], 0x1A0) as u32);
            g[AT] = li(0x404E_0000);
            f[4].set_u32h(g[AT] as u32); // f5
            f[4].set_u32l(0);
            f[18].set_d(f64::from(f[16].fl()));
            if !(f[18].d() < f[4].d()) {
                break 'done;
            }
        }
        g[V0] = 1;
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
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

/// `func_80060668(p)` (**guess**: picks a view or HUD setting from a
/// table): copies the 0x100-byte table at `0x800A5A2C` (8 rows of 4 (x, y)
/// float pairs) to the frame (`sp - 0x100`, in 12-byte steps, then the
/// last word) and sets four globals, `A = [0x800A5B64]`, `B =
/// [0x800A5B68]`, `C = [0x800A5B6C]`, `D = [0x800A6748]`, with `r = [p +
/// 0x1AC]` and `c = [p + 0x1C0]` (words, re-read at every use):
/// 1. `C = -1`, `D = 0`, `A = K0`, `B = 20.0`;
/// 2. `A = table[r][c].x * K1`, then `B = table[r][c].y` (moved as bits).
///    QUIRK: `r` and `c` aren't bounded; outside the table the pair comes
///    from the rest of the stack;
/// 3. if `r == 1` and `c != 3`: `C = 1`, and `D` = 1, 2, 3 for `c` = 0, 1,
///    2; if `r == 3`: `C` = 6 for `c == 1`, 5 for `c == 2`; if `r == 4` and
///    `c != 3`: `C` = 2, 3, 4 for `c` = 0, 1, 2;
/// 4. with `v = [p + 0x1C4]`: `A = A * K2` if `v == -1`, `A * K3` if `v ==
///    1` (`A` re-read);
/// 5. if bit 5 of `[p + 8]` is set, `B = 2.0`.
///
/// `K0`..`K3` are the floats at `0x800AD094`, `98`, `9C`, `A0`.
///
/// Leaves `v1`, `a1`, `a2` = the addresses of `A`, `B`, `C`, `a3 = -1`, `t0
/// = 4`, `t1 = 1`, `t2 = 3`, `t3 = 2`, `v0 = v`, and `t4`..`t9`, `at`,
/// `f4`..`f18` as the path left them.
///
/// Domain: `p` canonical; `table[r][c]` in RDRAM (`sp - 0x100 + 32 r + 8
/// c`, 32-bit); `table[r][c].x` and `K1`, and `A` with `K2`/`K3` when they
/// multiply, not NaN; `p`'s words not overlapping the globals.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80060668(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[SP] = addu(g[SP], (-0x108i64) as u64);
    // v1, a1, a2, t0 = &A, &B, &C, &D; v0 = the frame's table.
    g[T6] = li(0x800A_0000);
    g[V0] = addu(g[SP], 8);
    g[V1] = li(0x800A_0000);
    g[A1] = li(0x800A_0000);
    g[A2] = li(0x800A_0000);
    g[T0] = li(0x800A_0000);
    g[T6] = addu(g[T6], 0x5A2C);
    g[T0] = addu(g[T0], 0x6748);
    g[A2] = addu(g[A2], 0x5B6C);
    g[A1] = addu(g[A1], 0x5B68);
    g[V1] = addu(g[V1], 0x5B64);
    g[A3] = u64::MAX;
    g[T1] = 1;
    g[T9] = addu(g[T6], 0xFC);
    g[T4] = g[V0];
    loop {
        g[AT] = lw(m, g[T6], 0);
        g[T6] = addu(g[T6], 0xC);
        g[T4] = addu(g[T4], 0xC);
        sw(m, g[T4], -0xC, g[AT]);
        g[AT] = lw(m, g[T6], -8);
        sw(m, g[T4], -8, g[AT]);
        g[AT] = lw(m, g[T6], -4);
        sw(m, g[T4], -4, g[AT]);
        if g[T6] == g[T9] {
            break;
        }
    }
    g[AT] = lw(m, g[T6], 0);
    sw(m, g[T4], 0, g[AT]);
    // 1.
    g[AT] = li(0x800B_0000);
    f[4].set_u32l(lw(m, g[AT], -0x2F6C) as u32);
    g[AT] = li(0x41A0_0000);
    f[6].set_u32l(g[AT] as u32);
    sw(m, g[A2], 0, g[A3]);
    sw(m, g[T0], 0, 0);
    sw(m, g[V1], 0, u64::from(f[4].u32l()));
    sw(m, g[A1], 0, u64::from(f[6].u32l()));
    // 2. table[r][c] at v0 + 32 r + 8 c.
    g[T5] = lw(m, g[A0], 0x1AC);
    g[T9] = lw(m, g[A0], 0x1C0);
    g[AT] = li(0x800B_0000);
    g[T8] = sll(g[T5], 5);
    g[T7] = addu(g[V0], g[T8]);
    g[T6] = sll(g[T9], 3);
    g[T4] = addu(g[T7], g[T6]);
    f[8].set_u32l(lw(m, g[T4], 0) as u32);
    f[10].set_u32l(lw(m, g[AT], -0x2F68) as u32);
    g[AT] = li(0x800B_0000);
    f[16].set_fl(f[8].fl() * f[10].fl());
    sw(m, g[V1], 0, u64::from(f[16].u32l()));
    g[T5] = lw(m, g[A0], 0x1AC);
    g[T7] = lw(m, g[A0], 0x1C0);
    g[T8] = sll(g[T5], 5);
    g[T9] = addu(g[V0], g[T8]);
    g[T6] = sll(g[T7], 3);
    g[T4] = addu(g[T9], g[T6]);
    f[18].set_u32l(lw(m, g[T4], 4) as u32);
    sw(m, g[A1], 0, u64::from(f[18].u32l()));
    // 3. r == 1: t7 = r re-read on every path.
    g[T5] = lw(m, g[A0], 0x1AC);
    if g[T1] != g[T5] {
        g[T7] = lw(m, g[A0], 0x1AC);
    } else {
        g[T8] = lw(m, g[A0], 0x1C0);
        g[T2] = 3;
        if g[T2] == g[T8] {
            g[T7] = lw(m, g[A0], 0x1AC);
        } else {
            sw(m, g[A2], 0, g[T1]);
            g[V0] = lw(m, g[A0], 0x1C0);
            g[T3] = 2;
            if g[V0] == 0 {
                sw(m, g[T0], 0, g[T1]);
                g[V0] = lw(m, g[A0], 0x1C0);
            }
            if g[T1] == g[V0] {
                sw(m, g[T0], 0, g[T3]);
                g[V0] = lw(m, g[A0], 0x1C0);
            }
            g[T3] = 2;
            if g[T3] == g[V0] {
                sw(m, g[T0], 0, g[T2]);
            }
            g[T7] = lw(m, g[A0], 0x1AC);
        }
    }
    // r == 3.
    g[T2] = 3;
    g[T3] = 2;
    if g[T2] == g[T7] {
        g[V0] = lw(m, g[A0], 0x1C0);
        g[T9] = 6;
        g[T6] = 5;
        if g[T1] == g[V0] {
            sw(m, g[A2], 0, g[T9]);
            g[V0] = lw(m, g[A0], 0x1C0);
        }
        if g[T3] == g[V0] {
            sw(m, g[A2], 0, g[T6]);
        }
    }
    // r == 4.
    g[T4] = lw(m, g[A0], 0x1AC);
    g[T0] = 4;
    if g[T0] == g[T4] {
        g[V0] = lw(m, g[A0], 0x1C0);
        if g[T2] != g[V0] {
            if g[V0] == 0 {
                sw(m, g[A2], 0, g[T3]);
                g[V0] = lw(m, g[A0], 0x1C0);
            }
            if g[T1] == g[V0] {
                sw(m, g[A2], 0, g[T2]);
                g[V0] = lw(m, g[A0], 0x1C0);
            }
            if g[T3] == g[V0] {
                sw(m, g[A2], 0, g[T0]);
            }
        }
    }
    // 4.
    g[V0] = lw(m, g[A0], 0x1C4);
    if g[A3] != g[V0] {
        g[AT] = li(0x800B_0000);
        if g[T1] == g[V0] {
            f[10].set_u32l(lw(m, g[V1], 0) as u32);
            f[16].set_u32l(lw(m, g[AT], -0x2F60) as u32);
            f[18].set_fl(f[10].fl() * f[16].fl());
            sw(m, g[V1], 0, u64::from(f[18].u32l()));
        }
    } else {
        f[4].set_u32l(lw(m, g[V1], 0) as u32);
        f[6].set_u32l(lw(m, g[AT], -0x2F64) as u32);
        f[8].set_fl(f[4].fl() * f[6].fl());
        sw(m, g[V1], 0, u64::from(f[8].u32l()));
    }
    // 5.
    g[T5] = lw(m, g[A0], 8);
    g[AT] = li(0x4000_0000);
    g[T8] = g[T5] & 0x20;
    if g[T8] != 0 {
        f[4].set_u32l(g[AT] as u32);
        sw(m, g[A1], 0, u64::from(f[4].u32l()));
    }
    g[SP] = addu(g[SP], 0x108);
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

/// [`func_80063D0C`]'s four entries per pass: the x register, the two y
/// registers, in the C's rotation.
const POINT_SLOTS: [(usize, usize, usize); 4] = [(4, 6, 8), (10, 16, 18), (4, 6, 8), (10, 16, 18)];

/// `func_80063D0C(p)`: the index of the first of the 16 entries at
/// `0x800A5100` (12 bytes, x and y floats first) equal to the point `[p +
/// 0x50]`, `[p + 0x54]`, or -1. Compares are float `==` (so ±0 match, NaN
/// never), x first; y is re-read from `p` for each x that matches. IDO
/// unrolled the loop by four, with two register sets taking turns
/// ([`POINT_SLOTS`]).
///
/// Leaves `f0` = the point's x, `a1 = 16`, `v1` = the pass's first index
/// (16 at the end), and the last registers of the slots it reached.
///
/// Domain: `p` canonical; any float values.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80063D0C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    // v0 = the pass's first entry, v1 its index.
    g[V0] = li(0x800A_5100);
    g[V1] = 0;
    f[0].set_u32l(lw(m, g[A0], 0x50) as u32);
    g[A1] = 0x10;
    loop {
        f[4].set_u32l(lw(m, g[V0], 0) as u32);
        for (k, &(x, ya, yb)) in POINT_SLOTS.iter().enumerate() {
            let off = 0xC * k as i32;
            if f[0].fl() == f[x].fl() {
                f[ya].set_u32l(lw(m, g[A0], 0x54) as u32);
                f[yb].set_u32l(lw(m, g[V0], off + 4) as u32);
                if f[ya].fl() == f[yb].fl() {
                    g[V0] = addu(g[V1], k as u64);
                    return;
                }
            }
            if let Some(&(next, _, _)) = POINT_SLOTS.get(k + 1) {
                f[next].set_u32l(lw(m, g[V0], off + 0xC) as u32);
            }
        }
        g[V1] = addu(g[V1], 4);
        g[V0] = addu(g[V0], 0x30);
        if g[V1] == g[A1] {
            break;
        }
    }
    g[V0] = u64::MAX;
}

/// The float at `0x80120BF8`, next to [`FRAME_TIME`] (the frame time in
/// single precision? **guess**).
pub const FRAME_DT: u32 = 0x8012_0BF8;

/// `func_80064A88(x)`: `v = [0x8011A240] + x * dt` ([`FRAME_DT`]), stored,
/// then clamped to `0..1`: if `1 < v`, 1.0 is stored; then if `v < 0`, 0.0
/// is. Each store is read back. Returns `f0` = the value.
///
/// Leaves `v0 = 0x8011A240`, `at = 0x80120000`, `f2 = f0`, `f4` = the old
/// value, `f6 = dt`, `f8 = x * dt`, `f10` = the unclamped sum.
///
/// Domain: `x`, `dt` and the old value not NaN (a NaN sum is stored and
/// returned: only compared).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80064A88(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x3F80_0000);
    f[0].set_u32l(g[AT] as u32);
    g[AT] = li(0x8012_0000);
    f[6].set_u32l(lw(m, g[AT], 0xBF8) as u32);
    g[V0] = li(0x8011_A240);
    f[8].set_fl(f[12].fl() * f[6].fl());
    f[4].set_u32l(lw(m, g[V0], 0) as u32);
    f[10].set_fl(f[4].fl() + f[8].fl());
    sw(m, g[V0], 0, u64::from(f[10].u32l()));
    f[2].set_u32l(lw(m, g[V0], 0) as u32);
    if f[0].fl() < f[2].fl() {
        sw(m, g[V0], 0, u64::from(f[0].u32l()));
        f[2].set_u32l(lw(m, g[V0], 0) as u32);
    }
    f[0].set_u32l(0);
    if f[2].fl() < f[0].fl() {
        sw(m, g[V0], 0, u64::from(f[0].u32l()));
        f[2].set_u32l(lw(m, g[V0], 0) as u32);
    }
    f[0].set_u32l(f[2].u32l());
}

/// `func_80064AF4()`: counts the float `[0x8011A278]` down by dt
/// ([`FRAME_DT`]) while positive: if `0 < v`, `v - dt` is stored, read
/// back, and replaced by 0.0 if negative.
///
/// Leaves `v0 = 0x8011A240`, `at = 0x80120000`, `f2 = 0.0`, `f0` = the old
/// value, and on a count `f4 = dt`, `f6` = the difference, `f8` its read.
///
/// Domain: when the old value is positive, it and `dt` not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80064AF4(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = li(0x8011_A240);
    f[2].set_u32l(0);
    f[0].set_u32l(lw(m, g[V0], 0x38) as u32);
    g[AT] = li(0x8012_0000);
    if f[2].fl() < f[0].fl() {
        f[4].set_u32l(lw(m, g[AT], 0xBF8) as u32);
        f[6].set_fl(f[0].fl() - f[4].fl());
        sw(m, g[V0], 0x38, u64::from(f[6].u32l()));
        f[8].set_u32l(lw(m, g[V0], 0x38) as u32);
        if f[8].fl() < f[2].fl() {
            sw(m, g[V0], 0x38, u64::from(f[2].u32l()));
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

/// `func_80065C98(p, x)`: `[p + 0x68] = x` (a float, passed in `a1`)
/// unless `p` is null. Leaves `f12` = `x`'s bits.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80065C98(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    ctx.fpr[12].set_u32l(g[A1] as u32);
    if g[A0] != 0 {
        sw(m, g[A0], 0x68, u64::from(ctx.fpr[12].u32l()));
    }
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

/// `func_80065E18(p)`: `[p + 0x190] = [0x800A3080] * 0.25`, then the three
/// words at `0x800A3084` copied to `p + 0x194..` (as bits).
///
/// Leaves `v0 = 0x800A3084`, `at = 0x3E800000`, `f4` the scaled float, `f6
/// = 0.25`, `f8` the product, `f10`/`f16`/`f18` the copied words.
///
/// Domain: `p` canonical, not overlapping `0x800A3084..90` (re-read after
/// the first store); the scaled float not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80065E18(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x800A_0000);
    f[4].set_u32l(lw(m, g[AT], 0x3080) as u32);
    g[AT] = li(0x3E80_0000);
    f[6].set_u32l(g[AT] as u32);
    g[V0] = li(0x800A_3084);
    f[8].set_fl(f[4].fl() * f[6].fl());
    sw(m, g[A0], 0x190, u64::from(f[8].u32l()));
    f[10].set_u32l(lw(m, g[V0], 0) as u32);
    sw(m, g[A0], 0x194, u64::from(f[10].u32l()));
    f[16].set_u32l(lw(m, g[V0], 4) as u32);
    sw(m, g[A0], 0x198, u64::from(f[16].u32l()));
    f[18].set_u32l(lw(m, g[V0], 8) as u32);
    sw(m, g[A0], 0x19C, u64::from(f[18].u32l()));
}

/// [`func_8006B304`]'s six middle bands, top down: the threshold's offset
/// in the table at `0x8011C868`, the band's bit, and the registers the C
/// rotates through (period 3): the upper bound, the threshold, the lower
/// bound, the next band's threshold, and the bit's temporary.
const GRID_BANDS: [(i32, u32, usize, usize, usize, usize, usize); 6] = [
    (0x18, 0x40_0000, 16, 18, 4, 6, T6),
    (0x14, 0x20_0000, 8, 10, 16, 18, T7),
    (0x10, 0x10_0000, 4, 6, 8, 10, T8),
    (0xC, 0x8_0000, 16, 18, 4, 6, T9),
    (8, 0x4_0000, 8, 10, 16, 18, T0),
    (4, 0x2_0000, 4, 6, 8, 10, T1),
];

/// `func_8006B304(p)` (**guess**: which cells of a 3 x 8 grid a point
/// overlaps): with `x = [p + 0x50]`, `y = [p + 0x54]`, the thresholds `T[k]
/// = [0x8011C868 + 4k]` (k = 1..7), `h = [0x8011C890]`, `U[k] = [0x8011C858
/// + 4k]` (k = 1, 2) and `hx = [0x8011C88C]` (floats), stores to `[p +
/// 0x26C]` the mask `m`:
/// - rows (`r`): bit 23 if `T[7] - h < y`; bit `16 + k` (k = 6..1) if `y <
///   T[k + 1] + h` and `T[k] - h < y`; bit 16 if `y < T[1] + h`;
/// - columns: `m = (r >> 8 if U[2] - hx < x) | (r if x < U[2] + hx and U[1]
///   - hx < x) | (r << 8 if x < U[1] + hx)`.
///
/// Each bound is `T ± h` in f32, compared with `<`. `x` and `y` are
/// spilled to the frame (`sp - 0xC`, `sp - 8`) and `y` read back.
///
/// Leaves `f0 = x`, `f12 = hx`, `f2 = U[2]`, `f18`/`f4` its bounds, `a1 =
/// 0x8011C858`, `v1 = m`, `v0 = r`, `t3 = r << 8`, `t2 = r | 0x10000`, `at
/// = 0x80120000`, `f16 = U[1] + hx`, and `f6`..`f10`, `t0`, `t1`, `t6`..`t9`
/// from the rows as they went.
///
/// Domain: `p` canonical; the thresholds and half-widths not NaN where
/// they are added (`x` and `y` are only compared: any value).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8006B304(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A0], 0x50) as u32);
    g[SP] = addu(g[SP], (-0x10i64) as u64);
    // v1 = T, f12 = h, f2 = T[7], f10 = y.
    g[V1] = li(0x8011_C868);
    g[AT] = li(0x8012_0000);
    sw(m, g[SP], 4, u64::from(f[4].u32l()));
    f[6].set_u32l(lw(m, g[A0], 0x54) as u32);
    f[12].set_u32l(lw(m, g[AT], -0x3770) as u32);
    f[2].set_u32l(lw(m, g[V1], 0x1C) as u32);
    sw(m, g[SP], 8, u64::from(f[6].u32l()));
    f[10].set_u32l(lw(m, g[SP], 8) as u32);
    f[8].set_fl(f[2].fl() - f[12].fl());
    g[A1] = li(0x8012_0000);
    g[V0] = 0;
    g[A1] = addu(g[A1], (-0x37A8i64) as u64);
    // Rows: the top band, then the middle ones, then the bottom.
    if f[8].fl() < f[10].fl() {
        g[V0] = li(0x80_0000);
    }
    f[16].set_fl(f[2].fl() + f[12].fl());
    f[0].set_u32l(lw(m, g[SP], 8) as u32);
    let mut above = 16; // the register holding the band's upper bound
    for (k, &(off, bit, upper, t, lower, next, tmp)) in GRID_BANDS.iter().enumerate() {
        if k > 0 {
            f[upper].set_fl(f[above].fl() + f[12].fl());
        }
        if f[0].fl() < f[upper].fl() {
            f[t].set_u32l(lw(m, g[V1], off) as u32);
            g[AT] = li(bit);
            g[tmp] = g[V0] | g[AT];
            f[lower].set_fl(f[t].fl() - f[12].fl());
            if f[lower].fl() < f[0].fl() {
                g[V0] = g[tmp];
            }
        }
        f[next].set_u32l(lw(m, g[V1], off) as u32);
        above = next;
    }
    g[AT] = li(0x1_0000);
    g[T2] = g[V0] | g[AT];
    f[16].set_fl(f[10].fl() + f[12].fl());
    if f[0].fl() < f[16].fl() {
        g[V0] = g[T2];
    }
    // Columns: f12 = hx, f2 = U[2], f0 = x.
    g[AT] = li(0x8012_0000);
    f[12].set_u32l(lw(m, g[AT], -0x3774) as u32);
    f[2].set_u32l(lw(m, g[A1], 8) as u32);
    f[0].set_u32l(lw(m, g[SP], 4) as u32);
    g[V1] = 0;
    f[18].set_fl(f[2].fl() - f[12].fl());
    g[T3] = sll(g[V0], 8);
    g[SP] = addu(g[SP], 0x10);
    f[4].set_fl(f[2].fl() + f[12].fl());
    if f[18].fl() < f[0].fl() {
        g[V1] = sra(g[V0], 8);
    }
    if f[0].fl() < f[4].fl() {
        f[6].set_u32l(lw(m, g[A1], 4) as u32);
        f[8].set_fl(f[6].fl() - f[12].fl());
        if f[8].fl() < f[0].fl() {
            g[V1] = g[V1] | g[V0];
        }
    }
    f[10].set_u32l(lw(m, g[A1], 4) as u32);
    f[16].set_fl(f[10].fl() + f[12].fl());
    if f[0].fl() < f[16].fl() {
        g[V1] = g[V1] | g[T3];
    }
    sw(m, g[A0], 0x26C, g[V1]);
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

/// `func_8006C828(p)` (**guess**: a balance of six flags): `f0` = the sum,
/// in this order, of a step for each of the floats `v[i] = [p + 0x288 +
/// 4i]` above `K` (compared in double: `K < f64(v[i])`, `K` the double at
/// `0x800AD600`): `-S0` for `i = 0`, `-S` for `i` = 1, 2, `+S` for `i` =
/// 3..5 (`S0`, `S` the floats at `0x800AD608`, `0x800AD60C`). The first
/// step is `0.0 - S0`.
///
/// The C is a loop the compiler unrolled around a constant index (`v1 =
/// 8`): the tests of it (`at` from `slt`, `v1 < 0`) always go the same
/// way, so the port keeps the `at` values and leaves the dead arms out.
///
/// Leaves `f2 = f0`, `f12 = K`, `f14 = S`, `v0 = 2`, `v1 = 8`, `a1 = p +
/// 8`, `at = 0`, and `f4`..`f18` the loads and conversions.
///
/// Domain: `p` canonical; the six floats not NaN (each is converted to
/// double, a guarded operation); `S0`, `S` not NaN where they are used.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8006C828(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[A0], 0x288) as u32);
    g[AT] = li(0x800B_0000);
    f[12].u64 = ld(m, g[AT], -0x2A00);
    f[6].set_d(f64::from(f[4].fl()));
    f[0].set_u32l(0);
    let c = f[12].d() < f[6].d();
    g[AT] = li(0x800B_0000);
    g[V0] = g[A0];
    f[2].set_u32l(f[0].u32l());
    if c {
        f[14].set_u32l(lw(m, g[AT], -0x29F8) as u32);
        f[2].set_fl(f[0].fl() - f[14].fl());
    }
    f[8].set_u32l(lw(m, g[V0], 0x28C) as u32);
    g[V0] = 2;
    g[AT] = li(0x800B_0000);
    f[10].set_d(f64::from(f[8].fl()));
    f[14].set_u32l(lw(m, g[AT], -0x29F4) as u32);
    let c = f[12].d() < f[10].d();
    g[V1] = sll(g[V0], 2);
    g[A1] = addu(g[A0], g[V1]);
    g[AT] = slt(g[V1], 0xC);
    if c {
        f[2].set_fl(f[2].fl() - f[14].fl());
    }
    // v[2]: at = (8 < 12) = 1, subtract.
    f[16].set_u32l(lw(m, g[A1], 0x288) as u32);
    f[18].set_d(f64::from(f[16].fl()));
    if f[12].d() < f[18].d() {
        debug_assert!(g[AT] != 0);
        f[2].set_fl(f[2].fl() - f[14].fl());
    }
    // v[3]..v[5]: at = (8 < 8), (8 < 4) = 0 and v1 >= 0, add.
    f[4].set_u32l(lw(m, g[A1], 0x28C) as u32);
    g[AT] = slt(g[V1], 8);
    f[6].set_d(f64::from(f[4].fl()));
    if f[12].d() < f[6].d() {
        debug_assert!(g[AT] == 0);
        f[2].set_fl(f[2].fl() + f[14].fl());
    }
    f[8].set_u32l(lw(m, g[A1], 0x290) as u32);
    g[AT] = slt(g[V1], 4);
    f[10].set_d(f64::from(f[8].fl()));
    if f[12].d() < f[10].d() {
        debug_assert!(g[AT] == 0);
        f[2].set_fl(f[2].fl() + f[14].fl());
    }
    f[16].set_u32l(lw(m, g[A1], 0x294) as u32);
    f[18].set_d(f64::from(f[16].fl()));
    if f[12].d() < f[18].d() {
        debug_assert!((g[V1] as i64) >= 0);
        f[2].set_fl(f[2].fl() + f[14].fl());
    }
    f[0].set_u32l(f[2].u32l());
}

/// `func_8006D0C0(p, a1, q, flag)`: `[p + 0x2FC]` = -1.0 if `400 < d`,
/// else 0.0, where `d` = 0.0 if `[q + 8] < [p + 0x58]`; else 500.0 if bit
/// 9 of `[p + 0x64]` is set or `flag == 0`, else the float `[p + 0x184]`.
/// It also spills `a1` to its home slot (`sp + 4`).
///
/// Leaves `at = 0xBF800000`, `f0 = d`, `f4`, `f6` the compared pair, `f8
/// = 400.0`, and `f10 = -1.0` or `f16 = 0.0`; `t6`/`t7` the flag word and
/// bit when read.
///
/// Domain: `p`, `q` canonical; any float values (only compared).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8006D0C0(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    sw(m, g[SP], 4, g[A1]);
    f[6].set_u32l(lw(m, g[A0], 0x58) as u32);
    f[4].set_u32l(lw(m, g[A2], 8) as u32);
    if f[4].fl() < f[6].fl() {
        f[0].set_u32l(0);
    } else {
        g[T6] = lw(m, g[A0], 0x64);
        g[AT] = li(0x43FA_0000); // 500.0
        g[T7] = g[T6] & 0x200;
        if g[T7] == 0 && g[A3] != 0 {
            f[0].set_u32l(lw(m, g[A0], 0x184) as u32);
        } else {
            f[0].set_u32l(g[AT] as u32);
        }
    }
    g[AT] = li(0x43C8_0000); // 400.0
    f[8].set_u32l(g[AT] as u32);
    g[AT] = li(0xBF80_0000);
    if f[8].fl() < f[0].fl() {
        f[10].set_u32l(g[AT] as u32);
        sw(m, g[A0], 0x2FC, u64::from(f[10].u32l()));
    } else {
        f[16].set_u32l(0);
        sw(m, g[A0], 0x2FC, u64::from(f[16].u32l()));
    }
}

/// `func_8006D9DC(p, target)` (**guess**: an eased value): moves `v = [p +
/// 0x208]` toward `target` (a float, passed in `a1`; 0.0 instead when the
/// float `[p + 0x1A0] < 200`) by `rate * f32(dt)` (`dt` the double at
/// [`FRAME_TIME`], narrowed; `rate` the float at `0x800AD6A4` going up,
/// `0x800AD6A8` going down): `v + step` (or `v - step`) is stored, read
/// back, and replaced by `target` if it passed it. Then, if `f64(target) ==
/// 0.0` and `|v| < E` (in double, `E` at `0x800AD6B0`, `v` re-read), `v =
/// f32(f64(v) * 0.5)` is stored.
///
/// Leaves `f12 = target`, `f8`/`f9` = 0.0 (the high half written on every
/// path, the low one after), `f10 = f64(target)`, `at = 0x800B0000` (or
/// `0x3FE00000` after a halving), and `f0`, `f2`, `f4`, `f6`, `f16`, `f18`
/// as the path left them.
///
/// Domain: `p` canonical; no NaN operands: `target` and `v` (both
/// converted to double), `dt`, `rate` and `E` where used.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8006D9DC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x4348_0000); // 200.0
    f[6].set_u32l(g[AT] as u32);
    f[4].set_u32l(lw(m, g[A0], 0x1A0) as u32);
    f[12].set_u32l(g[A1] as u32);
    g[AT] = li(0x800B_0000);
    if f[4].fl() < f[6].fl() {
        f[12].set_u32l(0);
    }
    f[0].set_u32l(lw(m, g[A0], 0x208) as u32);
    if f[0].fl() < f[12].fl() {
        // Up, clamped at the target.
        f[8].set_u32l(lw(m, g[AT], -0x295C) as u32);
        g[AT] = li(0x8012_0000);
        f[10].u64 = ld(m, g[AT], 0xBF0);
        f[16].set_fl(fpu::cvt_s_d(f[10].d(), fpu::NEAREST));
        f[18].set_fl(f[8].fl() * f[16].fl());
        f[4].set_fl(f[0].fl() + f[18].fl());
        sw(m, g[A0], 0x208, u64::from(f[4].u32l()));
        f[6].set_u32l(lw(m, g[A0], 0x208) as u32);
        if f[12].fl() < f[6].fl() {
            sw(m, g[A0], 0x208, u64::from(f[12].u32l()));
        }
    } else {
        g[AT] = li(0x800B_0000);
        if f[12].fl() < f[0].fl() {
            // Down, clamped at the target.
            f[10].set_u32l(lw(m, g[AT], -0x2958) as u32);
            g[AT] = li(0x8012_0000);
            f[8].u64 = ld(m, g[AT], 0xBF0);
            f[16].set_fl(fpu::cvt_s_d(f[8].d(), fpu::NEAREST));
            f[18].set_fl(f[10].fl() * f[16].fl());
            f[4].set_fl(f[0].fl() - f[18].fl());
            sw(m, g[A0], 0x208, u64::from(f[4].u32l()));
            f[6].set_u32l(lw(m, g[A0], 0x208) as u32);
            if f[6].fl() < f[12].fl() {
                sw(m, g[A0], 0x208, u64::from(f[12].u32l()));
            }
        }
    }
    // A zero target halves a small value.
    f[8].set_u32h(0);
    f[8].set_u32l(0);
    f[10].set_d(f64::from(f[12].fl()));
    g[AT] = li(0x800B_0000);
    if f[8].d() == f[10].d() {
        f[0].set_u32l(lw(m, g[A0], 0x208) as u32);
        f[16].set_u32l(0);
        if f[0].fl() < f[16].fl() {
            f[2].set_fl(-f[0].fl());
        } else {
            f[2].set_u32l(f[0].u32l());
        }
        f[4].u64 = ld(m, g[AT], -0x2950);
        f[18].set_d(f64::from(f[2].fl()));
        g[AT] = li(0x3FE0_0000);
        if f[18].d() < f[4].d() {
            f[8].set_u32h(g[AT] as u32); // 0.5
            f[8].set_u32l(0);
            f[6].set_d(f64::from(f[0].fl()));
            f[10].set_d(f[6].d() * f[8].d());
            f[16].set_fl(fpu::cvt_s_d(f[10].d(), fpu::NEAREST));
            sw(m, g[A0], 0x208, u64::from(f[16].u32l()));
        }
    }
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

/// `func_8006E2FC(p, rgb, rgba, out)` (**guess**: a gauge's colours and
/// fill by mode): first `rgb = (0, 0xFF, 0)`, `rgba = (0xFF, 0xFF, 0xFF,
/// 0x64)` (bytes) and `*out = 0.0`; then by the mode `[p + 0x210]`:
/// - 0: the same colours again, then `*out = f32(f64([p + 0x1A0]) /
///   (f64([p + 0x7C]) * 0.75))`, read back and clamped to 1.0 if above;
/// - 1: `rgb = (0, 0xFF, 0)`, `rgba = (0xFF, 0x80, 0, 0xC8)`, `*out = [p
///   + 0x214] * 1.0`;
/// - 2: `rgb = (0xFF, 0xFF, 0)`, `rgba = (0xFF, 0x80, 0, 0xC8)`, `*out =
///   1.0`;
/// - otherwise nothing more.
///
/// Leaves `v0` = the mode, `v1 = 0xFF`, `t0 = 0x64`, `f4 = 0.0` (mode 1:
/// the product), `at`, `t6`..`t9`, `f0`, `f6`..`f18` as the mode's path
/// left them.
///
/// Domain: `p`, `rgb`, `rgba`, `out` canonical and disjoint; in mode 0
/// `[p + 0x7C]` and `[p + 0x1A0]` not NaN, in mode 1 `[p + 0x214]` not
/// NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8006E2FC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V1] = 0xFF;
    sb(m, g[A1], 0, 0);
    sb(m, g[A1], 1, g[V1]);
    sb(m, g[A1], 2, 0);
    f[4].set_u32l(0);
    g[T0] = 0x64;
    sb(m, g[A2], 0, g[V1]);
    sb(m, g[A2], 1, g[V1]);
    sb(m, g[A2], 2, g[V1]);
    sb(m, g[A2], 3, g[T0]);
    sw(m, g[A3], 0, u64::from(f[4].u32l()));
    g[V0] = lw(m, g[A0], 0x210);
    g[AT] = li(0x3F80_0000);
    if g[V0] == 0 {
        sb(m, g[A1], 0, 0);
        sb(m, g[A1], 1, g[V1]);
        sb(m, g[A1], 2, 0);
        sb(m, g[A2], 0, g[V1]);
        sb(m, g[A2], 1, g[V1]);
        sb(m, g[A2], 2, g[V1]);
        sb(m, g[A2], 3, g[T0]);
        f[0].set_u32l(g[AT] as u32);
        f[6].set_u32l(lw(m, g[A0], 0x7C) as u32);
        g[AT] = li(0x3FE8_0000);
        f[10].set_u32h(g[AT] as u32); // 0.75
        f[10].set_u32l(0);
        f[8].set_d(f64::from(f[6].fl()));
        f[18].set_u32l(lw(m, g[A0], 0x1A0) as u32);
        f[16].set_d(f[8].d() * f[10].d());
        f[4].set_d(f64::from(f[18].fl()));
        f[6].set_d(f[4].d() / f[16].d());
        f[8].set_fl(fpu::cvt_s_d(f[6].d(), fpu::NEAREST));
        sw(m, g[A3], 0, u64::from(f[8].u32l()));
        f[10].set_u32l(lw(m, g[A3], 0) as u32);
        if f[0].fl() < f[10].fl() {
            sw(m, g[A3], 0, u64::from(f[0].u32l()));
        }
    } else {
        g[AT] = 1;
        g[T6] = 0x80;
        if g[V0] == g[AT] {
            sb(m, g[A1], 0, 0);
            sb(m, g[A1], 1, g[V1]);
            sb(m, g[A1], 2, 0);
            g[T7] = 0xC8;
            g[AT] = li(0x3F80_0000);
            sb(m, g[A2], 0, g[V1]);
            sb(m, g[A2], 1, g[T6]);
            sb(m, g[A2], 2, 0);
            sb(m, g[A2], 3, g[T7]);
            f[0].set_u32l(g[AT] as u32);
            f[18].set_u32l(lw(m, g[A0], 0x214) as u32);
            f[4].set_fl(f[18].fl() * f[0].fl());
            sw(m, g[A3], 0, u64::from(f[4].u32l()));
            return;
        }
        g[AT] = 2;
        g[T8] = 0x80;
        if g[V0] == g[AT] {
            sb(m, g[A1], 0, g[V1]);
            sb(m, g[A1], 1, g[V1]);
            sb(m, g[A1], 2, 0);
            g[AT] = li(0x3F80_0000);
            f[0].set_u32l(g[AT] as u32);
            g[T9] = 0xC8;
            sb(m, g[A2], 0, g[V1]);
            sb(m, g[A2], 1, g[T8]);
            sb(m, g[A2], 2, 0);
            sb(m, g[A2], 3, g[T9]);
            sw(m, g[A3], 0, u64::from(f[0].u32l()));
        }
    }
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

/// `func_80071820(p, i, x)` (**guess**: charges one of six meters): `x` is
/// a float passed in `a2`. Unless `[0x800A52D4] != 0`, or bit 14 or 13 of
/// `[p + 0x60]` is set, or bit 25 of `[p + 0x64]` is (tested as the sign of
/// `[p + 0x64] << 6`):
/// - bit 23 of `[p + 0x60]` is cleared;
/// - with `q = p + 4i`: `[q + 0x288] += x * [p + 0xA0]`, stored, read back,
///   and clamped to 1.0 (stored and read back) if above;
/// - bit 0 of the word `[q + 0x2A0]` is set;
/// - `[q + 0x270]` = the value if it is below it (a maximum);
/// - `[p + 0x2C4] += x`.
///
/// Leaves `f12 = x`, `t6` the gate word, `v0` = `[p + 0x60]`, `t7`/`t8`
/// its bits; past the bit tests `t9`, `t0`, `t2 = 4i`, `v1 = q`, `at`
/// (`0x3F800000`, then `0xFF7FFFFF`), `t1` the cleared word, `t3`/`t4` the
/// bit word, `f0` the value, `f2 = 1.0`, `f4`..`f18` the loads and sums.
///
/// Domain: `p` and `q` canonical; `x`, `[p + 0xA0]`, `[q + 0x288]` and
/// `[p + 0x2C4]` not NaN when they are used (the compares take anything).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80071820(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = li(0x800A_0000);
    g[T6] = lw(m, g[T6], 0x52D4);
    f[12].set_u32l(g[A2] as u32);
    if g[T6] != 0 {
        return;
    }
    g[V0] = lw(m, g[A0], 0x60);
    g[T7] = g[V0] & 0x4000;
    g[T8] = g[V0] & 0x2000;
    if g[T7] != 0 || g[T8] != 0 {
        return;
    }
    g[T9] = lw(m, g[A0], 0x64);
    g[T2] = sll(g[A1], 2);
    g[AT] = li(0x3F80_0000);
    g[T0] = sll(g[T9], 6);
    g[V1] = addu(g[A0], g[T2]);
    if (g[T0] as i64) < 0 {
        return;
    }
    f[6].set_u32l(lw(m, g[A0], 0xA0) as u32);
    f[2].set_u32l(g[AT] as u32);
    g[AT] = li(0xFF7F_0000);
    f[8].set_fl(f[12].fl() * f[6].fl());
    g[AT] = g[AT] | 0xFFFF;
    g[T1] = g[V0] & g[AT];
    sw(m, g[A0], 0x60, g[T1]);
    f[4].set_u32l(lw(m, g[V1], 0x288) as u32);
    f[10].set_fl(f[4].fl() + f[8].fl());
    sw(m, g[V1], 0x288, u64::from(f[10].u32l()));
    f[0].set_u32l(lw(m, g[V1], 0x288) as u32);
    if f[2].fl() < f[0].fl() {
        sw(m, g[V1], 0x288, u64::from(f[2].u32l()));
        f[0].set_u32l(lw(m, g[V1], 0x288) as u32);
    }
    f[16].set_u32l(lw(m, g[V1], 0x270) as u32);
    g[T3] = lw(m, g[V1], 0x2A0);
    let above = f[16].fl() < f[0].fl();
    g[T4] = g[T3] | 1;
    sw(m, g[V1], 0x2A0, g[T4]);
    if above {
        sw(m, g[V1], 0x270, u64::from(f[0].u32l()));
    }
    f[18].set_u32l(lw(m, g[A0], 0x2C4) as u32);
    f[6].set_fl(f[18].fl() + f[12].fl());
    sw(m, g[A0], 0x2C4, u64::from(f[6].u32l()));
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

/// [`func_80073834`]'s five slots: the register its flag is loaded into,
/// the float it adds to (`p + off`), and the pair of registers the C uses.
const BUMP_SLOTS: [(usize, i32, usize, usize); 5] = [(T6, 0x3C8, 4, 6), (T7, 0x408, 8, 10), (T8, 0x448, 16, 18), (T9, 0x488, 4, 6), (T0, 0x4C8, 8, 10)];

/// `func_80073834(p)`: with `q = [p + 0x344]` and `d = [p + 0x250]`: for
/// k = 1..5, if the word `[q + 4k] != 0`, the float `[p + 0x388 + 0x40k]
/// += d`. `q` is re-read from `p` after each of the first four additions.
///
/// Leaves `v0 = q`, `f0 = d`, and the flags and sums in the slots'
/// registers ([`BUMP_SLOTS`]).
///
/// Domain: `p` and `q` canonical (unless `q` is 0, nothing more is read);
/// `d` and the slots added to not NaN.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80073834(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = lw(m, g[A0], 0x344);
    f[0].set_u32l(lw(m, g[A0], 0x250) as u32);
    if g[V0] == 0 {
        return;
    }
    g[T6] = lw(m, g[V0], 4);
    for (k, &(flag, off, a, b)) in BUMP_SLOTS.iter().enumerate() {
        let last = k + 1 == BUMP_SLOTS.len();
        if g[flag] != 0 {
            f[a].set_u32l(lw(m, g[A0], off) as u32);
            if !last {
                g[V0] = lw(m, g[A0], 0x344);
            }
            f[b].set_fl(f[a].fl() + f[0].fl());
            sw(m, g[A0], off, u64::from(f[b].u32l()));
        }
        if let Some(&(next, ..)) = BUMP_SLOTS.get(k + 1) {
            g[next] = lw(m, g[V0], 4 * (k as i32 + 2));
        }
    }
}

/// `func_80073C58(v, c, lo, hi, up, down)`: `v` clamped, in this order, to
/// at most `c + up`, at least `c - down`, at least `lo`, at most `hi`
/// (each a `<` compare, so NaN bounds are skipped), returned in `f0`. All
/// floats: `v`, `c` in `f12`/`f14`, `lo`, `hi` in `a2`/`a3` (spilled to
/// their home slots `sp + 8`/`sp + 0xC` and read back), `up`, `down` on
/// the stack.
///
/// Leaves `f12 = f0`, `f4 = up`, `f6 = down`, `f8 = lo`, `f10 = hi`.
///
/// Domain: `c`, `up`, `down` not NaN (added); the rest any value.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80073C58(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    f[4].set_u32l(lw(m, g[SP], 0x10) as u32);
    sw(m, g[SP], 8, g[A2]);
    sw(m, g[SP], 0xC, g[A3]);
    f[0].set_fl(f[14].fl() + f[4].fl());
    f[6].set_u32l(lw(m, g[SP], 0x14) as u32);
    f[8].set_u32l(lw(m, g[SP], 8) as u32);
    f[10].set_u32l(lw(m, g[SP], 0xC) as u32);
    if f[0].fl() < f[12].fl() {
        f[12].set_u32l(f[0].u32l());
    }
    f[0].set_fl(f[14].fl() - f[6].fl());
    if f[12].fl() < f[0].fl() {
        f[12].set_u32l(f[0].u32l());
    }
    if f[12].fl() < f[8].fl() {
        f[12].set_u32l(f[8].u32l());
    }
    if f[10].fl() < f[12].fl() {
        f[12].set_u32l(f[10].u32l());
    }
    f[0].set_u32l(f[12].u32l());
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

/// The grid [`func_8006B304`] reads: `U[0..3]` (4 floats) and, 0x10 bytes
/// on, `T[0..8]` (9), then the half-widths `hx`, `h` at `0x8011C88C`/`90`.
pub const GRID: u32 = 0x8011_C858;

/// `func_8007DA20()` (grid setup, **guess**): picks four floats by the mode
/// `[0x800A5B5C]`: `(C, D, A, B)` = the floats at `0x800ADC10` for mode 1,
/// `0x800ADC20` for mode 2 (through the frame: `C` at `sp - 0xC`, `D` at
/// `- 0x10`, `A` at `- 0x18`, `B` at `- 0x14`). QUIRK: any other mode
/// leaves those four frame words as they were, and uses them.
///
/// Then with `t = (A - B) / 3` and `s = (D - C) / 8`, it writes [`GRID`]:
/// `U = (B, B + t, A - t, A)`, `T = (C, C + s, 2s + C, 3s + C, .., 6s + C, D
/// - s, D)` (the multiples `k * s` computed first, then added), `hx = t /
/// 4`, `h = s / 4`. The stores go `U[3]`, `U[0]`, `T[0]`, `T[8]`, `U[2]`,
/// `U[1]`, `T[1]`..`T[7]`, `hx`, `h`.
///
/// Leaves `v0 = GRID`, `v1 = GRID + 0x10`, `at = 0x80120000`, `f0 = C`,
/// `f2 = D`, `f12 = A`, `f14 = B`, `f16 = t`, `f18 = s`, `f4`..`f10` the
/// last products, sums and constants.
///
/// Domain: the four values (from the ROM, or stale for other modes) not
/// NaN, nor any intermediate.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8007DA20(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[V0] = li(0x800A_0000);
    g[V0] = lw(m, g[V0], 0x5B5C);
    g[AT] = 1;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    let mode1 = g[V0] == g[AT];
    g[AT] = li(0x800B_0000);
    // The table for the mode: 0x800ADC10 (1) or 0x800ADC20 (2).
    let table = if mode1 {
        Some(-0x23F0)
    } else {
        g[AT] = 2;
        let mode2 = g[V0] == g[AT];
        g[AT] = li(0x800B_0000);
        mode2.then_some(-0x23E0)
    };
    if let Some(t) = table {
        f[0].set_u32l(lw(m, g[AT], t) as u32);
        g[AT] = li(0x800B_0000);
        f[2].set_u32l(lw(m, g[AT], t + 4) as u32);
        g[AT] = li(0x800B_0000);
        f[12].set_u32l(lw(m, g[AT], t + 8) as u32);
        g[AT] = li(0x800B_0000);
        f[14].set_u32l(lw(m, g[AT], t + 0xC) as u32);
        sw(m, g[SP], 0xC, u64::from(f[0].u32l()));
        sw(m, g[SP], 8, u64::from(f[2].u32l()));
        sw(m, g[SP], 0, u64::from(f[12].u32l()));
        sw(m, g[SP], 4, u64::from(f[14].u32l()));
    }
    // f12 = A, f14 = B, f0 = C, f2 = D; f16 = t, f18 = s; v0 = U, v1 = T.
    f[12].set_u32l(lw(m, g[SP], 0) as u32);
    f[14].set_u32l(lw(m, g[SP], 4) as u32);
    g[AT] = li(0x4040_0000);
    f[6].set_u32l(g[AT] as u32);
    f[4].set_fl(f[12].fl() - f[14].fl());
    f[0].set_u32l(lw(m, g[SP], 0xC) as u32);
    f[2].set_u32l(lw(m, g[SP], 8) as u32);
    g[AT] = li(0x4100_0000);
    f[16].set_fl(f[4].fl() / f[6].fl());
    f[6].set_u32l(g[AT] as u32);
    g[V0] = li(0x8011_C858);
    f[4].set_fl(f[2].fl() - f[0].fl());
    g[AT] = li(0x4000_0000);
    g[V1] = li(0x8011_C868);
    sw(m, g[V0], 0xC, u64::from(f[12].u32l()));
    sw(m, g[V0], 0, u64::from(f[14].u32l()));
    sw(m, g[V1], 0, u64::from(f[0].u32l()));
    sw(m, g[V1], 0x20, u64::from(f[2].u32l()));
    g[SP] = addu(g[SP], 0x18);
    f[18].set_fl(f[4].fl() / f[6].fl());
    f[10].set_fl(f[12].fl() - f[16].fl());
    f[8].set_fl(f[14].fl() + f[16].fl());
    sw(m, g[V0], 8, u64::from(f[10].u32l()));
    f[10].set_u32l(g[AT] as u32);
    g[AT] = li(0x4040_0000);
    sw(m, g[V0], 4, u64::from(f[8].u32l()));
    f[4].set_fl(f[10].fl() * f[18].fl());
    f[8].set_fl(f[0].fl() + f[18].fl());
    sw(m, g[V1], 4, u64::from(f[8].u32l()));
    f[8].set_u32l(g[AT] as u32);
    f[6].set_fl(f[4].fl() + f[0].fl());
    g[AT] = li(0x4080_0000);
    f[10].set_fl(f[8].fl() * f[18].fl());
    sw(m, g[V1], 8, u64::from(f[6].u32l()));
    f[6].set_u32l(g[AT] as u32);
    g[AT] = li(0x40A0_0000);
    f[8].set_fl(f[6].fl() * f[18].fl());
    f[4].set_fl(f[10].fl() + f[0].fl());
    sw(m, g[V1], 0xC, u64::from(f[4].u32l()));
    f[4].set_u32l(g[AT] as u32);
    f[10].set_fl(f[8].fl() + f[0].fl());
    g[AT] = li(0x40C0_0000);
    f[6].set_fl(f[4].fl() * f[18].fl());
    sw(m, g[V1], 0x10, u64::from(f[10].u32l()));
    f[10].set_u32l(g[AT] as u32);
    g[AT] = li(0x4080_0000);
    f[4].set_fl(f[10].fl() * f[18].fl());
    f[10].set_u32l(g[AT] as u32);
    f[8].set_fl(f[6].fl() + f[0].fl());
    g[AT] = li(0x8012_0000);
    sw(m, g[V1], 0x14, u64::from(f[8].u32l()));
    f[6].set_fl(f[4].fl() + f[0].fl());
    f[4].set_fl(f[16].fl() / f[10].fl());
    sw(m, g[V1], 0x18, u64::from(f[6].u32l()));
    f[8].set_fl(f[2].fl() - f[18].fl());
    sw(m, g[V1], 0x1C, u64::from(f[8].u32l()));
    sw(m, g[AT], -0x3774, u64::from(f[4].u32l()));
    g[AT] = li(0x4080_0000);
    f[6].set_u32l(g[AT] as u32);
    g[AT] = li(0x8012_0000);
    f[8].set_fl(f[18].fl() / f[6].fl());
    sw(m, g[AT], -0x3770, u64::from(f[8].u32l()));
}

/// `func_8007EE40()`: returns the float `[0x800A6700]` in `f0`. Leaves `at
/// = 0x800A0000`.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8007EE40(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mem, ctx) = enter(rdram, ctx);
    let m = &mem;
    let g = &mut ctx.gpr;
    g[AT] = li(0x800A_0000);
    ctx.fpr[0].set_u32l(lw(m, g[AT], 0x6700) as u32);
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

/// `func_80080148(i)`: zeroes the ten words at `0x8011DCF8 + 40i` (two,
/// then eight in the order `+4 +8 +C +0` per four) and the two floats at
/// `0x80120408 + 8i` (0.0). The index is 32-bit (`multu` by 10, `sll`).
///
/// Leaves `f0 = 0.0`, `a3 = 10`, `v0` = the floats' address,
/// `v1 = 10i`, `a2`/`a1` the words' addresses (`a1` past them), `t0 =
/// 0x80120408`, `t6 = 40i`, `t7 = 0x8011DCF8`, `t8 = 0x8011DD00`, `t9 = 8i`.
///
/// Domain: `i` with the areas in RDRAM.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80080148(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[A3] = 0xA;
    let (lo, _) = multu(g[A0], g[A3]);
    g[T7] = li(0x8011_DCF8);
    g[T8] = li(0x8011_DD00);
    g[T0] = li(0x8012_0000);
    ctx.fpr[0].set_u32l(0);
    g[V0] = 2;
    g[T0] = addu(g[T0], 0x408);
    g[T9] = sll(g[A0], 3);
    g[V1] = lo;
    g[T6] = sll(g[V1], 2);
    g[A2] = addu(g[T6], g[T7]);
    sw(m, g[A2], 0, 0);
    sw(m, g[A2], 4, 0);
    g[A1] = addu(g[T6], g[T8]);
    loop {
        g[V0] = addu(g[V0], 4);
        sw(m, g[A1], 4, 0);
        sw(m, g[A1], 8, 0);
        sw(m, g[A1], 0xC, 0);
        g[A1] = addu(g[A1], 0x10);
        sw(m, g[A1], -0x10, 0);
        if g[V0] == g[A3] {
            break;
        }
    }
    g[V0] = addu(g[T9], g[T0]);
    sw(m, g[V0], 0, u64::from(ctx.fpr[0].u32l()));
    sw(m, g[V0], 4, u64::from(ctx.fpr[0].u32l()));
}

/// `func_80080350(a0)`: returns 0.0 in `f0` and spills `a0` to its home
/// slot (`sp + 0`).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80080350(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    ctx.fpr[0].set_u32l(0);
    sw(m, g[SP], 0, g[A0]);
}

/// `func_80080D08(out)`: if the word `[0x800A6704] >= 0` (signed), copies
/// the 16 words at `0x8011DCB8` to `out` (as bits, word by word, through
/// the FPRs `f4`, `f6`, `f8`, `f10`, `f16`, `f18` in turn) and returns `v0 =
/// 1`; otherwise returns 0.
///
/// Leaves `t6` = the flag word, `v1 = 0x8011DCB8`, and on a copy those FPRs
/// holding the last words through them.
///
/// Domain: `out` canonical, not overlapping the source.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80080D08(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[T6] = li(0x800A_0000);
    g[T6] = lw(m, g[T6], 0x6704);
    g[V1] = li(0x8011_DCB8);
    if (g[T6] as i64) < 0 {
        g[V0] = 0;
        return;
    }
    g[V0] = 1;
    for k in 0..16 {
        let r = [4, 6, 8, 10, 16, 18][k % 6];
        f[r].set_u32l(lw(m, g[V1], 4 * k as i32) as u32);
        sw(m, g[A0], 4 * k as i32, u64::from(f[r].u32l()));
    }
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

/// `func_800827C8()` (a deliberate crash, **guess**: it sits right after
/// `model_error` (`func_800827C0`, a stand-in double)): stores the word 1
/// unaligned at address 1 (`swl t6, 1($zero)`, `swr t6, 4($zero)`). On
/// hardware that is an address error. Under the oracle the address lies
/// outside RDRAM and the host faults; in the port `n64mem`'s bounds check
/// panics, which aborts (a panic in an `extern "C"` function).
///
/// Leaves `t6 = 1` before the first store. Domain: none; every call dies
/// (crates/difftest/tests/unaligned.rs runs both sides in child
/// processes).
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800827C8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = 1;
    swl(m, 0, 1, g[T6]);
    swr(m, 0, 4, g[T6]);
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

/// `func_80085F78(p)`: `[p + 0x154] = [p + 0x148] * (1 + [p + 0x150] *
/// (45 / [p + 0x134] - 1))` (floats, in that order).
///
/// Leaves `at = 0x42340000`, `f0 = 45 / [p + 0x134]`, `f2 = 1.0`, `f4`,
/// `f8` the loads, `f10`, `f16`, `f18` the steps, `f6` the result.
///
/// Domain: `p` canonical; no NaN operands.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80085F78(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    let f = &mut ctx.fpr;
    g[AT] = li(0x3F80_0000);
    f[2].set_u32l(g[AT] as u32);
    g[AT] = li(0x4234_0000);
    f[4].set_u32l(g[AT] as u32);
    f[6].set_u32l(lw(m, g[A0], 0x134) as u32);
    f[8].set_u32l(lw(m, g[A0], 0x150) as u32);
    f[0].set_fl(f[4].fl() / f[6].fl());
    f[4].set_u32l(lw(m, g[A0], 0x148) as u32);
    f[10].set_fl(f[0].fl() - f[2].fl());
    f[16].set_fl(f[8].fl() * f[10].fl());
    f[18].set_fl(f[2].fl() + f[16].fl());
    f[6].set_fl(f[4].fl() * f[18].fl());
    sw(m, g[A0], 0x154, u64::from(f[6].u32l()));
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
