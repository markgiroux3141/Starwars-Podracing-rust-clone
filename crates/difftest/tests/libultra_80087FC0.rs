//! Pure libultra at 0x80087FC0..0x80095AA0 (game::libultra): the audio
//! heap (`alHeapDBAlloc`, `alHeapInit`), list links (`alLink`,
//! `alUnlink`), the audio parameter free list, a time-to-samples
//! conversion, a record initialiser, a halfword setter and four empty
//! functions. Recompiled C vs Rust; the models state each function's
//! stores and result, and whole RDRAM is compared.

// Tests are named after the functions (func_80087FC0), capitals included.
#![allow(non_snake_case)]

use difftest::rom::baserom;
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

/// A big-endian double at `a` (high word first).
fn wd(s: &mut State, a: u32, v: f64) {
    let b = v.to_bits();
    wr(s, a, (b >> 32) as u32);
    wr(s, a + 4, b as u32);
}

fn rd(s: &State, a: u32) -> f64 {
    f64::from_bits(u64::from(word(s, a)) << 32 | u64::from(word(s, a + 4)))
}

/// The game's data segment (`0x80098000..0x800AE8B0`) as the ROM image has
/// it (the time conversion reads its divisor from there).
fn load_data(s: &mut State) {
    let rom = baserom();
    let mut m = s.rdram.mem();
    for va in (0x8009_8000u32..0x800A_E8B0).step_by(4) {
        let o = (va - 0x8000_0400 + 0x1000) as usize;
        m.write_u32(va, u32::from_be_bytes(rom[o..o + 4].try_into().unwrap()));
    }
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

fn wordish() -> BoxedStrategy<u32> {
    prop_oneof![
        3 => 0u32..0x1000,
        1 => Just(0u32),
        1 => Just(u32::MAX),
        1 => Just(0x7FFF_FFFFu32),
        1 => Just(0x8000_0000u32),
        2 => any::<u32>(),
    ]
    .boxed()
}

// ---- the heap ----

const HEAP: u32 = 0x8030_0000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Fits, exactly fills, overflows, wraps; zero and odd sizes, products
    /// past 32 bits.
    #[test]
    fn func_80087FC0(seed: u64, base in prop_oneof![Just(0x8031_0000u32), wordish()], used in 0u32..0x400, len in prop_oneof![0u32..0x400, wordish()],
                     num in prop_oneof![0u32..20, wordish()], size in prop_oneof![0u32..64, wordish()], exact: bool) {
        let mut s = state(seed);
        let cur = base.wrapping_add(used);
        let n = num.wrapping_mul(size).wrapping_add(15) & !15;
        let len = if exact { used.wrapping_add(n) } else { len };
        wr(&mut s, HEAP, base);
        wr(&mut s, HEAP + 4, cur);
        wr(&mut s, HEAP + 8, len);
        wr(&mut s, SP_AT + 0x10, size);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (s.ctx.gpr[A0], s.ctx.gpr[A1], sext(HEAP), sext(num));
        let mut want = s.clone();
        wr(&mut want, SP_AT, s.ctx.gpr[A0] as u32);
        wr(&mut want, SP_AT + 4, s.ctx.gpr[A1] as u32);
        let fits = cur.wrapping_add(n) <= base.wrapping_add(len);
        if fits {
            wr(&mut want, HEAP + 4, cur.wrapping_add(n));
        }
        let after = run("func_80087FC0", libultra::func_80087FC0, &s)?;
        same_memory(&after, &want)?;
        prop_assert_eq!(after.ctx.gpr[V0], if fits { sext(cur) } else { 0 });
    }

    /// Bases aligned and not, any length.
    #[test]
    fn func_80088110(seed: u64, base in prop_oneof![(0u32..0x40).prop_map(|k| 0x8031_0000 + k), wordish()], len in wordish()) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, HEAP, 0x10);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(HEAP), sext(base), sext(len));
        let mut want = s.clone();
        let b = if base & 15 == 0 { base } else { base.wrapping_add(16 - (base & 15)) };
        for (k, v) in [b, b, len, 0].into_iter().enumerate() {
            wr(&mut want, HEAP + 4 * k as u32, v);
        }
        let after = run("func_80088110", libultra::func_80088110, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- links ----

const NODES: u32 = 0x8030_1000;

/// A node or null: one of four nodes at `NODES + 0x10 k`, so links may
/// point at the node itself or at each other.
fn link() -> BoxedStrategy<u32> {
    prop_oneof![1 => Just(0u32), 3 => (0u32..4).prop_map(|k| NODES + 0x10 * k)].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Every combination of null and set neighbours, including links back
    /// to the node itself.
    #[test]
    fn func_80088020(seed: u64, ln in 0u32..4, next in link(), prev in link()) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, NODES, 0x40);
        let ln = NODES + 0x10 * ln;
        wr(&mut s, ln, next);
        wr(&mut s, ln + 4, prev);
        s.ctx.gpr[A0] = sext(ln);
        let mut w = s.clone();
        let next = word(&w, ln);
        if next != 0 {
            let p = word(&w, ln + 4);
            wr(&mut w, next + 4, p);
        }
        let prev = word(&w, ln + 4);
        if prev != 0 {
            let n = word(&w, ln);
            wr(&mut w, prev, n);
        }
        let after = run("func_80088020", libultra::func_80088020, &s)?;
        same_memory(&after, &w)?;
    }

    /// `to` with and without a successor; `ln` equal to `to` or to its
    /// successor, or overlapping it (`to = ln + 4`: storing `ln->prev`
    /// changes `to->next`, which is re-read).
    #[test]
    fn func_80088050(seed: u64, ln in 0u32..4, to in 0u32..4, next in link(), overlap in prop::bool::weighted(0.2)) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, NODES, 0x50);
        let (ln, to) = (NODES + 0x10 * ln, NODES + 0x10 * to);
        let to = if overlap { ln + 4 } else { to };
        wr(&mut s, to, next);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(ln), sext(to));
        let mut w = s.clone();
        let old = word(&w, to);
        wr(&mut w, ln + 4, to);
        wr(&mut w, ln, old);
        let n = word(&w, to);
        if n != 0 {
            wr(&mut w, n + 4, ln);
        }
        wr(&mut w, to, ln);
        let after = run("func_80088050", libultra::func_80088050, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- the parameter free list ----

const GLOBALS: u32 = 0x800A_6990;
const G: u32 = 0x8030_2000;
const PARAMS: u32 = 0x8030_2100;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Pushing onto an empty or a full list, the record already the head.
    #[test]
    fn func_800884E8(seed: u64, head in prop_oneof![Just(0u32), (0u32..4).prop_map(|k| PARAMS + 0x20 * k)], p in 0u32..4) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, PARAMS, 0x80);
        wr(&mut s, GLOBALS, G);
        wr(&mut s, G + 0x2C, head);
        let p = PARAMS + 0x20 * p;
        s.ctx.gpr[A0] = sext(p);
        let mut want = s.clone();
        wr(&mut want, p, head);
        wr(&mut want, G + 0x2C, p);
        let after = run("func_800884E8", libultra::func_800884E8, &s)?;
        same_memory(&after, &want)?;
    }

    /// Taking from an empty list and from one whose head links on.
    #[test]
    fn func_80088500(seed: u64, head in prop_oneof![Just(0u32), (0u32..4).prop_map(|k| PARAMS + 0x20 * k)]) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, PARAMS, 0x80);
        wr(&mut s, GLOBALS, G);
        wr(&mut s, G + 0x2C, head);
        let mut want = s.clone();
        if head != 0 {
            let next = word(&s, head);
            wr(&mut want, G + 0x2C, next);
            wr(&mut want, head, 0);
        }
        let after = run("func_80088500", libultra::func_80088500, &s)?;
        same_memory(&after, &want)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(head));
    }
}

