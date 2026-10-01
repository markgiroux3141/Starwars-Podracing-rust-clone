//! Depth 2 at 0x8000EEE0..0x80014D20 (game::misc, game::math): a HUD record
//! scaled about the screen centre and faded, the screen markers of 40
//! flagged points and of the tracked points (projection, on-screen tests,
//! a size by distance, two record lists), the listed-record count and
//! reset, and tan in degrees. Recompiled C vs Rust with the callees as C.
//! The models replay the callees' C on a copy of the state in the same
//! order with the same arguments (stack arguments stored first), adding
//! the functions' own stores; whole RDRAM (and `f0` where there is a
//! result) is compared.

// Tests are named after the functions (func_8000EEE0), capitals included.
#![allow(non_snake_case)]

use difftest::rom::baserom;
use difftest::{compare, State};
use game::imports;
use game::misc::{HANDLE_SEGMENTS, RECORDS};
use game::recomp::{fpu, reg::*, RecompFn};
use game::{math, misc};
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

fn sext16(v: u32) -> u64 {
    v as u16 as i16 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    load_data(&mut s);
    s.randomise_memory(seed ^ 0x2EC0, RECORDS, 64 * 32);
    s
}

/// The game's data segment (`0x80098000..0x800AE8B0`) as the ROM image has
/// it: the functions and callees read constants from there.
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

