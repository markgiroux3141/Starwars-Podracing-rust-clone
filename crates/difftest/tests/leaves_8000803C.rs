//! The leaves at 0x8000803C..0x80008760 (game::misc): a table-entry room
//! test, empty functions, two on/off switches, a range test and a byte
//! store. Recompiled C vs Rust on random register files and memory, each
//! checked against its statement.

// Tests are named after the functions (func_8000803C), capitals included.
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

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s
}

fn sp() -> impl Strategy<Value = u64> {
    (0x8000_0000u32..0x807F_FFFC).prop_map(|v| sext(v & !3))
}

/// Words for sums and limits: edges around the signed and unsigned wrap,
/// and anything.
fn edge_word() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0u32), Just(1), Just(0x7FFF_FFFF), Just(0x8000_0000), Just(0xFFFF_FFFF), any::<u32>()]
}

/// 64-bit selectors: the special values, their low words with other upper
/// halves (the compares are 64-bit), and anything.
fn selector() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        Just(1u64),
        Just(u64::MAX),
        Just(2u64),
        Just(0x0000_0001_0000_0000),
        Just(0x0000_0001_0000_0001),
        Just(0x0000_0000_FFFF_FFFF),
        any::<u64>(),
    ]
}

const OBJ: u32 = 0x8030_0000;
const TABLE: u32 = 0x8031_0000;
const P: u32 = 0x8032_0000;
const Q: u32 = 0x8033_0000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// `[q] + [q+4] <u [p+0x54]`, or 0 when the entry's `p` is null.
    #[test]
    fn func_8000803C(seed: u64, k in -4i32..64, null: bool, a in edge_word(), b in edge_word(), limit in edge_word()) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = sext(OBJ);
        s.ctx.gpr[A1] = k as i64 as u64;
        let entry = (TABLE as i64 + 0x30 * k as i64) as u32;
        {
            let mut m = s.rdram.mem();
            m.write_u32(OBJ + 0x40, TABLE);
            m.write_u32(entry + 8, if null { 0 } else { P });
            m.write_u32(P + 0x38, Q);
            m.write_u32(P + 0x54, limit);
            m.write_u32(Q, a);
            m.write_u32(Q + 4, b);
        }
        let after = run("func_8000803C", misc::func_8000803C, &s)?;
        let want = !null && a.wrapping_add(b) < limit;
        prop_assert_eq!(after.ctx.gpr[V0], want as u64);
        prop_assert_eq!(after.ctx.gpr[V1], if null { 0 } else { sext(P) });
    }

    /// Empty: nothing changes.
    #[test]
    fn func_80008530(seed: u64) {
        let s = state(seed);
        let after = run("func_80008530", misc::func_80008530, &s)?;
        prop_assert_eq!(after.ctx.gpr, s.ctx.gpr);
    }

    /// `[sp] = a0`, nothing else.
    #[test]
    fn func_80008540(seed: u64, a0: u64, sp in sp()) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = a0;
        s.ctx.gpr[SP] = sp;
        let after = run("func_80008540", misc::func_80008540, &s)?;
        prop_assert_eq!(word(&after, sp as u32), a0 as u32);
        prop_assert_eq!(after.ctx.gpr, s.ctx.gpr);
    }

    /// 0 sets the "off" flag, 1 clears it, -1 toggles it; returns flag == 0.
    #[test]
    fn off_switches(seed: u64, which: bool, on in selector(), flag in prop_oneof![Just(0u32), Just(1), any::<u32>()]) {
        let (name, port, addr): (&str, RecompFn, u32) = if which {
            ("func_80008630", misc::func_80008630, 0x8009_A2C4)
        } else {
            ("func_80008694", misc::func_80008694, 0x8009_A2C0)
        };
        let mut s = state(seed);
        s.ctx.gpr[A0] = on;
        s.rdram.mem().write_u32(addr, flag);
        let after = run(name, port, &s)?;
        let new = match on {
            0 => 1,
            1 => 0,
            u64::MAX => (flag == 0) as u32,
            _ => flag,
        };
        prop_assert_eq!(word(&after, addr), new);
        prop_assert_eq!(after.ctx.gpr[V0], (new == 0) as u64);
        prop_assert_eq!(after.ctx.gpr[A0], sext(addr));
    }

    /// `0x8E <= c < 0x9E || c == 0x22`, signed 64-bit.
    #[test]
    fn func_80008718(seed: u64, c in prop_oneof![
        Just(0x22u64), Just(0x21), Just(0x23), Just(0x8D), Just(0x8E), Just(0x9D), Just(0x9E),
        Just(0x1_0000_0022), Just(0xFFFF_FFFF_0000_0090), Just(u64::MAX), Just(0x8000_0000_0000_0090),
        0u64..0x100, any::<u64>(),
    ]) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = c;
        let after = run("func_80008718", misc::func_80008718, &s)?;
        let v = c as i64;
        prop_assert_eq!(after.ctx.gpr[V0], ((0x8E..0x9E).contains(&v) || v == 0x22) as u64);
    }

    /// `[sp] = a0` and the byte at 0x8009A324 = a0's low byte.
    #[test]
    fn func_80008750(seed: u64, a0: u64, sp in sp().prop_filter("not the byte's word", |&v| v as u32 != 0x8009_A324)) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = a0;
        s.ctx.gpr[SP] = sp;
        let after = run("func_80008750", misc::func_80008750, &s)?;
        prop_assert_eq!(word(&after, sp as u32), a0 as u32);
        prop_assert_eq!(word(&after, 0x8009_A324) >> 24, u32::from(a0 as u8));
        prop_assert_eq!(word(&after, 0x8009_A324) & 0x00FF_FFFF, word(&s, 0x8009_A324) & 0x00FF_FFFF);
    }
}
