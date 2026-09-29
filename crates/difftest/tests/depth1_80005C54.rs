//! Depth 1 at 0x80005C54..0x80007F5C: animation tracks and objects
//! (game::anim) and a handle's length (game::misc). Recompiled C vs Rust
//! with the callees as C, each checked against its statement by simulating
//! the stores (the frame's included) on a copy of the input and comparing
//! all of RDRAM, the callees' effects taken from their statements.

// Tests are named after the functions (func_80005C54), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::recomp::{fpu, reg::*, RecompFn};
use game::{anim, misc};
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

fn rom_word(vaddr: u32) -> u32 {
    let o = (vaddr - 0x8000_0400 + 0x1000) as usize;
    u32::from_be_bytes(baserom()[o..o + 4].try_into().unwrap())
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
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![4 => -1.0e3f32..1.0e3f32, 2 => 0.0f32..1.0, 2 => (-4i32..=4).prop_map(|n| n as f32), 1 => float()].boxed()
}

/// Sorted keys (ties included) and a time among them: at a key, between two,
/// before the first or after the last.
fn track(n: std::ops::RangeInclusive<usize>) -> BoxedStrategy<(Vec<f32>, f32)> {
    (prop::collection::vec(prop_oneof![3 => 0.0f32..100.0, 1 => (0i32..8).prop_map(|n| n as f32)], n), 0u8..5, 0.0f32..1.0, any::<prop::sample::Index>(), float())
        .prop_map(|(mut k, mode, f, ix, any)| {
            k.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let i = ix.index(k.len());
            let x = match mode {
                0 => k[i],
                1 if i + 1 < k.len() => k[i] + f * (k[i + 1] - k[i]),
                2 => k[0] - 1.0,
                3 => k[k.len() - 1] + 1.0,
                _ => any,
            };
            (k, x)
        })
        .boxed()
}

