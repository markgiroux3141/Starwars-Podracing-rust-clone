//! The spline loader func_80030174 (spline_load): recompiled C vs
//! game::loader on real data from baserom.z64 (read at test time; nothing
//! ROM-derived is stored in the repository), checked against the format
//! as `assets::Spline` states it. Patched ROMs exist only in memory.
//!
//! rom_read_small is a test double (difftest::rom); heap_cursor and
//! heap_set_cursor are the recompiled C. Heap state: [`difftest::world`].

use assets::{Spline, SPLINE_POINT_SIZE};
use difftest::rom::{baserom, install_rom_doubles, install_rom_image, Image, ROM_READ_SMALL};
use difftest::world::{blocks, bytes, sext, world, Heap, HEAP_8MB};
use difftest::{compare, State};
use game::heap::CURSORS;
use game::loader::{self, SPLINE_BLOCK};
use game::recomp::reg::*;
use proptest::prelude::*;

const COUNT: usize = 91;
/// Where the test puts `out`.
const OUT: u32 = 0x8030_0000;

fn spline_load(s: &State) -> State {
    compare("func_80030174", loader::func_80030174, s).unwrap_or_else(|d| panic!("{d}"))
}

fn raw(i: usize) -> &'static [u8] {
    blocks().splines.entry(i)[0].unwrap()
}

fn before(seed: u64, heap: Heap, index: u64) -> State {
    let mut s = world(seed, heap);
    s.ctx.gpr[A0] = index;
    s.ctx.gpr[A1] = sext(OUT);
    s
}

/// A load of spline `i` at `cursor`, stated from the format: the ROM bytes
/// at the cursor with `+0xC` replaced by the first point's address, `*out`
/// = cursor, the cursor moved past the spline, nothing written above it.
fn check(before: &State, after: &mut State, i: usize, cursor: u32) {
    let raw = raw(i);
    let mut want = raw.to_vec();
    want[0xC..0x10].copy_from_slice(&(cursor + 0x10).to_be_bytes());
    assert_eq!(bytes(after, cursor, raw.len()), want, "spline {i}: bytes");
    assert_eq!(after.rdram.mem().read_u32(OUT), cursor, "spline {i}: *out");
    let end = cursor + raw.len() as u32;
    assert_eq!(after.rdram.mem().read_u32(CURSORS), end, "spline {i}: cursor");
    let (lo, hi) = (((end - 0x8000_0000) / 4) as usize, ((end + 0x1000 - 0x8000_0000) / 4) as usize);
    assert!(after.rdram.as_words()[lo..hi] == before.rdram.as_words()[lo..hi], "spline {i}: wrote above the cursor");
    // And the loaded spline parses to the same points as the ROM's.
    let loaded = Spline::parse(&bytes(after, cursor, raw.len())).unwrap();
    assert_eq!(loaded.points, Spline::parse(raw).unwrap().points, "spline {i}");
}

/// Every spline from a fresh heap: three reads (count, offsets, spline).
#[test]
fn every_spline() {
    let _rom = install_rom_doubles();
    assert_eq!(blocks().splines.count, COUNT);
    for i in 0..COUNT {
        let b = before(i as u64, HEAP_8MB, i as u64);
        let mut after = spline_load(&b);
        check(&b, &mut after, i, HEAP_8MB.cursor);
        assert_eq!(after.calls.len(), 3, "spline {i}: reads");
        assert!(after.calls.iter().all(|c| c.name == ROM_READ_SMALL));
        assert_eq!(raw(i).len(), 0x10 + SPLINE_POINT_SIZE * Spline::parse(raw(i)).unwrap().points.len());
    }
}

