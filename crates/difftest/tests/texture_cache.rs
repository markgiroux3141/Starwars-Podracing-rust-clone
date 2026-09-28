//! func_80030574 (texture_cache_trim): recompiled C vs
//! game::loader::func_80030574, a leaf that zeroes the texture cache words
//! above `a0`.

use difftest::world::{sext, CACHE_WORDS, STACK};
use difftest::{compare, Rng, State};
use game::loader::{self, TEXTURE_CACHE, TEXTURE_COUNT};
use game::recomp::reg::*;
use proptest::prelude::*;

fn at(k: u32) -> usize {
    ((TEXTURE_CACHE + 4 * k - 0x8000_0000) / 4) as usize
}

/// A cache like a game's: zeros, pointers into the heap on both sides of
/// `around`, `around` itself, and some arbitrary words.
fn state(seed: u64, around: u32) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(STACK);
    let mut rng = Rng::new(seed);
    let w = s.rdram.as_words_mut();
    for k in 0..CACHE_WORDS {
        w[at(k)] = match rng.next_u32() % 6 {
            0 | 1 => 0,
            2 => around.wrapping_sub(rng.next_u32() % 0x10_0000),
            3 => around.wrapping_add(rng.next_u32() % 0x10_0000),
            4 => around,
            _ => rng.next_u32(),
        };
    }
    s
}

/// Run both sides and check each word against the rule: zeroed iff
/// `a0 < sign-extended word` (unsigned 64-bit), and nothing else written.
fn check(before: &State) -> Result<(), TestCaseError> {
    let after = compare("func_80030574", loader::func_80030574, before).map_err(|d| TestCaseError::fail(d.to_string()))?;
    let a0 = before.ctx.gpr[A0];
    let (b, a) = (before.rdram.as_words(), after.rdram.as_words());
    for k in 0..CACHE_WORDS {
        let word = b[at(k)];
        let want = if a0 < sext(word) { 0 } else { word };
        prop_assert_eq!(a[at(k)], want, "word {} = {:#x}, a0 {:#x}", k, word, a0);
    }
    let (lo, hi) = (at(0), at(CACHE_WORDS));
    prop_assert!(a[..lo] == b[..lo] && a[hi..] == b[hi..], "wrote outside the cache");
    prop_assert_eq!(after.ctx.gpr[V0], sext(TEXTURE_COUNT));
    prop_assert_eq!(after.ctx.gpr[V1], sext(TEXTURE_COUNT));
    prop_assert!(after.calls.is_empty());
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Canonical heap addresses, as heap_set_level passes them.
    #[test]
    fn canonical_a0(around in 0x8014_0000u32..0x8064_0000, seed: u64) {
        let mut s = state(seed, around);
        s.ctx.gpr[A0] = sext(around);
        check(&s)?;
    }

    /// Any 64-bit value: a0 is only compared.
    #[test]
    fn any_a0(a0: u64, around: u32, seed: u64) {
        let mut s = state(seed, around);
        s.ctx.gpr[A0] = a0;
        check(&s)?;
    }
}

#[test]
fn edge_values() {
    for (k, a0) in [
        0,
        u64::MAX,
        sext(0x8000_0000),
        sext(0x7FFF_FFFF),
        sext(0xFFFF_FFFF),
        0x0000_0000_8019_8820, // zero-extended: below every KSEG0 entry
        0x0000_0000_FFFF_FFFF,
        0x8000_0000_0000_0000,
    ]
    .into_iter()
    .enumerate()
    {
        for around in [0x8019_8820, 0, 0xFFFF_FFFF, 0x7FFF_FFFF] {
            let mut s = state(k as u64 * 7 + u64::from(around), around);
            s.ctx.gpr[A0] = a0;
            check(&s).unwrap_or_else(|e| panic!("a0 {a0:#x}: {e}"));
        }
    }
    // All zero, all equal to a0, all above: nothing / nothing / everything.
    for (fill, a0, cleared) in [(0u32, 0u64, false), (0x8019_8820, sext(0x8019_8820), false), (0x8019_8824, sext(0x8019_8820), true)] {
        let mut s = State::new();
        for k in 0..CACHE_WORDS {
            s.rdram.as_words_mut()[at(k)] = fill;
        }
        s.ctx.gpr[A0] = a0;
        let after = compare("func_80030574", loader::func_80030574, &s).unwrap_or_else(|d| panic!("{d}"));
        let expect = if cleared { 0 } else { fill };
        assert!((0..CACHE_WORDS).all(|k| after.rdram.as_words()[at(k)] == expect), "fill {fill:#x}");
    }
}
