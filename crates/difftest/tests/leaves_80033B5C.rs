//! The float-moving leaves at 0x80033B5C..0x80033EEC (game::misc): two
//! object float getters, two table fills, a view copy and a 3x4 matrix
//! stack push. Recompiled C vs Rust, each checked against its statement by
//! simulating the stores on a copy of the input and comparing all of RDRAM
//! (and `f0` for the getters). Nothing here does float arithmetic, so NaN
//! bit patterns are in the domain.

// Tests are named after the functions (func_80033B5C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
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

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

/// `s` with the word stores applied in order.
fn expect(s: &State, stores: &[(u32, u32)]) -> State {
    let mut sim = s.clone();
    let mut m = sim.rdram.mem();
    for &(a, v) in stores {
        m.write_u32(a, v);
    }
    drop(m);
    sim
}

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
}

const PP: u32 = 0x8030_0000;
const OBJ: u32 = 0x8030_1000;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Null `pp`, null `*pp`, or an object's `+0x110`/`+0x114`.
    #[test]
    fn getters(seed: u64, mode in 0u8..3, x: u32, y: u32) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(PP, if mode == 1 { 0 } else { OBJ });
            m.write_u32(OBJ + 0x110, x);
            m.write_u32(OBJ + 0x114, y);
        }
        s.ctx.gpr[A0] = if mode == 0 { 0 } else { sext(PP) };
        for (name, port, want) in [("func_80033B5C", misc::func_80033B5C as RecompFn, x), ("func_80033B94", misc::func_80033B94, y)] {
            let after = run(name, port, &s)?;
            prop_assert_eq!(after.ctx.fpr[0].u32l(), if mode == 2 { want } else { 0 });
        }
    }

    #[test]
    fn table_fills(seed: u64) {
        let s = state(seed);
        let t = 0x800A_31E0;
        let f = 0x800A_5CA0;
        let data = |k: u32| word(&s, 0x800A_AA90 + 4 * k);
        let after = run("func_80033C70", misc::func_80033C70, &s)?;
        let mut st = vec![(t + 0x1A4, 0x11B), (t + 0x1A8, 0x11A), (t + 0x1AC, 0x11E), (t + 0x1B4, 0x800A_A640), (t + 0x1B8, 0x800A_A648), (t + 0x1D0, 0x120)];
        st.extend([(0x36C, 0), (0x39C, 1), (0x3A4, u32::MAX), (0x3A0, 2), (0x384, 3), (0x388, 4), (0x38C, 5), (0x390, 6), (0x394, 7), (0x398, 8)]
            .map(|(o, k)| (f + o, if k == u32::MAX { 0 } else { data(k) })));
        same_memory(&after, &expect(&s, &st))?;
        let after = run("func_80033D30", misc::func_80033D30, &s)?;
        let mut st = vec![(t + 0x47C, 0x11D), (t + 0x480, 0x11C), (t + 0x484, 0x11F), (t + 0x48C, 0x800A_A650), (t + 0x490, 0x800A_A654), (t + 0x4A8, 0x121)];
        st.extend([(0x984, 9), (0x988, 10), (0x98C, 11), (0x978, 12), (0x97C, 13), (0x980, 14)].map(|(o, k)| (f + o, data(k))));
        same_memory(&after, &expect(&s, &st))?;
    }

    #[test]
    fn func_80033E40(seed: u64, mtx: [u32; 16], v: [u32; 3]) {
        let mut s = state(seed);
        for (k, w) in mtx.iter().enumerate() {
            s.rdram.mem().write_u32(OBJ + 4 * k as u32, *w);
        }
        for (k, w) in v.iter().enumerate() {
            s.rdram.mem().write_u32(PP + 4 * k as u32, *w);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(OBJ), sext(PP));
        let after = run("func_80033E40", misc::func_80033E40, &s)?;
        let mut st: Vec<(u32, u32)> = (0..3).map(|k| (0x800A_3FDC + 4 * k, v[k as usize])).collect();
        st.extend((0..16).map(|k| (0x8011_2E20 + 4 * k, mtx[k as usize])));
        same_memory(&after, &expect(&s, &st))?;
    }

    /// Depths around the bound (and negative ones, which pass the signed
    /// test).
    #[test]
    fn func_80033EEC(seed: u64, depth in prop_oneof![-2i32..3, 29i32..35], mtx: [u32; 12]) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x800A_3FF0, depth as u32);
        for (k, w) in mtx.iter().enumerate() {
            s.rdram.mem().write_u32(OBJ + 4 * k as u32, *w);
        }
        s.ctx.gpr[A0] = sext(OBJ);
        let after = run("func_80033EEC", misc::func_80033EEC, &s)?;
        let mut st = vec![(0x800A_3FF4, 1)];
        if depth < 32 {
            let d = depth + 1;
            st.push((0x800A_3FF0, d as u32));
            let e = 0x8011_2EA0u32.wrapping_add((d * 48) as u32);
            st.extend((0..12).map(|k| (e + 4 * k, mtx[k as usize])));
        }
        same_memory(&after, &expect(&s, &st))?;
    }
}
