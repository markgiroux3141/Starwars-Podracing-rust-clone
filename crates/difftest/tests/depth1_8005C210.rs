//! Depth 1 at 0x8005C210..0x80065C74 (game::misc): the racers' markers, a
//! light rig and a level's light points, objects arriving at, placed at
//! and turning toward points, a child node's position and two vector
//! setters. Recompiled C vs Rust with the callees as C. The models replay
//! the callees' C on a copy of the state in the same order with the same
//! arguments (and the s registers and callee-saved FPRs the port holds),
//! adding the functions' own stores; whole RDRAM (and the result register)
//! is compared.

// Tests are named after the functions (func_8005C210), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
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

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

/// The game's data segment (`0x80098000..0x800AE8B0`) as the ROM image has
/// it: the test background is random, and the atan2 callee of
/// [`misc::func_80063EF4`] reads its series constants from there.
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

/// Bounded coordinates (no overflow in differences or squares).
fn coord() -> BoxedStrategy<f32> {
    prop_oneof![4 => -1.0e3f32..1.0e3f32, 2 => -1.0f32..1.0, 2 => (-12i32..=12).prop_map(|n| n as f32), 1 => Just(-0.0f32)].boxed()
}

fn some_word() -> BoxedStrategy<u32> {
    prop_oneof![Just(0u32), Just(1u32), any::<u32>()].boxed()
}

const O: u32 = 0x8030_0000;

// ---- the racers' markers ----

const RACERS: u32 = 0x8031_0000;
const OBJS: u32 = 0x8032_0000;

