//! The leaves at 0x80000520..0x80000554 (game::util): a setter, a clear, a
//! getter and two empty functions. Recompiled C vs Rust on random register
//! files and random globals, each checked against its one-line statement.

// Tests are named after the functions (func_8000052C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::recomp::{reg::*, RecompFn};
use game::util;
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

/// Edge values mixed with anything, for argument registers (any 64 bits).
fn arg() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        Just(u64::MAX),
        Just(0xFFFF_FFFF_8000_0000),
        Just(0x0000_0000_8000_0000),
        Just(0x1234_5678_0000_0003),
        any::<u32>().prop_map(|v| v as i32 as i64 as u64),
        any::<u64>(),
    ]
}

/// A canonical, word-aligned `sp` inside RDRAM.
fn sp() -> impl Strategy<Value = u64> {
    (0x8000_0000u32..0x807F_FFFC).prop_map(|v| (v & !3) as i32 as i64 as u64)
}

fn state(seed: u64, a0: u64, sp: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[A0] = a0;
    s.ctx.gpr[SP] = sp;
    s
}

const AT_800A: u64 = 0xFFFF_FFFF_800A_0000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// `[0x8009A270] = a0`.
    #[test]
    fn func_80000520(seed: u64, a0 in arg(), sp in sp()) {
        let s = state(seed, a0, sp);
        let after = run("func_80000520", util::func_80000520, &s)?;
        prop_assert_eq!(word(&after, 0x8009_A270), a0 as u32);
        prop_assert_eq!(after.ctx.gpr[AT], AT_800A);
    }

    /// `[0x8009A280] = 0`.
    #[test]
    fn func_8000052C(seed: u64, a0 in arg(), sp in sp()) {
        let s = state(seed, a0, sp);
        let after = run("func_8000052C", util::func_8000052C, &s)?;
        prop_assert_eq!(word(&after, 0x8009_A280), 0);
        prop_assert_eq!(after.ctx.gpr[AT], AT_800A);
    }

    /// `v0 = [0x8009A280]`, sign-extended.
    #[test]
    fn func_80000538(seed: u64, value: i32, sp in sp()) {
        let mut s = state(seed, 0, sp);
        s.rdram.mem().write_u32(0x8009_A280, value as u32);
        let after = run("func_80000538", util::func_80000538, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], value as i64 as u64);
    }

    /// Nothing at all.
    #[test]
    fn func_80000544(seed: u64, a0 in arg(), sp in sp()) {
        let s = state(seed, a0, sp);
        let after = run("func_80000544", util::func_80000544, &s)?;
        prop_assert_eq!(after.ctx.gpr, s.ctx.gpr);
        prop_assert!(after.rdram.as_words() == s.rdram.as_words());
    }

    /// `[sp] = a0`, nothing else.
    #[test]
    fn func_8000054C(seed: u64, a0 in arg(), sp in sp()) {
        let s = state(seed, a0, sp);
        let after = run("func_8000054C", util::func_8000054C, &s)?;
        prop_assert_eq!(word(&after, sp as u32), a0 as u32);
        prop_assert_eq!(after.ctx.gpr, s.ctx.gpr);
    }
}

/// func_80000538 reads the word func_8000052C clears (func_80000520 writes a
/// different one, 0x8009A270).
#[test]
fn clear_then_get() {
    let mut s = state(1, 0, 0xFFFF_FFFF_800A_2800);
    s.rdram.mem().write_u32(0x8009_A280, 0xDEAD_BEEF);
    let s = compare("func_8000052C", util::func_8000052C, &s).unwrap();
    let s = compare("func_80000538", util::func_80000538, &s).unwrap();
    assert_eq!(s.ctx.gpr[V0], 0);
}
