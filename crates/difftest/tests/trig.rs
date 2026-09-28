//! libultra's sinf and cosf (game::libultra). Recompiled C vs Rust with the
//! constant tables loaded from the ROM image, each checked bit for bit
//! against an independent model of the algorithm (which reads the same
//! constants) and the model against `std`'s sin/cos, which confirms the
//! names.

// Tests are named after the functions (func_8008A8C0), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::libultra;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

/// A double as `ld` reads it: the high word first.
fn double(s: &State, a: u32) -> f64 {
    f64::from_bits(u64::from(word(s, a)) << 32 | u64::from(word(s, a + 4)))
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

/// The data segment around the tables, from the ROM image.
const DATA: (u32, u32) = (0x800A_DDC0, 0x800A_E120);

fn state(seed: u64, x: f32) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    let rom = baserom();
    let at = |v: u32| (v - 0x8000_0400 + 0x1000) as usize;
    s.rdram.mem().write_bytes(DATA.0, &rom[at(DATA.0)..at(DATA.1)]);
    s.ctx.gpr[SP] = sext(0x800A_2800);
    s.ctx.fpr[12].set_fl(x);
    s
}

struct Consts {
    p: [f64; 4],
    rpi: f64,
    pihi: f64,
    pilo: f64,
    zero: u32,
    qnan: u32,
}

fn consts(s: &State, table: u32, zero: u32) -> Consts {
    Consts {
        p: [8, 0x10, 0x18, 0x20].map(|o| double(s, table + o)),
        rpi: double(s, table + 0x28),
        pihi: double(s, table + 0x30),
        pilo: double(s, table + 0x38),
        zero: word(s, zero),
        qnan: word(s, 0x800A_E110),
    }
}

fn poly(c: &Consts, dx: f64) -> f64 {
    let xsq = dx * dx;
    let p = ((c.p[3] * xsq + c.p[2]) * xsq + c.p[1]) * xsq + c.p[0];
    dx + dx * xsq * p
}

/// Round half away from zero, as `n = dn +/- 0.5` truncated.
fn round(dn: f64) -> i32 {
    if dn >= 0.0 { (dn + 0.5) as i32 } else { (dn - 0.5) as i32 }
}

fn xpt(x: f32) -> i32 {
    (x.to_bits() as i32 >> 22) & 0x1FF
}

fn sinf(s: &State, x: f32) -> u32 {
    let c = consts(s, 0x800A_DE20, 0x800A_DE60);
    let e = xpt(x);
    if e < 255 {
        return if e < 230 { x.to_bits() } else { (poly(&c, f64::from(x)) as f32).to_bits() };
    }
    if e < 310 {
        let dx = f64::from(x);
        let n = round(dx * c.rpi);
        let dn = f64::from(n);
        let r = poly(&c, dx - dn * c.pihi - dn * c.pilo) as f32;
        return if n & 1 == 0 { r.to_bits() } else { (-r).to_bits() };
    }
    if x.is_nan() { c.qnan } else { c.zero }
}

fn cosf(s: &State, x: f32) -> u32 {
    let c = consts(s, 0x800A_DDD0, 0x800A_DE10);
    if xpt(x) < 310 {
        let dx = f64::from(if 0.0 < x { x } else { -x });
        let n = round(dx * c.rpi + 0.5);
        let dn = f64::from(n) - 0.5;
        let r = poly(&c, dx - dn * c.pihi - dn * c.pilo) as f32;
        return if n & 1 == 0 { r.to_bits() } else { (-r).to_bits() };
    }
    if x.is_nan() { c.qnan } else { c.zero }
}

/// Inputs across the branches: tiny, small, reduced (near multiples of
/// pi/2 too), the 2^28 edge, huge, zeros, subnormals, infinities, NaNs and
/// any bit pattern.
fn input() -> impl Strategy<Value = f32> {
    prop_oneof![
        -1.0e4f32..1.0e4,
        -8.0f32..8.0,
        (-20i32..20).prop_map(|k| k as f32 * std::f32::consts::FRAC_PI_2),
        (-20i32..20, -4i32..5).prop_map(|(k, d)| f32::from_bits((k as f32 * std::f32::consts::FRAC_PI_2).to_bits().wrapping_add(d as u32))),
        -1.0e-3f32..1.0e-3,
        Just(0.0f32),
        Just(-0.0f32),
        (1u32..0x80_0000).prop_map(f32::from_bits),
        (0x4D7F_FFF0u32..0x4D80_0010).prop_map(f32::from_bits),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        Just(f32::NAN),
        Just(f32::from_bits(0xFFC0_0001)),
        any::<u32>().prop_map(f32::from_bits),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn func_8008A8C0(seed: u64, x in input()) {
        let s = state(seed, x);
        let after = run("func_8008A8C0", libultra::func_8008A8C0, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), sinf(&s, x), "sinf({:e}) bits {:#x}", x, x.to_bits());
    }

    #[test]
    fn func_8008A750(seed: u64, x in input()) {
        let s = state(seed, x);
        let after = run("func_8008A750", libultra::func_8008A750, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), cosf(&s, x), "cosf({:e}) bits {:#x}", x, x.to_bits());
    }
}

/// The model is sin and cos: within a few units of single precision of
/// `std`'s double results over |x| < 1e4 (where reduction is still good).
#[test]
fn models_are_sin_and_cos() {
    let s = state(1, 0.0);
    let mut worst = (0.0f64, 0.0f64);
    for k in -20_000..20_000 {
        let x = k as f32 * 0.5001;
        let (sv, cv) = (f32::from_bits(sinf(&s, x)), f32::from_bits(cosf(&s, x)));
        worst.0 = worst.0.max((f64::from(sv) - f64::from(x).sin()).abs());
        worst.1 = worst.1.max((f64::from(cv) - f64::from(x).cos()).abs());
    }
    assert!(worst.0 < 2e-6 && worst.1 < 2e-6, "worst errors {worst:?}");
}
