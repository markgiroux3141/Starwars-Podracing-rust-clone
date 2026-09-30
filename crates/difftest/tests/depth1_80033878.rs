//! Depth 1 at 0x80033878..0x80038D5C: node placement and heading, the 4x3
//! matrix stack's reset and matrices, a level by thresholds and a
//! projection append (game::misc), a list setter (game::anim), and a
//! material's render state and texture load (game::render). Recompiled C
//! vs Rust with the callees as C. The models replay the callees' C on a
//! copy of the state in the same order with the same arguments, adding
//! the functions' own stores, and whole RDRAM is compared.

// Tests are named after the functions (func_80033878), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::imports;
use game::misc::{MTX43_DEPTH, MTX43_STACK};
use game::recomp::{reg::*, RecompFn};
use game::render::DL2_HEAD;
use game::{anim, misc, render};
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
/// the ROM image has it: the callees read thresholds and series constants
/// from there.
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

/// A whole FPR saved with `sdc1` (high word first).
fn save_fpr(w: &mut State, s: &State, at: u32, f: usize) {
    let v = s.ctx.fpr[f].u64;
    wr(w, at, (v >> 32) as u32);
    wr(w, at + 4, v as u32);
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
fn sub(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? - ok(b)?)
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

// ---- misc: nodes ----

const PAIR: u32 = 0x8030_0000;
const NODE_A: u32 = 0x8030_0100;
const NODE_B: u32 = 0x8030_0200;
const VEC: u32 = 0x8030_0300;
const Q: u32 = 0x8030_0400;

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

fn transform() -> BoxedStrategy<[f32; 12]> {
    prop::array::uniform12(ordinary()).boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80033878(seed: u64, ta in transform(), tb in transform(), v in prop::array::uniform3(ordinary()),
                     null in prop_oneof![3 => Just(None), 1 => (0usize..3).prop_map(Some)]) {
        let mut s = state(seed);
        set_transform(&mut s, NODE_A, &ta);
        set_transform(&mut s, NODE_B, &tb);
        for c in 0..3 {
            wf(&mut s, VEC + 4 * c as u32, v[c]);
        }
        wr(&mut s, PAIR, if null == Some(1) { 0 } else { NODE_A });
        wr(&mut s, PAIR + 4, if null == Some(2) { 0 } else { NODE_B });
        let pair = if null == Some(0) { 0 } else { PAIR };
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(pair), sext(VEC));
        let mut w = s.clone();
        let sp = SP_AT - 0xE8;
        saves(&mut w, &s, 0xE8, &[(0x18, S0), (0x1C, RA)]);
        wr(&mut w, SP_AT + 4, VEC);
        if pair != 0 {
            let (a, b) = (word(&s, PAIR), word(&s, PAIR + 4));
            wr(&mut w, sp + 0x20, b);
            if a != 0 {
                if b != 0 {
                    prop_assume!(madd(row(&s, NODE_A, 1), -row(&s, NODE_B, 3)[1], v).is_some());
                }
                call_c(&mut w, 0xE8, &[(A0, sext(a)), (A1, sext(sp + 0x68))], imports::func_80017C18);
                call_c(&mut w, 0xE8, &[(A0, sext(sp + 0xA8)), (A1, sext(sp + 0x68))], imports::func_800156DC);
                call_c(&mut w, 0xE8, &[(A0, sext(sp + 0xD8)), (A1, sext(VEC))], imports::func_80015288);
                if b != 0 {
                    call_c(&mut w, 0xE8, &[(A0, sext(b)), (A1, sext(sp + 0x28))], imports::func_80017C18);
                    let k = -rf(&w, sp + 0x5C);
                    call_c(&mut w, 0xE8, &[(A0, sext(sp + 0xD8)), (A1, sext(sp + 0xD8)), (A2, sext(k.to_bits())), (A3, sext(sp + 0xB8))], imports::func_800155EC);
                }
                call_c(&mut w, 0xE8, &[(A0, sext(a)), (A1, sext(sp + 0xA8))], imports::func_80017BA8);
            }
        }
        let after = run("func_80033878", misc::func_80033878, &s)?;
        same_memory(&after, &w)?;
    }

    /// Each null in turn; else the game's atan2 of `(-N[1][0], N[1][1])`.
    #[test]
    fn func_80033BCC(seed: u64, t in transform(), null in prop_oneof![3 => Just(None), 1 => (0usize..5).prop_map(Some)]) {
        let mut s = state(seed);
        load_data(&mut s);
        set_transform(&mut s, NODE_A, &t);
        wr(&mut s, PAIR + 4, if null == Some(3) { 0 } else { 1 });
        wr(&mut s, PAIR + 8, if null == Some(4) { 0 } else { NODE_A });
        wr(&mut s, Q, if null == Some(2) { 0 } else { 5 });
        let p = if null == Some(0) { 0 } else { PAIR };
        let q = if null == Some(1) { 0 } else { Q };
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(p), sext(q));
        let mut w = s.clone();
        let sp = SP_AT - 0x60;
        saves(&mut w, &s, 0x60, &[(0x14, RA)]);
        let go = q != 0 && word(&s, q) != 0 && p != 0 && word(&s, p + 4) != 0 && word(&s, p + 8) != 0;
        let f0 = if go {
            call_c(&mut w, 0x60, &[(A0, sext(NODE_A)), (A1, sext(sp + 0x18))], imports::func_80017C18);
            let (y, x) = (-rf(&w, sp + 0x28), rf(&w, sp + 0x2C));
            w.ctx.fpr[12].set_fl(y);
            w.ctx.fpr[14].set_fl(x);
            call_c(&mut w, 0x60, &[], imports::func_80014F54);
            w.ctx.fpr[0].u32l()
        } else {
            0
        };
        let after = run("func_80033BCC", misc::func_80033BCC, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), f0);
        same_memory(&after, &w)?;
    }
}

