//! game::recomp::RecompContext must match recomp.h's recomp_context exactly:
//! same size, alignment and field offsets as the C compiler sees them, and
//! the same meaning for the fpr union halves.

use game::recomp::{Fpr, RecompContext};
use std::mem::{align_of, offset_of, size_of};

#[test]
fn size_and_offsets_match_c() {
    let c = oracle::ctx::layout();
    let f = |n: usize| offset_of!(RecompContext, fpr) + n * size_of::<Fpr>();
    let g = |n: usize| offset_of!(RecompContext, gpr) + n * 8;
    let rust = [
        size_of::<RecompContext>(),
        align_of::<RecompContext>(),
        g(0),
        g(1),
        g(31),
        f(0),
        f(1),
        f(31),
        offset_of!(RecompContext, hi),
        offset_of!(RecompContext, lo),
        offset_of!(RecompContext, f_odd),
        offset_of!(RecompContext, status_reg),
        offset_of!(RecompContext, mips3_float_mode),
        size_of::<Fpr>(),
        align_of::<Fpr>(),
        0, // fl
        4, // fh
        0, // u32l
        4, // u32h
    ];
    let names = [
        "sizeof(recomp_context)", "alignof(recomp_context)", "r0", "r1", "r31", "f0", "f1", "f31", "hi", "lo",
        "f_odd", "status_reg", "mips3_float_mode", "sizeof(fpr)", "alignof(fpr)", "fpr.fl", "fpr.fh", "fpr.u32l",
        "fpr.u32h",
    ];
    assert_eq!(c.len(), rust.len());
    for ((name, c), r) in names.iter().zip(&c).zip(&rust) {
        assert_eq!(c, r, "{name}: C says {c}, Rust says {r}");
    }
}

#[test]
fn c_field_writes_land_in_rust_fields() {
    let mut ctx = RecompContext::default();
    unsafe { oracle::ctx::ctx_poke(&mut ctx) };
    assert_eq!(ctx.gpr[1], 0x1111_1111_1111_1111);
    assert_eq!(ctx.gpr[31], 0x3131_3131_3131_3131);
    assert_eq!(ctx.fpr[0].fl(), 1.5);
    assert_eq!(ctx.fpr[0].fh(), -2.0);
    assert_eq!(ctx.fpr[1].u32l(), 0xAABB_CCDD);
    assert_eq!(ctx.fpr[1].u32h(), 0x1122_3344);
    assert_eq!(ctx.fpr[31].d(), 0.25);
    assert_eq!(ctx.hi, 0x4849);
    assert_eq!(ctx.lo, 0x4C4F);
    assert_eq!(ctx.status_reg, 0x3400_FF01);
    assert_eq!(ctx.mips3_float_mode, 1);
    // Nothing else was touched.
    assert!(ctx.gpr.iter().enumerate().all(|(i, &r)| r == 0 || i == 1 || i == 31));
}

#[test]
fn odd_fprs_alias_even_high_halves_in_fr0_mode() {
    let mut ctx = RecompContext::default();
    ctx.fix_f_odd();
    unsafe {
        oracle::ctx::ctx_write_f_odd(&mut ctx, 1, 0xCAFE_F00D);
        oracle::ctx::ctx_write_f_odd(&mut ctx, 31, 0x1234_5678);
    }
    assert_eq!(ctx.fpr[0].u32h(), 0xCAFE_F00D);
    assert_eq!(ctx.fpr[30].u32h(), 0x1234_5678);
    assert_eq!(ctx.fpr[1].u64, 0);
    assert_eq!(ctx.fpr[31].u64, 0);
}
