//! Depth 2 at 0x80016F0C..0x80017AC0 (game::math): a 4x4's translation and
//! angles, rotations from three angles (4x4 and packed 3x3) and about an
//! axis, and two planes (a triangle's, and a normal through a point).
//! Recompiled C vs Rust with the callees as C. The models replay the
//! callees' C on a copy of the state in the same order with the same
//! arguments (the float argument in `f12` too), adding the functions' own
//! stores; whole RDRAM is compared.

// Tests are named after the functions (func_80016F0C), capitals included.
#![allow(non_snake_case)]

use difftest::rom::baserom;
use difftest::{compare, State};
use game::imports;
use game::math;
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
    load_data(&mut s);
    s
}

/// The game's data segment (`0x80098000..0x800AE8B0`) as the ROM image has
/// it: the functions and their callees read constants from there.
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

fn wf(s: &mut State, a: u32, v: f32) {
    wr(s, a, v.to_bits());
}

fn rf(s: &State, a: u32) -> f32 {
    f32::from_bits(word(s, a))
}

fn rd(s: &State, a: u32) -> f64 {
    f64::from_bits(u64::from(word(s, a)) << 32 | u64::from(word(s, a + 4)))
}

fn v3(s: &State, a: u32) -> [f32; 3] {
    [0, 1, 2].map(|k| rf(s, a + 4 * k))
}

fn put3(s: &mut State, a: u32, v: [f32; 3]) {
    for (k, x) in v.iter().enumerate() {
        wf(s, a + 4 * k as u32, *x);
    }
}

/// Runs a callee's C on `w` with `sp` at `SP_AT - frame`, `args` set and
/// `f12` (a float argument) if given.
fn call_c(w: &mut State, frame: u32, args: &[(usize, u64)], f12: Option<f32>, f: RecompFn) {
    for &(r, v) in args {
        w.ctx.gpr[r] = v;
    }
    if let Some(x) = f12 {
        w.ctx.fpr[12].set_fl(x);
    }
    w.ctx.gpr[SP] = sext(SP_AT - frame);
    w.run(f);
}

fn saves(w: &mut State, s: &State, frame: u32, regs: &[(u32, usize)]) {
    for &(off, r) in regs {
        wr(w, SP_AT - frame + off, s.ctx.gpr[r] as u32);
    }
}

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

/// Finite values that stay finite through a few products.
fn fin() -> BoxedStrategy<f32> {
    prop_oneof![
        4 => -100.0f32..100.0,
        2 => -1.0f32..1.0,
        2 => (-4i32..=4).prop_map(|n| n as f32),
        1 => Just(-0.0f32),
        1 => (1u32..0x0080_0000).prop_map(f32::from_bits),
    ]
    .boxed()
}

fn angle() -> BoxedStrategy<f32> {
    prop_oneof![4 => -720.0f32..720.0, 1 => (-8i32..8).prop_map(|n| n as f32 * 45.0), 1 => Just(-0.0f32), 1 => float().prop_filter("moderate", |x| x.abs() < 1.0e5)].boxed()
}

const M: u32 = 0x8030_0000;
const OUT: u32 = 0x8030_0100;
const A: u32 = 0x8030_0200;
const B: u32 = 0x8030_0300;
const C: u32 = 0x8030_0400;

fn bits(x: f32) -> u64 {
    sext(x.to_bits())
}

// ---- func_8001723C / func_8001734C: rotations from three angles ----

fn euler_model(s: &State, stride: u32) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0x40;
    saves(&mut w, s, 0x40, &[(0x14, RA), (0x48, A2), (0x4C, A3), (0x40, A0)]);
    let m = s.ctx.gpr[A0] as u32;
    let angles = [A1, A2, A3].map(|r| f32::from_bits(s.ctx.gpr[r] as u32));
    for (k, (sa, ca)) in [(0x3C, 0x38), (0x34, 0x30), (0x2C, 0x28)].iter().enumerate() {
        call_c(&mut w, 0x40, &[(A1, sext(fr + sa)), (A2, sext(fr + ca))], Some(angles[k]), imports::func_80014CC0);
    }
    let r = |w: &State, o: u32| rf(w, fr + o);
    let (s1, c1, s2, c2, s3, c3) = (r(&w, 0x3C), r(&w, 0x38), r(&w, 0x34), r(&w, 0x30), r(&w, 0x2C), r(&w, 0x28));
    let (c1c3, c1s3, s1c3, s1s3) = (mul(c1, c3)?, mul(c1, s3)?, mul(s1, c3)?, mul(s1, s3)?);
    let e = [
        (0, 0, sub(c1c3, mul(s1s3, s2)?)?),
        (0, 1, add(mul(c1s3, s2)?, s1c3)?),
        (0, 2, mul(neg(s3)?, c2)?),
        (1, 0, mul(neg(s1)?, c2)?),
        (1, 1, mul(c1, c2)?),
        (1, 2, s2),
        (2, 0, add(mul(s1c3, s2)?, c1s3)?),
        (2, 1, sub(s1s3, mul(c1c3, s2)?)?),
        (2, 2, mul(c3, c2)?),
    ];
    for (i, j, v) in e {
        wf(&mut w, m + stride * i + 4 * j, v);
    }
    Some(w)
}

