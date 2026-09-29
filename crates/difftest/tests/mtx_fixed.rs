//! func_800344F4 and func_80034650 (game::misc): float matrices to the RSP's
//! s15.16 `Mtx` with clamps, small-value zeroing and a mirror flag.
//! Recompiled C vs Rust, each checked against the statement, with the
//! thresholds loaded from the ROM image.

// Tests are named after the functions (func_800344F4), capitals included.
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
const OUT: u32 = 0x8030_0000;
const M: u32 = 0x8030_0100;
const SETTINGS: u32 = 0x800D_697C;

fn rom_word(vaddr: u32) -> u32 {
    let o = (vaddr - 0x8000_0400 + 0x1000) as usize;
    u32::from_be_bytes(baserom()[o..o + 4].try_into().unwrap())
}

/// The converted word of one entry.
fn fixed(v: f32, neg: bool, lo: f32, hi: f32) -> u32 {
    let mut v = if neg { -v } else { v };
    if 0.0 < v {
        if 32000.0 < v {
            v = 32000.0;
        } else if v < hi {
            v = 0.0;
        }
    } else if v < -32000.0 {
        v = -32000.0;
    } else if lo < v {
        v = 0.0;
    }
    (v * 65536.0) as i32 as u32
}

/// The `Mtx`: integer halves then fraction halves, by entry.
fn mtx(words: &[u32; 16]) -> Vec<u16> {
    let mut out: Vec<u16> = words.iter().map(|w| (w >> 16) as u16).collect();
    out.extend(words.iter().map(|w| *w as u16));
    out
}

fn halves(s: &State, n: usize) -> Vec<u16> {
    let m = s.rdram.clone();
    let mut r = m;
    let mm = r.mem();
    (0..n as u32).map(|k| mm.read_u16(OUT + 2 * k)).collect()
}

fn entry() -> BoxedStrategy<f32> {
    let e = f32::from_bits(rom_word(0x800A_AAD4));
    prop_oneof![
        3 => -40000.0f32..40000.0,
        3 => -2.0f32..2.0,
        1 => prop::sample::select(vec![32000.0f32, -32000.0, e, -e, 0.0, -0.0, 32000.5, -32000.5, 1.0e-7, -1.0e-7]),
        1 => (-3i32..=3).prop_map(move |k| f32::from_bits((e.to_bits() as i32 + k) as u32)),
        1 => any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |v| !v.is_nan()),
    ]
    .boxed()
}

fn state(seed: u64, mirror: bool, settings: u32) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    let mut m = s.rdram.mem();
    m.write_u32(SETTINGS, if mirror { settings | 0x4000 } else { settings & !0x4000 });
    for a in [0x800A_AAD0u32, 0x800A_AAD4, 0x800A_AAD8, 0x800A_AADC] {
        m.write_u32(a, rom_word(a));
    }
    drop(m);
    s
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_800344F4(seed: u64, m in prop::array::uniform16(entry()), mirror: bool, settings: u32) {
        let mut s = state(seed, mirror, settings);
        for (k, v) in m.iter().enumerate() {
            s.rdram.mem().write_u32(M + 4 * k as u32, v.to_bits());
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(OUT), sext(M));
        let (lo, hi) = (f32::from_bits(rom_word(0x800A_AAD0)), f32::from_bits(rom_word(0x800A_AAD4)));
        let words: [u32; 16] = std::array::from_fn(|k| fixed(m[k], mirror && k % 4 == 0, lo, hi));
        let after = run("func_800344F4", misc::func_800344F4, &s)?;
        prop_assert_eq!(halves(&after, 32), mtx(&words));
    }

    #[test]
    fn func_80034650(seed: u64, m in prop::array::uniform12(entry()), mirror: bool, settings: u32) {
        let mut s = state(seed, mirror, settings);
        for (k, v) in m.iter().enumerate() {
            s.rdram.mem().write_u32(M + 4 * k as u32, v.to_bits());
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(OUT), sext(M));
        let (lo, hi) = (f32::from_bits(rom_word(0x800A_AAD8)), f32::from_bits(rom_word(0x800A_AADC)));
        let words: [u32; 16] = std::array::from_fn(|k| {
            let (i, j) = (k / 4, k % 4);
            if j == 3 {
                if i == 3 { 0x1_0000 } else { 0 }
            } else {
                fixed(m[3 * i + j], mirror && j == 0, lo, hi)
            }
        });
        let after = run("func_80034650", misc::func_80034650, &s)?;
        prop_assert_eq!(halves(&after, 32), mtx(&words));
    }
}
