//! Depth 1 at 0x800290A4..0x8002FB18: the fade step and two HUD panels
//! over the record setters, the track and profile tests and the pause
//! broadcast (game::misc), the button words (game::input) and the
//! heap-cursor test (game::heap). Recompiled C vs Rust with the callees as
//! C. The models replay the callees' C on a copy of the state in the same
//! order with the same arguments (their statements live in their own
//! tests), adding the functions' own stores, and whole RDRAM is compared.

// Tests are named after the functions (func_800290A4), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::imports;
use game::input::{BUTTON_WORDS, PAD_RECORDS};
use game::recomp::{reg::*, RecompFn};
use game::{heap, input, misc};
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

/// A halfword, sign-extended as `sll 16; sra 16` leaves it.
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

fn wd(s: &mut State, a: u32, v: f64) {
    let b = v.to_bits();
    wr(s, a, (b >> 32) as u32);
    wr(s, a + 4, b as u32);
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
fn sub(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? - ok(b)?)
}

/// IDO's float → unsigned idiom as the oracle runs it: the conversion in
/// round-toward-zero through the host's lrintf (0 for NaN and |x| > 2^31,
/// `i32::MIN` at ±2^31), a negative word giving `0xFFFFFFFF`; the flag
/// test reads 0, so its second path never runs.
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

/// A frame time: the game's, or anything ordinary.
fn frame_time() -> BoxedStrategy<f32> {
    prop_oneof![2 => Just(1.0f32 / 60.0), 1 => Just(1.0f32 / 30.0), 2 => 0.0f32..0.1, 2 => ordinary()].boxed()
}

/// Test words: zero, one, or anything.
fn flag() -> BoxedStrategy<u32> {
    prop_oneof![Just(0u32), Just(1u32), any::<u32>()].boxed()
}

const DT: u32 = 0x8012_0BF8;

// ---- misc: the fade step ----

const FADE: u32 = 0x800A_2604;
const FADE_H: u32 = 0x800A_2608;
const FADE_N: u32 = 0x800A_260C;
const FADE_K: u32 = 0x800A_9DEC;

/// The fade step by the statement, with the callees' C replayed; None
/// where a NaN would reach the subtraction.
fn fade_model(s: &State) -> Option<(State, u64)> {
    let mut w = s.clone();
    let sp = SP_AT - 0x28;
    wr(&mut w, sp + 0x1C, s.ctx.gpr[RA] as u32);
    let no = [(A0, sext(-103i32 as u32)), (A1, 0), (A2, 0), (A3, 0)];
    let f = rf(&w, FADE);
    if f == 255.0 || word(&w, FADE_N) as i32 > 0 {
        wr(&mut w, sp + 0x10, 255);
        wr(&mut w, sp + 0x24, 0);
        call_c(&mut w, 0x28, &no, imports::func_8000AB24);
        let n = (word(&w, FADE_N) as i32).wrapping_sub(1);
        wr(&mut w, FADE_N, n as u32);
        if n > 0 {
            return Some((w, 0));
        }
    }
    let mut f = rf(&w, FADE);
    let mut done = false;
    if f <= 0.0 {
        let h = (half(&w, FADE_H) as i16).wrapping_sub(1);
        wh(&mut w, FADE_H, h as u16);
        done = h <= 0;
        f = 0.0;
    } else {
        f = sub(f, mul(rf(&w, FADE_K), rf(&w, DT))?)?;
    }
    if f < 0.0 {
        f = 0.0;
        wh(&mut w, FADE_H, 3);
    }
    wf(&mut w, FADE, f);
    wr(&mut w, sp + 0x10, u(f));
    wr(&mut w, sp + 0x24, u32::from(done));
    call_c(&mut w, 0x28, &no, imports::func_8000AB24);
    if done {
        call_c(&mut w, 0x28, &[(A0, sext(-103i32 as u32)), (A1, 0)], imports::func_8000A920);
        wr(&mut w, 0x800A_4BD8, 1);
        wr(&mut w, 0x800A_4BDC, 0);
        wr(&mut w, FADE_N, 3);
        wf(&mut w, FADE, 255.0);
    }
    Some((w, u64::from(done)))
}

// ---- misc: the HUD panels ----

const PANEL_FLAGS: u32 = 0x8011_A240;

