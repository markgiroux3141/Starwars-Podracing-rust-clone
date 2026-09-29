//! Depth 1 at 0x800005B4..0x80004704: the point-in-triangle test
//! (game::math) and the collision query's setup, model-space transforms and
//! edge test (game::collide). Recompiled C vs Rust with the callees as C,
//! each checked against its statement by a model that computes the callees'
//! results from their own statements (NOTES, "Depth-0 leaves ...").
//!
//! NaN operands of arithmetic are outside the domain (NAN_CHECK in the
//! oracle, the callees' included); models return None for inputs that would
//! reach one, skipped before the C runs.

// Tests are named after the functions (func_800005B4), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::collide::{self, MODE, TRACKED};
use game::math;
use game::matrix::{MATRIX_DEPTH, MATRIX_STACK};
use game::recomp::{reg::*, RecompFn};
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

/// The ROM image's word at `vaddr` (data segment), for the game's constants.
fn rom_word(vaddr: u32) -> u32 {
    let o = (vaddr - 0x8000_0400 + 0x1000) as usize;
    u32::from_be_bytes(baserom()[o..o + 4].try_into().unwrap())
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

// ---- Guarded arithmetic: None when an operand is NaN ----

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
fn vsub(a: [f32; 3], b: [f32; 3]) -> Option<[f32; 3]> {
    Some([sub(a[0], b[0])?, sub(a[1], b[1])?, sub(a[2], b[2])?])
}

// ---- The callees' statements ----

/// func_80015538: `(a.y*b.z - b.y*a.z, a.z*b.x - b.z*a.x, a.x*b.y - b.x*a.y)`.
fn cross(a: [f32; 3], b: [f32; 3]) -> Option<[f32; 3]> {
    Some([
        sub(mul(a[1], b[2])?, mul(b[1], a[2])?)?,
        sub(mul(a[2], b[0])?, mul(b[2], a[0])?)?,
        sub(mul(a[0], b[1])?, mul(b[0], a[1])?)?,
    ])
}

/// `z*w + (x*u + y*v)`, the games's dot-product shape.
fn dot(zw: (f32, f32), xu: (f32, f32), yv: (f32, f32)) -> Option<f32> {
    add(mul(zw.0, zw.1)?, add(mul(xu.0, xu.1)?, mul(yv.0, yv.1)?)?)
}

/// func_80081A2C: the point of `a..b` closest to `p`.
fn closest(p: [f32; 3], a: [f32; 3], b: [f32; 3], e: f64) -> Option<[f32; 3]> {
    let d = vsub(b, a)?;
    let pd = dot((p[2], d[2]), (p[0], d[0]), (p[1], d[1]))?;
    let ad = dot((d[2], a[2]), (a[0], d[0]), (a[1], d[1]))?;
    let dd = dot((d[2], d[2]), (d[0], d[0]), (d[1], d[1]))?;
    if f64::from(ok(dd)?) <= e {
        return Some(a);
    }
    let u = sub(pd, ad)?;
    let t = div(u, dd)?;
    if t <= 0.0 {
        return Some(a);
    }
    if 1.0 <= t {
        return Some(b);
    }
    let q = div(u, dd)?;
    Some([add(mul(d[0], q)?, a[0])?, add(mul(d[1], q)?, a[1])?, add(mul(d[2], q)?, a[2])?])
}

/// func_8000097C's decision: whether it records (with `p`, `q`, `r` its
/// arguments, `S` the stored direction).
fn records(t: f32, big_t: f32, e: f32, p: [f32; 3], q: [f32; 3], sdir: [f32; 3], r: [f32; 3]) -> Option<bool> {
    if !(sub(big_t, e)? < t) {
        return Some(true);
    }
    let d = vsub(p, q)?;
    // The two sums are only compared, so they may be NaN themselves.
    let dot1 = add(mul(sdir[2], d[2])?, add(mul(d[0], sdir[0])?, mul(d[1], sdir[1])?)?)?;
    let dot2 = add(add(mul(d[0], r[0])?, mul(d[1], r[1])?)?, mul(d[2], r[2])?)?;
    Some(dot1 < dot2)
}

type Mtx = [[f32; 4]; 4];

/// func_800160BC: the inverse of a scaled rotation plus translation.
fn inverse(m: &Mtx) -> Option<Mtx> {
    let mut o = [[0.0f32; 4]; 4];
    for i in 0..3 {
        let s = dot((m[i][2], m[i][2]), (m[i][0], m[i][0]), (m[i][1], m[i][1]))?;
        for j in 0..3 {
            o[j][i] = div(m[i][j], s)?;
        }
    }
    o[3][3] = 1.0;
    let t = m[3];
    for j in 0..3 {
        o[3][j] = neg(dot((o[2][j], t[2]), (t[0], o[0][j]), (t[1], o[1][j]))?)?;
    }
    Some(o)
}

/// func_80016CAC: `m[3][j] + ((m[0][j]*v.x + m[1][j]*v.y) + m[2][j]*v.z)`.
fn point(v: [f32; 3], m: &Mtx) -> Option<[f32; 3]> {
    let c = |j: usize| add(m[3][j], add(add(mul(m[0][j], v[0])?, mul(m[1][j], v[1])?)?, mul(m[2][j], v[2])?)?);
    Some([c(0)?, c(1)?, c(2)?])
}

/// func_80016BF4: `v.z*m[2][j] + (m[0][j]*v.x + m[1][j]*v.y)`.
fn vec3x3(v: [f32; 3], m: &Mtx) -> Option<[f32; 3]> {
    let c = |j: usize| dot((v[2], m[2][j]), (m[0][j], v[0]), (m[1][j], v[1]));
    Some([c(0)?, c(1)?, c(2)?])
}

fn put_matrix(s: &mut State, depth: u32, m: &Mtx) {
    s.rdram.mem().write_u32(MATRIX_DEPTH, depth);
    let flat: Vec<f32> = m.iter().flatten().copied().collect();
    put(s, MATRIX_STACK + 64 * depth, &flat);
}

// ---- Strategies ----

/// Non-NaN floats: signed zeros, subnormals, ones, halves, small integers,
/// ordinary full-mantissa values, huge values, infinities, any bits.
fn float() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        Just(-1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        (-8i32..8).prop_map(|n| n as f32),
        -1.0e4f32..1.0e4f32,
        -1.0f32..1.0f32,
        Just(1.0e19f32),
        Just(f32::MAX),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

/// Mostly values whose products and sums stay finite, some small integers
/// (exact arithmetic, so ties happen) and signed zeros.
fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![
        4 => -1.0e3f32..1.0e3f32,
        2 => -1.0f32..1.0f32,
        3 => (-3i32..=3).prop_map(|n| n as f32),
        1 => Just(-0.0f32),
        1 => float(),
    ]
    .boxed()
}

fn vec3() -> BoxedStrategy<[f32; 3]> {
    prop_oneof![
        1 => prop::array::uniform3(float()),
        4 => prop::array::uniform3(ordinary()),
        2 => prop::array::uniform3((-2i32..=2).prop_map(|n| n as f32)),
    ]
    .boxed()
}

/// Matrices: a rotation about two axes times a scale plus a translation (the
/// shape func_800160BC inverts), small-integer ones, or anything ordinary.
fn matrix() -> BoxedStrategy<Mtx> {
    let rst = (0.0f32..6.3, 0.0f32..6.3, prop_oneof![Just(1.0f32), 0.25f32..4.0], prop::array::uniform3(ordinary())).prop_map(|(a, b, k, t)| {
        let (sa, ca, sb, cb) = (a.sin(), a.cos(), b.sin(), b.cos());
        [[k * ca, k * sa * cb, k * sa * sb, 0.0], [-k * sa, k * ca * cb, k * ca * sb, 0.0], [0.0, -k * sb, k * cb, 0.0], [t[0], t[1], t[2], 1.0]]
    });
    let ints = prop::array::uniform16((-2i32..=2).prop_map(|n| n as f32));
    let any16 = prop::array::uniform16(ordinary());
    prop_oneof![4 => rst, 1 => ints.prop_map(unflat), 1 => any16.prop_map(unflat)].boxed()
}

fn unflat(v: [f32; 16]) -> Mtx {
    let mut m = [[0.0f32; 4]; 4];
    for (i, x) in v.iter().enumerate() {
        m[i / 4][i % 4] = *x;
    }
    m
}

// ---- func_800005B4: point in triangle ----

const K1_AT: u32 = 0x800A_80F0;
const K2_AT: u32 = 0x800A_80F4;
const PTS: [u32; 7] = [0x8030_0000, 0x8030_0010, 0x8030_0020, 0x8030_0030, 0x8030_0040, 0x8030_0050, 0x8030_0060];

fn abs(x: f32) -> Option<f32> {
    if x < 0.0 {
        neg(x)
    } else {
        Some(x)
    }
}

/// `s(c)` and the absolute values.
fn size(c: [f32; 3]) -> Option<(f32, [f32; 3])> {
    let a = [abs(c[0])?, abs(c[1])?, abs(c[2])?];
    Some((add(add(a[0], a[1])?, a[2])?, a))
}

fn dominant(a: [f32; 3]) -> usize {
    if !(a[0] < a[1]) {
        if a[0] < a[2] {
            2
        } else {
            0
        }
    } else if !(a[1] < a[2]) {
        1
    } else {
        2
    }
}

/// The three edge crosses (None: outside the domain; Some(None): `u x v`
/// is zero).
fn crosses(v: &[[f32; 3]; 7]) -> Option<Option<[[f32; 3]; 3]>> {
    let [p, a, b, c, u, vv, w] = *v;
    let (e1, e2, e3) = (vsub(a, p)?, vsub(b, p)?, vsub(c, p)?);
    let n = cross(u, vv)?;
    if n.iter().all(|&x| x == 0.0) {
        return Some(None);
    }
    Some(Some([cross(e1, u)?, cross(e2, vv)?, cross(e3, w)?]))
}

fn in_triangle(v: &[[f32; 3]; 7], k1: f32, k2: f32) -> Option<u32> {
    let Some([c1, c2, c3]) = crosses(v)? else { return Some(0) };
    let (s1, a1) = size(c1)?;
    if k1 < s1 {
        let i = dominant(a1);
        return Some((if f64::from(c1[i]) < 0.0 { c2[i] <= 0.0 && c3[i] <= 0.0 } else { 0.0 <= c2[i] && 0.0 <= c3[i] }) as u32);
    }
    let (s2, a2) = size(c2)?;
    if s2 < k2 {
        return Some(1);
    }
    let i = dominant(a2);
    Some((if f64::from(c2[i]) < 0.0 { c3[i] <= 0.0 } else { 0.0 <= c3[i] }) as u32)
}

/// The seven vectors: a triangle `a b c` with its edges as `u v w` and `p`
/// inside, on it or anywhere; the same with a degenerate edge pair; or
/// seven arbitrary vectors.
fn triangle_case() -> BoxedStrategy<[[f32; 3]; 7]> {
    let tri = (vec3(), vec3(), vec3(), 0.0f32..1.0, 0.0f32..1.0, 0u8..6, vec3()).prop_filter_map("in domain", |(a, b, c, s, t, mode, any)| {
        let (u, v, w) = (vsub(b, a)?, vsub(c, b)?, vsub(a, c)?);
        let (s, t) = if s + t > 1.0 { (1.0 - s, 1.0 - t) } else { (s, t) };
        let p = match mode {
            0 | 1 => [0, 1, 2].map(|i| a[i] + s * (b[i] - a[i]) + t * (c[i] - a[i])),
            2 => a,
            3 => [0, 1, 2].map(|i| a[i] + s * (b[i] - a[i])),
            _ => any,
        };
        Some([p, a, b, c, u, v, w])
    });
    let flat = (vec3(), vec3(), vec3(), vec3(), vec3(), vec3(), vec3()).prop_map(|(a, b, c, d, e, f, g)| [a, b, c, d, e, f, g]);
    let degenerate = (vec3(), vec3(), vec3(), vec3(), vec3(), -2i32..=2).prop_map(|(p, a, b, c, u, k)| [p, a, b, c, u, u.map(|x| x * k as f32), u]);
    prop_oneof![5 => tri, 2 => flat, 1 => degenerate].boxed()
}

/// Thresholds: the ROM's, zero, negative (so an all-zero c1 still takes
/// the first test), or tied with `s(c1)` / `s(c2)` (`tie` 1 and 2).
fn threshold() -> BoxedStrategy<(u8, f32)> {
    prop_oneof![4 => Just((0u8, f32::NAN)), 1 => Just((3, 0.0f32)), 1 => Just((3, -1.0f32)), 1 => ordinary().prop_map(|x| (3, x)), 2 => Just((1, 0.0)), 2 => Just((2, 0.0))].boxed()
}

// ---- func_80000B00: nearest edge point ----

const EDGE_E_AT: u32 = 0x800A_DCC0;
const REC_E_AT: u32 = 0x800A_80F8;

fn rom_double(vaddr: u32) -> f64 {
    f64::from_bits(u64::from(rom_word(vaddr)) << 32 | u64::from(rom_word(vaddr + 4)))
}

#[derive(Clone, Debug)]
struct Edges {
    /// p, a, b, c, q, r.
    v: [[f32; 3]; 6],
    big_t: f32,
    sdir: [f32; 3],
    e_edge: f64,
    e_rec: f32,
    tail: [u32; 2],
}

/// Which point is offered and whether it's recorded: (D, k) and the
/// decision, or None if nothing is offered.
fn edges_model(x: &Edges) -> Option<Option<(f32, [f32; 3], bool)>> {
    let [p, a, b, c, q, r] = x.v;
    vsub(b, a)?;
    vsub(c, b)?;
    vsub(a, c)?;
    let ks = [closest(p, a, b, x.e_edge)?, closest(p, b, c, x.e_edge)?, closest(p, c, a, x.e_edge)?];
    let dist = |k: [f32; 3]| -> Option<f32> {
        let d = vsub(q, k)?;
        dot((d[2], d[2]), (d[0], d[0]), (d[1], d[1]))
    };
    let (mut best, mut k) = (dist(ks[0])?, ks[0]);
    for &kk in &ks[1..] {
        let dd = dist(kk)?;
        if dd < best {
            (best, k) = (dd, kk);
        }
    }
    if !(best <= x.big_t) {
        return Some(None);
    }
    Some(Some((best, k, records(best, x.big_t, x.e_rec, q, k, x.sdir, r)?)))
}

fn edges_case() -> BoxedStrategy<Edges> {
    let offset = prop::array::uniform3(prop_oneof![Just(0.0f32), -1.0f32..1.0]);
    (prop::array::uniform6(vec3()), prop_oneof![2 => Just(f32::INFINITY), 1 => Just(1.0e30f32), 2 => ordinary(), 1 => float()], vec3(), 0u8..4, any::<[u32; 2]>(), 0u8..5, offset)
        .prop_map(|(v, big_t, sdir, e, tail, near, off)| {
            let mut v = v;
            if near >= 2 {
                // p at an edge's midpoint and q near it, so that edge's
                // point is (usually) the nearest: each edge wins in turn.
                let (i, j) = [(1, 2), (2, 3), (3, 1)][near as usize - 2];
                v[0] = [0, 1, 2].map(|c| (v[i][c] + v[j][c]) * 0.5);
                v[4] = [0, 1, 2].map(|c| v[0][c] + off[c]);
            }
            let e_edge = match e {
                0 | 1 => rom_double(EDGE_E_AT),
                2 => 0.0,
                _ => 1.0,
            };
            let e_rec = if e == 3 { 0.0 } else { f32::from_bits(rom_word(REC_E_AT)) };
            Edges { v, big_t, sdir, e_edge, e_rec, tail }
        })
        .boxed()
}

// ---- func_800038E8 / func_80003B44 / func_80004704: model space ----

const P_AT: u32 = 0x8030_1000;
const POUT: u32 = 0x8030_1010;
const N_AT: u32 = 0x8030_1020;
const NOUT: u32 = 0x8030_1030;

/// pout, and with mode 3 (nout, [0x800AE954], [0x800AE958]).
fn to_model(flags: u32, p: [f32; 3], n: [f32; 3], m: &Mtx, mode: i16, d: (f32, f32)) -> Option<([f32; 3], Option<([f32; 3], f32, f32)>)> {
    if flags & 1 == 0 {
        return Some((p, (mode == 3).then_some((n, d.0, d.1))));
    }
    if flags & 2 == 0 {
        let t = [m[3][0], m[3][1], m[3][2]];
        let pout = vsub(p, t)?;
        if mode != 3 {
            return Some((pout, None));
        }
        let e = dot((t[2], n[2]), (n[0], t[0]), (n[1], t[1]))?;
        return Some((pout, Some((n, sub(d.0, e)?, sub(d.1, e)?))));
    }
    let inv = inverse(m)?;
    let pout = point(p, &inv)?;
    if mode != 3 {
        return Some((pout, None));
    }
    let nout = vec3x3(n, &inv)?;
    let u = inv[3];
    let e = dot((u[2], nout[2]), (nout[0], u[0]), (nout[1], u[1]))?;
    Some((pout, Some((nout, add(e, d.0)?, add(e, d.1)?))))
}

fn mode() -> BoxedStrategy<i16> {
    prop_oneof![4 => Just(3i16), 2 => Just(2i16), 1 => any::<i16>()].boxed()
}

fn flags() -> BoxedStrategy<u32> {
    prop_oneof![4 => 0u32..4, 1 => any::<u32>()].boxed()
}

// ---- func_80004704: records ----

const IN: u32 = 0x8030_2000;
const OUT: u32 = 0x8030_2400;

fn records_model(flags: u32, recs: &[[f32; 7]], m: &Mtx) -> Option<Vec<[u32; 7]>> {
    let t = [m[3][0], m[3][1], m[3][2]];
    let inv = if flags & 3 == 3 { Some(inverse(m)?) } else { None };
    recs.iter()
        .map(|r| {
            let (p, v) = ([r[0], r[1], r[2]], [r[3], r[4], r[5]]);
            let (p2, v2) = if flags & 1 == 0 {
                (p, v)
            } else if flags & 2 == 0 {
                (vsub(p, t)?, v)
            } else {
                let i = inv.as_ref().unwrap();
                (point(p, i)?, vec3x3(v, i)?)
            };
            Some([p2[0], p2[1], p2[2], v2[0], v2[1], v2[2], 0.0].map(f32::to_bits))
        })
        .collect()
}

// ---- func_80004000: query setup ----

const C_AT: u32 = 0x8030_3000;
const NRM_AT: u32 = 0x8030_3010;
const K_AT: u32 = 0x800A_8138;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(768))]

    #[test]
    fn func_800005B4(seed: u64, v in triangle_case(), (t1, k1) in threshold(), (t2, k2) in threshold()) {
        let (mut k1, mut k2) = (k1, k2);
        let rom = (f32::from_bits(rom_word(K1_AT)), f32::from_bits(rom_word(K2_AT)));
        let cs = crosses(&v);
        prop_assume!(cs.is_some());
        let cs = cs.unwrap();
        // Tie a threshold with s(c1) or s(c2) where it exists.
        let tie = |t: u8, k: f32, fallback: f32| match (t, cs) {
            (0, _) => fallback,
            (1, Some(c)) => size(c[0]).map_or(fallback, |x| x.0),
            (2, Some(c)) => size(c[1]).map_or(fallback, |x| x.0),
            (3, _) => k,
            _ => fallback,
        };
        k1 = tie(t1, k1, rom.0);
        k2 = tie(t2, k2, rom.1);
        let want = in_triangle(&v, k1, k2);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        for (a, x) in PTS.iter().zip(v.iter()) {
            put(&mut s, *a, x);
        }
        put(&mut s, K1_AT, &[k1]);
        put(&mut s, K2_AT, &[k2]);
        for (i, a) in PTS[4..].iter().enumerate() {
            s.rdram.mem().write_u32(SP_AT + 0x10 + 4 * i as u32, *a);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(PTS[0]), sext(PTS[1]), sext(PTS[2]), sext(PTS[3]));
        let after = run("func_800005B4", math::func_800005B4, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(want.unwrap()));
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }

    #[test]
    fn func_80000B00(seed: u64, x in edges_case(), tie_t: bool) {
        let mut x = x;
        if tie_t {
            // T equal to the best distance: `D <= T` holds at the tie.
            if let Some(Some((d, _, _))) = edges_model(&x) {
                x.big_t = d;
            }
        }
        let want = edges_model(&x);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        let at = |i: usize| 0x8030_0000 + 0x10 * i as u32;
        for (i, v) in x.v.iter().enumerate() {
            put(&mut s, at(i), v);
        }
        put(&mut s, TRACKED, &[x.big_t]);
        put(&mut s, TRACKED + 0x18, &x.sdir);
        put(&mut s, REC_E_AT, &[x.e_rec]);
        {
            let mut m = s.rdram.mem();
            m.write_u32(EDGE_E_AT, (x.e_edge.to_bits() >> 32) as u32);
            m.write_u32(EDGE_E_AT + 4, x.e_edge.to_bits() as u32);
            m.write_u32(0x800A_E938, x.tail[0]);
            m.write_u32(0x800A_EC7C, x.tail[1]);
            m.write_u32(SP_AT + 0x10, at(4));
            m.write_u32(SP_AT + 0x14, at(5));
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(at(0)), sext(at(1)), sext(at(2)), sext(at(3)));
        let after = run("func_80000B00", collide::func_80000B00, &s)?;
        match want.unwrap() {
            Some((d, k, true)) => {
                prop_assert_eq!(word(&after, TRACKED), d.to_bits());
                prop_assert_eq!(get3(&after, TRACKED + 8), bits3(k));
                prop_assert_eq!(get3(&after, TRACKED + 0x18), bits3(x.v[5]));
                prop_assert_eq!(word(&after, 0x800A_E8D8), x.tail[0]);
                prop_assert_eq!(word(&after, 0x800A_EC7C), 1);
            }
            _ => {
                for a in (TRACKED..TRACKED + 0x2C).step_by(4).chain([0x800A_EC7C]) {
                    prop_assert_eq!(word(&after, a), word(&s, a));
                }
            }
        }
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }

    #[test]
    fn func_800038E8(seed: u64, fl in flags(), p in vec3(), n in vec3(), m in matrix(), depth in 0u32..=32, md in mode(),
                     d1 in ordinary(), d2 in ordinary(), alias in 0u8..3, junk: [u32; 8]) {
        let want = to_model(fl, p, n, &m, md, (d1, d2));
        prop_assume!(want.is_some());
        let (pout_want, plane) = want.unwrap();
        let mut s = state(seed);
        put_matrix(&mut s, depth, &m);
        let (pout, nout) = match alias {
            0 => (POUT, NOUT),
            1 => (P_AT, N_AT),
            _ => (POUT, N_AT),
        };
        {
            let mut mm = s.rdram.mem();
            for (i, w) in junk.iter().enumerate() {
                mm.write_u32(0x8030_1010 + 4 * i as u32, *w);
            }
            mm.write_u16(MODE, md as u16);
            mm.write_u32(SP_AT + 0x10, N_AT);
        }
        put(&mut s, P_AT, &p);
        put(&mut s, N_AT, &n);
        put(&mut s, 0x800A_E96C, &[d1, d2]);
        let before = (word(&s, 0x800A_E954), word(&s, 0x800A_E958));
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (u64::from(fl), sext(pout), sext(P_AT), sext(nout));
        let after = run("func_800038E8", collide::func_800038E8, &s)?;
        prop_assert_eq!(get3(&after, pout), bits3(pout_want));
        match plane {
            Some((nw, r1, r2)) => {
                prop_assert_eq!(get3(&after, nout), bits3(nw));
                prop_assert_eq!((word(&after, 0x800A_E954), word(&after, 0x800A_E958)), (r1.to_bits(), r2.to_bits()));
            }
            None => {
                prop_assert_eq!(get3(&after, nout), get3(&s, nout));
                prop_assert_eq!((word(&after, 0x800A_E954), word(&after, 0x800A_E958)), before);
            }
        }
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }

    #[test]
    fn func_80003B44(seed: u64, fl in flags(), pp in vec3(), nn in vec3(), m in matrix(), depth in 0u32..=32, md in mode()) {
        let t = [m[3][0], m[3][1], m[3][2]];
        let want = if fl & 1 == 0 {
            Some((pp, nn))
        } else if fl & 2 == 0 {
            (|| Some(([add(t[0], pp[0])?, add(t[1], pp[1])?, add(t[2], pp[2])?], nn)))()
        } else {
            (|| Some((point(pp, &m)?, if md == 2 { nn } else { vec3x3(nn, &m)? })))()
        };
        prop_assume!(want.is_some());
        let (pw, nw) = want.unwrap();
        let mut s = state(seed);
        put_matrix(&mut s, depth, &m);
        put(&mut s, TRACKED + 8, &pp);
        put(&mut s, TRACKED + 0x18, &nn);
        s.rdram.mem().write_u16(MODE, md as u16);
        s.ctx.gpr[A0] = u64::from(fl);
        let after = run("func_80003B44", collide::func_80003B44, &s)?;
        prop_assert_eq!(get3(&after, TRACKED + 8), bits3(pw));
        prop_assert_eq!(get3(&after, TRACKED + 0x18), bits3(nw));
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }

    #[test]
    fn func_80004000(seed: u64, c in vec3(), r in ordinary(), n in vec3(), d1 in ordinary(), d2 in ordinary(),
                     k in prop_oneof![3 => Just(None), 1 => ordinary().prop_map(Some)]) {
        let k = k.unwrap_or(f32::from_bits(rom_word(K_AT)));
        let want = (|| {
            let rr = mul(r, r)?;
            let e = dot((n[2], c[2]), (c[0], n[0]), (c[1], n[1]))?;
            Some((rr, add(e, d1)?, add(e, d2)?, mul(rr, k)?))
        })();
        prop_assume!(want.is_some());
        let (rr, o1, o2, big_t) = want.unwrap();
        let mut s = state(seed);
        put(&mut s, C_AT, &c);
        put(&mut s, NRM_AT, &n);
        put(&mut s, K_AT, &[k]);
        s.rdram.mem().write_u32(SP_AT + 0x10, d2.to_bits());
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(C_AT), sext(r.to_bits()), sext(NRM_AT), sext(d1.to_bits()));
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            let ra = s.ctx.gpr[RA] as u32;
            m.write_u32(SP_AT - 4, ra);
            for (a, v) in [(0x800A_E8E0, r), (0x800A_E8DC, rr), (0x800A_E954, o1), (0x800A_E958, o2), (0x800A_E96C, o1), (0x800A_E970, o2), (TRACKED, big_t)] {
                m.write_u32(a, v.to_bits());
            }
            for i in 0..3 {
                m.write_u32(0x800A_E908 + 4 * i, c[i as usize].to_bits());
                m.write_u32(0x800A_E948 + 4 * i, n[i as usize].to_bits());
                m.write_u32(0x800A_E960 + 4 * i, n[i as usize].to_bits());
            }
            m.write_u32(0x800A_E8D8, 0);
            m.write_u16(MODE, 3);
            m.write_u32(0x800A_E93C, 0x8000_3348);
            m.write_u32(0x800A_E940, 0x8000_2FFC);
            m.write_u32(0x800A_EC78, 0);
            m.write_u32(0x800A_EC7C, 0);
            m.write_u32(MATRIX_DEPTH, 0);
            for i in 0..16u32 {
                m.write_u32(MATRIX_STACK + 4 * i, if i % 5 == 0 { 0x3F80_0000 } else { 0 });
            }
        }
        let after = run("func_80004000", collide::func_80004000, &s)?;
        let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
        if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
            return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
        }
    }

    #[test]
    fn func_80004704(seed: u64, fl in flags(), n in -2i64..=6, recs in prop::collection::vec(prop::array::uniform7(ordinary()), 6),
                     m in matrix(), depth in 0u32..=32, in_place: bool, junk in prop::collection::vec(any::<u32>(), 49)) {
        let count = n.max(0) as usize;
        let want = records_model(fl, &recs[..count], &m);
        prop_assume!(want.is_some());
        let want = want.unwrap();
        let mut s = state(seed);
        put_matrix(&mut s, depth, &m);
        let out = if in_place { IN } else { OUT };
        {
            let mut mm = s.rdram.mem();
            for (i, w) in junk.iter().enumerate() {
                mm.write_u32(OUT + 4 * i as u32, *w);
            }
        }
        for (i, r) in recs.iter().enumerate() {
            put(&mut s, IN + 28 * i as u32, r);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (u64::from(fl), n as u64, sext(out), sext(IN));
        let after = run("func_80004704", collide::func_80004704, &s)?;
        for i in 0..7 {
            for w in 0..7 {
                let a = out + 28 * i as u32 + 4 * w as u32;
                let expect = if i < count && w < 6 { want[i][w] } else { word(&s, a) };
                prop_assert_eq!(word(&after, a), expect, "record {} word {}", i, w);
            }
        }
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP_AT));
    }
}
