//! The LU pair under the 3x3 inverse `func_80016A20` (game::math):
//! `func_80016260` (Crout LU with implicit pivoting) and `func_800167E4`
//! (back-substitution). Recompiled C vs Rust, each checked against its
//! statement by a model that works on a copy of RDRAM, so swaps through a
//! stale pivot index outside the matrix are predicted too.
//!
//! NaN operands of arithmetic are outside the domain (NAN_CHECK in the
//! oracle); the models check each operand and return None, skipped before
//! the C runs. NaN results are in the domain, and so are NaNs that are only
//! compared or moved.

// Tests are named after the functions (func_80016260), capitals included.
#![allow(non_snake_case)]

use std::collections::BTreeMap;

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
/// Rows -1 and 3 around it are test data too (a stale pivot index).
const LU: u32 = 0x8030_0010;
const A: u32 = 0x8030_0100;
const INDX: u32 = 0x8030_0200;
const B: u32 = 0x8030_0310;

/// Stale frame words: `vv` and `imax` of `func_80016260`.
const VV: u32 = SP_AT - 0x28;
const IMAX: u32 = SP_AT - 0x10;

/// RDRAM as the model sees it: the input state plus the model's writes.
struct Mem<'a> {
    s: &'a State,
    w: BTreeMap<u32, u32>,
}

impl Mem<'_> {
    fn new(s: &State) -> Mem<'_> {
        Mem { s, w: BTreeMap::new() }
    }
    fn r(&self, a: u32) -> u32 {
        self.w.get(&a).copied().unwrap_or_else(|| word(self.s, a))
    }
    fn f(&self, a: u32) -> f32 {
        f32::from_bits(self.r(a))
    }
    fn set(&mut self, a: u32, v: u32) {
        self.w.insert(a, v);
    }
    fn setf(&mut self, a: u32, x: f32) {
        self.set(a, x.to_bits());
    }
    /// Every word the model wrote holds its value in `after`.
    fn check(&self, after: &State) -> Result<(), TestCaseError> {
        for (&a, &v) in &self.w {
            prop_assert_eq!(word(after, a), v, "word {:#010x}", a);
        }
        Ok(())
    }
}

fn operands(a: f32, b: f32) -> Option<(f32, f32)> {
    (!a.is_nan() && !b.is_nan()).then_some((a, b))
}
fn mul(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a * b)
}
fn sub(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a - b)
}
fn div(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a / b)
}
/// `neg.s` only below zero, so never of a NaN.
fn abs(x: f32) -> f32 {
    if x < 0.0 { -x } else { x }
}

fn at(lu: u32, i: i32, j: i32) -> u32 {
    lu.wrapping_add((16 * i + 4 * j) as u32)
}

/// `func_80016260`'s statement: the return value, or None (a NaN operand).
fn ludcmp(mem: &mut Mem, lu: u32, a: u32, indx: u32) -> Option<u32> {
    let vv = |i: i32| VV.wrapping_add((4 * i) as u32);
    for i in 0..3 {
        let mut big = 0.0f32;
        for j in 0..3 {
            let x = mem.r(at(a, i, j));
            mem.set(at(lu, i, j), x);
            let t = abs(mem.f(at(a, i, j)));
            if big < t {
                big = t;
                mem.setf(vv(i), div(1.0, t)?);
            }
        }
    }
    let mut imax = mem.r(IMAX) as i32;
    for j in 0..3 {
        for i in 0..j {
            let mut sum = mem.f(at(lu, i, j));
            for k in 0..i {
                sum = sub(sum, mul(mem.f(at(lu, i, k)), mem.f(at(lu, k, j)))?)?;
            }
            mem.setf(at(lu, i, j), sum);
        }
        let mut big = 0.0f32;
        for i in j..3 {
            let mut sum = mem.f(at(lu, i, j));
            for k in 0..j {
                sum = sub(sum, mul(mem.f(at(lu, i, k)), mem.f(at(lu, k, j)))?)?;
            }
            mem.setf(at(lu, i, j), sum);
            let dum = mul(abs(sum), mem.f(vv(i)))?;
            if big <= dum {
                big = dum;
                imax = i;
            }
        }
        if imax != j {
            for k in 0..3 {
                let (x, y) = (mem.r(at(lu, j, k)), mem.r(at(lu, imax, k)));
                mem.set(at(lu, imax, k), x);
                mem.set(at(lu, j, k), y);
            }
            let v = mem.r(vv(j));
            mem.set(vv(imax), v);
        }
        mem.set(indx + 4 * j as u32, imax as u32);
        let p = mem.f(at(lu, j, j));
        if p == 0.0 {
            return Some(0);
        }
        if j != 2 {
            let r = div(1.0, p)?;
            for i in j + 1..3 {
                let x = mul(mem.f(at(lu, i, j)), r)?;
                mem.setf(at(lu, i, j), x);
            }
        }
    }
    mem.set(IMAX, imax as u32);
    Some(1)
}

