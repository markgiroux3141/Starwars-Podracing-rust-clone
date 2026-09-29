//! func_800321F0 (game::misc): seven float fields updated by level and a
//! condition value, then clamped. Recompiled C vs Rust with the constants
//! loaded from the ROM image, each result checked against the statement
//! (the table in the port's doc: update, per-level constants, bounds).
//!
//! NaN operands of the arithmetic are outside the domain; the model returns
//! None for inputs that would reach one, skipped before the C runs.

// Tests are named after the functions (func_800321F0), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::misc;
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
const P: u32 = 0x8030_0000;
/// The function's constants in the data segment.
const DATA: (u32, u32) = (0x800A_A3C8, 0x800A_A468);

fn rom_f(vaddr: u32) -> f32 {
    let o = (vaddr - 0x8000_0400 + 0x1000) as usize;
    f32::from_be_bytes(baserom()[o..o + 4].try_into().unwrap())
}

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    let o = (DATA.0 - 0x8000_0400 + 0x1000) as usize;
    s.rdram.mem().write_bytes(DATA.0, &baserom()[o..o + (DATA.1 - DATA.0) as usize]);
    s
}

/// A constant of the statement: a data-segment float (by its address) or
/// an immediate value.
#[derive(Clone, Copy)]
enum K {
    At(u32),
    V(f32),
}

/// A constant's value: from the state's data segment (the ROM's, or
/// distinct random ones), or the immediate.
fn k(mem: &State, c: K) -> f32 {
    match c {
        K::At(a) => f32::from_bits(word(mem, a)),
        K::V(v) => v,
    }
}

/// The statement, stat by stat: (field offset, multiplicative?, per-level
/// constants (c, or (a, b) for the multiplicative ones), lo for level 1,
/// lo for levels 2..5, hi for level 1, hi for levels 2..5).
struct Row {
    off: u32,
    mul: bool,
    c: [(K, K); 5],
    lo: (K, K),
    hi: (K, K),
}

fn rows() -> [Row; 7] {
    use K::{At, V};
    let one = V(1.0);
    let pair = |a: u32| (At(a), At(a + 4));
    let c = |a: K| (a, V(0.0));
    [
        Row { off: 0, mul: false, c: [c(At(0x800A_A3C8)), c(At(0x800A_A3D4)), c(At(0x800A_A3D8)), c(At(0x800A_A3DC)), c(V(0.25))], lo: (At(0x800A_A3CC), At(0x800A_A3D0)), hi: (one, one) },
        Row { off: 4, mul: false, c: [c(V(116.0)), c(V(232.0)), c(V(348.0)), c(V(464.0)), c(At(0x800A_A3E0))], lo: (V(50.0), V(50.0)), hi: (V(1000.0), V(1000.0)) },
        Row { off: 0xC, mul: true, c: [pair(0x800A_A3E4), pair(0x800A_A3F4), pair(0x800A_A3FC), pair(0x800A_A404), pair(0x800A_A40C)], lo: (At(0x800A_A3EC), At(0x800A_A3F0)), hi: (V(5.0), V(5.0)) },
        Row { off: 0x10, mul: false, c: [c(V(40.0)), c(V(80.0)), c(V(120.0)), c(V(160.0)), c(V(200.0))], lo: (V(450.0), V(450.0)), hi: (At(0x800A_A414), At(0x800A_A418)) },
        Row { off: 0x14, mul: true, c: [pair(0x800A_A41C), pair(0x800A_A424), pair(0x800A_A42C), pair(0x800A_A434), pair(0x800A_A43C)], lo: (one, one), hi: (V(1000.0), V(1000.0)) },
        Row { off: 0x24, mul: false, c: [c(At(0x800A_A444)), c(At(0x800A_A448)), c(At(0x800A_A44C)), c(At(0x800A_A450)), c(V(8.0))], lo: (one, one), hi: (V(20.0), V(20.0)) },
        Row { off: 0x2C, mul: false, c: [c(At(0x800A_A454)), c(At(0x800A_A458)), c(At(0x800A_A45C)), c(At(0x800A_A460)), c(At(0x800A_A464))], lo: (V(0.0), V(0.0)), hi: (one, one) },
    ]
}

fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}

/// The new field (bits), or None if an operand would be NaN.
fn model(mem: &State, stat: usize, level: usize, field: f32, x: f32) -> Option<u32> {
    let r = &rows()[stat];
    let (c0, c1) = (k(mem, r.c[level - 1].0), k(mem, r.c[level - 1].1));
    ok(x)?;
    ok(field)?;
    let mut v = if r.mul {
        let t = ok(1.0 - x)?;
        let p = ok(c0 * t)?;
        field * ok(c1 + p)?
    } else {
        field + ok(c0 * x)?
    };
    let (lo, hi) = if level == 1 { (k(mem, r.lo.0), k(mem, r.hi.0)) } else { (k(mem, r.lo.1), k(mem, r.hi.1)) };
    if hi < v {
        v = hi;
    }
    if v < lo {
        v = lo;
    }
    Some(v.to_bits())
}

