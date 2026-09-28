//! The leaves at 0x8000C530..0x8000EA4C (game::misc): an empty function,
//! the push/pop of the current id, a getter, and the six optional bytes of
//! func_8000E9BC. Recompiled C vs Rust on random register files and memory,
//! each checked against its statement.

// Tests are named after the functions (func_8000C5F0), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc::{self, CURRENT_ID, CURRENT_WORD, ID_DEPTH, ID_STACK, SAVED_WORDS};
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

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(0x800A_2800);
    s
}

fn at(base: u32, k: i32) -> u32 {
    (base as i32 + 4 * k) as u32
}

/// An id stack of `depth` with ids 0..3, the ones SAVED_WORDS has room
/// for (id 3 would be CURRENT_ID itself), so ids repeat.
fn ids(s: &mut State, seed: u64, depth: i32, current: i32) {
    let mut rng = difftest::Rng::new(seed);
    let mut m = s.rdram.mem();
    m.write_u32(ID_DEPTH, depth as u32);
    m.write_u32(CURRENT_ID, current as u32);
    for k in -2..=depth.max(0) + 2 {
        m.write_u32(at(ID_STACK, k), rng.next_u32() % 3);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Empty: nothing changes.
    #[test]
    fn func_8000C530(seed: u64) {
        let s = state(seed);
        let after = run("func_8000C530", misc::func_8000C530, &s)?;
        prop_assert_eq!(after.ctx.gpr, s.ctx.gpr);
    }

    /// Save the current word, push `id`, load its saved word.
    #[test]
    fn func_8000C5F0(seed: u64, depth in 0i32..20, current in 0i32..3, id in 0u64..3) {
        let mut s = state(seed);
        ids(&mut s, seed, depth, current);
        s.ctx.gpr[A0] = id;
        let after = run("func_8000C5F0", misc::func_8000C5F0, &s)?;
        prop_assert_eq!(word(&after, at(SAVED_WORDS, current)), word(&s, CURRENT_WORD));
        prop_assert_eq!(word(&after, ID_DEPTH), (depth + 1) as u32);
        prop_assert_eq!(word(&after, at(ID_STACK, depth + 1)), id as u32);
        prop_assert_eq!(word(&after, CURRENT_ID), id as u32);
        // The saved word of `id`, after the save above (which may be it).
        prop_assert_eq!(word(&after, CURRENT_WORD), word(&after, at(SAVED_WORDS, id as i32)));
    }

    /// Save the current word, pop (not below 0), load the new current's word.
    #[test]
    fn func_8000C658(seed: u64, depth in -2i32..20, current in 0i32..3) {
        let mut s = state(seed);
        ids(&mut s, seed, depth, current);
        let after = run("func_8000C658", misc::func_8000C658, &s)?;
        let new_depth = if depth > 0 { depth - 1 } else { depth };
        let new_id = word(&s, at(ID_STACK, new_depth));
        prop_assert_eq!(word(&after, at(SAVED_WORDS, current)), word(&s, CURRENT_WORD));
        prop_assert_eq!(word(&after, ID_DEPTH), new_depth as u32);
        prop_assert_eq!(word(&after, CURRENT_ID), new_id);
        prop_assert_eq!(word(&after, CURRENT_WORD), word(&after, at(SAVED_WORDS, new_id as i32)));
    }

    /// Ids past the three saved words alias the globals after them (QUIRK):
    /// still C vs Rust, no statement.
    #[test]
    fn push_pop_big_ids(seed: u64, depth in 0i32..8, current in 0i32..12, id in 0u64..12, pop: bool) {
        let mut s = state(seed);
        ids(&mut s, seed, depth, current);
        s.ctx.gpr[A0] = id;
        if pop {
            run("func_8000C658", misc::func_8000C658, &s)?;
        } else {
            run("func_8000C5F0", misc::func_8000C5F0, &s)?;
        }
    }

    /// Returns `[0x8009B7E4]`.
    #[test]
    fn func_8000DA6C(seed: u64, v: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x8009_B7E4, v);
        let after = run("func_8000DA6C", misc::func_8000DA6C, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(v));
    }

    /// Six bytes at `q + 0x20..0x26`, negative values skipped; nothing if
    /// `p` or `q` is null.
    #[test]
    fn func_8000E9BC(seed: u64, null_p: bool, null_q: bool, vals in prop::array::uniform6(prop_oneof![Just(-1i16), Just(0), Just(0x7F), Just(0x1FF), any::<i16>()]), junk: [u16; 6]) {
        let mut s = state(seed);
        let (p, q) = (0x8030_0000u32, 0x8031_0000u32);
        s.ctx.gpr[A0] = if null_p { 0 } else { sext(p) };
        s.rdram.mem().write_u32(p + 0xC, if null_q { 0 } else { q });
        // a1-a3: the value in the low halfword, junk above it.
        for (k, r) in [A1, A2, A3].into_iter().enumerate() {
            s.ctx.gpr[r] = (u64::from(junk[k]) << 16) | u64::from(vals[k] as u16);
        }
        // The stack arguments: words at sp+0x10.., the value in the low halfword.
        for k in 0..3 {
            let w = (u32::from(junk[3 + k]) << 16) | u32::from(vals[3 + k] as u16);
            s.rdram.mem().write_u32(0x800A_2810 + 4 * k as u32, w);
        }
        let after = run("func_8000E9BC", misc::func_8000E9BC, &s)?;
        for k in 0..6 {
            let a = q + 0x20 + k as u32;
            let want = if null_p || null_q || vals[k] < 0 { byte(&s, a) } else { vals[k] as u8 };
            prop_assert_eq!(byte(&after, a), want, "byte {}", k);
        }
    }
}
