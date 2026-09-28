//! Star Wars Episode I: Racer (N64, USA), reimplemented in Rust.
//!
//! Phase 1 layout: each ported function keeps N64Recomp's signature and works
//! on the same RDRAM layout (through `n64mem`) and register file
//! ([`recomp::RecompContext`]), so it can stand in for the recompiled C and be
//! tested against it (`crates/difftest`).
//!
//! Ported functions are not `#[no_mangle]`: their Rust symbols don't collide
//! with the C ones when both are linked into the differential tests. Exporting
//! them under the C names, to override the recompiled code in the game build,
//! will be opt-in when that build exists.

pub mod asset;
pub mod recomp;
pub mod util;

use recomp::RecompFn;

/// A ported function and the original it replaces.
pub struct Ported {
    pub vram: u32,
    pub name: &'static str,
    pub func: RecompFn,
}

/// Every ported function (status `rust_verified` in `symbols/functions.csv`).
pub const PORTED: &[Ported] = &[
    Ported { vram: 0x8000_0554, name: "func_80000554", func: util::func_80000554 },
    Ported { vram: 0x8001_1940, name: "func_80011940", func: asset::func_80011940 },
];
