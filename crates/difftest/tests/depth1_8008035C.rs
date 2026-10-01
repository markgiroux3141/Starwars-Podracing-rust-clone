//! Depth 1 at 0x8008035C..0x800833B4 (game::misc, game::math): two values
//! along a spline walker, a 4x4 split into translation, rotation and scale
//! and composed back, a side test of three cross products, and four node
//! walks (a node's world matrix and the path to it, both recursive, a
//! path's matrix, and a tree's weights, recursive). Recompiled C vs Rust
//! with the callees as C. The models replay the callees' C on a copy of the
//! state in the same order with the same arguments (and the s registers
//! and callee-saved FPRs the port holds), adding the functions' own stores;
//! whole RDRAM (and the result register) is compared.

// Tests are named after the functions (func_8008035C), capitals included.
#![allow(non_snake_case)]

use difftest::rom::baserom;
use difftest::{compare, State};
use game::imports;
use game::recomp::{reg::*, RecompFn};
use game::{math, misc};
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
fn neg(a: f32) -> Option<f32> {
    Some(-ok(a)?)
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

/// Finite values that stay finite through a few matrix products (the
/// recursive walks run the C below the top level, where a model can't
/// check for NaN).
fn bounded() -> BoxedStrategy<f32> {
    prop_oneof![
        4 => -100.0f32..100.0,
        2 => -1.0f32..1.0,
        2 => (-3i32..=3).prop_map(|n| n as f32),
        1 => Just(-0.0f32),
        1 => (1u32..0x0080_0000).prop_map(f32::from_bits),
    ]
    .boxed()
}

// ---- the spline walker values ----

const W: u32 = 0x8030_0000;
const SPL: u32 = 0x8030_0100;
const PTS: u32 = 0x8030_1000;
const FLAG: u32 = 0x800A_6704;
const CAP: u32 = 0x800A_DC5C;
const FLOOR: u32 = 0x800A_DC60;

/// The walker at `W` whose point ([`game::spline::func_8003A568`]`(W, 0)`)
/// is `i`: the word `[W + 0x10]` without path bits, else the halfword of
/// point `p` that the bits select (`i` an s16 then).
fn walker(s: &mut State, path: Option<(u32, u32)>, i: i32) {
    match path {
        None => {
            wr(s, W + 0x2C, 0);
            wr(s, W + 0x10, i as u32);
        }
        Some((bits, p)) => {
            wr(s, W + 0x2C, bits);
            wr(s, W + 0x10, p);
            wr(s, W, SPL);
            wr(s, SPL + 0xC, PTS);
            wh(s, PTS + 84 * p + 2 * bits + 0x42, i as u16);
        }
    }
}

fn walker_point(w: &mut State, frame: u32) -> u32 {
    call_c(w, frame, &[(A0, sext(W)), (A1, 0)], imports::func_8003A568);
    w.ctx.gpr[V0] as u32
}

fn value_model(s: &State) -> Option<(State, u32)> {
    let mut w = s.clone();
    saves(&mut w, s, 0x18, &[(0x14, RA), (0x18, A0)]);
    let i = walker_point(&mut w, 0x18);
    let e = 0x8012_0408u32.wrapping_add(i.wrapping_mul(8));
    let r = add(mul(rf(&w, e + 4), rf(&w, W + 8))?, rf(&w, e))?;
    let f0 = if (word(&w, FLAG) as i32) < 0 {
        if 1.0 <= r {
            rf(&w, CAP)
        } else {
            r
        }
    } else if 1.0 < r {
        1.0
    } else {
        r
    };
    Some((w, f0.to_bits()))
}

fn path_strategy() -> BoxedStrategy<Option<(u32, u32)>> {
    prop_oneof![Just(None), (1u32..8, 0u32..4).prop_map(Some)].boxed()
}

fn value_case(seed: u64, path: Option<(u32, u32)>, i: i32, (a, b, x): (f32, f32, f32), flag: u32, cap: Option<f32>) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    load_data(&mut s);
    walker(&mut s, path, i);
    let e = 0x8012_0408u32.wrapping_add((i as u32).wrapping_mul(8));
    wf(&mut s, e, a);
    wf(&mut s, e + 4, b);
    wf(&mut s, W + 8, x);
    wr(&mut s, FLAG, flag);
    if let Some(c) = cap {
        wf(&mut s, CAP, c);
    }
    s.ctx.gpr[A0] = sext(W);
    let model = value_model(&s);
    prop_assume!(model.is_some());
    let (want, f0) = model.unwrap();
    let after = run("func_8008035C", misc::func_8008035C, &s)?;
    same_memory(&after, &want)?;
    prop_assert_eq!(after.ctx.fpr[0].u32l(), f0);
    Ok(())
}

