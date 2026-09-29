//! The game's arcsine and atan2 in degrees (func_80014D4C, func_80014F54;
//! game::math). Recompiled C vs Rust with their constants loaded from the
//! ROM image, each checked bit for bit against a model that follows the
//! code's operation order, plus a check that the models are close to
//! `std`'s asin/atan2 (the names).
//!
//! NaN inputs are outside the domain (the series' arithmetic is guarded);
//! so is atan2 with both arguments infinite.

// Tests are named after the functions (func_80014D4C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::math;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;
/// The constants of both functions.
const DATA: (u32, u32) = (0x800A_8790, 0x800A_87F0);

fn at(v: u32) -> usize {
    (v - 0x8000_0400 + 0x1000) as usize
}

fn f(a: u32) -> f32 {
    f32::from_be_bytes(baserom()[at(a)..at(a) + 4].try_into().unwrap())
}

fn d(a: u32) -> f64 {
    f64::from_be_bytes(baserom()[at(a)..at(a) + 8].try_into().unwrap())
}

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s.rdram.mem().write_bytes(DATA.0, &baserom()[at(DATA.0)..at(DATA.1)]);
    s
}

fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}

/// `f32(f64(r * 180) / D)`, as both functions convert.
fn degrees(r: f32, dd: f64) -> f32 {
    (f64::from(r * 180.0) / dd) as f32
}

fn asin_model(x: f32) -> Option<f32> {
    let c: [f32; 6] = std::array::from_fn(|k| f(0x800A_8790 + 4 * k as u32));
    let p: [f32; 4] = std::array::from_fn(|k| f(0x800A_87A8 + 4 * k as u32));
    ok(x)?;
    if c[0] < x {
        return Some(90.0);
    }
    if x < c[1] {
        return Some(-90.0);
    }
    let reduced = !(x < c[2] && c[3] < x);
    let y = if reduced {
        let s = (1.0 - x * x).sqrt();
        if x < 0.0 { -s } else { s }
    } else {
        x
    };
    let r = if y < c[4] && c[5] < y {
        y
    } else {
        let y2 = y * y;
        let (y3, y5) = (y * y2, y * y2 * y2);
        let y7 = y5 * y2;
        (((y3 * p[0] + y) + y5 * p[1]) + y7 * p[2]) + (y7 * y2) * p[3]
    };
    let dg = degrees(r, d(0x800A_87B8));
    Some(if !reduced { dg } else if x < 0.0 { -90.0 - dg } else { 90.0 - dg })
}

fn atan2_model(y: f32, x: f32) -> Option<f32> {
    let k: [f32; 4] = std::array::from_fn(|i| f(0x800A_87C0 + 4 * i as u32));
    let q: [f32; 4] = std::array::from_fn(|i| f(0x800A_87D0 + 4 * i as u32));
    let (k8, k9) = (f(0x800A_87E8), f(0x800A_87EC));
    ok(x)?;
    ok(y)?;
    let mut a;
    if k[1] <= x && x < k[0] {
        a = 90.0;
    } else if k[2] <= y && y < k[0] {
        a = 0.0;
    } else {
        let (ay, ax) = (if y < 0.0 { -y } else { y }, if x < 0.0 { -x } else { x });
        let swap = ax < ay;
        let t = if swap { ax / ay } else { ay / ax };
        if t < k[0] && k[3] <= t {
            a = 0.0;
        } else {
            let t = ok(t)?;
            let t2 = t * t;
            let t3 = t * t2;
            let t5 = t3 * t2;
            let t7 = t5 * t2;
            let p = (((t - t3 * q[0]) + t5 * q[1]) - t7 * q[2]) + (t7 * t2) * q[3];
            a = degrees(p, d(0x800A_87E0));
        }
        if swap {
            a = 90.0 - a;
        }
    }
    if x < k8 {
        a = 180.0 - a;
    }
    if y < k9 {
        a = -a;
    }
    Some(a)
}

/// `x` and its neighbours one ulp away.
fn around(x: f32) -> impl Strategy<Value = f32> {
    (-2i32..=2).prop_map(move |k| f32::from_bits((x.to_bits() as i32 + k) as u32))
}

