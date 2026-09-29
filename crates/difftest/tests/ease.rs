//! func_80033328 and func_800334F4 (game::misc): values eased toward a
//! target by a frame time (a double), with clamps, and a heading integrated
//! and wrapped. Recompiled C vs Rust, each checked against a model in the
//! code's operation order (double arithmetic where the code uses it).
//!
//! A fused multiply-add in double precision is almost never visible after
//! the result is rounded to f32: the two results must straddle an f32
//! rounding tie. The `separators` test builds such inputs for each of the
//! four double multiply-adds: it fixes the other operands and searches the
//! frame time (a RAM value the test sets) for one where the fused and
//! unfused forms round differently.
//!
//! NaN operands of arithmetic are outside the domain (NAN_CHECK); models
//! return None for inputs that would reach one.

// Tests are named after the functions (func_80033328), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc::{self, FRAME_TIME};
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
const X: u32 = 0x8030_0000;
const ANGLE: u32 = 0x8030_0100;

fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}

fn ok64(x: f64) -> Option<f64> {
    (!x.is_nan()).then_some(x)
}

/// The inputs of func_80033328.
#[derive(Clone, Copy, Debug)]
struct Ease {
    x: f32,
    angle: f32,
    target: f32,
    rate: f32,
    bias: f32,
    add: f32,
    dt: f64,
}

fn model_328(e: Ease) -> Option<(u32, u32)> {
    for v in [e.x, e.angle, e.target, e.rate, e.bias, e.add] {
        ok(v)?;
    }
    ok64(e.dt)?;
    let (mut x, mut rate) = (e.x, e.rate);
    if e.target < x {
        if 0.0 < x {
            rate = ok(rate * 5.0)?;
        }
        x = ok64(f64::from(x) - ok64(e.dt * f64::from(rate))?)? as f32;
        if x < e.target {
            x = e.target;
        }
    } else {
        if x < 0.0 {
            rate = ok(rate * 5.0)?;
        }
        x = ok64(f64::from(x) + ok64(e.dt * f64::from(rate))?)? as f32;
        if e.target < x {
            x = e.target;
        }
    }
    if 0.0 < e.bias && x < 0.0 {
        x = 0.0;
    }
    if e.bias < 0.0 && 0.0 < x {
        x = 0.0;
    }
    let sum = ok(ok(x + e.bias)? + e.add)?;
    let mut a = ok64(f64::from(e.angle) + ok64(f64::from(sum) * e.dt)?)? as f32;
    if 180.0 < a {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    Some((x.to_bits(), a.to_bits()))
}

/// The inputs of func_800334F4: `*x`, `a`, `b`, `s`, and the frame time.
fn model_4f4(x: f32, a: f32, b: f32, s: f32, dt: f64) -> Option<u32> {
    for v in [x, a, b, s] {
        ok(v)?;
    }
    ok64(dt)?;
    let mut t = ok(-ok(a / b)? * s)?;
    if 80.0 < t {
        t = 80.0;
    }
    if t < -80.0 {
        t = -80.0;
    }
    let d = ok(t - x)?;
    let p = ok64(ok64(f64::from(d) * 5.0)? * dt)?;
    Some((ok64(f64::from(x) + p)? as f32).to_bits())
}

fn state(seed: u64, dt: f64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    let mut m = s.rdram.mem();
    m.write_u32(FRAME_TIME, (dt.to_bits() >> 32) as u32);
    m.write_u32(FRAME_TIME + 4, dt.to_bits() as u32);
    drop(m);
    s
}

fn run_328(seed: u64, e: Ease) -> Result<(), TestCaseError> {
    let want = model_328(e);
    prop_assume!(want.is_some());
    let mut s = state(seed, e.dt);
    {
        let mut m = s.rdram.mem();
        m.write_u32(X, e.x.to_bits());
        m.write_u32(ANGLE, e.angle.to_bits());
        m.write_u32(SP_AT + 0x10, e.bias.to_bits());
        m.write_u32(SP_AT + 0x14, e.add.to_bits());
    }
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(X), sext(ANGLE), sext(e.target.to_bits()), sext(e.rate.to_bits()));
    let after = run("func_80033328", misc::func_80033328, &s)?;
    prop_assert_eq!((word(&after, X), word(&after, ANGLE)), want.unwrap(), "{:?}", e);
    Ok(())
}

