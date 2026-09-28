//! Depth-0 functions with `break` (IDO's divide checks, N64Recomp's
//! `do_break`), in game::misc and game::spline. Recompiled C vs Rust on random register files
//! and memory, each checked against its statement.
//!
//! Neither function can reach its `do_break` in its domain (see the ports'
//! docs), so the hook is tested directly: a port's call must reach the
//! oracle's trap. That aborts the process, so it runs in a child, like
//! texture_block_init.rs. So does the domain edge of func_80031FA4: a zero
//! modulus kills both the C (the host's divide faults) and the port.

// Tests are named after the functions (func_80031FA4), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::{misc, spline};
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;
use std::process::Command;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

fn half(s: &State, a: u32) -> u16 {
    (word(s, a & !3) >> (16 - 8 * (a & 2))) as u16
}

fn byte(s: &State, a: u32) -> u8 {
    (word(s, a & !3) >> (24 - 8 * (a & 3))) as u8
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP0: u32 = 0x800A_2800;

// ---------------------------------------------------------------------------
// func_80031FA4: cycling animation step

const O: u32 = 0x8030_0000;
const DEST: u32 = O + 0x1000;
const SRC: u32 = O + 0x2000;
const MAP: u32 = O + 0x3000;
const PHASE: u32 = O + 0x3100;

#[derive(Clone, Debug)]
struct Anim {
    timer: f32,
    step: f64,
    period: f32,
    modulus: i16,
    n: i16,
    dest: bool,
}

fn anim_state(seed: u64, a: &Anim) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP0);
    s.randomise_memory(seed ^ 9, O, 0x3200);
    let mut m = s.rdram.mem();
    m.write_u32(0x800A_2DD4, O);
    m.write_f64(0x8012_0BF0, a.step);
    m.write_u32(O + 4, if a.dest { DEST } else { 0 });
    m.write_u32(O + 8, SRC);
    m.write_u16(O + 0xC, a.modulus as u16);
    m.write_f32(O + 0x10, a.period);
    m.write_f32(O + 0x14, a.timer);
    m.write_u16(O + 0x18, a.n as u16);
    m.write_u32(O + 0x1C, MAP);
    m.write_u32(O + 0x20, PHASE);
    drop(m);
    s
}

/// The statement, on the state before: the timer after, the phases, and
/// the destination halfwords.
fn anim(s: &State, a: &Anim) -> (u32, Vec<u8>, Vec<u16>) {
    let mut t = (f64::from(a.timer) - a.step) as f32;
    let mut count = 0i32;
    while t < 0.0 {
        t += a.period;
        count += 1;
    }
    let mut phase: Vec<u8> = (0..256).map(|i| byte(s, PHASE + i)).collect();
    let mut dest: Vec<u16> = (0..256).map(|i| half(s, DEST + 2 * i)).collect();
    if a.dest && count > 0 {
        for i in 0..a.n.max(0) as usize {
            let v = ((i32::from(phase[i]) + count) % i32::from(a.modulus)) as u8;
            phase[i] = v;
            dest[byte(s, MAP + i as u32) as usize] = half(s, SRC + 2 * u32::from(v));
        }
    }
    (t.to_bits(), phase, dest)
}

fn anim_params() -> impl Strategy<Value = Anim> {
    let timer = prop_oneof![-50.0f32..50.0, Just(0.0f32), Just(-0.0f32), Just(1.5f32)];
    let step = prop_oneof![-10.0f64..60.0, Just(1.5f64), Just(0.1f64)];
    let period = prop_oneof![0.25f32..30.0, Just(1.0f32), Just(0.1f32)];
    let modulus = prop_oneof![1i16..40, Just(1i16), Just(-1i16), (-300i16..-1), Just(255i16), Just(256i16), Just(257i16), 1i16..i16::MAX];
    let n = prop_oneof![-2i16..40, Just(0i16), Just(255i16)];
    (timer, step, period, modulus, n, prop_oneof![5 => Just(true), 1 => Just(false)])
        .prop_map(|(timer, step, period, modulus, n, dest)| Anim { timer, step, period, modulus, n, dest })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80031FA4(seed: u64, a in anim_params()) {
        let s = anim_state(seed, &a);
        let after = run("func_80031FA4", misc::func_80031FA4, &s)?;
        let (t, phase, dest) = anim(&s, &a);
        prop_assert_eq!(word(&after, O + 0x14), t);
        for i in 0..256u32 {
            prop_assert_eq!(byte(&after, PHASE + i), phase[i as usize], "phase {}", i);
            prop_assert_eq!(half(&after, DEST + 2 * i), dest[i as usize], "dest {}", i);
        }
    }
}

