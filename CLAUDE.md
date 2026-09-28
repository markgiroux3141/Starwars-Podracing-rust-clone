# racer-rs — session rules

Behaviour-accurate Rust reimplementation of Star Wars Episode I: Racer (N64, USA), built on a static recompilation. Full spec: [RACER_RS_SPEC.md](RACER_RS_SPEC.md). Discoveries and decisions: [NOTES.md](NOTES.md). Progress: `symbols/functions.csv`.

## Start of session
- `cargo xtask verify-rom` must pass (checks `baserom.z64` against `rom/EXPECTED.sha1`).
- Read the latest session entry in NOTES.md.

## Per-function loop
1. Pick the target (`cargo xtask next-function`, or as directed).
2. Read the recompiled C, the disassembly, and any SW_RACER_RE / Ghidra notes.
3. Write down what the function does in one or two sentences before coding.
4. Write the Rust version against `n64mem` accessors only (no raw pointer arithmetic).
5. Differential test: proptest random inputs + edge cases (0, negative, max, NaN/inf) + captured snapshots where available.
6. Run it. If it diverges, find out why. **Never adjust the test to match.**
7. Update `symbols/functions.csv`, and NOTES.md if something non-obvious was learned.
8. Commit, naming the function and what it does.

## Correctness rules (SPEC §5.4)
- `wrapping_*` wherever MIPS could overflow; be explicit about signed vs unsigned (`slt`/`sltu`, `lb`/`lbu`, `lh`/`lhu`).
- Watch 64-bit ops and sign-extension of 32-bit results.
- `f32` stays `f32`: no silent widening, no `mul_add`. Honour local FCR31 rounding-mode switches around `cvt.w.s` (see NOTES.md).
- Never substitute `std` maths for the game's routines/tables. Port the RNG exactly, including call order.
- Reproduce uninitialised reads, OOB reads and padding dependence, marked `// QUIRK:`. Don't clean up quirks while porting.
- Keep logic tied to the original frame rate until the lift phase.

## Hard rules
- Never commit the ROM, anything extracted from it, RAM snapshots or state dumps. They live in gitignored dirs.
- Never hand-edit generated recomp output; use the patch mechanism or Rust replacements.
- Don't port a function whose callees aren't verified unless asked; stub through the oracle.
- Don't copy code from other projects without checking their licence (SPEC §3, §9).
- End each session by updating NOTES.md: done, in progress, surprises, suggested next step.
