//! Contract doubles for functions that touch hardware registers (NOTES.md,
//! "The OS boundary and message-queue doubles", plan item 3, and "Hardware
//! doubles"): the test supplies what the registers read, and both runs of a
//! difftest see the same values. Each double that has generated C is
//! checked against it in `tests/hw_doubles.rs` (`oracle::probes`).
//!
//! `osGetCount` (`func_8008C550`) is `mfc0 v0, Count; jr ra`: N64Recomp
//! can't translate `mfc0 Count` (recomp.toml skips the function), so the
//! oracle only has a stub for it, and its whole effect is `v0`.

use std::cell::RefCell;
use std::rc::Rc;

use game::recomp::{addu, lhu, li, lw, reg::*, s32, sll, srl, RecompContext};
use n64mem::Mem;
use oracle::doubles::{self, Installed};

/// `osGetCount`: `v0` = the next of `counts`, sign-extended (`mfc0` of a
/// 32-bit register). The k-th call of a run gets `counts[k % counts.len()]`
/// (the count restarts with each run, [`doubles::calls_in_run`]), so the C
/// and the Rust run of [`crate::compare`] read the same sequence. Nothing
/// else changes.
pub fn install_count(counts: Vec<u32>) -> Installed {
    assert!(!counts.is_empty(), "install_count needs at least one Count value");
    doubles::install("func_8008C550", move |_m: &mut Mem, ctx: &mut RecompContext| {
        let k = doubles::calls_in_run("func_8008C550") - 1;
        ctx.gpr[V0] = s32(counts[k % counts.len()]);
    })
}

/// `osAiGetLength` (`func_800883E0`): `t6 = 0xA4500000` (sign-extended, as
/// `lui` leaves it) and `v0` = the next of `lengths` sign-extended, the
/// AI_LEN register (`0xA4500004`) the k-th call of a run reads (as
/// [`install_count`]). Nothing else changes. AI_LEN holds 18 bits, so a
/// larger injected value is refused.
pub fn install_ai_length(lengths: Vec<u32>) -> Installed {
    assert!(!lengths.is_empty(), "install_ai_length needs at least one AI_LEN value");
    assert!(lengths.iter().all(|&l| l < 1 << 18), "AI_LEN is an 18-bit register: {lengths:x?}");
    doubles::install("func_800883E0", move |_m: &mut Mem, ctx: &mut RecompContext| {
        let k = doubles::calls_in_run("func_800883E0") - 1;
        ctx.gpr[T6] = li(0xA450_0000);
        ctx.gpr[V0] = s32(lengths[k % lengths.len()]);
    })
}

/// The MI interrupt mask register (`MI_INTR_MASK`, `0xA430000C`) as
/// [`install_int_mask`] models it: six mask bits (SP, SI, AI, VI, PI, DP in
/// bits 0..5) per run.
#[derive(Debug, Default)]
pub struct MiMask {
    /// The mask bits.
    pub bits: u32,
    /// The words the current run wrote to the register, in order.
    pub writes: Vec<u32>,
}

impl MiMask {
    /// A write: for each interrupt `k`, bit `2k` clears its mask bit and bit
    /// `2k + 1` sets it. Both bits of one pair at once, or any bit above 11,
    /// is refused (the hardware's answer isn't modelled).
    pub fn write(&mut self, v: u32) {
        assert!(v < 1 << 12, "MI_INTR_MASK <- {v:#x}: bits above 11 aren't modelled");
        for k in 0..6 {
            match (v >> (2 * k)) & 3 {
                0 => {}
                1 => self.bits &= !(1 << k),
                2 => self.bits |= 1 << k,
                _ => panic!("MI_INTR_MASK <- {v:#x}: clears and sets mask bit {k} at once"),
            }
        }
        self.writes.push(v);
    }
}

/// `osSetIntMask` (`func_80090500`): its generated C, step by step, with the
/// MI mask register read and written through [`MiMask`]. The run's first
/// call starts the register at `initial` (six bits) with no writes; the
/// returned handle shows it after the last run. CP0 Status goes through the
/// same runtime hooks as the C (`oracle::runtime`: read sign-extended,
/// write the low word, a change of FR traps); the two RDRAM words it reads,
/// `__OSGlobalIntMask` (`0x800A7B50`) and `__osRcpImTable` (halfwords at
/// `0x800ADF90`), come from RDRAM. Every register the C writes (`t0`..`t4`,
/// `at`, `v0`) is left as it leaves it.
pub fn install_int_mask(initial: u32) -> (Installed, Rc<RefCell<MiMask>>) {
    assert!(initial < 1 << 6, "MI_INTR_MASK reads six bits: {initial:#x}");
    let mi = Rc::new(RefCell::new(MiMask { bits: initial, writes: Vec::new() }));
    let state = Rc::clone(&mi);
    let installed = doubles::install("func_80090500", move |m: &mut Mem, ctx: &mut RecompContext| {
        let mut mi = state.borrow_mut();
        if doubles::calls_in_run("func_80090500") == 1 {
            *mi = MiMask { bits: initial, writes: Vec::new() };
        }
        // SAFETY: the oracle's runtime hook, on this thread's context.
        let status = unsafe { oracle::runtime::cop0_status_read(ctx) };
        let g = &mut ctx.gpr;
        g[T4] = status;
        g[V0] = g[T4] & 0xFF01;
        g[T0] = li(0x800A_0000);
        g[T0] = addu(g[T0], 0x7B50);
        g[T3] = lw(m, g[T0], 0);
        g[AT] = s32(-1i32 as u32);
        g[T0] = g[T3] ^ g[AT];
        g[T0] &= 0xFF00;
        g[V0] |= g[T0];
        // lw t2, 0xC(0xA4300000): the mask register.
        g[T2] = s32(mi.bits);
        g[T1] = srl(g[T3], 16);
        if g[T2] != 0 {
            g[AT] = s32(-1i32 as u32);
            g[T1] ^= g[AT];
            g[T1] &= 0x3F;
            g[T2] |= g[T1];
        }
        g[T2] = sll(g[T2], 16);
        g[V0] |= g[T2];
        g[AT] = li(0x003F_0000);
        g[T0] = g[A0] & g[AT];
        g[T0] &= g[T3];
        g[T0] = srl(g[T0], 15);
        g[T2] = li(0x800B_0000);
        g[T2] = addu(g[T2], g[T0]);
        g[T2] = lhu(m, g[T2], -0x2070);
        g[AT] = li(0xA430_0000);
        // sw t2, 0xC(at): the mask register.
        mi.write(g[T2] as u32);
        g[T0] = g[A0] & 0xFF01;
        g[T1] = g[T3] & 0xFF00;
        g[T0] &= g[T1];
        g[AT] = li(0xFFFF_0000) | 0xFF;
        g[T4] &= g[AT];
        g[T4] |= g[T0];
        let t4 = g[T4];
        // SAFETY: as above.
        unsafe { oracle::runtime::cop0_status_write(ctx, t4) };
    });
    (installed, mi)
}