fn float() -> BoxedStrategy<f32> {
    prop_oneof![
        4 => 0.0f32..1.0,
        2 => -2.0f32..2.0,
        2 => -1.0e3f32..1.0e3,
        1 => prop_oneof![Just(0.0f32), Just(-0.0), Just(1.0), Just(0.5), Just(f32::INFINITY), Just(f32::NEG_INFINITY)],
        1 => any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |v| !v.is_nan()),
    ]
    .boxed()
}

/// Field values in and around each stat's range, and ones on its bounds.
fn field(stat: usize) -> BoxedStrategy<f32> {
    let r = &rows()[stat];
    let rom = |c: K| match c {
        K::At(a) => rom_f(a),
        K::V(v) => v,
    };
    let (lo, hi) = (rom(r.lo.1), rom(r.hi.1));
    prop_oneof![
        3 => (lo - 0.5 * (hi - lo))..(hi + 0.5 * (hi - lo)),
        1 => prop::sample::select(vec![lo, hi, rom(r.lo.0), rom(r.hi.0), 0.0, -0.0]),
        1 => float(),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Every stat and level, plus levels and stats out of range (and
    /// 64-bit values: the stat's bound is `sltu`, the levels compare all 64
    /// bits). With `distinct`, each constant is the ROM's times a random
    /// factor in [0.8, 1.25], so addresses holding equal values in the ROM
    /// (e.g. the level-1 and later lower bounds of stat 0) are told apart
    /// while the values stay where the clamps act.
    #[test]
    fn func_800321F0((stat, fld) in (0usize..7).prop_flat_map(|s| (Just(s), field(s))), level in prop_oneof![1u64..=5, 0u64..8, Just(3 | 1 << 32)],
                     x in float(), others: [u32; 12], stat_junk in prop::option::weighted(0.05, prop_oneof![7u64..9, Just(1 << 32)]),
                     distinct in prop::option::weighted(0.5, prop::collection::vec(0.8f32..1.25, 40))) {
        let a1 = stat_junk.unwrap_or(stat as u64);
        let runs = a1 < 7 && (1..=5).contains(&level);
        let mut s = state(seed_of(stat, level, x, fld));
        if let Some(d) = &distinct {
            for (i, v) in d.iter().enumerate() {
                let a = DATA.0 + 4 * i as u32;
                s.rdram.mem().write_u32(a, (rom_f(a) * v).to_bits());
            }
        }
        let want = if runs { model(&s, stat, level as usize, fld, x) } else { Some(0) };
        prop_assume!(want.is_some());
        {
            let mut m = s.rdram.mem();
            for (i, w) in others.iter().enumerate() {
                m.write_u32(P + 4 * i as u32, *w);
            }
            m.write_u32(P + rows()[stat].off, fld.to_bits());
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(P), a1, level, sext(x.to_bits()));
        let after = run("func_800321F0", misc::func_800321F0, &s)?;
        prop_assert_eq!(word(&after, SP_AT + 0xC), x.to_bits());
        for i in 0..12u32 {
            let a = P + 4 * i;
            if runs && a == P + rows()[stat].off {
                prop_assert_eq!(word(&after, a), want.unwrap(), "stat {} level {} field {:e} x {:e}", stat, level, fld, x);
            } else {
                prop_assert_eq!(word(&after, a), word(&s, a));
            }
        }
    }
}

fn seed_of(stat: usize, level: u64, x: f32, fld: f32) -> u64 {
    (stat as u64) << 40 ^ level << 32 ^ u64::from(x.to_bits()) << 8 ^ u64::from(fld.to_bits())
}

/// Stat 6's lower bound is +0.0: a -0.0 result stays -0.0 (`-0 < 0` is
/// false), at every level.
#[test]
fn zero_bound_keeps_negative_zero() {
    for level in 1..=5u64 {
        let mut s = state(level);
        s.rdram.mem().write_u32(P + 0x2C, (-0.0f32).to_bits());
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(P), 6, level, sext((-0.0f32).to_bits()));
        let after = compare("func_800321F0", misc::func_800321F0, &s).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(word(&after, P + 0x2C), model(&s, 6, level as usize, -0.0, -0.0).unwrap());
        assert_eq!(word(&after, P + 0x2C), 0x8000_0000, "level {level}");
    }
}
