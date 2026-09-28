//! The texture and model block loaders (NOTES.md, "ROM asset loading path").
//!
//! They read the ROM through `rom_read` / `rom_read_small`, place data on the
//! asset heap ([`crate::heap`]) and patch pointers into what they load.

// Ports keep N64Recomp's names (func_800305E8), capitals included.
#![allow(non_snake_case)]

use crate::imports;
use crate::recomp::{addu, call, enter, lbu, lh, li, lw, reg::*, s32, sll, slt, subu, sw, RecompContext};

/// Texture block in the ROM: `u32 count`, then (pixels, palette) offset pairs.
pub const TEXTURE_BLOCK: u32 = 0x0102_ABB0;
/// Model block in the ROM: `u32 count`, then (mask, model) offset pairs.
pub const MODEL_BLOCK: u32 = 0x0141_E200;
/// One word per texture index: 0, or the address of the (pixels, palette)
/// pointer pair filled in when the texture was first loaded.
pub const TEXTURE_CACHE: u32 = 0x800D_9E00;
/// Number of textures, read from the block by `texture_block_init`.
pub const TEXTURE_COUNT: u32 = 0x800D_B890;
/// Set to 1 when a loader runs out of heap.
pub const OUT_OF_HEAP: u32 = 0x800A_2864;
/// Buffer the model loader reads relocation masks into.
pub const MASK_BUFFER: u32 = 0x8011_4528;

/// `func_80030328` (texture_read): `texture_read(hdr, &pixels, &palette, _)`.
///
/// `hdr` points at three words from the texture table: this entry's pixel
/// and palette offsets and the next entry's pixel offset. If
/// `heap_free() < next - pixels + 0x80` (signed) it stores 0 to `*palette`
/// then `*pixels`, sets [`OUT_OF_HEAP`], and returns. Otherwise it reads the
/// pixels to the heap cursor rounded up to 64 and stores that address in
/// `*pixels`. If there is a palette it reads it to the next 64-byte boundary
/// and stores that in `*palette`. Then it moves the cursor to the end of
/// the last part read.
///
/// When the palette offset is 0, `*palette` is **not written**: the caller's
/// word keeps whatever it held. `a3` is saved to the stack and never used.
///
/// Domain: canonical `hdr`, `pixels` and `palette` pointers; the 0x30-byte
/// frame and the argument slots above `sp` in RDRAM; and the callees'.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80030328(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x30i64) as u64);
    sw(m, g[SP], 0x1C, g[RA]);
    sw(m, g[SP], 0x18, g[S1]);
    g[S1] = g[A0];
    sw(m, g[SP], 0x14, g[S0]);
    sw(m, g[SP], 0x34, g[A1]);
    sw(m, g[SP], 0x38, g[A2]);
    sw(m, g[SP], 0x3C, g[A3]);
    call(imports::func_8002FC58, m, ctx);

    let g = &mut ctx.gpr;
    g[V1] = lw(m, g[S1], 0);
    g[T6] = lw(m, g[S1], 8);
    g[T8] = lw(m, g[SP], 0x38);
    g[A0] = subu(g[T6], g[V1]);
    g[T7] = addu(g[A0], 0x80);
    g[AT] = u64::from((g[V0] as i64) < (g[T7] as i64));
    if g[AT] != 0 {
        // Out of heap.
        sw(m, g[T8], 0, 0);
        g[T9] = lw(m, g[SP], 0x34);
        g[T1] = 1;
        g[AT] = li(0x800A_0000);
        sw(m, g[T9], 0, 0);
        sw(m, g[AT], 0x2864, g[T1]);
    } else {
        // beql delay slot (taken): lw v0, 4(s1)
        g[V0] = lw(m, g[S1], 4);
        // bnez v0 / (delay) subu s0, v0, v1 / else (b delay) move s0, a0
        g[S0] = subu(g[V0], g[V1]);
        if g[V0] == 0 {
            g[S0] = g[A0];
        }
        call(imports::func_8002FAFC, m, ctx);

        let g = &mut ctx.gpr;
        g[V1] = lw(m, g[S1], 0);
        g[A1] = addu(g[V0], 0x3F);
        g[AT] = li(0xFFFF_FFC0);
        g[T0] = li(TEXTURE_BLOCK);
        g[T2] = g[A1] & g[AT];
        g[A1] = g[T2];
        sw(m, g[SP], 0x20, g[T0]);
        sw(m, g[SP], 0x24, g[T2]);
        g[A2] = g[S0];
        g[A0] = addu(g[T0], g[V1]);
        call(imports::func_80011CDC, m, ctx); // rom_read(pixels)

        let g = &mut ctx.gpr;
        g[A3] = lw(m, g[SP], 0x24);
        g[T3] = lw(m, g[SP], 0x34);
        g[T0] = lw(m, g[SP], 0x20);
        g[AT] = li(0xFFFF_FFC0);
        sw(m, g[T3], 0, g[A3]);
        g[V0] = lw(m, g[S1], 4);
        g[A0] = addu(g[T0], g[V0]); // beqz delay slot
        if g[V0] != 0 {
            g[T5] = lw(m, g[S1], 8);
            g[A3] = addu(g[A3], g[S0]);
            g[A3] = addu(g[A3], 0x3F);
            g[A1] = g[A3] & g[AT];
            g[S0] = subu(g[T5], g[V0]);
            g[A2] = g[S0];
            sw(m, g[SP], 0x24, g[A1]);
            call(imports::func_80011CDC, m, ctx); // rom_read(palette)

            let g = &mut ctx.gpr;
            g[A3] = lw(m, g[SP], 0x24);
            g[T6] = lw(m, g[SP], 0x38);
            sw(m, g[T6], 0, g[A3]);
        }
        let g = &mut ctx.gpr;
        g[A0] = addu(g[A3], g[S0]);
        call(imports::func_8002FAC4, m, ctx);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x30);
}