/// Timers that land exactly on 0 (no period added) and exactly on -period.
#[test]
fn func_80031FA4_exact_edges() {
    for (timer, step, period) in [(1.5f32, 1.5f64, 2.0f32), (1.0, 3.0, 2.0), (0.0, 0.0, 1.0), (-0.0, 0.0, 1.0)] {
        let a = Anim { timer, step, period, modulus: 7, n: 5, dest: true };
        let s = anim_state(11, &a);
        let after = compare("func_80031FA4", misc::func_80031FA4, &s).unwrap_or_else(|d| panic!("{d}"));
        let (t, phase, _) = anim(&s, &a);
        assert_eq!(word(&after, O + 0x14), t, "{timer} {step} {period}");
        assert_eq!((0..5).map(|i| byte(&after, PHASE + i)).collect::<Vec<_>>(), phase[..5]);
    }
}

// ---------------------------------------------------------------------------
// func_8003ABA0: spline walker step

const SPLINE: u32 = 0x8031_0000;
const POINTS: u32 = 0x8031_1000;
const W: u32 = 0x8031_2000;
const NPOINTS: u32 = 8;

fn pt(i: u32) -> u32 {
    POINTS.wrapping_add(i.wrapping_mul(0x54))
}

#[derive(Clone, Debug)]
struct Walk {
    flag: i16,
    dir: u64,
    /// Next and previous counts per point.
    counts: Vec<(i16, i16)>,
    links: Vec<[i16; 5]>,
    idx: [u32; 4],
    ends: (u32, u32),
    choice: i32,
    bits: u32,
}

fn walk_params() -> impl Strategy<Value = Walk> {
    let count = prop_oneof![4 => 1i16..4, 2 => Just(0i16), 1 => Just(-1i16), 1 => Just(-2i16)];
    let idx = prop_oneof![5 => 0u32..NPOINTS, 1 => Just(u32::MAX)];
    (
        prop_oneof![Just(0i16), Just(1i16), any::<i16>()],
        prop_oneof![Just(1u64), Just(0u64), Just(0x1_0001u64), Just(0x2_0000u64), any::<u64>()],
        proptest::collection::vec((count.clone(), count), NPOINTS as usize),
        proptest::collection::vec([0i16..8, 0i16..8, 0i16..8, 0i16..8, 0i16..8], NPOINTS as usize),
        [idx.clone(), idx.clone(), idx.clone(), idx],
        (prop_oneof![3 => Just(0u32), 1 => any::<u32>()], prop_oneof![3 => Just(0u32), 1 => any::<u32>()]),
        prop_oneof![-4i32..8, 0i32..1000, -1000i32..0, -70_000i32..-30_000],
        any::<u32>(),
    )
        .prop_map(|(flag, dir, counts, links, idx, ends, choice, bits)| Walk { flag, dir, counts, links, idx, ends, choice, bits })
}

fn walk_state(seed: u64, w: &Walk) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP0);
    s.randomise_memory(seed ^ 3, SPLINE, 0x3000);
    let mut m = s.rdram.mem();
    m.write_u16(SPLINE, w.flag as u16);
    m.write_u32(SPLINE + 0xC, POINTS);
    for k in 0..NPOINTS {
        let (next, prev) = w.counts[k as usize];
        m.write_u16(pt(k), next as u16);
        m.write_u16(pt(k) + 2, prev as u16);
        for (j, &l) in w.links[k as usize].iter().enumerate() {
            m.write_u16(pt(k) + 4 + 2 * j as u32, l as u16);
        }
    }
    for (j, &i) in w.idx.iter().enumerate() {
        m.write_u32(W + 0x10 + 4 * j as u32, i);
    }
    m.write_u32(W + 0x20, w.ends.0);
    m.write_u32(W + 0x24, w.ends.1);
    m.write_u32(W + 0x28, w.choice as u32);
    m.write_u32(W + 0x2C, w.bits);
    drop(m);
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(SPLINE), w.dir, sext(W));
    s
}

