//! Depth 1 at 0x8002FDF8..0x8003365C: the fade resets, a pod's stats from
//! its parts and two node helpers (game::misc), emptying a pool
//! (game::pools), starting a block's animations (game::anim) and the
//! channel entry starters (game::channels). Recompiled C vs Rust with the
//! callees as C. The models replay the callees' C on a copy of the state
//! in the same order with the same arguments, adding the functions' own
//! stores, and whole RDRAM is compared.

// Tests are named after the functions (func_8002FDF8), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::channels::{CHANNELS, CURRENT, ENTRIES};
use game::imports;
use game::recomp::{reg::*, RecompFn};
use game::{anim, channels, misc, pools};
use proptest::prelude::*;
use std::process::Command;

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

fn byte(s: &State, a: u32) -> u8 {
    (word(s, a & !3) >> (8 * (3 - (a & 3)))) as u8
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

/// The game's data segment (`0x80098000..0x800AE8B0`, ROM `0x98C00..`) as
/// the ROM image has it: the stat update reads its constants from there.
fn load_data(s: &mut State) {
    let rom = baserom();
    let mut m = s.rdram.mem();
    for va in (0x8009_8000u32..0x800A_E8B0).step_by(4) {
        let o = (va - 0x8000_0400 + 0x1000) as usize;
        m.write_u32(va, u32::from_be_bytes(rom[o..o + 4].try_into().unwrap()));
    }
}

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
}

fn wr(s: &mut State, a: u32, v: u32) {
    s.rdram.mem().write_u32(a, v);
}

fn wh(s: &mut State, a: u32, v: u16) {
    s.rdram.mem().write_u16(a, v);
}

fn wb(s: &mut State, a: u32, v: u8) {
    s.rdram.mem().write_u8(a, v);
}

fn wf(s: &mut State, a: u32, v: f32) {
    wr(s, a, v.to_bits());
}

fn rf(s: &State, a: u32) -> f32 {
    f32::from_bits(word(s, a))
}

/// Runs a callee's C on `w` with `sp` at `SP_AT - frame` and `args` set.
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

fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}
fn mul(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? * ok(b)?)
}
fn add(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? + ok(b)?)
}

fn float() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        -1.0e4f32..1.0e4f32,
        -1.0f32..1.0,
        any::<u32>().prop_map(f32::from_bits).prop_filter("finite", |x| x.is_finite()),
    ]
    .boxed()
}

fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![4 => -1.0e3f32..1.0e3f32, 2 => -1.0f32..1.0, 2 => (-3i32..=3).prop_map(|n| n as f32), 1 => float()].boxed()
}

// ---- misc: the fade resets ----

