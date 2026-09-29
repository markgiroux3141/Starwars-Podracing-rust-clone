//! The reference implementation: N64Recomp's C, called over FFI from tests.
//!
//! `build.rs` compiles the generated C for each function listed in
//! `functions.txt` (from `generated/`, see `cargo xtask recomp`) against a
//! minimal stub runtime (`c/stub_runtime.c`). Anything outside a pure
//! function's reach (indirect calls, jump-table misses, `break`, syscalls,
//! COP0, calls to functions not compiled in) traps: it prints what was hit and
//! aborts the test process. A test can supply a [`doubles`] implementation for
//! a function that is not compiled in.

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
/// recomp.h's float conversions under each rounding mode (`c/fpu_probe.c`),
/// to check `game::recomp::fpu` against what generated code computes.
pub mod fpu_probe {
    extern "C" {
        pub fn fpu_probe_cvt_w_s(x: f32, mode: u32) -> i32;
        pub fn fpu_probe_cvt_w_d(x: f64, mode: u32) -> i32;
        pub fn fpu_probe_trunc_w_s(x: f32) -> i32;
        pub fn fpu_probe_trunc_w_d(x: f64) -> i32;
        pub fn fpu_probe_cvt_s_w(x: i32, mode: u32) -> f32;
        pub fn fpu_probe_cvt_s_d(x: f64, mode: u32) -> f32;
        pub fn fpu_probe_get_cop1_cs() -> u32;
        pub fn fpu_probe_asserts_on() -> i32;
    }
}

/// recomp.h's unaligned word accesses on a caller's RDRAM buffer
/// (`c/unaligned_probe.c`), to check `game::recomp`'s `lwl`/`lwr`/`swl`/`swr`.
pub mod unaligned_probe {
    extern "C" {
        pub fn unaligned_probe_lwl(rdram: *mut u8, initial: u64, base: u64, offset: u64) -> u64;
        pub fn unaligned_probe_lwr(rdram: *mut u8, initial: u64, base: u64, offset: u64) -> u64;
        pub fn unaligned_probe_swl(rdram: *mut u8, base: u64, offset: u64, value: u64);
        pub fn unaligned_probe_swr(rdram: *mut u8, base: u64, offset: u64, value: u64);
    }
}

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

/// Test doubles: Rust stand-ins for recompiled functions that are not compiled
/// into the oracle.
///
/// Every recompiled function not listed in `functions.txt` is a generated
/// stub that calls [`oracle_callee`] with its name. If the current thread has
/// [`install`]ed a double under that name, the double runs; otherwise the stub
/// traps. The recompiled caller and its Rust port reach the same stub (both
/// call the C symbol), so they run the same double.
///
/// Doubles are per thread (each test runs on its own thread) and last until
/// the returned [`Installed`] guard is dropped. Every call through a stub is
/// recorded with the registers at entry; `difftest` compares the C run's
/// calls with the Rust run's.
///
/// Only functions listed in `crates/oracle/doubles.txt` can have doubles
/// ([`LISTED`]); the list says whether each double reproduces the function's
/// contract or only stands in for it, and `cargo xtask next-function` reads it.
pub mod doubles {
    use game::recomp::{reg::*, s32, RecompContext};
    use n64mem::Mem;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fmt;
    use std::rc::Rc;

    type Body = Rc<RefCell<dyn FnMut(&mut Mem, &mut RecompContext)>>;

