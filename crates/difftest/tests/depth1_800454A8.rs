//! Depth 1 at 0x800454A8..0x8004F254 (game::misc): a race setup from its
//! objects, two node groups, the scene root and its lists, a two-record
//! gauge (FCR31 idiom), the track-select grid, the menu input state and
//! the "NAsn" messages. Recompiled C vs Rust with the callees as C. The
//! models replay the callees' C on a copy of the state in the same order
//! with the same arguments (and the s registers the port holds), adding
//! the functions' own stores, and whole RDRAM is compared.

// Tests are named after the functions (func_800454A8), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::imports;
use game::recomp::{reg::*, RecompFn};
use game::misc;
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

fn sext16(v: u32) -> u64 {
    v as u16 as i16 as i64 as u64
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
fn sub(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? - ok(b)?)
}

/// IDO's float → unsigned idiom as the oracle runs it (see
/// depth1_800290A4.rs).
fn u(x: f32) -> u32 {
    let w = if x.is_nan() || x.abs() > 2_147_483_648.0 {
        0
    } else if x.abs() == 2_147_483_648.0 {
        i32::MIN
    } else {
        x.trunc() as i32
    };
    if w < 0 {
        u32::MAX
    } else {
        w as u32
    }
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

const O: u32 = 0x8030_0000;
const ENTRIES_ARRAY: u32 = 0x8032_0000;

// ---- a race setup ----

fn key() -> BoxedStrategy<f32> {
    prop_oneof![3 => ordinary(), 1 => Just(f32::NAN), 1 => Just(-0.0f32), 1 => Just(f32::INFINITY)].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_800454A8(seed: u64, keys in proptest::collection::vec(key(), 0..5), null: bool, flag in prop_oneof![Just(0u32), any::<u32>()], laps: i32) {
        let mut s = state(seed);
        let list = 0x8030_2000u32;
        for (k, &x) in keys.iter().enumerate() {
            let v = 0x8031_0000 + 0x200 * k as u32;
            wr(&mut s, list + 4 * k as u32, v);
            wf(&mut s, v + 0x108, x);
        }
        wr(&mut s, list + 4 * keys.len() as u32, 0);
        wr(&mut s, O + 0x50, flag);
        wr(&mut s, O + 0xC4, 0x8030_3000);
        wr(&mut s, 0x8030_3004, laps as u32);
        wr(&mut s, 0x8009_B790, ENTRIES_ARRAY);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(O), if null { 0 } else { sext(list) });
        let mut w = s.clone();
        saves(&mut w, &s, 0x28, &[(0x20, S0), (0x24, RA)]);
        let mut m = 0.0f32;
        if !null {
            for &x in &keys {
                if m < x {
                    m = x;
                }
            }
        }
        wf(&mut w, O + 0xB68, m);
        wr(&mut w, O + 0xC, 0);
        if flag != 0 {
            wf(&mut w, O + 0x58, (laps as f32 - 1.0) / m);
        }
        call_c(&mut w, 0x28, &[(A0, 5), (A1, 1), (A2, sext(O + 0xB28)), (A3, 0)], imports::func_8000AEFC);
        wr(&mut w, SP_AT - 0x28 + 0x10, 0xFF);
        call_c(&mut w, 0x28, &[(A0, sext(-103i32 as u32)), (A1, 0), (A2, 0), (A3, 0)], imports::func_8000AB24);
        let after = run("func_800454A8", misc::func_800454A8, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- node groups and the scene root ----

fn group_model(s: &State, g: u32, count: u32, list: u32, at: u32, first: u32, stride: u32, per: u32) -> State {
    let mut w = s.clone();
    saves(&mut w, s, 0x40, &[(0x18, S0), (0x1C, S1), (0x20, S2), (0x24, S3), (0x28, S4), (0x2C, S5), (0x30, S6), (0x34, S7), (0x38, FP), (0x3C, RA)]);
    call_c(&mut w, 0x40, &[(A0, sext(g)), (A1, 0x5064)], imports::func_80018324);
    wr(&mut w, g + 0x14, count);
    wr(&mut w, g + 0x18, list);
    wr(&mut w, O + at, g);
    for i in 0..count / per {
        for j in 0..per {
            let n = first + stride * i + 0x58 * j;
            call_c(&mut w, 0x40, &[(A0, sext(n)), (A1, 0xD065)], imports::func_80018324);
            wr(&mut w, n + 0x14, 0);
            wr(&mut w, n + 0x18, 0);
            wr(&mut w, list + 4 * (per * i + j), n);
        }
    }
    w
}

const LIST_A: u32 = 0x8011_A2A8;
const LIST_A_END: u32 = 0x8011_A504;
const LIST_B: u32 = 0x8011_A508;
const LIST_B_END: u32 = 0x8011_A764;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn func_80045DA0(seed: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = sext(O);
        let w = group_model(&s, O + 0x1B30, 12, O + 0x16E0, 0xBC4, O + 0x1710, 0xB0, 2);
        let after = run("func_80045DA0", misc::func_80045DA0, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80045E80(seed: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = sext(O);
        let w = group_model(&s, O + 0x16C4, 18, O + 0x104C, 0xBB8, O + 0x1094, 0x108, 3);
        let after = run("func_80045E80", misc::func_80045E80, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80046764(seed: u64, count in 0u32..300) {
        let mut s = state(seed);
        wr(&mut s, 0x8009_A2A0, count);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        for a in (LIST_A..LIST_A_END).chain(LIST_B..LIST_B_END).step_by(4) {
            wr(&mut w, a, 0);
        }
        call_c(&mut w, 0x18, &[], imports::func_80005B80);
        let after = run("func_80046764", misc::func_80046764, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80046974(seed: u64) {
        let s = state(seed);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        call_c(&mut w, 0x18, &[(A0, sext(0x8011_A288)), (A1, 0x5064)], imports::func_80018324);
        wr(&mut w, 0x8011_A29C, 0x97);
        wr(&mut w, 0x8011_A2A0, LIST_A);
        let after = run("func_80046974", misc::func_80046974, &s)?;
        same_memory(&after, &w)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Slots holding objects whose first word is in the root's list (often
    /// several times), null slots, and objects inside the list itself (so
    /// a clear changes the word being looked for).
    #[test]
    fn func_80046870(seed: u64, slots in proptest::collection::vec(prop_oneof![4 => Just(None), 3 => (0u32..8).prop_map(|k| Some(0x8031_0000 + 4 * k)), 1 => (0u32..151).prop_map(|k| Some(LIST_A + 4 * k))], 146),
                     values in prop::array::uniform8(prop_oneof![Just(0u32), Just(1u32), any::<u32>()]), list in proptest::collection::vec(0usize..10, 151)) {
        let mut s = state(seed);
        for (k, &v) in values.iter().enumerate() {
            wr(&mut s, 0x8031_0000 + 4 * k as u32, v);
        }
        // The list: copies of the objects' words, or random.
        for (k, &c) in list.iter().enumerate() {
            let v = if c < 8 { values[c] } else { seed as u32 ^ (k as u32).wrapping_mul(0x9E37_79B9) };
            wr(&mut s, LIST_A + 4 * k as u32, v);
        }
        for (k, p) in slots.iter().enumerate() {
            wr(&mut s, 0x8011_A51C + 4 * k as u32, p.unwrap_or(0));
        }
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        for slot in (0x8011_A51Cu32..LIST_B_END).step_by(4) {
            let p = word(&w, slot);
            if p == 0 {
                continue;
            }
            for a in (LIST_A..LIST_A_END).step_by(4) {
                if word(&w, a) == word(&w, p) {
                    wr(&mut w, a, 0);
                }
            }
            wr(&mut w, slot, 0);
        }
        call_c(&mut w, 0x18, &[], imports::func_80005B80);
        let after = run("func_80046870", misc::func_80046870, &s)?;
        same_memory(&after, &w)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// A slot pointing at one of the list's first words, so clearing it
    /// changes the word looked for (re-read after each clear), with the
    /// list drawn from two values so the old one still has copies.
    #[test]
    fn func_80046870_self(seed: u64, j in 0u32..6, values in prop::array::uniform2(1u32..4), picks in proptest::collection::vec(any::<bool>(), 151), rest in prop::option::of(0u32..151)) {
        let mut s = state(seed);
        for (k, &b) in picks.iter().enumerate() {
            wr(&mut s, LIST_A + 4 * k as u32, values[usize::from(b)]);
        }
        for k in 0..146u32 {
            wr(&mut s, 0x8011_A51C + 4 * k, 0);
        }
        wr(&mut s, 0x8011_A51C, LIST_A + 4 * j);
        if let Some(r) = rest {
            wr(&mut s, 0x8011_A520, LIST_A + 4 * r);
        }
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        for slot in (0x8011_A51Cu32..LIST_B_END).step_by(4) {
            let p = word(&w, slot);
            if p == 0 {
                continue;
            }
            for a in (LIST_A..LIST_A_END).step_by(4) {
                if word(&w, a) == word(&w, p) {
                    wr(&mut w, a, 0);
                }
            }
            wr(&mut w, slot, 0);
        }
        call_c(&mut w, 0x18, &[], imports::func_80005B80);
        let after = run("func_80046870", misc::func_80046870, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- the gauge ----

const COUNTER: u32 = 0x800A_4BB8;

fn gauge_model(s: &State, kind8: bool, x: u32, y: u32, f: f32, uu: f32, v: f32) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x40;
    saves(&mut w, s, 0x40, &[(0x20, S0), (0x24, RA)]);
    let (a, b, x, y) = if kind8 { (0.375f32, 3.75f32, x, y) } else { (mul(sub(uu, 2.0)?, 0.125)?, mul(v, 0.125)?, x.wrapping_add(3), y.wrapping_add(1)) };
    let c = sub(1.0, f)?;
    let (cb, cv, bf, c255, f255) = (mul(c, b)?, mul(c, v)?, mul(b, f)?, mul(c, 255.0)?, mul(255.0, f)?);
    let yy = add(y as i32 as f32, cv)?;
    wf(&mut w, sp + 0x38, a);
    wf(&mut w, sp + 0x34, b);
    wr(&mut w, SP_AT + 8, y);
    wr(&mut w, SP_AT + 4, x);
    wr(&mut w, SP_AT + 0xC, f.to_bits());
    let take = |w: &mut State| {
        let k = word(w, COUNTER);
        wr(w, COUNTER, k.wrapping_add(1));
        sext16(k.wrapping_add(0x8D))
    };
    let id = take(&mut w);
    w.ctx.gpr[S0] = id;
    call_c(&mut w, 0x40, &[(A0, id), (A1, 1)], imports::func_8000A920);
    wr(&mut w, sp + 0x30, sext16(x) as u32);
    call_c(&mut w, 0x40, &[(A0, id), (A1, sext16(x)), (A2, sext16(y))], imports::func_8000AA04);
    wf(&mut w, sp + 0x2C, c);
    call_c(&mut w, 0x40, &[(A0, id), (A1, sext(a.to_bits())), (A2, sext(cb.to_bits()))], imports::func_8000AAC0);
    wr(&mut w, sp + 0x10, 0xFF);
    call_c(&mut w, 0x40, &[(A0, id), (A1, 0), (A2, 0), (A3, 0)], imports::func_8000AB24);
    let id = take(&mut w);
    w.ctx.gpr[S0] = id;
    call_c(&mut w, 0x40, &[(A0, id), (A1, 1)], imports::func_8000A920);
    call_c(&mut w, 0x40, &[(A0, id), (A1, sext16(x)), (A2, sext16(trunc(yy)))], imports::func_8000AA04);
    call_c(&mut w, 0x40, &[(A0, id), (A1, sext(a.to_bits())), (A2, sext(bf.to_bits()))], imports::func_8000AAC0);
    wr(&mut w, sp + 0x10, 0xFF);
    call_c(&mut w, 0x40, &[(A0, id), (A1, u64::from(u(c255) & 0xFF)), (A2, u64::from(u(f255) & 0xFF)), (A3, 0)], imports::func_8000AB24);
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80047A78(seed: u64, kind8: bool, x in prop_oneof![-0x40i32..0x180, any::<i32>()], y in prop_oneof![-0x40i32..0x100, any::<i32>()],
                     f in prop_oneof![3 => 0.0f32..=1.0, 1 => Just(0.0f32), 1 => Just(1.0f32), 1 => ordinary()], uu in ordinary(), v in ordinary(), k in 0u32..40) {
        let mut s = state(seed);
        wr(&mut s, O + 8, if kind8 { 8 } else { seed as u32 & 7 });
        wr(&mut s, COUNTER, k);
        wf(&mut s, SP_AT + 0x14, uu);
        wf(&mut s, SP_AT + 0x18, v);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(O), sext(x as u32), sext(y as u32), sext(f.to_bits()));
        let model = gauge_model(&s, kind8, x as u32, y as u32, f, uu, v);
        prop_assume!(model.is_some());
        let after = run("func_80047A78", misc::func_80047A78, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

/// Separators for a fused `f32(y) + c * v`, found by an exact search: with
/// `y = 1000` and `c * v` just below `1 - 2^-15`, the rounded product puts
/// the sum on the tie `1001 - 2^-15`, which rounds to 1001 (even), while the
/// exact sum rounds down, so the truncated halfword differs (1001 vs 1000).
#[test]
fn func_80047A78_fused_separators() {
    for (seed, (f, v)) in [(0x3D00_001Cu32, 0x3F84_2001u32), (0x3D00_0023, 0x3F84_2001), (0x3D00_0038, 0x3F84_2002)].into_iter().enumerate() {
        let mut s = state(seed as u64);
        wr(&mut s, O + 8, 8);
        wr(&mut s, COUNTER, 3);
        wr(&mut s, SP_AT + 0x18, v);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(O), 0x40, 1000, sext(f));
        let (f, v) = (f32::from_bits(f), f32::from_bits(v));
        assert_eq!(trunc(1000.0 + (1.0 - f) * v), 1001);
        let model = gauge_model(&s, true, 0x40, 1000, f, rf(&s, SP_AT + 0x14), v).unwrap();
        let after = compare("func_80047A78", misc::func_80047A78, &s).unwrap_or_else(|d| panic!("{d}"));
        same_memory(&after, &model).unwrap();
    }
}

// ---- the track-select grid ----

const COLOURS: [[u32; 3]; 4] = [[0x32, 0xFF, 0xFF], [0x44, 0xFF, 0x3E], [0xA3, 0xBE, 0x11], [0x9D, 0x59, 0x20]];

fn colour(w: &mut State, frame: u32, id: u64, rgb: [u32; 3]) {
    wr(w, SP_AT - frame + 0x10, 0xFE);
    call_c(w, frame, &[(A0, id), (A1, u64::from(rgb[0])), (A2, u64::from(rgb[1])), (A3, u64::from(rgb[2]))], imports::func_8000AB24);
}

fn grid_model(s: &State, full: bool) -> State {
    let mut w = s.clone();
    let sp = SP_AT - 0x78;
    saves(&mut w, s, 0x78, &[(0x20, S0), (0x24, S1), (0x28, S2), (0x2C, S3), (0x30, S4), (0x34, S5), (0x38, S6), (0x3C, S7), (0x40, FP), (0x44, RA)]);
    let img = |w: &State, off: u32| sext(word(w, O + off));
    for id in 0x7Fu64..0x9F {
        let im = img(&w, 0xC8);
        call_c(&mut w, 0x78, &[(A0, id), (A1, im)], imports::func_8000A44C);
    }
    if !full {
        return w;
    }
    for c in 0..4u32 {
        wr(&mut w, sp + 0x60, 0x8011_3E60 + 2 * c);
        wr(&mut w, sp + 0x58, 7 * c);
        wr(&mut w, sp + 0x4C, c);
        for k in 0..7u32 {
            let id = sext16(0x60 + 7 * c + k);
            let h = i32::from(half(&w, 0x8011_3E6C + 2 * c) as i16) >> ((2 * k) & 31);
            let q = h % 4;
            let im = img(&w, 0xC0);
            call_c(&mut w, 0x78, &[(A0, id), (A1, im)], imports::func_8000A44C);
            call_c(&mut w, 0x78, &[(A0, id), (A1, 0x8000)], imports::func_8000AC34);
            colour(&mut w, 0x78, id, COLOURS[c as usize]);
            call_c(&mut w, 0x78, &[(A0, sext(O)), (A1, u64::from(c)), (A2, u64::from(k))], imports::func_8002DAD0);
            if w.ctx.gpr[V0] == 0 {
                colour(&mut w, 0x78, id, [0x80, 0x80, 0x80]);
            }
            if byte(&w, O + 0x6C) != 0 && (1..=3).contains(&q) {
                let off = [0xBC, 0xB8, 0xB4][(q - 1) as usize];
                let im = img(&w, off);
                call_c(&mut w, 0x78, &[(A0, id), (A1, im)], imports::func_8000A44C);
            }
            let id2 = sext16((id as u32).wrapping_add(0x1C));
            let im = img(&w, 0xC4);
            call_c(&mut w, 0x78, &[(A0, id2), (A1, im)], imports::func_8000A44C);
            call_c(&mut w, 0x78, &[(A0, id2), (A1, 0x8000)], imports::func_8000AC34);
            colour(&mut w, 0x78, id2, [0xA3, 0xBE, 0x11]);
        }
    }
    wr(&mut w, sp + 0x60, 0x8011_3E68);
    wr(&mut w, sp + 0x58, 28);
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn func_8004BAC8(seed: u64, full in prop_oneof![1 => Just(0u32), 3 => Just(1u32), 1 => any::<u32>()], medals: [u16; 4], unlocked: [u8; 4], which in prop_oneof![Just(0u8), Just(1u8)], count in 0u32..0x100) {
        let mut s = state(seed);
        for c in 0..4u32 {
            wh(&mut s, 0x8011_3E6C + 2 * c, medals[c as usize]);
            wb(&mut s, 0x8011_3E68 + c, unlocked[c as usize]);
            wb(&mut s, 0x8011_368C + c, !unlocked[c as usize]);
        }
        wb(&mut s, O + 0x6C, which);
        wr(&mut s, 0x8009_B770, count);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(O), sext(full));
        let w = grid_model(&s, full != 0);
        let after = run("func_8004BAC8", misc::func_8004BAC8, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- the menu input state ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8004E0A0(seed: u64, i in 0u32..4, on in prop_oneof![1 => Just(0u32), 3 => Just(1u32)], held: u32, state_word: u32, newly: u32,
                     dx in prop_oneof![Just(0u32), Just(1u32), Just(u32::MAX), any::<u32>()], dy in prop_oneof![Just(0u32), Just(1u32), Just(u32::MAX), any::<u32>()]) {
        let mut s = state(seed);
        let (n_at, s_at, h_at) = (0x800A_4BA4 + 4 * i, 0x800A_4B94 + 4 * i, 0x800D_76F0 + 4 * i);
        wr(&mut s, 0x800A_4BD8, on);
        wr(&mut s, h_at, held);
        wr(&mut s, s_at, state_word);
        wr(&mut s, n_at, newly);
        wr(&mut s, 0x800A_5208 + 4 * i, dx);
        wr(&mut s, 0x800A_5218 + 4 * i, dy);
        s.ctx.gpr[A0] = u64::from(i);
        let mut w = s.clone();
        let sp = SP_AT - 0x30;
        saves(&mut w, &s, 0x30, &[(0x18, S0), (0x1C, S1), (0x20, S2), (0x24, RA)]);
        wr(&mut w, n_at, 0);
        if on != 0 {
            wr(&mut w, sp + 0x28, n_at);
            (w.ctx.gpr[S0], w.ctx.gpr[S1], w.ctx.gpr[S2]) = (u64::from(i), sext(h_at), u64::from(4 * i));
            for k in 0..24 {
                let b = 1u32 << k;
                let h = word(&w, h_at);
                call_c(&mut w, 0x30, &[(A0, u64::from(i)), (A1, sext(h & b)), (A2, sext(b))], imports::func_8004E488);
            }
            let st = word(&w, s_at);
            let (x_at, y_at) = (0x800A_5208 + 4 * i, 0x800A_5218 + 4 * i);
            if st & 1 << 18 != 0 {
                wr(&mut w, x_at, 0);
            } else if st & 1 << 16 != 0 {
                wr(&mut w, x_at, u32::MAX);
            } else if st & 1 << 17 != 0 {
                wr(&mut w, x_at, 1);
            }
            if st & 1 << 19 != 0 {
                wr(&mut w, y_at, 0);
            } else if st & 1 << 14 != 0 {
                wr(&mut w, y_at, u32::MAX);
            } else if st & 1 << 15 != 0 {
                wr(&mut w, y_at, 1);
            }
            let n = word(&w, n_at);
            if n & 1 << 20 != 0 && word(&w, x_at) == u32::MAX {
                wr(&mut w, s_at, st | 0x1_0000);
            }
            if n & 1 << 21 != 0 && word(&w, x_at) == 1 {
                let v = word(&w, s_at);
                wr(&mut w, s_at, v | 0x2_0000);
            }
            if n & 1 << 22 != 0 && word(&w, y_at) == u32::MAX {
                let v = word(&w, s_at);
                wr(&mut w, s_at, v | 0x4000);
            }
            if n & 1 << 23 != 0 && word(&w, y_at) == 1 {
                let v = word(&w, s_at);
                wr(&mut w, s_at, v | 0x8000);
            }
        }
        let after = run("func_8004E0A0", misc::func_8004E0A0, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- the NAsn messages ----

const POOL_LIST: u32 = 0x8030_0000;
const POOL_DESCS: u32 = 0x8030_0100;
const POOL_ELEMS: u32 = 0x8030_1000;
/// func_8005F31C, a verified callback: `[elem + 0x14] += 1`, `[elem + 0x18]
/// = arg` (see indirect.rs).
const MARK: u32 = 0x8005_F31C;
const CMAN: u32 = 0x634D_616E;
const LOCL: u32 = 0x4C6F_636C;
const RECS: u32 = 0x8011_8F90;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A "cMan" pool whose elements carry tags 0..3 (some flagged), and
    /// records that are "Locl", flagged by bit 5, or neither.
    #[test]
    fn func_8004F254(seed: u64, n in prop_oneof![Just(0i8), Just(-1i8), 1i8..6], kinds in prop::array::uniform6(0u8..3), flagged: [bool; 4], tags in prop::array::uniform4(0i16..4)) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 2, POOL_LIST, 0x800);
        wr(&mut s, 0x800A_2170, POOL_LIST);
        wr(&mut s, POOL_LIST, POOL_DESCS);
        wr(&mut s, POOL_LIST + 4, 0);
        wr(&mut s, POOL_DESCS, CMAN);
        wr(&mut s, POOL_DESCS + 8, 4);
        wr(&mut s, POOL_DESCS + 0xC, 0x68);
        wr(&mut s, POOL_DESCS + 0x10, POOL_ELEMS);
        wr(&mut s, POOL_DESCS + 0x24, MARK);
        for k in 0..4u32 {
            let e = POOL_ELEMS + 0x68 * k;
            wr(&mut s, e, CMAN);
            wh(&mut s, e + 4, tags[k as usize] as u16);
            let h = half(&s, e + 6);
            wh(&mut s, e + 6, if flagged[k as usize] { h | 0x100 } else { h & !0x100 });
        }
        for (k, &kind) in kinds.iter().enumerate() {
            let r = RECS + 0x88 * k as u32;
            wr(&mut s, r + 4, if kind == 0 { LOCL } else { seed as u32 | 1 });
            let fl = word(&s, r + 8);
            wr(&mut s, r + 8, if kind == 1 { fl | 0x20 } else { fl & !0x20 });
        }
        wb(&mut s, O + 0x71, n as u8);
        s.ctx.gpr[A0] = sext(O);
        let mut w = s.clone();
        let sp = SP_AT - 0x80;
        saves(&mut w, &s, 0x80, &[(0x18, S0), (0x1C, S1), (0x20, S2), (0x24, S3), (0x28, S4), (0x2C, S5), (0x30, S6), (0x34, S7), (0x38, FP), (0x3C, RA)]);
        wr(&mut w, SP_AT, O);
        let mut count = i32::from(byte(&w, O + 0x71) as i8);
        let (mut j, mut idx) = (0u32, 0i32);
        while idx < count {
            let r = RECS + 0x88 * idx as u32;
            if word(&w, r + 4) == LOCL || word(&w, r + 8) & 0x20 != 0 {
                wr(&mut w, sp + 0x5C, 0x4E41_736E);
                wr(&mut w, sp + 0x60, j + 1);
                let x = word(&w, r + 0x84);
                wr(&mut w, sp + 0x64, x);
                // The s registers the port holds (func_8003F714 saves s0).
                w.ctx.gpr[S0] = sext(r);
                w.ctx.gpr[S1] = u64::from(j);
                w.ctx.gpr[S2] = u64::from(j + 1);
                w.ctx.gpr[S3] = sext(idx as u32);
                (w.ctx.gpr[S4], w.ctx.gpr[S5], w.ctx.gpr[S6], w.ctx.gpr[S7], w.ctx.gpr[FP]) = (sext(LOCL), sext(0x4E41_736E), sext(CMAN), sext(sp + 0x5C), sext(0x800A_4BE0));
                call_c(&mut w, 0x80, &[(A0, sext(CMAN)), (A1, u64::from(j))], imports::func_8003F714);
                let e = w.ctx.gpr[V0];
                call_c(&mut w, 0x80, &[(A0, e), (A1, sext(sp + 0x5C))], imports::func_8003F99C);
                j += 1;
                count = i32::from(byte(&w, O + 0x71) as i8);
            }
            idx += 1;
        }
        let after = run("func_8004F254", misc::func_8004F254, &s)?;
        same_memory(&after, &w)?;
    }
}
