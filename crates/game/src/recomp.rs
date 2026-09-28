//! N64Recomp's calling convention, mirrored in Rust.
//!
//! Every recompiled function has the C signature
//! `void f(uint8_t* rdram, recomp_context* ctx)`. [`RecompContext`] matches
//! `recomp_context` in N64Recomp's `include/recomp.h` (pinned at `ffb39cd`)
//! field for field; `crates/oracle/tests/ctx_layout.rs` checks the offsets
//! against the C compiler.

use n64mem::{Mem, RDRAM_SIZE};

/// One FPU register: `fpr` in recomp.h, a union of `double`, `{float fl, fh}`,
/// `{uint32_t u32l, u32h}` and `uint64_t`. Stored as its 64 bits; `fl`/`u32l`
/// are the low half (first in memory on the little-endian hosts N64Recomp
/// supports) and `fh`/`u32h` the high half.
#[repr(C, align(8))]
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Fpr {
    pub u64: u64,
}

impl Fpr {
    #[inline]
    pub fn u32l(self) -> u32 {
        self.u64 as u32
    }
    #[inline]
    pub fn u32h(self) -> u32 {
        (self.u64 >> 32) as u32
    }
    #[inline]
    pub fn fl(self) -> f32 {
        f32::from_bits(self.u32l())
    }
    #[inline]
    pub fn fh(self) -> f32 {
        f32::from_bits(self.u32h())
    }
    #[inline]
    pub fn d(self) -> f64 {
        f64::from_bits(self.u64)
    }
    #[inline]
    pub fn set_u32l(&mut self, v: u32) {
        self.u64 = (self.u64 & !0xFFFF_FFFF) | u64::from(v);
    }
    #[inline]
    pub fn set_u32h(&mut self, v: u32) {
        self.u64 = (self.u64 & 0xFFFF_FFFF) | (u64::from(v) << 32);
    }
    #[inline]
    pub fn set_fl(&mut self, v: f32) {
        self.set_u32l(v.to_bits())
    }
    #[inline]
    pub fn set_fh(&mut self, v: f32) {
        self.set_u32h(v.to_bits())
    }
    #[inline]
    pub fn set_d(&mut self, v: f64) {
        self.u64 = v.to_bits()
    }
}

/// `recomp_context`. `gpr[n]` is C's `rN`, `fpr[n]` is `fN`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RecompContext {
    pub gpr: [u64; 32],
    pub fpr: [Fpr; 32],
    pub hi: u64,
    pub lo: u64,
    /// Odd FPU registers in 32-bit FPU mode (Status.FR = 0): the generated
    /// code writes `fN` (N odd) through `f_odd[(N - 1) * 2]`, so this must
    /// point at `fpr[0]`'s high word of the same context. See [`Self::fix_f_odd`].
    pub f_odd: *mut u32,
    pub status_reg: u32,
    pub mips3_float_mode: u8,
}

impl Default for RecompContext {
    fn default() -> Self {
        Self {
            gpr: [0; 32],
            fpr: [Fpr::default(); 32],
            hi: 0,
            lo: 0,
            f_odd: std::ptr::null_mut(),
            status_reg: 0,
            mips3_float_mode: 0,
        }
    }
}

impl RecompContext {
    /// Point `f_odd` at this context's `f0` high word, as the runtime does.
    /// Must be redone after the context is moved.
    pub fn fix_f_odd(&mut self) {
        // SAFETY: fpr[0] is 8 bytes; its high word is at +4 on little-endian hosts.
        self.f_odd = unsafe { (&mut self.fpr[0] as *mut Fpr).cast::<u32>().add(1) };
    }
}

/// The function type N64Recomp emits.
pub type RecompFn = unsafe extern "C" fn(rdram: *mut u8, ctx: *mut RecompContext);

/// Turn the raw arguments of a recompiled-function entry point into a memory
/// view and the register file.
///
/// # Safety
/// `rdram` must be an N64Recomp RDRAM buffer of [`RDRAM_SIZE`] bytes and `ctx`
/// a valid, exclusive `recomp_context`, both live for the duration of the call.
#[inline]
pub unsafe fn enter<'a>(rdram: *mut u8, ctx: *mut RecompContext) -> (Mem<'a>, &'a mut RecompContext) {
    (Mem::from_raw(rdram, RDRAM_SIZE), &mut *ctx)
}

/// Sign-extend a 32-bit result into a 64-bit register, as every 32-bit MIPS
/// ALU op does (recomp.h's `S32`, `ADD32`, ...).
#[inline]
pub fn s32(v: u32) -> u64 {
    v as i32 as i64 as u64
}

/// `addu`/`addiu`: 32-bit wrapping add of the low words, sign-extended (`ADD32`).
#[inline]
pub fn addu(a: u64, b: u64) -> u64 {
    s32((a as u32).wrapping_add(b as u32))
}

/// `subu`: 32-bit wrapping subtract of the low words, sign-extended (`SUB32`).
#[inline]
pub fn subu(a: u64, b: u64) -> u64 {
    s32((a as u32).wrapping_sub(b as u32))
}

/// `sll rd, rt, sa`: shift the low word, sign-extend (`S32(rt << sa)`).
#[inline]
pub fn sll(rt: u64, sa: u32) -> u64 {
    s32((rt as u32) << sa)
}

