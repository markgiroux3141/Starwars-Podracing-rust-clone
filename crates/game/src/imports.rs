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
    func_80030328, // texture_read
    func_800304AC, // texture_get
    func_800827C0, // model_load's error path (not yet understood)
}
