//! libaudio's event queue at 0x80088B70..0x8008FE60 (game::libultra): the
//! flushes (of one type, of one key, of everything) and adding a player,
//! each inside `osSetIntMask(1)` ... `osSetIntMask(old)`. `osSetIntMask` is
//! a contract double here (difftest::hw); `alUnlink`/`alLink` run as C.
//! Recompiled C vs Rust; the models replay the double's first call and the
//! list callees on a copy of the state in the same order with the same
//! arguments, adding the functions' own stores; whole RDRAM is compared.

// Tests are named after the functions (func_8008FB20), capitals included.
#![allow(non_snake_case)]

use difftest::rom::baserom;
use difftest::{compare, hw, State};
use game::imports;
use game::libultra;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

fn half(s: &State, a: u32) -> u16 {
    let w = word(s, a & !3);
    (if a & 2 == 0 { w >> 16 } else { w }) as u16
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;

fn wr(s: &mut State, a: u32, v: u32) {
    s.rdram.mem().write_u32(a, v);
}

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
}

/// The data segment as the ROM has it: `__OSGlobalIntMask` and the MI
/// command table the double reads.
fn load_data(s: &mut State) {
    let rom = baserom();
    let mut m = s.rdram.mem();
    for va in (0x8009_8000u32..0x800A_E8B0).step_by(4) {
        let o = (va - 0x8000_0400 + 0x1000) as usize;
        m.write_u32(va, u32::from_be_bytes(rom[o..o + 4].try_into().unwrap()));
    }
}

fn state(seed: u64, status: u32) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s.ctx.status_reg = status;
    load_data(&mut s);
    s
}

fn call_c(w: &mut State, frame: u32, args: &[(usize, u64)], f: RecompFn) {
    for &(r, v) in args {
        w.ctx.gpr[r] = v;
    }
    w.ctx.gpr[SP] = sext(SP_AT - frame);
    w.run(f);
}

fn saves(w: &mut State, s: &State, frame: u32, regs: &[(u32, usize)]) {
    for &(off, r) in regs {
        wr(w, SP_AT - frame + off, s.ctx.gpr[r] as u32);
    }
}

const Q: u32 = 0x8030_0000;
const ITEMS: u32 = 0x8030_1000;
const FREE: u32 = 0x8030_2000;

#[derive(Clone, Debug)]
struct Item {
    delta: u32,
    ty: i16,
    key: u32,
}

/// The queue at `Q`: free list `Q + 0` (`free` items from `FREE`), the
/// allocation list `Q + 8` (`items`, from `ITEMS`, 0x20 bytes each), both
/// `{next, prev}` doubly linked, the last `next` 0.
fn put_queue(s: &mut State, items: &[Item], free: usize, count: u32) {
    let link = |s: &mut State, head: u32, nodes: &[u32]| {
        wr(s, head, nodes.first().copied().unwrap_or(0));
        for (i, &n) in nodes.iter().enumerate() {
            wr(s, n, nodes.get(i + 1).copied().unwrap_or(0));
            wr(s, n + 4, if i == 0 { head } else { nodes[i - 1] });
        }
    };
    let alloc: Vec<u32> = (0..items.len() as u32).map(|i| ITEMS + 0x20 * i).collect();
    let freed: Vec<u32> = (0..free as u32).map(|i| FREE + 0x20 * i).collect();
    link(s, Q, &freed);
    wr(s, Q + 4, 0);
    link(s, Q + 8, &alloc);
    wr(s, Q + 0xC, 0);
    wr(s, Q + 0x10, count);
    for (it, &n) in items.iter().zip(&alloc) {
        wr(s, n + 8, it.delta);
        s.rdram.mem().write_u16(n + 0xC, it.ty as u16);
        wr(s, n + 0x10, it.key);
    }
}

fn items() -> BoxedStrategy<Vec<Item>> {
    prop::collection::vec(
        (any::<u32>(), prop_oneof![3 => 0i16..4, 1 => any::<i16>()], prop_oneof![3 => 0x8030_5000u32..0x8030_5004, 1 => any::<u32>()])
            .prop_map(|(delta, ty, key)| Item { delta, ty, key }),
        0..7,
    )
    .boxed()
}

/// The walk all three flushes share, on `w`: `pick(n)` decides; a picked
/// item adds its delta to the next one (if `add`), then is unlinked and
/// linked onto the free list.
fn flush(w: &mut State, frame: u32, add: bool, pick: impl Fn(&State, u32) -> bool) {
    let mut n = word(w, Q + 8);
    while n != 0 {
        let next = word(w, n);
        if pick(w, n) {
            if add && next != 0 {
                let d = word(w, next + 8).wrapping_add(word(w, n + 8));
                wr(w, next + 8, d);
            }
            call_c(w, frame, &[(A0, sext(n))], imports::func_80088020);
            call_c(w, frame, &[(A0, sext(n)), (A1, sext(Q))], imports::func_80088050);
        }
        n = next;
    }
}