fn word_model(s: &State) -> Option<(State, u64)> {
    let mut w = s.clone();
    saves(&mut w, s, 0x18, &[(0x14, RA), (0x18, A0)]);
    let i = walker_point(&mut w, 0x18);
    let x = mul(rf(&w, W + 8), 10.0)?;
    let j = if x < 0.0 { trunc(sub(x, rf(&w, FLOOR))?) } else { trunc(x) };
    let a = 0x8011_DCF8u32.wrapping_add(j.wrapping_add(i.wrapping_mul(10)).wrapping_mul(4));
    if !(0x8000_0000..0x8080_0000).contains(&a) {
        return None;
    }
    let v0 = sext(word(&w, a));
    Some((w, v0))
}

fn word_case(seed: u64, path: Option<(u32, u32)>, i: i32, x: f32, floor: Option<f32>) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    load_data(&mut s);
    s.randomise_memory(seed ^ 0x5EED, 0x8010_0000, 0x4_0000);
    walker(&mut s, path, i);
    wf(&mut s, W + 8, x);
    if let Some(c) = floor {
        wf(&mut s, FLOOR, c);
    }
    s.ctx.gpr[A0] = sext(W);
    let model = word_model(&s);
    prop_assume!(model.is_some());
    let (want, v0) = model.unwrap();
    let after = run("func_80080408", misc::func_80080408, &s)?;
    same_memory(&after, &want)?;
    prop_assert_eq!(after.ctx.gpr[V0], v0);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Both walker modes, the pair around the cap (r = 1 exactly through a
    /// zero product), the flag's sign, the cap constant perturbed.
    #[test]
    fn func_8008035C(seed: u64, path in path_strategy(), i in -0x8000i32..0x8000,
                     abx in prop_oneof![(float(), float(), float()), (ordinary(), ordinary(), ordinary()),
                                        (Just(1.0f32), ordinary(), Just(0.0f32)), (Just(1.0f32), Just(0.0f32), ordinary()),
                                        (Just(0.5f32), Just(0.5f32), Just(1.0f32)), (0.9f32..1.1, -0.1f32..0.1, -1.0f32..1.0)],
                     flag in prop_oneof![Just(0xFFFF_FFFFu32), Just(0u32), Just(0x8000_0000u32), Just(0x7FFF_FFFFu32), any::<u32>()],
                     cap in prop_oneof![Just(None), float().prop_map(Some)]) {
        value_case(seed, path, i, abx, flag, cap)?;
    }

    /// Both walker modes, `x` around integers and zero on both sides, large
    /// and infinite products (the index wraps), the floor constant
    /// perturbed (negative ones make the subtraction cancel).
    #[test]
    fn func_80080408(seed: u64, path in path_strategy(), i in -0x800i32..0x800,
                     x in prop_oneof![-100.0f32..100.0, (-1000i32..1000).prop_map(|n| n as f32 / 10.0), Just(0.0f32), Just(-0.0f32),
                                      Just(-1.0e-7f32), Just(f32::INFINITY), Just(f32::NEG_INFINITY), float()],
                     floor in prop_oneof![Just(None), float().prop_map(Some), (-2.0f32..2.0).prop_map(Some)]) {
        word_case(seed, path, i, x, floor)?;
    }
}

/// A float of the ROM's data segment, read at test time.
fn rom_f(va: u32) -> f32 {
    let o = (va - 0x8000_0400 + 0x1000) as usize;
    f32::from_bits(u32::from_be_bytes(baserom()[o..o + 4].try_into().unwrap()))
}

/// Separators for `x * 10` fused with the subtraction of the floor
/// constant: the difference is truncated, so fusing shows only where the
/// two roundings land on either side of an integer (the double rounding of
/// `fl(10 x) - c` into a coarser binade, or a cancelling negative constant
/// the test writes). The search runs here, over the floats next to `(c -
/// n) / 10`, for the ROM's constant (left in place) and two written ones: the
/// fused value is `fl(10 x - c)`, exact in f64 before its one rounding.
#[test]
fn func_80080408_separators() {
    for c in [None, Some(-0.75f32), Some(-0.25)] {
        let cv = c.unwrap_or_else(|| rom_f(FLOOR));
        let mut found = Vec::new();
        'n: for n in 1..400 {
            let near = ((cv as f64 - n as f64) / 10.0) as f32;
            for k in -150i32..150 {
                let x = f32::from_bits((near.to_bits() as i32 + k) as u32);
                let unfused = (x * 10.0) - cv;
                let fused = ((x as f64) * 10.0 - cv as f64) as f32;
                if x * 10.0 < 0.0 && trunc(unfused) != trunc(fused) {
                    found.push(x);
                    if found.len() == 3 {
                        break 'n;
                    }
                    break;
                }
            }
        }
        assert!(!found.is_empty(), "no separator for the fused x * 10 - c, c = {cv}");
        for (k, &x) in found.iter().enumerate() {
            word_case(0x5E9 + k as u64, None, 7, x, c).unwrap();
        }
    }
}