/// `func_800304AC` (texture_get): `texture_get(index, &pixels, &palette)`.
///
/// Index out of range (`index < 0` or `>= [TEXTURE_COUNT]`, signed 64-bit
/// compares): stores 0 to `*pixels` then `*palette`. Cached
/// (`TEXTURE_CACHE[index] = p != 0`): copies `p[0]` to `*pixels`, then
/// reloads the cache word and copies its `[1]` to `*palette`. Otherwise it
/// reads the entry's three table words with `rom_read_small`, calls
/// [`func_80030328`], and caches `&pixels` itself, the caller's word, not
/// the texture.
///
/// QUIRK: the cache points into whoever loaded the texture first (for models,
/// a word inside that model). A later hit copies whatever is there now: a
/// null pair if that first load ran out of heap, or the model's stale word
/// at `+4` if the texture has no palette (texture_read doesn't write it).
///
/// Domain: canonical `pixels`/`palette` pointers; the 0x40-byte frame and
/// argument slots above `sp` in RDRAM; cache entries 0 or canonical pointers;
/// and the callees'.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800304AC(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x44, g[A1]);
    sw(m, g[SP], 0x48, g[A2]);
    // bltz a0 / (delay) move a3, a0
    g[A3] = g[A0];
    let mut in_range = false;
    if (g[A0] as i64) >= 0 {
        g[T6] = lw(m, li(0x800E_0000), -0x4770); // TEXTURE_COUNT
        g[T0] = li(TEXTURE_CACHE);
        g[AT] = u64::from((g[A0] as i64) < (g[T6] as i64));
        g[T9] = sll(g[A3], 2); // bnez delay slot
        in_range = g[AT] != 0;
    }
    if !in_range {
        g[T7] = lw(m, g[SP], 0x44);
        sw(m, g[T7], 0, 0);
        g[T8] = lw(m, g[SP], 0x48);
        sw(m, g[T8], 0, 0);
    } else {
        g[V0] = addu(g[T9], g[T0]);
        g[V1] = lw(m, g[V0], 0);
        g[T6] = sll(g[A3], 3);
        g[T7] = addu(g[T6], 4);
        g[T8] = li(0x0103_0000); // beqz delay slot
        if g[V1] != 0 {
            g[T1] = lw(m, g[V1], 0);
            g[T2] = lw(m, g[SP], 0x44);
            sw(m, g[T2], 0, g[T1]);
            g[T3] = lw(m, g[V0], 0);
            g[T5] = lw(m, g[SP], 0x48);
            g[T4] = lw(m, g[T3], 4);
            sw(m, g[T5], 0, g[T4]);
        } else {
            g[T8] = li(TEXTURE_BLOCK);
            g[A0] = addu(g[T7], g[T8]);
            g[A1] = addu(g[SP], 0x28);
            g[A2] = 0xC;
            sw(m, g[SP], 0x20, g[V0]);
            call(imports::func_80011D60, m, ctx); // rom_read_small(table words)

            let g = &mut ctx.gpr;
            g[A0] = addu(g[SP], 0x28);
            g[A1] = lw(m, g[SP], 0x44);
            g[A2] = lw(m, g[SP], 0x48);
            g[A3] = 1;
            call(imports::func_80030328, m, ctx);

            let g = &mut ctx.gpr;
            g[V0] = lw(m, g[SP], 0x20);
            g[T9] = lw(m, g[SP], 0x44);
            sw(m, g[V0], 0, g[T9]);
        }
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x40);
}

/// Model tags `func_800305E8` accepts, in the order it tests them.
pub const MODEL_TAGS: [u32; 7] = [
    0x4D6F_646C, // Modl
    0x5472_616B, // Trak
    0x506F_6464, // Podd
    0x5061_7274, // Part
    0x5363_656E, // Scen
    0x4D41_6C74, // MAlt
    0x5075_7070, // Pupp
];