/// `lw`: the sign-extended word at `base + offset`. (`MEM_W` adds in 64 bits,
/// so `base` must be canonical for the C to agree; ports' domains say so.)
#[inline]
pub fn lw(mem: &Mem, base: u64, offset: i32) -> u64 {
    s32(mem.read_u32((base as u32).wrapping_add(offset as u32)))
}

/// `sw`: store the low word of `value` at `base + offset`.
#[inline]
pub fn sw(mem: &mut Mem, base: u64, offset: i32, value: u64) {
    mem.write_u32((base as u32).wrapping_add(offset as u32), value as u32)
}

/// `lui` + `ori`/`addiu`: a 32-bit constant, sign-extended.
#[inline]
pub fn li(v: u32) -> u64 {
    s32(v)
}

/// Call another N64Recomp-shaped function, as a `jal` in the original does.
///
/// Ports call their callees through the C symbols N64Recomp gives them,
/// declared in [`crate::imports`], never a Rust port directly. The linker
/// decides what each symbol is: the recompiled C, a Rust port exported under
/// that name, or (in the tests) an oracle stub or test double. So the
/// recompiled caller and its Rust port always reach the same callee (NOTES.md,
/// "How ports call other functions").
///
/// Like N64Recomp's `jal`, this does not write `$ra`. The whole register file
/// in `ctx` must be exactly what the original has at the call, since the
/// callee sees all of it; values a port keeps in locals must be written back
/// first and reloaded after.
#[inline]
pub fn call(f: RecompFn, mem: &mut Mem, ctx: &mut RecompContext) {
    assert_eq!(mem.len(), RDRAM_SIZE, "callees need the whole of RDRAM");
    ctx.fix_f_odd();
    // SAFETY: `mem` covers a full RDRAM buffer and is borrowed mutably, so no
    // access through it overlaps the call; `ctx` is exclusive and `f_odd`
    // points into it.
    unsafe { f(mem.as_mut_ptr(), ctx) }
}

/// Declare the recompiled functions that ports call (see [`call`]). Outside
/// `game`'s own unit tests they are `extern "C"` symbols that whoever links
/// `game` must define. In those unit tests nothing does, so each gets a
/// definition that aborts.
macro_rules! recomp_imports {
    ($($name:ident),* $(,)?) => {
        #[cfg(not(test))]
        extern "C" {
            $(pub fn $name(rdram: *mut u8, ctx: *mut $crate::recomp::RecompContext);)*
        }
        $(
            #[cfg(test)]
            #[no_mangle]
            pub unsafe extern "C" fn $name(_rdram: *mut u8, _ctx: *mut $crate::recomp::RecompContext) {
                eprintln!(concat!(stringify!($name), " is only linked in the oracle or the game build, not game's unit tests"));
                std::process::abort();
            }
        )*
        /// Every imported symbol.
        pub const NAMES: &[&str] = &[$(stringify!($name)),*];
    };
}
pub(crate) use recomp_imports;

/// Named register indices (o32 ABI names), for readability in ports.
pub mod reg {
    pub const ZERO: usize = 0;
    pub const AT: usize = 1;
    pub const V0: usize = 2;
    pub const V1: usize = 3;
    pub const A0: usize = 4;
    pub const A1: usize = 5;
    pub const A2: usize = 6;
    pub const A3: usize = 7;
    pub const T0: usize = 8;
    pub const T1: usize = 9;
    pub const T2: usize = 10;
    pub const T3: usize = 11;
    pub const T4: usize = 12;
    pub const T5: usize = 13;
    pub const T6: usize = 14;
    pub const T7: usize = 15;
    pub const S0: usize = 16;
    pub const S1: usize = 17;
    pub const S2: usize = 18;
    pub const S3: usize = 19;
    pub const S4: usize = 20;
    pub const S5: usize = 21;
    pub const S6: usize = 22;
    pub const S7: usize = 23;
    pub const T8: usize = 24;
    pub const T9: usize = 25;
    pub const K0: usize = 26;
    pub const K1: usize = 27;
    pub const GP: usize = 28;
    pub const SP: usize = 29;
    pub const FP: usize = 30;
    pub const RA: usize = 31;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_match_recomp_macros() {
        // ADD32 wraps in 32 bits and sign-extends, ignoring the upper halves.
        assert_eq!(addu(0x7FFF_FFFF, 1), 0xFFFF_FFFF_8000_0000);
        assert_eq!(addu(0xDEAD_0000_0000_0001, 0x1234_0000_0000_0002), 3);
        assert_eq!(sll(0x4000_0000, 1), 0xFFFF_FFFF_8000_0000);
        assert_eq!(sll(0xFFFF_FFFF_0000_0001, 2), 4);
        assert_eq!(subu(0, 1), u64::MAX);
        assert_eq!(subu(0x8000_0000, 1), 0x7FFF_FFFF);
    }

    #[test]
    fn fpr_halves() {
        let mut f = Fpr::default();
        f.set_fl(1.5);
        f.set_u32h(0xDEAD_BEEF);
        assert_eq!(f.u64, 0xDEAD_BEEF_3FC0_0000);
        assert_eq!(f.fl(), 1.5);
    }
}
