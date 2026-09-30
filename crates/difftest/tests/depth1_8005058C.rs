//! Depth 1 at 0x8005058C..0x8005B2D0 (game::misc): a camera transition
//! (start and step), a progress figure, the menu buttons pressed, a
//! texture's alpha from a level, a fading HUD panel (FCR31 idioms in double
//! and single), a HUD bar, hiding a HUD set and an activity test.
//! Recompiled C vs Rust with the callees as C. The models replay the
//! callees' C on a copy of the state in the same order with the same
//! arguments (and the s registers the port holds), adding the functions'
//! own stores; whole RDRAM (and the result register) is compared.

// Tests are named after the functions (func_8005058C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::imports;
use game::misc;
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

fn sext16(v: u32) -> u64 {
    v as u16 as i16 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;

/// The HUD record table (32-byte records), randomised in the tests of
/// functions that set records: the background pattern is the same in every
/// case.
const RECORDS: u32 = 0x800D_2190;

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
fn div(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? / ok(b)?)
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

fn some_word() -> BoxedStrategy<u32> {
    prop_oneof![Just(0u32), Just(1u32), any::<u32>()].boxed()
}

// ---- the camera transition ----

/// The six 4x4 matrices at 0x80118D60, translation rows at +0x30.
const MATS: u32 = 0x8011_8D60;
const D60: u32 = 0x8011_8D60;
const DA0: u32 = 0x8011_8DA0;
const DE0: u32 = 0x8011_8DE0;
const E20: u32 = 0x8011_8E20;
const E60: u32 = 0x8011_8E60;
const EA0: u32 = 0x8011_8EA0;
const MODE: u32 = 0x800A_4BC0;
const O: u32 = 0x8030_0000;
const VECS: u32 = 0x8030_1000;

fn matrix_values() -> BoxedStrategy<Vec<f32>> {
    prop::collection::vec(prop_oneof![8 => ordinary(), 1 => Just(f32::INFINITY), 1 => Just(f32::NEG_INFINITY)], 96).boxed()
}

fn fill_matrices(s: &mut State, vals: &[f32]) {
    for (k, &v) in vals.iter().enumerate() {
        wf(s, MATS + 4 * k as u32, v);
    }
}

/// Translation component `k` of the matrix at `m`.
fn tr(w: &State, m: u32, k: u32) -> f32 {
    rf(w, m + 0x30 + 4 * k)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// `p` and `q` in their own area or anywhere in the matrices (so the
    /// saves and the targets overlap); finite values only, so no
    /// difference is NaN.
    #[test]
    fn func_8005058C(seed: u64, vals in prop::collection::vec(ordinary(), 96), pq in prop::array::uniform6(ordinary()),
                     at in prop::array::uniform2(prop_oneof![Just(None), (0u32..93).prop_map(Some)]),
                     k in prop_oneof![Just(3u32), Just(0x1_0003u32), Just(5u32), any::<u32>()], keep in some_word(), first in some_word()) {
        let mut s = state(seed);
        fill_matrices(&mut s, &vals);
        for (i, &v) in pq.iter().enumerate() {
            wf(&mut s, VECS + 4 * i as u32, v);
        }
        let p = at[0].map_or(VECS, |k| MATS + 4 * k);
        let q = at[1].map_or(VECS + 0xC, |k| MATS + 4 * k);
        wr(&mut s, SP_AT + 0x10, first);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(p), sext(q), sext(k), sext(keep));
        let mut w = s.clone();
        let sp = SP_AT - 0x28;
        saves(&mut w, &s, 0x28, &[(0x14, RA), (0x28, A0), (0x2C, A1), (0x30, A2), (0x34, A3)]);
        if first == 0 {
            call_c(&mut w, 0x28, &[(A0, sext(E60)), (A1, sext(E20))], imports::func_800156DC);
            call_c(&mut w, 0x28, &[(A0, sext(DA0)), (A1, sext(D60))], imports::func_800156DC);
        }
        call_c(&mut w, 0x28, &[(A0, sext(DE0 + 0x30)), (A1, sext(p))], imports::func_80015288);
        call_c(&mut w, 0x28, &[(A0, sext(EA0 + 0x30)), (A1, sext(q))], imports::func_80015288);
        wh(&mut w, MODE, k as u16);
        if k as u16 == 3 && keep != 0 {
            call_c(&mut w, 0x28, &[(A0, sext(sp + 0x1C)), (A1, sext(D60 + 0x30)), (A2, sext(E20 + 0x30))], imports::func_8001535C);
            call_c(&mut w, 0x28, &[(A0, sext(DE0 + 0x30)), (A1, sext(EA0 + 0x30)), (A2, sext(sp + 0x1C))], imports::func_80015328);
        }
        let after = run("func_8005058C", misc::func_8005058C, &s)?;
        same_memory(&after, &w)?;
    }
}

