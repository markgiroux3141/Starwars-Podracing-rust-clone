//! The leaves at 0x8000AEB4..0x8000B254 (game::misc): accessors of the
//! 0x7C-byte ENTRIES and the selection of one of them. Recompiled C vs Rust
//! on random register files and memory, each checked against its
//! statement.

// Tests are named after the functions (func_8000AEB4), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc::{self, ENTRIES, SELECTED};
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

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

/// Where the test puts the array.
const ARRAY: u32 = 0x8030_0000;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(0x800A_2800);
    s.rdram.mem().write_u32(ENTRIES, ARRAY);
    s
}

fn entry(id: i32) -> u32 {
    (ARRAY as i32 + 0x7C * id) as u32
}

fn with_junk(id: i16, junk: u64) -> u64 {
    (junk & !0xFFFF) | u64::from(id as u16)
}

fn id() -> impl Strategy<Value = i16> {
    prop_oneof![-4i16..0, 0i16..64]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Flags of entry `k` (the whole low word, not a halfword), and `|=`.
    #[test]
    fn entry_flags(seed: u64, set: bool, k in -4i32..64, junk: u32, bits: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = (u64::from(junk) << 32) | u64::from(k as u32);
        s.ctx.gpr[A1] = bits;
        if set {
            let after = run("func_8000AED4", misc::func_8000AED4, &s)?;
            prop_assert_eq!(word(&after, entry(k)), word(&s, entry(k)) | bits as u32);
        } else {
            let after = run("func_8000AEB4", misc::func_8000AEB4, &s)?;
            prop_assert_eq!(after.ctx.gpr[V0], sext(word(&s, entry(k))));
        }
    }

    /// `+6 = y`, `+4 = x` (halfwords), `+8 = w`.
    #[test]
    fn func_8000AEFC(seed: u64, id in id(), junk: u64, x: u64, w: u64, y: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (x, w, y);
        let after = run("func_8000AEFC", misc::func_8000AEFC, &s)?;
        let e = entry(id.into());
        prop_assert_eq!(half(&after, e + 4), x as u16);
        prop_assert_eq!(half(&after, e + 6), y as u16);
        prop_assert_eq!(word(&after, e + 8), w as u32);
    }

    /// `+0xC = h` (halfword), `+0x10 = w`.
    #[test]
    fn func_8000B02C(seed: u64, id in id(), junk: u64, w: u64, h: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        (s.ctx.gpr[A1], s.ctx.gpr[A2]) = (w, h);
        let after = run("func_8000B02C", misc::func_8000B02C, &s)?;
        let e = entry(id.into());
        prop_assert_eq!(half(&after, e + 0xC), h as u16);
        prop_assert_eq!(word(&after, e + 0x10), w as u32);
    }

    /// `+0x78 = v`.
    #[test]
    fn func_8000B06C(seed: u64, id in id(), junk: u64, v: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        s.ctx.gpr[A1] = v;
        let after = run("func_8000B06C", misc::func_8000B06C, &s)?;
        prop_assert_eq!(word(&after, entry(id.into()) + 0x78), v as u32);
    }

    /// Bit 0 of the flags.
    #[test]
    fn func_8000B098(seed: u64, id in id(), junk: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        let after = run("func_8000B098", misc::func_8000B098, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(word(&s, entry(id.into())) & 1));
    }

    /// Deselect the old entry, select the new one.
    #[test]
    fn func_8000B1B0(seed: u64, old in prop_oneof![Just(-1i32), 0i32..32], id in prop_oneof![Just(-1i16), 0i16..32], junk: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        s.rdram.mem().write_u32(SELECTED, old as u32);
        let after = run("func_8000B1B0", misc::func_8000B1B0, &s)?;
        prop_assert_eq!(word(&after, SELECTED), id as i32 as u32);
        let flags = |k: i32| word(&s, entry(k));
        let mut want = std::collections::BTreeMap::new();
        if old != -1 {
            want.insert(old, flags(old) & !1);
        }
        if id != -1 {
            let k = i32::from(id);
            want.insert(k, want.get(&k).copied().unwrap_or_else(|| flags(k)) | 1);
            prop_assert_eq!(word(&after, 0x8009_B794), entry(k));
            prop_assert_eq!(word(&after, 0x8009_B79C), 0);
        } else {
            prop_assert_eq!(word(&after, 0x8009_B794), word(&s, 0x8009_B794));
        }
        for (k, f) in want {
            prop_assert_eq!(word(&after, entry(k)), f, "entry {}", k);
        }
    }
}
