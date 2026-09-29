//! Matrix leaves at 0x800160BC..0x80016DD8 (game::math): the inverse of a
//! scaled rotation plus translation, vec3/vec4 times a 4x4 matrix (with
//! and without translation), vec4 set and scale. Recompiled C vs Rust over
//! float edge values and ordinary full-mantissa ones, each checked against
//! its statement.
//!
//! NaN operands of arithmetic are outside these ports' domain (NAN_CHECK in
//! the oracle); models return None for inputs that would reach one, skipped
//! before the C runs.

// Tests are named after the functions (func_800160BC), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::math;
use game::recomp::{reg::*, RecompFn};
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

const SP_AT: u32 = 0x803F_0000;
const OUT: u32 = 0x8030_0000;
const V: u32 = 0x8030_0100;
const M: u32 = 0x8030_0200;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

fn float() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::from_bits(1)),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        Just(-1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        (-8i32..8).prop_map(|n| n as f32),
        -1.0e4f32..1.0e4f32,
        -1.0f32..1.0f32,
        Just(1.0e19f32),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![5 => -1.0e3f32..1.0e3f32, 3 => -1.0f32..1.0f32, 2 => (-8i32..8).prop_map(|n| n as f32), 1 => float()].boxed()
}

fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}

fn put(s: &mut State, a: u32, v: &[f32]) {
    let mut m = s.rdram.mem();
    for (k, x) in v.iter().enumerate() {
        m.write_u32(a + 4 * k as u32, x.to_bits());
    }
}

fn get(s: &State, a: u32, n: usize) -> Vec<u32> {
    (0..n as u32).map(|k| word(s, a + 4 * k)).collect()
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn mat() -> BoxedStrategy<[f32; 16]> {
    prop::array::uniform16(ordinary()).boxed()
}

fn v4() -> BoxedStrategy<[f32; 4]> {
    prop::array::uniform4(ordinary()).boxed()
}

/// `out` over the vector, or its own buffer.
fn out_at(alias: bool) -> u32 {
    if alias { V } else { OUT }
}

/// The inverse (statement in the port's doc), or None.
fn inverse(m: &[f32; 16]) -> Option<[f32; 16]> {
    let e = |i: usize, j: usize| m[4 * i + j];
    let sq = |i: usize| -> Option<f32> { ok(ok(e(i, 2) * e(i, 2))? + ok(ok(e(i, 0) * e(i, 0))? + ok(e(i, 1) * e(i, 1))?)?) };
    let s = [sq(0)?, sq(1)?, sq(2)?];
    let mut o = [0.0f32; 16];
    for i in 0..3 {
        for j in 0..3 {
            o[4 * j + i] = ok(e(i, j) / s[i])?;
        }
    }
    o[15] = 1.0;
    let t = [e(3, 0), e(3, 1), e(3, 2)];
    for j in 0..3 {
        let sum = ok(ok(t[0] * o[j])? + ok(t[1] * o[4 + j])?)?;
        o[12 + j] = -ok(ok(o[8 + j] * t[2])? + sum)?;
    }
    Some(o)
}

/// Row vector times matrix: `[x, y, z, w]` with `w` a weight (0 = none,
/// 1 = the translation row added, else `w * m[3][j]` first).
fn times(v: &[f32; 4], m: &[f32; 16], n: usize, form: u8) -> Option<Vec<f32>> {
    (0..n)
        .map(|j| {
            let e = |i: usize| m[4 * i + j];
            let s01 = ok(ok(e(0) * v[0])? + ok(e(1) * v[1])?)?;
            match form {
                // 80016BF4: v.z * m2j + (m0j * x + m1j * y)
                0 => Some(ok(v[2] * e(2))? + s01),
                // 80016CAC: m3j + ((m0j * x + m1j * y) + m2j * z)
                1 => Some(e(3) + ok(s01 + ok(e(2) * v[2])?)?),
                // 80016DD8: w * m3j + ((m0j * x + m1j * y) + m2j * z)
                _ => Some(ok(v[3] * e(3))? + ok(s01 + ok(e(2) * v[2])?)?),
            }
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Rotation-like matrices (rows of ordinary values) with a translation.
    #[test]
    fn func_800160BC(seed: u64, m in mat(), junk: [u32; 4]) {
        let want = inverse(&m);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, M, &m);
        put(&mut s, OUT + 0x40, &junk.map(f32::from_bits));
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(OUT), sext(M));
        let after = run("func_800160BC", math::func_800160BC, &s)?;
        prop_assert_eq!(get(&after, OUT, 16), bits(&want.unwrap()));
        prop_assert_eq!(get(&after, OUT + 0x40, 4), junk.to_vec());
    }

    #[test]
    fn func_80016BF4(seed: u64, v in v4(), m in mat(), alias: bool) {
        let mut s = state(seed);
        put(&mut s, V, &v);
        put(&mut s, M, &m);
        let out = out_at(alias);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(out), sext(V), sext(M));
        if let Some(want) = times(&v, &m, 3, 0) {
            let after = run("func_80016BF4", math::func_80016BF4, &s)?;
            prop_assert_eq!(get(&after, out, 3), bits(&want));
        }
        if let Some(want) = times(&v, &m, 3, 1) {
            let after = run("func_80016CAC", math::func_80016CAC, &s)?;
            prop_assert_eq!(get(&after, out, 3), bits(&want));
        }
        if let Some(want) = times(&v, &m, 4, 2) {
            let after = run("func_80016DD8", math::func_80016DD8, &s)?;
            prop_assert_eq!(get(&after, out, 4), bits(&want));
        }
    }

    /// Set moves bits (NaNs included); scale multiplies.
    #[test]
    fn func_80016D78(seed: u64, xyzw: [u32; 4], v in v4(), k in ordinary(), alias: bool) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(SP_AT + 0x10, xyzw[3]);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(OUT), sext(xyzw[0]), sext(xyzw[1]), sext(xyzw[2]));
        let after = run("func_80016D78", math::func_80016D78, &s)?;
        prop_assert_eq!(get(&after, OUT, 4), xyzw.to_vec());
        put(&mut s, V, &v);
        let out = out_at(alias);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(out), sext(k.to_bits()), sext(V));
        let after = run("func_80016DA0", math::func_80016DA0, &s)?;
        prop_assert_eq!(get(&after, out, 4), bits(&v.map(|x| x * k)));
    }
}