fn euler_case(seed: u64, angles: [f32; 3], stride: u32, name: &str, port: RecompFn) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    s.randomise_memory(seed ^ 0x7230, M, 0x40);
    s.ctx.gpr[A0] = sext(M);
    (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (bits(angles[0]), bits(angles[1]), bits(angles[2]));
    let want = euler_model(&s, stride);
    prop_assume!(want.is_some());
    let after = run(name, port, &s)?;
    same_memory(&after, &want.unwrap())?;
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8001723C(seed: u64, angles in prop::array::uniform3(angle())) {
        euler_case(seed, angles, 0x10, "func_8001723C", math::func_8001723C)?;
    }

    #[test]
    fn func_8001734C(seed: u64, angles in prop::array::uniform3(angle())) {
        euler_case(seed, angles, 0xC, "func_8001734C", math::func_8001734C)?;
    }
}

// ---- func_800179EC / func_80017AC0: planes ----

fn plane3_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0x48;
    saves(&mut w, s, 0x48, &[(0x1C, RA), (0x18, S1), (0x14, S0), (0x48, A0)]);
    let [out, a, b, c] = [A0, A1, A2, A3].map(|r| s.ctx.gpr[r] as u32);
    for k in 0..3 {
        let d = sub(rf(&w, b + 4 * k), rf(&w, a + 4 * k))?;
        wf(&mut w, fr + 0x3C + 4 * k, d);
    }
    for k in 0..3 {
        let d = sub(rf(&w, c + 4 * k), rf(&w, b + 4 * k))?;
        wf(&mut w, fr + 0x30 + 4 * k, d);
    }
    w.ctx.gpr[S1] = sext(a);
    call_c(&mut w, 0x48, &[(A0, sext(out)), (A1, sext(fr + 0x3C)), (A2, sext(fr + 0x30))], None, imports::func_80015538);
    if v3(&w, out).iter().any(|x| !x.is_finite()) {
        return None;
    }
    call_c(&mut w, 0x48, &[(A0, sext(out))], None, imports::func_800154D0);
    let (n, av) = (v3(&w, out), v3(&w, a));
    let d = add(mul(av[2], n[2])?, add(mul(n[0], av[0])?, mul(n[1], av[1])?)?)?;
    wf(&mut w, out + 0xC, d);
    Some(w)
}

