//! Leaves at 0x8002EA28..0x80036A1C: the pads update (game::input), the
//! 3x4 stack's multiplying push (game::misc) and box culling
//! (game::render). Recompiled C vs Rust, each checked against its
//! statement: the store-only ones by simulating the stores on a copy of the
//! input and comparing all of RDRAM, the culling by a model of its result.
//!
//! NaN operands of arithmetic are outside the domain (NAN_CHECK in the
//! oracle); models return None for inputs that would reach one, skipped
//! before the C runs.

// Tests are named after the functions (func_8002EA28), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::recomp::{reg::*, RecompFn};
use game::{input, misc, render};
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

// ---- func_8002EA28: pads update ----

const SETTINGS: u32 = 0x800D_697C;
const NO_PADS: u32 = 0x800D_7490;
const PRESENT: u32 = 0x800D_7498;
const BUTTONS: u32 = 0x800D_74A8;
const PADS: u32 = 0x800D_74C0;
const RECORDS: u32 = input::PAD_RECORDS;
const SKIP: u32 = 0x800A_26D8;

fn clamp_stick(v: i8) -> u16 {
    (v as i32).clamp(-100, 100) as i16 as u16
}

/// The statement's stores, on a copy of `s`.
fn pads_expect(s: &State) -> State {
    let mut sim = s.clone();
    let r32 = |a: u32| word(s, a);
    let mut m = sim.rdram.mem();
    m.write_u32(SP_AT - 4, s.ctx.gpr[S1] as u32);
    m.write_u32(SP_AT - 8, s.ctx.gpr[S0] as u32);
    if r32(SETTINGS) & 0x1000 != 0 {
        m.write_u32(RECORDS + 0x14, 0);
        m.write_u16(RECORDS, 0);
        m.write_u16(RECORDS + 2, 0);
        for k in 0..16u32 {
            m.write_u8(RECORDS + 4 + k, 0);
        }
    }
    if r32(NO_PADS) != u32::MAX && r32(SKIP) == 0 {
        for i in 0..4u32 {
            let rec = RECORDS + 0x18 * i;
            if r32(PRESENT + 4 * i) == 0 {
                m.write_u16(rec, 0);
                m.write_u16(rec + 2, 0);
                for k in 0..16u32 {
                    m.write_u8(rec + 4 + k, 0);
                }
            } else {
                let pad = PADS + 6 * i;
                let b = m.read_u16(pad);
                let (x, y) = (m.read_i8(pad + 2), m.read_i8(pad + 3));
                m.write_u16(BUTTONS, b);
                m.write_u16(rec, clamp_stick(x));
                m.write_u16(rec + 2, clamp_stick(y));
                m.write_u32(rec + 0x14, b as u32);
                for k in 0..16u32 {
                    m.write_u8(rec + 4 + k, ((b >> k) & 1) as u8);
                }
            }
        }
    }
    m.write_u32(SKIP, 0);
    drop(m);
    sim
}

fn stick() -> BoxedStrategy<i8> {
    prop_oneof![4 => any::<i8>(), 1 => prop::sample::select(vec![-128i8, -101, -100, -99, 0, 99, 100, 101, 127])].boxed()
}

// ---- func_80033F94: push m * top ----

const DEPTH: u32 = 0x800A_3FF0;
const FLAG: u32 = 0x800A_3FF4;
const STACK: u32 = 0x8011_2EA0;
const OBJ: u32 = 0x8030_1000;

/// `m * p` in the statement's order, element by element, or None.
fn push_product(mm: &[f32; 12], p: &[f32; 12]) -> Option<Vec<f32>> {
    let (me, pe) = (|r: usize, c: usize| mm[3 * r + c], |r: usize, c: usize| p[3 * r + c]);
    (0..12)
        .map(|k| {
            let (r, c) = (k / 3, k % 3);
            let s01 = add(mul(pe(0, c), me(r, 0))?, mul(pe(1, c), me(r, 1))?)?;
            if r < 3 {
                add(mul(me(r, 2), pe(2, c))?, s01)
            } else {
                add(pe(3, c), add(s01, mul(pe(2, c), me(3, 2))?)?)
            }
        })
        .collect()
}

// ---- func_80036A1C: box culling ----

const BOX: u32 = 0x8030_2000;

/// The statement: 0 culled, 1 visible, 2 all inside; None for a NaN
/// operand.
fn cull(bx: &[f32; 6], mm: &[f32; 16], all: bool) -> Option<u32> {
    cull_with(bx, mm, all, None)
}

