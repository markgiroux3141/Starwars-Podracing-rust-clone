//! Depth 1 at 0x8007B06C..0x8007EE4C (game::misc): a pulse effect's frame,
//! a node tree's first textured material and its animations (recursive),
//! a transform node from a pool of 50, a scene root reset and starting a
//! spline walker. Recompiled C vs Rust with the callees as C. The models
//! replay the callees' C on a copy of the state in the same order with the
//! same arguments (and the s registers and callee-saved FPRs the port
//! holds), adding the functions' own stores; whole RDRAM (and the result
//! register) is compared.

// Tests are named after the functions (func_8007B06C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
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
    s
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
fn narrow(x: f64) -> Option<f32> {
    (!x.is_nan()).then_some(x as f32)
}

/// The C cast (`trunc.w.s`, cvttss2si): 0x80000000 out of range and for
/// NaN.
fn trunc(x: f32) -> u32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        0x8000_0000
    } else {
        x.trunc() as i32 as u32
    }
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

const O: u32 = 0x8030_0000;
const DT: u32 = 0x8012_0BF0;

// ---- the pulse ----

const NODE: u32 = 0x8030_1000;
const MAT: u32 = 0x8030_2000;
const Q: u32 = 0x8030_2100;
const TX: u32 = 0x8030_2200;

fn clamp_byte(x: u32) -> u32 {
    if x as i32 <= 0 {
        1
    } else if x as i32 >= 255 {
        254
    } else {
        x
    }
}

