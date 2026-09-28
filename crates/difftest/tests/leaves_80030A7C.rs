//! The leaves at 0x80030A7C..0x80031F94 (game::misc): a header-list merge,
//! model_load's statistics getter, memset, four 28-byte channel records
//! (three of the setters call themselves for "all channels") and the
//! animation object's destination. Recompiled C vs Rust on random register
//! files and memory, each checked against its statement.

// Tests are named after the functions (func_80030A7C), capitals included.
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

fn byte(s: &State, a: u32) -> u8 {
    (word(s, a & !3) >> (24 - 8 * (a & 3))) as u8
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const O: u32 = 0x8030_0000;
const SP0: u32 = 0x800A_2800;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.randomise_memory(seed, O, 0x4000);
    s.ctx.gpr[SP] = sext(SP0);
    s
}

// ---------------------------------------------------------------------------
// func_80030A7C: merge q into p

const P: u32 = O;
const Q: u32 = O + 0x1000;
const TARGETS: u32 = O + 0x2000;
const DATA: u32 = 0x4461_7461;
const ANIM: u32 = 0x416E_696D;
const ALTN: u32 = 0x416C_744E;

#[derive(Clone, Debug)]
struct Lists {
    p_nodes: Vec<u32>,
    q_nodes: Vec<u32>,
    data: Option<(i32, Vec<u32>)>,
    anim: Option<Vec<u32>>,
    p_alt: Option<Vec<u32>>,
    q_alt: bool,
    q_vals: Vec<u32>,
}

fn nonzero() -> impl Strategy<Value = u32> {
    any::<u32>().prop_filter("a list word", |&w| w != 0 && w != u32::MAX)
}

fn lists() -> impl Strategy<Value = Lists> {
    (0usize..6).prop_flat_map(|nn| {
        (
            proptest::collection::vec(nonzero(), nn),
            proptest::collection::vec(prop_oneof![Just(0u32), nonzero()], nn),
            proptest::option::of(prop_oneof![
                (-2i32..1).prop_map(|n| (n, vec![])),
                proptest::collection::vec(any::<u32>(), 1..5).prop_map(|v| (v.len() as i32, v)),
            ]),
            proptest::option::of(proptest::collection::vec(nonzero(), 0..4)),
            proptest::option::of(proptest::collection::vec(0u32..32, 0..5)),
            any::<bool>(),
            proptest::collection::vec(any::<u32>(), 8),
        )
            .prop_map(|(p_nodes, q_nodes, data, anim, p_alt, q_alt, q_vals)| Lists { p_nodes, q_nodes, data, anim, p_alt, q_alt, q_vals })
    })
}

fn lists_state(seed: u64, l: &Lists) -> State {
    let mut s = state(seed);
    let mut m = s.rdram.mem();
    let mut p = Vec::new();
    p.extend(&l.p_nodes);
    p.push(u32::MAX);
    if let Some((n, words)) = &l.data {
        p.push(DATA);
        p.push(*n as u32);
        p.extend(words);
    }
    if let Some(words) = &l.anim {
        p.push(ANIM);
        p.extend(words);
        p.push(0);
    }
    if let Some(targets) = &l.p_alt {
        p.push(ALTN);
        p.extend(targets.iter().map(|t| TARGETS + 4 * t));
        p.push(0);
    } else {
        p.push(0x1234_5678);
    }
    for (k, w) in p.iter().enumerate() {
        m.write_u32(P + 4 * k as u32, *w);
    }
    let nn = l.q_nodes.len() as u32;
    for (k, w) in l.q_nodes.iter().enumerate() {
        m.write_u32(Q + 4 * k as u32, *w);
    }
    m.write_u32(Q + 4 * (nn + 1), if l.q_alt { ALTN } else { 0x8765_4321 });
    for (k, w) in l.q_vals.iter().enumerate() {
        m.write_u32(Q + 4 * (nn + 2 + k as u32), *w);
    }
    drop(m);
    (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(P), sext(Q));
    s
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn func_80030A7C(seed: u64, l in lists()) {
        let s = lists_state(seed, &l);
        let after = run("func_80030A7C", misc::func_80030A7C, &s)?;
        for (k, (&p, &q)) in l.p_nodes.iter().zip(&l.q_nodes).enumerate() {
            prop_assert_eq!(word(&after, P + 4 * k as u32), if q != 0 { q } else { p }, "node {}", k);
        }
        let mut targets: Vec<u32> = (0..32).map(|t| word(&s, TARGETS + 4 * t)).collect();
        if let (Some(ptrs), true) = (&l.p_alt, l.q_alt) {
            for (j, &t) in ptrs.iter().enumerate() {
                targets[t as usize] = l.q_vals[j];
            }
        }
        for (t, &w) in targets.iter().enumerate() {
            prop_assert_eq!(word(&after, TARGETS + 4 * t as u32), w, "target {}", t);
        }
        for k in 0..0x40 {
            prop_assert_eq!(word(&after, Q + 4 * k), word(&s, Q + 4 * k), "q {}", k);
        }
    }
}

// ---------------------------------------------------------------------------
// The rest

const CH: u32 = 0x800D_B8A0;

fn channel(i: u64) -> u32 {
    CH.wrapping_add((i as u32).wrapping_mul(28))
}

fn channel_state(seed: u64) -> State {
    let mut s = state(seed);
    s.randomise_memory(seed ^ 7, CH - 0x40, 0x100);
    s
}

