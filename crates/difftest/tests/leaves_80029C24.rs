//! The float leaves at 0x80029C24..0x800320E0 (game::misc): two scaled
//! counts and a flag, a float global's reset and getter, a bare epilogue,
//! and the stats normaliser. Recompiled C vs Rust, each checked against its
//! statement.
//!
//! NaN operands of arithmetic are outside these ports' domain (NAN_CHECK in
//! the oracle); models return None for inputs that would reach one.

// Tests are named after the functions (func_80029C24), capitals included.
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

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

fn rom_f(vaddr: u32) -> f32 {
    let o = (vaddr - 0x8000_0400 + 0x1000) as usize;
    f32::from_be_bytes(baserom()[o..o + 4].try_into().unwrap())
}

fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}

/// A C cast to int: `0x80000000` for NaN and out of range.
fn trunc(x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 { i32::MIN } else { x as i32 }
}

fn float() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        -1.0e3f32..1.0e3f32,
        -1.0f32..1.0f32,
        Just(f32::INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

// ---------------------------------------------------------------------------
// func_80029C24

fn b(s: &State, a: u32) -> u8 {
    s.rdram.clone().mem().read_u8(a)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Small record indices, entries with signed indices, words and bytes
    /// of any size, and `K` from the ROM or drawn.
    #[test]
    fn func_80029C24(seed: u64, i in 0u32..6, idx1 in -8i8..8, rec: [u8; 2], ent in prop::array::uniform8(any::<u32>()), idx2: i8, t1: u32,
                     byte: u8, v in 0u8..16, rom_k: bool, k in float()) {
        let k = if rom_k { rom_f(0x800A_9E08) } else { k };
        let mut s = state(seed);
        let r = 0x8011_98A8 + 56 * i;
        {
            let mut m = s.rdram.mem();
            m.write_u32(0x8011_A270, i);
            // r[0] selects e = E[r[0]] (signed), r[1] a byte.
            m.write_u8(r, idx1 as u8);
            m.write_u8(r + 1, rec[1]);
            let e = 0x800A_2DE0u32.wrapping_add((i32::from(idx1) * 16) as u32);
            m.write_u8(e + 3, v);
            m.write_u32(e + 4, ent[0]);
            m.write_u8(0x8011_A050 + 56 * u32::from(v), idx2 as u8);
            let e2 = 0x800A_2DE0u32.wrapping_add((i32::from(idx2) * 16) as u32);
            m.write_u32(e2 + 4, ent[1]);
            m.write_u8(0x8011_3E84 + u32::from(v), byte);
            m.write_u32(0x8011_3E78, t1);
            m.write_u32(0x800A_9E08, k.to_bits());
        }
        let e2 = 0x800A_2DE0u32.wrapping_add((i32::from(idx2) * 16) as u32);
        let (w1, w2) = (word(&s, e2 + 4), word(&s, 0x800A_2DE0u32.wrapping_add((i32::from(idx1) * 16) as u32) + 4));
        let a = trunc((u32::from(byte).wrapping_mul(w1) as i32 as f32) * k).wrapping_add(1);
        let bb = trunc((u32::from(b(&s, r + 1)).wrapping_mul(w2) as i32 as f32) * k).wrapping_add(1);
        let after = run("func_80029C24", misc::func_80029C24, &s)?;
        prop_assert_eq!(word(&after, 0x800D_6CC8), a as u32);
        prop_assert_eq!(word(&after, 0x800D_6CCC), bb as u32);
        let flag = u32::from((t1 as i32).wrapping_add(a) < bb);
        prop_assert_eq!(word(&after, 0x800D_6CC4) >> 16, flag);
    }

    #[test]
    fn float_global(seed: u64, x: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x800D_7740, x);
        let after = run("func_8002F060", misc::func_8002F060, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), x);
        let after = run("func_8002F000", misc::func_8002F000, &s)?;
        prop_assert_eq!([word(&after, 0x800A_26F4), word(&after, 0x800A_26F8), word(&after, 0x800D_7740)], [0, 1, 0]);
    }

    /// The epilogue restores from the frame, sign-extending the words.
    #[test]
    fn func_8002F9C0(seed: u64, frame: [u32; 16]) {
        let mut s = state(seed);
        for (k, w) in frame.iter().enumerate() {
            s.rdram.mem().write_u32(SP_AT + 4 * k as u32, *w);
        }
        let after = run("func_8002F9C0", misc::func_8002F9C0, &s)?;
        let g = &after.ctx.gpr;
        prop_assert_eq!([g[RA], g[S0], g[S1], g[S2]], [sext(frame[13]), sext(frame[10]), sext(frame[11]), sext(frame[12])]);
        prop_assert_eq!(after.ctx.fpr[20].u64, u64::from(frame[6]) << 32 | u64::from(frame[7]));
        prop_assert_eq!(after.ctx.fpr[22].u64, u64::from(frame[8]) << 32 | u64::from(frame[9]));
        prop_assert_eq!(g[SP], sext(SP_AT + 0x38));
    }
}