/// `func_800305E8` (model_load): load model `index` from the model block
/// and return a pointer just past its tag, or 0.
///
/// In order:
/// 1. Sets `[0x800A2848] = 1`, zeroes the stats `0x800D9DC8`, `DCC`, `DD0`,
///    and reads the block's count with `rom_read_small`.
/// 2. `index < 0` or `>= count` (signed 64-bit): returns 0.
/// 3. Reads the entry's (mask, model, next mask) offsets, then the mask into
///    `[MASK_BUFFER]`, then 12 bytes of the model to the cursor rounded up to 8
///    (`s1`). QUIRK: nothing checks for space before these reads.
/// 4. "Comp": the payload (`size - 12` bytes) goes to the top of the heap,
///    `(heap_end - (size - 12)) & ~7`, and is decompressed to `s1`. Otherwise
///    the model is read to `s1` directly. Either way it first checks
///    `heap_free() >= size + 8` (signed; `size` = decompressed size for Comp),
///    and for Comp also that the payload lies above `s1 + size` (unsigned).
///    If either check fails it sets [`OUT_OF_HEAP`] and returns 0, leaving
///    the cursor unmoved. Then the cursor moves to `s1 + size`.
/// 5. Records `[0x800D9DC0]` = cursor before and `[0x800D9DC4]` = after.
/// 6. Relocates word `i` for each set mask bit (MSB first): top byte `0x0A`
///    means [`func_800304AC`]`(word & 0xFFFFFF, &word, &word + 4)`; any other
///    nonzero word gets `+ s1`.
/// 7. Accepts one of [`MODEL_TAGS`] and returns `s1 + 4`. Otherwise it calls
///    `func_800827C0` (error) and, if that returns, returns `s1`.
/// 8. Stats: `[0x800D9DCC]` = bytes the textures took, `[0x800D9DC8]` =
///    bytes the model took.
///
/// The index is compared as a full 64-bit register, but only after the first
/// `rom_read_small`, which saves and restores `s0` (holding the index) with
/// `sw`/`lw`. So in effect the index is the sign-extended low word of `a0`.
///
/// QUIRK: the space checks ignore the decompressor's window (the 4 KB below
/// the payload). With less than 0x1000 bytes between the output's end and the
/// payload, window and output overwrite each other and the model can come
/// out corrupt (NOTES.md).
///
/// Domain: canonical `sp`, the 0x88-byte frame in RDRAM, and the callees'.
/// `a0` may have any upper half.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_800305E8(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[T6] = 1;
    g[AT] = li(0x800A_0000);
    sw(m, g[AT], 0x2848, g[T6]);
    g[SP] = addu(g[SP], (-0x88i64) as u64);
    g[AT] = li(0x800E_0000);
    sw(m, g[SP], 0x14, g[S0]);
    sw(m, g[AT], -0x6238, 0);
    g[S0] = g[A0];
    g[T7] = li(0x8011_0000);
    g[AT] = li(0x800E_0000);
    g[T7] = lw(m, g[T7], 0x4528); // MASK_BUFFER
    sw(m, g[AT], -0x6234, 0);
    g[A0] = li(0x0142_0000);
    sw(m, g[SP], 0x1C, g[RA]);
    g[AT] = li(0x800E_0000);
    g[A0] = addu(g[A0], (-0x1E00i64) as u64); // MODEL_BLOCK
    sw(m, g[SP], 0x18, g[S1]);
    sw(m, g[AT], -0x6230, 0);
    sw(m, g[SP], 0x28, g[A0]);
    g[A1] = addu(g[SP], 0x84);
    g[A2] = 4;
    sw(m, g[SP], 0x2C, g[T7]);
    call(imports::func_80011D60, m, ctx); // rom_read_small(count)

    let g = &mut ctx.gpr;
    // bltz s0 / (delay) lw t8, 0x84(sp); slt at, s0, t8 / bnez / (delay) lw t9, 0x28(sp)
    g[T8] = lw(m, g[SP], 0x84);
    let mut in_range = false;
    if (g[S0] as i64) >= 0 {
        g[AT] = u64::from((g[S0] as i64) < (g[T8] as i64));
        g[T9] = lw(m, g[SP], 0x28);
        in_range = g[AT] != 0;
    }
    if in_range {
        in_range_body(m, ctx);
    } else {
        ctx.gpr[V0] = 0;
    }

    // L_80030950: epilogue
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x1C);
    g[S0] = lw(m, g[SP], 0x14);
    g[S1] = lw(m, g[SP], 0x18);
    g[SP] = addu(g[SP], 0x88);
}

