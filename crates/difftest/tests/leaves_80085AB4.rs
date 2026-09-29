//! Float leaves at 0x80085AB4..0x80087CB0: the framebuffer histogram, the
//! viewport, the rectangle queue and its drawing (game::render), a scale
//! (game::misc), a rigid-transform inverse (game::math) and `sqrtf`
//! (game::libultra). Recompiled C vs Rust, each checked against its
//! statement, the ones that store by simulating the stores on a copy of the
//! input and comparing all of RDRAM.
//!
//! NaN operands of arithmetic are outside the domain (NAN_CHECK in the
//! oracle); models return None for them, skipped before the C runs.

// Tests are named after the functions (func_80085AB4), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::recomp::{reg::*, RecompFn};
use game::{libultra, math, misc, render};
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
const P: u32 = 0x8030_0000;
const Q: u32 = 0x8030_1000;
const SCREEN: u32 = 0x8011_4470;

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

fn operands(a: f32, b: f32) -> Option<(f32, f32)> {
    (!a.is_nan() && !b.is_nan()).then_some((a, b))
}
fn mul(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a * b)
}
fn add(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a + b)
}
fn sub(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a - b)
}
fn div(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a / b)
}
fn neg(a: f32) -> Option<f32> {
    (!a.is_nan()).then_some(-a)
}

/// `trunc.w.s`/`trunc.w.d` as the C cast does them on the host.
fn trunc(v: f64) -> u32 {
    if v.is_nan() || v >= 2_147_483_648.0 || v < -2_147_483_648.0 {
        0x8000_0000
    } else {
        v as i32 as u32
    }
}

fn edge() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::from_bits(1)),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        Just(-1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        Just(1.0e19f32),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![5 => -1.0e3f32..1.0e3f32, 3 => -1.0f32..1.0f32, 2 => (-8i32..8).prop_map(|n| n as f32), 1 => edge()].boxed()
}

fn screen() -> BoxedStrategy<(i16, i16)> {
    prop_oneof![
        3 => (prop::sample::select(vec![320i16, 640]), prop::sample::select(vec![240i16, 480])),
        2 => (1i16..700, 1i16..500),
        1 => (any::<i16>(), any::<i16>()),
    ]
    .boxed()
}

// ---- func_80085AB4 ----

const FB_PTR: u32 = 0x800A_68B0;
const FB: u32 = 0x8030_4000;
const BLUE: u32 = 0x8014_88C8;
const RED: u32 = 0x8014_8948;

/// The statement's sum with the products of the bins in `fused` fused
/// into their adds (the mutants' form).
fn hist_sum(red: &[u32; 32], fused: &[usize]) -> f32 {
    let mut sum = 0.0f32;
    for k in 2..16usize {
        let t = (red[k] as i32 >> 2) as f32 / 19200.0;
        let w = k as f32 - 1.0;
        sum = if fused.contains(&k) { t.mul_add(w, sum) } else { sum + t * w };
    }
    sum
}

/// Red counts on which fusing the bins `fused` changes the sum. A term is
/// small next to the sum, so its rounding rarely reaches the sum's last
/// bit: a deterministic search over counts finds one. (Weights 1, 2, 4
/// and 8 are exact products: fusing them changes nothing.)
fn hist_separator(fused: &[usize]) -> [u32; 32] {
    let mut seed = 0x2545_F491u32 ^ fused[0] as u32;
    for _ in 0..200_000 {
        let mut red = [0u32; 32];
        for c in &mut red[2..16] {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *c = seed % 256;
        }
        if hist_sum(&red, &[]) != hist_sum(&red, fused) {
            return red;
        }
    }
    panic!("no separator for bins {fused:?}");
}

