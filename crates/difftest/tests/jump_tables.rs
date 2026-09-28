//! Depth-0 functions with jump tables (game::misc). Recompiled C vs Rust
//! on random register files and memory, each checked against its statement.
//! Every case of each table is reached. In these functions the index is
//! bounded (`sltiu`) just before the table, so the C's `default:
//! switch_error` can't be reached and has no test here.

// Tests are named after the functions (func_80008F6C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc;
use game::recomp::reg::*;
use proptest::prelude::*;

fn run(name: &str, port: game::recomp::RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

/// The signed halfword at `a` (big-endian: offset 0 is the word's high half).
fn half(s: &State, a: u32) -> i16 {
    let w = s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize];
    (if a & 2 == 0 { w >> 16 } else { w }) as u16 as i16
}

// ---------------------------------------------------------------------------
// func_80008F6C: handle from (kind, a1, table index)

/// Table lengths by kind, from the bounds in the code.
const LEN: [u64; 8] = [0x33, 0x26, 0x39, 5, 0x68, 0xA9, 0x69, 0xA8];

/// Where kind `k`'s halfword table starts: back to back from 0x8009A6F0,
/// each 4-aligned (derived from the lengths, not from the port's offsets).
fn table(k: usize) -> u32 {
    let mut a = 0x8009_A6F0u32;
    for len in &LEN[..k] {
        a = (a + 2 * *len as u32 + 3) & !3;
    }
    a
}

fn handle_state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    // All eight tables, entries of both signs.
    s.randomise_memory(seed ^ 0x5A5A, 0x8009_A6F0, 0x8009_AC00 - 0x8009_A6F0);
    s
}

/// The statement: -1 for `index == -1`, `kind >= 8`, kinds 0/1 with `a1`
/// outside [0, 0x17), or `index` outside (0, len); else the handle.
fn handle(s: &State, kind: u64, a1: u64, index: u64) -> u64 {
    if index == u64::MAX || kind >= 8 {
        return u64::MAX;
    }
    let k = kind as usize;
    if k < 2 && !(0..0x17).contains(&(a1 as i64)) {
        return u64::MAX;
    }
    if (index as i64) <= 0 || index >= LEN[k] {
        return u64::MAX;
    }
    let entry = half(s, table(k) + 2 * index as u32) as i64 as u64;
    sext((kind as u32) << 24) | sext((a1 as u32) << 16) | entry | 0x8000
}

/// Indices near the kind's bounds, or anything.
fn index_for(kind: u64, mode: u8, raw: u64) -> u64 {
    let len = LEN.get(kind as usize).copied().unwrap_or(4);
    match mode {
        0 => len - 1 + raw % 3, // len-1, len, len+1
        1 => raw % 3,           // 0, 1, 2
        2 => u64::MAX,
        3 => raw | 1 << 63,     // negative
        4 => 1 + raw % (len - 1), // in range
        5 => (raw % len) | 1 << 32, // junk above a small value
        _ => raw,
    }
}

fn a1s() -> impl Strategy<Value = u64> {
    prop_oneof![
        (-2i64..0x19).prop_map(|v| v as u64),
        Just(0x1_0000_0005u64),
        Just(0x8000_0000u64),
        any::<u64>(),
    ]
}

fn kinds() -> impl Strategy<Value = u64> {
    prop_oneof![4 => 0u64..8, 1 => 8u64..10, 1 => Just(u64::MAX), 1 => Just(0x1_0000_0002u64), 1 => any::<u64>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn func_80008F6C(seed: u64, kind in kinds(), a1 in a1s(), mode in 0u8..8, raw: u64) {
        let index = index_for(kind, mode, raw);
        let mut s = handle_state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (kind, a1, index);
        let after = run("func_80008F6C", misc::func_80008F6C, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], handle(&s, kind, a1, index), "kind {:#x} a1 {:#x} index {:#x}", kind, a1, index);
    }
}

/// Every case of the table, each with a valid handle, both ends of the
/// index range and the first index past it; and both signs of entry.
#[test]
fn func_80008F6C_every_case() {
    for kind in 0..8u64 {
        let len = LEN[kind as usize];
        for (index, entry) in [(1, 0x1234u16), (len - 1, 0xFEDC), (len, 0x1111)] {
            let mut s = handle_state(kind);
            s.rdram.mem().write_u16(table(kind as usize) + 2 * index as u32, entry);
            (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (kind, 0x16, index);
            let after = compare("func_80008F6C", misc::func_80008F6C, &s).unwrap_or_else(|d| panic!("{d}"));
            let want = if index == len {
                u64::MAX
            } else {
                sext((kind as u32) << 24 | 0x16 << 16) | entry as i16 as i64 as u64 | 0x8000
            };
            assert_eq!(after.ctx.gpr[V0], want, "kind {kind} index {index}");
        }
    }
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

/// Non-NaN floats: signed zeros, subnormals, ones, halves, ordinary values
/// with full mantissas in the game's range, huge, infinities, and any other
/// non-NaN bit pattern.
fn float() -> impl Strategy<Value = f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::from_bits(1)),
        Just(-f32::from_bits(0x007F_FFFF)),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        -1.0e4f32..1.0e4f32,
        -1.0e4f32..1.0e4f32,
        -1.0e4f32..1.0e4f32,
        Just(-3.0e38f32),
        Just(f32::MAX),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
}

