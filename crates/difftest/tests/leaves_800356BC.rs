//! The leaves at 0x800356BC..0x80037BF0 (game::misc, skipping the two long
//! byte rewriters 0x8003594C and 0x8003609C): render-mode switches, an input
//! mask adjuster, two argument spills and a setter. Recompiled C vs Rust on
//! random register files and memory, each checked against its statement.

// Tests are named after the functions (func_800356BC), capitals included.
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

const SP0: u32 = 0x800A_2800;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP0);
    s
}

/// The four modes by (zb, aa) bit, first and second words.
const MODES: [(u32, u32); 4] = [(0x0C08_4000, 0x0302_4000), (0x0044_2230, 0x0011_2230), (0x0044_2048, 0x0011_2048), (0x0044_2078, 0x0011_2078)];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// ZBZB flips bit 0 of the mode index, AAEN bit 1; Full sets both words.
    #[test]
    fn func_800356BC(seed: u64, tag in 0usize..4, mode in 0usize..5, other: u32, second: u32, a1: u64,
                     a2 in prop_oneof![Just(0u64), Just(1u64), Just(0x1_0000_0001u64), any::<u64>()]) {
        let tags = [0x5A42_5A42u32, 0x4141_454E, 0x4675_6C6C, 0x1234_5678];
        let mut s = state(seed);
        let first = MODES.get(mode).map_or(other, |m| m.0);
        s.rdram.mem().write_u32(0x800A_3DA0, first);
        s.rdram.mem().write_u32(0x800A_3DA4, second);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(tags[tag]), a1, a2);
        let after = run("func_800356BC", misc::func_800356BC, &s)?;
        let want = match (tag, MODES.iter().position(|m| m.0 == first)) {
            (2, _) => (a2 as u32, a2 as u32),
            (0 | 1, Some(k)) => {
                let bit = 1 << tag;
                let on = a2 == 1;
                if (k & bit != 0) != on { MODES[k ^ bit] } else { (first, second) }
            }
            _ => (first, second),
        };
        prop_assert_eq!((word(&after, 0x800A_3DA0), word(&after, 0x800A_3DA4)), want);
        prop_assert_eq!(word(&after, SP0 + 4), a1 as u32);
    }

    /// Clear bits 16/17 by settings bits 6/5, then clear or swap bits 9/10.
    #[test]
    fn func_800358A0(seed: u64, buttons in prop_oneof![any::<u32>().prop_map(sext), any::<u64>(), (0u64..4).prop_map(|b| b << 9)],
                     settings: u32, enabled in prop_oneof![Just(0u32), any::<u32>()], swap in prop_oneof![Just(0u32), any::<u32>()]) {
        let mut s = state(seed);
        let mut m = s.rdram.mem();
        m.write_u32(0x800D_697C, settings);
        m.write_u32(0x800A_4744, enabled);
        m.write_u32(0x800A_3D60, swap);
        drop(m);
        s.ctx.gpr[A0] = buttons;
        let after = run("func_800358A0", misc::func_800358A0, &s)?;
        let mut b = buttons;
        if settings & 0x40 != 0 { b &= !0x1_0000; }
        if settings & 0x20 != 0 { b &= !0x2_0000; }
        if enabled == 0 {
            b &= !0x600;
        } else if swap != 0 && ((b >> 9) & 3 == 1 || (b >> 9) & 3 == 2) {
            b ^= 0x600;
        }
        prop_assert_eq!(after.ctx.gpr[V0], b);
    }

    /// Spills and the setter.
    #[test]
    fn small(seed: u64, k in 0usize..3, a0: u64) {
        let (name, port): (&str, RecompFn) = [
            ("func_80036094", misc::func_80036094 as RecompFn),
            ("func_80037BF0", misc::func_80037BF0),
            ("func_80036F7C", misc::func_80036F7C),
        ][k];
        let mut s = state(seed);
        s.ctx.gpr[A0] = a0;
        let after = run(name, port, &s)?;
        if k < 2 {
            prop_assert_eq!(word(&after, SP0), a0 as u32);
        } else {
            prop_assert_eq!((word(&after, 0x800A_3D30), word(&after, 0x800A_3D38)), (0x800D_B930, 0));
        }
    }
}
