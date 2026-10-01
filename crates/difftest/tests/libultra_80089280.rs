//! Pure libultra at 0x80089280..0x800895E0 (game::libultra): libaudio's
//! bank patching and an offset table's patching, the sound player's
//! records (allocate, current sound, free, a halfword setter) and three
//! empty functions. Recompiled C vs Rust; the models state each function's
//! stores and result, and whole RDRAM is compared.

// Tests are named after the functions (func_80089290), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::libultra;
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

fn wb(s: &mut State, a: u32, v: u8) {
    s.rdram.mem().write_u8(a, v);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn func_80089280(seed: u64) {
        let s = state(seed);
        let after = run("func_80089280", libultra::func_80089280, &s)?;
        same_memory(&after, &s)?;
    }

    #[test]
    fn func_80089288(seed: u64) {
        let s = state(seed);
        let after = run("func_80089288", libultra::func_80089288, &s)?;
        same_memory(&after, &s)?;
    }

    #[test]
    fn func_8008937C(seed: u64) {
        let s = state(seed);
        let after = run("func_8008937C", libultra::func_8008937C, &s)?;
        same_memory(&after, &s)?;
    }
}

// ---- func_80089290 / func_80089488: patching ----

const BASE: u32 = 0x8030_0000;

/// A bank at `BASE` with `n` sound offsets; sounds at `BASE + 0x100 + 0x40 j`
/// (`j` from `picks`, so they can be shared), each pointing at a wave at
/// `BASE + 0x800 + 0x40 j'`; the flag bytes, types and words random.
#[derive(Clone, Debug)]
struct Bank {
    done: u8,
    picks: Vec<u32>,
    waves: Vec<u32>,
    sflags: Vec<u8>,
    wflags: Vec<u8>,
    types: Vec<u8>,
    words: Vec<u32>,
}

fn bank() -> BoxedStrategy<Bank> {
    (
        prop_oneof![4 => Just(0u8), 1 => any::<u8>()],
        prop::collection::vec(0u32..4, 0..5),
        prop::collection::vec(0u32..3, 4),
        prop::collection::vec(prop_oneof![3 => Just(0u8), 1 => any::<u8>()], 4),
        prop::collection::vec(prop_oneof![3 => Just(0u8), 1 => any::<u8>()], 3),
        prop::collection::vec(prop_oneof![2 => Just(0u8), 2 => Just(1u8), 1 => any::<u8>()], 3),
        prop::collection::vec(prop_oneof![Just(0u32), any::<u32>()], 40),
    )
        .prop_map(|(done, picks, waves, sflags, wflags, types, words)| Bank { done, picks, waves, sflags, wflags, types, words })
        .boxed()
}

fn sound(j: u32) -> u32 {
    0x100 + 0x40 * j
}

fn wave(j: u32) -> u32 {
    0x800 + 0x40 * j
}

fn put_bank(s: &mut State, b: &Bank) {
    s.randomise_memory(0xBA4C, BASE, 0x1000);
    wb(s, BASE + 3, b.done);
    wh(s, BASE + 0xE, b.picks.len() as u16);
    for (i, &j) in b.picks.iter().enumerate() {
        wr(s, BASE + 0x10 + 4 * i as u32, sound(j));
    }
    for j in 0..4u32 {
        let so = BASE + sound(j);
        wb(s, so + 0xE, b.sflags[j as usize]);
        wr(s, so, b.words[j as usize]);
        wr(s, so + 4, b.words[4 + j as usize]);
        wr(s, so + 8, wave(b.waves[j as usize]));
    }
    for j in 0..3u32 {
        let wa = BASE + wave(j);
        wb(s, wa + 9, b.wflags[j as usize]);
        wb(s, wa + 8, b.types[j as usize]);
        wr(s, wa, b.words[8 + j as usize]);
        wr(s, wa + 0xC, b.words[12 + j as usize]);
        wr(s, wa + 0x10, b.words[16 + j as usize]);
    }
}

