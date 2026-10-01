//! Depth 2: func_80004160 (game::collide), the collision query's hit
//! applied to a sphere. Recompiled C vs Rust with the callees as C. The
//! model replays the callees' C on a copy of the state in the same order
//! with the same arguments (and the s registers the port holds), adding the
//! function's own stores; whole RDRAM and `v0` are compared. Its sign
//! tests `dd` and `sn` only pick a branch, so each fused multiply-add there
//! gets a constructed separator: an exact cancellation of the unfused sum,
//! the product's rounding error on the side that flips the branch, searched
//! at test time. The third, `fg = F . G`, has none: `Y = (D x s) x s` lies
//! in the plane of `s` and `D`, so do `B` and `E`, and `F = B x D`, `G = E x
//! D` are both parallel to `s x D`. The products `F_i G_i` share a sign
//! unless `G` (or `F`) is rounding noise, and a search of about two
//! million `u` around 197 zeros of `fg` (NOTES, session 14) found no fused
//! sum of another sign.

// Tests are named after the functions (func_80004160), capitals included.
#![allow(non_snake_case)]

use difftest::rom::baserom;
use difftest::{compare, State};
use game::collide;
use game::imports;
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
const FR: u32 = SP_AT - 0xC0;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

/// The game's data segment (`0x80098000..0x800AE8B0`) as the ROM image has
/// it: the callees read constants from there.
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

fn v3(s: &State, a: u32) -> [f32; 3] {
    [0, 1, 2].map(|k| rf(s, a + 4 * k))
}

fn put3(s: &mut State, a: u32, v: [f32; 3]) {
    for (k, x) in v.iter().enumerate() {
        wf(s, a + 4 * k as u32, *x);
    }
}

/// Runs a callee's C on `w` with `sp` at the port's frame and `args` set.
fn call_c(w: &mut State, args: &[(usize, u64)], f: RecompFn) {
    for &(r, v) in args {
        w.ctx.gpr[r] = v;
    }
    w.ctx.gpr[SP] = sext(FR);
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
fn div(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? / ok(b)?)
}
fn neg(a: f32) -> Option<f32> {
    Some(-ok(a)?)
}
fn sqrt(a: f32) -> Option<f32> {
    ok(ok(a)?.sqrt())
}
fn bits(x: f32) -> u64 {
    sext(x.to_bits())
}

const PB: u32 = 0x8030_0000;
const SB: u32 = 0x8030_0100;
const NB: u32 = 0x8030_0200;
const EB: u32 = 0x8030_0300;
const KB: u32 = 0x8030_0400;
const TRACKED: u32 = 0x800A_E8B0;
const KPT: u32 = TRACKED + 8;
const DIR: u32 = TRACKED + 0x18;
const COPY: u32 = 0x800A_E8D8;
const R2: u32 = 0x800A_E8DC;
const DEST: u32 = 0x8009_A280;
const L0: u32 = 0x800A_813C;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Path {
    Miss,
    /// `l < L0`; whether `dd < 0`.
    Short(bool),
    /// `fg >= 0`; whether `0 < sn`.
    Push(bool),
    /// `fg < 0`.
    Back,
}

#[derive(Clone, Debug)]
struct Hit {
    p: [f32; 3],
    k: [f32; 3],
    dir: [f32; 3],
    s: [f32; 3],
    r: f32,
    r2: f32,
    t: f32,
    h: f32,
    u: f32,
    copy: u32,
    place: u8,
    l0: Option<f32>,
}

/// `(p, s, N, e, k)` addresses: own buffers, or `e` over `N` (shifted by a
/// word too), `k` over `N` or `e`, `p` or `s` over `N` (written first).
fn places(k: u8) -> (u32, u32, u32, u32, u32) {
    match k {
        1 => (PB, SB, NB, NB, KB),
        2 => (PB, SB, NB, NB + 4, KB),
        3 => (PB, SB, NB, EB, NB),
        4 => (PB, SB, NB, EB, EB),
        5 => (NB, SB, NB, EB, KB),
        6 => (PB, NB, NB, EB, KB),
        7 => (PB, SB, NB, PB, KB),
        _ => (PB, SB, NB, EB, KB),
    }
}

