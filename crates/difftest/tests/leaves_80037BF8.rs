//! The float leaves at 0x80037BF8..0x8004110C (game::misc, game::spline): a
//! transform rotated about a pivot, a spline point's position, a screen
//! position as fractions, a wrapping texture scroll and a field copy.
//! Recompiled C vs Rust, each checked against its statement.
//!
//! NaN operands of arithmetic are outside the domain (NAN_CHECK); models
//! return None for inputs that would reach one.

// Tests are named after the functions (func_80037BF8), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::recomp::{reg::*, RecompFn};
use game::{misc, spline};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

fn half(s: &State, a: u32) -> i16 {
    let w = word(s, a & !3);
    (if a & 2 == 0 { w >> 16 } else { w }) as u16 as i16
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;
const T: u32 = 0x8030_0000;
const R: u32 = 0x8030_0100;
const P: u32 = 0x8030_0200;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}

fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![
        4 => -1.0e3f32..1.0e3,
        3 => -1.0f32..1.0,
        1 => prop_oneof![Just(0.0f32), Just(-0.0), Just(0.5), Just(f32::INFINITY)],
        1 => any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |v| !v.is_nan()),
    ]
    .boxed()
}

fn put(s: &mut State, a: u32, v: &[f32]) {
    let mut m = s.rdram.mem();
    for (k, x) in v.iter().enumerate() {
        m.write_u32(a + 4 * k as u32, x.to_bits());
    }
}

/// A C cast to int: `0x80000000` for NaN and out of range.
fn trunc(x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 { i32::MIN } else { x as i32 }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80037BF8(seed: u64, t in prop::array::uniform3(ordinary()), r in prop::array::uniform9(ordinary()), p in prop::array::uniform3(ordinary())) {
        let want: Option<Vec<u32>> = (0..3)
            .map(|k| {
                let mut v = t[k];
                for (i, pi) in p.iter().enumerate() {
                    v = ok(v + ok(ok(-*pi)? * r[3 * i + k])?)?;
                }
                Some((v + p[k]).to_bits())
            })
            .collect();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        put(&mut s, T + 0x24, &t);
        put(&mut s, R, &r);
        put(&mut s, P, &p);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(T), sext(R), sext(P));
        let after = run("func_80037BF8", misc::func_80037BF8, &s)?;
        prop_assert_eq!((0..3).map(|k| word(&after, T + 0x24 + 4 * k)).collect::<Vec<_>>(), want.unwrap());
    }

    #[test]
    fn func_8003A4A0(seed: u64, i in 0u32..20, pos: [u32; 3]) {
        let mut s = state(seed);
        let points = 0x8031_0000;
        {
            let mut m = s.rdram.mem();
            m.write_u32(T + 0xC, points);
            for (k, w) in pos.iter().enumerate() {
                m.write_u32(points + 84 * i + 0x10 + 4 * k as u32, *w);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(T), u64::from(i), sext(P));
        let after = run("func_8003A4A0", spline::func_8003A4A0, &s)?;
        prop_assert_eq!([word(&after, P), word(&after, P + 4), word(&after, P + 8)], pos);
    }

    #[test]
    fn func_8003D49C(seed: u64, h: [i16; 2]) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x8011_4470, (u32::from(h[0] as u16) << 16) | u32::from(h[1] as u16));
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(P), sext(P + 4));
        let after = run("func_8003D49C", misc::func_8003D49C, &s)?;
        prop_assert_eq!(word(&after, P), ((f64::from(h[0]) / 320.0) as f32).to_bits());
        prop_assert_eq!(word(&after, P + 4), ((f64::from(h[1]) / 240.0) as f32).to_bits());
    }

    /// Null `o`, null target, and positions and sizes of both signs (sizes
    /// positive mostly, as for a texture). `tie` scrolls from 0 by exactly
    /// the size (speed 1.0), so the wrap test sees `u == w`.
    #[test]
    fn func_8003E0A0(seed: u64, mode in 0u8..3, flags: u32, pos: [i16; 2], size in prop::array::uniform2(prop_oneof![1i16..256, any::<i16>()]),
                     sx in ordinary(), sy in ordinary(), tie: bool) {
        let (pos, sx, sy) = if tie { ([0, 0], 1.0, 1.0) } else { (pos, sx, sy) };
        let mut s = state(seed);
        let tex = 0x8030_0300;
        {
            let mut m = s.rdram.mem();
            m.write_u32(T, flags);
            m.write_u32(T + 8, if mode == 1 { 0 } else { tex });
            m.write_u16(T + 4, pos[0] as u16);
            m.write_u16(T + 6, pos[1] as u16);
            m.write_u16(tex + 4, size[0] as u16);
            m.write_u16(tex + 6, size[1] as u16);
        }
        let step = |u: i16, w: i16, sp: f32| -> Option<i16> {
            let mut u = trunc(ok(f32::from(u) + ok(sp * f32::from(w))?)?) as i16;
            if w < u {
                u = u.wrapping_sub(w);
            }
            if u < 0 {
                u = u.wrapping_add(w);
            }
            Some(u)
        };
        let want = if mode == 2 || mode == 1 { None } else { Some((step(pos[0], size[0], sx), step(pos[1], size[1], sy))) };
        if let Some((a, b)) = want {
            prop_assume!(a.is_some() && b.is_some());
        }
        s.ctx.gpr[A0] = if mode == 2 { 0 } else { sext(T) };
        (s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(sx.to_bits()), sext(sy.to_bits()));
        let after = run("func_8003E0A0", misc::func_8003E0A0, &s)?;
        match mode {
            2 => prop_assert_eq!(word(&after, T), flags),
            _ => prop_assert_eq!(word(&after, T), flags | 0x8000),
        }
        if let Some((Some(a), Some(b))) = want {
            prop_assert_eq!((half(&after, T + 4), half(&after, T + 6)), (a, b));
        } else if mode == 1 {
            prop_assert_eq!((half(&after, T + 4), half(&after, T + 6)), (pos[0], pos[1]));
        }
    }

    #[test]
    fn func_8004110C(seed: u64, v: u32) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 1, T, 0x300);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(T), sext(v));
        let after = run("func_8004110C", misc::func_8004110C, &s)?;
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            for k in 0..16 {
                m.write_u32(T + 0x224 + 4 * k, word(&s, T + 0x20 + 4 * k));
                m.write_u32(T + 0x264 + 4 * k, word(&s, T + 0x108 + 4 * k));
            }
            m.write_u32(T + 0x7C, v);
        }
        prop_assert!(after.rdram.as_words() == want.rdram.as_words(), "memory differs from the model");
    }
}
