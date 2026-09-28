//! The float-store leaves at 0x8000FCBC..0x800117E4 (game::misc): slot and
//! table setters for the blocks func_8000FE1C/FE78/FEF0 reset (words, float
//! triples, -1000 markers, colour bytes) and three float globals.
//! Recompiled C vs Rust, each checked against its statement by simulating
//! the stores on a copy of the input and comparing all of RDRAM.
//!
//! Nothing here does arithmetic on floats, so NaN bit patterns are in the
//! domain everywhere.

// Tests are named after the functions (func_8000FCBC), capitals included.
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
const V: u32 = 0x8030_0000;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

/// Stores, in program order.
enum St {
    W(u32, u32),
    B(u32, u8),
}

fn expect(s: &State, stores: &[St]) -> State {
    let mut sim = s.clone();
    let mut m = sim.rdram.mem();
    for st in stores {
        match *st {
            St::W(a, v) => m.write_u32(a, v),
            St::B(a, v) => m.write_u8(a, v),
        }
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

/// Indices near the bounds, negative ones, and 64-bit values whose low word
/// is small (the tests are 64-bit).
fn index(max: i64) -> impl Strategy<Value = u64> {
    prop_oneof![
        (-2i64..max + 2).prop_map(|v| v as u64),
        (0u64..max as u64).prop_map(|v| v | 1 << 40),
        (0u64..max as u64).prop_map(|v| v | 1 << 63),
    ]
}

/// A triple of float bits at V, NaNs included.
fn triple() -> impl Strategy<Value = [u32; 3]> {
    any::<[u32; 3]>()
}

fn put(s: &mut State, a: u32, v: &[u32]) {
    let mut m = s.rdram.mem();
    for (k, x) in v.iter().enumerate() {
        m.write_u32(a + 4 * k as u32, *x);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8000FCBC(seed: u64, i in index(2), w: u32, v in triple(), x: u32, bytes: [u32; 4]) {
        let mut s = state(seed);
        put(&mut s, V, &v);
        put(&mut s, SP_AT + 0x10, &bytes);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (i, sext(w), sext(V), sext(x));
        let after = run("func_8000FCBC", misc::func_8000FCBC, &s)?;
        let want = if (i as i64) >= 0 && (i as i64) < 2 {
            let k = i as u32;
            let mut st = vec![St::W(0x8009_B814 + 4 * k, w)];
            st.extend((0..3).map(|c| St::W(0x800D_57D0 + 12 * k + 4 * c, v[c as usize])));
            st.extend([0x800D_57B0, 0x800D_57A0, 0x800D_5790].map(|a| St::W(a + 4 * k, (-1000i32) as u32)));
            st.push(St::W(0x8009_B81C + 4 * k, x));
            st.extend((0..4).map(|c| St::B(0x8009_B824 + 4 * k + c, bytes[c as usize] as u8)));
            expect(&s, &st)
        } else {
            s.clone()
        };
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8000FD74(seed: u64, i in index(2), j in index(8), w: u32, x: u32, y: u32, bytes: [u32; 3]) {
        let mut s = state(seed);
        put(&mut s, SP_AT + 0x10, &[y, bytes[0], bytes[1], bytes[2]]);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (i, j, sext(w), sext(x));
        let after = run("func_8000FD74", misc::func_8000FD74, &s)?;
        let ok = |v: u64, n: i64| (v as i64) >= 0 && (v as i64) < n;
        let want = if ok(i, 2) && ok(j, 8) {
            let (i, j) = (i as u32, j as u32);
            let e = 32 * i + 4 * j;
            let mut st = vec![St::W(0x8009_B82C + e, w), St::W(0x800D_57E8 + e, y), St::W(0x800D_5828 + e, x)];
            st.extend((0..3).map(|c| St::B(0x800D_5868 + 24 * i + 3 * j + c, bytes[c as usize] as u8)));
            expect(&s, &st)
        } else {
            s.clone()
        };
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8000FEAC(seed: u64, k in 0u32..20, v in triple(), w: u32) {
        let mut s = state(seed);
        put(&mut s, V, &v);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (u64::from(k), sext(V), sext(w));
        let after = run("func_8000FEAC", misc::func_8000FEAC, &s)?;
        let mut st = vec![St::W(0x800D_5AA8 + 4 * k, w)];
        st.extend((0..3).map(|c| St::W(0x800D_59B8 + 12 * k + 4 * c, v[c as usize])));
        same_memory(&after, &expect(&s, &st))?;
    }

    /// Both triple setters; negative `k` passes the signed bound (QUIRK).
    #[test]
    fn func_8000FF54(seed: u64, k in prop_oneof![-3i64..43, (0i64..40).prop_map(|v| v | 1 << 40)], v in triple()) {
        let mut s = state(seed);
        put(&mut s, V, &v);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (k as u64, sext(V));
        let passes = k < 40;
        let kk = k as u32;
        let tri: Vec<St> = (0..3).map(|c| St::W((0x800D_5C60u32).wrapping_add(kk.wrapping_mul(12)).wrapping_add(4 * c), v[c as usize])).collect();
        let after = run("func_8000FF54", misc::func_8000FF54, &s)?;
        let want = if passes {
            let mut st = tri;
            st.push(St::W(0x800D_60A0u32.wrapping_add(kk.wrapping_mul(4)), (-1000i32) as u32));
            st.push(St::B(0x800D_5C38u32.wrapping_add(kk), 1));
            expect(&s, &st)
        } else {
            s.clone()
        };
        same_memory(&after, &want)?;
        let after = run("func_8000FFB8", misc::func_8000FFB8, &s)?;
        let tri: Vec<St> = (0..3).map(|c| St::W((0x800D_5C60u32).wrapping_add(kk.wrapping_mul(12)).wrapping_add(4 * c), v[c as usize])).collect();
        let want = if passes { expect(&s, &tri) } else { s.clone() };
        same_memory(&after, &want)?;
    }

    #[test]
    fn float_globals(seed: u64, x: u32, y: u32) {
        let mut s = state(seed);
        s.ctx.fpr[12].set_u32l(x);
        s.ctx.fpr[14].set_u32l(y);
        let after = run("func_80011764", misc::func_80011764, &s)?;
        same_memory(&after, &expect(&s, &[St::W(0x8009_B878, x), St::W(0x8009_B87C, y)]))?;
        let after = run("func_800117E4", misc::func_800117E4, &s)?;
        same_memory(&after, &expect(&s, &[St::W(0x8009_B880, x)]))?;
    }
}