fn put(seed: u64, x: &Hit) -> State {
    let mut s = state(seed);
    load_data(&mut s);
    s.randomise_memory(seed ^ 0x4160, TRACKED, 0x40);
    if let Some(l0) = x.l0 {
        wf(&mut s, L0, l0);
    }
    let (p, sv, n, e, k) = places(x.place);
    for a in [EB, KB, NB] {
        s.randomise_memory(seed ^ u64::from(a), a, 0x10);
    }
    put3(&mut s, sv, x.s);
    put3(&mut s, p, x.p);
    wf(&mut s, TRACKED, x.t);
    put3(&mut s, KPT, x.k);
    put3(&mut s, DIR, x.dir);
    wr(&mut s, COPY, x.copy);
    wf(&mut s, R2, x.r2);
    wf(&mut s, SP_AT + 0x10, x.u);
    wr(&mut s, SP_AT + 0x14, n);
    wr(&mut s, SP_AT + 0x18, e);
    wr(&mut s, SP_AT + 0x1C, k);
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(p), bits(x.r), sext(sv), bits(x.h));
    s
}

/// `p += D` (D in the frame), then `e = -N`, component by component.
fn push_out(w: &mut State, p: u32, n: u32, e: u32) -> Option<()> {
    for k in 0..3 {
        let v = add(rf(w, p + 4 * k), rf(w, FR + 0xB4 + 4 * k))?;
        wf(w, p + 4 * k, v);
    }
    for k in 0..3 {
        let v = neg(rf(w, n + 4 * k))?;
        wf(w, e + 4 * k, v);
    }
    Some(())
}

fn check_scale(w: &State, v: u32, c: f32) -> Option<()> {
    for k in 0..3 {
        mul(rf(w, v + 4 * k), c)?;
    }
    Some(())
}

/// `out = q * c + p`'s domain (func_800155EC).
fn check_madd(w: &State, p: u32, c: f32, q: u32) -> Option<()> {
    for k in 0..3 {
        add(mul(rf(w, q + 4 * k), c)?, rf(w, p + 4 * k))?;
    }
    Some(())
}

/// The steps up to `l = |(D x s) x s|` on `w` (the frame saved): `N`
/// copied, `D` normalised, the two crosses (their domains checked) and the
/// length.
fn to_length(w: &mut State, s: &State) -> Option<f32> {
    let (p, sv) = (s.ctx.gpr[A0] as u32, s.ctx.gpr[A2] as u32);
    let n = word(s, SP_AT + 0x14);
    for k in 0..3 {
        let v = word(w, DIR + 4 * k);
        wr(w, n + 4 * k, v);
    }
    for k in 0..3 {
        let d = sub(rf(w, p + 4 * k), rf(w, KPT + 4 * k))?;
        wf(w, FR + 0xB4 + 4 * k, d);
    }
    (w.ctx.gpr[S0], w.ctx.gpr[S1], w.ctx.gpr[S2]) = (sext(p), sext(n), sext(sv));
    call_c(w, &[(A0, sext(FR + 0xB4))], imports::func_800154D0);
    if v3(w, FR + 0xB4).iter().chain(v3(w, sv).iter()).any(|x| !x.is_finite()) {
        return None;
    }
    call_c(w, &[(A0, sext(FR + 0x6C)), (A1, sext(FR + 0xB4)), (A2, sext(sv))], imports::func_80015538);
    call_c(w, &[(A0, sext(FR + 0x90)), (A1, sext(FR + 0x6C)), (A2, sext(sv))], imports::func_80015538);
    if v3(w, FR + 0x90).iter().any(|x| !x.is_finite()) {
        return None;
    }
    call_c(w, &[(A0, sext(FR + 0x90))], imports::func_800153C0);
    Some(w.ctx.fpr[0].fl())
}