// ---- 4x4 split and compose ----

const M: u32 = 0x8030_2000;
const R: u32 = 0x8030_2100;
const T: u32 = 0x8030_2200;
const SC: u32 = 0x8030_2300;

fn split_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    saves(&mut w, s, 0x20, &[(0x1C, RA), (0x18, S1), (0x14, S0), (0x24, A1), (0x2C, A3)]);
    let [m, t, r, sc] = [A0, A1, A2, A3].map(|x| s.ctx.gpr[x] as u32);
    (w.ctx.gpr[S0], w.ctx.gpr[S1]) = (sext(r), sext(m));
    for i in 0..3u32 {
        let row = m + 0x10 * i;
        if v3(&w, row).iter().any(|x| x.is_nan()) {
            return None;
        }
        call_c(&mut w, 0x20, &[(A0, sext(row))], imports::func_800153C0);
        let l = w.ctx.fpr[0].fl();
        let inv = div(1.0, l)?;
        wf(&mut w, sc + 4 * i, l);
        let x = mul(rf(&w, row), inv)?;
        wf(&mut w, r + 0x10 * i, x);
        let y = mul(rf(&w, row + 4), inv)?;
        wf(&mut w, r + 0x10 * i + 4, y);
        let z = rf(&w, row + 8);
        wr(&mut w, r + 0x10 * i + 0xC, 0);
        wf(&mut w, r + 0x10 * i + 8, mul(z, inv)?);
    }
    for k in 0..3 {
        let v = word(&w, m + 0x30 + 4 * k);
        wr(&mut w, t + 4 * k, v);
    }
    wf(&mut w, r + 0x3C, 1.0);
    for k in 0..3 {
        wr(&mut w, r + 0x30 + 4 * k, 0);
    }
    Some(w)
}

fn compose_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    saves(&mut w, s, 0x20, &[(0x1C, RA), (0x24, A1), (0x20, A0), (0x2C, A3)]);
    let [out, a, b, c] = [A0, A1, A2, A3].map(|x| s.ctx.gpr[x] as u32);
    call_c(&mut w, 0x20, &[(A0, sext(out)), (A1, sext(b))], imports::func_800156DC);
    for k in 0..3 {
        add(rf(&w, a + 4 * k), rf(&w, out + 0x30 + 4 * k))?;
    }
    call_c(&mut w, 0x20, &[(A0, sext(out + 0x30)), (A1, sext(out + 0x30)), (A2, sext(a))], imports::func_80015328);
    let cw = [0, 1, 2].map(|k| word(&w, c + 4 * k));
    for i in 0..3u32 {
        for j in 0..4 {
            mul(rf(&w, out + 0x10 * i + 4 * j), f32::from_bits(cw[i as usize]))?;
        }
    }
    wr(&mut w, SP_AT - 0x20 + 0x10, out);
    call_c(&mut w, 0x20, &[(A0, sext(out)), (A1, sext(cw[0])), (A2, sext(cw[1])), (A3, sext(cw[2]))], imports::func_80017918);
    Some(w)
}

/// Where an output goes: its own buffer, or over the input matrix (`m`
/// itself, or shifted by some words: `m - 4` puts `r[i][3]` over `m[i][2]`).
fn place(own: u32, k: u8) -> u32 {
    match k {
        0 | 1 => own,
        2 => M - 4,
        _ => M + 4 * (k as u32 - 3),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Ordinary and edge rows (a zero row gives an infinite factor), the
    /// outputs apart or over the input.
    #[test]
    fn func_80081814(seed: u64, m in prop::array::uniform16(prop_oneof![3 => ordinary(), 1 => float(), 1 => Just(0.0f32)]),
                     zero_row in prop_oneof![3 => Just(None), 1 => (0u32..3).prop_map(Some)], places in prop::array::uniform3(0u8..8)) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, M, 0x400);
        for (k, &v) in m.iter().enumerate() {
            wf(&mut s, M + 4 * k as u32, v);
        }
        if let Some(i) = zero_row {
            for k in 0..3 {
                wf(&mut s, M + 0x10 * i + 4 * k, if k == 1 { -0.0 } else { 0.0 });
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(M), sext(place(T, places[0])), sext(place(R, places[1])), sext(place(SC, places[2])));
        let model = split_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_80081814", math::func_80081814, &s)?;
        same_memory(&after, &model.unwrap())?;
    }

    /// Ordinary matrices, translations and scales (zeros and signs
    /// included), `b` apart or the output itself.
    #[test]
    fn func_80081948(seed: u64, b in prop::array::uniform16(ordinary()), a in prop::array::uniform3(ordinary()), c in prop::array::uniform3(ordinary()), in_place: bool) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, M, 0x400);
        let bm = if in_place { R } else { M };
        for (k, &v) in b.iter().enumerate() {
            wf(&mut s, bm + 4 * k as u32, v);
        }
        for k in 0..3 {
            wf(&mut s, T + 4 * k as u32, a[k]);
            wf(&mut s, SC + 4 * k as u32, c[k]);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(R), sext(T), sext(bm), sext(SC));
        let model = compose_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_80081948", math::func_80081948, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- the side test ----

