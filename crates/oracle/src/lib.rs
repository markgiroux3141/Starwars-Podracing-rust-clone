//! The reference implementation: N64Recomp's C, called over FFI from tests.
//!
//! `build.rs` compiles the generated C for each function listed in
//! `functions.txt` (from `generated/`, see `cargo xtask recomp`) against a
//! minimal stub runtime (`c/stub_runtime.c`). Anything outside a pure
//! function's reach (indirect calls, jump-table misses, `break`, syscalls,
//! COP0, calls to functions not compiled in) traps: it prints what was hit and
//! aborts the test process.

use std::ffi::{c_char, CStr};

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

/// `recomp_context` as the C compiler lays it out (`c/ctx_shim.c`).
pub mod ctx {
    use game::recomp::RecompContext;

    extern "C" {
        pub fn ctx_layout_count() -> usize;
        pub fn ctx_layout(out: *mut usize);
        pub fn ctx_poke(ctx: *mut RecompContext);
        pub fn ctx_write_f_odd(ctx: *mut RecompContext, n: i32, v: u32);
    }

    /// Sizes and offsets, in the order of the enum in `ctx_shim.c`.
    pub fn layout() -> Vec<usize> {
        // SAFETY: ctx_layout writes exactly ctx_layout_count() entries.
        unsafe {
            let mut v = vec![0usize; ctx_layout_count()];
            ctx_layout(v.as_mut_ptr());
            v
        }
    }
}

/// The stub runtime's entry points (`c/stub_runtime.c`). Every one traps.
pub mod runtime {
    use std::ffi::c_char;

    extern "C" {
        pub fn get_function(vram: i32) -> *const u8;
        pub fn switch_error(func: *const c_char, vram: u32, jtbl: u32);
        pub fn do_break(vram: u32);
        pub fn pause_self(rdram: *mut u8);
        pub fn oracle_unexpected_call(name: *const c_char);
    }
}

/// The recompiled functions compiled into the oracle.
pub mod recomp {
    include!(concat!(env!("OUT_DIR"), "/oracle_funcs.rs"));

    /// Look a compiled-in function up by name.
    pub fn by_name(name: &str) -> Option<game::recomp::RecompFn> {
        FUNCTIONS.iter().find(|(n, _)| *n == name).map(|&(_, f)| f)
    }
}

/// Called by the stub runtime when generated code reaches something the
/// oracle doesn't provide. There is no way to unwind back through the C
/// frames, so this aborts the whole test process, loudly.
#[no_mangle]
pub extern "C" fn oracle_trap(msg: *const c_char) -> ! {
    // SAFETY: the C side always passes a NUL-terminated buffer.
    let msg = unsafe { CStr::from_ptr(msg) }.to_string_lossy();
    eprintln!("\n*** oracle trap: {msg}\n*** aborting the test process");
    std::process::abort()
}
