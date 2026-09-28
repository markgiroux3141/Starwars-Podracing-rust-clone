//! The ROM at test time, and test doubles for the game's ROM reads.
//!
//! `rom_read` (`func_80011CDC`) and `rom_read_small` (`func_80011D60`) sit on
//! top of the PI/message-queue chain (`func_80011B18` / `func_80011BDC` →
//! `osPiStartDma` / `osPiReadIo`, `osRecvMesg`, and `func_80008F28` between
//! chunks), which has indirect calls below it and can't be verified yet. The
//! doubles here replace the whole chain with a copy from baserom.z64, read
//! when the tests run. Nothing ROM-derived is stored in the repository.
//!
//! What the doubles reproduce (NOTES.md, "Test doubles for the ROM reads"):
//! - the data: exactly `size` bytes from ROM offset `a0` to RDRAM `a1`, and
//!   nothing if `size <= 0` (both originals test `blez` on the full register);
//! - the saved registers: `s0`-`s3`, `ra` and `sp` come back as their
//!   sign-extended low words, as the originals' `sw`/`lw` pairs leave them;
//! - the rest of the register file as the o32 ABI allows, with pseudo-random
//!   values ([`doubles::clobber_caller_saved`]).
//!
//! What they don't: the stack below `sp` (the real chain writes frames there),
//! the PI and message-queue state, and whatever `func_80008F28` does between
//! chunks. Requests outside what the originals handle cleanly abort the test:
//! PI DMA needs an 8-byte-aligned RDRAM address, a 2-byte-aligned ROM address
//! and an even length, and `rom_read_small` stores whole words (`sw`) to a
//! word-aligned destination from word-aligned ROM reads. Every transfer the
//! asset loaders make in the USA ROM meets these.

use game::recomp::{reg::*, RecompContext};
use n64mem::Mem;
use oracle::doubles::{self, Installed};
use std::path::PathBuf;
use std::sync::OnceLock;

/// baserom.z64 from the repo root. The oracle can't be built without it
/// (generated/ is ROM-derived), so its absence is an error, not a skip.
pub fn baserom() -> &'static [u8] {
    static ROM: OnceLock<Vec<u8>> = OnceLock::new();
    ROM.get_or_init(|| {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baserom.z64");
        let r = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}; run `cargo xtask verify-rom`", p.display()));
        assert_eq!(&r[..4], [0x80, 0x37, 0x12, 0x40], "baserom.z64 is not z64");
        assert_eq!(&r[0x20..0x33], b"STAR WARS EP1 RACER");
        r
    })
}

pub const ROM_READ: &str = "func_80011CDC";
pub const ROM_READ_SMALL: &str = "func_80011D60";

fn canonical(ctx: &RecompContext, r: usize, what: &str) -> u32 {
    let v = ctx.gpr[r];
    assert_eq!(v, v as u32 as i32 as i64 as u64, "{what}: register {r} = {v:#x} is not a sign-extended 32-bit value");
    v as u32
}

/// The arguments of either read, or None if `size <= 0`.
fn args(ctx: &RecompContext, name: &str, rom_len: usize) -> Option<(usize, u32, usize)> {
    let rom = canonical(ctx, A0, name);
    let dram = canonical(ctx, A1, name);
    let size = canonical(ctx, A2, name) as i32;
    if size <= 0 {
        return None;
    }
    let (rom, size) = (rom as usize, size as usize);
    assert!(rom + size <= rom_len, "{name}: ROM {rom:#x}+{size:#x} is past the end of the ROM");
    Some((rom, dram, size))
}

fn finish(ctx: &mut RecompContext) {
    doubles::restore_saved(ctx, &[S0, S1, S2, S3, RA, SP]);
    doubles::clobber_caller_saved(ctx, &[]);
}

/// `rom_read(rom, dram, size)`: PI DMA in 0x800-byte chunks.
pub fn rom_read(rom: &'static [u8]) -> impl FnMut(&mut Mem, &mut RecompContext) {
    move |mem, ctx| {
        if let Some((src, dram, size)) = args(ctx, ROM_READ, rom.len()) {
            assert!(
                dram % 8 == 0 && src % 2 == 0 && size % 2 == 0,
                "{ROM_READ}: ROM {src:#x} -> {dram:#010x}, {size:#x} bytes is not a clean PI DMA"
            );
            mem.write_bytes(dram, &rom[src..src + size]);
        }
        finish(ctx);
    }
}

/// `rom_read_small(rom, dram, size)`: word reads through `osPiReadIo`, stored
/// with `sw`, then the last `size % 4` bytes one at a time.
pub fn rom_read_small(rom: &'static [u8]) -> impl FnMut(&mut Mem, &mut RecompContext) {
    move |mem, ctx| {
        if let Some((src, dram, size)) = args(ctx, ROM_READ_SMALL, rom.len()) {
            assert!(
                dram % 4 == 0 && src % 4 == 0,
                "{ROM_READ_SMALL}: ROM {src:#x} -> {dram:#010x} is not word-aligned"
            );
            mem.write_bytes(dram, &rom[src..src + size]);
        }
        finish(ctx);
    }
}

/// Install both ROM-read doubles on this thread, reading baserom.z64.
pub fn install_rom_doubles() -> [Installed; 2] {
    let rom = baserom();
    [doubles::install(ROM_READ, rom_read(rom)), doubles::install(ROM_READ_SMALL, rom_read_small(rom))]
}
