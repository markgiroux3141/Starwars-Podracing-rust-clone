//! The float leaves at 0x8000550C..0x80006F4C (game::matrix, game::anim,
//! game::misc): the matrix stack's reset, push (multiply) and get, and the
//! animation objects' start, key interpolation, key search, range and
//! setters. Recompiled C vs Rust over float edge values and ordinary
//! full-mantissa ones, each checked against its statement.
//!
//! NaN operands of arithmetic are outside these ports' domain (NAN_CHECK in
//! the oracle); NaN is tested where values are only compared or copied.
//! `func_80006DE8` hangs for `b < a` (`pause_self`), tested in a child
//! process like texture_block_init.rs.

// Tests are named after the functions (func_8000550C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::anim::{self, OBJECTS};
use game::matrix::{self, MATRIX_DEPTH, MATRIX_STACK};
use game::misc;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;
use std::process::Command;

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

/// Mostly ordinary values, so products and sums stay finite.
fn ordinary() -> impl Strategy<Value = f32> {
    prop_oneof![
        4 => -1.0e4f32..1.0e4f32,
        2 => -1.0f32..1.0f32,
        2 => (-8i32..8).prop_map(|n| n as f32),
        1 => float(),
    ]
}

fn any_bits() -> impl Strategy<Value = u32> {
    prop_oneof![
        float().prop_map(f32::to_bits),
        (1u32..0x0080_0000).prop_map(|p| 0x7F80_0000 | p),
        (1u32..0x0080_0000).prop_map(|p| 0xFF80_0000 | p),
        any::<u32>(),
    ]
}

fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}

// ---------------------------------------------------------------------------
// Matrix stack

fn level(d: i32) -> u32 {
    MATRIX_STACK.wrapping_add((d as u32).wrapping_mul(64))
}

fn put_matrix(s: &mut State, a: u32, v: &[f32; 16]) {
    let mut m = s.rdram.mem();
    for (k, x) in v.iter().enumerate() {
        m.write_u32(a + 4 * k as u32, x.to_bits());
    }
}

fn matrix_words(s: &State, a: u32) -> [u32; 16] {
    std::array::from_fn(|k| word(s, a + 4 * k as u32))
}

/// Boxed: sixteen unboxed unions overflow the test thread's stack.
fn matrix() -> BoxedStrategy<[f32; 16]> {
    prop_oneof![
        prop::array::uniform16(ordinary().boxed()),
        prop::array::uniform16(float().boxed()),
    ]
    .boxed()
}

/// `M * T` as the code computes it; with `alias`, `M` is the destination,
/// so each stored entry replaces `M`'s word before the next is computed.
fn push_model(mut mm: [f32; 16], t: [f32; 16], alias: bool) -> Option<[u32; 16]> {
    let mut out = [0u32; 16];
    for k in 0..16 {
        let (i, j) = (k / 4, k % 4);
        let (mi, tc) = (|l: usize| mm[4 * i + l], |l: usize| t[4 * l + j]);
        let s01 = ok(ok(tc(0) * mi(0))? + ok(tc(1) * mi(1))?)?;
        let s012 = ok(s01 + ok(tc(2) * mi(2))?)?;
        let v = ok(mi(3) * tc(3))? + s012;
        out[k] = v.to_bits();
        if alias {
            mm[k] = v;
        }
    }
    Some(out)
}