const P: [u32; 4] = [0x8030_3000, 0x8030_3010, 0x8030_3020, 0x8030_3030];
const N: [u32; 3] = [0x8030_3040, 0x8030_3050, 0x8030_3060];
const K1: u32 = 0x800A_DCC8;
const K2: u32 = 0x800A_DCCC;

fn cross(a: [f32; 3], b: [f32; 3]) -> Option<[f32; 3]> {
    Some([
        sub(mul(a[1], b[2])?, mul(b[1], a[2])?)?,
        sub(mul(a[2], b[0])?, mul(b[2], a[0])?)?,
        sub(mul(a[0], b[1])?, mul(b[0], a[1])?)?,
    ])
}

/// IDO's `x < 0 ? -x : x` (keeps -0).
fn cabs(x: f32) -> f32 {
    if x < 0.0 {
        -x
    } else {
        x
    }
}

fn dominant(v: [f32; 3]) -> usize {
    let [a, b, c] = v.map(cabs);
    if a < b {
        if b < c {
            2
        } else {
            1
        }
    } else if a < c {
        2
    } else {
        0
    }
}

fn magnitude(v: [f32; 3]) -> Option<f32> {
    add(add(cabs(v[0]), cabs(v[1]))?, cabs(v[2]))
}

fn side_model(s: &State) -> Option<(State, u64)> {
    let mut w = s.clone();
    let sp = SP_AT - 0x78;
    saves(&mut w, s, 0x78, &[(0x14, RA), (0x78, A0), (0x7C, A1), (0x80, A2)]);
    let p = [A0, A1, A2, A3].map(|r| s.ctx.gpr[r] as u32);
    let n = [0, 1, 2].map(|k| word(s, SP_AT + 0x10 + 4 * k));
    let p0 = v3(&w, p[0]);
    for (k, off) in [(1, 0x6C), (2, 0x60), (3, 0x54)] {
        let pk = v3(&w, p[k]);
        for j in 0..3 {
            wf(&mut w, sp + off + 4 * j as u32, sub(pk[j], p0[j])?);
        }
    }
    cross(v3(&w, n[0]), v3(&w, n[1]))?;
    call_c(&mut w, 0x78, &[(A0, sext(sp + 0x48)), (A1, sext(n[0])), (A2, sext(n[1]))], imports::func_80015538);
    if v3(&w, sp + 0x48).iter().all(|&x| x == 0.0) {
        return Some((w, 0));
    }
    for (out, e, nn) in [(0x48, 0x6C, n[0]), (0x3C, 0x60, n[1]), (0x30, 0x54, n[2])] {
        cross(v3(&w, sp + e), v3(&w, nn))?;
        call_c(&mut w, 0x78, &[(A0, sext(sp + out)), (A1, sext(sp + e)), (A2, sext(nn))], imports::func_80015538);
    }
    let (c, d, e) = (v3(&w, sp + 0x48), v3(&w, sp + 0x3C), v3(&w, sp + 0x30));
    let agree = |x: f32, y: f32, z: f32| if (x as f64) < 0.0 { y <= 0.0 && z <= 0.0 } else { 0.0 <= y && 0.0 <= z };
    let v0 = if rf(&w, K1) < magnitude(c)? {
        let k = dominant(c);
        agree(c[k], d[k], e[k])
    } else if magnitude(d)? < rf(&w, K2) {
        true
    } else {
        let k = dominant(d);
        if (d[k] as f64) < 0.0 {
            e[k] <= 0.0
        } else {
            0.0 <= e[k]
        }
    };
    Some((w, u64::from(v0)))
}

fn side_case(seed: u64, p: [[f32; 3]; 4], n: [[f32; 3]; 3], k: (Option<f32>, Option<f32>)) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    load_data(&mut s);
    for (a, v) in P.iter().zip(&p).chain(N.iter().zip(&n)) {
        for (j, &x) in v.iter().enumerate() {
            wf(&mut s, a + 4 * j as u32, x);
        }
    }
    for (j, &a) in N.iter().enumerate() {
        wr(&mut s, SP_AT + 0x10 + 4 * j as u32, a);
    }
    if let Some(x) = k.0 {
        wf(&mut s, K1, x);
    }
    if let Some(x) = k.1 {
        wf(&mut s, K2, x);
    }
    for (r, &a) in [A0, A1, A2, A3].iter().zip(&P) {
        s.ctx.gpr[*r] = sext(a);
    }
    let model = side_model(&s);
    prop_assume!(model.is_some());
    let (want, v0) = model.unwrap();
    let after = run("func_80081BE8", math::func_80081BE8, &s)?;
    same_memory(&after, &want)?;
    prop_assert_eq!(after.ctx.gpr[V0], v0);
    Ok(())
}

