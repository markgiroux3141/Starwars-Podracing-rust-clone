//! The float leaves at 0x8000097C..0x80002E2C (game::misc, game::math): a
//! tracked-state update with a dot-product test, a ray/plane intersection
//! and two box rejects. Recompiled C vs Rust over float edge values and
//! ordinary full-mantissa ones, each checked against its statement.
//!
//! NaN operands of arithmetic are outside these ports' domain (NAN_CHECK in
//! the oracle, FCR31.EV on hardware); each model returns None for inputs
//! that would reach one, and those cases are skipped before the C runs.
//! NaN is tested where a value is only compared or copied.

// Tests are named after the functions (func_8000097C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::recomp::{reg::*, RecompFn};
use game::{math, misc};
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

/// The ROM image's word at `vaddr` (data segment: ROM = vaddr - 0x80000400
/// + 0x1000), for the game's own constants.
fn rom_word(vaddr: u32) -> u32 {
    let o = (vaddr - 0x8000_0400 + 0x1000) as usize;
    u32::from_be_bytes(baserom()[o..o + 4].try_into().unwrap())
}

/// Non-NaN floats: signed zeros, subnormals, ones, halves, small integers,
/// ordinary full-mantissa values in the game's range, huge values,
/// infinities, and any other non-NaN bit pattern.
fn float() -> impl Strategy<Value = f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::from_bits(1)),
        Just(-f32::from_bits(0x007F_FFFF)),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        Just(-1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        (-8i32..8).prop_map(|n| n as f32),
        -1.0e4f32..1.0e4f32,
        -1.0e4f32..1.0e4f32,
        -1.0f32..1.0f32,
        Just(1.0e19f32),
        Just(-3.0e38f32),
        Just(f32::MAX),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
}

/// Mostly ordinary values: vectors whose products and sums stay finite,
/// with some small integers (exact sums, so ties happen).
fn ordinary() -> impl Strategy<Value = f32> {
    prop_oneof![
        3 => -1.0e4f32..1.0e4f32,
        1 => -1.0f32..1.0f32,
        2 => (-8i32..8).prop_map(|n| n as f32),
        1 => float(),
    ]
}

fn vec3() -> impl Strategy<Value = [f32; 3]> {
    prop_oneof![[float(), float(), float()], [ordinary(), ordinary(), ordinary()]]
}

/// Any bits, NaNs included (for values that are only compared or copied).
fn any_bits() -> impl Strategy<Value = u32> {
    prop_oneof![
        float().prop_map(f32::to_bits),
        (1u32..0x0080_0000).prop_map(|p| 0x7F80_0000 | p),
        (1u32..0x0080_0000).prop_map(|p| 0xFF80_0000 | p),
        any::<u32>(),
    ]
}

fn put(s: &mut State, a: u32, v: &[f32]) {
    let mut m = s.rdram.mem();
    for (i, x) in v.iter().enumerate() {
        m.write_u32(a + 4 * i as u32, x.to_bits());
    }
}

fn get3(s: &State, a: u32) -> [u32; 3] {
    [word(s, a), word(s, a + 4), word(s, a + 8)]
}

fn bits3(v: [f32; 3]) -> [u32; 3] {
    v.map(f32::to_bits)
}

/// `x`, or None (outside the domain) if it is a NaN reaching another
/// guarded operation.
fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}

// ---------------------------------------------------------------------------
// func_8000097C: tracked-state update

const TRACKED: u32 = 0x800A_E8B0;
const E_AT: u32 = 0x800A_80F8;
const P: u32 = 0x8030_0000;
const Q: u32 = 0x8030_0100;
const R: u32 = 0x8030_0200;