fn pulse_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0xA8;
    saves(&mut w, s, 0xA8, &[(0x28, S0), (0x2C, RA)]);
    if word(&w, O + 0x78) == 0 {
        return Some(w);
    }
    let tt = rf(&w, O + 0x6C);
    if 0.0 == tt {
        return Some(w);
    }
    let t = div(rf(&w, O + 0x68), tt)?;
    w.ctx.gpr[S0] = sext(O);
    let mat = word(&w, O + 0x74);
    if mat != 0 {
        let [r, g, b, mut a] = [0, 1, 2, 3].map(|k| u32::from(byte(&w, O + 0x70 + k)));
        if t < 0.25 {
            a = trunc(mul(a as i32 as f32, mul(t, 4.0)?)?);
        }
        let (r, g, b, a) = (clamp_byte(r), clamp_byte(g), clamp_byte(b), clamp_byte(a));
        wr(&mut w, sp + 0x10, g);
        wr(&mut w, sp + 0x14, b);
        wr(&mut w, sp + 0x18, a);
        wf(&mut w, sp + 0x84, t);
        call_c(&mut w, 0xA8, &[(A0, sext(mat)), (A1, 0), (A2, 0), (A3, sext16(r))], imports::func_8000E9BC);
        let v = mul(add(mul(2.0, 3.5)?, 0.5)?, narrow(rd(&w, DT))?)?;
        let mat = word(&w, O + 0x74);
        call_c(&mut w, 0xA8, &[(A0, sext(mat)), (A1, 0), (A2, sext(v.to_bits()))], imports::func_8003E0A0);
    }
    let k = add(mul(6.0, sub(1.0, t)?)?, 0.5)?;
    wr(&mut w, sp + 0x10, O + 0x20);
    call_c(&mut w, 0xA8, &[(A0, sext(sp + 0x40)), (A1, sext(0x4000_0000)), (A2, sext(k.to_bits())), (A3, sext(k.to_bits()))], imports::func_80017918);
    let node = word(&w, O + 0x78);
    call_c(&mut w, 0xA8, &[(A0, sext(node)), (A1, sext(sp + 0x40))], imports::func_80017BA8);
    Some(w)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// No node, a zero duration, `t` around 0.25 and 1, bytes around 0 and
    /// 255, a material with or without its colour and scroll records.
    #[test]
    fn func_8007B06C(seed: u64, node: bool, tq in prop_oneof![Just((1.0f32, 4.0f32)), Just((1.0f32, 0.0f32)), Just((0.0f32, -0.0f32)), (ordinary(), ordinary()), (0.0f32..10.0, 1.0f32..10.0)],
                     rgba in prop::array::uniform4(prop_oneof![Just(0u8), Just(1u8), Just(254u8), Just(255u8), any::<u8>()]), mat in 0u8..4,
                     m4 in prop::array::uniform16(ordinary()), sizes in prop::array::uniform4(any::<u16>()), d in prop_oneof![Just(1.0f64 / 60.0), -1.0f64..1.0]) {
        let mut s = state(seed);
        wr(&mut s, O + 0x78, if node { NODE } else { 0 });
        wf(&mut s, O + 0x68, tq.0);
        wf(&mut s, O + 0x6C, tq.1);
        for (k, &v) in rgba.iter().enumerate() {
            s.rdram.mem().write_u8(O + 0x70 + k as u32, v);
        }
        wr(&mut s, O + 0x74, if mat == 0 { 0 } else { MAT });
        wr(&mut s, MAT + 0xC, if mat == 1 { 0 } else { Q });
        wr(&mut s, MAT + 8, if mat == 2 { 0 } else { TX });
        for (k, &v) in sizes.iter().enumerate() {
            wh(&mut s, if k < 2 { MAT + 4 + 2 * k as u32 } else { TX + 4 + 2 * (k as u32 - 2) }, v);
        }
        for (k, &v) in m4.iter().enumerate() {
            wf(&mut s, O + 0x20 + 4 * k as u32, v);
        }
        wd(&mut s, DT, d);
        s.ctx.gpr[A0] = sext(O);
        let model = pulse_model(&s);
        prop_assume!(model.is_some());
        let after = run("func_8007B06C", misc::func_8007B06C, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- node trees ----

const NODES: u32 = 0x8032_0000;
const RECS: u32 = 0x8033_0000;
const MATS: u32 = 0x8033_8000;

/// Node `i` at `NODES + 0x100 i`, children listed at `+0x80`. Kinds: 0 a
/// material node (type 0x3064; children are records whose first word is a
/// material or 0, the material's `+0xC` a texture or 0), 1 a group (bit
/// 14), 2 a transform (0xD065, also bit 14), 3 neither. Groups point only
/// to later nodes (or a childless leaf), so recursion ends.
fn tree(s: &mut State, nodes: &[(u8, u32, Vec<(u8, u32)>)]) {
    let leaf = NODES + 0x100 * 8;
    wr(s, leaf, 1);
    wr(s, leaf + 0x14, 0);
    let len = nodes.len() as u32;
    for (i, (kind, extra, kids)) in nodes.iter().enumerate() {
        let i = i as u32;
        let n = NODES + 0x100 * i;
        let ty = match kind {
            0 => 0x3064,
            1 => (extra | 0x4000) & !0x8000 | 0x10,
            2 => 0xD065,
            _ => (extra & !0x4000) | 1,
        };
        wr(s, n, ty);
        wr(s, n + 0x14, kids.len() as u32);
        wr(s, n + 0x18, n + 0x80);
        for (j, &(ck, cv)) in kids.iter().enumerate() {
            let j = j as u32;
            let c = if *kind == 0 {
                let r = RECS + 0x400 * i + 0x10 * j;
                let mat = if ck == 0 { 0 } else { MATS + 0x40 * (cv % 12) };
                wr(s, r, mat);
                if mat != 0 {
                    wr(s, mat + 0xC, if ck == 1 { 0 } else { 0x100 + cv % 6 });
                }
                r
            } else if i + 1 < len {
                NODES + 0x100 * (i + 1 + cv % (len - i - 1))
            } else {
                leaf
            };
            wr(s, n + 0x80 + 4 * j, c);
        }
    }
}

fn node_strategy() -> BoxedStrategy<Vec<(u8, u32, Vec<(u8, u32)>)>> {
    prop::collection::vec((0u8..4, any::<u32>(), prop::collection::vec((0u8..3, 0u32..64), 0..4)), 1..5).boxed()
}

fn find_model(s: &State, n: u32) -> (State, u64) {
    let mut w = s.clone();
    saves(&mut w, s, 0x28, &[(0x14, S0), (0x18, S1), (0x1C, S2), (0x20, S3), (0x24, RA)]);
    if s.ctx.gpr[A0] == 0 {
        return (w, 0);
    }
    (w.ctx.gpr[S2], w.ctx.gpr[S3]) = (s.ctx.gpr[A0], 0);
    call_c(&mut w, 0x28, &[(A0, s.ctx.gpr[A0])], imports::func_80017DA4);
    if w.ctx.gpr[V0] == 0x3064 {
        let count = word(&w, n + 0x14) as i32;
        let list = word(&w, n + 0x18);
        for j in 0..count.max(0) as u32 {
            let mat = word(&w, word(&w, list + 4 * j));
            if mat != 0 && word(&w, mat + 0xC) != 0 {
                let v = sext(word(&w, mat + 0xC));
                return (w, v);
            }
            if j + 1 >= word(&w, n + 0x14) {
                break;
            }
        }
        return (w, 0);
    }
    call_c(&mut w, 0x28, &[(A0, s.ctx.gpr[A0])], imports::func_80017DA4);
    if w.ctx.gpr[V0] & 0x4000 == 0 {
        return (w, 0);
    }
    w.ctx.gpr[S0] = 0;
    call_c(&mut w, 0x28, &[(A0, s.ctx.gpr[A0])], imports::func_80017DAC);
    w.ctx.gpr[S1] = 0;
    if w.ctx.gpr[V0] as i64 <= 0 {
        return (w, 0);
    }
    let mut j = 0u32;
    loop {
        let c = word(&w, word(&w, n + 0x18) + 4 * j);
        (w.ctx.gpr[S0], w.ctx.gpr[S1]) = (u64::from(j), u64::from(4 * j));
        call_c(&mut w, 0x28, &[(A0, sext(c))], imports::func_8007B430);
        let r = w.ctx.gpr[V0];
        w.ctx.gpr[S3] = r;
        if r != 0 {
            return (w, r);
        }
        j += 1;
        (w.ctx.gpr[S0], w.ctx.gpr[S1]) = (u64::from(j), u64::from(4 * j));
        call_c(&mut w, 0x28, &[(A0, s.ctx.gpr[A0])], imports::func_80017DAC);
        if !((j as i64) < w.ctx.gpr[V0] as i64) {
            return (w, 0);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_8007B430(seed: u64, nodes in node_strategy(), null: bool) {
        let mut s = state(seed);
        tree(&mut s, &nodes);
        s.ctx.gpr[A0] = if null { 0 } else { sext(NODES) };
        let (w, v0) = find_model(&s, NODES);
        let after = run("func_8007B430", misc::func_8007B430, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
    }
}

const ANIMS: u32 = 0x800A_F4C0;
const OBJS: u32 = 0x8034_0000;

fn low(v: u64, bits: u32) -> u64 {
    v & !0xFFFF_FFFF | u64::from(bits)
}

fn anim_model(s: &State, n: u32) -> Option<State> {
    let mut w = s.clone();
    let sp = SP_AT - 0x38;
    let fv = s.ctx.fpr[20].u64;
    wr(&mut w, sp + 0x18, (fv >> 32) as u32);
    wr(&mut w, sp + 0x1C, fv as u32);
    saves(&mut w, s, 0x38, &[(0x30, S4), (0x2C, S3), (0x28, S2), (0x34, RA), (0x24, S1), (0x20, S0)]);
    let (clr, set, sb) = (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3] as u32);
    (w.ctx.gpr[S2], w.ctx.gpr[S3], w.ctx.gpr[S4]) = (s.ctx.gpr[A0], clr, set);
    w.ctx.fpr[20].u64 = low(fv, sb);
    if s.ctx.gpr[A0] == 0 {
        return Some(w);
    }
    call_c(&mut w, 0x38, &[(A0, s.ctx.gpr[A0])], imports::func_80017DA4);
    if w.ctx.gpr[V0] & 0x4000 == 0 {
        return Some(w);
    }
    call_c(&mut w, 0x38, &[(A0, s.ctx.gpr[A0])], imports::func_80017DA4);
    if w.ctx.gpr[V0] == 0xD065 {
        for kind in [8u64, 9] {
            call_c(&mut w, 0x38, &[(A0, s.ctx.gpr[A0]), (A1, kind)], imports::func_80006D5C);
            let o = w.ctx.gpr[V0] as u32;
            if o != 0 {
                let rate = rf(&w, o + 0x110);
                let fl = word(&w, o + 0x100) & !(clr as u32);
                wr(&mut w, o + 0x100, fl);
                wr(&mut w, o + 0x100, fl | set as u32);
                let a1 = if rate <= 0.0 { sb } else { (-ok(f32::from_bits(sb))?).to_bits() };
                call_c(&mut w, 0x38, &[(A0, sext(o)), (A1, sext(a1))], imports::func_80006EB4);
            }
        }
    }
    w.ctx.gpr[S1] = 0;
    call_c(&mut w, 0x38, &[(A0, s.ctx.gpr[A0])], imports::func_80017DAC);
    w.ctx.gpr[S0] = 0;
    if w.ctx.gpr[V0] as i64 <= 0 {
        return Some(w);
    }
    let mut j = 0u32;
    loop {
        let c = word(&w, word(&w, n + 0x18) + 4 * j);
        call_c(&mut w, 0x38, &[(A0, sext(c)), (A1, clr), (A2, set), (A3, sext(sb))], imports::func_8007B544);
        j += 1;
        (w.ctx.gpr[S0], w.ctx.gpr[S1]) = (u64::from(4 * j), u64::from(j));
        call_c(&mut w, 0x38, &[(A0, s.ctx.gpr[A0])], imports::func_80017DAC);
        if !((j as i64) < w.ctx.gpr[V0] as i64) {
            return Some(w);
        }
    }
}

/// Up to 8 animation objects, each bound to one of the nodes (or none),
/// of kind 8, 9 or other, some disabled (bit 31), rates of both signs.
fn anims() -> BoxedStrategy<Vec<(u32, u8, bool, f32)>> {
    prop::collection::vec((0u32..6, 0u8..4, prop::bool::weighted(0.2), prop_oneof![Just(0.0f32), Just(-0.0f32), ordinary()]), 0..8).boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_8007B544(seed: u64, nodes in node_strategy(), objs in anims(), clr: u32, set: u32, sv in prop_oneof![ordinary(), Just(f32::NAN)], null: bool, root: bool) {
        let mut s = state(seed);
        // The port's own code runs for the root only (children go through
        // the C): make it a transform node with objects bound to it often.
        let mut nodes = nodes;
        let mut objs = objs;
        if root {
            nodes[0].0 = 2;
            for o in objs.iter_mut().step_by(2) {
                o.0 = 0;
            }
        }
        tree(&mut s, &nodes);
        // A NaN s would be negated for an object above 0, also in the
        // recursive calls (a node reached twice sees the NaN rate written the
        // first time): with NaN, no objects.
        wr(&mut s, 0x8009_A2A0, if sv.is_nan() { 0 } else { objs.len() as u32 });
        for (i, &(node, kind, off, rate)) in objs.iter().enumerate() {
            let o = OBJS + 0x200 * i as u32;
            wr(&mut s, ANIMS + 4 * i as u32, o);
            let fl = word(&s, o + 0x100) & !0x8000_000F;
            let k = [8u32, 9, 3, 8][kind as usize];
            wr(&mut s, o + 0x100, fl | k | if off { 0x8000_0000 } else { 0 });
            wr(&mut s, o + 0x124, NODES + 0x100 * node);
            wf(&mut s, o + 0x110, rate);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (if null { 0 } else { sext(NODES) }, sext(clr), sext(set), sext(sv.to_bits()));
        let model = anim_model(&s, NODES);
        prop_assume!(model.is_some());
        let after = run("func_8007B544", misc::func_8007B544, &s)?;
        same_memory(&after, &model.unwrap())?;
    }
}

// ---- the transform pool ----

const TABLE: u32 = 0x8011_CA58;
const POOL: u32 = 0x8011_CB20;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The table empty, `n` at index `k`, the first zero at `k`, or full
    /// with no match (so `i = 50` reads node 0's first word).
    #[test]
    fn func_8007BB28(seed: u64, mode in 0u8..4, k in prop_oneof![Just(0u32), Just(49u32), 0u32..50], n in prop_oneof![Just(0u32), any::<u32>().prop_map(|x| 0x8030_0000 | x & 0xFFFC)], node0 in prop_oneof![Just(0u32), any::<u32>()]) {
        let mut s = state(seed);
        for j in 0..50u32 {
            let v = 0x8031_0000 + 4 * j;
            let v = match mode {
                0 => if j == 0 { 0 } else { v },
                1 => if j == k { n } else if j > k { 0 } else { v },
                2 => if j >= k { 0 } else { v },
                _ => v,
            };
            wr(&mut s, TABLE + 4 * j, v);
        }
        wr(&mut s, POOL, node0);
        s.ctx.gpr[A0] = sext(n);
        let mut w = s.clone();
        let sp = SP_AT - 0x28;
        saves(&mut w, &s, 0x28, &[(0x14, RA)]);
        let t = |w: &State, j: u32| word(w, TABLE + 4 * j);
        let i = if t(&w, 0) == 0 || sext(t(&w, 0)) == s.ctx.gpr[A0] {
            0
        } else {
            (1..50).find(|&j| t(&w, j) == 0 || sext(t(&w, j)) == s.ctx.gpr[A0]).unwrap_or(50)
        };
        let slot = TABLE + 4 * i;
        let v0 = if word(&w, slot) != 0 {
            0
        } else {
            let node = POOL + 0x58 * i;
            wr(&mut w, sp + 0x18, node);
            wr(&mut w, sp + 0x1C, slot);
            wr(&mut w, SP_AT, n);
            call_c(&mut w, 0x28, &[(A0, sext(node)), (A1, 0xD065)], imports::func_80018324);
            wr(&mut w, slot, n);
            sext(node)
        };
        let after = run("func_8007BB28", misc::func_8007BB28, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
    }
}

// ---- the scene root and the walker ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn func_8007E0AC(seed: u64) {
        let s = state(seed);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        call_c(&mut w, 0x18, &[(A0, sext(0x8011_C950)), (A1, 0x5064)], imports::func_80018324);
        wr(&mut w, 0x8011_C964, 0);
        wr(&mut w, 0x8011_C968, 0x8011_C970);
        let after = run("func_8007E0AC", misc::func_8007E0AC, &s)?;
        same_memory(&after, &w)?;
    }
}

const SPLINE: u32 = 0x8031_0000;
const POINTS: u32 = 0x8031_1000;
const WALK: u32 = 0x8031_4000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A 16-point spline with 0..2 successors per point and its flag set
    /// or clear.
    #[test]
    fn func_8007EE4C(seed: u64, flag in prop_oneof![Just(0i16), Just(1i16), any::<i16>()], links in prop::collection::vec((0i16..3, 0i16..16), 16)) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 1, SPLINE, 0x5000);
        wh(&mut s, SPLINE, flag as u16);
        wr(&mut s, SPLINE + 4, 16);
        wr(&mut s, SPLINE + 0xC, POINTS);
        for (k, &(count, next)) in links.iter().enumerate() {
            let p = POINTS + 0x54 * k as u32;
            wh(&mut s, p, count as u16);
            wh(&mut s, p + 4, next as u16);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(WALK), sext(SPLINE));
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        wr(&mut w, WALK, SPLINE);
        for off in [0x20u32, 0x24, 0x28, 0x2C, 4, 0xC, 8] {
            wr(&mut w, WALK + off, 0);
        }
        wr(&mut w, SP_AT, WALK);
        call_c(&mut w, 0x18, &[(A0, sext(WALK)), (A1, 0)], imports::func_8003B250);
        let after = run("func_8007EE4C", misc::func_8007EE4C, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(WALK));
    }
}