fn push_model(s: &State) -> Option<(State, Path)> {
    let mut w = s.clone();
    let regs = [(0x20, S2), (0x18, S0), (0x24, RA), (0x1C, S1), (0xC4, A1), (0xCC, A3)];
    for (off, r) in regs {
        wr(&mut w, FR + off, s.ctx.gpr[r] as u32);
    }
    if !(rf(s, TRACKED) < rf(s, R2)) {
        return Some((w, Path::Miss));
    }
    let l = to_length(&mut w, s)?;
    let (p, sv) = (s.ctx.gpr[A0] as u32, s.ctx.gpr[A2] as u32);
    let (n, e, kp) = (word(s, SP_AT + 0x14), word(s, SP_AT + 0x18), word(s, SP_AT + 0x1C));
    let (r, h, u) = (f32::from_bits(s.ctx.gpr[A1] as u32), f32::from_bits(s.ctx.gpr[A3] as u32), rf(s, SP_AT + 0x10));
    let path;
    if !(l < rf(&w, L0)) {
        let inv = div(1.0, l)?;
        check_scale(&w, FR + 0x90, inv)?;
        call_c(&mut w, &[(A0, sext(FR + 0x90)), (A1, bits(inv)), (A2, sext(FR + 0x90))], imports::func_800155C0);
        let a = sqrt(sub(rf(&w, R2), mul(h, h)?)?)?;
        check_scale(&w, FR + 0x90, a)?;
        call_c(&mut w, &[(A0, sext(FR + 0x84)), (A1, bits(a)), (A2, sext(FR + 0x90))], imports::func_800155C0);
        check_madd(&w, FR + 0x84, h, sv)?;
        call_c(&mut w, &[(A0, sext(FR + 0x58)), (A1, sext(FR + 0x84)), (A2, bits(h)), (A3, sext(sv))], imports::func_800155EC);
        let c = sqrt(sub(rf(&w, R2), mul(u, u)?)?)?;
        check_scale(&w, FR + 0x90, c)?;
        call_c(&mut w, &[(A0, sext(FR + 0x78)), (A1, bits(c)), (A2, sext(FR + 0x90))], imports::func_800155C0);
        check_madd(&w, FR + 0x78, u, sv)?;
        call_c(&mut w, &[(A0, sext(FR + 0x4C)), (A1, sext(FR + 0x78)), (A2, bits(u)), (A3, sext(sv))], imports::func_800155EC);
        if v3(&w, FR + 0x58).iter().chain(v3(&w, FR + 0x4C).iter()).any(|x| !x.is_finite()) {
            return None;
        }
        call_c(&mut w, &[(A0, sext(FR + 0x40)), (A1, sext(FR + 0x58)), (A2, sext(FR + 0xB4))], imports::func_80015538);
        call_c(&mut w, &[(A0, sext(FR + 0x34)), (A1, sext(FR + 0x4C)), (A2, sext(FR + 0xB4))], imports::func_80015538);
        let (f, g) = (v3(&w, FR + 0x40), v3(&w, FR + 0x34));
        let fg = add(mul(g[2], f[2])?, add(mul(f[0], g[0])?, mul(f[1], g[1])?)?)?;
        if !(fg < 0.0) {
            let (sw, nv) = (v3(&w, sv), v3(&w, n));
            let sn = add(mul(nv[2], sw[2])?, add(mul(sw[0], nv[0])?, mul(sw[1], nv[1])?)?)?;
            let src = if 0.0 < sn { FR + 0x4C } else { FR + 0x58 };
            for k in 0..3 {
                let v = add(rf(&w, src + 4 * k), rf(&w, p + 4 * k))?;
                wf(&mut w, FR + 0xA8 + 4 * k, v);
            }
            for k in 0..3 {
                let v = word(&w, FR + 0x4C + 4 * k);
                wr(&mut w, e + 4 * k, v);
            }
            call_c(&mut w, &[(A0, sext(e))], imports::func_800154D0);
            let (kv, nv, pv) = (v3(&w, KPT), v3(&w, n), v3(&w, FR + 0xA8));
            let kd = add(add(mul(kv[0], nv[0])?, mul(kv[1], nv[1])?)?, mul(kv[2], nv[2])?)?;
            let td = add(mul(nv[2], pv[2])?, add(mul(pv[0], nv[0])?, mul(pv[1], nv[1])?)?)?;
            let c = sub(kd, td)?;
            check_madd(&w, p, c, n)?;
            call_c(&mut w, &[(A0, sext(p)), (A1, sext(p)), (A2, bits(c)), (A3, sext(n))], imports::func_800155EC);
            path = Path::Push(0.0 < sn);
        } else {
            let c = sub(r, sqrt(rf(&w, TRACKED))?)?;
            check_scale(&w, FR + 0xB4, c)?;
            call_c(&mut w, &[(A0, sext(FR + 0xB4)), (A1, bits(c)), (A2, sext(FR + 0xB4))], imports::func_800155C0);
            push_out(&mut w, p, n, e)?;
            path = Path::Back;
        }
    } else {
        let (sw, d) = (v3(&w, sv), v3(&w, FR + 0xB4));
        let dd = add(mul(sw[2], d[2])?, add(mul(d[0], sw[0])?, mul(d[1], sw[1])?)?)?;
        let st = sqrt(rf(&w, TRACKED))?;
        let c = if !(dd < 0.0) { sub(neg(u)?, st)? } else { neg(sub(h, st)?)? };
        check_scale(&w, n, c)?;
        call_c(&mut w, &[(A0, sext(FR + 0xB4)), (A1, bits(c)), (A2, sext(n))], imports::func_800155C0);
        push_out(&mut w, p, n, e)?;
        path = Path::Short(dd < 0.0);
    }
    for k in 0..3 {
        let v = word(&w, KPT + 4 * k);
        wr(&mut w, kp + 4 * k, v);
    }
    let v = word(&w, COPY);
    if v != 0 {
        wr(&mut w, DEST, v);
    }
    Some((w, path))
}

