//! The texture and model loaders: func_80030328 (texture_read), func_800304AC
//! (texture_get) and func_800305E8 (model_load), recompiled C vs
//! game::loader, on real data from baserom.z64 (read at test time; nothing
//! ROM-derived is stored in the repository).
//!
//! rom_read / rom_read_small are test doubles (difftest::rom). Every other
//! callee (the heap helpers, comp_decompress, texture_get/texture_read under
//! model_load) is the recompiled C, reached by both runs.
//!
//! Heap state: [`difftest::world`] (from the boot code; NOTES.md, "Asset
//! heap").

use difftest::rom::{baserom, install_rom_doubles, rom_read, ROM_READ};
use difftest::world::{align, blocks, bytes, sext, words, world, Heap, HEAP_4MB, HEAP_8MB};
use difftest::{compare, State};
use game::heap::CURSORS;
use game::loader::{self, OUT_OF_HEAP, TEXTURE_CACHE};
use game::recomp::reg::*;
use oracle::doubles;
use proptest::prelude::*;

fn model_load(s: &State) -> State {
    compare("func_800305E8", loader::func_800305E8, s).unwrap_or_else(|d| panic!("{d}"))
}

/// What a successful load of `index` from a fresh world should leave, stated
/// from the data formats (NOTES.md), not from the code: the model at
/// `align8(cursor)` with masked words relocated, and each texture loaded once,
/// in order of first reference, at the next 64-byte boundary (pixels, then
/// palette), with later references copying the first reference's pair.
struct Expected {
    base: u32,
    words: Vec<u32>,
    /// (address, bytes) of each texture part loaded.
    parts: Vec<(u32, &'static [u8])>,
    model_end: u32,
    end: u32,
}

fn expect(index: usize, cursor: u32) -> Expected {
    let b = blocks();
    let model = b.model(index).unwrap();
    let base = align(cursor, 8);
    let mut w: Vec<u32> = model.data.chunks(4).map(|c| u32::from_be_bytes(c.try_into().unwrap())).collect();
    let model_end = base + model.data.len() as u32;
    let mut cur = model_end;
    let mut parts = Vec::new();
    // texture index -> word index of the first reference (the cache entry)
    let mut first: std::collections::HashMap<usize, usize> = Default::default();
    for i in 0..w.len() {
        let bit = model.mask.get(i / 8).is_some_and(|m| m >> (7 - i % 8) & 1 != 0);
        if !bit {
            continue;
        }
        let v = w[i];
        if v >> 24 == 0x0A {
            let t = (v & 0xFF_FFFF) as usize;
            if t >= b.textures.count {
                w[i] = 0;
                w[i + 1] = 0;
            } else if let Some(&r) = first.get(&t) {
                w[i] = w[r];
                w[i + 1] = w[r + 1];
            } else {
                let tex = b.texture(t);
                let pix = align(cur, 64);
                parts.push((pix, tex.pixels));
                w[i] = pix;
                cur = pix + tex.pixels.len() as u32;
                if let Some(pal) = tex.palette {
                    let p = align(cur, 64);
                    parts.push((p, pal));
                    w[i + 1] = p;
                    cur = p + pal.len() as u32;
                }
                first.insert(t, i);
            }
        } else if v != 0 {
            w[i] = v.wrapping_add(base);
        }
    }
    Expected { base, words: w, parts, model_end, end: cur }
}

/// Every model from a fresh heap, both ways, checked against [`expect`].
#[test]
fn every_model() {
    let _rom = install_rom_doubles();
    let n = blocks().models.count;
    assert_eq!(n, 307);
    for i in 0..n {
        let mut before = world(i as u64, HEAP_8MB);
        before.ctx.gpr[A0] = i as u64;
        let mut after = model_load(&before);
        let e = expect(i, HEAP_8MB.cursor);
        assert_eq!(after.ctx.gpr[V0], sext(e.base + 4), "model {i}: return value");
        assert_eq!(words(&mut after, e.base, e.words.len()), e.words, "model {i}: relocated words");
        for &(at, data) in &e.parts {
            assert_eq!(bytes(&mut after, at, data.len()), data, "model {i}: texture part at {at:#x}");
        }
        let m = after.rdram.mem();
        assert_eq!(m.read_u32(CURSORS), e.end, "model {i}: cursor");
        assert_eq!(m.read_u32(OUT_OF_HEAP), 0, "model {i}");
        assert_eq!(m.read_u32(0x800A_2848), 1);
        assert_eq!(m.read_u32(0x800D_9DC0), HEAP_8MB.cursor, "model {i}: cursor before");
        assert_eq!(m.read_u32(0x800D_9DC4), e.model_end, "model {i}: cursor after the model");
        assert_eq!(m.read_u32(0x800D_9DC8), e.model_end - HEAP_8MB.cursor, "model {i}: model bytes");
        assert_eq!(m.read_u32(0x800D_9DCC), e.end - e.model_end, "model {i}: texture bytes");
        assert_eq!(m.read_u32(0x800D_9DD0), 0);
    }
}

/// Loading models one after another on the same heap, as a race setup does:
/// the texture cache fills, later models hit it, and the heap eventually runs
/// out, so every out-of-space path runs on real data. Checked in both memory
/// configurations and two orders.
#[test]
fn sequential_loads() {
    let _rom = install_rom_doubles();
    let n = blocks().models.count;
    for (heap, order) in [
        (HEAP_8MB, (0..n).collect::<Vec<_>>()),
        (HEAP_4MB, (0..n).rev().collect()),
        (HEAP_8MB, (0..n).map(|k| (k * 97) % n).collect()),
    ] {
        let mut s = world(heap.cursor as u64, heap);
        let (mut ok, mut failed, mut tex_failed) = (0, 0, 0);
        for &i in &order {
            let mut before = State { calls: Vec::new(), ..s };
            before.rdram.mem().write_u32(OUT_OF_HEAP, 0);
            before.ctx.gpr[A0] = i as u64;
            let mut after = model_load(&before);
            let oom = after.rdram.mem().read_u32(OUT_OF_HEAP) != 0;
            match (after.ctx.gpr[V0], oom) {
                (0, true) => failed += 1,
                (0, false) => panic!("model {i}: 0 without running out of heap"),
                (_, true) => tex_failed += 1,
                (_, false) => ok += 1,
            }
            s = after;
        }
        eprintln!("{heap:x?}: {ok} loaded, {tex_failed} loaded with a texture out of heap, {failed} out of heap");
        // "Loaded, but a texture ran out" needs a heap end in a narrow band;
        // texture_out_of_heap and heap_limits cover it.
        assert!(ok > 0 && failed > 0, "both outcomes should occur");
    }
}

/// Indices outside the block return 0 after the stores at the start.
///
/// model_load compares the full 64-bit register, but the index lives in
/// `s0`, which rom_read_small (the first call) saves with `sw` and restores
/// with `lw`. So by the compare, only the sign-extended low word is left:
/// `0x1_0000_0005` loads model 5, and `0x8000_0000` with a zero upper half
/// is negative.
#[test]
fn out_of_range_indices() {
    let _rom = install_rom_doubles();
    for a0 in [
        sext(307),
        sext(308),
        sext(-1i32 as u32),
        sext(i32::MIN as u32),
        sext(i32::MAX as u32),
        0x0000_0000_8000_0000,
        0x0000_0001_0000_0133,
    ] {
        let mut before = world(a0, HEAP_8MB);
        before.ctx.gpr[A0] = a0;
        let mut after = model_load(&before);
        assert_eq!(after.ctx.gpr[V0], 0, "index {a0:#x}");
        assert_eq!(after.calls.len(), 1, "index {a0:#x}: only the count is read");
        let m = after.rdram.mem();
        assert_eq!(m.read_u32(0x800A_2848), 1);
        assert_eq!(m.read_u32(0x800D_9DC8), 0);
        assert_eq!(m.read_u32(CURSORS), HEAP_8MB.cursor);
    }
}

#[test]
fn upper_half_of_the_index_is_dropped() {
    let _rom = install_rom_doubles();
    let mut plain = world(9, HEAP_8MB);
    plain.ctx.gpr[A0] = 5;
    let want = model_load(&plain).ctx.gpr[V0];
    assert_eq!(want, sext(align(HEAP_8MB.cursor, 8) + 4));
    for a0 in [0x0000_0001_0000_0005, 0xFFFF_FFFF_0000_0005, 0x7FFF_0000_0000_0005] {
        let mut before = world(9, HEAP_8MB);
        before.ctx.gpr[A0] = a0;
        assert_eq!(model_load(&before).ctx.gpr[V0], want, "index {a0:#x}");
    }
}

/// A model that fits with no room for its first texture: the load succeeds
/// (returns base + 4) but sets the out-of-heap flag. Every reference to that
/// texture gets a null pair, because the cache points at the first
/// reference's pair, which texture_read zeroed.
#[test]
fn texture_out_of_heap() {
    let _rom = install_rom_doubles();
    let b = blocks();
    let (i, e) = (0..b.models.count)
        .filter(|&i| !b.model(i).unwrap().compressed)
        .map(|i| (i, expect(i, HEAP_8MB.cursor)))
        .find(|(_, e)| !e.parts.is_empty())
        .unwrap();
    let mut before = world(11, Heap { end: e.model_end + 0x40, ..HEAP_8MB });
    before.ctx.gpr[A0] = i as u64;
    let mut after = model_load(&before);
    assert_eq!(after.ctx.gpr[V0], sext(e.base + 4));
    assert_eq!(after.rdram.mem().read_u32(OUT_OF_HEAP), 1);
    assert_eq!(after.rdram.mem().read_u32(CURSORS), e.model_end, "no texture was placed");
    let got = words(&mut after, e.base, e.words.len());
    let model = b.model(i).unwrap();
    let refs = model.texture_refs();
    assert!(!refs.is_empty());
    for r in refs.iter().filter(|r| r.index < b.textures.count) {
        assert_eq!(got[r.at / 4], 0, "model {i}: reference at {:#x}", r.at);
    }
}

/// How much heap model `i` needs: model bytes + 8, and for Comp models the
/// payload above the output; plus the textures.
fn need(i: usize) -> u32 {
    let e = expect(i, HEAP_8MB.cursor);
    e.end - HEAP_8MB.cursor
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    /// Heap ends around each model's thresholds: before the model fits, when
    /// a Comp payload would overlap the output, when the decompressor's
    /// window overlaps it, and when textures run out.
    #[test]
    fn heap_limits(i in 0usize..307, frac in 0.0f64..1.3, jitter in 0u32..0x400, cursor_off in 0u32..8) {
        let _rom = install_rom_doubles();
        // A window overlapping the output can corrupt the tag, which reaches
        // the error path (see tight_comp_layouts): give it its stand-in.
        let _err = doubles::install("func_800827C0", |_, ctx| doubles::clobber_caller_saved(ctx, &[]));
        let total = need(i);
        let cursor = HEAP_8MB.cursor + cursor_off;
        let end = cursor + (f64::from(total) * frac) as u32 + jitter;
        let mut before = world(i as u64, Heap { cursor, end, ..HEAP_8MB });
        before.ctx.gpr[A0] = i as u64;
        compare("func_800305E8", loader::func_800305E8, &before).map_err(|d| TestCaseError::fail(d.to_string()))?;
    }
}

/// A Comp model whose payload lands just above the output, so the
/// decompressor's window (the 4 KB below the payload) overlaps the output:
/// C and Rust must still agree, whatever the result.
///
/// QUIRK: model_load's space checks don't allow for the window, so with less
/// than 0x1000 bytes between output and payload the window and the output
/// overwrite each other and the model can come out corrupt. A corrupt tag
/// reaches the error path func_800827C0, which gets a double that just
/// returns (what it really does is not known).
#[test]
fn tight_comp_layouts() {
    let _rom = install_rom_doubles();
    let _err = doubles::install("func_800827C0", |_, ctx| doubles::clobber_caller_saved(ctx, &[]));
    let (mut intact, mut corrupt, mut error_path) = (0, 0, 0);
    let b = blocks();
    let comp: Vec<usize> = (0..b.models.count).filter(|&i| b.model(i).unwrap().compressed).collect();
    assert_eq!(comp.len(), 92);
    for &i in comp.iter().step_by(7) {
        let model = b.model(i).unwrap();
        let raw = b.models.entry(i)[1].unwrap();
        let payload = raw.len() as u32 - 12;
        let base = align(HEAP_8MB.cursor, 8);
        for slack in [0u32, 8, 0x100, 0x800, 0xFF8, 0x1000, 0x1008] {
            let end = base + model.data.len() as u32 + payload + slack;
            let mut before = world(i as u64 ^ u64::from(slack), Heap { end, ..HEAP_8MB });
            before.ctx.gpr[A0] = i as u64;
            let mut after = model_load(&before);
            // Compare the words neither relocation nor a texture pair (the
            // word after a masked one) can touch with the data. Textures may
            // run out of heap here, which zeroes pairs.
            let e = expect(i, HEAP_8MB.cursor);
            let got = words(&mut after, e.base, e.words.len());
            let masked = |k: usize| model.mask.get(k / 8).is_some_and(|m| m >> (7 - k % 8) & 1 != 0);
            let unmasked_ok =
                (0..e.words.len()).filter(|&k| !masked(k) && (k == 0 || !masked(k - 1))).all(|k| got[k] == e.words[k]);
            if after.calls.iter().any(|c| c.name == "func_800827C0") {
                error_path += 1;
            }
            if unmasked_ok {
                intact += 1;
            } else {
                // The payload goes at (end - payload) & ~7, so the window
                // starts up to 4 bytes lower than slack alone suggests.
                let window = ((end - payload) & !7) - 0x1000;
                assert!(window < e.model_end, "model {i}, slack {slack:#x}: corrupt with the window clear of the output");
                corrupt += 1;
            }
        }
    }
    eprintln!("tight Comp layouts: {intact} intact, {corrupt} corrupt ({error_path} reached the error path)");
    assert!(corrupt > 0, "some tight layout should corrupt the output");
}

/// A model whose tag isn't one of the seven reaches func_800827C0 (error
/// path, not understood). A double that just returns lets both runs go on:
/// the model loads but the result is the model base, not base + 4. The ROM
/// copy with the bad tag exists only in memory.
#[test]
fn unknown_tag_calls_the_error_path() {
    let b = blocks();
    let i = (0..b.models.count).find(|&i| !b.model(i).unwrap().compressed).unwrap();
    let model_rom = b.models.entry(i)[1].unwrap().as_ptr() as usize - baserom().as_ptr() as usize;
    let mut patched = baserom().to_vec();
    patched[model_rom..model_rom + 4].copy_from_slice(b"Xxxx");
    let patched: &'static [u8] = Box::leak(patched.into_boxed_slice());
    let _rom = install_rom_doubles();
    let _read = doubles::install(ROM_READ, rom_read(patched));
    let _err = doubles::install("func_800827C0", |_, ctx| doubles::clobber_caller_saved(ctx, &[]));

    let mut before = world(3, HEAP_8MB);
    before.ctx.gpr[A0] = i as u64;
    let after = model_load(&before);
    assert_eq!(after.ctx.gpr[V0], sext(align(HEAP_8MB.cursor, 8)), "returns the model base");
    assert!(after.calls.iter().any(|c| c.name == "func_800827C0"));
}

/// texture_get for every texture index and the out-of-range ones, from a
/// fresh cache, then again as a cache hit, with the pointer pair in a
/// scratch area.
#[test]
fn texture_get_every_index() {
    let _rom = install_rom_doubles();
    let b = blocks();
    const PAIR: u32 = 0x8010_0000;
    let mut hit = world(0, HEAP_8MB);
    for t in (0..b.textures.count as i64).chain([-1, 1648, 1700, i32::MAX as i64, i32::MIN as i64]) {
        // Fresh: the cache is empty.
        let mut before = world(t as u64, HEAP_8MB);
        before.ctx.gpr[A0] = t as u64;
        before.ctx.gpr[A1] = sext(PAIR);
        before.ctx.gpr[A2] = sext(PAIR + 4);
        let mut after = compare("func_800304AC", loader::func_800304AC, &before).unwrap_or_else(|d| panic!("{d}"));
        let pair = words(&mut after, PAIR, 2);
        if !(0..b.textures.count as i64).contains(&t) {
            assert_eq!(pair, [0, 0], "texture {t}");
            continue;
        }
        let tex = b.texture(t as usize);
        let pix = align(HEAP_8MB.cursor, 64);
        assert_eq!(pair[0], pix, "texture {t}: pixels");
        assert_eq!(bytes(&mut after, pix, tex.pixels.len()), tex.pixels, "texture {t}");
        match tex.palette {
            Some(p) => {
                assert_eq!(pair[1], align(pix + tex.pixels.len() as u32, 64), "texture {t}: palette");
                assert_eq!(bytes(&mut after, pair[1], p.len()), p);
            }
            None => assert_eq!(pair[1], before.rdram.as_words()[(PAIR + 4 - 0x8000_0000) as usize / 4], "untouched"),
        }
        assert_eq!(after.rdram.mem().read_u32(TEXTURE_CACHE + 4 * t as u32), PAIR);

        // Hit: a cache entry pointing at an earlier pair (here, the fresh
        // run's). Copies it into a new pair.
        let mut before = State { calls: Vec::new(), ..std::mem::replace(&mut hit, State::new()) };
        before.rdram.mem().write_u32(PAIR + 0x100, pair[0]);
        before.rdram.mem().write_u32(PAIR + 0x104, pair[1]);
        before.rdram.mem().write_u32(TEXTURE_CACHE + 4 * t as u32, PAIR + 0x100);
        before.ctx.gpr[A0] = t as u64;
        before.ctx.gpr[A1] = sext(PAIR + 0x200);
        before.ctx.gpr[A2] = sext(PAIR + 0x204);
        let mut after = compare("func_800304AC", loader::func_800304AC, &before).unwrap_or_else(|d| panic!("{d}"));
        assert!(after.calls.is_empty(), "a hit reads nothing");
        assert_eq!(words(&mut after, PAIR + 0x200, 2), pair);
        hit = after;
    }
}

/// texture_read directly: every texture's table words, with heaps from far
/// too small to roomy, and a palette-less texture's untouched palette word.
#[test]
fn texture_read_limits() {
    let _rom = install_rom_doubles();
    let b = blocks();
    const HDR: u32 = 0x8010_0000;
    let rom = baserom();
    let table = |k: usize| u32::from_be_bytes(rom[b.textures.base + 4 + 4 * k..][..4].try_into().unwrap());
    for t in (0..b.textures.count).step_by(5) {
        let (pix, pal, next) = (table(2 * t), table(2 * t + 1), table(2 * t + 2));
        let size = next - pix;
        for end in [0, size, size + 0x7F, size + 0x80, size + 0x80 + 64, size + 0x1000] {
            let mut before = world(t as u64 ^ u64::from(end), Heap { end: HEAP_8MB.cursor.wrapping_add(end), ..HEAP_8MB });
            let mut m = before.rdram.mem();
            m.write_u32(HDR, pix);
            m.write_u32(HDR + 4, pal);
            m.write_u32(HDR + 8, next);
            before.ctx.gpr[A0] = sext(HDR);
            before.ctx.gpr[A1] = sext(HDR + 0x20);
            before.ctx.gpr[A2] = sext(HDR + 0x24);
            let after = compare("func_80030328", loader::func_80030328, &before).unwrap_or_else(|d| panic!("{d}"));
            let oom = (end as i32) < (size + 0x80) as i32;
            assert_eq!(after.rdram.as_words()[(OUT_OF_HEAP - 0x8000_0000) as usize / 4], u32::from(oom), "texture {t}");
        }
    }
}
