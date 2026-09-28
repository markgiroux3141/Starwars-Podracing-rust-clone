//! The float leaves at 0x800078B4..0x8000C724 (game::misc): a record-word
//! scale and clamp, float stores, a positive test by table, the 32-byte
//! records' initialiser and float setters, the 0x7C-byte entries' float
//! setters, and two clamped accumulators. Recompiled C vs Rust over float
//! edge values and ordinary full-mantissa ones, each checked against its
//! statement.
//!
//! NaN operands of arithmetic are outside these ports' domain (NAN_CHECK in
//! the oracle); NaN is tested where values are only compared or copied.

// Tests are named after the functions (func_800078B4), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::misc::{self, ENTRIES, RECORDS};
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

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

fn rom_word(vaddr: u32) -> u32 {
    let o = (vaddr - 0x8000_0400 + 0x1000) as usize;
    u32::from_be_bytes(baserom()[o..o + 4].try_into().unwrap())
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

fn any_bits() -> impl Strategy<Value = u32> {
    prop_oneof![
        float().prop_map(f32::to_bits),
        (1u32..0x0080_0000).prop_map(|p| 0x7F80_0000 | p),
        (1u32..0x0080_0000).prop_map(|p| 0xFF80_0000 | p),
        any::<u32>(),
    ]
}

/// A C cast to int (`cvttss2si`): `0x80000000` for NaN and out of range.
fn trunc(x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// Integers near the edges of the scale, and anything.
fn int() -> impl Strategy<Value = i32> {
    prop_oneof![-3i32..3, -100_000i32..100_000, Just(i32::MIN), Just(i32::MAX), (1i32..=24).prop_map(|b| 1 << (b - 1)), any::<i32>()]
}

// ---------------------------------------------------------------------------
// func_800078B4: record words scaled by K

const SCALED: u32 = 0x800D_2038;
const K_AT: u32 = 0x800A_81C4;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_800078B4(seed: u64, on: bool, rom_k: bool, k in float(), tags in prop::array::uniform8(prop_oneof![Just(0x4Eu32), any::<u32>()]),
                     words in prop::array::uniform8(int())) {
        let k = if rom_k { f32::from_bits(rom_word(K_AT)) } else { k };
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(0x8009_A2B8, u32::from(on));
            m.write_u32(K_AT, k.to_bits());
            for r in 0..8 {
                m.write_u32(SCALED + 0x20 * r + 4, tags[r as usize]);
                m.write_u32(SCALED + 0x20 * r + 0x18, words[r as usize] as u32);
            }
        }
        let after = run("func_800078B4", misc::func_800078B4, &s)?;
        for r in 0..8u32 {
            let mut w = words[r as usize];
            if on {
                if tags[r as usize] != 0x4E {
                    w = trunc(w as f32 * k);
                }
                if w < 0 {
                    w = 0;
                }
            }
            prop_assert_eq!(word(&after, SCALED + 0x20 * r + 0x18), w as u32, "record {}", r);
            prop_assert_eq!(word(&after, SCALED + 0x20 * r + 4), tags[r as usize]);
        }
    }

    // -----------------------------------------------------------------------
    // func_80008F58 / func_80009134

    #[test]
    fn func_80008F58(seed: u64, x in any_bits(), y in any_bits()) {
        let mut s = state(seed);
        s.ctx.fpr[12].set_u32l(x);
        s.ctx.fpr[14].set_u32l(y);
        let after = run("func_80008F58", misc::func_80008F58, &s)?;
        prop_assert_eq!((word(&after, 0x8009_AD08), word(&after, 0x8009_AD0C)), (x, y));
    }

    /// `k` 0 or 1 reads the table at 0x8009AD30 by `i`, any other `k`
    /// (compared as 64 bits: `1 | 1 << 32` is not 1) the one at 0x8009AD10.
    #[test]
    fn func_80009134(seed: u64, k in prop_oneof![0u64..2, 2u64..12, Just(1 | 1 << 32), Just(1 << 32)], i in 0u32..8, v in any_bits()) {
        let mut s = state(seed);
        let at = if k < 2 { 0x8009_AD30 + 4 * i } else { 0x8009_AD10u32.wrapping_add((k as u32) << 2) };
        s.rdram.mem().write_u32(at, v);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (k, u64::from(i));
        let after = run("func_80009134", misc::func_80009134, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(0.0 < f32::from_bits(v)));
    }
}

// ---------------------------------------------------------------------------
// The 32-byte records