// ---- anim: a list setter ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80033928(seed: u64, n in 0usize..5, null: bool, x: u32, hi: u32) {
        let mut s = state(seed);
        let list = 0x8030_0000u32;
        for k in 0..n {
            wr(&mut s, list + 4 * k as u32, 0x8031_0000 + 0x200 * k as u32);
        }
        wr(&mut s, list + 4 * n as u32, 0);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (if null { 0 } else { sext(list) }, u64::from(hi) << 32 | u64::from(x));
        let mut w = s.clone();
        let sp = SP_AT - 0x28;
        save_fpr(&mut w, &s, sp + 0x10, 20);
        saves(&mut w, &s, 0x28, &[(0x1C, S0), (0x20, S1), (0x24, RA)]);
        if !null {
            for k in 0..n {
                call_c(&mut w, 0x28, &[(A0, sext(0x8031_0000 + 0x200 * k as u32)), (A1, sext(x))], imports::func_80006EB4);
            }
        }
        let after = run("func_80033928", anim::func_80033928, &s)?;
        prop_assert_eq!(after.ctx.fpr[20].u64, s.ctx.fpr[20].u64);
        same_memory(&after, &w)?;
    }
}

// ---- misc: the 4x3 stack ----

const RING_INDEX: u32 = 0x800A_3CC4;
const VIEW: u32 = 0x8011_2E20;
const DL2: u32 = 0x8038_0000;

fn matrix() -> BoxedStrategy<[f32; 16]> {
    prop::array::uniform16(ordinary()).boxed()
}

fn ring_index() -> BoxedStrategy<u32> {
    prop_oneof![Just(0u32), Just(3071u32), Just(3072u32), 0u32..3072].boxed()
}

fn next_entry(w: &mut State, frame: u32) -> u32 {
    call_c(w, frame, &[], imports::func_80033E08);
    w.ctx.gpr[V0] as u32
}

/// Appends a command to the list at DL2_HEAD as the code does.
fn append(w: &mut State, words: [u32; 2]) {
    let head = word(w, DL2_HEAD);
    wr(w, DL2_HEAD, head.wrapping_add(8));
    wr(w, head, words[0]);
    wr(w, head + 4, words[1]);
}