/// The double's first call, as the port makes it (`a0 = 1`); its old mask.
fn mask_off(w: &mut State, frame: u32) -> u64 {
    call_c(w, frame, &[(A0, 1)], imports::func_80090500);
    w.ctx.gpr[V0]
}

fn status() -> BoxedStrategy<u32> {
    prop_oneof![Just(0x2000_FF01u32), Just(0x2000_0000u32), any::<u32>()].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Types of either sign, `a1` with any high half (only its low halfword
    /// counts), lists empty to six long, a free list or none.
    #[test]
    fn func_8008FB20(seed: u64, st in status(), mi in 0u32..64, its in items(), free in 0usize..3, a1: u32, ty in prop_oneof![0i16..4, any::<i16>()], hi: u32) {
        let _d = hw::install_int_mask(mi);
        let mut s = state(seed, st);
        put_queue(&mut s, &its, free, its.len() as u32);
        s.ctx.gpr[A0] = sext(Q);
        s.ctx.gpr[A1] = u64::from(hi) << 32 | u64::from(a1 & 0xFFFF_0000 | ty as u16 as u32);
        let mut w = s.clone();
        saves(&mut w, &s, 0x40, &[(0x20, S3), (0x1C, S2), (0x24, RA), (0x18, S1), (0x14, S0), (0x44, A1)]);
        (w.ctx.gpr[S2], w.ctx.gpr[S3]) = (sext(Q), ty as i64 as u64);
        let old = mask_off(&mut w, 0x40);
        wr(&mut w, SP_AT - 0x40 + 0x2C, old as u32);
        flush(&mut w, 0x40, true, |w, n| half(w, n + 0xC) as i16 == ty);
        let after = run("func_8008FB20", libultra::func_8008FB20, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_8008FBCC(seed: u64, st in status(), mi in 0u32..64, its in items(), free in 0usize..3) {
        let _d = hw::install_int_mask(mi);
        let mut s = state(seed, st);
        put_queue(&mut s, &its, free, its.len() as u32);
        s.ctx.gpr[A0] = sext(Q);
        let mut w = s.clone();
        saves(&mut w, &s, 0x38, &[(0x20, S2), (0x24, RA), (0x1C, S1), (0x18, S0)]);
        w.ctx.gpr[S2] = sext(Q);
        let old = mask_off(&mut w, 0x38);
        wr(&mut w, SP_AT - 0x38 + 0x2C, old as u32);
        flush(&mut w, 0x38, false, |_, _| true);
        let after = run("func_8008FBCC", libultra::func_8008FBCC, &s)?;
        same_memory(&after, &w)?;
    }

    /// The key in `s3` canonical or not (a whole-register compare), keys
    /// that match some items.
    #[test]
    fn func_80088B70(seed: u64, st in status(), mi in 0u32..64, its in items(), free in 0usize..3,
                     key in prop_oneof![(0x8030_5000u32..0x8030_5004).prop_map(sext), any::<u32>().prop_map(sext), any::<u64>()]) {
        let _d = hw::install_int_mask(mi);
        let mut s = state(seed, st);
        put_queue(&mut s, &its, free, its.len() as u32);
        (s.ctx.gpr[S2], s.ctx.gpr[S3]) = (sext(Q), key);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        mask_off(&mut w, 0x18);
        flush(&mut w, 0x18, true, |w, n| sext(word(w, n + 0x10)) == key);
        let after = run("func_80088B70", libultra::func_80088B70, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_8008FE60(seed: u64, st in status(), mi in 0u32..64, words: [u32; 4], same: bool) {
        let _d = hw::install_int_mask(mi);
        let mut s = state(seed, st);
        let (syn, p) = (0x8030_3000u32, if same { 0x8030_3000u32 } else { 0x8030_4000 });
        wr(&mut s, syn, words[0]);
        wr(&mut s, syn + 0x20, words[1]);
        wr(&mut s, p, words[2]);
        wr(&mut s, p + 0x10, words[3]);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(syn), sext(p));
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA), (0x18, A0), (0x1C, A1)]);
        mask_off(&mut w, 0x18);
        let v = word(&w, syn + 0x20);
        wr(&mut w, p + 0x10, v);
        let v = word(&w, syn);
        wr(&mut w, p, v);
        wr(&mut w, syn, p);
        let after = run("func_8008FE60", libultra::func_8008FE60, &s)?;
        same_memory(&after, &w)?;
    }
}
