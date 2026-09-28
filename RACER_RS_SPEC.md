# racer-rs — Project Spec

A behaviour-accurate Rust reimplementation of **Star Wars Episode I: Racer (N64, USA)**, built incrementally on top of a static recompilation of the original ROM.

This is **not** a matching decompilation. Success is measured by behaviour (same game state, frame by frame, for the same inputs), not by byte-identical compiler output.

> Note for Claude Code: this spec was written from a snapshot of the ecosystem in late September 2026. Before starting, re-check the current state of the upstream projects listed in §3, since they move fast. Anything marked **(verify)** is a best guess that should be confirmed before relying on it.

---

## 1. Goals and non-goals

### Goals
- A native, playable PC build of the game where the game logic is written in idiomatic Rust.
- Every ported function is verified against the original's behaviour by automated differential tests.
- The game stays runnable throughout: Rust replaces recompiled code piece by piece ("ship of Theseus"), never a big-bang rewrite.
- A well-documented understanding of the game: named functions, typed structs, documented file formats.

### Non-goals
- Byte-matching the original ROM. We never need the original compiler (IDO/GCC) or a permuter.
- Supporting PAL or Japanese ROMs (at least until the USA build is complete).
- Shipping any ROM data, extracted assets, or copyrighted code in the repository.

---

## 2. Target ROM

Only one ROM is supported. Users supply their own legally obtained dump.

| Field        | Value                                   |
|--------------|-----------------------------------------|
| Title        | `STAR WARS EP1 RACER`                   |
| Game code    | `NEPE` (USA, NTSC)                      |
| Common name  | `Star Wars Episode I - Racer (U) [!].z64` |
| Header CRC1  | `0x72F70398`                            |
| Header CRC2  | `0x6556A98B`                            |
| Size         | 32 MB                                   |
| Entry point  | `0x80000400`                            |
| Main code    | ~608 KB, ROM `0x1000`–`0x99000`         |

These values come from the sp00nznet/racer recompilation project's README.

Requirements:
- **Byte order must be `.z64` (big-endian).** If the dump is `.v64` (byte-swapped) or `.n64` (little-endian), convert it first. The ROM tool (§6.1) should detect the byte order from the first four bytes (`80 37 12 40` = z64) and convert automatically.
- On first run, record the SHA-1 of the user's verified ROM in `rom/EXPECTED.sha1` locally (not committed) so later sessions can confirm they're working against the same file. Also verify the header CRCs above.
- Place the ROM at `baserom.z64` in the repo root. It **must** be in `.gitignore`.
- The game reportedly supports a higher-resolution mode with the Expansion Pak **(verify)**. Model RDRAM as 8 MB from day one so either mode works.

---

## 3. Existing work to build on

