//! The leaves at 0x80045634..0x800520C8 (game::misc): a list replace, the
//! player objects' flags, two mask arrays, an index-list fill (unbounded,
//! QUIRK), a slot choice, a record init, a table swap and two lookups in
//! four words. Recompiled C vs Rust on random register files and memory,
//! each checked against its statement.

// Tests are named after the functions (func_80045634), capitals included.
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

fn half(s: &State, a: u32) -> u16 {
    (word(s, a & !3) >> (16 - 8 * (a & 2))) as u16
}

fn byte(s: &State, a: u32) -> u8 {
    (word(s, a & !3) >> (24 - 8 * (a & 3))) as u8
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const O: u32 = 0x8030_0000;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed, O, 0x1000);
    s.ctx.gpr[SP] = sext(0x800A_2800);
    s
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Replace every `old` in the list with `new`.
    #[test]
    fn func_80045634(seed: u64, words in proptest::collection::vec(0u32..4, 0..10), old in 0u32..4, new in 1u32..8, null in 0usize..4) {
        let mut s = state(seed);
        let list = O + 0x100;
        s.rdram.mem().write_u32(O + 0x14, words.len() as u32);
        s.rdram.mem().write_u32(O + 0x18, list);
        for (k, &w) in words.iter().enumerate() {
            s.rdram.mem().write_u32(list + 4 * k as u32, w);
        }
        let args = [sext(O), u64::from(old), u64::from(new)];
        for r in 0..3 {
            s.ctx.gpr[A0 + r] = if r == null { 0 } else { args[r] };
        }
        let after = run("func_80045634", misc::func_80045634, &s)?;
        let active = null == 3 && old != 0;
        for (k, &w) in words.iter().enumerate() {
            prop_assert_eq!(word(&after, list + 4 * k as u32), if active && w == old { new } else { w });
        }
    }

    /// Each live player's object: +0xE = 0 for the current one, else 1; bit 2 of +0x10.
    #[test]
    fn func_80047920(seed: u64, n in -1i32..6, live in proptest::collection::vec(any::<bool>(), 6), current in -1i32..6) {
        let mut s = state(seed);
        let mut m = s.rdram.mem();
        m.write_u32(0x8011_A26C, n as u32);
        m.write_u32(0x800A_4BE8, current as u32);
        for k in 0..6u32 {
            let slot = O + 0x200 + 4 * k;
            m.write_u32(0x8011_A740 + 4 * k, slot);
            m.write_u32(slot, if live[k as usize] { O + 0x400 + 0x20 * k } else { 0 });
        }
        drop(m);
        let after = run("func_80047920", misc::func_80047920, &s)?;
        for k in 0..6u32 {
            let o = O + 0x400 + 0x20 * k;
            if live[k as usize] && (k as i32) < n {
                prop_assert_eq!(half(&after, o + 0xE), u16::from(k as i32 != current));
                prop_assert_eq!(word(&after, o + 0x10), word(&s, o + 0x10) | 4);
            } else {
                prop_assert_eq!((half(&after, o + 0xE), word(&after, o + 0x10)), (half(&s, o + 0xE), word(&s, o + 0x10)));
            }
        }
    }

    /// Clear, or set and record the new bits.
    #[test]
    fn func_8004E488(seed: u64, i in 0u64..4, on in prop_oneof![Just(0u64), any::<u64>()], mask in prop_oneof![any::<u32>().prop_map(sext), (0u32..8).prop_map(|b| 1u64 << b)], cur: u32, seen: u32) {
        let mut s = state(seed);
        let (a, b) = (0x800A_4B94 + 4 * i as u32, 0x800A_4BA4 + 4 * i as u32);
        s.rdram.mem().write_u32(a, cur);
        s.rdram.mem().write_u32(b, seen);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (i, on, mask);
        let after = run("func_8004E488", misc::func_8004E488, &s)?;
        let mk = mask as u32;
        if on == 0 {
            prop_assert_eq!((word(&after, a), word(&after, b)), (cur & !mk, seen));
        } else {
            prop_assert_eq!(word(&after, a), cur | mk);
            prop_assert_eq!(word(&after, b), if cur & mk == 0 { seen | mk } else { seen });
        }
    }

    /// -1 in the four words, then word i = i for i < (s8) [p + 0x70].
    #[test]
    fn func_8004F6E8(seed: u64, n in prop_oneof![-2i8..9, Just(i8::MAX)]) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 1, 0x800A_4B70, 0x220);
        s.rdram.mem().write_u8(O + 0x70, n as u8);
        s.ctx.gpr[A0] = sext(O);
        let after = run("func_8004F6E8", misc::func_8004F6E8, &s)?;
        for k in 0..130u32 {
            let a = 0x800A_4B7C + 4 * k;
            let want = if (k as i32) < i32::from(n) { k } else if k < 4 { u32::MAX } else { word(&s, a) };
            prop_assert_eq!(word(&after, a), want, "word {}", k);
        }
    }

    /// Four words = -1.
    #[test]
    fn func_8004FF7C(seed: u64) {
        let s = state(seed);
        let after = run("func_8004FF7C", misc::func_8004FF7C, &s)?;
        prop_assert_eq!([0x800A_4B6C, 0x800A_4B70, 0x800A_4B74, 0x800A_4B78].map(|a| word(&after, a)), [u32::MAX; 4]);
    }

    /// Swap two table words.
    #[test]
    fn func_80051994(seed: u64, a in 0u64..16, b in 0u64..16) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 2, 0x8011_A508, 0x40);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (a, b);
        let after = run("func_80051994", misc::func_80051994, &s)?;
        let t = |k: u64| 0x8011_A508 + 4 * k as u32;
        prop_assert_eq!((word(&after, t(a)), word(&after, t(b))), (word(&s, t(b)), word(&s, t(a))));
    }

    /// The first zero of four, and the index of a value among them.
    #[test]
    fn four_words(seed: u64, w in proptest::array::uniform4(prop_oneof![Just(0u32), 1u32..4, any::<u32>()]), x in prop_oneof![0u64..4, Just(0x1_0000_0001u64)], find: bool) {
        let mut s = state(seed);
        for k in 0..4 {
            s.rdram.mem().write_u32(0x8011_B1BC + 4 * k as u32, w[k]);
        }
        s.ctx.gpr[A0] = x;
        if find {
            let after = run("func_800520C8", misc::func_800520C8, &s)?;
            prop_assert_eq!(after.ctx.gpr[V0], w.iter().position(|&v| sext(v) == x).map_or(u64::MAX, |k| k as u64));
        } else {
            let after = run("func_80051FF4", misc::func_80051FF4, &s)?;
            prop_assert_eq!(after.ctx.gpr[V0], w.iter().position(|&v| v == 0).unwrap_or(4) as u64);
        }
    }

    /// The slot choice.
    #[test]
    fn func_8004FE30(seed: u64, mode in prop_oneof![Just(0u32), any::<u32>()], n in -1i8..8, sel in 0i8..3, key in -2i8..3, bytes in proptest::collection::vec(-2i8..3, 8), tags in proptest::collection::vec(any::<bool>(), 8)) {
        let mut s = state(seed);
        let mut m = s.rdram.mem();
        m.write_u32(O + 0x64, mode);
        m.write_u8(O + 0x71, n as u8);
        m.write_u8(O + 0x5D, sel as u8);
        m.write_u8(0x800A_21C2 + 12 * sel as u32, key as u8);
        for k in 0..8u32 {
            m.write_u8(O + 0x72 + k, bytes[k as usize] as u8);
            m.write_u32(0x8011_8F90 + 0x88 * k + 4, if tags[k as usize] { 0x4141_4949 } else { 0x4141_4948 });
        }
        drop(m);
        s.ctx.gpr[A0] = sext(O);
        let after = run("func_8004FE30", misc::func_8004FE30, &s)?;
        let range = 0..i64::from(n.max(0));
        let matches = |i: &i64| bytes[*i as usize] == key;
        let want = if mode != 0 {
            range.clone().filter(matches).last().unwrap_or(0)
        } else {
            range.clone().filter(|i| matches(i) && tags[*i as usize]).last()
                .or_else(|| range.clone().find(|i| tags[*i as usize]))
                .unwrap_or(-1)
        };
        prop_assert_eq!(after.ctx.gpr[V0], want as u64);
    }
}