/// Whether it records, and the differences it spills on the dot path.
fn tracked_model(t: f32, big_t: f32, e: f32, p: [f32; 3], q: [f32; 3], sdir: [f32; 3], r: [f32; 3]) -> Option<(bool, Option<[f32; 3]>)> {
    if !(big_t - e < t) {
        return Some((true, None));
    }
    let d = [ok(p[0] - q[0])?, ok(p[1] - q[1])?, ok(p[2] - q[2])?];
    let s01 = ok(ok(d[0] * sdir[0])? + ok(d[1] * sdir[1])?)?;
    let dot1 = ok(sdir[2] * d[2])? + s01;
    let r01 = ok(ok(d[0] * r[0])? + ok(d[1] * r[1])?)?;
    let dot2 = r01 + ok(d[2] * r[2])?;
    Some((dot1 < dot2, Some(d)))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(768))]

    /// `mode` 0: `T`, `E`, `t` as drawn; 1: `T = E = 0` and `t > 0`, so the
    /// dot test runs; 2: as 1 with `S = r` (the two sums differ only in
    /// their order); 3: as 1 with the ROM's `E`; 4: `t = T - E` exactly.
    #[test]
    fn func_8000097C(seed: u64, t_bits in any_bits(), big_t in float(), e in float(),
                     p in vec3(), q in vec3(), sdir in vec3(), r in vec3(), mode in 0u8..5,
                     tail: [u32; 2]) {
        let mut s = state(seed);
        let (mut t, mut big_t, mut e, mut sdir) = (f32::from_bits(t_bits), big_t, e, sdir);
        if mode == 4 {
            t = big_t - e;
        } else if mode >= 1 {
            big_t = 0.0;
            e = if mode == 3 { f32::from_bits(rom_word(E_AT)) } else { 0.0 };
            t = if t.is_nan() || t <= 0.0 { 1.0 } else { t };
            big_t += e; // T - E = 0 < t
        }
        if mode == 2 {
            sdir = r;
        }
        let want = tracked_model(t, big_t, e, p, q, sdir, r);
        prop_assume!(want.is_some());
        let (records, diffs) = want.unwrap();
        s.ctx.fpr[12].set_u32l(t.to_bits());
        put(&mut s, TRACKED, &[big_t]);
        put(&mut s, TRACKED + 0x18, &sdir);
        put(&mut s, E_AT, &[e]);
        {
            let mut m = s.rdram.mem();
            m.write_u32(0x800A_E938, tail[0]);
            m.write_u32(0x800A_EC7C, tail[1]);
        }
        put(&mut s, P, &p);
        put(&mut s, Q, &q);
        put(&mut s, R, &r);
        (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(P), sext(Q), sext(R));
        let after = run("func_8000097C", misc::func_8000097C, &s)?;
        if records {
            prop_assert_eq!(word(&after, TRACKED), t.to_bits());
            prop_assert_eq!(get3(&after, TRACKED + 8), bits3(q));
            prop_assert_eq!(get3(&after, TRACKED + 0x18), bits3(r));
            prop_assert_eq!(word(&after, 0x800A_E8D8), tail[0]);
            prop_assert_eq!(word(&after, 0x800A_EC7C), 1);
        } else {
            for a in (TRACKED..TRACKED + 0x2C).step_by(4).chain([0x800A_EC7C]) {
                prop_assert_eq!(word(&after, a), word(&s, a));
            }
        }
        if let Some(d) = diffs {
            prop_assert_eq!(get3(&after, SP_AT - 0xC), bits3(d));
        }
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }
}

// ---------------------------------------------------------------------------
// func_80001D34: ray/plane intersection

const PLANE: u32 = 0x8030_1000;
const RAY: u32 = 0x8030_1100;
const OUT: u32 = 0x8030_1200;
const K_AT: u32 = 0x800A_8128;

/// `(no, nv)` as the code sums them, or None.
fn dots(n: [f32; 3], o: [f32; 3], v: [f32; 3]) -> Option<(f32, f32)> {
    let no = ok(ok(n[2] * o[2])? + ok(ok(o[0] * n[0])? + ok(o[1] * n[1])?)?)?;
    let nv = ok(ok(n[2] * v[2])? + ok(ok(v[0] * n[0])? + ok(v[1] * n[1])?)?)?;
    Some((no, nv))
}