fn arg() -> impl Strategy<Value = u64> {
    prop_oneof![0u64..8, Just(0x1_0000_0000), Just(u64::MAX), any::<u32>().prop_map(sext), any::<u64>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The three model_load statistics words.
    #[test]
    fn func_80030B68(seed: u64) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 1, 0x800D_9DC0, 0x20);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(O), sext(O + 4), sext(O + 8));
        let after = run("func_80030B68", misc::func_80030B68, &s)?;
        prop_assert_eq!(
            (word(&after, O), word(&after, O + 4), word(&after, O + 8)),
            (word(&s, 0x800D_9DC8), word(&s, 0x800D_9DD0), word(&s, 0x800D_9DCC))
        );
    }

    /// memset.
    #[test]
    fn func_800313D8(seed: u64, off in 0u32..16, b: u64, n in prop_oneof![0u64..8, 0u64..0x300, Just(0x100)]) {
        let mut s = state(seed);
        let p = O + 0x100 + off;
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(p), b, n);
        let after = run("func_800313D8", misc::func_800313D8, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(p));
        for a in p - 4..p + n as u32 + 4 {
            let want = if a >= p && a < p + n as u32 { b as u8 } else { byte(&s, a) };
            prop_assert_eq!(byte(&after, a), want, "{:#x}", a);
        }
    }

    /// The channel's word +0.
    #[test]
    fn func_800314C0(seed: u64, i in prop_oneof![0u64..4, Just(u64::MAX), 4u64..8]) {
        let mut s = channel_state(seed);
        s.ctx.gpr[A0] = i;
        let after = run("func_800314C0", misc::func_800314C0, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(word(&s, channel(i))));
    }

    /// Set the value; +6 reset unless active with the same value and a2.
    #[test]
    fn func_800314DC(seed: u64, i in 0u64..4, value in arg(), a2 in prop_oneof![Just(0u64), arg()], a3 in prop_oneof![Just(0u64), arg()], active: bool, same: bool) {
        let mut s = channel_state(seed);
        let r = channel(i);
        if !active {
            s.rdram.mem().write_u16(r + 6, 0);
        } else if half(&s, r + 6) == 0 {
            s.rdram.mem().write_u16(r + 6, 0x40);
        }
        if same {
            s.rdram.mem().write_u16(r + 4, value as u16);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (i, value, a2, a3);
        let after = run("func_800314DC", misc::func_800314DC, &s)?;
        let keep = half(&s, r + 6) != 0 && half(&s, r + 4) == value as u16 && a2 != 0;
        prop_assert_eq!(half(&after, r + 6), if keep { half(&s, r + 6) } else { 0x8000 });
        prop_assert_eq!(half(&after, r + 4), value as u16);
        prop_assert_eq!(half(&after, r + 0x16), value as u16);
        prop_assert_eq!(half(&after, r + 8), if a2 == 0 { 0xFFFF } else if a3 == 0 { 1 } else { 2 });
        prop_assert_eq!(word(&after, SP0 + 4), value as u32);
    }

    /// Start, clear +0xC, clear +8: one channel, or all four for -1.
    #[test]
    fn channel_setters(seed: u64, k in 0usize..3, i in prop_oneof![0u64..4, Just(u64::MAX), Just(0xFFFF_FFFFu64), 4u64..6]) {
        let (name, port): (&str, RecompFn) = [
            ("func_80031560", misc::func_80031560 as RecompFn),
            ("func_800315D8", misc::func_800315D8),
            ("func_80031640", misc::func_80031640),
        ][k];
        let mut s = channel_state(seed);
        s.ctx.gpr[A0] = i;
        let after = run(name, port, &s)?;
        let all: Vec<u64> = if i == u64::MAX { (0..4).collect() } else { vec![i] };
        for c in -1i64..6 {
            let r = channel(c as u64);
            let hit = all.iter().any(|&x| channel(x) == r);
            let (w_c, h_6, h_8) = (word(&after, r + 0xC), half(&after, r + 6), half(&after, r + 8));
            let (w0, h60, h80) = (word(&s, r + 0xC), half(&s, r + 6), half(&s, r + 8));
            match (hit, k) {
                (true, 0) => prop_assert_eq!((w_c, h_6, h_8), (1, 0x8000, h80)),
                (true, 1) => prop_assert_eq!((w_c, h_6, h_8), (0, h60, h80)),
                (true, _) => prop_assert_eq!((w_c, h_6, h_8), (w0, h60, 0)),
                (false, _) => prop_assert_eq!((w_c, h_6, h_8), (w0, h60, h80)),
            }
        }
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP0));
    }

    /// [0x800DB910 + 4 i] = 1.
    #[test]
    fn func_80031BEC(seed: u64, i in 0u64..8) {
        let mut s = channel_state(seed);
        s.ctx.gpr[A0] = i;
        let after = run("func_80031BEC", misc::func_80031BEC, &s)?;
        for j in 0..8u32 {
            let a = 0x800D_B910 + 4 * j;
            prop_assert_eq!(word(&after, a), if u64::from(j) == i { 1 } else { word(&s, a) });
        }
    }

    /// The animation object's destination: [p + 8], or 0.
    #[test]
    fn animation_dest(seed: u64, set: bool) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x800A_2DD4, O + 0x200);
        s.ctx.gpr[A0] = sext(O + 0x300);
        let (name, port): (&str, RecompFn) =
            if set { ("func_80031F80", misc::func_80031F80) } else { ("func_80031F94", misc::func_80031F94) };
        let after = run(name, port, &s)?;
        prop_assert_eq!(word(&after, O + 0x204), if set { word(&s, O + 0x308) } else { 0 });
    }
}
