//! The two depth-0 functions with indirect calls (LOOKUP_FUNC), in the pool
//! registry (game::pools): func_8003F99C (one element's pool callback) and
//! func_8003FA24 (a broadcast to pools). The callbacks are real verified
//! functions compiled into the oracle, reached through get_function by C
//! and Rust alike: func_8005F31C marks an element (`+0x14` counts, `+0x18
//! = arg`), func_8006FED0 returns `[elem + 0x60]` (2 stops a broadcast).
//! Callbacks with no function, or with no C and no double, trap (child
//! process).

// Tests are named after the functions (func_8003F99C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::pools;
use game::recomp::{reg::*, RecompFn};
use proptest::prelude::*;
use std::process::Command;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const LIST: u32 = 0x8030_0000;
const DESCS: u32 = 0x8030_0100;
const ELEMS: u32 = 0x8030_1000;
const MARK: u32 = 0x8005_F31C;
const STOP: u32 = 0x8006_FED0;
const ALL: u32 = 0x416C_6C21;
const SIZE: u32 = 0x68;

#[derive(Clone, Debug)]
struct Pool {
    id: u32,
    count: i32,
    callback: u32,
    flagged: Vec<bool>,
    stops: Vec<bool>,
}

fn pools() -> impl Strategy<Value = Vec<Pool>> {
    let pool = (
        1u32..3,
        prop_oneof![0i32..4, Just(-1i32)],
        prop_oneof![3 => Just(MARK), 1 => Just(STOP), 1 => Just(0u32)],
        proptest::collection::vec(any::<bool>(), 4),
        proptest::collection::vec(prop_oneof![5 => Just(false), 1 => Just(true)], 4),
    )
        .prop_map(|(id, count, callback, flagged, stops)| Pool { id, count, callback, flagged, stops });
    proptest::collection::vec(pool, 0..4)
}

fn elem(k: usize, i: u32) -> u32 {
    ELEMS + 0x200 * k as u32 + SIZE * i
}

fn registry(seed: u64, pools: &[Pool]) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(0x800A_2800);
    s.randomise_memory(seed ^ 2, LIST, 0x1800);
    let mut m = s.rdram.mem();
    m.write_u32(0x800A_2170, LIST);
    for (k, p) in pools.iter().enumerate() {
        let d = DESCS + 0x40 * k as u32;
        m.write_u32(LIST + 4 * k as u32, d);
        m.write_u32(d, p.id);
        m.write_u32(d + 8, p.count as u32);
        m.write_u32(d + 0xC, SIZE);
        m.write_u32(d + 0x10, elem(k, 0));
        m.write_u32(d + 0x24, p.callback);
        for i in 0..4u32 {
            let e = elem(k, i);
            m.write_u32(e, p.id);
            m.write_u16(e + 6, if p.flagged[i as usize] { 0x100 } else { 0 });
            m.write_u32(e + 0x60, if p.stops[i as usize] { 2 } else { 7 });
        }
    }
    m.write_u32(LIST + 4 * pools.len() as u32, 0);
    drop(m);
    s
}

