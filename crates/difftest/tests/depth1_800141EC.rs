//! Depth 1 at 0x800141EC..0x80028070: the glyph setup (game::render),
//! degree trig, normalising, the 4x4 inverse through the LU pair
//! (game::math), a node's world matrix and flag walk (game::misc), and the
//! profile helpers (game::save). Recompiled C vs Rust with the callees as
//! C. Where a function hands its work to callees, the model replays the
//! callees' C on a copy of the state in the same order with the same
//! arguments (their statements live in their own tests), adding the
//! function's own stores; whole RDRAM (or the whole state) is compared.

// Tests are named after the functions (func_800141EC), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, diff_states, rom::baserom, State};
use game::imports;
use game::recomp::{reg::*, RecompFn};
use game::render::DL_HEAD;
use game::{math, misc, render, save};
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
/// the ROM image has it: the test background is random, and these
/// functions and their callees read constants (pi, 180, the asin series,
/// thresholds) from there.
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

fn same_state(after: &State, want: &State) -> Result<(), TestCaseError> {
    match diff_states(want, after).first() {
        None => Ok(()),
        Some(d) => Err(TestCaseError::fail(format!("model vs port: {d}"))),
    }
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

fn saves(want: &mut State, s: &State, frame: u32, regs: &[(u32, usize)]) {
    for &(off, r) in regs {
        wr(want, SP_AT - frame + off, s.ctx.gpr[r] as u32);
    }
}

/// Runs a callee's C on `w` with `sp` at `SP_AT - frame` and `args` set.
fn call_c(w: &mut State, frame: u32, args: &[(usize, u64)], f: RecompFn) {
    for &(r, v) in args {
        w.ctx.gpr[r] = v;
    }
    w.ctx.gpr[SP] = sext(SP_AT - frame);
    w.run(f);
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
fn div(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? / ok(b)?)
}
fn neg(a: f32) -> Option<f32> {
    Some(-ok(a)?)
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

// ---- math: degree trig and normalising ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80014CC0(seed: u64, deg in prop_oneof![3 => -720.0f32..720.0, 1 => (-8i32..8).prop_map(|n| 45.0 * n as f32), 1 => ordinary()]) {
        let mut s = state(seed);
        load_data(&mut s);
        let (sp_, cp) = (0x8030_0000u32, 0x8030_0004u32);
        s.ctx.fpr[12].set_fl(deg);
        (s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(sp_), sext(cp));
        let (p, d) = [0x800A_8780u32, 0x800A_8788].map(|a| f64::from_bits(u64::from(word(&s, a)) << 32 | u64::from(word(&s, a + 4)))).into();
        let r = ((f64::from(deg) * p) / d) as f32;
        prop_assume!(r.is_finite());
        let mut w = s.clone();
        saves(&mut w, &s, 0x20, &[(0x14, RA), (0x24, A1), (0x28, A2)]);
        wf(&mut w, SP_AT - 0x20 + 0x18, r);
        w.ctx.fpr[12].set_fl(r);
        call_c(&mut w, 0x20, &[], imports::func_8008A750);
        let c = w.ctx.fpr[0].u32l();
        wr(&mut w, cp, c);
        w.ctx.fpr[12].set_fl(r);
        call_c(&mut w, 0x20, &[], imports::func_8008A8C0);
        let sn = w.ctx.fpr[0].u32l();
        wr(&mut w, sp_, sn);
        let after = run("func_80014CC0", math::func_80014CC0, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80014F2C(seed: u64, x in prop_oneof![3 => -1.0f32..=1.0, 1 => prop::sample::select(vec![0.0f32, -0.0, 1.0, -1.0, 0.5, -0.5])]) {
        let mut s = state(seed);
        load_data(&mut s);
        s.ctx.fpr[12].set_fl(x);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        call_c(&mut w, 0x18, &[], imports::func_80014D4C);
        let a = w.ctx.fpr[0].fl();
        prop_assume!(!a.is_nan());
        w.ctx.gpr[RA] = sext(s.ctx.gpr[RA] as u32);
        w.ctx.gpr[SP] = sext(SP_AT);
        w.ctx.gpr[AT] = sext(0x42B4_0000);
        w.ctx.fpr[4].set_u32l(0x42B4_0000);
        w.ctx.fpr[0].set_fl(90.0 - a);
        let after = run("func_80014F2C", math::func_80014F2C, &s)?;
        same_state(&after, &w)?;
    }

    #[test]
    fn func_800151E0(seed: u64, v in prop::array::uniform2(ordinary()), k in prop_oneof![3 => Just(None), 1 => ordinary().prop_map(Some)], tie: bool) {
        normalise(seed, &v, k, tie, 0x800A_87F0, imports::func_800151C0, math::func_800151E0, "func_800151E0")?;
    }

    #[test]
    fn func_800154D0(seed: u64, v in prop::array::uniform3(ordinary()), k in prop_oneof![3 => Just(None), 1 => ordinary().prop_map(Some)], tie: bool) {
        normalise(seed, &v, k, tie, 0x800A_87F4, imports::func_800153C0, math::func_800154D0, "func_800154D0")?;
    }
}

/// The normalise pair: the length's C, then the port's compare and
/// quotients; `tie` sets `K` to the length itself.
#[allow(clippy::too_many_arguments)]
fn normalise(seed: u64, v: &[f32], k: Option<f32>, tie: bool, k_at: u32, len: RecompFn, port: RecompFn, name: &str) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    load_data(&mut s);
    let at = 0x8030_0000u32;
    for (i, x) in v.iter().enumerate() {
        wf(&mut s, at + 4 * i as u32, *x);
    }
    s.ctx.gpr[A0] = sext(at);
    // The length's own domain: no NaN (inf - inf can't arise from squares).
    let mut probe = s.clone();
    call_c(&mut probe, 0x18, &[], len);
    let n = probe.ctx.fpr[0].fl();
    if let Some(k) = k {
        wf(&mut s, k_at, k);
    }
    if tie {
        wf(&mut s, k_at, n);
    }
    let kk = rf(&s, k_at);
    let mut w = s.clone();
    saves(&mut w, &s, 0x18, &[(0x14, RA), (0x18, A0)]);
    call_c(&mut w, 0x18, &[], len);
    let n = w.ctx.fpr[0].fl();
    w.ctx.gpr[AT] = sext(0x800B_0000);
    w.ctx.fpr[4].set_fl(kk);
    w.ctx.gpr[A0] = sext(at);
    w.ctx.fpr[2].set_fl(n);
    if kk <= n {
        let q: Option<Vec<f32>> = v.iter().map(|&x| div(x, n)).collect();
        if q.is_none() {
            return Ok(());
        }
        let q = q.unwrap();
        let regs: &[(usize, usize)] = if v.len() == 2 { &[(6, 8), (10, 16)] } else { &[(6, 8), (10, 16), (18, 4)] };
        for (i, &(src, dst)) in regs.iter().enumerate() {
            w.ctx.fpr[src].set_fl(v[i]);
            w.ctx.fpr[dst].set_fl(q[i]);
            wf(&mut w, at + 4 * i as u32, q[i]);
        }
    }
    w.ctx.gpr[RA] = sext(s.ctx.gpr[RA] as u32);
    w.ctx.gpr[SP] = sext(SP_AT);
    w.ctx.fpr[0].set_fl(n);
    let after = run(name, port, &s)?;
    same_state(&after, &w)
}

// ---- math: func_80016A20, the inverse through the LU pair ----

fn inverse_model(s: &State, out: u32, m: u32) -> Option<State> {
    let f = SP_AT - 0xC8;
    let mut w = s.clone();
    saves(&mut w, s, 0xC8, &[(0x40, S7), (0x44, RA), (0x34, S4), (0x3C, S6), (0x38, S5), (0x30, S3), (0x2C, S2), (0x28, S1), (0x24, S0), (0xCC, A1)]);
    for (off, r) in [(0x18u32, 22usize), (0x10, 20)] {
        let v = s.ctx.fpr[r].u64;
        wr(&mut w, f + off, (v >> 32) as u32);
        wr(&mut w, f + off + 4, v as u32);
    }
    let indx = f + 0xB4;
    w.ctx.gpr[S7] = sext(indx);
    w.ctx.gpr[S4] = sext(out);
    call_c(&mut w, 0xC8, &[(A0, sext(out)), (A1, sext(m)), (A2, sext(indx))], imports::func_80016260);
    if w.ctx.gpr[V0] == 0 {
        return Some(w);
    }
    let b = f + 0xA8;
    for j in 0..3u32 {
        for k in 0..3u32 {
            wf(&mut w, b + 4 * k, 0.0);
        }
        wf(&mut w, b + 4 * j, 1.0);
        // The registers the port holds at the call (lubksb saves some).
        let regs = [(S0, sext(f + 0xB4)), (S6, sext(f + 0xB4)), (S1, sext(f + 0xB4)), (S2, sext(b + 4 * j)), (S5, sext(b)), (S3, u64::from(4 * j))];
        w.ctx.fpr[20].set_u32l(0);
        w.ctx.fpr[22].set_u32l(0x3F80_0000);
        for (r, v) in regs {
            w.ctx.gpr[r] = v;
        }
        call_c(&mut w, 0xC8, &[(A0, sext(out)), (A1, sext(indx)), (A2, sext(b))], imports::func_800167E4);
        for k in 0..3u32 {
            let v = word(&w, b + 4 * k);
            wr(&mut w, f + 0x68 + 4 * j + 16 * k, v);
        }
    }
    for r in 0..3u32 {
        for c in 0..3u32 {
            let v = word(&w, f + 0x68 + 16 * r + 4 * c);
            wr(&mut w, out + 16 * r + 4 * c, v);
        }
        wr(&mut w, out + 16 * r + 12, 0);
    }
    wf(&mut w, out + 0x3C, 1.0);
    let t = [rf(&w, m + 0x30), rf(&w, m + 0x34), rf(&w, m + 0x38)];
    for j in 0..3u32 {
        let o = |r: u32| rf(&w, out + 16 * r + 4 * j);
        let v = neg(add(mul(o(2), t[2])?, add(mul(t[0], o(0))?, mul(t[1], o(1))?)?)?)?;
        wf(&mut w, out + 0x30 + 4 * j, v);
    }
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(384))]

    #[test]
    fn func_80016A20(seed: u64, a in prop::array::uniform9(prop_oneof![4 => -100.0f32..100.0, 1 => (-2i32..=2).prop_map(|n| n as f32)]),
                     t in prop::array::uniform3(ordinary()), junk in prop::collection::vec(any::<u32>(), 16), same: bool) {
        let mut s = state(seed);
        load_data(&mut s);
        let (out, m) = (0x8030_0000u32, 0x8030_0100u32);
        for (i, w) in junk.iter().enumerate() {
            wr(&mut s, out + 4 * i as u32, *w);
        }
        for r in 0..3u32 {
            for c in 0..3u32 {
                wf(&mut s, m + 16 * r + 4 * c, a[(3 * r + c) as usize]);
            }
        }
        for (i, v) in t.iter().enumerate() {
            wf(&mut s, m + 0x30 + 4 * i as u32, *v);
        }
        // `same`: rows 0 and 1 equal, so the decomposition may fail.
        if same {
            for c in 0..3u32 {
                wf(&mut s, m + 16 + 4 * c, a[c as usize]);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(out), sext(m));
        let want = inverse_model(&s, out, m);
        prop_assume!(want.is_some());
        let after = run("func_80016A20", math::func_80016A20, &s)?;
        same_memory(&after, &want.unwrap())?;
    }
}

