//! Depth 2 at 0x8003398C..0x8003F974 (game::anim, misc, render, save,
//! pools): starting a list of animation objects, the model-view matrix,
//! the render state reset, a save block's checksum and a debug message.
//! Recompiled C vs Rust with the callees as C. The models replay the
//! callees' C on a copy of the state in the same order with the same
//! arguments (and the s registers and callee-saved FPRs the port holds),
//! adding the functions' own stores; whole RDRAM (and `v0` where there is
//! a result) is compared.

// Tests are named after the functions (func_8003398C), capitals included.
#![allow(non_snake_case)]

use difftest::rom::baserom;
use difftest::{compare, State};
use game::imports;
use game::misc::{MTX43_DEPTH, MTX43_STACK};
use game::recomp::{reg::*, RecompFn};
use game::render::DL2_HEAD;
use game::{anim, misc, pools, render, save};
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
/// it.
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

fn call_c(w: &mut State, frame: u32, args: &[(usize, u64)], f: RecompFn) -> u64 {
    for &(r, v) in args {
        w.ctx.gpr[r] = v;
    }
    w.ctx.gpr[SP] = sext(SP_AT - frame);
    w.run(f);
    w.ctx.gpr[V0]
}

fn saves(w: &mut State, s: &State, frame: u32, regs: &[(u32, usize)]) {
    for &(off, r) in regs {
        wr(w, SP_AT - frame + off, s.ctx.gpr[r] as u32);
    }
}

