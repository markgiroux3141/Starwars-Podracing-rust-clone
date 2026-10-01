//! Contract doubles for functions that only read a hardware register
//! (NOTES.md, "The OS boundary and message-queue doubles", plan item 3):
//! the test supplies what the register reads, and both runs of a difftest
//! see the same values.
//!
//! `osGetCount` (`func_8008C550`) is `mfc0 v0, Count; jr ra`: N64Recomp
//! can't translate `mfc0 Count` (recomp.toml skips the function), so the
//! oracle only has a stub for it, and its whole effect is `v0`.

use game::recomp::{reg::*, s32, RecompContext};
use n64mem::Mem;
use oracle::doubles::{self, Installed};

/// `osGetCount`: `v0` = the next of `counts`, sign-extended (`mfc0` of a
/// 32-bit register). The k-th call of a run gets `counts[k % counts.len()]`
/// (the count restarts with each run, [`doubles::calls_in_run`]), so the C
/// and the Rust run of [`crate::compare`] read the same sequence. Nothing
/// else changes.
pub fn install_count(counts: Vec<u32>) -> Installed {
    assert!(!counts.is_empty(), "install_count needs at least one Count value");
    doubles::install("func_8008C550", move |_m: &mut Mem, ctx: &mut RecompContext| {
        let k = doubles::calls_in_run("func_8008C550") - 1;
        ctx.gpr[V0] = s32(counts[k % counts.len()]);
    })
}
