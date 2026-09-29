//! Float leaves at 0x80071820..0x80080350: a meter charge, a slot bump, a
//! double clamp, the grid setup, a getter and two zeroing functions
//! (game::misc), two "any object done" walks (game::anim) and the spline
//! walker's placement by segment (game::spline). Recompiled C vs Rust, each
//! checked against its statement, the ones that store by simulating the
//! stores on a copy of the input and comparing all of RDRAM.
//!
//! NaN operands of arithmetic are outside the domain (NAN_CHECK in the
//! oracle); models return None for them, skipped before the C runs.

// Tests are named after the functions (func_80071820), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::recomp::{reg::*, RecompFn};
use game::{anim, misc, spline};
use n64mem::Mem;
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

/// Halves (ties), anything, or NaN (for values only compared).
fn halves_or_nan() -> BoxedStrategy<f32> {
    prop_oneof![4 => (-8i32..8).prop_map(|n| n as f32 * 0.5), 2 => ordinary(), 1 => Just(f32::NAN)].boxed()
}

// ---- func_80071820 ----

struct Charge {
    gate: u32,
    w60: u32,
    w64: u32,
    i: u32,
    x: f32,
    rate: f32,
    value: f32,
    max: f32,
    bits: u32,
    total: f32,
}

fn charge(s: &State, c: &Charge) -> Option<State> {
    let mut sim = s.clone();
    let mut m = sim.rdram.mem();
    if c.gate == 0 && c.w60 & 0x6000 == 0 && ((c.w64 << 6) as i32) >= 0 {
        let q = P + 4 * c.i;
        m.write_u32(P + 0x60, c.w60 & 0xFF7F_FFFF);
        let mut v = add(c.value, mul(c.x, c.rate)?)?;
        m.write_f32(q + 0x288, v);
        if 1.0 < v {
            v = 1.0;
            m.write_f32(q + 0x288, v);
        }
        m.write_u32(q + 0x2A0, c.bits | 1);
        if c.max < v {
            m.write_f32(q + 0x270, v);
        }
        m.write_f32(P + 0x2C4, add(c.total, c.x)?);
    }
    drop(m);
    Some(sim)
}

// ---- func_800736AC / func_8007B7BC ----

const LIST: u32 = 0x8030_2000;
const OBJS: u32 = 0x8030_3000;

fn any_done(objs: &[(u32, f32, f32)]) -> u64 {
    objs.iter().any(|&(flags, end, time)| flags & 0x1000_0000 == 0 || end <= time) as u64
}

fn put_list(s: &mut State, objs: &[(u32, f32, f32)]) {
    let mut m = s.rdram.mem();
    for (k, &(flags, end, time)) in objs.iter().enumerate() {
        let o = OBJS + 0x200 * k as u32;
        m.write_u32(LIST + 4 * k as u32, o);
        m.write_u32(o + 0x100, flags);
        m.write_f32(o + 0x108, end);
        m.write_f32(o + 0x114, time);
    }
    m.write_u32(LIST + 4 * objs.len() as u32, 0);
}

fn object() -> BoxedStrategy<(u32, f32, f32)> {
    (prop_oneof![3 => Just(0x1000_0000u32), 1 => any::<u32>()], halves_or_nan(), halves_or_nan()).boxed()
}

// ---- func_8007DA20 ----

const MODE: u32 = 0x800A_5B5C;
const TABLES: u32 = 0x800A_DC10;

fn rom_f32(v: u32) -> f32 {
    let at = (v - 0x8000_0400 + 0x1000) as usize;
    f32::from_bits(u32::from_be_bytes(baserom()[at..at + 4].try_into().unwrap()))
}

/// The stores for `(c, d, a, b)`, or None.
fn grid_setup(m: &mut Mem, [c, d, a, b]: [f32; 4]) -> Option<()> {
    let t = div(sub(a, b)?, 3.0)?;
    let s = div(sub(d, c)?, 8.0)?;
    let g = misc::GRID;
    let tt = g + 0x10;
    m.write_f32(g + 0xC, a);
    m.write_f32(g, b);
    m.write_f32(tt, c);
    m.write_f32(tt + 0x20, d);
    m.write_f32(g + 8, sub(a, t)?);
    m.write_f32(g + 4, add(b, t)?);
    m.write_f32(tt + 4, add(c, s)?);
    for k in 2..7 {
        m.write_f32(tt + 4 * k, add(mul(k as f32, s)?, c)?);
    }
    m.write_f32(tt + 0x1C, sub(d, s)?);
    m.write_f32(0x8011_C88C, div(t, 4.0)?);
    m.write_f32(0x8011_C890, div(s, 4.0)?);
    Some(())
}