/// `L = L + d * dt` clamped to 254 above, 0.0 stored over it below 0.
fn level(w: &mut State, at: u32, up: bool, s: f32) -> Option<()> {
    let d = if up { s } else { mul(s, -1.0)? };
    let mut l = add(rf(w, at), mul(d, rf(w, DT))?)?;
    if 254.0 < l {
        l = 254.0;
    }
    wf(w, at, l);
    if l < 0.0 {
        wf(w, at, 0.0);
    }
    Some(())
}

/// A920(id, 1), AA04(id, x, y), AB24(id, colour) with the fifth argument
/// stored first; `sp` at `SP_AT - frame`.
fn widget(w: &mut State, frame: u32, id: u32, x: u32, y: u32, rgba: [u32; 4]) {
    call_c(w, frame, &[(A0, u64::from(id)), (A1, 1)], imports::func_8000A920);
    call_c(w, frame, &[(A0, u64::from(id)), (A1, sext16(x)), (A2, sext16(y))], imports::func_8000AA04);
    wr(w, SP_AT - frame + 0x10, rgba[3]);
    call_c(w, frame, &[(A0, u64::from(id)), (A1, u64::from(rgba[0])), (A2, u64::from(rgba[1])), (A3, u64::from(rgba[2]))], imports::func_8000AB24);
}

const GREEN: [u32; 4] = [0xA3, 0xBE, 0x11, 0xFE];

fn cyan(a: u32) -> [u32; 4] {
    [0x32, 0xFF, 0xFF, a]
}

fn s16w(v: u32) -> u32 {
    v as u16 as i16 as i32 as u32
}

#[derive(Clone, Debug)]
struct Panel {
    args: [u32; 3],
    flags: [u32; 4],
    bits: u32,
    levels: [f32; 2],
    s: f32,
    dt: f32,
    k: f32,
}

fn level_value() -> BoxedStrategy<f32> {
    prop_oneof![
        2 => 0.0f32..=254.0,
        1 => prop::sample::select(vec![0.0f32, -0.0, 254.0, 255.0, 253.5, -1.0, f32::INFINITY, f32::NEG_INFINITY]),
        1 => ordinary(),
    ]
    .boxed()
}

fn panel() -> impl Strategy<Value = Panel> {
    let arg = prop_oneof![3 => (-0x40i32..0x180).prop_map(|v| v as u32), 1 => any::<u32>()];
    (
        prop::array::uniform3(arg),
        prop::array::uniform4(flag()),
        prop_oneof![Just(0u32), Just(0x4000), Just(0x8000), Just(0xC000), any::<u32>()],
        prop::array::uniform2(level_value()),
        prop_oneof![3 => Just(838.2f32), 1 => Just(0.0f32), 1 => Just(-0.0f32), 3 => ordinary()],
        frame_time(),
        prop_oneof![Just(0.3333f32), ordinary()],
    )
        .prop_map(|(args, flags, bits, levels, s, dt, k)| Panel { args, flags, bits, levels, s, dt, k })
}

/// Writes a panel's inputs: flags at `PANEL_FLAGS + offs[i]`, levels at
/// `levels_at`, `s` and `k` at their addresses.
fn panel_state(seed: u64, p: &Panel, offs: [u32; 4], levels_at: u32, s_at: u32, k_at: u32) -> State {
    let mut s = state(seed);
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(p.args[0]), sext(p.args[1]), sext(p.args[2]));
    for (o, v) in offs.iter().zip(p.flags) {
        wr(&mut s, PANEL_FLAGS + o, v);
    }
    wr(&mut s, 0x800A_4B94, p.bits);
    wf(&mut s, levels_at, p.levels[0]);
    wf(&mut s, levels_at + 4, p.levels[1]);
    wf(&mut s, s_at, p.s);
    wf(&mut s, k_at, p.k);
    wf(&mut s, DT, p.dt);
    s
}

