//! The float leaves at 0x80017520..0x800183C0 (game::math, game::misc):
//! 4x4 scale/translation/identity builders and row scaling, node transform
//! setters and getters, float field accessors, a node header initialiser
//! and a settings-block reset. Recompiled C vs Rust, each checked against
//! its statement by simulating the stores on a copy of the input and
//! comparing all of RDRAM (and `f0` for the getters).
//!
//! Only func_80017918 does arithmetic (products); NaN operands of it are
//! outside its domain. Everything else moves bits, NaNs included.

// Tests are named after the functions (func_80017520), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::recomp::{reg::*, RecompFn};
use game::{math, misc};
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
const M: u32 = 0x8030_0000;
const N: u32 = 0x8030_0100;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

const ONE: u32 = 0x3F80_0000;

/// Word and halfword stores, in program order.
enum St {
    W(u32, u32),
    H(u32, u16),
}

fn expect(s: &State, stores: &[St]) -> State {
    let mut sim = s.clone();
    let mut m = sim.rdram.mem();
    for st in stores {
        match *st {
            St::W(a, v) => m.write_u32(a, v),
            St::H(a, v) => m.write_u16(a, v),
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

/// A float passed in a GPR (junk above it).
fn freg(bits: u32, junk: u32) -> u64 {
    u64::from(junk) << 32 | u64::from(bits)
}

fn float() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        -1.0e4f32..1.0e4f32,
        -1.0f32..1.0f32,
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

/// The 4x4 builders: which words hold which value (index -> bits).
fn matrix(entries: &[(usize, u32)]) -> [u32; 16] {
    let mut w = [0u32; 16];
    for &(k, v) in entries {
        w[k] = v;
    }
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80017520(seed: u64, x: u32, y: u32, z: u32, junk: [u32; 3]) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(M), freg(x, junk[0]), freg(y, junk[1]), freg(z, junk[2]));
        for (name, port, entries) in [
            ("func_80017520", math::func_80017520 as RecompFn, matrix(&[(0, x), (5, y), (10, z), (15, ONE)])),
            ("func_80017580", math::func_80017580, matrix(&[(0, ONE), (5, ONE), (10, ONE), (12, x), (13, y), (14, z), (15, ONE)])),
        ] {
            let after = run(name, port, &s)?;
            let mut st: Vec<St> = (0..16).map(|k| St::W(M + 4 * k as u32, entries[k])).collect();
            st.push(St::W(SP_AT + 0xC, z));
            same_memory(&after, &expect(&s, &st))?;
        }
        let after = run("func_80017874", math::func_80017874, &s)?;
        let id = matrix(&[(0, ONE), (5, ONE), (10, ONE), (15, ONE)]);
        same_memory(&after, &expect(&s, &(0..16).map(|k| St::W(M + 4 * k as u32, id[k])).collect::<Vec<_>>()))?;
    }

    /// Rows 0..2 scaled, row 3 copied (any bits there).
    #[test]
    fn func_80017918(seed: u64, sc in [float(), float(), float()], rows in prop::array::uniform12(float()), last: [u32; 4]) {
        let prods: Vec<f32> = (0..12).map(|k| rows[k] * sc[k / 4]).collect();
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for k in 0..12 {
                m.write_u32(N + 4 * k as u32, rows[k].to_bits());
            }
            for k in 0..4 {
                m.write_u32(N + 0x30 + 4 * k as u32, last[k]);
            }
            m.write_u32(SP_AT + 0x10, N);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(M), freg(sc[0].to_bits(), 1), freg(sc[1].to_bits(), 2), freg(sc[2].to_bits(), 3));
        let after = run("func_80017918", math::func_80017918, &s)?;
        let mut st: Vec<St> = vec![St::W(SP_AT + 0xC, sc[2].to_bits())];
        st.extend(prods.iter().enumerate().map(|(k, p)| St::W(M + 4 * k as u32, p.to_bits())));
        st.extend((0..4).map(|k| St::W(M + 0x30 + 4 * k, last[k as usize])));
        same_memory(&after, &expect(&s, &st))?;
    }

    /// Node transform set/get and translation, with the flags halfword.
    #[test]
    fn node_transforms(seed: u64, mtx: [u32; 16], node: [u32; 12], xyz: [u32; 3], flags: u16) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            for k in 0..16 {
                m.write_u32(M + 4 * k as u32, mtx[k]);
            }
            for k in 0..12 {
                m.write_u32(N + 0x1C + 4 * k as u32, node[k]);
            }
            m.write_u16(N + 0xC, flags);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(N), freg(xyz[0], 7), freg(xyz[1], 8), freg(xyz[2], 9));
        let after = run("func_80017B7C", misc::func_80017B7C, &s)?;
        same_memory(&after, &expect(&s, &[
            St::W(SP_AT + 0xC, xyz[2]), St::W(N + 0x40, xyz[0]), St::W(N + 0x44, xyz[1]), St::H(N + 0xC, flags | 3), St::W(N + 0x48, xyz[2]),
        ]))?;
        s.ctx.gpr[A1] = sext(M);
        let after = run("func_80017BA8", misc::func_80017BA8, &s)?;
        let mut st: Vec<St> = (0..12).map(|k| St::W(N + 0x1C + 4 * k as u32, mtx[4 * (k / 3) + k % 3])).collect();
        st.push(St::H(N + 0xC, flags | 3));
        same_memory(&after, &expect(&s, &st))?;
        for (name, port) in [("func_80017C18", misc::func_80017C18 as RecompFn), ("func_80017C98", misc::func_80017C98)] {
            let after = run(name, port, &s)?;
            let st: Vec<St> = (0..16)
                .map(|k| St::W(M + 4 * k as u32, if k % 4 < 3 { node[3 * (k / 4) + k % 4] } else if k == 15 { ONE } else { 0 }))
                .collect();
            same_memory(&after, &expect(&s, &st))?;
        }
    }

    /// Indexed floats at +0x1C (k < 8, 64-bit signed) and the fields by k.
    #[test]
    fn node_fields(seed: u64, k in prop_oneof![-2i64..10, (0i64..8).prop_map(|v| v | 1 << 40)], x: u32, f0: u32, which in prop_oneof![0u64..7, Just(2 | 1 << 32)]) {
        let mut s = state(seed);
        s.ctx.fpr[0].set_u32l(f0);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(N), k as u64, freg(x, 5));
        let stores = (0..8).contains(&k);
        let after = run("func_80017D58", misc::func_80017D58, &s)?;
        let st = if stores { vec![St::W(N + 0x1C + 4 * k as u32, x)] } else { vec![] };
        same_memory(&after, &expect(&s, &st))?;
        let after = run("func_80017D80", misc::func_80017D80, &s)?;
        let want = if stores { word(&s, N + 0x1C + 4 * k as u32) } else { f0 };
        prop_assert_eq!(after.ctx.fpr[0].u32l(), want);
        s.ctx.gpr[A1] = which;
        let off = match which { 2 => Some(0x148), 3 => Some(0x14C), 5 => Some(0x150), _ => None };
        let after = run("func_80018084", misc::func_80018084, &s)?;
        let st = off.map(|o| vec![St::W(N + o, x)]).unwrap_or_default();
        same_memory(&after, &expect(&s, &st))?;
        let after = run("func_800180C8", misc::func_800180C8, &s)?;
        prop_assert_eq!(after.ctx.fpr[0].u32l(), off.map_or(0xBF80_0000, |o| word(&s, N + o)));
    }

    #[test]
    fn func_80017E20(seed: u64, v: [u32; 6]) {
        let mut s = state(seed);
        for (k, w) in v.iter().enumerate() {
            s.rdram.mem().write_u32(N + 8 + 4 * k as u32, *w);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(N), sext(M));
        let after = run("func_80017E20", misc::func_80017E20, &s)?;
        same_memory(&after, &expect(&s, &(0..6).map(|k| St::W(M + 4 * k as u32, v[k])).collect::<Vec<_>>()))?;
    }

    /// Types with and without bit 14, and 0xD065 (also with junk above).
    #[test]
    fn func_80018324(seed: u64, ty in prop_oneof![Just(0xD065u64), Just(0xD065 | 1 << 32), Just(0xD064), any::<u16>().prop_map(u64::from), any::<u32>().prop_map(|v| sext(v))]) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(N), ty);
        let after = run("func_80018324", misc::func_80018324, &s)?;
        let mut st = vec![St::W(N, ty as u32), St::W(N + 4, u32::MAX), St::W(N + 8, u32::MAX), St::H(N + 0xC, 0), St::H(N + 0xE, 0), St::W(N + 0x10, 0)];
        if ty & 0x4000 != 0 {
            st.extend([St::W(N + 0x14, 0), St::W(N + 0x18, 0)]);
            if ty == 0xD065 {
                st.extend((0x1C..0x58).step_by(4).map(|o| St::W(N + o, if [0x1C, 0x2C, 0x3C].contains(&o) { ONE } else { 0 })));
            }
        }
        same_memory(&after, &expect(&s, &st))?;
    }

    #[test]
    fn func_800183C0(seed: u64) {
        let s = state(seed);
        let after = run("func_800183C0", misc::func_800183C0, &s)?;
        let b = 0x800D_6960;
        let mut st = vec![St::W(b + 0x1C, 9), St::W(b + 0x18, 0), St::H(b + 0x20, 0), St::H(b + 0x22, 2), St::W(b + 0x2C, 0), St::W(b + 0x30, 1), St::H(b + 0x34, 30), St::H(b + 0x36, 0), St::W(b + 0x38, 0)];
        st.extend((0..6).map(|k| St::W(b + 4 * k, 0)));
        st.extend([St::W(b + 0x28, ONE), St::W(b + 0x24, ONE)]);
        same_memory(&after, &expect(&s, &st))?;
    }
}
