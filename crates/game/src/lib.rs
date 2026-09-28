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
pub mod math;
pub mod misc;
pub mod recomp;
pub mod util;

/// The translate crate's drafts of every ported function, built from
/// generated/ with the `translated` feature (see build.rs). difftest's
/// `translated` feature swaps them in for the ports, which validates the
/// translator against the existing difftests. Never committed.
#[cfg(all(feature = "translated", not(test)))]
#[allow(non_snake_case, unused_mut, unused_variables, unused_assignments)]
pub mod translated {
    include!(concat!(env!("OUT_DIR"), "/translated.rs"));
}

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
    Ported { vram: 0x8000_787C, name: "func_8000787C", func: misc::func_8000787C },
    Ported { vram: 0x8000_7A44, name: "func_80007A44", func: misc::func_80007A44 },
    Ported { vram: 0x8000_7CE4, name: "func_80007CE4", func: misc::func_80007CE4 },
    Ported { vram: 0x8000_803C, name: "func_8000803C", func: misc::func_8000803C },
    Ported { vram: 0x8000_8530, name: "func_80008530", func: misc::func_80008530 },
    Ported { vram: 0x8000_8540, name: "func_80008540", func: misc::func_80008540 },
    Ported { vram: 0x8000_8630, name: "func_80008630", func: misc::func_80008630 },
    Ported { vram: 0x8000_8694, name: "func_80008694", func: misc::func_80008694 },
    Ported { vram: 0x8000_8718, name: "func_80008718", func: misc::func_80008718 },
    Ported { vram: 0x8000_8750, name: "func_80008750", func: misc::func_80008750 },
    Ported { vram: 0x8000_8F6C, name: "func_80008F6C", func: misc::func_80008F6C },
    Ported { vram: 0x8000_9278, name: "func_80009278", func: misc::func_80009278 },
    Ported { vram: 0x8000_92B0, name: "func_800092B0", func: misc::func_800092B0 },
    Ported { vram: 0x8000_9524, name: "func_80009524", func: misc::func_80009524 },
    Ported { vram: 0x8000_953C, name: "func_8000953C", func: misc::func_8000953C },
    Ported { vram: 0x8000_955C, name: "func_8000955C", func: misc::func_8000955C },
    Ported { vram: 0x8000_A418, name: "func_8000A418", func: misc::func_8000A418 },
    Ported { vram: 0x8000_A920, name: "func_8000A920", func: misc::func_8000A920 },
    Ported { vram: 0x8000_AA78, name: "func_8000AA78", func: misc::func_8000AA78 },
    Ported { vram: 0x8000_AB24, name: "func_8000AB24", func: misc::func_8000AB24 },
    Ported { vram: 0x8000_ABD4, name: "func_8000ABD4", func: misc::func_8000ABD4 },
    Ported { vram: 0x8000_AC0C, name: "func_8000AC0C", func: misc::func_8000AC0C },
    Ported { vram: 0x8000_AC34, name: "func_8000AC34", func: misc::func_8000AC34 },
    Ported { vram: 0x8000_AC60, name: "func_8000AC60", func: misc::func_8000AC60 },
    Ported { vram: 0x8000_AEB4, name: "func_8000AEB4", func: misc::func_8000AEB4 },
    Ported { vram: 0x8000_AED4, name: "func_8000AED4", func: misc::func_8000AED4 },
    Ported { vram: 0x8000_AEFC, name: "func_8000AEFC", func: misc::func_8000AEFC },
    Ported { vram: 0x8000_B02C, name: "func_8000B02C", func: misc::func_8000B02C },
    Ported { vram: 0x8000_B06C, name: "func_8000B06C", func: misc::func_8000B06C },
    Ported { vram: 0x8000_B098, name: "func_8000B098", func: misc::func_8000B098 },
    Ported { vram: 0x8000_B1B0, name: "func_8000B1B0", func: misc::func_8000B1B0 },
    Ported { vram: 0x8000_C530, name: "func_8000C530", func: misc::func_8000C530 },
    Ported { vram: 0x8000_C5F0, name: "func_8000C5F0", func: misc::func_8000C5F0 },
    Ported { vram: 0x8000_C658, name: "func_8000C658", func: misc::func_8000C658 },
    Ported { vram: 0x8000_DA6C, name: "func_8000DA6C", func: misc::func_8000DA6C },
    Ported { vram: 0x8000_E9BC, name: "func_8000E9BC", func: misc::func_8000E9BC },
    Ported { vram: 0x8000_FCA4, name: "func_8000FCA4", func: misc::func_8000FCA4 },
    Ported { vram: 0x8000_FE1C, name: "func_8000FE1C", func: misc::func_8000FE1C },
    Ported { vram: 0x8000_FE78, name: "func_8000FE78", func: misc::func_8000FE78 },
    Ported { vram: 0x8000_FEF0, name: "func_8000FEF0", func: misc::func_8000FEF0 },
    Ported { vram: 0x8000_FFF8, name: "func_8000FFF8", func: misc::func_8000FFF8 },
    Ported { vram: 0x8001_0014, name: "func_80010014", func: misc::func_80010014 },
    Ported { vram: 0x8001_0040, name: "func_80010040", func: misc::func_80010040 },
    Ported { vram: 0x8001_004C, name: "func_8001004C", func: misc::func_8001004C },
    Ported { vram: 0x8001_1778, name: "func_80011778", func: misc::func_80011778 },
    Ported { vram: 0x8001_1814, name: "func_80011814", func: misc::func_80011814 },
    Ported { vram: 0x8001_1824, name: "func_80011824", func: misc::func_80011824 },
    Ported { vram: 0x8001_1838, name: "func_80011838", func: misc::func_80011838 },
    Ported { vram: 0x8001_1918, name: "func_80011918", func: misc::func_80011918 },
    Ported { vram: 0x8001_1928, name: "func_80011928", func: misc::func_80011928 },
    Ported { vram: 0x8001_1940, name: "func_80011940", func: asset::func_80011940 },
    Ported { vram: 0x8001_1DF0, name: "func_80011DF0", func: misc::func_80011DF0 },
    Ported { vram: 0x8001_1E54, name: "func_80011E54", func: misc::func_80011E54 },
    Ported { vram: 0x8001_1ECC, name: "func_80011ECC", func: misc::func_80011ECC },
    Ported { vram: 0x8001_1EE8, name: "func_80011EE8", func: misc::func_80011EE8 },
    Ported { vram: 0x8001_1F04, name: "func_80011F04", func: misc::func_80011F04 },
    Ported { vram: 0x8001_2B5C, name: "func_80012B5C", func: misc::func_80012B5C },
    Ported { vram: 0x8001_4C98, name: "func_80014C98", func: misc::func_80014C98 },
    Ported { vram: 0x8001_514C, name: "func_8001514C", func: math::func_8001514C },
    Ported { vram: 0x8001_5170, name: "func_80015170", func: math::func_80015170 },
    Ported { vram: 0x8001_51C0, name: "func_800151C0", func: math::func_800151C0 },
    Ported { vram: 0x8001_523C, name: "func_8001523C", func: math::func_8001523C },
    Ported { vram: 0x8001_7D48, name: "func_80017D48", func: misc::func_80017D48 },
    Ported { vram: 0x8001_7D50, name: "func_80017D50", func: misc::func_80017D50 },
    Ported { vram: 0x8001_7DA4, name: "func_80017DA4", func: misc::func_80017DA4 },
    Ported { vram: 0x8001_7DAC, name: "func_80017DAC", func: misc::func_80017DAC },
    Ported { vram: 0x8001_7DB4, name: "func_80017DB4", func: misc::func_80017DB4 },
    Ported { vram: 0x8001_7DDC, name: "func_80017DDC", func: misc::func_80017DDC },
    Ported { vram: 0x8001_7DE4, name: "func_80017DE4", func: misc::func_80017DE4 },
    Ported { vram: 0x8001_7DEC, name: "func_80017DEC", func: misc::func_80017DEC },
    Ported { vram: 0x8001_7DF4, name: "func_80017DF4", func: misc::func_80017DF4 },
    Ported { vram: 0x8001_7E54, name: "func_80017E54", func: misc::func_80017E54 },
    Ported { vram: 0x8001_7E5C, name: "func_80017E5C", func: misc::func_80017E5C },
    Ported { vram: 0x8001_7E70, name: "func_80017E70", func: misc::func_80017E70 },
    Ported { vram: 0x8001_7E88, name: "func_80017E88", func: misc::func_80017E88 },
    Ported { vram: 0x8001_7EDC, name: "func_80017EDC", func: misc::func_80017EDC },
    Ported { vram: 0x8001_7EE4, name: "func_80017EE4", func: misc::func_80017EE4 },
    Ported { vram: 0x8001_7EEC, name: "func_80017EEC", func: misc::func_80017EEC },
    Ported { vram: 0x8001_7EF4, name: "func_80017EF4", func: misc::func_80017EF4 },
    Ported { vram: 0x8001_7EFC, name: "func_80017EFC", func: misc::func_80017EFC },
    Ported { vram: 0x8001_7F0C, name: "func_80017F0C", func: misc::func_80017F0C },
    Ported { vram: 0x8001_7F20, name: "func_80017F20", func: misc::func_80017F20 },
    Ported { vram: 0x8001_7F28, name: "func_80017F28", func: misc::func_80017F28 },
    Ported { vram: 0x8001_8114, name: "func_80018114", func: misc::func_80018114 },
    Ported { vram: 0x8001_811C, name: "func_8001811C", func: misc::func_8001811C },
    Ported { vram: 0x8001_8164, name: "func_80018164", func: misc::func_80018164 },
    Ported { vram: 0x8001_82FC, name: "func_800182FC", func: misc::func_800182FC },
    Ported { vram: 0x8001_83A8, name: "func_800183A8", func: misc::func_800183A8 },
    Ported { vram: 0x8001_83B0, name: "func_800183B0", func: misc::func_800183B0 },
    Ported { vram: 0x8001_8440, name: "func_80018440", func: misc::func_80018440 },
    Ported { vram: 0x8001_8448, name: "func_80018448", func: misc::func_80018448 },
    Ported { vram: 0x8001_8450, name: "func_80018450", func: misc::func_80018450 },
    Ported { vram: 0x8001_8460, name: "func_80018460", func: misc::func_80018460 },
    Ported { vram: 0x8001_8470, name: "func_80018470", func: misc::func_80018470 },
    Ported { vram: 0x8001_F464, name: "func_8001F464", func: misc::func_8001F464 },
    Ported { vram: 0x8002_FAC4, name: "func_8002FAC4", func: heap::func_8002FAC4 },
    Ported { vram: 0x8002_FAFC, name: "func_8002FAFC", func: heap::func_8002FAFC },
    Ported { vram: 0x8002_FC58, name: "func_8002FC58", func: heap::func_8002FC58 },
    Ported { vram: 0x8002_FC80, name: "func_8002FC80", func: heap::func_8002FC80 },
    Ported { vram: 0x8002_FF38, name: "func_8002FF38", func: loader::func_8002FF38 },
    Ported { vram: 0x8003_0130, name: "func_80030130", func: loader::func_80030130 },
    Ported { vram: 0x8003_0154, name: "func_80030154", func: loader::func_80030154 },
    Ported { vram: 0x8003_0174, name: "func_80030174", func: loader::func_80030174 },
    Ported { vram: 0x8003_0328, name: "func_80030328", func: loader::func_80030328 },
    Ported { vram: 0x8003_043C, name: "func_8003043C", func: loader::func_8003043C },
    Ported { vram: 0x8003_04AC, name: "func_800304AC", func: loader::func_800304AC },
    Ported { vram: 0x8003_0574, name: "func_80030574", func: loader::func_80030574 },
    Ported { vram: 0x8003_05E8, name: "func_800305E8", func: loader::func_800305E8 },
];
