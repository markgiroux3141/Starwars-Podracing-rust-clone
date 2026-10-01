//! Depth 2 at 0x80000D80..0x8000D90C (game::math, collide, anim, misc): a
//! line through two points meeting a plane, the collision query's ray
//! against a triangle, three animation tracks driving a node (translation,
//! scale, a halfword wrapped into a range), two sound requests, a sound's
//! timing and the debug page's edit dispatch. Recompiled C vs Rust with the
//! callees as C. The models replay the callees' C on a copy of the state in
//! the same order with the same arguments (and the s registers the port
//! holds), adding the functions' own stores; whole RDRAM (and the result
//! register) is compared.

// Tests are named after the functions (func_80000D80), capitals included.
#![allow(non_snake_case)]

use difftest::rom::baserom;
use difftest::{compare, State};
use game::imports;
use game::pools::POOLS;
use game::recomp::{fpu, reg::*, RecompFn};
use game::{anim, collide, math, misc};
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
/// it: the test background is random, and the functions and their callees
/// read constants from there.
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

fn put3(s: &mut State, a: u32, v: [f32; 3]) {
    for (k, x) in v.iter().enumerate() {
        wf(s, a + 4 * k as u32, *x);
    }
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
fn neg(a: f32) -> Option<f32> {
    Some(-ok(a)?)
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

/// Finite values of the game's range (full mantissas), the signed zeros,
/// small integers and subnormals: no product or sum of a few of them
/// overflows.
fn fin() -> BoxedStrategy<f32> {
    prop_oneof![
        4 => -1.0e3f32..1.0e3f32,
        2 => -1.0f32..1.0,
        2 => (-4i32..=4).prop_map(|n| n as f32),
        1 => Just(-0.0f32),
        1 => (1u32..0x0080_0000).prop_map(f32::from_bits),
        1 => (-64i32..64).prop_map(|n| n as f32 + 0.5),
    ]
    .boxed()
}

fn fin3() -> BoxedStrategy<[f32; 3]> {
    prop::array::uniform3(fin()).boxed()
}

/// A factor pair whose f32 product is inexact with a positive rounding
/// error (exact - rounded > 0), searched here.
fn inexact_up(seed: u32) -> (f32, f32) {
    for k in 0..10_000u32 {
        let a = 1.1f32 + (seed.wrapping_mul(7919).wrapping_add(k) % 1000) as f32 * 0.013_7;
        let b = 3.3f32 + (k % 997) as f32 * 0.021_1;
        let err = f64::from(a) * f64::from(b) - f64::from(a * b);
        if err > 0.0 {
            return (a, b);
        }
    }
    panic!("no inexact product found");
}

// ---- func_80000D80: a line through two points meets a plane ----

const OUT: u32 = 0x8030_0000;
const O: u32 = 0x8030_0100;
const P: u32 = 0x8030_0200;
const N: u32 = 0x8030_0300;
const NORM_K: u32 = 0x800A_87F4;

fn line_plane_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0x38;
    let [out, o, p, n] = [A0, A1, A2, A3].map(|r| s.ctx.gpr[r] as u32);
    saves(&mut w, s, 0x38, &[(0x1C, RA), (0x18, S0), (0x44, A3), (0x3C, A1)]);
    for k in 0..3 {
        let d = sub(rf(&w, p + 4 * k), rf(&w, o + 4 * k))?;
        wf(&mut w, fr + 0x20 + 4 * k, d);
    }
    w.ctx.gpr[S0] = sext(out);
    call_c(&mut w, 0x38, &[(A0, sext(fr + 0x20))], imports::func_800154D0);
    let (d, ov, nv) = (v3(&w, fr + 0x20), v3(&w, o), v3(&w, n));
    let no = add(mul(nv[2], ov[2])?, add(mul(ov[0], nv[0])?, mul(ov[1], nv[1])?)?)?;
    let dn = add(mul(nv[2], d[2])?, add(mul(d[0], nv[0])?, mul(d[1], nv[1])?)?)?;
    let t = div(sub(rf(&w, SP_AT + 0x10), no)?, dn)?;
    for k in 0..3u32 {
        let x = mul(d[k as usize], t)?;
        wf(&mut w, out + 4 * k, x);
    }
    for k in 0..3 {
        let x = add(rf(&w, out + 4 * k), rf(&w, o + 4 * k))?;
        wf(&mut w, out + 4 * k, x);
    }
    Some(w)
}

/// Where `out` goes: its own buffer, over `o` (shifted by a word either
/// way, so the re-reads of `o` see earlier stores), over `n` or over `p`.
fn out_place(k: u8) -> u32 {
    match k {
        0 | 1 => OUT,
        2 => O,
        3 => O + 4,
        4 => O - 4,
        5 => N,
        _ => P,
    }
}

fn line_plane_case(seed: u64, o: [f32; 3], p: [f32; 3], n: [f32; 3], wv: f32, place: u8, k: Option<f32>) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    load_data(&mut s);
    if let Some(k) = k {
        wf(&mut s, NORM_K, k);
    }
    s.randomise_memory(seed ^ 0x0D80, OUT - 0x10, 0x20);
    put3(&mut s, O, o);
    put3(&mut s, P, p);
    put3(&mut s, N, n);
    wf(&mut s, SP_AT + 0x10, wv);
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(out_place(place)), sext(O), sext(P), sext(N));
    let want = line_plane_model(&s);
    prop_assume!(want.is_some());
    let after = run("func_80000D80", math::func_80000D80, &s)?;
    same_memory(&after, &want.unwrap())?;
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Random lines and planes, `out` over its inputs, the normalising
    /// threshold perturbed, short directions (`p` next to `o`, below the
    /// threshold, so not normalised).
    #[test]
    fn func_80000D80(seed: u64, o in fin3(), p in prop_oneof![3 => fin3().prop_map(|p| (p, false)), 1 => prop::array::uniform3(-1.0e-3f32..1.0e-3).prop_map(|d| (d, true))],
                     n in fin3(), wv in fin(), place in 0u8..7, k in prop_oneof![3 => Just(None), 1 => float().prop_map(Some), 1 => (0.0f32..2.0).prop_map(Some)]) {
        let p = if p.1 { [0, 1, 2].map(|c| o[c] + p.0[c]) } else { p.0 };
        line_plane_case(seed, o, p, n, wv, place, k)?;
    }
}