fn push_case(seed: u64, x: &Hit) -> Result<Path, TestCaseError> {
    let s = put(seed, x);
    let model = push_model(&s);
    prop_assume!(model.is_some());
    let (want, path) = model.unwrap();
    let after = run("func_80004160", collide::func_80004160, &s)?;
    same_memory(&after, &want)?;
    prop_assert_eq!(after.ctx.gpr[V0], u64::from(path != Path::Miss));
    Ok(path)
}

fn path_of(x: &Hit) -> Option<Path> {
    push_model(&put(1, x)).map(|(_, p)| p)
}

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

fn fin3() -> BoxedStrategy<[f32; 3]> {
    prop::array::uniform3(fin()).boxed()
}

/// A hit at `k` with `p` inside the sphere (or anywhere), `s` of any
/// length (short or along `p - k` for the `l < L0` path), `h` and `u`
/// inside the radius mostly, `T` around `|p - k|^2`, `R2 = r * r` or not.
fn hit() -> BoxedStrategy<Hit> {
    (
        (fin3(), prop::array::uniform3(-1.0f32..1.0), prop_oneof![3 => Just(None), 1 => fin3().prop_map(Some)], fin3()),
        (prop_oneof![4 => 0.5f32..20.0, 1 => fin()], prop_oneof![4 => Just(None), 1 => fin().prop_map(Some)], prop_oneof![3 => Just(None), 1 => fin().prop_map(Some), 1 => Just(Some(0.0f32))]),
        (prop_oneof![3 => -0.99f32..0.99, 1 => -1.5f32..1.5], prop_oneof![3 => -0.99f32..0.99, 1 => -1.5f32..1.5]),
        (prop_oneof![3 => fin3(), 1 => prop::array::uniform3(-0.05f32..0.05)], prop_oneof![4 => Just(None), 1 => (-3.0f32..3.0).prop_map(Some)]),
        (prop_oneof![Just(0u32), any::<u32>()], 0u8..10, prop_oneof![5 => Just(None), 1 => (0.0f32..0.1).prop_map(Some)]),
    )
        .prop_map(|((k, off, pa, dir), (r, r2, t), (hf, uf), (s, along), (copy, place, l0))| {
            let p = pa.unwrap_or([0, 1, 2].map(|c| k[c] + off[c] * r));
            let d = [0, 1, 2].map(|c| p[c] - k[c]);
            let s = along.map_or(s, |a| d.map(|x| x * a));
            let r2 = r2.unwrap_or(r * r);
            let t = t.unwrap_or(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
            Hit { p, k, dir, s, r, r2, t, h: hf * r, u: uf * r, copy, place, l0 }
        })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1536))]

    #[test]
    fn func_80004160(seed: u64, x in hit()) {
        push_case(seed, &x)?;
    }
}

