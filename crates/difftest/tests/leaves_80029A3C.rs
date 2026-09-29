//! Two stragglers at depth 0: the profile-record reset (game::save,
//! func_80029A3C) and a conditional 16-word copy (game::misc,
//! func_80080D08). Recompiled C vs Rust, each checked against its
//! statement by simulating the stores on a copy of the input and comparing
//! all of RDRAM.

// Tests are named after the functions (func_80029A3C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::recomp::{reg::*, RecompFn};
use game::{misc, save};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(0x803F_0000);
    s
}

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80029A3C(seed: u64, which in prop_oneof![Just(0u32), Just(1u32), any::<u32>()], i in prop_oneof![0u32..4, 0u32..200]) {
        let mut s = state(seed);
        s.randomise_memory(seed, 0x8011_3680, 0x3F0);
        s.randomise_memory(seed ^ 1, 0x8011_3E60, 44 * 200);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(which), u64::from(i));
        let mut want = s.clone();
        let record = match which {
            0 => Some((0x8011_3E60 + 44 * i, 0xFFu8)),
            1 => Some((0x8011_3694 + 44 * i, i as u8)),
            _ => None,
        };
        if let Some((r, b5)) = record {
            let mut m = want.rdram.mem();
            for (off, v) in [(3u32, 0u8), (4, 0), (5, b5), (6, 0), (7, 0xFF), (8, 1), (9, 1), (0xA, 1), (0xB, 0), (0x1C, 1), (0, 0), (1, 0), (2, 0)] {
                m.write_u8(r + off, v);
            }
            m.write_u32(r + 0x18, 400);
            m.write_u32(r + 0x14, 0x22E01);
            for k in 0..4 {
                m.write_u16(r + 0xC + 2 * k, 0);
            }
            for k in 0..7 {
                m.write_u8(r + 0x1D + k, 0);
                m.write_u8(r + 0x24 + k, 0xFF);
            }
            if which == 1 {
                m.write_u8(r + 0x2B, 0);
            }
        }
        let after = run("func_80029A3C", save::func_80029A3C, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80080D08(seed: u64, flag in prop_oneof![Just(0u32), Just(u32::MAX), any::<u32>()], src: [u32; 16]) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(0x800A_6704, flag);
            for (k, w) in src.iter().enumerate() {
                m.write_u32(0x8011_DCB8 + 4 * k as u32, *w);
            }
        }
        s.ctx.gpr[A0] = sext(0x8030_0000);
        let mut want = s.clone();
        if (flag as i32) >= 0 {
            let mut m = want.rdram.mem();
            for (k, w) in src.iter().enumerate() {
                m.write_u32(0x8030_0000 + 4 * k as u32, *w);
            }
        }
        let after = run("func_80080D08", misc::func_80080D08, &s)?;
        prop_assert_eq!(after.ctx.gpr[game::recomp::reg::V0], u64::from((flag as i32) >= 0));
        same_memory(&after, &want)?;
    }
}
