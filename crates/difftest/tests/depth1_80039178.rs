//! Depth 1 at 0x80039178..0x80041214 (without func_8003BAB0): the CRC-32
//! (game::save), a spline segment's evaluation (game::spline), the render
//! reset (game::render), two thunks (game::misc), and the pool messages
//! "Qery", "Aloc", "Free" and the debug hook (game::pools). Recompiled C
//! vs Rust with the callees as C. The models replay the callees' C on a
//! copy of the state in the same order with the same arguments, adding
//! the functions' own stores, and whole RDRAM is compared.

// Tests are named after the functions (func_80039178), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::imports;
use game::recomp::{reg::*, RecompFn};
use game::render::DL_HEAD;
use game::{misc, pools, render, save, spline};
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
/// the ROM image has it: the spline bases live there.
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

// ---- save: crc32 ----

const CRC_TABLE: u32 = 0x8011_4070;
const BYTES: u32 = 0x8030_0000;

/// CRC-32 MSB first (polynomial 0x04C11DB7, initial and final XOR all
/// ones), bit by bit.
fn crc32_bits(bytes: &[u8]) -> u32 {
    let mut c = u32::MAX;
    for &b in bytes {
        c ^= u32::from(b) << 24;
        for _ in 0..8 {
            c = if c & 0x8000_0000 != 0 { c << 1 ^ 0x04C1_1DB7 } else { c << 1 };
        }
    }
    !c
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// With the table built here (entry 1 = 0 before) the result is the
    /// CRC; with a table already there (any words, entry 1 nonzero) the
    /// lookup model.
    #[test]
    fn func_80039178(seed: u64, n in prop_oneof![Just(0i32), Just(-1i32), 1i32..300, Just(i32::MIN)], build: bool, entry1: u32) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 3, BYTES, 0x200);
        wr(&mut s, CRC_TABLE + 4, if build { 0 } else { entry1 | 1 });
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(BYTES), sext(n as u32));
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        wr(&mut w, SP_AT, BYTES);
        if build {
            wr(&mut w, SP_AT + 4, n as u32);
            call_c(&mut w, 0x18, &[], imports::func_800390C0);
        }
        let mut c = u32::MAX;
        for k in 0..n.max(0) as u32 {
            let b = u32::from(byte(&s, BYTES + k));
            c = word(&w, CRC_TABLE + 4 * (b ^ (c >> 24))) ^ (c << 8);
        }
        let bytes: Vec<u8> = (0..n.max(0) as u32).map(|k| byte(&s, BYTES + k)).collect();
        if build {
            prop_assert_eq!(!c, crc32_bits(&bytes));
        }
        let after = run("func_80039178", save::func_80039178, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(!c));
        same_memory(&after, &w)?;
    }
}

// ---- spline: a segment ----

const SPLINE: u32 = 0x8030_0000;
const RECORDS: u32 = 0x8031_0000;
const IDX: u32 = 0x8030_0100;
const OUT: u32 = 0x8030_0200;

/// `out[j] = v.w * m[3][j] + ((m[0][j] * v.x + m[1][j] * v.y) + m[2][j] *
/// v.z)` as func_80016DD8 computes it; None where a NaN would reach an
/// operation.
fn weights(t: [f32; 4], m: &[f32; 16]) -> Option<[f32; 4]> {
    let mut w = [0.0; 4];
    for j in 0..4 {
        w[j] = add(mul(t[3], m[12 + j])?, add(add(mul(m[j], t[0])?, mul(m[4 + j], t[1])?)?, mul(m[8 + j], t[2])?)?)?;
    }
    Some(w)
}

/// `w3 * Q_3 + ((Q_0 * w0 + Q_1 * w1) + Q_2 * w2)` per component.
fn blend(s: &State, q: [u32; 4], w: [f32; 4]) -> Option<[f32; 3]> {
    let mut out = [0.0; 3];
    for c in 0..3 {
        let p = q.map(|a| rf(s, a + 4 * c as u32));
        out[c] = add(mul(w[3], p[3])?, add(add(mul(p[0], w[0])?, mul(p[1], w[1])?)?, mul(p[2], w[2])?)?)?;
    }
    Some(out)
}

