//! Depth 1 at 0x80083D80..0x800877AC (game::misc, game::render): a tree's
//! first box in world space (recursive, through a global matrix), the last
//! key below 20 of a node's weights, the distance to the nearest of a point
//! list, a camera record's parameters and two setters over the four
//! records, and three light setters from floats. Recompiled C vs Rust with
//! the callees as C. The models replay the callees' C on a copy of the
//! state in the same order with the same arguments (and the s registers the
//! port holds), adding the functions' own stores; whole RDRAM (and the
//! result register) is compared.

// Tests are named after the functions (func_80083D80), capitals included.
#![allow(non_snake_case)]

use difftest::rom::baserom;
use difftest::{compare, State};
use game::imports;
use game::recomp::{reg::*, RecompFn};
use game::{misc, render};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
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

/// The game's data segment (`0x80098000..0x800AE8B0`) as the ROM image has
/// it: the test background is random, and the functions read constants
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

fn wf(s: &mut State, a: u32, v: f32) {
    wr(s, a, v.to_bits());
}

fn rf(s: &State, a: u32) -> f32 {
    f32::from_bits(word(s, a))
}

fn v3(s: &State, a: u32) -> [f32; 3] {
    [0, 1, 2].map(|k| rf(s, a + 4 * k))
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

/// `sd` of an FPR: high word at `+0`, low at `+4`.
fn save_double(w: &mut State, a: u32, v: u64) {
    wr(w, a, (v >> 32) as u32);
    wr(w, a + 4, v as u32);
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
fn div(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? / ok(b)?)
}

/// The C cast (`trunc.w.s`, cvttss2si): 0x80000000 out of range and for
/// NaN.
fn trunc(x: f32) -> u32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        0x8000_0000
    } else {
        x.trunc() as i32 as u32
    }
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

/// Matrix entries small enough that many products in a row stay finite
/// (the recursive walk multiplies a global matrix by every transform it
/// visits, below the top level where a model can't check for NaN).
fn unit() -> BoxedStrategy<f32> {
    prop_oneof![4 => -0.5f32..0.5, 1 => Just(0.0f32), 1 => Just(-0.0f32), 1 => Just(0.5f32), 1 => Just(-0.5f32)].boxed()
}

// ---- the tree's first box ----

const NODES: u32 = 0x8032_0000;
const LEAF: u32 = NODES + 0x100 * 8;
const BOXES: u32 = 0x8033_0000;
const TYPES: [u32; 8] = [0x5064, 0xD064, 0xD065, 0x3064, 0x8001, 0x5066, 0x4000, 0xC000];
const MTX: u32 = 0x8012_0D40;
const OUT: u32 = 0x8034_0000;

#[derive(Clone, Debug)]
struct Node {
    ty: u8,
    kids: Vec<(bool, u32)>,
    m: [f32; 12],
    bx: [f32; 6],
}

fn node() -> BoxedStrategy<Node> {
    (0u8..8, prop::collection::vec((prop::bool::weighted(0.2), 0u32..64), 0..4), prop::array::uniform12(unit()), prop::array::uniform6(ordinary()))
        .prop_map(|(ty, kids, m, bx)| Node { ty, kids, m, bx })
        .boxed()
}

fn node_at(i: u32) -> u32 {
    NODES + 0x100 * i
}

