//! Float leaves at 0x8006B304..0x8006E2FC (game::misc): a grid-cell mask,
//! a flag balance, a threshold flag, an eased value and a gauge's colours.
//! Recompiled C vs Rust, each checked against its statement: the ones that
//! only store by simulating the stores on a copy of the input and comparing
//! all of RDRAM.
//!
//! The ROM repeats constants at addresses the code tells apart (both steps
//! of `func_8006C828` are 0.33, both rates of `func_8006D9DC` 3.2), so the
//! tests also run with distinct stand-ins. NaN operands of arithmetic
//! (conversions to and from double included) are outside the domain;
//! models return None for them, skipped before the C runs.

// Tests are named after the functions (func_8006B304), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::misc;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;
const P: u32 = 0x8030_0000;
const Q: u32 = 0x8030_1000;

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
/// `cvt.d.s`, guarded.
fn wide(a: f32) -> Option<f64> {
    (!a.is_nan()).then_some(f64::from(a))
}
/// `cvt.s.d`, guarded.
fn narrow(a: f64) -> Option<f32> {
    (!a.is_nan()).then_some(a as f32)
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

/// Halves (ties), anything, or NaN (for values only compared).
fn halves_or_nan() -> BoxedStrategy<f32> {
    prop_oneof![4 => (-16i32..16).prop_map(|n| n as f32 * 0.5), 2 => ordinary(), 1 => Just(f32::NAN)].boxed()
}

fn rom_word(v: u32) -> u32 {
    let at = (v - 0x8000_0400 + 0x1000) as usize;
    u32::from_be_bytes(baserom()[at..at + 4].try_into().unwrap())
}

// ---- func_8006B304 ----

const T: u32 = 0x8011_C868;
const H: u32 = 0x8011_C890;
const U: u32 = 0x8011_C858;
const HX: u32 = 0x8011_C88C;

/// The mask of the statement (`t[k]` for k = 1..7, `u[k]` for k = 1, 2).
fn grid(x: f32, y: f32, t: &[f32; 8], h: f32, u: &[f32; 3], hx: f32) -> Option<u32> {
    let mut r = 0u32;
    if sub(t[7], h)? < y {
        r = 0x80_0000;
    }
    for k in (1..=6).rev() {
        if y < add(t[k + 1], h)? && sub(t[k], h)? < y {
            r |= 1 << (16 + k);
        }
    }
    if y < add(t[1], h)? {
        r |= 0x1_0000;
    }
    let mut m = 0;
    let (lo2, hi2) = (sub(u[2], hx)?, add(u[2], hx)?);
    if lo2 < x {
        m = r >> 8;
    }
    if x < hi2 && sub(u[1], hx)? < x {
        m |= r;
    }
    if x < add(u[1], hx)? {
        m |= r << 8;
    }
    Some(m)
}

// ---- func_8006C828 ----

const K: u32 = 0x800A_D600;
const S0: u32 = 0x800A_D608;
const S1: u32 = 0x800A_D60C;

// ---- func_8006D9DC ----

const RATE_UP: u32 = 0x800A_D6A4;
const RATE_DOWN: u32 = 0x800A_D6A8;
const E: u32 = 0x800A_D6B0;
const FRAME_TIME: u32 = 0x8012_0BF0;

struct Ease {
    speed: f32,
    target: f32,
    v: f32,
    dt: f64,
    up: f32,
    down: f32,
    e: f64,
}

/// The final `[p + 0x208]`, or None.
fn ease(q: &Ease) -> Option<f32> {
    let t = if q.speed < 200.0 { 0.0 } else { q.target };
    let mut v = q.v;
    if v < t {
        v = add(v, mul(q.up, narrow(q.dt)?)?)?;
        if t < v {
            v = t;
        }
    } else if t < v {
        v = sub(v, mul(q.down, narrow(q.dt)?)?)?;
        if v < t {
            v = t;
        }
    }
    if wide(t)? == 0.0 {
        let a = if v < 0.0 { -v } else { v };
        if wide(a)? < q.e {
            v = narrow(wide(v)? * 0.5)?;
        }
    }
    Some(v)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Integer thresholds and half-widths make points land on the bounds.
    #[test]
    fn func_8006B304(seed: u64, x in halves_or_nan(), y in halves_or_nan(),
                     t in prop::array::uniform8(prop_oneof![3 => (-8i32..8).prop_map(|n| n as f32), 1 => ordinary()]),
                     h in prop_oneof![Just(0.0f32), Just(0.5f32), Just(1.0f32), 0.0f32..3.0, ordinary()],
                     u in prop::array::uniform3(prop_oneof![3 => (-8i32..8).prop_map(|n| n as f32), 1 => ordinary()]),
                     hx in prop_oneof![Just(0.0f32), Just(0.5f32), Just(1.0f32), 0.0f32..3.0, ordinary()]) {
        let want = grid(x, y, &t, h, &u, hx);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_f32(P + 0x50, x);
            m.write_f32(P + 0x54, y);
            for k in 1..8u32 {
                m.write_f32(T + 4 * k, t[k as usize]);
            }
            m.write_f32(H, h);
            m.write_f32(U + 4, u[1]);
            m.write_f32(U + 8, u[2]);
            m.write_f32(HX, hx);
        }
        s.ctx.gpr[A0] = sext(P);
        let mut want_s = s.clone();
        {
            let mut m = want_s.rdram.mem();
            m.write_f32(SP_AT - 0xC, x);
            m.write_f32(SP_AT - 8, y);
            m.write_u32(P + 0x26C, want.unwrap());
        }
        let after = run("func_8006B304", misc::func_8006B304, &s)?;
        same_memory(&after, &want_s)?;
    }

    #[test]
    fn func_8006C828(seed: u64, v in prop::array::uniform6(prop_oneof![2 => (-4i32..4).prop_map(|n| n as f32 * 0.25), 2 => 0.0f32..2.0, 1 => ordinary()]),
                     rom: bool, k in prop_oneof![Just(0.5f64), Just(0.0f64), -1.0f64..2.0], steps in prop::array::uniform2(ordinary())) {
        let (k, s0, s1) = if rom {
            (f64::from_bits(u64::from(rom_word(K)) << 32 | u64::from(rom_word(K + 4))), f32::from_bits(rom_word(S0)), f32::from_bits(rom_word(S1)))
        } else {
            (k, steps[0], steps[1])
        };
        let mut want = Some(0.0f32);
        for (i, x) in v.iter().enumerate() {
            let step = if i == 0 { s0 } else { s1 };
            want = want.and_then(|acc| {
                if k < f64::from(*x) {
                    if i < 3 { sub(acc, step) } else { add(acc, step) }
                } else {
                    Some(acc)
                }
            });
        }
        prop_assume!(want.is_some());
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for (i, x) in v.iter().enumerate() {
                m.write_f32(P + 0x288 + 4 * i as u32, *x);
            }
            m.write_f64(K, k);
            m.write_f32(S0, s0);
            m.write_f32(S1, s1);
        }
        s.ctx.gpr[A0] = sext(P);
        let after = run("func_8006C828", misc::func_8006C828, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.unwrap().to_bits());
    }

    #[test]
    fn func_8006D0C0(seed: u64, a1: u32, qa in halves_or_nan(), pb in halves_or_nan(), flags in prop_oneof![Just(0x200u32), Just(0u32), any::<u32>()],
                     flag in prop_oneof![Just(0u32), any::<u32>()], d in prop_oneof![Just(400.0f32), Just(400.5f32), halves_or_nan()]) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_f32(Q + 8, qa);
            m.write_f32(P + 0x58, pb);
            m.write_u32(P + 0x64, flags);
            m.write_f32(P + 0x184, d);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(P), sext(a1), sext(Q), sext(flag));
        let v = if qa < pb { 0.0 } else if flags & 0x200 != 0 || flag == 0 { 500.0 } else { d };
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            m.write_u32(SP_AT + 4, a1);
            m.write_f32(P + 0x2FC, if 400.0 < v { -1.0 } else { 0.0 });
        }
        let after = run("func_8006D0C0", misc::func_8006D0C0, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8006D9DC(seed: u64, speed in prop_oneof![Just(200.0f32), Just(199.5f32), 0.0f32..400.0, ordinary()],
                     target in prop_oneof![Just(0.0f32), Just(-0.0f32), halves_or_nan()],
                     v in prop_oneof![-0.2f32..0.2, Just(0.1f32), Just(-0.1f32), Just(0.125f32), Just(-0.125f32), halves_or_nan()],
                     dt in prop_oneof![Just(1.0f64 / 60.0), 0.0f64..0.1, Just(0.0f64), Just(0.5f64)],
                     rom: bool, rates in prop::array::uniform2(prop_oneof![0.0f32..10.0, ordinary()]),
                     e in prop_oneof![Just(0.1f64), Just(0.125f64), 0.0f64..1.0]) {
        let (up, down, e) = if rom {
            (f32::from_bits(rom_word(RATE_UP)), f32::from_bits(rom_word(RATE_DOWN)), f64::from_bits(u64::from(rom_word(E)) << 32 | u64::from(rom_word(E + 4))))
        } else {
            (rates[0], rates[1], e)
        };
        let q = Ease { speed, target, v, dt, up, down, e };
        let want = ease(&q);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_f32(P + 0x1A0, speed);
            m.write_f32(P + 0x208, v);
            m.write_f64(FRAME_TIME, dt);
            m.write_f32(RATE_UP, up);
            m.write_f32(RATE_DOWN, down);
            m.write_f64(E, e);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(P), sext(target.to_bits()));
        let mut want_s = s.clone();
        want_s.rdram.mem().write_f32(P + 0x208, want.unwrap());
        let after = run("func_8006D9DC", misc::func_8006D9DC, &s)?;
        same_memory(&after, &want_s)?;
    }

    #[test]
    fn func_8006E2FC(seed: u64, mode in prop_oneof![0u32..4, any::<u32>()], a in ordinary(), b in prop_oneof![0.0f32..2000.0, ordinary()], c in ordinary()) {
        const RGB: u32 = 0x8030_2000;
        const RGBA: u32 = 0x8030_2010;
        const OUT: u32 = 0x8030_2020;
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(P + 0x210, mode);
            m.write_f32(P + 0x7C, a);
            m.write_f32(P + 0x1A0, b);
            m.write_f32(P + 0x214, c);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(P), sext(RGB), sext(RGBA), sext(OUT));
        let out = match mode {
            0 => {
                let r = narrow(wide(b).unwrap() / (wide(a).unwrap() * 0.75)).unwrap_or(f32::NAN);
                Some(if 1.0 < r { 1.0 } else { r })
            }
            1 => mul(c, 1.0),
            2 => Some(1.0),
            _ => Some(0.0),
        };
        prop_assume!(out.is_some() && !(mode == 0 && (a.is_nan() || b.is_nan())));
        // A NaN quotient (0 / 0, inf / inf) is narrowed: a guarded operand.
        prop_assume!(!(mode == 0 && (f64::from(b) / (f64::from(a) * 0.75)).is_nan()));
        let (rgb, rgba) = match mode {
            0 | 1 => ([0, 0xFF, 0], if mode == 0 { [0xFF, 0xFF, 0xFF, 0x64] } else { [0xFF, 0x80, 0, 0xC8] }),
            2 => ([0xFF, 0xFF, 0], [0xFF, 0x80, 0, 0xC8]),
            _ => ([0, 0xFF, 0], [0xFF, 0xFF, 0xFF, 0x64]),
        };
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            for (k, v) in rgb.iter().enumerate() {
                m.write_u8(RGB + k as u32, *v);
            }
            for (k, v) in rgba.iter().enumerate() {
                m.write_u8(RGBA + k as u32, *v);
            }
            m.write_f32(OUT, out.unwrap());
        }
        let after = run("func_8006E2FC", misc::func_8006E2FC, &s)?;
        same_memory(&after, &want)?;
    }
}