fn run_4f4(seed: u64, x: f32, a: f32, b: f32, sc: f32, dt: f64) -> Result<(), TestCaseError> {
    let want = model_4f4(x, a, b, sc, dt);
    prop_assume!(want.is_some());
    let mut s = state(seed, dt);
    s.rdram.mem().write_u32(X, x.to_bits());
    s.rdram.mem().write_u32(SP_AT + 0x10, sc.to_bits());
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(X), sext(a.to_bits()), 0x1234, sext(b.to_bits()));
    let after = run("func_800334F4", misc::func_800334F4, &s)?;
    prop_assert_eq!(word(&after, X), want.unwrap(), "x {:e} a {:e} b {:e} s {:e} dt {:e}", x, a, b, sc, dt);
    Ok(())
}

fn float() -> BoxedStrategy<f32> {
    prop_oneof![
        3 => -200.0f32..200.0,
        2 => -1.0f32..1.0,
        1 => prop_oneof![Just(0.0f32), Just(-0.0), Just(180.0), Just(-180.0), Just(80.0), Just(-80.0), Just(f32::INFINITY)],
        1 => any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |v| !v.is_nan()),
    ]
    .boxed()
}

fn frame_time() -> BoxedStrategy<f64> {
    prop_oneof![Just(1.0 / 60.0), Just(1.0 / 30.0), 0.0f64..0.1, -1.0f64..1.0].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn func_80033328(seed: u64, x in float(), angle in float(), target in float(), rate in float(), bias in float(), add in float(), dt in frame_time()) {
        run_328(seed, Ease { x, angle, target, rate, bias, add, dt })?;
    }

    #[test]
    fn func_800334F4(seed: u64, x in float(), a in float(), b in float(), sc in float(), dt in frame_time()) {
        run_4f4(seed, x, a, b, sc, dt)?;
    }
}

/// A frame time near `dt0` where `f32(a +- p * dt)` rounds differently
/// with the product rounded (the code) and fused (a mutant). It aims the
/// sum at an f32 rounding tie, then scans nearby frame times. The product
/// must be about as large as `a`, or its f64 rounding error is far below
/// the sum's last bit and fusing never shows.
fn separator(a: f32, p: f64, sub: bool, dt0: f64) -> f64 {
    let sign = if sub { -1.0 } else { 1.0 };
    let r = (f64::from(a) + sign * p * dt0) as f32;
    // The ties between neighbouring f32s around the expected result, nearest
    // first; a short scan of frame times around each (one step moves the
    // sum by about one f64 ulp, so a single tie is rarely hit exactly).
    for j in 0..400i32 {
        for jj in [j, -j] {
            let rj = f32::from_bits((r.to_bits() as i32 + jj) as u32);
            let tie = (f64::from(rj) + f64::from(f32::from_bits(rj.to_bits() + 1))) / 2.0;
            let dt_tie = (tie - f64::from(a)) / (sign * p);
            for k in -64i64..=64 {
                let dt = f64::from_bits((dt_tie.to_bits() as i64 + k) as u64);
                let plain = (f64::from(a) + sign * (p * dt)) as f32;
                let fused = (sign * p).mul_add(dt, f64::from(a)) as f32;
                if plain.to_bits() != fused.to_bits() {
                    return dt;
                }
            }
        }
    }
    panic!("no separator for a = {a:e}, p = {p:e}");
}

#[test]
fn separators() {
    // 1. Step down (x <= 0 above the target: no 5x): x - dt * rate.
    let (x, rate) = (-1.25f32, 75.0f32);
    let dt = separator(x, f64::from(rate), true, 1.0 / 60.0);
    run_328(1, Ease { x, angle: 10.0, target: -100.0, rate, bias: 0.0, add: 0.0, dt }).unwrap();
    // 2. Step up (x >= 0 below the target): x + dt * rate.
    let (x, rate) = (1.25f32, 75.0f32);
    let dt = separator(x, f64::from(rate), false, 1.0 / 60.0);
    run_328(2, Ease { x, angle: 10.0, target: 100.0, rate, bias: 0.0, add: 0.0, dt }).unwrap();
    // 3. The heading: x lands on the target whatever dt is (a huge rate),
    //    so the sum (x + bias) + add is fixed.
    let (target, bias, add, angle) = (700.0f32, 25.0f32, 25.0f32, 12.5f32);
    let sum = (target + bias) + add;
    let dt = separator(angle, f64::from(sum), false, 1.0 / 60.0);
    run_328(3, Ease { x: 5.0, angle, target, rate: 1.0e9, bias, add, dt }).unwrap();
    // 4. func_800334F4: t = -(a / b) * s = 6 (unclamped), x + (f64(t - x) *
    //    5) * dt.
    let (x, a, b, sc) = (0.3f32, -2.0f32, 1.0f32, 3.0f32);
    let d = -(a / b) * sc - x;
    let dt = separator(x, f64::from(d) * 5.0, false, 1.0 / 60.0);
    run_4f4(4, x, a, b, sc, dt).unwrap();
}