/// The transition state at 0x8011AC24: D (3), E (3), t, T.
const ST: u32 = 0x8011_AC24;
const NEW: u32 = 0x800A_4BD4;
const DT: u32 = 0x8012_0BF8;
const SHORT: u32 = 0x800A_B350;

fn step_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x68;
    saves(&mut w, s, 0x68, &[(0x14, RA), (0x68, A0)]);
    call_c(&mut w, 0x68, &[(A0, sext(sp + 0x44)), (A1, sext(D60 + 0x30))], imports::func_80015288);
    call_c(&mut w, 0x68, &[(A0, sext(sp + 0x38)), (A1, sext(E20 + 0x30))], imports::func_80015288);
    if word(&w, NEW) != 0 {
        for k in 0..3 {
            let d = sub(tr(&w, EA0, k), tr(&w, E60, k))?;
            let e = sub(tr(&w, DE0, k), tr(&w, DA0, k))?;
            wf(&mut w, ST + 4 * k, d);
            wf(&mut w, ST + 0xC + 4 * k, e);
        }
        wf(&mut w, ST + 0x18, 0.0);
        wf(&mut w, ST + 0x1C, 0.5);
        let dx = rf(&w, ST);
        if dx < 500.0 && -500.0 < dx {
            let dy = rf(&w, ST + 4);
            if dy < 500.0 && -500.0 < dy && word(&w, O + 0x38) == 1 {
                let short = word(&w, SHORT);
                wr(&mut w, ST + 0x1C, short);
            }
        }
        wr(&mut w, NEW, 0);
    }
    let (t, tt) = (rf(&w, ST + 0x18), rf(&w, ST + 0x1C));
    if t < tt {
        let t1 = add(t, rf(&w, DT))?;
        wf(&mut w, ST + 0x18, t1);
        let d = [sub(tr(&w, EA0, 0), tr(&w, E60, 0))?, sub(tr(&w, EA0, 1), tr(&w, E60, 1))?, sub(tr(&w, EA0, 2), tr(&w, E60, 2))?];
        wf(&mut w, sp + 0x24, d[1]);
        wf(&mut w, sp + 0x1C, d[2]);
        if tt < t1 {
            wf(&mut w, ST + 0x18, tt);
        }
        let f = div(rf(&w, ST + 0x18), rf(&w, ST + 0x1C))?;
        let mode3 = half(&w, MODE) as i16 == 3;
        for k in 0..3 {
            let v = add(mul(d[k as usize], f)?, tr(&w, E60, k))?;
            wf(&mut w, E20 + 0x30 + 4 * k, v);
        }
        if !mode3 {
            for k in 0..3 {
                let v = add(mul(sub(tr(&w, DE0, k), tr(&w, DA0, k))?, f)?, tr(&w, DA0, k))?;
                wf(&mut w, D60 + 0x30 + 4 * k, v);
            }
        }
    } else {
        wh(&mut w, MODE, 5);
        if word(&w, O + 8) == 8 {
            wh(&mut w, MODE, 0);
        }
        let busy = word(&w, 0x800A_4BD0);
        wr(&mut w, NEW, 1);
        if busy != 0 {
            let v = word(&w, 0x800A_4BC4);
            wr(&mut w, 0x800A_4BD0, 0);
            let flip = u32::from(v == 0);
            wr(&mut w, 0x800A_4BC4, flip);
            if word(&w, O + 0x38) == 1 && word(&w, O + 0x34) != 3 && flip != 0 {
                wh(&mut w, 0x800A_219C, 1);
            }
        }
        call_c(&mut w, 0x68, &[(A0, sext(E60)), (A1, sext(E20))], imports::func_800156DC);
        call_c(&mut w, 0x68, &[(A0, sext(DA0)), (A1, sext(D60))], imports::func_800156DC);
        if word(&w, 0x800A_2198) != u32::MAX && half(&w, 0x800A_219C) == 0 {
            wr(&mut w, 0x800A_4BC4, 0);
        }
    }
    Some(w)
}

/// A translation difference: the ±500 ties, just inside, or anything.
fn delta() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(500.0f32),
        Just(-500.0f32),
        Just(500.0f32.next_down()),
        Just((-500.0f32).next_up()),
        ordinary(),
        -600.0f32..600.0,
    ]
    .boxed()
}

