//! The leaves at 0x80012B5C..0x80017F64 (game::misc): a record lookup with
//! an uninitialised-stack QUIRK, a gDPPipeSync append, and a run of struct
//! field accessors at 0x80017D48. Recompiled C vs Rust on random register
//! files and memory, each checked against its statement.

// Tests are named after the functions (func_80012B5C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc::{self, DL_HEAD, RECORDS_170};
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

const STACK: u32 = 0x800A_2800;
const O: u32 = 0x8030_0000;
const TABLE: u32 = 0x8031_0000;
const PA: u32 = 0x8032_0000;
const PB: u32 = 0x8032_0010;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(STACK);
    s
}


proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A hit copies the record's halfwords at +2/+0xE; a miss copies the two
    /// stack words below `sp` that it never wrote (QUIRK).
    #[test]
    fn func_80012B5C(seed: u64, near: bool, delta in -2i32..12, any_c: u8, junk: u64, first: u8, span in 0u8..8,
                     no_table: bool, a_null: bool, b_null: bool) {
        let mut s = state(seed);
        let last = first.saturating_add(span);
        // Mostly around the range, so both edges are hit; junk above the byte.
        let ch = if near { (i32::from(first) + delta).clamp(0, 255) as u8 } else { any_c };
        let c = (junk & !0xFF) | u64::from(ch);
        {
            let mut m = s.rdram.mem();
            m.write_u32(O + 0x5C, if no_table { 0 } else { TABLE });
            m.write_u8(O + 0x5A, first);
            m.write_u8(O + 0x5B, last);
        }
        s.randomise_memory(seed, TABLE, 0x1000);
        s.ctx.gpr[A0] = c;
        s.ctx.gpr[A1] = sext(O);
        s.ctx.gpr[A2] = if a_null { 0 } else { sext(PA) };
        s.ctx.gpr[A3] = if b_null { 0 } else { sext(PB) };
        let after = run("func_80012B5C", misc::func_80012B5C, &s)?;
        let hit = !no_table && ch >= first && ch <= last;
        let (a, b) = if hit {
            let rec = TABLE + 16 * u32::from(ch - first);
            let h = |at: u32| (word(&s, at & !3) >> (16 - 8 * (at & 2))) as u16 as i16 as i32 as u32;
            (h(rec + 2), h(rec + 0xE))
        } else {
            (word(&s, STACK - 4), word(&s, STACK - 8))
        };
        if !a_null {
            prop_assert_eq!(word(&after, PA), a);
        }
        if !b_null {
            prop_assert_eq!(word(&after, PB), b);
        }
        prop_assert_eq!(word(&after, STACK), c as u32);
    }

    /// Append gDPPipeSync and advance the pointer.
    #[test]
    fn func_80014C98(seed: u64, at in (0x8030_0000u32..0x8040_0000).prop_map(|v| v & !7)) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(DL_HEAD, at);
        let after = run("func_80014C98", misc::func_80014C98, &s)?;
        prop_assert_eq!((word(&after, at), word(&after, at + 4)), (0xE700_0000, 0));
        prop_assert_eq!(word(&after, DL_HEAD), at + 8);
    }

    /// The plain getters and setters: `[o + off]`.
    #[test]
    fn accessors(seed: u64, k in 0usize..13, v: u64) {
        // (name, port, offset, kind: 0 word get, 1 half get, 2 word set)
        let (name, port, off, kind): (&str, RecompFn, u32, u8) = [
            ("func_80017D48", misc::func_80017D48 as RecompFn, 0x1C, 2),
            ("func_80017D50", misc::func_80017D50, 0x1C, 0),
            ("func_80017DA4", misc::func_80017DA4, 0, 0),
            ("func_80017DAC", misc::func_80017DAC, 0x14, 0),
            ("func_80017DDC", misc::func_80017DDC, 0x20, 1),
            ("func_80017DE4", misc::func_80017DE4, 0x22, 1),
            ("func_80017DEC", misc::func_80017DEC, 0x24, 0),
            ("func_80017E54", misc::func_80017E54, 0x14, 0),
            ("func_80017EDC", misc::func_80017EDC, 0, 0),
            ("func_80017EE4", misc::func_80017EE4, 4, 0),
            ("func_80017EEC", misc::func_80017EEC, 4, 2),
            ("func_80017EF4", misc::func_80017EF4, 0, 0),
            ("func_80017F20", misc::func_80017F20, 0, 3),
        ][k];
        let mut s = state(seed);
        s.randomise_memory(seed, O, 0x40);
        s.ctx.gpr[A0] = sext(O);
        s.ctx.gpr[A1] = v;
        let after = run(name, port, &s)?;
        let w = word(&s, O + (off & !3));
        match kind {
            0 => prop_assert_eq!(after.ctx.gpr[V0], sext(w)),
            1 => prop_assert_eq!(after.ctx.gpr[V0], (w >> (16 - 8 * (off & 2))) as u16 as i16 as i64 as u64),
            2 => prop_assert_eq!(word(&after, O + off), v as u32),
            _ => prop_assert_eq!(after.ctx.gpr[V0], 4),
        }
    }

    /// Word `k` of `[o + 0x18]`: with (80017DB4) and without (80017E5C) a
    /// null check.
    #[test]
    fn array_getters(seed: u64, checked: bool, null: bool, k in -4i32..64) {
        let mut s = state(seed);
        let arr = 0x8031_0100u32;
        s.rdram.mem().write_u32(O + 0x18, arr);
        s.ctx.gpr[A0] = if checked && null { 0 } else { sext(O) };
        s.ctx.gpr[A1] = k as i64 as u64;
        let (name, port): (&str, RecompFn) =
            if checked { ("func_80017DB4", misc::func_80017DB4) } else { ("func_80017E5C", misc::func_80017E5C) };
        let after = run(name, port, &s)?;
        let want = if checked && null { 0 } else { sext(word(&s, (arr as i32 + 4 * k) as u32)) };
        prop_assert_eq!(after.ctx.gpr[V0], want);
    }

    /// Two fields (`+0x2C`, `+0x28`) or zeros.
    #[test]
    fn func_80017DF4(seed: u64, zero in prop_oneof![Just(0u64), Just(1), Just(0x1_0000_0000)]) {
        let mut s = state(seed);
        s.randomise_memory(seed, O, 0x40);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(O), zero, sext(PA), sext(PB));
        let after = run("func_80017DF4", misc::func_80017DF4, &s)?;
        let want = if zero != 0 { (0, 0) } else { (word(&s, O + 0x2C), word(&s, O + 0x28)) };
        prop_assert_eq!((word(&after, PA), word(&after, PB)), want);
    }

    /// `[o + 8] = v` only for `which == 2`.
    #[test]
    fn func_80017E70(seed: u64, which in prop_oneof![Just(2u64), Just(1), Just(0x1_0000_0002), any::<u64>()], v: u64) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(O), which, v);
        let after = run("func_80017E70", misc::func_80017E70, &s)?;
        prop_assert_eq!(word(&after, O + 8), if which == 2 { v as u32 } else { word(&s, O + 8) });
    }

    /// Flag bits 3 and 6 as 1 | 2, or -1.
    #[test]
    fn func_80017E88(seed: u64, null: bool, which in prop_oneof![Just(1u64), Just(0), Just(0x1_0000_0001)], flags: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(O, flags);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (if null { 0 } else { sext(O) }, which);
        let after = run("func_80017E88", misc::func_80017E88, &s)?;
        let want = if null || which != 1 { u64::MAX } else { u64::from((flags >> 3) & 1 | (flags >> 5) & 2) };
        prop_assert_eq!(after.ctx.gpr[V0], want);
    }

    /// `[o] |= bits` and `[o] &= !bits`.
    #[test]
    fn flag_words(seed: u64, set: bool, old: u32, bits: u64) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(O, old);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(O), bits);
        let (name, port): (&str, RecompFn) =
            if set { ("func_80017EFC", misc::func_80017EFC) } else { ("func_80017F0C", misc::func_80017F0C) };
        let after = run(name, port, &s)?;
        prop_assert_eq!(word(&after, O), if set { old | bits as u32 } else { old & !(bits as u32) });
    }

    /// Record `k` for `0 <= k < 4`, else 0.
    #[test]
    fn func_80017F28(seed: u64, k in prop_oneof![-3i64..8, Just(i64::MIN), Just(0x1_0000_0001)]) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = k as u64;
        let after = run("func_80017F28", misc::func_80017F28, &s)?;
        let want = if (0..4).contains(&k) { sext(RECORDS_170 + 0x170 * k as u32) } else { 0 };
        prop_assert_eq!(after.ctx.gpr[V0], want);
    }
}
