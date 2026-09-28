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

/// `lh`: the sign-extended halfword at `base + offset` (`MEM_H`; same
/// canonical-`base` caveat as [`lw`]).
#[inline]
pub fn lh(mem: &Mem, base: u64, offset: i32) -> u64 {
    mem.read_i16((base as u32).wrapping_add(offset as u32)) as i64 as u64
}

/// `lbu`: the zero-extended byte at `base + offset` (`MEM_BU`).
#[inline]
pub fn lbu(mem: &Mem, base: u64, offset: i32) -> u64 {
    u64::from(mem.read_u8((base as u32).wrapping_add(offset as u32)))
}

/// N64Recomp's `LD` (`ldc1`/`ld`): the doubleword at `base + offset`, high
/// word first in memory.
#[inline]
pub fn ld(mem: &Mem, base: u64, offset: i32) -> u64 {
    let a = (base as u32).wrapping_add(offset as u32);
    (u64::from(mem.read_u32(a)) << 32) | u64::from(mem.read_u32(a.wrapping_add(4)))
}

/// N64Recomp's `SD` (`sdc1`/`sd`): the low word to `+4` first, then the high
/// word to `+0`.
#[inline]
pub fn sd(mem: &mut Mem, base: u64, offset: i32, value: u64) {
    let a = (base as u32).wrapping_add(offset as u32);
    mem.write_u32(a.wrapping_add(4), value as u32);
    mem.write_u32(a, (value >> 32) as u32);
}

/// `lhu`: the zero-extended halfword at `base + offset` (`MEM_HU`).
#[inline]
pub fn lhu(mem: &Mem, base: u64, offset: i32) -> u64 {
    u64::from(mem.read_u16((base as u32).wrapping_add(offset as u32)))
}

/// `lb`: the sign-extended byte at `base + offset` (`MEM_B`).
#[inline]
pub fn lb(mem: &Mem, base: u64, offset: i32) -> u64 {
    mem.read_u8((base as u32).wrapping_add(offset as u32)) as i8 as i64 as u64
}

/// `sw`: store the low word of `value` at `base + offset`.
#[inline]
pub fn sw(mem: &mut Mem, base: u64, offset: i32, value: u64) {
    mem.write_u32((base as u32).wrapping_add(offset as u32), value as u32)
}

/// `sh`: store the low halfword of `value` at `base + offset`.
#[inline]
pub fn sh(mem: &mut Mem, base: u64, offset: i32, value: u64) {
    mem.write_u16((base as u32).wrapping_add(offset as u32), value as u16)
}

/// `sb`: store the low byte of `value` at `base + offset`.
#[inline]
pub fn sb(mem: &mut Mem, base: u64, offset: i32, value: u64) {
    mem.write_u8((base as u32).wrapping_add(offset as u32), value as u8)
}

/// `sra rd, rt, sa`: N64Recomp's `S32(SIGNED(rt) >> sa)`, an arithmetic shift
/// of the **whole 64-bit register**, then truncated and sign-extended. For a
/// non-canonical `rt`, upper-half bits reach the low word (on hardware the
/// result is undefined); ports follow the C (NOTES.md, session 5).
#[inline]
pub fn sra(rt: u64, sa: u32) -> u64 {
    s32(((rt as i64) >> sa) as u32)
}

/// `srl rd, rt, sa`: `S32(U32(rt) >> sa)`, a logical shift of the low word.
#[inline]
pub fn srl(rt: u64, sa: u32) -> u64 {
    s32((rt as u32) >> sa)
}

/// `sllv rd, rt, rs`: [`sll`] by the low 5 bits of `rs`.
#[inline]
pub fn sllv(rt: u64, rs: u64) -> u64 {
    sll(rt, (rs & 31) as u32)
}

/// `srav rd, rt, rs`: [`sra`] (whole-register, as in the C) by the low 5 bits of `rs`.
#[inline]
pub fn srav(rt: u64, rs: u64) -> u64 {
    sra(rt, (rs & 31) as u32)
}

/// `srlv rd, rt, rs`: [`srl`] by the low 5 bits of `rs`.
#[inline]
pub fn srlv(rt: u64, rs: u64) -> u64 {
    srl(rt, (rs & 31) as u32)
}