fn id_arg(id: i32, junk: u32) -> u64 {
    (u64::from(junk) << 32) | u64::from((junk << 16) | (id as u16 as u32))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Ids around the bound 200 and negative ones (QUIRK: accepted), a
    /// count on either side of the id.
    #[test]
    fn func_8000A44C(seed: u64, id in prop_oneof![-3i32..3, 195i32..205, -3i32..205, -26_000i32..=i32::from(i16::MAX)],
                     count in prop_oneof![-5i32..210, any::<i32>()], p: u32, junk: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x8009_B770, count as u32);
        s.ctx.gpr[A0] = id_arg(id, junk);
        s.ctx.gpr[A1] = sext(p);
        let after = run("func_8000A44C", misc::func_8000A44C, &s)?;
        prop_assert_eq!(word(&after, SP_AT), s.ctx.gpr[A0] as u32);
        if id < 200 {
            let r = RECORDS.wrapping_add((id * 32) as u32);
            prop_assert_eq!(word(&after, 0x8009_B770), (if id < count { count } else { id + 1 }) as u32);
            prop_assert_eq!(word(&after, r), 0);
            prop_assert_eq!([word(&after, r + 8), word(&after, r + 0xC), word(&after, r + 0x10)], [0x3F80_0000, 0x3F80_0000, 0]);
            prop_assert_eq!([word(&after, r + 0x14), word(&after, r + 0x18), word(&after, r + 0x1C)], [1, 0xFFFF_FFFF, p]);
            prop_assert_eq!(word(&after, r + 4), word(&s, r + 4));
        } else {
            prop_assert_eq!(word(&after, 0x8009_B770), count as u32);
        }
    }

    #[test]
    fn func_8000AA04(seed: u64, id in prop_oneof![Just(-201i32), -3i32..10, any::<i16>().prop_map(i32::from)], x: u32, y: u32, junk: u32) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = id_arg(id, junk);
        (s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(x), sext(y));
        let after = run("func_8000AA04", misc::func_8000AA04, &s)?;
        let (hx, hy) = (x as u16 as i16, y as u16 as i16);
        prop_assert_eq!([word(&after, SP_AT), word(&after, SP_AT + 4), word(&after, SP_AT + 8)], [s.ctx.gpr[A0] as u32, x, y]);
        if id == -201 {
            prop_assert_eq!([word(&after, 0x8009_B784), word(&after, 0x8009_B788)], [f32::from(hx).to_bits(), f32::from(hy).to_bits()]);
        } else if id >= 0 {
            let r = RECORDS + 32 * id as u32;
            prop_assert_eq!(word(&after, r), (u32::from(hx as u16) << 16) | u32::from(hy as u16));
        }
    }

    #[test]
    fn func_8000AAC0(seed: u64, id in prop_oneof![-3i32..10, any::<i16>().prop_map(i32::from)], x in any_bits(), y in any_bits(), junk: [u32; 3]) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = id_arg(id, junk[0]);
        s.ctx.gpr[A1] = u64::from(junk[1]) << 32 | u64::from(x);
        s.ctx.gpr[A2] = u64::from(junk[2]) << 32 | u64::from(y);
        let after = run("func_8000AAC0", misc::func_8000AAC0, &s)?;
        if id >= 0 {
            let r = RECORDS + 32 * id as u32;
            prop_assert_eq!([word(&after, r + 8), word(&after, r + 0xC)], [x, y]);
        }
        let after = run("func_8000AAF8", misc::func_8000AAF8, &s)?;
        if id >= 0 {
            prop_assert_eq!(word(&after, RECORDS + 32 * id as u32 + 0x10), x);
        }
    }
}

// ---------------------------------------------------------------------------
// The 0x7C-byte entries