// ---- func_8007EE98 ----

/// Points the successor halfwords name lie within an s16 of `PTS` (about
/// 2.75 MB either way); the walker is beyond that, so its stores can't
/// land on a point that is read after them.
const WALKER: u32 = 0x8070_0000;
const SPL: u32 = 0x8070_0100;
const PTS: u32 = 0x8030_5000;

#[derive(Clone, Debug)]
struct Point {
    next: [i16; 2],
    ids: [i16; 8],
}

/// Point `i`'s successor `j`, read from memory as the C does (any index).
fn next_of(s: &State, i: i32, j: usize) -> i16 {
    let at = PTS.wrapping_add((0x54 * i) as u32) + 4 + 2 * j as u32;
    let w = word(s, at & !3);
    (if at & 2 == 0 { w >> 16 } else { w }) as u16 as i16
}

/// The walker's stores for `v`, on a copy of `s`.
fn place(s: &State, f_flag: i16, pts: &[Point], v: i32) -> State {
    let mut sim = s.clone();
    let mut m = sim.rdram.mem();
    let n = pts.len() as i32;
    let k = v / 10;
    let frac = ((v as f32) - (k as f32) * 10.0) / 10.0;
    if k < n {
        m.write_u32(WALKER + 0x10, k as u32);
        let a = next_of(s, k, 0);
        m.write_u32(WALKER + 0x14, a as i32 as u32);
        if f_flag != 1 {
            let b = next_of(s, a.into(), 0);
            m.write_u32(WALKER + 0x18, b as i32 as u32);
            m.write_u32(WALKER + 0x1C, next_of(s, b.into(), 0) as i32 as u32);
        }
        m.write_u32(WALKER + 0x2C, 0);
        m.write_f32(WALKER + 8, frac);
    } else if let Some((p, j)) = (0..pts.len()).flat_map(|p| (0..8).map(move |j| (p, j))).find(|&(p, j)| pts[p].ids[j] as i32 == k) {
        m.write_u32(WALKER + 0x2C, j as u32);
        m.write_u32(WALKER + 0x10, p as u32);
        m.write_f32(WALKER + 8, frac);
        let a = next_of(s, p as i32, j & 1);
        m.write_u32(WALKER + 0x14, a as i32 as u32);
        if f_flag != 1 {
            let b = next_of(s, a.into(), (j >> 1) & 1);
            m.write_u32(WALKER + 0x18, b as i32 as u32);
            m.write_u32(WALKER + 0x1C, next_of(s, b.into(), (j >> 2) & 1) as i32 as u32);
        }
    }
    drop(m);
    sim
}