/// `a * b` as both products sum it: `a[i][3] * b[3][j] + ((b[0][j] *
/// a[i][0] + b[1][j] * a[i][1]) + b[2][j] * a[i][2])`, or None.
fn product(a: &[f32; 16], b: &[f32; 16]) -> Option<Vec<f32>> {
    (0..16)
        .map(|k| {
            let (i, j) = (k / 4, k % 4);
            let (ai, bj) = (|l: usize| a[4 * i + l], |l: usize| b[4 * l + j]);
            let s = ok(ok(ok(bj(0) * ai(0))? + ok(bj(1) * ai(1))?)? + ok(bj(2) * ai(2))?)?;
            Some(ok(ai(3) * bj(3))? + s)
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// `out = a * b` from copies, so `out` may be either input.
    #[test]
    fn func_80015724(seed: u64, a in mat(), b in mat(), alias in 0u8..3) {
        let want = product(&a, &b);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, V, &a);
        put(&mut s, M, &b);
        let out = [OUT, V, M][alias as usize];
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(out), sext(V), sext(M));
        let after = run("func_80015724", math::func_80015724, &s)?;
        prop_assert_eq!(get(&after, out, 16), bits(&want.unwrap()));
    }

    /// `m = n * m` in place (`n` separate).
    #[test]
    fn func_80015C30(seed: u64, m0 in mat(), n in mat()) {
        let want = product(&n, &m0);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, M, &m0);
        put(&mut s, V, &n);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(M), sext(V));
        let after = run("func_80015C30", math::func_80015C30, &s)?;
        prop_assert_eq!(get(&after, M, 16), bits(&want.unwrap()));
    }
}