/// The separators for the fused products with inexact weights, as the
/// mutants fuse them (the loop's bins in pairs, four apart), C vs Rust.
#[test]
fn func_80085AB4_fused_separators() {
    for fused in [&[4usize, 8][..], &[6, 10], &[7, 11], &[12], &[13], &[14], &[15]] {
        let red = hist_separator(fused);
        let (w, h) = (64i16, 64i16);
        let mut pixels: Vec<u16> = red.iter().enumerate().flat_map(|(k, &n)| std::iter::repeat((k as u16) << 11).take(n as usize)).collect();
        pixels.resize(4096, 0);
        let mut s = state(fused[0] as u64);
        {
            let mut m = s.rdram.mem();
            m.write_u32(FB_PTR, FB);
            m.write_u16(SCREEN, w as u16);
            m.write_u16(SCREEN + 2, h as u16);
            for (k, p) in pixels.iter().enumerate() {
                m.write_u16(FB + 2 * k as u32, *p);
            }
        }
        let mut all = red;
        all[0] += 4096 - red.iter().sum::<u32>();
        let after = compare("func_80085AB4", render::func_80085AB4, &s).unwrap_or_else(|d| panic!("{fused:?}: {d}"));
        assert_eq!(after.ctx.fpr[0].u32l(), hist_sum(&all, &[]).to_bits(), "{fused:?}");
    }
}

// ---- func_80087814 / func_800879B8 ----

const COUNT: u32 = 0x800A_6978;
const QUEUE: u32 = 0x8014_8B60;
const DL_HEAD: u32 = 0x8012_17B0;
const DL: u32 = 0x8030_6000;
const BORDER: u32 = 0x8012_0E10;