/// The statement: the walker's words `+0..+0x30` after the step.
fn walk(s: &State, w: &Walk) -> Vec<u32> {
    let mut v: Vec<u32> = (0..12).map(|k| word(s, W + 4 * k)).collect();
    let f = |off: usize| off / 4;
    let h = |a: u32| half(s, a) as i16 as i32;
    let flag = w.flag != 0;
    if w.dir as u16 as i16 == 1 {
        v[f(0x24)] = 0;
        if v[f(0x20)] == 0 {
            let p = pt(if flag { v[f(0x14)] } else { v[f(0x1C)] });
            let count = h(p);
            if count == 0 {
                v[f(0x20)] = 1;
                v[f(8)] = 1.0f32.to_bits();
            } else {
                let choice = v[f(0x28)] as i32;
                let k = i32::from(if choice < count { choice as i16 } else { (choice % count) as i16 });
                let next = h(p.wrapping_add(4).wrapping_add((k as u32).wrapping_mul(2)));
                let bits = v[f(0x2C)] as i32;
                v[f(0x2C)] = (if flag { (bits >> 1) | k } else { (bits >> 1) | k.wrapping_shl(2) }) as u32;
                v[f(0x10)] = v[f(0x14)];
                if flag {
                    v[f(0x14)] = next as u32;
                } else {
                    let (a, b) = (v[f(0x18)], v[f(0x1C)]);
                    v[f(0x1C)] = next as u32;
                    v[f(0x14)] = a;
                    v[f(0x18)] = b;
                }
            }
        }
    } else {
        v[f(0x20)] = 0;
        if v[f(0x24)] == 0 {
            let p = pt(v[f(0x10)]);
            let count = h(p + 2);
            if count == 0 {
                v[f(0x24)] = 1;
                v[f(8)] = 0;
            } else {
                let choice = v[f(0x28)] as i32;
                let j = if choice < count { choice } else { choice % count };
                let prev = h(p.wrapping_add(8).wrapping_add((j as u32).wrapping_mul(2)));
                let bits = v[f(0x2C)] << 1;
                v[f(0x2C)] = if flag { bits & 1 } else { bits & 7 };
                if v[f(0x10)] as i32 != h(pt(prev as u32) + 4) {
                    v[f(0x2C)] |= 1;
                }
                if !flag {
                    v[f(0x1C)] = v[f(0x18)];
                    v[f(0x18)] = v[f(0x14)];
                }
                v[f(0x14)] = v[f(0x10)];
                v[f(0x10)] = prev as u32;
            }
        }
    }
    v
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn func_8003ABA0(seed: u64, w in walk_params()) {
        let s = walk_state(seed, &w);
        let after = run("func_8003ABA0", spline::func_8003ABA0, &s)?;
        let want = walk(&s, &w);
        for (k, want) in want.iter().enumerate() {
            prop_assert_eq!(word(&after, W + 4 * k as u32), *want, "+{:#x}", 4 * k);
        }
        prop_assert_eq!(word(&after, SP0 + 4), w.dir as u32);
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP0));
    }
}

// ---------------------------------------------------------------------------
// Traps, in a child process

const CHILD_ENV: &str = "BREAKS_CHILD";

/// In the child: "hook" calls the runtime's do_break the way the ports do;
/// "c"/"rust" run func_80031FA4 with a zero modulus. Reaching the end means
/// nothing trapped.
#[test]
fn trap_child() {
    let Ok(which) = std::env::var(CHILD_ENV) else { return };
    match which.as_str() {
        // SAFETY: a runtime hook with a plain integer argument.
        "hook" => unsafe { game::imports::runtime::do_break(0x8003_2064) },
        side => {
            let a = Anim { timer: -1.0, step: 0.0, period: 2.0, modulus: 0, n: 3, dest: true };
            let mut s = anim_state(5, &a);
            let f = match side {
                "c" => oracle::recomp::by_name("func_80031FA4").unwrap(),
                _ => difftest::port_under_test("func_80031FA4", misc::func_80031FA4),
            };
            s.run(f);
        }
    }
    std::process::exit(0);
}

fn child(which: &str) -> (bool, String) {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "trap_child", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, which)
        .output()
        .unwrap();
    (out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned())
}

/// A port's do_break reaches the oracle's trap, with the address.
#[test]
fn do_break_traps() {
    let (ok, stderr) = child("hook");
    assert!(!ok, "do_break returned");
    assert!(stderr.contains("break instruction at 0x80032064"), "unexpected stderr:\n{stderr}");
}

/// A zero modulus is outside func_80031FA4's domain: the C dies in the
/// host's divide (before its break 7), the port in div's assert.
#[test]
fn func_80031FA4_zero_modulus_dies() {
    for side in ["c", "rust"] {
        let (ok, stderr) = child(side);
        assert!(!ok, "{side}: returned with a zero modulus");
        if side == "rust" {
            assert!(stderr.contains("div by zero"), "unexpected stderr:\n{stderr}");
        }
    }
}