// ---- func_80001E80: the query's ray against a triangle ----

const PLANE: u32 = 0x8030_1000;
const TA: u32 = 0x8030_1100;
const TB: u32 = 0x8030_1200;
const TC: u32 = 0x8030_1300;
const RAY: u32 = 0x8030_1400;
const TRACKED: u32 = 0x800A_E8B0;
const FRONT: u32 = 0x8009_A274;
const BACK: u32 = 0x8009_A278;
const HIT: u32 = 0x800A_EC7C;
const SRC: u32 = 0x800A_E938;

fn dot(a: [f32; 3], b: [f32; 3]) -> Option<f32> {
    add(mul(a[2], b[2])?, add(mul(b[0], a[0])?, mul(b[1], a[1])?)?)
}

/// func_80001D34's domain for `plane`, `ray` (no NaN operand of its
/// guarded operations, `cvt.d.s` included), its hit point written to the
/// frame (no aliasing to check).
fn ray_plane_ok(s: &State, plane: u32, ray: u32) -> Option<()> {
    let (n, d) = (v3(s, plane), rf(s, plane + 0xC));
    let (o, v, l) = (v3(s, ray), v3(s, ray + 0xC), rf(s, ray + 0x18));
    let no = add(mul(n[2], o[2])?, add(mul(o[0], n[0])?, mul(o[1], n[1])?)?)?;
    let nv = ok(add(mul(n[2], v[2])?, add(mul(v[0], n[0])?, mul(v[1], n[1])?)?)?)?;
    let lo = f64::from_bits(u64::from(word(s, 0x800A_8128)) << 32 | u64::from(word(s, 0x800A_812C)));
    let hi = f64::from_bits(u64::from(word(s, 0x800A_8130)) << 32 | u64::from(word(s, 0x800A_8134)));
    if lo <= f64::from(nv) && f64::from(nv) <= hi {
        return Some(());
    }
    let t = div(sub(d, no)?, nv)?;
    if t < 0.0 || l < t {
        return Some(());
    }
    for k in 0..3 {
        let x = mul(v[k], t)?;
        // Bounded hit points keep func_800005B4's crosses finite.
        if !(add(x, o[k])?.abs() < 1.0e15) {
            return None;
        }
    }
    Some(())
}

fn ray_tri_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0x78;
    let [pl, a, b, c] = [A0, A1, A2, A3].map(|r| s.ctx.gpr[r] as u32);
    let ray = word(s, SP_AT + 0x10);
    saves(&mut w, s, 0x78, &[(0x34, RA), (0x30, S2), (0x2C, S1), (0x28, S0)]);
    let n = v3(s, pl);
    let e = dot(n, v3(s, ray + 0xC))?;
    let back = 0.0 < e;
    if word(s, if back { BACK } else { FRONT }) == 0 {
        return Some(w);
    }
    wr(&mut w, fr + 0x38, u32::from(back));
    wr(&mut w, SP_AT + 0xC, c);
    ray_plane_ok(&w, pl, ray)?;
    (w.ctx.gpr[S0], w.ctx.gpr[S1], w.ctx.gpr[S2]) = (sext(pl), sext(b), sext(a));
    call_c(&mut w, 0x78, &[(A0, sext(pl)), (A1, sext(ray)), (A2, sext(fr + 0x48))], imports::func_80001D34);
    let t = w.ctx.fpr[0].fl();
    if t < 0.0 || !(t < rf(&w, TRACKED)) {
        return Some(w);
    }
    for (at, (x, y)) in [(0x6C, (b, a)), (0x60, (c, b)), (0x54, (a, c))] {
        for k in 0..3 {
            let d = sub(rf(&w, x + 4 * k), rf(&w, y + 4 * k))?;
            wf(&mut w, fr + at + 4 * k, d);
        }
    }
    wf(&mut w, fr + 0x40, t);
    wr(&mut w, fr + 0x10, fr + 0x6C);
    wr(&mut w, fr + 0x14, fr + 0x60);
    wr(&mut w, fr + 0x18, fr + 0x54);
    call_c(&mut w, 0x78, &[(A0, sext(fr + 0x48)), (A1, sext(a)), (A2, sext(b)), (A3, sext(c))], imports::func_800005B4);
    if w.ctx.gpr[V0] == 0 {
        return Some(w);
    }
    wr(&mut w, HIT, 1);
    wf(&mut w, TRACKED, t);
    let h = v3(&w, fr + 0x48);
    put3(&mut w, TRACKED + 8, h);
    let v = word(&w, SRC);
    wr(&mut w, TRACKED + 0x28, v);
    let n = v3(&w, pl);
    let dir = if back { [neg(n[0])?, neg(n[1])?, neg(n[2])?] } else { n };
    put3(&mut w, TRACKED + 0x18, dir);
    Some(w)
}

