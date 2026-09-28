//! The reference implementation: N64Recomp's C, called over FFI from tests.
//!
//! For now this only exposes the memory macros (see `c/layout_shim.c`) so
//! n64mem's layout can be checked against them. Recompiled game functions will
//! be added here as they are targeted.

/// Raw access to N64Recomp's `MEM_*` / `LD` / `SD` macros.
pub mod layout {
    extern "C" {
        pub fn layout_write_w(rdram: *mut u8, vaddr: u32, v: i32);
        pub fn layout_write_h(rdram: *mut u8, vaddr: u32, v: i16);
        pub fn layout_write_b(rdram: *mut u8, vaddr: u32, v: i8);
        pub fn layout_write_d(rdram: *mut u8, vaddr: u32, v: u64);
        pub fn layout_read_w(rdram: *mut u8, vaddr: u32) -> i32;
        pub fn layout_read_h(rdram: *mut u8, vaddr: u32) -> i16;
        pub fn layout_read_hu(rdram: *mut u8, vaddr: u32) -> u16;
        pub fn layout_read_b(rdram: *mut u8, vaddr: u32) -> i8;
        pub fn layout_read_bu(rdram: *mut u8, vaddr: u32) -> u8;
        pub fn layout_read_d(rdram: *mut u8, vaddr: u32) -> u64;
    }
}
