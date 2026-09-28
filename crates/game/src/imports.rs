//! The recompiled functions that ports call, by their N64Recomp C symbols.
//!
//! A port calls a callee as `recomp::call(imports::func_X, &mut mem, ctx)`.
//! Whatever links `game` defines these: the oracle in the tests (the
//! recompiled C if it is compiled in, else a stub that runs a test double or
//! traps), and later the game build (the recompiled C, or a Rust port
//! exported under the same name).

use crate::recomp::recomp_imports;

recomp_imports! {
    func_80011940, // comp_decompress
    func_80011CDC, // rom_read
    func_80011D60, // rom_read_small
    func_8002FAC4, // heap_set_cursor
    func_8002FAFC, // heap_cursor
    func_8002FC58, // heap_free
    func_8002FC80, // heap_check
    func_8002FF38, // sprite_load
    func_80030328, // texture_read
    func_800304AC, // texture_get
    func_800827C0, // model_load's error path (not yet understood)
}

/// Hooks into the runtime that N64Recomp's generated code calls (recomp.h),
/// for ports that must do the same. Like the functions above, whoever links
/// `game` defines them: the oracle's stub runtime in the tests (every hook
/// traps), the runtime in the game build.
pub mod runtime {
    #[cfg(not(test))]
    extern "C" {
        /// What N64Recomp emits for a branch to itself (`b .`), the idle loop
        /// of a thread with nothing left to do. It doesn't return.
        pub fn pause_self(rdram: *mut u8);

        /// What N64Recomp emits as a jump table's `default:`, an index that
        /// matched no case: the function's name, the `jr` and the table's
        /// address. The generated C carries on after the switch if it returns.
        pub fn switch_error(func: *const core::ffi::c_char, vram: u32, jtbl: u32);

        /// What N64Recomp emits for `break` (in the game, IDO's checks after
        /// `div`: a zero divisor, `INT_MIN / -1`), with the instruction's
        /// address. The generated C carries on after it if it returns.
        pub fn do_break(vram: u32);
    }

    #[cfg(test)]
    #[no_mangle]
    pub unsafe extern "C" fn pause_self(_rdram: *mut u8) {
        eprintln!("pause_self is only linked in the oracle or the game build, not game's unit tests");
        std::process::abort();
    }

    #[cfg(test)]
    #[no_mangle]
    pub unsafe extern "C" fn switch_error(_func: *const core::ffi::c_char, _vram: u32, _jtbl: u32) {
        eprintln!("switch_error is only linked in the oracle or the game build, not game's unit tests");
        std::process::abort();
    }

    #[cfg(test)]
    #[no_mangle]
    pub unsafe extern "C" fn do_break(_vram: u32) {
        eprintln!("do_break is only linked in the oracle or the game build, not game's unit tests");
        std::process::abort();
    }
}