/// `(f0, out)`: the result and, on a hit, the point.
fn ray_model(n: [f32; 3], d: f32, o: [f32; 3], v: [f32; 3], len: f32, k: (f64, f64)) -> Option<(u32, Option<[u32; 3]>)> {
    let miss = Some(((-1.0f32).to_bits(), None));
    let (no, nv) = dots(n, o, v)?;
    let e = f64::from(nv);
    if k.0 <= e && e <= k.1 {
        return miss;
    }
    let t = ok(d - no)? / nv;
    if t < 0.0 || len < t {
        return miss;
    }
    let t = ok(t)?;
    let prod = [ok(v[0] * t)?, ok(v[1] * t)?, ok(v[2] * t)?];
    Some((t.to_bits(), Some([(prod[0] + o[0]).to_bits(), (prod[1] + o[1]).to_bits(), (prod[2] + o[2]).to_bits()])))
}

fn rom_k() -> (f64, f64) {
    let dbl = |a| f64::from_bits(u64::from(rom_word(a)) << 32 | u64::from(rom_word(a + 4)));
    (dbl(K_AT), dbl(K_AT + 8))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// `kmode` 0: the ROM's constants; 1: drawn; 2/3: `K1` (then `K2`) is
    /// exactly `f64(nv)`. `dmode` 1 puts `d = no` (so `t` is a zero).
    /// `lmode` 1 puts `L = t`.
    #[test]
    fn func_80001D34(seed: u64, n in vec3(), d in float(), o in vec3(), v in vec3(),
                     len_bits in any_bits(), kmode in 0u8..4, k in (-1.0e-2f64..1.0e-2, -1.0e-2f64..1.0e-2),
                     dmode in 0u8..3, lmode in 0u8..3, f20: u64) {
        let Some((no, nv)) = dots(n, o, v) else { return Err(TestCaseError::reject("NaN dot")) };
        let e = f64::from(nv);
        let k = match kmode {
            0 => rom_k(),
            1 => (k.0.min(k.1), k.0.max(k.1)),
            2 => (e, f64::INFINITY),
            _ => (f64::NEG_INFINITY, e),
        };
        let d = if dmode == 1 { no } else { d };
        let mut len = f32::from_bits(len_bits);
        if lmode == 1 {
            let t = (d - no) / nv;
            len = t;
        }
        let want = ray_model(n, d, o, v, len, k);
        prop_assume!(want.is_some());
        let (ret, point) = want.unwrap();
        let mut s = state(seed);
        s.ctx.fpr[20].u64 = f20;
        put(&mut s, PLANE, &[n[0], n[1], n[2], d]);
        put(&mut s, RAY, &[o[0], o[1], o[2], v[0], v[1], v[2], len]);
        {
            let mut m = s.rdram.mem();
            for (a, x) in [(K_AT, k.0), (K_AT + 8, k.1)] {
                m.write_u32(a, (x.to_bits() >> 32) as u32);
                m.write_u32(a + 4, x.to_bits() as u32);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(PLANE), sext(RAY), sext(OUT));
        let after = run("func_80001D34", math::func_80001D34, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), ret);
        match point {
            Some(pt) => prop_assert_eq!(get3(&after, OUT), pt),
            None => prop_assert_eq!(get3(&after, OUT), get3(&s, OUT)),
        }
        // f20 saved at sp - 8 (high word first) and restored.
        prop_assert_eq!([word(&after, SP_AT - 8), word(&after, SP_AT - 4)], [(f20 >> 32) as u32, f20 as u32]);
        prop_assert_eq!(after.ctx.fpr[20].u64, f20);
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }
}

// ---------------------------------------------------------------------------
// func_80002BD4 / func_80002E2C: box rejects

const CENTRE: u32 = 0x800A_E908;
const HALF: u32 = 0x800A_E8E0;
const PTS: [u32; 4] = [0x8030_2000, 0x8030_2100, 0x8030_2200, 0x8030_2300];

