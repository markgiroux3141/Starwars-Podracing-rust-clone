//! The leaves at 0x8000FCA4..0x80010080 (game::misc): a byte setter, three
//! table resets, two bounded setters and the handle decoder. Recompiled C vs
//! Rust on random register files and memory, each checked against its
//! statement.

// Tests are named after the functions (func_8000FCA4), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc::{self, HANDLE_SEGMENTS};
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

/// Every word in `[from, to)` equals `v`; the words just outside don't change.
fn filled(before: &State, after: &State, from: u32, to: u32, v: u32) {
    for a in (from..to).step_by(4) {
        assert_eq!(word(after, a), v, "{a:#x}");
    }
    assert_eq!(word(after, from - 4), word(before, from - 4));
    assert_eq!(word(after, to), word(before, to));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Byte 3 of word `k` at 0x8009B824 = `b`.
    #[test]
    fn func_8000FCA4(seed: u64, k in -4i32..32, b: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = k as i64 as u64;
        s.ctx.gpr[A1] = b;
        let after = run("func_8000FCA4", misc::func_8000FCA4, &s)?;
        let a = (0x8009_B827i64 + 4 * k as i64) as u32;
        prop_assert_eq!(byte(&after, a), b as u8);
        prop_assert_eq!(word(&after, 0x800A_2804), b as u32);
    }

    /// Clear byte `k` for `k < 40`: a signed 64-bit compare, but the address
    /// uses the low word (QUIRK), so `i64::MIN` clears byte 0.
    #[test]
    fn func_8000FFF8(seed: u64, k in prop_oneof![-8i64..48, Just(i64::MIN), Just(i64::MIN + 3), Just(0x1_0000_0000)]) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = k as u64;
        let after = run("func_8000FFF8", misc::func_8000FFF8, &s)?;
        let target = 0x800D_5C38u32.wrapping_add(k as u32);
        for a in 0x800D_5C30..0x800D_5C68u32 {
            let hit = k < 40 && a == target;
            prop_assert_eq!(byte(&after, a), if hit { 0 } else { byte(&s, a) }, "{:#x}", a);
        }
    }

    /// For `k < 10` (signed): `[0x800D5F80 + 4k] = a`, `[0x800D5FA8 + 4k] = b`.
    #[test]
    fn func_80010014(seed: u64, k in prop_oneof![-4i64..14, Just(0x1_0000_0002), Just(i64::MIN + 1)], a: u64, b: u64) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (k as u64, a, b);
        let after = run("func_80010014", misc::func_80010014, &s)?;
        let lo = (k as u32).wrapping_mul(4); // the address uses the low word
        let (x, y) = (0x800D_5F80u32.wrapping_add(lo), 0x800D_5FA8u32.wrapping_add(lo));
        if k < 10 {
            prop_assert_eq!(word(&after, y), b as u32);
            if x != y {
                prop_assert_eq!(word(&after, x), a as u32);
            }
        } else {
            prop_assert_eq!(word(&after, x), word(&s, x));
            prop_assert_eq!(word(&after, y), word(&s, y));
        }
    }

    /// `base + (i << shift)` from the segment table.
    #[test]
    fn func_8001004C(seed: u64, h: u64, shift in prop_oneof![0u32..8, Just(31), Just(32), any::<u32>()], base: u32) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = h;
        let seg = ((h & 0xE000) >> 13) as u32;
        s.rdram.mem().write_u32(HANDLE_SEGMENTS + 8 * seg, shift);
        s.rdram.mem().write_u32(HANDLE_SEGMENTS + 8 * seg + 4, base);
        let after = run("func_8001004C", misc::func_8001004C, &s)?;
        let i = ((h & 0x1FFC) >> 2) as u32;
        prop_assert_eq!(after.ctx.gpr[V0], sext(base.wrapping_add(i << (shift & 31))));
    }
}

#[test]
fn func_8000FE1C() {
    let s = state(1);
    let after = compare("func_8000FE1C", misc::func_8000FE1C, &s).unwrap_or_else(|d| panic!("{d}"));
    filled(&s, &after, 0x8009_B814, 0x8009_B81C, u32::MAX);
    filled(&s, &after, 0x8009_B82C, 0x8009_B86C, u32::MAX);
    // The words between the blocks are untouched.
    for a in (0x8009_B81C..0x8009_B82C).step_by(4) {
        assert_eq!(word(&after, a), word(&s, a));
    }
}

#[test]
fn func_8000FE78() {
    let s = state(2);
    let after = compare("func_8000FE78", misc::func_8000FE78, &s).unwrap_or_else(|d| panic!("{d}"));
    filled(&s, &after, 0x800D_5AA8, 0x800D_5AF8, -9999i32 as u32);
}

#[test]
fn func_8000FEF0() {
    let s = state(3);
    let after = compare("func_8000FEF0", misc::func_8000FEF0, &s).unwrap_or_else(|d| panic!("{d}"));
    filled(&s, &after, 0x800D_5F80, 0x800D_5FD0, u32::MAX);
    filled(&s, &after, 0x800D_5C38, 0x800D_5C60, 0);
}

#[test]
fn func_80010040() {
    let s = state(4);
    let after = compare("func_80010040", misc::func_80010040, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(word(&after, 0x8009_B86C), 0);
}