/// `func_800305E8` from `L_80030660` (index in range) to the epilogue.
fn in_range_body(m: &mut n64mem::Mem, ctx: &mut RecompContext) {
    let g = &mut ctx.gpr;
    g[T1] = sll(g[S0], 3);
    g[A0] = addu(g[T9], g[T1]);
    g[A0] = addu(g[A0], 4);
    g[A1] = addu(g[SP], 0x6C);
    g[A2] = 0xC;
    call(imports::func_80011D60, m, ctx); // rom_read_small(mask, model, next offsets)

    let g = &mut ctx.gpr;
    g[T0] = lw(m, g[SP], 0x70); // model offset
    g[T2] = lw(m, g[SP], 0x74); // next entry's mask offset
    g[V0] = lw(m, g[SP], 0x6C); // mask offset
    g[T4] = lw(m, g[SP], 0x28);
    g[T3] = subu(g[T2], g[T0]);
    sw(m, g[SP], 0x3C, g[T3]); // model size in the ROM
    g[A1] = lw(m, g[SP], 0x2C);
    g[A2] = subu(g[T0], g[V0]);
    g[A0] = addu(g[T4], g[V0]);
    call(imports::func_80011CDC, m, ctx); // rom_read(mask -> mask buffer)
    call(imports::func_8002FAFC, m, ctx);

    let g = &mut ctx.gpr;
    g[T0] = lw(m, g[SP], 0x70);
    g[T6] = lw(m, g[SP], 0x28);
    g[A3] = addu(g[V0], 7);
    g[AT] = li(0xFFFF_FFF8);
    g[A1] = g[A3] & g[AT];
    sw(m, g[SP], 0x44, g[V0]);
    g[S1] = g[A1];
    g[A2] = 0xC;
    g[A0] = addu(g[T6], g[T0]);
    call(imports::func_80011CDC, m, ctx); // rom_read(first 12 bytes -> s1)

    let g = &mut ctx.gpr;
    g[T7] = lw(m, g[S1], 0);
    g[AT] = li(0x436F_6D70); // "Comp"
    g[A2] = lw(m, g[SP], 0x3C); // bne delay slot
    if g[T7] == g[AT] {
        g[T8] = lw(m, g[S1], 8); // decompressed size
        g[A2] = addu(g[A2], (-0xCi64) as u64);
        sw(m, g[SP], 0x38, g[A2]);
        sw(m, g[SP], 0x3C, g[T8]);
        call(imports::func_8002FC58, m, ctx);

        let g = &mut ctx.gpr;
        g[A3] = lw(m, g[SP], 0x3C);
        g[A2] = lw(m, g[SP], 0x38);
        g[T1] = 1;
        g[T9] = addu(g[A3], 8);
        g[AT] = u64::from((g[V0] as i64) < (g[T9] as i64));
        g[T2] = li(0x800E_0000); // beqz delay slot
        if g[AT] != 0 {
            g[AT] = li(0x800A_0000);
            sw(m, g[AT], 0x2864, g[T1]);
            g[V0] = 0;
            return;
        }
        g[T2] = lw(m, g[T2], -0x6244); // heap end
        g[AT] = li(0xFFFF_FFF8);
        g[T4] = addu(g[S1], g[A3]);
        g[S0] = subu(g[T2], g[A2]);
        g[T3] = g[S0] & g[AT];
        g[AT] = u64::from(g[T3] < g[T4]);
        g[S0] = g[T3]; // beqz delay slot
        if g[AT] != 0 {
            g[T5] = 1;
            g[AT] = li(0x800A_0000);
            sw(m, g[AT], 0x2864, g[T5]);
            g[V0] = 0;
            return;
        }
        g[T0] = lw(m, g[SP], 0x70);
        g[T6] = lw(m, g[SP], 0x28);
        g[A1] = g[S0];
        g[V1] = addu(g[T0], 0xC);
        g[A0] = addu(g[T6], g[V1]);
        call(imports::func_80011CDC, m, ctx); // rom_read(payload -> top of heap)

        let g = &mut ctx.gpr;
        g[A0] = g[S0];
        g[A1] = g[S1];
        call(imports::func_80011940, m, ctx); // comp_decompress(payload, s1)

        let g = &mut ctx.gpr;
        g[T7] = lw(m, g[SP], 0x3C);
        g[A0] = addu(g[S1], g[T7]);
        call(imports::func_8002FAC4, m, ctx);
    } else {
        call(imports::func_8002FC58, m, ctx);

        let g = &mut ctx.gpr;
        g[T8] = lw(m, g[SP], 0x3C);
        g[T0] = lw(m, g[SP], 0x70);
        g[T2] = lw(m, g[SP], 0x28);
        g[T9] = addu(g[T8], 8);
        g[AT] = u64::from((g[V0] as i64) < (g[T9] as i64));
        g[A0] = addu(g[T2], g[T0]); // beqz delay slot
        if g[AT] != 0 {
            g[T1] = 1;
            g[AT] = li(0x800A_0000);
            sw(m, g[AT], 0x2864, g[T1]);
            g[V0] = 0;
            return;
        }
        g[A1] = g[S1];
        g[A2] = lw(m, g[SP], 0x3C);
        call(imports::func_80011CDC, m, ctx); // rom_read(model -> s1)

        let g = &mut ctx.gpr;
        g[T3] = lw(m, g[SP], 0x3C);
        g[A0] = addu(g[S1], g[T3]);
        call(imports::func_8002FAC4, m, ctx);
    }

    // L_800307E4 (t4 = [sp+0x44] is loaded on both paths)
    let g = &mut ctx.gpr;
    g[T4] = lw(m, g[SP], 0x44);
    g[AT] = li(0x800E_0000);
    sw(m, g[AT], -0x6240, g[T4]);
    call(imports::func_8002FAFC, m, ctx);

    let g = &mut ctx.gpr;
    g[AT] = li(0x800E_0000);
    sw(m, g[AT], -0x623C, g[V0]);
    call(imports::func_8002FAFC, m, ctx);

    let g = &mut ctx.gpr;
    g[T0] = lw(m, g[SP], 0x3C);
    sw(m, g[SP], 0x40, g[V0]);
    sw(m, g[SP], 0x60, g[S1]);
    g[T5] = s32(((g[T0] as i64) >> 2) as u32);
    g[S0] = g[S1];
    g[T0] = g[T5];
    g[A3] = 0; // blez delay slot
    if (g[T5] as i64) > 0 {
        relocate(m, ctx);
    }

    // L_800308A8: tag check
    let g = &mut ctx.gpr;
    g[A0] = lw(m, g[S1], 0);
    g[AT] = li(MODEL_TAGS[0]);
    g[T9] = addu(g[S1], 4); // delay slot of the first beq
    let mut known = g[A0] == g[AT];
    let mut k = 0;
    while !known && k + 1 < MODEL_TAGS.len() {
        k += 1;
        // lui (possibly in the previous delay slot) + ori: the full tag.
        g[AT] = li(MODEL_TAGS[k]);
        known = g[A0] == g[AT];
        // Tests 1..=5 have the next tag's lui in their delay slot; the last
        // one (bne) has a nop.
        if let Some(&next) = MODEL_TAGS.get(k + 1) {
            g[AT] = li(next & 0xFFFF_0000);
        }
    }
    if known {
        sw(m, g[SP], 0x60, g[T9]);
    } else {
        call(imports::func_800827C0, m, ctx);
    }

    // L_80030918
    call(imports::func_8002FAFC, m, ctx);
    let g = &mut ctx.gpr;
    g[T3] = lw(m, g[SP], 0x40);
    g[T5] = li(0x800E_0000);
    g[T4] = li(0x800E_0000);
    g[T4] = lw(m, g[T4], -0x6240);
    g[T5] = lw(m, g[T5], -0x623C);
    g[AT] = li(0x800E_0000);
    g[T2] = subu(g[V0], g[T3]);
    sw(m, g[AT], -0x6234, g[T2]);
    g[AT] = li(0x800E_0000);
    g[T1] = subu(g[T5], g[T4]);
    sw(m, g[AT], -0x6238, g[T1]);
    g[V0] = lw(m, g[SP], 0x60);
}

