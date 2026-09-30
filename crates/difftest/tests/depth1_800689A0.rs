//! Depth 1 at 0x800689A0..0x80079714 (game::misc): a boost charge, the
//! closest approach of two moving points, steering from the nearest
//! "Test" element, a part's timer, collecting and handing out a node
//! tree's materials (recursive), pushing a point out of a slab and a
//! rate-limited control value. Recompiled C vs Rust with the callees as C.
//! The models replay the callees' C on a copy of the state in the same
//! order with the same arguments (and the s registers the port holds),
//! adding the functions' own stores; whole RDRAM (and the result register)
//! is compared. The data segment is loaded from the ROM (the callees read
//! constants from it).

// Tests are named after the functions (func_800689A0), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::imports;
use game::misc;
use game::recomp::{reg::*, RecompFn};
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
    load_data(&mut s);
    s
}

/// The game's data segment (`0x80098000..0x800AE8B0`) as the ROM image has
/// it: the test background is random, and [`func_80081700`]'s constant and
/// these functions' thresholds live there.
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

fn wd(s: &mut State, a: u32, v: f64) {
    let b = v.to_bits();
    wr(s, a, (b >> 32) as u32);
    wr(s, a + 4, b as u32);
}

fn rf(s: &State, a: u32) -> f32 {
    f32::from_bits(word(s, a))
}

