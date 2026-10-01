//! Depth 1 at 0x80006FE4 (game::misc): the game's audio heap allocation
//! through libultra's `alHeapDBAlloc` (`func_80087FC0`). Recompiled C vs
//! Rust with the callee as C; the model replays the callee's C with the
//! same arguments and adds the function's own frame stores.

// Tests are named after the functions (func_80006FE4), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::imports;
use game::misc;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;
const HEAP: u32 = 0x8030_0000;

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
}

fn wr(s: &mut State, a: u32, v: u32) {
    s.rdram.mem().write_u32(a, v);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Allocations that fit and that don't, any first argument.
    #[test]
    fn func_80006FE4(seed: u64, x: u32, used in 0u32..0x100, len in prop_oneof![0u32..0x200, any::<u32>()],
                     num in prop_oneof![0u32..8, any::<u32>()], size in prop_oneof![0u32..64, any::<u32>()]) {
        let mut s = State::new();
        s.randomise_registers(seed);
        s.ctx.gpr[SP] = sext(SP_AT);
        let base = 0x8031_0000u32;
        wr(&mut s, HEAP, base);
        wr(&mut s, HEAP + 4, base + used);
        wr(&mut s, HEAP + 8, len);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(x), sext(HEAP), sext(num), sext(size));
        let mut w = s.clone();
        let sp = SP_AT - 0x28;
        for (off, v) in [(0x34, size), (0x30, num), (0x1C, s.ctx.gpr[RA] as u32), (0x28, x), (0x2C, HEAP), (0x10, size)] {
            wr(&mut w, sp + off, v);
        }
        (w.ctx.gpr[A0], w.ctx.gpr[A1], w.ctx.gpr[A2], w.ctx.gpr[A3]) = (0, 0, sext(HEAP), sext(num));
        w.ctx.gpr[SP] = sext(sp);
        w.run(imports::func_80087FC0);
        let v0 = w.ctx.gpr[V0];
        wr(&mut w, sp + 0x24, v0 as u32);
        let after = run("func_80006FE4", misc::func_80006FE4, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
        prop_assert_eq!(word(&after, sp + 0x24), v0 as u32);
    }
}