/// `func_800167E4`'s statement, or None (a NaN operand).
fn lubksb(mem: &mut Mem, lu: u32, indx: u32, b: u32) -> Option<()> {
    let bb = |i: i32| b.wrapping_add((4 * i) as u32);
    let mut ii = -1i32;
    for i in 0..3 {
        let ip = mem.r(indx + 4 * i as u32) as i32;
        let bi = mem.r(bb(i));
        let mut sum = mem.f(bb(ip));
        mem.set(bb(ip), bi);
        if ii >= 0 {
            for j in ii..i {
                sum = sub(sum, mul(mem.f(at(lu, i, j)), mem.f(bb(j)))?)?;
            }
        } else if sum != 0.0 {
            ii = i;
        }
        mem.setf(bb(i), sum);
    }
    for i in (0..3).rev() {
        let mut sum = mem.f(bb(i));
        for j in i + 1..3 {
            sum = sub(sum, mul(mem.f(at(lu, i, j)), mem.f(bb(j)))?)?;
        }
        let q = div(sum, mem.f(at(lu, i, i)))?;
        mem.setf(bb(i), q);
    }
    Some(())
}

fn edge() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::from_bits(1)),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        Just(-1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        Just(1.0e19f32),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

fn signed(bits: u32, neg: bool) -> f32 {
    f32::from_bits(bits | (neg as u32) << 31)
}

fn elem() -> BoxedStrategy<f32> {
    prop_oneof![
        6 => -1.0e3f32..1.0e3f32,
        3 => -1.0f32..1.0f32,
        3 => (-3i32..=3).prop_map(|n| n as f32),
        1 => Just(-0.0f32),
        1 => (1u32..0x0080_0000, any::<bool>()).prop_map(|(b, s)| signed(b, s)),
        1 => edge(),
    ]
    .boxed()
}

/// Subnormals below 2^-128, whose reciprocal is infinite, or zero.
fn tiny() -> BoxedStrategy<f32> {
    prop_oneof![3 => (1u32..0x0020_0000, any::<bool>()).prop_map(|(b, s)| signed(b, s)), 1 => Just(0.0f32)].boxed()
}

/// Row-major 3x3: general, a zero row, a repeated row (singular), small
/// integers (exact cancellation, zero pivots), and a zero first column
/// with tiny rows (every `dum` of column 0 is `0 * inf`, so the stale
/// `imax` stays).
fn matrix() -> BoxedStrategy<[f32; 9]> {
    let m = || prop::array::uniform9(elem());
    prop_oneof![
        5 => m(),
        1 => (m(), 0usize..3).prop_map(|(mut m, r)| {
            m[3 * r..3 * r + 3].fill(0.0);
            m
        }),
        1 => (m(), 0usize..3, 0usize..3).prop_map(|(mut m, r, s)| {
            for k in 0..3 {
                m[3 * s + k] = m[3 * r + k];
            }
            m
        }),
        1 => prop::array::uniform9((-2i32..=2).prop_map(|n| n as f32)),
        1 => prop::array::uniform9(tiny()).prop_map(|mut m| {
            m[0] = 0.0;
            m[3] = 0.0;
            m[6] = 0.0;
            m
        }),
    ]
    .boxed()
}

/// Rows of 4 floats from `base`, the fourth left as it is.
fn put_rows(s: &mut State, base: u32, m: &[f32; 9]) {
    let mut mem = s.rdram.mem();
    for i in 0..3 {
        for j in 0..3 {
            mem.write_u32(at(base, i as i32, j as i32), m[3 * i + j].to_bits());
        }
    }
}

fn put(s: &mut State, a: u32, v: &[f32]) {
    let mut m = s.rdram.mem();
    for (k, x) in v.iter().enumerate() {
        m.write_u32(a + 4 * k as u32, x.to_bits());
    }
}

/// The state for `func_80016260`: matrix, stale frame words, rows -1 and 3
/// around `lu`.
fn ludcmp_state(seed: u64, m: &[f32; 9], alias: bool, stale: &[f32; 3], imax: i32, around: &[f32; 8]) -> (State, u32) {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    put_rows(&mut s, A, m);
    put(&mut s, VV, stale);
    s.rdram.mem().write_u32(IMAX, imax as u32);
    put(&mut s, LU - 0x10, &around[..4]);
    put(&mut s, LU + 0x30, &around[4..]);
    let lu = if alias { A } else { LU };
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(lu), sext(A), sext(INDX));
    (s, lu)
}

