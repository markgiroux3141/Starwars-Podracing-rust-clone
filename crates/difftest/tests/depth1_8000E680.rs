//! Depth 1 at 0x8000E680..0x800129B8 (game::misc): screen-to-record
//! positions, the node-tree walks (two of them recursive: the recursion
//! reaches the C like any callee), the screen projection, the record
//! switch-off loop and three thunks. Recompiled C vs Rust with the callees
//! as C. The thunks are checked against "the callee's C run with the
//! thunk's frame and arguments" (the whole state); the others against
//! their statements, simulating the stores where the frames are known.

// Tests are named after the functions (func_8000E680), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, diff_states, State};
use game::imports;
use game::misc::{self, RECORDS};
use game::recomp::{fpu, reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
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

/// A thunk's statement: the callee's C run on a copy with `sp` lowered by
/// `frame`, `saves` stored in the frame and `args` in registers, then `ra`
/// reloaded (sign-extended low word) and `sp` restored.
fn thunk(s: &State, frame: u32, saves: &[(u32, u32)], args: &[(usize, u64)], f: RecompFn) -> State {
    let mut w = s.clone();
    for &(off, v) in saves {
        wr(&mut w, SP_AT - frame + off, v);
    }
    for &(r, v) in args {
        w.ctx.gpr[r] = v;
    }
    w.ctx.gpr[SP] = sext(SP_AT - frame);
    w.run(f);
    w.ctx.gpr[RA] = sext(s.ctx.gpr[RA] as u32);
    w.ctx.gpr[SP] = sext(SP_AT);
    w
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

fn float() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        -1.0e4f32..1.0e4f32,
        -1.0f32..1.0,
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![4 => -1.0e3f32..1.0e3f32, 2 => -1.0f32..1.0, 2 => (-3i32..=3).prop_map(|n| n as f32), 1 => float()].boxed()
}

const SCREEN: u32 = 0x8011_4470;

// ---- func_8000E680 / func_8000E738: screen pixels to 320x240 ----

/// The two scaled coordinates, or None outside the domain.
fn scaled(x: i16, y: i16, w: i16, h: i16) -> Option<(u64, u64)> {
    let sx = mul(div(f32::from(x), f32::from(w))?, 320.0)?;
    let sy = mul(div(f32::from(y), f32::from(h))?, 240.0)?;
    let t = |v: f32| sext(fpu::trunc_w_s(v) as u16 as i16 as i32 as u32);
    Some((t(sx), t(sy)))
}

fn screen() -> BoxedStrategy<(i16, i16)> {
    prop_oneof![3 => Just((320i16, 240i16)), 3 => Just((640, 480)), 2 => (1i16..1000, 1i16..1000), 1 => (any::<i16>(), any::<i16>())].boxed()
}

fn coord() -> BoxedStrategy<u64> {
    prop_oneof![4 => (0u32..640).prop_map(u64::from), 2 => any::<i16>().prop_map(|v| sext(v as i32 as u32)), 1 => any::<u64>()].boxed()
}

fn screen_case(seed: u64, id: u64, x: u64, y: u64, scr: (i16, i16), f: RecompFn) -> Result<Option<(State, State)>, TestCaseError> {
    let Some((sx, sy)) = scaled(x as i16, y as i16, scr.0, scr.1) else { return Ok(None) };
    let mut s = state(seed);
    wh(&mut s, SCREEN, scr.0 as u16);
    wh(&mut s, SCREEN + 2, scr.1 as u16);
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (id, x, y);
    // The callee's C with the three halfwords, after the spills.
    let saves = [(0x14, s.ctx.gpr[RA] as u32), (0x18, id as u32), (0x1C, x as u32), (0x20, y as u32)];
    let want = thunk(&s, 0x18, &saves, &[(A0, sext(id as u16 as i16 as i32 as u32)), (A1, sx), (A2, sy)], f);
    Ok(Some((s, want)))
}

fn id() -> BoxedStrategy<u64> {
    prop_oneof![4 => (0u32..40).prop_map(u64::from), 1 => Just(sext((-201i32) as u32)), 1 => any::<i16>().prop_map(|v| sext(v as i32 as u32)), 1 => any::<u64>()].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8000E680(seed: u64, id in id(), x in coord(), y in coord(), scr in screen()) {
        let case = screen_case(seed, id, x, y, scr, imports::func_8000AA04)?;
        prop_assume!(case.is_some());
        let (s, want) = case.unwrap();
        let after = run("func_8000E680", misc::func_8000E680, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8000E738(seed: u64, id in id(), x in coord(), y in coord(), scr in screen()) {
        let case = screen_case(seed, id, x, y, scr, imports::func_8000AA78)?;
        prop_assume!(case.is_some());
        let (s, want) = case.unwrap();
        let after = run("func_8000E738", misc::func_8000E738, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_8000E8C4 / func_8000EA4C: node trees ----

#[derive(Clone, Debug)]
enum Tree {
    Null,
    /// Type 0x3064: its children's first words.
    Leaf(Vec<u32>),
    /// A type with bit 14 set, and children.
    Group(u32, Vec<Tree>),
    /// Any other type.
    Other(u32),
}

fn tree() -> BoxedStrategy<Tree> {
    let word = prop_oneof![3 => Just(0u32), 1 => 1u32..8, 1 => any::<u32>()];
    let leaf = prop_oneof![
        1 => Just(Tree::Null),
        4 => prop::collection::vec(word, 0..4).prop_map(Tree::Leaf),
        1 => any::<u32>().prop_filter("not a group or leaf", |t| t & 0x4000 == 0 && *t != 0x3064).prop_map(Tree::Other),
    ];
    leaf.prop_recursive(3, 16, 4, |inner| {
        (any::<u32>().prop_map(|t| (t | 0x4000) & !0), prop::collection::vec(inner, 0..4)).prop_map(|(t, k)| Tree::Group(if t == 0x3064 { 0x4064 } else { t }, k))
    })
    .boxed()
}

/// Lays a tree out from `*next`, returning the node's address (0 for Null).
/// Leaf children are words at their own addresses; `targets` gives, for
/// func_8000EA4C, pointers to put in the leaf words instead of the values.
fn layout(s: &mut State, t: &Tree, next: &mut u32, leaf_word: &mut dyn FnMut(&mut State, u32, u32) -> u32) -> u32 {
    let mut alloc = |n: u32| {
        let a = *next;
        *next += (n + 0xF) & !0xF;
        a
    };
    match t {
        Tree::Null => 0,
        Tree::Other(ty) => {
            let n = alloc(0x20);
            wr(s, n, *ty);
            n
        }
        Tree::Leaf(words) => {
            let n = alloc(0x20);
            let arr = alloc(4 * words.len() as u32 + 4);
            wr(s, n, 0x3064);
            wr(s, n + 0x14, words.len() as u32);
            wr(s, n + 0x18, arr);
            for (i, w) in words.iter().enumerate() {
                let c = alloc(0x20);
                wr(s, arr + 4 * i as u32, c);
                let v = leaf_word(s, *w, c);
                wr(s, c, v);
            }
            n
        }
        Tree::Group(ty, kids) => {
            let n = alloc(0x20);
            let arr = alloc(4 * kids.len() as u32 + 4);
            wr(s, n, *ty);
            wr(s, n + 0x14, kids.len() as u32);
            wr(s, n + 0x18, arr);
            for (i, k) in kids.iter().enumerate() {
                let c = layout(s, k, next, leaf_word);
                wr(s, arr + 4 * i as u32, c);
            }
            n
        }
    }
}

fn first_word(t: &Tree) -> u32 {
    match t {
        Tree::Null | Tree::Other(_) => 0,
        Tree::Leaf(w) => w.iter().copied().find(|&v| v != 0).unwrap_or(0),
        Tree::Group(_, k) => k.iter().map(first_word).find(|&v| v != 0).unwrap_or(0),
    }
}

const ARENA: u32 = 0x8031_0000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8000E8C4(seed: u64, t in tree()) {
        let mut s = state(seed);
        let mut next = ARENA;
        let n = layout(&mut s, &t, &mut next, &mut |_, w, _| w);
        s.ctx.gpr[A0] = sext(n);
        let after = run("func_8000E8C4", misc::func_8000E8C4, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(first_word(&t)));
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }

    #[test]
    fn func_8000EA4C(seed: u64, t in tree(), vals in prop::array::uniform6(prop_oneof![3 => -1i16..6, 1 => any::<i16>()]), hi: [u16; 6]) {
        let mut s = state(seed);
        let mut next = ARENA;
        // Each leaf word w becomes a pointer p to an object whose [p + 0xC]
        // is a byte block q (or 0, and p itself 0 for w == 0); expectations
        // are collected as (address, byte).
        let mut blocks: Vec<u32> = vec![];
        let mut qs = 0x8038_0000u32;
        let n = layout(&mut s, &t, &mut next, &mut |s, w, c| {
            if w == 0 {
                return 0;
            }
            let p = c + 4;
            let q = if w & 1 == 0 { 0 } else { qs };
            if q != 0 {
                qs += 0x40;
                for i in 0..8 {
                    wb(s, q + 0x20 + i, 0xA5);
                }
                blocks.push(q);
            }
            wr(s, p + 0xC, q);
            p
        });
        let arg = |i: usize| u64::from(hi[i]) << 16 | u64::from(vals[i] as u16);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(n), arg(0), arg(1), arg(2));
        for i in 0..3 {
            wr(&mut s, SP_AT + 0x10 + 4 * i as u32, arg(3 + i) as u32);
        }
        let after = run("func_8000EA4C", misc::func_8000EA4C, &s)?;
        for q in blocks {
            for i in 0..8u32 {
                let expect = if i < 6 && vals[i as usize] >= 0 { vals[i as usize] as u8 } else { 0xA5 };
                prop_assert_eq!(byte(&after, q + 0x20 + i), expect, "block {:#X} byte {}", q, i);
            }
        }
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }
}

// ---- func_8000EBE8: projection ----

const MTX: u32 = 0x8011_2E20;
const CAM: u32 = 0x800A_3FDC;
const K_AT: u32 = 0x800A_8680;
const MIRROR: u32 = 0x800D_697C;

#[derive(Clone, Debug)]
struct Proj {
    vp: [i16; 4],
    p: [f32; 3],
    m: [f32; 16],
    cam: [f32; 3],
    local: u32,
    mirror: u32,
    k: f64,
    junk: [u32; 4],
}

/// func_80081730: four outputs, each re-reading `v` (disjoint here).
fn point4(v: [f32; 3], m: &[f32; 16]) -> Option<[f32; 4]> {
    let c = |j: usize| add(m[12 + j], add(add(mul(m[j], v[0])?, mul(m[4 + j], v[1])?)?, mul(m[8 + j], v[2])?)?);
    Some([c(0)?, c(1)?, c(2)?, c(3)?])
}

fn project(s: &State, x: &Proj) -> Option<State> {
    let mut w = s.clone();
    let f = SP_AT - 0xA8;
    wr(&mut w, f + 0x14, s.ctx.gpr[RA] as u32);
    for (i, v) in x.m.iter().enumerate() {
        wf(&mut w, f + 0x68 + 4 * i as u32, *v);
    }
    for (i, r) in [A1, A2, A3].iter().enumerate() {
        wr(&mut w, SP_AT + 4 + 4 * i as u32, s.ctx.gpr[*r] as u32);
    }
    let half = |v: i16| (v as i32 / 2) as f32;
    let quarter = |v: i16| (v as i32 / 4) as f32;
    let (hw, hh) = (half(x.vp[0]), half(x.vp[1]));
    let (hw2, hh2) = (mul(hw, 0.5)?, mul(hh, 0.5)?);
    wf(&mut w, f + 0x24, hw);
    wf(&mut w, f + 0x20, hh);
    wf(&mut w, f + 0x34, hw2);
    wf(&mut w, f + 0x30, hh2);
    let (ox, oy) = (sub(quarter(x.vp[2]), hw2)?, sub(quarter(x.vp[3]), hh2)?);
    wf(&mut w, f + 0x2C, ox);
    wf(&mut w, f + 0x28, oy);
    let (sx, sy, zw, ww) = (0x8030_1000u32, 0x8030_1004u32, 0x8030_1008u32, 0x8030_100Cu32);
    wf(&mut w, sx, -1000.0);
    wf(&mut w, sy, -1000.0);
    let v = if x.local != 0 {
        x.p
    } else {
        let d = [sub(x.p[0], x.cam[0])?, sub(x.p[1], x.cam[1])?, sub(x.p[2], x.cam[2])?];
        for (i, c) in d.iter().enumerate() {
            wf(&mut w, f + 0x48 + 4 * i as u32, *c);
        }
        d
    };
    let mut r = point4(v, &x.m)?;
    if x.mirror & 0x4000 != 0 {
        r[0] = -ok(r[0])?;
    }
    for (i, c) in r.iter().enumerate() {
        wf(&mut w, f + 0x54 + 4 * i as u32, *c);
    }
    // Widened for the compare: a NaN w (inf - inf in the product) is guarded.
    let wv = ok(r[3])?;
    if !(x.k < f64::from(wv)) {
        return Some(w);
    }
    let big_x = add(mul(add(div(r[0], wv)?, 1.0)?, hw2)?, ox)?;
    let big_y = add(mul(sub(1.0, div(r[1], wv)?)?, hh2)?, oy)?;
    wf(&mut w, zw, div(r[2], wv)?);
    wf(&mut w, ww, wv);
    if sub(ox, 8.0)? < big_x
        && big_x < add(add(hw, ox)?, 8.0)?
        && sub(oy, 8.0)? < big_y
        && big_y < add(add(hh, oy)?, 8.0)?
    {
        wf(&mut w, sx, big_x);
        wf(&mut w, sy, big_y);
    }
    Some(w)
}

fn proj_case() -> BoxedStrategy<Proj> {
    let vp = prop_oneof![3 => Just([640i16, 480, 1280, 960]), 1 => Just([-640i16, 480, 1283, -957]), 2 => prop::array::uniform4(any::<i16>())];
    let persp = (0.5f32..2.0, 0.5f32..2.0).prop_map(|(a, b)| {
        let mut m = [0.0f32; 16];
        (m[0], m[5], m[10], m[11], m[14]) = (a, b, 1.0, 1.0, -1.0);
        m
    });
    let m = prop_oneof![2 => persp, 1 => prop::array::uniform16(ordinary())];
    let k = prop_oneof![3 => Just(None), 1 => (-1.0f64..1.0).prop_map(Some)];
    (vp, prop::array::uniform3(ordinary()), m, prop::array::uniform3(ordinary()), prop_oneof![Just(0u32), Just(1u32)], prop_oneof![Just(0u32), Just(0x4000u32), any::<u32>()], k, any::<[u32; 4]>())
        .prop_map(|(vp, p, m, cam, local, mirror, k, junk)| Proj { vp, p, m, cam, local, mirror, k: k.unwrap_or(0.1), junk })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(768))]

    #[test]
    fn func_8000EBE8(seed: u64, x in proj_case(), tie: bool) {
        let mut x = x;
        if tie {
            // K equal to w, the threshold's tie.
            let v = if x.local != 0 { Some(x.p) } else { (|| Some([sub(x.p[0], x.cam[0])?, sub(x.p[1], x.cam[1])?, sub(x.p[2], x.cam[2])?]))() };
            if let Some(r) = v.and_then(|v| point4(v, &x.m)) {
                x.k = f64::from(r[3]);
            }
        }
        proj_check(seed, &x)?;
    }
}

fn proj_check(seed: u64, x: &Proj) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    let vp = 0x8030_0000u32;
    for (i, v) in x.vp.iter().enumerate() {
        wh(&mut s, vp + 0x10 + if i < 2 { 2 * i as u32 } else { 8 + 2 * (i as u32 - 2) }, *v as u16);
    }
    for (i, v) in x.m.iter().enumerate() {
        wf(&mut s, MTX + 4 * i as u32, *v);
    }
    for (i, v) in x.cam.iter().enumerate() {
        wf(&mut s, CAM + 4 * i as u32, *v);
    }
    wr(&mut s, MIRROR, x.mirror);
    wr(&mut s, K_AT, (x.k.to_bits() >> 32) as u32);
    wr(&mut s, K_AT + 4, x.k.to_bits() as u32);
    let p = 0x8030_0100u32;
    for (i, v) in x.p.iter().enumerate() {
        wf(&mut s, p + 4 * i as u32, *v);
    }
    for (i, v) in x.junk.iter().enumerate() {
        wr(&mut s, 0x8030_1000 + 4 * i as u32, *v);
    }
    wr(&mut s, SP_AT + 0x10, 0x8030_1008);
    wr(&mut s, SP_AT + 0x14, 0x8030_100C);
    wr(&mut s, SP_AT + 0x18, x.local);
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(vp), sext(p), sext(0x8030_1000), sext(0x8030_1004));
    let Some(want) = project(&s, x) else { return Ok(()) };
    let after = run("func_8000EBE8", misc::func_8000EBE8, &s)?;
    same_memory(&after, &want)
}