#[test]
fn func_8000550C() {
    for seed in 0..8 {
        let s = state(seed);
        let after = compare("func_8000550C", matrix::func_8000550C, &s).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(word(&after, MATRIX_DEPTH), 0);
        let id: [u32; 16] = std::array::from_fn(|k| if k % 5 == 0 { 1.0f32.to_bits() } else { 0 });
        assert_eq!(matrix_words(&after, MATRIX_STACK), id);
        assert_eq!(word(&after, MATRIX_STACK + 0x40), word(&s, MATRIX_STACK + 0x40));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Depths around the bound (and negative ones, which the signed test
    /// lets through), `M` separate from the stack or the new top itself.
    #[test]
    fn func_8000556C(seed: u64, depth in prop_oneof![-2i32..3, 28i32..35, 32i32..=i32::MAX], mm in matrix(), t in matrix(),
                     alias in prop::bool::weighted(0.2)) {
        let pushes = depth < 32;
        let alias = alias && pushes;
        let dst = level(depth.wrapping_add(1));
        let want = if pushes { push_model(mm, t, alias) } else { None };
        prop_assume!(!pushes || want.is_some());
        let mut s = state(seed);
        s.rdram.mem().write_u32(MATRIX_DEPTH, depth as u32);
        let at_m = if alias { dst } else { 0x8030_0000 };
        if pushes {
            put_matrix(&mut s, level(depth), &t);
        }
        put_matrix(&mut s, at_m, &mm);
        s.ctx.gpr[A0] = sext(at_m);
        let after = run("func_8000556C", matrix::func_8000556C, &s)?;
        if pushes {
            prop_assert_eq!(word(&after, MATRIX_DEPTH), depth.wrapping_add(1) as u32);
            prop_assert_eq!(matrix_words(&after, dst), want.unwrap());
        } else {
            prop_assert_eq!(word(&after, MATRIX_DEPTH), depth as u32);
            prop_assert_eq!(matrix_words(&after, level(32)), matrix_words(&s, level(32)));
        }
    }

    /// The top matrix, word by word; with `out` over the depth word, the
    /// depth changes part way (the model re-reads it the same way).
    #[test]
    fn func_800059A8(seed: u64, depth in 0i32..=32, shift in 0u32..16, alias in prop::bool::weighted(0.25)) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 1, MATRIX_STACK, 33 * 64);
        s.rdram.mem().write_u32(MATRIX_DEPTH, depth as u32);
        // Keep re-read depths in range: small values in the matrices.
        if alias {
            for k in 0..33 * 16 {
                let v = word(&s, MATRIX_STACK + 4 * k) % 33;
                s.rdram.mem().write_u32(MATRIX_STACK + 4 * k, v);
            }
        }
        let out = if alias { MATRIX_DEPTH - 4 * shift } else { 0x8030_0000 };
        let mut sim = s.clone();
        {
            let mut m = sim.rdram.mem();
            for k in 0..16 {
                let d = m.read_u32(MATRIX_DEPTH);
                let v = m.read_u32(MATRIX_STACK.wrapping_add(d << 6).wrapping_add(4 * k));
                m.write_u32(out + 4 * k, v);
            }
        }
        s.ctx.gpr[A0] = sext(out);
        let after = run("func_800059A8", matrix::func_800059A8, &s)?;
        prop_assert_eq!(matrix_words(&after, out), matrix_words(&sim, out));
    }
}

// ---------------------------------------------------------------------------
// Animation objects