fn fill(x0: u32, y0: u32, x1: u32, y1: u32) -> [u32; 2] {
    [0xF600_0000 | (x1 & 0x3FF) << 14 | (y1 & 0x3FF) << 2, (x0 & 0x3FF) << 14 | (y0 & 0x3FF) << 2]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80085AB4(seed: u64, w in prop_oneof![1i16..24, Just(0i16), -3i16..0], h in prop_oneof![1i16..16, Just(0i16)],
                     pixels in prop::collection::vec(any::<u16>(), 384), junk in prop::array::uniform32(any::<u32>())) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(FB_PTR, FB);
            m.write_u16(SCREEN, w as u16);
            m.write_u16(SCREEN + 2, h as u16);
            for (k, p) in pixels.iter().enumerate() {
                m.write_u16(FB + 2 * k as u32, *p);
            }
            for k in 0..32u32 {
                m.write_u32(BLUE + 4 * k, junk[k as usize]);
                m.write_u32(RED + 4 * k, junk[31 - k as usize]);
            }
        }
        let n = (w as i32).wrapping_mul(h as i32).max(0) as usize;
        let (mut blue, mut red) = ([0u32; 32], [0u32; 32]);
        for p in &pixels[..n] {
            blue[((p >> 1) & 0x1F) as usize] += 1;
            red[((p >> 11) & 0x1F) as usize] += 1;
        }
        let mut sum = 0.0f32;
        for k in 2..16usize {
            sum = sum + ((red[k] as i32 >> 2) as f32 / 19200.0) * (k as f32 - 1.0);
        }
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            for k in 0..32u32 {
                m.write_u32(BLUE + 4 * k, blue[k as usize]);
                m.write_u32(RED + 4 * k, red[k as usize]);
            }
        }
        let after = run("func_80085AB4", render::func_80085AB4, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), sum.to_bits());
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80085F78(seed: u64, a in ordinary(), b in ordinary(), c in ordinary()) {
        let want = (|| mul(c, add(1.0, mul(b, sub(div(45.0, a)?, 1.0)?)?)?))();
        prop_assume!(want.is_some());
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_f32(P + 0x134, a);
            m.write_f32(P + 0x150, b);
            m.write_f32(P + 0x148, c);
        }
        s.ctx.gpr[A0] = sext(P);
        let mut want_s = s.clone();
        want_s.rdram.mem().write_f32(P + 0x154, want.unwrap());
        let after = run("func_80085F78", misc::func_80085F78, &s)?;
        same_memory(&after, &want_s)?;
    }

    #[test]
    fn func_80086178(seed: u64, rect in prop::array::uniform4(prop_oneof![0i32..640, any::<i32>()]), scr in screen(),
                     full in prop_oneof![Just(0u32), any::<u32>()], bits: u32) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for (k, v) in rect.iter().enumerate() {
                m.write_u32(P + 0x20 + 4 * k as u32, *v as u32);
            }
            m.write_u16(SCREEN, scr.0 as u16);
            m.write_u16(SCREEN + 2, scr.1 as u16);
            m.write_u32(0x8009_B7E8, full);
            m.write_u32(0x800D_5710, bits);
        }
        s.ctx.gpr[A0] = sext(P);
        let (sx, sy) = (f64::from(scr.0) / 320.0, f64::from(scr.1) / 240.0);
        let t = |v: i32, k: f64| trunc(f64::from(v as f32) * k);
        let (x0, y0, x1, y1) = (t(rect[0], sx), t(rect[1], sy), t(rect[2], sx), t(rect[3], sy));
        let mut vp = [
            (x1.wrapping_sub(x0) << 1).wrapping_add(8),
            (y1.wrapping_sub(y0) << 1).wrapping_add(8),
            0x92,
            0,
            x0.wrapping_add(x1) << 1,
            y0.wrapping_add(y1) << 1,
            0x36C,
        ];
        if full != 0 {
            vp[0] = 0x500;
            vp[1] = 0x3C0;
            vp[4] = if bits & 1 != 0 { 0 } else { 0x500 };
            vp[5] = if bits & 2 != 0 { 0 } else { 0x3C0 };
        }
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            for (k, off) in [0x10u32, 0x12, 0x14, 0, 0x18, 0x1A, 0x1C].iter().enumerate() {
                if *off != 0 {
                    m.write_u16(P + off, vp[k] as u16);
                }
            }
        }
        let after = run("func_80086178", render::func_80086178, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8008681C(seed: u64, mm in prop::array::uniform16(ordinary())) {
        let e = |i: usize, j: usize| mm[4 * i + j];
        let mut o = [0.0f32; 16];
        let ok = (|| {
            for i in 0..3 {
                o[4 * i] = e(0, i);
                o[4 * i + 2] = neg(e(1, i))?;
                o[4 * i + 1] = e(2, i);
            }
            let t = [e(3, 0), e(3, 1), e(3, 2)];
            for j in 0..3 {
                o[12 + j] = neg(add(mul(o[8 + j], t[2])?, add(mul(t[0], o[j])?, mul(t[1], o[4 + j])?)?)?)?;
            }
            for i in 0..4 {
                o[4 * i + 3] = e(i, 3);
            }
            Some(())
        })();
        prop_assume!(ok.is_some());
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for (k, x) in mm.iter().enumerate() {
                m.write_f32(Q + 4 * k as u32, *x);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(P), sext(Q));
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            for (k, x) in o.iter().enumerate() {
                m.write_f32(P + 4 * k as u32, *x);
            }
        }
        let after = run("func_8008681C", math::func_8008681C, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80087814(seed: u64, r in prop::array::uniform4(prop_oneof![-4i16..700, Just(0i16), Just(1i16), any::<i16>()]), scr in screen(),
                     n in prop_oneof![0i16..31, 29i16..33, any::<i16>()]) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u16(COUNT, n as u16);
            m.write_u16(SCREEN, scr.0 as u16);
            m.write_u16(SCREEN + 2, scr.1 as u16);
        }
        // Negative counts index below the queue: keep them in RDRAM.
        prop_assume!(n >= -1000);
        for (k, v) in [A0, A1, A2, A3].iter().zip(r) {
            s.ctx.gpr[*k] = sext(v as i32 as u32);
        }
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            for (k, v) in r.iter().enumerate() {
                m.write_u32(SP_AT + 4 * k as u32, *v as i32 as u32);
            }
            if n < 31 {
                let (mut x0, mut y0) = (r[0].wrapping_sub(1), r[1].wrapping_sub(1));
                let (mut x1, mut y1) = (r[2].wrapping_add(1), r[3].wrapping_add(1));
                x0 = x0.max(0);
                y0 = y0.max(0);
                if (scr.0 as i32 - 1) < x1 as i32 {
                    x1 = (scr.0 as i32 - 1) as i16;
                }
                if (scr.1 as i32 - 1) < y1 as i32 {
                    y1 = (scr.1 as i32 - 1) as i16;
                }
                if x0 < x1 && y0 < y1 {
                    let (sx, sy) = (f64::from(scr.0) / 320.0, f64::from(scr.1) / 240.0);
                    let e = QUEUE.wrapping_add((8 * n as i32) as u32);
                    m.write_u16(COUNT, (n as u16).wrapping_add(1));
                    m.write_u16(e, trunc(f64::from(x0) * sx) as u16);
                    m.write_u16(e + 2, trunc(f64::from(y0) * sy) as u16);
                    m.write_u16(e + 4, trunc(f64::from(x1) * sx) as u16);
                    m.write_u16(e + 6, trunc(f64::from(y1) * sy) as u16);
                }
            }
        }
        let after = run("func_80087814", render::func_80087814, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_800879B8(seed: u64, entries in prop::collection::vec(prop::array::uniform4(prop_oneof![0i16..1100, any::<i16>()]), 0..8),
                     scr in screen(), border in prop::array::uniform4(prop_oneof![0i32..700, any::<i32>()]), s0: u32) {
        let mut s = state(seed);
        s.ctx.gpr[S0] = sext(s0);
        {
            let mut m = s.rdram.mem();
            m.write_u32(DL_HEAD, DL);
            m.write_u16(COUNT, entries.len() as u16);
            for (i, e) in entries.iter().enumerate() {
                for (k, v) in e.iter().enumerate() {
                    m.write_u16(QUEUE + 8 * i as u32 + 2 * k as u32, *v as u16);
                }
            }
            m.write_u16(SCREEN, scr.0 as u16);
            m.write_u16(SCREEN + 2, scr.1 as u16);
            for (k, v) in border.iter().enumerate() {
                m.write_u32(BORDER + 4 * k as u32, *v as u32);
            }
        }
        let mut words = vec![0xE700_0000u32, 0, 0xE300_0A01, 0, 0xE200_001C, 0x0F5A_4240];
        for e in &entries {
            let v = |k: usize| e[k] as i32 as u32;
            words.extend(fill(v(0).wrapping_sub(1), v(1).wrapping_sub(1), v(2).wrapping_add(1), v(3).wrapping_add(1)));
        }
        let (sx, sy) = (scr.0 as f32 / 320.0, scr.1 as f32 / 240.0);
        let sc = |v: i32, k: f32| trunc(f64::from(v as f32 * k));
        let (l, t, r, b) = (sc(border[0], sx), sc(border[1], sy), sc(border[2], sx), sc(border[3], sy));
        let cmds = [
            fill(l, b.wrapping_sub(2), r, b.wrapping_add(2)),
            fill(l.wrapping_sub(2), t, l.wrapping_add(2), b),
            fill(r.wrapping_sub(2), t, r.wrapping_add(2), b),
            fill(l, t.wrapping_sub(2), r, t.wrapping_add(2)),
        ];
        for c in cmds {
            words.extend(c);
        }
        words.extend([0xE700_0000, 0]);
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            for (k, w) in words.iter().enumerate() {
                m.write_u32(DL + 4 * k as u32, *w);
            }
            let end = DL + 4 * words.len() as u32;
            m.write_u32(DL_HEAD, end);
            m.write_u16(COUNT, 0);
            let frame = SP_AT - 0x50;
            m.write_u32(frame + 4, s0);
            m.write_u32(frame + 0xC, (b & 0x3FF) << 2);
            m.write_u32(frame + 8, (t & 0x3FF) << 2);
            // The third and fourth border commands' addresses.
            m.write_u32(frame + 0x20, end - 0x18);
            m.write_u32(frame + 0x1C, end - 0x10);
        }
        let after = run("func_800879B8", render::func_800879B8, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80087CB0(seed: u64, x in prop_oneof![0.0f32..1.0e6, ordinary()]) {
        prop_assume!(!x.is_nan());
        let mut s = state(seed);
        s.ctx.fpr[12].set_fl(x);
        let after = run("func_80087CB0", libultra::func_80087CB0, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), x.sqrt().to_bits());
    }
}
