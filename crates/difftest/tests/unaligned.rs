//! The unaligned word accesses: game::recomp's `lwl`/`lwr`/`swl`/`swr`
//! against recomp.h's `do_lwl`/`do_lwr`/`do_swl`/`do_swr` run on the same
//! RDRAM (oracle::unaligned_probe), and func_800827C8, the one game
//! function using them: a deliberate crash (a store to address 1), which
//! both sides must die of. Those runs happen in child processes.

use difftest::State;
use game::recomp::{lwl, lwr, swl, swr};
use game::misc;
use n64mem::Mem;
use oracle::unaligned_probe::*;
use proptest::prelude::*;
use std::process::Command;

const CHILD_ENV: &str = "UNALIGNED_CHILD";

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

/// Addresses in RDRAM (canonical), at every misalignment, and offsets.
fn place() -> impl Strategy<Value = (u64, i32)> {
    (0x8000_0100u32..0x807F_FF00, -0x100i32..0x100).prop_map(|(b, o)| (sext(b), o))
}

fn mem(s: &mut State) -> Mem<'_> {
    s.rdram.mem()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn loads_match_recomp_h((base, off) in place(), initial: u64, seed: u64) {
        let mut s = State::new();
        let a = (base as u32).wrapping_add(off as u32) & !3;
        s.randomise_memory(seed, a, 4);
        let (l, r) = {
            let m = mem(&mut s);
            (lwl(&m, initial, base, off), lwr(&m, initial, base, off))
        };
        let p = s.rdram.as_mut_ptr();
        // SAFETY: p is the full RDRAM buffer; the address is inside it.
        let (cl, cr) = unsafe { (unaligned_probe_lwl(p, initial, base, off as i64 as u64), unaligned_probe_lwr(p, initial, base, off as i64 as u64)) };
        prop_assert_eq!(l, cl, "lwl");
        prop_assert_eq!(r, cr, "lwr");
    }

    #[test]
    fn stores_match_recomp_h((base, off) in place(), value: u64, seed: u64, right: bool) {
        let a = (base as u32).wrapping_add(off as u32) & !3;
        let mut c = State::new();
        c.randomise_memory(seed, a, 4);
        let mut rust = c.clone();
        {
            let mut m = mem(&mut rust);
            if right { swr(&mut m, base, off, value) } else { swl(&mut m, base, off, value) }
        }
        let p = c.rdram.as_mut_ptr();
        // SAFETY: as above.
        unsafe {
            if right {
                unaligned_probe_swr(p, base, off as i64 as u64, value)
            } else {
                unaligned_probe_swl(p, base, off as i64 as u64, value)
            }
        }
        let w = |s: &State| s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize];
        prop_assert_eq!(w(&rust), w(&c));
    }
}

/// The MIPS meaning, for a few cases by hand: `lwl`/`lwr` at the two ends
/// of a word assemble it, `swl`/`swr` split it.
#[test]
fn an_unaligned_word_round_trips() {
    let mut s = State::new();
    {
        let mut m = mem(&mut s);
        m.write_u32(0x8030_0000, 0x1122_3344);
        m.write_u32(0x8030_0004, 0x5566_7788);
    }
    let m = mem(&mut s);
    // The word at 0x80300001: bytes 22 33 44 55.
    let hi = lwl(&m, 0, sext(0x8030_0000), 1);
    let w = lwr(&m, hi, sext(0x8030_0000), 4);
    assert_eq!(w, sext(0x2233_4455));
    drop(m);
    let mut m = mem(&mut s);
    swl(&mut m, sext(0x8030_0000), 1, 0xAABB_CCDD);
    swr(&mut m, sext(0x8030_0000), 4, 0xAABB_CCDD);
    assert_eq!((m.read_u32(0x8030_0000), m.read_u32(0x8030_0004)), (0x11AA_BBCC, 0xDD66_7788));
}

/// In the child: run one side of func_800827C8. Reaching the end means it
/// returned instead of crashing.
#[test]
fn crash_child() {
    let Ok(side) = std::env::var(CHILD_ENV) else { return };
    let mut s = State::new();
    let f = match side.as_str() {
        "c" => oracle::recomp::by_name("func_800827C8").unwrap(),
        "rust" => difftest::port_under_test("func_800827C8", misc::func_800827C8),
        _ => unreachable!(),
    };
    s.run(f);
    std::process::exit(0);
}

/// func_800827C8 stores to address 1: the C faults on the host (outside
/// RDRAM), the port stops at n64mem's bounds check.
#[test]
fn func_800827C8_crashes() {
    for side in ["c", "rust"] {
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "crash_child", "--nocapture", "--test-threads=1"])
            .env(CHILD_ENV, side)
            .output()
            .unwrap();
        assert!(!out.status.success(), "{side}: returned instead of crashing");
        if side == "rust" {
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(stderr.contains("is outside the"), "{side}: unexpected stderr:\n{stderr}");
        }
    }
}
