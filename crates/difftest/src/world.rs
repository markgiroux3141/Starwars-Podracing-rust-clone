//! A realistic machine state for the asset loaders' tests: the heap, texture
//! cache and globals as the boot code leaves them, and the asset blocks from
//! baserom.z64 (read at test time).
//!
//! Heap state comes from the code that sets it up at boot (NOTES.md, "Asset
//! heap"), in the Expansion Pak configuration unless a test says otherwise:
//! - heap end `[0x800D9DBC]` = `[0x80114538]` − 0x1400, with `[0x80114538]`
//!   the third framebuffer that func_80039A30 computes (8 MB, 640 wide,
//!   32-bit): 0x8063B800;
//! - mask buffer `[0x80114528]` = 0x8014D800 and cursor 0 = 0x80198820, as
//!   func_80030B90 leaves them right after heap_init (start 0x8014D7E0).
//!   Other boot-time allocations after that aren't modelled (**guess**: at
//!   the first load the cursor is somewhere above this);
//! - level 0 with slots 1..9 zero, texture count 1648 (the block's count, as
//!   texture_block_init stores it), texture cache zeroed (texture_block_init),
//!   `[0x800A2864]` = 0.

use crate::rom::baserom;
use crate::State;
use assets::Blocks;
use game::heap::{CURSORS, HEAP_END, HEAP_TOP, LEVEL};
use game::loader::{MASK_BUFFER, OUT_OF_HEAP, TEXTURE_CACHE, TEXTURE_COUNT};
use game::recomp::reg::*;
use std::sync::OnceLock;

/// Where the tests put `sp`.
pub const STACK: u32 = 0x800A_2800;
/// Texture cache words texture_block_init clears.
pub const CACHE_WORDS: u32 = 1700;

/// Heap configuration at the first load.
#[derive(Clone, Copy, Debug)]
pub struct Heap {
    pub cursor: u32,
    pub end: u32,
    pub top: u32,
    pub mask: u32,
}

/// With the Expansion Pak (the ROM's hi-res mode).
pub const HEAP_8MB: Heap = Heap { cursor: 0x8019_8820, end: 0x8063_A400, top: 0x8063_B800, mask: 0x8014_D800 };
/// Without: 320 wide, 16-bit framebuffers, no reserve.
pub const HEAP_4MB: Heap = Heap { cursor: 0x8017_3020, end: 0x8038_F800, top: 0x8038_F800, mask: 0x8014_D800 };

/// Sign-extend a 32-bit value into a register.
pub fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

pub fn align(v: u32, a: u32) -> u32 {
    (v + a - 1) & !(a - 1)
}

/// The four asset blocks of baserom.z64.
pub fn blocks() -> &'static Blocks<'static> {
    static B: OnceLock<Blocks<'static>> = OnceLock::new();
    B.get_or_init(|| Blocks::read(baserom()).expect("asset blocks"))
}

/// Random registers from `seed`, `sp` at [`STACK`], and the heap, texture
/// cache and flags as above.
pub fn world(seed: u64, heap: Heap) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(STACK);
    let mut m = s.rdram.mem();
    m.write_u32(LEVEL, 0);
    m.write_u32(CURSORS, heap.cursor);
    for k in 1..10 {
        m.write_u32(CURSORS + 4 * k, 0);
    }
    m.write_u32(HEAP_END, heap.end);
    m.write_u32(HEAP_TOP, heap.top);
    m.write_u32(MASK_BUFFER, heap.mask);
    m.write_u32(TEXTURE_COUNT, blocks().textures.count as u32);
    for k in 0..CACHE_WORDS {
        m.write_u32(TEXTURE_CACHE + 4 * k, 0);
    }
    m.write_u32(OUT_OF_HEAP, 0);
    s
}

pub fn words(s: &mut State, at: u32, n: usize) -> Vec<u32> {
    let m = s.rdram.mem();
    (0..n).map(|k| m.read_u32(at + 4 * k as u32)).collect()
}

pub fn bytes(s: &mut State, at: u32, n: usize) -> Vec<u8> {
    let mut v = vec![0; n];
    s.rdram.mem().read_bytes(at, &mut v);
    v
}