fn rd(s: &State, a: u32) -> f64 {
    f64::from_bits(u64::from(word(s, a)) << 32 | u64::from(word(s, a + 4)))
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
/// `cvt.s.d` (guarded).
fn narrow(x: f64) -> Option<f32> {
    (!x.is_nan()).then_some(x as f32)
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

fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![4 => -1.0e3f32..1.0e3f32, 2 => -1.0f32..1.0, 2 => (-3i32..=3).prop_map(|n| n as f32), 1 => float()].boxed()
}

/// Bounded coordinates (no overflow in differences or squares).
fn coord() -> BoxedStrategy<f32> {
    prop_oneof![4 => -1.0e3f32..1.0e3f32, 2 => -1.0f32..1.0, 2 => (-12i32..=12).prop_map(|n| n as f32), 1 => Just(-0.0f32)].boxed()
}

fn dt() -> BoxedStrategy<f64> {
    prop_oneof![Just(1.0f64 / 60.0), Just(1.0f64 / 30.0), Just(0.0f64), 0.0f64..0.2, (-1.0e3f64..1.0e3)].boxed()
}

const O: u32 = 0x8030_0000;
const DT: u32 = 0x8012_0BF0;

// ---- the boost charge ----

fn boost_model(s: &State) -> Option<(State, u32)> {
    let mut w = s.clone();
    saves(&mut w, s, 0x18, &[(0x14, RA)]);
    let fl = word(&w, O + 0x60);
    if fl & 1 << 23 != 0 {
        let c = add(rf(&w, O + 0x1A8), mul(narrow(rd(&w, DT))?, 1.5)?)?;
        wf(&mut w, O + 0x1A8, c);
    } else {
        if 0.0 < rf(&w, O + 0x1A8) {
            wr(&mut w, SP_AT, O);
            let d = narrow(rd(&w, DT))?;
            w.ctx.fpr[12].set_fl(5.0);
            w.ctx.fpr[14].set_fl(d);
            call_c(&mut w, 0x18, &[], imports::func_80081700);
            let c = mul(rf(&w, O + 0x1A8), w.ctx.fpr[0].fl())?;
            wf(&mut w, O + 0x1A8, c);
        }
        if rf(&w, O + 0x1A8) < rf(&w, 0x800A_D520) {
            wf(&mut w, O + 0x1A8, 0.0);
        }
    }
    let fl = word(&w, O + 0x60);
    if fl & 0x200 != 0 {
        wr(&mut w, O + 0x60, fl & 0xFF7F_FFFF);
    }
    let c = rf(&w, O + 0x1A8);
    let f0 = if 0.0 < c { div(mul(rf(&w, O + 0x88), c)?, add(c, rf(&w, 0x800A_D524))?)? } else { 0.0 };
    Some((w, f0.to_bits()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Both flag bits, charges around 0 and `K1` (with `K1` perturbed to 0
    /// or below so an undecayed charge can equal it), and time steps.
    #[test]
    fn func_800689A0(seed: u64, bits in prop::array::uniform2(any::<bool>()), flags: u32, c in prop_oneof![Just(0.0f32), Just(-0.0f32), Just(0.001f32), ordinary(), 0.0f32..2.0],
                     k1 in prop_oneof![3 => Just(None), 1 => prop_oneof![Just(0.0f32), Just(-1.0f32), ordinary()].prop_map(Some)], tie: bool, g in ordinary(), d in dt()) {
        let mut s = state(seed);
        let fl = (flags & !(1 << 23 | 0x200)) | u32::from(bits[0]) << 23 | u32::from(bits[1]) << 9;
        wr(&mut s, O + 0x60, fl);
        if let Some(k) = k1 {
            wf(&mut s, 0x800A_D520, k);
        }
        let k = rf(&s, 0x800A_D520);
        wf(&mut s, O + 0x1A8, if tie && k <= 0.0 { k } else { c });
        wf(&mut s, O + 0x88, g);
        wd(&mut s, DT, d);
        s.ctx.gpr[A0] = sext(O);
        let model = boost_model(&s);
        prop_assume!(model.is_some());
        let (w, f0) = model.unwrap();
        let after = run("func_800689A0", misc::func_800689A0, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), f0);
    }
}

// ---- the closest approach ----

const PTS: u32 = 0x8030_0000;

fn approach_model(s: &State) -> Option<(State, u32)> {
    let mut w = s.clone();
    let sp = SP_AT - 0x40;
    saves(&mut w, s, 0x40, &[(0x14, S0), (0x18, S1), (0x1C, RA)]);
    let [a, b, c, d] = [A0, A1, A2, A3].map(|r| s.ctx.gpr[r] as u32);
    let (p, q, u, v) = (word(&w, SP_AT + 0x10), word(&w, SP_AT + 0x14), word(&w, SP_AT + 0x18), word(&w, SP_AT + 0x1C));
    for k in 0..2 {
        let x = sub(rf(&w, b + 4 * k), rf(&w, a + 4 * k))?;
        wf(&mut w, u + 4 * k, x);
    }
    for k in 0..2 {
        let x = sub(rf(&w, d + 4 * k), rf(&w, c + 4 * k))?;
        wf(&mut w, v + 4 * k, x);
    }
    let kk = rf(&w, 0x800A_D5E8);
    let wx = sub(rf(&w, a), rf(&w, c))?;
    wf(&mut w, sp + 0x34, wx);
    let wy = sub(rf(&w, a + 4), rf(&w, c + 4))?;
    wf(&mut w, sp + 0x38, wy);
    let ex = sub(rf(&w, u), rf(&w, v))?;
    wf(&mut w, sp + 0x28, ex);
    let ey = sub(rf(&w, u + 4), rf(&w, v + 4))?;
    wf(&mut w, sp + 0x2C, ey);
    let n = add(mul(ey, ey)?, mul(ex, ex)?)?;
    if n < kk && -ok(n)? < kk {
        return Some((w, (-1.0f32).to_bits()));
    }
    let mut t = div(-ok(add(mul(ey, wy)?, mul(wx, ex)?)?)?, n)?;
    if t < 0.0 {
        t = 0.0;
    }
    if 1.0 < t {
        t = 1.0;
    }
    // A NaN t (0 / 0) reaches the callee's guarded multiply.
    ok(t)?;
    wf(&mut w, sp + 0x20, t);
    (w.ctx.gpr[S0], w.ctx.gpr[S1]) = (s.ctx.gpr[A0], s.ctx.gpr[A2]);
    call_c(&mut w, 0x40, &[(A0, sext(p)), (A1, s.ctx.gpr[A0]), (A2, sext(t.to_bits())), (A3, sext(u))], imports::func_80015190);
    call_c(&mut w, 0x40, &[(A0, sext(q)), (A1, s.ctx.gpr[A2]), (A2, sext(t.to_bits())), (A3, sext(v))], imports::func_80015190);
    Some((w, t.to_bits()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Four points, sometimes moving in parallel (`d - c = b - a`, so `n =
    /// 0`) or nearly so.
    #[test]
    fn func_8006B9C8(seed: u64, pts in prop::array::uniform8(coord()), parallel in 0u8..4, nudge in prop_oneof![Just(0.0f32), Just(0.01f32), Just(1.0e-3f32)], same_ac: bool, tie_k: bool) {
        let mut s = state(seed);
        let mut pts = pts;
        if parallel == 0 {
            pts[6] = pts[4] + (pts[2] - pts[0]) + nudge;
            pts[7] = pts[5] + (pts[3] - pts[1]);
        }
        if same_ac {
            // w = a - c = 0, so t = -0.0.
            (pts[4], pts[5]) = (pts[0], pts[1]);
        }
        if tie_k {
            // K = n exactly, as the function computes n.
            let (ux, uy, vx, vy) = (pts[2] - pts[0], pts[3] - pts[1], pts[6] - pts[4], pts[7] - pts[5]);
            let (ex, ey) = (ux - vx, uy - vy);
            wf(&mut s, 0x800A_D5E8, ey * ey + ex * ex);
        }
        for (k, &x) in pts.iter().enumerate() {
            wf(&mut s, PTS + 0x10 * (k as u32 / 2) + 4 * (k as u32 % 2), x);
        }
        for (k, at) in [0x8030_0100u32, 0x8030_0110, 0x8030_0120, 0x8030_0130].into_iter().enumerate() {
            wr(&mut s, SP_AT + 0x10 + 4 * k as u32, at);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(PTS), sext(PTS + 0x10), sext(PTS + 0x20), sext(PTS + 0x30));
        let model = approach_model(&s);
        prop_assume!(model.is_some());
        let (w, f0) = model.unwrap();
        let after = run("func_8006B9C8", misc::func_8006B9C8, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), f0);
    }
}

// ---- steering from the nearest "Test" element ----

const POOL_LIST: u32 = 0x8031_0000;
const POOL_DESC: u32 = 0x8031_0100;
const POOL_ELEMS: u32 = 0x8031_1000;
const TEST: u32 = 0x5465_7374;
/// func_8005F31C, a verified callback: `[elem + 0x14] += 1`, `[elem + 0x18]
/// = arg` (see indirect.rs).
const MARK: u32 = 0x8005_F31C;

fn steer_model(s: &State, o: u32) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0xA0;
    saves(&mut w, s, 0xA0, &[(0x24, RA)]);
    wf(&mut w, o + 0x150, 0.0);
    wr(&mut w, sp + 0x10, 4);
    wr(&mut w, sp + 0x1C, sp + 0x3C);
    wr(&mut w, sp + 0x18, sp + 0x68);
    wr(&mut w, sp + 0x14, sp + 0x4C);
    wr(&mut w, SP_AT, o);
    wf(&mut w, sp + 0x28, 1.0);
    call_c(&mut w, 0xA0, &[(A0, sext(TEST)), (A1, sext(o + 0x50)), (A2, sext(0x451C_4000)), (A3, sext(o))], imports::func_8003FDCC);
    let n = w.ctx.gpr[V0];
    if n as i64 <= 0 {
        return Some(w);
    }
    wr(&mut w, sp + 0x9C, n as u32);
    call_c(&mut w, 0xA0, &[(A0, sext(sp + 0x5C)), (A1, sext(o + 0x30)), (A2, sext(sp + 0x68))], imports::func_80015538);
    let (cx, cy, cz) = (rf(&w, sp + 0x5C), rf(&w, sp + 0x60), rf(&w, sp + 0x64));
    let (vx, vy, vz) = (rf(&w, o + 0x194), rf(&w, o + 0x198), rf(&w, o + 0x19C));
    let x = add(mul(vz, cz)?, add(mul(cx, vx)?, mul(cy, vy)?)?)?;
    let r = ok(rf(&w, sp + 0x4C))?.sqrt();
    let mut k = 1.0f32;
    if n == 1 && word(&w, word(&w, sp + 0x3C) + 0x60) & 0x20 != 0 {
        let (dx, dy, dz) = (rf(&w, sp + 0x68), rf(&w, sp + 0x6C), rf(&w, sp + 0x70));
        let (rx, ry, rz) = (rf(&w, o + 0x30), rf(&w, o + 0x34), rf(&w, o + 0x38));
        if add(mul(dz, rz)?, add(mul(rx, dx)?, mul(ry, dy)?)?)? < 0.0 {
            k = -1.0;
        }
    }
    if 0.0 < x || x < 0.0 {
        let q = div(sub(50.0, r)?, 5.0)?;
        let v = mul(k, mul(div(mul(q, q)?, 10.0)?, 8.0)?)?;
        let old = rf(&w, o + 0x1F0);
        let now = if 0.0 < x { add(old, v)? } else { sub(old, v)? };
        wf(&mut w, o + 0x1F0, now);
    }
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// A "Test" pool (enabled or not) of up to 4 elements near `o`, `o`
    /// itself among them or not, flagged ones, bit 5 set on the elements or
    /// not, and the dot products of both signs.
    #[test]
    fn func_8006D7F0(seed: u64, count in 0u32..5, enabled in prop_oneof![3 => Just(1u32), 1 => Just(0u32)], mine: bool, flags in prop::array::uniform4(0u8..4),
                     offs in prop::collection::vec(prop::array::uniform3(prop_oneof![3 => -40.0f32..40.0, 1 => coord()]), 4), rows in prop::array::uniform6(coord()), pos in prop::array::uniform3(coord())) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 2, POOL_LIST, 0x2000);
        wr(&mut s, 0x800A_2170, POOL_LIST);
        wr(&mut s, POOL_LIST, POOL_DESC);
        wr(&mut s, POOL_LIST + 4, 0);
        wr(&mut s, POOL_DESC, TEST);
        wr(&mut s, POOL_DESC + 4, enabled);
        wr(&mut s, POOL_DESC + 8, count);
        wr(&mut s, POOL_DESC + 0xC, 0x200);
        wr(&mut s, POOL_DESC + 0x10, POOL_ELEMS);
        // o is the first element when `mine`.
        let o = if mine { POOL_ELEMS } else { O };
        for k in 0..4u32 {
            let e = POOL_ELEMS + 0x200 * k;
            let fl = flags[k as usize];
            let h = half(&s, e + 6);
            wh(&mut s, e + 6, if fl == 1 { h | 0x100 } else { h & !0x100 });
            let w60 = word(&s, e + 0x60);
            wr(&mut s, e + 0x60, if fl >= 2 { w60 | 0x20 } else { w60 & !0x20 });
            for c in 0..3 {
                wf(&mut s, e + 0x50 + 4 * c, pos[c as usize] + offs[k as usize][c as usize]);
            }
        }
        for c in 0..3 {
            wf(&mut s, o + 0x50 + 4 * c, pos[c as usize]);
            wf(&mut s, o + 0x30 + 4 * c, rows[c as usize]);
            wf(&mut s, o + 0x194 + 4 * c, rows[3 + c as usize]);
        }
        wf(&mut s, o + 0x1F0, pos[0]);
        s.ctx.gpr[A0] = sext(o);
        let model = steer_model(&s, o);
        prop_assume!(model.is_some());
        let after = run("func_8006D7F0", misc::func_8006D7F0, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- the part's timer ----

fn timer_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x40;
    saves(&mut w, s, 0x40, &[(0x14, RA)]);
    let fl = word(&w, O + 0x60);
    wr(&mut w, O + 0x60, fl & !0x10);
    wf(&mut w, O + 0x1A4, 0.0);
    let t = sub(rf(&w, O + 0x310), narrow(rd(&w, DT))?)?;
    wf(&mut w, O + 0x310, t);
    if rf(&w, O + 0x310) <= 0.0 {
        wr(&mut w, sp + 0x1C, 0x536E_6170);
        wr(&mut w, SP_AT, O);
        call_c(&mut w, 0x40, &[(A0, sext(O)), (A1, sext(sp + 0x1C))], imports::func_8003F99C);
        let k = rf(&w, 0x800A_D7F4);
        for j in 0..6 {
            let x = rf(&w, O + 0x288 + 4 * j);
            let f = word(&w, O + 0x2A0 + 4 * j);
            wr(&mut w, O + 0x2A0 + 4 * j, f & !8);
            if k < x {
                wf(&mut w, O + 0x288 + 4 * j, k);
            }
        }
        let fl = word(&w, O + 0x60);
        wr(&mut w, O + 0x60, fl & !0x800);
    }
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Timers around 0 (and `dt`), `o` in a pool with the MARK callback (or
    /// flagged), and six values around `K`.
    #[test]
    fn func_8006FC80(seed: u64, t in prop_oneof![Just(0.0f32), Just(-0.0f32), Just(1.0f32 / 60.0), ordinary()], d in dt(), flagged: bool,
                     vals in prop::array::uniform6(prop_oneof![Just(0.1f32), Just(0.1f32.next_up()), ordinary()])) {
        let mut s = state(seed);
        wr(&mut s, 0x800A_2170, POOL_LIST);
        wr(&mut s, POOL_LIST, POOL_DESC);
        wr(&mut s, POOL_LIST + 4, 0);
        wr(&mut s, POOL_DESC, 7);
        wr(&mut s, POOL_DESC + 0x24, MARK);
        wr(&mut s, O, 7);
        let h = half(&s, O + 6);
        wh(&mut s, O + 6, if flagged { h | 0x100 } else { h & !0x100 });
        wf(&mut s, O + 0x310, t);
        wd(&mut s, DT, d);
        for (j, &v) in vals.iter().enumerate() {
            wf(&mut s, O + 0x288 + 4 * j as u32, v);
        }
        s.ctx.gpr[A0] = sext(O);
        let model = timer_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_8006FC80", misc::func_8006FC80, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- the materials ----

const N_AT: u32 = 0x8011_C8D8;
const C_AT: u32 = 0x8011_C8DC;
const LIST: u32 = 0x8011_C8B0;
const NODES: u32 = 0x8032_0000;
const MATS: u32 = 0x8033_0000;

/// A tree: node `i` at `NODES + 0x100 i` with its child list at `+0x80`;
/// `kind` 0 is a material node (type 0x3064: its children are records
/// whose first word is a material or 0), 1 a group (bit 14: children are
/// nodes), 2 neither.
fn tree(s: &mut State, nodes: &[(u8, u32, Vec<(u8, u32)>)]) {
    // A leaf of neither kind that groups at the end can point to.
    let leaf = NODES + 0x100 * 8;
    wr(s, leaf, 1);
    wr(s, leaf + 0x14, 0);
    let len = nodes.len() as u32;
    for (i, (kind, extra, kids)) in nodes.iter().enumerate() {
        let i = i as u32;
        let n = NODES + 0x100 * i;
        let ty = match kind {
            0 => 0x3064,
            1 => extra | 0x4000,
            _ => (extra & !0x4000) | 1,
        };
        wr(s, n, ty);
        wr(s, n + 0x14, kids.len() as u32);
        wr(s, n + 0x18, n + 0x80);
        for (j, &(ck, cv)) in kids.iter().enumerate() {
            let j = j as u32;
            let c = if *kind == 0 {
                // A record with a material (or none), the material's id
                // at +8 (or 0).
                let r = MATS + 0x400 * i + 0x10 * j;
                let mat = if ck == 0 { 0 } else { MATS + 0x8000 + 0x40 * (cv % 12) };
                wr(s, r, mat);
                if mat != 0 {
                    wr(s, mat + 8, if ck == 1 { 0 } else { 0x100 + cv % 6 });
                }
                r
            } else if i + 1 < len {
                // Only later nodes, so the recursion ends.
                NODES + 0x100 * (i + 1 + cv % (len - i - 1))
            } else {
                leaf
            };
            wr(s, n + 0x80 + 4 * j, c);
        }
    }
}

fn node_strategy() -> BoxedStrategy<Vec<(u8, u32, Vec<(u8, u32)>)>> {
    prop::collection::vec((0u8..3, any::<u32>(), prop::collection::vec((0u8..3, 0u32..64), 0..4)), 1..5).boxed()
}

fn collect_model(s: &State, n: u32) -> State {
    let mut w = s.clone();
    saves(&mut w, s, 0x28, &[(0x18, S0), (0x1C, S1), (0x20, S2), (0x24, RA)]);
    if word(&w, N_AT) >= 10 || s.ctx.gpr[A0] == 0 {
        return w;
    }
    (w.ctx.gpr[S0], w.ctx.gpr[S2]) = (sext(N_AT), s.ctx.gpr[A0]);
    call_c(&mut w, 0x28, &[(A0, s.ctx.gpr[A0])], imports::func_80017DA4);
    if w.ctx.gpr[V0] == 0x3064 {
        if word(&w, n + 0x14) as i32 <= 0 {
            return w;
        }
        let mut j = 0u32;
        loop {
            let cnt = word(&w, N_AT);
            if cnt >= 10 {
                return w;
            }
            let a = word(&w, word(&w, word(&w, n + 0x18) + 4 * j));
            if a != 0 {
                let b = word(&w, a + 8);
                if b != 0 && !(0..cnt).any(|i| word(&w, word(&w, LIST + 4 * i) + 8) == b) {
                    wr(&mut w, LIST + 4 * cnt, a);
                    wr(&mut w, N_AT, cnt + 1);
                }
            }
            j += 1;
            if j as i32 >= word(&w, n + 0x14) as i32 {
                return w;
            }
        }
    }
    call_c(&mut w, 0x28, &[(A0, s.ctx.gpr[A0])], imports::func_80017DA4);
    if w.ctx.gpr[V0] & 0x4000 == 0 {
        return w;
    }
    recurse(&mut w, s.ctx.gpr[A0], n, imports::func_8007531C);
    w
}

/// The self-calls on each child, through the C, with the s registers the
/// port holds.
fn recurse(w: &mut State, a0: u64, n: u32, f: RecompFn) {
    w.ctx.gpr[S1] = 0;
    call_c(w, 0x28, &[(A0, a0)], imports::func_80017DAC);
    let mut j = 0u32;
    if w.ctx.gpr[V0] as i64 <= 0 {
        return;
    }
    loop {
        let c = word(w, word(w, n + 0x18) + 4 * j);
        (w.ctx.gpr[S0], w.ctx.gpr[S1]) = (u64::from(4 * j), u64::from(j));
        call_c(w, 0x28, &[(A0, sext(c))], f);
        j += 1;
        (w.ctx.gpr[S0], w.ctx.gpr[S1], w.ctx.gpr[S2]) = (u64::from(4 * j), u64::from(j), a0);
        call_c(w, 0x28, &[(A0, a0)], imports::func_80017DAC);
        if !((j as i64) < w.ctx.gpr[V0] as i64) {
            return;
        }
    }
}

fn handout_model(s: &State, n: u32) -> State {
    let mut w = s.clone();
    saves(&mut w, s, 0x28, &[(0x18, S0), (0x1C, S1), (0x20, S2), (0x24, RA)]);
    if word(&w, C_AT) as i32 >= 5 || s.ctx.gpr[A0] == 0 {
        return w;
    }
    w.ctx.gpr[S2] = s.ctx.gpr[A0];
    call_c(&mut w, 0x28, &[(A0, s.ctx.gpr[A0])], imports::func_80017DA4);
    if w.ctx.gpr[V0] == 0x3064 {
        if word(&w, n + 0x14) as i32 <= 0 {
            return w;
        }
        let mut i = word(&w, 0x800A_66C4) as i32;
        let mut j = 0u32;
        loop {
            let c = word(&w, word(&w, n + 0x18) + 4 * j);
            if word(&w, c) != 0 {
                i = i.wrapping_add(1).wrapping_rem(word(&w, N_AT) as i32);
                let v = word(&w, LIST.wrapping_add((i as u32).wrapping_mul(4)));
                wr(&mut w, c, v);
                let k = word(&w, C_AT);
                wr(&mut w, C_AT, k.wrapping_add(1));
            }
            j += 1;
            if j as i32 >= word(&w, n + 0x14) as i32 {
                break;
            }
        }
        wr(&mut w, 0x800A_66C4, i as u32);
        return w;
    }
    call_c(&mut w, 0x28, &[(A0, s.ctx.gpr[A0])], imports::func_80017DA4);
    if w.ctx.gpr[V0] & 0x4000 == 0 {
        return w;
    }
    recurse(&mut w, s.ctx.gpr[A0], n, imports::func_80075490);
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Trees of material nodes and groups, the list partly filled (with
    /// materials that repeat), `N` near 10, and a null root.
    #[test]
    fn func_8007531C(seed: u64, nodes in node_strategy(), n in prop_oneof![0u32..4, 7u32..11, any::<u32>()], null: bool) {
        let mut s = state(seed);
        // Every material gets an id first, so the list's entries often
        // match (the search stopping at a match shows in its registers).
        for j in 0..12u32 {
            wr(&mut s, MATS + 0x8000 + 0x40 * j + 8, 0x100 + j % 6);
        }
        tree(&mut s, &nodes);
        wr(&mut s, N_AT, n);
        for i in 0..10u32 {
            wr(&mut s, LIST + 4 * i, MATS + 0x8000 + 0x40 * ((seed as u32 >> i) % 12));
        }
        s.ctx.gpr[A0] = if null { 0 } else { sext(NODES) };
        let w = collect_model(&s, NODES);
        let after = run("func_8007531C", misc::func_8007531C, &s)?;
        same_memory(&after, &w)?;
    }

    /// The same trees, `N` from 1 to 10, the rotating index anything (so
    /// the remainder can be negative), and `C` around 5.
    #[test]
    fn func_80075490(seed: u64, nodes in node_strategy(), n in 1u32..11, i in prop_oneof![0u32..12, (-12i32..0).prop_map(|x| x as u32), any::<u32>().prop_map(|x| x & 0x7FFF_FFFF)],
                     c in prop_oneof![0u32..6, Just(u32::MAX), any::<u32>()], null: bool) {
        let mut s = state(seed);
        tree(&mut s, &nodes);
        wr(&mut s, N_AT, n);
        wr(&mut s, C_AT, c);
        wr(&mut s, 0x800A_66C4, i);
        for k in 0..10u32 {
            wr(&mut s, LIST + 4 * k, MATS + 0x8000 + 0x40 * ((seed as u32 >> k) % 12));
        }
        s.ctx.gpr[A0] = if null { 0 } else { sext(NODES) };
        let w = handout_model(&s, NODES);
        let after = run("func_80075490", misc::func_80075490, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- the slab ----

fn slab_model(s: &State, p: u32, q: u32) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x50;
    saves(&mut w, s, 0x50, &[(0x18, S0), (0x1C, RA)]);
    wr(&mut w, SP_AT + 8, q);
    if word(&w, word(&w, word(&w, O + 0x1E70) + 0x18)) == 14 {
        return Some(w);
    }
    for c in 0..3 {
        let v = word(&w, p + 4 * c);
        wr(&mut w, sp + 0x44 + 4 * c, v);
    }
    w.ctx.gpr[S0] = sext(O);
    call_c(&mut w, 0x50, &[(A0, sext(sp + 0x2C)), (A1, sext(sp + 0x44)), (A2, sext(O + 0x3C0))], imports::func_8001535C);
    call_c(&mut w, 0x50, &[(A0, sext(sp + 0x20)), (A1, sext(sp + 0x44)), (A2, sext(O + 0x400))], imports::func_8001535C);
    let dot = |w: &State, d: u32, nn: u32| -> Option<f32> {
        add(mul(rf(w, nn + 8), rf(w, d + 8))?, add(mul(rf(w, d), rf(w, nn))?, mul(rf(w, d + 4), rf(w, nn + 4))?)?)
    };
    let a = dot(&w, sp + 0x2C, O + 0x390)?;
    let b = dot(&w, sp + 0x20, O + 0x3D0)?;
    let h = mul(rf(&w, O + 0x1E64), 0.5)?;
    let (sv, nn, go) = if mul(a, a)? < mul(b, b)? {
        let sv = -ok(sub(a, h)?)?;
        (sv, O + 0x390, sv < 0.0)
    } else {
        let sv = -ok(add(h, b)?)?;
        (sv, O + 0x3D0, 0.0 < sv)
    };
    if go {
        call_c(&mut w, 0x50, &[(A0, sext(q)), (A1, sext(sp + 0x44)), (A2, sext(sv.to_bits())), (A3, sext(nn))], imports::func_800155EC);
    }
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The kind 14 (skipped) or not, two planes, a point between them or
    /// outside, a thickness around the distances.
    #[test]
    fn func_80075A3C(seed: u64, kind in prop_oneof![1 => Just(14u32), 4 => 0u32..14], pl in prop::array::uniform12(coord()), pt in prop::array::uniform3(coord()), r in prop_oneof![Just(0.0f32), coord()], same: bool) {
        let mut s = state(seed);
        let mut pl = pl;
        if same {
            // Both planes the same: A = B.
            for c in 0..3 {
                pl[3 + c] = pl[c];
                pl[9 + c] = pl[6 + c];
            }
        }
        wr(&mut s, O + 0x1E70, 0x8031_0000);
        wr(&mut s, 0x8031_0018, 0x8031_0100);
        wr(&mut s, 0x8031_0100, kind);
        for (k, at) in [0x3C0u32, 0x400, 0x390, 0x3D0].into_iter().enumerate() {
            for c in 0..3 {
                wf(&mut s, O + at + 4 * c, pl[3 * k + c as usize]);
            }
        }
        wf(&mut s, O + 0x1E64, r);
        let (p, q) = (0x8031_0200u32, 0x8031_0300u32);
        for c in 0..3 {
            wf(&mut s, p + 4 * c, pt[c as usize]);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(O), sext(p), sext(q));
        let model = slab_model(&s, p, q);
        prop_assume!(model.is_some());
        let after = run("func_80075A3C", misc::func_80075A3C, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- the rate-limited value ----

fn rate_model(s: &State, k: u32) -> Option<(State, u32)> {
    let mut w = s.clone();
    let sp = SP_AT - 0x30;
    saves(&mut w, s, 0x30, &[(0x1C, RA), (0x38, A2), (0x3C, A3)]);
    let dtv = rd(&w, DT);
    let (a, b) = (rf(&w, SP_AT + 0x10), rf(&w, SP_AT + 0x14));
    let (a, b) = (ok(a)?, ok(b)?);
    if dtv.is_nan() {
        return None;
    }
    let big_a = narrow(f64::from(a) * dtv)?;
    let mut big_b = narrow(f64::from(b) * dtv)?;
    wf(&mut w, SP_AT + 0x10, big_a);
    wf(&mut w, SP_AT + 0x14, big_b);
    let fl = word(&w, O + 0x60);
    let at = O.wrapping_add(k.wrapping_mul(12));
    let c = rf(&w, at + 0x1614);
    let mut v = f32::from_bits(s.ctx.gpr[A2] as u32);
    if fl & 1 << 23 != 0 {
        v = div(v, 2.0)?;
    }
    if fl & 0x200 != 0 {
        v = f32::from_bits(s.ctx.gpr[A3] as u32);
        big_b = mul(big_b, 2.0)?;
        wf(&mut w, SP_AT + 0x14, big_b);
    }
    wr(&mut w, sp + 0x24, at);
    wr(&mut w, SP_AT, O);
    wf(&mut w, sp + 0x2C, c);
    wf(&mut w, sp + 0x10, big_b);
    wf(&mut w, sp + 0x14, big_a);
    ok(c)?;
    w.ctx.fpr[12].set_fl(v);
    w.ctx.fpr[14].set_fl(c);
    let (lo, hi) = (word(&w, SP_AT + 0x18), word(&w, SP_AT + 0x1C));
    call_c(&mut w, 0x30, &[(A2, sext(lo)), (A3, sext(hi))], imports::func_80073C58);
    let r = w.ctx.fpr[0].fl();
    let res = if word(&w, O + 0x64) & 1 << 25 != 0 { mul(add(r, c)?, 0.5)? } else { r };
    wf(&mut w, at + 0x1614, res);
    Some((w, res.to_bits()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Both flag bits of `[o + 0x60]` and bit 25 of `[o + 0x64]`, targets
    /// near the limits, and time steps.
    #[test]
    fn func_80079714(seed: u64, k in 0u32..8, bits in prop::array::uniform3(any::<bool>()), flags: u32, flags2: u32, vals in prop::array::uniform6(ordinary()), c in ordinary(), d in dt()) {
        let mut s = state(seed);
        wr(&mut s, O + 0x60, (flags & !(1 << 23 | 0x200)) | u32::from(bits[0]) << 23 | u32::from(bits[1]) << 9);
        wr(&mut s, O + 0x64, (flags2 & !(1 << 25)) | u32::from(bits[2]) << 25);
        wf(&mut s, O + 12 * k + 0x1614, c);
        wd(&mut s, DT, d);
        for (j, &v) in vals[2..].iter().enumerate() {
            wf(&mut s, SP_AT + 0x10 + 4 * j as u32, v);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(O), u64::from(k), sext(vals[0].to_bits()), sext(vals[1].to_bits()));
        let model = rate_model(&s, k);
        prop_assume!(model.is_some());
        let (w, f0) = model.unwrap();
        let after = run("func_80079714", misc::func_80079714, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), f0);
    }
}

/// `R1 . d == 0` with a nonzero cross product: `R1 = (1, 0, 0)`, the one
/// element (bit 5 set) at `d = (0, 5, 0)`, `V = (0, 0, 1)`, so `x = 5` and
/// `k` stays 1 (`0 < 0` is false).
#[test]
fn func_8006D7F0_orthogonal() {
    for seed in 0..3u64 {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 2, POOL_LIST, 0x2000);
        wr(&mut s, 0x800A_2170, POOL_LIST);
        wr(&mut s, POOL_LIST, POOL_DESC);
        wr(&mut s, POOL_LIST + 4, 0);
        wr(&mut s, POOL_DESC, TEST);
        wr(&mut s, POOL_DESC + 4, 1);
        wr(&mut s, POOL_DESC + 8, 1);
        wr(&mut s, POOL_DESC + 0xC, 0x200);
        wr(&mut s, POOL_DESC + 0x10, POOL_ELEMS);
        let e = POOL_ELEMS;
        let h = half(&s, e + 6);
        wh(&mut s, e + 6, h & !0x100);
        let w60 = word(&s, e + 0x60);
        wr(&mut s, e + 0x60, w60 | 0x20);
        for (c, (p, d, r1, v)) in [(1.0f32, 0.0f32, 1.0f32, 0.0f32), (2.0, 5.0, 0.0, 0.0), (3.0, 0.0, 0.0, 1.0)].into_iter().enumerate() {
            let c = c as u32;
            wf(&mut s, O + 0x50 + 4 * c, p);
            wf(&mut s, e + 0x50 + 4 * c, p + d);
            wf(&mut s, O + 0x30 + 4 * c, r1);
            wf(&mut s, O + 0x194 + 4 * c, v);
        }
        wf(&mut s, O + 0x1F0, seed as f32);
        s.ctx.gpr[A0] = sext(O);
        let w = steer_model(&s, O).unwrap();
        assert_ne!(rf(&w, O + 0x1F0), seed as f32);
        let after = compare("func_8006D7F0", misc::func_8006D7F0, &s).unwrap_or_else(|d| panic!("{d}"));
        same_memory(&after, &w).unwrap();
    }
}

/// A separator for a fused `x = Vz * cz + (cx * Vx + cy * Vy)`: with `R1 =
/// (0, 1, 0)` the cross product is `c = (dz, 0, -dx)`; with `V = (1, 0, Vz)`
/// and `dz = -fl(Vz * cz)`, the unfused sum is exactly 0 (no update) while
/// the exact product's rounding error survives a fused one.
#[test]
fn func_8006D7F0_fused_separator() {
    for (seed, (vz, cz)) in [(1.1f32, 3.3f32), (0.7, 1.3), (2.9, 0.37)].into_iter().enumerate() {
        let p = vz * cz;
        assert_ne!(vz.mul_add(cz, -p), 0.0);
        let mut s = state(seed as u64);
        s.randomise_memory(seed as u64 ^ 2, POOL_LIST, 0x2000);
        wr(&mut s, 0x800A_2170, POOL_LIST);
        wr(&mut s, POOL_LIST, POOL_DESC);
        wr(&mut s, POOL_LIST + 4, 0);
        wr(&mut s, POOL_DESC, TEST);
        wr(&mut s, POOL_DESC + 4, 1);
        wr(&mut s, POOL_DESC + 8, 1);
        wr(&mut s, POOL_DESC + 0xC, 0x200);
        wr(&mut s, POOL_DESC + 0x10, POOL_ELEMS);
        let e = POOL_ELEMS;
        let h = half(&s, e + 6);
        wh(&mut s, e + 6, h & !0x100);
        let w60 = word(&s, e + 0x60);
        wr(&mut s, e + 0x60, w60 & !0x20);
        // d = (-cz, 0, -p): c = (dz, 0, -dx) = (-p, 0, cz).
        for (c, (d, r1, v)) in [(-cz, 0.0f32, 1.0f32), (0.0, 1.0, 0.0), (-p, 0.0, vz)].into_iter().enumerate() {
            let c = c as u32;
            wf(&mut s, O + 0x50 + 4 * c, 0.0);
            wf(&mut s, e + 0x50 + 4 * c, d);
            wf(&mut s, O + 0x30 + 4 * c, r1);
            wf(&mut s, O + 0x194 + 4 * c, v);
        }
        wf(&mut s, O + 0x1F0, 1.0);
        s.ctx.gpr[A0] = sext(O);
        let w = steer_model(&s, O).unwrap();
        assert_eq!(rf(&w, O + 0x1F0), 1.0);
        let after = compare("func_8006D7F0", misc::func_8006D7F0, &s).unwrap_or_else(|d| panic!("{d}"));
        same_memory(&after, &w).unwrap();
    }
}

/// One "Test" element at `d` from `o` (at the origin), with `R1`, `V` and
/// bit 5 of the element as given; the state for the pinned cases.
fn steer_case(seed: u64, d: [f32; 3], r1: [f32; 3], v: [f32; 3], bit5: bool) -> State {
    let mut s = state(seed);
    s.randomise_memory(seed ^ 2, POOL_LIST, 0x2000);
    wr(&mut s, 0x800A_2170, POOL_LIST);
    wr(&mut s, POOL_LIST, POOL_DESC);
    wr(&mut s, POOL_LIST + 4, 0);
    wr(&mut s, POOL_DESC, TEST);
    wr(&mut s, POOL_DESC + 4, 1);
    wr(&mut s, POOL_DESC + 8, 1);
    wr(&mut s, POOL_DESC + 0xC, 0x200);
    wr(&mut s, POOL_DESC + 0x10, POOL_ELEMS);
    let e = POOL_ELEMS;
    let h = half(&s, e + 6);
    wh(&mut s, e + 6, h & !0x100);
    let w60 = word(&s, e + 0x60);
    wr(&mut s, e + 0x60, if bit5 { w60 | 0x20 } else { w60 & !0x20 });
    for c in 0..3 {
        wf(&mut s, O + 0x50 + 4 * c as u32, 0.0);
        wf(&mut s, e + 0x50 + 4 * c as u32, d[c]);
        wf(&mut s, O + 0x30 + 4 * c as u32, r1[c]);
        wf(&mut s, O + 0x194 + 4 * c as u32, v[c]);
    }
    wf(&mut s, O + 0x1F0, 1.0);
    s.ctx.gpr[A0] = sext(O);
    s
}

fn steer_check(s: &State) {
    let w = steer_model(s, O).unwrap();
    let after = compare("func_8006D7F0", misc::func_8006D7F0, s).unwrap_or_else(|d| panic!("{d}"));
    same_memory(&after, &w).unwrap();
}

/// Separators for fusing `cx * Vx` into `x`'s partial sum and `ry * dy`
/// into the sign test's: `R1 = (1, 1, 0)`, `d = (0, dy, 3)` give `c = (3,
/// -3, dy)`; with `V = (vx, vy, 1)` and `dy = -(fl(3 vx) + fl(-3 vy))` the
/// unfused `x` is 0, and `vy` is searched until the fused partial sum
/// differs. For the sign: `R1 = (1, ry, 0)`, `d = (-fl(ry dy), dy, 1)` make
/// the unfused `R1 . d` exactly 0 (`k = 1`) while the fused one is the
/// product's (negative) rounding error (`k = -1`).
#[test]
fn func_8006D7F0_fused_separators() {
    let vx = 1.1f32;
    let vy = (0..10_000).map(|i| 0.7f32 + i as f32 * 1.0e-4).find(|&vy| {
        let q = -3.0 * vy;
        (3.0 * vx + q) != 3.0f32.mul_add(vx, q)
    }).unwrap();
    let dy = -((3.0 * vx) + (-3.0 * vy));
    steer_check(&steer_case(1, [0.0, dy, 3.0], [1.0, 1.0, 0.0], [vx, vy, 1.0], false));
    let (ry, dy) = (0..10_000).map(|i| (1.3f32 + i as f32 * 1.0e-4, 2.7f32)).find(|&(ry, dy)| ry.mul_add(dy, -(ry * dy)) < 0.0).unwrap();
    let dx = -(ry * dy);
    steer_check(&steer_case(2, [dx, dy, 1.0], [1.0, ry, 0.0], [0.0, 0.0, 1.0], true));
}

/// `s` exactly 0 in both branches of the slab push: with `n1 = n2 = (0, 0,
/// 1)`, `h = 1` (`[o + 0x1E64] = 2`) and `P.z - p1.z = 1`, `A = h` so `s =
/// -(A - h) = -0.0` on the first plane (`A * A < B * B` with `B` large);
/// with `P.z - p2.z = -1` and `A` large, `s = -(h + B) = -0.0` on the second.
#[test]
fn func_80075A3C_zero_push() {
    for (seed, (z1, z2)) in [(0.0f32, 5.0f32), (-5.0, 0.0), (4.0, 2.0)].into_iter().enumerate() {
        let mut s = state(seed as u64);
        wr(&mut s, O + 0x1E70, 0x8031_0000);
        wr(&mut s, 0x8031_0018, 0x8031_0100);
        wr(&mut s, 0x8031_0100, 0);
        for c in 0..3 {
            wf(&mut s, O + 0x390 + 4 * c, if c == 2 { 1.0 } else { 0.0 });
            wf(&mut s, O + 0x3D0 + 4 * c, if c == 2 { 1.0 } else { 0.0 });
            wf(&mut s, O + 0x3C0 + 4 * c, if c == 2 { z1 } else { 0.0 });
            wf(&mut s, O + 0x400 + 4 * c, if c == 2 { z2 } else { 0.0 });
        }
        wf(&mut s, O + 0x1E64, 2.0);
        let (p, q) = (0x8031_0200u32, 0x8031_0300u32);
        for c in 0..3 {
            wf(&mut s, p + 4 * c, if c == 2 { 1.0 } else { 0.0 });
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(O), sext(p), sext(q));
        let w = slab_model(&s, p, q).unwrap();
        let after = compare("func_80075A3C", misc::func_80075A3C, &s).unwrap_or_else(|d| panic!("{d}"));
        same_memory(&after, &w).unwrap();
    }
}

/// More separators for `func_8006D7F0`'s sign-only sums. `x`'s second
/// product: `R1 = (1, 1, 0)`, `d = (0, 1, 3)` (`c = (3, -3, 1)`) and `V =
/// (v, v, 0)` cancel `fl(3v) + fl(-3v)` exactly, which either fusion breaks.
/// The sign test `R1 . d` with `R1 = (r, r, 0)`, `d = (a, -a, 1)` (`x = -2ra`
/// with `V = (0, 0, 1)`): its first product fused leaves `ra - fl(ra)`,
/// searched negative. Its `z` product: `R1 = (1, 0, rz)`, `d = (-fl(rz dz),
/// 0, dz)`, `V = (0, 1, 0)`: fused, `rz dz - fl(rz dz)`, searched negative.
#[test]
fn func_8006D7F0_fused_separators_2() {
    let v = 1.1f32;
    assert_ne!(3.0f32.mul_add(v, -(3.0 * v)), 0.0);
    steer_check(&steer_case(3, [0.0, 1.0, 3.0], [1.0, 1.0, 0.0], [v, v, 0.0], false));
    let a = 2.7f32;
    let r = (0..10_000).map(|i| 1.3f32 + i as f32 * 1.0e-4).find(|&r| r.mul_add(a, -(r * a)) < 0.0).unwrap();
    steer_check(&steer_case(4, [a, -a, 1.0], [r, r, 0.0], [0.0, 0.0, 1.0], true));
    let dz = 2.3f32;
    let rz = (0..10_000).map(|i| 0.9f32 + i as f32 * 1.0e-4).find(|&rz| dz.mul_add(rz, -(dz * rz)) < 0.0).unwrap();
    steer_check(&steer_case(5, [-(dz * rz), 0.0, dz], [1.0, 0.0, rz], [0.0, 1.0, 0.0], true));
}