/// A factor pair (from `seed`) whose f32 product is inexact, with its
/// rounding error (exact - rounded) of the sign `up`.
fn inexact(seed: u32, up: bool, scale: f32) -> (f32, f32) {
    for k in 0..10_000u32 {
        let a = (1.1f32 + (seed.wrapping_mul(7919).wrapping_add(k) % 1000) as f32 * 0.013_7) * scale;
        let b = 3.3f32 + (k % 997) as f32 * 0.021_1;
        let err = f64::from(a) * f64::from(b) - f64::from(a * b);
        if (err > 0.0) == up && err != 0.0 {
            return (a, b);
        }
    }
    panic!("no inexact product found");
}

fn base() -> Hit {
    Hit {
        p: [0.0, 0.0, 0.5],
        k: [0.0, 0.0, 0.0],
        dir: [0.0, 0.0, 1.0],
        s: [1.0, 0.3, 0.8],
        r: 2.0,
        r2: 4.0,
        t: 0.25,
        h: 1.5,
        u: 0.5,
        copy: 0x1234_5678,
        place: 0,
        l0: None,
    }
}

/// Runs `x` until it takes `want`'s path, trying `p` offsets and `h`, `u`.
fn steer(x: Hit, want: fn(Path) -> bool) -> Hit {
    for off in [[0.0f32, 0.0, 0.5], [0.3, 0.4, 1.2], [-0.5, 0.2, 0.1], [0.2, -0.6, -0.3], [0.9, 0.1, 0.4]].iter() {
        for (h, u) in [(1.5f32, 0.5f32), (-1.5, 0.5), (0.5, -1.5), (1.0, 1.0), (-1.0, -1.2), (0.2, 1.8)] {
            let y = Hit { p: *off, t: off.iter().map(|v| v * v).sum(), h, u, ..x.clone() };
            if path_of(&y).is_some_and(want) {
                return y;
            }
        }
    }
    panic!("no configuration takes the wanted path: {x:?}");
}

/// The sign test `0 < sn`, `sn = N.z*s.z + (s.x*N.x + s.y*N.y)`: two terms
/// cancel exactly and the third product (or the cancelled one) is inexact
/// with a positive error, so its fused sum is positive.
#[test]
fn func_80004160_sn_separators() {
    let mut cases = Vec::new();
    let (a, b) = inexact(1, true, 1.0);
    cases.push(Hit { dir: [1.0, 1.0, a], s: [-(a * b), 0.0, b], ..base() });
    let (a, b) = inexact(2, true, 1.0);
    cases.push(Hit { dir: [a, 1.0, 0.0], s: [b, -(a * b), 0.7], ..base() });
    let (a, b) = inexact(3, true, 1.0);
    cases.push(Hit { dir: [1.0, a, 0.0], s: [-(a * b), b, -0.7], ..base() });
    for (i, x) in cases.into_iter().enumerate() {
        let x = steer(x, |p| p == Path::Push(false));
        assert_eq!(push_case(0x5A + i as u64, &x).unwrap_or_else(|e| panic!("{i}: {e}")), Path::Push(false));
    }
}

/// `D` (`p - K` normalised by func_800154D0's C) for `x`.
fn frame_d(x: &Hit) -> [f32; 3] {
    let s = put(2, x);
    let mut w = s.clone();
    to_length(&mut w, &s).unwrap();
    v3(&w, FR + 0xB4)
}

/// `v` near `target` with `fl(v * f) == want` exactly, or None.
fn solve(f: f32, want: f32) -> Option<f32> {
    let near = want / f;
    (-200i32..200).map(|k| f32::from_bits((near.to_bits() as i32 + k) as u32)).find(|v| v * f == want)
}

