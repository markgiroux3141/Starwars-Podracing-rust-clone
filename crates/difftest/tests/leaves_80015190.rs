//! The vector leaves at 0x80015190..0x800156DC (game::math): vec2/vec3
//! set, copy, negate, compare, add, subtract, dot, length, distances, cross,
//! scale, multiply-adds, matrix rows and a 4x4 copy. Recompiled C vs Rust
//! over float edge values and ordinary full-mantissa ones, each checked
//! against its statement.
//!
//! NaN operands of arithmetic are outside these ports' domain (NAN_CHECK in
//! the oracle); models return None for inputs that would reach one, skipped
//! before the C runs. NaN is tested where values are only copied or
//! compared.

// Tests are named after the functions (func_80015190), capitals included.
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
const A: u32 = 0x8030_0100;
const B: u32 = 0x8030_0200;
const C: u32 = 0x8030_0300;

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
        Just(-f32::from_bits(0x007F_FFFF)),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        Just(-1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        (-8i32..8).prop_map(|n| n as f32),
        -1.0e4f32..1.0e4f32,
        -1.0e4f32..1.0e4f32,
        -1.0f32..1.0f32,
        Just(1.0e19f32),
        Just(-3.0e38f32),
        Just(f32::MAX),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

/// Mostly ordinary values (products of similar size, where rounding each
/// step matters), with edge values mixed in.
fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![4 => -1.0e4f32..1.0e4f32, 2 => -1.0f32..1.0f32, 2 => (-8i32..8).prop_map(|n| n as f32), 2 => float()].boxed()
}

fn v3() -> BoxedStrategy<[f32; 3]> {
    [ordinary(), ordinary(), ordinary()].boxed()
}