/// Small exact values (zeros of both signs, ties of magnitude) most of the
/// time, so signs, ties and the zero test are hit.
fn small() -> BoxedStrategy<f32> {
    prop_oneof![3 => (-2i32..=2).prop_map(|n| n as f32), 1 => Just(-0.0f32), 1 => Just(0.5f32), 1 => Just(-0.5f32), 2 => ordinary()].boxed()
}

fn vec3(e: BoxedStrategy<f32>) -> BoxedStrategy<[f32; 3]> {
    prop::array::uniform3(e).boxed()
}

fn threshold() -> BoxedStrategy<Option<f32>> {
    prop_oneof![Just(None), (0u32..8).prop_map(|n| Some(n as f32 * 0.5)), float().prop_map(Some)].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Small exact geometry (ties, zeros, parallel normals), ordinary and
    /// edge floats, and thresholds perturbed (around the magnitudes of
    /// small vectors). Constructed: a threshold equal to `|c|` or `|d|`
    /// (the `<` ties), and `c = 0` (`p1 = p0`) with a negative first
    /// threshold or `d = 0` (`p2 = p0`) with a zero second one, so the
    /// dominant component's sign test sees exactly 0.
    #[test]
    fn func_80081BE8(seed: u64, p in prop::array::uniform4(prop_oneof![3 => vec3(small()), 1 => vec3(float())]),
                     n in prop::array::uniform3(prop_oneof![3 => vec3(small()), 1 => vec3(float())]), parallel in 0u8..4,
                     k in (threshold(), threshold()), same in 0u8..8, tie in 0u8..8) {
        let (mut p, mut n, mut k) = (p, n, k);
        match parallel {
            0 => n[1] = n[0].map(|x| x * 2.0),
            1 => n[1] = n[0].map(|x| -x),
            _ => {}
        }
        if (1..4).contains(&same) {
            p[same as usize] = p[0];
        }
        let e = |p: &[[f32; 3]; 4], j: usize| -> Option<[f32; 3]> { Some([sub(p[j][0], p[0][0])?, sub(p[j][1], p[0][1])?, sub(p[j][2], p[0][2])?]) };
        match tie {
            1 => k.0 = e(&p, 1).and_then(|e1| cross(e1, n[0])).and_then(magnitude).or(k.0),
            2 => k = (Some(f32::MAX), e(&p, 2).and_then(|e2| cross(e2, n[1])).and_then(magnitude).or(k.1)),
            3 => {
                p[2] = p[0];
                k = (Some(f32::MAX), Some(0.0));
            }
            4 => {
                p[1] = p[0];
                k.0 = Some(-1.0);
            }
            _ => {}
        }
        side_case(seed, p, n, k)?;
    }
}

// ---- node walks ----

const NODES: u32 = 0x8032_0000;
const LEAF: u32 = NODES + 0x100 * 8;
const TYPES: [u32; 8] = [0x5064, 0xD064, 0xD065, 0x3064, 0x8001, 0x5066, 0x4000, 0xC000];
const OUT: u32 = 0x8034_0000;
const PARENT: u32 = 0x8034_1000;
const PIVOT: u32 = 0x8034_2000;
const LIST: u32 = 0x8034_4000;
const FOUND: u32 = 0x8034_5000;

#[derive(Clone, Debug)]
struct Node {
    ty: u8,
    flags: u16,
    kids: Vec<(bool, u32)>,
    m: [f32; 12],
    p: [f32; 3],
}

fn node() -> BoxedStrategy<Node> {
    (0u8..8, any::<u16>(), prop::collection::vec((prop::bool::weighted(0.2), 0u32..64), 0..4), prop::array::uniform12(bounded()), prop::array::uniform3(bounded()))
        .prop_map(|(ty, flags, kids, m, p)| Node { ty, flags, kids, m, p })
        .boxed()
}

fn nodes() -> BoxedStrategy<Vec<Node>> {
    prop::collection::vec(node(), 1..6).boxed()
}

fn node_at(i: u32) -> u32 {
    NODES + 0x100 * i
}