/// X exactly at the left margin `ox - 8` (not drawn: the test is strict).
#[test]
fn func_8000EBE8_left_margin() {
    let mut m = [0.0f32; 16];
    (m[0], m[5], m[10], m[15]) = (1.0, 1.0, 1.0, 1.0);
    let x = Proj { vp: [64, 64, 400, 400], p: [-1.5, 0.0, 0.5], m, cam: [0.0; 3], local: 1, mirror: 0, k: 0.1, junk: [0; 4] };
    proj_check(1, &x).unwrap();
}

// ---- func_800116E8: switch the listed records off ----

const IDS: u32 = 0x800D_6140;
const BYTES: u32 = 0x800D_68C0;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_800116E8(seed: u64, ids in prop::collection::vec(prop_oneof![6 => Just(u32::MAX), 4 => 0u32..200, 1 => Just((-201i32) as u32), 1 => Just((-103i32) as u32),
                                                                        1 => Just((-104i32) as u32), 1 => any::<i16>().prop_map(|v| v as i32 as u32), 1 => any::<u32>()], 80),
                     bytes in prop::collection::vec(any::<u8>(), 80), flags in prop::collection::vec(any::<u32>(), 200), globals: [u32; 3]) {
        let mut s = state(seed);
        for (i, v) in ids.iter().enumerate() {
            wr(&mut s, IDS + 4 * i as u32, *v);
        }
        for (i, b) in bytes.iter().enumerate() {
            wb(&mut s, BYTES + i as u32, *b);
        }
        for (i, f) in flags.iter().enumerate() {
            wr(&mut s, RECORDS + 32 * i as u32 + 0x14, *f);
        }
        wr(&mut s, 0x8009_B778, globals[0]);
        wr(&mut s, 0x8009_B77C, globals[1]);
        wr(&mut s, 0x8009_B780, globals[2]);
        let mut want = s.clone();
        let f = SP_AT - 0x28;
        for (off, r) in [(0x24, RA), (0x20, S3), (0x18, S1), (0x14, S0), (0x1C, S2)] {
            wr(&mut want, f + off, s.ctx.gpr[r] as u32);
        }
        for i in 0..80u32 {
            // Read as the loop goes: a record's flags may lie in the list.
            let v = word(&want, IDS + 4 * i);
            if v != u32::MAX {
                // func_8000A920(v, 0): its spill, then the switch.
                let id = v as u16 as i16;
                wr(&mut want, f, id as i32 as u32);
                match id {
                    -201 => wr(&mut want, 0x8009_B778, 0),
                    -103 => wb(&mut want, 0x8009_B77F, 0),
                    -104 => wb(&mut want, 0x8009_B783, 0),
                    i if i >= 0 => {
                        let a = RECORDS + 32 * i as u32 + 0x14;
                        let fl = word(&want, a) & !0x20;
                        wr(&mut want, a, fl);
                    }
                    _ => {}
                }
            }
            wb(&mut want, BYTES + i, 0);
        }
        let after = run("func_800116E8", misc::func_800116E8, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- The thunks: func_800118F8, func_80011EA4, func_800129B8 ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn func_800118F8(seed: u64, w in 1i16..64, h in 1i16..64, xs in prop::array::uniform2(prop_oneof![Just(-1000i32), 0i32..60]), e in prop::array::uniform2(prop_oneof![Just(-1i32), Just(0i32)])) {
        // func_8000F5A0's domain: a small depth buffer, its lists off (or
        // in range), no counted list.
        let mut s = state(seed);
        let z = 0x8034_0000u32;
        wr(&mut s, 0x8011_4528, z);
        wh(&mut s, SCREEN, w as u16);
        wh(&mut s, SCREEN + 2, h as u16);
        for k in 0..2u32 {
            wr(&mut s, 0x8009_B814 + 4 * k, e[k as usize] as u32);
            wr(&mut s, 0x800D_57A0 + 4 * k, xs[k as usize] as u32);
            wr(&mut s, 0x800D_57A8 + 4 * k, 8);
        }
        for i in 0..20u32 {
            for base in [0x800D_5AF8u32, 0x800D_5B98] {
                wr(&mut s, base + 4 * i, u32::MAX);
            }
        }
        for j in 0..40u32 {
            wb(&mut s, 0x800D_5C38 + j, 0);
        }
        wr(&mut s, 0x8009_B86C, 0);
        let want = thunk(&s, 0x18, &[(0x14, s.ctx.gpr[RA] as u32)], &[], imports::func_8000F5A0);
        let after = run("func_800118F8", misc::func_800118F8, &s)?;
        same_state(&after, &want)?;
    }

    #[test]
    fn func_80011EA4(seed: u64, k: u64, count in 0u32..6, table: [u32; 6]) {
        let mut s = state(seed);
        // func_80011E54's table and count (see its port).
        for (i, v) in table.iter().enumerate() {
            wr(&mut s, 0x800D_6940 + 4 * i as u32, *v);
        }
        wr(&mut s, misc::POINTER_COUNT, count);
        s.ctx.gpr[A0] = k;
        let t6 = sext((k as u32) << 16);
        let want = thunk(&s, 0x18, &[(0x14, s.ctx.gpr[RA] as u32), (0x18, k as u32)], &[(T6, t6), (A0, sext(k as u16 as i16 as i32 as u32))], imports::func_80011E54);
        let after = run("func_80011EA4", misc::func_80011EA4, &s)?;
        same_state(&after, &want)?;
    }

    #[test]
    fn func_800129B8(seed: u64, k in 0u32..4, text in prop::collection::vec(prop_oneof![4 => 0x20u8..=b'z', 1 => Just(0u8)], 1..12)) {
        let mut s = state(seed);
        // A font per k (func_800129E4's layout): characters 0x20..'z', a
        // glyph table of 16-byte records with advances, no extended table.
        let (fonts, str_at) = (0x8032_0000u32, 0x8033_0000u32);
        for i in 0..4u32 {
            let font = fonts + 0x1000 * i;
            wr(&mut s, 0x800D_6940 + 4 * i, font);
            wb(&mut s, font + 0x5A, 0x20);
            wb(&mut s, font + 0x5B, b'z');
            wr(&mut s, font + 0x5C, font + 0x100);
            wr(&mut s, font + 0x60, 0);
            for c in 0..0x60u32 {
                wh(&mut s, font + 0x100 + 16 * c + 2, (c % 7 + i) as u16);
            }
        }
        for (i, b) in text.iter().enumerate() {
            wb(&mut s, str_at + i as u32, *b);
        }
        wb(&mut s, str_at + text.len() as u32, 0);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(str_at), u64::from(k));
        let font = word(&s, 0x800D_6940 + 4 * k);
        let want = thunk(&s, 0x18, &[(0x14, s.ctx.gpr[RA] as u32)], &[(T6, u64::from(k << 2)), (A1, sext(font))], imports::func_800129E4);
        let after = run("func_800129B8", misc::func_800129B8, &s)?;
        same_state(&after, &want)?;
    }
}
