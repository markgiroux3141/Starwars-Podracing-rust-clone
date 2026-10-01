//! The hardware doubles (difftest::hw) against N64Recomp's generated C of
//! the same functions (oracle::probes). The C reads and writes device
//! registers past RDRAM (`0xA4xxxxxx`), so it runs on a buffer that covers
//! them, the register's word placed where the C's `MEM_W` looks for it;
//! the double runs on an ordinary state, through the oracle's stub. Both
//! must leave the same registers, Status and RDRAM, and the double must
//! write the C's word to its register model.

// Tests are named after the functions (func_80090500), capitals included.
#![allow(non_snake_case)]

use std::sync::{Mutex, OnceLock};

use difftest::rom::baserom;
use difftest::{hw, State};
use game::imports;
use game::recomp::{reg::*, RecompContext, RecompFn};
use proptest::prelude::*;

const MI_INTR_MASK: u32 = 0xA430_000C;
const AI_LEN: u32 = 0xA450_0004;
const GLOBAL_MASK: u32 = 0x800A_7B50;
const IM_TABLE: u32 = 0x800A_DF90;

/// Bytes from RDRAM's base (KSEG0 `0x80000000`) to the end of the highest
/// register used; one buffer for every test (about 580 MB, mostly never
/// touched).
const BIG: usize = (AI_LEN - 0x8000_0000) as usize + 4;

fn big() -> &'static Mutex<Vec<u8>> {
    static B: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();
    B.get_or_init(|| Mutex::new(vec![0u8; BIG]))
}

fn wr(s: &mut State, a: u32, v: u32) {
    s.rdram.mem().write_u32(a, v);
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

/// The halfwords of `__osRcpImTable` as the ROM has them.
fn rom_table() -> Vec<u16> {
    let rom = baserom();
    (0..64u32)
        .map(|k| {
            let o = (IM_TABLE + 2 * k - 0x8000_0400 + 0x1000) as usize;
            u16::from_be_bytes([rom[o], rom[o + 1]])
        })
        .collect()
}

/// Runs the generated C of `name` on the big buffer: RDRAM is `s`'s, the
/// register words are `regs` (address, value). Returns the context after
/// and each register word after.
fn run_c(name: &str, s: &State, regs: &[(u32, u32)]) -> (Box<RecompContext>, Vec<u32>, bool) {
    let f: RecompFn = oracle::probes::by_name(name).unwrap_or_else(|| panic!("no probe for {name}"));
    let mut buf = big().lock().unwrap_or_else(|e| e.into_inner());
    let words = s.rdram.as_words();
    // SAFETY: the words' own bytes, as N64Recomp's MEM_ macros see them.
    let bytes = unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), 4 * words.len()) };
    buf[..bytes.len()].copy_from_slice(bytes);
    let reg_at = |a: u32| (a - 0x8000_0000) as usize;
    for &(a, v) in regs {
        buf[reg_at(a)..reg_at(a) + 4].copy_from_slice(&v.to_ne_bytes());
    }
    let mut ctx = s.ctx.clone();
    ctx.fix_f_odd();
    // SAFETY: the buffer covers RDRAM and every register the C touches; ctx
    // is exclusive.
    unsafe { f(buf.as_mut_ptr(), &mut *ctx) };
    let after: Vec<u32> = regs.iter().map(|&(a, _)| u32::from_ne_bytes(buf[reg_at(a)..reg_at(a) + 4].try_into().unwrap())).collect();
    let rdram_same = buf[..bytes.len()] == *bytes;
    (ctx, after, rdram_same)
}

fn same_ctx(c: &RecompContext, d: &RecompContext) -> Result<(), TestCaseError> {
    for r in 0..32 {
        prop_assert_eq!(c.gpr[r], d.gpr[r], "${}", difftest::GPR_NAMES[r]);
    }
    for r in 0..32 {
        prop_assert_eq!(c.fpr[r].u64, d.fpr[r].u64, "$f{}", r);
    }
    prop_assert_eq!((c.hi, c.lo), (d.hi, d.lo));
    prop_assert_eq!(c.status_reg, d.status_reg);
    Ok(())
}

