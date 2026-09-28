//! The leaves at 0x80009278..0x80009580 (game::misc): the three-entry
//! "recently seen" list (push, search) and test/set/clear on the flag words
//! at 0x800D2140. Recompiled C vs Rust on random register files and memory,
//! each checked against its statement.

// Tests are named after the functions (func_80009278), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc::{self, FLAG_WORDS, RECENT, RECENT_NEXT};
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

fn half(s: &State, a: u32) -> u16 {
    let w = word(s, a & !3);
    if a & 2 == 0 { (w >> 16) as u16 } else { w as u16 }
}

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s
}

/// Values in the list: a few, so matches happen, including both signs.
fn entry() -> impl Strategy<Value = u16> {
    prop_oneof![Just(0u16), Just(1), Just(7), Just(0x7FFF), Just(0x8000), Just(0xFFFF), any::<u16>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// `RECENT[next] = v; next = (next + 1) % 3` (signed remainder), for
    /// in-range slots and the out-of-range ones the QUIRK allows.
    #[test]
    fn func_80009278(seed: u64, next in prop_oneof![0i16..3, Just(3), Just(-1), Just(-2), Just(40), Just(-40)], v: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = v;
        s.rdram.mem().write_u16(RECENT_NEXT, next as u16);
        let after = run("func_80009278", misc::func_80009278, &s)?;
        let slot = (RECENT as i32 + 2 * next as i32) as u32;
        let new_next = ((next as i32 + 1) % 3) as u16;
        if slot != RECENT_NEXT {
            prop_assert_eq!(half(&after, slot), v as u16);
        }
        prop_assert_eq!(half(&after, RECENT_NEXT), new_next);
        prop_assert_eq!(after.ctx.gpr[V0], next as i64 as u64);
    }

    /// 1 if `v` (all 64 bits) equals one of the three halfwords sign-extended.
    #[test]
    fn func_800092B0(seed: u64, list in [entry(), entry(), entry()], v in prop_oneof![
        entry().prop_map(|h| h as i16 as i64 as u64),
        entry().prop_map(u64::from),
        entry().prop_map(|h| (h as i16 as i64 as u64) ^ 0x1_0000_0000),
        any::<u64>(),
    ]) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = v;
        {
            let mut m = s.rdram.mem();
            for (k, h) in list.iter().enumerate() {
                m.write_u16(RECENT + 2 * k as u32, *h);
            }
        }
        let after = run("func_800092B0", misc::func_800092B0, &s)?;
        let found = list.iter().any(|&h| h as i16 as i64 as u64 == v);
        prop_assert_eq!(after.ctx.gpr[V0], found as u64);
    }

    /// Test, set and clear `FLAG_WORDS[k]` with `bits`.
    #[test]
    fn flag_words(seed: u64, op in 0usize..3, k in -8i32..256, old: u32, bits in prop_oneof![Just(0u64), Just(u64::MAX), any::<u64>()]) {
        let (name, port): (&str, RecompFn) = [
            ("func_80009524", misc::func_80009524 as RecompFn),
            ("func_8000953C", misc::func_8000953C),
            ("func_8000955C", misc::func_8000955C),
        ][op];
        let addr = (FLAG_WORDS as i32 + 4 * k) as u32;
        let mut s = state(seed);
        s.ctx.gpr[A0] = k as i64 as u64;
        s.ctx.gpr[A1] = bits;
        s.rdram.mem().write_u32(addr, old);
        let after = run(name, port, &s)?;
        let old64 = old as i32 as i64 as u64;
        match op {
            0 => {
                prop_assert_eq!(after.ctx.gpr[V0], old64 & bits);
                prop_assert_eq!(word(&after, addr), old);
            }
            1 => prop_assert_eq!(word(&after, addr), old | bits as u32),
            _ => prop_assert_eq!(word(&after, addr), old & !(bits as u32)),
        }
    }
}

/// Pushing three values makes each findable; a fourth push evicts the
/// oldest.
#[test]
fn push_then_search() {
    let mut s = state(1);
    s.rdram.mem().write_u16(RECENT_NEXT, 0);
    for v in [10u64, 20, 30, 40] {
        s.ctx.gpr[A0] = v;
        s = compare("func_80009278", misc::func_80009278, &s).unwrap_or_else(|d| panic!("{d}"));
    }
    for (v, want) in [(10u64, 0u64), (20, 1), (30, 1), (40, 1), (50, 0)] {
        s.ctx.gpr[A0] = v;
        s = compare("func_800092B0", misc::func_800092B0, &s).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(s.ctx.gpr[V0], want, "{v}");
    }
}