fn any_bits() -> BoxedStrategy<u32> {
    prop_oneof![
        float().prop_map(f32::to_bits),
        (1u32..0x0080_0000).prop_map(|p| 0x7F80_0000 | p),
        (1u32..0x0080_0000).prop_map(|p| 0xFF80_0000 | p),
        any::<u32>(),
    ]
    .boxed()
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

/// Where `out` goes: its own buffer or over one of the inputs.
fn out_at(alias: u8) -> u32 {
    [OUT, A, B][alias as usize % 3]
}

/// A float passed in a GPR: junk in the upper half.
fn freg(x: f32, junk: u32) -> u64 {
    u64::from(junk) << 32 | u64::from(x.to_bits())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// out = q * s + p (2 components), p = A, q = B.
    #[test]
    fn func_80015190(seed: u64, p in [ordinary(), ordinary()], q in [ordinary(), ordinary()], k in ordinary(), alias in 0u8..3, junk: u32) {
        let want: Option<Vec<f32>> = (0..2).map(|i| Some(ok(q[i] * k)? + p[i])).collect();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, A, &p);
        put(&mut s, B, &q);
        let out = out_at(alias);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(out), sext(A), freg(k, junk), sext(B));
        let after = run("func_80015190", math::func_80015190, &s)?;
        prop_assert_eq!(get(&after, out, 2), bits(&want.unwrap()));
    }

    /// Set and copy move bits, NaNs included.
    #[test]
    fn func_80015268(seed: u64, x in any_bits(), y in any_bits(), z in any_bits(), junk: [u32; 3]) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = sext(OUT);
        (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (u64::from(junk[0]) << 32 | u64::from(x), u64::from(junk[1]) << 32 | u64::from(y), u64::from(junk[2]) << 32 | u64::from(z));
        let after = run("func_80015268", math::func_80015268, &s)?;
        prop_assert_eq!(get(&after, OUT, 3), vec![x, y, z]);
        prop_assert_eq!(word(&after, SP_AT + 0xC), z);
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for (k, v) in [x, y, z].iter().enumerate() {
                m.write_u32(A + 4 * k as u32, *v);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(OUT), sext(A));
        let after = run("func_80015288", math::func_80015288, &s)?;
        prop_assert_eq!(get(&after, OUT, 3), vec![x, y, z]);
    }

    #[test]
    fn func_800152A4(seed: u64, v in v3(), alias: bool) {
        let mut s = state(seed);
        put(&mut s, A, &v);
        let out = if alias { A } else { OUT };
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(out), sext(A));
        let after = run("func_800152A4", math::func_800152A4, &s)?;
        prop_assert_eq!(get(&after, out, 3), bits(&v.map(|x| -x)));
    }

    /// Equal, signed zeros, NaN (never equal) and one component different.
    #[test]
    fn func_800152CC(seed: u64, a in prop::array::uniform3(any_bits()), b in prop::array::uniform3(any_bits()), mode in 0u8..5, which in 0usize..3) {
        let mut b = b;
        match mode {
            0 => b = a,
            1 => { b = a; b[which] ^= 0x8000_0000; }
            2 => { b = a; b[which] = b[which].wrapping_add(1); }
            _ => {}
        }
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for k in 0..3 {
                m.write_u32(A + 4 * k as u32, a[k]);
                m.write_u32(B + 4 * k as u32, b[k]);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(A), sext(B));
        let after = run("func_800152CC", math::func_800152CC, &s)?;
        let eq = (0..3).all(|k| f32::from_bits(a[k]) == f32::from_bits(b[k]));
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(eq));
    }

    /// out = b + a and out = a - b.
    #[test]
    fn func_80015328(seed: u64, a in v3(), b in v3(), alias in 0u8..3) {
        let mut s = state(seed);
        put(&mut s, A, &a);
        put(&mut s, B, &b);
        let out = out_at(alias);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(out), sext(A), sext(B));
        let after = run("func_80015328", math::func_80015328, &s)?;
        prop_assert_eq!(get(&after, out, 3), bits(&[b[0] + a[0], b[1] + a[1], b[2] + a[2]]));
        let after = run("func_8001535C", math::func_8001535C, &s)?;
        let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        prop_assert_eq!(get(&after, out, 3), bits(&d));
    }

    /// dot, length, squared distance and distance.
    #[test]
    fn func_80015390(seed: u64, a in v3(), b in v3()) {
        let mut s = state(seed);
        put(&mut s, A, &a);
        put(&mut s, B, &b);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(A), sext(B));
        let dot = (|| Some(ok(b[2] * a[2])? + ok(ok(a[0] * b[0])? + ok(a[1] * b[1])?)?))();
        if let Some(dot) = dot {
            let after = run("func_80015390", math::func_80015390, &s)?;
            prop_assert_eq!(after.ctx.fpr[0].u32l(), dot.to_bits());
        }
        let len = (|| Some(ok(ok(a[2] * a[2])? + ok(ok(a[0] * a[0])? + ok(a[1] * a[1])?)?)?.sqrt()))();
        if let Some(len) = len {
            let after = run("func_800153C0", math::func_800153C0, &s)?;
            prop_assert_eq!(after.ctx.fpr[0].u32l(), len.to_bits());
        }
        let dsq = (|| {
            let d = [ok(a[0] - b[0])?, ok(a[1] - b[1])?, ok(a[2] - b[2])?];
            Some(ok(ok(d[0] * d[0])? + ok(d[1] * d[1])?)? + ok(d[2] * d[2])?)
        })();
        if let Some(dsq) = dsq {
            let after = run("func_800153EC", math::func_800153EC, &s)?;
            prop_assert_eq!(after.ctx.fpr[0].u32l(), dsq.to_bits());
        }
        // vec2 distance of b - a, and vec3 distance of b - a, spilled.
        let d2 = (|| {
            let d = [ok(b[0] - a[0])?, ok(b[1] - a[1])?];
            Some((d, ok(ok(d[1] * d[1])? + ok(d[0] * d[0])?)?.sqrt()))
        })();
        if let Some((d, r)) = d2 {
            let after = run("func_80015428", math::func_80015428, &s)?;
            prop_assert_eq!(after.ctx.fpr[0].u32l(), r.to_bits());
            prop_assert_eq!(get(&after, SP_AT - 8, 2), bits(&d));
        }
        let d3 = (|| {
            let d = [ok(b[0] - a[0])?, ok(b[1] - a[1])?, ok(b[2] - a[2])?];
            Some((d, ok(ok(d[2] * d[2])? + ok(ok(d[0] * d[0])? + ok(d[1] * d[1])?)?)?.sqrt()))
        })();
        if let Some((d, r)) = d3 {
            let after = run("func_80015470", math::func_80015470, &s)?;
            prop_assert_eq!(after.ctx.fpr[0].u32l(), r.to_bits());
            prop_assert_eq!(get(&after, SP_AT - 0xC, 3), bits(&d));
        }
    }

    /// out = a x b, all three computed before the stores.
    #[test]
    fn func_80015538(seed: u64, a in v3(), b in v3(), alias in 0u8..3) {
        let want = (|| Some([
            ok(a[1] * b[2])? - ok(b[1] * a[2])?,
            ok(a[2] * b[0])? - ok(b[2] * a[0])?,
            ok(a[0] * b[1])? - ok(b[0] * a[1])?,
        ]))();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, A, &a);
        put(&mut s, B, &b);
        let out = out_at(alias);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(out), sext(A), sext(B));
        let after = run("func_80015538", math::func_80015538, &s)?;
        prop_assert_eq!(get(&after, out, 3), bits(&want.unwrap()));
    }

    /// out = v * s.
    #[test]
    fn func_800155C0(seed: u64, v in v3(), k in ordinary(), alias: bool, junk: u32) {
        let mut s = state(seed);
        put(&mut s, A, &v);
        let out = if alias { A } else { OUT };
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(out), freg(k, junk), sext(A));
        let after = run("func_800155C0", math::func_800155C0, &s)?;
        prop_assert_eq!(get(&after, out, 3), bits(&v.map(|x| x * k)));
    }

    /// out = q * s + p (p = A, q = B).
    #[test]
    fn func_800155EC(seed: u64, p in v3(), q in v3(), k in ordinary(), alias in 0u8..3, junk: u32) {
        let want: Option<Vec<f32>> = (0..3).map(|i| Some(ok(q[i] * k)? + p[i])).collect();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, A, &p);
        put(&mut s, B, &q);
        let out = out_at(alias);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(out), sext(A), freg(k, junk), sext(B));
        let after = run("func_800155EC", math::func_800155EC, &s)?;
        prop_assert_eq!(get(&after, out, 3), bits(&want.unwrap()));
    }

    /// out = p * t + q * u (u in a1, q = a2 = B, t in a3, p = stack = C).
    #[test]
    fn func_80015630(seed: u64, p in v3(), q in v3(), t in ordinary(), u in ordinary(), alias in 0u8..3, junk: [u32; 2]) {
        let want: Option<Vec<f32>> = (0..3).map(|i| Some(ok(p[i] * t)? + ok(q[i] * u)?)).collect();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, C, &p);
        put(&mut s, B, &q);
        s.rdram.mem().write_u32(SP_AT + 0x10, C);
        let out = [OUT, B, C][alias as usize];
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(out), freg(u, junk[0]), sext(B), freg(t, junk[1]));
        let after = run("func_80015630", math::func_80015630, &s)?;
        prop_assert_eq!(get(&after, out, 3), bits(&want.unwrap()));
    }

    /// Row set/get and the 4x4 copy move bits.
    #[test]
    fn matrix_rows(seed: u64, k in 0u32..8, v in prop::array::uniform3(any_bits()), mtx in prop::array::uniform16(any_bits())) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for (i, x) in v.iter().enumerate() {
                m.write_u32(A + 4 * i as u32, *x);
            }
            for (i, x) in mtx.iter().enumerate() {
                m.write_u32(B + 4 * i as u32, *x);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(OUT), u64::from(k), sext(A));
        let after = run("func_80015694", math::func_80015694, &s)?;
        prop_assert_eq!(get(&after, OUT + 16 * k, 3), v.to_vec());
        prop_assert_eq!(after.ctx.gpr[V0], sext(OUT + 16 * k));
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(B), u64::from(k % 4), sext(OUT));
        let after = run("func_800156B8", math::func_800156B8, &s)?;
        prop_assert_eq!(get(&after, OUT, 3), mtx[4 * (k % 4) as usize..][..3].to_vec());
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(OUT), sext(B));
        let after = run("func_800156DC", math::func_800156DC, &s)?;
        prop_assert_eq!(get(&after, OUT, 16), mtx.to_vec());
    }
}
