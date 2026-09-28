//! The leaves at 0x80011778..0x80011F38 (game::misc): colour and flag
//! setters, a word table and its reset, a pointer table with a clamped
//! selector, and a halfword pair. Recompiled C vs Rust on random register
//! files and memory, each checked against its statement.

// Tests are named after the functions (func_80011778), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc::{self, HALF_PAIR, POINTER_COUNT, POINTER_SELECTED, POINTER_TABLE_6940, WORD_TABLE_6140};
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

fn half(s: &State, a: u32) -> u16 {
    (word(s, a & !3) >> (16 - 8 * (a & 2))) as u16
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

fn four(s: &State, a: u32) -> [u8; 4] {
    word(s, a).to_be_bytes()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Four low bytes to 0x8009B874 (80011778) or 0x800A1CCC (80011F04,
    /// which also clears [0x800D6938]).
    #[test]
    fn colours(seed: u64, which: bool, r: u64, g: u64, b: u64, a: u64) {
        let (name, port, at): (&str, RecompFn, u32) = if which {
            ("func_80011778", misc::func_80011778, 0x8009_B874)
        } else {
            ("func_80011F04", misc::func_80011F04, 0x800A_1CCC)
        };
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (r, g, b, a);
        let after = run(name, port, &s)?;
        prop_assert_eq!(four(&after, at), [r as u8, g as u8, b as u8, a as u8]);
        prop_assert_eq!(word(&after, 0x800A_2800), r as u32);
        prop_assert_eq!(word(&after, 0x800A_280C), a as u32);
        if !which {
            prop_assert_eq!(word(&after, 0x800D_6938), 0);
        }
    }

    /// `WORD_TABLE_6140[k] = v` for any `k` (unbounded, QUIRK).
    #[test]
    fn func_80011824(seed: u64, k in -8i32..128, v: u64) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (k as i64 as u64, v);
        let after = run("func_80011824", misc::func_80011824, &s)?;
        prop_assert_eq!(word(&after, (WORD_TABLE_6140 as i32 + 4 * k) as u32), v as u32);
    }

    /// Clamp the low halfword of `a0` to `0..count` and select.
    #[test]
    fn func_80011E54(seed: u64, k: i16, junk: u64, count in prop_oneof![Just(0i32), 1i32..9, Just(-3), Just(0x1_0005)]) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = (junk & !0xFFFF) | u64::from(k as u16);
        s.rdram.mem().write_u32(POINTER_COUNT, count as u32);
        let after = run("func_80011E54", misc::func_80011E54, &s)?;
        let mut i = i32::from(k);
        if i >= count {
            i = i32::from((count - 1) as i16);
        }
        if i < 0 {
            i = 0;
        }
        prop_assert_eq!(word(&after, POINTER_SELECTED), word(&s, POINTER_TABLE_6940 + 4 * i as u32));
    }

    /// Set the pair, then read it back into two other halfwords.
    #[test]
    fn half_pair(seed: u64, x: u64, y: u64) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (x, y);
        let mut after = run("func_80011ECC", misc::func_80011ECC, &s)?;
        prop_assert_eq!((half(&after, HALF_PAIR), half(&after, HALF_PAIR + 2)), (x as u16, y as u16));
        let (px, py) = (0x8030_0002u32, 0x8030_0010u32);
        (after.ctx.gpr[A0], after.ctx.gpr[A1]) = (sext(px), sext(py));
        let back = run("func_80011EE8", misc::func_80011EE8, &after)?;
        prop_assert_eq!((half(&back, px), half(&back, py)), (x as u16, y as u16));
        prop_assert_eq!(back.ctx.gpr[T7], y as u16 as i16 as i64 as u64);
    }

    /// Flag setters.
    #[test]
    fn flags(seed: u64, k in 0usize..3) {
        let (name, port, at, want): (&str, RecompFn, u32, u32) = [
            ("func_80011918", misc::func_80011918 as RecompFn, 0x8009_B810, 1),
            ("func_80011928", misc::func_80011928, 0x8009_B810, 0),
            ("func_80011814", misc::func_80011814, 0x8009_B870, 1 << 24),
        ][k];
        let s = state(seed);
        let after = run(name, port, &s)?;
        if k == 2 {
            prop_assert_eq!(byte(&after, at), 1);
            prop_assert_eq!(word(&after, at) & 0x00FF_FFFF, word(&s, at) & 0x00FF_FFFF);
        } else {
            prop_assert_eq!(word(&after, at), want);
        }
    }
}

#[test]
fn func_80011838() {
    let s = state(1);
    let after = compare("func_80011838", misc::func_80011838, &s).unwrap_or_else(|d| panic!("{d}"));
    for k in 0..80u32 {
        assert_eq!(word(&after, WORD_TABLE_6140 + 4 * k), u32::MAX);
        assert_eq!(byte(&after, 0x800D_68C0 + k), 0);
    }
    assert_eq!(word(&after, WORD_TABLE_6140 + 320), word(&s, WORD_TABLE_6140 + 320));
    assert_eq!(word(&after, 0x800D_6910), word(&s, 0x800D_6910));
}

#[test]
fn func_80011DF0() {
    let s = state(2);
    let after = compare("func_80011DF0", misc::func_80011DF0, &s).unwrap_or_else(|d| panic!("{d}"));
    let want = [0x800A_1BDC, 0x800A_1B78, 0x800A_1B14, 0x800A_1B78, 0x800A_1C40, 0x800A_1BDC, 0x800A_1AB0];
    for (k, w) in want.iter().enumerate() {
        assert_eq!(word(&after, POINTER_TABLE_6940 + 4 * k as u32), *w);
    }
    assert_eq!(word(&after, POINTER_COUNT), 7);
    assert_eq!(word(&after, POINTER_SELECTED), 0x800A_1BDC);
}