/// Colour -103 = (0, 0, 0, 255), then func_80038DF8(0x3E4, 0x3E8, 0xFF x 4)
/// with the last two on the stack, in a 0x20-byte frame.
fn fade_calls(w: &mut State) {
    let sp = SP_AT - 0x20;
    wr(w, sp + 0x10, 0xFF);
    call_c(w, 0x20, &[(A0, sext(-103i32 as u32)), (A1, 0), (A2, 0), (A3, 0)], imports::func_8000AB24);
    wr(w, sp + 0x10, 0xFF);
    wr(w, sp + 0x14, 0xFF);
    call_c(w, 0x20, &[(A0, 0x3E4), (A1, 0x3E8), (A2, 0xFF), (A3, 0xFF)], imports::func_80038DF8);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn func_8002FDF8(seed: u64) {
        let s = state(seed);
        let mut w = s.clone();
        saves(&mut w, &s, 0x20, &[(0x1C, RA)]);
        let b = 0x800D_6960;
        wr(&mut w, b + 0x18, 0);
        wh(&mut w, b + 0x20, 2);
        for k in 0..6 {
            wr(&mut w, b + 4 * k, 0);
        }
        fade_calls(&mut w);
        let after = run("func_8002FDF8", misc::func_8002FDF8, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), 0);
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80030FA0(seed: u64) {
        let s = state(seed);
        let mut w = s.clone();
        saves(&mut w, &s, 0x20, &[(0x1C, RA)]);
        fade_calls(&mut w);
        let after = run("func_80030FA0", misc::func_80030FA0, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- misc: stats from parts ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80032F2C(seed: u64, stats in prop::array::uniform15(ordinary()),
                     levels in prop::array::uniform7(prop_oneof![4 => 1u8..=5, 1 => Just(0u8), 1 => Just(6u8), 1 => any::<u8>()]),
                     conds in prop::array::uniform7(prop_oneof![Just(0u8), Just(255u8), Just(128u8), any::<u8>()])) {
        let mut s = state(seed);
        load_data(&mut s);
        let (dst, src, lv, cd) = (0x8030_0000u32, 0x8030_0100u32, 0x8030_0200u32, 0x8030_0208u32);
        for (k, &x) in stats.iter().enumerate() {
            wf(&mut s, src + 4 * k as u32, x);
        }
        for i in 0..7u32 {
            wb(&mut s, lv + i, levels[i as usize]);
            wb(&mut s, cd + i, conds[i as usize]);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(dst), sext(src), sext(lv), sext(cd));
        let mut w = s.clone();
        saves(&mut w, &s, 0x38, &[(0x20, S0), (0x24, S1), (0x28, S2), (0x2C, S3), (0x30, S4), (0x34, RA)]);
        let f20 = s.ctx.fpr[20].u64;
        wr(&mut w, SP_AT - 0x38 + 0x18, (f20 >> 32) as u32);
        wr(&mut w, SP_AT - 0x38 + 0x1C, f20 as u32);
        for k in 0..15 {
            let v = word(&s, src + 4 * k);
            wr(&mut w, dst + 4 * k, v);
        }
        for i in 0..7u32 {
            let x = f32::from(byte(&s, cd + i)) / 255.0;
            // The stat update's domain: the field and the result of the
            // level that runs not NaN (the fields are finite; a product of
            // finite values can overflow, so check).
            let field = [0u32, 4, 0xC, 0x10, 0x14, 0x24, 0x2C][i as usize];
            prop_assume!(rf(&w, dst + field).is_finite());
            call_c(&mut w, 0x38, &[(A0, sext(dst)), (A1, u64::from(i)), (A2, u64::from(byte(&s, lv + i))), (A3, sext(x.to_bits()))], imports::func_800321F0);
            prop_assume!(!rf(&w, dst + field).is_nan());
        }
        let after = run("func_80032F2C", misc::func_80032F2C, &s)?;
        prop_assert_eq!(after.ctx.fpr[20].u64, f20);
        same_memory(&after, &w)?;
    }
}

// ---- misc: two node helpers ----

/// A node's 3x4 transform at `+0x1C..+0x4C`, row by row.
fn set_transform(s: &mut State, n: u32, t: &[f32; 12]) {
    for (k, &x) in t.iter().enumerate() {
        wf(s, n + 0x1C + 4 * k as u32, x);
    }
}

fn row(s: &State, n: u32, r: u32) -> [f32; 3] {
    [0, 1, 2].map(|c| rf(s, n + 0x1C + 12 * r + 4 * c))
}

/// `q * k + p` component by component, None for a NaN operand.
fn madd(q: [f32; 3], k: f32, p: [f32; 3]) -> Option<[f32; 3]> {
    let mut out = [0.0; 3];
    for c in 0..3 {
        out[c] = add(mul(q[c], k)?, p[c])?;
    }
    Some(out)
}

const PAIR: u32 = 0x8030_0000;
const NODE_A: u32 = 0x8030_0100;
const NODE_B: u32 = 0x8030_0200;
const OUT: u32 = 0x8030_0300;
const Q: u32 = 0x8030_0400;
const QT: u32 = 0x8030_0500;

fn transform() -> BoxedStrategy<[f32; 12]> {
    prop::array::uniform12(ordinary()).boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80033590(seed: u64, ta in transform(), tb in transform(), null in prop_oneof![3 => Just(None), 1 => (0usize..3).prop_map(Some)]) {
        let mut s = state(seed);
        set_transform(&mut s, NODE_A, &ta);
        set_transform(&mut s, NODE_B, &tb);
        wr(&mut s, PAIR, if null == Some(1) { 0 } else { NODE_A });
        wr(&mut s, PAIR + 4, if null == Some(2) { 0 } else { NODE_B });
        let pair = if null == Some(0) { 0 } else { PAIR };
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(pair), sext(OUT));
        let mut w = s.clone();
        let sp = SP_AT - 0xA8;
        saves(&mut w, &s, 0xA8, &[(0x18, S0), (0x1C, RA)]);
        if pair == 0 || word(&s, PAIR) == 0 {
            call_c(&mut w, 0xA8, &[(A0, sext(OUT)), (A1, 0), (A2, 0), (A3, 0)], imports::func_80015268);
        } else {
            let b = word(&s, PAIR + 4);
            if b != 0 {
                prop_assume!(madd(row(&s, NODE_A, 1), row(&s, NODE_B, 3)[1], row(&s, NODE_A, 3)).is_some());
            }
            wr(&mut w, sp + 0x20, b);
            call_c(&mut w, 0xA8, &[(A0, sext(NODE_A)), (A1, sext(sp + 0x68))], imports::func_80017C18);
            for c in 0..3 {
                let v = word(&w, sp + 0x98 + 4 * c);
                wr(&mut w, OUT + 4 * c, v);
            }
            if b != 0 {
                call_c(&mut w, 0xA8, &[(A0, sext(b)), (A1, sext(sp + 0x28))], imports::func_80017C18);
                let k = word(&w, sp + 0x5C);
                call_c(&mut w, 0xA8, &[(A0, sext(OUT)), (A1, sext(OUT)), (A2, sext(k)), (A3, sext(sp + 0x78))], imports::func_800155EC);
            }
        }
        let after = run("func_80033590", misc::func_80033590, &s)?;
        same_memory(&after, &w)?;
    }

    /// Each null in turn; the flag bit, and `d.y` on both sides of `±lim`
    /// (ties included: integers make `d.y == lim` common).
    #[test]
    fn func_8003365C(seed: u64, ta in transform(), tb in transform(), v in prop::array::uniform3(ordinary()),
                     null in prop_oneof![4 => Just(None), 1 => (0usize..5).prop_map(Some)],
                     flags in prop_oneof![Just(0u32), Just(1u32 << 30), Just(1u32 << 31), any::<u32>()],
                     lim in prop_oneof![ordinary(), (0i32..4).prop_map(|n| n as f32), Just(-0.0f32), Just(f32::INFINITY)],
                     tie in prop_oneof![2 => Just(None), 1 => (any::<bool>(), -3i8..=3, 0u8..4).prop_map(Some)]) {
        let (mut tb, mut v, mut lim) = (tb, v, lim);
        // d.y exactly -lim or +lim: integers, so the subtraction is exact.
        if let Some((below, y, l)) = tie {
            (tb[10], lim) = (f32::from(y), f32::from(l));
            v[1] = if below { tb[10] + lim } else { tb[10] - lim };
        }
        let mut s = state(seed);
        set_transform(&mut s, NODE_A, &ta);
        set_transform(&mut s, NODE_B, &tb);
        for c in 0..3 {
            wf(&mut s, OUT + 4 * c, v[c as usize]);
        }
        wr(&mut s, PAIR, if null == Some(3) { 0 } else { NODE_A });
        wr(&mut s, PAIR + 4, if null == Some(4) { 0 } else { NODE_B });
        wr(&mut s, Q, if null == Some(2) { 0 } else { QT });
        wr(&mut s, QT + 0x100, flags);
        let pair = if null == Some(0) { 0 } else { PAIR };
        let q = if null == Some(1) { 0 } else { Q };
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(pair), sext(q), sext(OUT), sext(lim.to_bits()));
        let mut w = s.clone();
        let sp = SP_AT - 0xB8;
        saves(&mut w, &s, 0xB8, &[(0x14, RA)]);
        wr(&mut w, SP_AT + 4, q);
        wr(&mut w, SP_AT + 8, OUT);
        wr(&mut w, SP_AT + 0xC, lim.to_bits());
        let go = pair != 0 && q != 0 && word(&s, q) != 0 && word(&s, pair) != 0 && word(&s, pair + 4) != 0;
        if go {
            // The domain, from the inputs: B[3] and v, then lim (bit clear)
            // and the move.
            let bt = row(&s, NODE_B, 3);
            prop_assume!(bt.iter().chain(&v).all(|x| !x.is_nan()));
            let dy = bt[1] - v[1];
            let bit = flags & (1 << 30) != 0;
            prop_assume!(bit || !lim.is_nan());
            let moves = bit || dy < -lim || lim < dy;
            if moves {
                prop_assume!(!dy.is_nan() && madd(row(&s, NODE_A, 1), -dy, row(&s, NODE_A, 3)).is_some());
            }
            wr(&mut w, sp + 0x30, NODE_B);
            wr(&mut w, sp + 0x34, NODE_A);
            call_c(&mut w, 0xB8, &[(A0, sext(NODE_A)), (A1, sext(sp + 0x78))], imports::func_80017C18);
            call_c(&mut w, 0xB8, &[(A0, sext(NODE_B)), (A1, sext(sp + 0x38))], imports::func_80017C18);
            for c in 0..3 {
                let x = word(&w, sp + 0x68 + 4 * c);
                wr(&mut w, sp + 0x18 + 4 * c, x);
            }
            call_c(&mut w, 0xB8, &[(A0, sext(sp + 0x24)), (A1, sext(sp + 0x18)), (A2, sext(OUT))], imports::func_8001535C);
            for c in 0..3 {
                let x = word(&w, sp + 0x18 + 4 * c);
                wr(&mut w, OUT + 4 * c, x);
            }
            prop_assert_eq!(rf(&w, sp + 0x28).to_bits(), dy.to_bits());
            if moves {
                call_c(&mut w, 0xB8, &[(A0, sext(sp + 0xA8)), (A1, sext(sp + 0xA8)), (A2, sext((-dy).to_bits())), (A3, sext(sp + 0x88))], imports::func_800155EC);
                call_c(&mut w, 0xB8, &[(A0, sext(NODE_A)), (A1, sext(sp + 0x78))], imports::func_80017BA8);
            }
        }
        let after = run("func_8003365C", misc::func_8003365C, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- pools: emptying a pool ----

const POOL_LIST: u32 = 0x8030_0000;
const POOL_DESCS: u32 = 0x8030_0100;

/// Pools with the given ids, counts 1..4, element size 0x68.
fn pool_registry(s: &mut State, seed: u64, ids: &[u32]) {
    s.randomise_memory(seed ^ 2, POOL_LIST, 0x400);
    let mut m = s.rdram.mem();
    m.write_u32(0x800A_2170, POOL_LIST);
    for (k, &id) in ids.iter().enumerate() {
        let d = POOL_DESCS + 0x40 * k as u32;
        m.write_u32(POOL_LIST + 4 * k as u32, d);
        m.write_u32(d, id);
        m.write_u32(d + 8, 1 + k as u32);
        m.write_u32(d + 0xC, 0x68);
    }
    m.write_u32(POOL_LIST + 4 * ids.len() as u32, 0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80030304(seed: u64, ids in proptest::collection::vec(1u32..4, 0..4), id in 0u32..5) {
        let mut s = state(seed);
        pool_registry(&mut s, seed, &ids);
        s.ctx.gpr[A0] = u64::from(id);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        call_c(&mut w, 0x18, &[(A0, u64::from(id)), (A1, 0), (A2, 0)], imports::func_8003FB78);
        let after = run("func_80030304", pools::func_80030304, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], 0);
        same_memory(&after, &w)?;
    }
}

// ---- anim: starting a block's animations ----

const BLOCK: u32 = 0x8032_0000;
const OBJS: u32 = 0x8033_0000;
const TARGETS: u32 = 0x8034_0000;
const DATA: u32 = 0x4461_7461;
const ANIM: u32 = 0x416E_696D;

#[derive(Clone, Debug)]
struct Block {
    lead: Vec<u32>,
    data: Option<(i32, Vec<u32>)>,
    anim: Option<Vec<usize>>,
    other: u32,
}

fn block() -> impl Strategy<Value = Block> {
    let word = any::<u32>().prop_filter("not -1", |&w| w != u32::MAX);
    (
        proptest::collection::vec(word.clone(), 0..4),
        prop::option::of((prop_oneof![3 => 0i32..4, 1 => -2i32..0], proptest::collection::vec(word, 4))),
        prop_oneof![3 => proptest::collection::vec(0usize..6, 0..5).prop_map(Some), 1 => Just(None)],
        any::<u32>(),
    )
        .prop_map(|(lead, data, anim, other)| Block { lead, data, anim, other })
}

/// Lays the block out at BLOCK: the lead words, -1, the optional "Data"
/// chunk (count `n`, then `max(n, 0)` words), then "Anim" and its list of
/// objects (0-terminated), or another word.
fn write_block(s: &mut State, b: &Block) {
    let mut words = b.lead.clone();
    words.push(u32::MAX);
    if let Some((n, body)) = &b.data {
        words.push(DATA);
        words.push(*n as u32);
        words.extend(&body[..(*n).max(0) as usize]);
    }
    match &b.anim {
        Some(list) => {
            words.push(ANIM);
            words.extend(list.iter().map(|&k| OBJS + 0x200 * k as u32));
            words.push(0);
        }
        None => words.push(if b.other == ANIM { 0 } else { b.other }),
    }
    for (k, &x) in words.iter().enumerate() {
        wr(s, BLOCK + 4 * k as u32, x);
    }
}

/// Six objects: flag words from the strategy (kind 8 with a target or a
/// null target among them).
fn write_objects(s: &mut State, flags: &[u32; 6], targets: &[bool; 6]) {
    for k in 0..6u32 {
        let o = OBJS + 0x200 * k;
        wr(s, o + 0x100, flags[k as usize]);
        wr(s, o + 0x124, if targets[k as usize] { TARGETS + 0x20 * k } else { 0 });
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80030964(seed: u64, b in block(), count in 0u32..8,
                     flags in prop::array::uniform6(prop_oneof![Just(8u32), Just(0x28u32), any::<u32>()]), targets: [bool; 6]) {
        let mut s = state(seed);
        write_block(&mut s, &b);
        write_objects(&mut s, &flags, &targets);
        wr(&mut s, 0x8009_A2A0, count);
        s.ctx.gpr[A0] = sext(BLOCK);
        let mut w = s.clone();
        let sp = SP_AT - 0x28;
        saves(&mut w, &s, 0x28, &[(0x14, S0), (0x18, S1), (0x1C, RA)]);
        let mut v = BLOCK;
        while word(&w, v) != u32::MAX {
            v += 4;
        }
        v += 4;
        if word(&w, v) == DATA {
            let n = word(&w, v + 4) as i32;
            v += 8;
            if n > 0 {
                v += 4 * n as u32;
            }
        }
        let mut result = 0;
        let mut lo = u32::MAX;
        if word(&w, v) == ANIM {
            result = v + 4;
            wr(&mut w, sp + 0x20, result);
            let mut e = result;
            let mut o = word(&w, e);
            while o != 0 {
                call_c(&mut w, 0x28, &[(A0, sext(o))], imports::func_80005BB8);
                lo = lo.min(word(&w, e));
                e += 4;
                o = word(&w, e);
            }
        } else {
            wr(&mut w, sp + 0x20, 0);
        }
        if lo != u32::MAX {
            let d = word(&w, 0x800D_9DC4).wrapping_sub(lo);
            wr(&mut w, 0x800D_9DD0, d);
            let x = word(&w, 0x800D_9DC8).wrapping_sub(d);
            wr(&mut w, 0x800D_9DC8, x);
        } else {
            wr(&mut w, 0x800D_9DD0, 0);
        }
        let after = run("func_80030964", anim::func_80030964, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(result));
        same_memory(&after, &w)?;
    }
}

// ---- channels: starting and stopping entries ----

/// Sets up channel `i`'s current entry and state and entry `k`'s words.
fn channel_state(seed: u64, i: u32, k: i32, cur: i32, e4: u32, stopped: bool) -> State {
    let mut s = state(seed);
    wr(&mut s, CURRENT + 4 * i, cur as u32);
    wr(&mut s, ENTRIES.wrapping_add((12 * k) as u32) + 4, e4);
    wh(&mut s, CHANNELS + 28 * i + 8, if stopped { 0 } else { 1 + (seed as u16 & 0x7FFE) });
    (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (u64::from(i), sext(k as u32));
    s
}

/// The starter by the statement: `value` None takes the entry's halfword.
fn start_model(s: &State, i: u32, k: i32, value: Option<u32>) -> State {
    let mut w = s.clone();
    saves(&mut w, s, 0x20, &[(0x18, S0), (0x1C, RA)]);
    if let Some(v) = value {
        wr(&mut w, SP_AT + 8, v);
    }
    let cur = word(s, CURRENT + 4 * i) as i32;
    let e = ENTRIES.wrapping_add((12 * k) as u32);
    let busy = word(s, e + 4) != 0 || half(s, CHANNELS + 28 * i + 8) != 0;
    if k > cur || (k == cur && !busy) {
        wr(&mut w, CURRENT + 4 * i, k as u32);
        let h = value.map_or(half(s, e), |v| v as u16);
        call_c(&mut w, 0x20, &[(A0, u64::from(i)), (A1, u64::from(h)), (A2, sext(word(s, e + 4))), (A3, sext(word(s, e + 8)))], imports::func_800314DC);
    }
    w
}

fn entry_case() -> impl Strategy<Value = (u32, i32, i32, u32, bool)> {
    (0u32..4, -2i32..13, prop_oneof![Just(-1i32), Just(0), Just(1)], prop_oneof![Just(0u32), any::<u32>()], any::<bool>())
        .prop_map(|(i, k, d, e4, stopped)| (i, k, k.wrapping_add(d), e4, stopped))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_800319F4(seed: u64, (i, k, cur, e4, stopped) in entry_case(), far in prop_oneof![4 => Just(None), 1 => any::<i32>().prop_map(Some)]) {
        let cur = far.unwrap_or(cur);
        let s = channel_state(seed, i, k, cur, e4, stopped);
        let w = start_model(&s, i, k, None);
        let after = run("func_800319F4", channels::func_800319F4, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80031AB0(seed: u64, (i, k, cur, e4, stopped) in entry_case(), value: u32) {
        let mut s = channel_state(seed, i, k, cur, e4, stopped);
        s.ctx.gpr[A2] = sext(value);
        let w = start_model(&s, i, k, Some(value));
        let after = run("func_80031AB0", channels::func_80031AB0, &s)?;
        same_memory(&after, &w)?;
    }

    /// Channels 0..3 and -1 (QUIRK: the store lands before CURRENT).
    #[test]
    fn func_80031BBC(seed: u64, i in -1i32..4) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = sext(i as u32);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        wr(&mut w, CURRENT.wrapping_add((4 * i) as u32), u32::MAX);
        call_c(&mut w, 0x18, &[(A0, sext(i as u32))], imports::func_80031640);
        let after = run("func_80031BBC", channels::func_80031BBC, &s)?;
        same_memory(&after, &w)?;
    }
}

const CHILD_ENV: &str = "DEPTH1_8002FDF8_CHILD";
const PAUSE: &str = "pause_self: branch-to-self idle loop";

/// In the child: one side of a starter with `k = 13`. Reaching the end
/// means it returned instead of hanging.
#[test]
fn hang_child() {
    let Ok(which) = std::env::var(CHILD_ENV) else { return };
    let (side, name) = which.split_once(':').unwrap();
    let mut s = channel_state(7, 1, 13, 0, 0, true);
    s.ctx.gpr[A2] = 5;
    let port: RecompFn = if name == "func_800319F4" { channels::func_800319F4 } else { channels::func_80031AB0 };
    let f = match side {
        "c" => oracle::recomp::by_name(name).unwrap(),
        _ => difftest::port_under_test(name, port),
    };
    s.run(f);
    std::process::exit(0);
}

/// QUIRK: `k >= 13` hangs, in the C and in the ports alike.
#[test]
fn func_800319F4_and_func_80031AB0_hang_from_13() {
    for name in ["func_800319F4", "func_80031AB0"] {
        for side in ["c", "rust"] {
            let out = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "hang_child", "--nocapture", "--test-threads=1"])
                .env(CHILD_ENV, format!("{side}:{name}"))
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(!out.status.success(), "{side} {name}: returned instead of hanging");
            assert!(stderr.contains(PAUSE), "{side} {name}: unexpected stderr:\n{stderr}");
        }
    }
}