/// [`cull`], optionally with the product `M[r][c] * min[r]` fused into
/// the add that reads it, for the corners that select `min[r]` (the fused
/// mutants' form; `fused = (c, r)`).
fn cull_with(bx: &[f32; 6], mm: &[f32; 16], all: bool, fused: Option<(usize, usize)>) -> Option<u32> {
    for a in 0..3 {
        if bx[a + 3] < bx[a] {
            return Some(0);
        }
    }
    let me = |r: usize, c: usize| mm[4 * r + c];
    // p[c][r][sel]: M[r][c] times the min (0) or max (1) coordinate r.
    let mut p = [[[0.0f32; 2]; 3]; 4];
    for (c, pc) in p.iter_mut().enumerate() {
        for (r, pr) in pc.iter_mut().enumerate() {
            for (sel, v) in pr.iter_mut().enumerate() {
                *v = mul(me(r, c), bx[r + 3 * sel])?;
            }
        }
    }
    let mut state = [-2i32; 3];
    let mut flag = [if all { -1i32 } else { 0 }; 3];
    let update = |state: &mut [i32; 3], flag: &mut [i32; 3], a: usize, side: i32| {
        if side != 0 {
            flag[a] = 0;
            if state[a] == -side {
                state[a] = 0;
            } else if state[a] == -2 {
                state[a] = side;
            }
        } else {
            state[a] = 0;
            if flag[a] != 0 {
                flag[a] = 1;
            }
        }
    };
    for k in 0..8usize {
        let sel = [(k >> 2) & 1, (k >> 1) & 1, k & 1];
        let fz = |c: usize, r: usize| fused == Some((c, r)) && sel[r] == 0;
        let coord = |c: usize| {
            let pr = |r: usize| p[c][r][sel[r]];
            let s01 = if fz(c, 0) {
                me(0, c).mul_add(bx[0], pr(1))
            } else if fz(c, 1) {
                me(1, c).mul_add(bx[1], pr(0))
            } else {
                add(pr(0), pr(1))?
            };
            let s = if fz(c, 2) { me(2, c).mul_add(bx[2], s01) } else { add(s01, pr(2))? };
            add(me(3, c), s)
        };
        let w = coord(3)?;
        for a in 0..2 {
            if flag[a] != 0 || state[a] != 0 {
                let x = coord(a)?;
                let side = if !(0.0 < w) {
                    if w.is_nan() {
                        return None;
                    }
                    if -w < x {
                        1
                    } else if x < w {
                        -1
                    } else {
                        0
                    }
                } else if w < x {
                    1
                } else if x < -w {
                    -1
                } else {
                    0
                };
                update(&mut state, &mut flag, a, side);
            }
        }
        if flag[2] != 0 || state[2] != 0 {
            let side = if 0.0 < w { if w < coord(2)? { 1 } else { 0 } } else { -1 };
            update(&mut state, &mut flag, 2, side);
        }
    }
    Some(if flag.iter().all(|&f| f != 0) {
        2
    } else if state == [0; 3] {
        1
    } else {
        0
    })
}