/// 0 if every point is beyond the same face, else 1 (or None).
fn box_model(c: [f32; 3], h: f32, pts: &[[f32; 3]]) -> Option<(u32, Vec<[f32; 3]>)> {
    let rel: Vec<[f32; 3]> = pts
        .iter()
        .map(|p| Some([ok(p[0] - c[0])?, ok(p[1] - c[1])?, ok(p[2] - c[2])?]))
        .collect::<Option<_>>()?;
    let lo = -h;
    for axis in 0..3 {
        if rel.iter().all(|r| r[axis] < lo) || rel.iter().all(|r| h < r[axis]) {
            return Some((0, rel));
        }
    }
    Some((1, rel))
}

/// A box and points: drawn freely, or on a grid of multiples of `h` around
/// the centre, so points sit exactly on faces and beyond them.
fn box_case(n: usize) -> impl Strategy<Value = ([f32; 3], f32, Vec<[f32; 3]>)> {
    let free = ([float(), float(), float()], float(), prop::collection::vec(vec3(), n));
    let grid = (
        [(-4i32..4), (-4i32..4), (-4i32..4)],
        1i32..4,
        prop::collection::vec([(-3i32..=3), (-3i32..=3), (-3i32..=3)], n),
    )
        .prop_map(|(c, h, ps)| {
            let c = c.map(|x| x as f32 * 8.0);
            let pts = ps.iter().map(|p| [0, 1, 2].map(|i| c[i] + (p[i] * h) as f32)).collect();
            (c, h as f32, pts)
        });
    // Points all beyond one face (the grid's offsets shifted past it).
    let beyond = (
        [(-4i32..4), (-4i32..4), (-4i32..4)],
        1i32..4,
        prop::collection::vec([(-3i32..=3), (-3i32..=3), (-3i32..=3)], n),
        0usize..3,
        any::<bool>(),
        0u8..3,
    )
        .prop_map(|(c, h, ps, axis, high, edge)| {
            let c = c.map(|x| x as f32 * 8.0);
            let pts = ps
                .iter()
                .map(|p| {
                    let mut q = [0, 1, 2].map(|i| c[i] + (p[i] * h) as f32);
                    // edge 0: every point strictly beyond; 1/2: one on the face.
                    let off = h as f32 + 1.0 + p[axis].unsigned_abs() as f32;
                    q[axis] = if high { c[axis] + off } else { c[axis] - off };
                    q
                })
                .enumerate()
                .map(|(k, mut q)| {
                    if edge > 0 && k == 0 {
                        q[axis] = if high { c[axis] + h as f32 } else { c[axis] - h as f32 };
                    }
                    q
                })
                .collect();
            (c, h as f32, pts)
        });
    prop_oneof![free, grid, beyond]
}

fn box_run(name: &str, port: RecompFn, seed: u64, (c, h, pts): ([f32; 3], f32, Vec<[f32; 3]>), frame: u32, slots: &[u32])
    -> Result<(), TestCaseError> {
    let want = box_model(c, h, &pts);
    prop_assume!(want.is_some());
    let (ret, rel) = want.unwrap();
    let mut s = state(seed);
    put(&mut s, CENTRE, &c);
    put(&mut s, HALF, &[h]);
    for (p, a) in pts.iter().zip(PTS) {
        put(&mut s, a, p);
    }
    for (r, a) in [A0, A1, A2, A3].iter().zip(PTS).take(pts.len()) {
        s.ctx.gpr[*r] = sext(a);
    }
    let after = run(name, port, &s)?;
    prop_assert_eq!(after.ctx.gpr[V0], u64::from(ret));
    // The differences, spilled to the frame.
    for (r, off) in rel.iter().zip(slots) {
        prop_assert_eq!(get3(&after, SP_AT - frame + off), bits3(*r));
    }
    prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn func_80002BD4(seed: u64, case in box_case(4)) {
        box_run("func_80002BD4", misc::func_80002BD4, seed, case, 0x30, &[0x24, 0x18, 0xC, 0])?;
    }

    #[test]
    fn func_80002E2C(seed: u64, case in box_case(3)) {
        box_run("func_80002E2C", misc::func_80002E2C, seed, case, 0x28, &[0x1C, 0x10, 0x4])?;
    }
}