/// All splines one after another on one heap: each lands at the previous
/// one's end, unaligned beyond 4 (sizes are 0x10 + 0x54n).
#[test]
fn sequential_loads() {
    let _rom = install_rom_doubles();
    let mut s = world(5, HEAP_8MB);
    s.ctx.gpr[A1] = sext(OUT);
    let mut cursor = HEAP_8MB.cursor;
    for i in (0..COUNT).rev() {
        let mut b = State { calls: Vec::new(), ..s };
        b.ctx.gpr[A0] = i as u64;
        b.ctx.gpr[A1] = sext(OUT);
        let mut after = spline_load(&b);
        check(&b, &mut after, i, cursor);
        cursor += raw(i).len() as u32;
        s = after;
    }
}

/// Out of range: `*out = 0`, nothing loaded, only the count read. The
/// index is spilled to the stack and reloaded (`sw`/`lw`) before the signed
/// compares, so it is in effect its sign-extended low word: a zero-extended
/// `0x8000_0000` is negative.
#[test]
fn out_of_range() {
    let _rom = install_rom_doubles();
    for index in [COUNT as u64, 92, u64::MAX, 0xFFFF_FFFF_8000_0000, 0x8000_0000, 0x1_0000_005B] {
        let b = before(7, HEAP_8MB, index);
        let mut after = spline_load(&b);
        assert_eq!(after.rdram.mem().read_u32(OUT), 0, "{index:#x}");
        assert_eq!(after.rdram.mem().read_u32(CURSORS), HEAP_8MB.cursor, "{index:#x}");
        assert_eq!(after.calls.len(), 1, "{index:#x}");
    }
}

/// Junk above the index's low word is dropped (the stack round trip).
#[test]
fn index_is_the_low_word() {
    let _rom = install_rom_doubles();
    for (index, i) in [(0x1_0000_0000u64, 0usize), (0xDEAD_0000_0000_0005, 5), (0x7FFF_FFFF_0000_005A, 90)] {
        let b = before(10, HEAP_8MB, index);
        let mut after = spline_load(&b);
        check(&b, &mut after, i, HEAP_8MB.cursor);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Any spline at any word-aligned cursor, any registers.
    #[test]
    fn any_cursor(i in 0usize..COUNT, cursor in 0x8019_0000u32..0x802F_0000, seed: u64) {
        let _rom = install_rom_doubles();
        let cursor = cursor & !3;
        let b = before(seed, Heap { cursor, ..HEAP_8MB }, i as u64);
        let mut after = compare("func_80030174", loader::func_80030174, &b).map_err(|d| TestCaseError::fail(d.to_string()))?;
        check(&b, &mut after, i, cursor);
    }
}

/// No space check: a spline loads past the heap end.
#[test]
fn no_space_check() {
    let _rom = install_rom_doubles();
    let heap = Heap { cursor: HEAP_8MB.end - 0x10, ..HEAP_8MB };
    let b = before(8, heap, 1);
    let mut after = spline_load(&b);
    check(&b, &mut after, 1, heap.cursor);
    assert!(after.rdram.mem().read_u32(CURSORS) > HEAP_8MB.end);
}

/// A patched point count only drives the pointless loop, whose `v1`
/// heap_set_cursor's callee overwrites afterwards: the load is the same
/// whatever the count says (the loader doesn't check it against the size).
/// C vs Rust covers the registers.
#[test]
fn patched_point_counts() {
    for count in [0u32, 1, 0x8000_0000, 0xFFFF_FFFF, 100_000] {
        let base = SPLINE_BLOCK as usize;
        let rom = baserom();
        let first = base + u32::from_be_bytes(rom[base + 4..base + 8].try_into().unwrap()) as usize;
        let _rom = install_rom_image(Image::from(rom).patch(first + 4, &count.to_be_bytes()));
        let b = before(9, HEAP_8MB, 0);
        let mut after = spline_load(&b);
        let got = bytes(&mut after, HEAP_8MB.cursor + 4, 4);
        assert_eq!(got, count.to_be_bytes());
        assert_eq!(after.rdram.mem().read_u32(CURSORS), HEAP_8MB.cursor + raw(0).len() as u32, "count {count:#x}");
        assert_eq!(after.rdram.mem().read_u32(OUT), HEAP_8MB.cursor);
    }
}