/// A perspective-like matrix (w = z plus noise) or an arbitrary one.
fn cull_matrix() -> BoxedStrategy<[f32; 16]> {
    let persp = (0.5f32..2.0, 0.5f32..2.0, 0.5f32..1.5, -2.0f32..2.0, prop::array::uniform16(-0.2f32..0.2), any::<bool>())
        .prop_map(|(a, b, c, d, noise, clean)| {
            let mut m = [a, 0.0, 0.0, 0.0, 0.0, b, 0.0, 0.0, 0.0, 0.0, c, 1.0, 0.0, 0.0, d, 0.0];
            if !clean {
                for (x, n) in m.iter_mut().zip(noise) {
                    *x += n;
                }
            }
            m
        });
    // X = x, Y = y, Z = z, w = z exactly: corners of integer boxes land on
    // the planes.
    let unit = [1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    prop_oneof![3 => persp, 1 => Just(unit), 1 => prop::array::uniform16(ordinary())].boxed()
}

/// Boxes around the view axis at various depths and sizes, sometimes
/// inverted on one axis, sometimes with a NaN (only compared before an
/// inverted axis returns), or arbitrary corners.
fn cull_box() -> BoxedStrategy<[f32; 6]> {
    let around = (-20.0f32..20.0, -20.0f32..20.0, -10.0f32..60.0, prop::array::uniform3(0.0f32..15.0))
        .prop_map(|(x, y, z, h)| [x - h[0], y - h[1], z - h[2], x + h[0], y + h[1], z + h[2]]);
    let small = (-1.0f32..1.0, -1.0f32..1.0, 2.0f32..20.0, 0.0f32..0.5).prop_map(|(x, y, z, h)| [x - h, y - h, z - h, x + h, y + h, z + h]);
    // Small integers: ties with the planes, w = 0, zero-size axes.
    let ints = prop::array::uniform6(-3i32..=3).prop_map(|v| {
        let mut b = [0.0f32; 6];
        for a in 0..3 {
            b[a] = v[a].min(v[a + 3]) as f32;
            b[a + 3] = v[a].max(v[a + 3]) as f32;
        }
        b
    });
    let base = prop_oneof![3 => around, 2 => small, 2 => ints, 1 => prop::array::uniform6(ordinary())];
    (base, prop::option::weighted(0.1, 0usize..3), prop::option::weighted(0.05, 0usize..6))
        .prop_map(|(mut b, flip, nan)| {
            if let Some(a) = flip {
                b.swap(a, a + 3);
            }
            if let Some(k) = nan {
                b[k] = f32::NAN;
            }
            b
        })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8002EA28(seed: u64, settings in prop_oneof![Just(0x1000u32), Just(0u32), any::<u32>()],
                     no_pads in prop_oneof![3 => Just(0u32), 1 => Just(u32::MAX), 1 => any::<u32>()],
                     skip in prop_oneof![3 => Just(0u32), 1 => any::<u32>()],
                     present in prop::array::uniform4(prop_oneof![Just(0u32), Just(1u32), any::<u32>()]),
                     buttons: [u16; 4], sticks in prop::array::uniform8(stick()), errs: [u8; 4],
                     records in prop::array::uniform24(any::<u32>()), shared: u16) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(SETTINGS, settings);
            m.write_u32(NO_PADS, no_pads);
            m.write_u32(SKIP, skip);
            m.write_u16(BUTTONS, shared);
            for i in 0..4u32 {
                m.write_u32(PRESENT + 4 * i, present[i as usize]);
                let pad = PADS + 6 * i;
                m.write_u16(pad, buttons[i as usize]);
                m.write_u8(pad + 2, sticks[2 * i as usize] as u8);
                m.write_u8(pad + 3, sticks[2 * i as usize + 1] as u8);
                m.write_u8(pad + 4, errs[i as usize]);
            }
            for (k, w) in records.iter().enumerate() {
                m.write_u32(RECORDS + 4 * k as u32, *w);
            }
        }
        let after = run("func_8002EA28", input::func_8002EA28, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(RECORDS));
        same_memory(&after, &pads_expect(&s))?;
    }

    /// Depths around the bound (and negative ones, which pass the signed
    /// test).
    #[test]
    fn func_80033F94(seed: u64, depth in prop_oneof![-2i32..3, 29i32..35],
                     mm in prop::array::uniform12(ordinary()), p in prop::array::uniform12(ordinary())) {
        let mut s = state(seed);
        let d = depth.wrapping_add(1);
        let entry = STACK.wrapping_add((d * 48) as u32);
        {
            let mut m = s.rdram.mem();
            m.write_u32(DEPTH, depth as u32);
            for k in 0..12u32 {
                m.write_f32(OBJ + 4 * k, mm[k as usize]);
                m.write_f32(entry.wrapping_sub(48) + 4 * k, p[k as usize]);
            }
        }
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            m.write_u32(FLAG, 1);
            if depth < 32 {
                let Some(out) = push_product(&mm, &p) else {
                    return Err(TestCaseError::reject("NaN operand"));
                };
                m.write_u32(DEPTH, d as u32);
                for (k, x) in out.iter().enumerate() {
                    m.write_f32(entry + 4 * k as u32, *x);
                }
            }
        }
        s.ctx.gpr[A0] = sext(OBJ);
        let after = run("func_80033F94", misc::func_80033F94, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80036A1C(seed: u64, bx in cull_box(), mm in cull_matrix(), all in prop_oneof![Just(0u32), Just(1u32), any::<u32>()]) {
        let want = cull(&bx, &mm, all != 0);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for (k, x) in bx.iter().enumerate() {
                m.write_f32(BOX + 4 * k as u32, *x);
            }
            for (k, x) in mm.iter().enumerate() {
                m.write_f32(render::CULL_MATRIX + 4 * k as u32, *x);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(BOX), sext(all));
        let after = run("func_80036A1C", render::func_80036A1C, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], want.unwrap() as u64);
    }
}

