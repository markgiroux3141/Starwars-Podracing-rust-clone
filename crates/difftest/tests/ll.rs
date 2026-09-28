//! libultra's 64-bit helpers from `ll.c` (game::libultra), 0x8008AAE0..
//! 0x8008AD74. Recompiled C vs Rust with the 64-bit arguments in register
//! pairs, each checked against Rust's own u64/i64 arithmetic.
//!
//! `__ll_div` and `__ll_mod` of `INT64_MIN` by -1 reach IDO's `break 6`:
//! recomp.h's `DDIV` computes it without a fault, so the runtime's
//! `do_break` traps. That runs in a child process, C and Rust alike.

// Tests are named after the functions (func_8008AAE0), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::libultra;
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

const SP0: u32 = 0x800A_2800;

/// The pair `(hi, lo)` for a 64-bit value, each word sign-extended.
fn pair(v: u64) -> (u64, u64) {
    (sext((v >> 32) as u32), sext(v as u32))
}

fn state(seed: u64, a: u64, b: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP0);
    (s.ctx.gpr[A0], s.ctx.gpr[A1]) = pair(a);
    (s.ctx.gpr[A2], s.ctx.gpr[A3]) = pair(b);
    s
}

fn result(s: &State) -> u64 {
    (s.ctx.gpr[V0] << 32) | (s.ctx.gpr[V1] & 0xFFFF_FFFF)
}

fn value() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        Just(1u64),
        Just(u64::MAX),
        Just(i64::MIN as u64),
        Just(i64::MAX as u64),
        Just(0xFFFF_FFFFu64),
        Just(1u64 << 32),
        (-10i64..10).prop_map(|v| v as u64),
        (0u32..64).prop_map(|k| 1u64 << k),
        any::<u32>().prop_map(u64::from),
        any::<u32>().prop_map(sext),
        any::<u64>(),
    ]
}

fn shift() -> impl Strategy<Value = u64> {
    prop_oneof![0u64..64, 64u64..70, Just(u64::MAX), any::<u64>()]
}

/// The two-argument helpers: name, port, and what they compute (`None`
/// outside the domain: a zero divisor, or `INT64_MIN / -1`).
type Op = fn(u64, u64) -> Option<u64>;
const HELPERS: [(&str, RecompFn, Op); 7] = [
    ("func_8008AB0C", libultra::func_8008AB0C, |a, b| a.checked_rem(b)),
    ("func_8008AB48", libultra::func_8008AB48, |a, b| a.checked_div(b)),
    ("func_8008ABB0", libultra::func_8008ABB0, |a, b| a.checked_rem(b)),
    ("func_8008ABEC", libultra::func_8008ABEC, |a, b| (a as i64).checked_div(b as i64).map(|q| q as u64)),
    ("func_8008AC48", libultra::func_8008AC48, |a, b| Some(a.wrapping_mul(b))),
    ("func_8008ACD8", libultra::func_8008ACD8, |a, b| {
        let (a, b) = (a as i64, b as i64);
        let r = a.checked_rem(b)?;
        Some((if (r < 0 && b > 0) || (r > 0 && b < 0) { r + b } else { r }) as u64)
    }),
    ("func_8008AAE0", libultra::func_8008AAE0, |a, n| Some(a >> (n & 63))),
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn two_argument_helpers(seed: u64, k in 0usize..7, a in value(), b in value()) {
        let (name, port, op) = HELPERS[k];
        let Some(want) = op(a, b) else { return Ok(()) };
        let s = state(seed, a, b);
        let after = run(name, port, &s)?;
        prop_assert_eq!(result(&after), want, "{}({:#x}, {:#x})", name, a, b);
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP0));
    }

    #[test]
    fn shifts(seed: u64, k in 0usize..3, a in value(), n in shift()) {
        let (name, port, want): (&str, RecompFn, u64) = [
            ("func_8008AAE0", libultra::func_8008AAE0 as RecompFn, a >> (n & 63)),
            ("func_8008AB84", libultra::func_8008AB84, a << (n & 63)),
            ("func_8008AD74", libultra::func_8008AD74, ((a as i64) >> (n & 63)) as u64),
        ][k];
        let s = state(seed, a, n);
        let after = run(name, port, &s)?;
        prop_assert_eq!(result(&after), want);
    }

    /// Quotient and remainder by the (sign-extended, QUIRK) halfword divisor.
    #[test]
    fn func_8008AC78(seed: u64, a in value(), d in prop_oneof![1u16..10, Just(0x8000u16), Just(0xFFFFu16), 1u16..=u16::MAX]) {
        let mut s = state(seed, 0, a);
        let (q, r) = (0x8030_0000u32, 0x8030_0010u32);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(q), sext(r));
        s.rdram.mem().write_u32(SP0 + 0x10, 0x1234_0000 | u32::from(d));
        let after = run("func_8008AC78", libultra::func_8008AC78, &s)?;
        let dv = d as i16 as i64 as u64;
        let read = |at: u32| (u64::from(word(&after, at)) << 32) | u64::from(word(&after, at + 4));
        prop_assert_eq!((read(q), read(r)), (a / dv, a % dv));
    }
}

/// `__ll_rem` is unsigned (libultra's source), `__ll_mod` follows the divisor.
#[test]
fn rem_and_mod_signs() {
    for (a, b, m) in [(-7i64, 2i64, 1i64), (7, -2, -1), (-7, -2, -1), (7, 2, 1)] {
        let (ua, ub) = (a as u64, b as u64);
        let after = compare("func_8008ABB0", libultra::func_8008ABB0, &state(1, ua, ub)).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(result(&after), ua % ub, "__ll_rem({a}, {b})");
        let after = compare("func_8008ACD8", libultra::func_8008ACD8, &state(1, ua, ub)).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(result(&after) as i64, m, "__ll_mod({a}, {b})");
    }
}

const CHILD_ENV: &str = "LL_CHILD";

#[test]
fn trap_child() {
    let Ok(which) = std::env::var(CHILD_ENV) else { return };
    let (name, side) = which.split_once(':').unwrap();
    let port = if name == "func_8008ABEC" { libultra::func_8008ABEC as RecompFn } else { libultra::func_8008ACD8 };
    let mut s = state(3, i64::MIN as u64, u64::MAX);
    let f = match side {
        "c" => oracle::recomp::by_name(name).unwrap(),
        _ => difftest::port_under_test(name, port),
    };
    s.run(f);
    std::process::exit(0);
}

/// `INT64_MIN / -1` and `mod -1` reach `break 6` in C and Rust alike.
#[test]
fn int64_min_by_minus_one_breaks() {
    for (name, at) in [("func_8008ABEC", "0x8008AC30"), ("func_8008ACD8", "0x8008AD20")] {
        for side in ["c", "rust"] {
            let out = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "trap_child", "--nocapture", "--test-threads=1"])
                .env(CHILD_ENV, format!("{name}:{side}"))
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(!out.status.success(), "{name} {side}: returned");
            assert!(stderr.contains(&format!("break instruction at {at}")), "{name} {side}: unexpected stderr:\n{stderr}");
        }
    }
}
