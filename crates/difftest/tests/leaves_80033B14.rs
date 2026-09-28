//! The leaves at 0x80033B14..0x80035698 (game::misc, game::render): an object flag test,
//! two ring allocators, a counter, three display-list writers (projection
//! matrix plus gSPForceMatrix) and a flag setter. Recompiled C vs Rust on
//! random register files and memory, each checked against its statement.

// Tests are named after the functions (func_80033B14), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::{misc, render};
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

const O: u32 = 0x8030_0000;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed, O, 0x400);
    s.ctx.gpr[SP] = sext(0x800A_2800);
    s
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// p != 0, *p != 0, bit 29 of [*p + 0x100].
    #[test]
    fn func_80033B14(seed: u64, null_p: bool, null_o: bool, flags: u32) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(O, if null_o { 0 } else { O + 0x100 });
        s.rdram.mem().write_u32(O + 0x200, flags);
        s.ctx.gpr[A0] = if null_p { 0 } else { sext(O) };
        let after = run("func_80033B14", misc::func_80033B14, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(!null_p && !null_o && flags & 0x2000_0000 != 0));
    }

    /// The next ring entry: index + 1, 0 at the count, signed test.
    #[test]
    fn rings(seed: u64, k in 0usize..2, idx in prop_oneof![-3i32..4, 250i32..260, 0xBFCi32..0xC04, any::<i32>()]) {
        let (name, port, at, base, count, size): (&str, RecompFn, u32, u32, i32, u32) = [
            ("func_80033DD0", misc::func_80033DD0 as RecompFn, 0x800A_3CC0, 0x800E_0C50, 0x100, 32),
            ("func_80033E08", misc::func_80033E08, 0x800A_3CC4, 0x800E_2C50, 0xC00, 64),
        ][k];
        let mut s = state(seed);
        s.rdram.mem().write_u32(at, idx as u32);
        let after = run(name, port, &s)?;
        let next = idx.wrapping_add(1);
        let next = if next < count { next } else { 0 };
        prop_assert_eq!(word(&after, at), next as u32);
        prop_assert_eq!(after.ctx.gpr[V0], sext(base.wrapping_add((next as u32).wrapping_mul(size))));
    }

    /// [0x800A3FF4] = 1; decrement [0x800A3FF0] if positive.
    #[test]
    fn func_800344C8(seed: u64, n in prop_oneof![-2i32..3, any::<i32>()]) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x800A_3FF0, n as u32);
        let after = run("func_800344C8", misc::func_800344C8, &s)?;
        prop_assert_eq!(word(&after, 0x800A_3FF4), 1);
        prop_assert_eq!(word(&after, 0x800A_3FF0), if n > 0 { (n - 1) as u32 } else { n as u32 });
    }

    /// gSPMatrix(proj, projection | load), gSPForceMatrix(mvp).
    #[test]
    fn display_lists(seed: u64, k in 0usize..3, proj: u32, mvp: u32, flag: u32) {
        let (name, port): (&str, RecompFn) = [
            ("func_80034DA8", render::func_80034DA8 as RecompFn),
            ("func_8003527C", render::func_8003527C),
            ("func_800352E4", render::func_800352E4),
        ][k];
        let mut s = state(seed);
        let dl = O + 0x100;
        let mut m = s.rdram.mem();
        m.write_u32(0x8011_2C90, dl);
        m.write_u32(0x800A_3FF8, flag);
        if k == 1 {
            m.write_u32(O + 0x34, proj);
            m.write_u32(O + 0x38, mvp);
        } else {
            m.write_u32(0x8011_34D0, proj);
            m.write_u32(0x8011_34D4, mvp);
        }
        drop(m);
        s.ctx.gpr[A0] = sext(O);
        let after = run(name, port, &s)?;
        let want = [0xDA38_0003, proj, 0xDC38_000E, mvp, 0xDB0C_0000, 0x0001_0000];
        for (j, w) in want.iter().enumerate() {
            prop_assert_eq!(word(&after, dl + 4 * j as u32), *w, "word {}", j);
        }
        prop_assert_eq!(word(&after, 0x8011_2C90), dl + 24);
        prop_assert_eq!(word(&after, 0x800A_3FF8), if k == 0 { 0 } else { flag });
    }

    /// [0x800A3D9C] = (v == 1).
    #[test]
    fn func_80035698(seed: u64, v in prop_oneof![0u64..3, Just(0x1_0000_0001), any::<u64>()]) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = v;
        let after = run("func_80035698", misc::func_80035698, &s)?;
        prop_assert_eq!(word(&after, 0x800A_3D9C), u32::from(v == 1));
    }
}

#[test]
fn func_80033DC4() {
    let s = state(1);
    let after = compare("func_80033DC4", misc::func_80033DC4, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(after.ctx.gpr, s.ctx.gpr);
}
