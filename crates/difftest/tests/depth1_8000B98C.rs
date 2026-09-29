//! Depth 1 at 0x8000B98C..0x8000D9A8 (game::misc): the nearer selected
//! entry's matrix, the selection test, the debug menu's tuning and setting
//! editors, the id push on key 8 and the id-stack reset. Recompiled C vs
//! Rust with the callees as C, each checked against its statement by
//! simulating the stores (frames and the callees' spills included) on a
//! copy of the input and comparing all of RDRAM.

// Tests are named after the functions (func_8000B98C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc::{self, CURRENT_ID, CURRENT_WORD, ENTRIES, ID_DEPTH, ID_STACK, RECORDS_170, SAVED_WORDS};
use game::pools::POOLS;
use game::recomp::{fpu, reg::*, RecompFn};
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

fn saves(want: &mut State, s: &State, frame: u32, regs: &[(u32, usize)]) {
    for &(off, r) in regs {
        wr(want, SP_AT - frame + off, s.ctx.gpr[r] as u32);
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
        Just(-1.0f32),
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

/// func_8000C6C8's effect on `*p`: `+= x*y`, then clamped (lo first).
fn clamp_add(old: f32, x: f32, y: f32, lo: f32, hi: f32) -> Option<f32> {
    let mut v = add(old, mul(x, y)?)?;
    if v < lo {
        v = lo;
    }
    if hi < v {
        v = hi;
    }
    Some(v)
}

// ---- func_8000B98C / func_8000BB78: records and entries ----

const ENTRY_BASE: u32 = 0x8030_6000;

fn identity() -> [u32; 16] {
    std::array::from_fn(|i| if i % 5 == 0 { 1.0f32.to_bits() } else { 0 })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(768))]

    #[test]
    fn func_8000B98C(seed: u64, p in prop::array::uniform3(ordinary()), flags in prop::array::uniform2(any::<u32>()), idx in prop::array::uniform2(0u32..8),
                     pos in prop::collection::vec(prop::array::uniform3(ordinary()), 8), same: bool, at in 0u8..4, mats in prop::collection::vec(any::<u32>(), 16 * 8)) {
        let (mut pos, mut p) = (pos, p);
        if same {
            // Equal distances: e_2 wins the tie.
            pos[idx[1] as usize] = pos[idx[0] as usize];
        }
        if at == 1 || at == 2 {
            // A distance of exactly 0.
            p = pos[idx[at as usize - 1] as usize];
        }
        let mut s = state(seed);
        wr(&mut s, ENTRIES, ENTRY_BASE);
        for e in 0..8u32 {
            let at = ENTRY_BASE + 0x7C * e;
            for (i, w) in mats[16 * e as usize..16 * (e as usize + 1)].iter().enumerate() {
                wr(&mut s, at + 0x14 + 4 * i as u32, *w);
            }
            for c in 0..3 {
                wf(&mut s, at + 0x44 + 4 * c, pos[e as usize][c as usize]);
            }
        }
        let rec = |k: u32| RECORDS_170 + 0x170 * k;
        for k in 0..2 {
            wr(&mut s, rec(k + 1), flags[k as usize]);
            wr(&mut s, rec(k + 1) + 4, idx[k as usize]);
        }
        let (out, pp) = (0x8030_5000u32, 0x8030_5100u32);
        for c in 0..3 {
            wf(&mut s, pp + 4 * c, p[c as usize]);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(out), sext(pp));
        // The distances.
        let dist = |e: u32| -> Option<f32> {
            let q = pos[e as usize];
            let (dx, dy, dz) = (sub(p[0], q[0])?, sub(p[1], q[1])?, sub(p[2], q[2])?);
            add(mul(dz, dz)?, add(mul(dx, dx)?, mul(dy, dy)?)?)
        };
        let on = [flags[0] & 1 != 0, flags[1] & 1 != 0];
        let d1 = if on[0] { dist(idx[0]) } else { Some(-1.0) };
        let d2 = if on[1] { dist(idx[1]) } else { Some(-1.0) };
        prop_assume!(d1.is_some() && d2.is_some());
        let (d1, d2) = (d1.unwrap(), d2.unwrap());
        let mut want = s.clone();
        saves(&mut want, &s, 0x40, &[(0x1C, RA), (0x18, S0), (0x40, A0)]);
        wf(&mut want, SP_AT - 0x40 + 0x24, d1);
        wf(&mut want, SP_AT - 0x40 + 0x20, d2);
        wr(&mut want, SP_AT - 0x40 + 0x2C, rec(1));
        wr(&mut want, SP_AT - 0x40 + 0x28, rec(2));
        wr(&mut want, SP_AT - 0x40 + 0x3C, flags[0]);
        if on[0] {
            wr(&mut want, SP_AT - 0x40 + 0x38, flags[1]);
            wr(&mut want, SP_AT - 0x40 + 0x34, idx[0]);
        }
        if on[1] {
            wr(&mut want, SP_AT - 0x40 + 0x30, idx[1]);
        }
        let m = if !(d2 < 0.0) && !(d1 < d2) {
            Some(idx[1])
        } else if !(d1 < 0.0) {
            Some(idx[0])
        } else {
            None
        };
        let words: Vec<u32> = match m {
            Some(e) => (0..16).map(|i| word(&s, ENTRY_BASE + 0x7C * e + 0x14 + 4 * i)).collect(),
            None => identity().to_vec(),
        };
        for (i, w) in words.iter().enumerate() {
            wr(&mut want, out + 4 * i as u32, *w);
        }
        let after = run("func_8000B98C", misc::func_8000B98C, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8000BB78(seed: u64, o in prop_oneof![Just(0x8030_7000u32), any::<u32>()], recs in prop::array::uniform4((any::<u32>(), prop_oneof![Just(0x8030_7000u32), any::<u32>()]))) {
        let mut s = state(seed);
        for (k, (f, v)) in recs.iter().enumerate() {
            wr(&mut s, RECORDS_170 + 0x170 * k as u32, *f);
            wr(&mut s, RECORDS_170 + 0x170 * k as u32 + 4, *v);
        }
        s.ctx.gpr[A0] = sext(o);
        let mut want = s.clone();
        saves(&mut want, &s, 0x28, &[(0x24, RA), (0x20, S3), (0x1C, S2), (0x18, S1), (0x14, S0)]);
        let hit = recs.iter().any(|&(f, v)| f & 1 != 0 && v == o);
        let after = run("func_8000BB78", misc::func_8000BB78, &s)?;
        same_memory(&after, &want)?;
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(hit));
    }
}

// ---- func_8000CC1C: tuning editor ----

const TEST: u32 = 0x5465_7374;
const REGISTRY: u32 = 0x8030_0000;
const DESC: u32 = 0x8030_0100;
const ELEMS: u32 = 0x8030_1000;
const STRIDE: u32 = 0x1E80;
const LINK: u32 = 0x8035_0000;
const TAG: u32 = 0x8009_B7E0;

/// `(o, y, lo, hi, o')` for cases 1..15; `None` entries read the ROM.
fn cases() -> [(u32, Result<f32, u32>, Result<f32, u32>, f32, u32); 15] {
    let k = |a: u32| Err(a);
    [
        (0x6C, Ok(0.01), k(0x800A_85F0), 1.0, 0x1C),
        (0x70, Ok(1.0), Ok(10.0), 1000.0, 0x20),
        (0x74, Ok(1.0), Ok(10.0), 1000.0, 0x24),
        (0x78, Ok(0.01), Ok(0.02), 10.0, 0x28),
        (0x7C, Ok(1.0), Ok(100.0), 2000.0, 0x2C),
        (0x80, Ok(0.5), Ok(2.0), 1000.0, 0x30),
        (0x84, Ok(0.5), Ok(2.0), 1000.0, 0x34),
        (0x88, Ok(1.0), Ok(10.0), 1000.0, 0x38),
        (0x8C, k(0x800A_85F4), k(0x800A_85F4), 30.0, 0x3C),
        (0x90, k(0x800A_85F8), k(0x800A_85F8), 20.0, 0x40),
        (0x94, k(0x800A_85FC), Ok(3.0), 30.0, 0x44),
        (0x98, Ok(0.01), Ok(0.0), 1.0, 0x48),
        (0x9C, k(0x800A_8600), Ok(1.0), 100.0, 0x4C),
        (0xA0, Ok(0.01), Ok(0.0), 1.0, 0x50),
        (0xA8, k(0x800A_8604), k(0x800A_8604), 20.0, 0x54),
    ]
}

/// The literal the C loads for 0.01 and 0.02 (not the f32 nearest to the
/// decimal: `0x3C23D70A` and `0x3CA3D70A`).
fn lit(v: f32) -> f32 {
    match v {
        x if x == 0.01 => f32::from_bits(0x3C23_D70A),
        x if x == 0.02 => f32::from_bits(0x3CA3_D70A),
        x => x,
    }
}

#[derive(Clone, Debug)]
struct Pods {
    /// (tag, flags) per element.
    elems: Vec<(i16, u16)>,
    tag: i32,
    k: u32,
    x: f32,
    fields: Vec<f32>,
    id: u32,
}

fn put_pods(s: &mut State, p: &Pods) {
    wr(s, POOLS, REGISTRY);
    wr(s, REGISTRY, DESC);
    wr(s, REGISTRY + 4, 0);
    wr(s, DESC, p.id);
    wr(s, DESC + 8, p.elems.len() as u32);
    wr(s, DESC + 0xC, STRIDE);
    wr(s, DESC + 0x10, ELEMS);
    for (i, &(t, f)) in p.elems.iter().enumerate() {
        let e = ELEMS + STRIDE * i as u32;
        wh(s, e + 4, t as u16);
        wh(s, e + 6, f);
        wr(s, e + 0x1E70, LINK + 0x100 * i as u32);
        for (j, v) in p.fields.iter().enumerate() {
            wf(s, e + 0x6C + 4 * j as u32, *v);
        }
    }
    wr(s, TAG, p.tag as u32);
}

/// func_8003F714("Test", t): the first element with that tag, bit 8 clear.
fn find(p: &Pods, t: i32) -> Option<u32> {
    if p.id != TEST {
        return None;
    }
    p.elems.iter().position(|&(tg, f)| i32::from(tg) == t && f & 0x100 == 0).map(|i| ELEMS + STRIDE * i as u32)
}

fn pods() -> BoxedStrategy<Pods> {
    // Flags with bit 9 but not 8: the lookup filters bit 8, so only other
    // bits can show a wrong test of the element it returns.
    let elem = (prop_oneof![3 => 0i16..5, 1 => any::<i16>()], prop_oneof![3 => Just(0u16), 1 => Just(0x100u16), 1 => Just(0x200u16), 1 => any::<u16>()]);
    (prop::collection::vec(elem, 0..5), prop_oneof![3 => -1i32..5, 1 => any::<i32>()], prop_oneof![2 => Just(0u32), 4 => 0u32..17, 1 => 17u32..20, 1 => any::<u32>()],
     ordinary(), prop::collection::vec(prop_oneof![3 => 0.0f32..100.0, 1 => ordinary()], 40), prop_oneof![9 => Just(TEST), 1 => any::<u32>()])
        .prop_map(|(elems, tag, k, x, fields, id)| Pods { elems, tag, k, x, fields, id })
        .boxed()
}

fn tuning(s: &State, p: &Pods) -> Option<State> {
    let mut w = s.clone();
    saves(&mut w, s, 0x38, &[(0x1C, S0), (0x20, S1), (0x24, RA), (0x38, A0)]);
    wr(&mut w, 0x8009_B804, 1);
    wf(&mut w, SP_AT + 4, p.x);
    // func_8003F714's own frame: s0 (0x8009B7E0) below ours.
    wr(&mut w, SP_AT - 0x3C, TAG);
    let Some(e) = find(p, p.tag) else { return Some(w) };
    if p.k >= 17 {
        return Some(w);
    }
    let x = p.x;
    match p.k {
        1..=15 => {
            let (o, y, lo, hi, o2) = cases()[p.k as usize - 1];
            let rd = |r: Result<f32, u32>| r.map(lit).unwrap_or_else(|a| rf(s, a));
            let (y, lo) = (rd(y), rd(lo));
            let v = clamp_add(rf(s, e + o), x, y, lo, hi)?;
            wf(&mut w, e + o, v);
            wf(&mut w, word(s, e + 0x1E70) + o2, v);
            wf(&mut w, SP_AT - 0x38 + 0x10, hi);
            wf(&mut w, SP_AT - 0x38 + 0xC, lo);
        }
        16 => {
            let old = rf(s, e + 0x108);
            let r = ok(old)?.sqrt();
            let v = clamp_add(r, x, 1.0, 10.0, 500.0)?;
            wf(&mut w, SP_AT - 0x38 + 0x2C, v);
            wf(&mut w, SP_AT - 0x38 + 0x10, 500.0);
            wf(&mut w, SP_AT - 0x38 + 0xC, 10.0);
            wf(&mut w, e + 0x108, mul(v, v)?);
        }
        _ => {
            // Step the selection.
            let d = if 0.0 < x { 1.0f32 } else { -1.0 };
            wf(&mut w, SP_AT + 4, d);
            let mut t = fpu::trunc_w_s(add(p.tag as f32, d)?) as i32;
            wr(&mut w, TAG, t as u32);
            if find(p, t).is_some() {
                return Some(w);
            }
            if d < 0.0 {
                loop {
                    t = t.wrapping_add(1);
                    wr(&mut w, TAG, t as u32);
                    if find(p, t).is_none() {
                        t = t.wrapping_sub(1);
                        wr(&mut w, TAG, t as u32);
                        break;
                    }
                }
            } else {
                wr(&mut w, TAG, 0);
            }
        }
    }
    Some(w)
}

/// Stepping onto a selectable element whose flags have bits other than 8:
/// the lookup already skips bit 8, so only this shows a wrong flag test.
#[test]
fn func_8000CC1C_step_flags() {
    for (i, (d, flags)) in [(1.0f32, 0x200u16), (-1.0, 0x200), (1.0, 0xFEFF), (-1.0, 0)].into_iter().enumerate() {
        // The current tag (2) must itself be selectable, or it returns first.
        let p = Pods { elems: vec![(0, 0), (1, flags), (2, 0), (3, flags), (4, 0x100)], tag: 2, k: 0, x: d, fields: vec![0.0; 40], id: TEST };
        let mut s = state(i as u64);
        put_pods(&mut s, &p);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (0, sext(d.to_bits()));
        let want = tuning(&s, &p).unwrap();
        let after = compare("func_8000CC1C", misc::func_8000CC1C, &s).unwrap_or_else(|e| panic!("{i}: {e}"));
        same_memory(&after, &want).unwrap_or_else(|e| panic!("{i}: {e}"));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn func_8000CC1C(seed: u64, p in pods(), ks in prop::array::uniform6(ordinary())) {
        let mut s = state(seed);
        put_pods(&mut s, &p);
        for (i, v) in ks.iter().enumerate() {
            if i % 2 == 0 {
                // Perturb the ROM's constants sometimes (odd i: keep them).
                wf(&mut s, 0x800A_85F0 + 4 * i as u32, *v);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (u64::from(p.k), sext(p.x.to_bits()));
        let want = tuning(&s, &p);
        prop_assume!(want.is_some());
        let after = run("func_8000CC1C", misc::func_8000CC1C, &s)?;
        same_memory(&after, &want.unwrap())?;
    }
}

// ---- func_8000D5EC / func_8000D7DC / func_8000D9A8: settings and ids ----

const FLAGS: u32 = 0x8009_B7D8;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(768))]

    #[test]
    fn func_8000D5EC(seed: u64, k in prop_oneof![4 => 0u64..9, 1 => 9u64..12, 1 => any::<u64>()], x in ordinary(), flags: u32,
                     vals in prop::array::uniform4(prop_oneof![3 => 0.0f32..300.0, 1 => ordinary()]), int0 in prop_oneof![0i32..8, any::<i32>()], toggles in prop::array::uniform3(prop_oneof![Just(0u32), Just(1u32), any::<u32>()])) {
        let mut s = state(seed);
        wr(&mut s, FLAGS, flags);
        wr(&mut s, 0x8009_B7D0, int0 as u32);
        let floats = [0x800A_5B64u32, 0x800A_5B68, 0x800A_5B54, 0x800A_5B58];
        for (a, v) in floats.iter().zip(vals.iter()) {
            wf(&mut s, *a, *v);
        }
        wr(&mut s, 0x800A_52D4, toggles[0]);
        wr(&mut s, 0x800A_52D0, toggles[1]);
        wr(&mut s, 0x800D_697C, toggles[2]);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (k, sext(x.to_bits()));
        let mut want = s.clone();
        saves(&mut want, &s, 0x20, &[(0x1C, RA)]);
        let ranges = [(0.001f32, 0.2f32, 2.0f32, 8u32), (0.5, 2.0, 200.0, 8), (1.0, 20.0, 1000.0, 8), (1.0, 20.0, 500.0, 8)];
        let lits = [f32::from_bits(0x3A83_126F), f32::from_bits(0x3E4C_CCCD)];
        match k {
            0 => {
                let sum = (|| add(int0 as f32, mul(x, 1.0)?))();
                prop_assume!(sum.is_some());
                let mut v = fpu::trunc_w_s(sum.unwrap()) as i32;
                wr(&mut want, 0x8009_B7D0, v as u32);
                if v < 0 {
                    v = 0;
                    wr(&mut want, 0x8009_B7D0, 0);
                }
                if 6 < v {
                    wr(&mut want, 0x8009_B7D0, 6);
                }
                wr(&mut want, SP_AT - 0x20 + 0x10, 6);
            }
            1 if flags & 4 != 0 => wr(&mut want, 0x800A_52D4, u32::from(toggles[0] == 0)),
            6 if flags & 0x10 != 0 => wr(&mut want, 0x800A_52D0, u32::from(toggles[1] == 0)),
            7 if flags & 0x20 != 0 => wr(&mut want, 0x800D_697C, toggles[2] ^ 0x4000),
            2..=5 if flags & 8 != 0 => {
                let i = k as usize - 2;
                let (y, lo, hi, _) = ranges[i];
                let (y, lo) = if i == 0 { (lits[0], lits[1]) } else { (y, lo) };
                let v = clamp_add(vals[i], x, y, lo, hi);
                prop_assume!(v.is_some());
                wf(&mut want, floats[i], v.unwrap());
                wf(&mut want, SP_AT - 0x20 + 0x10, hi);
                wf(&mut want, SP_AT - 0x20 + 0xC, lo);
            }
            _ => {}
        }
        let after = run("func_8000D5EC", misc::func_8000D5EC, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8000D7DC(seed: u64, k in prop_oneof![Just(8u64), any::<u64>()], flags: u32, cur in 0u32..3, depth in 0u32..6, saved: [u32; 3], word0: u32) {
        let mut s = state(seed);
        wr(&mut s, FLAGS, flags);
        wr(&mut s, CURRENT_ID, cur);
        wr(&mut s, CURRENT_WORD, word0);
        wr(&mut s, ID_DEPTH, depth);
        for (i, v) in saved.iter().enumerate() {
            wr(&mut s, SAVED_WORDS + 4 * i as u32, *v);
        }
        s.ctx.gpr[A0] = k;
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA)]);
        if k == 8 && flags & 2 != 0 {
            // func_8000C5F0(1).
            wr(&mut want, SAVED_WORDS + 4 * cur, word0);
            wr(&mut want, ID_DEPTH, depth + 1);
            wr(&mut want, ID_STACK + 4 * (depth + 1), 1);
            wr(&mut want, CURRENT_ID, 1);
            let w1 = word(&want, SAVED_WORDS + 4);
            wr(&mut want, CURRENT_WORD, w1);
        }
        let after = run("func_8000D7DC", misc::func_8000D7DC, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8000D9A8(seed: u64, saved0 in prop_oneof![Just((-2i32) as u32), any::<u32>()], flags6: u16, n in 1u32..4, other in any::<bool>(), junk: [u32; 4]) {
        let mut s = state(seed);
        wr(&mut s, SAVED_WORDS, saved0);
        wr(&mut s, 0x8009_B7E4, junk[0]);
        wr(&mut s, ID_DEPTH, junk[1]);
        wr(&mut s, CURRENT_ID, junk[2]);
        wr(&mut s, CURRENT_WORD, junk[3]);
        // A pool registry with (maybe) another pool first, then "Jdge".
        let (reg, d0, d1, el) = (0x8030_0000u32, 0x8030_0100u32, 0x8030_0200u32, 0x8030_1000u32);
        wr(&mut s, POOLS, reg);
        let mut ds = vec![];
        if other {
            ds.push(d0);
        }
        ds.push(d1);
        for (i, d) in ds.iter().enumerate() {
            wr(&mut s, reg + 4 * i as u32, *d);
        }
        wr(&mut s, reg + 4 * ds.len() as u32, 0);
        wr(&mut s, d0, 0x506F_6473);
        wr(&mut s, d1, 0x4A64_6765);
        wr(&mut s, d1 + 8, n);
        wr(&mut s, d1 + 0xC, 0x40);
        wr(&mut s, d1 + 0x10, el);
        wh(&mut s, el + 6, flags6);
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA)]);
        if saved0 == (-2i32) as u32 {
            wr(&mut want, CURRENT_ID, u32::MAX);
        }
        wr(&mut want, CURRENT_WORD, saved0);
        wr(&mut want, 0x8009_B7E4, 0);
        wr(&mut want, ID_DEPTH, 0);
        // func_8003F800("Jdge", 0): the current pool and index.
        wr(&mut want, 0x800A_4AA4, d1);
        wr(&mut want, 0x8011_8D10, 0);
        let id = if flags6 & 0x1000 != 0 {
            0
        } else {
            wr(&mut want, CURRENT_WORD, 0);
            2
        };
        wr(&mut want, CURRENT_ID, id);
        wr(&mut want, ID_STACK, id);
        let after = run("func_8000D9A8", misc::func_8000D9A8, &s)?;
        same_memory(&after, &want)?;
    }
}