#[derive(Clone, Debug)]
struct Tri {
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    n: [f32; 3],
    d: f32,
    o: [f32; 3],
    v: [f32; 3],
    l: f32,
    big_t: f32,
    flags: (u32, u32),
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// A triangle, its plane (true or random), and a ray aimed at a point of the
/// plane inside or near it, with the length and the tracked time around
/// the hit's time.
fn tri() -> BoxedStrategy<Tri> {
    let small = || prop_oneof![4 => -100.0f32..100.0, 1 => (-4i32..=4).prop_map(|n| n as f32), 1 => fin()].boxed();
    let p3 = move || prop::array::uniform3(small()).boxed();
    (
        (p3(), p3(), p3(), any::<bool>(), p3(), small()),
        (-0.5f32..1.5, -0.5f32..1.5, p3(), prop_oneof![-5.0f32..20.0, Just(0.0f32)]),
        (prop_oneof![0.5f32..2.0, Just(1.0f32)], prop_oneof![Just(None), small().prop_map(Some)], prop_oneof![0.5f32..2.0, Just(1.0f32)], prop_oneof![Just(None), small().prop_map(Some)]),
        (prop_oneof![Just(0u32), Just(1u32), any::<u32>()], prop_oneof![Just(0u32), Just(1u32), any::<u32>()]),
    )
        .prop_map(|((a, b, c, true_plane, rn, rd), (u, wv, v, t0), (lf, lr, tf, tr), flags)| {
            let (n, d) = if true_plane {
                let n = cross(sub3(b, a), sub3(c, a));
                (n, n[0] * a[0] + n[1] * a[1] + n[2] * a[2])
            } else {
                (rn, rd)
            };
            let (e1, e2) = (sub3(b, a), sub3(c, a));
            let q = [0, 1, 2].map(|k| a[k] + u * e1[k] + wv * e2[k]);
            let o = [0, 1, 2].map(|k| q[k] - v[k] * t0);
            let l = lr.unwrap_or(t0 * lf);
            let big_t = tr.unwrap_or(t0 * tf);
            Tri { a, b, c, n, d, o, v, l, big_t, flags }
        })
        .boxed()
}

fn ray_tri_case(seed: u64, x: &Tri) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    load_data(&mut s);
    s.randomise_memory(seed ^ 0x1E80, TRACKED, 0x3D0);
    put3(&mut s, PLANE, x.n);
    wf(&mut s, PLANE + 0xC, x.d);
    put3(&mut s, TA, x.a);
    put3(&mut s, TB, x.b);
    put3(&mut s, TC, x.c);
    put3(&mut s, RAY, x.o);
    put3(&mut s, RAY + 0xC, x.v);
    wf(&mut s, RAY + 0x18, x.l);
    wf(&mut s, TRACKED, x.big_t);
    wr(&mut s, FRONT, x.flags.0);
    wr(&mut s, BACK, x.flags.1);
    wr(&mut s, SP_AT + 0x10, RAY);
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(PLANE), sext(TA), sext(TB), sext(TC));
    let want = ray_tri_model(&s);
    prop_assume!(want.is_some());
    let after = run("func_80001E80", collide::func_80001E80, &s)?;
    same_memory(&after, &want.unwrap())?;
    Ok(())
}

/// `t` of the ray (func_80001D34's C on a copy), for ties.
fn ray_t(x: &Tri) -> f32 {
    let mut s = state(1);
    load_data(&mut s);
    put3(&mut s, PLANE, x.n);
    wf(&mut s, PLANE + 0xC, x.d);
    put3(&mut s, RAY, x.o);
    put3(&mut s, RAY + 0xC, x.v);
    wf(&mut s, RAY + 0x18, x.l);
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(PLANE), sext(RAY), sext(0x8030_2000));
    s.run(imports::func_80001D34);
    s.ctx.fpr[0].fl()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn func_80001E80(seed: u64, x in tri()) {
        ray_tri_case(seed, &x)?;
    }
}

