# racer-rs — Notes

Running log of discoveries and decisions. Newest session at the bottom.

## Facts established

### ROM (verified 2026-09-28, `cargo xtask verify-rom`)
- The user's dump is `rom/Star Wars Episode I - Racer (USA).n64` but is actually **v64 (byte-swapped)**, not n64. Extensions lie; detect from the first word.
- Header matches SPEC §2: `STAR WARS EP1 RACER`, `NEPE`, version 0, entry `0x80000400`, CRCs `72F70398 6556A98B`.
- Boot checksum recomputed from data matches under **CIC-6102**, so the dump is intact.
- SHA-1 of the z64 image is recorded locally in `rom/EXPECTED.sha1` (gitignored).
- Header libultra field `0x00001449` → built against **libultra 2.0I**.
- Build tag string at ROM `0x1110`: `v07Apr99.1553`.

### Graphics microcode (SPEC §10 Q4)
- ROM `0xAEF08`: `RSP Gfx ucode F3DEX.NoN   fifo 2.08  Yoshitaka Yasumoto 1999 Nintendo.`
- **F3DEX2 family (2.08, NoN, FIFO)**. The name says "F3DEX", but F3DEX 1.x stopped at 1.23. Static display lists in the uncompressed data segment (`0x99000–0x100000`) confirm it: 11 `gSPEndDisplayList` with the F3DEX2 opcode `0xDF` and none with the F3DEX opcode `0xB8`; `G_VTX` `0x01` beats `0x04`. Confirm with RT64's ucode detection when the scaffold runs.

### Expansion Pak
- Supported: the ROM has "Expansion Pak enhanced/supported/not detected" strings, and code reads `osMemSize` (`0x80000318`) in about 25 places. Hi-res 640×480i mode. Model RDRAM as 8 MB.

### Strings
- Main code (ROM `0x1000–0x99000`) contains essentially no strings: no source filenames or asserts.
- About 250 UI strings sit in the block at ROM `0x0A0000` (just after code), e.g. `INSERT A CONTROLLER INTO`. These are the main string anchors for Phase 2 matching against SW_RACER_RE.
- Everything past about `0x100000` looks compressed or binary (assets).

### FPU control register (SPEC §10 Q5)
- ~340 `cfc1`/`ctc1 $31` pairs in game code, nearly all in the pattern *save FCR31 → set rounding mode → `cvt.w.s` → restore*. This is a compiler idiom for float→int conversions, not a global mode change. **The recompiler must honour the local rounding mode**, and Rust ports of these conversions need to reproduce the rounding mode the idiom selects.
- Other FCR31 writes: `0x8008d370` (`$k1`, exception handler context restore) and `0x80093c24` (likely `__osSetFpcCsr`). **(verify)** what the initial FCR31 value is.
- 557 `trunc.w.s` as well. Both idioms exist; this doesn't settle IDO vs GCC by itself, though the cfc1/ctc1 dance is characteristic of IDO **(verify)**.

### 64-bit instructions
- `ld`/`sd`/`dsll32`/`dsra32`/`ddiv`/`ddivu`/`dmultu` are confined to about `0x8008a000–0x8008d000`, probably libultra `__ll_*` 64-bit helpers and `_Printf`. Game code appears to be 32-bit. `cvt.l.s`/`cvt.s.l` at `0x8008c648`/`0x8008c700` (libultra `__f_to_ll` family?).

### Code segment and function boundaries (2026-09-28, `tools/find_functions.py`)
- Boot: `0x80000400` sets `$sp = 0x800A2830` and does `jr $t2` to `0x80000450`. That routine copies ROM `0x1120–0xAF4B0` to `0x80000520` (a copy onto itself, since IPL3 already loaded it), clears from `0x800AE8B0` up to `0x80400000` (BSS), then calls `func_8002F4D0`. **The loaded image is ROM `0x1000–0xAF4B0`, VRAM `0x80000400–0x800AE8B0`.**
- **CPU .text is ROM `0x1000–0x98BF0`, VRAM `0x80000400–0x80097FF0` (size `0x97BF0`).** RSP code follows: rspboot (`0xD0` bytes, `0x80097FF0–0x800980C0`, referenced by lui/addiu at `0x80007148`/`0x80084C40` and from data at `0x8009A2D8`), then F3DEX2 text from `0x800980C0` (the first COP2 word). Parts of it decode as CPU code and must not be treated as functions.
- Data inside .text: the build tag string `v07Apr99.1553` at `0x80000510–0x8000051F` (referenced from code at `0x8002F2F4`).
- **1374 functions**: 932 found through `jal`, 23 through lui/addiu pointers, and 418 only in gaps between functions (only reachable through pointer tables), plus the entry. The cross-check figure of ~878 named + ~500 statics is consistent. Every function start follows a `jr`/`j`/`b` plus delay slot or padding. 64 jump tables, all `sltiu`-bounded but one. All agree with N64Recomp's own table-size rule.
- One fallthrough: `0x8008D400` (`jal 0x80095990` with `a0 = 0`, i.e. destroy-self, never returns) runs into `0x8008D410`, so its extent covers it (size `0x18`, not `0x10`). One overlap: `0x8008D044` (returns via `jr $s2`) sits inside the exception handler `0x8008CB10`'s extent, which branches around it. We don't reproduce the cross-check's "143 split-function fallthroughs". Our control-flow traversal needs no splitting, so that figure likely reflects their method, not the ROM.
- The OS/libultra range starts **before** `0x8008C000`: cache ops appear at `0x80087CC0` and `0x80088AD0`.
- 12 functions can't be translated by N64Recomp: cache/TLB/eret/non-Status COP0, and `trunc.l.d`/`trunc.l.s` in `0x8008C560`/`0x8008C57C` (`__d_to_ll`/`__f_to_ll`-like). They are `ignored` in `recomp.toml` and must come from the runtime.

### N64Recomp behaviour that matters (pinned `ffb39cd`)
- Symbol file (`Context::from_symbol_file`, `src/config.cpp`): `[[section]]` needs `rom`, `vram`, `size`, `name` (optional `got_address`, `relocs`). Each `functions` entry needs `name`, `vram`, `size`. Without `relocs` the section is non-relocatable and addresses are emitted as literals. `entrypoint` renames the function at that vram with ROM `0x1000` to `recomp_entrypoint`.
- A function's C is its word range, and nothing more. **A function that falls off its end simply returns**, so fallthrough functions must cover their continuation. Branches out of a function are only legal to another function's start (tail call); otherwise it warns and emits a `goto` to a missing label. A `jal` to an unknown in-section address creates a `static_*` function. `cargo xtask recomp` fails on either.
- Jump-table size = consecutive entries that fall inside the function's own range (stopping at the next table), so function extents must include every case label.
- `get_cop1_cs()` returns **only the rounding-mode bits** of FCR31. Flags and cause bits read back as 0. Code that tests them after a conversion, e.g. `0x8008C598` (`andi 0x78` after `cvt.l.d`), therefore behaves differently under the recomp than on hardware. **The oracle is not hardware-exact there**; such functions will need an emulator snapshot or a different reference.
- In 32-bit FPU mode, odd FPRs are written through `ctx->f_odd[(n-1)*2]`, i.e. the high word of `f(n-1)`. `f_odd` must point at `&ctx->f0.u32h`.