/// `P = p × V`, `p` as a 4x4 with column 3 = (0, 0, 0, 1), in the code's
/// order of operations.
fn product(p: &[f32; 12], v: &[f32; 16]) -> Option<[f32; 16]> {
    let mut out = [0.0; 16];
    for r in 0..4 {
        for c in 0..4 {
            let s = add(add(mul(p[3 * r], v[c])?, mul(p[3 * r + 1], v[4 + c])?)?, mul(p[3 * r + 2], v[8 + c])?)?;
            out[4 * r + c] = if r == 3 { add(v[12 + c], s)? } else { s };
        }
    }
    Some(out)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_8003483C(seed: u64, depth in 0u32..8) {
        let mut s = state(seed);
        wr(&mut s, MTX43_DEPTH, depth);
        let mut w = s.clone();
        save_fpr(&mut w, &s, SP_AT - 0x20 + 0x10, 20);
        saves(&mut w, &s, 0x20, &[(0x1C, RA)]);
        wr(&mut w, MTX43_DEPTH, 0);
        wr(&mut w, 0x800A_3FF4, 1);
        wr(&mut w, 0x800A_3FF8, 0);
        let one = sext(0x3F80_0000);
        for (r, v) in [[one, 0, 0], [0, one, 0], [0, 0, one], [0, 0, 0]].into_iter().enumerate() {
            call_c(&mut w, 0x20, &[(A0, sext(MTX43_STACK + 12 * r as u32)), (A1, v[0]), (A2, v[1]), (A3, v[2])], imports::func_80015268);
        }
        let after = run("func_8003483C", misc::func_8003483C, &s)?;
        prop_assert_eq!(after.ctx.fpr[20].u64, s.ctx.fpr[20].u64);
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80034948(seed: u64, depth in 0u32..6, p in prop::array::uniform12(ordinary()), m in matrix(), index in ring_index()) {
        let mut s = state(seed);
        load_data(&mut s);
        wr(&mut s, MTX43_DEPTH, depth);
        let at = MTX43_STACK + 48 * depth;
        for (k, &x) in p.iter().enumerate() {
            wf(&mut s, at + 4 * k as u32, x);
        }
        for (k, &x) in m.iter().enumerate() {
            wf(&mut s, 0x8011_2E60 + 4 * k as u32, x);
        }
        wr(&mut s, RING_INDEX, index);
        let mut w = s.clone();
        saves(&mut w, &s, 0x20, &[(0x14, RA)]);
        wr(&mut w, SP_AT - 0x20 + 0x1C, at);
        let e = next_entry(&mut w, 0x20);
        wr(&mut w, 0x8011_34D0, e);
        call_c(&mut w, 0x20, &[(A0, sext(e)), (A1, sext(at))], imports::func_80034650);
        let e = next_entry(&mut w, 0x20);
        wr(&mut w, 0x8011_34D4, e);
        call_c(&mut w, 0x20, &[(A0, sext(e)), (A1, sext(0x8011_2E60))], imports::func_800344F4);
        let after = run("func_80034948", misc::func_80034948, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80034E20(seed: u64, depth in 1u32..6, p in prop::array::uniform12(ordinary()), v in matrix(), index in ring_index(),
                     flag in prop_oneof![Just(0u32), Just(1u32), any::<u32>()], o in prop::array::uniform3(ordinary())) {
        let mut s = state(seed);
        load_data(&mut s);
        wr(&mut s, MTX43_DEPTH, depth);
        let at = MTX43_STACK + 48 * (depth - 1);
        for (k, &x) in p.iter().enumerate() {
            wf(&mut s, at + 4 * k as u32, x);
        }
        for (k, &x) in v.iter().enumerate() {
            wf(&mut s, VIEW + 4 * k as u32, x);
        }
        wr(&mut s, 0x800A_3FEC, flag);
        for c in 0..3 {
            wf(&mut s, 0x800A_3FDC + 4 * c as u32, o[c]);
        }
        wr(&mut s, RING_INDEX, index);
        wr(&mut s, DL2_HEAD, DL2);
        // The model: the offset, the product, the offset back.
        let mut q = p;
        if flag != 0 {
            for c in 0..3 {
                let x = sub(q[9 + c], o[c]);
                prop_assume!(x.is_some());
                q[9 + c] = x.unwrap();
            }
        }
        let prod = product(&q, &v);
        prop_assume!(prod.is_some_and(|m| m.iter().all(|x| !x.is_nan())));
        let prod = prod.unwrap();
        if flag != 0 {
            for c in 0..3 {
                let x = add(o[c], q[9 + c]);
                prop_assume!(x.is_some_and(|x| !x.is_nan()));
                q[9 + c] = x.unwrap();
            }
        }
        prop_assume!(q.iter().all(|x| !x.is_nan()));
        let mut w = s.clone();
        let sp = SP_AT - 0xD0;
        for (k, f) in [20, 22, 24, 26, 28, 30].into_iter().enumerate() {
            save_fpr(&mut w, &s, sp + 0x10 + 8 * k as u32, f);
        }
        saves(&mut w, &s, 0xD0, &[(0x44, RA)]);
        for c in 9..12 {
            wf(&mut w, at + 4 * c as u32, q[c]);
        }
        for (k, &x) in prod.iter().enumerate() {
            wf(&mut w, sp + 0x8C + 4 * k as u32, x);
        }
        wr(&mut w, sp + 0xCC, at);
        // The callees save f20/f22 in their frames: at the calls the port
        // holds V's entries there (low words; the high words untouched).
        for (f, k) in [(20, 10), (22, 2), (24, 6), (26, 11), (28, 3), (30, 7)] {
            w.ctx.fpr[f].set_u32l(v[k].to_bits());
        }
        let e = next_entry(&mut w, 0xD0);
        wr(&mut w, sp + 0x88, e);
        call_c(&mut w, 0xD0, &[(A0, sext(e)), (A1, sext(at))], imports::func_80034650);
        append(&mut w, [0xDA38_0003, e]);
        let e = next_entry(&mut w, 0xD0);
        wr(&mut w, sp + 0x88, e);
        call_c(&mut w, 0xD0, &[(A0, sext(e)), (A1, sext(sp + 0x8C))], imports::func_800344F4);
        append(&mut w, [0xDC38_000E, e]);
        append(&mut w, [0xDB0C_0000, 0x1_0000]);
        let after = run("func_80034E20", misc::func_80034E20, &s)?;
        for f in [20, 22, 24, 26, 28, 30] {
            prop_assert_eq!(after.ctx.fpr[f].u64, s.ctx.fpr[f].u64);
        }
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80038D5C(seed: u64, m in matrix(), index in ring_index()) {
        let mut s = state(seed);
        load_data(&mut s);
        for (k, &x) in m.iter().enumerate() {
            wf(&mut s, VIEW + 4 * k as u32, x);
        }
        wr(&mut s, RING_INDEX, index);
        let pp = 0x8030_0000u32;
        wr(&mut s, pp, DL2);
        s.ctx.gpr[A0] = sext(pp);
        let mut w = s.clone();
        let sp = SP_AT - 0x20;
        saves(&mut w, &s, 0x20, &[(0x14, RA)]);
        wr(&mut w, SP_AT, pp);
        wr(&mut w, sp + 0x18, DL2);
        let e = next_entry(&mut w, 0x20);
        wr(&mut w, sp + 0x1C, e);
        call_c(&mut w, 0x20, &[(A0, sext(e)), (A1, sext(VIEW))], imports::func_800344F4);
        wr(&mut w, DL2, 0xDA38_0007);
        wr(&mut w, DL2 + 4, e);
        wr(&mut w, pp, DL2 + 8);
        let after = run("func_80038D5C", misc::func_80038D5C, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- misc: a level by thresholds ----

fn key() -> BoxedStrategy<f32> {
    prop_oneof![
        3 => 0.0f32..10.0,
        1 => Just(10.0f32),
        1 => Just(-1.0f32),
        1 => 10.0f32..100.0,
        1 => Just(f32::NAN),
        1 => Just(f32::from_bits(0x411F_FFFF)),
        1 => ordinary(),
    ]
    .boxed()
}

/// The count of keys from k[1] before the first that is -1.0 or not below
/// 10.0, at most 7.
fn level(k: &[f32; 8]) -> i32 {
    (1..8).take_while(|&j| k[j] != -1.0 && k[j] < 10.0).count() as i32
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80038294(seed: u64, n in prop_oneof![1 => Just(0i32), 1 => Just(-1i32), 4 => 1i32..9, 1 => any::<i32>()],
                     flag in prop_oneof![1 => Just(0u32), 3 => Just(1u32), 1 => any::<u32>()], keys in prop::array::uniform8(key())) {
        let mut s = state(seed);
        let o = 0x8030_0000u32;
        wr(&mut s, o + 0x14, n as u32);
        wr(&mut s, 0x800A_3FE8, flag);
        for (j, &x) in keys.iter().enumerate() {
            wf(&mut s, o + 0x1C + 4 * j as u32, x);
        }
        s.ctx.gpr[A0] = sext(o);
        let v0 = if n <= 0 {
            -1
        } else if flag == 0 {
            0
        } else if 10.0 < keys[0] {
            -1
        } else {
            let c = level(&keys);
            if c < n { c } else { -1 }
        };
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        wr(&mut w, SP_AT, o);
        let after = run("func_80038294", misc::func_80038294, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(v0 as u32));
        same_memory(&after, &w)?;
    }
}

// ---- render: a material's render state and texture ----

const MAT: u32 = 0x8030_0000;
const CACHE: u32 = 0x8011_2DE4;

fn rgba(s: &State, at: u32) -> u32 {
    u32::from_be_bytes([0, 1, 2, 3].map(|k| byte(s, at + k)))
}

/// The render-state model by the statement (reading `w` as it goes).
fn render_state(w: &mut State, force: bool) {
    let b = |w: &State, k: u32| u32::from(byte(w, MAT + k));
    let same = |w: &State, a: u32, c: u32, n: u32| (0..n).all(|k| byte(w, a + k) == byte(w, c + k));
    if force || half(w, CACHE) != half(w, MAT + 4) {
        append(w, [0xE300_0A01, if half(w, MAT + 4) == 1 { 0 } else { 0x10_0000 }]);
    }
    if force || !(same(w, MAT + 6, CACHE + 2, 8) && same(w, MAT + 0xE, CACHE + 0xA, 8)) {
        let w0 = 0xFC00_0000 | (b(w, 6) & 0xF) << 20 | (b(w, 8) & 0x1F) << 15 | (b(w, 0xA) & 7) << 12 | (b(w, 0xC) & 7) << 9 | (b(w, 0xE) & 0xF) << 5 | (b(w, 0x10) & 0x1F);
        let w1 = b(w, 7) << 28
            | (b(w, 0xF) & 0xF) << 24
            | (b(w, 0x12) & 7) << 21
            | (b(w, 0x14) & 7) << 18
            | (b(w, 9) & 7) << 15
            | (b(w, 0xB) & 7) << 12
            | (b(w, 0xD) & 7) << 9
            | (b(w, 0x11) & 7) << 6
            | (b(w, 0x13) & 7) << 3
            | (b(w, 0x15) & 7);
        append(w, [w0, w1]);
    }
    if force || word(w, CACHE + 0x14) != word(w, MAT + 0x18) || word(w, CACHE + 0x18) != word(w, MAT + 0x1C) {
        let m = word(w, MAT + 0x18) | word(w, MAT + 0x1C);
        append(w, [0xE200_001C, m]);
        append(w, [0xE200_1E01, m & 3]);
    }
    if word(w, MAT) & 1 != 0 && (force || !same(w, MAT + 0x20, 0x8011_2E00, 6)) {
        append(w, [0xFA00_0000 | b(w, 0x20) << 8 | b(w, 0x21), rgba(w, MAT + 0x22)]);
    }
    if word(w, MAT) & 2 != 0 && (force || !same(w, MAT + 0x26, 0x8011_2E06, 4)) {
        append(w, [0xFB00_0000, rgba(w, MAT + 0x26)]);
    }
    if word(w, MAT) & 4 == 0 {
        for k in 0..4 {
            let v = half(w, 0x800A_3D44 + 2 * k) as u8;
            wb(w, MAT + 0x2A + k, v);
        }
    }
    if force || !same(w, MAT + 0x2A, 0x8011_2E0A, 4) {
        append(w, [0xF800_0000, rgba(w, MAT + 0x2A)]);
    }
    if word(w, MAT) & 8 != 0 && (force || !same(w, MAT + 0x2E, 0x8011_2E0E, 4)) {
        append(w, [0xF900_0000, rgba(w, MAT + 0x2E)]);
    }
}

/// Parts of the cached state: (material offset, cache offset, length).
const PARTS: [(u32, u32, u32); 9] = [(4, 0, 2), (6, 2, 8), (0xE, 0xA, 8), (0x18, 0x14, 4), (0x1C, 0x18, 4), (0x20, 0x1C, 6), (0x26, 0x22, 4), (0x2A, 0x26, 4), (0x2E, 0x2A, 4)];

fn material() -> impl Strategy<Value = (Vec<u8>, u32, [bool; 9], [u8; 9], [u16; 4])> {
    (
        proptest::collection::vec(any::<u8>(), 0x32),
        prop_oneof![0u32..16, any::<u32>()],
        prop::array::uniform9(prop_oneof![2 => Just(true), 1 => Just(false)]),
        prop::array::uniform9(any::<u8>()),
        prop::array::uniform4(any::<u16>()),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Each cached part equal (copied) or differing in one byte, the
    /// halfword at `+4` often 1, `force` zero or not.
    #[test]
    fn func_80035BF0(seed: u64, (bytes, flags, equal, which, fog) in material(), force in prop_oneof![3 => Just(0u32), 1 => Just(1u32), 1 => any::<u32>()], one: bool) {
        let mut s = state(seed);
        for (k, &x) in bytes.iter().enumerate() {
            wb(&mut s, MAT + k as u32, x);
        }
        wr(&mut s, MAT, flags);
        if one {
            wh(&mut s, MAT + 4, 1);
        }
        for (k, v) in fog.iter().enumerate() {
            wh(&mut s, 0x800A_3D44 + 2 * k as u32, *v);
        }
        for (i, &(mo, co, n)) in PARTS.iter().enumerate() {
            for k in 0..n {
                let x = byte(&s, MAT + mo + k);
                wb(&mut s, CACHE + co + k, x);
            }
            if !equal[i] {
                let k = u32::from(which[i]) % n;
                let x = byte(&s, CACHE + co + k);
                wb(&mut s, CACHE + co + k, x ^ (1 << (which[i] % 8)));
            }
        }
        wr(&mut s, DL2_HEAD, DL2);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(MAT), sext(force));
        let mut w = s.clone();
        saves(&mut w, &s, 0x20, &[(0x18, S0), (0x1C, RA)]);
        wr(&mut w, SP_AT + 4, force);
        render_state(&mut w, force != 0);
        let after = run("func_80035BF0", render::func_80035BF0, &s)?;
        same_memory(&after, &w)?;
    }
}

const TILES: u32 = 0x8031_0000;

fn texture_model(s: &State, st: (u32, u32)) -> State {
    let mut w = s.clone();
    saves(&mut w, s, 0x28, &[(0x18, S0), (0x1C, S1), (0x20, S2), (0x24, RA)]);
    wr(&mut w, SP_AT + 4, st.0);
    wr(&mut w, SP_AT + 8, st.1);
    let (ss, tt) = (st.0 as i16 as i32 as u32, st.1 as i16 as i32 as u32);
    call_c(&mut w, 0x28, &[(A0, sext(word(s, MAT))), (A1, sext(0x8011_2E14))], imports::func_8003609C);
    let b = |w: &State, k: u32| u32::from(byte(w, MAT + k));
    let h = |w: &State, k: u32| half(w, MAT + k) as i16 as i32 as u32;
    let c = [0xD700_0002 | (b(&w, 0xF) & 7) << 11 | (b(&w, 0xE) & 7) << 8, h(&w, 0x14) << 16 | (h(&w, 0x16) & 0xFFFF)];
    append(&mut w, c);
    if word(&w, MAT) & 0x100 == 0 {
        let fmt = (b(&w, 0xC) & 7) << 21 | if b(&w, 0xD) == 3 { 3 << 19 } else { 2 << 19 };
        let c = [0xFD00_0000 | fmt, word(&w, MAT + 0x38)];
        append(&mut w, c);
        append(&mut w, [0xF500_0000 | fmt, 0x0700_0000]);
        append(&mut w, [0xE600_0000, 0]);
        let n = (h(&w, 0x1A) as i32).min(0x7FF) as u32;
        let c = [0xF300_0000, 0x0700_0000 | (n & 0xFFF) << 12 | (h(&w, 0x18) & 0xFFF)];
        append(&mut w, c);
    }
    let pal = word(&w, MAT + 0x3C);
    if pal != 0 {
        append(&mut w, [0xFD10_0000, pal]);
        append(&mut w, [0xE800_0000, 0]);
        append(&mut w, [0xF500_0100, 0x0700_0000]);
        append(&mut w, [0xE600_0000, 0]);
        let c = [0xF000_0000, if b(&w, 0xD) == 1 { 0x073F_C000 } else { 0x0703_C000 }];
        append(&mut w, c);
    }
    for k in 0..7u32 {
        let e = word(&w, MAT + 0x1C + 4 * k);
        if e == 0 {
            continue;
        }
        let d = |w: &State, j: u32| u32::from(byte(w, e + j));
        let e16 = |w: &State, j: u32| half(w, e + j) as i16 as i32 as u32;
        let w0 = 0xF500_0000 | (b(&w, 0xC) & 7) << 21 | (b(&w, 0xD) & 3) << 19 | d(&w, 2) << 9 | (e16(&w, 0) & 0x1FF);
        let w1 = k << 24 | (d(&w, 3) & 3) << 18 | (d(&w, 5) & 0xF) << 14 | (d(&w, 7) & 0xF) << 10 | ((d(&w, 3) & 0xF0) >> 4 & 3) << 8 | (d(&w, 4) & 0xF) << 4 | (d(&w, 6) & 0xF);
        append(&mut w, [w0, w1]);
        let x = |w: &State, j: u32, o: u32| e16(w, j).wrapping_add(o) & 0xFFF;
        let c = [0xF200_0000 | x(&w, 8, ss) << 12 | x(&w, 0xA, tt), k << 24 | x(&w, 0xC, ss) << 12 | x(&w, 0xE, tt)];
        append(&mut w, c);
    }
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80036314(seed: u64, bytes in proptest::collection::vec(any::<u8>(), 0x40), mode: u32, flags in prop_oneof![Just(0u32), Just(0x100u32), any::<u32>()],
                     d in prop_oneof![Just(1u8), Just(3u8), any::<u8>()], lens in (prop_oneof![0i16..0x900, any::<i16>()], any::<i16>()),
                     palette in prop_oneof![Just(0u32), any::<u32>()], tiles in prop::array::uniform7(any::<bool>()),
                     tile_bytes in proptest::collection::vec(any::<u8>(), 7 * 0x10), st in (any::<u32>(), any::<u32>())) {
        let mut s = state(seed);
        for (k, &x) in bytes.iter().enumerate() {
            wb(&mut s, MAT + k as u32, x);
        }
        wr(&mut s, MAT, flags);
        wb(&mut s, MAT + 0xD, d);
        wh(&mut s, MAT + 0x1A, lens.0 as u16);
        wh(&mut s, MAT + 0x18, lens.1 as u16);
        wr(&mut s, MAT + 0x3C, palette);
        for k in 0..7u32 {
            let e = TILES + 0x20 * k;
            wr(&mut s, MAT + 0x1C + 4 * k, if tiles[k as usize] { e } else { 0 });
            for j in 0..0x10u32 {
                wb(&mut s, e + j, tile_bytes[(0x10 * k + j) as usize]);
            }
        }
        wr(&mut s, 0x8011_2E14, mode);
        wr(&mut s, DL2_HEAD, DL2);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(MAT), sext(st.0), sext(st.1));
        let w = texture_model(&s, st);
        let after = run("func_80036314", render::func_80036314, &s)?;
        same_memory(&after, &w)?;
    }
}