/// Node `i` at `NODES + 0x100 i` (type `TYPES[ty]`, children at `+0xC0`,
/// the 3x4 at `+0x1C`), children null or later nodes (the last node's go
/// to a leaf of type 0x3064). A 0x3064 node's first child is instead its
/// box record `BOXES + 0x40 i` (six floats at `+8`); the leaf has one too.
fn tree(s: &mut State, nodes: &[Node], leaf_box: [f32; 6]) {
    wr(s, LEAF, 0x3064);
    wr(s, LEAF + 0x14, 0);
    wr(s, LEAF + 0x18, LEAF + 0xC0);
    wr(s, LEAF + 0xC0, BOXES + 0x40 * 8);
    for (k, &x) in leaf_box.iter().enumerate() {
        wf(s, BOXES + 0x40 * 8 + 8 + 4 * k as u32, x);
    }
    let len = nodes.len() as u32;
    for (i, nd) in nodes.iter().enumerate() {
        let i = i as u32;
        let n = node_at(i);
        let ty = TYPES[nd.ty as usize];
        wr(s, n, ty);
        wr(s, n + 0x14, nd.kids.len() as u32);
        wr(s, n + 0x18, n + 0xC0);
        for (k, &x) in nd.m.iter().enumerate() {
            wf(s, n + 0x1C + 4 * k as u32, x);
        }
        for (j, &(null, pick)) in nd.kids.iter().enumerate() {
            let c = if null {
                0
            } else if i + 1 < len {
                node_at(i + 1 + pick % (len - i - 1))
            } else {
                LEAF
            };
            wr(s, n + 0xC0 + 4 * j as u32, c);
        }
        if ty == 0x3064 {
            wr(s, n + 0xC0, BOXES + 0x40 * i);
            for (k, &x) in nd.bx.iter().enumerate() {
                wf(s, BOXES + 0x40 * i + 8 + 4 * k as u32, x);
            }
        }
    }
}