### ROM asset loading path (2026-09-28, session 3; `tools/xref.py`)
- libultra PI: `func_80087D70` = `osPiStartDma(mb, pri, dir, devAddr, dramAddr, size, mq)` (writes OSIoMesg type 0xB/0xC, fields +2/+4/+8/+0xC/+0x10, piHandle 0; `osJamMesg`/`osSendMesg` on `osPiGetCmdQueue()` = `func_8008C900`). `__osPiDevMgr` at `0x800A7B80`, created by `func_8008BDC0` (`osCreatePiManager`), which stores `func_800944E0` (`__osPiRawStartDma`) and `func_800945C0` (`__osEPiRawStartDma`, reads `__osCurrentHandle[]` at `0x800A7BA0`). Other names inferred from the same function: `func_800880E0` osCreateMesgQueue, `func_8008B810` osCreateThread, `func_8008B960` osStartThread, `func_80087E80` osRecvMesg, `func_80087CC0` osInvalDCache **(med confidence; by call shape)**.
- Only three game functions call `osPiStartDma`: `func_80006F60` (blocking read, queue `0x800D9BF8`), `func_80007594` (audio DMA callback with 0x400-byte buffer cache; reached only indirectly) and **`func_80011B18(devAddr, dram, size)`**, the game's ROM read (one-time init `func_8002E034`, then polls `osRecvMesg` non-blocking, calling `func_80008F28` while waiting).
- `func_80011CDC(rom, dram, size)` splits into 0x800-byte `func_80011B18` reads; `func_80011D60` is the same over `func_80011BDC` (small/unaligned reads for headers). Every asset loader goes through these.
- **Asset blocks** (same four blocks as the PC `out_*block.bin` files; they tile ROM `0x0102ABB0`–`0x01FF30F0` exactly, apart from alignment padding). Each block starts with `u32 count`, then an offset table relative to the block start; the entry after the last gives the end.

  | Block | ROM base | Count | Table | Loader |
  |---|---|---|---|---|
  | texture | `0x0102ABB0` | 1648 | (pixels, palette) pairs, palette 0 if none (138 entries) | `func_800304AC(idx, &p0, &p1)` via cache `0x800D9E00`; `func_80030328` reads both parts 0x40-aligned. `func_8003043C` reads the count into `0x800DB890` and **hangs (`b .`) if count > 1700**. |
  | spline | `0x012C7F30` | 91 | single offsets | `func_80030174`; 0x10-byte header, pointer at +0xC relocated to +0x10 |
  | sprite | `0x013307F0` | 173 | single offsets | `func_8002FF38`; reads a 0x14-byte header, tests byte +4 == 2 |
  | model | `0x0141E200` | 307 | (mask, model) pairs | `func_800305E8(idx)` |
- Model load (`func_800305E8`): the mask (`mask..model`) goes into the buffer at `[0x80114528]`. The model goes at the heap cursor `func_8002FAFC()`, aligned to 8. If its first word is `"Comp"`, the compressed payload is DMA'd to `(heap_end [0x800D9DBC] - (csize-12)) & ~7`, i.e. the top of the heap, and `func_80011940(src, dst)` decompresses it to the cursor. Out of space sets `0x800A2864 = 1` and returns 0. Then every model word whose mask bit is set (bit 31-(i&31) of mask word i>>5, MSB first) is relocated: top byte `0x0A` means a texture reference (`func_800304AC(word & 0xFFFFFF, &word, &word+4)` writes the pixel and palette pointers), otherwise nonzero words get `+ model base`. It then checks the tag (`Modl` `Trak` `Podd` `Part` `Scen` `MAlt` `Pupp`), returning model+4, or calls `func_800827C0` (error). 92 of the 307 models are compressed.
- **Texture descriptors live in the models, not the texture block.** On N64 they sit relative to the relocated `0x0A00_iiii` reference word R: **format u16 at R−0x2C, width u16 at R−0x28, height u16 at R−0x26**, and width×4 / height×4 u16s at R−0x34/−0x32 (these agree everywhere). This is the PC MaterialTexture layout (blender-swe1r facts: format +0x0C, width +0x10, height +0x12, reference +0x3C) with the reference 4 bytes earlier. **The PC offsets read as format 0 on N64.** The loader writes the pixel pointer at R and the palette pointer at R+4.
- Format codes are `(G_IM_FMT << 8) | G_IM_SIZ`. Only five occur: 0x200 CI4 (1378 textures), 0x201 CI8 (90), 0x3 RGBA32 (45), 0x400 I4 (70), 0x401 I8 (20). Palettes are exactly 32 bytes for CI4 and 512 for CI8 (16/256 RGBA5551 BE entries), and absent otherwise.
- **Texture data is linear rows padded to 8 bytes** (one TMEM line; e.g. 74×47 CI4 has 40-byte rows). Level 0 is first; 35 textures are followed by a mipmap chain (each level halves, rows padded again; e.g. 32×64 CI4 = 1024+256+128+64+32). With that rule, all 1603 textures referenced by the 4095 in-range references have a fitting descriptor (1568 exact, 35 mipmapped). 2 textures have two different descriptors, one of which fits. 45 textures are referenced by no model (maybe by code or sprites). 31 references have index ≥ 1648; `texture_get` gives them null pointers.
- Not verified from N64 code: where the game turns the descriptor into `G_SETTIMG`/`G_SETTILE`, the TLUT type (RGBA16 assumed), and how I/IA texels are combined. Decoded PNGs look right (skies, pod parts, flags with alpha).
- ROM `0x100000`–`0x0102ABB0` is not these blocks; it looks like VADPCM audio (not examined yet).