/// Constructed cases: `e` exactly 0 unfused (the sign test's tie) with each
/// product's rounding error positive, so a fused sum turns the face around;
/// `v` in the plane (`e = 0`); the origin on the plane (`t = +-0`); `t`
/// equal to the tracked time; a hit on each face.
#[test]
fn func_80001E80_cases() {
    let base = Tri {
        a: [-10.0, -10.0, 5.0],
        b: [10.0, -10.0, 5.0],
        c: [0.0, 10.0, 5.0],
        n: [0.0, 0.0, 1.0],
        d: 5.0,
        o: [1.0, 1.0, 20.0],
        v: [0.0, 0.0, -1.0],
        l: 100.0,
        big_t: 1000.0,
        flags: (1, 1),
    };
    let mut cases = Vec::new();
    // Separators: (n.z v.z) + ((v.x n.x) + (v.y n.y)) with two terms
    // cancelling exactly; front faces stop (flag 0), back faces go on.
    let (p, q) = inexact_up(1);
    cases.push(Tri { n: [1.0, 1.0, p], v: [-(p * q), 0.0, q], flags: (0, 1), ..base.clone() });
    let (p, q) = inexact_up(2);
    cases.push(Tri { n: [p, 1.0, 0.0], v: [q, -(p * q), 3.0], flags: (0, 1), ..base.clone() });
    let (p, q) = inexact_up(3);
    cases.push(Tri { n: [1.0, p, 0.0], v: [-(p * q), q, -3.0], flags: (0, 1), ..base.clone() });
    // e = 0 by a direction in the plane, with each flag.
    for flags in [(1, 0), (0, 1), (1, 1)] {
        cases.push(Tri { v: [1.0, 0.0, 0.0], flags, ..base.clone() });
    }
    // The origin on the plane: t = -0 (v down) and +0 (v up, back face).
    cases.push(Tri { o: [1.0, 1.0, 5.0], ..base.clone() });
    cases.push(Tri { o: [1.0, 1.0, 5.0], v: [0.0, 0.0, 1.0], ..base.clone() });
    // Hits on both faces, and misses outside the triangle.
    cases.push(base.clone());
    cases.push(Tri { o: [1.0, 1.0, -20.0], v: [0.0, 0.0, 1.0], ..base.clone() });
    cases.push(Tri { o: [30.0, 1.0, 20.0], ..base.clone() });
    // t equal to the tracked time, on either face.
    for v in [[0.0, 0.0, -1.0], [0.1, 0.0, -0.7], [0.0, 0.0, 1.5]] {
        let mut x = Tri { v, o: [1.0, 1.0, if v[2] < 0.0 { 20.0 } else { -20.0 }], ..base.clone() };
        x.big_t = ray_t(&x);
        assert!(x.big_t > 0.0);
        cases.push(x);
    }
    for (i, x) in cases.iter().enumerate() {
        ray_tri_case(0xE80 + i as u64, x).unwrap_or_else(|e| panic!("case {i}: {e}"));
    }
}

// ---- func_80006120 / func_80006200 / func_8000651C: tracks to a node ----

const OBJ: u32 = 0x8030_3000;
const KEYS: u32 = 0x8030_3200;
const VALS: u32 = 0x8030_3400;
const NODE: u32 = 0x8030_3800;
const TEX: u32 = 0x8030_3900;

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

/// The sampler's case, with `t` between keys, or None outside its domain.
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

/// func_80005CD4's domain (`stride` 1) or func_80005DA8's (3): the value
/// is computed without a NaN operand.
fn sample_ok(k: &[f32], vals: &[f32], x: f32, i: usize, stride: usize) -> Option<()> {
    let (case, t) = where_(k, x, i)?;
    if case == 2 {
        for c in 0..stride {
            let (a, b) = (vals[stride * i + c], vals[stride * (i + 1) + c]);
            add(mul(a, sub(1.0, t)?)?, mul(b, t)?)?;
        }
    }
    Some(())
}

#[derive(Clone, Debug)]
struct Track {
    keys: Vec<f32>,
    x: f32,
    i: usize,
    vals: Vec<f32>,
    blend: bool,
    flags: u32,
    x2: i32,
    i2: usize,
    w: f32,
    node: bool,
}

fn weight() -> BoxedStrategy<f32> {
    prop_oneof![Just(0.0f32), Just(1.0f32), Just(0.5f32), Just(-0.0f32), 0.0f32..1.0, fin()].boxed()
}

fn track_case() -> BoxedStrategy<Track> {
    (
        track(2..=6),
        any::<prop::sample::Index>(),
        prop::collection::vec(fin(), 18),
        (any::<bool>(), any::<u32>(), prop_oneof![-2i32..110, any::<i32>()], any::<prop::sample::Index>()),
        (weight(), prop_oneof![3 => Just(true), 1 => Just(false)], prop_oneof![3 => Just(None), 1 => (0i32..4000).prop_map(Some)]),
    )
        .prop_map(|((keys, x), ix, vals, (blend, flags, x2, ix2), (w, node, big))| {
            let n = keys.len();
            let flags = if blend { flags | 0x2000_0000 } else { flags & !0x2000_0000 };
            // Keys and times past 2^24, where the second time's int-to-float
            // conversion rounds (to nearest; toward zero would differ).
            let (keys, x, x2) = match big {
                Some(d) => (keys.iter().map(|k| 16_777_216.0 + k * 40.0).collect(), 16_777_216.0 + x * 40.0, 16_777_216 + d),
                None => (keys, x, x2),
            };
            Track { i: ix.index(n - 1), i2: ix2.index(n - 1), keys, x, vals, blend, flags, x2, w, node }
        })
        .boxed()
}

fn put_track(s: &mut State, t: &Track, node: u32) {
    wr(s, OBJ + 0x11C, KEYS);
    wr(s, OBJ + 0x120, VALS);
    for (j, v) in t.keys.iter().enumerate() {
        wf(s, KEYS + 4 * j as u32, *v);
    }
    for (j, v) in t.vals.iter().enumerate() {
        wf(s, VALS + 4 * j as u32, *v);
    }
    wf(s, OBJ + 0x114, t.x);
    wr(s, OBJ + 0x118, t.i as u32);
    wr(s, OBJ + 0x100, t.flags);
    wr(s, OBJ + 0xEC, t.x2 as u32);
    wr(s, OBJ + 0xE8, t.i2 as u32);
    wf(s, OBJ + 0xE4, t.w);
    wr(s, OBJ + 0x124, if t.node { node } else { 0 });
    s.ctx.gpr[A0] = sext(OBJ);
}