fn plane_np_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    saves(&mut w, s, 0x18, &[(0x14, RA), (0x18, A0), (0x20, A2)]);
    let [out, n, p] = [A0, A1, A2].map(|r| s.ctx.gpr[r] as u32);
    for k in 0..3 {
        let v = word(&w, n + 4 * k);
        wr(&mut w, out + 4 * k, v);
    }
    if v3(&w, out).iter().any(|x| x.is_nan()) {
        return None;
    }
    call_c(&mut w, 0x18, &[(A0, sext(out))], None, imports::func_800154D0);
    let (o, pv) = (v3(&w, out), v3(&w, p));
    let d = add(mul(pv[2], o[2])?, add(mul(o[0], pv[0])?, mul(o[1], pv[1])?)?)?;
    wf(&mut w, out + 0xC, d);
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Triangles (degenerate ones too: a corner repeated), `out` over a
    /// corner.
    #[test]
    fn func_800179EC(seed: u64, a in prop::array::uniform3(fin()), b in prop::array::uniform3(fin()), c in prop::array::uniform3(fin()),
                     dup in 0u8..6, place in 0u8..6, junk: u32) {
        let mut s = state(seed);
        let b = if dup == 0 { a } else { b };
        let c = if dup == 1 { b } else { c };
        put3(&mut s, A, a);
        put3(&mut s, B, b);
        put3(&mut s, C, c);
        wr(&mut s, OUT + 0xC, junk);
        let out = match place { 0 => A, 1 => B, 2 => C, _ => OUT };
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(out), sext(A), sext(B), sext(C));
        let want = plane3_model(&s);
        prop_assume!(want.is_some());
        let after = run("func_800179EC", math::func_800179EC, &s)?;
        same_memory(&after, &want.unwrap())?;
    }

    /// Normals long, short (left as they are) and zero, `out` over `n` (also
    /// shifted a word) or `p`.
    #[test]
    fn func_80017AC0(seed: u64, n in prop_oneof![prop::array::uniform3(fin()), prop::array::uniform3(-1.0e-3f32..1.0e-3), Just([0.0f32; 3])],
                     p in prop::array::uniform3(fin()), place in 0u8..7, junk: [u32; 4]) {
        let mut s = state(seed);
        for (k, j) in junk.iter().enumerate() {
            wr(&mut s, OUT + 4 * k as u32, *j);
        }
        put3(&mut s, A, n);
        put3(&mut s, B, p);
        let out = match place { 0 => A, 1 => A + 4, 2 => A - 4, 3 => B, 4 => B - 4, _ => OUT };
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(out), sext(A), sext(B));
        let want = plane_np_model(&s);
        prop_assume!(want.is_some());
        let after = run("func_80017AC0", math::func_80017AC0, &s)?;
        same_memory(&after, &want.unwrap())?;
    }
}

// ---- func_800175E0: a rotation about an axis ----

const POLE_UP: u32 = 0x800A_8810;
const POLE_DOWN: u32 = 0x800A_8814;

fn axis_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0x70;
    save_double(&mut w, fr + 0x10, s.ctx.fpr[20].u64);
    saves(&mut w, s, 0x70, &[(0x1C, RA), (0x70, A0), (0x78, A2)]);
    let m = s.ctx.gpr[A0] as u32;
    let a = f32::from_bits(s.ctx.gpr[A1] as u32);
    let (x, y) = (f32::from_bits(s.ctx.gpr[A2] as u32), f32::from_bits(s.ctx.gpr[A3] as u32));
    call_c(&mut w, 0x70, &[(A1, sext(fr + 0x68)), (A2, sext(fr + 0x6C))], Some(a), imports::func_80014CC0);
    let (sn, c) = (rf(&w, fr + 0x68), rf(&w, fr + 0x6C));
    let z = rf(&w, SP_AT + 0x10);
    let put = |w: &mut State, e: &[(u32, f32)]| {
        for &(o, v) in e {
            wf(w, m + o, v);
        }
    };
    if rf(&w, POLE_UP) <= z {
        put(&mut w, &[(0, c), (0x14, c), (4, sn), (8, 0.0), (0x18, 0.0), (0x20, 0.0), (0x10, neg(sn)?), (0x24, 0.0), (0x28, 1.0)]);
    } else if z <= rf(&w, POLE_DOWN) {
        put(&mut w, &[(0, c), (0x14, c), (4, neg(sn)?), (8, 0.0), (0x18, 0.0), (0x20, 0.0), (0x24, 0.0), (0x28, 1.0), (0x10, sn)]);
    } else {
        let t = sub(1.0, c)?;
        let (sx, sy, sz, yy) = (mul(sn, x)?, mul(sn, y)?, mul(sn, z)?, mul(y, y)?);
        wf(&mut w, fr + 0x64, t);
        wf(&mut w, fr + 0x54, sx);
        wf(&mut w, fr + 0x50, sy);
        wf(&mut w, fr + 0x4C, sz);
        wf(&mut w, fr + 0x30, yy);
        let cyy = mul(c, yy)?;
        let xx = mul(x, x)?;
        wf(&mut w, fr + 0x38, cyy);
        let cxx = mul(c, xx)?;
        let q = sub(sub(1.0, xx)?, yy)?;
        wf(&mut w, fr + 0x34, cxx);
        let oq = sub(1.0, q)?;
        wf(&mut w, fr + 0x28, oq);
        let m00 = add(div(add(mul(cxx, q)?, cyy)?, oq)?, xx)?;
        wf(&mut w, m, m00);
        let m11 = add(div(add(mul(cyy, q)?, cxx)?, oq)?, yy)?;
        wf(&mut w, m + 0x14, m11);
        let m22 = add(add(q, cxx)?, cyy)?;
        wf(&mut w, m + 0x28, m22);
        let txy = mul(t, mul(x, y)?)?;
        put(&mut w, &[(4, add(txy, sz)?), (0x10, sub(txy, sz)?)]);
        let tzx = mul(t, mul(z, x)?)?;
        put(&mut w, &[(8, sub(tzx, sy)?)]);
        let tzy = mul(t, mul(z, y)?)?;
        put(&mut w, &[(0x18, add(tzy, sx)?), (0x24, sub(tzy, sx)?), (0x20, add(tzx, sy)?)]);
    }
    put(&mut w, &[(0x30, 0.0), (0x34, 0.0), (0x38, 0.0), (0xC, 0.0), (0x1C, 0.0), (0x2C, 0.0), (0x3C, 1.0)]);
    Some(w)
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 {
        [0.0, 0.0, 1.0]
    } else {
        v.map(|x| x / l)
    }
}