/// The relocation loop of `func_800305E8` (`L_8003081C` to `L_800308A8`):
/// `s0` walks the model, `a3` counts words, `t0` is the word count.
fn relocate(m: &mut n64mem::Mem, ctx: &mut RecompContext) {
    loop {
        let g = &mut ctx.gpr;
        g[T6] = lw(m, g[SP], 0x2C); // mask buffer
        g[T7] = s32(((g[A3] as i64) >> 5) as u32);
        g[T8] = sll(g[T7], 2);
        g[T9] = addu(g[T6], g[T8]);
        g[T1] = lw(m, g[T9], 0);
        g[T2] = g[A3] & 0x1F;
        g[T3] = 0x1F;
        g[T4] = subu(g[T3], g[T2]);
        g[T5] = 1;
        g[T7] = sll(g[T5], (g[T4] & 31) as u32);
        g[T6] = g[T1] & g[T7];
        if g[T6] == 0 {
            // beql delay slot (taken): addiu a3, a3, 1, straight to the loop test.
            g[A3] = addu(g[A3], 1);
        } else {
            g[V0] = lw(m, g[S0], 0);
            g[AT] = li(0x00FF_FFFF);
            g[A0] = g[V0] & g[AT];
            g[AT] = li(0xFF00_0000);
            g[V1] = g[V0] & g[AT];
            g[AT] = li(0x0A00_0000);
            g[A1] = g[S0]; // bne delay slot
            if g[V1] == g[AT] {
                g[A2] = addu(g[S0], 4);
                sw(m, g[SP], 0x68, g[A3]);
                sw(m, g[SP], 0x28, g[T0]);
                call(imports::func_800304AC, m, ctx); // texture_get(index, &word, &word + 4)

                let g = &mut ctx.gpr;
                g[A3] = lw(m, g[SP], 0x68);
                g[T0] = lw(m, g[SP], 0x28);
            } else {
                g[T8] = addu(g[V0], g[S1]); // beqz delay slot
                if g[V0] != 0 {
                    sw(m, g[S0], 0, g[T8]);
                }
            }
            let g = &mut ctx.gpr;
            g[A3] = addu(g[A3], 1);
        }
        // L_800308A0: bne a3, t0 / (delay) addiu s0, s0, 4
        let g = &mut ctx.gpr;
        g[S0] = addu(g[S0], 4);
        if g[A3] == g[T0] {
            break;
        }
    }
}

/// Sprite block in the ROM: `u32 count`, then one offset per sprite (and one
/// for the end of the last).
pub const SPRITE_BLOCK: u32 = 0x0133_07F0;

/// `func_8002FF38` (sprite_load): load sprite `index` to the heap cursor and
/// return a pointer to its header, or 0.
///
/// In order:
/// 1. Reads the block's count with `rom_read_small`. `index < 0` or
///    `>= count` (signed 64-bit, after that call restored `s0` from its low
///    word, so in effect the sign-extended low word of `a0`): returns 0.
/// 2. Reads the entry's offset and the next one, then the 0x14-byte header
///    to the cursor as it is (not aligned).
/// 3. If the format byte `+4` is 2 (CI) and the palette offset `+8` is 0, sets
///    the cursor back to the header and returns it. QUIRK: the header isn't
///    reserved, so the next allocation overwrites it. No USA sprite does this.
/// 4. Reads `(s16) +0xC` 8-byte page entries to header + 0x14 and stores that
///    address in `+0x10`. A count of 0 or less reads nothing.
/// 5. Counts `s1` up to the page count and zeroes it again (dead loop).
/// 6. If `+8` is nonzero, reads the palette, from `+8` up to page 0's offset,
///    to the next 16-byte boundary and stores its address in `+8`.
/// 7. Reads each page, from its offset up to the next page's (the last up to
///    the next sprite's), to the next 16-byte boundary after the previous
///    part, and stores its address in the entry's `+4`. The page count is
///    re-read from the header after each page.
/// 8. Sets the cursor to the end of the last part read, rounded up to 16, and
///    returns the header.
///
/// QUIRK: there is no space check, so a sprite near the heap end is written
/// past it. QUIRK: with a palette and a page count of 0 or less, page 0's
/// offset is read from a table that wasn't loaded (whatever is at header +
/// 0x18). QUIRK: a negative page count moves the cursor below the header's
/// end (`+ 8 * count` is added unchecked).
///
/// Domain: canonical `sp` and a word-aligned cursor (`rom_read_small` stores
/// words); the 0x70-byte frame in RDRAM; and the callees'.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8002FF38(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x70i64) as u64);
    sw(m, g[SP], 0x34, g[RA]);
    sw(m, g[SP], 0x18, g[S0]);
    g[S0] = g[A0];
    sw(m, g[SP], 0x30, g[S6]);
    sw(m, g[SP], 0x2C, g[S5]);
    sw(m, g[SP], 0x28, g[S4]);
    sw(m, g[SP], 0x24, g[S3]);
    sw(m, g[SP], 0x20, g[S2]);
    sw(m, g[SP], 0x1C, g[S1]);
    call(imports::func_8002FAFC, m, ctx); // heap_cursor

    let g = &mut ctx.gpr;
    g[S6] = li(SPRITE_BLOCK);
    g[S2] = g[V0];
    g[A0] = g[S6];
    g[A1] = addu(g[SP], 0x58);
    g[A2] = 4;
    call(imports::func_80011D60, m, ctx); // rom_read_small(count)

    let g = &mut ctx.gpr;
    // bltz s0 / (delay) lw t6, 0x58(sp); slt at, s0, t6 / bnez / (delay) sll t7, s0, 2
    g[T6] = lw(m, g[SP], 0x58);
    let mut in_range = false;
    if (g[S0] as i64) >= 0 {
        g[AT] = u64::from((g[S0] as i64) < (g[T6] as i64));
        g[T7] = sll(g[S0], 2);
        in_range = g[AT] != 0;
    }
    if in_range {
        sprite_body(m, ctx);
        let g = &mut ctx.gpr;
        g[V0] = g[S4];
    } else {
        ctx.gpr[V0] = 0;
    }

    // L_80030108: epilogue
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x34);
    g[S0] = lw(m, g[SP], 0x18);
    g[S1] = lw(m, g[SP], 0x1C);
    g[S2] = lw(m, g[SP], 0x20);
    g[S3] = lw(m, g[SP], 0x24);
    g[S4] = lw(m, g[SP], 0x28);
    g[S5] = lw(m, g[SP], 0x2C);
    g[S6] = lw(m, g[SP], 0x30);
    g[SP] = addu(g[SP], 0x70);
}