fn basis(s: &State, at: u32) -> [f32; 16] {
    std::array::from_fn(|k| rf(s, at + 4 * k as u32))
}

/// The segment by the statement; None outside the domain.
fn spline_model(s: &State, flags: u32, t: f32) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x68;
    saves(&mut w, s, 0x68, &[(0x14, S0), (0x18, S1), (0x1C, RA)]);
    wr(&mut w, SP_AT + 4, flags);
    let tt = [mul(mul(mul(1.0, t)?, t)?, t)?, mul(mul(1.0, t)?, t)?, mul(1.0, t)?, 1.0];
    for (k, x) in tt.iter().enumerate() {
        wf(&mut w, sp + 0x58 + 4 * k as u32, *x);
    }
    let kind = half(s, SPLINE) as i16;
    let r = |k: u32| RECORDS.wrapping_add(0x54u32.wrapping_mul(word(s, IDX + 4 * k)));
    let pts = if kind == 0 { [r(0) + 0x10, r(1) + 0x10, r(2) + 0x10, r(3) + 0x10] } else { [r(0) + 0x10, r(0) + 0x34, r(1) + 0x28, r(1) + 0x10] };
    let bases = if kind == 0 { [0x800A_4750u32, 0x800A_4790, 0x800A_47D0] } else { [0x800A_4810, 0x800A_4850, 0x800A_4890] };
    let run_basis = |w: &mut State, b: u32| -> Option<[f32; 4]> {
        let v = weights(tt, &basis(w, b))?;
        call_c(w, 0x68, &[(A0, sext(sp + 0x48)), (A1, sext(sp + 0x58)), (A2, sext(b))], imports::func_80016DD8);
        Some(v)
    };
    let wt = run_basis(&mut w, bases[0])?;
    if flags & 1 != 0 {
        let o = blend(&w, pts, wt)?;
        for c in 0..3 {
            wf(&mut w, OUT + 4 * c as u32, o[c]);
        }
    }
    if flags & 8 != 0 {
        let o = if kind == 1 { [0.0, 0.0, 1.0] } else { blend(&w, [r(0) + 0x1C, r(1) + 0x1C, r(2) + 0x1C, r(3) + 0x1C], wt)? };
        for c in 0..3 {
            wf(&mut w, OUT + 0x24 + 4 * c as u32, o[c]);
        }
    }
    for (bit, b, at) in [(2u32, bases[1], 0xCu32), (4, bases[2], 0x18)] {
        if flags & bit != 0 {
            for (k, &p) in pts.iter().enumerate() {
                wr(&mut w, sp + 0x44 - 4 * k as u32, p);
            }
            let wd = run_basis(&mut w, b)?;
            let o = blend(&w, pts, wd)?;
            for c in 0..3 {
                wf(&mut w, OUT + at + 4 * c as u32, o[c]);
            }
        }
    }
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Each type with each flag combination; the bases from the ROM, or
    /// perturbed (they're repeated constants).
    #[test]
    fn func_8003A5D0(seed: u64, kind in prop_oneof![Just(0i16), Just(1i16), Just(2i16), any::<i16>()], flags in prop_oneof![0u32..16, any::<u32>()],
                     t in prop_oneof![3 => 0.0f32..=1.0, 1 => Just(0.0f32), 1 => Just(1.0f32), 1 => ordinary()], idx in prop::array::uniform4(0u32..6),
                     points in proptest::collection::vec(ordinary(), 6 * 12), perturb in prop::option::weighted(0.2, (0usize..6, 0usize..16, ordinary()))) {
        let mut s = state(seed);
        load_data(&mut s);
        wh(&mut s, SPLINE, kind as u16);
        wr(&mut s, SPLINE + 0xC, RECORDS);
        for k in 0..4u32 {
            wr(&mut s, IDX + 4 * k, idx[k as usize]);
        }
        for (i, chunk) in points.chunks(12).enumerate() {
            for (k, &x) in chunk.iter().enumerate() {
                wf(&mut s, RECORDS + 0x54 * i as u32 + 0x10 + 4 * k as u32, x);
            }
        }
        if let Some((b, k, x)) = perturb {
            wf(&mut s, [0x800A_4750u32, 0x800A_4790, 0x800A_47D0, 0x800A_4810, 0x800A_4850, 0x800A_4890][b] + 4 * k as u32, x);
        }
        wr(&mut s, SP_AT + 0x10, OUT);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(SPLINE), sext(flags), sext(t.to_bits()), sext(IDX));
        let model = spline_model(&s, flags, t);
        prop_assume!(model.is_some());
        let after = run("func_8003A5D0", spline::func_8003A5D0, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- render: reset and the screen rectangle; misc: two thunks ----

const DL: u32 = 0x8038_0000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_8003D444(seed: u64, settings: u32, wh_: (i16, i16), shade: u16) {
        let mut s = state(seed);
        wr(&mut s, DL_HEAD, DL);
        wr(&mut s, 0x800D_697C, settings);
        wh(&mut s, 0x8011_447C, shade);
        wh(&mut s, 0x8011_4470, wh_.0 as u16);
        wh(&mut s, 0x8011_4472, wh_.1 as u16);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        call_c(&mut w, 0x18, &[], imports::func_8003D110);
        let (sw, sh) = (half(&w, 0x8011_4470) as i16, half(&w, 0x8011_4472) as i16);
        call_c(&mut w, 0x18, &[(A0, 0), (A1, sext((i32::from(sw) - 1) as u32)), (A2, 0), (A3, sext((i32::from(sh) - 1) as u32))], imports::func_8003B300);
        let after = run("func_8003D444", render::func_8003D444, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_8003E18C(seed: u64, sxy in prop::array::uniform2(ordinary()), halves in prop::array::uniform4(any::<i16>()), null_o: bool, null_t: bool, junk: u64) {
        let mut s = state(seed);
        let (p, o, t) = (0x8030_0000u32, 0x8031_0000u32, 0x8031_1000u32);
        wr(&mut s, p, if null_o { 0 } else { o });
        wr(&mut s, o + 8, if null_t { 0 } else { t });
        for (k, &h) in halves.iter().enumerate() {
            wh(&mut s, [o + 4, o + 6, t + 4, t + 6][k], h as u16);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(p), junk);
        s.ctx.gpr[A2] = u64::from((junk >> 32) as u32) << 32 | u64::from(sxy[0].to_bits());
        s.ctx.gpr[A3] = u64::from(junk as u32) << 32 | u64::from(sxy[1].to_bits());
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        wr(&mut w, SP_AT, p);
        wr(&mut w, SP_AT + 4, junk as u32);
        call_c(&mut w, 0x18, &[(A0, sext(word(&s, p))), (A1, sext(sxy[0].to_bits())), (A2, sext(sxy[1].to_bits()))], imports::func_8003E0A0);
        let after = run("func_8003E18C", misc::func_8003E18C, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_80041214(seed: u64, v in prop_oneof![Just(0u32), Just(1u32), Just(2u32), Just(3u32), any::<u32>()]) {
        let mut s = state(seed);
        let o = 0x8030_0000u32;
        wr(&mut s, o + 0x80, v);
        s.ctx.gpr[A0] = sext(o);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        if v == 1 || v == 2 {
            call_c(&mut w, 0x18, &[(A0, sext(o)), (A1, sext(v))], imports::func_8004110C);
        } else {
            wr(&mut w, o + 0x7C, v);
        }
        let after = run("func_80041214", misc::func_80041214, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- pools: messages ----

const POOL_LIST: u32 = 0x8030_0000;
const POOL_DESCS: u32 = 0x8030_0100;
const POOL_ELEMS: u32 = 0x8030_1000;
/// func_8005F31C, a verified callback: `[elem + 0x14] += 1`, `[elem + 0x18]
/// = arg` (see indirect.rs).
const MARK: u32 = 0x8005_F31C;
const SIZE: u32 = 0x68;

#[derive(Clone, Debug)]
struct Pool {
    id: u32,
    count: i32,
    callback: u32,
    flagged: [bool; 4],
}

fn pools_strategy() -> impl Strategy<Value = Vec<Pool>> {
    let pool = (1u32..4, prop_oneof![0i32..=4, Just(-1i32)], prop_oneof![3 => Just(MARK), 1 => Just(0u32)], prop::array::uniform4(any::<bool>()))
        .prop_map(|(id, count, callback, flagged)| Pool { id, count, callback, flagged });
    proptest::collection::vec(pool, 0..4)
}

fn elem(k: usize, i: u32) -> u32 {
    POOL_ELEMS + 0x200 * k as u32 + SIZE * i
}

fn registry(s: &mut State, seed: u64, pools: &[Pool]) {
    s.randomise_memory(seed ^ 2, POOL_LIST, 0x1800);
    let mut m = s.rdram.mem();
    m.write_u32(0x800A_2170, POOL_LIST);
    for (k, p) in pools.iter().enumerate() {
        let d = POOL_DESCS + 0x40 * k as u32;
        m.write_u32(POOL_LIST + 4 * k as u32, d);
        m.write_u32(d, p.id);
        m.write_u32(d + 8, p.count as u32);
        m.write_u32(d + 0xC, SIZE);
        m.write_u32(d + 0x10, elem(k, 0));
        m.write_u32(d + 0x24, p.callback);
        for i in 0..4u32 {
            let e = elem(k, i);
            m.write_u32(e, p.id);
            let h = m.read_u16(e + 6);
            m.write_u16(e + 6, if p.flagged[i as usize] { h | 0x100 } else { h & !0x100 });
        }
    }
    m.write_u32(POOL_LIST + 4 * pools.len() as u32, 0);
}

fn send(w: &mut State, frame: u32, e: u32, msg: u32) {
    call_c(w, frame, &[(A0, sext(e)), (A1, sext(msg))], imports::func_8003F99C);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_8003F8FC(seed: u64, ab: (u32, u32)) {
        let mut s = state(seed);
        let src = 0x8030_0000u32;
        s.randomise_memory(seed ^ 4, src, 0x40);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(ab.0), sext(ab.1), sext(src));
        let mut w = s.clone();
        let sp = SP_AT - 0x70;
        saves(&mut w, &s, 0x70, &[(0x14, RA)]);
        wr(&mut w, sp + 0x2C, ab.0);
        wr(&mut w, sp + 0x30, ab.1);
        for k in 0..14u32 {
            let v = word(&s, src + 4 * k);
            wr(&mut w, sp + 0x34 + 4 * k, v);
        }
        call_c(&mut w, 0x70, &[(A0, 0xEE06), (A1, sext(sp + 0x2C))], imports::func_80018450);
        let after = run("func_8003F8FC", pools::func_8003F8FC, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_8003FB34(seed: u64, pools in pools_strategy(), which in (0usize..4, 0u32..4), x: u32, null: bool) {
        let mut s = state(seed);
        registry(&mut s, seed, &pools);
        let e = if null { 0 } else { elem(which.0.min(pools.len().saturating_sub(1)), which.1) };
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(e), sext(x));
        let mut w = s.clone();
        let sp = SP_AT - 0x60;
        saves(&mut w, &s, 0x60, &[(0x14, RA)]);
        for (off, v) in [(0x1C, 0), (0x20, 0x5165_7279), (0x24, x), (0x28, sp + 0x1C), (0x2C, 0x5165_7279)] {
            wr(&mut w, sp + off, v);
        }
        send(&mut w, 0x60, e, sp + 0x20);
        let r = word(&w, sp + 0x1C);
        let after = run("func_8003FB34", pools::func_8003FB34, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(r));
        same_memory(&after, &w)?;
    }

    /// The answer written: the element is placed in the frame so that the
    /// MARK callback's `+0x14` increment lands on the answer word `r` (its
    /// id word and flags sit in frame words the function doesn't write).
    #[test]
    fn func_8003FB34_answer(seed: u64, x: u32) {
        let mut s = state(seed);
        let pools = [Pool { id: 1, count: 1, callback: MARK, flagged: [false; 4] }];
        registry(&mut s, seed, &pools);
        let sp = SP_AT - 0x60;
        let e = sp + 8;
        wr(&mut s, e, 1);
        wh(&mut s, e + 6, 0);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(e), sext(x));
        let after = run("func_8003FB34", pools::func_8003FB34, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], 1);
        prop_assert_eq!(word(&after, sp + 0x20), sp + 0x20);
    }

    #[test]
    fn func_8003FBD4(seed: u64, pools in pools_strategy(), id in 0u32..5) {
        let mut s = state(seed);
        registry(&mut s, seed, &pools);
        s.ctx.gpr[A0] = u64::from(id);
        let mut w = s.clone();
        let sp = SP_AT - 0x58;
        saves(&mut w, &s, 0x58, &[(0x14, RA)]);
        let mut v0 = 0;
        if let Some(k) = pools.iter().position(|p| p.id == id) {
            let p = &pools[k];
            if p.callback != 0 {
                for i in 0..p.count.max(0) as u32 {
                    let e = elem(k, i);
                    let h = half(&w, e + 6);
                    if h & 0x100 != 0 {
                        wh(&mut w, e + 6, h & 0xFEFF);
                        wr(&mut w, sp + 0x24, 0x416C_6F63);
                        wr(&mut w, sp + 0x44, e);
                        send(&mut w, 0x58, e, sp + 0x24);
                        v0 = e;
                        break;
                    }
                }
            }
        }
        let after = run("func_8003FBD4", pools::func_8003FBD4, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(v0));
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_8003FC94(seed: u64, pools in pools_strategy(), id in 0u32..5) {
        let mut s = state(seed);
        registry(&mut s, seed, &pools);
        s.ctx.gpr[A0] = u64::from(id);
        let mut w = s.clone();
        let sp = SP_AT - 0x50;
        saves(&mut w, &s, 0x50, &[(0x14, S0), (0x18, S1), (0x1C, S2), (0x20, S3), (0x24, S4), (0x28, S5), (0x2C, RA)]);
        wr(&mut w, sp + 0x38, 0x4672_6565);
        for (k, p) in pools.iter().enumerate() {
            if p.id != id {
                continue;
            }
            let mut i = 0i32;
            while i < word(&w, POOL_DESCS + 0x40 * k as u32 + 8) as i32 {
                let e = elem(k, i as u32);
                if half(&w, e + 6) & 0x100 == 0 {
                    // The s registers the port holds: s0 = e, s1 = i, s2 =
                    // the descriptor, s3 = the message, s4 = the slot, s5 =
                    // id (the callback's callees may save them).
                    w.ctx.gpr[S0] = sext(e);
                    w.ctx.gpr[S1] = sext(i as u32);
                    w.ctx.gpr[S2] = sext(POOL_DESCS + 0x40 * k as u32);
                    w.ctx.gpr[S3] = sext(sp + 0x38);
                    w.ctx.gpr[S4] = sext(POOL_LIST + 4 * k as u32);
                    w.ctx.gpr[S5] = u64::from(id);
                    send(&mut w, 0x50, e, sp + 0x38);
                    let h = half(&w, e + 6);
                    wh(&mut w, e + 6, h | 0x100);
                }
                i = i32::from((i + 1) as i16);
            }
        }
        let after = run("func_8003FC94", pools::func_8003FC94, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_8003FD7C(seed: u64, pools in pools_strategy(), which in (0usize..4, 0u32..4), null: bool) {
        let mut s = state(seed);
        registry(&mut s, seed, &pools);
        let e = if null { 0 } else { elem(which.0.min(pools.len().saturating_sub(1)), which.1) };
        s.ctx.gpr[A0] = sext(e);
        let mut w = s.clone();
        let sp = SP_AT - 0x38;
        saves(&mut w, &s, 0x38, &[(0x14, RA)]);
        if e != 0 && half(&s, e + 6) & 0x100 == 0 {
            wr(&mut w, sp + 0x18, 0x4672_6565);
            wr(&mut w, SP_AT, e);
            send(&mut w, 0x38, e, sp + 0x18);
            let h = half(&w, e + 6);
            wh(&mut w, e + 6, h | 0x100);
        }
        let after = run("func_8003FD7C", pools::func_8003FD7C, &s)?;
        same_memory(&after, &w)?;
    }
}