// ---- the time conversion ----

const RATE_AT: u32 = 0x8030_3000;
const DIVISOR: u32 = 0x800A_DD80;

fn samples(t: u32, rate: u32, k: f64) -> Option<u32> {
    let p = (t as i32 as f32) * (rate as i32 as f32);
    let q = f64::from(p) / k;
    if q.is_nan() {
        return None;
    }
    let n = (q + 0.5) as f32;
    Some(trunc(n) & !15)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Times and rates of both signs and every size (the f32 products
    /// round, the conversion saturates), halfway sums, the ROM's divisor or
    /// another.
    #[test]
    fn func_800883F8(seed: u64, t in prop_oneof![0u32..100_000, (-100_000i32..0).prop_map(|v| v as u32), any::<u32>()],
                     rate in prop_oneof![Just(32_000u32), Just(44_100u32), Just(22_050u32), 0u32..100_000, any::<u32>()],
                     k in prop_oneof![Just(None), prop_oneof![Just(1.0f64), Just(2.0f64), Just(-1.0f64), Just(0.0f64), 1.0f64..1.0e7].prop_map(Some)]) {
        let mut s = state(seed);
        load_data(&mut s);
        if let Some(k) = k {
            wd(&mut s, DIVISOR, k);
        }
        wr(&mut s, RATE_AT + 0x44, rate);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(RATE_AT), sext(t));
        let v0 = samples(t, rate, rd(&s, DIVISOR));
        prop_assume!(v0.is_some());
        let after = run("func_800883F8", libultra::func_800883F8, &s)?;
        same_memory(&after, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(v0.unwrap()));
    }
}