fn x2f(t: &Track) -> f32 {
    fpu::cvt_s_w(t.x2 as u32, fpu::NEAREST)
}

/// The vec3 sample `p` at `fr + at`, blended as func_80006120 and
/// func_80006200 do with the second sample at `fr + at2`.
fn vec3_blend(w: &mut State, t: &Track, frame: u32, at: u32, at2: u32) -> Option<()> {
    let fr = SP_AT - frame;
    sample_ok(&t.keys, &t.vals, t.x, t.i, 3)?;
    w.ctx.gpr[S0] = sext(OBJ);
    call_c(w, frame, &[(A0, sext(fr + at)), (A1, sext(OBJ)), (A2, sext(t.x.to_bits())), (A3, t.i as u64)], imports::func_80005DA8);
    if !t.blend {
        return Some(());
    }
    sample_ok(&t.keys, &t.vals, x2f(t), t.i2, 3)?;
    call_c(w, frame, &[(A0, sext(fr + at2)), (A1, sext(OBJ)), (A2, sext(x2f(t).to_bits())), (A3, t.i2 as u64)], imports::func_80005DA8);
    for k in 0..3 {
        let x = mul(rf(w, fr + at + 4 * k), rf(w, OBJ + 0xE4))?;
        wf(w, fr + at + 4 * k, x);
    }
    let om = sub(1.0, rf(w, OBJ + 0xE4))?;
    for k in 0..3 {
        add(mul(rf(w, fr + at2 + 4 * k), om)?, rf(w, fr + at + 4 * k))?;
    }
    call_c(w, frame, &[(A0, sext(fr + at)), (A1, sext(fr + at)), (A2, sext(om.to_bits())), (A3, sext(fr + at2))], imports::func_800155EC);
    Some(())
}

fn translation_model(s: &State, t: &Track) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0x40;
    saves(&mut w, s, 0x40, &[(0x18, S0), (0x1C, RA)]);
    vec3_blend(&mut w, t, 0x40, 0x34, 0x28)?;
    let n = word(&w, OBJ + 0x124);
    if n != 0 {
        let p = [0, 1, 2].map(|k| sext(word(&w, fr + 0x34 + 4 * k)));
        call_c(&mut w, 0x40, &[(A0, sext(n)), (A1, p[0]), (A2, p[1]), (A3, p[2])], imports::func_80017B7C);
    }
    Some(w)
}

/// A node's 3x4 transform with rows long enough to normalise.
fn transform() -> BoxedStrategy<[f32; 12]> {
    prop::array::uniform12(prop_oneof![3 => -10.0f32..10.0, 1 => (-2i32..=2).prop_map(|n| n as f32)])
        .prop_map(|mut m| {
            for r in 0..3 {
                if m[3 * r..3 * r + 3].iter().all(|x| x.abs() < 0.01) {
                    m[3 * r] = 1.0;
                }
            }
            m
        })
        .boxed()
}

fn scale_model(s: &State, t: &Track) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0xD8;
    saves(&mut w, s, 0xD8, &[(0x18, S0), (0x1C, RA)]);
    vec3_blend(&mut w, t, 0xD8, 0xCC, 0xC0)?;
    let n = word(&w, OBJ + 0x124);
    if n != 0 {
        let p = v3(&w, fr + 0xCC);
        if p.iter().any(|x| !x.is_finite() || x.abs() > 1.0e6) {
            return None;
        }
        call_c(&mut w, 0xD8, &[(A0, sext(n)), (A1, sext(fr + 0x80))], imports::func_80017C18);
        call_c(&mut w, 0xD8, &[(A0, sext(fr + 0x80)), (A1, sext(fr + 0x34)), (A2, sext(fr + 0x40)), (A3, sext(fr + 0x28))], imports::func_80081814);
        call_c(&mut w, 0xD8, &[(A0, sext(fr + 0x80)), (A1, sext(fr + 0x34)), (A2, sext(fr + 0x40)), (A3, sext(fr + 0xCC))], imports::func_80081948);
        let n = word(&w, OBJ + 0x124);
        call_c(&mut w, 0xD8, &[(A0, sext(n)), (A1, sext(fr + 0x80))], imports::func_80017BA8);
    }
    Some(w)
}

