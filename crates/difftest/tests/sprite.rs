//! The sprite loader func_8002FF38 (sprite_load) and its two wrappers,
//! func_80030130 and func_80030154: recompiled C vs game::loader, on real
//! data from baserom.z64 (read at test time; nothing ROM-derived is stored
//! in the repository). Patched ROMs exist only in memory.
//!
//! rom_read_small is a test double (difftest::rom). heap_cursor and
//! heap_set_cursor (and heap_check below it) are the recompiled C, and so is
//! sprite_load when a wrapper is under test.
//!
//! Heap state: [`difftest::world`].

use assets::Sprite;
use difftest::rom::{baserom, install_rom_doubles, rom_read_small, ROM_READ_SMALL};
use difftest::world::{align, blocks, bytes, sext, words, world, Heap, HEAP_8MB, STACK};
use difftest::{compare, State};
use game::heap::{CURSORS, HEAP_END};
use game::loader::{self, OUT_OF_HEAP};
use game::recomp::reg::*;
use oracle::doubles;
use proptest::prelude::*;

const COUNT: usize = 173;

fn sprite_load(s: &State) -> State {
    compare("func_8002FF38", loader::func_8002FF38, s).unwrap_or_else(|d| panic!("{d}"))
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

/// The raw bytes of sprite `i` in baserom.z64.
fn raw(i: usize) -> &'static [u8] {
    blocks().sprites.entry(i)[0].unwrap()
}