fn box_model(s: &State) -> (State, u64) {
    let mut w = s.clone();
    let sp = SP_AT - 0x88;
    saves(&mut w, s, 0x88, &[(0x28, S5), (0x24, S4), (0x20, S3), (0x2C, RA), (0x1C, S2), (0x18, S1), (0x14, S0)]);
    let [n, out] = [A0, A1].map(|r| s.ctx.gpr[r] as u32);
    if n == 0 {
        return (w, 0);
    }
    let flags = s.ctx.gpr[A2] | 1;
    if s.ctx.gpr[A2] & 1 == 0 {
        call_c(&mut w, 0x88, &[(A0, sext(MTX))], imports::func_80017874);
    }
    let ty = word(&w, n);
    if ty & 0x4000 == 0 {
        if ty != 0x3064 {
            return (w, 0);
        }
        let first = word(&w, word(&w, n + 0x18));
        call_c(&mut w, 0x88, &[(A0, sext(first)), (A1, sext(out))], imports::func_80017E20);
        for p in [out, out + 0xC] {
            call_c(&mut w, 0x88, &[(A0, sext(p)), (A1, sext(p)), (A2, sext(MTX))], imports::func_80016CAC);
        }
        return (w, 1);
    }
    let copy = match ty {
        0xD065 => Some(imports::func_80017C18 as RecompFn),
        0xD064 => Some(imports::func_80017C98 as RecompFn),
        _ => None,
    };
    if let Some(copy) = copy {
        call_c(&mut w, 0x88, &[(A0, sext(n)), (A1, sext(sp + 0x34))], copy);
        call_c(&mut w, 0x88, &[(A0, sext(MTX)), (A1, sext(sp + 0x34))], imports::func_80015C30);
    }
    let count = word(&w, n + 0x14) as i32;
    for i in 0..count.max(0) as u32 {
        let child = word(&w, word(&w, n + 0x18) + 4 * i);
        let g = &mut w.ctx.gpr;
        (g[S0], g[S1], g[S2], g[S3], g[S4], g[S5]) = (u64::from(i), u64::from(4 * i), sext(count as u32), sext(n), flags, sext(out));
        call_c(&mut w, 0x88, &[(A0, sext(child)), (A1, sext(out)), (A2, flags)], imports::func_80083D80);
        if w.ctx.gpr[V0] != 0 {
            return (w, 1);
        }
    }
    (w, 0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Random trees of every node kind (a box found at once, deep, after a
    /// failed sibling subtree, or nowhere), the matrix reset or kept (bit 0
    /// of the flags, other bits set too), a null node.
    #[test]
    fn func_80083D80(seed: u64, nodes in prop::collection::vec(node(), 1..6), leaf_box in prop::array::uniform6(ordinary()),
                     flags in prop_oneof![Just(0u64), Just(1u64), any::<u32>().prop_map(sext)], start in prop::array::uniform16(unit()),
                     null in prop::bool::weighted(0.05)) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, OUT, 0x40);
        tree(&mut s, &nodes, leaf_box);
        for (k, &x) in start.iter().enumerate() {
            wf(&mut s, MTX + 4 * k as u32, x);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (if null { 0 } else { sext(NODES) }, sext(OUT), flags);
        let (want, v0) = box_model(&s);
        let after = run("func_80083D80", misc::func_80083D80, &s)?;
        same_memory(&after, &want)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
    }
}

// ---- the last key below 20 ----

const N1: u32 = 0x8030_0000;

fn key_model(s: &State) -> (State, u64) {
    let mut w = s.clone();
    saves(&mut w, s, 0x18, &[(0x14, RA), (0x18, A0)]);
    let n = s.ctx.gpr[A0] as u32;
    let count = word(&w, n + 0x14) as i32;
    let wk = |j: u32| rf(&w, n + 0x1C + 4 * j);
    if count <= 0 || 20.0 < wk(0) {
        return (w, u64::MAX);
    }
    let stop = |x: f32| x == -1.0 || !(x < 20.0);
    let k = if stop(wk(1)) {
        0
    } else {
        let mut j = 2;
        while j < 8 && !stop(wk(j)) {
            j += 1;
        }
        j - 1
    };
    (w, if (k as i32) < count { u64::from(k) } else { u64::MAX })
}

fn key() -> BoxedStrategy<f32> {
    prop_oneof![
        4 => 0.0f32..19.0,
        1 => Just(-1.0f32),
        1 => Just(20.0f32),
        1 => Just(f32::from_bits(20.0f32.to_bits() - 1)),
        1 => Just(f32::from_bits(20.0f32.to_bits() + 1)),
        1 => Just(f32::NAN),
        1 => Just(-0.0f32),
        1 => Just(f32::from_bits((-1.0f32).to_bits() + 1)),
        1 => 19.0f32..30.0,
        1 => float(),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Keys around 20 and -1 (ulps either side, NaN), all eight below 20,
    /// counts around `k` and non-positive.
    #[test]
    fn func_800844C8(seed: u64, keys in prop::array::uniform8(key()), all_low: bool, count in prop_oneof![-2i32..10, any::<i32>()]) {
        let mut s = state(seed);
        for (j, &x) in keys.iter().enumerate() {
            let x = if all_low && j > 0 && !(x < 20.0 && x != -1.0) { 19.5 } else { x };
            wf(&mut s, N1 + 0x1C + 4 * j as u32, x);
        }
        wr(&mut s, N1 + 0x14, count as u32);
        s.ctx.gpr[A0] = sext(N1);
        let (want, v0) = key_model(&s);
        let after = run("func_800844C8", misc::func_800844C8, &s)?;
        same_memory(&after, &want)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
    }
}

// ---- the nearest point ----

const COUNT: u32 = 0x800A_694C;
const POINTS: u32 = 0x8014_89C8;
const Q: u32 = 0x8030_1000;

fn nearest_model(s: &State) -> Option<(State, u32)> {
    let mut w = s.clone();
    let sp = SP_AT - 0x30;
    saves(&mut w, s, 0x30, &[(0x28, S3), (0x24, S2), (0x2C, RA), (0x20, S1), (0x1C, S0)]);
    save_double(&mut w, sp + 0x10, s.ctx.fpr[20].u64);
    let p = s.ctx.gpr[A0] as u32;
    let count = word(&w, COUNT) as i32;
    if count <= 0 {
        return Some((w, (-1.0f32).to_bits()));
    }
    let a = v3(&w, p);
    let mut least = None;
    for i in 0..count as u32 {
        let b = v3(&w, POINTS + 0xC * i);
        let d = [0, 1, 2].map(|k| sub(a[k], b[k]));
        let [dx, dy, dz] = [d[0]?, d[1]?, d[2]?];
        let q = add(add(mul(dx, dx)?, mul(dy, dy)?)?, mul(dz, dz)?)?;
        least = Some(match least {
            Some(l) if !(q < l) => l,
            _ => q,
        });
    }
    Some((w, least?.sqrt().to_bits()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Up to eight points (duplicates and ties of distance included),
    /// counts of zero and below.
    #[test]
    fn func_80085EB0(seed: u64, pts in prop::collection::vec(prop::array::uniform3(ordinary()), 0..8), p in prop::array::uniform3(ordinary()),
                     count in prop_oneof![4 => Just(None), 1 => (-3i32..1).prop_map(Some)], dup: bool) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, POINTS, 0x100);
        for (i, q) in pts.iter().enumerate() {
            let q = if dup && i == pts.len() - 1 && i > 0 { pts[0] } else { *q };
            for k in 0..3 {
                wf(&mut s, POINTS + 0xC * i as u32 + 4 * k as u32, q[k]);
            }
        }
        wr(&mut s, COUNT, count.unwrap_or(pts.len() as i32) as u32);
        for k in 0..3 {
            wf(&mut s, Q + 4 * k as u32, p[k]);
        }
        s.ctx.gpr[A0] = sext(Q);
        let model = nearest_model(&s);
        prop_assume!(model.is_some());
        let (want, f0) = model.unwrap();
        let after = run("func_80085EB0", misc::func_80085EB0, &s)?;
        same_memory(&after, &want)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), f0);
    }
}