/// `sd` of the FPRs `f20`..`f30` from `at` down (`f30` at `at`): high word
/// first.
fn save_fprs(w: &mut State, s: &State, frame: u32, at: u32) {
    for (k, r) in [30usize, 28, 26, 24, 22, 20].iter().enumerate() {
        let a = SP_AT - frame + at - 8 * k as u32;
        let v = s.ctx.fpr[*r].u64;
        wr(w, a, (v >> 32) as u32);
        wr(w, a + 4, v as u32);
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

/// The C cast (`trunc.w.s`): 0x80000000 out of range and for NaN.
fn trunc(x: f32) -> u32 {
    fpu::trunc_w_s(x)
}

/// IDO's float-to-unsigned idiom as the oracle runs it: toward zero, a
/// negative word (or the out-of-range 0) as the conversion gives it, then
/// `0xFFFFFFFF` for a negative word (NOTES, "FPU control register").
fn to_unsigned(x: f32) -> u32 {
    let v = fpu::cvt_w_s(x, fpu::TO_ZERO);
    if (v as i32) < 0 {
        u32::MAX
    } else {
        v
    }
}

fn f32w(v: i32) -> f32 {
    fpu::cvt_s_w(v as u32, fpu::NEAREST)
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

const SCREEN: u32 = 0x8011_4470;

/// A record id word: none (-1), a record, a special id, or anything whose
/// low halfword is negative.
fn record_word() -> BoxedStrategy<u32> {
    prop_oneof![
        2 => Just(u32::MAX),
        4 => 0u32..64,
        1 => prop_oneof![Just(-103i32), Just(-104), Just(-201)].prop_map(|v| v as u32),
        1 => (any::<u16>(), 0x8000u16..).prop_map(|(h, l)| u32::from(h) << 16 | u32::from(l)),
    ]
    .boxed()
}

fn screen() -> BoxedStrategy<(i16, i16)> {
    prop_oneof![4 => Just((320i16, 240i16)), 1 => (prop_oneof![1i16..400, -400i16..0], prop_oneof![1i16..300, -300i16..0])].boxed()
}

// ---- func_8000EEE0: a HUD record scaled and faded ----

#[derive(Clone, Debug)]
struct Hud {
    a0: u64,
    x: i32,
    y: i32,
    scale: f32,
    k: f32,
    t: f32,
    rgb: [u32; 3],
    wh: (i16, i16),
    consts: Option<(f32, f32)>,
}

fn hud() -> BoxedStrategy<Hud> {
    (
        (any::<u64>(), prop_oneof![0i16..64, Just(-103i16), Just(-104i16), Just(-201i16), any::<i16>()]),
        (prop_oneof![-2000i32..2000, any::<i32>()], prop_oneof![-2000i32..2000, any::<i32>()]),
        (fin(), prop_oneof![fin(), 0.5f32..2.0], prop_oneof![Just(0.0f32), Just(1.0f32), 0.0f32..1.0, fin(), Just(-1.0f32), Just(33.0f32)]),
        (any::<[u32; 3]>(), screen(), prop_oneof![3 => Just(None), 1 => (fin(), fin()).prop_map(Some)]),
    )
        .prop_map(|((hi, id), (x, y), (scale, k, t), (rgb, wh, consts))| Hud {
            a0: hi & !0xFFFF | u64::from(id as u16),
            x,
            y,
            scale,
            k,
            t,
            rgb,
            wh,
            consts,
        })
        .boxed()
}

fn put_hud(s: &mut State, h: &Hud) {
    wh(s, SCREEN, h.wh.0 as u16);
    wh(s, SCREEN + 2, h.wh.1 as u16);
    if let Some((k1, k2)) = h.consts {
        wf(s, 0x800A_8688, k1);
        wf(s, 0x800A_868C, k2);
    }
    wf(s, SP_AT + 0x10, h.k);
    wf(s, SP_AT + 0x14, h.t);
    for (i, c) in h.rgb.iter().enumerate() {
        wr(s, SP_AT + 0x18 + 4 * i as u32, *c);
    }
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (h.a0, sext(h.x as u32), sext(h.y as u32), sext(h.scale.to_bits()));
}

/// `trunc(f32(half) + f32(v - half) * k)` as the s16 argument.
fn scaled(v: i32, half: i32, k: f32) -> Option<u64> {
    let t = trunc(add(f32w(half), mul(f32w(v.wrapping_sub(half)), k)?)?);
    Some(sext16(t))
}

fn hud_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0x28;
    saves(&mut w, s, 0x28, &[(0x24, RA), (0x20, S0), (0x28, A0), (0x2C, A1), (0x30, A2), (0x34, A3)]);
    let id = sext16(s.ctx.gpr[A0] as u32);
    let (x, y) = (s.ctx.gpr[A1] as u32 as i32, s.ctx.gpr[A2] as u32 as i32);
    let (big_w, big_h) = (i32::from(half(s, SCREEN) as i16), i32::from(half(s, SCREEN + 2) as i16));
    let k = rf(s, SP_AT + 0x10);
    let (xa, ya) = (scaled(x, big_w / 2, k)?, scaled(y, big_h / 2, k)?);
    w.ctx.gpr[S0] = id;
    call_c(&mut w, 0x28, &[(A0, id), (A1, xa), (A2, ya)], imports::func_8000E680);
    let t = rf(&w, SP_AT + 0x14);
    let alpha = to_unsigned(mul(t, 130.0)?);
    wr(&mut w, fr + 0x10, alpha);
    let rgb = [0, 1, 2].map(|i| u64::from(word(&w, SP_AT + 0x18 + 4 * i) & 0xFF));
    call_c(&mut w, 0x28, &[(A0, id), (A1, rgb[0]), (A2, rgb[1]), (A3, rgb[2])], imports::func_8000AB24);
    let size = add(mul(sub(1.0, rf(&w, SP_AT + 0x14))?, rf(&w, 0x800A_8688))?, rf(&w, 0x800A_868C))?;
    let v = mul(rf(&w, SP_AT + 0xC), size)?;
    call_c(&mut w, 0x28, &[(A0, id), (A1, sext(v.to_bits())), (A2, sext(v.to_bits()))], imports::func_8000AAC0);
    call_c(&mut w, 0x28, &[(A0, id), (A1, 1)], imports::func_8000A920);
    Some(w)
}

fn hud_case(seed: u64, h: &Hud) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    put_hud(&mut s, h);
    let want = hud_model(&s);
    prop_assume!(want.is_some());
    let after = run("func_8000EEE0", misc::func_8000EEE0, &s)?;
    same_memory(&after, &want.unwrap())?;
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Screens of either sign and odd sizes (halved toward zero), ids of
    /// every kind with any high half, alphas past 255 and negative, the
    /// size constants perturbed.
    #[test]
    fn func_8000EEE0(seed: u64, h in hud()) {
        hud_case(seed, &h)?;
    }
}

/// Separators for the fused `f32(half) + f32(v - half) * k` before the
/// truncation: the floats around `(n - half) / (v - half)` are searched
/// here for one whose unfused and fused sums (the fused one exact in f64
/// before its rounding) truncate differently, for `x` and for `y`.
#[test]
fn func_8000EEE0_separators() {
    let base = Hud { a0: 7, x: 0, y: 0, scale: 1.0, k: 1.0, t: 0.5, rgb: [1, 2, 3], wh: (320, 240), consts: None };
    let mut cases = Vec::new();
    for (axis, half) in [(0, 160i32), (1, 120)] {
        let mut found = 0;
        'search: for v in [-900i32, -1700, 2500, -333] {
            let d = f32w(v - half);
            for n in -600i32..600 {
                let near = ((n - half) as f32) / d;
                for u in -150i32..150 {
                    let k = f32::from_bits((near.to_bits() as i32 + u) as u32);
                    let unfused = f32w(half) + d * k;
                    let fused = (f64::from(half) + f64::from(d) * f64::from(k)) as f32;
                    if trunc(unfused) != trunc(fused) {
                        let mut h = base.clone();
                        h.k = k;
                        if axis == 0 {
                            h.x = v;
                        } else {
                            h.y = v;
                        }
                        cases.push(h);
                        found += 1;
                        if found == 3 {
                            break 'search;
                        }
                        break;
                    }
                }
            }
        }
        assert!(found > 0, "no separator for axis {axis}");
    }
    for (i, h) in cases.iter().enumerate() {
        hud_case(0x8EE + i as u64, h).unwrap_or_else(|e| panic!("{i}: {e}"));
    }
}

