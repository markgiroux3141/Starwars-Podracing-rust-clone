//! The leaves at 0x80038DBC..0x80039984 (game::misc, game::render, game::save): a settings bit, six
//! optional halfwords, the Lights1/Lights2 fillers (with a QUIRK in the
//! second light), spills, the CRC-32 table and copies of save records.
//! Recompiled C vs Rust on random register files and memory, each checked
//! against its statement.

// Tests are named after the functions (func_80038DBC), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::{misc, render, save};
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

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

const O: u32 = 0x8030_0000;
const SP0: u32 = 0x800A_2800;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed, O, 0x100);
    s.randomise_memory(seed ^ 5, 0x800A_3D40, 0x2C0);
    s.ctx.gpr[SP] = sext(SP0);
    s
}

/// Three colour halfwords at `a` (the low byte of each counts) and three
/// direction halfwords at `a + 8`.
const RGB: u32 = O;
const RGB2: u32 = O + 0x10;
const DIR: u32 = O + 0x20;

fn colour(s: &State, a: u32) -> [u8; 3] {
    [byte(s, a + 1), byte(s, a + 3), byte(s, a + 5)]
}

fn direction(s: &State) -> [u8; 3] {
    [0, 1, 2].map(|k| 0u8.wrapping_sub(half(s, DIR + 2 * k) as u8))
}

/// The light bytes a filler writes at `base`: colour at +0 and +4 (or the
/// given offsets), direction at +0x10.
fn light_bytes(s: &State, base: u32, rgb: u32, at: u32) -> Vec<(u32, u8)> {
    let c = colour(s, rgb);
    (0..3).flat_map(|k| [(base + at + k, c[k as usize]), (base + at + 4 + k, c[k as usize])]).collect()
}

fn slot(i: i64) -> u32 {
    0x800A_3DC8 + 0x28 * i as u32
}

