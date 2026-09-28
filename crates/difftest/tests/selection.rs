//! Two of the larger depth-0 functions (game::misc): the track selection
//! state (func_80024704) and the racer list (func_800281F0). Recompiled C
//! vs Rust on random save data, each checked against its statement.

// Tests are named after the functions (func_80024704), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

fn byte(s: &State, a: u32) -> u8 {
    (word(s, a & !3) >> (24 - 8 * (a & 3))) as u8
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const P: u32 = 0x8030_0000;
const W: u32 = 0x8011_A240;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed, P, 0x100);
    s.randomise_memory(seed ^ 1, 0x8011_3680, 0x40);
    s.randomise_memory(seed ^ 2, 0x8011_3E60, 0x40);
    s.randomise_memory(seed ^ 3, W, 0x40);
    s.ctx.gpr[SP] = sext(0x800A_2800);
    s.ctx.gpr[A0] = sext(P);
    s
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80024704(seed: u64, current: bool, flag in prop_oneof![Just(0u8), any::<u8>()], c in -1i8..5, n in prop_oneof![0u8..9, Just(0u8)], bits: u8, sel in -2i32..10) {
        let mut s = state(seed);
        let save = if current { 0x8011_3E60 } else { 0x8011_3680 };
        let (flag_at, bits_at) = if current { (0xB, 8) } else { (0xF, 0xC) };
        let mut m = s.rdram.mem();
        m.write_u8(P + 0x6C, u8::from(current));
        m.write_u8(P + 0x5E, c as u8);
        m.write_u8(save + flag_at, flag);
        m.write_u8((0x800A_21B4i64 + i64::from(c)) as u32, n);
        m.write_u8((i64::from(save) + bits_at + i64::from(c)) as u32, bits);
        m.write_u32(W + 0x30, sel as u32);
        drop(m);
        let after = run("func_80024704", misc::func_80024704, &s)?;
        // Read back: for c = 3 (profile 0x80113680) the circuit's bits byte
        // is the flag byte itself.
        let flag = byte(&s, save + flag_at);
        let bits = byte(&s, (i64::from(save) + bits_at + i64::from(c)) as u32);
        let n = byte(&s, (0x800A_21B4i64 + i64::from(c)) as u32);
        let circuits = if flag == 0 { 2 } else { 3 };
        let tracks = (0..u32::from(n)).filter(|&i| i < 8 && bits >> i & 1 != 0).count() as i32;
        prop_assert_eq!(word(&after, W + 0x28), circuits as u32);
        prop_assert_eq!(word(&after, W + 0x2C), tracks as u32);
        prop_assert_eq!(word(&after, W + 0x30), sel.min(tracks - 1) as u32);
        prop_assert_eq!(word(&after, W + 0x20), u32::from(c > 0));
        prop_assert_eq!(word(&after, W + 0x24), u32::from(i32::from(c) < circuits));
    }

    #[test]
    fn func_800281F0(seed: u64, profile in 0i8..4, mask in prop_oneof![Just(0u32), Just(0x7F_FFFFu32), any::<u32>()]) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 4, 0x800D_6CD0, 0xC8);
        s.rdram.mem().write_u8(P + 0x6F, profile as u8);
        s.rdram.mem().write_u32(0x8011_3E74 + 0x2C * profile as u32, mask);
        let after = run("func_800281F0", misc::func_800281F0, &s)?;
        let all = mask | 0x2_2E01;
        let racers: Vec<u32> = (0..23).filter(|i| all >> i & 1 != 0).collect();
        for k in 0..23u32 {
            let e = 0x800D_6CD8 + 8 * k;
            let id = racers.get(k as usize).copied().unwrap_or(u32::MAX);
            prop_assert_eq!((word(&after, e), byte(&after, e + 4), byte(&after, e + 5)), (id, 0xFF, 0), "entry {}", k);
            prop_assert_eq!(word(&after, e + 4) & 0xFFFF, word(&s, e + 4) & 0xFFFF);
        }
        prop_assert_eq!(word(&after, 0x8011_A26C), racers.len() as u32);
    }
}
