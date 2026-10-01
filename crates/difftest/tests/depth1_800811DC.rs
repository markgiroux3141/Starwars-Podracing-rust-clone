//! Depth 1 at 0x800811DC (game::misc): the game's 64-bit tick count from
//! `osGetCount` (`func_8008C550`, a contract double here: `difftest::hw`)
//! and `__ll_lshift`. Recompiled C vs Rust, both reading the same injected
//! Count; the model replays `__ll_lshift`'s C and adds the function's own
//! stores.

// Tests are named after the functions (func_800811DC), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, hw, State};
use game::imports;
use game::misc;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;
const HIGH: u32 = 0x800A_675C;
const LAST: u32 = 0x800A_6760;

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
}

fn wr(s: &mut State, a: u32, v: u32) {
    s.rdram.mem().write_u32(a, v);
}

fn count() -> BoxedStrategy<u32> {
    prop_oneof![Just(0u32), Just(u32::MAX), Just(0x7FFF_FFFFu32), Just(0x8000_0000u32), any::<u32>()].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Count above, equal to and below the last value (a wrap), high words
    /// at their limits (the increment wraps too).
    #[test]
    fn func_800811DC(seed: u64, c in count(), last in prop_oneof![count(), Just(1u32)], hi in count(), equal: bool) {
        let last = if equal { c } else { last };
        let _count = hw::install_count(vec![c, !c]);
        let mut s = State::new();
        s.randomise_registers(seed);
        s.ctx.gpr[SP] = sext(SP_AT);
        wr(&mut s, HIGH, hi);
        wr(&mut s, LAST, last);
        let mut w = s.clone();
        let sp = SP_AT - 0x20;
        wr(&mut w, sp + 0x14, s.ctx.gpr[RA] as u32);
        let hi = if c < last { hi.wrapping_add(1) } else { hi };
        wr(&mut w, HIGH, hi);
        wr(&mut w, LAST, c);
        wr(&mut w, sp + 0x1C, c);
        (w.ctx.gpr[A0], w.ctx.gpr[A1], w.ctx.gpr[A2], w.ctx.gpr[A3]) = (0, sext(hi), 0, 0x20);
        w.ctx.gpr[SP] = sext(sp);
        w.run(imports::func_8008AB84);
        prop_assert_eq!((w.ctx.gpr[V0], w.ctx.gpr[V1]), (sext(hi), 0));
        let after = run("func_800811DC", misc::func_800811DC, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!((after.ctx.gpr[V0], after.ctx.gpr[V1]), (sext(hi), sext(c)));
    }
}

/// The count double gives the k-th `osGetCount` call of a run `counts[k]`,
/// counting only its own calls: a ROM-read double called in between
/// doesn't shift it.
#[test]
fn count_double_counts_its_own_calls() {
    unsafe extern "C" fn reads(rdram: *mut u8, ctx: *mut game::recomp::RecompContext) {
        imports::func_8008C550(rdram, ctx);
        let first = (*ctx).gpr[V0];
        (*ctx).gpr[A0] = 0;
        (*ctx).gpr[A1] = sext(0x8030_0000);
        (*ctx).gpr[A2] = 0;
        imports::func_80011D60(rdram, ctx);
        imports::func_8008C550(rdram, ctx);
        (*ctx).gpr[V1] = first;
    }
    let _count = hw::install_count(vec![7, 0x8000_0009, 11]);
    let _rom = difftest::rom::install_rom_doubles();
    let mut s = State::new();
    s.ctx.gpr[SP] = sext(SP_AT);
    s.run(reads);
    assert_eq!((s.ctx.gpr[V1], s.ctx.gpr[V0]), (7, sext(0x8000_0009)));
    assert_eq!(s.calls.len(), 3);
}
