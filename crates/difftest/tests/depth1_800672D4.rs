//! Depth 1 at 0x800672D4..0x80068410 (game::misc): a pod's engine step and
//! its speed limit. Recompiled C vs Rust with the callees as C. The models
//! replay the callees' C on a copy of the state in the same order with the
//! same arguments, adding the functions' own stores; whole RDRAM (and the
//! result register) is compared. The data segment is loaded from the ROM
//! (the callee `func_80081700` reads a constant there, the functions their
//! thresholds).

// Tests are named after the functions (func_800672D4), capitals included.
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

const SP_AT: u32 = 0x803F_0000;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    load_data(&mut s);
    s
}

/// The game's data segment (`0x80098000..0x800AE8B0`) as the ROM image has
/// it (the callee's constant and these thresholds live there).
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

fn wd(s: &mut State, a: u32, v: f64) {
    let b = v.to_bits();
    wr(s, a, (b >> 32) as u32);
    wr(s, a + 4, b as u32);
}

fn rf(s: &State, a: u32) -> f32 {
    f32::from_bits(word(s, a))
}

fn rd(s: &State, a: u32) -> f64 {
    f64::from_bits(u64::from(word(s, a)) << 32 | u64::from(word(s, a + 4)))
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
fn okd(x: f64) -> Option<f64> {
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
/// `cvt.s.d` (guarded).
fn narrow(x: f64) -> Option<f32> {
    Some(okd(x)? as f32)
}
/// `cvt.d.s` (guarded).
fn widen(x: f32) -> Option<f64> {
    Some(f64::from(ok(x)?))
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
const V: u32 = 0x8030_2000;
const DT: u32 = 0x8012_0BF0;
const POOL_LIST: u32 = 0x8031_0000;
const POOL_DESC: u32 = 0x8031_0100;
/// func_8005F31C, a verified callback: `[elem + 0x14] += 1`, `[elem + 0x18]
/// = arg` (see indirect.rs).
const MARK: u32 = 0x8005_F31C;

fn engine_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x80;
    let sb = s.ctx.gpr[A2] as u32;
    let sv = f32::from_bits(sb);
    wr(&mut w, sp + 0x14, s.ctx.gpr[RA] as u32);
    wr(&mut w, SP_AT + 4, V);
    wr(&mut w, SP_AT + 8, sb);
    // d: the reversed velocity or the heading.
    let rev = word(&w, O + 0x64) & 0x400 != 0;
    for k in 0..3 {
        let d = if rev { -ok(rf(&w, O + 0x160 + 4 * k))? } else { rf(&w, O + 0x194 + 4 * k) };
        wf(&mut w, sp + 0x28 + 4 * k, d);
    }
    let lo = rf(&w, O + 0xA4);
    let m = sub(rf(&w, O + 0x94), lo)?;
    let e0 = sub(sv, lo)?;
    wf(&mut w, sp + 0x7C, m);
    let x = rf(&w, O + 0x28);
    let a = if x < 0.0 { -ok(x)? } else { x };
    let q = mul(a, rf(&w, O + 0xA8))?;
    let e = if 3.0 < q { sub(e0, sub(q, 3.0)?)? } else { e0 };
    let dt = okd(rd(&w, DT))?;
    let dtf = narrow(dt)?;
    // 1: the boost timer and bit 9.
    if rf(&w, 0x800A_D4C4) < sv {
        let t = add(rf(&w, O + 0x2C8), dtf)?;
        wf(&mut w, O + 0x2C8, t);
        if 3.0 < rf(&w, O + 0x2C8) {
            let f = word(&w, O + 0x60);
            wr(&mut w, O + 0x60, f | 0x1000);
        }
    } else {
        wf(&mut w, O + 0x2C8, 0.0);
    }
    let f = word(&w, O + 0x64);
    let fast = 30.0 < rf(&w, SP_AT + 8);
    wr(&mut w, O + 0x64, if fast { f | 0x200 } else { f & !0x200 });
    // 2: the thrust.
    if !(12.0 < e) {
        let r = div(sub(12.0, e)?, sub(12.0, rf(&w, sp + 0x7C))?)?;
        let p = add(rf(&w, O + 0x1B4), mul(dtf, sub(1.0, r)?)?)?;
        wf(&mut w, O + 0x1B4, p);
        if rf(&w, sp + 0x7C) < e && rf(&w, O + 0x1B4) < 0.0 {
            wr(&mut w, SP_AT, O);
            wr(&mut w, SP_AT + 4, V);
            wf(&mut w, sp + 0x70, e);
            w.ctx.fpr[12].set_fl(4.0);
            w.ctx.fpr[14].set_fl(dtf);
            call_c(&mut w, 0x80, &[], imports::func_80081700);
            let p = mul(rf(&w, O + 0x1B4), w.ctx.fpr[0].fl())?;
            wf(&mut w, O + 0x1B4, p);
        }
    } else {
        let step = if !(0.0 <= rf(&w, O + 0x1A0)) { 2.0 * dt } else { dt };
        let p = narrow(widen(rf(&w, O + 0x1B4))? + step)?;
        wf(&mut w, O + 0x1B4, p);
    }
    // 3: the force.
    let ff = mul(mul(mul(dtf, 30.0)?, rf(&w, O + 0x190))?, rf(&w, O + 0x1B4))?;
    wf(&mut w, O + 0x1B0, ff);
    if rf(&w, O + 0x2FC) < 0.0 && 0.0 <= rf(&w, O + 0x1A0) && 0.0 < rf(&w, O + 0x1B0) {
        let ff = mul(rf(&w, O + 0x1B0), add(mul(rf(&w, O + 0x2FC), rf(&w, 0x800A_D4C8))?, 1.0)?)?;
        wf(&mut w, O + 0x1B0, ff);
    }
    // 4: the hit.
    let ff = rf(&w, O + 0x1B0);
    if e < ff {
        let p = rf(&w, O + 0x1B4);
        let q2 = narrow(widen(ff)? / dt)?;
        wf(&mut w, O + 0x1B0, e);
        let p8 = mul(p, 8.0)?;
        if 0.0 < p {
            let np = -ok(div(p, 5.0)?)?;
            wf(&mut w, O + 0x1B4, np);
        }
        if 4.0 < p8 && word(&w, O + 0x60) & 1 << 24 == 0 {
            wr(&mut w, sp + 0x40, 0x4869_7474);
            wr(&mut w, sp + 0x44, 0x426F_746D);
            wf(&mut w, sp + 0x48, mul(q2, 0.5)?);
            wr(&mut w, SP_AT, O);
            wr(&mut w, SP_AT + 4, V);
            call_c(&mut w, 0x80, &[(A0, sext(O)), (A1, sext(sp + 0x40))], imports::func_8003F99C);
        }
        let f = word(&w, O + 0x60);
        wr(&mut w, O + 0x60, f | 0x100_0000);
    } else {
        let f = word(&w, O + 0x60);
        wr(&mut w, O + 0x60, f & 0xFEFF_FFFF);
    }
    // 5: v += d F.
    for k in 0..3 {
        let v = add(mul(rf(&w, sp + 0x28 + 4 * k), rf(&w, O + 0x1B0))?, rf(&w, V + 4 * k))?;
        wf(&mut w, V + 4 * k, v);
    }
    Some(w)
}

/// A speed near a boundary: the engine's `e` compares against 12, `M` and
/// `F`, `s` against 30 and `K`.
fn speed() -> BoxedStrategy<f32> {
    prop_oneof![Just(30.0f32), Just(30.0f32.next_up()), Just(12.0f32), 0.0f32..40.0, ordinary()].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Both flag bits, speeds around the thresholds (with `lo = 0` often,
    /// so `e = s`), `K` at the ROM's 99999 or below `s`, the timer around 3,
    /// thrusts of both signs, negative `[o + 0x2FC]` and `[o + 0x1A0]`, and
    /// `o` in a pool whose callback records the hit message.
    #[test]
    fn func_800672D4(seed: u64, flags in prop::array::uniform3(any::<bool>()), fl: u32, sv in speed(), lo in prop_oneof![Just(0.0f32), ordinary()], hi in prop_oneof![Just(10.0f32), Just(12.0f32), ordinary()],
                     turn in prop_oneof![Just(0.0f32), -3.0f32..3.0, ordinary()], k in prop_oneof![Just(None), prop_oneof![Just(10.0f32), ordinary()].prop_map(Some)],
                     timer in prop_oneof![Just(3.0f32), 2.9f32..3.1, ordinary()], vals in prop::array::uniform6(ordinary()), dirs in prop::array::uniform9(ordinary()),
                     thrust in prop_oneof![Just(0.5f32), Just(0.0f32), Just(1.0f32), 0.0f32..2.0, ordinary()], tie: bool,
                     d in prop_oneof![Just(1.0f64 / 60.0), Just(0.0f64), 0.0f64..0.1, -1.0f64..1.0]) {
        let mut s = state(seed);
        let f60 = (fl & !(1 << 24)) | u32::from(flags[0]) << 24;
        wr(&mut s, O + 0x60, f60);
        let f64_ = (fl.rotate_left(7) & !0x400) | u32::from(flags[1]) << 10;
        wr(&mut s, O + 0x64, f64_);
        wf(&mut s, O + 0xA4, lo);
        wf(&mut s, O + 0x94, hi);
        wf(&mut s, O + 0x28, turn);
        wf(&mut s, O + 0xA8, vals[0]);
        if let Some(k) = k {
            wf(&mut s, 0x800A_D4C4, k);
        }
        wf(&mut s, O + 0x2C8, timer);
        wf(&mut s, O + 0x1B4, thrust);
        wf(&mut s, O + 0x1A0, vals[2]);
        wf(&mut s, O + 0x190, vals[3]);
        wf(&mut s, O + 0x2FC, vals[4]);
        for c in 0..3 {
            wf(&mut s, O + 0x160 + 4 * c as u32, dirs[c]);
            wf(&mut s, O + 0x194 + 4 * c as u32, dirs[3 + c]);
            wf(&mut s, V + 4 * c as u32, dirs[6 + c]);
        }
        wd(&mut s, DT, d);
        // o in a pool with the MARK callback (or not, by bit 8 of +6).
        wr(&mut s, 0x800A_2170, POOL_LIST);
        wr(&mut s, POOL_LIST, POOL_DESC);
        wr(&mut s, POOL_LIST + 4, 0);
        wr(&mut s, POOL_DESC, 7);
        wr(&mut s, POOL_DESC + 0x24, MARK);
        wr(&mut s, O, 7);
        let h = half(&s, O + 6);
        wh(&mut s, O + 6, if flags[2] { h | 0x100 } else { h & !0x100 });
        // s = lo gives e = 0 (with q <= 3), the tie against F = 0 when dt = 0.
        let sv = if tie { lo } else { sv };
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(O), sext(V), sext(sv.to_bits()));
        let model = engine_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_800672D4", misc::func_800672D4, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- the speed limit ----

/// `r *= d(a)`, replaying func_80081700(a, dt) with `o` spilled.
fn decay(w: &mut State, a: f32, dtf: f32) -> Option<()> {
    // The callee's domain: `r = dt / K`, then `1 - r / (r + a)` (0 / 0 for
    // `dt = a = 0`).
    let rr = div(dtf, rf(w, 0x800A_DCB0))?;
    ok(div(rr, add(rr, ok(a)?)?)?)?;
    wr(w, SP_AT, O);
    w.ctx.fpr[12].set_fl(a);
    w.ctx.fpr[14].set_fl(dtf);
    call_c(w, 0x18, &[], imports::func_80081700);
    let r = mul(rf(w, O + 0x1A4), w.ctx.fpr[0].fl())?;
    wf(w, O + 0x1A4, r);
    Some(())
}

fn speed_model(s: &State) -> Option<(State, u32)> {
    let mut w = s.clone();
    wr(&mut w, SP_AT - 0x18 + 0x14, s.ctx.gpr[RA] as u32);
    let b = if word(&w, O + 0x60) & 1 << 23 != 0 || word(&w, O + 0x64) & 0x2000 != 0 { 4.0f32 } else { 1.5 };
    let second = word(&w, word(&w, O + 0x1E70) + 8) & 8 == 0;
    let k = if second { 0x800A_D500u32 } else { 0x800A_D4EC };
    let kk = |w: &State, i: u32| rf(w, k + 4 * i);
    let x = rf(&w, O + 0x18C);
    let dt = okd(rd(&w, DT))?;
    let dtf = narrow(dt)?;
    let r = |w: &State| rf(w, O + 0x1A4);
    if kk(&w, 0) < x {
        let v = add(r(&w), mul(mul(dtf, x)?, b)?)?;
        wf(&mut w, O + 0x1A4, v);
        let c = if kk(&w, 1) <= x { kk(&w, 2) } else { div(x, sub(1.0, x)?)? };
        if c < r(&w) {
            let a = rf(&w, O + 0x84);
            decay(&mut w, a, dtf)?;
        }
    } else if !(x < kk(&w, 3)) {
        if second && r(&w) < kk(&w, 5) {
            decay(&mut w, 10.0, dtf)?;
        } else {
            let a = rf(&w, O + 0x84);
            decay(&mut w, a, dtf)?;
        }
    } else {
        let v = add(r(&w), mul(mul(dtf, x)?, b)?)?;
        wf(&mut w, O + 0x1A4, v);
        if kk(&w, 4) < x && r(&w) < mul(0.5, x)? {
            decay(&mut w, 20.0, dtf)?;
        }
    }
    if word(&w, O + 0x60) & 0x200 != 0 {
        let a = rf(&w, O + 0x80);
        decay(&mut w, a, dtf)?;
    }
    let rv = r(&w);
    let (p, q) = (rf(&w, O + 0x7C), rf(&w, O + 0x78));
    let mut v = if 0.0 < rv {
        div(mul(p, rv)?, add(rv, q)?)?
    } else {
        let nr = -ok(rv)?;
        div(mul(-ok(p)?, nr)?, add(nr, q)?)?
    };
    v = mul(v, rf(&w, O + 0x1AC))?;
    let f64_ = word(&w, O + 0x64);
    if rf(&w, O + 0x184) < 15.0 {
        if f64_ & 1 << 27 == 0 {
            let t = f64_ | 0x800_0000;
            wr(&mut w, O + 0x64, t);
            if rf(&w, O + 0x244) < 1.0 {
                wr(&mut w, O + 0x64, t | 0x1000_0000);
            }
        }
        v = mul(v, rf(&w, O + 0x244))?;
    } else {
        wr(&mut w, O + 0x64, f64_ & 0xF7FF_FFFF);
    }
    let f60 = word(&w, O + 0x60);
    v = add(v, rf(&w, O + 0x240))?;
    if f60 & 1 << 26 != 0 && v < 75.0 {
        v = 75.0;
    }
    if f60 & 0x80 != 0 && rf(&w, O + 0x2FC) < -0.5 {
        let m = if word(&w, O + 0x64) & 1 << 25 != 0 { rf(&w, 0x800A_D518) } else { rf(&w, 0x800A_D51C) };
        v = mul(v, m)?;
    }
    Some((w, v.to_bits()))
}

/// A throttle on or near each constant of the set in use, or anything.
fn throttle() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.1f32),
        Just(0.1f32.next_up()),
        Just(-0.1f32),
        Just((-0.1f32).next_down()),
        Just(0.99f32),
        Just(-0.6f32),
        Just(-0.6f32.next_up()),
        Just(0.0f32),
        -1.0f32..1.0,
        ordinary(),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Both constant sets, every flag bit tested, throttles on the
    /// constants, rates around the caps (0.2, `0.5 x`, `x / (1 - x)`) and
    /// both signs, `[o + 0x184]` around 15, `[o + 0x244]` around 1, and sums
    /// around 75.
    #[test]
    fn func_80068410(seed: u64, bits in prop::array::uniform8(any::<bool>()), fl: u32, x in throttle(), r in prop_oneof![Just(0.2f32), Just(0.0f32), Just(-0.0f32), -2.0f32..2.0, ordinary()],
                     vals in prop::array::uniform6(ordinary()), f184 in prop_oneof![Just(15.0f32), Just(14.0f32), ordinary()], f244 in prop_oneof![Just(1.0f32), Just(0.5f32), ordinary()],
                     f240 in prop_oneof![Just(75.0f32), 50.0f32..100.0, ordinary()], f2fc in prop_oneof![Just(-0.5f32), Just(-1.0f32), ordinary()], d in prop_oneof![Just(1.0f64 / 60.0), 0.0f64..0.1]) {
        let mut s = state(seed);
        let f60 = (fl & !(1 << 23 | 0x200 | 1 << 26 | 0x80)) | u32::from(bits[0]) << 23 | u32::from(bits[1]) << 9 | u32::from(bits[2]) << 26 | u32::from(bits[3]) << 7;
        wr(&mut s, O + 0x60, f60);
        let f64_ = (fl.rotate_left(11) & !(0x2000 | 1 << 27 | 1 << 25)) | u32::from(bits[4]) << 13 | u32::from(bits[5]) << 27 | u32::from(bits[6]) << 25;
        wr(&mut s, O + 0x64, f64_);
        wr(&mut s, O + 0x1E70, 0x8031_0000);
        let desc = word(&s, 0x8031_0008);
        wr(&mut s, 0x8031_0008, if bits[7] { desc | 8 } else { desc & !8 });
        wf(&mut s, O + 0x18C, x);
        wf(&mut s, O + 0x1A4, r);
        for (k, off) in [0x84u32, 0x80, 0x7C, 0x78, 0x1AC, 0x240].into_iter().enumerate() {
            wf(&mut s, O + off, vals[k]);
        }
        wf(&mut s, O + 0x240, f240);
        wf(&mut s, O + 0x184, f184);
        wf(&mut s, O + 0x244, f244);
        wf(&mut s, O + 0x2FC, f2fc);
        wd(&mut s, DT, d);
        s.ctx.gpr[A0] = sext(O);
        let model = speed_model(&s);
        prop_assume!(model.is_some());
        let (w, f0) = model.unwrap();
        let after = run("func_80068410", misc::func_80068410, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), f0);
    }
}

/// The thrust ties inside `e < F`: with `lo = 0`, `hi = s = -3` (`M = e =
/// -3`, so `r = 1` and step 2 leaves `p`), `dt = 1/60` and `F = dt * 30 * g
/// * p`: `p = 0` gives `F = 0 > e` (the `0 < p` tie), and `p = 0.5` with `g =
/// 4` gives `F = 1 > e` and `8 p = 4` (the `4 < 8 p` tie, bit 24 clear so
/// the message would go); `p = -1` meets the `M < e` tie of step 2 with a
/// negative thrust (the decay would apply); `[o + 0x2FC] = 0` with `0 < F`
/// meets the `[o + 0x2FC] < 0` tie (the factor is 1 either way, but the
/// registers differ).
#[test]
fn func_800672D4_ties() {
    for (seed, (p, g, w2fc)) in [(0.0f32, 1.0f32, 1.0f32), (0.5, 4.0, 1.0), (0.5, 8.0, 1.0), (-1.0, 1.0, 1.0), (0.5, 4.0, 0.0)].into_iter().enumerate() {
        let mut s = state(seed as u64);
        wr(&mut s, O + 0x60, 0);
        wr(&mut s, O + 0x64, 0);
        wf(&mut s, O + 0xA4, 0.0);
        wf(&mut s, O + 0x94, -3.0);
        wf(&mut s, O + 0x28, 0.0);
        wf(&mut s, O + 0xA8, 0.0);
        wf(&mut s, O + 0x2C8, 0.0);
        wf(&mut s, O + 0x1B4, p);
        wf(&mut s, O + 0x1A0, 1.0);
        wf(&mut s, O + 0x190, g);
        wf(&mut s, O + 0x2FC, w2fc);
        for c in 0..3 {
            wf(&mut s, O + 0x194 + 4 * c, 1.0 + c as f32);
            wf(&mut s, V + 4 * c, 10.0);
        }
        wd(&mut s, DT, 1.0 / 60.0);
        wr(&mut s, 0x800A_2170, POOL_LIST);
        wr(&mut s, POOL_LIST, POOL_DESC);
        wr(&mut s, POOL_LIST + 4, 0);
        wr(&mut s, POOL_DESC, 7);
        wr(&mut s, POOL_DESC + 0x24, MARK);
        wr(&mut s, O, 7);
        let h = half(&s, O + 6);
        wh(&mut s, O + 6, h & !0x100);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(O), sext(V), sext((-3.0f32).to_bits()));
        let w = engine_model(&s).unwrap();
        let after = compare("func_800672D4", misc::func_800672D4, &s).unwrap_or_else(|d| panic!("{d}"));
        same_memory(&after, &w).unwrap();
    }
}