// ---- misc: a node's world matrix and flag walk ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80017FD0(seed: u64, a in prop::array::uniform16(ordinary()), mm in prop::array::uniform16(ordinary())) {
        let mut s = state(seed);
        let (o, m) = (0x8030_0000u32, 0x8030_1000u32);
        for i in 0..16u32 {
            wf(&mut s, o + 0x30 + 4 * i, a[i as usize]);
            wf(&mut s, m + 4 * i, mm[i as usize]);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(o), sext(m));
        let mut w = s.clone();
        saves(&mut w, &s, 0x20, &[(0x1C, RA), (0x18, S0)]);
        for i in 0..16u32 {
            wf(&mut w, o + 0xB0 + 4 * i, mm[i as usize]);
        }
        call_c(&mut w, 0x20, &[(A0, sext(o + 0x70)), (A1, sext(o + 0x30)), (A2, sext(o + 0xB0))], imports::func_80015724);
        let after = run("func_80017FD0", misc::func_80017FD0, &s)?;
        same_memory(&after, &w)?;
    }
}

#[derive(Clone, Debug)]
struct Node {
    ty: u32,
    words: [u32; 2],
    kids: Vec<Node>,
}

fn node() -> BoxedStrategy<Node> {
    let leaf = (prop_oneof![Just(0x4000u32), any::<u32>()], any::<[u32; 2]>()).prop_map(|(ty, words)| Node { ty, words, kids: vec![] });
    leaf.prop_recursive(3, 12, 3, |inner| {
        (prop_oneof![3 => Just(0x4000u32), 1 => any::<u32>()], any::<[u32; 2]>(), prop::collection::vec(inner, 0..3)).prop_map(|(ty, words, kids)| Node { ty, words, kids })
    })
    .boxed()
}