// ---------------------------------------------------------------------------
// func_800320E0: the stats normaliser

const S: u32 = 0x8030_0000;
const OUT: u32 = 0x8030_0100;

fn normalise(st: &State, s: &[f32; 12]) -> Option<[u32; 7]> {
    let kf = |a: u32| f32::from_bits(word(st, a));
    let (k1, k2, k3) = (kf(0x800A_A3A0), kf(0x800A_A3A4), kf(0x800A_A3A8));
    for i in [0, 1, 3, 4, 5, 9] {
        ok(s[i])?;
    }
    let r3 = ok((s[3] * 1.0).sqrt())?;
    let r5 = ok((s[5] * 0.5).sqrt())?;
    let v = [
        s[0] * 1.0,
        ok(s[1] / 1000.0)?,
        1.0 - ok(r3 / k1)?,
        ok(ok(s[4] - 450.0)? / 200.0)?,
        ok(8.0 / r5)? - k2,
        ok(s[9] / 20.0)?,
        s[11],
    ];
    let mut out = [0u32; 7];
    for (o, mut x) in out.iter_mut().zip(v) {
        if x < k3 {
            x = k3;
        }
        if 1.0 < x {
            x = 1.0;
        }
        *o = x.to_bits();
    }
    Some(out)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Stats in their ranges (from func_800321F0's bounds) and edge values;
    /// the constants from the ROM, or perturbed.
    #[test]
    fn func_800320E0(seed: u64, s in prop::array::uniform12(prop_oneof![
                         3 => 0.0f32..1.0, 2 => 0.0f32..1000.0, 2 => 400.0f32..700.0, 1 => float()]),
                     s11_bits: u32, nan11: bool, perturb in prop::option::weighted(0.3, [0.5f32..2.0, 0.5f32..2.0, -0.5f32..1.0])) {
        let mut st = state(seed);
        let mut s = s;
        for (k, a) in [0x800A_A3A0u32, 0x800A_A3A4, 0x800A_A3A8].iter().enumerate() {
            let v = rom_f(*a) * perturb.map_or(1.0, |p| p[k]);
            st.rdram.mem().write_u32(*a, v.to_bits());
        }
        if nan11 {
            s[11] = f32::from_bits(s11_bits);
        }
        let want = normalise(&st, &s);
        prop_assume!(want.is_some());
        for (k, x) in s.iter().enumerate() {
            st.rdram.mem().write_u32(S + 4 * k as u32, x.to_bits());
        }
        (st.ctx.gpr[A0], st.ctx.gpr[A1]) = (sext(OUT), sext(S));
        let after = run("func_800320E0", misc::func_800320E0, &st)?;
        let got: Vec<u32> = (0..7).map(|k| word(&after, OUT + 4 * k)).collect();
        prop_assert_eq!(got, want.unwrap().to_vec());
    }
}