// ---- func_800117A4 / func_800117F0: the listed records ----

fn put_list(s: &mut State, list: &[u32]) {
    for (i, v) in list.iter().enumerate() {
        wr(s, 0x800D_6140 + 4 * i as u32, *v);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_800117A4(seed: u64, n in prop_oneof![Just(80i32), Just(81i32), 0i32..100, any::<i32>()], hi: u32, list in prop::collection::vec(record_word(), 80)) {
        let mut s = state(seed);
        put_list(&mut s, &list);
        s.ctx.gpr[A0] = if hi & 1 == 0 { sext(n as u32) } else { u64::from(hi) << 32 | u64::from(n as u32) };
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        let a0 = s.ctx.gpr[A0];
        wr(&mut w, 0x8009_B884, if (a0 as i64) < 0x51 { a0 as u32 } else { 0x50 });
        call_c(&mut w, 0x18, &[], imports::func_800116E8);
        let after = run("func_800117A4", misc::func_800117A4, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_800117F0(seed: u64, list in prop::collection::vec(record_word(), 80), b: u32) {
        let mut s = state(seed);
        put_list(&mut s, &list);
        wr(&mut s, 0x8009_B870, b);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        wr(&mut w, 0x8009_B870, b & 0x00FF_FFFF);
        call_c(&mut w, 0x18, &[], imports::func_800116E8);
        let after = run("func_800117F0", misc::func_800117F0, &s)?;
        same_memory(&after, &w)?;
    }

    /// Angles of every size and sign, multiples of 90 (cosine 0 or near).
    #[test]
    fn func_80014D20(seed: u64, deg in prop_oneof![-720.0f32..720.0, (-8i32..8).prop_map(|n| n as f32 * 90.0), (-8i32..8).prop_map(|n| n as f32 * 45.0), float().prop_filter("moderate", |x| x.abs() < 1.0e6)]) {
        let mut s = state(seed);
        s.ctx.fpr[12].set_fl(deg);
        let mut w = s.clone();
        let fr = SP_AT - 0x28;
        saves(&mut w, &s, 0x28, &[(0x14, RA)]);
        call_c(&mut w, 0x28, &[(A1, sext(fr + 0x20)), (A2, sext(fr + 0x1C))], imports::func_80014CC0);
        let q = div(rf(&w, fr + 0x20), rf(&w, fr + 0x1C));
        prop_assume!(q.is_some());
        let after = run("func_80014D20", math::func_80014D20, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), q.unwrap().to_bits());
    }
}

// ---- func_800105DC / func_80010B34: screen markers ----

const MATRIX: u32 = 0x8011_2E20;
const CAMERA: u32 = 0x800A_3FDC;
const MIRROR: u32 = 0x800D_697C;
const FADE: u32 = 0x800A_26F4;
const VP: u32 = 0x8030_0000;
const LIST1: u32 = 0x800D_5F80;
const LIST2: u32 = 0x800D_5FA8;

#[derive(Clone, Debug)]
struct Scene {
    wh: (i16, i16),
    vp: [i16; 4],
    h14: i16,
    h1c: i16,
    m: [f32; 16],
    cam: [f32; 3],
    mirror: u32,
    dist: Option<f32>,
    small: Option<f32>,
    pts: Vec<[f32; 3]>,
    flags: Vec<u8>,
    handles: Vec<u32>,
    segs: [u32; 16],
    recs: Vec<u32>,
    fade: u32,
    count: u32,
}

/// `(x, y, z, w) = p * M` with `w = p.z` (a perspective), `w = 1` (affine)
/// or a random matrix.
fn matrix() -> BoxedStrategy<[f32; 16]> {
    prop_oneof![
        3 => (0.5f32..2.0, 0.5f32..2.0, prop::array::uniform4(-0.05f32..0.05)).prop_map(|(ax, ay, n)| {
            [ax, n[0], 0.0, 0.0, n[1], ay, 0.0, 0.0, n[2], n[3], 1.0, 1.0, 0.0, 0.0, 0.0, 0.0]
        }),
        2 => (0.5f32..2.0, 0.5f32..2.0, -2.0f32..2.0).prop_map(|(ax, ay, c)| {
            [ax, 0.0, 0.0, 0.0, 0.0, ay, 0.0, 0.0, 0.0, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0]
        }),
        1 => prop::array::uniform16(prop_oneof![-3.0f32..3.0, Just(0.0f32)]),
    ]
    .boxed()
}

fn point() -> BoxedStrategy<[f32; 3]> {
    prop_oneof![
        3 => (0.3f32..30.0, -1.3f32..1.3, -1.3f32..1.3).prop_map(|(z, a, b)| [a * z, b * z, z]),
        2 => (-1.3f32..1.3, -1.3f32..1.3, -2.0f32..2.0).prop_map(|(a, b, z)| [a, b, z]),
        1 => prop::array::uniform3(-50.0f32..50.0),
    ]
    .boxed()
}

fn scene() -> BoxedStrategy<Scene> {
    (
        (screen(), prop_oneof![3 => Just([640i16, 480, 640, 480]), 1 => Just([256i16, 256, 256, 256]), 1 => prop::array::uniform4(1i16..800)], -4i16..8, -4i16..8),
        (matrix(), prop_oneof![3 => Just([0.0f32; 3]), 1 => prop::array::uniform3(-2.0f32..2.0)], prop_oneof![Just(0u32), Just(0x4000u32), any::<u32>()],
         prop_oneof![3 => Just(None), 1 => (0.0f32..60.0).prop_map(Some)], prop_oneof![3 => Just(None), 1 => (0.0f32..5.0).prop_map(Some)]),
        (prop::collection::vec(point(), 40), prop::collection::vec(prop_oneof![1 => Just(0u8), 3 => any::<u8>()], 40)),
        (prop::collection::vec(prop_oneof![1 => Just(0xFFFF_FC18u32), 1 => 0u32..0x1_0000, 1 => any::<u32>()], 52),
         prop::array::uniform16(prop_oneof![0u32..8, 0u32..3000, any::<u32>()]).prop_map(|mut a| { for k in 0..8 { a[2 * k] &= 31; } a })),
        (prop::collection::vec(record_word(), 20), prop_oneof![Just(0u32), any::<u32>()], prop_oneof![0u32..13, Just(0u32)]),
    )
        .prop_map(|((wh, vp, h14, h1c), (m, cam, mirror, dist, small), (pts, flags), (handles, segs), (recs, fade, count))| Scene {
            wh, vp, h14, h1c, m, cam, mirror, dist, small, pts, flags, handles, segs, recs, fade, count,
        })
        .boxed()
}

/// Addresses each marker function uses: points, outputs X, Y, the distance
/// threshold, the handles.
struct Layout {
    pts: u32,
    x: u32,
    y: u32,
    dist: u32,
    handles: u32,
}

const L105: Layout = Layout { pts: 0x800D_5C60, x: 0x800D_5E40, y: 0x800D_5EE0, dist: 0x8009_B8C8, handles: 0x800D_60A0 };
const LB34: Layout = Layout { pts: 0x800D_5898, x: 0x800D_5958, y: 0x800D_5988, dist: 0x800A_86AC, handles: 0x800D_6070 };

fn put_scene(s: &mut State, sc: &Scene, l: &Layout) {
    wh(s, SCREEN, sc.wh.0 as u16);
    wh(s, SCREEN + 2, sc.wh.1 as u16);
    for (k, v) in sc.vp.iter().enumerate() {
        wh(s, VP + [0x10, 0x12, 0x18, 0x1A][k], *v as u16);
    }
    wh(s, VP + 0x14, sc.h14 as u16);
    wh(s, VP + 0x1C, sc.h1c as u16);
    for (k, v) in sc.m.iter().enumerate() {
        wf(s, MATRIX + 4 * k as u32, *v);
    }
    for (k, v) in sc.cam.iter().enumerate() {
        wf(s, CAMERA + 4 * k as u32, *v);
    }
    wr(s, MIRROR, sc.mirror);
    if let Some(d) = sc.dist {
        wf(s, l.dist, d);
    }
    if let Some(z) = sc.small {
        wf(s, 0x800A_86A8, z);
    }
    for (i, p) in sc.pts.iter().enumerate() {
        for (k, v) in p.iter().enumerate() {
            wf(s, l.pts + 12 * i as u32 + 4 * k as u32, *v);
        }
    }
    for (i, f) in sc.flags.iter().enumerate() {
        s.rdram.mem().write_u8(0x800D_5C38 + i as u32, *f);
    }
    for (i, h) in sc.handles.iter().enumerate() {
        wr(s, l.handles + 4 * i as u32, *h);
    }
    for (k, v) in sc.segs.iter().enumerate() {
        wr(s, HANDLE_SEGMENTS + 4 * k as u32, *v);
    }
    for (k, r) in sc.recs.iter().enumerate() {
        wr(s, LIST1 + 4 * k as u32, *r);
    }
    wr(s, FADE, sc.fade);
    wr(s, 0x8009_B86C, sc.count);
    s.ctx.gpr[A0] = sext(VP);
}

/// The projection's frame outputs after func_8000EBE8's C at `frame`.
fn project(w: &mut State, frame: u32, p: u32, at: (u32, u32, u32, u32)) {
    let fr = SP_AT - frame;
    wr(w, fr + 0x10, fr + at.2);
    wr(w, fr + 0x14, fr + at.3);
    wr(w, fr + 0x18, 0);
    call_c(w, frame, &[(A0, sext(VP)), (A1, sext(p)), (A2, sext(fr + at.0)), (A3, sext(fr + at.1))], imports::func_8000EBE8);
}

fn on_screen(w: &State, sx: f32, sy: f32, left: f32) -> bool {
    let (big_w, big_h) = (f32w(i32::from(half(w, SCREEN) as i16)), f32w(i32::from(half(w, SCREEN + 2) as i16)));
    left < sx && sx < big_w && 0.0 < sy && sy < big_h
}

/// `j = trunc((f32(h[o + 0x1C]) + zw * f32(h[o + 0x14])) * 256.0)` and
/// whether it passes against the decoded handle `v` (`j < 0` or `j < v`).
fn j_passes(w: &mut State, frame: u32, zw: f32, handle: u32) -> Option<bool> {
    let h14 = f32w(i32::from(half(w, VP + 0x14) as i16));
    let h1c = f32w(i32::from(half(w, VP + 0x1C) as i16));
    let j = trunc(mul(add(h1c, mul(zw, h14)?)?, 256.0)?) as i32;
    call_c(w, frame, &[(A0, sext(handle))], imports::func_8001004C);
    let v = w.ctx.gpr[V0] as i64;
    Some(j < 0 || i64::from(j) < v)
}

/// A record placed: on, position, the float, the size, the colour.
fn place(w: &mut State, frame: u32, r: u32, (sx, sy): (f32, f32), e: f32, size: f32, alpha: u32) {
    let id = sext16(r);
    call_c(w, frame, &[(A0, id), (A1, 1)], imports::func_8000A920);
    call_c(w, frame, &[(A0, id), (A1, sext16(trunc(sx))), (A2, sext16(trunc(sy)))], imports::func_8000E680);
    call_c(w, frame, &[(A0, id), (A1, sext(e.to_bits()))], imports::func_8000AAF8);
    call_c(w, frame, &[(A0, id), (A1, sext(size.to_bits())), (A2, sext(size.to_bits()))], imports::func_8000AAC0);
    wr(w, SP_AT - frame + 0x10, alpha);
    call_c(w, frame, &[(A0, id), (A1, 0xFF), (A2, 0xFF), (A3, 0xFF)], imports::func_8000AB24);
}

fn markers40_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0xD8;
    saves(&mut w, s, 0xD8, &[(0x5C, S1), (0x58, S0), (0x7C, RA), (0x78, FP), (0x74, S7), (0x70, S6), (0x6C, S5), (0x68, S4), (0x64, S3), (0x60, S2), (0xD8, A0)]);
    save_fprs(&mut w, s, 0xD8, 0x50);
    for k in 0..10 {
        let a = sext16(u32::from(half(&w, LIST1 + 4 * k + 2)));
        call_c(&mut w, 0xD8, &[(A0, a), (A1, 0)], imports::func_8000A920);
        let b = sext16(u32::from(half(&w, LIST2 + 4 * k + 2)));
        call_c(&mut w, 0xD8, &[(A0, b), (A1, 0)], imports::func_8000A920);
    }
    let mut placed = 0u32;
    for i in 0..40u32 {
        let (x, y) = (L105.x + 4 * i, L105.y + 4 * i);
        wr(&mut w, x, 0xFFFF_FC18);
        wr(&mut w, y, 0xFFFF_FC18);
        if byte(&w, 0x800D_5C38 + i) == 0 {
            continue;
        }
        let p = L105.pts + 12 * i;
        call_c(&mut w, 0xD8, &[(A0, sext(p)), (A1, sext(CAMERA))], imports::func_80015470);
        if !(w.ctx.fpr[0].fl() < rf(&w, L105.dist)) {
            continue;
        }
        call_c(&mut w, 0xD8, &[], imports::func_8002F054);
        let alpha = if w.ctx.gpr[V0] == 0 { 255.0f32 } else { 128.0 };
        project(&mut w, 0xD8, p, (0xCC, 0xC8, 0xB0, 0xAC));
        let (sx, sy, zw, ww) = (rf(&w, fr + 0xCC), rf(&w, fr + 0xC8), rf(&w, fr + 0xB0), rf(&w, fr + 0xAC));
        if !(0.0 < sx) {
            continue;
        }
        let mut size = if !(ww <= rf(&w, 0x800A_86A8)) { div(100.0, ww)? } else { 1000.0 };
        if 2.0 < size {
            size = 2.0;
        }
        if !on_screen(&w, sx, sy, 0.0) {
            continue;
        }
        wr(&mut w, x, trunc(sx));
        wr(&mut w, y, trunc(sy));
        let handle = word(&w, L105.handles + 4 * i);
        let h14 = f32w(i32::from(half(&w, VP + 0x14) as i16));
        let h1c = f32w(i32::from(half(&w, VP + 0x1C) as i16));
        let j = trunc(mul(add(h1c, mul(zw, h14)?)?, 256.0)?) as i32;
        if handle == 0xFFFF_FC18 {
            continue;
        }
        call_c(&mut w, 0xD8, &[(A0, sext(handle))], imports::func_8001004C);
        if !(j < 0 || i64::from(j) < w.ctx.gpr[V0] as i64) {
            continue;
        }
        let e = div(sub(160.0, sx)?, 3.0)?;
        if placed >= 10 {
            continue;
        }
        let r1 = word(&w, LIST1 + 4 * placed);
        if r1 != u32::MAX {
            place(&mut w, 0xD8, r1, (sx, sy), e, 1.0, to_unsigned(alpha));
        }
        let r2 = word(&w, LIST2 + 4 * placed);
        if r2 != u32::MAX {
            place(&mut w, 0xD8, r2, (sx, sy), e, size, to_unsigned(alpha) & 0xFF);
        }
        placed += 1;
    }
    Some(w)
}

fn markers_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0xE0;
    saves(&mut w, s, 0xE0, &[(0x74, S7), (0x70, S6), (0x7C, RA), (0x78, FP), (0x6C, S5), (0x68, S4), (0x64, S3), (0x60, S2), (0x5C, S1), (0x58, S0)]);
    save_fprs(&mut w, s, 0xE0, 0x50);
    if (word(&w, 0x8009_B86C) as i32) <= 0 {
        return Some(w);
    }
    let mut i = 0u32;
    loop {
        'next: {
            let (x, y) = (LB34.x + 4 * i, LB34.y + 4 * i);
            wr(&mut w, x, 0xFFFF_FC18);
            wr(&mut w, y, 0xFFFF_FC18);
            let p = LB34.pts + 12 * i;
            call_c(&mut w, 0xE0, &[(A0, sext(p)), (A1, sext(CAMERA))], imports::func_80015470);
            if !(w.ctx.fpr[0].fl() < rf(&w, LB34.dist)) {
                break 'next;
            }
            project(&mut w, 0xE0, p, (0xD4, 0xD0, 0xB0, 0xAC));
            let (sx, sy, zw) = (rf(&w, fr + 0xD4), rf(&w, fr + 0xD0), rf(&w, fr + 0xB0));
            if !on_screen(&w, sx, sy, -4.0) {
                break 'next;
            }
            wr(&mut w, x, trunc(sx));
            wr(&mut w, y, trunc(sy));
            let handle = word(&w, LB34.handles + 4 * i);
            if !j_passes(&mut w, 0xE0, zw, handle)? {
                break 'next;
            }
            let r = word(&w, LIST1 + 4 * i);
            if r == u32::MAX {
                break 'next;
            }
            place(&mut w, 0xE0, r, (sx, sy), 0.0, 1.0, to_unsigned(255.0) & 0xFF);
        }
        i += 1;
        if !((i as i32) < word(&w, 0x8009_B86C) as i32) {
            break;
        }
    }
    Some(w)
}

