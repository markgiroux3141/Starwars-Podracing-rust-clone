//! The leaves at 0x8002D968..0x8002FE94 (game::misc): byte and flag tests,
//! argument spills, setters, two memory fills, a heap-level lookup and a
//! power-of-two round-up. Recompiled C vs Rust on random register files
//! and memory, each checked against its statement.

// Tests are named after the functions (func_8002D968), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

fn byte(s: &State, a: u32) -> u8 {
    (word(s, a & !3) >> (24 - 8 * (a & 3))) as u8
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const O: u32 = 0x8030_0000;
const SP0: u32 = 0x800A_2800;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed, O, 0x200);
    s.ctx.gpr[SP] = sext(SP0);
    s
}

/// Register values: small, byte edges, sign edges, junk above the low
/// word, anything.
fn arg() -> impl Strategy<Value = u64> {
    prop_oneof![
        0u64..8,
        Just(0x7F),
        Just(0x80),
        Just(0xFF),
        Just(0x100),
        Just(0x1_0000_0002),
        Just(u64::MAX),
        (-8i64..8).prop_map(|v| v as u64),
        any::<u32>().prop_map(sext),
        any::<u64>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// With bit 14 of the flag word, whether the first three bytes agree.
    #[test]
    fn func_8002D968(seed: u64, flag: u32, same in 0usize..4, bytes: [u8; 3], diff in 1u8..=255) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x8009_B7D8, flag);
        let (a, b) = (O + 0x21, O + 0x42);
        for k in 0..3 {
            s.rdram.mem().write_u8(a + k as u32, bytes[k]);
            s.rdram.mem().write_u8(b + k as u32, if k < same { bytes[k] } else { bytes[k] ^ diff });
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(a), sext(b));
        let after = run("func_8002D968", misc::func_8002D968, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(flag & 0x4000 != 0 && same == 3));
    }

    /// 4 if (s8) a < 3 and (u8) b < 6, else 3; both spilled.
    #[test]
    fn func_8002D9D0(seed: u64, a in arg(), b in arg()) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (a, b);
        let after = run("func_8002D9D0", misc::func_8002D9D0, &s)?;
        let want = if (a as u8 as i8) < 3 && (b as u8) < 6 { 4 } else { 3 };
        prop_assert_eq!(after.ctx.gpr[V0], want);
        prop_assert_eq!((word(&after, SP0), word(&after, SP0 + 4)), (a as u32, b as u32));
    }

    /// Bit (u8) bit of table[(s8) i], the table chosen by [p + 0x6C].
    #[test]
    fn func_8002DAD0(seed: u64, flag in prop_oneof![Just(0u8), any::<u8>()], i in arg(), bit in arg()) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 1, 0x8011_3600, 0x100);
        s.randomise_memory(seed ^ 2, 0x8011_3DE0, 0x100);
        s.rdram.mem().write_u8(O + 0x6C, flag);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(O), i, bit);
        let after = run("func_8002DAD0", misc::func_8002DAD0, &s)?;
        let table = if flag != 0 { 0x8011_3E68u32 } else { 0x8011_368C };
        let v = byte(&s, table.wrapping_add(i as u8 as i8 as u32));
        prop_assert_eq!(after.ctx.gpr[V0], u64::from((u32::from(v) >> (bit as u8 & 31)) & 1));
        prop_assert_eq!((word(&after, SP0 + 4), word(&after, SP0 + 8)), (i as u32, bit as u32));
    }

    /// With [p + 0x6C]: three halfwords 0x3FFF and one 0xFF; else bit 5.
    #[test]
    fn func_8002DC7C(seed: u64, flag in prop_oneof![Just(0u8), any::<u8>()], good in [any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()], junk: [u16; 4], w: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u8(O + 0x6C, flag);
        let want_h = [0x3FFFu16, 0x3FFF, 0x3FFF, 0xFF];
        for k in 0..4 {
            let v = if good[k] { want_h[k] } else if junk[k] == want_h[k] { junk[k] ^ 0x8000 } else { junk[k] };
            s.rdram.mem().write_u16(0x8011_3E6C + 2 * k as u32, v);
        }
        s.rdram.mem().write_u32(0x8011_3688, w);
        s.ctx.gpr[A0] = sext(O);
        let after = run("func_8002DC7C", misc::func_8002DC7C, &s)?;
        let want = if flag != 0 { good.iter().all(|&g| g) } else { w & 0x20 != 0 };
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(want));
    }

    /// The empty functions that spill their arguments.
    #[test]
    fn spills(seed: u64, k in 0usize..3, args in [arg(), arg(), arg(), arg()]) {
        let (name, port, n, at): (&str, RecompFn, usize, u32) = [
            ("func_8002DFB0", misc::func_8002DFB0 as RecompFn, 2, SP0),
            ("func_8002FE80", misc::func_8002FE80, 4, SP0),
            ("func_8002F9E0", misc::func_8002F9E0, 4, SP0),
        ][k];
        let mut s = state(seed);
        s.ctx.gpr[A0..=A3].copy_from_slice(&args);
        let after = run(name, port, &s)?;
        for r in 0..n {
            prop_assert_eq!(word(&after, at + 4 * r as u32), args[r] as u32);
        }
        prop_assert_eq!(word(&after, at + 4 * n as u32), word(&s, at + 4 * n as u32));
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP0));
    }

    /// Setters and getters of fixed words.
    #[test]
    fn fixed_words(seed: u64, k in 0usize..4, v: u32) {
        let mut s = state(seed);
        for a in [0x800A_268C, 0x800A_2690, 0x800A_26F4, 0x800A_26F8, 0x800A_26FC] {
            s.rdram.mem().write_u32(a, v.rotate_left(a));
        }
        let (name, port): (&str, RecompFn) = [
            ("func_8002E028", misc::func_8002E028 as RecompFn),
            ("func_8002E0A8", misc::func_8002E0A8),
            ("func_8002F054", misc::func_8002F054),
            ("func_8002F1CC", misc::func_8002F1CC),
        ][k];
        let after = run(name, port, &s)?;
        match k {
            0 => prop_assert_eq!(word(&after, 0x800A_268C), 0),
            1 => prop_assert_eq!(word(&after, 0x800A_2690), 0),
            2 => prop_assert_eq!(after.ctx.gpr[V0], sext(word(&s, 0x800A_26F4))),
            _ => {
                prop_assert_eq!(word(&after, 0x800A_26F8), 0);
                prop_assert_eq!(byte(&after, 0x800A_26FC), 1);
                prop_assert_eq!(word(&after, 0x800A_26FC) & 0xFF_FFFF, word(&s, 0x800A_26FC) & 0xFF_FFFF);
            }
        }
    }

    /// [0x800D7498 + 4 i].
    #[test]
    fn func_8002E8D4(seed: u64, i in prop_oneof![0u64..64, (-64i64..0).prop_map(|v| v as u64), Just(0x1_0000_0003u64), (0u64..0x10_0000)]) {
        let mut s = state(seed);
        s.randomise_memory(seed, 0x800D_7300, 0x400);
        s.ctx.gpr[A0] = i;
        let after = run("func_8002E8D4", misc::func_8002E8D4, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(word(&s, 0x800D_7498u32.wrapping_add((i as u32) << 2))));
    }

    /// ra = [sp + 0x1C], sp += 0x20.
    #[test]
    fn func_8002F740(seed: u64, w: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(SP0 + 0x1C, w);
        let after = run("func_8002F740", misc::func_8002F740, &s)?;
        prop_assert_eq!(after.ctx.gpr[RA], sext(w));
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP0 + 0x20));
    }

    /// Fill from 0x8014D7E0 to osMemSize | 0x80000000 with 0xBBCCBBCC.
    #[test]
    fn func_8002F480(seed: u64, size in prop_oneof![
        Just(0x40_0000u32), Just(0x80_0000), Just(0x14_D7E0), Just(0x14_D7E1), Just(0x14_D7E4), Just(0x14_D7E5),
        Just(0x10_0000), Just(0x8040_0000), Just(0), 0x14_D7D0u32..0x15_0000, 0x14_D7E0u32..0x80_0000,
    ]) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x8000_0318, size);
        let after = run("func_8002F480", misc::func_8002F480, &s)?;
        let (start, end) = (0x8014_D7E0u32, size | 0x8000_0000);
        for a in [start - 4, start, start + 4, (end & !3).wrapping_sub(4), end & !3, (end + 3) & !3] {
            if !(0x8000_0000..0x8080_0000).contains(&a) {
                continue;
            }
            let want = if a >= start && a < end { 0xBBCC_BBCC } else { word(&s, a) };
            prop_assert_eq!(word(&after, a), want, "{:#x}", a);
        }
    }

    /// The level whose region holds p.
    #[test]
    fn func_8002FB4C(seed: u64, level in -2i32..12, steps in proptest::collection::vec(0u32..0x1000, 12), base in 0x8010_0000u32..0x8040_0000, pick in 0usize..14, delta in -2i64..3) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x800A_2868, level as u32);
        // Rising cursors (the heap's invariant), some equal; p near one.
        let mut c = base;
        let mut cursors = Vec::new();
        for (k, st) in steps.iter().enumerate() {
            c += st & !3;
            cursors.push(c);
            s.rdram.mem().write_u32(0x800D_9DD8 + 4 * k as u32, c);
        }
        let p = sext(cursors.get(pick).copied().unwrap_or(base)).wrapping_add(delta as u64);
        s.ctx.gpr[A0] = p;
        let after = run("func_8002FB4C", misc::func_8002FB4C, &s)?;
        let top = level.wrapping_sub(1);
        let want = if top <= 0 {
            i64::from(top) + 1
        } else {
            (1..=top as usize).rev().find(|&l| p >= sext(cursors[l])).map_or(1, |l| l as i64 + 1)
        };
        prop_assert_eq!(after.ctx.gpr[V0], want as u64);
    }

    /// Next power of two, at least 16, with the QUIRKs above 2^30 and for
    /// negative values.
    #[test]
    fn func_8002FE94(seed: u64, n in prop_oneof![
        0u64..40,
        (0u32..31).prop_flat_map(|b| (-1i64..=1).prop_map(move |d| (1i64 << b).wrapping_add(d) as u64)),
        Just(0x4000_0000u64), Just(0x4000_0001), Just(0x7FFF_FFFF), Just(0x8000_0000),
        (-40i64..0).prop_map(|v| v as u64),
        any::<u32>().prop_map(sext),
        any::<u64>(),
    ]) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = n;
        let after = run("func_8002FE94", misc::func_8002FE94, &s)?;
        let low = n & 0x7FFF_FFFF;
        let top = if low == 0 { 0 } else { 1u64 << (63 - low.leading_zeros()) };
        let b = if top == 1 { 0 } else { top }; // bit 0 is found after v1 has shifted out
        let mut v = if (b as i64) < (n as i64) { sext((b as u32) << 1) } else { b };
        if (v as i64) < 16 {
            v = 16;
        }
        prop_assert_eq!(after.ctx.gpr[V0], v);
    }
}

#[test]
fn func_8002F1E4() {
    let s = state(3);
    let after = compare("func_8002F1E4", misc::func_8002F1E4, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(after.ctx.gpr, s.ctx.gpr);
}

/// 0xDDEEDDEE over [0x800AE8B0, 0x80149C60), nothing else.
#[test]
fn func_8002F440() {
    let mut s = state(4);
    s.randomise_memory(5, 0x800A_E800, 0x9_C000);
    let after = compare("func_8002F440", misc::func_8002F440, &s).unwrap_or_else(|d| panic!("{d}"));
    for a in (0x800A_E800u32..0x8014_A000).step_by(4) {
        let want = if (0x800A_E8B0..0x8014_9C60).contains(&a) { 0xDDEE_DDEE } else { word(&s, a) };
        assert_eq!(word(&after, a), want, "{a:#x}");
    }
}