// ---- the camera records ----

const RECORDS: u32 = 0x8012_0DF0;

fn record(k: u32) -> u32 {
    RECORDS + 0x170 * k
}

fn camera_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    saves(&mut w, s, 0x18, &[(0x14, RA), (0x18, A0), (0x24, A3)]);
    let k = s.ctx.gpr[A0] as u32;
    let p = record(k);
    let args = [s.ctx.gpr[A1] as u32, s.ctx.gpr[A2] as u32, s.ctx.gpr[A3] as u32, word(s, SP_AT + 0x10), word(s, SP_AT + 0x14)];
    for (i, (&x, off)) in args.iter().zip([0x134, 0x138, 0x140, 0x144, 0x13C]).enumerate() {
        let v = f32::from_bits(x);
        let store = if i == 4 { 0.0 <= v } else { 0.0 < v };
        if store {
            wr(&mut w, p + off, x);
        }
    }
    // The callee's operands, checked before its C runs.
    mul(rf(&w, p + 0x148), add(1.0, mul(rf(&w, p + 0x150), sub(div(45.0, rf(&w, p + 0x134))?, 1.0)?)?)?)?;
    call_c(&mut w, 0x18, &[(A0, sext(p))], imports::func_80085F78);
    Some(w)
}

fn param() -> BoxedStrategy<f32> {
    prop_oneof![
        2 => ordinary(),
        1 => Just(0.0f32),
        1 => Just(-0.0f32),
        1 => Just(f32::from_bits(1)),
        1 => Just(-f32::from_bits(1)),
        1 => Just(f32::INFINITY),
        1 => Just(f32::NEG_INFINITY),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Every record, parameters of both signs and zero (and the smallest
    /// subnormals, infinities), the record's fields random.
    #[test]
    fn func_80086730(seed: u64, k in 0u32..4, args in prop::array::uniform5(param()), fields in prop::array::uniform9(ordinary())) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, RECORDS, 0x600);
        for (j, &x) in fields.iter().enumerate() {
            wf(&mut s, record(k) + 0x134 + 4 * j as u32, x);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (u64::from(k), sext(args[0].to_bits()), sext(args[1].to_bits()), sext(args[2].to_bits()));
        wf(&mut s, SP_AT + 0x10, args[3]);
        wf(&mut s, SP_AT + 0x14, args[4]);
        let model = camera_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_80086730", misc::func_80086730, &s)?;
        same_memory(&after, &model.unwrap())?;
    }

    /// Any value, all four records.
    #[test]
    fn func_80087754(seed: u64, v: u32) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, RECORDS, 0x600);
        s.ctx.gpr[A0] = sext(v);
        let mut want = s.clone();
        saves(&mut want, &s, 0x28, &[(0x20, S2), (0x18, S0), (0x1C, S1), (0x24, RA)]);
        for j in 0..4 {
            wr(&mut want, record(j) + 0x168, v);
        }
        let after = run("func_80087754", misc::func_80087754, &s)?;
        same_memory(&after, &want)?;
    }

    /// Every selector (and others, 64-bit ones included), any value.
    #[test]
    fn func_800877AC(seed: u64, k in prop_oneof![2u64..8, Just(0x1_0000_0004u64), any::<u32>().prop_map(sext)], v: u32) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, RECORDS, 0x600);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (k, sext(v));
        let mut want = s.clone();
        saves(&mut want, &s, 0x28, &[(0x20, S3), (0x14, S0), (0x1C, S2), (0x18, S1), (0x24, RA)]);
        for j in 0..4 {
            let g = &mut want.ctx.gpr;
            (g[S0], g[S1], g[S2], g[S3]) = (sext(record(j)), k, sext(v), sext(0x8012_13B0));
            call_c(&mut want, 0x28, &[(A0, sext(record(j))), (A1, k), (A2, sext(v))], imports::func_8001811C);
        }
        let after = run("func_800877AC", misc::func_800877AC, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- lights ----

const AMB: u32 = 0x8030_2000;
const DIF: u32 = 0x8030_2010;
const DIR: u32 = 0x8030_2020;
const SHORT_A: u32 = 0x800A_DD6C;
const SHORT_B: u32 = 0x800A_DD70;

/// The direction as the light setters store it, into `sp + 0x24`, and its
/// truncated halfwords; `None` outside the domain.
fn direction(w: &mut State, frame: u32, short: u32) -> Option<[u32; 3]> {
    let sp = SP_AT - frame;
    let d = v3(w, DIR);
    if d.iter().any(|x| x.is_nan()) {
        return None;
    }
    call_c(w, frame, &[(A0, sext(DIR))], imports::func_800153C0);
    let l = w.ctx.fpr[0].fl();
    let v = if l < rf(w, short) {
        [0.0, 0.0, -1.0]
    } else {
        let s = div(120.0, l)?;
        [mul(d[0], s)?, mul(d[1], s)?, mul(d[2], s)?]
    };
    for k in 0..3 {
        wf(w, sp + 0x24 + 4 * k as u32, v[k]);
    }
    Some(v.map(trunc))
}

fn halves(w: &mut State, at: u32, v: [u32; 3]) {
    for (k, &x) in v.iter().enumerate() {
        wh(w, at + 2 * k as u32, x as u16);
    }
}

fn light_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x48;
    saves(&mut w, s, 0x48, &[(0x14, RA), (0x48, A0), (0x4C, A1), (0x50, A2), (0x54, A3)]);
    let v = direction(&mut w, 0x48, SHORT_A)?;
    let (amb, dif) = (v3(&w, AMB).map(trunc), v3(&w, DIF).map(trunc));
    halves(&mut w, sp + 0x40, amb);
    halves(&mut w, sp + 0x38, dif);
    halves(&mut w, sp + 0x30, v);
    let i = s.ctx.gpr[A0];
    if i != u64::MAX {
        call_c(&mut w, 0x48, &[(A0, i), (A1, sext(sp + 0x40)), (A2, sext(sp + 0x38)), (A3, sext(sp + 0x30))], imports::func_80038ED0);
    } else {
        call_c(&mut w, 0x48, &[(A0, sext(sp + 0x40)), (A1, sext(sp + 0x38)), (A2, sext(sp + 0x30))], imports::func_80038E58);
    }
    Some(w)
}