/// The sign test `dd < 0` on the `l < L0` path (`s` short): `dd = s.z*D.z +
/// (D.x*s.x + D.y*s.y)` with two terms cancelling exactly and the fused
/// product's error negative, so the fused sum turns negative.
#[test]
fn func_80004160_dd_separators() {
    // u = 0.3: with base()'s u = 0.5 both branches scale N by -1.0 (h - u =
    // 2 sqrt(T)), and a flipped branch would leave the same result.
    let flat = Hit { p: [0.3, 0.4, 0.0], t: 0.25, s: [0.0, 0.0, 0.0], u: 0.3, ..base() };
    let d = frame_d(&flat);
    let tilt = Hit { p: [0.3, 0.4, 1.2], t: 1.69, ..flat.clone() };
    let dt = frame_d(&tilt);
    assert!(d[0] != 0.0 && d[1] != 0.0 && d[2] == 0.0 && dt[2] != 0.0, "{d:?} {dt:?}");
    let err = |a: f32, b: f32| f64::from(a) * f64::from(b) - f64::from(a * b);
    let mut cases = Vec::new();
    // Fused D.x*s.x: fl(D.y*s.y) = -fl(D.x*s.x).
    for k in 0..400u32 {
        let a = 0.011f32 + k as f32 * 0.000_17;
        if err(d[0], a) < 0.0 {
            if let Some(b) = solve(d[1], -(d[0] * a)) {
                cases.push(Hit { s: [a, b, 0.0], ..flat.clone() });
                break;
            }
        }
    }
    // Fused D.y*s.y.
    for k in 0..400u32 {
        let b = 0.013f32 + k as f32 * 0.000_19;
        if err(d[1], b) < 0.0 {
            if let Some(a) = solve(d[0], -(d[1] * b)) {
                cases.push(Hit { s: [a, b, 0.0], ..flat.clone() });
                break;
            }
        }
    }
    // Fused s.z*D.z against D.x*s.x (s.y = 0).
    for k in 0..400u32 {
        let a = 0.012f32 + k as f32 * 0.000_23;
        if let Some(c) = solve(dt[2], -(dt[0] * a)) {
            if err(c, dt[2]) < 0.0 {
                cases.push(Hit { s: [a, 0.0, c], ..tilt.clone() });
                break;
            }
        }
    }
    assert_eq!(cases.len(), 3, "a dd separator wasn't found");
    for (i, x) in cases.iter().enumerate() {
        assert_eq!(push_case(0xDD + i as u64, x).unwrap_or_else(|e| panic!("{i}: {e}")), Path::Short(false), "{i}");
    }
}

/// Ties: `T = R2` (a miss), `l` equal to the written threshold, every
/// path once with each aliasing placement.
#[test]
fn func_80004160_cases() {
    let mut cases = vec![Hit { t: 4.0, ..base() }, Hit { t: 4.0, r2: 4.0, h: 0.0, ..base() }];
    // l = L0 exactly: the model's l (func_800153C0's C) written as L0.
    let probe = put(4, &base());
    let l = to_length(&mut probe.clone(), &probe).unwrap();
    cases.push(Hit { l0: Some(l), ..base() });
    cases.push(Hit { l0: Some(f32::from_bits(l.to_bits() + 1)), ..base() });
    for place in 0..8 {
        for s in [[1.0f32, 0.3, 0.8], [1.0, 0.3, -0.8], [0.001, 0.002, 0.0], [0.0, 0.0, 0.5], [0.0, 0.0, -0.5]] {
            for (h, u) in [(1.5f32, 0.5f32), (-1.5, -0.5), (0.5, -1.9)] {
                cases.push(Hit { place, s, h, u, ..base() });
            }
        }
    }
    let mut seen = Vec::new();
    for (i, x) in cases.iter().enumerate() {
        let p = push_case(0xCA5E + i as u64, x).unwrap_or_else(|e| panic!("case {i}: {e}"));
        if !seen.contains(&p) {
            seen.push(p);
        }
    }
    for p in [Path::Miss, Path::Short(false), Path::Short(true), Path::Push(false), Path::Push(true), Path::Back] {
        assert!(seen.contains(&p), "no case takes {p:?}: {seen:?}");
    }
}