fn halfword_model(s: &State, t: &Track, k: u32) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0x38;
    saves(&mut w, s, 0x38, &[(0x3C, A1), (0x14, RA), (0x38, A0)]);
    sample_ok(&t.keys, &t.vals, t.x, t.i, 1)?;
    call_c(&mut w, 0x38, &[(A0, sext(fr + 0x28)), (A1, sext(OBJ)), (A2, sext(t.x.to_bits())), (A3, t.i as u64)], imports::func_80005CD4);
    if t.blend {
        // QUIRK: the vec3 sampler, its 12 bytes over the float sample.
        sample_ok(&t.keys, &t.vals, x2f(t), t.i2, 3)?;
        call_c(&mut w, 0x38, &[(A0, sext(fr + 0x24)), (A1, sext(OBJ)), (A2, sext(x2f(t).to_bits())), (A3, t.i2 as u64)], imports::func_80005DA8);
        let wv = rf(&w, OBJ + 0xE4);
        let p = mul(wv, rf(&w, fr + 0x28))?;
        wf(&mut w, fr + 0x28, p);
        let sum = add(mul(sub(1.0, wv)?, rf(&w, fr + 0x24))?, rf(&w, fr + 0x28))?;
        wf(&mut w, fr + 0x28, sum);
    }
    let n = word(&w, OBJ + 0x124);
    if n == 0 {
        return Some(w);
    }
    let tex = word(&w, n + 8);
    if tex == 0 {
        return Some(w);
    }
    let j = if k != 0 { 6 } else { 4 };
    let l = half(&w, tex + j) as i16;
    let v = fpu::trunc_w_s(mul(f32::from(l), rf(&w, fr + 0x28))?);
    wh(&mut w, n + j, v as u16);
    let rd = |w: &State| (half(w, n + j) as i16, half(w, tex + j) as i16);
    let (mut v, mut l) = rd(&w);
    while l < v {
        wh(&mut w, n + j, (v as i32 - l as i32) as u16);
        (v, l) = rd(&w);
    }
    while v < 0 {
        let l = half(&w, tex + j) as i16;
        wh(&mut w, n + j, (v as i32 + l as i32) as u16);
        v = half(&w, n + j) as i16;
    }
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80006120(seed: u64, t in track_case(), junk: [u32; 4]) {
        let mut s = state(seed);
        put_track(&mut s, &t, NODE);
        for (k, j) in junk.iter().enumerate() {
            wr(&mut s, NODE + 0xC + 0x34 * (k as u32 / 2) + 4 * (k as u32 % 2), *j);
        }
        let want = translation_model(&s, &t);
        prop_assume!(want.is_some());
        let after = run("func_80006120", anim::func_80006120, &s)?;
        same_memory(&after, &want.unwrap())?;
    }

    #[test]
    fn func_80006200(seed: u64, t in track_case(), m in transform(), flags: u16) {
        let mut s = state(seed);
        put_track(&mut s, &t, NODE);
        wh(&mut s, NODE + 0xC, flags);
        for (k, x) in m.iter().enumerate() {
            wf(&mut s, NODE + 0x1C + 4 * k as u32, *x);
        }
        let want = scale_model(&s, &t);
        prop_assume!(want.is_some());
        let after = run("func_80006200", anim::func_80006200, &s)?;
        same_memory(&after, &want.unwrap())?;
    }

    /// Both halfwords, limits small and large, of either sign or 0, values
    /// far past the limit (many wrap steps), and the limit's halfword over
    /// the value's (`t = n`).
    #[test]
    fn func_8000651C(seed: u64, t in track_case(), k in prop_oneof![Just(0u32), Just(1u32), any::<u32>()],
                     l in prop_oneof![3 => 1i16..40, 1 => Just(0i16), 1 => -40i16..0, 1 => any::<i16>()],
                     other: [u16; 2], tex_is_node in prop_oneof![5 => Just(false), 1 => Just(true)], tex0 in prop_oneof![5 => Just(false), 1 => Just(true)], ku: u32) {
        let mut s = state(seed);
        let t = Track { vals: t.vals.iter().map(|v| v * 0.05).collect(), ..t };
        put_track(&mut s, &t, NODE);
        let tex = if tex0 { 0 } else if tex_is_node { NODE } else { TEX };
        wr(&mut s, NODE + 8, tex);
        wh(&mut s, NODE + 4, other[0]);
        wh(&mut s, NODE + 6, other[1]);
        if tex == TEX {
            wh(&mut s, TEX + 4, l as u16);
            wh(&mut s, TEX + 6, l as u16);
        }
        s.ctx.gpr[A1] = if ku & 1 == 0 { sext(k) } else { u64::from(k) << 32 | u64::from(ku & 0x10) };
        let want = halfword_model(&s, &t, s.ctx.gpr[A1] as u32);
        prop_assume!(want.is_some());
        let after = run("func_8000651C", anim::func_8000651C, &s)?;
        same_memory(&after, &want.unwrap())?;
    }
}

/// A second time past 2^24 that the int-to-float conversion rounds
/// (16777219 to nearest is 16777220, toward zero 16777218), between keys.
#[test]
fn func_80006120_big_time() {
    let t = Track {
        keys: vec![16_777_216.0, 16_777_300.0, 16_777_400.0],
        x: 16_777_250.0,
        i: 0,
        vals: (0..18).map(|k| k as f32 * 1.5 - 7.0).collect(),
        blend: true,
        flags: 0x2000_0000,
        x2: 16_777_219,
        i2: 0,
        w: 0.25,
        node: true,
    };
    let mut s = state(0x1234);
    put_track(&mut s, &t, NODE);
    let want = translation_model(&s, &t).unwrap();
    let after = compare("func_80006120", anim::func_80006120, &s).unwrap_or_else(|e| panic!("{e}"));
    same_memory(&after, &want).unwrap();
}

// ---- func_80008B14 / func_80008B68: sound requests ----

const SLOTS: u32 = 0x800D_2038;

#[derive(Clone, Debug)]
struct Req {
    id: u64,
    a1: u64,
    pitch: f32,
    vol: f32,
    keep: u32,
    slots: Vec<u32>,
    bytes: [u8; 3],
    stereo: u32,
    frame: u32,
    k: Option<f32>,
    hi: [u32; 2],
}

