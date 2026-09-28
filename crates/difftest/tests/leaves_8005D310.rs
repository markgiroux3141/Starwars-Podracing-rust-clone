//! The leaves at 0x8005D310..0x8007F23C (game::misc): setters, getters,
//! spills, a flag copy, a six-word summary, an uninitialised stack read
//! (QUIRK) and a list lookup that can run one past (QUIRK). Recompiled C vs
//! Rust on random register files and memory, each checked against its
//! statement.

// Tests are named after the functions (func_8005D310), capitals included.
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

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const O: u32 = 0x8030_0000;
const SP0: u32 = 0x800A_2800;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed, O, 0x400);
    s.ctx.gpr[SP] = sext(SP0);
    s
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Fixed-address setters and the getter.
    #[test]
    fn fixed(seed: u64, k in 0usize..4, a: u64, b: u64) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (a, b);
        let (name, port): (&str, RecompFn) = [
            ("func_8005D310", misc::func_8005D310 as RecompFn),
            ("func_8005EEFC", misc::func_8005EEFC),
            ("func_80065804", misc::func_80065804),
            ("func_8007B41C", misc::func_8007B41C),
        ][k];
        let after = run(name, port, &s)?;
        match k {
            0 => prop_assert_eq!((word(&after, 0x800A_59FC), word(&after, 0x800A_5A00)), (a as u32, b as u32)),
            1 => prop_assert_eq!(after.ctx.gpr[V0], sext(word(&s, 0x8011_AC8C))),
            2 => prop_assert_eq!(word(&after, 0x8011_C840), a as u32),
            _ => prop_assert_eq!(word(&after, 0x8011_C8E0), a as u32),
        }
    }

    /// p-relative setters, skipped for p == 0.
    #[test]
    fn pointer_setters(seed: u64, null: bool, v: u64, which: bool) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (if null { 0 } else { sext(O) }, v);
        if which {
            let after = run("func_8005F31C", misc::func_8005F31C, &s)?;
            if !null {
                prop_assert_eq!(word(&after, O + 0x18), v as u32);
                prop_assert_eq!(word(&after, O + 0x14), word(&s, O + 0x14).wrapping_add(1));
            }
        } else {
            let after = run("func_80065CB0", misc::func_80065CB0, &s)?;
            if !null {
                prop_assert_eq!(word(&after, O + 0xF0), v as u32);
            }
        }
    }

    /// The spills and stubs.
    #[test]
    fn spills(seed: u64, k in 0usize..7, a0: u64) {
        let (name, port): (&str, RecompFn) = [
            ("func_80062C78", misc::func_80062C78 as RecompFn),
            ("func_8006506C", misc::func_8006506C),
            ("func_8007AFE0", misc::func_8007AFE0),
            ("func_8007C3BC", misc::func_8007C3BC),
            ("func_80073708", misc::func_80073708),
            ("func_8007F22C", misc::func_8007F22C),
            ("func_8007F23C", misc::func_8007F23C),
        ][k];
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (a0, sext(O + 0x20));
        let after = run(name, port, &s)?;
        prop_assert_eq!(word(&after, SP0), a0 as u32);
        if k >= 4 {
            prop_assert_eq!(after.ctx.gpr[V0], 0);
        }
        if k >= 5 {
            prop_assert_eq!(word(&after, O + 0x20), 0);
        }
    }

    /// Bit 12 of [p + 0x60] by bit 13 of the table word.
    #[test]
    fn func_8006C6D0(seed: u64, i in 0u64..8, t: u32, flags: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x800D_76F0 + 4 * i as u32, t);
        s.rdram.mem().write_u32(O + 0x60, flags);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(O), i);
        let after = run("func_8006C6D0", misc::func_8006C6D0, &s)?;
        prop_assert_eq!(word(&after, O + 0x60), if t & 0x2000 != 0 { flags | 0x1000 } else { flags });
    }

    /// Bit 0 if any of w0..w2 has bit 2 or 4, bit 1 if any of w3..w5 has.
    #[test]
    fn func_8006C708(seed: u64, w in proptest::array::uniform6(prop_oneof![Just(0u32), Just(4u32), Just(0x10u32), any::<u32>().prop_map(|v| v & !0x14)])) {
        let mut s = state(seed);
        for (k, &v) in w.iter().enumerate() {
            s.rdram.mem().write_u32(O + 0x2A0 + 4 * k as u32, v);
        }
        s.ctx.gpr[A0] = sext(O);
        let after = run("func_8006C708", misc::func_8006C708, &s)?;
        let any = |r: std::ops::Range<usize>| w[r].iter().any(|v| v & 0x14 != 0);
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(any(0..3)) | (u64::from(any(3..6)) << 1));
    }

    /// QUIRK: whether the unwritten frame word [sp - 8] is 2.
    #[test]
    fn func_8006E008(seed: u64, frame in prop_oneof![Just(2u32), any::<u32>()], a0: u64) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(SP0 - 8, frame);
        s.ctx.gpr[A0] = a0;
        let after = run("func_8006E008", misc::func_8006E008, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(frame == 2));
        prop_assert_eq!(word(&after, SP0), a0 as u32);
    }

    /// The flags word.
    #[test]
    fn func_8006FED0(seed: u64, flags: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(O + 0x60, flags);
        s.ctx.gpr[A0] = sext(O);
        let after = run("func_8006FED0", misc::func_8006FED0, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(flags));
    }

    /// The record for x, or 0; one past the list reads record 0's first word.
    #[test]
    fn func_8007BA9C(seed: u64, len in 0usize..51, x in prop_oneof![0u32..60, Just(0u32)], first: u32, pick: u32) {
        let mut s = state(seed);
        let list = 0x8011_CA58;
        for k in 0..50 {
            let v = if k < len { 1 + k as u32 } else { 0 };
            s.rdram.mem().write_u32(list + 4 * k as u32, v);
        }
        s.rdram.mem().write_u32(0x8011_CB20, first);
        s.ctx.gpr[A0] = u64::from(if pick % 4 == 0 { x } else { 1 + pick % (len as u32 + 1) });
        let x = s.ctx.gpr[A0] as u32;
        let after = run("func_8007BA9C", misc::func_8007BA9C, &s)?;
        let entry = |i: usize| word(&s, list + 4 * i as u32);
        let i = (0..50).find(|&i| entry(i) == 0 || entry(i) == x).unwrap_or(50);
        let want = if entry(i) != 0 { sext(0x8011_CB20 + 0x58 * i as u32) } else { 0 };
        prop_assert_eq!(after.ctx.gpr[V0], want);
    }
}

#[test]
fn func_800735B4() {
    let s = state(1);
    let after = compare("func_800735B4", misc::func_800735B4, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(after.ctx.gpr, s.ctx.gpr);
}

/// QUIRK: with all 50 entries in use and no match, "entry 50" is record 0's
/// first word, and a nonzero one gives record 50.
#[test]
fn func_8007BA9C_one_past() {
    for first in [0u32, 7] {
        let mut s = state(2);
        for k in 0..50u32 {
            s.rdram.mem().write_u32(0x8011_CA58 + 4 * k, 100 + k);
        }
        s.rdram.mem().write_u32(0x8011_CB20, first);
        s.ctx.gpr[A0] = 5;
        let after = compare("func_8007BA9C", misc::func_8007BA9C, &s).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(after.ctx.gpr[V0], if first != 0 { sext(0x8011_CB20 + 0x58 * 50) } else { 0 });
    }
}