/// Lays a node out: `+0` type, `+4`, `+8` the words, `+0x14` count, `+0x18`
/// the children array. Returns its address; `all` collects every node.
fn put_node(s: &mut State, n: &Node, next: &mut u32, all: &mut Vec<(u32, Node)>) -> u32 {
    let at = *next;
    *next += 0x20;
    let arr = *next;
    *next += 4 * n.kids.len() as u32 + 0x10;
    wr(s, at, n.ty);
    wr(s, at + 4, n.words[0]);
    wr(s, at + 8, n.words[1]);
    wr(s, at + 0x14, n.kids.len() as u32);
    wr(s, at + 0x18, arr);
    all.push((at, n.clone()));
    for (i, k) in n.kids.iter().enumerate() {
        let c = put_node(s, k, next, all);
        wr(s, arr + 4 * i as u32, c);
    }
    at
}

fn walk(w: &mut State, at: u32, n: &Node, which: u64, v: u64, flags: u64, op: i32, arrs: &dyn Fn(u32) -> u32) {
    let word_at = at + if which == 0 { 8 } else { 4 };
    if flags & 0x10 != 0 {
        let old = word(w, word_at);
        match op {
            2 => wr(w, word_at, old | v as u32),
            3 => wr(w, word_at, old & v as u32),
            1 => wr(w, word_at, v as u32),
            _ => {}
        }
    }
    if flags & 0x20 != 0 && n.ty & 0x4000 != 0 {
        for (i, k) in n.kids.iter().enumerate() {
            let c = word(w, arrs(at) + 4 * i as u32);
            walk(w, c, k, which, v, flags & 0x10, op, arrs);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(384))]

    #[test]
    fn func_800181BC(seed: u64, n in prop::option::of(node()), which in prop_oneof![Just(0u64), Just(2u64), any::<u64>()], v: u64,
                     flags in prop_oneof![Just(0x30u64), Just(0x10u64), Just(0x20u64), any::<u64>()], op in prop_oneof![1i32..4, any::<i32>()]) {
        let mut s = state(seed);
        let mut next = 0x8031_0000u32;
        let mut all = vec![];
        let at = n.as_ref().map_or(0, |n| put_node(&mut s, n, &mut next, &mut all));
        wr(&mut s, SP_AT + 0x10, op as u32);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(at), which, v, flags);
        let mut w = s.clone();
        let ret = if at == 0 || (which != 0 && which != 2) {
            0
        } else {
            let arrs = |a: u32| word(&s, a + 0x18);
            let nd = n.as_ref().unwrap();
            walk(&mut w, at, nd, which, v, flags, op, &arrs);
            // v0 = the word's address, unless calls follow: then the last
            // callee's (the type word, or the child count).
            if flags & 0x20 == 0 {
                sext(at + if which == 0 { 8 } else { 4 })
            } else if nd.ty & 0x4000 == 0 {
                sext(nd.ty)
            } else {
                sext(nd.kids.len() as u32)
            }
        };
        let after = run("func_800181BC", misc::func_800181BC, &s)?;
        for (a, _) in &all {
            for off in [4u32, 8] {
                prop_assert_eq!(word(&after, a + off), word(&w, a + off), "node {:#X} +{}", a, off);
            }
        }
        prop_assert_eq!(after.ctx.gpr[V0], ret);
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }
}