fn second_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x40;
    saves(&mut w, s, 0x40, &[(0x14, RA), (0x40, A0)]);
    let on = s.ctx.gpr[A1];
    if on != 0 {
        saves(&mut w, s, 0x40, &[(0x44, A1), (0x48, A2), (0x4C, A3)]);
        let v = direction(&mut w, 0x40, SHORT_B)?;
        let col = v3(&w, DIF).map(trunc);
        halves(&mut w, sp + 0x38, col);
        halves(&mut w, sp + 0x30, v);
    }
    let i = sext(word(&w, sp + 0x40));
    let on = if on != 0 { sext(word(&w, sp + 0x44)) } else { on };
    call_c(&mut w, 0x40, &[(A0, i), (A1, on), (A2, sext(sp + 0x38)), (A3, sext(sp + 0x30))], imports::func_80038FE8);
    Some(w)
}

fn slot() -> BoxedStrategy<u64> {
    prop_oneof![4 => 0u64..12, 2 => Just(u64::MAX), 1 => Just(12u64), 1 => Just(sext(0xFFFF_FFFE)), 1 => any::<u32>().prop_map(sext)].boxed()
}

fn colour() -> BoxedStrategy<f32> {
    prop_oneof![4 => -1.0f32..300.0, 1 => (0i32..256).prop_map(|n| n as f32), 1 => float(), 1 => Just(f32::INFINITY), 1 => Just(f32::NAN)].boxed()
}