/// Sums just below and exactly at a multiple of 16, where truncating and
/// rounding the sum differ after the `& ~15` (15.7 truncates to 15 and
/// stays below 16), and negative ones (truncation toward zero, then the
/// mask rounding down).
#[test]
fn func_800883F8_cases() {
    for (t, rate, k) in [(152u32, 1u32, 10.0f64), (31, 1, 2.0), (161, 1, 10.0), (-33i32 as u32, 1, 2.0), (-152i32 as u32, 1, 10.0), (319, 1, 10.0)] {
        let mut s = state(0x4A1F + u64::from(t));
        load_data(&mut s);
        wd(&mut s, DIVISOR, k);
        wr(&mut s, RATE_AT + 0x44, rate);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(RATE_AT), sext(t));
        let after = compare("func_800883F8", libultra::func_800883F8, &s).unwrap();
        assert_eq!(after.ctx.gpr[V0], sext(samples(t, rate, k).unwrap()));
    }
}

// ---- setters and empty functions ----

const REC: u32 = 0x8030_4000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Any 64-bit value (only its low halfword, sign-extended, is stored).
    #[test]
    fn func_80088B00(seed: u64, v: u64) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(REC), v);
        let mut want = s.clone();
        wr(&mut want, SP_AT + 4, v as u32);
        wr(&mut want, REC + 0x3C, v as u16 as i16 as i32 as u32);
        let after = run("func_80088B00", libultra::func_80088B00, &s)?;
        same_memory(&after, &want)?;
    }

    /// Any three words.
    #[test]
    fn func_80095AA0(seed: u64, a: u32, b: u32, c: u32) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 0x5EED, REC, 0x20);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(REC), sext(a), sext(b), sext(c));
        let mut want = s.clone();
        wr(&mut want, REC, 0);
        wr(&mut want, REC + 4, a);
        wr(&mut want, REC + 8, b);
        wh(&mut want, REC + 0xC, 0);
        wh(&mut want, REC + 0xE, 0);
        wr(&mut want, REC + 0x10, c);
        let after = run("func_80095AA0", libultra::func_80095AA0, &s)?;
        same_memory(&after, &want)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Nothing changes, whatever the state.
    #[test]
    fn func_800883F0_empty(seed: u64) {
        let s = state(seed);
        for (name, port) in [
            ("func_800883F0", libultra::func_800883F0 as RecompFn),
            ("func_80088530", libultra::func_80088530),
            ("func_80088BEC", libultra::func_80088BEC),
            ("func_80088BF4", libultra::func_80088BF4),
        ] {
            let after = run(name, port, &s)?;
            same_memory(&after, &s)?;
            prop_assert_eq!(after.ctx.gpr, s.ctx.gpr);
        }
    }
}
