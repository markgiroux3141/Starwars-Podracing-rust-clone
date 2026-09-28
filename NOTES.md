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
- Python 3.12; project venv at `.venv/` with rabbitizer 1.16.2, spimdisasm 1.42.4, splat64.

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