fn points() -> BoxedStrategy<Vec<Point>> {
    (1usize..6)
        .prop_flat_map(|n| {
            let idx = move || 0i16..n as i16;
            prop::collection::vec(
                (prop::array::uniform2(idx()), prop::array::uniform8(prop_oneof![3 => Just(-1i16), 2 => (n as i16)..(n as i16 + 6), 1 => any::<i16>()])),
                n,
            )
        })
        .prop_map(|v| v.into_iter().map(|(next, ids)| Point { next, ids }).collect())
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80071820(seed: u64, gate in prop_oneof![3 => Just(0u32), 1 => any::<u32>()],
                     w60 in prop_oneof![3 => any::<u32>().prop_map(|w| w & !0x6000), 1 => any::<u32>()],
                     w64 in prop_oneof![3 => any::<u32>().prop_map(|w| w & !0x0200_0000), 1 => any::<u32>()],
                     i in 0u32..6, x in prop_oneof![Just(0.0f32), ordinary()], rate in prop_oneof![0.0f32..2.0, ordinary()],
                     value in prop_oneof![0.0f32..1.2, Just(1.0f32), Just(0.0f32), ordinary()], max in prop_oneof![Just(-0.0f32), halves_or_nan()],
                     bits: u32, total in ordinary()) {
        let c = Charge { gate, w60, w64, i, x, rate, value, max, bits, total };
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(0x800A_52D4, gate);
            m.write_u32(P + 0x60, w60);
            m.write_u32(P + 0x64, w64);
            m.write_f32(P + 0xA0, rate);
            m.write_f32(P + 4 * i + 0x288, value);
            m.write_f32(P + 4 * i + 0x270, max);
            m.write_u32(P + 4 * i + 0x2A0, bits);
            m.write_f32(P + 0x2C4, total);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(P), u64::from(i), sext(x.to_bits()));
        let want = charge(&s, &c);
        prop_assume!(want.is_some());
        let after = run("func_80071820", misc::func_80071820, &s)?;
        same_memory(&after, &want.unwrap())?;
    }

    #[test]
    fn func_800736AC(seed: u64, objs in prop::collection::vec(object(), 0..5)) {
        let mut s = state(seed);
        put_list(&mut s, &objs);
        s.ctx.gpr[A0] = sext(LIST);
        let after = run("func_800736AC", anim::func_800736AC, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], any_done(&objs));
    }

    #[test]
    fn func_8007B7BC(seed: u64, objs in prop::collection::vec(object(), 0..5), i in 0u32..4) {
        let mut s = state(seed);
        put_list(&mut s, &objs);
        s.rdram.mem().write_u32(0x8011_C8F0 + 4 * i, LIST);
        s.ctx.gpr[A0] = u64::from(i);
        let after = run("func_8007B7BC", anim::func_8007B7BC, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], any_done(&objs));
    }

    #[test]
    fn func_80073834(seed: u64, null: bool, flags in prop::array::uniform5(prop_oneof![Just(0u32), any::<u32>()]),
                     d in ordinary(), slots in prop::array::uniform5(ordinary())) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(P + 0x344, if null { 0 } else { Q });
            m.write_f32(P + 0x250, d);
            for k in 0..5u32 {
                m.write_u32(Q + 4 + 4 * k, flags[k as usize]);
                m.write_f32(P + 0x3C8 + 0x40 * k, slots[k as usize]);
            }
        }
        s.ctx.gpr[A0] = sext(P);
        let mut want = s.clone();
        if !null {
            let mut m = want.rdram.mem();
            for k in 0..5u32 {
                if flags[k as usize] != 0 {
                    let v = add(slots[k as usize], d);
                    prop_assume!(v.is_some());
                    m.write_f32(P + 0x3C8 + 0x40 * k, v.unwrap());
                }
            }
        }
        let after = run("func_80073834", misc::func_80073834, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80073C58(seed: u64, v in prop_oneof![Just(-0.0f32), halves_or_nan()], c in ordinary(), lo in halves_or_nan(), hi in halves_or_nan(),
                     up in prop_oneof![Just(0.0f32), Just(0.5f32), ordinary()], down in prop_oneof![Just(0.0f32), Just(0.5f32), ordinary()]) {
        let (Some(top), Some(bottom)) = (add(c, up), sub(c, down)) else {
            return Err(TestCaseError::reject("NaN operand"));
        };
        let mut want = v;
        if top < want {
            want = top;
        }
        if want < bottom {
            want = bottom;
        }
        if want < lo {
            want = lo;
        }
        if hi < want {
            want = hi;
        }
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_f32(SP_AT + 0x10, up);
            m.write_f32(SP_AT + 0x14, down);
        }
        s.ctx.fpr[12].set_fl(v);
        s.ctx.fpr[14].set_fl(c);
        (s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(lo.to_bits()), sext(hi.to_bits()));
        let mut want_s = s.clone();
        {
            let mut m = want_s.rdram.mem();
            m.write_f32(SP_AT + 8, lo);
            m.write_f32(SP_AT + 0xC, hi);
        }
        let after = run("func_80073C58", misc::func_80073C58, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want.to_bits());
        same_memory(&after, &want_s)?;
    }

    }