/// `(t, T, dt)`: the ties `t == T` and `t + dt == T`, the fresh `(0, 0.5)`,
/// small steps, NaN, anything.
fn times() -> BoxedStrategy<(f32, f32, f32)> {
    prop_oneof![
        (ordinary(), ordinary(), ordinary()),
        (0.0f32..1.0, 0.0f32..1.0, 0.0f32..0.1),
        ordinary().prop_map(|t| (t, t, 0.25)),
        (0.0f32..1.0).prop_map(|a| (a, a.next_up(), 0.0)),
        (0.0f32..0.5).prop_map(|x| (0.0, x, x)),
        Just((0.0f32, 0.5f32, 1.0 / 60.0)),
        Just((f32::NAN, 0.5f32, 0.1)),
        Just((0.0f32, f32::NAN, 0.1)),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Matrices with some infinities, the ±500 ties on `dx`/`dy` (with
    /// `t(E60) = 0` so the difference is exact), and every flag of step 4.
    #[test]
    fn func_8005065C(seed: u64, vals in matrix_values(), exact: bool, dxy in prop::array::uniform2(delta()), new in some_word(), tt in times(),
                     mode in prop_oneof![Just(3u16), Just(5u16), any::<u16>()], flags in prop::array::uniform8(any::<bool>()), short in prop_oneof![Just(0.3f32), ordinary()]) {
        let mut s = state(seed);
        fill_matrices(&mut s, &vals);
        if exact {
            for k in 0..2 {
                wf(&mut s, E60 + 0x30 + 4 * k, 0.0);
                wf(&mut s, EA0 + 0x30 + 4 * k, dxy[k as usize]);
            }
        }
        wr(&mut s, NEW, new);
        wf(&mut s, ST + 0x18, tt.0);
        wf(&mut s, ST + 0x1C, tt.1);
        wf(&mut s, DT, tt.2);
        wh(&mut s, MODE, mode);
        wf(&mut s, SHORT, short);
        let pick = |b: bool, v: u32, r: u32| if b { v } else { r };
        wr(&mut s, O + 8, pick(flags[0], 8, seed as u32));
        wr(&mut s, O + 0x38, pick(flags[1], 1, seed as u32 >> 3));
        wr(&mut s, O + 0x34, pick(flags[2], 3, 7));
        wr(&mut s, 0x800A_4BD0, pick(flags[3], 0, (seed >> 32) as u32 | 1));
        wr(&mut s, 0x800A_4BC4, pick(flags[4], 0, pick(flags[7], 1, seed as u32)));
        wr(&mut s, 0x800A_2198, pick(flags[5], u32::MAX, 2));
        wh(&mut s, 0x800A_219C, pick(flags[6], 0, 1) as u16);
        s.ctx.gpr[A0] = sext(O);
        let model = step_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_8005065C", misc::func_8005065C, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- the progress figure ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_800521C0(seed: u64, flags: u32, bit: bool, k in prop_oneof![4 => ordinary(), 1 => Just(f32::INFINITY)], x in prop_oneof![4 => ordinary(), 1 => Just(f32::INFINITY)],
                     a in ordinary(), b in ordinary(), n in prop_oneof![-3i32..10, any::<i32>()]) {
        let mut s = state(seed);
        let (p, o) = (0x8030_0000u32, 0x8030_1000u32);
        wr(&mut s, p + 8, if bit { flags | 2 } else { flags & !2 });
        wf(&mut s, p + 0x74, x);
        wf(&mut s, 0x800A_CE38, k);
        wr(&mut s, p + 0x84, o);
        wf(&mut s, o + 0xE8, a);
        wf(&mut s, o + 0xE0, b);
        wr(&mut s, p + 0x78, n as u32);
        s.ctx.gpr[A0] = sext(p);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        let f0 = if bit {
            sub(k, x).unwrap()
        } else {
            call_c(&mut w, 0x18, &[(A0, sext(p))], imports::func_80052134);
            w.ctx.fpr[0].fl()
        };
        let after = run("func_800521C0", misc::func_800521C0, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), f0.to_bits());
    }
}

// ---- the menu buttons ----

const PRESSED: u32 = 0x800D_7700;
const SEL: u32 = 0x800A_59A8;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Pressed words that are 0, miss the mask or hit it; a mask register
    /// that is canonical or not (only the first path uses it unreloaded).
    #[test]
    fn func_80053220(seed: u64, gate in prop::array::uniform2(some_word()), n in prop_oneof![Just(0u32), Just(1u32), Just(2u32), any::<u32>()], sel in 0u32..6,
                     kinds in prop::array::uniform6(0u8..3), mask: u32, raw in prop_oneof![3 => Just(None), 1 => any::<u64>().prop_map(Some)], other in some_word()) {
        let mut s = state(seed);
        wr(&mut s, 0x800A_5998, gate[0]);
        wr(&mut s, 0x800A_59A0, gate[1]);
        wr(&mut s, 0x800A_52BC, n);
        wr(&mut s, SEL, sel);
        wr(&mut s, 0x800A_26F4, other);
        for (k, &kind) in kinds.iter().enumerate() {
            let v = match kind { 0 => 0, 1 => !mask, _ => mask ^ (seed as u32 & !mask) };
            wr(&mut s, PRESSED + 4 * k as u32, v);
        }
        let a0 = raw.unwrap_or(sext(mask));
        s.ctx.gpr[A0] = a0;
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        let p = |w: &State, k: u32| sext(word(w, PRESSED + 4 * k));
        let v0 = if gate[0] != 0 && gate[1] == 0 {
            0
        } else if (n as i32) < 2 {
            p(&w, 0) & a0
        } else {
            wr(&mut w, SP_AT, a0 as u32);
            call_c(&mut w, 0x18, &[], imports::func_8002F054);
            let m = sext(a0 as u32);
            if w.ctx.gpr[V0] != 0 {
                p(&w, word(&w, SEL)) & m
            } else if p(&w, 0) & m != 0 {
                wr(&mut w, SEL, 0);
                1
            } else if p(&w, 1) & m != 0 {
                wr(&mut w, SEL, 1);
                1
            } else {
                0
            }
        };
        let after = run("func_80053220", misc::func_80053220, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
    }
}

// ---- the texture alpha ----

const TEX: u32 = 0x8030_0000;
const PIX: u32 = 0x8030_1000;

fn alpha_model(s: &State, id: u32, level: f32) -> Option<State> {
    let mut w = s.clone();
    saves(&mut w, s, 0x18, &[(0x14, RA), (0x18, A0)]);
    let lv = if level < 0.0 {
        0.0
    } else if 1.0 < level {
        1.0
    } else {
        level
    };
    wf(&mut w, SP_AT + 4, lv);
    call_c(&mut w, 0x18, &[(A0, sext16(id)), (A1, s.ctx.gpr[A0])], imports::func_8000ABD4);
    let p = w.ctx.gpr[V0] as u32;
    if p != 0 {
        for i in 0..256 {
            let a = p + 2 * i;
            let px = half(&w, a);
            let r = i32::from((px & 0xF800) >> 8);
            let v = if r > 0 {
                let thr = trunc(add(-242.0, mul(lv, 492.0)?)?) as i32;
                if r - (i32::from(px & 0x3E) << 2) < thr {
                    px | 1
                } else {
                    px & 0xFFFE
                }
            } else {
                px & 0xFFFE
            };
            wh(&mut w, a, v);
        }
    }
    Some(w)
}

/// Levels around the clamps, levels putting the threshold on a multiple
/// of 8 (so `R - B` can equal it) and one either side, anything.
fn level() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        Just(1.0f32),
        Just(1.0f32.next_up()),
        Just(-1.0f32),
        Just(f32::NAN),
        0.0f32..1.0,
        (-31i32..32, -1i32..=1).prop_map(|(m, d)| {
            let x = (242 + 8 * m) as f32 / 492.0;
            match d {
                -1 => x.next_down(),
                1 => x.next_up(),
                _ => x,
            }
        }),
        ordinary(),
    ]
    .boxed()
}

