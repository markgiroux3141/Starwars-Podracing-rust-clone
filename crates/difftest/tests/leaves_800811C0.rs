//! Maths leaves at 0x800811C0..0x8008241C (game::math): a double store, a
//! signed power, a ratio, point times matrix, projection onto a plane, the
//! closest point on a segment, plane values and a 2D cross product.
//! Recompiled C vs Rust, each checked against its statement; the ones that
//! write through a pointer by a model over a copy of RDRAM (so an output
//! that aliases an input, which the C re-reads, is predicted too).
//!
//! NaN operands of arithmetic are outside the domain (NAN_CHECK in the
//! oracle); models return None for them, skipped before the C runs.

// Tests are named after the functions (func_800811C0), capitals included.
#![allow(non_snake_case)]

use std::collections::BTreeMap;

use difftest::{compare, rom::baserom, State};
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
const A: u32 = 0x8030_0000;
const B: u32 = 0x8030_0100;
const C: u32 = 0x8030_0200;
const OUT: u32 = 0x8030_0300;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

fn operands(a: f32, b: f32) -> Option<(f32, f32)> {
    (!a.is_nan() && !b.is_nan()).then_some((a, b))
}
fn mul(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a * b)
}
fn add(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a + b)
}
fn sub(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a - b)
}
fn div(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a / b)
}
fn neg(a: f32) -> Option<f32> {
    (!a.is_nan()).then_some(-a)
}

/// RDRAM as a model sees it: the input plus the model's writes.
struct Mem<'a> {
    s: &'a State,
    w: BTreeMap<u32, u32>,
}

impl Mem<'_> {
    fn new(s: &State) -> Mem<'_> {
        Mem { s, w: BTreeMap::new() }
    }
    fn f(&self, a: u32) -> f32 {
        f32::from_bits(self.w.get(&a).copied().unwrap_or_else(|| word(self.s, a)))
    }
    fn set(&mut self, a: u32, x: f32) {
        self.w.insert(a, x.to_bits());
    }
    fn check(&self, after: &State) -> Result<(), TestCaseError> {
        for (&a, &v) in &self.w {
            prop_assert_eq!(word(after, a), v, "word {:#010x}", a);
        }
        Ok(())
    }
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

fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![5 => -1.0e3f32..1.0e3f32, 3 => -1.0f32..1.0f32, 2 => (-8i32..8).prop_map(|n| n as f32), 1 => edge()].boxed()
}

fn put(s: &mut State, a: u32, v: &[f32]) {
    let mut m = s.rdram.mem();
    for (k, x) in v.iter().enumerate() {
        m.write_f32(a + 4 * k as u32, *x);
    }
}

fn rom_word(v: u32) -> u32 {
    let at = (v - 0x8000_0400 + 0x1000) as usize;
    u32::from_be_bytes(baserom()[at..at + 4].try_into().unwrap())
}