/// Node `i` at `NODES + 0x100 i`: the type word `TYPES[ty]`, the halfword
/// flags at `+0xC`, the child count at `+0x14` and list pointer at `+0x18`
/// (to `+0xC0`), the 3x4 at `+0x1C`, the pivot floats at `+0x4C`. Children
/// are null or later nodes (the last node's go to a childless leaf of type
/// 0x3064), so every walk ends.
fn tree(s: &mut State, nodes: &[Node]) {
    wr(s, LEAF, 0x3064);
    wr(s, LEAF + 0x14, 0);
    let len = nodes.len() as u32;
    for (i, nd) in nodes.iter().enumerate() {
        let i = i as u32;
        let n = node_at(i);
        wr(s, n, TYPES[nd.ty as usize]);
        wh(s, n + 0xC, nd.flags);
        wr(s, n + 0x14, nd.kids.len() as u32);
        wr(s, n + 0x18, n + 0xC0);
        for (k, &x) in nd.m.iter().chain(&nd.p).enumerate() {
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
    }
}

/// One of the nodes, the leaf, or an address no node has.
fn target(len: usize, k: u32) -> u32 {
    let len = len as u32;
    match k % (len + 2) {
        x if x < len => node_at(x),
        x if x == len => LEAF,
        _ => 0x8031_0000,
    }
}

/// `L` of node `n` at `at`: the 3x4 rows with column 3 `(0, 0, 0, 1)`.
fn local_matrix(w: &mut State, n: u32, at: u32) {
    for r in 0..4 {
        for c in 0..3 {
            let v = word(w, n + 0x1C + 12 * r + 4 * c);
            wr(w, at + 16 * r + 4 * c, v);
        }
        wr(w, at + 16 * r + 12, if r == 3 { 0x3F80_0000 } else { 0 });
    }
}

/// The pivot step on `L`'s translation (at `t`), `q` the pivot node.
fn pivot(w: &mut State, n: u32, q: u32, t: u32) -> Option<()> {
    for j in 0..3 {
        for (row, off) in [(0, 0x4C), (1, 0x50), (2, 0x54)] {
            let v = add(rf(w, t + 4 * j), mul(neg(rf(w, q + off))?, rf(w, n + 0x1C + 12 * row + 4 * j))?)?;
            wf(w, t + 4 * j, v);
        }
        let v = add(rf(w, t + 4 * j), rf(w, q + 0x4C + 4 * j))?;
        wf(w, t + 4 * j, v);
    }
    Some(())
}

fn copy16(w: &mut State, from: u32, to: u32) {
    for k in 0..16 {
        let v = word(w, from + 4 * k);
        wr(w, to + 4 * k, v);
    }
}

fn world_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0xE8;
    saves(&mut w, s, 0xE8, &[(0x30, S6), (0x28, S4), (0x20, S2), (0x1C, S1), (0x34, RA), (0x2C, S5), (0x24, S3), (0x18, S0)]);
    let [target, out, n, parent] = [A0, A1, A2, A3].map(|r| s.ctx.gpr[r] as u32);
    if n == 0 {
        return Some(w);
    }
    let ty = word(&w, n);
    call_c(&mut w, 0xE8, &[(A0, sext(sp + 0x90)), (A1, sext(parent))], imports::func_800156DC);
    let transform = ty == 0xD064 || ty == 0xD065;
    if ty != 0x5064 && !transform {
        if target == n {
            copy16(&mut w, sp + 0x90, out);
        }
        return Some(w);
    }
    if transform {
        let q = if ty == 0xD065 { n } else { word(&w, sp + 0xD4) };
        local_matrix(&mut w, n, sp + 0x50);
        if half(&w, n + 0xC) & 0x10 != 0 {
            pivot(&mut w, n, q, sp + 0x80)?;
        }
        call_c(&mut w, 0xE8, &[(A0, sext(sp + 0x90)), (A1, sext(sp + 0x50)), (A2, sext(parent))], imports::func_80015724);
    }
    if target == n {
        copy16(&mut w, sp + 0x90, out);
    }
    let count = word(&w, n + 0x14) as i32;
    for i in 0..count.max(0) as u32 {
        let child = word(&w, word(&w, n + 0x18) + 4 * i);
        if child != 0 {
            let g = &mut w.ctx.gpr;
            (g[S0], g[S1], g[S2], g[S4], g[S5], g[S6]) = (u64::from(4 * i), sext(4 * count as u32), sext(out), sext(n), sext(sp + 0x90), sext(target));
            call_c(&mut w, 0xE8, &[(A0, sext(target)), (A1, sext(out)), (A2, sext(child)), (A3, sext(sp + 0x90))], imports::func_80082C80);
        }
    }
    Some(w)
}

