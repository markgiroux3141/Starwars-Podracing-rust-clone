//! The pool registry leaves at 0x8003F300..0x8003FB78 (game::misc): element
//! init, a search by tag, count get/set and an iterator. Recompiled C vs
//! Rust on random registries (duplicate ids included), each checked
//! against its statement.

// Tests are named after the functions (func_8003F300), capitals included.
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

const LIST: u32 = 0x8030_0000;
const DESCS: u32 = 0x8030_0100;
const ELEMS: u32 = 0x8030_1000;

#[derive(Clone, Debug)]
struct Pool {
    id: u32,
    tag: u32,
    count: i32,
    size: u32,
}

fn pools() -> impl Strategy<Value = Vec<Pool>> {
    let pool = (0u32..3, any::<u32>(), prop_oneof![0i32..6, Just(-1i32)], prop_oneof![Just(8u32), Just(12u32), Just(16u32)])
        .prop_map(|(id, tag, count, size)| Pool { id: 0x100 + id, tag, count, size });
    proptest::collection::vec(pool, 0..4)
}

fn desc(k: usize) -> u32 {
    DESCS + 0x20 * k as u32
}

fn base(k: usize) -> u32 {
    ELEMS + 0x100 * k as u32
}

fn registry(seed: u64, pools: &[Pool]) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(0x800A_2800);
    s.randomise_memory(seed ^ 2, LIST, 0x2000);
    let mut m = s.rdram.mem();
    m.write_u32(0x800A_2170, LIST);
    for (k, p) in pools.iter().enumerate() {
        m.write_u32(LIST + 4 * k as u32, desc(k));
        let d = desc(k);
        m.write_u32(d, p.id);
        m.write_u32(d + 4, p.tag);
        m.write_u32(d + 8, p.count as u32);
        m.write_u32(d + 0xC, p.size);
        m.write_u32(d + 0x10, base(k));
    }
    m.write_u32(LIST + 4 * pools.len() as u32, 0);
    drop(m);
    s
}

fn first(pools: &[Pool], id: u64) -> Option<usize> {
    pools.iter().position(|p| u64::from(p.id) == id)
}

fn ids() -> impl Strategy<Value = u64> {
    prop_oneof![0x100u64..0x104, Just(0x1_0000_0101u64)]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Every pool with the id: element k gets (id, k, tag).
    #[test]
    fn func_8003F300(seed: u64, pools in pools(), id in ids()) {
        let mut s = registry(seed, &pools);
        s.ctx.gpr[A0] = id;
        let after = run("func_8003F300", misc::func_8003F300, &s)?;
        for (k, p) in pools.iter().enumerate() {
            for e in 0..6u32 {
                let a = base(k) + e * p.size;
                if u64::from(p.id) == id && (e as i32) < p.count {
                    prop_assert_eq!((word(&after, a), half(&after, a + 4), half(&after, a + 6)), (p.id, e as u16, p.tag as u16));
                } else {
                    prop_assert_eq!((word(&after, a), word(&after, a + 4)), (word(&s, a), word(&s, a + 4)));
                }
            }
        }
    }

    /// The first element, in any pool with the id, with bit 8 of +6 clear
    /// and +4 == tag.
    #[test]
    fn func_8003F714(seed: u64, pools in pools(), id in ids(), tag in prop_oneof![0i64..4, Just(0x1_0000_0001i64), Just(-1i64)], flags in proptest::collection::vec(prop_oneof![Just(0u16), Just(0x100u16), any::<u16>()], 24), tags in proptest::collection::vec(prop_oneof![0u16..4, Just(0xFFFFu16)], 24)) {
        let mut s = registry(seed, &pools);
        for (k, p) in pools.iter().enumerate() {
            for e in 0..6u32 {
                let a = base(k) + e * p.size;
                s.rdram.mem().write_u16(a + 4, tags[k * 6 + e as usize]);
                s.rdram.mem().write_u16(a + 6, flags[k * 6 + e as usize]);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (id, tag as u64);
        let after = run("func_8003F714", misc::func_8003F714, &s)?;
        let want = pools.iter().enumerate().filter(|(_, p)| u64::from(p.id) == id).find_map(|(k, p)| {
            (0..p.count.max(0) as u32).map(|e| base(k) + e * p.size).find(|&a| {
                half(&s, a + 6) & 0x100 == 0 && half(&s, a + 4) as i16 as i64 == tag
            })
        });
        prop_assert_eq!(after.ctx.gpr[V0], want.map_or(0, sext));
        prop_assert_eq!(after.ctx.gpr[S0], s.ctx.gpr[S0]);
    }

    /// Count of the first pool with the id.
    #[test]
    fn func_8003F7B8(seed: u64, pools in pools(), id in ids()) {
        let mut s = registry(seed, &pools);
        s.ctx.gpr[A0] = id;
        let after = run("func_8003F7B8", misc::func_8003F7B8, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], first(&pools, id).map_or(0, |k| sext(pools[k].count as u32)));
    }

    /// Begin at i, then step until the end.
    #[test]
    fn iteration(seed: u64, pools in pools(), id in ids(), i in -1i64..7, steps in 0usize..8) {
        let mut s = registry(seed, &pools);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (id, i as u64);
        let mut after = run("func_8003F800", misc::func_8003F800, &s)?;
        let pool = first(&pools, id).filter(|&k| i < i64::from(pools[k].count));
        let elem = |k: usize, e: i64| sext(base(k).wrapping_add((e as u32).wrapping_mul(pools[k].size)));
        prop_assert_eq!(after.ctx.gpr[V0], pool.map_or(0, |k| elem(k, i)));
        prop_assert_eq!(word(&after, 0x800A_4AA4), pool.map_or(0, desc));
        let mut e = i;
        let mut live = pool;
        for _ in 0..steps {
            let before = after.clone();
            after = run("func_8003F890", misc::func_8003F890, &before)?;
            let want = match live {
                Some(k) => {
                    e += 1;
                    if e < i64::from(pools[k].count) { elem(k, e) } else { live = None; 0 }
                }
                None => 0,
            };
            prop_assert_eq!(after.ctx.gpr[V0], want);
            prop_assert_eq!(word(&after, 0x800A_4AA4), live.map_or(0, desc));
        }
    }

    /// Set count and base of the first pool with the id; count * size.
    #[test]
    fn func_8003FB78(seed: u64, pools in pools(), id in ids(), count in 0u32..100, b: u32) {
        let mut s = registry(seed, &pools);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (id, u64::from(count), sext(b));
        let after = run("func_8003FB78", misc::func_8003FB78, &s)?;
        match first(&pools, id) {
            Some(k) => {
                prop_assert_eq!(after.ctx.gpr[V0], u64::from(count * pools[k].size));
                prop_assert_eq!((word(&after, desc(k) + 8), word(&after, desc(k) + 0x10)), (count, b));
            }
            None => prop_assert_eq!(after.ctx.gpr[V0], 0),
        }
    }
}