/// `n.xyz . p` in the C's order: `n.z p.z + (p.x n.x + p.y n.y)`.
fn plane_dot(n: &[f32; 4], p: &[f32; 3]) -> Option<f32> {
    add(mul(n[2], p[2])?, add(mul(p[0], n[0])?, mul(p[1], n[1])?)?)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_800811C0(seed: u64, x: u64) {
        let mut s = state(seed);
        s.ctx.fpr[12].u64 = x;
        let after = run("func_800811C0", math::func_800811C0, &s)?;
        prop_assert_eq!((word(&after, 0x800A_6750), word(&after, 0x800A_6754)), ((x >> 32) as u32, x as u32));
    }

    #[test]
    fn func_80081680(seed: u64, x in prop_oneof![Just(0.0f32), Just(-0.0f32), ordinary()], n in prop_oneof![-2i32..10, Just(1i32), Just(2i32)],
                     scale in ordinary(), max in prop_oneof![Just(f32::NAN), Just(0.0f32), Just(-0.0f32), ordinary()]) {
        let want = (|| {
            let positive = 0.0 < x;
            let a = if positive { x } else { neg(x)? };
            let mut p = a;
            for _ in 1..n.max(1) {
                p = mul(p, a)?;
            }
            p = mul(p, scale)?;
            if max < p {
                p = max;
            }
            if positive { Some(p) } else { neg(p) }
        })();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        s.ctx.fpr[12].set_fl(x);
        (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(n as u32), sext(scale.to_bits()), sext(max.to_bits()));
        let after = run("func_80081680", math::func_80081680, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.unwrap().to_bits());
        prop_assert_eq!(word(&after, SP_AT + 0xC), max.to_bits());
    }

    #[test]
    fn func_80081700(seed: u64, a in ordinary(), b in ordinary(), rom: bool, k in ordinary()) {
        let k = if rom { f32::from_bits(rom_word(0x800A_DCB0)) } else { k };
        let want = (|| {
            let r = div(b, k)?;
            sub(1.0, div(r, add(r, a)?)?)
        })();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        s.rdram.mem().write_f32(0x800A_DCB0, k);
        s.ctx.fpr[12].set_fl(a);
        s.ctx.fpr[14].set_fl(b);
        let after = run("func_80081700", math::func_80081700, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.unwrap().to_bits());
    }

    /// `out` may be `v` itself (re-read per component).
    #[test]
    fn func_80081730(seed: u64, v in prop::array::uniform4(ordinary()), m in prop::array::uniform16(ordinary()), alias: bool) {
        let mut s = state(seed);
        put(&mut s, A, &v);
        put(&mut s, B, &m);
        let out = if alias { A } else { OUT };
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(out), sext(A), sext(B));
        let mut mem = Mem::new(&s);
        let ok = (|| {
            for j in 0..4u32 {
                let e = |i: u32| mem.f(B + 16 * i + 4 * j);
                let x = add(e(3), add(add(mul(e(0), mem.f(A))?, mul(e(1), mem.f(A + 4))?)?, mul(e(2), mem.f(A + 8))?)?)?;
                mem.set(out + 4 * j, x);
            }
            Some(())
        })();
        prop_assume!(ok.is_some());
        let after = run("func_80081730", math::func_80081730, &s)?;
        mem.check(&after)?;
    }

    /// `out` may be `p` itself.
    #[test]
    fn func_800819A4(seed: u64, n in prop::array::uniform4(ordinary()), p in prop::array::uniform3(ordinary()), alias: bool) {
        let mut s = state(seed);
        put(&mut s, A, &n);
        put(&mut s, B, &p);
        let out = if alias { B } else { OUT };
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(A), sext(B), sext(out));
        let mut mem = Mem::new(&s);
        let ok = (|| {
            let t = sub(n[3], plane_dot(&n, &p)?)?;
            for k in 0..3u32 {
                let x = mul(mem.f(A + 4 * k), t)?;
                mem.set(out + 4 * k, x);
            }
            for k in 0..3u32 {
                let x = add(mem.f(out + 4 * k), mem.f(B + 4 * k))?;
                mem.set(out + 4 * k, x);
            }
            Some(())
        })();
        prop_assume!(ok.is_some());
        let after = run("func_800819A4", math::func_800819A4, &s)?;
        mem.check(&after)?;
    }

    #[test]
    fn func_80081A2C(seed: u64, p in prop::array::uniform3(ordinary()), a in prop::array::uniform3(ordinary()),
                     b in prop::array::uniform3(ordinary()), same: bool, rom: bool, e in prop_oneof![Just(0.0f64), 0.0f64..10.0]) {
        let b = if same { a } else { b };
        let e = if rom { f64::from_bits(u64::from(rom_word(0x800A_DCC0)) << 32 | u64::from(rom_word(0x800A_DCC4))) } else { e };
        let want = (|| -> Option<[f32; 3]> {
            let d = [sub(b[0], a[0])?, sub(b[1], a[1])?, sub(b[2], a[2])?];
            let pd = add(mul(d[2], p[2])?, add(mul(p[0], d[0])?, mul(p[1], d[1])?)?)?;
            let ad = add(mul(d[2], a[2])?, add(mul(a[0], d[0])?, mul(a[1], d[1])?)?)?;
            let dd = add(mul(d[2], d[2])?, add(mul(d[0], d[0])?, mul(d[1], d[1])?)?)?;
            if f64::from(dd) <= e {
                return Some(a);
            }
            let u = sub(pd, ad)?;
            let t = div(u, dd)?;
            if t <= 0.0 {
                return Some(a);
            }
            if 1.0 <= t {
                return Some(b);
            }
            let q = div(u, dd)?;
            Some([add(a[0], mul(d[0], q)?)?, add(a[1], mul(d[1], q)?)?, add(a[2], mul(d[2], q)?)?])
        })();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, A, &p);
        put(&mut s, B, &a);
        put(&mut s, C, &b);
        s.rdram.mem().write_f64(0x800A_DCC0, e);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(A), sext(B), sext(C), sext(OUT));
        let after = run("func_80081A2C", math::func_80081A2C, &s)?;
        let got: Vec<u32> = (0..3).map(|k| word(&after, OUT + 4 * k)).collect();
        prop_assert_eq!(got, want.unwrap().map(f32::to_bits).to_vec());
    }

    #[test]
    fn func_800823A4(seed: u64, n in prop::array::uniform4(ordinary()), p in prop::array::uniform3(ordinary())) {
        let want = plane_dot(&n, &p).and_then(|d| sub(n[3], d));
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, A, &n);
        put(&mut s, B, &p);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(A), sext(B));
        let after = run("func_800823A4", math::func_800823A4, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.unwrap().to_bits());
    }

    #[test]
    fn func_800823DC(seed: u64, n in prop::array::uniform4(ordinary()), p in prop::array::uniform3(ordinary())) {
        let want = (|| add(p[2], div(sub(n[3], plane_dot(&n, &p)?)?, n[2])?))();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, A, &n);
        put(&mut s, B, &p);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(A), sext(B));
        let after = run("func_800823DC", math::func_800823DC, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.unwrap().to_bits());
    }

    #[test]
    fn func_8008241C(seed: u64, a in prop::array::uniform2(ordinary()), b in prop::array::uniform2(ordinary())) {
        let want = (|| sub(mul(a[0], b[1])?, mul(b[0], a[1])?))();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, A, &a);
        put(&mut s, B, &b);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(A), sext(B));
        let after = run("func_8008241C", math::func_8008241C, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.unwrap().to_bits());
    }
}