/// `func_8002FF38` from `L_8002FF9C` (index in range) up to the final
/// `move v0, s4`: `s2` is the placement cursor, `s4` the header.
fn sprite_body(m: &mut n64mem::Mem, ctx: &mut RecompContext) {
    let g = &mut ctx.gpr;
    g[A0] = addu(g[S6], g[T7]);
    g[A0] = addu(g[A0], 4);
    g[A1] = addu(g[SP], 0x50);
    g[A2] = 8;
    call(imports::func_80011D60, m, ctx); // rom_read_small(offset, next offset)

    let g = &mut ctx.gpr;
    g[T0] = lw(m, g[SP], 0x50);
    g[A1] = g[S2];
    g[A2] = 0x14;
    g[A0] = addu(g[S6], g[T0]);
    call(imports::func_80011D60, m, ctx); // rom_read_small(header -> cursor)

    let g = &mut ctx.gpr;
    g[T8] = lbu(m, g[S2], 4);
    g[AT] = 2;
    g[S4] = g[S2];
    g[T0] = lw(m, g[SP], 0x50); // bne delay slot
    if g[T8] == g[AT] {
        g[T9] = lw(m, g[S2], 8);
        if g[T9] == 0 {
            // CI without a palette: straight to L_800300FC.
            set_cursor_to_s2(m, ctx);
            return;
        }
    }

    // L_8002FFE4: the page table
    g[S3] = lh(m, g[S4], 0xC);
    g[S2] = addu(g[S2], 0x14);
    g[T0] = addu(g[T0], 0x14);
    g[A2] = sll(g[S3], 3);
    g[S3] = g[A2];
    g[A0] = addu(g[S6], g[T0]);
    g[A1] = g[S2];
    call(imports::func_80011D60, m, ctx); // rom_read_small(page table -> header + 0x14)

    let g = &mut ctx.gpr;
    g[V0] = lh(m, g[S4], 0xC);
    sw(m, g[S4], 0x10, g[S2]);
    g[S1] = 0;
    g[S5] = li(0xFFFF_FFF0); // blez delay slot
    if (g[V0] as i64) > 0 {
        // Counts s1 up to the page count, then drops it.
        g[S1] = addu(g[S1], 1);
        loop {
            g[AT] = u64::from((g[S1] as i64) < (g[V0] as i64));
            if g[AT] == 0 {
                break;
            }
            g[S1] = addu(g[S1], 1); // bnel delay slot
        }
        g[S1] = 0;
    }

    // L_8003002C: the palette
    g[V1] = lw(m, g[S4], 8);
    g[T2] = lw(m, g[SP], 0x50);
    g[T0] = addu(g[T2], g[V1]); // beqz delay slot
    if g[V1] != 0 {
        g[T3] = lw(m, g[S4], 0x10);
        g[S2] = addu(g[S2], g[S3]);
        g[S2] = addu(g[S2], 0xF);
        g[T4] = lw(m, g[T3], 4); // page 0's offset
        g[S2] &= g[S5];
        g[A1] = g[S2];
        g[S3] = subu(g[T4], g[V1]);
        g[A2] = g[S3];
        g[A0] = addu(g[S6], g[T0]);
        call(imports::func_80011D60, m, ctx); // rom_read_small(palette)

        let g = &mut ctx.gpr;
        sw(m, g[S4], 8, g[S2]);
        g[V0] = lh(m, g[S4], 0xC);
    }

    // L_8003006C: the pages
    let g = &mut ctx.gpr;
    g[S2] = addu(g[S2], g[S3]);
    g[S5] = li(0xFFFF_FFF0);
    g[S2] = addu(g[S2], 0xF);
    g[S2] &= g[S5]; // blez delay slot
    if (g[V0] as i64) > 0 {
        g[S0] = 0;
        loop {
            let g = &mut ctx.gpr;
            g[T5] = addu(g[S1], 1);
            g[T6] = lw(m, g[SP], 0x54); // bne delay slot: the next sprite's offset
            if g[T5] == g[V0] {
                // Last page: up to the end of the sprite.
                g[T7] = lw(m, g[SP], 0x50);
                g[T8] = lw(m, g[S4], 0x10);
                g[A3] = subu(g[T6], g[T7]);
                g[V1] = addu(g[T8], g[S0]); // b delay slot
            } else {
                g[T9] = lw(m, g[S4], 0x10);
                g[V1] = addu(g[T9], g[S0]);
                g[A3] = lw(m, g[V1], 0xC); // the next page's offset
            }
            // L_800300B0
            g[V0] = lw(m, g[V1], 4);
            g[T1] = lw(m, g[SP], 0x50);
            g[A1] = g[S2];
            g[S3] = subu(g[A3], g[V0]);
            g[T0] = addu(g[V0], g[T1]);
            g[A0] = addu(g[S6], g[T0]);
            g[A2] = g[S3];
            call(imports::func_80011D60, m, ctx); // rom_read_small(page)

            let g = &mut ctx.gpr;
            g[T2] = lw(m, g[S4], 0x10);
            g[S1] = addu(g[S1], 1);
            g[T3] = addu(g[T2], g[S0]);
            sw(m, g[T3], 4, g[S2]);
            g[V0] = lh(m, g[S4], 0xC);
            g[S2] = addu(g[S2], g[S3]);
            g[S2] = addu(g[S2], 0xF);
            g[AT] = u64::from((g[S1] as i64) < (g[V0] as i64));
            g[S0] = addu(g[S0], 8);
            g[S2] &= g[S5]; // bnez delay slot
            if g[AT] == 0 {
                break;
            }
        }
    }
    set_cursor_to_s2(m, ctx);
}