fn axis_case(seed: u64, a: f32, axis: [f32; 3], poles: Option<(f32, f32)>) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    s.randomise_memory(seed ^ 0x75E0, M, 0x40);
    if let Some((up, down)) = poles {
        wf(&mut s, POLE_UP, up);
        wf(&mut s, POLE_DOWN, down);
    }
    wf(&mut s, SP_AT + 0x10, axis[2]);
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(M), bits(a), bits(axis[0]), bits(axis[1]));
    let want = axis_model(&s);
    prop_assume!(want.is_some());
    let after = run("func_800175E0", math::func_800175E0, &s)?;
    same_memory(&after, &want.unwrap())?;
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Unit axes, near the poles, axes of other lengths; the pole
    /// thresholds perturbed.
    #[test]
    fn func_800175E0(seed: u64, a in angle(), axis in prop_oneof![
                         4 => prop::array::uniform3(-1.0f32..1.0).prop_map(unit),
                         1 => (-0.05f32..0.05, -0.05f32..0.05, prop_oneof![Just(1.0f32), Just(-1.0f32)]).prop_map(|(x, y, s)| unit([x, y, s])),
                         1 => prop::array::uniform3(fin())],
                     poles in prop_oneof![3 => Just(None), 1 => (0.5f32..1.0, -1.0f32..-0.5).prop_map(Some)]) {
        axis_case(seed, a, axis, poles)?;
    }
}

/// Ties: `z` exactly at each pole threshold (read from the ROM image).
#[test]
fn func_800175E0_poles() {
    let rom = |va: u32| {
        let o = (va - 0x8000_0400 + 0x1000) as usize;
        f32::from_bits(u32::from_be_bytes(baserom()[o..o + 4].try_into().unwrap()))
    };
    let (up, down) = (rom(POLE_UP), rom(POLE_DOWN));
    for (i, z) in [up, down, f32::from_bits(up.to_bits() - 1), f32::from_bits(down.to_bits() - 1)].into_iter().enumerate() {
        let r = (1.0 - z * z).max(0.0).sqrt();
        axis_case(0x5E0 + i as u64, 37.0, [r * 0.6, r * 0.8, z], None).unwrap_or_else(|e| panic!("{i}: {e}"));
    }
}

// ---- func_80016F0C: a 4x4's translation and angles ----

fn acos(w: &mut State, frame: u32, x: f32) -> f32 {
    call_c(w, frame, &[], Some(x), imports::func_80014F2C);
    w.ctx.fpr[0].fl()
}