fn markers_model(s: &State, o: u32) -> State {
    let mut w = s.clone();
    saves(&mut w, s, 0x40, &[(0x18, S0), (0x1C, S1), (0x20, S2), (0x24, S3), (0x28, S4), (0x2C, S5), (0x30, S6), (0x34, S7), (0x38, FP), (0x3C, RA)]);
    let fl = word(&w, o + 8);
    if fl & 0xF != 1 || fl & 0x20 != 0 || word(&w, o + 0x1BC) as i32 <= 0 {
        return w;
    }
    let mut i = 0u32;
    loop {
        let r = word(&w, 0x8011_B1B8).wrapping_add(0x88 * i);
        let e = word(&w, r + 0x84);
        (w.ctx.gpr[S0], w.ctx.gpr[S1], w.ctx.gpr[S2], w.ctx.gpr[S3], w.ctx.gpr[S4]) = (sext(e), sext(r), sext(e + 0x50), u64::from(i), u64::from(0x88 * i));
        (w.ctx.gpr[S5], w.ctx.gpr[S6], w.ctx.gpr[S7], w.ctx.gpr[FP]) = (sext(o), sext(0x8011_B1B8), 0x200_0000, sext(0x8011_B1BC));
        let k = sext16(u32::from(half(&w, e + 4)));
        call_c(&mut w, 0x40, &[(A0, k), (A1, sext(e + 0x50)), (A2, sext(-9999i32 as u32))], imports::func_8000FEAC);
        let h = half(&w, r + 0x5C) as i16;
        if word(&w, e + 0x60) & 0x5000 == 0 && word(&w, e + 0x64) & 1 << 25 == 0 && h > 0 {
            let player = r == word(&w, 0x8011_B1BC) || r == word(&w, 0x8011_B1C0);
            if !player || word(&w, 0x800A_52BC) as i32 >= 2 {
                let v = if player { -i32::from(h) } else { i32::from(h) };
                let k = sext16(u32::from(half(&w, e + 4)));
                call_c(&mut w, 0x40, &[(A0, k), (A1, sext(e + 0x50)), (A2, sext(v as u32))], imports::func_8000FEAC);
            }
        }
        i += 1;
        if i as i32 >= word(&w, o + 0x1BC) as i32 {
            return w;
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Up to six racers with markers 0..7, each skipped by a flag or `h <=
    /// 0` or not, the two player records among them (or not), and an `o`
    /// whose count word is marker 0's word, so the first call ends the loop.
    #[test]
    fn func_8005C210(seed: u64, fl in prop_oneof![Just(1u32), Just(0x21u32), Just(0x11u32), any::<u32>()], n in prop_oneof![0i32..7, Just(-1i32)],
                     racers in prop::collection::vec((prop_oneof![3 => 0u16..8, 1 => 0xFFF8u16..=0xFFFF], prop_oneof![3 => Just(0u8), 1 => 1u8..3], prop_oneof![1 => Just(0i16), 1 => Just(-3i16), 4 => 1i16..300]), 6),
                     players in prop::array::uniform2(0u32..6), count in prop_oneof![Just(1u32), Just(2u32), Just(3u32), any::<u32>()], alias: bool) {
        let mut s = state(seed);
        let o = if alias { (0x800D_5AA8 + 4 * u32::from(racers[0].0 & 7)) - 0x1BC } else { O };
        wr(&mut s, o + 8, fl);
        wr(&mut s, o + 0x1BC, n as u32);
        wr(&mut s, 0x8011_B1B8, RACERS);
        for (i, &(k, skip, h)) in racers.iter().enumerate() {
            // With the alias, racer 0 is skipped (h = 0): its second call
            // would write h over the count and run past the six racers.
            let h = if alias && i == 0 { 0 } else { h };
            let k = if alias && i == 0 { k & 7 } else { k };
            let (r, e) = (RACERS + 0x88 * i as u32, OBJS + 0x100 * i as u32);
            wr(&mut s, r + 0x84, e);
            wh(&mut s, e + 4, k);
            wh(&mut s, r + 0x5C, h as u16);
            let (a, b) = (word(&s, e + 0x60), word(&s, e + 0x64));
            wr(&mut s, e + 0x60, if skip == 1 { a | 0x1000 } else { a & !0x5000 });
            wr(&mut s, e + 0x64, if skip == 2 { b | 1 << 25 } else { b & !(1 << 25) });
        }
        wr(&mut s, 0x8011_B1BC, RACERS + 0x88 * players[0]);
        wr(&mut s, 0x8011_B1C0, RACERS + 0x88 * players[1]);
        wr(&mut s, 0x800A_52BC, count);
        s.ctx.gpr[A0] = sext(o);
        let w = markers_model(&s, o);
        let after = run("func_8005C210", misc::func_8005C210, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- the light rig and points ----

fn d_save(w: &mut State, at: u32, v: u64) {
    wr(w, at, (v >> 32) as u32);
    wr(w, at + 4, v as u32);
}

fn rig_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x40;
    d_save(&mut w, sp + 0x20, s.ctx.fpr[20].u64);
    saves(&mut w, s, 0x40, &[(0x2C, S0), (0x30, S1), (0x34, RA)]);
    let (i, base) = (s.ctx.gpr[A0], s.ctx.gpr[A1]);
    let sv = f32::from_bits(s.ctx.gpr[A2] as u32);
    let (k1, k2) = (rf(&w, 0x800A_D008), rf(&w, 0x800A_D00C));
    let q = mul(sv, 0.75)?;
    let rows: [(u64, u32, u32, f32, [u32; 3]); 8] = [
        (0, 0, 0x3F80_0000, mul(sv, k1)?, [0xFF, 0xFF, 0xC8]),
        (1, 1, 0x3F40_0000, sv, [0xFF, 0xFF, 0xC8]),
        (2, 2, 0x3F21_47AE, q, [0xB4, 0xFF, 0xB4]),
        (3, 3, 0x3EDC_28F6, add(sv, sv)?, [0xFF, 0xFF, 0xC8]),
        (4, 4, 0x3E80_0000, mul(sv, k2)?, [0xFF, 0xC8, 0xC8]),
        (4, 5, 0xBE38_51EC, q, [0xFF, 0xC8, 0xC8]),
        (5, 6, 0xBEDC_28F6, sv, [0xB4, 0xFF, 0xB4]),
        (6, 7, 0xBFAC_CCCD, mul(sv, 0.5)?, [0xFF, 0xB4, 0xB4]),
    ];
    // The registers the port holds at the calls: s0 = i, s1 = w, f20 = s.
    (w.ctx.gpr[S0], w.ctx.gpr[S1]) = (i, base);
    w.ctx.fpr[20].set_u32l(sv.to_bits());
    for (row, &(j, n, x, y, rgb)) in rows.iter().enumerate() {
        if row == 2 {
            wf(&mut w, sp + 0x3C, q);
        }
        wf(&mut w, sp + 0x10, y);
        for (c, &v) in rgb.iter().enumerate() {
            wr(&mut w, sp + 0x14 + 4 * c as u32, v);
        }
        let wn = if n == 0 { base } else { sext((base as u32).wrapping_add(n)) };
        call_c(&mut w, 0x40, &[(A0, i), (A1, j), (A2, wn), (A3, sext(x))], imports::func_8000FD74);
    }
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_8005D10C(seed: u64, i in prop_oneof![0u64..2, Just(2u64), Just(u64::MAX), any::<u64>()], base: u32, raw in prop::option::of(any::<u64>()), sv in prop_oneof![4 => ordinary(), 1 => Just(f32::INFINITY), 1 => Just(f32::NAN)],
                     k in prop::array::uniform2(prop_oneof![Just(0.8f32), Just(1.4f32), ordinary()])) {
        let mut s = state(seed);
        wf(&mut s, 0x800A_D008, k[0]);
        wf(&mut s, 0x800A_D00C, k[1]);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (i, raw.unwrap_or(sext(base)), sext(sv.to_bits()));
        let model = rig_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_8005D10C", misc::func_8005D10C, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

const LSTR: u32 = 0x4C53_7472;
const DATA: u32 = 0x4461_7461;
const LIST: u32 = 0x8030_0000;

fn points_model(s: &State, p: u32) -> State {
    let mut w = s.clone();
    let sp = SP_AT - 0x50;
    saves(&mut w, s, 0x50, &[(0x14, S0), (0x18, S1), (0x1C, S2), (0x20, S3), (0x24, S4), (0x28, S5), (0x2C, RA)]);
    let mut a = p;
    while word(&w, a) != u32::MAX {
        a += 4;
    }
    if word(&w, a + 4) != DATA {
        return w;
    }
    let n = word(&w, a + 8) as i32;
    let mut at = a + 12;
    for k in 0..n.max(0) as u32 {
        if word(&w, at) == LSTR {
            for c in 0..3 {
                let v = word(&w, at + 4 + 4 * c);
                wr(&mut w, sp + 0x34 + 4 * c, v);
            }
            (w.ctx.gpr[S0], w.ctx.gpr[S1], w.ctx.gpr[S2], w.ctx.gpr[S3], w.ctx.gpr[S4], w.ctx.gpr[S5]) = (sext(at), u64::from(k), sext(at + 0x10), sext(LSTR), sext(sp + 0x34), sext(n as u32));
            call_c(&mut w, 0x50, &[(A0, u64::from(k)), (A1, sext(sp + 0x34))], imports::func_8000FF54);
            at += 0x10;
        } else {
            at += 4;
        }
    }
    w
}

/// A list entry: a light point or one other word.
fn entry() -> BoxedStrategy<Option<[f32; 3]>> {
    prop_oneof![3 => prop::array::uniform3(ordinary()).prop_map(Some), 1 => Just(None)].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Words before the -1 (or `p` on it), the "Data" tag or another, a
    /// count that is 0, negative, fewer or more than the entries written.
    #[test]
    fn func_8005EE18(seed: u64, skip in 0u32..4, tag in prop_oneof![3 => Just(DATA), 1 => any::<u32>()], n in prop_oneof![0i32..12, Just(-1i32)], entries in prop::collection::vec(entry(), 0..10)) {
        let mut s = state(seed);
        for k in 0..skip {
            wr(&mut s, LIST + 4 * k, k.wrapping_mul(0x9E37_79B9) & 0x7FFF_FFFF);
        }
        let mut at = LIST + 4 * skip;
        wr(&mut s, at, u32::MAX);
        wr(&mut s, at + 4, tag);
        wr(&mut s, at + 8, n as u32);
        at += 12;
        for e in &entries {
            match e {
                Some(v) => {
                    wr(&mut s, at, LSTR);
                    for (c, &x) in v.iter().enumerate() {
                        wf(&mut s, at + 4 + 4 * c as u32, x);
                    }
                    at += 16;
                }
                None => {
                    wr(&mut s, at, 0x1234_5678);
                    at += 4;
                }
            }
        }
        // Past the entries: more words that are neither -1 nor LStr.
        for k in 0..16 {
            wr(&mut s, at + 4 * k, 0x0BAD_0000 | k);
        }
        s.ctx.gpr[A0] = sext(LIST);
        let w = points_model(&s, LIST);
        let after = run("func_8005EE18", misc::func_8005EE18, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- objects and points ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Distances around 10: `(6, 8, 0)` is exactly 10.
    #[test]
    fn func_800636D0(seed: u64, mode in prop_oneof![Just(2u32), some_word()], a in prop::array::uniform3(coord()), b in prop::array::uniform3(coord()),
                     ten in prop_oneof![Just(None), Just(Some([6.0f32, 8.0, 0.0])), Just(Some([10.0f32, 0.0, 0.0])), Just(Some([0.0f32, 10.0f32.next_up(), 0.0]))]) {
        let mut s = state(seed);
        wr(&mut s, O + 8, mode);
        let b = ten.unwrap_or(b);
        let a = if ten.is_some() { [0.0; 3] } else { a };
        for c in 0..3 {
            wf(&mut s, O + 0x44 + 4 * c as u32, a[c]);
            wf(&mut s, O + 0x50 + 4 * c as u32, b[c]);
        }
        s.ctx.gpr[A0] = sext(O);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA), (0x18, A0)]);
        if mode == 2 {
            let v = word(&w, O + 0x4C);
            wr(&mut w, O + 0x58, v);
        }
        call_c(&mut w, 0x18, &[(A0, sext(O + 0x50)), (A1, sext(O + 0x44))], imports::func_80015470);
        let v0 = u32::from(w.ctx.fpr[0].fl() <= 10.0);
        wr(&mut w, O + 0xA0, v0);
        let after = run("func_800636D0", misc::func_800636D0, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(v0));
    }

    #[test]
    fn func_80063E08(seed: u64, k in prop_oneof![0u32..32, (-8i32..0).prop_map(|k| k as u32)]) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(O), sext(k));
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA), (0x18, A0)]);
        call_c(&mut w, 0x18, &[(A0, sext(O + 0x50)), (A1, sext(0x800A_5100u32.wrapping_add(k.wrapping_mul(12))))], imports::func_80015288);
        wr(&mut w, O + 0xA0, 0);
        let after = run("func_80063E08", misc::func_80063E08, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80064A48(seed: u64, k in 0u32..8) {
        let mut s = state(seed);
        let (n, list) = (0x8030_1000u32, 0x8030_2000u32);
        wr(&mut s, n + 0x30, list);
        for j in 0..8 {
            wr(&mut s, list + 4 * j, 0x8030_4000 + 0x100 * j);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(O), sext(n), u64::from(k));
        let mut w = s.clone();
        let sp = SP_AT - 0x60;
        saves(&mut w, &s, 0x60, &[(0x14, RA), (0x60, A0)]);
        call_c(&mut w, 0x60, &[(A0, sext(0x8030_4000 + 0x100 * k)), (A1, sext(sp + 0x1C))], imports::func_80017C18);
        call_c(&mut w, 0x60, &[(A0, sext(O)), (A1, sext(sp + 0x4C))], imports::func_80015288);
        let after = run("func_80064A48", misc::func_80064A48, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80065C50(seed: u64, null: bool, v in 0u32..16) {
        let mut s = state(seed);
        let o = if null { 0 } else { O };
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(o), sext(0x8030_2000 + 4 * v));
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        if !null {
            call_c(&mut w, 0x18, &[(A0, sext(O + 0x50)), (A1, s.ctx.gpr[A1])], imports::func_80015288);
        }
        let after = run("func_80065C50", misc::func_80065C50, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80065C74(seed: u64, null: bool, v in 0u32..16) {
        let mut s = state(seed);
        let o = if null { 0 } else { O };
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(o), sext(0x8030_2000 + 4 * v));
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        if !null {
            call_c(&mut w, 0x18, &[(A0, sext(O + 0x74)), (A1, s.ctx.gpr[A1])], imports::func_80015288);
        }
        let after = run("func_80065C74", misc::func_80065C74, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- turning toward the target ----

const DT: u32 = 0x8012_0BF8;

/// The heading the callees compute for `o`'s two points (their C on a
/// copy), and the state after them.
fn heading(s: &State) -> Option<(State, f32)> {
    let mut w = s.clone();
    let sp = SP_AT - 0x30;
    saves(&mut w, s, 0x30, &[(0x14, RA), (0x30, A0)]);
    call_c(&mut w, 0x30, &[(A0, sext(sp + 0x24)), (A1, sext(O + 0x50)), (A2, sext(O + 0x44))], imports::func_8001535C);
    wf(&mut w, sp + 0x2C, 0.0);
    call_c(&mut w, 0x30, &[(A0, sext(sp + 0x24))], imports::func_800153C0);
    let (dx, dy) = (ok(rf(&w, sp + 0x24))?, rf(&w, sp + 0x28));
    w.ctx.fpr[12].set_fl(-dx);
    w.ctx.fpr[14].set_fl(dy);
    call_c(&mut w, 0x30, &[], imports::func_80014F54);
    let a = w.ctx.fpr[0].fl();
    Some((w, a))
}

fn turn_model(s: &State) -> Option<State> {
    let (mut w, a) = heading(s)?;
    wf(&mut w, O + 0x6C, a);
    let h = rf(&w, O + 0x68);
    let mut e = sub(a, h)?;
    if e < -180.0 {
        e = add(e, 360.0)?;
    }
    if 180.0 < e {
        e = sub(e, 360.0)?;
    }
    if !(5.0 < e) && !(e < -5.0) {
        let v = word(&w, O + 0x6C);
        wr(&mut w, O + 0x68, v);
    } else {
        let p = mul(90.0, rf(&w, DT))?;
        let v = if e < 0.0 { sub(h, p)? } else { add(h, p)? };
        wf(&mut w, O + 0x68, v);
    }
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// `h` set from the computed heading `a` so that `a - h` is one of the
    /// boundaries (±5, ±180, 0, ±0) or near them, or anything.
    #[test]
    fn func_80063EF4(seed: u64, a in prop::array::uniform3(coord()), b in prop::array::uniform3(coord()),
                     e0 in prop_oneof![Just(5.0f32), Just(-5.0f32), Just(180.0f32), Just(-180.0f32), Just(0.0f32), Just(-0.0f32), Just(5.0f32.next_up()), Just((-5.0f32).next_down()),
                                       Just(180.0f32.next_up()), Just((-180.0f32).next_down()), -400.0f32..400.0],
                     free in prop::option::of(ordinary()), dt in prop_oneof![Just(1.0f32 / 60.0), 0.0f32..0.1, ordinary()]) {
        let mut s = state(seed);
        load_data(&mut s);
        for c in 0..3 {
            wf(&mut s, O + 0x44 + 4 * c as u32, a[c]);
            wf(&mut s, O + 0x50 + 4 * c as u32, b[c]);
        }
        wf(&mut s, DT, dt);
        s.ctx.gpr[A0] = sext(O);
        let got = heading(&s);
        prop_assume!(got.is_some());
        let a = got.unwrap().1;
        let h = free.unwrap_or_else(|| {
            // Step h by ulps until a - h is e0 exactly (when it can be).
            let mut h = a - e0;
            for _ in 0..16 {
                let d = a - h;
                if d == e0 || !d.is_finite() {
                    break;
                }
                h = if d < e0 { h.next_down() } else { h.next_up() };
            }
            h
        });
        wf(&mut s, O + 0x68, h);
        let model = turn_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_80063EF4", misc::func_80063EF4, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- the contact points ----

const TABLE: u32 = 0x800A_5CA0;
const K: u32 = 0x800A_D48C;
const NRM: u32 = 0x8030_2000;
const PT: u32 = 0x8030_2100;
const SREGS: [usize; 9] = [S0, S1, S2, S3, S4, S5, S6, S7, FP];

/// What the port holds at a call: s0..s7, fp, then f20, f22, f24, f26.
struct Held {
    s: [u64; 9],
    f: [u64; 4],
}

impl Held {
    fn call(&self, w: &mut State, args: &[(usize, u64)], f: RecompFn) {
        for (k, &r) in SREGS.iter().enumerate() {
            w.ctx.gpr[r] = self.s[k];
        }
        for (k, r) in [20, 22, 24, 26].into_iter().enumerate() {
            w.ctx.fpr[r].u64 = self.f[k];
        }
        call_c(w, 0x1F8, args, f);
    }
}

/// `v` with its low word replaced (`mtc1`/`lwc1` of an even FPR).
fn low(v: u64, bits: u32) -> u64 {
    v & !0xFFFF_FFFF | u64::from(bits)
}

fn div(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? / ok(b)?)
}

fn contact_model(s: &State) -> Option<(State, u32)> {
    let mut w = s.clone();
    let sp = SP_AT - 0x1F8;
    let (o, p, r_bits) = (s.ctx.gpr[A0] as u32, s.ctx.gpr[A1] as u32, s.ctx.gpr[A3] as u32);
    let r = f32::from_bits(r_bits);
    d_save(&mut w, sp + 0x38, s.ctx.fpr[26].u64);
    saves(&mut w, s, 0x1F8, &[(0x64, RA), (0x60, FP), (0x5C, S7), (0x58, S6), (0x54, S5), (0x50, S4), (0x4C, S3), (0x48, S2), (0x44, S1), (0x40, S0)]);
    d_save(&mut w, sp + 0x30, s.ctx.fpr[24].u64);
    d_save(&mut w, sp + 0x28, s.ctx.fpr[22].u64);
    d_save(&mut w, sp + 0x20, s.ctx.fpr[20].u64);
    saves(&mut w, s, 0x1F8, &[(0x1F8, A0), (0x1FC, A1), (0x200, A2)]);
    let mut h = Held { s: SREGS.map(|r| s.ctx.gpr[r]), f: [s.ctx.fpr[20].u64, s.ctx.fpr[22].u64, low(s.ctx.fpr[24].u64, r_bits), low(s.ctx.fpr[26].u64, 0)] };
    // The offsets: (0, 0, 0), T[1], T[1] with x negated, T[0].
    let k = word(&w, word(&w, word(&w, o + 0x1E70) + 0x18));
    let t = TABLE.wrapping_add(k.wrapping_mul(0x6C));
    (h.s[0], h.s[3]) = (sext(k), sext(sp + 0x14C));
    h.call(&mut w, &[(A0, sext(sp + 0x14C)), (A1, 0), (A2, 0), (A3, 0)], imports::func_80015268);
    (h.s[1], h.s[2], h.s[7]) = (sext(t), sext(t + 0xC), 2);
    h.call(&mut w, &[(A0, sext(sp + 0x158)), (A1, sext(t + 0xC))], imports::func_80015288);
    h.s[0] = sext(sp + 0x164);
    h.call(&mut w, &[(A0, sext(sp + 0x164)), (A1, sext(t + 0xC))], imports::func_80015288);
    let x = -ok(rf(&w, sp + 0x164))?;
    wf(&mut w, sp + 0x164, x);
    (h.s[0], h.s[7]) = (sext(sp + 0x17C), 4);
    h.call(&mut w, &[(A0, sext(sp + 0x170)), (A1, sext(t))], imports::func_80015288);
    // The four points.
    let (l2, l1) = (word(&w, K), word(&w, K + 4));
    (h.f[0], h.f[1]) = (low(h.f[0], l1), low(h.f[1], l2));
    let n = word(&w, SP_AT + 0x10);
    wr(&mut w, sp + 0x78, o + 0x40);
    wr(&mut w, sp + 0x7C, o + 0x30);
    (h.s[1], h.s[3], h.s[8]) = (sext(n), sext(sp + 0x90), sext(o + 0x20));
    for i in 0..4u32 {
        let c = sp + 0x14C + 12 * i;
        let wi = sp + 0xE4 + 12 * i;
        (h.s[0], h.s[2], h.s[4], h.s[5], h.s[6]) = (sext(sp + 0x9C + 12 * i), sext(wi), sext(c), u64::from(i), sext(sp + 0x12C + 4 * i));
        let (cx, cy, cz) = (word(&w, c), word(&w, c + 4), word(&w, c + 8));
        h.call(&mut w, &[(A0, sext(sp + 0x90)), (A1, sext(cx)), (A2, sext(o + 0x20))], imports::func_800155C0);
        h.call(&mut w, &[(A0, sext(sp + 0x90)), (A1, sext(sp + 0x90)), (A2, sext(cy)), (A3, sext(o + 0x30))], imports::func_800155EC);
        h.call(&mut w, &[(A0, sext(sp + 0x90)), (A1, sext(sp + 0x90)), (A2, sext(cz)), (A3, sext(o + 0x40))], imports::func_800155EC);
        h.call(&mut w, &[(A0, sext(wi)), (A1, sext(p)), (A2, sext(sp + 0x90))], imports::func_80015328);
        let nz = rf(&w, n + 8);
        let d = if nz < f32::from_bits(l1) && f32::from_bits(l2) < nz {
            let z = sub(rf(&w, wi + 8), r)?;
            wf(&mut w, wi + 8, z);
            r
        } else {
            let (nx, ny) = (rf(&w, n), rf(&w, n + 4));
            let (tx, ty, tz) = (rf(&w, sp + 0x90), rf(&w, sp + 0x94), rf(&w, sp + 0x98));
            let e = sub(div(sub(mul(-ok(nx)?, tx)?, mul(ny, ty)?)?, nz)?, tz)?;
            let z = add(rf(&w, wi + 8), e)?;
            wf(&mut w, wi + 8, z);
            let z = sub(z, r)?;
            wf(&mut w, wi + 8, z);
            sub(r, e)?
        };
        for q in 0..3 {
            let v = word(&w, n + 4 * q);
            wr(&mut w, sp + 0x9C + 12 * i + 4 * q, v);
        }
        wf(&mut w, sp + 0x12C + 4 * i, d);
    }
    // The four matrices, if (f32(count) - 40) / 60 < 1 as doubles.
    let lim = f64::from(sub(word(&w, o + 0x1998) as i32 as f32, 40.0)? / 60.0);
    if !(lim < 1.0) {
        let v = word(&w, K + 0x10);
        for i in 0..4 {
            wr(&mut w, o + 0x12C8 + 0x40 * i, v);
        }
    } else {
        let (far, scale) = (word(&w, K + 8), word(&w, K + 0xC));
        (h.f[0], h.f[1]) = (low(h.f[0], scale), low(h.f[1], far));
        (h.s[3], h.s[7], h.s[8]) = (sext(sp + 0x1B8), sext(sp + 0x1C8), sext(sp + 0x1D8));
        for i in 0..4u32 {
            let wi = sp + 0xE4 + 12 * i;
            (h.s[2], h.s[4], h.s[5], h.s[6]) = (sext(wi), sext(o + 0x40 * i), u64::from(i), sext(sp + 0x12C + 4 * i));
            if !(0.0 < rf(&w, sp + 0x12C + 4 * i)) {
                wr(&mut w, o + 0x12C8 + 0x40 * i, far);
                continue;
            }
            let (ni, mi) = (sp + 0x9C + 12 * i, o + 0x1290 + 0x40 * i);
            (h.s[0], h.s[1]) = (sext(ni), sext(mi));
            h.call(&mut w, &[(A0, sext(sp + 0x1B8))], imports::func_80017874);
            for q in 0..3 {
                let v = word(&w, ni + 4 * q);
                wr(&mut w, sp + 0x1D8 + 4 * q, v);
            }
            for q in 0..3 {
                let v = word(&w, o + 0x30 + 4 * q);
                wr(&mut w, sp + 0x1C8 + 4 * q, v);
            }
            h.call(&mut w, &[(A0, sext(sp + 0x1B8)), (A1, sext(sp + 0x1C8)), (A2, sext(sp + 0x1D8))], imports::func_80015538);
            h.call(&mut w, &[(A0, sext(sp + 0x1C8)), (A1, sext(sp + 0x1D8)), (A2, sext(sp + 0x1B8))], imports::func_80015538);
            for q in 0..3 {
                let v = word(&w, wi + 4 * q);
                wr(&mut w, sp + 0x1E8 + 4 * q, v);
            }
            h.call(&mut w, &[(A0, sext(mi)), (A1, sext(sp + 0x1B8))], imports::func_800156DC);
            wr(&mut w, sp + 0x10, mi);
            h.call(&mut w, &[(A0, sext(mi)), (A1, sext(scale)), (A2, sext(scale)), (A3, sext(scale))], imports::func_80017918);
        }
    }
    // The result.
    let (d1, d2) = (rf(&w, sp + 0x130), rf(&w, sp + 0x134));
    let m = if 0.0 < d2 && d2 < d1 { d2 } else { d1 };
    let f0 = if !(m < 0.0) {
        sub(m, 2.0)?.to_bits()
    } else if !(r < 0.0) {
        r_bits
    } else {
        word(&w, K + 0x14)
    };
    Some((w, f0))
}

/// A normal's z: inside, on and just outside ±0.05, 0, or anything.
fn nz() -> BoxedStrategy<f32> {
    let q = f32::from_bits(0x3D4C_CCCD);
    prop_oneof![Just(0.0f32), Just(-0.0f32), Just(q), Just(-q), Just(q.next_down()), Just((-q).next_up()), Just(q.next_up()), -1.0f32..1.0, coord()].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Table entries 0..3, bounded rows, points and normals, `r` at 0, -0 and
    /// negative, the count around 100 (the matrices' test), and the ROM's
    /// constants or perturbed ones.
    #[test]
    fn func_80066144(seed: u64, k in 0u32..4, offs in prop::array::uniform6(coord()), rows in prop::array::uniform9(coord()), pv in prop::array::uniform3(coord()),
                     n in (coord(), coord(), nz()), r in prop_oneof![Just(0.0f32), Just(-0.0f32), Just(-1.0f32), coord()],
                     count in prop_oneof![Just(99u32), Just(100u32), Just(101u32), 0u32..200, any::<u32>()], rom: bool, ks in prop::array::uniform6(ordinary())) {
        let mut s = state(seed);
        wr(&mut s, O + 0x1E70, 0x8030_3000);
        wr(&mut s, 0x8030_3018, 0x8030_3100);
        wr(&mut s, 0x8030_3100, k);
        let t = TABLE + 0x6C * k;
        for (q, &v) in offs.iter().enumerate() {
            wf(&mut s, t + 4 * q as u32, v);
        }
        for (q, &v) in rows.iter().enumerate() {
            wf(&mut s, O + 0x20 + 0x10 * (q as u32 / 3) + 4 * (q as u32 % 3), v);
        }
        for (q, &v) in pv.iter().enumerate() {
            wf(&mut s, PT + 4 * q as u32, v);
        }
        wf(&mut s, NRM, n.0);
        wf(&mut s, NRM + 4, n.1);
        wf(&mut s, NRM + 8, n.2);
        wr(&mut s, SP_AT + 0x10, NRM);
        wr(&mut s, O + 0x1998, count);
        let rom_k = [0xBD4C_CCCDu32, 0x3D4C_CCCD, 0xC7C3_5000, 0x3B83_126F, 0xC7C3_5000, 0x47C3_5000];
        for q in 0..6 {
            wr(&mut s, K + 4 * q as u32, if rom { rom_k[q] } else { ks[q].to_bits() });
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A3]) = (sext(O), sext(PT), sext(r.to_bits()));
        let model = contact_model(&s);
        prop_assume!(model.is_some());
        let (w, f0) = model.unwrap();
        let after = run("func_80066144", misc::func_80066144, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), f0);
    }
}

/// The racer base re-read after each racer: marker 23722's triple
/// (`0x800D59B8 + 12k`) covers `0x8011B1B0..0x8011B1BB`, so racer 0's `z`
/// becomes the base `[0x8011B1B8]` and racer 1 is read from there.
#[test]
fn func_8005C210_base_moves() {
    for seed in 0..4u64 {
        let mut s = state(seed);
        let moved = 0x8033_0000u32;
        wr(&mut s, O + 8, 1);
        wr(&mut s, O + 0x1BC, 3);
        wr(&mut s, 0x8011_B1B8, RACERS);
        for (i, base) in [(0u32, RACERS), (1, moved), (2, moved)] {
            let (r, e) = (base + 0x88 * i, OBJS + 0x100 * i + 0x100 * (base == moved) as u32 * 8);
            wr(&mut s, r + 0x84, e);
            wh(&mut s, e + 4, if i == 0 { 23722 } else { i as u16 });
            wh(&mut s, r + 0x5C, 10 + i as u16);
            let (a, b) = (word(&s, e + 0x60), word(&s, e + 0x64));
            wr(&mut s, e + 0x60, a & !0x5000);
            wr(&mut s, e + 0x64, b & !(1 << 25));
            if i == 0 {
                wr(&mut s, e + 0x58, moved);
            }
        }
        wr(&mut s, 0x8011_B1BC, 0);
        wr(&mut s, 0x8011_B1C0, 0);
        s.ctx.gpr[A0] = sext(O);
        let w = markers_model(&s, O);
        assert_eq!(word(&w, 0x8011_B1B8), moved);
        let after = compare("func_8005C210", misc::func_8005C210, &s).unwrap_or_else(|d| panic!("{d}"));
        same_memory(&after, &w).unwrap();
    }
}

/// `0 < d2` at `d2 = 0` with `d1 > 0`: rows `R[0] = (1, 0, 1)`, `R[1] =
/// (0, 1, 0)`, `R[2] = 0` make `t.z = c.x`, the normal `(0, 0, 1)` makes `e
/// = -t.z`, so `d = r + c.x`: `2 T1.x` for the second point and `0` for the
/// third (its `x` negated) with `r = T1.x`.
#[test]
fn func_80066144_ties() {
    for (seed, (tx, r)) in [(3.0f32, 3.0f32), (5.0, 5.0), (3.0, -0.0)].into_iter().enumerate() {
        let mut s = state(seed as u64);
        wr(&mut s, O + 0x1E70, 0x8030_3000);
        wr(&mut s, 0x8030_3018, 0x8030_3100);
        wr(&mut s, 0x8030_3100, 0);
        for (q, v) in [1.0f32, 2.0, 0.5, tx, 1.5, 0.25].into_iter().enumerate() {
            wf(&mut s, TABLE + 4 * q as u32, v);
        }
        for (q, v) in [1.0f32, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0].into_iter().enumerate() {
            wf(&mut s, O + 0x20 + 0x10 * (q as u32 / 3) + 4 * (q as u32 % 3), v);
        }
        for q in 0..3 {
            wf(&mut s, PT + 4 * q, 0.0);
        }
        for (q, v) in [0.0f32, 0.0, 1.0].into_iter().enumerate() {
            wf(&mut s, NRM + 4 * q as u32, v);
        }
        wr(&mut s, SP_AT + 0x10, NRM);
        wr(&mut s, O + 0x1998, 200);
        for (q, v) in [0xBD4C_CCCDu32, 0x3D4C_CCCD, 0xC7C3_5000, 0x3B83_126F, 0xC7C3_5000, 0x47C3_5000].into_iter().enumerate() {
            wr(&mut s, K + 4 * q as u32, v);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A3]) = (sext(O), sext(PT), sext(r.to_bits()));
        let (w, f0) = contact_model(&s).unwrap();
        let sp = SP_AT - 0x1F8;
        assert_eq!(rf(&w, sp + 0x134), r - tx);
        let after = compare("func_80066144", misc::func_80066144, &s).unwrap_or_else(|d| panic!("{d}"));
        same_memory(&after, &w).unwrap();
        assert_eq!(after.ctx.fpr[0].u32l(), f0);
    }
}