fn req() -> BoxedStrategy<Req> {
    (
        (prop_oneof![0u64..8, Just(sext(0xFFFF_FFFF)), any::<u32>().prop_map(sext)], any::<u32>().prop_map(sext), fin(),
         prop_oneof![Just(0.0f32), Just(-0.0f32), Just(1.0f32), (1u32..0x0080_0000).prop_map(f32::from_bits), 0.0f32..2.0, fin()]),
        (prop_oneof![Just(0u32), Just(1u32), any::<u32>()], prop::collection::vec(any::<u32>(), 64)),
        (any::<[u8; 3]>(), any::<u32>(), any::<u32>(), prop_oneof![3 => Just(None), 1 => fin().prop_map(Some)], any::<[u32; 2]>()),
    )
        .prop_map(|((id, a1, pitch, vol), (keep, slots), (bytes, stereo, frame, k, hi))| Req { id, a1, pitch, vol, keep, slots, bytes, stereo, frame, k, hi })
        .boxed()
}

fn put_req(s: &mut State, r: &Req) {
    load_data(s);
    for (i, w) in r.slots.iter().enumerate() {
        wr(s, SLOTS + 4 * i as u32, *w);
    }
    s.rdram.mem().write_u8(0x8009_B77F, r.bytes[0]);
    s.rdram.mem().write_u8(0x8011_3685, r.bytes[1]);
    s.rdram.mem().write_u8(0x8011_3686, r.bytes[2]);
    wr(s, 0x8011_3688, r.stereo);
    wr(s, 0x8012_0BE8, r.frame);
    if let Some(k) = r.k {
        wf(s, 0x800A_81E0, k);
    }
    wr(s, SP_AT + 0x10, r.keep);
    // Non-canonical high halves of the float arguments (they go through
    // mtc1/mfc1, so only the low words count).
    s.ctx.gpr[A0] = r.id;
    s.ctx.gpr[A1] = r.a1;
    s.ctx.gpr[A2] = u64::from(r.hi[0]) << 32 | u64::from(r.pitch.to_bits());
    s.ctx.gpr[A3] = u64::from(r.hi[1]) << 32 | u64::from(r.vol.to_bits());
}