| Project | What it is | How we use it |
|---|---|---|
| [sp00nznet/racer](https://github.com/sp00nznet/racer) | N64 static recompilation of this exact ROM using N64Recomp. Boots to a stable main loop, loads assets, submits display lists to RT64. As of the snapshot, visible frame output was in progress and audio was stubbed. | **Primary scaffold and oracle.** Its `symbols.toml` (~878 named functions + ~500 statics) and recompiled C are our reference implementation. |
| [N64Recomp](https://github.com/N64Recomp/N64Recomp) | Static recompiler: MIPS → literal C, one C function per MIPS function. MIT licensed. | Generates the reference C code. |
| [N64ModernRuntime](https://github.com/N64Recomp/N64ModernRuntime) | Runtime (librecomp + ultramodern) that reimplements libultra: threads, message queues, DMA, VI, etc. | Runs the recompiled game. |
| [RT64](https://github.com/rt64/rt64) | Modern N64 graphics renderer (D3D12/Vulkan/Metal). | Renders the game's display lists until we have our own renderer. |
| [SW_RACER_RE](https://github.com/tim-tim707/SW_RACER_RE) | Non-matching decomp of the **PC** version (~71% of functions analysed as of June 2026). Also documents asset file formats (Blender addon). | Source of function names, struct layouts and file format knowledge. The PC and N64 builds share a lot of game code, but it's x86, so mapping is by logic and strings, not addresses. |
| [OpenSWE1R](https://github.com/OpenSWE1R) | Older PC-version reverse engineering. | Secondary reference. |
| [decompals/ultralib](https://github.com/decompals/ultralib) | Reconstructed libultra source. | Identifying and understanding SDK functions in the ROM. |

**First task in any fresh setup:** check the licence of each repo above. In particular, confirm sp00nznet/racer's licence before copying or forking any of its code. If it has none, contact the author before reusing code, or reuse only its *findings* (addresses, notes) and regenerate everything else ourselves.

---

## 4. Tools

### Required
| Tool | Purpose | Notes |
|---|---|---|
| Git (with submodules) | Source control | N64Recomp and the runtime use submodules; clone with `--recurse-submodules`. |
| Rust (stable, via rustup) | Main implementation language | Use a Cargo workspace. |
| C/C++ toolchain | Building recomp output, runtime, RT64 | N64Recomp needs C++20. sp00nznet/racer builds with MSVC / Visual Studio 2022 on Windows. On Linux/macOS use clang **(verify the runtime builds there)**. |
| CMake ≥ 3.20 | Building N64Recomp, runtime, RT64 | |
| Python 3.10+ | ROM tools, analysis scripts | Use a venv. |
| N64Recomp | MIPS → C recompiler | Build `N64RecompCLI` from source. Its README says it needs an ELF for metadata, but sp00nznet/racer drives it from `symbols.toml`, so newer versions evidently accept symbol files **(verify against current N64Recomp)**. |
| N64ModernRuntime + RT64 | Runtime and renderer | Pinned as submodules of the scaffold. |
| rabbitizer (`pip install rabbitizer`) | MIPS instruction decoding | For our own analysis scripts (call graphs, disassembly). |
| An accurate N64 emulator | Whole-game reference and RAM snapshots | **ares** for accuracy; **mupen64plus** (or a frontend with a debugger) for memory inspection and breakpoints. |

### Strongly recommended
| Tool | Purpose |
|---|---|
| Ghidra + an N64 ROM loader extension | Interactive exploration, cross-references, struct recovery. Import `symbols.toml` names via script. |
| splat (`pip install splat64`) + spimdisasm | Clean disassembly / segment splitting, and finding data vs code boundaries. |
| m2c | Quick first-draft C from assembly when the recompiled C is too literal to read. |
| Unicorn Engine (MIPS, big-endian) | Independent second oracle for pure functions, to catch any recompiler bugs. |
| cargo-nextest, proptest | Fast test runs; property-based input generation for differential tests. |

### Reference docs
- [n64brew wiki](https://n64brew.dev) — hardware, RDRAM, PI/SI/VI, RSP/RDP.
- F3DEX / F3DEX2 GBI documentation (display-list command formats). Identify which microcode the game uses early, since RT64 needs it (the scaffold calls `loadUCodeGBI` before processing display lists).
- VR4300 manual — CPU and FPU behaviour.

---

## 5. Architecture

### 5.1 The core idea

1. The recompiled game runs as-is. Every original function exists as a C function with the N64Recomp signature, roughly `void func_80012345(uint8_t* rdram, recomp_context* ctx)`.
2. We write a Rust replacement for one function, exported with that **same signature** (`extern "C"`), operating on the **same emulated RDRAM layout**.
3. At link time the Rust version overrides the recompiled one (N64Recomp's patch mechanism already relies on linker precedence for overrides). The game runs with the Rust function in place.
4. A test harness calls both versions with identical inputs and compares all outputs.
5. Once most of a subsystem is in Rust, we "lift" it: replace raw RDRAM access with native Rust structs, and turn the `ctx`-based signatures into normal Rust function calls internally.
6. Eventually the recompiled C and the runtime go away entirely.

### 5.2 Cargo workspace layout

```
racer-rs/
├── SPEC.md                 # this file
├── CLAUDE.md               # session rules for Claude Code (see §8)
├── NOTES.md                # running log of discoveries and decisions
├── baserom.z64             # user's ROM (gitignored)
├── scaffold/               # the recomp build (fork/submodule of sp00nznet/racer, or our own)
├── symbols/
│   └── functions.csv       # address, name, source, status, confidence, notes (see §7)
├── crates/
│   ├── rom/                # ROM verification, byte-order conversion, segment extraction
│   ├── n64mem/             # RDRAM model with typed big-endian accessors matching the runtime's layout
│   ├── oracle/             # builds recompiled C via the `cc` crate and exposes it over FFI (test-only)
│   ├── game/               # the Rust reimplementation, organised by subsystem
│   ├── difftest/           # differential test harness and state capture/replay
│   └── platform/           # later: windowing, input, audio, rendering (SDL3/winit + wgpu)
├── tools/                  # Python analysis scripts (call graph, string dump, Ghidra import)
└── xtask/                  # `cargo xtask` commands: verify-rom, recomp, build-scaffold, next-function
```

### 5.3 Memory model (important, verify first)

N64Recomp's runtime does **not** store RDRAM as plain big-endian bytes. Read the scaffold's recomp header (the `MEM_W` / `MEM_H` / `MEM_B` macros) and replicate its exact addressing in `n64mem`, including how virtual addresses (`0x80xxxxxx`) map to offsets and how byte and halfword accesses are swizzled. Every Rust function in the first phase must go through `n64mem` accessors, never raw pointer arithmetic, so that when the layout question is settled it's settled in one place.

Write a test on day one: poke known values through the C macros, read them back through `n64mem`, and vice versa.

### 5.4 Correctness rules for ported code

These are the classic ways a behaviour port silently diverges. Treat them as hard rules.

- **Integer arithmetic:** use `wrapping_*` ops wherever the MIPS code could overflow. Be explicit about signed vs unsigned; MIPS `slt` vs `sltu`, `lb` vs `lbu`, `lh` vs `lhu` all matter.
- **Register width:** the VR4300 is a 64-bit MIPS III CPU. Most game code will be 32-bit, but watch for `dadd`/`dsll`/`ld`/`sd`, and sign-extension of 32-bit results into 64-bit registers.
- **Floating point:** use `f32` where the original uses single precision, and never let a computation silently widen to `f64` or narrow back. Do not use `f32::mul_add`. Check whether the game changes the FPU rounding mode or flush-to-zero via `ctc1` (FCR31) and record it in NOTES.md.
- **Maths library:** never substitute Rust `std` `sin`/`cos`/`sqrt`/`atan2` for the game's own routines or lookup tables. Port the game's versions exactly; results feed physics and will diverge otherwise.
- **Random numbers:** port the game's RNG exactly, including where and how often it's called. RNG call order is part of the behaviour.
- **Undefined-ish behaviour:** if the original reads uninitialised memory, reads out of bounds, or depends on struct padding, reproduce it and leave a `// QUIRK:` comment. Cleaning it up is a separate, deliberate decision logged in NOTES.md.
- **Timing:** game logic is tied to the original frame/VI rate. Don't decouple it until the lift phase, and only with replay tests passing.

---

## 6. Phases and milestones

Each milestone has a concrete "done when".

### Phase 0 — Environment and ROM
- `cargo xtask verify-rom`: detects byte order, converts to z64 if needed, checks header name/code/CRCs, records SHA-1 locally.
- Build N64Recomp from source.
- **Done when:** ROM verified, N64Recomp builds, `.gitignore` covers ROM, extracted data, build output.

### Phase 1 — Scaffold and oracle
- Get the recompilation building (fork or submodule of sp00nznet/racer, subject to the licence check in §3). Note its current status: whether frames render yet, whether audio works.
- Create `oracle`: compile the recompiled C (not the full runtime) into a static library the Rust tests can call. Stub whatever a pure function needs (e.g. `LOOKUP_FUNC`) in a minimal test runtime.
- Create `n64mem` and the layout test from §5.3.
- Port one trivial leaf function (a string or memory utility, or small maths helper) to Rust and prove the differential test passes.
- **Done when:** `cargo test` runs one C-vs-Rust differential test on a real function from the ROM.

### Phase 2 — Mapping
- `tools/callgraph.py`: build the call graph from the recompiled C (direct calls) plus `LOOKUP_FUNC` sites (indirect calls, flagged separately). Output depth from leaves.
- Populate `symbols/functions.csv` from `symbols.toml`, libultra identification, and debug strings (the ROM contains source file names and assert strings, which are excellent anchors).
- Cross-reference SW_RACER_RE: match N64 functions to PC functions via shared strings, constants, and call structure. Record matches with a confidence level.
- Identify the graphics microcode and the asset compression format (SW_RACER_RE's Blender addon documents the file formats).
- **Done when:** every function in the table has a depth, a status, and either a name or a placeholder, and the call graph renders.

### Phase 3 — Leaves first
- Port functions in order of call-graph depth, starting with pure leaves: maths, vector/matrix ops, string/memory utilities, decompression, table lookups.
- Asset decompression is a high-value early target: it's self-contained, well documented from the PC side, and easy to verify by comparing decompressed output byte for byte.
- **Done when:** all depth-0 non-libultra functions are ported and verified.

### Phase 4 — Whole-game verification
Pick whichever reference runs the full game first:
- **Option A (preferred):** the scaffold renders and runs races. Add input recording/playback and a per-frame state dump (pod positions, velocities, timers, RNG state, race state) to it.
- **Option B:** until then, use an emulator. Capture RDRAM snapshots at chosen points (title screen, race start, mid-race) and use them to seed function-level tests with realistic state.
- Build replay tests: a recorded input sequence must produce identical state dumps between the pure-recomp build and the build with Rust replacements.
- **Done when:** at least one full-race replay passes with every Rust function swapped in so far.

### Phase 5 — Subsystems
Work upward through the call graph by subsystem. Suggested order, roughly by self-containment:
1. Asset loading and file formats
2. Maths and physics primitives
3. Pod physics and collision
4. AI racers
5. Race logic (laps, timing, positions, damage/repair)
6. Menus, UI, save data (Controller Pak)
7. Scene graph / render submission (still emitting display lists)

- **Done when:** a subsystem is fully Rust, replay tests pass, and it's been lifted from raw RDRAM access to native structs internally.

### Phase 6 — Platform layer
- Input: map controller/keyboard to the game's controller state.
- Audio: the scaffold stubs audio. Either recompile the audio microcode path via N64Recomp's RSP support, or reimplement the audio in Rust once the synth code and sound bank formats are understood.
- Rendering: start by feeding display lists to RT64; later, optionally, a native `wgpu` renderer driven directly by the scene graph (skipping display lists entirely).
- **Done when:** input, audio, and rendering run through `platform` with no dependency on the recomp runtime for those features.

### Phase 7 — Remove the scaffold
- Every game function is Rust; the recompiled C and N64ModernRuntime are no longer linked.
- Replay suite still passes against recorded reference dumps.
- **Done when:** `cargo run --release -- baserom.z64` launches the game, loading assets directly from the user's ROM.

---

## 7. Function tracking

`symbols/functions.csv` is the single source of truth for progress. Columns:

| Column | Meaning |
|---|---|
| `vram` | Address, e.g. `0x80012345` |
| `name` | Current name (`func_80012345` until understood) |
| `name_source` | `scaffold`, `libultra`, `sw_racer_re`, `strings`, `ours` |
| `depth` | Call-graph depth from leaves |
| `subsystem` | e.g. `math`, `physics`, `ai`, `ui` |
| `status` | `recomp` → `rust_draft` → `rust_verified` → `lifted` |
| `test_kind` | `unit_diff`, `snapshot_diff`, `replay` |
| `confidence` | Name/semantics confidence: `low` / `med` / `high` |
| `notes` | Short free text; longer notes go in NOTES.md |

`cargo xtask next-function` should propose the next target: lowest depth, all callees already `rust_verified`, preferring the current subsystem.

---

## 8. Claude Code session workflow

Put a condensed version of this section in `CLAUDE.md`.

### Per-function loop
1. Pick the target (`cargo xtask next-function`, or as directed).
2. Read the recompiled C, the disassembly, and any SW_RACER_RE / Ghidra notes for it.
3. Write down what the function does in one or two sentences before writing code.
4. Write the Rust version against `n64mem`, following §5.4.
5. Write the differential test: random inputs via proptest, plus edge cases (zero, negative, max values, NaN/inf for floats), plus any captured real-state snapshots.
6. Run it. If it diverges, find out why; never "fix" the test to match.
7. Update `functions.csv` and, if something non-obvious was learned, NOTES.md.
8. Commit with a message naming the function and what it does.

### Rules
- Never commit the ROM, anything extracted from it, RAM snapshots, or recorded state dumps derived from it. Keep them in gitignored directories.
- Never hand-edit generated recomp output. Fixes go in the scaffold's patch mechanism or in Rust replacements.
- Don't port a function whose callees aren't verified, unless explicitly asked; stub the dependency through the oracle instead.
- Don't "clean up" quirks during porting. Mark them `// QUIRK:` and move on.
- End each session by updating NOTES.md with: what was done, what's in progress, anything surprising, and the suggested next step.

---

## 9. Legal hygiene

- Users provide their own ROM; the repo contains no ROM bytes, assets, or extracted data.
- Rust code is an original reimplementation written for interoperability and preservation. Don't copy code from other projects without checking and honouring their licence (see §3).
- Star Wars Episode I: Racer is a trademark of Lucasfilm Ltd. The project should say so and make no claim of affiliation.
- This is not legal advice; decompilation and reimplementation law varies by country.

---

## 10. Open questions to resolve early

1. What is sp00nznet/racer's licence, and do we fork it, submodule it, or rebuild the recomp ourselves from its findings?
2. Does the current scaffold render frames yet? This decides Phase 4 option A vs B.
3. What is N64Recomp's exact RDRAM layout and function signature in the version we pin? (§5.3)
4. Which graphics microcode does the game use?
5. Does the game touch FPU control registers (rounding / flush-to-zero)?
6. Which compiler built the original (IDO vs GCC)? Not needed for matching, but it affects idioms in the assembly and how N64Recomp handles some constructs.
7. Are there overlays or code loaded outside the main segment?
