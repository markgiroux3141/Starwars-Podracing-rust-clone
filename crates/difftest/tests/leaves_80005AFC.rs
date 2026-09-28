//! The leaves at 0x80005AFC..0x80005BB8 and the five empty functions at
//! 0x800066DC..0x80006704 (game::misc). Recompiled C vs Rust on random
//! register files and random globals, each checked against its statement.

// Tests are named after the functions (func_8000052C), capitals included.
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

/// `which` arguments: the two that select something, the same low words
/// with other upper halves (a 64-bit compare rejects them), and anything.
fn which() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(3u64),
        Just(5u64),
        Just(0x0000_0001_0000_0003),
        Just(0xFFFF_FFFF_0000_0005),
        Just(4u64),
        Just(0u64),
        Just(u64::MAX),
        any::<u64>(),
    ]
}

/// Values for signed words: the edges and anything.
fn value() -> impl Strategy<Value = i32> {
    prop_oneof![Just(0), Just(1), Just(-1), Just(i32::MIN), Just(i32::MAX), any::<i32>()]
}

fn sp() -> impl Strategy<Value = u64> {
    (0x8000_0000u32..0x807F_FFFC).prop_map(|v| sext(v & !3))
}

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s
}

const A29C: u32 = 0x8009_A29C;
const A290: u32 = 0x8009_A290;
const A28C: u32 = 0x8009_A28C;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Decrement `[0x8009A29C]` if positive.
    #[test]
    fn func_80005AFC(seed: u64, v in value()) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(A29C, v as u32);
        let after = run("func_80005AFC", misc::func_80005AFC, &s)?;
        let want = if v > 0 { v - 1 } else { v };
        prop_assert_eq!(word(&after, A29C), want as u32);
        prop_assert_eq!(after.ctx.gpr[V0], v as i64 as u64);
        prop_assert_eq!(after.ctx.gpr[T6], sext((v as u32).wrapping_sub(1)));
    }

    /// Store `a1` by `which`.
    #[test]
    fn func_80005B1C(seed: u64, which in which(), a1: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = which;
        s.ctx.gpr[A1] = a1;
        let after = run("func_80005B1C", misc::func_80005B1C, &s)?;
        let (w290, w28c) = (word(&s, A290), word(&s, A28C));
        prop_assert_eq!(word(&after, A290), if which == 3 { a1 as u32 } else { w290 });
        prop_assert_eq!(word(&after, A28C), if which == 5 { a1 as u32 } else { w28c });
    }

    /// Load by `which`, else -1.
    #[test]
    fn func_80005B44(seed: u64, which in which(), x: i32, y: i32) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = which;
        s.rdram.mem().write_u32(A290, x as u32);
        s.rdram.mem().write_u32(A28C, y as u32);
        let after = run("func_80005B44", misc::func_80005B44, &s)?;
        let want = match which {
            3 => x as i64 as u64,
            5 => y as i64 as u64,
            _ => u64::MAX,
        };
        prop_assert_eq!(after.ctx.gpr[V0], want);
    }

    /// The five empty one-argument functions: `[sp] = a0`, nothing else.
    #[test]
    fn spill_a0(seed: u64, k in 0usize..5, a0: u64, sp in sp()) {
        let (name, port): (&str, RecompFn) = [
            ("func_800066DC", misc::func_800066DC as RecompFn),
            ("func_800066E4", misc::func_800066E4),
            ("func_800066EC", misc::func_800066EC),
            ("func_800066F4", misc::func_800066F4),
            ("func_800066FC", misc::func_800066FC),
        ][k];
        let mut s = state(seed);
        s.ctx.gpr[A0] = a0;
        s.ctx.gpr[SP] = sp;
        let after = run(name, port, &s)?;
        prop_assert_eq!(word(&after, sp as u32), a0 as u32);
        prop_assert_eq!(after.ctx.gpr, s.ctx.gpr);
    }
}

/// A round trip through the setter and getter, for both slots.
#[test]
fn set_then_get() {
    for (which, v) in [(3u64, 0x1234_5678u64), (5, sext(0x8765_4321))] {
        let mut s = state(which);
        s.ctx.gpr[A0] = which;
        s.ctx.gpr[A1] = v;
        let mut s = compare("func_80005B1C", misc::func_80005B1C, &s).unwrap();
        s.ctx.gpr[A0] = which;
        let s = compare("func_80005B44", misc::func_80005B44, &s).unwrap();
        assert_eq!(s.ctx.gpr[V0], v);
    }
}

/// Clears `[0x8009A2A0]` and exactly the 300 words from 0x800AF4C0.
#[test]
fn func_80005B80() {
    for seed in 0..8 {
        let mut s = state(seed);
        s.randomise_memory(seed, 0x800A_F400, 0x800);
        let after = compare("func_80005B80", misc::func_80005B80, &s).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(word(&after, 0x8009_A2A0), 0);
        for a in (0x800A_F400..0x800A_FC00).step_by(4) {
            let want = if (0x800A_F4C0..0x800A_F970).contains(&a) { 0 } else { word(&s, a) };
            assert_eq!(word(&after, a), want, "{a:#x}");
        }
        assert_eq!(after.ctx.gpr[V0], sext(0x800A_F970));
        assert_eq!(after.ctx.gpr[V1], sext(0x800A_F970));
        assert_eq!(after.ctx.gpr[AT], sext(0x800A_0000));
    }
}