/// A plain speed-limit state: constant set 2 (bit 3 clear) or 1, `B = 4`
/// or 1.5, throttle `x`, rate `r`, and benign other fields (no bit-9 decay,
/// no clamps, `[o + 0x184]` above 15).
fn speed_case(seed: u64, set1: bool, b4: bool, x: f32, r: f32, d: f64) -> State {
    let mut s = state(seed);
    wr(&mut s, O + 0x60, if b4 { 1 << 23 } else { 0 });
    wr(&mut s, O + 0x64, 0);
    wr(&mut s, O + 0x1E70, 0x8031_0000);
    wr(&mut s, 0x8031_0008, if set1 { 8 } else { 0 });
    wf(&mut s, O + 0x18C, x);
    wf(&mut s, O + 0x1A4, r);
    for (off, v) in [(0x84u32, 2.0f32), (0x80, 2.0), (0x7C, 1.0), (0x78, 1.0), (0x1AC, 1.0), (0x240, 0.0), (0x184, 20.0), (0x244, 1.0), (0x2FC, 0.0)] {
        wf(&mut s, O + off, v);
    }
    wd(&mut s, DT, d);
    s.ctx.gpr[A0] = sext(O);
    s
}

fn speed_check(s: &State) {
    let (w, f0) = speed_model(s).unwrap();
    let after = compare("func_80068410", misc::func_80068410, s).unwrap_or_else(|d| panic!("{d}"));
    same_memory(&after, &w).unwrap();
    assert_eq!(after.ctx.fpr[0].u32l(), f0);
}

