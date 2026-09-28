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
//!
//! Ports call other functions through their C symbols ([`imports`],
//! [`recomp::call`]), so anything that links this crate must define those
//! symbols: the oracle does in the tests. Tools that only need data formats
//! use the `assets` crate, which doesn't depend on this one.

pub mod asset;
pub mod heap;
pub mod imports;
pub mod loader;
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
    Ported { vram: 0x8002_FAC4, name: "func_8002FAC4", func: heap::func_8002FAC4 },
    Ported { vram: 0x8002_FAFC, name: "func_8002FAFC", func: heap::func_8002FAFC },
    Ported { vram: 0x8002_FC58, name: "func_8002FC58", func: heap::func_8002FC58 },
    Ported { vram: 0x8002_FC80, name: "func_8002FC80", func: heap::func_8002FC80 },
    Ported { vram: 0x8002_FF38, name: "func_8002FF38", func: loader::func_8002FF38 },
    Ported { vram: 0x8003_0130, name: "func_80030130", func: loader::func_80030130 },
    Ported { vram: 0x8003_0154, name: "func_80030154", func: loader::func_80030154 },
    Ported { vram: 0x8003_0328, name: "func_80030328", func: loader::func_80030328 },
    Ported { vram: 0x8003_043C, name: "func_8003043C", func: loader::func_8003043C },
    Ported { vram: 0x8003_04AC, name: "func_800304AC", func: loader::func_800304AC },
    Ported { vram: 0x8003_05E8, name: "func_800305E8", func: loader::func_800305E8 },
];