/// A table entry the MI mask write accepts: each of the six pairs clear,
/// set or neither.
fn command() -> BoxedStrategy<u16> {
    prop::array::uniform6(0u16..3).prop_map(|p| p.iter().enumerate().map(|(k, &c)| c << (2 * k)).sum()).boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Random masks, Status, global mask and initial MI bits; the ROM's
    /// table, or table entries written (valid commands only).
    #[test]
    fn func_80090500(seed: u64, im: u32, status: u32, global in prop_oneof![Just(0x003F_FF01u32), any::<u32>()], mi in 0u32..64,
                     table in prop_oneof![Just(None), prop::collection::vec(command(), 64).prop_map(Some)], hi: u32) {
        let mut s = State::new();
        s.randomise_registers(seed);
        s.ctx.gpr[A0] = if hi & 1 == 0 { im as i32 as i64 as u64 } else { u64::from(hi) << 32 | u64::from(im) };
        s.ctx.status_reg = status;
        wr(&mut s, GLOBAL_MASK, global);
        let entries = table.unwrap_or_else(rom_table);
        for (k, e) in entries.iter().enumerate() {
            s.rdram.mem().write_u16(IM_TABLE + 2 * k as u32, *e);
        }
        let (c, regs, rdram_same) = run_c("func_80090500", &s, &[(MI_INTR_MASK, mi)]);
        prop_assert!(rdram_same, "the C wrote RDRAM");
        let (_d, model) = hw::install_int_mask(mi);
        let mut d = s.clone();
        d.run(imports::func_80090500);
        same_ctx(&c, &d.ctx)?;
        prop_assert!(d.rdram.as_words() == s.rdram.as_words(), "the double wrote RDRAM");
        prop_assert_eq!(&model.borrow().writes, &vec![regs[0]]);
    }

    #[test]
    fn func_800883E0(seed: u64, len in prop_oneof![Just(0u32), 0u32..1 << 18, Just((1 << 18) - 8)]) {
        let mut s = State::new();
        s.randomise_registers(seed);
        let (c, _, rdram_same) = run_c("func_800883E0", &s, &[(AI_LEN, len)]);
        prop_assert!(rdram_same);
        let _d = hw::install_ai_length(vec![len]);
        let mut d = s.clone();
        d.run(imports::func_800883E0);
        same_ctx(&c, &d.ctx)?;
    }
}

/// The ROM's table holds valid commands only (the double refuses others).
#[test]
fn rom_im_table_is_valid() {
    let mut m = hw::MiMask::default();
    for e in rom_table() {
        m.write(u32::from(e));
    }
}

/// The register model: clear/set pairs, and each run starts again from the
/// injected bits (with no writes).
#[test]
fn mi_mask_model() {
    // SP, AI, PI masked; then pairs (from bit 0): clear SP, set SI, nothing
    // for AI and VI, clear PI, set DP.
    let mut m = hw::MiMask { bits: 0b01_0101, writes: Vec::new() };
    m.write(0b10_01_00_00_10_01);
    assert_eq!(m.bits, 0b10_0110, "SI, AI and DP");
    let (_d, model) = hw::install_int_mask(0b11_0000);
    let mut s = State::new();
    s.ctx.status_reg = 0x2000_0000;
    wr(&mut s, GLOBAL_MASK, 0x003F_FF01);
    for e in 0..64u32 {
        s.rdram.mem().write_u16(IM_TABLE + 2 * e, 0x0555);
    }
    s.ctx.gpr[A0] = 0x003F_FF01;
    let mut first = s.clone();
    first.run(imports::func_80090500);
    assert_eq!(model.borrow().bits, 0, "0x555 clears all six");
    let mut second = s.clone();
    second.run(imports::func_80090500);
    assert_eq!(first.ctx.gpr[V0], second.ctx.gpr[V0], "a new run starts from the injected bits");
    assert_eq!(model.borrow().writes, vec![0x555]);
    assert_eq!(word(&first, GLOBAL_MASK), 0x003F_FF01);
}

/// A write with both bits of a pair set is refused.
#[test]
#[should_panic(expected = "clears and sets")]
fn mi_mask_refuses_both() {
    hw::MiMask::default().write(0b11 << 4);
}
