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

### FPU control register (SPEC §10 Q5; settled session 6)
- **Initial FCR31 is `0x01000800`**: FS (flush subnormals to zero) | EV (trap on invalid operation), round to nearest. Written once by `func_8008B580` (libultra's `__osInitialize_common`, which also sets Status.CU1) through `func_80093C20` (`__osSetFpcCsr`: `cfc1 v0; ctc1 a0`). The only other FCR31 write is the exception handler's context restore (`0x8008D370`).
- **Every game-side `cfc1` (35 functions) is IDO's float→unsigned idiom**, not a plain rounding switch: save FCR31; write 1 (round toward zero, *all enables off*, FS off); `cvt.w.s`; `cfc1` and test the V/Z/O/U flags (`andi 0x78`). No flags: result = the word, and a negative word becomes `0xFFFFFFFF`. Flags: subtract 2^31 (`0x4F000000`), convert again, OR `0x80000000` (or `0xFFFFFFFF` if that overflows too); restore FCR31. It works on hardware only because the `ctc1 1` turns EV off for the conversion. The compiler interleaves unrelated instructions, so the regions look irregular.
- 35 flag-testing functions: `8000DA78 8000EEE0 8000FA2C 80010080 800105DC 80010B34 80011F38 80019BB4 8001A408 8001C404 8001D05C 8001E6C0 80021F84 80022798 800228C0 80028E78 800290A4 8002B574 8002BBA4 8002C780 8002CC28 8002D048 800444B0 80046DC4 80047A78 80047DB0 80055D38 80056464 80056844 80058058 80059E54 8005B3F0 8005C36C 80060DE4 8007C074`. `cargo xtask next-function` flags them ("tests FCR31 flags").
- **The oracle differs from hardware there**: `get_cop1_cs()` returns the rounding bits only, so the flag test always sees 0 and the second path never runs. Inputs in [2^31, 2^32) come out `0xFFFFFFFF` in the C and correct on hardware. Ports follow the C (the path stays in the code, dead, as it is in the C); a lift-phase decision.
- 557 `trunc.w.s`, 72 `trunc.w.d`, 192 `cvt.w.s`, 24 `cvt.w.d`; no `round`/`ceil`/`floor` anywhere.
- **Ports use `fpu::to_unsigned_s(g, f, &mut fcr31, save, tmp, dst, src)`** (session 10, first in `func_80011F38`): the whole idiom as the C runs it (cfc1 save, ctc1 tmp, cvt, the dead flag path, `at = 0x4F000000` from the delay slot, the result in `g[tmp]`), restoring FCR31 at its end. That equals the C only if no conversion or arithmetic sits between the idiom and its restoring `ctc1` in the caller, and `g[save]` isn't rewritten there; check that per call site. The dead path's `sub.s` would run in round-toward-zero in the C, which Rust can't: a `debug_assert` marks it. Translator drafts keep the idiom expanded; replace it by hand. Session 11 used it in `func_800290A4`, `8002C780`, `8002CC28` and `80047A78` (seven idioms, each restored in place).
- **The double form, `fpu::to_unsigned_d`** (session 12, `func_80056464`, `80056844`: three `cvt.w.d` idioms each for colour bytes `u(n * 0.5)`). IDO's `beql` shape: `at = 0x41E00000` (2^31's high word) is set by the caller before the idiom, `mfc1 tmp, dst` sits in the branch-likely delay slot, and the dead path writes `at` to the odd register `dst + 1` (`f_odd`) before `sub.d`. The single idiom in these functions has the same `beql` shape with `at = 0x4F000000` set before; `to_unsigned_s` fits it (it sets `at` again, to the same value).

### Floats in ports (decision, session 6; `game::recomp::fpu`)
How N64Recomp's C does floats, measured on this host (`oracle/c/fpu_probe.c`, `difftest/tests/fpu.rs`):
- **Arithmetic** is host IEEE single/double, round to nearest, **with subnormals** (the hardware flushes them: FS). `MUL_S` etc. are plain `*`, never fused. Rust `f32` ops compile to the same SSE instructions, so ports use them directly. For non-NaN operands, one `+ - * / sqrt` computed in f64 and narrowed equals the f32 op, so that mistake is invisible; `mul_add` is not, and a mutant test must catch it (it takes ordinary-range values, see below).
- **`NAN_CHECK` is an active `assert`** in the oracle (no `NDEBUG`). It guards every arithmetic operand, `neg`, `sqrt`, `cvt.d.s` and `cvt.s.d`, but **not** compares, moves or float→int conversions. A NaN operand stops the C (exit `0xC0000409`, no dialog; `difftest/tests/nan_domain.rs`). **So NaN operands of guarded ops are outside every port's domain**; the hardware would trap too (EV). NaN *results* (`inf - inf`, `0 * inf`) are computed and compared bit for bit (x86 default NaN `0xFFC00000`, not MIPS `0x7FBFFFFF`: another C-vs-hardware difference ports follow). NaN payloads are tested where the C doesn't look at them.
- **Conversions honour the host rounding mode** (`set_cop1_cs` = `fesetround`). Rust can't set it, so **ports keep FCR31's rounding bits in a local `fcr31`** (0 at entry: boot sets round-to-nearest and the idiom restores) and pass it to `fpu::cvt_w_s/cvt_w_d/cvt_s_w/cvt_s_d`. `get_cop1_cs` gives `fcr31`, `set_cop1_cs(v)` sets `fcr31 = v & 3`. Arithmetic only ever runs round-to-nearest; the translator adds a `debug_assert` in functions that write FCR31.
- **Out-of-range and NaN conversions are host-specific**, reproduced exactly by the helpers: `CVT_W_S` (MSVC `lrintf`) gives **0** for NaN and |x| > 2^31, but `0x80000000` for 2^31 itself; `CVT_W_D` (`lrint`) gives 0 whenever the rounded value is outside `i32`; `TRUNC_W_*` (C cast = `cvttss2si`) gives `0x80000000`. glibc would differ (64-bit `long`), and so does the hardware.
- Odd FPRs are only reached through `f_odd` (`mtc1`/`mfc1` of a double's high half), never with an odd `CHECK_FR`.
- **Test strategies for floats** need signed zeros, subnormals, halfway cases, infinities *and* ordinary values with full mantissas in the game's range (e.g. ±1e4). Edge values alone missed both fused-multiply-add mutants: random bit patterns give terms of wildly different size, and half-integers square exactly.

### 64-bit instructions
- `ld`/`sd`/`dsll32`/`dsra32`/`ddiv`/`ddivu`/`dmultu` are confined to about `0x8008a000–0x8008d000`, probably libultra `__ll_*` 64-bit helpers and `_Printf`. Game code appears to be 32-bit. `cvt.l.s`/`cvt.s.l` at `0x8008c648`/`0x8008c700` (libultra `__f_to_ll` family?).
- **libultra's `ll.c` is `0x8008AAE0..0x8008ADA0`** (session 7, confirmed by shape and ported): `__ull_rshift`, `__ull_rem`, `__ull_div`, `__ll_lshift`, `__ll_rem`, `__ll_div`, `__ll_mul`, `__ull_divremi`, `__ll_mod`, `__ll_rshift`, in source order. 64-bit values travel as register pairs (`a0:a1`, `a2:a3`, result `v0:v1`, high word first), spilled and re-read with `ld`. `__ll_rem` is unsigned (its source's `unsigned long long % long long`); `__ull_divremi` loads its u16 divisor with `lh` (QUIRK). N64Recomp's `DDIV` computes `INT64_MIN / -1` without faulting, so `__ll_div`/`__ll_mod` reach IDO's `break 6` and the runtime's `do_break` traps (the only reachable `break` so far).

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

### Depth-0 leaves ported in session 6 (`game::misc`, `game::math`)
Mostly by address, drafted with the translator. Beyond `functions.csv`'s notes:
- **Records at `0x800D2190`** (32 bytes, id = the low halfword of `a0`, signed): flags `+0x14` (bit `0x20` "on"), halfwords `+4`/`+6`, four bytes `+0x18`, a pointer `+0x1C`. Ids -201/-103/-104 name globals at `0x8009B778..83` instead. `func_8000A920` (on/off) and `func_8000AB24` (four bytes) have 172 and 149 call sites; the -103/-104 "on" is the fourth byte. **Entries behind `[0x8009B790]`** (0x7C bytes) with a selected index at `0x8009B798`.
- `func_8000C5F0`/`8000C658`: push/pop of a current id with a saved word per id. **`SAVED_WORDS` (`0x8009B7F4`) has room for 3 ids**; id 3 is `CURRENT_ID` itself.
- `func_8001004C`: a 16-bit handle decode through 8 (shift, base) pairs at `0x8009B888`.
- `func_80014C98` appends `gDPPipeSync` to the display list pointer at `0x801217B0`.
- QUIRKs worth knowing: `func_80012B5C` copies two **uninitialised stack words** to its outputs when its lookup misses. `func_8000FFF8`/`80010014` compare the index as 64 bits but address with its low word. The "three recently seen" list (`func_80009278`/`800092B0`) doesn't bound its slot.
- The flag at `0x8009A2B8` gates `func_80007A44`, `func_8000787C` (`[0x8009A328] = trunc(x * 32000)`) and `func_80008F28`: probably "audio running" (**guess**).
- `game::math`: vec2 add/scale/length/dist_sq (`0x8001514C..`), the first float ports.