/// Cases random inputs don't reach: `t` just above 0.25 with an alpha of
/// 200 (`trunc(200 * 4t)` is 201 there, so the 0.25 bound shows), and a
/// `t = 1.67` (`0x3FD5C28F`), where a fused `6 (1 - t) + 0.5` rounds
/// differently: found by an exact search, since the product must be
/// rounded to a coarser ulp than the sum (`|6 (1 - t)|` in `[4, 4.5)`,
/// the sum below 4); elsewhere the product is exact or rounding twice
/// agrees with rounding once.
#[test]
fn func_8007B06C_separators() {
    let near = f32::from_bits(0x3FD5_C28F);
    assert_ne!(6.0f32 * (1.0 - near) + 0.5, 6.0f32.mul_add(1.0 - near, 0.5));
    for (seed, (x, tt)) in [(1.0077f32, 4.0f32), (near, 1.0)].into_iter().enumerate() {
        let mut s = state(seed as u64);
        wr(&mut s, O + 0x78, NODE);
        wf(&mut s, O + 0x68, x);
        wf(&mut s, O + 0x6C, tt);
        for (k, v) in [10u8, 20, 30, 200].into_iter().enumerate() {
            s.rdram.mem().write_u8(O + 0x70 + k as u32, v);
        }
        wr(&mut s, O + 0x74, MAT);
        wr(&mut s, MAT + 0xC, Q);
        wr(&mut s, MAT + 8, TX);
        for k in 0..16u32 {
            wf(&mut s, O + 0x20 + 4 * k, 1.0 + k as f32);
        }
        wd(&mut s, DT, 1.0 / 60.0);
        s.ctx.gpr[A0] = sext(O);
        let model = pulse_model(&s).unwrap();
        let after = compare("func_8007B06C", misc::func_8007B06C, &s).unwrap_or_else(|d| panic!("{d}"));
        same_memory(&after, &model).unwrap();
    }
}