/// `slt`/`slti`: 1 if `a < b` as signed **64-bit** values (`SIGNED(a) < SIGNED(b)`),
/// else 0. Immediates arrive sign-extended.
#[inline]
pub fn slt(a: u64, b: u64) -> u64 {
    u64::from((a as i64) < (b as i64))
}

/// `sltu`/`sltiu`: 1 if `a < b` as unsigned 64-bit values, else 0. An
/// `sltiu` immediate arrives sign-extended, as C converts it.
#[inline]
pub fn sltu(a: u64, b: u64) -> u64 {
    u64::from(a < b)
}

/// `mult`: `(lo, hi)` of the signed 64-bit product of the low words, each
/// half sign-extended. In N64Recomp, `lo`/`hi` are locals of the generated
/// function (starting at 0), not `ctx->lo`/`ctx->hi`.
#[inline]
pub fn mult(a: u64, b: u64) -> (u64, u64) {
    let p = i64::from(a as i32).wrapping_mul(i64::from(b as i32)) as u64;
    (s32(p as u32), s32((p >> 32) as u32))
}

/// `multu`: [`mult`] with the low words as unsigned.
#[inline]
pub fn multu(a: u64, b: u64) -> (u64, u64) {
    let p = u64::from(a as u32) * u64::from(b as u32);
    (s32(p as u32), s32((p >> 32) as u32))
}

/// `div`: `(lo, hi)` = quotient and remainder of the low words as signed,
/// computed in 64 bits as the C does (so `i32::MIN / -1` gives `i32::MIN`,
/// remainder 0), each sign-extended.
///
/// A zero divisor is undefined behaviour in the C (it faults on the hosts
/// N64Recomp supports), not the hardware's defined garbage: callers' domains
/// must exclude it. The game guards its divisions with `break 7`.
#[inline]
pub fn div(a: u64, b: u64) -> (u64, u64) {
    let (a, b) = (i64::from(a as i32), i64::from(b as i32));
    assert!(b != 0, "div by zero: undefined in N64Recomp's C");
    (s32((a / b) as u32), s32((a % b) as u32))
}

