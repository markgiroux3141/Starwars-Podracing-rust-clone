//! `__osDisableInt` and `__osRestoreInt` (game::libultra): CP0 Status through
//! the runtime hooks, which the oracle keeps in the context's `status_reg`
//! (NOTES.md, "CP0 Status"). Recompiled C vs Rust; each side runs on its
//! own copy of the context, so `difftest::compare` compares the final Status
//! too. The checks below state what each does to it.

// Tests are named after the functions (func_8008CA80), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::libultra;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const FR: u32 = 1 << 26;

fn same_memory(after: &State, before: &State) -> Result<(), TestCaseError> {
    prop_assert!(after.rdram.as_words() == before.rdram.as_words(), "RDRAM changed");
    Ok(())
}

/// Status values: the boot value's shape (CU1, IE, interrupt masks), IE
/// on and off, all ones, and anything.
fn status() -> BoxedStrategy<u32> {
    prop_oneof![
        Just(0x2000_FF01u32),
        Just(0x2000_FF00u32),
        Just(0xFFFF_FFFFu32),
        Just(0u32),
        Just(1u32),
        Just(0x8000_0001u32),
        any::<u32>(),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// IE cleared, everything else kept; the old IE in `v0`.
    #[test]
    fn func_8008CA80(seed: u64, st in status()) {
        let mut s = State::new();
        s.randomise_registers(seed);
        s.ctx.status_reg = st;
        let after = run("func_8008CA80", libultra::func_8008CA80, &s)?;
        prop_assert_eq!(after.ctx.status_reg, st & !1);
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(st & 1));
        prop_assert_eq!(after.ctx.gpr[T0], sext(st));
        same_memory(&after, &s)?;
    }

    /// Status ORed with `a0` (any 64-bit value; the low word is kept), as
    /// long as that leaves FR alone (a change of FR traps).
    #[test]
    fn func_8008CAA0(seed: u64, st in status(), a0 in prop_oneof![Just(0u64), Just(1u64), any::<u32>().prop_map(sext), any::<u64>()]) {
        let a0 = if st & FR == 0 { a0 & !u64::from(FR) } else { a0 };
        let mut s = State::new();
        s.randomise_registers(seed);
        s.ctx.status_reg = st;
        s.ctx.gpr[A0] = a0;
        let after = run("func_8008CAA0", libultra::func_8008CAA0, &s)?;
        prop_assert_eq!(after.ctx.status_reg, st | a0 as u32);
        prop_assert_eq!(after.ctx.gpr[T0], sext(st) | a0);
        same_memory(&after, &s)?;
    }

    /// Disabling then restoring with the returned bit gives Status back,
    /// whatever IE was (each step compared C against Rust).
    #[test]
    fn func_8008CA80_round_trip(seed: u64, st in status()) {
        let mut s = State::new();
        s.randomise_registers(seed);
        s.ctx.status_reg = st;
        let mut mid = run("func_8008CA80", libultra::func_8008CA80, &s)?;
        mid.ctx.gpr[A0] = mid.ctx.gpr[V0];
        let after = run("func_8008CAA0", libultra::func_8008CAA0, &mid)?;
        prop_assert_eq!(after.ctx.status_reg, st);
    }
}