fn stale_imax() -> BoxedStrategy<i32> {
    prop_oneof![3 => 0i32..3, 1 => -1i32..=3].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn func_80016260(seed: u64, m in matrix(), alias in prop::bool::weighted(0.2),
                     stale in prop::array::uniform3(elem()), imax in stale_imax(),
                     around in prop::array::uniform8(elem())) {
        let (s, lu) = ludcmp_state(seed, &m, alias, &stale, imax, &around);
        let mut mem = Mem::new(&s);
        let want = ludcmp(&mut mem, lu, A, INDX);
        prop_assume!(want.is_some());
        let after = run("func_80016260", math::func_80016260, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(want.unwrap()));
        mem.check(&after)?;
        // The fourth float of each row is not touched.
        for i in 0..3 {
            prop_assert_eq!(word(&after, at(lu, i, 3)), word(&s, at(lu, i, 3)));
        }
    }

    /// Back-substitution of an arbitrary `lu` and pivot list.
    #[test]
    fn func_800167E4(seed: u64, m in matrix(), b in prop::array::uniform3(elem()),
                     indx in (0i32..3, 1i32..3, 2i32..3), wild in prop::option::weighted(0.15, (0usize..3, -1i32..=3)),
                     around in prop::array::uniform2(elem())) {
        let mut ix = [indx.0, indx.1, indx.2];
        if let Some((k, v)) = wild {
            ix[k] = v;
        }
        let mut s = State::new();
        s.randomise_registers(seed);
        s.ctx.gpr[SP] = sext(SP_AT);
        put_rows(&mut s, LU, &m);
        put(&mut s, B, &b);
        put(&mut s, B - 4, &around[..1]);
        put(&mut s, B + 12, &around[1..]);
        for (k, v) in ix.iter().enumerate() {
            s.rdram.mem().write_u32(INDX + 4 * k as u32, *v as u32);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(LU), sext(INDX), sext(B));
        let mut mem = Mem::new(&s);
        prop_assume!(lubksb(&mut mem, LU, INDX, B).is_some());
        let after = run("func_800167E4", math::func_800167E4, &s)?;
        mem.check(&after)?;
    }

    /// The pair as `func_80016A20` uses it: decompose, then solve for a
    /// unit vector (or any `b`) with the C's own decomposition.
    #[test]
    fn decompose_then_solve(seed: u64, m in matrix(), stale in prop::array::uniform3(elem()),
                            b in prop_oneof![(0usize..3).prop_map(|k| {
                                let mut e = [0.0f32; 3];
                                e[k] = 1.0;
                                e
                            }), prop::array::uniform3(elem())]) {
        let (s, _) = ludcmp_state(seed, &m, false, &stale, 0, &[0.0; 8]);
        let mut mem = Mem::new(&s);
        prop_assume!(ludcmp(&mut mem, LU, A, INDX) == Some(1));
        let mut s = run("func_80016260", math::func_80016260, &s)?;
        put(&mut s, B, &b);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(LU), sext(INDX), sext(B));
        let mut mem = Mem::new(&s);
        prop_assume!(lubksb(&mut mem, LU, INDX, B).is_some());
        let after = run("func_800167E4", math::func_800167E4, &s)?;
        mem.check(&after)?;
    }
}
