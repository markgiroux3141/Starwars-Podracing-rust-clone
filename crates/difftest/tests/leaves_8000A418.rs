//! The leaves at 0x8000A418..0x8000AC90 (game::misc): a bounded list append
//! and the setters/getters of the 32-byte RECORDS (and the three global ids
//! -201, -103, -104). Recompiled C vs Rust on random register files and
//! memory, each checked against its statement.

// Tests are named after the functions (func_8000A418), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc::{self, RECORDS};
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

/// Ids: the three globals, other negatives, and records.
fn id() -> impl Strategy<Value = i16> {
    prop_oneof![Just(-201i16), Just(-103), Just(-104), Just(-1), Just(-202), Just(-102), 0i16..128]
}

/// `a0` with `id` in its low halfword and anything above it.
fn with_junk(id: i16, junk: u64) -> u64 {
    (junk & !0xFFFF) | u64::from(id as u16)
}

fn record(id: i16) -> u32 {
    (RECORDS as i32 + 32 * i32::from(id)) as u32
}

fn on() -> impl Strategy<Value = u64> {
    prop_oneof![Just(0u64), Just(1), Just(0x1_0000_0000), any::<u64>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Append while the count is below 64 (signed).
    #[test]
    fn func_8000A418(seed: u64, count in -4i32..70, v: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = v;
        s.rdram.mem().write_u32(0x8009_B774, count as u32);
        let after = run("func_8000A418", misc::func_8000A418, &s)?;
        if count < 64 {
            prop_assert_eq!(word(&after, (0x800D_3A90i64 + 4 * count as i64) as u32), v as u32);
            prop_assert_eq!(word(&after, 0x8009_B774), (count + 1) as u32);
        } else {
            prop_assert_eq!(word(&after, 0x8009_B774), count as u32);
        }
    }

    /// On/off for the globals and the records' bit 0x20.
    #[test]
    fn func_8000A920(seed: u64, id in id(), junk: u64, on in on()) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        s.ctx.gpr[A1] = on;
        let after = run("func_8000A920", misc::func_8000A920, &s)?;
        let on = on != 0;
        match id {
            -201 => prop_assert_eq!(word(&after, 0x8009_B778), on as u32),
            -103 => prop_assert_eq!(byte(&after, 0x8009_B77F), if on { 0xFF } else { 0 }),
            -104 => prop_assert_eq!(byte(&after, 0x8009_B783), if on { 0xFF } else { 0 }),
            id if id >= 0 => {
                let old = word(&s, record(id) + 0x14);
                prop_assert_eq!(word(&after, record(id) + 0x14), if on { old | 0x20 } else { old & !0x20 });
            }
            _ => {
                for a in [0x8009_B778, 0x8009_B77C, 0x8009_B780] {
                    prop_assert_eq!(word(&after, a), word(&s, a));
                }
            }
        }
        prop_assert_eq!(word(&after, 0x800A_2800), s.ctx.gpr[A0] as u32);
        prop_assert_eq!(after.ctx.gpr[A0], id as i64 as u64);
    }

    /// Halfwords at +4/+6 for `id >= 0`.
    #[test]
    fn func_8000AA78(seed: u64, id in id(), junk: u64, x: u64, y: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        s.ctx.gpr[A1] = x;
        s.ctx.gpr[A2] = y;
        let after = run("func_8000AA78", misc::func_8000AA78, &s)?;
        if id >= 0 {
            prop_assert_eq!(half(&after, record(id) + 4), x as u16);
            prop_assert_eq!(half(&after, record(id) + 6), y as u16);
        }
    }

    /// Four bytes to a global or a record; the fourth from `[sp + 0x10]`.
    #[test]
    fn func_8000AB24(seed: u64, id in id(), junk: u64, r: u64, g: u64, b: u64, a: u32) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        s.ctx.gpr[A1] = r;
        s.ctx.gpr[A2] = g;
        s.ctx.gpr[A3] = b;
        s.rdram.mem().write_u32(0x800A_2810, a);
        let after = run("func_8000AB24", misc::func_8000AB24, &s)?;
        let at = match id {
            -103 => Some(0x8009_B77C),
            -104 => Some(0x8009_B780),
            id if id >= 0 => Some(record(id) + 0x18),
            _ => None,
        };
        if let Some(at) = at {
            let want = u32::from_be_bytes([r as u8, g as u8, b as u8, a as u8]);
            prop_assert_eq!(word(&after, at), want);
        }
        prop_assert_eq!(after.ctx.gpr[A1], r & 0xFF);
    }

    /// `[p + 8]` for the record's pointer `p`, or 0; negative ids read
    /// before the table (QUIRK).
    #[test]
    fn func_8000ABD4(seed: u64, id in prop_oneof![-8i16..0, 0i16..128], junk: u64, null: bool, v: u32) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        let p = 0x8030_0000u32;
        s.rdram.mem().write_u32(record(id) + 0x1C, if null { 0 } else { p });
        s.rdram.mem().write_u32(p + 8, v);
        let after = run("func_8000ABD4", misc::func_8000ABD4, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], if null { 0 } else { sext(v) });
    }

    /// Set the record's pointer for `id >= 0`.
    #[test]
    fn func_8000AC0C(seed: u64, id in prop_oneof![-8i16..0, 0i16..128], junk: u64, p: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        s.ctx.gpr[A1] = p;
        let after = run("func_8000AC0C", misc::func_8000AC0C, &s)?;
        let want = if id >= 0 { p as u32 } else { word(&s, record(id) + 0x1C) };
        prop_assert_eq!(word(&after, record(id) + 0x1C), want);
    }

    /// Flags `|=` / `&= !` for any id (no bounds check, QUIRK).
    #[test]
    fn record_flags(seed: u64, set: bool, id in prop_oneof![-8i16..0, 0i16..128], junk: u64, bits: u64) {
        let (name, port): (&str, RecompFn) =
            if set { ("func_8000AC34", misc::func_8000AC34) } else { ("func_8000AC60", misc::func_8000AC60) };
        let mut s = state(seed);
        s.ctx.gpr[A0] = with_junk(id, junk);
        s.ctx.gpr[A1] = bits;
        let after = run(name, port, &s)?;
        let old = word(&s, record(id) + 0x14);
        let want = if set { old | bits as u32 } else { old & !(bits as u32) };
        prop_assert_eq!(word(&after, record(id) + 0x14), want);
    }
}

/// The -103 global's fourth byte is what func_8000A920 switches.
#[test]
fn colour_then_off() {
    let mut s = state(3);
    s.ctx.gpr[A0] = (-103i64) as u64;
    s.ctx.gpr[A1] = 0x11;
    s.ctx.gpr[A2] = 0x22;
    s.ctx.gpr[A3] = 0x33;
    s.rdram.mem().write_u32(0x800A_2810, 0x44);
    let mut s = compare("func_8000AB24", misc::func_8000AB24, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(word(&s, 0x8009_B77C), 0x1122_3344);
    s.ctx.gpr[A0] = (-103i64) as u64;
    s.ctx.gpr[A1] = 0;
    let s = compare("func_8000A920", misc::func_8000A920, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(word(&s, 0x8009_B77C), 0x1122_3300);
}
