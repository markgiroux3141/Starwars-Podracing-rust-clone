//! The leaves at 0x80018114..0x8001F48C (game::misc, game::save): field setters and
//! getters chosen by a selector, empty functions and two flag reads.
//! Recompiled C vs Rust on random register files and memory, each checked
//! against its statement.

// Tests are named after the functions (func_8001811C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::{misc, save};
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

const O: u32 = 0x8030_0000;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(0x800A_2800);
    s.randomise_memory(seed, O, 0x200);
    s.ctx.gpr[A0] = sext(O);
    s
}

/// Selectors: the four that pick a field, their low words with junk above,
/// and others.
fn selector() -> impl Strategy<Value = u64> {
    prop_oneof![3u64..7, Just(0), Just(2), Just(0x1_0000_0004), Just(u64::MAX), any::<u64>()]
}

fn field(k: u64) -> Option<u32> {
    match k {
        4 => Some(0x15C),
        3 => Some(0x164),
        6 => Some(0x158),
        5 => Some(0x160),
        _ => None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Set the selected field, touch no other.
    #[test]
    fn func_8001811C(seed: u64, k in selector(), v: u64) {
        let mut s = state(seed);
        (s.ctx.gpr[A1], s.ctx.gpr[A2]) = (k, v);
        let after = run("func_8001811C", misc::func_8001811C, &s)?;
        for off in [0x158, 0x15C, 0x160, 0x164] {
            let want = if field(k) == Some(off) { v as u32 } else { word(&s, O + off) };
            prop_assert_eq!(word(&after, O + off), want, "{:#x}", off);
        }
    }

    /// Get the selected field, or -1.
    #[test]
    fn func_80018164(seed: u64, k in selector()) {
        let mut s = state(seed);
        s.ctx.gpr[A1] = k;
        let after = run("func_80018164", misc::func_80018164, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], field(k).map_or(u64::MAX, |off| sext(word(&s, O + off))));
    }

    /// `[o + 8]` for 0, `[o + 4]` for 2, else 0.
    #[test]
    fn func_800182FC(seed: u64, k in selector()) {
        let mut s = state(seed);
        s.ctx.gpr[A1] = k;
        let after = run("func_800182FC", misc::func_800182FC, &s)?;
        let want = match k {
            0 => sext(word(&s, O + 8)),
            2 => sext(word(&s, O + 4)),
            _ => 0,
        };
        prop_assert_eq!(after.ctx.gpr[V0], want);
    }

    /// The plain accessors.
    #[test]
    fn accessors(seed: u64, k in 0usize..3, v: u64) {
        let (name, port, off, set): (&str, RecompFn, u32, bool) = [
            ("func_80018114", misc::func_80018114 as RecompFn, 0x168, true),
            ("func_800183A8", misc::func_800183A8, 4, false),
            ("func_800183B0", misc::func_800183B0, 4, true),
        ][k];
        let mut s = state(seed);
        s.ctx.gpr[A1] = v;
        let after = run(name, port, &s)?;
        if set {
            prop_assert_eq!(word(&after, O + off), v as u32);
        } else {
            prop_assert_eq!(after.ctx.gpr[V0], sext(word(&s, O + off)));
        }
    }

    /// Empty functions change nothing; func_80018450 spills two arguments.
    #[test]
    fn empties(seed: u64, k in 0usize..4, a0: u64, a1: u64) {
        let (name, port): (&str, RecompFn) = [
            ("func_80018440", misc::func_80018440 as RecompFn),
            ("func_80018448", misc::func_80018448),
            ("func_80018460", misc::func_80018460),
            ("func_80018450", misc::func_80018450),
        ][k];
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (a0, a1);
        let after = run(name, port, &s)?;
        prop_assert_eq!(after.ctx.gpr, s.ctx.gpr);
        if k == 3 {
            prop_assert_eq!((word(&after, 0x800A_2800), word(&after, 0x800A_2804)), (a0 as u32, a1 as u32));
        }
    }

    /// Bit 1 of [0x80113688].
    #[test]
    fn func_8001F464(seed: u64, w: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x8011_3688, w);
        let after = run("func_8001F464", save::func_8001F464, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(w >> 1 & 1));
    }
}

#[test]
fn func_80018470() {
    let s = state(1);
    let after = compare("func_80018470", misc::func_80018470, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(word(&after, 0x800A_21AC), 5);
}