fn panel1_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x38;
    let (x, y, h) = (s.ctx.gpr[A0] as u32, s.ctx.gpr[A1] as u32, s.ctx.gpr[A2] as u32);
    wr(&mut w, sp + 0x1C, s.ctx.gpr[RA] as u32);
    for (i, v) in [x, y, h].into_iter().enumerate() {
        wr(&mut w, SP_AT + 4 * i as u32, v);
    }
    let sv = rf(&w, 0x800A_9ED8);
    let (f20, f24) = (word(&w, PANEL_FLAGS + 0x20), word(&w, PANEL_FLAGS + 0x24));
    level(&mut w, 0x800A_2654, f20 != 0, sv)?;
    level(&mut w, 0x800A_2658, f24 != 0, sv)?;
    let scale = mul(h as i32 as f32, rf(&w, 0x800A_9EDC))?;
    wr(&mut w, sp + 0x2C, s16w(x.wrapping_sub(0x1B)));
    widget(&mut w, 0x38, 0xAF, x.wrapping_sub(0x1B), y.wrapping_sub(0xB), GREEN);
    wr(&mut w, sp + 0x28, s16w(x.wrapping_sub(0x14)));
    call_c(&mut w, 0x38, &[(A0, 0xAE), (A1, 1)], imports::func_8000A920);
    call_c(&mut w, 0x38, &[(A0, 0xAE), (A1, sext16(x.wrapping_sub(0x14))), (A2, sext16(y.wrapping_sub(7)))], imports::func_8000AA04);
    let a = u(rf(&w, 0x800A_2654));
    wr(&mut w, sp + 0x10, a);
    call_c(&mut w, 0x38, &[(A0, 0xAE), (A1, 0x32), (A2, 0xFF), (A3, 0xFF)], imports::func_8000AB24);
    let yh = y.wrapping_add(h);
    wr(&mut w, sp + 0x24, yh);
    widget(&mut w, 0x38, 0xB2, x.wrapping_sub(0x1B), yh.wrapping_add(0xF), GREEN);
    call_c(&mut w, 0x38, &[(A0, 0xB1), (A1, 1)], imports::func_8000A920);
    call_c(&mut w, 0x38, &[(A0, 0xB1), (A1, sext16(x.wrapping_sub(0x14))), (A2, sext16(yh.wrapping_add(0x17)))], imports::func_8000AA04);
    let a = u(rf(&w, 0x800A_2658));
    wr(&mut w, sp + 0x10, a);
    call_c(&mut w, 0x38, &[(A0, 0xB1), (A1, 0x32), (A2, 0xFF), (A3, 0xFF)], imports::func_8000AB24);
    widget(&mut w, 0x38, 0xB4, x.wrapping_sub(0x11), y.wrapping_add(0x12), GREEN);
    call_c(&mut w, 0x38, &[(A0, 0xB4), (A1, sext(0x3F80_0000)), (A2, sext(scale.to_bits()))], imports::func_8000AAC0);
    let bits = word(&w, 0x800A_4B94);
    if word(&w, PANEL_FLAGS + 0x24) != 0 && bits & 0x8000 != 0 {
        widget(&mut w, 0x38, 0xB3, x.wrapping_sub(0x20), yh.wrapping_sub(0x13), cyan(0xFE));
    }
    let bits = word(&w, 0x800A_4B94);
    if word(&w, PANEL_FLAGS + 0x20) != 0 && bits & 0x4000 != 0 {
        wr(&mut w, sp + 0x2C, s16w(x.wrapping_sub(0x20)));
        widget(&mut w, 0x38, 0xB0, x.wrapping_sub(0x20), y.wrapping_sub(0x11), cyan(0xFE));
    }
    Some(w)
}