#[test]
fn func_8004DFEC() {
    let mut s = state(5);
    s.randomise_memory(6, 0x800A_4B90, 0x20);
    let after = compare("func_8004DFEC", misc::func_8004DFEC, &s).unwrap_or_else(|d| panic!("{d}"));
    for k in 0..4 {
        assert_eq!((word(&after, 0x800A_4B94 + 4 * k), word(&after, 0x800A_4BA4 + 4 * k)), (u32::MAX, 0));
    }
}

/// The record's defaults and `+0x72 + i = i`.
#[test]
fn func_80050208() {
    let mut s = state(7);
    s.ctx.gpr[A0] = sext(O);
    let after = compare("func_80050208", misc::func_80050208, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!((word(&after, O + 0x64), word(&after, O + 0x68)), (0, u32::MAX));
    let bytes = [(0x6C, 1), (0x6D, 0), (0x6E, 0), (0x6F, 0), (0x70, 1), (0x71, 12), (0x8E, 3), (0x8F, 2), (0x90, 2)];
    for (off, v) in bytes {
        assert_eq!(byte(&after, O + off), v, "+{off:#x}");
    }
    for i in 0..23 {
        assert_eq!(byte(&after, O + 0x72 + i), i as u8, "entry {i}");
    }
    assert_eq!(byte(&after, O + 0x89), byte(&s, O + 0x89));
}
