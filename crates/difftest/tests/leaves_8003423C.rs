//! The matrix-moving leaves at 0x8003423C..0x800347D8 (game::misc): the
//! 3x4 matrix stack's top as 3x4 and as 4x4, and a 4x4 copy. Recompiled C
//! vs Rust, each checked against a word-by-word simulation (the depth is
//! re-read before every word, so an `out` over it is simulated too).

// Tests are named after the functions (func_8003423C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;
const STACK: u32 = 0x8011_2EA0;
const DEPTH: u32 = 0x800A_3FF0;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
}

/// Copy the top entry word by word as the code does: word `k` of the
/// entry goes to `out + dst[k]`, re-reading the depth each time; then the
/// constant stores.
fn simulate(s: &State, out: u32, dst: &[u32], consts: &[(u32, u32)]) -> State {
    let mut sim = s.clone();
    let mut m = sim.rdram.mem();
    for (k, &o) in dst.iter().enumerate() {
        let d = m.read_u32(DEPTH);
        let v = m.read_u32(STACK.wrapping_add(d.wrapping_mul(48)).wrapping_add(4 * k as u32));
        m.write_u32(out + o, v);
    }
    for &(o, v) in consts {
        m.write_u32(out + o, v);
    }
    drop(m);
    sim
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Depths 0..=32, `out` separate or over the depth word (entries kept
    /// small so re-read depths stay in range).
    #[test]
    fn stack_top(seed: u64, depth in 0u32..=32, alias in prop::bool::weighted(0.25), shift in 0u32..12) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 9, STACK, 33 * 48);
        if alias {
            for k in 0..33 * 12 {
                let w = s.rdram.as_words()[((STACK - 0x8000_0000) / 4 + k) as usize] % 33;
                s.rdram.mem().write_u32(STACK + 4 * k, w);
            }
        }
        s.rdram.mem().write_u32(DEPTH, depth);
        let out = if alias { DEPTH - 4 * shift } else { 0x8030_0000 };
        s.ctx.gpr[A0] = sext(out);
        let dst3: Vec<u32> = (0..12).map(|k| 4 * k).collect();
        let after = run("func_8003423C", misc::func_8003423C, &s)?;
        same_memory(&after, &simulate(&s, out, &dst3, &[]))?;
        if !alias {
            let dst4: Vec<u32> = (0..12).map(|k| 16 * (k / 3) + 4 * (k % 3)).collect();
            let after = run("func_80034374", misc::func_80034374, &s)?;
            same_memory(&after, &simulate(&s, out, &dst4, &[(0xC, 0), (0x1C, 0), (0x2C, 0), (0x3C, 0x3F80_0000)]))?;
        }
    }

    #[test]
    fn func_800347D8(seed: u64, mtx: [u32; 16]) {
        let mut s = state(seed);
        for (k, w) in mtx.iter().enumerate() {
            s.rdram.mem().write_u32(0x8030_1000 + 4 * k as u32, *w);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(0x8030_2000), sext(0x8030_1000));
        let after = run("func_800347D8", misc::func_800347D8, &s)?;
        let mut want = s.clone();
        for (k, w) in mtx.iter().enumerate() {
            want.rdram.mem().write_u32(0x8030_2000 + 4 * k as u32, *w);
        }
        same_memory(&after, &want)?;
    }
}