fn angles_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0x80;
    wr(&mut w, fr + 0x1C, s.ctx.gpr[RA] as u32);
    save_double(&mut w, fr + 0x10, s.ctx.fpr[20].u64);
    let (m, out) = (s.ctx.gpr[A0] as u32, s.ctx.gpr[A1] as u32);
    for (from, to) in [(m + 0x30, out), (m + 0x10, fr + 0x74), (m + 0x20, fr + 0x68)] {
        for k in 0..3 {
            let v = word(&w, from + 4 * k);
            wr(&mut w, to + 4 * k, v);
        }
    }
    let n0 = neg(rf(&w, m))?;
    wf(&mut w, fr + 0x5C, n0);
    let n1 = neg(rf(&w, m + 4))?;
    wf(&mut w, fr + 0x60, n1);
    wr(&mut w, SP_AT + 4, out);
    let n2 = neg(rf(&w, m + 8))?;
    wf(&mut w, fr + 0x58, 0.0);
    wf(&mut w, fr + 0x64, n2);
    let (r1x, r1y) = (rf(&w, fr + 0x74), rf(&w, fr + 0x78));
    wf(&mut w, fr + 0x50, r1x);
    wf(&mut w, fr + 0x54, r1y);
    call_c(&mut w, 0x80, &[(A0, sext(fr + 0x50))], None, imports::func_800153C0);
    let l = ok(w.ctx.fpr[0].fl())?;
    let ld = f64::from(l);
    let (k1, k2, k3) = (rd(&w, 0x800A_87F8), rd(&w, 0x800A_8800), rd(&w, 0x800A_8808));
    if !(ld < k1) {
        let q = div(rf(&w, fr + 0x54), l)?;
        if !(1.0 < q) {
            let arg = div(rf(&w, fr + 0x54), l)?;
            wf(&mut w, fr + 0x40, l);
            let a = acos(&mut w, 0x80, arg);
            let v = if 0.0 < rf(&w, fr + 0x74) { neg(a)? } else { a };
            wf(&mut w, out + 0xC, v);
        } else {
            wf(&mut w, out + 0xC, 0.0);
        }
    } else {
        let arg = neg(rf(&w, fr + 0x5C))?;
        wf(&mut w, fr + 0x40, l);
        let a = acos(&mut w, 0x80, arg);
        let (ny, rz) = (0.0 < rf(&w, fr + 0x60), 0.0 < rf(&w, fr + 0x7C));
        let v = if ny != rz { a } else { neg(a)? };
        wf(&mut w, out + 0x14, v);
        wf(&mut w, out + 0xC, 0.0);
    }
    if !(ld < k2) {
        let (vx, vy, vz) = (rf(&w, fr + 0x50), rf(&w, fr + 0x54), rf(&w, fr + 0x58));
        let (rx, ry, rz) = (rf(&w, fr + 0x74), rf(&w, fr + 0x78), rf(&w, fr + 0x7C));
        let dot = add(mul(rz, vz)?, add(mul(vx, rx)?, mul(vy, ry)?)?)?;
        let q = div(dot, l)?;
        if !(1.0 <= q) {
            save_double(&mut w, fr + 0x28, ld.to_bits());
            let a = acos(&mut w, 0x80, q);
            wf(&mut w, out + 0x10, a);
        } else {
            wf(&mut w, out + 0x10, 0.0);
        }
    } else {
        wf(&mut w, out + 0x10, 90.0);
    }
    if rf(&w, fr + 0x7C) < 0.0 {
        let v = neg(rf(&w, out + 0x10))?;
        wf(&mut w, out + 0x10, v);
    }
    wf(&mut w, fr + 0x4C, 0.0);
    let wx = neg(rf(&w, fr + 0x54))?;
    wf(&mut w, fr + 0x44, wx);
    save_double(&mut w, fr + 0x28, ld.to_bits());
    let vx = rf(&w, fr + 0x50);
    wf(&mut w, fr + 0x48, vx);
    call_c(&mut w, 0x80, &[(A0, sext(fr + 0x44))], None, imports::func_800153C0);
    let l2 = w.ctx.fpr[0].fl();
    if k3 <= ld {
        let (wx, wy, wz) = (rf(&w, fr + 0x44), rf(&w, fr + 0x48), rf(&w, fr + 0x4C));
        let (nx, ny, nz) = (rf(&w, fr + 0x5C), rf(&w, fr + 0x60), rf(&w, fr + 0x64));
        let dot = add(mul(nz, wz)?, add(mul(wx, nx)?, mul(wy, ny)?)?)?;
        let q = div(dot, l2)?;
        if !(1.0 <= q) {
            if !(q <= -1.0) {
                let a = acos(&mut w, 0x80, q);
                wf(&mut w, out + 0x14, a);
            } else {
                wf(&mut w, out + 0x14, 180.0);
            }
        } else {
            wf(&mut w, out + 0x14, 0.0);
        }
        if rf(&w, fr + 0x64) < 0.0 {
            let v = neg(rf(&w, out + 0x14))?;
            wf(&mut w, out + 0x14, v);
        }
    }
    Some(w)
}

