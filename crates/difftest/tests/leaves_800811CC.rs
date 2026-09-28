//! The leaves at 0x800811CC..0x80086CC8 (game::misc): flags, a memory
//! compare, strcmp, a spill, the RSP graphics task setup, a record flag and
//! three halfwords. Recompiled C vs Rust on random register files and
//! memory, each checked against its statement.

// Tests are named after the functions (func_800811CC), capitals included.
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

fn half(s: &State, a: u32) -> u16 {
    (word(s, a & !3) >> (16 - 8 * (a & 2))) as u16
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const O: u32 = 0x8030_0000;
const SP0: u32 = 0x800A_2800;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed, O, 0x400);
    s.ctx.gpr[SP] = sext(SP0);
    s
}

fn put(s: &mut State, a: u32, bytes: &[u8]) {
    s.rdram.mem().write_bytes(a, bytes);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn flags(seed: u64, set: bool) {
        let s = state(seed);
        if set {
            let after = run("func_800811CC", misc::func_800811CC, &s)?;
            prop_assert_eq!(word(&after, 0x800A_6758), 1);
        } else {
            let after = run("func_80081260", misc::func_80081260, &s)?;
            prop_assert_eq!(word(&after, 0x800A_675C), 0);
        }
    }

    /// Equal first n bytes (n <= 0 is equal).
    #[test]
    fn func_80081530(seed: u64, a in proptest::collection::vec(0u8..3, 0..24), diff_at in 0usize..24, n in prop_oneof![-2i64..26, Just(i64::MIN)], off_a in 0u32..4, off_b in 0u32..4) {
        let mut s = state(seed);
        let mut b = a.clone();
        if let Some(x) = b.get_mut(diff_at) {
            *x ^= 1;
        }
        let (pa, pb) = (O + 0x100 + off_a, O + 0x200 + off_b);
        put(&mut s, pa, &a);
        put(&mut s, pb, &b);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(pa), sext(pb), n as u64);
        let after = run("func_80081530", misc::func_80081530, &s)?;
        let len = n.clamp(0, a.len() as i64) as usize;
        let beyond = n > a.len() as i64;
        if !beyond {
            let want = a[..len] == b[..len];
            prop_assert_eq!(after.ctx.gpr[V0], u64::from(want));
        }
    }

    /// strcmp: -1, 0 or 1 by the first differing unsigned byte.
    #[test]
    fn func_800815FC(seed: u64, a in proptest::collection::vec(prop_oneof![1u8..4, Just(0x80u8), Just(0xFFu8)], 0..8), b in proptest::collection::vec(prop_oneof![1u8..4, Just(0x80u8), Just(0xFFu8)], 0..8), same: bool) {
        let mut s = state(seed);
        let b = if same { a.clone() } else { b };
        let (pa, pb) = (O + 0x100, O + 0x200);
        put(&mut s, pa, &[a.as_slice(), &[0]].concat());
        put(&mut s, pb, &[b.as_slice(), &[0]].concat());
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(pa), sext(pb));
        let after = run("func_800815FC", misc::func_800815FC, &s)?;
        let want = match a.cmp(&b) {
            std::cmp::Ordering::Less => u64::MAX,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        };
        prop_assert_eq!(after.ctx.gpr[V0], want);
    }

    #[test]
    fn func_800834DC(seed: u64, args: [u64; 4]) {
        let mut s = state(seed);
        s.ctx.gpr[A0..=A3].copy_from_slice(&args);
        let after = run("func_800834DC", misc::func_800834DC, &s)?;
        for k in 0..4 {
            prop_assert_eq!(word(&after, SP0 + 4 * k as u32), args[k] as u32);
        }
    }

    /// The graphics task's boot, output and data fields; F3DEX2 for type 5.
    #[test]
    fn func_80084C30(seed: u64, ty in prop_oneof![Just(5u64), 0u64..8, Just(0x1_0005u64), any::<u64>()], out: u32, size: u32, data: u32) {
        let mut s = state(seed);
        let task = O + 0x100;
        let mut m = s.rdram.mem();
        m.write_u32(0x8014_88C0, task);
        m.write_u32(0x800D_B894, out);
        m.write_u32(0x800D_B898, size);
        m.write_u32(0x8012_17B4, data);
        drop(m);
        s.ctx.gpr[A0] = ty;
        let after = run("func_80084C30", misc::func_80084C30, &s)?;
        let five = ty as u16 as i16 == 5;
        let want = [
            (8, 0x8009_7FF0),
            (0xC, 0xD0),
            (0x10, if five { 0x8009_80C0 } else { word(&s, task + 0x10) }),
            (0x18, if five { 0x800A_E1D0 } else { word(&s, task + 0x18) }),
            (0x28, out),
            (0x2C, size),
            (0x30, data),
        ];
        for (off, w) in want {
            prop_assert_eq!(word(&after, task + off), w, "+{:#x}", off);
        }
        prop_assert_eq!(word(&after, SP0), ty as u32);
    }

    /// Bit 0 of +0 by v's sign, +4 = v.
    #[test]
    fn func_8008635C(seed: u64, i in 0u64..4, v in prop_oneof![Just(0u64), Just(u64::MAX), any::<u32>().prop_map(sext), any::<u64>()], flags: u32) {
        let mut s = state(seed);
        let r = 0x8012_0DF0 + 0x170 * i as u32;
        s.rdram.mem().write_u32(r, flags);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (i, v);
        let after = run("func_8008635C", misc::func_8008635C, &s)?;
        prop_assert_eq!(word(&after, r), if (v as i64) >= 0 { flags | 1 } else { flags & !1 });
        prop_assert_eq!(word(&after, r + 4), v as u32);
    }

    #[test]
    fn func_80086CC8(seed: u64, a: u64, b: u64, c: u64) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (a, b, c);
        let after = run("func_80086CC8", misc::func_80086CC8, &s)?;
        prop_assert_eq!([0, 2, 4].map(|k| half(&after, 0x8014_88B8 + k)), [a as u16, b as u16, c as u16]);
    }
}