/// A box and matrix on which fusing `M[r][c] * min[r]` changes the result
/// (found by a deterministic search). Fused min products only feed corners
/// 0..6, whose registers the last corner overwrites, so only a changed
/// side shows them, and random inputs almost never land within an ulp of
/// a plane. Here one other product shares the partial sum, every other
/// entry of its column is 0, and the coordinate is compared with exactly
/// the unfused sum: for c < 3, `w = M[3][3]` is that sum, and fusing
/// (rounding once, upwards here) puts corner 0 outside; for the w column,
/// `X = M[3][0]` is that sum and the products are negated so that the
/// fused `w` comes out below it. The other corners are far outside.
fn cull_separator(c: usize, r: usize) -> ([f32; 6], [f32; 16]) {
    let other = if r == 0 { 1 } else { 0 };
    let mut seed = 0x9E37_79B9u32 ^ (c * 3 + r) as u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        1.0 + (seed >> 9) as f32 / (1u32 << 23) as f32
    };
    for _ in 0..10_000 {
        let (a, b, a2, b2) = (next(), next(), next(), next());
        let c2 = a2 * b2;
        let (s1, s2) = (a * b + c2, a.mul_add(b, c2));
        if (c < 3 && s2 <= s1) || (c == 3 && s2 >= s1) {
            continue;
        }
        let mut bx = [0.0f32; 6];
        let mut mm = [0.0f32; 16];
        if c < 3 {
            (bx[r], bx[other]) = (b, b2);
            (bx[r + 3], bx[other + 3]) = (b * 1048576.0, b2 * 1048576.0);
            (mm[4 * r + c], mm[4 * other + c]) = (a, a2);
            mm[15] = s1;
        } else {
            (bx[r], bx[other]) = (-b, -b2);
            (mm[4 * r + 3], mm[4 * other + 3]) = (-a, -a2);
            mm[12] = s1;
        }
        let (plain, fused) = (cull(&bx, &mm, false), cull_with(&bx, &mm, false, Some((c, r))));
        if plain.is_some() && fused.is_some() && plain != fused {
            return (bx, mm);
        }
    }
    panic!("no separator for column {c}, row {r}");
}

/// The separators for every (column, row) pair, C vs Rust.
#[test]
fn func_80036A1C_fused_separators() {
    for c in 0..4 {
        for r in 0..3 {
            let (bx, mm) = cull_separator(c, r);
            let mut s = state(c as u64 * 3 + r as u64);
            {
                let mut m = s.rdram.mem();
                for (k, x) in bx.iter().enumerate() {
                    m.write_f32(BOX + 4 * k as u32, *x);
                }
                for (k, x) in mm.iter().enumerate() {
                    m.write_f32(render::CULL_MATRIX + 4 * k as u32, *x);
                }
            }
            (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(BOX), 0);
            let after = compare("func_80036A1C", render::func_80036A1C, &s).unwrap_or_else(|d| panic!("column {c}, row {r}: {d}"));
            assert_eq!(after.ctx.gpr[V0], u64::from(cull(&bx, &mm, false).unwrap()), "column {c}, row {r}");
        }
    }
}

/// `w = 0` at the last corner with the y axis still open (every corner's
/// y above |w|): the `w <= 0` branch leaves `-w` in `f8`, which nothing
/// after it overwrites (z computes nothing when `w <= 0`).
#[test]
fn func_80036A1C_w_zero_at_the_last_corner() {
    let unit = [1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    let bx = [-1.0f32, 2.0, -1.0, 1.0, 3.0, 0.0];
    let mut s = state(7);
    {
        let mut m = s.rdram.mem();
        for (k, x) in bx.iter().enumerate() {
            m.write_f32(BOX + 4 * k as u32, *x);
        }
        for (k, x) in unit.iter().enumerate() {
            m.write_f32(render::CULL_MATRIX + 4 * k as u32, *x);
        }
    }
    (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(BOX), 0);
    let after = compare("func_80036A1C", render::func_80036A1C, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(after.ctx.gpr[V0], u64::from(cull(&bx, &unit, false).unwrap()));
    assert_eq!(after.ctx.fpr[8].u32l(), 0x8000_0000, "-w in f8");
}

/// Every result class occurs with these strategies (a check on the test,
/// not the port).
#[test]
fn cull_outcomes_occur() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let mut seen = [0u32; 3];
    let strat = (cull_box(), cull_matrix(), any::<bool>());
    for _ in 0..2000 {
        let (b, m, all) = strat.new_tree(&mut runner).unwrap().current();
        if let Some(r) = cull(&b, &m, all) {
            seen[r as usize] += 1;
        }
    }
    assert!(seen.iter().all(|&n| n >= 50), "{seen:?}");
}