fn patch_model(s: &State, base: u32, tbl: u32) -> State {
    let mut w = s.clone();
    let b = BASE;
    if byte(&w, b + 3) != 0 {
        return w;
    }
    wb(&mut w, b + 3, 1);
    let mut i = 0u32;
    while (i as i32) < i32::from(half(&w, b + 0xE) as i16) {
        let so = word(&w, b + 0x10 + 4 * i).wrapping_add(base);
        wr(&mut w, b + 0x10 + 4 * i, so);
        if byte(&w, so + 0xE) == 0 {
            let v0 = word(&w, so);
            wb(&mut w, so + 0xE, 1);
            wr(&mut w, so, v0.wrapping_add(base));
            let (v4, v8) = (word(&w, so + 4), word(&w, so + 8));
            wr(&mut w, so + 4, v4.wrapping_add(base));
            wr(&mut w, so + 8, v8.wrapping_add(base));
            let wa = v8.wrapping_add(base);
            if byte(&w, wa + 9) == 0 {
                let x = word(&w, wa);
                let ty = byte(&w, wa + 8);
                wb(&mut w, wa + 9, 1);
                wr(&mut w, wa, x.wrapping_add(tbl));
                match ty {
                    0 => {
                        let (x10, xc) = (word(&w, wa + 0x10), word(&w, wa + 0xC));
                        wr(&mut w, wa + 0x10, x10.wrapping_add(base));
                        if xc != 0 {
                            wr(&mut w, wa + 0xC, xc.wrapping_add(base));
                        }
                    }
                    1 => {
                        let xc = word(&w, wa + 0xC);
                        if xc != 0 {
                            wr(&mut w, wa + 0xC, xc.wrapping_add(base));
                        }
                    }
                    _ => {}
                }
            }
        }
        i += 1;
    }
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Banks already patched or not, sounds and waves shared or flagged,
    /// every wave type (and others), loop words zero or not.
    #[test]
    fn func_80089290(seed: u64, b in bank(), tbl: u32) {
        let mut s = state(seed);
        put_bank(&mut s, &b);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A3]) = (sext(BASE), sext(BASE), sext(tbl));
        let want = patch_model(&s, BASE, tbl);
        let after = run("func_80089290", libultra::func_80089290, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80089488(seed: u64, n in prop_oneof![0i16..8, Just(-1i16)], words in prop::collection::vec(any::<u32>(), 8), base: u32) {
        let mut s = state(seed);
        let t = 0x8030_2000u32;
        wh(&mut s, t + 2, n as u16);
        for (i, v) in words.iter().enumerate() {
            wr(&mut s, t + 4 + 8 * i as u32, *v);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(t), sext(base));
        let mut w = s.clone();
        for i in 0..n.max(0) as u32 {
            let v = word(&w, t + 4 + 8 * i).wrapping_add(base);
            wr(&mut w, t + 4 + 8 * i, v);
        }
        let after = run("func_80089488", libultra::func_80089488, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- func_800894D0 / func_80089570 / func_80089590 / func_800895E0 ----

const PLAYER: u32 = 0x8030_4000;
const RECS: u32 = 0x8030_5000;
const SND: u32 = 0x8030_6000;

fn put_player(s: &mut State, count: i32, used: &[u32], cur: u32) {
    s.randomise_memory(0x5ACD, RECS, 48 * 8);
    wr(s, PLAYER + 0x40, RECS);
    wr(s, PLAYER + 0x44, count as u32);
    wr(s, PLAYER + 0x3C, cur);
    for (k, u) in used.iter().enumerate() {
        wr(s, RECS + 48 * k as u32 + 0x1C, *u);
    }
}

fn used() -> BoxedStrategy<Vec<u32>> {
    prop::collection::vec(prop_oneof![Just(0u32), any::<u32>()], 8).boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Counts up to 8 (non-positive too), records free or not, volumes of
    /// every byte value (signed division by 0x7F).
    #[test]
    fn func_800894D0(seed: u64, count in prop_oneof![0i32..9, Just(-1i32)], u in used(), vol: u8) {
        let mut s = state(seed);
        put_player(&mut s, count, &u, 0);
        wb(&mut s, SND + 0xD, vol);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(PLAYER), sext(SND));
        let mut w = s.clone();
        let free = (0..count.max(0) as usize).find(|&k| u[k] == 0);
        let v0 = match free {
            Some(k) => {
                let r = RECS + 48 * k as u32;
                wr(&mut w, r + 0x1C, SND);
                wh(&mut w, r + 0x20, 5);
                wr(&mut w, r + 0x28, 0);
                wb(&mut w, r + 0x2E, 0x40);
                wb(&mut w, r + 0x2F, 0);
                wr(&mut w, r + 0x24, 1.0f32.to_bits());
                wh(&mut w, r + 0x2C, ((i32::from(vol) * 0x7FFF) / 0x7F) as u16);
                k as u64
            }
            None => u64::MAX,
        };
        let after = run("func_800894D0", libultra::func_800894D0, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
    }

    #[test]
    fn func_80089570(seed: u64, cur in 0u32..8, x: u32) {
        let mut s = state(seed);
        put_player(&mut s, 8, &[0; 8], cur);
        wr(&mut s, RECS + 48 * cur + 0x28, x);
        s.ctx.gpr[A0] = sext(PLAYER);
        let after = run("func_80089570", libultra::func_80089570, &s)?;
        same_memory(&after, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(x));
    }

    /// The record busy (`+0x28` nonzero) or not, the current index equal to
    /// `k` or not (and `-1`), `a1` with any high bits (its low halfword
    /// counts).
    #[test]
    fn func_80089590(seed: u64, k in 0u32..8, busy in prop_oneof![Just(0u32), any::<u32>()], cur in prop_oneof![0u32..8, Just(u32::MAX)], hi: u32) {
        let mut s = state(seed);
        put_player(&mut s, 8, &[1; 8], cur);
        wr(&mut s, RECS + 48 * k + 0x28, busy);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(PLAYER), u64::from(hi) << 16 | u64::from(k));
        let mut w = s.clone();
        wr(&mut w, SP_AT + 4, (u64::from(hi) << 16 | u64::from(k)) as u32);
        if busy == 0 {
            wr(&mut w, RECS + 48 * k + 0x1C, 0);
            if cur == k {
                wr(&mut w, PLAYER + 0x3C, u32::MAX);
            }
        }
        let after = run("func_80089590", libultra::func_80089590, &s)?;
        same_memory(&after, &w)?;
    }

    #[test]
    fn func_800895E0(seed: u64, k in 0u32..8, v: u32, hi: u32) {
        let mut s = state(seed);
        put_player(&mut s, 8, &[1; 8], 0);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(PLAYER), u64::from(hi) << 16 | u64::from(k), sext(v));
        let mut w = s.clone();
        wr(&mut w, SP_AT + 4, (u64::from(hi) << 16 | u64::from(k)) as u32);
        wr(&mut w, SP_AT + 8, v);
        wh(&mut w, RECS + 48 * k + 0x20, (v & 0xFF) as u16);
        let after = run("func_800895E0", libultra::func_800895E0, &s)?;
        same_memory(&after, &w)?;
    }
}
