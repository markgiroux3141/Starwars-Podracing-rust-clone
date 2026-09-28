//! func_80000554 (zero `a1` words at `a0`): recompiled C vs game::util port.

use difftest::{compare, State};
use game::recomp::reg::{A0, A1};
use game::util::func_80000554;
use proptest::prelude::*;

const NAME: &str = "func_80000554";
/// Scratch area the destination is placed in; randomised per case.
const WINDOW: u32 = 0x8020_0000;
const WINDOW_LEN: u32 = 0x1_0000;

fn sext(v: i32) -> u64 {
    v as i64 as u64
}

/// Run both versions from a random state with the given arguments.
fn check(seed: u64, a0: u64, a1: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed.rotate_left(17), WINDOW, WINDOW_LEN);
    s.ctx.gpr[A0] = a0;
    s.ctx.gpr[A1] = a1;
    match compare(NAME, func_80000554, &s) {
        Ok(after) => after,
        Err(d) => panic!("{d}"),
    }
}

/// Independent statement of what the function should do to memory: exactly
/// the `n` words at `dst` become zero and nothing else changes.
fn assert_cleared(before: &State, after: &State, dst: u32, n: i64) {
    let n = n.max(0) as usize;
    let (b, a) = (before.rdram.as_words(), after.rdram.as_words());
    let first = ((dst - 0x8000_0000) / 4) as usize;
    assert!(a[..first] == b[..first], "words below {dst:#010x} changed");
    assert!(a[first..first + n].iter().all(|&w| w == 0), "not all {n} words at {dst:#010x} cleared");
    assert!(a[first + n..] == b[first + n..], "words after the {n} at {dst:#010x} changed");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Random registers and memory, destination anywhere in the window, count
    /// from negative through a few thousand words.
    #[test]
    fn matches_oracle(seed in any::<u64>(), word in 0u32..(WINDOW_LEN / 4), count in -16i32..=4096) {
        let dst = WINDOW + 4 * word;
        let mut before = State::new();
        before.randomise_registers(seed);
        before.randomise_memory(seed.rotate_left(17), WINDOW, WINDOW_LEN);
        before.ctx.gpr[A0] = sext(dst as i32);
        before.ctx.gpr[A1] = sext(count);
        let after = compare(NAME, func_80000554, &before).map_err(|d| TestCaseError::fail(d.to_string()))?;
        assert_cleared(&before, &after, dst, count as i64);
    }

    /// Every count 0..=64 hits every combination of the single-store prologue
    /// (count & 3) and the four-store loop.
    #[test]
    fn small_counts(seed in any::<u64>(), count in 0i32..=64) {
        check(seed, sext(WINDOW as i32), sext(count));
    }
}

#[test]
fn zero_and_negative_counts_do_nothing() {
    for n in [0, -1, -4, -5, i32::MIN, i32::MIN + 3] {
        let after = check(1, sext(WINDOW as i32), sext(n));
        assert_eq!(after.ctx.gpr[2], 0, "v0 for n = {n}");
    }
}

#[test]
fn counts_around_the_unroll() {
    for n in 1..=9 {
        check(n as u64, sext(WINDOW as i32 + 4), sext(n));
    }
}

#[test]
fn start_and_end_of_rdram() {
    // First words of RDRAM.
    check(2, sext(0x8000_0000u32 as i32), sext(7));
    // Right up to the end of the 8 MB (Expansion Pak) range.
    let n = 0x103;
    check(3, sext((0x8080_0000u32 - 4 * n) as i32), sext(n as i32));
    // The whole top megabyte, as a large count.
    check(4, sext(0x8070_0000u32 as i32), sext(0x4_0000));
}

#[test]
fn blez_compares_all_64_bits() {
    // Positive low word, negative as a 64-bit register: nothing happens.
    check(5, sext(WINDOW as i32), 0x8000_0000_0000_0010);
    check(6, sext(WINDOW as i32), 0xFFFF_FFFF_0000_0004);
}

#[test]
fn upper_register_halves_that_the_code_ignores() {
    // a0 is only used through 32-bit adds, so its upper half is discarded.
    check(7, 0x0000_0000_8020_0000, sext(9));
    check(8, 0x1234_5678_8020_0010, sext(12));
    // a1 with non-canonical upper bits and a multiple-of-4 low part: the
    // count used is the low word (the loop bound comes from `sll a1, 2`).
    check(9, sext(WINDOW as i32), 0x0000_0001_0000_0004);
}