fn request_model(s: &State, vol: Option<f32>) -> State {
    let mut w = s.clone();
    let fr = SP_AT - 0x20;
    saves(&mut w, s, 0x20, &[(0x24, A1), (0x1C, RA)]);
    if let Some(vol) = vol {
        wr(&mut w, fr + 0x10, 0x40);
        wr(&mut w, fr + 0x14, word(s, SP_AT + 0x10));
        let prio = s.ctx.gpr[A1] as u16 as i16 as i64 as u64;
        call_c(&mut w, 0x20, &[(A1, prio), (A2, sext(s.ctx.gpr[A2] as u32)), (A3, sext(vol.to_bits()))], imports::func_80008760);
    }
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Volumes around 0 (both zeros, the smallest subnormal, negatives),
    /// NaN (no call).
    #[test]
    fn func_80008B14(seed: u64, r in req(), nan: bool) {
        let mut s = state(seed);
        let r = if nan { Req { vol: f32::NAN, ..r } } else { r };
        put_req(&mut s, &r);
        let want = request_model(&s, (0.0 < r.vol).then_some(r.vol));
        let after = run("func_80008B14", misc::func_80008B14, &s)?;
        same_memory(&after, &want)?;
    }

    /// Volumes around 1.0 (1.0 itself, the floats next to it).
    #[test]
    fn func_80008B68(seed: u64, r in req(), near1 in prop_oneof![Just(None), (-3i32..=3).prop_map(Some)]) {
        let mut s = state(seed);
        let r = match near1 {
            Some(k) => Req { vol: f32::from_bits((1.0f32.to_bits() as i32 + k) as u32), ..r },
            None => r,
        };
        put_req(&mut s, &r);
        let want = request_model(&s, Some(if r.vol < 1.0 { 1.0 } else { r.vol }));
        let after = run("func_80008B68", misc::func_80008B68, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_800091B0: a sound's timing ----

const TABLE_A: u32 = 0x8009_A32C;
const TABLE_B: u32 = 0x8009_A388;
const DEFAULT: u32 = 0x800A_FA54;
const CHAIN: u32 = 0x8030_6000;

/// The handle lookup's chain (as in depth1_80005C54.rs): every slot the
/// lookup may use points at the same object, whose chain ends at `n`.
fn put_handle(s: &mut State, kind: u32, idx: u32, j: u32, bit15: bool, n: i32) -> u32 {
    let handle = kind << 24 | idx << 16 | u32::from(bit15) << 15 | j;
    wr(s, DEFAULT, CHAIN);
    wr(s, TABLE_A + 4 * idx, CHAIN);
    wr(s, TABLE_B + 4 * kind, CHAIN);
    let (p, q, r) = (CHAIN + 0x100, CHAIN + 0x200, CHAIN + 0x300);
    wr(s, CHAIN + 0xC, p);
    wr(s, p + 4 * j + 0x10, q);
    wr(s, q + 8, r);
    wr(s, r + 4, n as u32);
    handle
}

fn timing_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    saves(&mut w, s, 0x20, &[(0x18, S0), (0x1C, RA), (0x24, A1), (0x2C, A3)]);
    let kind = s.ctx.gpr[A0];
    w.ctx.gpr[S0] = kind;
    call_c(&mut w, 0x20, &[(A0, s.ctx.gpr[A2])], imports::func_80007F5C);
    let d = w.ctx.fpr[0].fl();
    match kind {
        0 | 1 => {
            let i = word(&w, SP_AT + 4);
            let v = add(d, 1.0)?;
            wf(&mut w, 0x8009_AD30u32.wrapping_add(i.wrapping_mul(4)), v);
            wr(&mut w, 0x8009_AD8Cu32.wrapping_add(i.wrapping_mul(4)), kind as u32);
        }
        2 => {
            let j = word(&w, SP_AT + 0xC);
            let v = add(rf(&w, 0x8009_ADFCu32.wrapping_add(j.wrapping_mul(4))), 0.25)?;
            wf(&mut w, 0x8009_AD18, v);
        }
        _ => {
            let v = add(d, 1.0)?;
            wf(&mut w, 0x8009_AD10u32.wrapping_add((kind as u32).wrapping_mul(4)), v);
        }
    }
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_800091B0(seed: u64, kind in prop_oneof![Just(0i32), Just(1i32), Just(2i32), 3i32..64, -64i32..0],
                     (hk, idx, j, bit15, n) in (0u32..8, 0u32..23, 0u32..4, any::<bool>(), prop_oneof![Just(0i32), 1i32..100_000, any::<i32>()]),
                     i in -0x40i32..0x40, jj in -0x40i32..0x40, x in fin(), kk in prop_oneof![3 => Just(None), 1 => fin().prop_map(Some)]) {
        let mut s = state(seed);
        load_data(&mut s);
        if let Some(k) = kk {
            wf(&mut s, 0x800A_81C8, k);
        }
        let h = put_handle(&mut s, hk, idx, j, bit15, n);
        wf(&mut s, 0x8009_ADFCu32.wrapping_add((jj as u32).wrapping_mul(4)), x);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(kind as u32), sext(i as u32), sext(h), sext(jj as u32));
        let want = timing_model(&s);
        prop_assume!(want.is_some());
        let after = run("func_800091B0", misc::func_800091B0, &s)?;
        same_memory(&after, &want.unwrap())?;
    }
}

// ---- func_8000D90C: the debug page's edit dispatch ----

const PAGE: u32 = 0x8009_B800;
const TEST: u32 = 0x5465_7374;
const REGISTRY: u32 = 0x8030_7000;
const DESC: u32 = 0x8030_7100;
const ELEMS: u32 = 0x8030_8000;
const STRIDE: u32 = 0x1E80;
const LINK: u32 = 0x8036_0000;

/// A "Test" pool for func_8000CC1C (as in depth1_8000B98C.rs), fields
/// non-negative (one is square-rooted), and the settings func_8000D5EC
/// edits.
fn put_menu(s: &mut State, elems: &[(i16, u16)], tag: i32, fields: &[f32], settings: &[f32; 4], words: &[u32; 5]) {
    wr(s, POOLS, REGISTRY);
    wr(s, REGISTRY, DESC);
    wr(s, REGISTRY + 4, 0);
    wr(s, DESC, TEST);
    wr(s, DESC + 8, elems.len() as u32);
    wr(s, DESC + 0xC, STRIDE);
    wr(s, DESC + 0x10, ELEMS);
    for (i, &(t, f)) in elems.iter().enumerate() {
        let e = ELEMS + STRIDE * i as u32;
        wh(s, e + 4, t as u16);
        wh(s, e + 6, f);
        wr(s, e + 0x1E70, LINK + 0x100 * i as u32);
        for (j, v) in fields.iter().enumerate() {
            wf(s, e + 0x6C + 4 * j as u32, *v);
        }
    }
    wr(s, 0x8009_B7E0, tag as u32);
    for (a, v) in [0x800A_5B64u32, 0x800A_5B68, 0x800A_5B54, 0x800A_5B58].iter().zip(settings) {
        wf(s, *a, *v);
    }
    wr(s, 0x8009_B7D8, words[0]);
    wr(s, 0x8009_B7D0, words[1] % 8);
    wr(s, 0x800A_52D4, words[2]);
    wr(s, 0x800A_52D0, words[3]);
    wr(s, 0x800D_697C, words[4]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8000D90C(seed: u64, page in prop_oneof![Just(0u32), Just(1u32), Just(2u32), any::<u32>()], k in prop_oneof![4 => 0u64..20, 1 => any::<u32>().prop_map(u64::from)],
                     x in prop_oneof![fin(), Just(1.0f32), Just(-1.0f32)], hi: u32,
                     elems in prop::collection::vec((0i16..5, prop_oneof![Just(0u16), Just(0x100u16), Just(0x200u16)]), 0..5), tag in -1i32..5,
                     fields in prop::collection::vec(0.0f32..100.0, 40), settings in prop::array::uniform4(0.0f32..300.0), words: [u32; 5]) {
        let mut s = state(seed);
        load_data(&mut s);
        put_menu(&mut s, &elems, tag, &fields, &settings, &words);
        wr(&mut s, PAGE, page);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (k, u64::from(hi) << 32 | u64::from(x.to_bits()));
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA)]);
        let callee = match page {
            0 => Some(imports::func_8000D5EC as RecompFn),
            1 => Some(imports::func_8000CC1C as RecompFn),
            _ => None,
        };
        if let Some(f) = callee {
            call_c(&mut want, 0x18, &[(A1, sext(x.to_bits()))], f);
        }
        let after = run("func_8000D90C", misc::func_8000D90C, &s)?;
        same_memory(&after, &want)?;
    }
}