fn path_model(s: &State) -> State {
    let mut w = s.clone();
    let sp = SP_AT - 0x58;
    saves(&mut w, s, 0x58, &[(0x40, FP), (0x3C, S7), (0x38, S6), (0x44, RA), (0x34, S5), (0x30, S4), (0x2C, S3), (0x28, S2), (0x24, S1), (0x20, S0), (0x64, A3)]);
    let [target, n, path, depth] = [A0, A1, A2, A3].map(|r| s.ctx.gpr[r] as u32);
    if n == 0 {
        return w;
    }
    let (max, found) = (word(&w, SP_AT + 0x10), word(&w, SP_AT + 0x14));
    if (depth as i32) >= (max.wrapping_sub(1) as i32) || word(&w, found) != 0 {
        return w;
    }
    let ty = word(&w, n);
    wr(&mut w, sp + 0x48, ty);
    if target == n {
        wr(&mut w, found, 1);
        let end = if ty & 0x8000 != 0 { depth.wrapping_add(1) } else { depth };
        wr(&mut w, path.wrapping_add(end.wrapping_mul(4)), 0);
    } else if ty & 0x4000 != 0 {
        let d = depth.wrapping_add(u32::from(ty & 0x8000 != 0));
        let count = word(&w, n + 0x14) as i32;
        if count > 0 && word(&w, found) == 0 {
            let mut i = 0u32;
            loop {
                let child = word(&w, word(&w, n + 0x18) + 4 * i);
                if child != 0 {
                    wr(&mut w, sp + 0x10, max);
                    wr(&mut w, sp + 0x14, found);
                    let g = &mut w.ctx.gpr;
                    (g[S0], g[S2], g[S3], g[S4], g[S5], g[S6], g[S7], g[FP]) =
                        (u64::from(4 * i), sext(4 * count as u32), sext(found), sext(d), sext(max), sext(n), sext(target), sext(path));
                    call_c(&mut w, 0x58, &[(A0, sext(target)), (A1, sext(child)), (A2, sext(path)), (A3, sext(d))], imports::func_80082FA4);
                }
                i += 1;
                if i as i32 >= count || word(&w, found) != 0 {
                    break;
                }
            }
        }
    }
    if ty & 0x8000 != 0 && word(&w, found) != 0 {
        wr(&mut w, path.wrapping_add(depth.wrapping_mul(4)), n);
    }
    w
}

fn chain_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0xE8;
    saves(&mut w, s, 0xE8, &[(0x3C, RA), (0x38, S5), (0x34, S4), (0x30, S3), (0x2C, S2), (0x28, S1), (0x24, S0)]);
    save_double(&mut w, sp + 0x18, s.ctx.fpr[22].u64);
    save_double(&mut w, sp + 0x10, s.ctx.fpr[20].u64);
    let [path, out] = [A0, A1].map(|r| s.ctx.gpr[r] as u32);
    for (k, v) in [(4, 0), (8, 0), (0xC, 0), (0x10, 0), (0x18, 0), (0x1C, 0), (0x20, 0), (0x24, 0), (0x2C, 0), (0x30, 0), (0x34, 0), (0x38, 0)]
        .into_iter()
        .chain([0, 0x14, 0x28, 0x3C].map(|k| (k, 0x3F80_0000)))
    {
        wr(&mut w, out + k, v);
    }
    if word(&w, path) == 0 {
        return Some(w);
    }
    let mut q = word(&w, sp + 0xDC);
    let mut cur = path;
    loop {
        let n = word(&w, cur);
        let ty = word(&w, n);
        if ty & 0x8000 != 0 {
            if ty == 0xD065 {
                q = n;
            }
            local_matrix(&mut w, n, sp + 0x58);
            if half(&w, n + 0xC) & 0x10 != 0 {
                pivot(&mut w, n, q, sp + 0x88)?;
            }
            call_c(&mut w, 0xE8, &[(A0, sext(out)), (A1, sext(sp + 0x58)), (A2, sext(out))], imports::func_80015724);
        }
        cur += 4;
        if word(&w, cur) == 0 {
            break;
        }
    }
    wr(&mut w, sp + 0xDC, q);
    Some(w)
}

fn weights_model(s: &State) -> State {
    let mut w = s.clone();
    let sp = SP_AT - 0x38;
    saves(&mut w, s, 0x38, &[(0x30, S4), (0x28, S2), (0x34, RA), (0x2C, S3), (0x24, S1), (0x20, S0)]);
    save_double(&mut w, sp + 0x18, s.ctx.fpr[20].u64);
    let [n, list] = [A0, A1].map(|r| s.ctx.gpr[r] as u32);
    if n == 0 || list == 0 {
        return w;
    }
    let ty = word(&w, n);
    let set = |w: &mut State, k: u32, x: u32| {
        if k < 8 {
            wr(w, n + 0x1C + 4 * k, x);
        }
    };
    if ty == 0x5066 {
        w.ctx.fpr[20].set_u32l(0);
        let mut k = 0u32;
        if 0.0 <= rf(&w, list) {
            loop {
                let x = if k == 0 { 0 } else { word(&w, list + 4 * k) };
                set(&mut w, k, x);
                k += 1;
                if !(0.0 <= rf(&w, list + 4 * k)) {
                    break;
                }
            }
        }
        set(&mut w, k, 0xBF80_0000);
    }
    if ty & 0x4000 != 0 {
        let count = word(&w, n + 0x14) as i32;
        for i in 0..count.max(0) as u32 {
            let child = word(&w, word(&w, n + 0x18) + 4 * i);
            let g = &mut w.ctx.gpr;
            (g[S0], g[S1], g[S2], g[S3], g[S4]) = (u64::from(4 * i), 0, sext(n), sext(4 * count as u32), sext(list));
            call_c(&mut w, 0x38, &[(A0, sext(child)), (A1, sext(list))], imports::func_800833B4);
        }
    }
    w
}

