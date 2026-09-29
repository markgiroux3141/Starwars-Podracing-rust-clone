//! The recompiled functions that ports call, by their N64Recomp C symbols.
//!
//! A port calls a callee as `recomp::call(imports::func_X, &mut mem, ctx)`.
//! Whatever links `game` defines these: the oracle in the tests (the
//! recompiled C if it is compiled in, else a stub that runs a test double or
//! traps), and later the game build (the recompiled C, or a Rust port
//! exported under the same name).

use crate::recomp::recomp_imports;

recomp_imports! {
    func_8000097C, // collision record step
    func_8000550C, // matrix stack reset
    func_800059A8, // matrix stack top
    func_80005B80, // object table clear
    func_80005CAC, // key fraction
    func_80006704, // key segment
    func_80007A44, // slot +0x18 clear
    func_80007CE4, // handle lookup
    func_80008718, // special sound id
    func_8000A44C, // record init
    func_8000AED4, // entry flags |=
    func_80011940, // comp_decompress
    func_80011CDC, // rom_read
    func_80011D60, // rom_read_small
    func_80015538, // vec3 cross
    func_800155EC, // vec3 multiply-add
    func_800160BC, // matrix inverse
    func_80016BF4, // vec3 x 3x3
    func_80016CAC, // point x 4x4
    func_80017874, // 4x4 identity
    func_80017C18, // node transform to 4x4
    func_80017C98, // node transform to 4x4 (same code)
    func_80017DA4, // node type word
    func_80017E70, // node +8 setter
    func_80017EE4, // [o + 4]
    func_80017EEC, // [o + 4] = v
    func_8002FAC4, // heap_set_cursor
    func_8002FAFC, // heap_cursor
    func_8002FC58, // heap_free
    func_8002FC80, // heap_check
    func_8002FF38, // sprite_load
    func_80030328, // texture_read
    func_800304AC, // texture_get
    func_80031560, // channel start (self-call for all four)
    func_800315D8, // channel +0xC = 0 (self-call)
    func_80031640, // channel +8 = 0 (self-call)
    func_8003D488, // [0x800A48D4] = v & 0xFFFF
    func_8003F7B8, // pool count by id
    func_80051FF4, // first zero of four words
    func_80081A2C, // closest point on a segment
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

        /// N64Recomp's `LOOKUP_FUNC`: the function starting at `vram` (the
        /// register's low word), for an indirect call (`jalr`). Ports call
        /// what it returns with [`crate::recomp::call`], as the generated C
        /// does. In the oracle it resolves like a direct call (the
        /// compiled-in C, or a stub running a double) and traps for an
        /// address where no function starts.
        pub fn get_function(vram: i32) -> Option<crate::recomp::RecompFn>;
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
    pub unsafe extern "C" fn get_function(_vram: i32) -> Option<crate::recomp::RecompFn> {
        eprintln!("get_function is only linked in the oracle or the game build, not game's unit tests");
        std::process::abort();
    }

    #[cfg(test)]
    #[no_mangle]
    pub unsafe extern "C" fn do_break(_vram: u32) {
        eprintln!("do_break is only linked in the oracle or the game build, not game's unit tests");
        std::process::abort();
    }
}
