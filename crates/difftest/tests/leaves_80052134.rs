//! Float leaves at 0x80052134..0x80065E18 (game::misc): a wrapped
//! progress figure, a table-driven settings pick, a point lookup, a clamped
//! accumulator and countdown, and two stores. Recompiled C vs Rust, each
//! checked against its statement: the store-only ones by simulating the
//! stores on a copy of the input and comparing all of RDRAM.
//!
//! NaN operands of arithmetic are outside the domain (NAN_CHECK in the
//! oracle); models return None for inputs that would reach one, skipped
//! before the C runs. NaNs that are only compared or moved are in it.

// Tests are named after the functions (func_80052134), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::misc;
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
const P: u32 = 0x8030_0000;
const O: u32 = 0x8030_1000;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
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
fn neg(a: f32) -> Option<f32> {
    (!a.is_nan()).then_some(-a)
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

/// Quarters (ties at 0.5 apart), fractions, anything.
fn fraction() -> BoxedStrategy<f32> {
    prop_oneof![2 => (0i32..8).prop_map(|n| n as f32 * 0.25), 3 => 0.0f32..1.0, 1 => ordinary()].boxed()
}

fn rom_f32(v: u32) -> f32 {
    let at = (v - 0x8000_0400 + 0x1000) as usize;
    f32::from_bits(u32::from_be_bytes(baserom()[at..at + 4].try_into().unwrap()))
}

// ---- func_80052134 ----

fn wrapped(a: f32, b: f32, n: i32) -> Option<f32> {
    let mut d = sub(a, b)?;
    if a < b {
        d = neg(d)?;
    }
    if 0.5 < d {
        d = sub(1.0, d)?;
    }
    let r = sub(add(n as f32, a)?, d)?;
    Some(if r < 0.0 { 0.0 } else { r })
}

// ---- func_80060668 ----

const TABLE: u32 = 0x800A_5A2C;
const GA: u32 = 0x800A_5B64;
const GB: u32 = 0x800A_5B68;
const GC: u32 = 0x800A_5B6C;
const GD: u32 = 0x800A_6748;
const K: u32 = 0x800A_D094;

struct Pick {
    k: [f32; 4],
    r: i32,
    c: i32,
    v: i32,
    flags: u32,
}

/// The statement's stores on a copy of `s`, or None.
fn pick(s: &State, q: &Pick) -> Option<State> {
    let mut sim = s.clone();
    let mut m = sim.rdram.mem();
    let frame = SP_AT - 0x100;
    for k in 0..64 {
        m.write_u32(frame + 4 * k, word(s, TABLE + 4 * k));
    }
    m.write_u32(GC, u32::MAX);
    m.write_u32(GD, 0);
    m.write_f32(GA, q.k[0]);
    m.write_f32(GB, 20.0);
    let e = frame.wrapping_add((32 * q.r) as u32).wrapping_add((8 * q.c) as u32);
    let x = m.read_f32(e);
    m.write_f32(GA, mul(x, q.k[1])?);
    let y = m.read_u32(e + 4);
    m.write_u32(GB, y);
    let (r, c) = (q.r, q.c);
    if r == 1 && c != 3 {
        m.write_u32(GC, 1);
        if (0..3).contains(&c) {
            m.write_u32(GD, c as u32 + 1);
        }
    }
    if r == 3 && (c == 1 || c == 2) {
        m.write_u32(GC, if c == 1 { 6 } else { 5 });
    }
    if r == 4 && (0..3).contains(&c) {
        m.write_u32(GC, c as u32 + 2);
    }
    let a = m.read_f32(GA);
    match q.v {
        -1 => m.write_f32(GA, mul(a, q.k[2])?),
        1 => m.write_f32(GA, mul(a, q.k[3])?),
        _ => {}
    }
    if q.flags & 0x20 != 0 {
        m.write_f32(GB, 2.0);
    }
    drop(m);
    Some(sim)
}

fn index() -> BoxedStrategy<i32> {
    prop_oneof![6 => 0i32..5, 2 => 0i32..8, 1 => -2i32..11].boxed()
}

// ---- func_80063D0C ----

const POINTS: u32 = 0x800A_5100;

fn point_value() -> BoxedStrategy<f32> {
    prop_oneof![
        3 => prop::sample::select(vec![0.0f32, -0.0, 1.0, 2.5]),
        1 => Just(f32::NAN),
        1 => ordinary(),
    ]
    .boxed()
}

// ---- func_80064A88 / func_80064AF4 ----

const S: u32 = 0x8011_A240;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80052134(seed: u64, a in fraction(), b in fraction(), n in prop_oneof![0i32..10, any::<i32>()]) {
        let want = wrapped(a, b, n);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(P + 0x84, O);
            m.write_u32(P + 0x78, n as u32);
            m.write_f32(O + 0xE8, a);
            m.write_f32(O + 0xE0, b);
        }
        s.ctx.gpr[A0] = sext(P);
        let after = run("func_80052134", misc::func_80052134, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.unwrap().to_bits());
    }

    /// The ROM's constants, or distinct stand-ins (the code tells the four
    /// addresses apart).
    #[test]
    fn func_80060668(seed: u64, table in prop::collection::vec(ordinary().prop_map(f32::to_bits), 64),
                     rom_k: bool, k in prop::array::uniform4(ordinary()), r in index(), c in index(),
                     v in prop_oneof![Just(-1i32), Just(1i32), Just(0i32), any::<i32>()],
                     flags in prop_oneof![Just(0x20u32), Just(0u32), any::<u32>()]) {
        let k = if rom_k { [0, 4, 8, 12].map(|o| rom_f32(K + o)) } else { k };
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for (i, w) in table.iter().enumerate() {
                m.write_u32(TABLE + 4 * i as u32, *w);
            }
            for (i, x) in k.iter().enumerate() {
                m.write_f32(K + 4 * i as u32, *x);
            }
            m.write_u32(P + 0x1AC, r as u32);
            m.write_u32(P + 0x1C0, c as u32);
            m.write_u32(P + 0x1C4, v as u32);
            m.write_u32(P + 8, flags);
        }
        s.ctx.gpr[A0] = sext(P);
        let q = Pick { k, r, c, v, flags };
        let want = pick(&s, &q);
        prop_assume!(want.is_some());
        let after = run("func_80060668", misc::func_80060668, &s)?;
        same_memory(&after, &want.unwrap())?;
    }

    #[test]
    fn func_80063D0C(seed: u64, pts in prop::array::uniform32(point_value()), px in point_value(), py in point_value()) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for k in 0..16u32 {
                m.write_f32(POINTS + 0xC * k, pts[2 * k as usize]);
                m.write_f32(POINTS + 0xC * k + 4, pts[2 * k as usize + 1]);
            }
            m.write_f32(P + 0x50, px);
            m.write_f32(P + 0x54, py);
        }
        s.ctx.gpr[A0] = sext(P);
        let want = (0..16).find(|&k| pts[2 * k] == px && pts[2 * k + 1] == py).map_or(u64::MAX, |k| k as u64);
        let after = run("func_80063D0C", misc::func_80063D0C, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], want);
    }

    #[test]
    fn func_80064A88(seed: u64, old in prop_oneof![-0.5f32..1.5, Just(0.0f32), Just(-0.0f32), Just(1.0f32), ordinary()],
                     x in prop_oneof![Just(-1.0f32), ordinary()], dt in prop_oneof![0.0f32..0.1, Just(0.0f32), Just(1.0f32), ordinary()]) {
        let sum = add(old, mul(x, dt).unwrap_or(f32::NAN));
        prop_assume!(sum.is_some() && !x.is_nan());
        let mut v = sum.unwrap();
        if 1.0 < v {
            v = 1.0;
        }
        if v < 0.0 {
            v = 0.0;
        }
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_f32(S, old);
            m.write_f32(misc::FRAME_DT, dt);
        }
        s.ctx.fpr[12].set_fl(x);
        let mut want = s.clone();
        want.rdram.mem().write_f32(S, v);
        let after = run("func_80064A88", misc::func_80064A88, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), v.to_bits());
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80064AF4(seed: u64, old in prop_oneof![-0.5f32..1.5, Just(0.0f32), Just(-0.0f32), ordinary()],
                     dt in prop_oneof![0.0f32..0.1, Just(0.5f32), ordinary()]) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_f32(S + 0x38, old);
            m.write_f32(misc::FRAME_DT, dt);
        }
        let mut want = s.clone();
        if 0.0 < old {
            let v = sub(old, dt);
            prop_assume!(v.is_some());
            let v = v.unwrap();
            want.rdram.mem().write_f32(S + 0x38, if v < 0.0 { 0.0 } else { v });
        }
        let after = run("func_80064AF4", misc::func_80064AF4, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80065C98(seed: u64, null: bool, x: u32) {
        let mut s = state(seed);
        let p = if null { 0 } else { P };
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(p), sext(x));
        let mut want = s.clone();
        if !null {
            want.rdram.mem().write_u32(P + 0x68, x);
        }
        let after = run("func_80065C98", misc::func_80065C98, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80065E18(seed: u64, x in ordinary(), rest: [u32; 3]) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_f32(0x800A_3080, x);
            for (i, w) in rest.iter().enumerate() {
                m.write_u32(0x800A_3084 + 4 * i as u32, *w);
            }
        }
        s.ctx.gpr[A0] = sext(P);
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            m.write_f32(P + 0x190, x * 0.25);
            for (i, w) in rest.iter().enumerate() {
                m.write_u32(P + 0x194 + 4 * i as u32, *w);
            }
        }
        let after = run("func_80065E18", misc::func_80065E18, &s)?;
        same_memory(&after, &want)?;
    }
}