### Depth-0 leaves ported in session 7 (`game::misc`)
By address, drafted with the translator, one test file per group. What they show:
- **Handles** (`func_80008F6C`): `(kind << 24) | (a1 << 16) | table[kind][i] | 0x8000` with eight s16 tables back to back from `0x8009A6F0` (lengths 0x33, 0x26, 0x39, 5, 0x68, 0xA9, 0x69, 0xA8; entry 0 never read); probably what `func_80007CE4` decodes (**guess**).
- **`track_name`** (`func_8002D598`): 25 `"~~Name"` strings from `0x800A98E4`, back to back and 4-aligned (checked against the ROM image in the test).
- **Spline walker** (`func_8003ABA0` step, `func_8003B250` start, `func_8003A568`, `func_8003A4E8`, `func_8003A50C`): a walker struct `w` with `+0` the spline, `+0x10..+0x1C` point indices (current first), `+0x20`/`+0x24` forward/backward end flags, `+0x28` a fork choice (`% count`), `+0x2C` path bits, `+8` a float (1.0 at the forward end, 0.0 at the backward end). The spline header's `+0` halfword (F) changes how indices shift. QUIRKs: backward with F != 0 sets the bits to `(bits << 1) & 1` = 0; with an end flag already set `a1` is an uninitialised stack word; `func_8003A568` indexes the point's halfwords with the whole shifted bit field.
- **Animation object** at `[0x800A2DD4]` (`func_80031FA4` step, `func_80031F80`/`F94` destination): a timer `+0x14` minus the double at `0x80120BF0`, a period `+0x10`, `n` phases (`+0x20`, bytes) advanced mod `+0xC`, `dest[map[i]] = src[phase[i]]` (halfwords); palette cycling (**guess**).
- **Render state**: modes at `0x800A3DA0`/`A4` switched by "ZBZB"/"AAEN"/"Full" (`func_800356BC`; four mode pairs, 0x0C084000/0x03024000 with neither). Display-list writers: `gSPMatrix(proj)` + `gSPForceMatrix(mvp)` to `[0x80112C90]` (from `0x801134D0`/`D4` or a camera's `+0x34`/`+0x38`), and a prologue to `[0x801217B0]` (`func_8003D370`: gSPTexture, combine `0xFCFFFFFF/0xFFFE793C`, othermodes by the flags `[0x800A4960]`). **Lights**: `Lights1` at `0x800A3DB0`, twelve `Lights2` slots of 0x28 at `0x800A3DC8`, a count word per slot at `0x800A3FA8`; QUIRK in `func_80038FE8`: the second light's red byte lands on `+0x19`, where green overwrites it.
- **Save data** (**guess** at the meaning): a 0x3F0-byte block at `0x80113680` (copied whole to `0x80113A70` by `func_8003960C`), 0x2C-byte records from `0x80113694` copied to and from `0x80113E60` (`func_80039914`/`80039984`); `crc32_table_init` (`func_800390C0`) builds the MSB-first CRC-32 table (poly `0x04C11DB7`) at `0x80114070`, probably for its checksum. The settings word `[0x800D697C]` (bit 6 by `func_80038DBC`) and flags at `0x800A4744`/`0x800A3D60` adjust an input mask (`func_800358A0`: bits 16/17 cleared, bits 9/10 cleared or swapped).
- **Heap and loader**: `heap_level_of(p)` (`func_8002FB4C`) scans the level cursors down; `model_load_stats` (`func_80030B68`) returns the model/texture byte counts. Debug fills: `0xDDEEDDEE` over `[0x800AE8B0, 0x80149C60)` (BSS start) and `0xBBCCBBCC` from the heap start to the end of RDRAM (re-reading `osMemSize` per word).
- **Pool registry** at `[0x800A2170]` (`func_8003F300`..`func_8003FB78`, `func_8003F99C`, `func_8003FA24`): a 0-terminated list of descriptors, `+0` id, `+4` tag, `+8` count, `+0xC` element size, `+0x10` elements, `+0x24` a callback. Elements: `+0` id, halfword `+4` index, halfword `+6` flags (bit 8 = skip). Functions init elements (every pool with the id; QUIRK: s16 index), find by tag, get/set count, iterate (`0x800A4AA4`/`0x80118D10`), call one element's callback, and broadcast to pools with an id or `"All!"` (a callback returning 2 stops it).
- **Racers and tracks**: `func_800281F0` builds the 23-entry racer list at `0x800D6CD8` (`{id, 0xFF, 0}`, then `{-1, 0xFF, 0}`) from the profile's unlock mask `[0x80113E74 + 0x2C * p]` ORed with `0x22E01` (six always available), count at `0x8011A26C`. `func_80024704` sets the track selection at `0x8011A240` (circuits 3, or 2 if a save flag byte is clear; unlocked tracks = the bits below `[0x800A21B4 + c]` of a per-circuit save byte; selection clamped; left/right flags). For circuit 3 the saved-data bits byte (`0x80113680 + 0xC + 3`) *is* the flag byte `+0xF` (found by a wrong test layout; see the session log).
- **RSP task** (`func_80084C30`): the `OSTask` at `[0x801488C0]`: rspboot `0x80097FF0` (0xD0 bytes), output buffer from `0x800DB894`/`98`, data from `[0x801217B4]`, and for type 5 the F3DEX2 text `0x800980C0` and data `0x800AE1D0`.
- `strcmp` (`func_800815FC`, -1/0/1), `memset` (`func_800313D8`), a byte compare (`func_80081530`).
- **`model_error` (`func_800827C0`) only spills `a0`.** It stays unported: loader.rs checks calls to it through its stand-in double, and a compiled-in function can't have one.
- Also: `framebuffers_init` confirmed against the NOTES formula; four 28-byte records at `0x800DB8A0` (channels, **guess**) whose setters call themselves for "all four" on -1; ring allocators of 256 × 32 bytes (`0x800E0C50`) and 3072 × 64 bytes (`0x800E2C50`) that hand out entry 1 first; a 190-entry (x, y, byte) list at `0x80118958`/`0x80118C50`; a header-list merge (`func_80030A7C`: node list, "Data"/"Anim" skipped, "AltN" pointers).

### Depth-0 leaves ported in session 8 (`game::math`, `matrix`, `anim`, `misc`)
Mostly float leaves by address, drafted with the translator. What they show:
- **Maths library** (`game::math`, 0x80015190..0x80017918 and 0x800160BC..0x80016DD8): vec2/vec3/vec4 set, copy, negate, IEEE compare, add, sub, dot, length, distances, cross, scale, multiply-adds; 4x4 row-major matrices: identity, scale, translation, row scaling, row set/get, copy, `out = a * b` (both copied to a frame first, so aliasing is safe), `m = n * m` in place (`n` read in place), vec3/vec4 times matrix (row vectors), the inverse of a scaled rotation plus translation (transpose over squared row lengths). Every sum is in a fixed order the ports keep, e.g. dot = `b.z*a.z + (a.x*b.x + a.y*b.y)`. Plus a ray/plane intersection (`func_80001D34`) with a double-precision parallel band.
- **The game's asin and atan2 in degrees** (`func_80014D4C`, `func_80014F54`): odd series with coefficients in the data segment (asin: 1/6, 3/40, 15/336, 0.047446; atan: 1/3, 1/5, 1/7, 0.063235: the last terms tuned), reductions (asin through `sqrt(1-x^2)` outside ±0.7071; atan2 by octant swap), `r * 180` in f32 then `/ pi` in double, quadrant fixups at ±1e-4. Worst error against `std` 0.0037° (asin) and 0.094° (atan2).
- **Matrix stack** (`game::matrix`): 33 matrices at `0x800AEC80`, depth `[0x8009A29C]` (bound 32, signed); push = `M * top`. A second stack of **3x4 matrices** (48 bytes) at `0x80112EA0`, depth `[0x800A3FF0]`, flag `[0x800A3FF4]` (push `func_80033EEC`, pop `func_800344C8`, top as 3x4/4x4 `func_8003423C`/`80034374`). Both "get" functions re-read the depth per word (QUIRK).
- **Animation objects** (`game::anim`, **guess**): the object table at `0x800AF4C0`; objects have float keys at `[+0x11C]` (count `+0x104`), time `+0x114`, rate `+0x110`, range `+0xF0..+0xF8` (setting `b < a` hangs in `pause_self`), `+0x124` a target node whose halfword `+0xC` bit 3 is cleared for kind 8.
- **Nodes** (misc, **guess**: model nodes): flags halfword `+0xC` (`|= 3` on change), 3x4 transform `+0x1C..+0x48`; `func_80018324` initialises a header, type `0xD065` getting an identity transform.
- **Pod stats** (**guess**): `func_800321F0(p, stat, level, x)` updates seven fields by level 1..5 (`+= c*x`, or `*= b + a*(1-x)` for stats 2 and 4) and clamps them (hi first); `func_800320E0` normalises them for display. The port is table-driven (`STATS`). The ROM repeats values at addresses the code distinguishes (level-1 vs later bounds), so the test perturbs the constants.
- **Text width** (`func_800129E4`): glyph advances (`+2` of 16-byte glyphs) up to NUL or `~n`; `~~` literal, other `~X` skipped; lowercase folded for fonts whose last char is below `a`; chars from 0x97 remapped through `0x800A1C86`/`0x800A1CD8`. QUIRKs: first char 0 lets the main table override extended glyphs; a `~` before the NUL runs past the string.
- **Eased values** (`func_80033328`, `func_800334F4`): steps scaled by the frame time, a double at `0x80120BF0` (`misc::FRAME_TIME`), computed in double and rounded to f32.
- Records at `0x800D2190` and entries at `[0x8009B790]` gained float setters; the 0x800D5xxx blocks reset by `func_8000FE1C/FE78/FEF0` gained their setters (vec3 slots, -1000 markers, colour bytes from stack arguments).

### Mutant tooling for floats (session 8)
- `tools/fmagen.py FILE FUNC...` writes a fused-multiply-add mutant for every add/sub fed by a product in straight-line ports, naming the factors by a register or unmodified memory that still holds them (symbolic tracking of loads, stores, moves, `sp`). `tools/genmut.py` turns `{fn, line, nth, new}` specs into `mutants.py` patterns anchored at the function (unique by construction).
- **A fused term can be tiny next to the sum** (asin's `y^7*P3`, `y^9*P4`, atan's `t^9*Q4`): fusing changes the result only a few times per million inputs, so random tests miss it. A search over the series found separating inputs, pinned in `trig_deg.rs`. Not equivalent, just rare.
- **Double multiply-adds rounded to f32** differ from their fused form only when the two f64 results straddle an f32 rounding tie. `ease.rs` builds such inputs: it fixes the other operands and searches the frame time (a RAM double the test sets) near a tie.
- Store-only functions are checked by simulating the stores on a copy of the input and comparing all of RDRAM (slower: shrinking takes long for failing mutants).
- **Session 9 additions.** `tools/fmapair.py FILE FUNC...` handles control flow: for every product it finds the adds reading it later in the same block (or one nested in it), following it through memory (an `sw` and a later `lw` of the same `g[R] + off`, `g[R]` not rewritten), and captures the factors at the product (`let fma = (p, q);`) so loops that reload them still work; `genmut.py` entries take `"also": [...]` for such two-line mutants. `mutants.py` sets `PROPTEST_MAX_SHRINK_ITERS=0` (a mutant is caught by the first failure; shrinking whole-RDRAM checks took minutes per mutant) and retries file writes (a Windows lock once made it die with a mutant left in `math.rs`).
- **Separators by construction** (session 9). Products that feed only corners or bins whose registers are overwritten later show only when they flip a comparison or a truncation, which random inputs essentially never do. `leaves_8002EA28.rs` builds, for each culling product, a matrix with one other product in the partial sum and the compared coordinate set to exactly the unfused sum; `leaves_8003B324.rs` scans consecutive floats of `cs` until a coordinate's `fl(S * k)` rounds to `-4.5` while the exact product lies just above (only below zero at a power of two can the float spacing past the integer be finer, which a fused `+ 0.5` needs to truncate differently); `leaves_80085AB4.rs` searches bin counts. Multiplications by 1, 2, 4, 8 and 0.5 are exact, so their fused forms are equivalent (found four times).
- **Ties at signed zero.** Several `<` vs `<=` mutants against a clamp are equivalent except at `-0` vs `+0`; strategies need `-0.0` explicitly, with the other operands arranged to meet it (pinned cases where that takes several conditions).

### Depth-0 leaves ported in session 9 (`game::math`, `render`, `input`, `pools`, `anim`, `spline`, `misc`, `libultra`)
The last depth-0 float leaves, by address, drafted with the translator. What they show:
- **LU pair** (`game::math`): `func_80016260` is Numerical Recipes' `ludcmp` shape for n = 3 on 4x4-stride rows (Crout, implicit pivoting, no parity), `func_800167E4` its `lubksb`; the only caller, `func_80016A20`, inverts a 3x3 through them. QUIRKs: the machine code updates `vv[i] = 1/t` whenever the row maximum grows, and its singular test sits inside that update, where it can't fire, so an all-zero row leaves `vv[i]` as a stale stack word; `imax` is a frame word carried between columns, stale at column 0 (kept when every `dum` is NaN). With n = 3 IDO's unrolled-by-four loops and the remainder loop's repeat above the diagonal never run; the ports leave them out with `debug_assert`s.
- **Controller pads** (`game::input`, new): `func_8002EA28` turns the four `OSContPad`s at `0x800D74C0` into 0x18-byte records at `0x800D74D8` (stick clamped to ±100, one byte per button bit, the button word); presence words at `0x800D7498`; QUIRK: one shared halfword `0x800D74A8` gets every present pad's buttons.
- **Culling** (`render::func_80036A1C`): an AABB's 8 corners times the matrix at `0x80112E60` (row vectors, products precomputed into a frame table), classified per axis: x/y against `|w|`, z as behind (`w <= 0`) or beyond (`w < Z`); returns 0 culled, 1 visible, 2 all inside (with a flag).
- **Screen quad** (`render::func_8003B324`): 14 F3DEX2 commands (vertex load from `0x800A4920`, `G_MODIFYVTX` Z/XY/ST, `G_TRI2`) for a rectangle rotated about its centre, corrected for the screen's aspect when `|(H/240)/(W/320) - 1| > 0.05`.
- **Pools**: `func_8003FDCC` is a nearest-elements query (sorted insertion by squared distance, capped, skipping one element and flagged ones; `"All!"` matches every pool). The descriptor's `+4` is a flag word (bit 0 enabled), not a tag.
- **Grid**: `func_8007DA20` sets up thresholds at `0x8011C858` (`misc::GRID`) that `func_8006B304` reads to build a 3 x 8 cell mask; QUIRK: modes other than 1 and 2 read four stale frame words.
- **Spline walker**: `func_8007EE98` places a walker from a segment number (`v / 10`, the remainder as the fraction); fork segments are found among the points' extra ids; QUIRK: the search stops by setting its indices to 99999, so a point count above 100000 would carry on.
- **Rect queue** (`render::func_80087814`/`func_800879B8`): up to 31 screen rectangles queued as halfword quads at `0x80148B60` (count `0x800A6978`), drawn as `G_FILLRECT`s plus a four-rectangle border from `0x80120E10`. `func_80086178` builds a viewport; `func_80085AB4` histograms the framebuffer's red and blue channels and returns a weighted sum over red bins 2..15.
- `func_80087CB0` is `sqrtf` (`game::libultra`).
- Plus maths helpers at `0x800811C0..0x8008241C` (plane value and height, projection, closest point on a segment, point times matrix, a signed power), animation "any done" walks (`anim::func_800736AC`, `func_8007B7BC`), and small setters and clamps in `misc`.
- **Large integer ones** (session 9): render-state setters (`render::func_8003594C`: material flags and a rewrite of combiner selector bytes 1/2/8/9 to 4 or 6; `func_8003609C`: two-bit mode fields to `SetOtherMode_H` commands on `[0x80112C90]`; `func_8003D110`: a fixed render-state reset list), an unrotated screen quad with one corner moved (`func_8003B860`), the depth-buffer probes (`render::func_8000F5A0`: 8 x 8 window counts at two points, `x < -500` gives 50; depth samples for three point lists, -1000 for none; QUIRK: unbounded reads), the texture load commands (`render::func_800125E4`, by format 0..3) and the profile-record reset (`save::func_80029A3C`: working copy at `0x80113E60`, saved at `0x80113694`, 44 bytes; the default unlock mask 0x22E01).
- **`func_800827C8`** stores the word 1 unaligned at address 1: a deliberate crash (address error on hardware; the host faults in the C, the port's bounds check panics). Tested in child processes.

### Depth 1, session 10 (`collide`, `math`, `anim`, `misc`, `save`, `render`)
48 ports in six address-ordered groups (`depth1_800005B4.rs` .. `depth1_800141EC.rs`). What they show:
- **Collision query** (`collide`, **guess**): `func_80004000` sets up a sphere (centre `0x800AE908`, radius `0x800AE8E0`, `r*r` at `0x800AE8DC`) and a plane pair (normal `0x800AE948`/`960`, offsets `0x800AE954`/`58`, world copies at `96C`/`970`), mode `[0x800AE934] = 3`, callbacks `0x80003348`/`0x80002FFC`, `[TRACKED] = r*r*K`, and resets the matrix stack. `func_800038E8`/`80004704` move the query (and 28-byte point+direction records) into the space of the matrix stack's top (translation only with flags bit 1 clear, else through `func_800160BC`'s inverse); `func_80003B44` moves the result back. `func_80000B00` offers the nearest of a triangle's three edge points to the record step; `math::func_800005B4` is an edge-cross point-in-triangle test.
- **Sound request** (`misc::func_80008760`, **guess**): 8 slots of 0x20 at `0x800D2038` (`+0` -1 free / -2 pending, `+4` id, `+8` keep, `+0xC` frame `[0x80120BE8]`, `+0x10` priority, `+0x14` pitch*2, `+0x18` volume, `+0x1C` pan). A kept id refreshes its slot unless the old one is louder and from this frame; else the first free slot or the lowest priority below the request. Volume scaled by the settings bytes `0x80113685`/`86`; QUIRK: the pan (forced to 0x40 without stereo) is written into the caller's argument slot.
- **Debug menu** (`misc::func_8000CC1C`, `func_8000D5EC`, **guess**): tuning values of the selected "Test" pool element (16 clamp-adds through `func_8000C6C8`, copied to `[e + 0x1E70]`), stepping the selected tag (QUIRK: stepping down past a gap wraps to the top of the run above), settings toggles gated by `[0x8009B7D8]`. The id stack (`SAVED_WORDS`, `CURRENT_ID`...) is reset by `func_8000D9A8` from a "Jdge" pool element (QUIRK: not tested for null).
- **Text**: `render::func_80011F38` draws a glyph rectangle and grows the text box `0x800D6914`; `render::func_800141EC` sets a glyph up (colour, combine, the font's texture prologue by format 0/2, metrics to `0x800D691C..34`, `func_800125E4`).
- Other QUIRKs: `func_8000B98C` gives the identity when only the second record is selected; `func_800181BC` passes only flag `0x10` to its children (the walk stops one level down) and doesn't preserve `v0` across its calls; `func_8000B254` tests a node's type twice (the second branch unreachable); `func_80006EC0` stores `trunc(old time)` as an integer.
- Recursive functions (`func_8000E8C4`, `8000EA4C`, `800181BC`) call themselves through the C symbol, so the difftest checks the top level only, like any callee.

### Testing depth 1 (session 10)
- **The test RDRAM background is random, not the ROM image** (`difftest::background`). Constants a function or its callees read from the data segment (pi, 180, asin coefficients, thresholds) are random unless the test writes them: two `func_80014CC0` mutants survived that way. `depth1_800141EC.rs::load_data` copies the ROM's data segment (`0x80098000..0x800AE8B0`) in; earlier groups wrote the constants they use explicitly.
- **Models replay callees' C.** For thunks and loops over callees the model runs the callee's C on a copy of the state (`State::run(imports::func_X)`) with the thunk's frame and arguments, then compares the whole state (`difftest::diff_states`) or RDRAM. Callees save `s` registers in their frames, so the replay must set the `s` registers the port holds at each call (two model bugs, `func_80016A20`, `func_800141EC`).
- A callee's stack spills land in the caller's frame (`func_8000C6C8` spills `lo` to `[sp + 0xC]`, `func_8003F714` saves `s0` just below): model them to compare all of RDRAM.
- **Mutants in git worktrees** (`git worktree add --detach <scratch>/wt <commit>`, the group's files copied in, `baserom.z64` copied to its root, `RACER_GENERATED_DIR`/`RACER_N64RECOMP_DIR` pointing at the main tree, a separate `CARGO_TARGET_DIR`) keep the main tree editable while mutants run; two ran in parallel. **Sync with `cp`, not PowerShell `Copy-Item`**: `Copy-Item` keeps the source's timestamps, cargo saw nothing newer than its last build and compiled the test against the old `game`, and a whole run reported "caught" for build failures. **Check the unmutated baseline line of every run**: a crashing baseline (a NaN reaching the C) makes every "caught" meaningless.
- `tools/tidy_draft.py` turns a draft into port style; `tools/stage_ports.py` stages one group's lines of `lib.rs`/`functions.txt`/`imports.rs` when later groups are already in the tree (a module holding several groups is staged from a saved copy with `git hash-object -w` + `git update-index --cacheinfo`).
- `hi`/`lo` are locals in N64Recomp's C: ports write `let (lo, _) = multu(..)`, and docs don't list them as leftovers.

### Depth 1, session 11 (`misc`, `input`, `heap`, `anim`, `pools`, `channels`, `render`, `save`, `spline`)
50 ports in five address-ordered groups (`depth1_800290A4.rs` .. `depth1_800454A8.rs`), `0x800290A4..0x8004F254`. Left for later (large): `func_80012BF0` (0x11D0 bytes), `func_80014568` (0x730), `func_8003BAB0` (0x1660, float). What they show:
- **FCR31 idiom** in four more functions (`func_800290A4`, `8002C780`, `8002CC28`, `80047A78`), all through `fpu::to_unsigned_s`: each restore follows its idiom directly. In `func_800290A4` the save (`cfc1 t9`) sits in both arms of an `if`, and both arms reach the `ctc1` with the same FCR31, so one call covers them.
- **Fade and HUD** (`misc`, **guess**): `func_800290A4` lowers colour -103's alpha from 255 (`[0x800A2604]` by `850 * dt`), then turns it off; `func_8002C780`/`8002CC28` move two levels each (`0x800A2654..60`, `±s * dt`, clamped to `[0, 254]`) and lay out records through the setters `func_8000A920`/`AA04`/`AAC0`/`AB24`; `func_80047A78` is a two-record gauge from the counter `[0x800A4BB8]`.
- **Button words** (`input::func_8002ECA0`, `BUTTON_WORDS` `0x800D76F0`): per pad, held `+0`, pressed `+0x10`, released `+0x20`, stick x/y floats `+0x30`/`+0x40`. Bits 0..13 are record bytes, bits 14..23 stick tests against the doubles `0.3, -0.3, -0.2, 0.2` at `0x800A9FA8..90` (bands between 0.2 and 0.3 are bits 20..23). `misc::func_8004E0A0` feeds them through `func_8004E488` into the menu state `[0x800A4B94 + 4i]` and directions `0x800A5208`/`5218`.
- **Pool messages** (`pools`): one-word or struct messages sent through `func_8003F99C`/`8003FA24`: `"Paws"` (`misc::func_8002F0EC`), `"Qery"` with an answer word (`func_8003FB34`), `"Aloc"` to the first element whose `+6` bit 8 is set (clearing it, `func_8003FBD4`), `"Free"` to each element with the bit clear (setting it, `func_8003FC94`/`FD7C`), `"NAsn"` to `"cMan"` elements (`misc::func_8004F254`). So bit 8 of `+6` reads as "free".
- **Channels** (`channels`): `CURRENT` `0x800A290C` (the entry each channel plays), `ENTRIES` `0x800A2870` (13 entries of 12 bytes). QUIRKs: `k >= 13` hangs in `func_800319F4`/`80031AB0` (tested in a child process); `func_80031BBC(-1)` stores before `CURRENT`.
- **3x4 matrix stack** (`misc::MTX43_STACK` `0x80112EA0`, four rows of three floats, which the ports' docs call 4x3; depth `MTX43_DEPTH` `0x800A3FF0`): reset (`func_8003483C`), to ring `Mtx` entries (`func_80034948`), and `func_80034E20`: `gSPMatrix` of the matrix below the depth, then `gSPForceMatrix` of its product with the 4x4 at `0x80112E20` (QUIRK: a camera offset `[0x800A3FDC]` is subtracted and added back, and the round trip rounds). `render::DL2_HEAD` `0x80112C90` is the list these and the material functions append to.
- **Materials** (`render::func_80035BF0`, `80036314`): othermode, combiner (16 bytes at `+6`, in `gDPSetCombineLERP` order), prim/env/fog/blend colours, each only if it differs from a cached copy at `0x80112DE4..` (which these functions read but never update), then the texture load (`gSPTexture`, `LoadBlock`, `LoadTLUT`, seven tiles).
- **Others**: `save::func_80039178` is **crc32** (MSB first, poly `0x04C11DB7`, init and xorout -1; checked against a bitwise CRC); `spline::func_8003A5D0` evaluates a segment (weights `(t^3, t^2, t, 1) × B` for six bases at `0x800A4750..`, type 0 with four points, other types a point and its handles); the scene root `0x8011A288` has a 151-entry child list at `0x8011A2A8` (`func_80046974`, cleared by `80046764`, pruned by `80046870`); `func_8004BAC8` builds the track-select grid (QUIRK: a medal re-initialises the record and loses its colour).

### Testing depth 1 (session 11)
- **Callees save callee-saved FPRs too.** `func_800344F4`/`80034650` save `f20`/`f22` in their frames; at those calls `func_80034E20` holds matrix entries there, so the model has to set them before replaying (as with `s` registers). The first model mismatch was that, not the port.
- **Ties need constructed values**, again: stick thresholds as the f32-rounded doubles (a stick of 20 or 30 meets them exactly; the ROM's doubles never are), levels of exactly -0.0 (`s = ±0`, `dt` of the other sign), `d.y == ±lim` with integers. Each was a surviving mutant first.
- **An answer nobody writes**: `func_8003FB34` returns a word only its callback could set, and the verified callback doesn't. A constructed case places the element in the caller's frame so the callback's `+0x14` increment lands on the answer word.
- `append(&mut w, [..reads of w..])` doesn't compile (two-phase borrows don't cover it): hoist the words. Nested unboxed strategies (`uniform4` of tuples holding `uniform16`) overflow the test thread's stack: box them.
- `mutants.py` marks mutants that crash the test binary `caught/abort`; with a passing baseline those are real catches (the port reads a wild pointer).
- Equivalent mutants this session: a no-op `& 0xFFFFFF` (`func_80035BF0`), `min(h, 0x7FF)` against `slt 0x800` (`func_80036314`), and stores `func_80018324` already makes (`func_80045DA0`/`45E80`).

### Depth 1, session 12 (`misc`)
36 ports in five address-ordered groups (`depth1_8005058C.rs`, `depth1_8005C210.rs`, `depth1_800689A0.rs`, `depth1_8007B06C.rs`, `depth1_800672D4.rs`), `0x8005058C..0x8007EE4C`. What they show:
- **Camera transition** (**guess**): six 4x4 matrices at `0x80118D60`, `DA0`, `DE0`, `E20`, `E60`, `EA0`, translation rows at `+0x30` (so the vec3 at `0x80118E10` is `t(DE0)`). `func_8005058C` saves `E60 = E20` and `DA0 = D60` (unless its fifth argument is set), sets the targets `t(DE0)` and `t(EA0)` and the mode halfword `[0x800A4BC0]`; `func_8005065C` moves `t(E20)` from `t(E60)` toward `t(EA0)` and `t(D60)` from `t(DA0)` toward `t(DE0)` by `s = t / T` (state at `0x8011AC24`: two deltas, `t`, `T`; `T` is 0.5, or the ROM's 0.3 when `|dx|, |dy| < 500` and `[o + 0x38] == 1`), then saves again and toggles `[0x800A4BC4]`.
- **HUD records**: `func_80056844` lays out records 0..13 and 25 (two layouts by `flags & 1`, a slide `90 h` by `flags & 2`/`4`), `func_80056464` record 26, `func_80057ED4` record 23 plus a screen rectangle, `func_80057F48` hides 27..34, or 35..42 with two players when `k` isn't the second. Positions are computed at run time from float constants, some through frame round trips (`sw` of an `i16`, `lh` of the low half back).
- **Racers and steering** (**guess**): racer records of `0x88` bytes from `[0x8011B1B8]`, the players' at `[0x8011B1BC]`/`[0x8011B1C0]` (`func_8005C210` sets their markers through `func_8000FEAC`, `-h` for the players when `[0x800A52BC] >= 2`). `func_80063EF4` turns the heading `[o + 0x68]` toward `atan2(-d.x, d.y)` at 90 degrees per second outside a ±5 degree band; `func_8006D7F0` steers `[o + 0x1F0]` away from the nearest `"Test"` pool element (`func_8003FDCC`, skipping `o` itself); `func_8006B9C8` is the closest approach of two points moving in 2D (`t` clamped to `[0, 1]`, -1 when parallel).
- **Pod physics** (**guess**): `dt` is the double at `0x80120BF0`, narrowed where it is used in f32 (the float at `0x80120BF8`, read by `func_8005065C` and `80063EF4`, is probably its f32 copy). `func_800689A0` a boost charge `[o + 0x1A8]`; `func_800672D4` the engine step (thrust `[o + 0x1B4]`, force `[o + 0x1B0]`, a `"Hitt"`/`"Botm"` message to the pod's own pool element); `func_80068410` the speed limit (a saturating curve of the rate `[o + 0x1A4]`, two constant sets by bit 3 of `[[o + 0x1E70] + 8]`); `func_80066144` four ground-contact points (offsets from `0x800A5CA0 + 0x6C k`, rotated by `o + 0x20..0x40`, projected on a plane, one matrix each at `o + 0x1290 + 0x40 i`); `func_80079714` a rate-limited value (`func_80073C58`); `func_80075A3C` pushes a point out of a slab of two planes.
- **Node trees** (recursive through the C, like the session-10 self-calls; type `0x3064` a material node, bit 14 a group, `0xD065` a transform): `func_8007531C` collects up to 10 distinct materials (by `[mat + 8]`) into `0x8011C8B0` (count `0x8011C8D8`); `func_80075490` hands them out round robin (index `[0x800A66C4]`, count `0x8011C8DC` below 5; its `div` needs a nonzero count); `func_8007B430` finds the first material with a texture `[mat + 0xC]`; `func_8007B544` sets the animations (`func_80006D5C` kinds 8 and 9) of transform nodes; `func_8007BB28` gives a node a transform from a pool of 50 (`0x8011CA58`, nodes `0x8011CB20 + 0x58 i`).
- **Other**: `func_80055AEC` sets a texture's RGBA5551 alpha bits by `R - B < trunc(-242 + 492 level)` over 256 pixels (unrolled by four; the copies' registers as a table, `ALPHA_COPIES`); `func_80053220` the buttons pressed by the pad that controls the menu (`0x800D7700` = `BUTTON_WORDS + 0x10`); `func_8005B2D0` an activity test with a coordinate box; `func_8005EE18` reads a level's `"Data"`/`"LStr"` light points.
- QUIRKs: `func_8007BB28` with a full table uses `T[50]`, node 0's first word; `func_8005D10C` sets light-rig entry 4 twice (entry 7 never); `func_8005EE18`'s scan for the -1 word is unbounded.

### Testing depth 1 (session 12)
- **Callees read data-segment constants too**: atan2 (`func_80014F54`) reads its series from `0x800A87C0..`; on the random background a NaN reached a guarded op in about one run in five and aborted the whole binary (only the translated run showed it, since proptest seeds each run). `func_80081700` reads `0x800ADCB0`. Load the data segment (`load_data`) whenever a callee might read constants.
- **The background pattern is the same in every case**: the HUD record table's flags were identical in all 256 cases, so turning off record 14 instead of 13 went unnoticed. Randomise the regions a function writes into (`randomise_memory(seed, RECORDS, ..)`).
- **NaN inside recursive C calls**: a model guards the top level only; children run the C. A NaN `s` in `func_8007B544` reached a negation two levels down (and a node reached twice sees the NaN rate the first visit wrote): keep NaN inputs to cases that stay in the domain (here, no animation objects). The same for a callee's own divisions (`func_80081700`'s `0 / 0` at `a = dt = 0`): check them before replaying.
- **Sign-only sums need separators by cancellation**: in `func_8006D7F0` the sums only choose a branch. For each product, a case where the unfused sum is exactly 0 (the other term is `-fl(product)`, arranged through the geometry) and the fused one is the product's rounding error, its sign searched to the side that flips the branch. Five were needed.
- **Fused `a * b + c` with a truncation or a binade**: fused and unfused agree when the product is exact or `c` is a multiple of the product's ulp; `6 (1 - t) + 0.5` differs only where the product rounds to a coarser ulp than the sum (`|6 (1 - t)|` in `[4, 4.5)`, `t = 1.67`). `trunc(-242 + 492 level)` separators are levels putting the sum just across a multiple of 8. All found by exact searches.
- **More constructed ties**: `-0.0` results (`a == c` gives `t = -0`), a threshold written equal to the computed value (`K = n`), `e == F` through `r = 1` so a step leaves the value unchanged, identical planes (`A = B`), `s = 0` pushes, `h` stepped by ulps until `a - h` is exactly the boundary, marker 23722, whose position triple covers the racer base word.
- **A "caught" is only meaningful if the filtered tests pass unmutated**: a new pinned test that failed on its own made mutants look caught. Rerun the baseline after every test change.
- Shell: `cd X && (A) & (B) & wait` backgrounds the `cd` with `A`, so `B` runs in the old directory; start each worktree's run as its own command.
- Equivalent mutants this session include: `<` vs `<=` where the tied values store the same bits (clamps to the compared value, `1 < t` storing 1.0, `K < x` storing `K`), products by 0.5 and by ±1 fused, a first store overwritten by a second, reloads that equal the register (callees that leave it), and a sign test only reached for `|e| > 5`.

### Module layout (session 8)
`game::misc` was split by subsystem (pure moves; `functions.csv` notes name each function's module): `pools` (the registry at `[0x800A2170]`), `render` (display-list writers, render modes, lights, `framebuffers_init`, the RSP task), `save` (save block copies, crc32 table, racer list, track selection), `spline` (walker and point helpers), `channels` (`0x800DB8A0`); `heap_level_of` went to `heap`, `model_load_stats` to `loader`. `misc` keeps what is still unknown, by address. New ports go in the subsystem's module (`register_ports.py MODULE`). Session 9 added `input` (controller pads). Session 10 added `collide` (the collision query at `0x800AE8B0..0x800AE974`, a **guess** at the subsystem; `misc::TRACKED` and `func_8000097C` stay in `misc`). The 3x4 matrix stack (`0x80112EA0`) is still in `misc`, beside its session-7 push/pop. Sessions 11 and 12 added no module; session 11's constants: `input::BUTTON_WORDS`, `channels::CURRENT`/`ENTRIES` (not `misc::ENTRIES`, a different array), `misc::MTX43_STACK`/`MTX43_DEPTH`, `render::DL2_HEAD`.

### Splines (session 6; `assets::Spline`, `spline_load` = `func_80030174`)
- Block: `u32 count` (91), then single offsets. Entry: a 16-byte header, then `count` points of **0x54 bytes**, exactly filling the entry (all 91).
- Header: `+0` unknown, `+4` point count, `+8` **segment count = points + one per extra successor at a fork** (all 91), `+0xC` stale. **The loader overwrites `+0xC` with the first point's address** (cursor + 0x10); it doesn't relocate the old value.
- Point: `+0` successor count (0-2), `+2` predecessor count (0-3); successor indices from `+4`, predecessors from `+8` (one point has three, so the list runs into `+0xC`). **Unused slots hold stale bytes that read as ASCII text** (authoring-tool leftovers): always honour the counts. Links are mutual (5076 points). Then four f32 triples: position `+0x10`, an often-(0,0,1) vector `+0x1C`, two handle-like points `+0x28`/`+0x34` (**guess**: Bézier handles). Then ten i16 at `+0x40`: usually the point's own index twice, `next_count - 1` extra segment ids (all in `count..segments`), -1 padding.
- 41 splines are open (one start, one end), 50 closed.
- `spline_load(index, &out)`: index = sign-extended low word (stack round trip, like the other loaders). **No space check, no alignment.** A pointless loop counts to `+4` (its `v1` is overwritten by `heap_check` afterwards).

### Translator (session 6; `crates/translate`, `cargo xtask translate`)
- Parses N64Recomp's C (a small C expression parser). Each statement must match one instruction's shape (about 40 integer shapes, plus float, `c1cs` and FCR31 shapes), or the function is refused with a reason. It never guesses.
- CFG: a conditional branch's delay slot gets its own block on the taken edge. It is hoisted above the branch when the fall-through starts with the same instructions (non-likely), with `let cN = cond;` if it overwrites a register the condition reads. Branch-likely delay slots stay on the taken edge. Jump threading, chain merging, `lui`+`addiu`/`ori` folding.
- Structuring: Ramsey's "Beyond Relooper" (dominator tree; `break 'b` to merge nodes, `continue 'l` for back edges), then passes that remove fall-through jumps, splice unused blocks, merge `if a { if b {..} }` into `&&`, hoist loop exits and fix saved-condition polarity. Irreducible graphs would get a `match pc` state machine; none exist in the game's code.
- Output style matches the ports: `g[REG]`, `m`, `ctx.fpr[n]`, `call(imports::func_X, m, ctx)` with `let g = &mut ctx.gpr;` re-borrowed after calls, and `hi`/`lo`/`c1cs`/`fcr31` locals as N64Recomp has them.
- **Validation:** `cargo test -p difftest --features translated` swaps a draft of every `PORTED` function in for the port (`game`'s build.rs, `difftest::port_under_test`). All 131 translate and pass all difftests. Translator mutants (dropped saved condition, wrong tail strip, missing negation, `sra`→`srl`) are caught.
- **Coverage** (`--survey`, end of session 7): **1101 of 1103 game-side functions translate**; the two left use `lwl`/`lwr`/`swl`/`swr`. No irreducible graphs. In the OS range (`>= 0x80087CC0`, 258 functions) the first refusals left are unaligned accesses (16), `cop0_status_read/write` (6) and a few float shapes (`cvt.l`-family, `f.u64` in odd places).
- **64-bit shapes** (session 7): `LD`/`SD` of GPRs, `x << (N + 32)`, `SIGNED(x) >> (N + 32)`, `x >> (N + 32)` and the constant `dsll`/`dsrl`/`dsra`, the variable forms `(rs & 63)`, `a + b` without `ADD32` (`daddu`/`daddiu`), `DMULT`/`DMULTU`/`DDIV`/`DDIVU(a, b, &lo, &hi)` (the parser takes unary `&`), `dmtc1`/`dmfc1` (`f.u64`). Helpers `dmult`/`dmultu`/`ddiv`/`ddivu` in `game::recomp` follow recomp.h.
- **`LOOKUP_FUNC`** (session 7): `LOOKUP_FUNC(v)(rdram, ctx)` becomes `call(imports::runtime::get_function(v as i32).expect(..), m, ctx)`, with `g` re-borrowed after like a direct call (without it the draft doesn't compile). The oracle's `get_function` searches a table build.rs generates from funcs.h (every function start: the compiled-in C, or the stub that runs a double or traps), so an indirect call resolves exactly like a direct call to the same function; any other address traps.
- **Jump tables** (session 7). N64Recomp's shape (`recompilation.cpp`, `cgenerator.cpp`): at the table's `addu` it declares `gpr jr_addend_JR = rIndex;` (the index register *before* the `addu`, already `<< 2`); the table's `lw` becomes an **`addiu`**, so the register holds the entry's *address*, and nothing is read; the `jr`'s delay slot runs before `switch (jr_addend >> 2) { case k: goto L; ... default: switch_error(__func__, jr, table); }`; after the switch comes the dead copy of the delay slot, which **the default reaches if `switch_error` returns**. The translator makes a `Term::Switch` and an `S::Match` (cases with one target share an arm; the default a block calling `imports::runtime::switch_error` then falling through). All 63 tables in the generated C have an `sltiu` within 40 lines before (on the same register, checked for the five depth-0 functions), and no default is reachable in any ported function; unit tests cover the default's fallthrough, and translator mutants (swapped case targets, addend from the wrong register) are caught by the difftests.
- **`break`** (session 7): `do_break(vram)` becomes `imports::runtime::do_break`. In the game it is IDO's divide check: `div; bnez divisor; break 7` and `li at,-1; bne divisor,at; lui at,0x8000; bne dividend,at; break 6`. **The C's `div` comes first and faults on the host for a zero divisor** (`idiv`), so `break 7` is never reached in the oracle; `game::recomp::div` asserts likewise. `break 6` can't happen in the host C either (64-bit division). Neither depth-0 function with `break` can reach it in its domain; `crates/difftest/tests/breaks.rs` tests the hook in a child process.
- **Loop-exit hoisting bug, fixed in session 8**: a loop body ending in `if c { continue } else { X }` had `X` moved after the loop even when `X` could fall through (a trailing `continue` already stripped), so two loops sharing a header lost the outer iteration (`func_80034650`'s draft converted one row). Now only non-falling-through exits are hoisted; 4 of 1068 drafts changed, none ported. Drafts of unported functions are not validated until they are ported, so a draft's control flow still needs reading.
- **Label collision, fixed in session 9**: a block that begins with its branch (a C label right before an `if`) never got its own address (cfg.rs set block addresses at the first plain statement, and a branch's statements sit inside the `if`), so it kept the previous instruction's and its structured label collided with another's. A `break` meant for the outer block then bound to the inner one and was dropped as a fall-through, which re-ran IDO's copied delay-slot instruction (a likely branch filled from its target, branching past it): harmless for the idempotent `slt`s in the four ported drafts it touched (`func_800129E4`, `func_800321F0`, `func_800344F4`, `func_80034650`), wrong in general. Blocks now take the branch's address, and label names get the block number if two reachable blocks ever share an address; 46 of 1330 drafts changed beyond label names, 18 had had a duplicated label. Unit test: `label_before_a_branch_gets_its_own_name` (fails without the fix).
- **Unaligned accesses** (session 9): `do_lwl`/`do_lwr`/`do_swl`/`do_swr` become `lwl`/`lwr`/`swl`/`swr` in `game::recomp`, which mirror recomp.h exactly (checked against it through `oracle::unaligned_probe`, `c/unaligned_probe.c`, in `unaligned.rs`). All 1103 game-side functions translate now.
- A draft is not a port. The per-function loop still applies, and the part that takes the time is the statement, the doc comment (domain, leftovers, QUIRKs), the independent check and the mutants.

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
Ports that must do what generated code does with the runtime call the same hook: `game::imports::runtime` declares them (`pause_self`; since session 7 `switch_error(func, jr, table)`, `do_break(vram)` and `get_function(vram)`, the last returning the function an indirect call goes to), and whoever links `game` defines them. That's the oracle's stub runtime in tests, where every hook traps. Tests of code that reaches a trap run in a child process (`crates/difftest/tests/texture_block_init.rs`, like `crates/oracle/tests/traps.rs`).

### The OS boundary and message-queue doubles (design, session 6; not built yet)
**What blocks game code** (`symbols/callgraph.csv`; OS range = `>= 0x80087CC0`). 363 unverified game functions reach the OS range through 65 OS functions they call directly. By what each of those reaches:

| Family | Boundary fns | Game fns reaching one | Examples |
|---|---|---|---|
| pure (computation only) | 26 | 316 | `__ll_lshift`/`__ull_div`/`__ll_mul` (8008AB84/AB48/AC48), `sinf`/`cosf` (8008A8C0/A750), `osContGetReadData`, `alBnkfNew`, `osCreateThread` |
| hw (device registers, CP0; no queues) | 22 | 258 | `osGetCount` (8008C550), `osAiGetLength` (8008ADA0), `osInvalDCache`, `osStartThread` (CP0 via `__osDisableInt`) |
| msg (message queues / threads) | 14 | 187 | `osRecvMesg` 179, `osPfs*` 165, `osContStartReadData` 162, `8008A710` 109, `osPiStartDma` 58 |
| indirect (function pointers) | 3 | 206 | `alAudioFrame` (80088538) 161, `sprintf` (8008A6B4, `_Printf`'s output callback) 88, `alInit` 4 |

If a family's boundary functions were satisfied, how many of the 363 would have nothing else in the OS range: pure 66; pure + hw 135; + msg 157; **+ indirect 363**. So message queues alone unblock little: **indirect calls (audio and sprintf) gate more game code than message queues do**.

**`func_80008F28` is not a message wait.** It is `if ([0x8009A2B8]) func_8002E124()`. The flag is also tested by `func_80007A44` and `func_8000787C`, and looks like "audio running" (**guess**). `func_8002E124` services audio: `osAiGetLength`, the 64-bit helpers, then `func_800073A4` toward `alAudioFrame`. So under the ROM reads, `heap_set_level`'s `func_80007E80` wait and `func_8002E034`'s handshake is the audio frame, reaching all four families. No double of the message layer alone makes it honest.

**Plan, in order of payoff** (status at the end of session 7: 1 started, 2 done):
1. **Pure OS functions: port them, don't double them.** Done so far: `sinf`, `cosf` and the ten `ll.c` helpers (`game::libultra`). They are recompiled C like game code and the translator handles most (the 64-bit helpers need `ld`/`sd`/`dsllv`/`ddivu`/`dmultu` shapes). `sinf`/`cosf` must be ported (the rule on the game's own maths routines). Replacing libultra wholesale (SPEC Phases 6-7) means these ports are the replacement.
2. **Indirect calls: support `LOOKUP_FUNC`.** (Done in session 7; see "Translator".) The oracle's stub `get_function(vram)` should resolve to the compiled-in C, a double, or a trap, like direct callees. Ports then call `recomp::call(get_function(target))` through a runtime hook, and the translator emits that instead of refusing. That covers `sprintf`'s callback and `alAudioFrame`'s handlers, and the ~50 functions whose only blocker is an indirect call.
3. **hw doubles take injected values.** `osGetCount`, `osAiGetLength`, `osAiGetStatus` and friends return what the test supplies. That is a contract: the function's effect *is* returning the register, and both sides of a difftest see the same value.
4. **Message-queue contract doubles** over the real RDRAM layout, for a single-threaded test world:
   - `osCreateMesgQueue(mq, msg, count)`: port it (pure). Layout (`func_800880E0`): `+0 mtqueue`, `+4 fullqueue` (both `&__osThreadTail` = `0x800A7BB0` when empty), `+8 validCount`, `+0xC first`, `+0x10 msgCount`, `+0x14 msg` (array of words).
   - `osSendMesg(mq, m, flag)` (8008C930): full (`validCount >= msgCount`): NOBLOCK returns -1; **BLOCK is refused** (it waits for another thread). Otherwise `msg[(first + validCount) % msgCount] = m`, `validCount++`, return 0. `osJamMesg` (8008C7B0): the same but `first = (first + msgCount - 1) % msgCount` and the message goes at the new `first`.
   - `osRecvMesg(mq, &m, flag)` (80087E80): with a message, `*m = msg[first]` if `m != 0`, `first = (first + 1) % msgCount`, `validCount--`, return 0. **Empty + NOBLOCK returns -1; empty + BLOCK is refused**, like the ROM doubles refuse bad transfers, unless the test installed a scripted *event source* for that queue (e.g. "PI DMA done", "audio thread replied"). The source posts the message, and the double records it in the call trace.
   - Waking a waiting thread (`mtqueue`/`fullqueue` not `__osThreadTail`) is refused: no other threads exist. `osStartThread` of a thread that would preempt is refused for the same reason. `osCreateThread` is pure and gets ported.
   - Register leftovers: like the ROM doubles, restore callee-saved registers sign-extended and scramble caller-saved ones. Stack below `sp` isn't reproduced.
   - Device drivers on top (`osPiStartDma`, `osContStartReadData`, `osPfs*`, `osAiSetNextBuffer`) get their own contracts: DMA from the ROM image or a test input script, then a completion message on the caller's queue through the msg doubles.
5. `func_8002E124` (audio service) and with it `func_80008F28`, the ROM-read chain and `heap_set_level` become portable once 1-4 cover `alAudioFrame`'s tree. Until then the ROM doubles stay the contract for the chain.

### CP0 Status: `__osDisableInt`/`__osRestoreInt` (decision, session 13)
- **What they are.** `__osDisableInt` (8008CA80): `t0 = Status; Status = t0 & ~1; v0 = t0 & 1` (clear IE, return the old IE). `__osRestoreInt` (8008CAA0): `Status |= a0`. Each is a leaf; the only non-pure step is `mfc0`/`mtc0 Status`, which N64Recomp emits as calls to the runtime hooks `cop0_status_read(ctx)` / `cop0_status_write(ctx, value)` (recomp.h). 14 OS functions call them (osRecvMesg, osSendMesg, osJamMesg, osStartThread's helpers ...), so they gate those 14 and everything above.
- **Family: neither a hw double nor a pure port, but a port over a runtime hook**, like `pause_self` or `do_break`. They have generated C, and the device they touch is the runtime's, not RDRAM or a register block a double would fake. A double would also hide the very thing a game build must get right (the IE bit's round trip). So they get **ported**, calling `imports::runtime::cop0_status_read/write` exactly where the C does, and the translator learns both shapes.
- **Where Status lives: `recomp_context::status_reg`**, the u32 field N64Recomp's recomp.h puts in the context for the runtime's use. The oracle's stub runtime stops trapping: a read returns `status_reg` sign-extended (`mfc0` of a 32-bit register), a write stores the low word. A write that changes the FR bit (26) still traps: that switches the FPU register mode (`mips3_float_mode`, `f_odd`), which nothing in the game does after boot and the oracle doesn't model. Because the field is in the context, each side of a difftest runs on its own copy and `difftest::diff_states` already compares it (`Diff::StatusReg`); tests set `s.ctx.status_reg`. The game build's runtime (ours, later; N64ModernRuntime is GPL and not linked) must honour the same contract, and is where real interrupt masking would happen.
- What this doesn't cover: `osGetCount` (8008C550, `mfc0 Count`) has no generated C at all (recomp.toml skips it), so it stays plan item 3, a contract double returning test-injected values. Other CP0 users (`mtc0 Compare` 80097980, the exception handler, TLB, cache ops) are skipped by recomp.toml and are the runtime's job.

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
- An unquoted heredoc (`<<EOF`) runs backticks in its body as command substitutions: a doc comment with `` `code` `` in it hung a command (session 9). Quote the delimiter or write a file.
- Claude Code's Bash tool fails to parse a heredoc whose body has an odd number of `'` (a Rust lifetime or label, `'found`), even with `<<'EOF'`. Write the text to a scratch file and `cat` it instead (session 5).
- Working-tree files may have CRLF endings (git converts on commit). Scripted multi-line replacements must match the file's endings. A mutant runner must insist each pattern is **unique** in the file: a replacement that lands in another function looks like a missed mutant, or like a caught one if it breaks the build (session 6).

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

### 2026-09-28 — Session 6

**Done**
- **`cargo xtask translate`** (new crate `translate`, no dependency on `game`; Facts: "Translator"). It drafts register-exact ports from the generated C, integer and float. **Validated**: every existing port's draft passes every existing difftest (`--features translated`; 39 at first, all 131 at the end), and four translator mutants are caught. `--survey`: 1046 of 1103 game-side functions translate.
- **92 ports, 131/1374 verified** (was 39), all drafted with the translator: 86 depth-0 leaves in 7 address-ordered batches, one test file per group (`leaves_8000803C.rs` .. `leaves_80018114.rs`), plus 5 float leaves and `spline_load`. 82 reachable port mutants, all caught in the end. Four missed in the first round: two fused multiply-adds and a range edge, which led to better strategies, and one that landed in another function. Plus 4 helper and 4 translator mutants.
- **Pace:** batches of 11-23 took 2.3 to 6.5 minutes each from first draft to csv update, mutants included (about 12-30 s per function; smaller functions are faster). Session 5 recorded no timings; it verified 30 functions in the whole session.
- **Floats: designed and started** (Facts: "FPU control register", "Floats in ports"). `game::recomp::fpu` gives conversions keyed on a local `fcr31`, checked against recomp.h's own macros on this host (`oracle/c/fpu_probe.c`, `difftest/tests/fpu.rs`). The translator does floats. 5 float leaves ported (vec2 ops in the new `game::math`, a `trunc.w.s` store). `nan_domain.rs` pins that a NaN operand stops the C.
- **FCR31 settled:** boot sets `0x01000800` (FS|EV). All 35 game-side `cfc1` sites are IDO's float→unsigned idiom, which tests FCR31 flags the oracle can't see. `next-function` flags them.
- **Message-queue doubles scoped** (Facts: "The OS boundary and message-queue doubles"): the families of the 65 OS functions under 363 game functions, what each would unblock, the queue doubles' contract (blocking with no message or no room is refused, unless a scripted event source feeds the queue), and the order of work. 22 libultra functions named in functions.csv.
- **Splines decoded** (`assets::Spline`, all 91 checked; Facts: "Splines") and `spline_load` ported.
- `tools/mark_verified.py`; `game::recomp` gained `lb lhu sh sb sra srl sllv srav srlv slt sltu mult multu div divu ld sd` and `fpu`.
- Tests: 209 (was 110).

**Surprises**
- **The OS range isn't mainly message queues.** Indirect calls (`alAudioFrame`'s handlers, `sprintf`'s output callback) gate 206 of the 363 blocked game functions. `func_80008F28`, the "wait" under the ROM reads and heap_set_level, is `if ([0x8009A2B8]) func_8002E124()`: the audio service, which reaches every OS family.
- **Every game FCR31 read is a flag test** (unsigned conversion), not a rounding switch, and it is invisible to the oracle.
- MSVC's `lrintf` gives **0** for out-of-range values, but `0x80000000` for exactly 2^31. The C cast gives `0x80000000`. These results are host-specific.
- `NAN_CHECK` is live in the oracle, so NaN operands of arithmetic are outside every float port's domain. Compares, moves and conversions to int aren't guarded.
- Float tests with only edge values missed fused-multiply-add mutants. Ordinary-range values caught them.
- `spline_load` overwrites the header's `+0xC` rather than relocating it, and unused spline link slots hold stale ASCII.
- Five of my own statements were wrong, with C and Rust agreeing each time. The causes, all found in the code or layout: the three-id `SAVED_WORDS`; the 64-bit compare vs low-word address; spline_load's low-word index; `heap_check` overwriting `v1`; and an assumption placed after the run in a float test. A sixth miss was a mutant that landed in another function (the runner now requires unique patterns).

**In progress / not done**
- The partial float design sections rode along in commit d1db849; this entry completes them.
- Not translated yet: jump tables (39 functions, e.g. `func_80008F6C`), `break` (8), `LOOKUP_FUNC` (8), unaligned accesses (2). Large depth-0 functions skipped by the batches: `func_8000F5A0`, `func_800125E4`, `func_800129E4`.
- The message-queue, hw and indirect-call doubles are designed, not built. heap_set_level still waits for them.
- `game::misc` is about 2600 lines, ordered by address; it wants splitting as subsystems become clear.
- Still no swap build that runs ports as each other's callees (the translated-draft swap is test-only and swaps one function at a time).

**Suggested next step**
- Keep batching depth-0 leaves with the translator (`cargo xtask next-function -n 60`; `0x8002D968..0x8002FE94` is next). 278 depth-0 game functions remain (121 of 399 are verified), 268 of them translatable.
- Add jump tables to the translator: `switch` into Rust `match`, validated like the rest. That unblocks 39 functions.
- Then the OS boundary in the Facts' order: port the pure OS functions (64-bit helpers, `sinf`/`cosf`), add `LOOKUP_FUNC` support (the oracle's `get_function` resolving to C or doubles), then the hw and message-queue doubles.

### 2026-09-28 — Session 7

**Done**
- **133 ports, 264/1374 verified** (was 131). Depth 0: 242 of the 400 game-side functions (was 121 of 399), plus 12 in libultra. By group, one difftest file each: 4 jump-table functions (`jump_tables.rs`), 2 `break` functions (`breaks.rs`), 10 leaf batches `0x8002D968..0x80086CC8` (`leaves_8002D968.rs` .. `leaves_800811CC.rs`), two of the large skipped ones (`selection.rs`: track selection and racer list), the two indirect-call functions (`indirect.rs`), `sinf`/`cosf` (`trig.rs`) and the ten `ll.c` helpers (`ll.rs`). About 330 port mutants, all caught except the equivalents, each argued or searched (see Surprises).
- **Translator: jump tables, `break`, 64-bit shapes, `LOOKUP_FUNC`.** 1101 of 1103 game-side functions translate (was 1046); the two left use unaligned accesses. Every addition was validated on the ports that needed it (`--features translated`: all 271 difftests pass on the drafts) and by translator mutants (swapped case targets, addend register, `dsra` logical, `dsllv` mask, `ddivu`/`ddiv`, indirect target). Facts: "Translator".
- **Runtime hooks** `switch_error`, `do_break`, `get_function`. **The oracle resolves `LOOKUP_FUNC`** through a generated table of every function start, the same C or stub a direct call reaches, and traps otherwise.
- **`game::libultra`**: `sinf`/`cosf` (names confirmed against `std` through an independent model, the constants loaded from the ROM image in the test) and `ll.c`. OS plan items 1 (started) and 2 (done).
- **`tools/mutants.py`** (committed): unique-pattern check, CRLF-aware, restores the file and proptest-regressions files, per-mutant test filter, timeouts for endless mutants, `--show`, run subsets by index. `register_ports.py` and `mark_verified.py` now read and write UTF-8.
- Tests: 326 (was 209).
- **Pace:** leaf batches of 5-19 took 3.7 to 9 minutes each, first draft to commit, mutants included (about 20-40 s per function; batch 9's 19 took 5 minutes). Similar to session 6, with more non-trivial functions. Mutant runs without a test filter took 20 s per mutant instead of 2-5; always filter.

**Surprises**
- **N64Recomp turns a jump table's `lw` into an `addiu`**, so after the `jr` the register holds the entry's address, not the case address the hardware loads. Ports follow the C and say so. All 63 tables are `sltiu`-bounded, so no `switch_error` default is reachable anywhere ported.
- **`do_break` is unreachable in game code under the oracle.** IDO checks the divisor after the `div`, and the host's divide faults first. The one reachable `break` is `break 6` in `__ll_div`/`__ll_mod`: recomp.h's `DDIV` returns `INT64_MIN / -1` without faulting.
- **`sinf`/`cosf` take NaN** (their branch does no arithmetic, so `NAN_CHECK` never runs), and they have genuinely equivalent mutants. A fused multiply-add inside the small branch's polynomial changes no result: an exhaustive search over all 2^27 inputs of that branch found none. `n * pihi` is exact by construction (Cody-Waite), so fusing it changes nothing either. The fused final step and a fused `n * pilo` are caught.
- Game QUIRKs found: `func_80038FE8` writes the second light's red byte to `+0x19`, where green overwrites it. The spline walker's backward bits are always 0 (`(bits << 1) & 1`). There are uninitialised frame reads (`func_8006E008`, the walker's `a1`), a 50-entry lookup that reads one past into the records (`func_8007BA9C`), an index fill with an unbounded count (`func_8004F6E8`), an s16 loop index that never ends above 32767 (`func_8003F300`), and a sign-extended u16 divisor (`__ull_divremi`).
- One wrong statement of mine, C and Rust agreeing: in `func_80024704`'s test, for circuit 3 the circuit's bits byte is the save flag byte itself, so writing one overwrote the other. Found in the layout; the model now reads both back. Four more failures were my test models overflowing in debug arithmetic (`2 * k as u32` and the like), not statements; the regression files they left were deleted.
- `model_error` is just a spill of `a0`, but porting it would break loader.rs's call tracing (a compiled-in function can't have a double), so it stays a stand-in.
- Environment: scratch files must go to the session scratchpad by its full path. `$TMPDIR/..` is `AppData\Local`, where four were written by mistake (moved). A mutant that makes a loop endless needs a timeout (`mutants.py` now kills the test binary).

**In progress / not done**
- Large depth-0 integer functions still skipped: `func_8000F5A0`, `func_800125E4`, `func_800129E4` (text width with `~` escapes and a font table), `func_80029A3C`, `func_8003594C`/`8003609C` (unrolled byte rewriters), `func_8003B860`, `func_8003D110`, and `func_800321F0`, the 900-instruction float jump table: seven fields by `a1`, five levels `a2`, `field += coefficient * a3`, clamped; pod upgrades by category, level and health (**guess**). Also `func_800827C8` (unaligned `swl`) and `model_error`.
- The depth-0 float leaves (about 150, from `func_8000097C`) are untouched this session; `next-function` lists them after the integer ones.
- OS plan items 3 and 4 (hw doubles with injected values, message-queue contract doubles) are not started.
- `game::misc` is about 6000 lines. Subsystems are now clear enough to split it: pools, render state and lights, save/profile, spline walker, channels.

**Suggested next step**
- Batch the depth-0 float leaves with the translator (`cargo xtask next-function -n 60`), using the float strategy from `jump_tables.rs`/`leaves_8001514C.rs` (ordinary full-mantissa values, not only edges).
- Split `game::misc` into subsystem modules before it grows further (`register_ports.py` takes the module name).
- Then the large integer ones (`func_800129E4` first, then `func_800321F0` with a table-driven port), and OS item 3: hw doubles for `osGetCount`, `osAiGetLength` and friends.

### 2026-09-28 — Session 8

**Done**
- **`game::misc` split** (one pure-move commit): `pools`, `render`, `save`, `spline`, `channels`, plus `heap_level_of` to `heap` and `model_load_stats` to `loader`; later `matrix` and `anim` (with their integer siblings moved in). Facts: "Module layout", "Depth-0 leaves ported in session 8".
- **101 ports, 365/1374 verified** (was 264). Depth 0: **343 of 400 game-side functions** (was 242). 99 were float leaves in 13 address-ordered batches, one difftest file each (`leaves_8000097C.rs` .. `leaves_80037BF8.rs`, `trig_deg.rs`, `ease.rs`, `mtx_fixed.rs`). The other two are the large ones from the brief: `func_800129E4` (text width, `text_width.rs`) and `func_800321F0` (the 900-instruction stat update, table-driven, `stats.rs`).
- **About 470 port mutants, all caught** except the equivalents argued below, including **about 250 fused multiply-adds** (every multiply feeding an add or subtract, f32 and f64).
- **Tools**: `tools/fmagen.py` (fused-mutant generator for straight-line ports) and `tools/genmut.py` (function-anchored mutant patterns). Facts: "Mutant tooling for floats".
- **Translator bug fixed** (loop-exit hoisting with shared loop headers; unit test; 4 of 1068 drafts change, none ported). Facts: "Translator".
- Tests: **406** (was 326), all passing; `--features translated`: all **350** difftests pass on the drafts (was 271), every ported function translating.

**Pace**
- Leaf batches of 4-19 took 8-15 minutes each from install to commit, drafting overlapped with test waits (about 30-70 s per function), similar to session 7 per function, with more floats and more mutants per function. The trig pair took 13 minutes, `func_800129E4` 7, `func_800321F0` 17.
- **Mutant runtime dominates the float batches**: about 9 s per mutant, so the 183 of the matrix batch took 27 minutes. Whole-RDRAM simulate checks make failing mutants slow to shrink (up to 140 s).

**Surprises**
- **Fused mutants of tiny terms are real but rare**: the last terms of the asin/atan series change a result a few times per million inputs; a search found separators (pinned in `trig_deg.rs`). **Double multiply-adds** feeding an f32 result only show at f32 rounding ties; `ease.rs` builds inputs by searching the frame time (a RAM double). The first search aimed at one tie and failed; aiming at several nearby ties works.
- **The ROM repeats constants at addresses the code tells apart** (`func_800321F0`'s level-1 vs later bounds): the test perturbs the constant block to catch address mix-ups.
- Equivalent mutants, argued: clamps `<` → `<=` against a nonzero bound (equal floats with a nonzero bound have the same bits: `func_800321F0`'s hi, `func_800320E0`'s bounds); integer ties in `func_8000C724` (stores the value already there); `func_800078B4`'s clamp at 0 (stores 0 over 0); `func_80014F54`'s `K3 <= t` (t is never negative).
- My own test mistakes, not port bugs: out-of-RDRAM indices from over-wide strategies (twice; now bounded and stated in the domain), a stack overflow from unboxed 16-element strategies, a wrong text-width expectation (the font had lowercase), and three strategies too narrow for a mutant (a boundary char, a signed index, a tie). Each regression file was deleted.
- `func_80034650`'s draft was wrong (the translator bug); the difftest would have caught it, but reading the draft did first.

**In progress / not done**
- Depth 0 left: 47 float leaves (`next-function` lists them from `func_80016260`, a Gauss-Jordan inverse with pivoting, and `func_800167E4`, its back-substitution), 8 large integer functions (`func_8000F5A0`, `func_800125E4`, `func_80029A3C`, `func_8003594C`, `func_8003609C`, `func_8003B860`, `func_8003D110`), `func_800827C8` (unaligned `swl`) and `model_error` (stand-in, on purpose).
- OS plan item 3 (hw doubles with injected values) and the pure OS ports (`osCreateMesgQueue` ...) were not started.
- `func_80015724`/`80015C30` (the 4x4 products) are straight-line ports in the C's order, not loops: their register rotation isn't regular enough for a table.

**Suggested next step**
- Finish the depth-0 float leaves (47), then the 8 large integer ones; that completes Phase 3's done-when (all depth-0 non-libultra functions verified, `model_error` excepted by decision).
- Run mutants in the background while drafting the next batch through a separate target dir (`CARGO_TARGET_DIR` in the scratchpad), as this session did for `translate`.
- Then OS item 3 and the pure OS functions.

### 2026-09-28 — Session 9

**Done**
- **54 ports, 419/1374 verified** (was 365). Depth 0: **397 of 400 game-side functions** (was 343); left are the entry `func_80000400`, `func_80011F38` and `model_error`. All 47 remaining float leaves but `func_80011F38`, in address-ordered groups with one difftest file each (`lu.rs`, `leaves_8002EA28.rs`, `leaves_8003B324.rs`, `leaves_80052134.rs`, `leaves_8006B304.rs`, `leaves_80071820.rs`, `leaves_800811C0.rs`, `leaves_80085AB4.rs`, `leaves_80029A3C.rs`), all seven large integer ones (`large_int.rs`, `leaves_80029A3C.rs`) and `func_800827C8` (`unaligned.rs`). New module `game::input`. Facts: "Depth-0 leaves ported in session 9".
- **About 380 mutants** (each port's reachable ones, fused multiply-adds for every product feeding an add, in loops and through memory too), all caught except 11 argued equivalent (below). Several needed constructed separators (Facts: "Mutant tooling", session 9 additions).
- **Translator**: the label-collision bug fixed (Facts: "Translator"; 46 drafts changed, four ported ones harmlessly), and unaligned `lwl`/`lwr`/`swl`/`swr` supported through new `game::recomp` helpers checked against recomp.h itself. **All 1103 game-side functions translate.** Decision on `func_800827C8`: teach the translator (17 OS functions need the same shapes), not a hand port.
- **Tools**: `tools/fmapair.py` (fused mutants with control flow and memory), `genmut.py` `"also"` edits, `mutants.py` no-shrink (caught mutants went from ~45 s to ~2 s) and write retries.
- Tests: 472 (was 406), all passing; `--features translated`: all **415** difftests pass on the drafts (was 350), every ported function translating.

**Pace**
- Groups of 5-10 functions took about 20-40 minutes each from draft to mutants done, with drafting, docs and tests overlapped with mutant runs. The first (the LU pair) took about an hour including building `fmapair`. Mutant rounds of 50-150 ran 10-30 minutes once shrinking was off.

**Surprises**
- **The translator's label collision** (Facts): IDO fills a likely branch's delay slot with its target's first instruction and branches past it; with the collision, the draft re-ran that instruction. Found by reading `func_8003FDCC`'s draft, which had two nested blocks with the same label.
- **Separators by construction**: min-corner products of the culling (only a flipped side shows them), the quad's `* ws + 0.5` before truncation (a window that exists only below zero at a power of two, and not at all with `ws = 1.25`, too few mantissa bits), histogram terms far below the sum's ulp. Signed-zero ties needed `-0.0` in strategies, and one pinned case.
- Equivalent mutants, argued: pad y clamped at exactly 100 (`<=` vs `<` moves the same bits through the same registers); fused `* 0.5`, `* 2`, `* 4`, `* 1` products (exact: the quad's centre, the grid's `2s`/`4s`, the histogram's weights 1, 2, 4 and 8); `t0 < 0` vs `< -1` with `t0 = w << 6` (a multiple of 64). `func_8007EE98`'s fused `f32(k) * 10` is exact in the domain (points in RDRAM or an s16 match bound `k`), so not run.
- **Dead code in the LU pair**: with n = 3 IDO's unrolled loops never run, nor does one remainder loop's repeat; a missed mutant in the latter showed it. The ports leave them out with `debug_assert`s.
- **Two of the ROM's constant pairs repeat** (`func_8006C828`'s steps 0.33, `func_8006D9DC`'s rates 3.2): the tests perturb them.
- My own mistakes, not port bugs: the quad's command count (17 in my statement, 14 in the code; C and Rust agreed; found by recounting the pointer steps); a separator search with too few mantissa bits; a mutant filter naming a function instead of a test (five mutants "missed" in 1.5 s because no test ran: a miss that fast means a wrong filter); a proptest block closed in the wrong place. The one regression file from a wrong model was deleted.
- `mutants.py` died once while restoring a file (a transient Windows lock) and left a mutant in `math.rs`; every pattern's uniqueness check found and undid it. Now it retries.
- **Porting a function can break a test that uses it as a fixture**: `indirect.rs`'s `bad_callbacks_trap` used `func_8003B860` as "a function with no C in the oracle", and porting it made the callback run instead of trap. Only a `--no-fail-fast` full run showed it (plain `cargo test` stops at the first failing binary). The fixture is now the depth-27 `func_8004AF60`, and the test asserts it is still unported. Fixtures of that kind should be deep functions.
- Environment: an unquoted heredoc containing backticks runs them as command substitutions (a doc comment hung a command). Use a quoted `<<'EOF'` or a file.

**In progress / not done**
- Depth 0 left (game side): `func_80011F38` (a 0x6AC-byte float function around the FCR31-flag idiom; drafted, 460 lines, not ported), `model_error` (stand-in, by decision) and the entry `func_80000400` (indirect `jr`).
- OS plan item 3 (hw doubles with injected values) and the pure OS ports were not started.

**Suggested next step**
- Port `func_80011F38` (its dead flag path as the C has it, NOTES "FPU control register"); then Phase 3 is done except the entry and `model_error`.
- Then OS item 3 and the pure OS functions (`osCreateMesgQueue` ...), now that unaligned accesses translate.

### 2026-09-29 — Session 10

**Done**
- **Goal 1: `func_80011F38`** (glyph rectangle, `render`), the last depth-0 float function: Phase 3 is done except the entry `func_80000400` and `model_error` (by decision). First port of IDO's float-to-unsigned idiom, through the new shared `fpu::to_unsigned_s` (Facts: "FPU control register"). 44 of 45 mutants; the miss is equivalent (masking a value at most 1024).
- **Goal 2: depth 1 started: 48 ports in six address-ordered groups**, `0x800005B4..0x80028070` (Facts: "Depth 1, session 10"). New module `collide`. **468/1374 verified** (was 419); depth 1 is 50 of 219.
- **445 mutants** over the six groups (45 fused multiply-adds), all caught except four argued equivalent: `func_8000BB78`'s null test on a record `func_80017F28` never returns null for `k < 4`; `func_8000D9A8`'s `-2` branch, whose effects are all overwritten; and two fusions of a product by 0.5 in `func_8000EBE8` (exact). Several needed constructed cases: each triangle edge winning in turn, an exact sound-volume tie, stepping the menu selection onto an element with flag bits other than 8, flag words of exactly 1, `K == w` and a point exactly on the screen margin.
- Tools: `tools/tidy_draft.py`, `tools/stage_ports.py` (Facts: "Testing depth 1").
- Tests: **524** (was 472), all passing with `--no-fail-fast`; `--features translated`: all **467** difftests pass on the drafts (was 415), every ported function translating.

**Surprises**
- **The test RDRAM background is random, not the ROM.** Callees that read constants from the data segment ran on random ones; two mutants survived until the test loaded the ROM's data segment (Facts).
- **Four of my models were wrong, none of the ports.** `func_800181BC`'s `v0` isn't preserved across its calls (my statement said it returns the word's address; it only does without the recursion flag); two replayed callees saved `s` registers my replay hadn't set; a projection model let a NaN `w` reach the C's guarded conversion (the process aborted: a crashing baseline). The regression file they left was deleted.
- **A whole mutant run was void**: syncing the worktree with PowerShell `Copy-Item` kept old timestamps, cargo compiled the test against the previous `game`, and every mutant "caught" a build failure. Worktrees are synced with `cp` now and each run's unmutated baseline is checked first.
- **A latent flake in `loader.rs::heap_limits`** (session 4): random heap ends can make the decompressor's window corrupt a model's tag, which calls `model_error` (`func_800827C0`), and that test had no stand-in installed, so the oracle trapped and aborted the binary (seen once, in the translated full run). It now installs the stand-in, as `tight_comp_layouts` does.
- Porting in address order means most depth-1 functions are thunks or small wrappers around callees; replaying the callees' C in the model (Facts) was the productive pattern.

**In progress / not done**
- Group 7 (`func_800290A4`, `8002C780`, `8002CC28`: three more FCR31-idiom functions, fades and HUD positions calling the record setters; `8002DA0C`..`8002FB18`: track-unlock and save helpers; `8002ECA0` on the pad update) is drafted (scratch, not kept) but not ported. The idiom functions need the expanded idiom replaced by `fpu::to_unsigned_s` by hand; in `func_800290A4` the save (`cfc1 t9`) sits in both arms of an earlier `if`.
- Stretch goals (OS hardware doubles, pure OS ports) not started.

**Suggested next step**
- Continue depth 1 in address order from `func_800290A4` (`cargo xtask next-function -n 400`, depth-1 rows sorted by address), one test file per group, mutants in a worktree while the next group is drafted.
- For every group: load the ROM data segment into the test state where constants matter, replay callees' C in the models with the registers the port holds, and check each mutant run's baseline.

### 2026-09-29 — Session 11

**Done**
- Start checks: `verify-rom`, `recomp`; `cargo test --no-fail-fast` 524 passed, `--features translated` 467 passed (every ported function translating).
- **Goal 1: depth 1 continued in address order, five groups of ten**, `0x800290A4..0x8004F254` (Facts: "Depth 1, session 11"; test files `depth1_800290A4.rs`, `depth1_8002FDF8.rs`, `depth1_80033878.rs`, `depth1_80039178.rs`, `depth1_800454A8.rs`). **518/1374 verified** (was 468); depth 1 is 100 of 219. Seven more FCR31 idioms through `fpu::to_unsigned_s`, including `func_800290A4`'s save in both arms of an `if`.
- **542 mutants** over the five groups (103 fused multiply-adds, mostly from `fmapair.py` on the two matrix and spline products), all caught except six argued equivalent (Facts: "Testing depth 1 (session 11)"). Constructed cases were needed for: stick-threshold ties, levels of exactly -0.0, `d.y == ±lim`, a query answer only a callback could write, an object pointer aliasing the list it prunes, and a fused add before a truncation (separators from an exact search).
- Tests: **580** (was 524), all passing with `--no-fail-fast`; `--features translated`: all **523** difftests pass on the drafts (was 467).

**Surprises**
- **Callee-saved FPRs.** A model mismatch in `func_80034E20` was the callees saving `f20`/`f22` in their frames: at the calls the port holds matrix entries there, so models set FPRs the way they already set `s` registers.
- **Session 10 left three large depth-1 functions** behind the group boundary (`func_80012BF0`, `func_80014568`), and this session added `func_8003BAB0` (0x1660 bytes of float code) to them; all three are still unported.
- **The same name twice**: `channels::ENTRIES` (13 channel entries at `0x800A2870`) and `misc::ENTRIES` (a pointer to 0x7C-byte entries) are different things in different modules.
- Heredocs with an odd number of apostrophes failed again when writing doc files; docs went through a Python file instead. A nested unboxed strategy overflowed a test thread's stack (boxed now).

**In progress / not done**
- Stretch goals (OS hardware doubles, pure OS functions) not started.
- `func_80012BF0`, `func_80014568`, `func_8003BAB0`.

**Suggested next step**
- Continue depth 1 in address order from `func_8005058C` (`8005058C`, `8005065C`, `800521C0`, `80053220`, `80055AEC`, `80056464` and `80056844` with FCR31 idioms, `80057ED4`, `80057F48`, ...), one test file per group, mutants in two worktrees.
- For every group: set the `s` registers **and callee-saved FPRs** the port holds before replaying callees; construct ties for every `<`/`<=` a random input can't reach; check each mutant run's baseline.

### 2026-09-30 — Session 12

**Done**
- Start checks: `verify-rom`, `recomp`; `cargo test --no-fail-fast` 580 passed, `--features translated` 523 passed (every ported function translating).
- **Goal 1: depth 1 continued in address order, five groups**, `0x8005058C..0x8007EE4C` (Facts: "Depth 1, session 12"; test files `depth1_8005058C.rs`, `depth1_8005C210.rs`, `depth1_800689A0.rs`, `depth1_8007B06C.rs`, `depth1_800672D4.rs`). **554/1374 verified** (was 518); depth 1 is 136 of 219. Group 12 includes the large `func_80056844` (0xBA4, a HUD layout of 70 setter calls); group 16 is the two pod-engine functions `func_800672D4` and `func_80068410`.
- **The FCR31 idiom in double**: new `fpu::to_unsigned_d` (Facts: "FPU control register"), used by `func_80056464` and `80056844`.
- **Mutants: 741** over the five groups (237 + 155 + 156 + 80 + 113), 720 caught, 21 argued equivalent (Facts: "Testing depth 1 (session 12)"). Constructed cases were needed for sign-only sums (five cancellation separators), fused sums across a binade or a multiple of 8, `-0.0` results, thresholds written equal to computed values, and ties reached only through a step that leaves a value unchanged.
- Tests: **628** (was 580), all passing with `--no-fail-fast`; `--features translated`: all **571** difftests pass on the drafts (was 523).

**Surprises**
- **Callees read data-segment constants**: atan2's series on the random background produced a NaN in about one run in five, aborting the binary; only the translated full run showed it. `load_data` is now in every test whose callees might read constants.
- **The fixed background pattern** hid a whole class of mutants (record flags the same in every case); record-setting tests randomise the record table now.
- **NaN reaching guarded ops inside recursive C calls**, where the model can't check it, and inside a callee (`func_80081700`'s `0 / 0`): models reject those inputs before replaying.
- Three of my models or statements were wrong, none of the ports: `func_80057F48`'s player test inverted, `func_8005C210`'s alias case leaving the domain, and `func_8006B9C8`'s `0 / 0` reaching a callee. The regression files they left were deleted.
- A pinned test that failed on its own made its mutants look caught; the baseline is rerun after every test change now.

**In progress / not done**
- `func_80012BF0`, `func_80014568`, `func_8003BAB0` (the three large depth-1 functions) are still unported.
- Stretch goals (OS hardware doubles, pure OS functions) not started.

**Suggested next step**
- Continue depth 1 in address order from `func_8007EE4C`'s successor (`cargo xtask next-function -n 400`, depth-1 rows sorted by address: `8008035C`, `80080408`, `80081814`, `80081948`, `80081BE8`, ...), one test file per group, mutants in two worktrees (each run started as its own command).
- For float functions whose sums only pick a branch, plan the cancellation separators with the port; load the data segment from the start.