fn save_fpr(w: &mut State, s: &State, a: u32, r: usize) {
    let v = s.ctx.fpr[r].u64;
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

fn bits(x: f32) -> u64 {
    sext(x.to_bits())
}

// ---- func_8003398C: starting animation objects ----

const LIST: u32 = 0x8030_2000;
const OBJS: u32 = 0x8030_0000;
const KEYS: u32 = 0x8030_3000;

#[derive(Clone, Debug)]
struct Start {
    n: usize,
    null: bool,
    t: f32,
    x: f32,
    y: f32,
    mode: u32,
    w: f32,
    z: f32,
    keys: Vec<f32>,
    flags: Vec<u32>,
}

fn start() -> BoxedStrategy<Start> {
    let signed = || prop_oneof![Just(0.0f32), Just(-0.0f32), Just(-1.0f32), 0.0f32..50.0, (1u32..0x0080_0000).prop_map(f32::from_bits), fin()].boxed();
    (
        (0usize..4, prop_oneof![5 => Just(false), 1 => Just(true)], signed(), 0.0f32..50.0, 0.0f32..50.0),
        (prop_oneof![Just(0u32), Just(1u32), any::<u32>()], signed(), signed()),
        (prop::collection::vec(0.0f32..60.0, 2..6), prop::collection::vec(any::<u32>(), 4)),
    )
        .prop_map(|((n, null, t, a, b), (mode, w, z), (mut keys, flags))| {
            keys.sort_by(|p, q| p.partial_cmp(q).unwrap());
            let (x, y) = if a <= b { (a, b) } else { (b, a) };
            Start { n, null, t, x, y, mode, w, z, keys, flags }
        })
        .boxed()
}

fn start_model(s: &State, st: &Start) -> State {
    let mut w = s.clone();
    let fr = SP_AT - 0x60;
    saves(&mut w, s, 0x60, &[(0x5C, RA), (0x58, S4), (0x54, S3), (0x50, S2), (0x4C, S1), (0x48, S0), (0x60, A0)]);
    for (off, r) in [(0x40, 30), (0x38, 28), (0x20, 22), (0x30, 26), (0x28, 24), (0x18, 20)] {
        save_fpr(&mut w, s, fr + off, r);
    }
    let list = s.ctx.gpr[A0] as u32;
    w.ctx.fpr[22].set_fl(st.t);
    w.ctx.fpr[28].set_fl(st.x);
    w.ctx.fpr[30].set_fl(st.y);
    if list == 0 {
        return w;
    }
    w.ctx.fpr[26].set_fl(st.w);
    w.ctx.fpr[24].set_fl(st.z);
    if word(&w, list) == 0 {
        return w;
    }
    w.ctx.fpr[20].set_u32l(0);
    (w.ctx.gpr[S1], w.ctx.gpr[S2], w.ctx.gpr[S3], w.ctx.gpr[S4]) = (sext(0x0400_0000), sext(st.mode), sext(0x0200_0010), sext(0x0200_0000));
    let mut cur = list;
    loop {
        w.ctx.gpr[S0] = sext(cur);
        let o = sext(word(&w, cur));
        call_c(&mut w, 0x60, &[(A0, o), (A1, bits(st.x)), (A2, bits(st.y))], imports::func_80006DE8);
        if st.mode == 0 {
            call_c(&mut w, 0x60, &[(A0, o), (A1, sext(0x0400_0000))], imports::func_80006E50);
            call_c(&mut w, 0x60, &[(A0, o), (A1, sext(0x0200_0000))], imports::func_80006E60);
        } else {
            call_c(&mut w, 0x60, &[(A0, o), (A1, sext(0x0200_0010))], imports::func_80006E50);
            call_c(&mut w, 0x60, &[(A0, o), (A1, sext(0x0400_0000))], imports::func_80006E60);
            if 0.0 < st.z {
                call_c(&mut w, 0x60, &[(A0, o), (A1, 0x40)], imports::func_80006E50);
                call_c(&mut w, 0x60, &[(A0, o), (A1, bits(st.z))], imports::func_80006F28);
            } else {
                call_c(&mut w, 0x60, &[(A0, o), (A1, 0x40)], imports::func_80006E60);
            }
        }
        if 0.0 <= st.t {
            if !(0.0 < st.w) {
                call_c(&mut w, 0x60, &[(A0, o), (A1, bits(st.t))], imports::func_80006E74);
            } else {
                call_c(&mut w, 0x60, &[(A0, o), (A1, bits(st.t)), (A2, bits(st.w))], imports::func_80006EC0);
            }
        }
        let next = word(&w, cur + 4);
        cur += 4;
        if next == 0 {
            break;
        }
    }
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Lists of 0 to 3 objects (or a null list), both modes, times and
    /// weights around 0 (both zeros, negative, the smallest subnormal).
    #[test]
    fn func_8003398C(seed: u64, st in start()) {
        let mut s = state(seed);
        for i in 0..st.n {
            let o = OBJS + 0x200 * i as u32;
            wr(&mut s, LIST + 4 * i as u32, o);
            wr(&mut s, o + 0x100, st.flags[i]);
            wr(&mut s, o + 0x104, st.keys.len() as u32);
            wr(&mut s, o + 0x11C, KEYS);
        }
        wr(&mut s, LIST + 4 * st.n as u32, 0);
        for (j, k) in st.keys.iter().enumerate() {
            wf(&mut s, KEYS + 4 * j as u32, *k);
        }
        wr(&mut s, SP_AT + 0x10, st.mode);
        wf(&mut s, SP_AT + 0x14, st.w);
        wf(&mut s, SP_AT + 0x18, st.z);
        s.ctx.gpr[A0] = if st.null { 0 } else { sext(LIST) };
        (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (bits(st.t), bits(st.x), bits(st.y));
        let w = start_model(&s, &st);
        let after = run("func_8003398C", anim::func_8003398C, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- func_800349C4: the model-view matrix ----

const VIEW: u32 = 0x8011_2E20;
const OUT: u32 = 0x8011_2E60;
const CAM: u32 = 0x800A_3FDC;
const REL: u32 = 0x800A_3FEC;

fn view_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    wr(&mut w, SP_AT - 0x48 + 0x44, s.ctx.gpr[RA] as u32);
    for (off, r) in [(0x38, 30), (0x30, 28), (0x28, 26), (0x20, 24), (0x18, 22), (0x10, 20)] {
        save_fpr(&mut w, s, SP_AT - 0x48 + off, r);
    }
    let e = MTX43_STACK.wrapping_add(48u32.wrapping_mul(word(s, MTX43_DEPTH)));
    wr(&mut w, 0x800A_3FF4, 0);
    wr(&mut w, 0x800A_3FF8, 1);
    if word(&w, REL) != 0 {
        for k in 0..3 {
            let v = sub(rf(&w, e + 0x24 + 4 * k), rf(&w, CAM + 4 * k))?;
            wf(&mut w, e + 0x24 + 4 * k, v);
        }
    }
    let vm = |w: &State, i: u32, j: u32| rf(w, VIEW + 0x10 * i + 4 * j);
    let v3: Vec<[f32; 4]> = (0..3).map(|i| [0, 1, 2, 3].map(|j| vm(&w, i, j))).collect();
    for i in 0..4u32 {
        for j in 0..4u32 {
            let ei = |k: u32| rf(&w, e + 0xC * i + 4 * k);
            let s01 = add(mul(v3[0][j as usize], ei(0))?, mul(v3[1][j as usize], ei(1))?)?;
            let v = if i < 3 {
                add(mul(ei(2), v3[2][j as usize])?, s01)?
            } else {
                add(vm(&w, 3, j), add(s01, mul(v3[2][j as usize], ei(2))?)?)?
            };
            wf(&mut w, OUT + 0x10 * i + 4 * j, v);
        }
    }
    if word(&w, REL) != 0 {
        for k in 0..3 {
            let v = add(rf(&w, CAM + 4 * k), rf(&w, e + 0x24 + 4 * k))?;
            wf(&mut w, e + 0x24 + 4 * k, v);
        }
    }
    // The callee-saved FPRs the port holds (V's entries), which deeper
    // callees save.
    for (r, (i, j)) in [(20, (2, 2)), (22, (0, 2)), (24, (1, 2)), (26, (2, 3)), (28, (0, 3)), (30, (1, 3))] {
        w.ctx.fpr[r].set_fl(v3[i][j]);
    }
    call_c(&mut w, 0x48, &[], imports::func_80034948);
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_800349C4(seed: u64, depth in 0u32..8, e in prop::array::uniform12(fin()), v in prop::array::uniform16(fin()), c in prop::array::uniform3(fin()), rel in prop_oneof![Just(0u32), any::<u32>()]) {
        let mut s = state(seed);
        wr(&mut s, MTX43_DEPTH, depth);
        let ea = MTX43_STACK + 48 * depth;
        for (k, x) in e.iter().enumerate() {
            wf(&mut s, ea + 4 * k as u32, *x);
        }
        for (k, x) in v.iter().enumerate() {
            wf(&mut s, VIEW + 4 * k as u32, *x);
        }
        for (k, x) in c.iter().enumerate() {
            wf(&mut s, CAM + 4 * k as u32, *x);
        }
        wr(&mut s, REL, rel);
        let want = view_model(&s);
        prop_assume!(want.is_some());
        let after = run("func_800349C4", misc::func_800349C4, &s)?;
        same_memory(&after, &want.unwrap())?;
    }
}

// ---- func_80038C3C / func_8003931C / func_8003F974 ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80038C3C(seed: u64, head in (0x8030_4000u32..0x8030_5000).prop_map(|h| h & !7), src in prop::collection::vec(any::<u32>(), 13)) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x38C, 0x8011_2DD0, 0x60);
        for (k, v) in src.iter().enumerate() {
            wr(&mut s, 0x800A_3D68 + 4 * k as u32, *v);
        }
        wr(&mut s, DL2_HEAD, head);
        let mut w = s.clone();
        wr(&mut w, SP_AT - 0x18 + 0x14, s.ctx.gpr[RA] as u32);
        wr(&mut w, 0x8011_2DD8, 0x0020_0405);
        for k in 0..13 {
            let v = word(&w, 0x800A_3D68 + 4 * k);
            wr(&mut w, 0x8011_2DE0 + 4 * k, v);
        }
        wr(&mut w, 0x8011_2E14, 0);
        wr(&mut w, 0x8011_2E18, u32::MAX);
        let cmds = [(0xD9FF_FFFFu32, 0x0020_0405u32, true), (0xD9F0_FDFF, 0, false), (0xD700_0000, 0, false), (0xE700_0000, 0, false), (0xDE00_0000, 0x800A_4090, false)];
        for (hi, lo, hi_first) in cmds {
            let h = word(&w, DL2_HEAD);
            wr(&mut w, DL2_HEAD, h.wrapping_add(8));
            if hi_first {
                wr(&mut w, h, hi);
                wr(&mut w, h + 4, lo);
            } else {
                wr(&mut w, h + 4, lo);
                wr(&mut w, h, hi);
            }
        }
        call_c(&mut w, 0x18, &[(A0, sext(0x800A_3D68)), (A1, 1)], imports::func_80035BF0);
        let after = run("func_80038C3C", render::func_80038C3C, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_8003931C(seed: u64, table_built: bool) {
        let mut s = state(seed);
        let p = 0x8030_6000u32;
        s.randomise_memory(seed ^ 0x931C, p, 0x400);
        if !table_built {
            wr(&mut s, 0x8011_4074, 0);
        }
        s.ctx.gpr[A0] = sext(p);
        let mut w = s.clone();
        wr(&mut w, SP_AT - 0x18 + 0x14, s.ctx.gpr[RA] as u32);
        let v0 = call_c(&mut w, 0x18, &[(A0, sext(p + 4)), (A1, 0x3EC)], imports::func_80039178);
        let after = run("func_8003931C", save::func_8003931C, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
    }

    #[test]
    fn func_8003F974(seed: u64, a: u32, src in prop::collection::vec(any::<u32>(), 14)) {
        let mut s = state(seed);
        let p = 0x8030_7000u32;
        for (k, v) in src.iter().enumerate() {
            wr(&mut s, p + 4 * k as u32, *v);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(a), sext(p));
        let mut w = s.clone();
        wr(&mut w, SP_AT - 0x18 + 0x14, s.ctx.gpr[RA] as u32);
        call_c(&mut w, 0x18, &[(A0, sext(a)), (A1, sext(0x416C_6C21)), (A2, sext(p))], imports::func_8003F8FC);
        let after = run("func_8003F974", pools::func_8003F974, &s)?;
        same_memory(&after, &w)?;
    }
}