    /// How far a listed double goes (doubles.txt).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Kind {
        /// Reproduces the function's observable effects as NOTES.md documents
        /// them, so callers can be verified against it.
        Contract,
        /// Gets a test past a call; claims nothing about the function.
        StandIn,
    }

    include!(concat!(env!("OUT_DIR"), "/oracle_doubles.rs"));

    /// The kind `name` is listed with in doubles.txt, if any.
    pub fn listed(name: &str) -> Option<Kind> {
        LISTED.iter().find(|(n, _)| *n == name).map(|&(_, k)| k)
    }

    thread_local! {
        static DOUBLES: RefCell<HashMap<&'static str, Body>> = RefCell::new(HashMap::new());
        static TRACE: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    }

    /// One call through a stub: which function, and the GPRs at entry.
    #[derive(Clone, PartialEq, Eq)]
    pub struct Call {
        pub name: &'static str,
        pub gpr: [u64; 32],
    }

    impl fmt::Debug for Call {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let g = &self.gpr;
            write!(
                f,
                "{}(a0 {:#x}, a1 {:#x}, a2 {:#x}, a3 {:#x}; sp {:#x})",
                self.name, g[A0], g[A1], g[A2], g[A3], g[SP]
            )
        }
    }

    /// Keeps a double installed; dropping it puts back whatever was there.
    #[must_use = "the double is uninstalled when this guard is dropped"]
    pub struct Installed {
        name: &'static str,
        prev: Option<Body>,
    }

    impl Drop for Installed {
        fn drop(&mut self) {
            DOUBLES.with_borrow_mut(|d| match self.prev.take() {
                Some(p) => d.insert(self.name, p),
                None => d.remove(self.name),
            });
        }
    }

    /// Run `body` whenever the recompiled function `name` is called on this
    /// thread. `name` must be a function not compiled into the oracle (one
    /// that is compiled in is called directly and never reaches a double),
    /// and must be listed in doubles.txt.
    pub fn install(name: &'static str, body: impl FnMut(&mut Mem, &mut RecompContext) + 'static) -> Installed {
        assert!(
            !crate::recomp::FUNCTIONS.iter().any(|(n, _)| *n == name),
            "{name} is compiled into the oracle, so a double for it would never run"
        );
        assert!(
            listed(name).is_some(),
            "{name} is not listed in crates/oracle/doubles.txt; add it as `contract` or `stand-in`"
        );
        let body: Body = Rc::new(RefCell::new(body));
        let prev = DOUBLES.with_borrow_mut(|d| d.insert(name, body));
        Installed { name, prev }
    }

    /// Clear this thread's call record.
    pub fn start_trace() {
        TRACE.with_borrow_mut(Vec::clear);
    }

    /// Take this thread's call record.
    pub fn take_trace() -> Vec<Call> {
        TRACE.with_borrow_mut(std::mem::take)
    }

    /// What a function that follows the o32 ABI leaves behind, for doubles
    /// of functions whose real register effects we can't reproduce (their
    /// callees aren't verified). Registers the ABI lets a callee change, other
    /// than `keep`, get pseudo-random values: `at`, `v0`-`v1`, `a0`-`a3`,
    /// `t0`-`t9`, `hi`, `lo` and `f0`-`f19`. So a port that wrongly relies on
    /// one surviving the call diverges. The values depend only on the call's
    /// position in the trace, so the C and Rust runs get the same ones.
    pub fn clobber_caller_saved(ctx: &mut RecompContext, keep: &[usize]) {
        let n = TRACE.with_borrow(Vec::len) as u64;
        let mut x = n.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xC1B0_BBE2;
        let mut next = || {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        };
        for r in [AT, V0, V1, A0, A1, A2, A3, T0, T1, T2, T3, T4, T5, T6, T7, T8, T9] {
            let v = s32((next() >> 32) as u32);
            if !keep.contains(&r) {
                ctx.gpr[r] = v;
            }
        }
        ctx.hi = s32((next() >> 32) as u32);
        ctx.lo = s32((next() >> 32) as u32);
        for f in &mut ctx.fpr[..20] {
            f.u64 = next();
        }
    }

    /// Registers a function saves with `sw` and restores with `lw` come back
    /// as their sign-extended low words (and `sp` passes through `ADD32`).
    pub fn restore_saved(ctx: &mut RecompContext, regs: &[usize]) {
        for &r in regs {
            ctx.gpr[r] = s32(ctx.gpr[r] as u32);
        }
    }

    pub(crate) fn dispatch(name: &'static str, mem: &mut Mem, ctx: &mut RecompContext) -> bool {
        let Some(body) = DOUBLES.with_borrow(|d| d.get(name).cloned()) else { return false };
        TRACE.with_borrow_mut(|t| t.push(Call { name, gpr: ctx.gpr }));
        // A panic can't unwind through the C frames above us: report it and
        // abort, the same way a trap does.
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (body.borrow_mut())(mem, ctx)));
        if let Err(e) = run {
            let msg = e
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| e.downcast_ref::<&str>().copied())
                .unwrap_or("(no message)");
            crate::trap_now(&format!("test double for {name} panicked: {msg}"));
        }
        true
    }
}

/// Entry point of the generated stubs (see [`doubles`]).
///
/// # Safety
/// Called only by the stubs, with a NUL-terminated name that lives for the
/// whole program (a string literal) and the caller's `rdram` and `ctx`.
#[no_mangle]
pub unsafe extern "C" fn oracle_callee(name: *const c_char, rdram: *mut u8, ctx: *mut game::recomp::RecompContext) {
    let cname = CStr::from_ptr(name);
    // SAFETY: the stubs pass string literals, which live for the whole program.
    let name: &'static str = std::mem::transmute::<&str, &'static str>(cname.to_str().unwrap());
    let mut mem = n64mem::Mem::from_raw(rdram, n64mem::RDRAM_SIZE);
    if !doubles::dispatch(name, &mut mem, &mut *ctx) {
        runtime::oracle_unexpected_call(cname.as_ptr());
    }
}

/// Called by the stub runtime when generated code reaches something the
/// oracle doesn't provide. There is no way to unwind back through the C
/// frames, so this aborts the whole test process, loudly.
#[no_mangle]
pub extern "C" fn oracle_trap(msg: *const c_char) -> ! {
    // SAFETY: the C side always passes a NUL-terminated buffer.
    let msg = unsafe { CStr::from_ptr(msg) }.to_string_lossy();
    trap_now(&msg)
}

/// Print straight to the process's stderr (the test harness captures
/// `eprintln!` and would lose it in the abort), then abort.
fn trap_now(msg: &str) -> ! {
    use std::io::Write;
    let _ = write!(std::io::stderr(), "\n*** oracle trap: {msg}\n*** aborting the test process\n");
    std::process::abort()
}