const O: u32 = 0x8030_1000;
const KEYS: u32 = 0x8030_2000;
const COUNT: u32 = 0x8009_A2A0;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Registration, the start value by flag bit 5, and for kind 8 the
    /// target's halfword bit 3 (target null, separate, or at `o + 0xF4` so
    /// that halfword is the flags' high half, which the flags re-read sees).
    #[test]
    fn func_80005BB8(seed: u64, n in 0u32..300, flags: u32, kind8: bool, target_mode in 0u8..3, start: [u32; 2], half: u16) {
        let mut s = state(seed);
        let flags = if kind8 { (flags & !0xF) | 8 } else { flags };
        let target = [0, 0x8030_3000, O + 0xF4][target_mode as usize];
        {
            let mut m = s.rdram.mem();
            m.write_u32(COUNT, n);
            m.write_u32(O + 0x100, flags);
            m.write_u32(O + 0x108, start[0]);
            m.write_u32(O + 0x10C, start[1]);
            m.write_u32(O + 0x124, target);
            if target_mode == 1 {
                m.write_u16(0x8030_300C, half);
            }
        }
        s.ctx.gpr[A0] = sext(O);
        let after = run("func_80005BB8", anim::func_80005BB8, &s)?;
        prop_assert_eq!(word(&after, OBJECTS + 4 * n), O);
        prop_assert_eq!(word(&after, COUNT), n + 1);
        let v = if flags & 0x20 != 0 { start[1] } else { start[0] };
        prop_assert_eq!([word(&after, O + 0xF0), word(&after, O + 0xF4), word(&after, O + 0xF8), word(&after, O + 0xFC)], [0, v, v, v]);
        prop_assert_eq!(word(&after, O + 0xDC), 0);
        let clears = flags & 0xF == 8 && target != 0;
        let flags_after = if clears && target_mode == 2 { flags & !0x0008_0000 } else { flags };
        prop_assert_eq!(word(&after, O + 0x100), flags_after | 0x0100_0000);
        if target_mode == 1 {
            let h = after.rdram.clone().mem().read_u16(0x8030_300C);
            prop_assert_eq!(h, if clears { half & 0xFFF7 } else { half });
        }
    }

    /// `(x - k[i]) / (k[i+1] - k[i])`; equal keys give a zero denominator.
    #[test]
    fn func_80005CAC(seed: u64, x in float(), k0 in float(), k1 in float(), i in 0u32..8, eq: bool, junk: u32) {
        let k1 = if eq { k0 } else { k1 };
        let (num, den) = (x - k0, k1 - k0);
        prop_assume!(!num.is_nan() && !den.is_nan());
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(O + 0x11C, KEYS);
            m.write_u32(KEYS + 4 * i, k0.to_bits());
            m.write_u32(KEYS + 4 * i + 4, k1.to_bits());
        }
        s.ctx.gpr[A0] = sext(O);
        s.ctx.gpr[A1] = u64::from(junk) << 32 | u64::from(x.to_bits());
        s.ctx.gpr[A2] = u64::from(i);
        let after = run("func_80005CAC", anim::func_80005CAC, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), (num / den).to_bits());
    }

    /// Sorted keys (with repeats) and times on, between and outside them;
    /// unsorted keys and NaN times or keys too (only compared).
    #[test]
    fn func_80006704(seed: u64, n in 2u32..10, keys in prop::collection::vec(prop_oneof![4 => (-8i32..8).prop_map(|v| v as f32), 1 => float()], 10),
                     sorted: bool, u_mode in 0u8..4, pick in 0usize..10, u_bits in any_bits(), nan_key in prop::option::weighted(0.1, 0usize..10)) {
        let mut keys = keys;
        if sorted {
            keys.sort_by(f32::total_cmp);
        }
        let mut kb: Vec<u32> = keys.iter().map(|k| k.to_bits()).collect();
        if let Some(j) = nan_key {
            kb[j] = 0x7FC0_0000;
        }
        let kf = |j: usize| f32::from_bits(kb[j]);
        let u = match u_mode {
            0 => f32::from_bits(u_bits),
            1 => kf(pick % n as usize),
            2 => kf(pick % n as usize) + 0.5,
            _ => kf(pick % n as usize) - 0.5,
        };
        let n_us = n as usize;
        let want = if kf(n_us - 1) < u {
            n_us as i64 - 2
        } else if u < kf(0) {
            0
        } else {
            let mut i = n_us - 2;
            while u < kf(i) {
                i -= 1;
            }
            i as i64
        };
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(O + 0x104, n);
            m.write_u32(O + 0x11C, KEYS);
            m.write_u32(O + 0x114, u.to_bits());
            for (j, b) in kb.iter().enumerate() {
                m.write_u32(KEYS + 4 * j as u32, *b);
            }
        }
        s.ctx.gpr[A0] = sext(O);
        let after = run("func_80006704", anim::func_80006704, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], want as u64);
    }

    /// `a <= b` (or unordered: never NaN here): clamp at 0 and store.
    #[test]
    fn func_80006DE8(seed: u64, a in float(), b in float(), eq: bool, junk: [u32; 2]) {
        let b = if eq { a } else { b };
        prop_assume!(!(b < a));
        let clamp = |x: f32| if x < 0.0 { 0.0 } else { x };
        let (ca, cb) = (clamp(a), clamp(b));
        prop_assume!(!(cb - ca).is_nan());
        let mut s = state(seed);
        s.ctx.gpr[A0] = sext(O);
        s.ctx.gpr[A1] = u64::from(junk[0]) << 32 | u64::from(a.to_bits());
        s.ctx.gpr[A2] = u64::from(junk[1]) << 32 | u64::from(b.to_bits());
        let after = run("func_80006DE8", anim::func_80006DE8, &s)?;
        prop_assert_eq!([word(&after, O + 0xF0), word(&after, O + 0xF4), word(&after, O + 0xF8)],
                        [ca.to_bits(), cb.to_bits(), (cb - ca).to_bits()]);
    }

    /// Plain float stores from `a1` (any bits) and the `f12` spill.
    #[test]
    fn float_stores(seed: u64, x in any_bits(), junk: u32) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = sext(O);
        s.ctx.gpr[A1] = u64::from(junk) << 32 | u64::from(x);
        let after = run("func_80006EB4", anim::func_80006EB4, &s)?;
        prop_assert_eq!(word(&after, O + 0x110), x);
        let after = run("func_80006F28", anim::func_80006F28, &s)?;
        prop_assert_eq!(word(&after, O + 0xDC), x);
        let mut s = state(seed);
        s.ctx.fpr[12].set_u32l(x);
        let after = run("func_80006F4C", misc::func_80006F4C, &s)?;
        prop_assert_eq!(word(&after, SP_AT), x);
    }
}

const CHILD_ENV: &str = "LEAVES_8000550C_CHILD";
const PAUSE: &str = "pause_self: branch-to-self idle loop";

/// In the child: run one side of func_80006DE8 with `b < a`. Reaching the
/// end means it returned instead of hanging.
#[test]
fn hang_child() {
    let Ok(side) = std::env::var(CHILD_ENV) else { return };
    let mut s = state(3);
    s.ctx.gpr[A0] = sext(O);
    s.ctx.gpr[A1] = u64::from(2.0f32.to_bits());
    s.ctx.gpr[A2] = u64::from(1.0f32.to_bits());
    let f = match side.as_str() {
        "c" => oracle::recomp::by_name("func_80006DE8").unwrap(),
        "rust" => difftest::port_under_test("func_80006DE8", anim::func_80006DE8),
        _ => unreachable!(),
    };
    s.run(f);
    std::process::exit(0);
}

/// QUIRK: `b < a` hangs, in the C and in the port alike.
#[test]
fn func_80006DE8_reversed_range_hangs() {
    for side in ["c", "rust"] {
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "hang_child", "--nocapture", "--test-threads=1"])
            .env(CHILD_ENV, side)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{side}: returned instead of hanging");
        assert!(stderr.contains(PAUSE), "{side}: unexpected stderr:\n{stderr}");
    }
}