/// `v = -0` meeting `hi = +0` after the other clamps pass it by: `hi < v`
/// is false (a `<=` would store +0).
#[test]
fn func_80073C58_signed_zero_at_hi() {
    let mut s = state(3);
    {
        let mut m = s.rdram.mem();
        m.write_f32(SP_AT + 0x10, 0.5);
        m.write_f32(SP_AT + 0x14, 0.5);
    }
    s.ctx.fpr[12].set_fl(-0.0);
    s.ctx.fpr[14].set_fl(0.0);
    (s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(f32::NAN.to_bits()), 0);
    let after = compare("func_80073C58", misc::func_80073C58, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(after.ctx.fpr[0].u32l(), 0x8000_0000);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

/// Modes 1 and 2 read their tables (the ROM's or stand-ins); any other
    /// mode uses the stale frame words (QUIRK).
    #[test]
    fn func_8007DA20(seed: u64, mode in prop_oneof![Just(1u32), Just(2u32), Just(0u32), any::<u32>()], rom: bool,
                     tables in prop::array::uniform8(ordinary()), stale in prop::array::uniform4(ordinary())) {
        let tables = if rom { std::array::from_fn(|k| rom_f32(TABLES + 4 * k as u32)) } else { tables };
        let mut s = state(seed);
        let frame = SP_AT - 0x18;
        {
            let mut m = s.rdram.mem();
            m.write_u32(MODE, mode);
            for (k, x) in tables.iter().enumerate() {
                m.write_f32(TABLES + 4 * k as u32, *x);
            }
            // (A, B, D, C) at +0, +4, +8, +0xC.
            for (k, x) in stale.iter().enumerate() {
                m.write_f32(frame + 4 * k as u32, *x);
            }
        }
        let mut want = s.clone();
        let vals = {
            let mut m = want.rdram.mem();
            let t = match mode {
                1 => Some(&tables[..4]),
                2 => Some(&tables[4..]),
                _ => None,
            };
            if let Some(t) = t {
                m.write_f32(frame + 0xC, t[0]);
                m.write_f32(frame + 8, t[1]);
                m.write_f32(frame, t[2]);
                m.write_f32(frame + 4, t[3]);
            }
            let r = |k: u32| m.read_f32(frame + k);
            [r(0xC), r(8), r(0), r(4)]
        };
        let ok = grid_setup(&mut want.rdram.mem(), vals);
        prop_assume!(ok.is_some());
        let after = run("func_8007DA20", misc::func_8007DA20, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8007EE40(seed: u64, x: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x800A_6700, x);
        let after = run("func_8007EE40", misc::func_8007EE40, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), x);
    }

    #[test]
    fn func_8007EE98(seed: u64, f_flag in prop_oneof![Just(0i16), Just(1i16), any::<i16>()], pts in points(),
                     v in prop_oneof![3 => 0i32..120, 2 => -60i32..0, 1 => 0i32..400_000]) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(WALKER, SPL);
            m.write_u16(SPL, f_flag as u16);
            m.write_u32(SPL + 4, pts.len() as u32);
            m.write_u32(SPL + 0xC, PTS);
            for (i, p) in pts.iter().enumerate() {
                let at = PTS + 0x54 * i as u32;
                m.write_u16(at + 4, p.next[0] as u16);
                m.write_u16(at + 6, p.next[1] as u16);
                for (j, id) in p.ids.iter().enumerate() {
                    m.write_u16(at + 0x42 + 2 * j as u32, *id as u16);
                }
            }
        }
        // Negative segments are points below the table (background
        // memory), and their successors anywhere within an s16 of it.
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(WALKER), sext(v as u32));
        let want = place(&s, f_flag, &pts, v);
        let after = run("func_8007EE98", spline::func_8007EE98, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80080148(seed: u64, i in 0u32..8) {
        let mut s = state(seed);
        s.randomise_memory(seed, 0x8011_DCF8, 0x200);
        s.randomise_memory(seed ^ 1, 0x8012_0408, 0x40);
        s.ctx.gpr[A0] = u64::from(i);
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            for k in 0..10 {
                m.write_u32(0x8011_DCF8 + 40 * i + 4 * k, 0);
            }
            m.write_u32(0x8012_0408 + 8 * i, 0);
            m.write_u32(0x8012_0408 + 8 * i + 4, 0);
        }
        let after = run("func_80080148", misc::func_80080148, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80080350(seed: u64, a0: u32) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = sext(a0);
        let mut want = s.clone();
        want.rdram.mem().write_u32(SP_AT, a0);
        let after = run("func_80080350", misc::func_80080350, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), 0);
        same_memory(&after, &want)?;
    }
}