/// A pixel: random, no red, or `R - B` a multiple of 8 near a threshold.
fn pixel() -> BoxedStrategy<u16> {
    prop_oneof![
        any::<u16>(),
        any::<u16>().prop_map(|p| p & 0x07FF),
        (1u16..32, 0u16..32, any::<u16>()).prop_map(|(r, b, rest)| r << 11 | b << 1 | rest & 0x07C1),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80055AEC(seed: u64, id in prop_oneof![0i32..48, -6i32..0, Just(0x1_0005i32)], level in level(), pixels in prop::collection::vec(pixel(), 256), null in 0u8..6) {
        let mut s = state(seed);
        let entry = RECORDS.wrapping_add(0x1C).wrapping_add((id as i16 as i32 as u32).wrapping_mul(32));
        wr(&mut s, entry, if null == 0 { 0 } else { TEX });
        wr(&mut s, TEX + 8, if null == 1 { 0 } else { PIX });
        for (i, &p) in pixels.iter().enumerate() {
            wh(&mut s, PIX + 2 * i as u32, p);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(id as u32), sext(level.to_bits()));
        let model = alpha_model(&s, id as u32, level);
        prop_assume!(model.is_some());
        let after = run("func_80055AEC", misc::func_80055AEC, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

/// Separators for a fused `-242 + level * 492`, found by an exact search:
/// at these levels the rounded product puts the sum on one side of a
/// threshold that is a multiple of 8 and the exact sum on the other, so a
/// pixel with `R - B` equal to the lower threshold flips its alpha.
#[test]
fn func_80055AEC_fused_separators() {
    for (seed, (lv, t)) in [(0x3F02_9A04u32, 8i32), (0x3EEB_2FDF, -16), (0x3F77_278B, 232)].into_iter().enumerate() {
        let level = f32::from_bits(lv);
        let unfused = trunc(-242.0 + level * 492.0) as i32;
        let fused = trunc(level.mul_add(492.0, -242.0)) as i32;
        assert_eq!((unfused.min(fused), (unfused - fused).abs()), (t, 1));
        let mut s = state(seed as u64);
        wr(&mut s, RECORDS + 0x1C + 32 * 3, TEX);
        wr(&mut s, TEX + 8, PIX);
        // R - B = t: R = t (B = 0) for t >= 8, else R = 8, B = 8 - t.
        let (r, b) = if t >= 8 { (t / 8, 0) } else { (1, (8 - t) / 8) };
        for i in 0..256u32 {
            let rest = (seed as u16).wrapping_mul(i as u16 ^ 0x5A5A) & 0x07C1;
            wh(&mut s, PIX + 2 * i, (r << 11 | b << 1) as u16 | rest);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (3, sext(lv));
        let model = alpha_model(&s, 3, level).unwrap();
        let after = compare("func_80055AEC", misc::func_80055AEC, &s).unwrap_or_else(|d| panic!("{d}"));
        same_memory(&after, &model).unwrap();
    }
}

// ---- the fading panel ----

const LEVEL: u32 = 0x800D_7740;

fn panel_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x50;
    saves(&mut w, s, 0x50, &[(0x1C, RA)]);
    call_c(&mut w, 0x50, &[], imports::func_8002F060);
    let v = rf(&w, LEVEL);
    wf(&mut w, sp + 0x30, v);
    if v <= 0.0 {
        call_c(&mut w, 0x50, &[(A0, 0x1A), (A1, 0)], imports::func_8000A920);
        return Some(w);
    }
    call_c(&mut w, 0x50, &[(A0, 0x1A), (A1, 1)], imports::func_8000A920);
    let y = sub(90.0, mul(sub(1.0, v)?, 80.0)?)?;
    wf(&mut w, sp + 0x24, y);
    call_c(&mut w, 0x50, &[(A0, 0x1A), (A1, 160), (A2, sext16(trunc(y)))], imports::func_8000AA04);
    call_c(&mut w, 0x50, &[(A0, 0x1A), (A1, sext(32.5f32.to_bits())), (A2, sext(0x407A_0000))], imports::func_8000AAC0);
    let a = u(mul(254.0, v)?);
    wr(&mut w, sp + 0x10, a);
    call_c(&mut w, 0x50, &[(A0, 0x1A), (A1, 0), (A2, 55), (A3, 71)], imports::func_8000AB24);
    let (y0, y1) = (sub(y, 30.0)?, add(y, 30.0)?);
    call_c(&mut w, 0x50, &[(A0, 95), (A1, sext16(trunc(y0))), (A2, 220), (A3, sext16(trunc(y1)))], imports::func_80087814);
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Levels in [0, 1] and beyond, the hide boundary (0, -0), levels
    /// whose alpha 254 v reaches 2^31, infinities and NaN.
    #[test]
    fn func_80056464(seed: u64, v in prop_oneof![4 => 0.0f32..=1.0, 1 => Just(0.0f32), 1 => Just(-0.0f32), 1 => Just(f32::MIN_POSITIVE),
                                                  1 => Just(2_147_483_648.0f32 / 254.0), 1 => 8.0e6f32..1.0e7, 1 => Just(f32::INFINITY),
                                                  1 => Just(f32::NEG_INFINITY), 1 => Just(f32::NAN), 2 => ordinary()],
                     count in 0u32..40) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, RECORDS, 0x800);
        wf(&mut s, LEVEL, v);
        wr(&mut s, 0x800A_6978, count);
        let model = panel_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_80056464", misc::func_80056464, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- the HUD bar and set ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn func_80057ED4(seed: u64, count in 0u32..40) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, RECORDS, 0x800);
        wr(&mut s, 0x800A_6978, count);
        let mut w = s.clone();
        saves(&mut w, &s, 0x20, &[(0x1C, RA)]);
        call_c(&mut w, 0x20, &[(A0, 0x17), (A1, 1)], imports::func_8000A920);
        call_c(&mut w, 0x20, &[(A0, 0x17), (A1, 0), (A2, 0x76)], imports::func_8000AA04);
        call_c(&mut w, 0x20, &[(A0, 0x17), (A1, sext(0x43A0_0000)), (A2, sext(0x4080_0000))], imports::func_8000AAC0);
        wr(&mut w, SP_AT - 0x20 + 0x10, 0xFF);
        call_c(&mut w, 0x20, &[(A0, 0x17), (A1, 0), (A2, 0), (A3, 0)], imports::func_8000AB24);
        call_c(&mut w, 0x20, &[(A0, 0x14), (A1, 0x75), (A2, 0x12C), (A3, 0x7B)], imports::func_80087814);
        let after = run("func_80057ED4", misc::func_80057ED4, &s)?;
        same_memory(&after, &w)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The four words making `func_80051FF4` return 2 half the time, and
    /// `k` the second word, its low word only, or anything.
    #[test]
    fn func_80057F48(seed: u64, two: bool, words in prop::array::uniform4(some_word()), which in 0u8..3, k: u64) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, RECORDS, 0x800);
        let mut words = words;
        if two {
            words[0] |= 1;
            words[1] |= 0x100;
            words[2] = 0;
        }
        for (i, &x) in words.iter().enumerate() {
            wr(&mut s, 0x8011_B1BC + 4 * i as u32, x);
        }
        s.ctx.gpr[A0] = match which { 0 => sext(words[1]), 1 => u64::from(words[1]) | 1 << 40, _ => k };
        let mut w = s.clone();
        saves(&mut w, &s, 0x20, &[(0x14, S0), (0x18, S1), (0x1C, RA)]);
        call_c(&mut w, 0x20, &[], imports::func_80051FF4);
        let (first, last) = if w.ctx.gpr[V0] == 2 && s.ctx.gpr[A0] != sext(words[1]) { (0x23u64, [0x29u64, 0x2A]) } else { (0x1B, [0x21, 0x22]) };
        w.ctx.gpr[S1] = 6;
        for i in 0..6 {
            w.ctx.gpr[S0] = i;
            call_c(&mut w, 0x20, &[(A0, first + i), (A1, 0)], imports::func_8000A920);
        }
        w.ctx.gpr[S0] = 6;
        for id in last {
            call_c(&mut w, 0x20, &[(A0, id), (A1, 0)], imports::func_8000A920);
        }
        let after = run("func_80057F48", misc::func_80057F48, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- the activity test ----

const BOX: u32 = 0x800A_CFA0;
const R: u32 = 0x8030_0000;
const OB: u32 = 0x8030_1000;
const NODE: u32 = 0x8030_2000;
const HW: u32 = 0x8030_3000;

fn active_model(s: &State) -> Option<(State, u64)> {
    let mut w = s.clone();
    saves(&mut w, s, 0x18, &[(0x14, RA)]);
    let bx = [rf(&w, BOX), rf(&w, BOX + 4), rf(&w, BOX + 8), rf(&w, BOX + 0xC)];
    let (x, y) = (rf(&w, OB + 0x50), rf(&w, OB + 0x54));
    if word(&w, R + 0x1AC) == 1 && word(&w, R + 0x1C0) == 3 && bx[0] < x && x < bx[1] && bx[2] < y && y < bx[3] && word(&w, OB + 0x140) != 0 {
        wr(&mut w, SP_AT + 4, OB);
        let n = sext(word(&w, OB + 0x140));
        call_c(&mut w, 0x18, &[(A0, n)], imports::func_800183A8);
        let p = w.ctx.gpr[V0] as u32;
        if p != 0 && half(&w, p) & 8 != 0 {
            return Some((w, 0));
        }
    }
    if word(&w, OB + 0x64) & 1 << 25 != 0 {
        return Some((w, 0));
    }
    if half(&w, OB + 0x10C) as i16 >= 5 {
        return Some((w, 1));
    }
    let v = ok(rf(&w, OB + 0x1A0))?;
    Some((w, u64::from(f64::from(v) < 60.0)))
}

/// A coordinate on either bound, halfway between, or anything.
fn coordinate(lo: f32, hi: f32, k: u8, any: f32) -> f32 {
    match k {
        0 => lo,
        1 => hi,
        2 => lo + (hi - lo) * 0.5,
        _ => any,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8005B2D0(seed: u64, modes in prop::array::uniform2(any::<bool>()), bx in prop_oneof![Just([-6081.0f32, -5086.0, -2801.0, -1182.0]), prop::array::uniform4(ordinary())],
                     xy in prop::array::uniform2((0u8..5, ordinary())), node in 0u8..4, flags: u32, bit25: bool, h in prop_oneof![Just(5i16), Just(4i16), any::<i16>()],
                     v in prop_oneof![Just(60.0f32), Just(60.0f32.next_down()), Just(60.0f32.next_up()), Just(-0.0f32), Just(f32::NAN), ordinary()]) {
        let mut s = state(seed);
        wr(&mut s, R + 0x1AC, if modes[0] { 1 } else { seed as u32 });
        wr(&mut s, R + 0x1C0, if modes[1] { 3 } else { seed as u32 >> 5 });
        for (k, &b) in bx.iter().enumerate() {
            wf(&mut s, BOX + 4 * k as u32, b);
        }
        wf(&mut s, OB + 0x50, coordinate(bx[0], bx[1], xy[0].0, xy[0].1));
        wf(&mut s, OB + 0x54, coordinate(bx[2], bx[3], xy[1].0, xy[1].1));
        // [o + 0x140]: null, or a node whose +4 is null or a halfword with bit
        // 3 clear or set.
        wr(&mut s, OB + 0x140, if node == 0 { 0 } else { NODE });
        wr(&mut s, NODE + 4, if node == 1 { 0 } else { HW });
        let hw = half(&s, HW);
        wh(&mut s, HW, if node == 3 { hw | 8 } else { hw & !8 });
        wr(&mut s, OB + 0x64, if bit25 { flags | 1 << 25 } else { flags & !(1 << 25) });
        wh(&mut s, OB + 0x10C, h as u16);
        wf(&mut s, OB + 0x1A0, v);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(R), sext(OB));
        let model = active_model(&s);
        prop_assume!(model.is_some());
        let (w, v0) = model.unwrap();
        let after = run("func_8005B2D0", misc::func_8005B2D0, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The early 0: both modes, ordered boxes with `(x, y)` inside (or on a
    /// bound), a node whose halfword has bit 3 set or clear.
    #[test]
    fn func_8005B2D0_box(seed: u64, lo in prop::array::uniform2(-1.0e4f32..0.0), size in prop::array::uniform2(1.0f32..1.0e3), t in prop::array::uniform2(prop_oneof![3 => 0.0f32..1.0, 1 => Just(0.0f32), 1 => Just(1.0f32)]),
                         node in 1u8..4, flags: u32, bit25: bool, h in prop_oneof![Just(5i16), Just(4i16), any::<i16>()], v in prop_oneof![Just(60.0f32), ordinary()]) {
        let mut s = state(seed);
        wr(&mut s, R + 0x1AC, 1);
        wr(&mut s, R + 0x1C0, 3);
        let bx = [lo[0], lo[0] + size[0], lo[1], lo[1] + size[1]];
        for (k, &b) in bx.iter().enumerate() {
            wf(&mut s, BOX + 4 * k as u32, b);
        }
        wf(&mut s, OB + 0x50, bx[0] + (bx[1] - bx[0]) * t[0]);
        wf(&mut s, OB + 0x54, bx[2] + (bx[3] - bx[2]) * t[1]);
        wr(&mut s, OB + 0x140, NODE);
        wr(&mut s, NODE + 4, if node == 1 { 0 } else { HW });
        let hw = half(&s, HW);
        wh(&mut s, HW, if node == 3 { hw | 8 } else { hw & !8 });
        wr(&mut s, OB + 0x64, if bit25 { flags | 1 << 25 } else { flags & !(1 << 25) });
        wh(&mut s, OB + 0x10C, h as u16);
        wf(&mut s, OB + 0x1A0, v);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(R), sext(OB));
        let model = active_model(&s);
        prop_assume!(model.is_some());
        let (w, v0) = model.unwrap();
        let after = run("func_8005B2D0", misc::func_8005B2D0, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
    }
}

// ---- the race HUD ----

const HF: u32 = 0x1B8;
const FADE: u32 = 0x800A_59B0;
const GREEN: (u64, u64, u64) = (0x59, 0x8C, 0x36);
const BLUE: (u64, u64, u64) = (0, 0x6E, 0x8F);

fn on(w: &mut State, id: u64, v: bool) {
    call_c(w, HF, &[(A0, id), (A1, u64::from(v))], imports::func_8000A920);
}

fn at(w: &mut State, id: u64, x: u32, y: u32) {
    call_c(w, HF, &[(A0, id), (A1, sext16(x)), (A2, sext16(y))], imports::func_8000AA04);
}

fn scale(w: &mut State, id: u64, a: f32, b: f32) {
    call_c(w, HF, &[(A0, id), (A1, sext(a.to_bits())), (A2, sext(b.to_bits()))], imports::func_8000AAC0);
}

fn colour(w: &mut State, id: u64, c: (u64, u64, u64), alpha: u32) {
    wr(w, SP_AT - HF + 0x10, alpha);
    call_c(w, HF, &[(A0, id), (A1, c.0), (A2, c.1), (A3, c.2)], imports::func_8000AB24);
}

fn hud_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - HF;
    let flags = s.ctx.gpr[A0] as u32;
    saves(&mut w, s, HF, &[(0x1C, RA), (HF, A0)]);
    call_c(&mut w, HF, &[], imports::func_80051FF4);
    if w.ctx.gpr[V0] as i64 >= 2 {
        return Some(w);
    }
    let w = &mut w;
    if flags & 1 != 0 {
        on(w, 6, true);
        wr(w, sp + 0x28, 23);
        at(w, 6, 23, 220);
        scale(w, 6, 270.0, 2.0);
        colour(w, 6, GREEN, 0x40);
        on(w, 5, true);
        at(w, 5, 23, 20);
        scale(w, 5, 270.0, 2.0);
        colour(w, 5, GREEN, 0x40);
        for id in [13, 0, 11, 4, 12, 1] {
            on(w, id, false);
        }
    } else {
        on(w, 5, true);
        wr(w, sp + 0x28, 0);
        at(w, 5, 0, 21);
        scale(w, 5, 320.0, 2.0);
        colour(w, 5, GREEN, 0x40);
        on(w, 6, false);
        on(w, 13, true);
        at(w, 13, 0, 23);
        scale(w, 13, 80.0, 1.0);
        colour(w, 13, BLUE, 0xFE);
        wf(w, sp + 0xA8, 34.0);
        on(w, 0, true);
        at(w, 0, 18, 34);
        colour(w, 0, GREEN, 0xFE);
        wf(w, sp + 0xAC, 66.0);
        wf(w, sp + 0xA8, 35.0);
        on(w, 11, true);
        wr(w, sp + 0x28, 35);
        at(w, 11, 66, 35);
        scale(w, 11, 21.5, 1.0);
        colour(w, 11, GREEN, 0xFE);
        on(w, 4, true);
        at(w, 4, 109, 35);
        colour(w, 4, GREEN, 0xFE);
        wf(w, sp + 0xAC, 211.0);
        on(w, 12, true);
        at(w, 12, 211, 35);
        scale(w, 12, 21.5, 1.0);
        colour(w, 12, GREEN, 0xFE);
        wf(w, sp + 0xA8, 34.0);
        on(w, 1, true);
        at(w, 1, 254, 34);
        colour(w, 1, GREEN, 0xFE);
    }
    on(w, 3, true);
    wf(w, sp + 0x24, 229.0);
    wr(w, sp + 0x28, 180);
    at(w, 3, 229, 180);
    colour(w, 3, BLUE, 0xFE);
    wf(w, sp + 0xAC, 243.0);
    on(w, 2, true);
    at(w, 2, 243, 180);
    scale(w, 2, 6.0, 1.0);
    colour(w, 2, BLUE, 0xFE);
    wf(w, sp + 0xAC, 267.0);
    on(w, 10, true);
    at(w, 10, 267, 180);
    colour(w, 10, BLUE, 0xFE);
    if flags & 4 == 0 {
        let h = if flags & 2 != 0 {
            call_c(w, HF, &[], imports::func_8002F060);
            let d = sub(1.0, rf(w, 0x800D_7740))?;
            let mut h = add(d, d)?;
            if 1.0 < h {
                h = 1.0;
            }
            mul(h, sub(1.0, rf(w, FADE))?)?
        } else {
            0.0
        };
        wf(w, sp + 0xA4, h);
        on(w, 7, true);
        let k = mul(90.0, h)?;
        wf(w, sp + 0x28, k);
        wr(w, sp + 0x24, 275);
        let y = sub(164.0, k)?;
        wf(w, sp + 0x20, y);
        at(w, 7, 275, trunc(y));
        scale(w, 7, 1.0, div(k, 2.0)?);
        colour(w, 7, GREEN, 0xFE);
        let y4 = sub(y, 4.0)?;
        wf(w, sp + 0xA8, y4);
        on(w, 9, true);
        at(w, 9, 275, trunc(y4));
        colour(w, 9, GREEN, 0xFE);
        on(w, 8, false);
    } else {
        on(w, 7, true);
        wr(w, sp + 0x28, 21);
        at(w, 7, 22, 21);
        scale(w, 7, 1.0, 99.5);
        colour(w, 7, GREEN, 0xFE);
        on(w, 8, true);
        let x = trunc(rf(w, 0x800A_CEDC));
        at(w, 8, x, 21);
        scale(w, 8, 1.0, 99.5);
        colour(w, 8, GREEN, 0xFE);
        on(w, 9, true);
        at(w, 9, 275, 160);
        colour(w, 9, GREEN, 0xFE);
    }
    let l = rf(w, FADE);
    wf(w, sp + 0x2C, l);
    on(w, 0x19, 0.0 < l);
    let x2 = trunc(rf(w, 0x800A_CEE0));
    at(w, 0x19, x2, 60);
    scale(w, 0x19, 15.625, 3.90625);
    let alpha = u(mul(254.0, l)?);
    colour(w, 0x19, (0, 55, 71), alpha);
    Some(w.clone())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Every layout (flags bits 0..2, others random), `func_80051FF4`
    /// returning 0..4, the slide's clamp tie `v = 0.5`, fade levels in
    /// [0, 1] and beyond, and perturbed ROM positions.
    #[test]
    fn func_80056844(seed: u64, flags in prop_oneof![0u32..8, any::<u32>()], n in prop_oneof![3 => 0u32..2, 1 => 2u32..5], v in prop_oneof![Just(0.5f32), 0.0f32..=1.0, ordinary()],
                     l in prop_oneof![4 => 0.0f32..=1.0, 1 => Just(0.0f32), 1 => Just(-0.0f32), 1 => Just(f32::MIN_POSITIVE), 1 => Just(f32::INFINITY), 1 => Just(f32::NAN), 2 => ordinary()],
                     xs in prop::array::uniform2(prop_oneof![Just(283.0f32), Just(232.75f32), ordinary()])) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, RECORDS, 0x800);
        for i in 0..4u32 {
            wr(&mut s, 0x8011_B1BC + 4 * i, if i < n { (seed as u32 >> i) | 1 } else { 0 });
        }
        wf(&mut s, 0x800D_7740, v);
        wf(&mut s, FADE, l);
        wf(&mut s, 0x800A_CEDC, xs[0]);
        wf(&mut s, 0x800A_CEE0, xs[1]);
        s.ctx.gpr[A0] = sext(flags);
        let model = hud_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_80056844", misc::func_80056844, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}