### Asset heap (session 4; `game::heap`)
A bump allocator with a stack of levels.
- `heap_init` (`func_80030FF8`, called once from `func_80031324`): start `[0x800D9DB8] = 0x8014D7E0` (a constant). End `[0x800D9DBC] = [0x80114538] − 2·(reserve & ~0x3F)`, where reserve = 0 if `osMemSize` < 8 MB and 4 × 640 = 0xA00 otherwise, so the end is `[0x80114538] − 0x1400` with the Expansion Pak. `cursors[0] = start` and slots 1..9 = 0. **Nothing found writes `0x80114538`** through lui/addiu (probably a struct base), so its value is unknown.
- **Cursors:** 10 words at `0x800D9DD8`, one per level, zero after the last in use; they end exactly where the texture cache (`0x800D9E00`) begins. The level is at `0x800A2868`. `heap_set_level` (`func_8002FA00`) sets the level to `a0`, copies `cursors[a0−1]` into `cursors[a0]` and zeroes the slots above.
- **heap_set_level in full** (session 5, from the disassembly; not ported): if `level < a0` is false and `[0x800A68A0] == 0` it first calls `func_8002E034` and sets `[0x800A68A0] = 1` (so only the first call does). Then `level = a0`, `cursors[a0] = cursors[a0−1]`, `texture_cache_trim(cursors[a0−1])` (`func_80030574`: zeroes every cache word above that cursor, so textures in the freed area are forgotten), `func_80007E80(1)`, zeroes the slots above `a0` (while `level < 9`), and clears `0x800A2864`.
  - `func_80007E80` (only caller: heap_set_level) walks two slot tables, 23 words at `0x8009A32C` and 8 at `0x8009A388`. For each nonzero slot it calls `func_80007A80` and `func_80007A44` and zeroes the slot. If any slot was set it zeroes `[0x800B05B0]` and calls `func_80008F28` until **another thread** raises that word to 5. `func_80007CE4` (a handle lookup) indexes the same tables. Probably stop-all-sounds and wait a few ticks (**guess**).
  - `func_8002E034`: `func_8002DFB0(0x3F, 0)`; while `[0x800A2690]`: `func_8002E2FC`, `func_8002E124` (osRecvMesg and more); `[0x800A2698] = 0`; `func_8002DFB0(0x40, 0)`; `[0x800A2690] = 1`. Also called by rom_read_dma and `func_80011BDC`. A handshake with another thread; effects unknown.
  - **Why it isn't ported:** neither callee can be doubled honestly. A double for `func_80007E80` could only cover "all 31 slots empty", where it does nothing (a stand-in, not a contract). `func_8002E034` can be avoided with the precondition `[0x800A68A0] != 0`, but its first-call effects are unknown. `func_80008F28` (under both) is the blocker, as it is under the ROM reads.
- `heap_cursor` (`func_8002FAFC`) = `cursors[level]`. `heap_set_cursor(p)` (`func_8002FAC4`) stores `p` there, then calls `heap_check` (`func_8002FC80`), a **stripped assert**. It finds the last nonzero slot by walking from slot 1 to the first zero and compares it (`sltu`) with `[0x80114538]`, but both outcomes just return. QUIRK: the walk is unbounded and runs into the texture cache if slots 1..9 are all nonzero. `heap_free` (`func_8002FC58`) = `[0x800D9DBC] − heap_cursor()`, 32-bit, can go negative.
- **Mask buffer:** `[0x80114528]` is set once, by `func_80030B90` at init: the cursor rounded up to 64, after which the cursor moves up by 640×15×32 (8 MB) or 320×15×32 bytes, +0x40. So it is a 0x4B000-byte buffer (0x25800 with 4 MB) near the heap start. Six other functions read it; the model loader uses it for relocation masks.
- **The top, `0x80114538`, is the third framebuffer.** `framebuffers_init` (`func_80039A30`) writes three framebuffer addresses at `0x80114530..38`: `fb[k] = (osMemSize | 0x80000000) − (k+1)·(w·240·bpp + x) + x`. With 8 MB: w = 640, 4 bytes/pixel, x = 2·4·640 = 0x1400, giving fb = 0x8076A000, 0x806D2C00, **0x8063B800**. With 4 MB: w = 320, 2 bytes/pixel, x = 0, and the third is 0x8038F800. So the heap end is **0x8063A400** with the Expansion Pak and **0x8038F800** without, and after `func_80030B90` the level-0 cursor is 0x80198820 (8 MB) or 0x80173020 (4 MB). These are derived from code. The one guess is how much other boot code allocates before the first model load (the loader tests start from those cursors).

### Texture and model loaders (session 4; `game::loader`, all ported)
- `texture_get(idx, &pix, &pal)` (`func_800304AC`): out of range → both 0. On a miss it reads the 3 table words (pix, pal, next pix) with `rom_read_small` and calls `texture_read`, then sets **`cache[idx] = &pix`, the caller's word**. A hit copies `cache[idx][0]` and `[1]`. So the cache points into the model that first referenced the texture. QUIRK: if that first load ran out of heap, every later reference gets the null pair; for palette-less textures, the "palette" copied is whatever that model had at `+4`.
- `texture_read(hdr, &pix, &pal, _)` (`func_80030328`): needs `heap_free() ≥ size + 0x80` (signed; size = next − pix), otherwise it stores 0 to `*pal` then `*pix` and sets `0x800A2864`. Pixels go to align64(cursor), the palette to the next 64-byte boundary, and the cursor ends after the last part. **With no palette, `*pal` is not written at all.**
- `model_load(idx)` (`func_800305E8`), beyond the earlier facts:
  - The index is compared as a 64-bit register, but it sits in `s0`, which the first `rom_read_small` saves with `sw` and restores with `lw`. **In effect it is the sign-extended low word**: `0x1_0000_0005` loads model 5.
  - QUIRK: the mask and the 12-byte header are read (to the mask buffer, and to align8(cursor)) **before any space check**, so near a full heap the header write lands past the heap end.
  - QUIRK: **the space checks ignore the decompressor's window.** The Comp payload goes at `(end − payload) & ~7`, and the only requirement is that it lies above the output. With less than 0x1000 bytes between them the 4 KB window below the payload overlaps the output, and the model comes out corrupt: 56 of 98 sampled tight layouts did, 3 with a bad tag (which reaches `func_800827C0`). The `& ~7` can push the window 4 bytes into the output even at exactly 0x1000 of slack.
  - It sets `[0x800A2848] = 1` and zeroes stats at entry. At the end, `0x800D9DC0` = cursor before the model, `DC4` = after it, `DC8` = model bytes, `DCC` = texture bytes (`DD0` stays 0). An unknown tag calls `func_800827C0` (not understood) and, if that returns, returns the model base instead of base+4.
- Checked against an independent statement of the format (`crates/difftest/tests/loader.rs`, `expect`): all 307 models from a fresh 8 MB heap give exactly the predicted relocated words, texture placement, bytes, cursor and stats. So the relocation and texture facts above hold for every model.
- `texture_block_init` (`func_8003043C`, ported session 5): reads the count into `0x800DB890`, then zeroes **all 1700 cache words whatever the count** (`0x800D9E00` up to the count word itself). The count test is `slti 0x6A5`, signed: 1701 or more hangs, negative counts don't. N64Recomp emits the `b .` as a single `pause_self(rdram)` call; in its C, a `pause_self` that returned would fall through into the clearing loop.
- `texture_cache_trim` (`func_80030574`, ported session 5): zeroes each cache word `w` with `a0 < sext(w)`, a full 64-bit `sltu`, so a zero-extended `a0` clears every KSEG0 entry. Called only by heap_set_level.

