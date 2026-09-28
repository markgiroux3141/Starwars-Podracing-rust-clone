//! Depth-0 functions with jump tables (game::misc). Recompiled C vs Rust
//! on random register files and memory, each checked against its statement.
//! Every case of each table is reached. In these functions the index is
//! bounded (`sltiu`) just before the table, so the C's `default:
//! switch_error` can't be reached and has no test here.

// Tests are named after the functions (func_80008F6C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc;
use game::recomp::reg::*;
use proptest::prelude::*;

fn run(name: &str, port: game::recomp::RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

/// The signed halfword at `a` (big-endian: offset 0 is the word's high half).
fn half(s: &State, a: u32) -> i16 {
    let w = s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize];
    (if a & 2 == 0 { w >> 16 } else { w }) as u16 as i16
}

// ---------------------------------------------------------------------------
// func_80008F6C: handle from (kind, a1, table index)

/// Table lengths by kind, from the bounds in the code.
const LEN: [u64; 8] = [0x33, 0x26, 0x39, 5, 0x68, 0xA9, 0x69, 0xA8];

/// Where kind `k`'s halfword table starts: back to back from 0x8009A6F0,
/// each 4-aligned (derived from the lengths, not from the port's offsets).
fn table(k: usize) -> u32 {
    let mut a = 0x8009_A6F0u32;
    for len in &LEN[..k] {
        a = (a + 2 * *len as u32 + 3) & !3;
    }
    a
}

fn handle_state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    // All eight tables, entries of both signs.
    s.randomise_memory(seed ^ 0x5A5A, 0x8009_A6F0, 0x8009_AC00 - 0x8009_A6F0);
    s
}

/// The statement: -1 for `index == -1`, `kind >= 8`, kinds 0/1 with `a1`
/// outside [0, 0x17), or `index` outside (0, len); else the handle.
fn handle(s: &State, kind: u64, a1: u64, index: u64) -> u64 {
    if index == u64::MAX || kind >= 8 {
        return u64::MAX;
    }
    let k = kind as usize;
    if k < 2 && !(0..0x17).contains(&(a1 as i64)) {
        return u64::MAX;
    }
    if (index as i64) <= 0 || index >= LEN[k] {
        return u64::MAX;
    }
    let entry = half(s, table(k) + 2 * index as u32) as i64 as u64;
    sext((kind as u32) << 24) | sext((a1 as u32) << 16) | entry | 0x8000
}

/// Indices near the kind's bounds, or anything.
fn index_for(kind: u64, mode: u8, raw: u64) -> u64 {
    let len = LEN.get(kind as usize).copied().unwrap_or(4);
    match mode {
        0 => len - 1 + raw % 3, // len-1, len, len+1
        1 => raw % 3,           // 0, 1, 2
        2 => u64::MAX,
        3 => raw | 1 << 63,     // negative
        4 => 1 + raw % (len - 1), // in range
        5 => (raw % len) | 1 << 32, // junk above a small value
        _ => raw,
    }
}

fn a1s() -> impl Strategy<Value = u64> {
    prop_oneof![
        (-2i64..0x19).prop_map(|v| v as u64),
        Just(0x1_0000_0005u64),
        Just(0x8000_0000u64),
        any::<u64>(),
    ]
}

fn kinds() -> impl Strategy<Value = u64> {
    prop_oneof![4 => 0u64..8, 1 => 8u64..10, 1 => Just(u64::MAX), 1 => Just(0x1_0000_0002u64), 1 => any::<u64>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn func_80008F6C(seed: u64, kind in kinds(), a1 in a1s(), mode in 0u8..8, raw: u64) {
        let index = index_for(kind, mode, raw);
        let mut s = handle_state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (kind, a1, index);
        let after = run("func_80008F6C", misc::func_80008F6C, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], handle(&s, kind, a1, index), "kind {:#x} a1 {:#x} index {:#x}", kind, a1, index);
    }
}

/// Every case of the table, each with a valid handle, both ends of the
/// index range and the first index past it; and both signs of entry.
#[test]
fn func_80008F6C_every_case() {
    for kind in 0..8u64 {
        let len = LEN[kind as usize];
        for (index, entry) in [(1, 0x1234u16), (len - 1, 0xFEDC), (len, 0x1111)] {
            let mut s = handle_state(kind);
            s.rdram.mem().write_u16(table(kind as usize) + 2 * index as u32, entry);
            (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (kind, 0x16, index);
            let after = compare("func_80008F6C", misc::func_80008F6C, &s).unwrap_or_else(|d| panic!("{d}"));
            let want = if index == len {
                u64::MAX
            } else {
                sext((kind as u32) << 24 | 0x16 << 16) | entry as i16 as i64 as u64 | 0x8000
            };
            assert_eq!(after.ctx.gpr[V0], want, "kind {kind} index {index}");
        }
    }
}