/// Where a load of sprite `i` at `cursor` puts everything, stated from the
/// format (NOTES.md, "Sprites") through `assets::Sprite`, not from the code:
/// the header at the cursor, the page table right after it, then the palette
/// (if any) and each page, each at the next 16-byte boundary. Returns the
/// words the header and table should hold, the (address, bytes) of each
/// part, and the new cursor.
struct Expected {
    header: Vec<u32>,
    parts: Vec<(u32, &'static [u8])>,
    end: u32,
}

fn expect(i: usize, cursor: u32) -> Expected {
    let raw = raw(i);
    let s = Sprite::parse(raw).unwrap();
    let n = s.pages.len();
    let table = cursor + 0x14;
    // Header and table as in the ROM, then the pointers patched in.
    let mut header: Vec<u32> = (0..(0x14 + 8 * n) / 4).map(|k| be32(raw, 4 * k)).collect();
    header[0x10 / 4] = table;
    let mut parts = Vec::new();
    let mut at = table + 8 * n as u32;
    if let Some(p) = s.palette {
        at = align(at, 16);
        header[8 / 4] = at;
        parts.push((at, p));
        at += p.len() as u32;
    }
    for (k, page) in s.pages.iter().enumerate() {
        at = align(at, 16);
        header[(0x14 + 8 * k + 4) / 4] = at;
        parts.push((at, page.texels));
        at += page.texels.len() as u32;
    }
    Expected { header, parts, end: align(at, 16) }
}

/// Check a load of sprite `i` at `cursor` against [`expect`], and that
/// nothing above the new cursor was written.
fn check(before: &State, after: &mut State, i: usize, cursor: u32) {
    let e = expect(i, cursor);
    assert_eq!(after.ctx.gpr[V0], sext(cursor), "sprite {i}: returns the header");
    assert_eq!(words(after, cursor, e.header.len()), e.header, "sprite {i}: header and page table");
    for &(at, data) in &e.parts {
        assert_eq!(at % 16, 0);
        assert_eq!(bytes(after, at, data.len()), data, "sprite {i}: part at {at:#x}");
    }
    assert_eq!(after.rdram.mem().read_u32(CURSORS), e.end, "sprite {i}: cursor");
    let (lo, hi) = (((e.end - 0x8000_0000) / 4) as usize, ((e.end + 0x1000 - 0x8000_0000) / 4) as usize);
    assert!(after.rdram.as_words()[lo..hi] == before.rdram.as_words()[lo..hi], "sprite {i}: wrote above the cursor");
}

/// Every sprite from a fresh heap, checked against the format.
#[test]
fn every_sprite() {
    let _rom = install_rom_doubles();
    assert_eq!(blocks().sprites.count, COUNT);
    for i in 0..COUNT {
        let mut before = world(i as u64, HEAP_8MB);
        before.ctx.gpr[A0] = i as u64;
        let mut after = sprite_load(&before);
        check(&before, &mut after, i, HEAP_8MB.cursor);
        // count, (offset, next), header, table, [palette], pages
        let s = Sprite::parse(raw(i)).unwrap();
        assert_eq!(after.calls.len(), 4 + usize::from(s.palette.is_some()) + s.pages.len(), "sprite {i}: reads");
        assert!(after.calls.iter().all(|c| c.name == ROM_READ_SMALL));
        assert_eq!(after.rdram.mem().read_u32(OUT_OF_HEAP), 0);
    }
}

/// All sprites one after another on the same heap, each landing at the
/// previous one's (16-aligned) end, in two orders.
#[test]
fn sequential_loads() {
    let _rom = install_rom_doubles();
    for order in [(0..COUNT).collect::<Vec<_>>(), (0..COUNT).map(|k| (k * 59) % COUNT).collect()] {
        let mut s = world(order[1] as u64, HEAP_8MB);
        let mut cursor = HEAP_8MB.cursor;
        for &i in &order {
            let mut before = State { calls: Vec::new(), ..s };
            before.ctx.gpr[A0] = i as u64;
            let mut after = sprite_load(&before);
            check(&before, &mut after, i, cursor);
            cursor = after.rdram.mem().read_u32(CURSORS);
            s = after;
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any sprite at any word-aligned cursor (the header goes there as it
    /// is; the parts after it are 16-aligned), with any register values.
    #[test]
    fn any_cursor(i in 0usize..COUNT, cursor in 0x8019_0000u32..0x8030_0000, seed: u64) {
        let _rom = install_rom_doubles();
        let cursor = cursor & !3;
        let mut before = world(seed, Heap { cursor, ..HEAP_8MB });
        before.ctx.gpr[A0] = i as u64;
        let mut after = compare("func_8002FF38", loader::func_8002FF38, &before)
            .map_err(|d| TestCaseError::fail(d.to_string()))?;
        check(&before, &mut after, i, cursor);
    }
}

/// Every cursor alignment mod 16 that the ROM reads allow (word-aligned),
/// on a sprite with a palette and one without.
#[test]
fn cursor_alignment() {
    let _rom = install_rom_doubles();
    let b = blocks();
    let with = (0..COUNT).find(|&i| b.sprite(i).unwrap().palette.is_some()).unwrap();
    let without = (0..COUNT).find(|&i| b.sprite(i).unwrap().palette.is_none() && i != 110).unwrap();
    for i in [with, without] {
        for off in [0, 4, 8, 12] {
            let cursor = HEAP_8MB.cursor + off;
            let mut before = world(off.into(), Heap { cursor, ..HEAP_8MB });
            before.ctx.gpr[A0] = i as u64;
            let mut after = sprite_load(&before);
            check(&before, &mut after, i, cursor);
        }
    }
}

/// Indices outside the block return 0 after reading the count, and leave
/// the heap alone. As in model_load, the index is compared as a 64-bit
/// register but only after rom_read_small restored `s0` from its low word,
/// so `0x8000_0000` with a zero upper half is negative and `0x1_0000_00AD`
/// is 173.
#[test]
fn out_of_range_indices() {
    let _rom = install_rom_doubles();
    for a0 in [
        sext(173),
        sext(174),
        sext(-1i32 as u32),
        sext(i32::MIN as u32),
        sext(i32::MAX as u32),
        0x0000_0000_8000_0000,
        0x0000_0001_0000_00AD,
    ] {
        let mut before = world(a0, HEAP_8MB);
        before.ctx.gpr[A0] = a0;
        let mut after = sprite_load(&before);
        assert_eq!(after.ctx.gpr[V0], 0, "index {a0:#x}");
        assert_eq!(after.calls.len(), 1, "index {a0:#x}: only the count is read");
        assert_eq!(after.rdram.mem().read_u32(CURSORS), HEAP_8MB.cursor);
    }
}

#[test]
fn upper_half_of_the_index_is_dropped() {
    let _rom = install_rom_doubles();
    for a0 in [0x0000_0001_0000_0005, 0xFFFF_FFFF_0000_0005, 0x7FFF_0000_0000_0005] {
        let mut before = world(9, HEAP_8MB);
        before.ctx.gpr[A0] = a0;
        let mut after = sprite_load(&before);
        check(&before, &mut after, 5, HEAP_8MB.cursor);
    }
}

/// Sprite 110 is 1x1 I4 with no pages: the table read is empty, `+0x10`
/// still gets the table's address, and the cursor ends at align16(header +
/// 0x14).
#[test]
fn sprite_without_pages() {
    let _rom = install_rom_doubles();
    let s = blocks().sprite(110).unwrap();
    assert!(s.pages.is_empty() && s.palette.is_none() && raw(110).len() == 0x14);
    for off in [0, 4, 8, 12] {
        let cursor = HEAP_8MB.cursor + off;
        let mut before = world(110 + u64::from(off), Heap { cursor, ..HEAP_8MB });
        before.ctx.gpr[A0] = 110;
        let mut after = sprite_load(&before);
        check(&before, &mut after, 110, cursor);
        assert_eq!(after.rdram.mem().read_u32(CURSORS), align(cursor + 0x14, 16));
        assert_eq!(after.calls.len(), 4);
        assert_eq!(after.calls[3].gpr[A2], 0, "the empty table read");
    }
}

/// There is no space check: a sprite loaded with the heap nearly full is
/// written past the heap end, and nothing is flagged.
#[test]
fn no_space_check() {
    let _rom = install_rom_doubles();
    let i = (0..COUNT).max_by_key(|&i| raw(i).len()).unwrap();
    let mut before = world(1, Heap { end: HEAP_8MB.cursor + 0x40, ..HEAP_8MB });
    before.ctx.gpr[A0] = i as u64;
    let mut after = sprite_load(&before);
    check(&before, &mut after, i, HEAP_8MB.cursor);
    let m = after.rdram.mem();
    assert!(m.read_u32(CURSORS) > m.read_u32(HEAP_END) + 0x1_0000);
    assert_eq!(m.read_u32(OUT_OF_HEAP), 0);
}

/// A copy of the ROM with sprite `i`'s bytes changed by `patch`, as a
/// rom_read_small double (on top of [`install_rom_doubles`]).
fn patched(i: usize, patch: impl FnOnce(&mut [u8])) -> doubles::Installed {
    let at = raw(i).as_ptr() as usize - baserom().as_ptr() as usize;
    let mut rom = baserom().to_vec();
    patch(&mut rom[at..at + raw(i).len()]);
    let rom: &'static [u8] = Box::leak(rom.into_boxed_slice());
    doubles::install(ROM_READ_SMALL, rom_read_small(rom))
}

/// QUIRK: a CI sprite (`+4 == 2`) whose palette offset is 0 returns at once,
/// with the cursor set back to the header, so the header isn't reserved and
/// the next load overwrites it. No USA sprite does this; here a CI4 sprite's
/// palette offset is zeroed.
#[test]
fn ci_sprite_without_palette_is_not_reserved() {
    let b = blocks();
    let i = (0..COUNT).find(|&i| b.sprite(i).unwrap().format_code == 0x200).unwrap();
    let _rom = install_rom_doubles();
    let _patch = patched(i, |s| s[8..12].copy_from_slice(&[0; 4]));

    let mut before = world(4, HEAP_8MB);
    before.ctx.gpr[A0] = i as u64;
    let mut after = sprite_load(&before);
    let cursor = HEAP_8MB.cursor;
    assert_eq!(after.ctx.gpr[V0], sext(cursor), "returns the header");
    assert_eq!(after.calls.len(), 3, "count, offsets, header: no table, palette or pages");
    let mut header: Vec<u32> = (0..5).map(|k| be32(raw(i), 4 * k)).collect();
    header[2] = 0;
    assert_eq!(words(&mut after, cursor, 5), header, "the header as read, +0x10 not patched");
    assert_eq!(after.rdram.mem().read_u32(CURSORS), cursor, "not reserved");

    // The next load goes to the same place.
    let j = (i + 1) % COUNT;
    let mut next = State { calls: Vec::new(), ..after };
    next.ctx.gpr[A0] = j as u64;
    let mut after = sprite_load(&next);
    check(&next, &mut after, j, cursor);
}

/// QUIRK: with a palette and no pages, the palette's size comes from "page
/// 0's offset" in a table that was never read: the word at header + 0x18,
/// whatever it holds. Sprite 110 (no pages, 0x14 bytes) is patched to have a
/// palette at 0x14; the test plants 0x34 at header + 0x18, so the "palette"
/// is the 32 bytes that follow sprite 110 in the ROM.
#[test]
fn palette_without_pages_reads_an_unloaded_offset() {
    let _rom = install_rom_doubles();
    let _patch = patched(110, |s| s[8..12].copy_from_slice(&0x14u32.to_be_bytes()));
    let cursor = HEAP_8MB.cursor;
    let mut before = world(110, HEAP_8MB);
    before.rdram.mem().write_u32(cursor + 0x18, 0x34);
    before.ctx.gpr[A0] = 110;
    let mut after = sprite_load(&before);
    let pal = align(cursor + 0x14, 16);
    assert_eq!(after.rdram.mem().read_u32(cursor + 8), pal);
    let follows = &baserom()[raw(110).as_ptr() as usize - baserom().as_ptr() as usize + 0x14..][..0x20];
    assert_eq!(bytes(&mut after, pal, 0x20), follows);
    assert_eq!(after.rdram.mem().read_u32(CURSORS), align(pal + 0x20, 16));
}

/// QUIRK: a negative page count reads no table, but `8 * count` is still
/// added to the cursor, which ends up inside the header's own 0x14 bytes.
#[test]
fn negative_page_count_moves_the_cursor_back() {
    let _rom = install_rom_doubles();
    let _patch = patched(110, |s| s[0xC..0xE].copy_from_slice(&(-1i16).to_be_bytes()));
    for off in [0, 4, 8, 12] {
        let cursor = HEAP_8MB.cursor + off;
        let mut before = world(u64::from(off), Heap { cursor, ..HEAP_8MB });
        before.ctx.gpr[A0] = 110;
        let after = sprite_load(&before);
        assert_eq!(after.ctx.gpr[V0], sext(cursor));
        assert_eq!(after.rdram.as_words()[((CURSORS - 0x8000_0000) / 4) as usize], align(cursor + 0x14 - 8, 16));
    }
}

/// func_80030154 is `sprite_load(a0)` with its own frame; func_80030130 is
/// `sprite_load(a1)` and spills `a0`. Both call the recompiled sprite_load.
#[test]
fn wrappers() {
    let _rom = install_rom_doubles();
    for (k, &i) in [0u64, 17, 110, 160, 172, 173, sext(-1i32 as u32)].iter().enumerate() {
        let mut before = world(k as u64, HEAP_8MB);
        before.ctx.gpr[A0] = i;
        let mut after = compare("func_80030154", loader::func_80030154, &before).unwrap_or_else(|d| panic!("{d}"));
        if i < COUNT as u64 {
            check(&before, &mut after, i as usize, HEAP_8MB.cursor);
        } else {
            assert_eq!(after.ctx.gpr[V0], 0);
        }

        let mut before = world(k as u64 + 100, HEAP_8MB);
        before.ctx.gpr[A1] = i;
        let mut after = compare("func_80030130", loader::func_80030130, &before).unwrap_or_else(|d| panic!("{d}"));
        if i < COUNT as u64 {
            check(&before, &mut after, i as usize, HEAP_8MB.cursor);
        } else {
            assert_eq!(after.ctx.gpr[V0], 0);
        }
        assert_eq!(after.rdram.as_words()[((STACK - 0x8000_0000) / 4) as usize], before.ctx.gpr[A0] as u32, "a0 spilled to its slot");
    }
}
