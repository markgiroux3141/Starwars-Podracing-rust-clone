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
pub mod misc;
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
    Ported { vram: 0x8000_0520, name: "func_80000520", func: util::func_80000520 },
    Ported { vram: 0x8000_052C, name: "func_8000052C", func: util::func_8000052C },
    Ported { vram: 0x8000_0538, name: "func_80000538", func: util::func_80000538 },
    Ported { vram: 0x8000_0544, name: "func_80000544", func: util::func_80000544 },
    Ported { vram: 0x8000_054C, name: "func_8000054C", func: util::func_8000054C },
    Ported { vram: 0x8000_0554, name: "func_80000554", func: util::func_80000554 },
    Ported { vram: 0x8000_5AFC, name: "func_80005AFC", func: misc::func_80005AFC },
    Ported { vram: 0x8000_5B1C, name: "func_80005B1C", func: misc::func_80005B1C },
    Ported { vram: 0x8000_5B44, name: "func_80005B44", func: misc::func_80005B44 },
    Ported { vram: 0x8000_5B80, name: "func_80005B80", func: misc::func_80005B80 },
    Ported { vram: 0x8000_66DC, name: "func_800066DC", func: misc::func_800066DC },
    Ported { vram: 0x8000_66E4, name: "func_800066E4", func: misc::func_800066E4 },
    Ported { vram: 0x8000_66EC, name: "func_800066EC", func: misc::func_800066EC },
    Ported { vram: 0x8000_66F4, name: "func_800066F4", func: misc::func_800066F4 },
    Ported { vram: 0x8000_66FC, name: "func_800066FC", func: misc::func_800066FC },
    Ported { vram: 0x8000_6D5C, name: "func_80006D5C", func: misc::func_80006D5C },
    Ported { vram: 0x8000_6E50, name: "func_80006E50", func: misc::func_80006E50 },
    Ported { vram: 0x8000_6E60, name: "func_80006E60", func: misc::func_80006E60 },
    Ported { vram: 0x8000_6F34, name: "func_80006F34", func: misc::func_80006F34 },
    Ported { vram: 0x8000_6F3C, name: "func_80006F3C", func: misc::func_80006F3C },
    Ported { vram: 0x8000_6FD4, name: "func_80006FD4", func: misc::func_80006FD4 },
    Ported { vram: 0x8000_6FDC, name: "func_80006FDC", func: misc::func_80006FDC },
    Ported { vram: 0x8000_758C, name: "func_8000758C", func: misc::func_8000758C },
    Ported { vram: 0x8000_7710, name: "func_80007710", func: misc::func_80007710 },
    Ported { vram: 0x8000_7A44, name: "func_80007A44", func: misc::func_80007A44 },
    Ported { vram: 0x8000_7CE4, name: "func_80007CE4", func: misc::func_80007CE4 },
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
    Ported { vram: 0x8003_0574, name: "func_80030574", func: loader::func_80030574 },
    Ported { vram: 0x8003_05E8, name: "func_800305E8", func: loader::func_800305E8 },
];