/// The elements a broadcast marks (MARK callbacks), by the statement.
fn broadcast(pools: &[Pool], id: u32) -> Vec<(usize, u32)> {
    let mut marked = Vec::new();
    for (k, p) in pools.iter().enumerate() {
        if id != p.id && id != ALL {
            continue;
        }
        if p.callback != 0 {
            for i in 0..p.count.max(0) as u32 {
                if p.flagged[i as usize] {
                    continue;
                }
                if p.callback == MARK {
                    marked.push((k, i));
                } else if p.stops[i as usize] {
                    return marked;
                }
            }
        }
        if id != ALL {
            break;
        }
    }
    marked
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The first pool with the element's id calls back cb(elem, arg).
    #[test]
    fn func_8003F99C(seed: u64, pools in pools(), which in 0usize..4, i in 0u32..4, null: bool, arg: u32) {
        let mut s = registry(seed, &pools);
        let e = elem(which.min(pools.len().saturating_sub(1)), i);
        // An element whose pool's callback is MARK is visible; STOP does
        // nothing to memory.
        let target = pools.iter().position(|p| p.id == word(&s, e));
        s.ctx.gpr[A0] = if null { 0 } else { sext(e) };
        s.ctx.gpr[A1] = sext(arg);
        let after = run("func_8003F99C", pools::func_8003F99C, &s)?;
        let marked = !null
            && !pools.is_empty()
            && target.is_some_and(|k| pools[k].callback == MARK)
            && word(&s, e + 4) & 0x100 == 0;
        if marked {
            prop_assert_eq!(word(&after, e + 0x18), arg);
            prop_assert_eq!(word(&after, e + 0x14), word(&s, e + 0x14).wrapping_add(1));
        } else {
            prop_assert_eq!((word(&after, e + 0x14), word(&after, e + 0x18)), (word(&s, e + 0x14), word(&s, e + 0x18)));
        }
        prop_assert_eq!(after.ctx.gpr[SP], sext(0x800A_2800));
    }

    /// Broadcast: MARK the unflagged elements of the matching pools (all for
    /// "All!"), until a STOP element answers 2.
    #[test]
    fn func_8003FA24(seed: u64, pools in pools(), id in prop_oneof![1u32..4, Just(ALL)], arg: u32) {
        let mut s = registry(seed, &pools);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(id), sext(arg));
        let after = run("func_8003FA24", pools::func_8003FA24, &s)?;
        let marked = broadcast(&pools, id);
        for k in 0..pools.len() {
            for i in 0..4u32 {
                let e = elem(k, i);
                let want = if marked.contains(&(k, i)) { (word(&s, e + 0x14).wrapping_add(1), arg) } else { (word(&s, e + 0x14), word(&s, e + 0x18)) };
                prop_assert_eq!((word(&after, e + 0x14), word(&after, e + 0x18)), want, "pool {} element {}", k, i);
            }
        }
        for r in [S0, S1, S2, S3, S4, S5, S6, S7, FP, RA, SP] {
            prop_assert_eq!(after.ctx.gpr[r], s.ctx.gpr[r]);
        }
    }
}

const CHILD_ENV: &str = "INDIRECT_CHILD";

/// In the child: func_8003F99C with the callback at the given address.
#[test]
fn trap_child() {
    let Ok(which) = std::env::var(CHILD_ENV) else { return };
    let (side, callback) = which.split_once(':').unwrap();
    let pools = [Pool { id: 1, count: 1, callback: u32::from_str_radix(callback, 16).unwrap(), flagged: vec![false; 4], stops: vec![false; 4] }];
    let mut s = registry(5, &pools);
    s.ctx.gpr[A0] = sext(elem(0, 0));
    let f = match side {
        "c" => oracle::recomp::by_name("func_8003F99C").unwrap(),
        _ => difftest::port_under_test("func_8003F99C", pools::func_8003F99C),
    };
    s.run(f);
    std::process::exit(0);
}

/// An address where no function starts traps in get_function; a function
/// with no C in the oracle and no double traps in its stub. C and Rust alike.
/// The unported function is a depth-27 one, so it stays unported longest.
#[test]
fn bad_callbacks_trap() {
    assert!(oracle::recomp::by_name("func_8004AF60").is_none(), "func_8004AF60 is in the oracle now: pick an unported function");
    for (callback, message) in [("80010000", "LOOKUP_FUNC(0x80010000)"), ("8004AF60", "call to func_8004AF60")] {
        for side in ["c", "rust"] {
            let out = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "trap_child", "--nocapture", "--test-threads=1"])
                .env(CHILD_ENV, format!("{side}:{callback}"))
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(!out.status.success(), "{side} {callback}: returned");
            assert!(stderr.contains(message), "{side} {callback}: unexpected stderr:\n{stderr}");
        }
    }
}