fn slot_index() -> impl Strategy<Value = i64> {
    prop_oneof![4 => 0i64..12, 1 => Just(12i64), 1 => Just(-1i64), 1 => Just(i64::MIN)]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Bit 6 of [0x800D697C]: set for 0, cleared otherwise.
    #[test]
    fn func_80038DBC(seed: u64, off in prop_oneof![Just(0u64), any::<u64>()], w: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x800D_697C, w);
        s.ctx.gpr[A0] = off;
        let after = run("func_80038DBC", misc::func_80038DBC, &s)?;
        prop_assert_eq!(word(&after, 0x800D_697C), if off == 0 { w | 0x40 } else { w & !0x40 });
    }

    /// Nonnegative arguments stored as halfwords.
    #[test]
    fn func_80038DF8(seed: u64, args in [any::<i64>(), any::<i64>(), any::<i64>(), any::<i64>()], e: i32, f: i32, neg: [bool; 6], zero in 0usize..8) {
        let mut s = state(seed);
        let mut vals: Vec<i64> = args.iter().chain(&[i64::from(e), i64::from(f)]).zip(neg).map(|(&v, n)| if n { -(v.abs().max(1)) } else { v.abs() }).collect();
        if let Some(v) = vals.get_mut(zero) {
            *v = 0; // 0 counts as nonnegative
        }
        for r in 0..4 {
            s.ctx.gpr[A0 + r] = vals[r] as u64;
        }
        s.rdram.mem().write_u32(SP0 + 0x10, vals[4] as u32);
        s.rdram.mem().write_u32(SP0 + 0x14, vals[5] as u32);
        let after = run("func_80038DF8", misc::func_80038DF8, &s)?;
        for (k, a) in [0x800A_3D4C, 0x800A_3D50, 0x800A_3D44, 0x800A_3D46, 0x800A_3D48, 0x800A_3D4A].into_iter().enumerate() {
            let want = if vals[k] >= 0 { vals[k] as u16 } else { half(&s, a) };
            prop_assert_eq!(half(&after, a), want, "argument {}", k);
        }
    }

    /// Lights1 at 0x800A3DB0.
    #[test]
    fn func_80038E58(seed: u64) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(RGB), sext(RGB2), sext(DIR));
        let after = run("func_80038E58", render::func_80038E58, &s)?;
        let base = 0x800A_3DB0;
        let d = direction(&s);
        for (a, b) in light_bytes(&s, base, RGB, 0).into_iter().chain(light_bytes(&s, base, RGB2, 8)).chain((0..3).map(|k| (base + 0x10 + k, d[k as usize]))) {
            prop_assert_eq!(byte(&after, a), b, "{:#x}", a);
        }
    }

    /// Slot i's ambient and first light, for 0 <= i < 12.
    #[test]
    fn func_80038ED0(seed: u64, i in slot_index()) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (i as u64, sext(RGB), sext(RGB2), sext(DIR));
        let after = run("func_80038ED0", render::func_80038ED0, &s)?;
        for j in 0..12 {
            let base = slot(j);
            let mut want: Vec<(u32, u8)> = (0..0x28).map(|k| (base + k, byte(&s, base + k))).collect();
            if j == i {
                let d = direction(&s);
                let writes = light_bytes(&s, base, RGB, 0).into_iter().chain(light_bytes(&s, base, RGB2, 8)).chain((0..3).map(|k| (base + 0x10 + k, d[k as usize])));
                for (a, b) in writes {
                    want[(a - base) as usize].1 = b;
                }
            }
            for (a, b) in want {
                prop_assert_eq!(byte(&after, a), b, "slot {} {:#x}", j, a);
            }
        }
    }

    /// Copy Lights1 into slot i, flag 1.
    #[test]
    fn func_80038F68(seed: u64, i in slot_index()) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = i as u64;
        let after = run("func_80038F68", render::func_80038F68, &s)?;
        for j in 0..12 {
            for k in 0..10 {
                let want = if j == i && k < 6 { word(&s, 0x800A_3DB0 + 4 * k) } else { word(&s, slot(j) + 4 * k) };
                prop_assert_eq!(word(&after, slot(j) + 4 * k), want);
            }
            let f = 0x800A_3FA8 + 4 * j as u32;
            prop_assert_eq!(word(&after, f), if j == i { 1 } else { word(&s, f) });
        }
    }

    /// The second light: flag 1 without, 2 with, and the misplaced red.
    #[test]
    fn func_80038FE8(seed: u64, i in slot_index(), on in prop_oneof![Just(0u64), Just(0x1_0000_0000u64), any::<u64>()]) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (i as u64, on, sext(RGB), sext(DIR));
        let after = run("func_80038FE8", render::func_80038FE8, &s)?;
        for j in 0..12 {
            let base = slot(j);
            let f = 0x800A_3FA8 + 4 * j as u32;
            let mut want: Vec<u8> = (0..0x28).map(|k| byte(&s, base + k)).collect();
            if j == i {
                prop_assert_eq!(word(&after, f), if on == 0 { 1 } else { 2 });
                if on != 0 {
                    let [r, g, b] = colour(&s, RGB);
                    let d = direction(&s);
                    // QUIRK: red at +0x19, then green over it; +0x18 untouched.
                    want[0x19] = g;
                    want[0x1A] = b;
                    want[0x1C] = r;
                    want[0x1D] = g;
                    want[0x1E] = b;
                    want[0x20..0x23].copy_from_slice(&d);
                }
            } else {
                prop_assert_eq!(word(&after, f), word(&s, f));
            }
            for (k, w) in want.iter().enumerate() {
                prop_assert_eq!(byte(&after, base + k as u32), *w, "slot {} +{:#x}", j, k);
            }
        }
    }

    /// Spills.
    #[test]
    fn func_80039090(seed: u64, args: [u64; 4]) {
        let mut s = state(seed);
        s.ctx.gpr[A0..=A3].copy_from_slice(&args);
        let after = run("func_80039090", misc::func_80039090, &s)?;
        for k in 0..4 {
            prop_assert_eq!(word(&after, SP0 + 4 * k as u32), args[k] as u32);
        }
    }

    /// Copies of 0x2C-byte records between the save block and 0x80113E60.
    #[test]
    fn records(seed: u64, back: bool, a in 0u64..4, b in 0u64..4) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 9, 0x8011_3680, 0x400);
        s.randomise_memory(seed ^ 10, 0x8011_3E60, 0xB0);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (a, b);
        let (name, port): (&str, RecompFn) = if back { ("func_80039984", save::func_80039984) } else { ("func_80039914", save::func_80039914) };
        let after = run(name, port, &s)?;
        let save = |r: u64| 0x8011_3694 + 0x2C * r as u32;
        let cur = |r: u64| 0x8011_3E60 + 0x2C * r as u32;
        let (src, dst) = if back { (cur(b), save(a)) } else { (save(b), cur(a)) };
        for k in 0..11 {
            prop_assert_eq!(word(&after, dst + 4 * k), word(&s, src + 4 * k));
        }
        prop_assert_eq!(word(&after, dst + 0x2C), word(&s, dst + 0x2C));
    }
}

#[test]
fn empties() {
    for (name, port) in [("func_800390A4", misc::func_800390A4 as RecompFn), ("func_800390AC", misc::func_800390AC)] {
        let s = state(2);
        let after = compare(name, port, &s).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(after.ctx.gpr, s.ctx.gpr);
    }
}

/// The MSB-first CRC-32 table (polynomial 0x04C11DB7).
#[test]
fn func_800390C0() {
    let mut s = state(3);
    s.randomise_memory(4, 0x8011_4060, 0x420);
    let after = compare("func_800390C0", save::func_800390C0, &s).unwrap_or_else(|d| panic!("{d}"));
    for v in 0..256u32 {
        let mut c = v << 24;
        for _ in 0..8 {
            c = if c & 0x8000_0000 != 0 { (c << 1) ^ 0x04C1_1DB7 } else { c << 1 };
        }
        assert_eq!(word(&after, 0x8011_4070 + 4 * v), c, "entry {v}");
    }
    assert_eq!(word(&after, 0x8011_4470), word(&s, 0x8011_4470));
    assert_eq!(word(&after, 0x8011_406C), word(&s, 0x8011_406C));
}

/// The 0x3F0-byte save block copy.
#[test]
fn func_8003960C() {
    let mut s = state(6);
    s.randomise_memory(7, 0x8011_3680, 0x800);
    let after = compare("func_8003960C", save::func_8003960C, &s).unwrap_or_else(|d| panic!("{d}"));
    for k in 0..0xFC {
        assert_eq!(word(&after, 0x8011_3A70 + 4 * k), word(&s, 0x8011_3680 + 4 * k));
    }
    assert_eq!(word(&after, 0x8011_3E60), word(&s, 0x8011_3E60));
}