// ---- save: unlock counting and the profile reset ----

const COUNT: u32 = 0x8011_A270;
const N_AT: u32 = 0x800A_21B4;
const T_AT: u32 = 0x800A_22E8;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80024874(seed: u64, c in 0i8..4, cur in -1i8..8, ns in prop::array::uniform4(0u8..8), rows in prop::collection::vec(prop_oneof![0u32..8, Just(u32::MAX), Just(0xFF), any::<u32>()], 28),
                     flag: i8, tables in prop::array::uniform2(prop::array::uniform4(any::<u8>())), old: u32) {
        let mut s = state(seed);
        let r = 0x8030_0000u32;
        wb(&mut s, r + 0x5E, c as u8);
        wb(&mut s, r + 0x5D, cur as u8);
        wb(&mut s, r + 0x6C, flag as u8);
        for (i, n) in ns.iter().enumerate() {
            wb(&mut s, N_AT + i as u32, *n);
        }
        for (i, v) in rows.iter().enumerate() {
            wr(&mut s, T_AT + 4 * i as u32, *v);
        }
        for (t, base) in [0x8011_3E68u32, 0x8011_368C].iter().enumerate() {
            for i in 0..4u32 {
                wb(&mut s, base + i, tables[t][i as usize]);
            }
        }
        wr(&mut s, COUNT, old);
        s.ctx.gpr[A0] = sext(r);
        let mut w = s.clone();
        saves(&mut w, &s, 0x30, &[(0x20, S3), (0x2C, RA), (0x28, S5), (0x24, S4), (0x1C, S2), (0x18, S1), (0x14, S0)]);
        wr(&mut w, COUNT, 0);
        let table = if flag != 0 { 0x8011_3E68u32 } else { 0x8011_368C };
        let mut i = 0u32;
        while (i as i32) < i32::from(byte(&w, N_AT.wrapping_add(c as i32 as u32))) {
            if (cur as i32) == word(&w, T_AT + 28 * c as u32 + 4 * i) as i32 {
                break;
            }
            // func_8002DAD0(r, c, i & 0xFF): its spills, then the bit.
            wr(&mut w, SP_AT - 0x30 + 4, c as i32 as u32);
            wr(&mut w, SP_AT - 0x30 + 8, i & 0xFF);
            if byte(&w, table + c as u32) >> (i & 31) & 1 != 0 {
                let n = word(&w, COUNT);
                wr(&mut w, COUNT, n + 1);
            }
            i += 1;
        }
        let after = run("func_80024874", save::func_80024874, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80028070(seed: u64, junk in prop::collection::vec(any::<u32>(), 11 * 16 + 4 * 11)) {
        let mut s = state(seed);
        for (i, v) in junk[..11 * 16].iter().enumerate() {
            wr(&mut s, 0x8011_3E60 + 4 * i as u32, *v);
        }
        for (i, v) in junk[11 * 16..].iter().enumerate() {
            wr(&mut s, 0x8011_3694 + 4 * i as u32, *v);
        }
        let mut w = s.clone();
        saves(&mut w, &s, 0x20, &[(0x14, S0), (0x1C, RA), (0x18, S1), (0x20, A0)]);
        for i in 0..12u64 {
            call_c(&mut w, 0x20, &[(S0, i), (A0, 0), (A1, i)], imports::func_80029A3C);
        }
        w.ctx.gpr[S1] = 4;
        for i in 0..4u64 {
            call_c(&mut w, 0x20, &[(S0, i), (A0, 1), (A1, i)], imports::func_80029A3C);
        }
        let after = run("func_80028070", save::func_80028070, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- render: func_800141EC, glyph setup ----

const CURRENT_FONT: u32 = 0x800A_1D8C;
const COLOUR: u32 = 0x800A_1CCC;
const LIST: u32 = 0x8032_0000;

#[derive(Clone, Debug)]
struct Glyph {
    c: u64,
    format: u32,
    first: u8,
    last: u8,
    table: bool,
    glyphs: Vec<[u16; 8]>,
    colour: [u8; 4],
    count: u32,
}

fn glyph_case() -> BoxedStrategy<Glyph> {
    let g = prop::array::uniform8(prop_oneof![3 => 0u16..64, 1 => Just(0xFFFFu16), 1 => any::<u16>()]);
    (prop_oneof![3 => 0x20u64..0x7F, 1 => any::<u64>()], prop_oneof![Just(0u32), Just(2u32), Just(1u32), any::<u32>()],
     prop_oneof![Just(0x20u8), Just(0x41u8), any::<u8>()], prop_oneof![Just(b'Z'), Just(b'z'), Just(b'a'), Just(b'`'), any::<u8>()], prop::bool::weighted(0.9),
     prop::collection::vec(g, 0x60), any::<[u8; 4]>(), 0u32..8)
        .prop_map(|(c, format, first, last, table, glyphs, colour, count)| Glyph { c, format, first, last, table, glyphs, colour, count })
        .boxed()
}

fn glyph_model(s: &State, x: &Glyph, font: u32, gt: u32) -> State {
    let mut w = s.clone();
    saves(&mut w, s, 0x20, &[(0x1C, RA), (0x18, S0), (0x20, A0)]);
    let mut head = word(s, DL_HEAD);
    let mut put = |w: &mut State, a: u32, b: u32| {
        wr(w, head, a);
        wr(w, head + 4, b);
        head += 8;
        wr(w, DL_HEAD, head);
    };
    let col = u32::from_be_bytes(x.colour);
    put(&mut w, 0xFA00_0000, col);
    wr(&mut w, 0x800D_6938, 1);
    put(&mut w, 0xFCFF_97FF, 0xFF2D_FEFF);
    if x.format == 0 {
        for (a, b) in [(0xE300_1001, 0xC000), (0xFD10_0000, 0x800A_1DD0), (0xE800_0000, 0), (0xF500_0100, 0x0700_0000), (0xE600_0000, 0), (0xF000_0000, 0x073F_C000), (0xE700_0000, 0)] {
            put(&mut w, a, b);
        }
    }
    if x.format == 2 {
        let img = word(s, font + 0x48);
        for (a, b) in [(0xE300_1001, 0x8000), (0xFD10_0000, img), (0xE800_0000, 0), (0xF500_0100, 0x0700_0000), (0xE600_0000, 0), (0xF000_0000, 0x0703_C000), (0xE700_0000, 0), (0xE600_0000, 0)] {
            put(&mut w, a, b);
        }
    }
    let mut c = x.c as u8;
    if (b'a'..=b'z').contains(&c) && x.last < b'a' {
        c -= 0x20;
        wb(&mut w, SP_AT + 3, c);
    }
    if !x.table || c < x.first || x.last < c {
        return w;
    }
    let g = gt + 16 * u32::from(c - x.first);
    if half(s, g + 8) == 0xFFFF {
        return w;
    }
    for (a, off) in [(0x800D_691Cu32, 6u32), (0x800D_6920, 4), (0x800D_6924, 0xC), (0x800D_6928, 0xE), (0x800D_692C, 8), (0x800D_6930, 0xA)] {
        wr(&mut w, a, half(s, g + off) as i16 as i32 as u32);
    }
    wr(&mut w, 0x800D_6934, if x.format == 0 { u32::from(byte(s, g + 1)) } else { 0 });
    let tex = byte(s, g);
    call_c(&mut w, 0x20, &[(S0, sext(font)), (A0, sext(font)), (A1, u64::from(tex))], imports::func_800125E4);
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(768))]

    #[test]
    fn func_800141EC(seed: u64, x in glyph_case(), images in prop::array::uniform8(any::<u32>())) {
        let mut s = state(seed);
        let (font, gt) = (0x8033_0000u32, 0x8033_1000u32);
        wr(&mut s, DL_HEAD, LIST);
        wr(&mut s, CURRENT_FONT, font);
        for (i, b) in x.colour.iter().enumerate() {
            wb(&mut s, COLOUR + i as u32, *b);
        }
        wr(&mut s, font, x.format);
        wr(&mut s, font + 4, x.count);
        for (i, v) in images.iter().enumerate() {
            wr(&mut s, font + 8 + 4 * i as u32, *v);
        }
        wr(&mut s, font + 0x48, images[7] ^ 0x5A5A);
        wb(&mut s, font + 0x5A, x.first);
        wb(&mut s, font + 0x5B, x.last);
        wr(&mut s, font + 0x5C, if x.table { gt } else { 0 });
        // Glyphs only for 0x60 characters; others outside that stay in RDRAM.
        for (i, g) in x.glyphs.iter().enumerate() {
            for (j, h) in g.iter().enumerate() {
                wh(&mut s, gt + 16 * i as u32 + 2 * j as u32, *h);
            }
        }
        s.ctx.gpr[A0] = x.c;
        let want = glyph_model(&s, &x, font, gt);
        let after = run("func_800141EC", render::func_800141EC, &s)?;
        same_memory(&after, &want)?;
    }
}