fn weight() -> BoxedStrategy<f32> {
    prop_oneof![4 => 0.0f32..100.0, 1 => Just(0.0f32), 1 => Just(-0.0f32), 1 => Just(f32::INFINITY), 1 => (1u32..0x0080_0000).prop_map(f32::from_bits)].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Random trees of every node kind, with and without pivots, any
    /// target (none, the leaf, a node reached twice). Only the top node's
    /// 0xD064 takes its pivot from the frame (the QUIRK reads near address
    /// 0 below a transform).
    #[test]
    fn func_80082C80(seed: u64, nodes in nodes(), tk: u32, parent in prop::array::uniform16(bounded()), q in prop::array::uniform3(bounded()), null in prop::bool::weighted(0.05)) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, OUT, 0x40);
        tree(&mut s, &nodes);
        for i in 1..nodes.len() as u32 {
            if word(&s, node_at(i)) == 0xD064 {
                let f = half(&s, node_at(i) + 0xC);
                wh(&mut s, node_at(i) + 0xC, f & !0x10);
            }
        }
        for (k, &x) in parent.iter().enumerate() {
            wf(&mut s, PARENT + 4 * k as u32, x);
        }
        for (k, &x) in q.iter().enumerate() {
            wf(&mut s, PIVOT + 0x4C + 4 * k as u32, x);
        }
        wr(&mut s, SP_AT - 0x14, PIVOT);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(target(nodes.len(), tk)), sext(OUT), if null { 0 } else { sext(NODES) }, sext(PARENT));
        let model = world_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_80082C80", misc::func_80082C80, &s)?;
        same_memory(&after, &model.unwrap())?;
    }

    /// Random trees, any target, depths around the limit (`max - 1`
    /// wrapping at `i32::MIN`), negative depths, `found` already set.
    #[test]
    fn func_80082FA4(seed: u64, nodes in nodes(), tk: u32, depth in -2i32..6,
                     max in prop_oneof![3 => 0i32..10, 1 => Just(i32::MIN), 1 => Just(i32::MIN + 1), 1 => any::<i32>()],
                     found in prop_oneof![4 => Just(0u32), 1 => any::<u32>()], null in prop::bool::weighted(0.05)) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, LIST - 0x40, 0x100);
        tree(&mut s, &nodes);
        wr(&mut s, FOUND, found);
        wr(&mut s, SP_AT + 0x10, max as u32);
        wr(&mut s, SP_AT + 0x14, FOUND);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(target(nodes.len(), tk)), if null { 0 } else { sext(NODES) }, sext(LIST), depth as i64 as u64);
        let want = path_model(&s);
        let after = run("func_80082FA4", misc::func_80082FA4, &s)?;
        same_memory(&after, &want)?;
    }

    /// Lists of any nodes (repeats, non-transforms, pivots before and after
    /// a 0xD065), the empty list.
    #[test]
    fn func_80083190(seed: u64, nodes in nodes(), list in prop::collection::vec(0u32..8, 0..7), q in prop::array::uniform3(bounded())) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, OUT, 0x40);
        tree(&mut s, &nodes);
        for (k, &i) in list.iter().enumerate() {
            wr(&mut s, LIST + 4 * k as u32, node_at(i % nodes.len() as u32));
        }
        wr(&mut s, LIST + 4 * list.len() as u32, 0);
        for (k, &x) in q.iter().enumerate() {
            wf(&mut s, PIVOT + 0x4C + 4 * k as u32, x);
        }
        wr(&mut s, SP_AT - 0xC, PIVOT);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(LIST), sext(OUT));
        let model = chain_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_80083190", misc::func_80083190, &s)?;
        same_memory(&after, &model.unwrap())?;
    }

    /// Random trees, weight lists of 0..10 (zeros of both signs, infinity)
    /// ended by a negative or NaN, null node or list.
    #[test]
    fn func_800833B4(seed: u64, nodes in nodes(), weights in prop::collection::vec(weight(), 0..11),
                     end in prop_oneof![Just(-1.0f32), Just(f32::NAN), -100.0f32..-0.001, Just(f32::NEG_INFINITY), Just(-1.0e-40f32)], null in 0u8..12) {
        let mut s = state(seed);
        tree(&mut s, &nodes);
        for (k, &x) in weights.iter().chain([end].iter()).enumerate() {
            wf(&mut s, LIST + 4 * k as u32, x);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (if null == 0 { 0 } else { sext(NODES) }, if null == 1 { 0 } else { sext(LIST) });
        let want = weights_model(&s);
        let after = run("func_800833B4", misc::func_800833B4, &s)?;
        same_memory(&after, &want)?;
    }
}
