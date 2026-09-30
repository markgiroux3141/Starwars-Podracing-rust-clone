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
    func_80005BB8, // object start
    func_80005CAC, // key fraction
    func_80006704, // key segment
    func_80006D5C, // animation object by id and kind
    func_80006EB4, // [o + 0x110] = x
    func_80007A44, // slot +0x18 clear
    func_80007CE4, // handle lookup
    func_80008718, // special sound id
    func_8000A44C, // record init
    func_8000A920, // record or global on/off
    func_8000AA04, // record +0/+2 halfwords
    func_8000AA78, // record +4/+6 halfwords
    func_8000AAC0, // record +8/+0xC floats
    func_8000AB24, // record or global colour bytes
    func_8000ABD4, // record texture pixels
    func_8000AC34, // record flags |=
    func_8000AED4, // entry flags |=
    func_8000AEFC, // entry +6/+4/+8
    func_8000C5F0, // push current id
    func_8000C6C8, // float clamp-add
    func_8000C724, // int clamp-add
    func_8000DA6C, // [0x8009B7E4]
    func_8000E8C4, // node tree first word (self)
    func_8000E9BC, // six optional bytes
    func_8000EA4C, // node tree bytes (self)
    func_8000F5A0, // depth probes
    func_8000FD74, // 2x8 table entry
    func_8000FEAC, // marker word and triple
    func_8000FF54, // light triple k
    func_80011940, // comp_decompress
    func_80011CDC, // rom_read
    func_80011D60, // rom_read_small
    func_80011E54, // select pointer k
    func_800125E4, // texture load
    func_800129E4, // text width
    func_80014D4C, // asin in degrees
    func_80014F54, // atan2 in degrees
    func_80015190, // vec2 multiply-add
    func_800151C0, // vec2 length
    func_80015268, // vec3 set
    func_80015288, // vec3 copy
    func_800152CC, // vec3 equal
    func_80015328, // vec3 add
    func_8001535C, // vec3 subtract
    func_800153C0, // vec3 length
    func_80015470, // vec3 distance
    func_80015538, // vec3 cross
    func_800155C0, // vec3 scale
    func_800155EC, // vec3 multiply-add
    func_800156DC, // 4x4 copy
    func_80015724, // 4x4 product
    func_800160BC, // matrix inverse
    func_80016260, // ludcmp (n = 3)
    func_800167E4, // lubksb (n = 3)
    func_80016BF4, // vec3 x 3x3
    func_80016CAC, // point x 4x4
    func_80016DD8, // vec4 x 4x4
    func_80017874, // 4x4 identity
    func_80017918, // 4x4 rows scaled
    func_80017BA8, // 4x4 to node transform
    func_80017C18, // node transform to 4x4
    func_80017C98, // node transform to 4x4 (same code)
    func_80017DA4, // node type word
    func_80017DAC, // [o + 0x14]
    func_80017DB4, // child k
    func_80017E70, // node +8 setter
    func_80017EE4, // [o + 4]
    func_80017EEC, // [o + 4] = v
    func_80017EF4, // [o]
    func_80017F20, // returns 4
    func_80017F28, // RECORDS_170 + 0x170 k
    func_800181BC, // node flags walk (self)
    func_80018324, // node header init
    func_800183A8, // [o + 4] (second)
    func_80018450, // empty (two arguments)
    func_80029A3C, // profile record reset
    func_8002D968, // three bytes equal (bit 14)
    func_8002D9D0, // 4 or 3
    func_8002DAD0, // unlock bit
    func_8002EA28, // pads update
    func_8002F054, // [0x800A26F4]
    func_8002F060, // f0 = [0x800D7740]
    func_8002FAC4, // heap_set_cursor
    func_8002FAFC, // heap_cursor
    func_8002FC58, // heap_free
    func_8002FC80, // heap_check
    func_8002FF38, // sprite_load
    func_80030328, // texture_read
    func_800304AC, // texture_get
    func_800314DC, // channel set
    func_80031560, // channel start (self-call for all four)
    func_800315D8, // channel +0xC = 0 (self-call)
    func_80031640, // channel +8 = 0 (self-call)
    func_800321F0, // stat update
    func_80033E08, // Mtx ring next
    func_800344F4, // 4x4 float to Mtx
    func_80034650, // 4x3 float to Mtx
    func_8003609C, // render mode switches
    func_80038DF8, // six halfwords
    func_800390C0, // crc32_table_init
    func_8003B250, // start a spline walker
    func_8003B300, // four screen words
    func_8003D110, // render state reset
    func_8003D488, // [0x800A48D4] = v & 0xFFFF
    func_8003E0A0, // texture scroll
    func_8003F714, // pool element by tag
    func_8003F7B8, // pool count by id
    func_8003F800, // pool iteration begin
    func_8003F99C, // element callback
    func_8003FA24, // pool broadcast
    func_8003FB78, // pool count and base
    func_8003FDCC, // nearest pool elements
    func_8004110C, // save a pose
    func_8004E488, // mask bits set/clear
    func_80051FF4, // first zero of four words
    func_80052134, // progress figure
    func_80073C58, // float clamp
    func_8007531C, // material collect (self)
    func_80075490, // material hand-out (self)
    func_8007B430, // first textured material (self)
    func_8007B544, // set a tree's animations (self)
    func_80081530, // n bytes equal
    func_80081700, // 1 - r / (r + a)
    func_80081730, // point x 4x4 (w = 1)
    func_80081A2C, // closest point on a segment
    func_800827C0, // model_load's error path (not yet understood)
    func_80087814, // queue a screen rectangle
    func_8008A750, // cosf
    func_8008A8C0, // sinf
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
