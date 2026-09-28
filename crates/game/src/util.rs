//! Small memory utilities.

use crate::recomp::{addu, enter, reg::*, sll, RecompContext};

/// `func_80000554`: zero `a1` consecutive words starting at `a0`; nothing if
/// `a1 <= 0` (signed, full 64-bit register).
///
/// The original is a compiler-unrolled loop: first `a1 & 3` single stores,
/// then four per iteration. The temporaries it leaves behind (`v0` = the
/// single-store count or 0, `v1`, `a2`, `a3`, `t6`, `t7`, `t8`) are reproduced
/// exactly, since the differential test compares the whole register file.
///
/// Domain: `a0` word-aligned and `a0 .. a0 + 4 * a1` inside RDRAM, with
/// registers holding sign-extended 32-bit values, as they do on hardware for
/// 32-bit code. Outside it the original faults (address error) or runs off
/// the end of memory, and so do both the generated C and this port.
///
/// # Safety
/// N64Recomp entry point: see [`crate::recomp::enter`].
pub unsafe extern "C" fn func_80000554(rdram: *mut u8, ctx: *mut RecompContext) {
    let (mut mem, ctx) = enter(rdram, ctx);
    let g = &mut ctx.gpr;
    let a0 = g[A0];
    let a1 = g[A1];

    // blez a1, end  /  (delay) move v0, zero
    g[V0] = 0;
    if (a1 as i64) <= 0 {
        return;
    }

    // andi a3, a1, 3  /  beqz a3, L_80000584  /  (delay) move a2, a3
    let rem = a1 & 3;
    g[A3] = rem;
    g[A2] = rem;
    if rem != 0 {
        // sll t6, zero, 2  /  addu v1, a0, t6
        g[T6] = sll(0, 2);
        let mut v1 = addu(a0, g[T6]);
        let mut v0 = g[V0];
        loop {
            // addiu v0, v0, 1  /  sw zero, 0(v1)  /  bne a2, v0  /  (delay) addiu v1, v1, 4
            v0 = addu(v0, 1);
            mem.write_u32(v1 as u32, 0);
            v1 = addu(v1, 4);
            if rem == v0 {
                break;
            }
        }
        g[V0] = v0;
        g[V1] = v1;
        // beq v0, a1, end  /  (delay) sll t7, v0, 2
        g[T7] = sll(v0, 2);
        if v0 == a1 {
            return;
        }
    } else {
        // Branch target L_80000584: sll t7, v0, 2 (v0 is 0 here).
        g[T7] = sll(g[V0], 2);
    }

    // sll t8, a1, 2  /  addu a2, t8, a0  /  addu v1, a0, t7
    g[T8] = sll(a1, 2);
    let end = addu(g[T8], a0);
    g[A2] = end;
    let mut v1 = addu(a0, g[T7]);
    loop {
        // addiu v1, v1, 16, then stores at -12, -8, -4 and (delay slot) -16.
        v1 = addu(v1, 0x10);
        let p = v1 as u32;
        mem.write_u32(p.wrapping_sub(0xC), 0);
        mem.write_u32(p.wrapping_sub(0x8), 0);
        mem.write_u32(p.wrapping_sub(0x4), 0);
        mem.write_u32(p.wrapping_sub(0x10), 0);
        if v1 == end {
            break;
        }
    }
    g[V1] = v1;
}