fn angles_case(seed: u64, mat: &[f32; 16], place: u8, ks: Option<[f64; 3]>) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    if let Some(ks) = ks {
        for (a, k) in [0x800A_87F8u32, 0x800A_8800, 0x800A_8808].iter().zip(ks) {
            wr(&mut s, *a, (k.to_bits() >> 32) as u32);
            wr(&mut s, a + 4, k.to_bits() as u32);
        }
    }
    for (k, v) in mat.iter().enumerate() {
        wf(&mut s, M + 4 * k as u32, *v);
    }
    s.randomise_memory(seed ^ 0x6F0C, OUT, 0x18);
    let out = match place {
        0 => M,
        1 => M + 0x10,
        2 => M + 0x20,
        _ => OUT,
    };
    (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(M), sext(out));
    let want = angles_model(&s);
    prop_assume!(want.is_some());
    let after = run("func_80016F0C", math::func_80016F0C, &s)?;
    same_memory(&after, &want.unwrap())?;
    Ok(())
}

/// A rotation matrix (from func_8001723C's C, so the angles round-trip
/// through the game's own maths), or a random one; rows with a short or
/// zero xy part (the `l < K` paths).
fn rotation(a: [f32; 3], t: [f32; 3]) -> [f32; 16] {
    let mut s = state(1);
    s.ctx.gpr[A0] = sext(M);
    (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (bits(a[0]), bits(a[1]), bits(a[2]));
    s.run(oracle::recomp::by_name("func_8001723C").unwrap());
    let mut m = [0.0f32; 16];
    for (k, v) in m.iter_mut().enumerate() {
        *v = rf(&s, M + 4 * k as u32);
    }
    m[12..15].copy_from_slice(&t);
    m
}

fn matrix() -> BoxedStrategy<[f32; 16]> {
    prop_oneof![
        3 => (prop::array::uniform3(angle()), prop::array::uniform3(fin())).prop_map(|(a, t)| rotation(a, t)),
        2 => prop::array::uniform16(fin()),
        1 => (prop::array::uniform16(fin()), prop::array::uniform2(-1.0e-3f32..1.0e-3)).prop_map(|(mut m, s)| { m[4] = s[0]; m[5] = s[1]; m }),
        1 => (prop::array::uniform16(fin()), any::<bool>()).prop_map(|(mut m, z)| { m[4] = 0.0; m[5] = if z { 0.0 } else { -0.0 }; m }),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(768))]

    #[test]
    fn func_80016F0C(seed: u64, mat in matrix(), place in prop_oneof![3 => Just(3u8), 1 => 0u8..3]) {
        angles_case(seed, &mat, place, None)?;
    }
}

/// Ties: `l` equal to the three thresholds (row 1's xy part `(0.5, 0)`,
/// `l = 0.5` exactly, and the doubles written as 0.5 or the next double
/// either side), the acos arguments at exactly 1 and -1 (row 1 along y, row
/// 0 along x), signs at `+-0`.
#[test]
fn func_80016F0C_cases() {
    let half = 0.5f64;
    let mut cases = Vec::new();
    for k in [half, f64::from_bits(half.to_bits() + 1), f64::from_bits(half.to_bits() - 1)] {
        let mut m = rotation([10.0, 20.0, 30.0], [1.0, 2.0, 3.0]);
        (m[4], m[5], m[6]) = (0.5, 0.0, 0.7);
        cases.push((m, Some([k; 3])));
        (m[4], m[5]) = (0.0, -0.5);
        cases.push((m, Some([k; 3])));
    }
    let mut m = rotation([0.0, 0.0, 0.0], [0.0; 3]);
    (m[4], m[5], m[6]) = (0.0, 1.0, 0.0);
    for row0 in [[1.0f32, 0.0, 0.0], [-1.0, 0.0, 0.0], [1.0, 0.0, -0.0], [0.6, 0.8, 0.0]] {
        m[0..3].copy_from_slice(&row0);
        cases.push((m, None));
    }
    (m[4], m[5], m[6]) = (-0.0, 2.0, -0.0);
    cases.push((m, None));
    (m[4], m[5], m[6]) = (0.0, -1.0, 0.0);
    cases.push((m, None));
    for (i, (m, ks)) in cases.iter().enumerate() {
        angles_case(0x6F0 + i as u64, m, 3, *ks).unwrap_or_else(|e| panic!("{i}: {e}"));
    }
}
