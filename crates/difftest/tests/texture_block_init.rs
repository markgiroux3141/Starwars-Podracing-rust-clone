//! func_8003043C (texture_block_init): recompiled C vs
//! game::loader::func_8003043C. The count comes from baserom.z64, or from a
//! patched image in memory (difftest::rom::Image); rom_read_small is the
//! test double.
//!
//! A count of 1701 or more hangs (`b .`, which N64Recomp turns into the
//! runtime's `pause_self`; the oracle's traps). That aborts the process, so
//! those cases run in a child process, like crates/oracle/tests/traps.rs:
//! each child runs one side (C or Rust) on one count and must die in
//! `pause_self`.

use difftest::rom::{baserom, install_rom_image, Image};
use difftest::world::{sext, world, CACHE_WORDS, HEAP_8MB};
use difftest::{compare, State};
use game::loader::{self, TEXTURE_BLOCK, TEXTURE_CACHE, TEXTURE_COUNT};
use game::recomp::reg::*;
use proptest::prelude::*;
use std::process::Command;

const CHILD_ENV: &str = "TEXTURE_BLOCK_INIT_CHILD";
const PAUSE: &str = "pause_self: branch-to-self idle loop";

/// The ROM with the texture count replaced (None: as it is, 1648).
fn image(count: Option<i32>) -> Image {
    let img = Image::from(baserom());
    match count {
        Some(c) => img.patch(TEXTURE_BLOCK as usize, &c.to_be_bytes()),
        None => img,
    }
}

/// A world whose texture cache (and the count word after it) holds the
/// background pattern, not zeros, so the clearing is visible.
fn before(seed: u64) -> State {
    let mut s = world(seed, HEAP_8MB);
    s.randomise_memory(seed, TEXTURE_CACHE, 4 * CACHE_WORDS + 4);
    s
}

/// Run both sides, check the cache is cleared and the count stored.
fn check(count: Option<i32>, seed: u64) -> Result<(), TestCaseError> {
    let _rom = install_rom_image(image(count));
    let before = before(seed);
    let after = compare("func_8003043C", loader::func_8003043C, &before).map_err(|d| TestCaseError::fail(d.to_string()))?;
    let w = |a: u32| after.rdram.as_words()[((a - 0x8000_0000) / 4) as usize];
    let want = count.unwrap_or(1648);
    prop_assert_eq!(w(TEXTURE_COUNT), want as u32);
    for k in 0..CACHE_WORDS {
        prop_assert_eq!(w(TEXTURE_CACHE + 4 * k), 0, "cache word {}", k);
    }
    // Just past the count: untouched.
    let b = |a: u32| before.rdram.as_words()[((a - 0x8000_0000) / 4) as usize];
    prop_assert_eq!(w(TEXTURE_COUNT + 4), b(TEXTURE_COUNT + 4));
    prop_assert_eq!(w(TEXTURE_CACHE - 4), b(TEXTURE_CACHE - 4));
    let g = &after.ctx.gpr;
    prop_assert_eq!(g[V0], sext(TEXTURE_COUNT));
    prop_assert_eq!(g[V1], sext(TEXTURE_COUNT));
    prop_assert_eq!(g[AT], 1);
    prop_assert_eq!(g[T6], sext(want as u32));
    prop_assert_eq!(after.calls.len(), 1);
    Ok(())
}

#[test]
fn real_count() {
    check(None, 1).unwrap();
}

/// Up to 1700 is fine, and the compare is signed, so negative counts are
/// too. The whole cache is cleared whatever the count.
#[test]
fn edge_counts() {
    for (k, c) in [0, 1, 1647, 1648, 1699, 1700, -1, i32::MIN].into_iter().enumerate() {
        check(Some(c), k as u64).unwrap_or_else(|e| panic!("count {c}: {e}"));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn counts_that_return(count in i32::MIN..=1700, seed: u64) {
        check(Some(count), seed)?;
    }
}

/// In the child: run one side on one count. Reaching the end means it
/// returned instead of hanging.
#[test]
fn hang_child() {
    let Ok(which) = std::env::var(CHILD_ENV) else { return };
    let (side, count) = which.split_once(':').unwrap();
    let _rom = install_rom_image(image(Some(count.parse().unwrap())));
    let mut s = before(7);
    let f = match side {
        "c" => oracle::recomp::by_name("func_8003043C").unwrap(),
        "rust" => difftest::port_under_test("func_8003043C", loader::func_8003043C),
        _ => unreachable!(),
    };
    s.run(f);
    std::process::exit(0);
}

/// QUIRK: a count of 1701 or more hangs, in the C and in the port alike.
#[test]
fn big_counts_hang() {
    for count in [1701, 1702, 3296, i32::MAX] {
        for side in ["c", "rust"] {
            let out = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "hang_child", "--nocapture", "--test-threads=1"])
                .env(CHILD_ENV, format!("{side}:{count}"))
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(!out.status.success(), "{side}, count {count}: returned instead of hanging");
            assert!(stderr.contains(PAUSE), "{side}, count {count}: unexpected stderr:\n{stderr}");
        }
    }
}