fn dir() -> BoxedStrategy<[f32; 3]> {
    prop_oneof![
        3 => prop::array::uniform3(ordinary()),
        1 => Just([0.0f32, 0.0, 0.0]),
        1 => prop::array::uniform3(-0.01f32..0.01),
        1 => prop::array::uniform3(float()),
    ]
    .boxed()
}

fn light_state(seed: u64, amb: [f32; 3], dif: [f32; 3], d: [f32; 3], short: (u32, Option<f32>)) -> State {
    let mut s = state(seed);
    load_data(&mut s);
    s.randomise_memory(seed ^ 0x5EED, 0x800A_3DB0, 0x228);
    for k in 0..3 {
        wf(&mut s, AMB + 4 * k as u32, amb[k]);
        wf(&mut s, DIF + 4 * k as u32, dif[k]);
        wf(&mut s, DIR + 4 * k as u32, d[k]);
    }
    if let Some(x) = short.1 {
        wf(&mut s, short.0, x);
    }
    s
}

/// A threshold: the ROM's, or one written (some equal to a direction's
/// length, for the `<` tie).
fn threshold() -> BoxedStrategy<Option<f32>> {
    prop_oneof![2 => Just(None), 1 => float().prop_map(Some), 1 => (0u32..4).prop_map(|n| Some(n as f32))].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Slots in and out of range and -1 (the default), colours past the
    /// halfword range and NaN, directions long, short, zero and edge.
    #[test]
    fn func_80086A20(seed: u64, i in slot(), amb in prop::array::uniform3(colour()), dif in prop::array::uniform3(colour()), d in dir(), short in threshold()) {
        let mut s = light_state(seed, amb, dif, d, (SHORT_A, short));
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (i, sext(AMB), sext(DIF), sext(DIR));
        let model = light_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_80086A20", render::func_80086A20, &s)?;
        same_memory(&after, &model.unwrap())?;
    }

    /// Light on or off (any nonzero word), slots, colours and directions as
    /// for the first light.
    #[test]
    fn func_80086B8C(seed: u64, i in slot(), on in prop_oneof![Just(0u64), Just(1u64), any::<u32>().prop_map(sext)], dif in prop::array::uniform3(colour()), d in dir(), short in threshold()) {
        let mut s = light_state(seed, [0.0; 3], dif, d, (SHORT_B, short));
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (i, on, sext(DIF), sext(DIR));
        let model = second_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_80086B8C", render::func_80086B8C, &s)?;
        same_memory(&after, &model.unwrap())?;
    }

    /// Slots in and out of range, and -1.
    #[test]
    fn func_80086CA0(seed: u64, i in slot()) {
        let mut s = state(seed);
        load_data(&mut s);
        s.randomise_memory(seed ^ 0x5EED, 0x800A_3DB0, 0x228);
        s.ctx.gpr[A0] = i;
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA)]);
        if i != u64::MAX {
            call_c(&mut want, 0x18, &[(A0, i)], imports::func_80038F68);
        }
        let after = run("func_80086CA0", render::func_80086CA0, &s)?;
        same_memory(&after, &want)?;
    }
}

/// The direction's length exactly at the threshold: `l < K` is false, so
/// the direction is scaled (a `<=` would give `(0, 0, -1)`).
#[test]
fn func_80086A20_tie() {
    for (k, d) in [[3.0f32, 4.0, 0.0], [0.006, 0.008, 0.0], [1.0, 2.0, 2.0]].into_iter().enumerate() {
        let l = ((d[2] * d[2]) + ((d[0] * d[0]) + (d[1] * d[1]))).sqrt();
        for (name, port, at) in [("func_80086A20", render::func_80086A20 as RecompFn, SHORT_A), ("func_80086B8C", render::func_80086B8C as RecompFn, SHORT_B)] {
            let mut s = light_state(0x71E + k as u64, [1.0, 2.0, 3.0], [4.0, 5.0, 6.0], d, (at, Some(l)));
            (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (3, if at == SHORT_A { sext(AMB) } else { 1 }, sext(DIF), sext(DIR));
            let want = if at == SHORT_A { light_model(&s) } else { second_model(&s) }.unwrap();
            let after = compare(name, port, &s).unwrap();
            same_memory(&after, &want).unwrap();
        }
    }
}