// ---------------------------------------------------------------------------
// func_80029298: offset [+8] of forty records by kind

const RECORDS: u32 = 0x800A_4C00;

/// Kinds: every case (-1..=4), just outside, and any halfword.
fn record_kind() -> impl Strategy<Value = i16> {
    prop_oneof![4 => -1i16..5, 1 => Just(-2i16), 1 => Just(5i16), 1 => any::<i16>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80029298(seed: u64, kinds in proptest::collection::vec(record_kind(), 40), x in float()) {
        let mut s = State::new();
        s.randomise_registers(seed);
        s.randomise_memory(seed ^ 0x77, RECORDS, 40 * 0x20);
        for (k, &kind) in kinds.iter().enumerate() {
            s.rdram.mem().write_u16(RECORDS + 0x20 * k as u32 + 0x18, kind as u16);
        }
        s.ctx.fpr[12].set_fl(x);
        let after = run("func_80029298", misc::func_80029298, &s)?;
        for (k, &kind) in kinds.iter().enumerate() {
            let r = RECORDS + 0x20 * k as u32;
            let (w8, w14) = match kind {
                -1 | 4 => (Some(-145.0f32 + x), None),
                0 => (Some(-60.0 + x), None),
                1 => (Some(-157.0 + x), Some(-157.0 + x)),
                2 => (Some(-157.0 + x), None),
                _ => (None, None),
            };
            for off in (0..0x20).step_by(4) {
                let want = match off {
                    8 => w8.map(f32::to_bits),
                    0x14 => w14.map(f32::to_bits),
                    _ => None,
                }
                .unwrap_or(word(&s, r + off));
                prop_assert_eq!(word(&after, r + off), want, "record {} (kind {}) +{:#x}", k, kind, off);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// func_8002D598: track_name

/// The 25 names, found in the ROM image independently of the port's table:
/// consecutive NUL-terminated strings from the first, each 4-aligned.
fn track_names() -> Vec<u32> {
    let rom = difftest::rom::baserom();
    let at = |v: u32| (v - 0x8000_0400 + 0x1000) as usize;
    let mut out = vec![0x800A_98E4u32];
    while out.len() < 25 {
        let a = *out.last().unwrap();
        let len = rom[at(a)..].iter().position(|&b| b == 0).unwrap() as u32;
        out.push((a + len + 1 + 3) & !3);
    }
    for &a in &out {
        assert_eq!(&rom[at(a)..at(a) + 2], b"~~", "{a:#x}");
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8002D598(seed: u64, track in prop_oneof![4 => 0u64..25, 1 => 25u64..27, 1 => Just(u64::MAX), 1 => Just(0x1_0000_0003u64), 1 => any::<u64>()]) {
        let mut s = State::new();
        s.randomise_registers(seed);
        s.ctx.gpr[A0] = track;
        let after = run("func_8002D598", misc::func_8002D598, &s)?;
        let want = if track < 25 { sext(track_names()[track as usize]) } else { 0 };
        prop_assert_eq!(after.ctx.gpr[V0], want);
    }
}

#[test]
fn func_8002D598_every_track() {
    let names = track_names();
    for track in 0..26u64 {
        let mut s = State::new();
        s.randomise_registers(track);
        s.ctx.gpr[A0] = track;
        let after = compare("func_8002D598", misc::func_8002D598, &s).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(after.ctx.gpr[V0], names.get(track as usize).map_or(0, |&a| sext(a)), "track {track}");
    }
}

// ---------------------------------------------------------------------------
// func_80063344: float pair by type and index

const OBJ: u32 = 0x8030_0000;
const X: u32 = OBJ + 0x100;
const Y: u32 = OBJ + 0x104;

/// Where each type's pair comes from (independent of the port's table):
/// halfword table and scale address, or a pair of constant words, or zero.
enum Source {
    Table(u32, u32),
    Words(u32),
    Zero,
}

fn source(ty: u32) -> Source {
    match ty {
        1 => Source::Table(0x800A_3090, 0x800A_D3EC),
        2 => Source::Table(0x800A_3104, 0x800A_D3F0),
        3 => Source::Table(0x800A_313C, 0x800A_D3F4),
        4 => Source::Words(0x800A_D3F8),
        5 => Source::Words(0x800A_D400),
        7 => Source::Words(0x800A_D408),
        8 => Source::Table(0x800A_31B0, 0x800A_D41C),
        9 => Source::Table(0x800A_31C4, 0x800A_D420),
        10 => Source::Table(0x800A_317C, 0x800A_D410),
        11 => Source::Table(0x800A_319C, 0x800A_D418),
        12 => Source::Table(0x800A_318C, 0x800A_D414),
        _ => Source::Zero,
    }
}

fn pair_state(seed: u64, ty: u32, i: u32, scales: &[f32; 9]) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed ^ 0x1234, OBJ, 0x200);
    // The tables (entries of both signs, and the word before the first) and
    // the constants; then scales that aren't NaN.
    s.randomise_memory(seed ^ 0x4321, 0x800A_3000, 0x200);
    s.randomise_memory(seed ^ 0x5678, 0x800A_D3E0, 0x50);
    for (k, &sc) in scales.iter().enumerate() {
        let a = [0x800A_D3EC, 0x800A_D3F0, 0x800A_D3F4, 0x800A_D410, 0x800A_D414, 0x800A_D418, 0x800A_D41C, 0x800A_D420, 0][k];
        if a != 0 {
            s.rdram.mem().write_f32(a, sc);
        }
    }
    s.rdram.mem().write_u32(OBJ + 8, ty);
    s.rdram.mem().write_u32(OBJ + 0x88, i);
    (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(OBJ), sext(X), sext(Y));
    s
}

/// The statement: `None` if nothing is stored, else the two words.
fn pair(s: &State, ty: u32, i: u32) -> Option<(u32, u32)> {
    if i == u32::MAX && [1, 2, 3, 6].contains(&ty) {
        return None;
    }
    Some(match source(ty) {
        Source::Zero => (0, 0),
        Source::Words(a) => (word(s, a), word(s, a + 4)),
        Source::Table(t, sc) => {
            let scale = f32::from_bits(word(s, sc));
            let e = t.wrapping_add(i.wrapping_mul(4));
            ((half(s, e) as f32 * scale).to_bits(), (half(s, e + 2) as f32 * scale).to_bits())
        }
    })
}

fn pair_type() -> impl Strategy<Value = u32> {
    prop_oneof![6 => 0u32..14, 1 => Just(u32::MAX), 1 => Just(0x8000_0001u32), 1 => any::<u32>()]
}

fn pair_index() -> impl Strategy<Value = u32> {
    prop_oneof![4 => 0u32..8, 2 => Just(u32::MAX), 1 => 8u32..0x40]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn func_80063344(seed: u64, ty in pair_type(), i in pair_index(), scales in [float(), float(), float(), float(), float(), float(), float(), float(), float()]) {
        let s = pair_state(seed, ty, i, &scales);
        let after = run("func_80063344", misc::func_80063344, &s)?;
        let want = pair(&s, ty, i).unwrap_or((word(&s, X), word(&s, Y)));
        prop_assert_eq!((word(&after, X), word(&after, Y)), want, "type {} i {:#x}", ty, i);
    }
}

/// Every case, with an index, and with -1.
#[test]
fn func_80063344_every_case() {
    for ty in 0..14 {
        for i in [0, 3, u32::MAX] {
            let s = pair_state(u64::from(ty), ty, i, &[1.5, -2.25, 0.1, 3.0, -7.0, 1e-3, 12.5, -0.5, 0.0]);
            let after = compare("func_80063344", misc::func_80063344, &s).unwrap_or_else(|d| panic!("{d}"));
            let want = pair(&s, ty, i).unwrap_or((word(&s, X), word(&s, Y)));
            assert_eq!((word(&after, X), word(&after, Y)), want, "type {ty} i {i:#x}");
        }
    }
}

/// QUIRK: the index is read again after `x` is stored. With `x` at
/// `[obj + 0x88]` and a subnormal scale, `x`'s bits (3 * 2^-149 = bits 3)
/// become the index `y` is taken from.
#[test]
fn func_80063344_rereads_the_index() {
    let mut s = pair_state(7, 1, 0, &[f32::from_bits(1), 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0]);
    s.rdram.mem().write_u16(0x800A_3090, 3); // tbl[0].0
    s.rdram.mem().write_u16(0x800A_3090 + 4 * 3 + 2, 5); // tbl[3].1
    s.ctx.gpr[A1] = sext(OBJ + 0x88);
    let after = compare("func_80063344", misc::func_80063344, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(word(&after, OBJ + 0x88), 3);
    assert_eq!(word(&after, Y), 5); // 5 * 2^-149
}