// ---- func_80005C54: reset ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn func_80005C54(seed: u64, junk in prop::collection::vec(any::<u32>(), 331)) {
        let mut s = state(seed);
        wr(&mut s, 0x8009_A2A0, junk[330]);
        for (i, w) in junk[..330].iter().enumerate() {
            wr(&mut s, 0x800A_F4C0 + 4 * i as u32, *w);
        }
        let mut want = s.clone();
        wr(&mut want, SP_AT - 4, s.ctx.gpr[RA] as u32);
        wr(&mut want, 0x8009_A2A0, 0);
        for a in (0x800A_F4C0..0x800A_F9E8).step_by(4) {
            wr(&mut want, a, 0);
        }
        let after = run("func_80005C54", anim::func_80005C54, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_80005CD4 / func_80005DA8: track values ----

const OBJ: u32 = 0x8030_0000;
const KEYS: u32 = 0x8030_1000;
const VALS: u32 = 0x8030_2000;
const OUT: u32 = 0x8030_3000;

/// Which case: 0 after `k[i + 1]`, 1 at or before `k[i]`, 2 between (with
/// `t`), or None outside the domain.
fn where_(k: &[f32], x: f32, i: usize) -> Option<(u8, f32)> {
    if k[i + 1] < x {
        return Some((0, 0.0));
    }
    if x <= k[i] {
        return Some((1, 0.0));
    }
    let d = sub(k[i + 1], k[i])?;
    Some((2, div(sub(x, k[i])?, d)?))
}

fn frame_saves(want: &mut State, s: &State, frame: u32, saves: &[(u32, usize)]) {
    for &(off, r) in saves {
        wr(want, SP_AT - frame + off, s.ctx.gpr[r] as u32);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80005CD4(seed: u64, (k, x) in track(2..=6), ix: prop::sample::Index, vals in prop::collection::vec(ordinary(), 6), out0: u32) {
        let i = ix.index(k.len() - 1);
        let w = where_(&k, x, i);
        prop_assume!(w.is_some());
        let (case, t) = w.unwrap();
        let value = match case {
            0 => Some(vals[i + 1]),
            1 => Some(vals[i]),
            _ => (|| add(mul(t, vals[i + 1])?, mul(sub(1.0, t)?, vals[i])?))(),
        };
        prop_assume!(value.is_some());
        let mut s = state(seed);
        wr(&mut s, OBJ + 0x11C, KEYS);
        wr(&mut s, OBJ + 0x120, VALS);
        for (j, v) in k.iter().enumerate() {
            wr(&mut s, KEYS + 4 * j as u32, v.to_bits());
        }
        for (j, v) in vals.iter().enumerate() {
            wr(&mut s, VALS + 4 * j as u32, v.to_bits());
        }
        wr(&mut s, OUT, out0);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(OUT), sext(OBJ), sext(x.to_bits()), i as u64);
        let mut want = s.clone();
        frame_saves(&mut want, &s, 0x30, &[(0x1C, RA), (0x18, S0), (0x30, A0)]);
        if case == 2 {
            wr(&mut want, SP_AT - 0x30 + 0x20, 4 * i as u32);
        }
        wr(&mut want, OUT, value.unwrap().to_bits());
        let after = run("func_80005CD4", anim::func_80005CD4, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80005DA8(seed: u64, (k, x) in track(2..=6), ix: prop::sample::Index, vals in prop::collection::vec(ordinary(), 18), out0: [u32; 3]) {
        let i = ix.index(k.len() - 1);
        let w = where_(&k, x, i);
        prop_assume!(w.is_some());
        let (case, t) = w.unwrap();
        let v = |j: usize, c: usize| vals[3 * j + c];
        let value: Option<[f32; 3]> = match case {
            0 => Some([v(i + 1, 0), v(i + 1, 1), v(i + 1, 2)]),
            1 => Some([v(i, 0), v(i, 1), v(i, 2)]),
            _ => (|| {
                let s = sub(1.0, t)?;
                let c = |c: usize| add(mul(v(i, c), s)?, mul(v(i + 1, c), t)?);
                Some([c(0)?, c(1)?, c(2)?])
            })(),
        };
        prop_assume!(value.is_some());
        let mut s = state(seed);
        wr(&mut s, OBJ + 0x11C, KEYS);
        wr(&mut s, OBJ + 0x120, VALS);
        for (j, x) in k.iter().enumerate() {
            wr(&mut s, KEYS + 4 * j as u32, x.to_bits());
        }
        for (j, x) in vals.iter().enumerate() {
            wr(&mut s, VALS + 4 * j as u32, x.to_bits());
        }
        for (j, w) in out0.iter().enumerate() {
            wr(&mut s, OUT + 4 * j as u32, *w);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(OUT), sext(OBJ), sext(x.to_bits()), i as u64);
        let mut want = s.clone();
        frame_saves(&mut want, &s, 0x48, &[(0x24, RA), (0x20, S2), (0x1C, S1), (0x18, S0)]);
        if case == 2 {
            for c in 0..3 {
                wr(&mut want, SP_AT - 0x48 + 0x3C + 4 * c as u32, v(i, c).to_bits());
                wr(&mut want, SP_AT - 0x48 + 0x30 + 4 * c as u32, v(i + 1, c).to_bits());
            }
        }
        for (c, x) in value.unwrap().iter().enumerate() {
            wr(&mut want, OUT + 4 * c as u32, x.to_bits());
        }
        let after = run("func_80005DA8", anim::func_80005DA8, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_8000646C: step track to its targets ----

const LIST: u32 = 0x8030_4000;
const TARGETS: u32 = 0x8030_5000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8000646C(seed: u64, (k, u) in track(2..=6), ix: prop::sample::Index, words in prop::collection::vec(any::<u32>(), 6),
                     list in prop::option::of(prop::collection::vec(0u32..4, 0..5)), junk in prop::collection::vec(any::<u32>(), 16)) {
        let seg = ix.index(k.len() - 1);
        let w = if k[seg + 1] < u { words[seg + 1] } else { words[seg] };
        let mut s = state(seed);
        wr(&mut s, OBJ + 0x114, u.to_bits());
        wr(&mut s, OBJ + 0x118, seg as u32);
        wr(&mut s, OBJ + 0x11C, KEYS);
        wr(&mut s, OBJ + 0x120, VALS);
        for (j, x) in k.iter().enumerate() {
            wr(&mut s, KEYS + 4 * j as u32, x.to_bits());
        }
        for (j, x) in words.iter().enumerate() {
            wr(&mut s, VALS + 4 * j as u32, *x);
        }
        for (j, x) in junk.iter().enumerate() {
            wr(&mut s, TARGETS + 4 * j as u32, *x);
        }
        let target = |t: u32| TARGETS + 0x10 * t;
        match &list {
            None => wr(&mut s, OBJ + 0x124, 0),
            Some(l) => {
                wr(&mut s, OBJ + 0x124, LIST);
                for (j, t) in l.iter().enumerate() {
                    wr(&mut s, LIST + 4 * j as u32, target(*t));
                }
                wr(&mut s, LIST + 4 * l.len() as u32, 0);
            }
        }
        s.ctx.gpr[A0] = sext(OBJ);
        let mut want = s.clone();
        frame_saves(&mut want, &s, 0x28, &[(0x24, RA), (0x20, S2), (0x1C, S1), (0x18, S0)]);
        for t in list.iter().flatten() {
            wr(&mut want, target(*t) + 8, w);
        }
        let after = run("func_8000646C", anim::func_8000646C, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_80006E74 / func_80006EC0: set a track's time ----

/// func_80006704: the key segment of `u` (n >= 2).
fn segment(k: &[f32], u: f32) -> u32 {
    let n = k.len();
    if k[n - 1] < u {
        return (n - 2) as u32;
    }
    if u < k[0] {
        return 0;
    }
    (0..=n - 2).rev().find(|&i| !(u < k[i])).unwrap() as u32
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80006E74(seed: u64, (k, u) in track(2..=6), flags: u32, old: [u32; 2]) {
        let mut s = state(seed);
        wr(&mut s, OBJ + 0x100, flags);
        wr(&mut s, OBJ + 0x104, k.len() as u32);
        wr(&mut s, OBJ + 0x114, old[0]);
        wr(&mut s, OBJ + 0x118, old[1]);
        wr(&mut s, OBJ + 0x11C, KEYS);
        for (j, x) in k.iter().enumerate() {
            wr(&mut s, KEYS + 4 * j as u32, x.to_bits());
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(OBJ), sext(u.to_bits()));
        let mut want = s.clone();
        frame_saves(&mut want, &s, 0x18, &[(0x14, RA), (0x18, A0)]);
        wr(&mut want, OBJ + 0x114, u.to_bits());
        wr(&mut want, OBJ + 0x118, segment(&k, u));
        wr(&mut want, OBJ + 0x100, flags | 0x0100_0000);
        let after = run("func_80006E74", anim::func_80006E74, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80006EC0(seed: u64, (k, u) in track(2..=6), old_t in float(), wv: u32, flags: u32, old_seg: u32, junk: [u32; 4]) {
        let mut s = state(seed);
        wr(&mut s, OBJ + 0x100, flags);
        wr(&mut s, OBJ + 0x104, k.len() as u32);
        wr(&mut s, OBJ + 0x114, old_t.to_bits());
        wr(&mut s, OBJ + 0x118, old_seg);
        wr(&mut s, OBJ + 0x11C, KEYS);
        for (j, w) in junk.iter().enumerate() {
            wr(&mut s, OBJ + 0xE0 + 4 * j as u32, *w);
        }
        for (j, x) in k.iter().enumerate() {
            wr(&mut s, KEYS + 4 * j as u32, x.to_bits());
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(OBJ), sext(u.to_bits()), sext(wv));
        let mut want = s.clone();
        frame_saves(&mut want, &s, 0x18, &[(0x14, RA), (0x18, A0)]);
        wr(&mut want, OBJ + 0x100, flags | 0x2000_0000);
        wr(&mut want, OBJ + 0x114, u.to_bits());
        wr(&mut want, OBJ + 0xE0, wv);
        wr(&mut want, OBJ + 0xE4, 0);
        wr(&mut want, OBJ + 0xE8, old_seg);
        wr(&mut want, OBJ + 0xEC, fpu::trunc_w_s(old_t));
        wr(&mut want, OBJ + 0x118, segment(&k, u));
        let after = run("func_80006EC0", anim::func_80006EC0, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_80007F5C: a handle's length ----

const TABLE_A: u32 = 0x8009_A32C;
const TABLE_B: u32 = 0x8009_A388;
const DEFAULT: u32 = 0x800A_FA54;
const K_AT: u32 = 0x800A_81C8;
const CHAIN: u32 = 0x8030_6000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80007F5C(seed: u64, kind in prop_oneof![Just(0u32), Just(1u32), 2u32..8], idx in 0u32..23, j in 0u32..4, bit15: bool,
                     present: bool, n in prop_oneof![Just(0i32), 1i32..100_000, any::<i32>()], k in prop_oneof![3 => Just(None), 1 => float().prop_map(Some)]) {
        let k = k.map_or(rom_word(K_AT), f32::to_bits);
        let handle = kind << 24 | idx << 16 | u32::from(bit15) << 15 | j;
        let o = if present { CHAIN } else { 0 };
        let mut s = state(seed);
        // Every slot the lookup may use points at the same object (or 0).
        wr(&mut s, DEFAULT, o);
        wr(&mut s, TABLE_A + 4 * idx, o);
        wr(&mut s, TABLE_B + 4 * kind, o);
        wr(&mut s, K_AT, k);
        // o + 0xC -> P; P + 4j + 0x10 -> Q; Q + 8 -> R; R + 4 = n.
        let (p, q, r) = (CHAIN + 0x100, CHAIN + 0x200, CHAIN + 0x300);
        wr(&mut s, CHAIN + 0xC, p);
        wr(&mut s, p + 4 * j + 0x10, q);
        wr(&mut s, q + 8, r);
        wr(&mut s, r + 4, n as u32);
        s.ctx.gpr[A0] = sext(handle);
        let want = if present { mul(fpu::cvt_s_w(n as u32, fpu::NEAREST), f32::from_bits(k)) } else { Some(0.0) };
        prop_assume!(want.is_some());
        let after = run("func_80007F5C", misc::func_80007F5C, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.unwrap().to_bits());
        prop_assert_eq!(word(&after, SP_AT), handle);
    }
}