/// The entries' float setters, simulated word by word: the pointer is
/// re-read before each field, so a field stored over it moves the rest.
fn entries_sim(s: &State, id: i32, fields: &[(u32, i32)]) -> State {
    let mut sim = s.clone();
    let mut m = sim.rdram.mem();
    let v0 = (id as u32).wrapping_mul(0x7C);
    for &(val, off) in fields {
        let base = m.read_u32(ENTRIES);
        m.write_u32(base.wrapping_add(v0).wrapping_add(off as u32), val);
    }
    drop(m);
    sim
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// `overlap`: the pointer is set so that the second field lands on it,
    /// with that field's value a valid pointer (the rest follow it).
    #[test]
    fn func_8000AF4C(seed: u64, id in -2i32..10, args in prop::array::uniform6(any_bits()), overlap: bool, junk: u32) {
        let mut s = state(seed);
        let v0 = (id as u32).wrapping_mul(0x7C);
        let mut args = args;
        let base = if overlap {
            args[1] = 0x8032_0000;
            ENTRIES.wrapping_sub(v0).wrapping_sub(0x58)
        } else {
            0x8031_0000
        };
        {
            let mut m = s.rdram.mem();
            m.write_u32(ENTRIES, base);
            for (k, a) in args[3..].iter().enumerate() {
                m.write_u32(SP_AT + 0x10 + 4 * k as u32, *a);
            }
        }
        s.ctx.gpr[A0] = id_arg(id, junk);
        (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(args[0]), sext(args[1]), sext(args[2]));
        let fields: Vec<(u32, i32)> = args.iter().zip([0x54, 0x58, 0x5C, 0x60, 0x64, 0x68]).map(|(a, o)| (*a, o)).collect();
        let mut want = entries_sim(&s, id, &fields);
        want.rdram.mem().write_u32(SP_AT, s.ctx.gpr[A0] as u32);
        want.rdram.mem().write_u32(SP_AT + 0xC, args[2]);
        let after = run("func_8000AF4C", misc::func_8000AF4C, &s)?;
        prop_assert!(after.rdram.as_words() == want.rdram.as_words(), "memory differs from the model");
        let fields = &fields[..3];
        let mut s2 = s.clone();
        s2.rdram.mem().write_u32(ENTRIES, base.wrapping_sub(0x18));
        let mut want = entries_sim(&s2, id, &fields.iter().map(|&(v, o)| (v, o + 0x18)).collect::<Vec<_>>());
        want.rdram.mem().write_u32(SP_AT, s.ctx.gpr[A0] as u32);
        want.rdram.mem().write_u32(SP_AT + 0xC, args[2]);
        let after = run("func_8000AFD4", misc::func_8000AFD4, &s2)?;
        prop_assert!(after.rdram.as_words() == want.rdram.as_words(), "func_8000AFD4: memory differs from the model");
    }
}

// ---------------------------------------------------------------------------
// func_8000C6C8 / func_8000C724: clamped accumulators

const P: u32 = 0x8030_0000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// `lo`/`hi` any bits (only compared), sometimes equal to the sum: as
    /// the other zero when the sum is zero (`tie` 3 and 4 make it one), so
    /// a `<=` would store different bits.
    #[test]
    fn func_8000C6C8(seed: u64, old in float(), x in float(), y in float(), lo in any_bits(), hi in any_bits(), tie in 0u8..5) {
        let prod = x * y;
        prop_assume!(!prod.is_nan());
        let old = if tie >= 3 { -prod } else { old };
        let sum = old + prod;
        let opp = if sum == 0.0 { -sum } else { sum };
        let (lo, hi) = match tie {
            1 | 3 => (opp.to_bits(), hi),
            2 | 4 => (lo, opp.to_bits()),
            _ => (lo, hi),
        };
        let mut v = sum;
        if v < f32::from_bits(lo) {
            v = f32::from_bits(lo);
        }
        if f32::from_bits(hi) < v {
            v = f32::from_bits(hi);
        }
        let mut s = state(seed);
        s.rdram.mem().write_u32(P, old.to_bits());
        s.rdram.mem().write_u32(SP_AT + 0x10, hi);
        s.ctx.gpr[A0] = sext(P);
        (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(x.to_bits()), sext(y.to_bits()), sext(lo));
        let after = run("func_8000C6C8", misc::func_8000C6C8, &s)?;
        prop_assert_eq!(word(&after, P), v.to_bits());
        prop_assert_eq!(word(&after, SP_AT + 0xC), lo);
    }

    /// `lo` is the whole `a3` (64-bit compare); `hi` the stack word.
    #[test]
    fn func_8000C724(seed: u64, old in int(), x in float(), y in float(), lo in int(), hi in int(), lo_junk in prop::option::weighted(0.1, any::<u32>()), tie in 0u8..4) {
        let prod = x * y;
        prop_assume!(!prod.is_nan());
        let v = trunc(old as f32 + prod);
        let (lo, hi) = match tie {
            1 => (v, hi),
            2 => (lo, v),
            _ => (lo, hi),
        };
        let a3 = match lo_junk {
            Some(j) => u64::from(j) << 32 | u64::from(lo as u32),
            None => lo as i64 as u64,
        };
        let mut s = state(seed);
        s.rdram.mem().write_u32(P, old as u32);
        s.rdram.mem().write_u32(SP_AT + 0x10, hi as u32);
        s.ctx.gpr[A0] = sext(P);
        (s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(x.to_bits()), sext(y.to_bits()), a3);
        let after = run("func_8000C724", misc::func_8000C724, &s)?;
        let (mut stored, mut ret) = (v as u32, v as i64 as u64);
        if (v as i64) < a3 as i64 {
            stored = a3 as u32;
            ret = a3;
        }
        if i64::from(hi) < ret as i64 {
            stored = hi as u32;
        }
        prop_assert_eq!(word(&after, P), stored);
        prop_assert_eq!(after.ctx.gpr[V0], ret);
    }
}