/// `L_800300FC`: `heap_set_cursor(s2)`.
fn set_cursor_to_s2(m: &mut n64mem::Mem, ctx: &mut RecompContext) {
    ctx.gpr[A0] = ctx.gpr[S2];
    call(imports::func_8002FAC4, m, ctx);
}

/// `func_80030130`: `sprite_load(a1)`, returning its result. `a0` is only
/// spilled to its argument slot. Nothing in the code or data refers to this
/// function (it was found between two others), so it may be dead.
///
/// Domain: canonical `sp`, the 0x18-byte frame and `a0`'s argument slot in
/// RDRAM, and [`func_8002FF38`]'s.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80030130(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x18, g[A0]);
    g[A0] = g[A1];
    call(imports::func_8002FF38, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80030154`: `sprite_load(a0)`, returning its result, through a
/// stack frame. The game calls sprite_load through this (about 100 call
/// sites).
///
/// Domain: canonical `sp`, the 0x18-byte frame in RDRAM, and
/// [`func_8002FF38`]'s.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80030154(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    call(imports::func_8002FF38, m, ctx);
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// Spline block in the ROM: `u32 count`, then single offsets.
pub const SPLINE_BLOCK: u32 = 0x012C_7F30;

/// `func_80030174` (spline_load): `spline_load(index, &out)`.
///
/// Reads the block's count; if `index` is negative or not below it (signed
/// compares), stores 0 to `*out` and returns. The index is spilled to the
/// stack before the first read and reloaded with `lw`, so in effect it is
/// the sign-extended low word (as in `model_load` and `sprite_load`). Otherwise it reads
/// entries `index` and `index + 1` of the offset table, reads the spline
/// (from the first up to the second) to the heap cursor with
/// `rom_read_small`, stores the cursor in `*out`, **overwrites** the
/// header's `+0xC` with `cursor + 0x10` (the first point; whatever the ROM
/// had there is stale), and moves the cursor past the spline (NOTES.md,
/// "Splines").
///
/// Like `sprite_load` there is **no space check** and no alignment: the
/// spline goes at the cursor as it is (QUIRK: `rom_read_small` stores words,
/// so an unaligned cursor would fault; every block entry is a multiple of 4
/// bytes). It reloads the cursor with `heap_cursor` twice after the read. A
/// pointless loop counts `v1` up to the point count (header `+4`, 2^31
/// iterations if that is `0x7FFFFFFF`), as in `sprite_load`; `heap_check`,
/// under the final `heap_set_cursor`, overwrites `v1` anyway.
///
/// `v0` on the out-of-range path is whatever `rom_read_small` left there.
///
/// Domain: canonical `out` in RDRAM; the 0x40-byte frame and the argument
/// slots above `sp` in RDRAM; and the callees'.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80030174(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x40i64) as u64);
    g[A3] = g[A0];
    g[A0] = li(SPLINE_BLOCK);
    sw(m, g[SP], 0x14, g[RA]);
    sw(m, g[SP], 0x44, g[A1]);
    sw(m, g[SP], 0x1C, g[A0]);
    g[A1] = addu(g[SP], 0x30);
    sw(m, g[SP], 0x40, g[A3]);
    g[A2] = 4;
    call(imports::func_80011D60, m, ctx); // rom_read_small(block, &count, 4)

    let g = &mut ctx.gpr;
    g[A3] = lw(m, g[SP], 0x40);
    g[T6] = lw(m, g[SP], 0x30);
    g[T7] = lw(m, g[SP], 0x44);
    g[AT] = slt(g[A3], g[T6]);
    'done: {
        if (g[A3] as i64) >= 0 {
            g[T8] = lw(m, g[SP], 0x1C); // bltz delay slot
            if g[AT] != 0 {
                g[T9] = sll(g[A3], 2);
                g[A0] = addu(g[T8], g[T9]);
                g[A0] = addu(g[A0], 4);
                g[A1] = addu(g[SP], 0x28);
                g[A2] = 8;
                call(imports::func_80011D60, m, ctx); // the two offsets

                let g = &mut ctx.gpr;
                g[T0] = lw(m, g[SP], 0x28);
                sw(m, g[SP], 0x3C, g[T0]);
                call(imports::func_8002FAFC, m, ctx); // heap_cursor

                let g = &mut ctx.gpr;
                g[T1] = lw(m, g[SP], 0x2C);
                g[T2] = lw(m, g[SP], 0x28);
                g[T3] = lw(m, g[SP], 0x1C);
                g[T4] = lw(m, g[SP], 0x3C);
                g[A2] = subu(g[T1], g[T2]);
                sw(m, g[SP], 0x34, g[V0]);
                sw(m, g[SP], 0x38, g[A2]);
                g[A1] = g[V0];
                g[A0] = addu(g[T3], g[T4]);
                call(imports::func_80011D60, m, ctx); // the spline
                call(imports::func_8002FAFC, m, ctx); // heap_cursor

                let g = &mut ctx.gpr;
                g[T5] = lw(m, g[SP], 0x44);
                sw(m, g[T5], 0, g[V0]);
                call(imports::func_8002FAFC, m, ctx); // heap_cursor

                let g = &mut ctx.gpr;
                g[T7] = lw(m, g[SP], 0x44);
                g[T6] = addu(g[V0], 0x10);
                g[V1] = 0;
                g[T8] = lw(m, g[T7], 0);
                sw(m, g[T8], 0xC, g[T6]);
                g[A1] = lw(m, g[T7], 0);
                g[T0] = lw(m, g[SP], 0x38);
                g[T9] = lw(m, g[SP], 0x34);
                g[A0] = lw(m, g[A1], 4);
                g[V1] = addu(g[V1], 1);
                // The pointless loop.
                if (g[A0] as i64) > 0 {
                    loop {
                        g[AT] = slt(g[V1], g[A0]);
                        if g[AT] == 0 {
                            break;
                        }
                        g[V1] = addu(g[V1], 1);
                    }
                }
                g[A0] = addu(g[T9], g[T0]);
                call(imports::func_8002FAC4, m, ctx); // heap_set_cursor
                break 'done;
            }
        }
        // Out of range.
        sw(m, g[T7], 0, 0);
    }
    let g = &mut ctx.gpr;
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x40);
}

/// `func_8003043C` (texture_block_init): read the texture count into
/// [`TEXTURE_COUNT`] with `rom_read_small`, then zero [`TEXTURE_CACHE`].
///
/// The cache is always 1700 words (`0x800D9E00` up to the count itself at
/// `0x800DB890`), four per iteration, whatever the count. A count of 1701 or
/// more (signed) hangs instead: QUIRK, `b .` before the loop, an assert with
/// nothing else left. N64Recomp turns that into one call to the runtime's
/// `pause_self`, which never returns; the port makes the same call, and would
/// carry on into the loop just as the C does if it did return.
///
/// Leaves `v0 = v1 = 0x800DB890`, `at = 1` and `t6` = the count.
///
/// Domain: canonical `sp`, the 0x18-byte frame in RDRAM, and
/// `rom_read_small`'s.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_8003043C(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[SP] = addu(g[SP], (-0x18i64) as u64);
    sw(m, g[SP], 0x14, g[RA]);
    g[A0] = li(TEXTURE_BLOCK);
    g[A1] = li(TEXTURE_COUNT);
    g[A2] = 4;
    call(imports::func_80011D60, m, ctx); // rom_read_small(count -> TEXTURE_COUNT)

    let g = &mut ctx.gpr;
    g[T6] = lw(m, li(0x800E_0000), -0x4770);
    g[V0] = li(TEXTURE_CACHE);
    g[AT] = u64::from((g[T6] as i64) < 0x6A5);
    g[V1] = li(0x800E_0000); // bne delay slot
    if g[AT] == 0 {
        // L_80030478: b . (the delay slot is a nop)
        imports::runtime::pause_self(m.as_mut_ptr());
    }
    // L_80030480
    g[V1] = addu(g[V1], (-0x4770i64) as u64);
    loop {
        g[V0] = addu(g[V0], 0x10);
        sw(m, g[V0], -0xC, 0);
        sw(m, g[V0], -0x8, 0);
        sw(m, g[V0], -0x4, 0);
        sw(m, g[V0], -0x10, 0); // bne delay slot
        if g[V0] == g[V1] {
            break;
        }
    }
    g[RA] = lw(m, g[SP], 0x14);
    g[SP] = addu(g[SP], 0x18);
}

/// `func_80030574` (texture_cache_trim): zero every [`TEXTURE_CACHE`] word
/// that is above `a0`, so textures loaded above a heap cursor are forgotten.
///
/// All 1700 words are visited, four per iteration. The test is `sltu a0,
/// word` on the full registers, with the word sign-extended by `lw`: a word is
/// zeroed when `a0 < word` as unsigned 64-bit values. Zero words never are.
/// `a0` is only compared, so any value is in the domain; a zero-extended
/// (non-canonical) address is below every sign-extended KSEG0 pointer and
/// clears them all.
///
/// Leaves `v0 = v1 = 0x800DB890`, `t6`-`t9` the last group's words (as read,
/// before any zeroing) and `at` the last compare.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80030574(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let m = &mut mem;
    let g = &mut ctx.gpr;
    g[V1] = li(TEXTURE_CACHE);
    g[V0] = li(TEXTURE_COUNT); // the end of the cache
    g[T6] = lw(m, g[V1], 0);
    loop {
        // Each test: sltu at, a0, tN / beql at, zero (delay: the next load)
        // / else sw zero and the same load.
        g[AT] = u64::from(g[A0] < g[T6]);
        if g[AT] != 0 {
            sw(m, g[V1], 0, 0);
        }
        g[T7] = lw(m, g[V1], 4);
        g[AT] = u64::from(g[A0] < g[T7]);
        if g[AT] != 0 {
            sw(m, g[V1], 4, 0);
        }
        g[T8] = lw(m, g[V1], 8);
        g[AT] = u64::from(g[A0] < g[T8]);
        if g[AT] != 0 {
            sw(m, g[V1], 8, 0);
        }
        g[T9] = lw(m, g[V1], 0xC);
        g[AT] = u64::from(g[A0] < g[T9]);
        if g[AT] != 0 {
            sw(m, g[V1], 0xC, 0);
        }
        g[V1] = addu(g[V1], 0x10);
        // bnel v1, v0 / (delay, if taken) lw t6, 0(v1)
        if g[V1] == g[V0] {
            break;
        }
        g[T6] = lw(m, g[V1], 0);
    }
}