fn markers40_case(seed: u64, sc: &Scene) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    put_scene(&mut s, sc, &L105);
    let want = markers40_model(&s);
    prop_assume!(want.is_some());
    let after = run("func_800105DC", misc::func_800105DC, &s)?;
    same_memory(&after, &want.unwrap())?;
    Ok(())
}

fn markers_case(seed: u64, sc: &Scene) -> Result<(), TestCaseError> {
    let mut s = state(seed);
    put_scene(&mut s, sc, &LB34);
    let want = markers_model(&s);
    prop_assume!(want.is_some());
    let after = run("func_80010B34", misc::func_80010B34, &s)?;
    same_memory(&after, &want.unwrap())?;
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_800105DC(seed: u64, sc in scene()) {
        markers40_case(seed, &sc)?;
    }

    #[test]
    fn func_80010B34(seed: u64, sc in scene()) {
        markers_case(seed, &sc)?;
    }
}

/// An affine scene (`w = 1`, `zw = p.z` exactly, `x = p.x`, `y = p.y`) on a
/// 256 x 256 viewport (`X = (x + 1) * 64`, `Y = (1 - y) * 64`), every point
/// flagged, the handles decoding to `base` (segment 0, shift 0).
fn affine(base: u32) -> Scene {
    Scene {
        wh: (120, 100),
        vp: [256, 256, 256, 256],
        h14: 3,
        h1c: 1,
        m: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
        cam: [0.0; 3],
        mirror: 0,
        dist: None,
        small: None,
        pts: vec![[0.0, 0.0, 0.5]; 40],
        flags: vec![1; 40],
        handles: vec![0; 52],
        segs: [0, base, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        recs: (0..20).map(|k| k as u32).collect(),
        fade: 0,
        count: 12,
    }
}

/// Ties: points exactly on each screen edge (`X = -4` and `X = W` for the
/// tracked markers, `X = 0` for the 40, `Y = 0`, `Y = H`), a distance equal
/// to the threshold, `w` equal to the size threshold and to 50 (size 2.0
/// exactly), `j` equal to the decoded handle, and fused `j` sums separated
/// (searched here: `zw = p.z` around `(n / 256 - h1C) / h14`).
fn tie_scenes() -> Vec<Scene> {
    let mut out = Vec::new();
    let mut edges = affine(0x10_0000);
    edges.wh = (120, 60);
    // X = (x + 1) * 64: -4 at x = -1.0625, 0 at x = -1, 120 = W at x = 0.875;
    // Y = (1 - y) * 64: 30 at y = 0.53125, 0 at y = 1, 60 = H at y = 0.0625.
    let xs = [-1.0625f32, -1.0, 0.875, 0.0, 0.0, 3.0];
    let ys = [0.53125f32, 0.53125, 0.53125, 1.0, 0.0625, 4.0];
    for i in 0..40 {
        edges.pts[i] = [xs[i % 6], ys[i % 6], 0.5];
    }
    out.push(edges);
    // |p| = 5 exactly against a threshold of 5 (and just above).
    for d in [5.0f32, f32::from_bits(5.0f32.to_bits() + 1)] {
        let mut sc = affine(0x10_0000);
        sc.dist = Some(d);
        sc.m[10] = 0.0;
        for i in 0..40 {
            sc.pts[i] = [0.3, 0.4, 0.0];
        }
        sc.pts[0] = [3.0, 4.0, 0.0];
        sc.m[0] = 0.1;
        sc.m[5] = 0.1;
        out.push(sc);
    }
    // w = the size threshold (read from the ROM image) and w = 50 (100 / w = 2).
    let rom = baserom();
    let o = (0x800A_86A8u32 - 0x8000_0400 + 0x1000) as usize;
    let small = f32::from_bits(u32::from_be_bytes(rom[o..o + 4].try_into().unwrap()));
    for ww in [small, 50.0, 25.0] {
        let mut sc = affine(0x10_0000);
        sc.m[15] = ww;
        out.push(sc);
    }
    // j = 256 (h1C = 1, h14 = 0) against v = 256 and 257.
    for base in [256u32, 257, 255] {
        let mut sc = affine(base);
        sc.h14 = 0;
        out.push(sc);
    }
    // Separators for the fused j sum: v = the larger of the two j's.
    for (h14, h1c) in [(3i16, 1i16), (7, -2), (5, 3)] {
        let mut found = 0;
        'n: for n in 1..3000i32 {
            let near = ((n as f32 / 256.0) - f32::from(h1c)) / f32::from(h14);
            for u in -100i32..100 {
                let z = f32::from_bits((near.to_bits() as i32 + u) as u32);
                if !(z > 0.0 && z < 1000.0) {
                    continue;
                }
                let unfused = trunc((f32::from(h1c) + z * f32::from(h14)) * 256.0) as i32;
                let fused = trunc(((f64::from(h1c) + f64::from(z) * f64::from(h14)) as f32) * 256.0) as i32;
                if unfused != fused && unfused >= 0 && fused >= 0 {
                    let mut sc = affine(unfused.max(fused) as u32);
                    sc.h14 = h14;
                    sc.h1c = h1c;
                    for i in 0..40 {
                        sc.pts[i] = [0.0, 0.0, z];
                    }
                    out.push(sc);
                    found += 1;
                    if found == 2 {
                        break 'n;
                    }
                    break;
                }
            }
        }
        assert!(found > 0, "no j separator for h14 = {h14}, h1C = {h1c}");
    }
    out
}

#[test]
fn func_800105DC_cases() {
    for (i, sc) in tie_scenes().iter().enumerate() {
        markers40_case(0x105 + i as u64, sc).unwrap_or_else(|e| panic!("{i}: {e}"));
    }
}

#[test]
fn func_80010B34_cases() {
    for (i, sc) in tie_scenes().iter().enumerate() {
        markers_case(0xB34 + i as u64, sc).unwrap_or_else(|e| panic!("{i}: {e}"));
    }
}