fn asin_input() -> impl Strategy<Value = f32> {
    let edges = [0x800A_8790u32, 0x800A_8794, 0x800A_8798, 0x800A_879C, 0x800A_87A0, 0x800A_87A4].map(f);
    prop_oneof![
        4 => -1.0f32..1.0,
        2 => -1.2f32..1.2,
        1 => -0.01f32..0.01,
        2 => prop::sample::select(edges.to_vec()).prop_flat_map(around),
        1 => prop_oneof![Just(0.0f32), Just(-0.0), Just(1.0), Just(-1.0), Just(f32::INFINITY), Just(f32::NEG_INFINITY), Just(f32::from_bits(1))],
        1 => any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
}

fn atan_input() -> impl Strategy<Value = f32> {
    prop_oneof![
        4 => -1.0e4f32..1.0e4,
        2 => -1.0f32..1.0,
        2 => prop_oneof![Just(1.0e-4f32), Just(-1.0e-4f32)].prop_flat_map(around),
        1 => -1.0e-3f32..1.0e-3,
        1 => prop_oneof![Just(0.0f32), Just(-0.0), Just(f32::INFINITY), Just(f32::NEG_INFINITY), Just(f32::MAX)],
        1 => any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn func_80014D4C(seed: u64, x in asin_input()) {
        let want = asin_model(x).unwrap();
        let mut s = state(seed);
        s.ctx.fpr[12].set_fl(x);
        let after = run("func_80014D4C", math::func_80014D4C, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.to_bits(), "x = {:e}", x);
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }

    /// Pairs: free, on the axes' bands, with |y| = |x| (the swap test), and
    /// with a ratio at the series' small-t threshold.
    #[test]
    fn func_80014F54(seed: u64, y in atan_input(), x in atan_input(), mode in 0u8..4, k in -2i32..=2) {
        let (y, x) = match mode {
            1 => (if k < 0 { -x } else { x }, x),
            2 => (f32::from_bits((1.0e-4f32 * x.abs()).to_bits().wrapping_add(k as u32)), x),
            _ => (y, x),
        };
        let want = atan2_model(y, x);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        s.ctx.fpr[12].set_fl(y);
        s.ctx.fpr[14].set_fl(x);
        let after = run("func_80014F54", math::func_80014F54, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.unwrap().to_bits(), "y = {:e}, x = {:e}", y, x);
    }
}

/// The names: the models stay close to asin and atan2 in degrees. The
/// series are truncated: over this grid the worst errors are 0.0037
/// degrees for asin and 0.094 for atan2 (its last term's coefficient is
/// tuned, not 1/9).
#[test]
fn models_are_asin_and_atan2() {
    let mut worst = (0.0f64, 0.0f64);
    for i in -1000..=1000 {
        let x = i as f32 / 1000.0;
        let got = f64::from(asin_model(x).unwrap());
        worst.0 = worst.0.max((got - f64::from(x).asin().to_degrees()).abs());
        for j in -20..=20 {
            let (y, x) = (i as f32 / 10.0, j as f32 / 2.0 + 0.25);
            let got = f64::from(atan2_model(y, x).unwrap());
            worst.1 = worst.1.max((got - f64::from(y).atan2(f64::from(x)).to_degrees()).abs());
        }
    }
    assert!(worst.0 < 0.005 && worst.1 < 0.1, "worst errors {worst:?}");
}

/// Inputs where fusing one of the series' smallest terms (asin's `y^7*P3`
/// and `y^9*P4`, atan2's `t^9*Q4`) changes the result: the product is tiny
/// next to the sum, so only an exact sum within a fraction of an ulp of a
/// rounding midpoint shows it (found by a search over the series with the
/// ROM's constants, a few per million inputs). atan2 takes `(t, 1)`, so `t`
/// is exact.
#[test]
fn fused_term_separators() {
    let asin = [0x3E35_8DFEu32, 0x3E73_B019, 0x3E9E_040F, 0x3ECB_0089, 0x3EEB_2AED, 0x3F04_94BF];
    for (k, bits) in asin.into_iter().enumerate() {
        let x = f32::from_bits(bits);
        let mut s = state(k as u64);
        s.ctx.fpr[12].set_fl(x);
        let after = compare("func_80014D4C", math::func_80014D4C, &s).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(after.ctx.fpr[0].u32l(), asin_model(x).unwrap().to_bits(), "x = {bits:#010X}");
    }
    let atan = [0x3EE2_74ECu32, 0x3EF1_04FB, 0x3F09_0E0A, 0x3F0F_30FC];
    for (k, bits) in atan.into_iter().enumerate() {
        let y = f32::from_bits(bits);
        let mut s = state(k as u64);
        s.ctx.fpr[12].set_fl(y);
        s.ctx.fpr[14].set_fl(1.0);
        let after = compare("func_80014F54", math::func_80014F54, &s).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(after.ctx.fpr[0].u32l(), atan2_model(y, 1.0).unwrap().to_bits(), "y = {bits:#010X}");
    }
}
