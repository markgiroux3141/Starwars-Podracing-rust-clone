//! The 2-D vector leaves at 0x8001514C..0x8001528C (game::math): add, scale,
//! length and squared distance, plus func_8000787C (game::misc), a
//! float-to-int volume store. Recompiled C vs Rust over float edge values,
//! each checked against its statement.
//!
//! NaN operands are outside these ports' domain: N64Recomp's NAN_CHECK
//! asserts on them in the oracle (and FCR31.EV would trap on hardware). NaN
//! payloads are tested where the C doesn't look at them.

// Tests are named after the functions (func_8001514C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::recomp::{reg::*, RecompFn};
use game::{math, misc};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s
}

/// Non-NaN floats: signed zeros, subnormals, ones, halves, ordinary values,
/// huge, infinities, and any other non-NaN bit pattern.
fn float() -> impl Strategy<Value = f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::from_bits(1)),
        Just(f32::from_bits(0x007F_FFFF)),
        Just(-f32::from_bits(1)),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        Just(-1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        // Ordinary values with full mantissas, the game's range: inexact
        // products of similar size, where rounding each step matters.
        -1.0e4f32..1.0e4f32,
        -1.0e4f32..1.0e4f32,
        Just(1.0e19f32),
        Just(-3.0e38f32),
        Just(f32::MAX),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
}

fn put(s: &mut State, a: u32, v: [f32; 2]) {
    let mut m = s.rdram.mem();
    m.write_u32(a, v[0].to_bits());
    m.write_u32(a + 4, v[1].to_bits());
}

fn get(s: &State, a: u32) -> [u32; 2] {
    [word(s, a), word(s, a + 4)]
}

const OUT: u32 = 0x8030_0000;
const A: u32 = 0x8030_0100;
const B: u32 = 0x8030_0200;

/// Where `out` goes: its own buffer, or on top of an input.
fn out_at(alias: u8) -> u32 {
    [OUT, A, B][alias as usize % 3]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// `out = b + a`, component by component (so aliasing works).
    #[test]
    fn func_8001514C(seed: u64, a in [float(), float()], b in [float(), float()], alias in 0u8..3) {
        let mut s = state(seed);
        put(&mut s, A, a);
        put(&mut s, B, b);
        let out = out_at(alias);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(out), sext(A), sext(B));
        let after = run("func_8001514C", math::func_8001514C, &s)?;
        let want = [(b[0] + a[0]).to_bits(), (b[1] + a[1]).to_bits()];
        prop_assert_eq!(get(&after, out), want);
    }

    /// `out = v * s`, `s` in `a1`.
    #[test]
    fn func_80015170(seed: u64, v in [float(), float()], k in float(), junk: u32, alias: bool) {
        let mut s = state(seed);
        put(&mut s, A, v);
        let out = if alias { A } else { OUT };
        s.ctx.gpr[A0] = sext(out);
        s.ctx.gpr[A1] = (u64::from(junk) << 32) | u64::from(k.to_bits());
        s.ctx.gpr[A2] = sext(A);
        let after = run("func_80015170", math::func_80015170, &s)?;
        prop_assert_eq!(get(&after, out), [(v[0] * k).to_bits(), (v[1] * k).to_bits()]);
    }

    /// `sqrt(y*y + x*x)` in `f0`. Squares are never negative or NaN for
    /// non-NaN inputs, so the square root never sees a NaN operand.
    #[test]
    fn func_800151C0(seed: u64, v in [float(), float()]) {
        let mut s = state(seed);
        put(&mut s, A, v);
        s.ctx.gpr[A0] = sext(A);
        let after = run("func_800151C0", math::func_800151C0, &s)?;
        let want = (v[1] * v[1] + v[0] * v[0]).sqrt();
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.to_bits());
    }

    /// `(a.x - b.x)^2 + (a.y - b.y)^2` in `f0`. Opposite infinities (a NaN
    /// difference, then a NaN operand of the multiply) are outside the domain.
    #[test]
    fn func_8001523C(seed: u64, a in [float(), float()], b in [float(), float()]) {
        let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
        // inf - inf is a NaN operand for the multiply: outside the domain.
        prop_assume!(!dx.is_nan() && !dy.is_nan());
        let mut s = state(seed);
        put(&mut s, A, a);
        put(&mut s, B, b);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(A), sext(B));
        let after = run("func_8001523C", math::func_8001523C, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), (dx * dx + dy * dy).to_bits());
    }

    /// With the flag set, `[0x8009A328] = trunc(x * 32000)`: halfway and
    /// boundary products, out-of-range products give 0x80000000.
    #[test]
    fn func_8000787C(seed: u64, x in prop_oneof![
        float(),
        (-64i32..64).prop_map(|n| (n as f32 + 0.5) / 32000.0),
        Just(67_108.86f32), Just(67_108.87f32), Just(-67_108.87f32), Just(-67_108.86f32),
    ]) {
        let mut s = state(seed);
        s.ctx.fpr[12].set_fl(x);
        s.rdram.mem().write_u32(0x8009_A2B8, 1);
        let after = run("func_8000787C", misc::func_8000787C, &s)?;
        let p = x * 32000.0;
        let want = if p.is_nan() || p >= 2_147_483_648.0 || p < -2_147_483_648.0 { 0x8000_0000 } else { p as i32 as u32 };
        prop_assert_eq!(word(&after, 0x8009_A328), want);
    }

    /// With the flag clear nothing happens, and `f12` (any bits, NaN
    /// payloads included: nothing checks it on this path) is untouched.
    #[test]
    fn func_8000787C_off(seed: u64, bits in prop_oneof![
        (1u32..0x0080_0000).prop_map(|p| 0x7F80_0000 | p),
        (1u32..0x0080_0000).prop_map(|p| 0xFF80_0000 | p),
        any::<u32>(),
    ]) {
        let mut s = state(seed);
        s.ctx.fpr[12].set_u32l(bits);
        s.rdram.mem().write_u32(0x8009_A2B8, 0);
        let after = run("func_8000787C", misc::func_8000787C, &s)?;
        prop_assert_eq!(after.ctx.fpr[12].u32l(), bits);
        prop_assert_eq!(word(&after, 0x8009_A328), word(&s, 0x8009_A328));
    }
}

/// Square roots of negative sums can't occur (squares are non-negative),
/// but a negative zero can: sqrt(-0) is -0 in IEEE, and the sum of two
/// squares of zeros is +0 either way.
#[test]
fn length_of_zero_vectors() {
    for v in [[0.0f32, 0.0], [-0.0, -0.0], [0.0, -0.0]] {
        let mut s = state(9);
        put(&mut s, A, v);
        s.ctx.gpr[A0] = sext(A);
        let after = compare("func_800151C0", math::func_800151C0, &s).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(after.ctx.fpr[0].u32l(), 0);
    }
}