/// The speed limit's ties and separators, in both constant sets, with `dt
/// = 0.5` where exact values are needed: `r == 0.5 x` after the update (`x =
/// -0.5`, `r` from 0.125), `r` between `0.5 x` and a slightly larger bound
/// (from 0.1245), `r == c` for `c = x / (1 - x) = 1` (`x = 0.5`, `B = 4`),
/// `x == K1` with `r = 500` between `x / (1 - x)` and `K2`, and the fused `(dt
/// x) B + r` for `B = 1.5` with `r = -fl(fl(dt x) * 1.5)`, so the unfused
/// sum is exactly 0 and the fused one the product's rounding error.
#[test]
fn func_80068410_ties() {
    let mut seed = 0;
    for set1 in [false, true] {
        let mut cases = vec![(false, -0.5f32, 0.125f32, 0.5f64), (false, -0.5, 0.1245, 0.5), (true, 0.5, 0.0, 0.5), (false, 0.99, 500.0, 1.0 / 60.0)];
        for x in [0.3f32, -0.3] {
            let d = (1.0f64 / 60.0) as f32;
            let p1 = d * x;
            assert_ne!(p1.mul_add(1.5, -(p1 * 1.5)), 0.0);
            cases.push((false, x, -(p1 * 1.5), 1.0 / 60.0));
        }
        for (b4, x, r, d) in cases {
            seed += 1;
            speed_check(&speed_case(seed, set1, b4, x, r, d));
        }
    }
}