fn panel2_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x30;
    let y = s.ctx.gpr[A1] as u32;
    wr(&mut w, sp + 0x1C, s.ctx.gpr[RA] as u32);
    for (i, r) in [A0, A1, A2].into_iter().enumerate() {
        wr(&mut w, SP_AT + 4 * i as u32, s.ctx.gpr[r] as u32);
    }
    let sv = rf(&w, 0x800A_9EE0);
    let (f18, f1c) = (word(&w, PANEL_FLAGS + 0x18), word(&w, PANEL_FLAGS + 0x1C));
    level(&mut w, 0x800A_265C, f18 != 0, sv)?;
    level(&mut w, 0x800A_2660, f1c != 0, sv)?;
    let scale = mul(221.0, rf(&w, 0x800A_9EE4))?;
    wr(&mut w, sp + 0x24, s16w(y.wrapping_sub(0xE)));
    widget(&mut w, 0x30, 0xAB, 0x13, y.wrapping_sub(0xE), GREEN);
    wr(&mut w, sp + 0x20, s16w(y.wrapping_sub(7)));
    call_c(&mut w, 0x30, &[(A0, 0xAA), (A1, 1)], imports::func_8000A920);
    call_c(&mut w, 0x30, &[(A0, 0xAA), (A1, 0x16), (A2, sext16(y.wrapping_sub(7)))], imports::func_8000AA04);
    let a = u(rf(&w, 0x800A_265C));
    wr(&mut w, sp + 0x10, a);
    call_c(&mut w, 0x30, &[(A0, 0xAA), (A1, 0x32), (A2, 0xFF), (A3, 0xFF)], imports::func_8000AB24);
    widget(&mut w, 0x30, 0xA8, 0x109, y.wrapping_sub(0xE), GREEN);
    call_c(&mut w, 0x30, &[(A0, 0xA7), (A1, 1)], imports::func_8000A920);
    call_c(&mut w, 0x30, &[(A0, 0xA7), (A1, 0x110), (A2, sext16(y.wrapping_sub(7)))], imports::func_8000AA04);
    let a = u(rf(&w, 0x800A_2660));
    wr(&mut w, sp + 0x10, a);
    call_c(&mut w, 0x30, &[(A0, 0xA7), (A1, 0x32), (A2, 0xFF), (A3, 0xFF)], imports::func_8000AB24);
    widget(&mut w, 0x30, 0xAD, 0x30, y.wrapping_sub(4), GREEN);
    call_c(&mut w, 0x30, &[(A0, 0xAD), (A1, sext(scale.to_bits())), (A2, sext(0x3F80_0000))], imports::func_8000AAC0);
    if word(&w, PANEL_FLAGS + 0x14) != 0 {
        widget(&mut w, 0x30, 0xA9, 0xE6, y.wrapping_sub(0x13), cyan(0xFE));
    }
    if word(&w, PANEL_FLAGS + 0x10) != 0 {
        wr(&mut w, sp + 0x24, s16w(y.wrapping_sub(0x13)));
        widget(&mut w, 0x30, 0xAC, 0xC, y.wrapping_sub(0x13), cyan(0xFE));
    }
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_800290A4(seed: u64,
                     f in prop_oneof![2 => Just(255.0f32), 2 => 0.0f32..=255.0, 1 => prop::sample::select(vec![0.0f32, -0.0, 1.0, -1.0, 254.5, 256.0]), 1 => ordinary()],
                     n in prop_oneof![3 => -1i32..=2, 1 => any::<i32>()],
                     h in prop_oneof![3 => -1i16..=2, 1 => any::<i16>()],
                     k in prop_oneof![Just(850.0f32), ordinary()], dt in frame_time()) {
        let mut s = state(seed);
        wf(&mut s, FADE, f);
        wh(&mut s, FADE_H, h as u16);
        wr(&mut s, FADE_N, n as u32);
        wf(&mut s, FADE_K, k);
        wf(&mut s, DT, dt);
        let model = fade_model(&s);
        prop_assume!(model.is_some());
        let (w, v0) = model.unwrap();
        let after = run("func_800290A4", misc::func_800290A4, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_8002C780(seed: u64, p in panel()) {
        let s = panel_state(seed, &p, [0x20, 0x24, 0x28, 0x2C], 0x800A_2654, 0x800A_9ED8, 0x800A_9EDC);
        let model = panel1_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_8002C780", misc::func_8002C780, &s)?;
        same_memory(&after, &model.unwrap())?;
    }

    #[test]
    fn func_8002CC28(seed: u64, p in panel()) {
        let s = panel_state(seed, &p, [0x18, 0x1C, 0x14, 0x10], 0x800A_265C, 0x800A_9EE0, 0x800A_9EE4);
        let model = panel2_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_8002CC28", misc::func_8002CC28, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

/// Levels of exactly -0.0: `-0.0 + d * dt` stays -0.0 when the product is
/// -0.0 (`s = ±0`, `dt` of the other sign, or `s * -1.0` with `dt > 0`),
/// and `L < 0` must not store +0.0 over it. Every flag combination.
#[test]
fn func_8002C780_and_func_8002CC28_negative_zero_levels() {
    for (seed, flags) in [[0u32, 0, 0, 0], [1, 0, 1, 0], [0, 1, 0, 1], [1, 1, 1, 1]].into_iter().enumerate() {
        for s in [0.0f32, -0.0] {
            for dt in [1.0f32 / 60.0, -1.0 / 60.0] {
                let p = Panel { args: [0x40, 0x30, 0x10], flags, bits: 0xC000, levels: [-0.0, -0.0], s, dt, k: 0.3333 };
                let st = panel_state(seed as u64, &p, [0x20, 0x24, 0x28, 0x2C], 0x800A_2654, 0x800A_9ED8, 0x800A_9EDC);
                let after = compare("func_8002C780", misc::func_8002C780, &st).unwrap_or_else(|d| panic!("{d}"));
                same_memory(&after, &panel1_model(&st).unwrap()).unwrap();
                let st = panel_state(seed as u64, &p, [0x18, 0x1C, 0x14, 0x10], 0x800A_265C, 0x800A_9EE0, 0x800A_9EE4);
                let after = compare("func_8002CC28", misc::func_8002CC28, &st).unwrap_or_else(|d| panic!("{d}"));
                same_memory(&after, &panel2_model(&st).unwrap()).unwrap();
            }
        }
    }
}

// ---- misc: the track and profile tests ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8002DA0C(seed: u64, k in prop_oneof![3 => -2i8..=4, 1 => any::<i8>()], hi_i: u32,
                     c in prop_oneof![3 => 0u8..8, 1 => any::<u8>()], hi_j: u32,
                     table in prop_oneof![Just(0i16), Just(-1i16), any::<i16>()], bits: u8) {
        let mut s = state(seed);
        let (i, j) = (hi_i << 8 | u32::from(k as u8), hi_j << 8 | u32::from(c));
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(i), sext(j));
        let kk = i32::from(k);
        wh(&mut s, 0x8011_3E6C_u32.wrapping_add((2 * kk) as u32), table as u16);
        wb(&mut s, 0x8011_3E68_u32.wrapping_add(kk as u32), bits);
        let mut w = s.clone();
        let sp = SP_AT - 0x28;
        let r = i32::from(half(&s, 0x8011_3E6C_u32.wrapping_add((2 * kk) as u32)) as i16) >> ((2 * u32::from(c)) & 31);
        let q = r % 4;
        wr(&mut w, sp + 0x14, s.ctx.gpr[RA] as u32);
        wr(&mut w, SP_AT, i);
        wr(&mut w, SP_AT + 4, j);
        wr(&mut w, sp + 0x1C, u32::from(c));
        wh(&mut w, sp + 0x26, q as u16);
        call_c(&mut w, 0x28, &[(A0, kk as i64 as u64), (A1, u64::from(c)), (A2, u64::from(c))], imports::func_8002D9D0);
        let v = if w.ctx.gpr[V0] == 3 && q == 0 {
            1
        } else if kk < 3 {
            u64::from(u32::from(byte(&s, 0x8011_3E68_u32.wrapping_add(kk as u32))) & (1u32 << ((u32::from(c) + 1) & 31)) == 0)
        } else {
            0
        };
        let after = run("func_8002DA0C", misc::func_8002DA0C, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], v);
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_8002DB20(seed: u64, k in prop_oneof![3 => -1i8..=3, 1 => any::<i8>()], count in prop_oneof![3 => 0u8..10, 1 => any::<u8>()],
                     n in prop_oneof![3 => -1i32..6, 1 => any::<i32>()], flag: u8, tables in prop::array::uniform16(any::<u8>())) {
        let mut s = state(seed);
        let p = 0x8030_0000u32;
        wb(&mut s, p + 0x5E, k as u8);
        wb(&mut s, p + 0x6C, flag);
        wb(&mut s, 0x800A_21B4_u32.wrapping_add(i32::from(k) as u32), count);
        for (t, &b) in tables.iter().enumerate() {
            let base = if t < 8 { 0x8011_3E68u32 } else { 0x8011_368C };
            wb(&mut s, base.wrapping_add((i32::from(k) + (t % 8) as i32 - 4) as u32), b);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(p), sext(n as u32));
        let mut w = s.clone();
        let sp = SP_AT - 0x30;
        for (off, r) in [(0x2C, RA), (0x28, S4), (0x24, S3), (0x20, S2), (0x1C, S1), (0x18, S0)] {
            wr(&mut w, sp + off, s.ctx.gpr[r] as u32);
        }
        let mut found = -1i32;
        let mut seen = -1i32;
        let mut i = 0i32;
        loop {
            let k = i64::from(byte(&w, p + 0x5E) as i8);
            if i >= i32::from(byte(&w, 0x800A_21B4_u32.wrapping_add(k as u32))) {
                break;
            }
            call_c(&mut w, 0x30, &[(A0, sext(p)), (A1, k as u64), (A2, (i & 0xFF) as u64)], imports::func_8002DAD0);
            if w.ctx.gpr[V0] != 0 {
                seen = seen.wrapping_add(1);
                if seen == n {
                    found = i;
                    break;
                }
            }
            i += 1;
        }
        let after = run("func_8002DB20", misc::func_8002DB20, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(found as u32));
        same_memory(&after, &w)?;
    }

    /// Each of the three vec3 tests failing in turn (by one component, or
    /// through NaN or a signed zero), or all holding.
    #[test]
    fn func_8002DBD8(seed: u64, idx in 0u32..12, v in prop::array::uniform6(ordinary()),
                     fail in prop_oneof![Just(None), (0usize..3, 0usize..3, prop_oneof![Just(f32::NAN), ordinary()]).prop_map(Some)],
                     zeros in prop::array::uniform3(any::<bool>())) {
        let mut s = state(seed);
        let o = 0x8030_0000u32;
        wr(&mut s, o + 0x34, idx);
        let e = 0x800A_4C00 + 32 * idx;
        // e + 0xC against A (0x80118E50) and B (0x80118ED0), e against C
        // (0x80118E10). Equal copies, with -0.0 for 0.0 where asked.
        let refs = [(e + 0xC, 0x8011_8E50u32), (e + 0xC, 0x8011_8ED0), (e, 0x8011_8E10)];
        for (t, &(a, b)) in refs.iter().enumerate() {
            let base = if t == 2 { &v[3..6] } else { &v[0..3] };
            for c in 0..3 {
                wf(&mut s, a + 4 * c as u32, base[c]);
                let mut x = base[c];
                if zeros[t] && x == 0.0 {
                    x = -x;
                }
                wf(&mut s, b + 4 * c as u32, x);
            }
        }
        if let Some((t, c, x)) = fail {
            wf(&mut s, refs[t].1 + 4 * c as u32, x);
        }
        s.ctx.gpr[A0] = sext(o);
        let eq = |a: u32, b: u32| (0..3).all(|c| rf(&s, a + 4 * c) == rf(&s, b + 4 * c));
        let v0 = u64::from(!refs.iter().all(|&(a, b)| eq(a, b)));
        let mut w = s.clone();
        wr(&mut w, SP_AT - 0x18 + 0x14, s.ctx.gpr[RA] as u32);
        wr(&mut w, SP_AT, o);
        let after = run("func_8002DBD8", misc::func_8002DBD8, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
        same_memory(&after, &w)?;
    }

    /// Each pair differing in turn (in one of its three bytes), or none.
    #[test]
    fn func_8002DCF4(seed: u64, flags in prop_oneof![Just(0x4000u32), Just(0u32), any::<u32>()], bytes in prop::array::uniform12(any::<u8>()),
                     differ in prop_oneof![Just(None), (0usize..4, 0usize..3).prop_map(Some)], d in 1u8..=255) {
        let mut s = state(seed);
        wr(&mut s, 0x8009_B7D8, flags);
        let pairs = [(0x8011_3694u32, 0x800A_9ABCu32), (0x8011_36C0, 0x800A_9AC0), (0x8011_36EC, 0x800A_9AC4), (0x8011_3718, 0x800A_9AC8)];
        for (i, &(a, b)) in pairs.iter().enumerate() {
            for c in 0..3 {
                let x = bytes[3 * i + c];
                wb(&mut s, a + c as u32, x);
                let y = if differ == Some((i, c)) { x.wrapping_add(d) } else { x };
                wb(&mut s, b + c as u32, y);
            }
        }
        let v0 = u64::from(flags & 0x4000 != 0 && differ.is_none());
        let mut w = s.clone();
        wr(&mut w, SP_AT - 0x18 + 0x14, s.ctx.gpr[RA] as u32);
        let after = run("func_8002DCF4", misc::func_8002DCF4, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
        same_memory(&after, &w)?;
    }
}

// ---- input: the button words ----

const PADS_SETTINGS: u32 = 0x800D_697C;
const PADS_NONE: u32 = 0x800D_7490;
const PADS_SKIP: u32 = 0x800A_26D8;
const PADS_PRESENT: u32 = 0x800D_7498;
const PADS: u32 = 0x800D_74C0;

/// Stick halfwords at and around the thresholds (0.2 and 0.3 of 100) and
/// beyond.
fn stick_half() -> BoxedStrategy<i16> {
    prop_oneof![
        2 => prop::sample::select(vec![0i16, -31, -30, -29, -21, -20, -19, 19, 20, 21, 29, 30, 31]),
        2 => -35i16..=35,
        1 => -101i16..=101,
        1 => any::<i16>(),
    ]
    .boxed()
}

/// A threshold: the ROM's; its f32-rounded double, which a stick of 20 or
/// 30 meets exactly (the ROM's own doubles are never met, so `<=` against
/// `<` would be equivalent there); the double of another stick value's
/// float; or anything.
fn threshold(rom: f64) -> BoxedStrategy<f64> {
    prop_oneof![
        2 => Just(rom),
        2 => Just(f64::from(rom as f32)),
        1 => (-35i16..=35).prop_map(|n| f64::from(f32::from(n) / 100.0)),
        1 => -2.0f64..2.0,
    ]
    .boxed()
}

type Record = (i16, i16, [u8; 16]);

/// Four pad records: sticks near the thresholds, mostly-zero button bytes.
fn pad_records() -> BoxedStrategy<[Record; 4]> {
    let bytes = prop::array::uniform16(prop_oneof![3 => Just(0u8), 1 => any::<u8>()]).boxed();
    prop::array::uniform4((stick_half(), stick_half(), bytes)).boxed()
}

/// Four pads: buttons and stick bytes.
fn pads() -> BoxedStrategy<[(u16, i8, i8); 4]> {
    prop::array::uniform4((any::<u16>(), any::<i8>(), any::<i8>())).boxed()
}

/// The button word of one pad record by the statement.
fn buttons(s: &State, r: u32, [a, b, c, d]: [f64; 4]) -> (u32, f32, f32) {
    let mut v = 0u32;
    for (k, off) in [0x13u32, 0x12, 7, 6, 5, 4, 9, 8, 0x11, 0x10, 0xF, 0xE, 0xD, 0xC].into_iter().enumerate() {
        if byte(s, r + off) != 0 {
            v |= 1 << k;
        }
    }
    let fx = f32::from(half(s, r) as i16) / 100.0;
    let fy = f32::from(half(s, r + 2) as i16) / 100.0;
    let (x, y) = (f64::from(fx), f64::from(fy));
    let tests = [a < y, y < b, x < b, a < x, c < x && x < d, c < y && y < d, b <= x && x <= c, d <= x && x <= a, d <= y && y <= a, b <= y && y <= c];
    for (k, t) in tests.into_iter().enumerate() {
        if t {
            v |= 1 << (14 + k);
        }
    }
    (v, fx, fy)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8002ECA0(seed: u64, update: bool, present in prop::array::uniform4(prop_oneof![Just(0u32), Just(1u32)]),
                     pads in pads(), records in pad_records(),
                     old in prop::array::uniform4(any::<u32>()),
                     a in threshold(0.3), b in threshold(-0.3), c in threshold(-0.2), d in threshold(0.2)) {
        let mut s = state(seed);
        wr(&mut s, PADS_SETTINGS, 0);
        wr(&mut s, PADS_NONE, 0);
        wr(&mut s, PADS_SKIP, u32::from(!update));
        for i in 0..4u32 {
            let (bt, x, y) = pads[i as usize];
            wr(&mut s, PADS_PRESENT + 4 * i, present[i as usize]);
            wh(&mut s, PADS + 6 * i, bt);
            wb(&mut s, PADS + 6 * i + 2, x as u8);
            wb(&mut s, PADS + 6 * i + 3, y as u8);
            let r = PAD_RECORDS + 0x18 * i;
            let (x, y, bytes) = records[i as usize];
            wh(&mut s, r, x as u16);
            wh(&mut s, r + 2, y as u16);
            for (k, &v) in bytes.iter().enumerate() {
                wb(&mut s, r + 4 + k as u32, v);
            }
            wr(&mut s, BUTTON_WORDS + 4 * i, old[i as usize]);
        }
        for (at, v) in [(0x800A_9FA8u32, a), (0x800A_9FA0, b), (0x800A_9F98, c), (0x800A_9F90, d)] {
            wd(&mut s, at, v);
        }
        let mut w = s.clone();
        let sp = SP_AT - 0x30;
        wr(&mut w, sp + 0x2C, s.ctx.gpr[RA] as u32);
        for (off, f) in [(0x20, 24), (0x18, 22), (0x10, 20)] {
            let v = s.ctx.fpr[f].u64;
            wr(&mut w, sp + off, (v >> 32) as u32);
            wr(&mut w, sp + off + 4, v as u32);
        }
        call_c(&mut w, 0x30, &[], imports::func_8002EA28);
        let mut v0 = 0;
        for i in 0..4u32 {
            let (v, fx, fy) = buttons(&w, PAD_RECORDS + 0x18 * i, [a, b, c, d]);
            let o = word(&w, BUTTON_WORDS + 4 * i);
            wr(&mut w, BUTTON_WORDS + 0x10 + 4 * i, (o ^ v) & v);
            wr(&mut w, BUTTON_WORDS + 0x20 + 4 * i, (o ^ v) & o);
            wr(&mut w, BUTTON_WORDS + 4 * i, v);
            wf(&mut w, BUTTON_WORDS + 0x30 + 4 * i, fx);
            wf(&mut w, BUTTON_WORDS + 0x40 + 4 * i, fy);
            v0 = v;
        }
        let after = run("func_8002ECA0", input::func_8002ECA0, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(v0));
        for f in [20, 22, 24] {
            prop_assert_eq!(after.ctx.fpr[f].u64, s.ctx.fpr[f].u64);
        }
        same_memory(&after, &w)?;
    }
}

// ---- misc: the pause broadcast; heap: below the cursor ----

const POOL_LIST: u32 = 0x8030_0000;
const POOL_DESCS: u32 = 0x8030_0100;
const POOL_ELEMS: u32 = 0x8030_1000;
/// func_8005F31C, a verified callback: `[elem + 0x14] += 1`, `[elem + 0x18]
/// = arg` (see indirect.rs).
const MARK: u32 = 0x8005_F31C;

/// `counts.len()` pools of ids 1.. with MARK callbacks and `count` elements each,
/// `flagged` marking some with bit 8 of `+6` (skipped by broadcasts).
fn pool_registry(s: &mut State, seed: u64, counts: &[u32], flagged: u32) {
    s.randomise_memory(seed ^ 2, POOL_LIST, 0x1800);
    let mut m = s.rdram.mem();
    m.write_u32(0x800A_2170, POOL_LIST);
    for (k, &count) in counts.iter().enumerate() {
        let d = POOL_DESCS + 0x40 * k as u32;
        m.write_u32(POOL_LIST + 4 * k as u32, d);
        m.write_u32(d, k as u32 + 1);
        m.write_u32(d + 8, count);
        m.write_u32(d + 0xC, 0x68);
        m.write_u32(d + 0x10, POOL_ELEMS + 0x200 * k as u32);
        m.write_u32(d + 0x24, MARK);
        for i in 0..4u32 {
            let e = POOL_ELEMS + 0x200 * k as u32 + 0x68 * i;
            m.write_u16(e + 6, if flagged >> (4 * k as u32 + i) & 1 != 0 { 0x100 } else { 0 });
        }
    }
    m.write_u32(POOL_LIST + 4 * counts.len() as u32, 0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_8002F0EC(seed: u64, paused in prop_oneof![3 => Just(0u32), 1 => any::<u32>()], counts in proptest::collection::vec(0u32..=4, 0..3), flagged: u32) {
        let mut s = state(seed);
        pool_registry(&mut s, seed, &counts, flagged);
        wr(&mut s, 0x800A_26F8, paused);
        let mut w = s.clone();
        let sp = SP_AT - 0x28;
        wr(&mut w, sp + 0x14, s.ctx.gpr[RA] as u32);
        if paused == 0 {
            wr(&mut w, 0x800A_26F4, 0);
            wr(&mut w, sp + 0x18, 0x5061_7773);
            wr(&mut w, sp + 0x1C, u32::MAX);
            wr(&mut w, sp + 0x20, word(&s, 0x8009_B7E4));
            call_c(&mut w, 0x28, &[(A0, sext(0x416C_6C21)), (A1, sext(sp + 0x18))], imports::func_8003FA24);
        }
        let after = run("func_8002F0EC", misc::func_8002F0EC, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_8002FB18(seed: u64, level in 0u32..4, cursor: u32, p in prop_oneof![any::<u32>(), Just(0u32), Just(u32::MAX), (-2i32..=2).prop_map(|d| d as u32)], relative: bool) {
        let mut s = state(seed);
        wr(&mut s, 0x800A_2868, level);
        wr(&mut s, 0x800D_9DD8 + 4 * level, cursor);
        let p = if relative { cursor.wrapping_add(p) } else { p };
        s.ctx.gpr[A0] = sext(p);
        let mut w = s.clone();
        wr(&mut w, SP_AT - 0x18 + 0x14, s.ctx.gpr[RA] as u32);
        wr(&mut w, SP_AT, p);
        let after = run("func_8002FB18", heap::func_8002FB18, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(p < cursor));
        same_memory(&after, &w)?;
    }
}