### Sprites (session 4; `assets::Sprite`, `cargo xtask extract`)
**From the loader** (`sprite_load(idx)`, `func_8002FF38`, ported session 5; all 173 sprites checked against `assets::Sprite` in `crates/difftest/tests/sprite.rs`), the sprite block is `u32 count` then single offsets. It is reached through `func_80030154` (a thunk with a frame, ~100 call sites); `func_80030130` (`sprite_load(a1)`) is referenced by nothing in code or data. Beyond the points below: the index is in effect the sign-extended low word (like model_load, `s0` passes through rom_read_small's `sw`/`lw`). **There is no space check at all.** With a palette but a page count ≤ 0, the palette size comes from "page 0's offset" at header + 0x18, a word that was never loaded (QUIRK). A negative count still adds `8 * count` to the cursor, moving it back into the header (QUIRK). The page count is re-read from the header after every page.
- It reads the 0x14-byte header to the **unaligned heap cursor**, then `count = (s16) +0xC` 8-byte page entries right after it (at `+0x14`; the loader ignores `+0x10`). It stores a pointer to them in `+0x10`.
- If the palette offset `+8` ≠ 0, the palette (from `+8` up to page 0's offset) goes to the next 16-byte boundary and `+8` becomes its pointer. Then each page's texels (from its offset up to the next page's, the last up to the sprite's end) go 16-byte aligned, and the entry's `+4` becomes the pointer. The cursor ends after the last page; it returns the header.
- **The `+4 == 2` test:** if the format is CI and `+8` is 0 it returns at once with the cursor set back to the header. So the header isn't reserved, and the next allocation overwrites it (QUIRK). No sprite in the USA ROM takes this path: every CI sprite has a palette.
- There's a pointless loop that counts `s1` up to the page count and then zeroes it.
- QUIRK: all reads use `rom_read_small` (word `sw`s), so an unaligned cursor would fault; model/texture ends keep it word-aligned in practice.

**From the data** (all 173 checked in `crates/assets/tests/rom.rs`):
- Header: `+0` u16 width, `+2` u16 height, `+4` u8 `G_IM_FMT`, `+5` u8 `G_IM_SIZ`. The format code is the same `(fmt << 8) | siz` as texture descriptors: RGBA32 ×18, CI4 ×40, CI8 ×48, I4 ×66, I8 ×1. `+6` u16 is always 0. `+0xE` u16 is always 32 (maybe the maximum page height; **guess**). `+0x10` u32 is always 0x14, the page table offset.
- Page entry: u16 width, u16 height, u32 offset. **Pages tile the image row-major** in rows of page[0]'s height: `rows = ceil(h / page_h)`, `cols = pages / rows`. The last row and column are smaller (e.g. 640×240 CI8 = 10×8 pages of 64×32, the last row 64×16). Every page is exactly `stride(width) × height` bytes, with rows padded to 8 bytes like textures. The data follows the page table with no gap, palette first. Palettes are exactly 32 (CI4) or 512 (CI8) bytes.
- Oddities: sprite 110 is 1×1 I4 with **no pages** (20 bytes, header only). Sprite 145 declares 193×84 but its pages are 3 columns of 64 = 192 wide (one pixel column uncovered).
- **Still guessed:** the palette is RGBA5551 and I4/I8 map to grey plus alpha (as for textures; the TLUT/combine state isn't located). The row-major order is confirmed visually (HUD gauges and panels join seamlessly across page seams) and by the short last rows. Sprite 160 (640×240) is a collage of horizontal scenes; how it's drawn (e.g. strip by strip) is unknown. Nothing here has been checked against the drawing code, which isn't located.

### Depth-0 leaves ported in session 5 (`game::util`, `game::misc`)
Subsystems unknown unless stated; `game::misc` holds them by address until they are. What they show about the data:
- `0x80000520`/`52C`/`538`: setter of `[0x8009A270]`, clear and getter of `[0x8009A280]`. `0x80005B1C`/`B44`: set/get `[0x8009A290]` (selector 3) and `[0x8009A28C]` (selector 5), others ignored or −1, selector compared as a 64-bit register. `0x80005AFC`: decrement `[0x8009A29C]` if positive.
- **Object table** at `0x800AF4C0`: 300 words, count in use at `[0x8009A2A0]`, both cleared by `func_80005B80`. `func_80006D5C(id, kind)` finds an object `o` with `[o+0x100]` (flags) bit 31 clear, `flags & 0xF == kind` and `[o+0x124] == id`. `func_80006E50`/`E60` set/clear flag bits. The count isn't bounded by 300 (QUIRK).
- **Slot tables** `0x8009A32C` (23 words) and `0x8009A388` (8): `func_80007CE4(handle)` looks up `A[byte 2]` for byte 3 ∈ {0, 1}, else `B[byte 3]`, if bit 15 is set, and returns `[0x800AFA54]` otherwise. Unbounded indices (QUIRK). func_80007E80 walks the same tables (Asset heap, above). `func_80007A44` zeroes `+0x18` of eight 0x20-byte records at `0x800D2038` if `[0x8009A2B8]`.
- `func_80007710` = **audio_dma_new** (med): `*a0 = 0x800AFAC0`, returns the audio DMA callback `0x80007594`, the shape of libultra's `ALDMANew`.
- Twelve are empty (`jr ra`), most spilling their arguments to the caller's slots, presumably compiled-out debug hooks. They're still ported, since callers reach them.
- N64Recomp's `sra` shifts the full 64-bit register, then truncates: for a non-canonical input, upper-half bits reach the low word (undefined on hardware). Ports follow the C.

### Asset compression: "Comp"/"Wolf" LZSS (`func_80011940`)
Header (12 bytes, read by the loader): `"Comp"`, `"Wolf"` (all 92), `u32` decompressed size (BE). The stream follows at +12; `func_80011940(src = stream, dst)` returns the end of the output in `v0`. It does not know the output size and stops only at the terminator.
- Ring buffer: 4096 bytes at **`src - 0x1000`**, the memory just below the compressed input, **never initialised** (QUIRK). The write position starts at 1. Every output byte is also stored at the write position, which then advances mod 0x1000.
- Loop: read a flag byte and consume its bits **LSB first**. Bit 1 = literal: copy one input byte. Bit 0 = reference: two bytes `b0 b1`, `offset = ((b0 & 0xF) << 8) | b1`, `length = (b0 >> 4) + 2` (2..17). **Offset 0 ends the stream at once**, even in the middle of a flag byte. Otherwise copy `length` bytes from window `(offset + k) & 0xFFF`, each read before the write of the same step, so overlapping references repeat.
- Per byte the order is: read the window, write `dst`, write the window. So if output, window and input overlap in RDRAM, the game's result depends on that order. The heap placement can make them overlap when heap space is tight; a port must keep the order.
- Checked on all 92 compressed models (session 3): each decompresses to exactly the declared size and begins with a valid tag, **no reference ever reads an unwritten window byte** (so the uninitialised window doesn't matter for real data), and 0–17 bytes of padding follow the terminator.
- Register leftovers matter for the difftest: `a2 = 1`, `a3` = the flag bit that held the terminator, `t0` = the last flag byte, `$at` keeps its input value if the terminator is the first token, and `s0`–`s2` are restored sign-extended from their low words.

### How ports call other functions (decision, session 4)
**Ports call callees through N64Recomp's C symbols** (`func_XXXXXXXX`, recomp signature), declared once in `game::imports` and called with `game::recomp::call(imports::func_X, &mut mem, ctx)`. Ports never call another Rust port directly. The linker decides what each symbol is:
- **In the tests**, the oracle defines *every* recompiled function: the generated C if it is listed in `crates/oracle/functions.txt`, otherwise a generated stub (`oracle_callee`) that runs the test double installed on the current thread (`oracle::doubles::install`) or traps. The recompiled caller and its Rust port call the same symbol, so they always reach the same callee: C for verified callees, a double for unverifiable ones.
- **In the game build (later)**, each symbol is either the recompiled C or a Rust port exported under that name (opt-in `#[no_mangle]`, with that function's C left out). Recompiled callers and Rust callers then agree automatically, the same way N64Recomp's own patch overrides work.

Why this and not a runtime function table: one resolution mechanism for C and Rust callers, so they can't disagree. It needs no global mutable state, and the stubs build.rs already generated serve both sides. The costs:
- Anything that links `game` must define the imported symbols. `game`'s own unit tests get aborting definitions (`cfg(test)` in `recomp_imports!`). **Tools must not depend on `game`**: `lzss` moved to `assets::lzss` for that reason, and `assets`/`xtask` no longer link `game`.
- In the difftest binary a callee is always the C (or a double), never the callee's Rust port, so each port is verified against C callees only. Running all ports together needs a separate "swap" binary (C of ported functions left out, Rust exported under the C names). That is the game build's job, or a later test.
- A double can only replace a function that isn't compiled in (`install` refuses otherwise).

Rules for writing a port with calls:
- `jal` doesn't write `$ra` in N64Recomp. `sw $ra` saves the caller's incoming value, and `lw $ra` gives it back sign-extended.
- At every call, the whole register file in `ctx` must match the C's, since the callee sees all of it. Values kept in Rust locals are written back before the call and reloaded after.
- Each call through a stub is recorded with its GPRs at entry, and `difftest::compare` requires the C and Rust runs to make the same calls with the same registers.

### Test doubles for the ROM reads (session 4)
`difftest::rom` has doubles for `rom_read` (`func_80011CDC`) and `rom_read_small` (`func_80011D60`) that copy from baserom.z64, read at test time. They replace the unverifiable PI/`osRecvMesg`/`func_80008F28` chain below them.
- **Reproduced:** exactly `size` bytes, or nothing if `size <= 0`. `s0`–`s3`, `ra` and `sp` come back as sign-extended low words (their `sw`/`lw` pairs). All other caller-saved state (`at v0 v1 a0–a3 t0–t9 hi lo f0–f19`) gets deterministic pseudo-random values, so a port that relies on something surviving a call diverges.
- **Not reproduced:** stack contents below `sp` (the real chain writes frames there), PI/message-queue state, and the side effects of `func_80008F28` between 0x800-byte chunks.
- **Refused (the test aborts):** non-canonical arguments; ROM reads past the end; for `rom_read`, anything that isn't a clean PI DMA (RDRAM 8-aligned, ROM 2-aligned, even length); for `rom_read_small`, unaligned words (it does `lw` from the PI bus and `sw` to RDRAM). Every asset-block entry in the USA ROM starts 4-aligned and is a multiple of 4 long, so the loaders never hit these. PI DMA's behaviour for odd or unaligned transfers is not modelled.
- **Patched ROMs** (session 5): `difftest::rom::Image` is baserom.z64 with byte ranges replaced, without copying the 32 MB image. Both doubles take one; `install_rom_image` installs both.

### Which functions have doubles (decision, session 5)
`crates/oracle/doubles.txt` lists every function a test may replace, as `contract` or `stand-in`. The oracle's build.rs checks it against funcs.h and functions.txt, `oracle::doubles::install` refuses unlisted names, and `cargo xtask next-function` reads the same file.
- **contract:** reproduces the function's observable effects as far as NOTES documents them (rom_read, rom_read_small). next-function counts it as a satisfied callee, flags targets "via doubles", and `--toward` stops searching below it.
- **stand-in:** gets a test past a call with no behavioural claim (model_error). Doesn't count for readiness.

### Runtime hooks (session 5)
Ports that must do what generated code does with the runtime call the same hook: `game::imports::runtime` declares them (`pause_self` so far), and whoever links `game` defines them. That's the oracle's stub runtime in tests, where every hook traps. Tests of code that reaches a trap run in a child process (`crates/difftest/tests/texture_block_init.rs`, like `crates/oracle/tests/traps.rs`).

## Upstream projects (checked 2026-09-28)

| Project | Licence | State / how we use it |
|---|---|---|
| sp00nznet/racer | **none** (all rights reserved) | Last commit 2026-05-31. Boots to main loop, DLs reach RT64, **no visible frames, no audio, not playable**. Needs *unpublished* local patches to N64ModernRuntime. Generated C isn't committed. MSVC-only, hardcodes `C:/vcpkg`. **Reference only: don't copy code.** |
| N64Recomp | MIT | Pinned locally at `ffb39cd` (2026-05-27) in `third_party/`. Accepts `symbols_file_path` + `rom_file_path` (no ELF needed); its README is stale on this. |
| N64ModernRuntime | GPL-3.0 | CI builds with clang-cl + Ninja. Linking it makes the combined binary GPL-3.0. |
| RT64 | MIT | |
| SW_RACER_RE (PC) | AGPL-3.0 | 71% analysed. Use names and facts only, not code. Asset formats: louriccia/blender-swe1r (GPL-3.0). |
| decompals/ultralib | none | Reading/identification only. |
| OpenSWE1R | GPL-2.0 | Dormant since 2020. |

Findings from sp00nznet/racer we can use as facts (addresses, not code):
- One code section: ROM `0x1000`, VRAM `0x80000400`, size `0x98000`. **No overlays** (§10 Q7).
- About 880 functions, including 143 split-function fallthroughs that N64Recomp needed help with. libultra/OS lives at about `0x8008C000+`.
- Runtime needs: PI handle at `0x800A7BC0`/`0x800A7FC0`; VI state at `0x800A7F00`; `osMemSize` at `0x80000318`.

### N64Recomp memory model (SPEC §5.3; from `include/recomp.h` @ ffb39cd)
- `rdram` is indexed by `vaddr - 0xFFFFFFFF80000000` (addresses are sign-extended 64-bit `gpr`s).
- Stored as **host-endian 32-bit words**. Word access is direct; halfwords XOR the address with 2; bytes XOR with 3. `LD`/`SD` are two `MEM_W`s, high word first.
- Signature: `void f(uint8_t* rdram, recomp_context* ctx)`. `ctx` holds `gpr r0..r31` (u64), `fpr f0..f31` (union of double / {fl,fh} / {u32l,u32h} / u64), `hi`, `lo`, `f_odd`, `status_reg`, `mips3_float_mode`.

## Environment (this machine)
- Windows 10, Rust 1.92 stable-msvc, CMake 4.1.
- MSVC: VS 2022 Community (14.42) and VS 2026 Community (14.51). Ninja ships with both.
- **clang-cl is not installed** (VS "C++ Clang tools" component). May be needed for N64ModernRuntime/RT64.
- clippy is not installed for the toolchain (`rustup component add clippy`).
- Python 3.12; project venv at `.venv/` with rabbitizer 1.16.2, spimdisasm 1.42.4, splat64, and Pillow (session 3; only for looking at extracted PNGs, not used by any committed tool).
- Claude Code's Bash tool fails to parse a heredoc whose body has an odd number of `'` (a Rust lifetime or label, `'found`), even with `<<'EOF'`. Write the text to a scratch file and `cat` it instead (session 5).

## Session log

### 2026-09-28 — Session 1
**Done**
- Phase 0 complete:
  - git repo, Cargo workspace, `.gitignore` (ROM, `rom/`, `third_party/`, `generated/`, dumps, build output).
  - `crates/rom` + `cargo xtask verify-rom`: detects byte order, converts to z64, checks header name/code/entry/CRCs, **recomputes the CIC checksum from data**, and records/checks SHA-1. ROM verified.
  - N64Recomp cloned at `ffb39cd` into `third_party/N64Recomp` and built with MSVC (VS 2022): `third_party/N64Recomp/build/Release/N64Recomp.exe`.
- Phase 1 started:
  - `crates/n64mem`: 8 MB RDRAM plus typed accessors that mirror `recomp.h`.
  - `crates/oracle`: builds a C shim against the real `recomp.h` via `cc`. The §5.3 layout test passes both directions (C→Rust, Rust→C, all byte lanes, doublewords, the Expansion Pak range).
- Upstream licence check done (table above). ROM survey answered §10 Q4 (F3DEX2), Q5 (local rounding-mode idiom only) and Q7 (no overlays).

**Surprises**
- The ROM file was v64 despite its `.n64` extension.
- sp00nznet/racer has **no licence**, renders no frames yet, and depends on unpublished runtime patches. So Phase 4 will be **option B (emulator snapshots)** for now.

**Decision (user, 2026-09-28): build our own symbol file.**
- Function boundaries come from our own analysis (spimdisasm/rabbitizer over ROM `0x1000–0x99000`).
- sp00nznet/racer's published addresses serve only as a cross-check. None of its files are copied into the repo or used as build input.
- The oracle gets its own minimal stub runtime. N64ModernRuntime (GPL-3.0) is not linked into tests.

**Suggested next step**
- Write `tools/find_functions.py` to generate `symbols/racer.syms.toml` from our own analysis, run N64Recomp into `generated/`, compile one leaf function into `oracle` with a minimal stub runtime, and port it to Rust. That completes Phase 1.

### 2026-09-28 — Session 2

**First port target: `func_80000554`** (written before coding)
- Zeroes `a1` consecutive 32-bit words starting at `a0` and does nothing if `a1 <= 0`; `a1` is compared as a signed 64-bit register. It is a compiler-unrolled loop: first `a1 & 3` single stores, then four stores per iteration.
- It is a leaf with no floats, calls or stack. Callee-clobbered registers are left as the loop leaves them (`v0` = `a1 & 3`, or 0 if `a1` is a multiple of 4; `v1`, `a2`, `a3`, `t6`, `t7`, `t8`), and the port has to reproduce them exactly.

**Done**
- `tools/find_functions.py`: our own boundary analysis (rabbitizer). Recursive descent from the entrypoint through `jal`s, tail calls and jump tables (the same lui/addiu/addu/lw pattern N64Recomp recognises), then lui/addiu pointers, then gap code, then data words. Extents are iterated to a fixed point. It writes `symbols/racer.syms.toml` and `symbols/functions.csv`, keeping hand-edited rows, and reports ambiguities. It is deterministic. `cargo xtask find-functions` runs it.
- `recomp.toml` + `cargo xtask recomp`: checks `baserom.z64` against `rom/EXPECTED.sha1`, wipes `generated/`, runs N64Recomp, and fails if it creates statics or warns. The result is 1362 C files, byte-identical across runs, with no hand edits. Every generated file spans exactly its symbol's `[vram, vram+size)`.
- `crates/game`: `recomp::RecompContext` (`#[repr(C)]` mirror of `recomp_context`) plus `s32`/`addu`/`sll` helpers matching `S32`/`ADD32`, and the first port, `util::func_80000554`. Ports are not `#[no_mangle]`, so they don't clash with the C symbols in tests.
- `crates/oracle`: `build.rs` compiles `generated/<name>.c` for each name in `functions.txt`. Unselected callees get generated trapping stubs. It builds with `/fp:strict` (or `-frounding-math`). It adds `c/stub_runtime.c` (every runtime hook traps via Rust `oracle_trap` → abort, with no MSVC dialog) and `c/ctx_shim.c`. Tests: context size/offsets/union halves/`f_odd` against the C compiler, and each stub aborts loudly (child-process test).
- `crates/difftest`: `State` (8 MB RDRAM pre-filled with a fixed pseudo-random pattern, so zero stores are visible, plus a boxed context) and `compare()`, which runs C and Rust on clones and diffs every GPR, `hi`/`lo`, FPR bits, status/mode, `f_odd` and all of RDRAM. `tests/func_80000554.rs` has 2×512 proptest cases plus edge cases: counts 0, negative, `i32::MIN`, 1–9, the end of 8 MB, a whole MB, 64-bit `blez`, and non-canonical upper halves. It also checks memory against an independent statement of the behaviour.
- **Phase 1 done**: `cargo test` runs a C-vs-Rust differential test on `func_80000554`. It passed first time, and deliberately broken ports (a dropped store; a wrong `v0`) are caught with readable diffs.

**Surprises**
- The loaded image runs to ROM `0xAF4B0`, and .text ends at `0x98BF0`, not `0x99000`. The last `0x410` bytes before `0x99000` are RSP code, part of which decodes as plausible CPU code.
- N64Recomp can't translate `trunc.l.*`. It also drops FCR31 flag bits (see Facts), so the oracle is not a hardware reference for FCR31-flag code.
- The ported function's "return value" in `v0` is a dead loop counter (0 when `a1` is a multiple of 4). Ports must keep such leftovers because the difftest compares whole register files.
- A plain `abort()` from C risks an MSVC error dialog in a non-interactive run. Traps go through Rust `process::abort` instead.

**In progress / not done**
- `cargo xtask next-function` (SPEC §7) doesn't exist yet. `depth` in functions.csv is filled from direct calls and tail calls, and is empty for functions with indirect calls or on cycles.
- No names yet (all `func_XXXXXXXX`). The 12 non-recompilable OS functions will need names, which N64Recomp matches against its built-in lists, before a runtime can supply them.

**Suggested next step**
- Phase 2: `tools/callgraph.py` (or extend find_functions' call data) with depth, flagging indirect calls. Identify libultra by signature (the range starts ≤ `0x80087CC0`). Then port more depth-0 integer leaves (332 leaves have no floats; see functions.csv) using the same difftest pattern, and add `cargo xtask next-function`.

### 2026-09-28 — Session 3

**Done**
- **Call graph:** `find_functions.py` writes `symbols/callgraph.csv`, one row per call site: `call`, `tail`, `fallthrough`, plus `jalr`/`jr` for indirect sites with unknown targets. Depth is computed over the SCC condensation of the direct edges, so every function has one (324 were empty; max 27). Mutually recursive functions share a depth, and indirect edges don't count.
- **`cargo xtask next-function`** (`xtask/src/next_function.rs`): a function is ready when it is `recomp`, not ignored, has no indirect sites, and all its callees are `rust_verified`/`lifted`. Ranking: `--subsystem`, depth, outside the OS range, integer before float, then FCR31. It flags float, `get_cop1_cs` (FCR31 read: the oracle has rounding bits only) and the OS range. `--toward func_X` limits it to what X reaches and lists what's blocked below it.
- **`tools/xref.py`**: `dis`, `callers [-r N]`, `callees`, `refs ADDR [ADDR2]`, `pi`. Used to trace the asset path (Facts above).
- **Asset path found** (Facts: "ROM asset loading path"). From PI registers → `osPiStartDma` → the game's ROM read (`func_80011B18`/`CDC`/`D60`) → the four block loaders. The blocks are the PC version's texture/spline/sprite/model blocks, same table format, tiling ROM 0x0102ABB0–0x01FF30F0.
- **Decompressor ported: `func_80011940` (`comp_decompress`)**, a depth-0 integer leaf, now `rust_verified`. Its format was written in NOTES before porting ("Asset compression"). `game::asset::func_80011940` is register- and access-order-exact; `game::asset::lzss` is the slice version. `crates/difftest/tests/func_80011940.rs` covers proptest token streams (256), random-byte streams (24), terminators on every flag bit and with any length nibble, register leftovers, aliasing layouts, and **all 92 compressed models from baserom.z64**: C, port and slice version agree byte for byte, in both a roomy and a tight (overlapping) heap layout. A batched-read mutant and a wrong-`t3` mutant are both caught.
- **`crates/assets` + `cargo xtask extract`**: raw blocks, decompressed models and masks, and **1603 textures as PNG** in `extracted/` (gitignored; the command refuses to run otherwise), plus `extracted/textures.csv`. `crates/assets/tests/rom.rs` pins the invariants (counts, tiling, descriptor fits, palette sizes).
- functions.csv: 10 asset functions named (`ours`, med) and 5 PI functions (`libultra`, high). 2/1374 verified. Tests: 43.

**Surprises**
- The ring buffer is the 4 KB *below the compressed input*, uninitialised. It's harmless for real data (no stream reads an unwritten byte), but the game's heap placement (input at the top of the heap, output at the cursor) can make output, window and input overlap when memory is tight. So the port keeps the per-byte read→dst→window order.
- `a0`/`a1` are dereferenced raw, so a zero-extended `0x0000_0000_8xxx_xxxx` pointer is *non-canonical* and crashes the C (an address error on hardware). Unlike `func_80000554`, upper halves of pointer arguments are outside the domain here.
- Two "mutants" I first tried were equivalent: swapping two stores of the same byte, and `t1 == 16`, which is impossible at exit because `t1` then holds the terminator's nibble. Pick mutants that can actually fire.
- The texture descriptor is the PC layout shifted by 4 bytes, not identical. The PC offsets gave format 0 everywhere.
- `texture_block_init` spins forever (`b .`) if the texture count exceeds 1700 (QUIRK, in the notes column).
- A `*.proptest-regressions` file with three seeds for minimal synthetic cases (no ROM data) is committed, as proptest recommends.

**In progress / not done**
- Sprites (0x14-byte header, byte +4 == 2) and splines are dumped raw, not decoded. Audio (ROM 0x100000–0x0102ABB0, VADPCM-looking) not examined.
- The code that consumes texture descriptors (display-list building) isn't located, so the TLUT type and I/IA combine are assumptions.
- Loader functions (`model_load` etc.) aren't portable yet: they call the PI/OS wrappers (osInvalDCache is not recompilable) and heap functions `func_8002FAFC`/`func_8002FAC4`/`func_8002FC58`.

**Suggested next step**
- Port the heap helpers `func_8002FAFC` (cursor), `func_8002FAC4` (set cursor) and `func_8002FC58` (free space), probably tiny leaves, via `cargo xtask next-function --toward func_800305E8`. `--toward` currently reports that model_load reaches 116 functions (49 ready now), blocked below only by `func_80088538` (3 indirect calls), probably via the wait-loop helper `func_80008F28`. Then difftest `model_load`'s relocation logic with the ROM read stubbed: feed the DMA from baserom.z64 in an oracle stub, which needs a small "ROM read" hook in the stub runtime. After that, decode sprites (find their loader's use of the 0x14-byte header) for a second visual.

### 2026-09-28 — Session 4

**Done**
- **How ports call callees: decided and built** (Facts: "How ports call other functions"). Ports call the N64Recomp C symbols (`game::imports`, `recomp::call`), and the linker picks the implementation. The oracle now defines every recompiled function, as either the generated C or a stub that runs a per-thread test double (`oracle::doubles`) or traps. Calls through stubs are traced with their registers, and `difftest::compare` requires identical traces. `lzss` moved to `assets::lzss`, so `assets`/`xtask` no longer link `game`.
- **ROM-read doubles** (`difftest::rom`; Facts: "Test doubles for the ROM reads"). `rom_read`/`rom_read_small` copy from baserom.z64 at test time, restore saved registers the way the originals' `sw`/`lw` do, scramble caller-saved state, and refuse transfers the originals couldn't do cleanly.
- **7 ports, all `rust_verified`, 9/1374 in total:** the heap helpers `heap_cursor`, `heap_check`, `heap_set_cursor`, `heap_free` (Facts: "Asset heap"), then `texture_read`, `texture_get`, `model_load` (Facts: "Texture and model loaders").
  - Heap state is derived from `heap_init`, `mask_buffer_init` and `framebuffers_init` for both memory sizes; the only guess is other boot allocations before the first load.
  - The tests load all 307 models from a fresh heap and check each against an independent statement of the format. They also cover sequential loads (texture cache hits, running out of heap in 8 MB and 4 MB layouts), proptest heap limits, tight Comp layouts, out-of-range and non-canonical indices, texture out-of-heap, the unknown-tag error path (a returning double for `func_800827C0`), and every texture through `texture_get`/`texture_read`.
  - Five reachable mutants are caught, and double panics/traps now print past the harness's output capture.
- **Sprites decoded** (Facts: "Sprites"): `assets::Sprite`, `decode_sprite`, and `cargo xtask extract` writes 172 sprite PNGs and `sprites.csv`.
- Tests: 67 (was 43).

**Surprises**
- `model_load`'s index check is 64-bit, but the index's upper half is already gone: the first callee spills `s0` with `sw`/`lw`. A faithful double's register behaviour matters, not just its data.
- **The Comp space checks ignore the decompression window.** With less than 4 KB between the output and the payload, the model comes out corrupt: 56 of 98 sampled tight layouts did, and 3 reached the error path. `& ~7` makes it happen even at exactly 0x1000 bytes of slack.
- The Comp header (and the mask) are read before any space check.
- The texture cache stores the address of the *first referencing model's* pointer pair, not the texture. If that first load ran out of heap, later references get nulls.
- `heap_check` is an assert with nothing left in either branch.
- Three of my own expected values were wrong at first (a count off by one, the index truncation, the `& ~7` window edge). Each time C and Rust agreed and the cause was in the code, and the test's statement was corrected to match the understood behaviour, not the output.

**In progress / not done**
- `func_800827C0` (model_error) isn't understood; the tests that reach it use a double that just returns.
- The PI/`osRecvMesg`/`func_80008F28` chain under the ROM reads is still unverifiable (indirect calls below `func_80088538`).
- `sprite_load`, `texture_block_init`, `heap_set_level` and `heap_init` aren't ported. `next-function` still reports the loaders' neighbours as blocked, because it doesn't know that rom_read/rom_read_small have doubles.
- No "swap" build yet that runs Rust ports as each other's callees.

**Suggested next step**
- Teach `cargo xtask next-function` about doubled functions (e.g. a `doubled` column or a list in the oracle), so ports above rom_read become "ready with doubles". Then port `sprite_load` (`func_8002FF38`) against the sprite facts, plus `texture_block_init` (`func_8003043C`, its `b .` hang is a QUIRK that needs a child-process test) and `heap_set_level` (`func_8002FA00`; its callees `func_8002E034`, `func_80030574` and `func_80007E80` need checking first).
- After that, the depth-0 leaves (510 ready; mostly tiny getters/setters at `0x8000052x` and `0x80005Axx`) are cheap, bulk-verifiable progress.

### 2026-09-28 — Session 5

**Done**
- **next-function knows about test doubles** (Facts: "Which functions have doubles"). `crates/oracle/doubles.txt` lists them as `contract` or `stand-in`. The oracle build checks the list and `install` refuses unlisted names, so it can't drift from the tests. `next-function` counts contract doubles as satisfied, flags "via doubles", and `--toward` stops below them. The ranking is unchanged.
- **30 ports, all `rust_verified`, 39/1374 in total:**
  - `sprite_load` (`func_8002FF38`) plus its thunk `func_80030154` and `func_80030130` (Facts: "Sprites"). All 173 sprites checked against `assets::Sprite` (patched pointers, 16-byte alignment, cursor, nothing written above it), sequential loads, proptest cursors, alignment mod 16, out-of-range and non-canonical indices, sprite 110, no space check, and three in-memory ROM patches for the quirks.
  - `texture_block_init` (`func_8003043C`) with its hang. `pause_self` is a runtime hook the port calls too (Facts: "Runtime hooks"). The hang is tested in child processes for C and Rust.
  - `texture_cache_trim` (`func_80030574`), heap_set_level's leaf callee.
  - 25 depth-0 leaves in three groups (Facts: "Depth-0 leaves"), one difftest file per group (`leaves_80000520.rs`, `leaves_80005AFC.rs`, `leaves_80006D5C.rs`).
- `difftest::world` (the loaders' boot-derived heap, shared) and `difftest::rom::Image` (patched ROMs without a 32 MB copy). `recomp::lh`/`lbu`.
- Every port's tests catch 3–5 reachable mutants (21 tried in total). Proptest seed files written by mutant runs were deleted.
- Tests: 110 (was 67).

**Surprises**
- **sprite_load has no space check at all**, unlike the texture and model loaders. It also has two more quirks than NOTES had: a palette with no pages takes its size from an unloaded word, and a negative page count moves the cursor back.
- N64Recomp turns `b .` into one `pause_self()` call, not a loop. Its C would carry on into the following code if that ever returned, so the port mirrors that.
- texture_block_init clears 1700 cache words whatever the count. The count only decides whether it hangs.
- `func_80030130` has no references anywhere: not in code, not as a data word.
- All ports passed their difftests first time. The mutants are what showed the tests have teeth.

**In progress / not done**
- **heap_set_level not ported** (Facts: "Asset heap"): `func_80007E80` and `func_8002E034` can't be doubled honestly. Both bottom out in `func_80008F28` and other threads.
  - **Decision (after the session, user deferred to Claude):** no `rust_draft` with stand-ins. It is one function, and a port tested only where its callees do nothing would look more verified than it is. It waits for honest contract doubles of the libultra thread/message layer (see the next step), which also unblock the ROM-read chain and about 360 other game functions.
- spline_load (`func_80030174`) is ready via doubles and not ported. About 490 depth-0 leaves remain ready.
- Still no "swap" build that runs ports as each other's callees.

**Suggested next step**
- Port `spline_load` (`func_80030174`, ready via rom_read_small) and decode splines in `assets` (0x10-byte header, pointer at +0xC relocated to +0x10).
- Keep batching depth-0 leaves: `cargo xtask next-function -n 60` lists them; `0x80008530..0x8000AC60` is next, with groups by address and one test file each. `tools/register_ports.py MODULE ADDR:comment ...` adds ports to `PORTED` and `functions.txt` in address order.