/// `divu`: [`div`] with the low words as unsigned; same zero-divisor caveat.
#[inline]
pub fn divu(a: u64, b: u64) -> (u64, u64) {
    let (a, b) = (a as u32, b as u32);
    assert!(b != 0, "divu by zero: undefined in N64Recomp's C");
    (s32(a / b), s32(a % b))
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

/// Float conversions as N64Recomp's C computes them (NOTES.md, "Floats").
///
/// The C switches the **host** rounding mode with `fesetround` when the game
/// writes FCR31 (`set_cop1_cs`), and `CVT_*` honour it. Rust can't change the
/// host mode (the compiler assumes round-to-nearest), so ports keep FCR31's
/// rounding bits in a local and pass them to these helpers. Only the
/// conversions take a mode: float arithmetic in ports is round-to-nearest,
/// which is the only mode the game computes in (its one idiom that sets
/// another mode converts and restores).
///
/// Out-of-range and NaN results follow what recomp.h gives on the MSVC
/// host, checked against it in `crates/difftest/tests/fpu.rs`. They are
/// host-specific and **not** the hardware's.
pub mod fpu {
    /// FCR31 rounding modes (bits 0-1).
    pub const NEAREST: u32 = 0;
    pub const TO_ZERO: u32 = 1;
    pub const UP: u32 = 2;
    pub const DOWN: u32 = 3;

    fn round_f64(x: f64, mode: u32) -> f64 {
        match mode & 3 {
            NEAREST => x.round_ties_even(),
            TO_ZERO => x.trunc(),
            UP => x.ceil(),
            _ => x.floor(),
        }
    }

    /// `CVT_W_S` (`cvt.w.s`): `lrintf` under `mode`. On MSVC, NaN and
    /// magnitudes above 2^31 give 0, but 2^31 itself gives `0x80000000`.
    pub fn cvt_w_s(x: f32, mode: u32) -> u32 {
        if x.is_nan() || x.abs() > 2_147_483_648.0 {
            return 0;
        }
        // Exact in f64; 2^31 wraps to 0x80000000, as on the host.
        round_f64(f64::from(x), mode) as i64 as u32
    }

    /// `CVT_W_D` (`cvt.w.d`): `lrint` under `mode`. On MSVC, NaN and any
    /// value whose rounded result is outside `i32` give 0.
    pub fn cvt_w_d(x: f64, mode: u32) -> u32 {
        let r = round_f64(x, mode);
        if r.is_nan() || !(-2_147_483_648.0..=2_147_483_647.0).contains(&r) {
            return 0;
        }
        r as i32 as u32
    }

    /// `TRUNC_W_S` (`trunc.w.s`): the C cast `(int32_t)x`, which on x86-64 is
    /// `cvttss2si`: NaN and out-of-range give `0x80000000`.
    pub fn trunc_w_s(x: f32) -> u32 {
        trunc_w_d(f64::from(x))
    }

    /// `TRUNC_W_D` (`trunc.w.d`): `(int32_t)x`, `0x80000000` when out of range.
    pub fn trunc_w_d(x: f64) -> u32 {
        let t = x.trunc();
        if t.is_nan() || !(-2_147_483_648.0..=2_147_483_647.0).contains(&t) {
            return 0x8000_0000;
        }
        t as i32 as u32
    }

    /// `x` (exact in f64) rounded to f32 under `mode`.
    fn to_f32(x: f64, mode: u32) -> f32 {
        let r = x as f32; // round to nearest, even
        if x.is_nan() || f64::from(r) == x {
            return r;
        }
        // The two floats either side of x (r is one of them; ±inf counts as
        // a float, so overflow rounds to MAX or inf by mode).
        let (below, above) = if f64::from(r) < x { (r, r.next_up()) } else { (r.next_down(), r) };
        match mode & 3 {
            NEAREST => r,
            TO_ZERO => {
                if x > 0.0 {
                    below
                } else {
                    above
                }
            }
            UP => above,
            _ => below,
        }
    }

    /// `CVT_S_W` (`cvt.s.w`): `(float)(int32_t)w` under `mode` (only values
    /// beyond 2^24 round).
    pub fn cvt_s_w(w: u32, mode: u32) -> f32 {
        to_f32(f64::from(w as i32), mode)
    }

    /// `CVT_S_D` (`cvt.s.d`): `(float)x` under `mode`.
    pub fn cvt_s_d(x: f64, mode: u32) -> f32 {
        to_f32(x, mode)
    }
}

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
    fn shifts_compares_and_muldiv_match_recomp_macros() {
        // sra shifts the whole register (S32(SIGNED(x) >> n)).
        assert_eq!(sra(0xFFFF_FFFF_8000_0000, 4), 0xFFFF_FFFF_F800_0000);
        assert_eq!(sra(0x0000_0001_0000_0000, 4), 0x1000_0000);
        assert_eq!(srl(0xFFFF_FFFF_8000_0000, 4), 0x0800_0000);
        assert_eq!(sllv(1, 33), 2);
        assert_eq!(slt(u64::MAX, 0), 1);
        assert_eq!(sltu(u64::MAX, 0), 0);
        // multu: U64(U32(a)) * U64(U32(b)), halves sign-extended.
        assert_eq!(multu(0xFFFF_FFFF, 0xFFFF_FFFF), (1, 0xFFFF_FFFF_FFFF_FFFE));
        assert_eq!(mult(u64::MAX, 2), (u64::MAX - 1, u64::MAX));
        // div in 64 bits: i32::MIN / -1 doesn't trap.
        assert_eq!(div(0xFFFF_FFFF_8000_0000, u64::MAX), (0xFFFF_FFFF_8000_0000, 0));
        assert_eq!(div((-7i64) as u64, 2), ((-3i64) as u64, u64::MAX));
        assert_eq!(divu(0xFFFF_FFFF, 2), (0x7FFF_FFFF, 1));
    }

    #[test]
    fn byte_and_halfword_access() {
        let mut r = n64mem::Rdram::new();
        let mut m = r.mem();
        sw(&mut m, s32(0x8000_1000), 0, 0x1234_80FF);
        assert_eq!(lb(&m, s32(0x8000_1000), 3), u64::MAX);
        assert_eq!(lbu(&m, s32(0x8000_1000), 2), 0x80);
        assert_eq!(lhu(&m, s32(0x8000_1000), 2), 0x80FF);
        sh(&mut m, s32(0x8000_1000), 0, 0xAAAA_5678);
        sb(&mut m, s32(0x8000_1000), 3, 0x01);
        assert_eq!(lw(&m, s32(0x8000_1000), 0), 0x5678_8001);
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
