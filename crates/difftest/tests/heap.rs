//! The heap helpers: func_8002FAFC (heap_cursor), func_8002FC80 (heap_check),
//! func_8002FAC4 (heap_set_cursor), func_8002FC58 (heap_free). Recompiled C
//! vs game::heap. FAC4 and FC58 call FC80 and FAFC, which are compiled into
//! the oracle, so both runs call the same C callee.

use difftest::{compare, Rng, State};
use game::heap::{self, CURSORS, HEAP_END, HEAP_TOP, LEVEL};
use game::recomp::reg::*;
use game::recomp::RecompFn;
use proptest::prelude::*;

/// Texture cache, right after the 10 cursor slots. func_8002FC80's walk runs
/// into it when slots 1..=9 are all in use.
const TEX_CACHE: u32 = 0x800D_9E00;
/// A stack in the boot stack's range (sp starts at 0x800A2830). The thread
/// that runs the loaders has its own stack, not located yet; any RDRAM that
/// doesn't overlap the data works.
const STACK: u32 = 0x800A_2800;

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

/// Random registers and memory, then the heap globals: `level`, the slots
/// (slot 0 first), the heap end and top.
fn setup(seed: u64, level: i32, slots: &[u32], end: u32, top: u32) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(STACK);
    let mut m = s.rdram.mem();
    m.write_u32(LEVEL, level as u32);
    for (k, &v) in slots.iter().enumerate() {
        m.write_u32(CURSORS + 4 * k as u32, v);
    }
    m.write_u32(HEAP_END, end);
    m.write_u32(HEAP_TOP, top);
    s
}

/// Slots as the game keeps them: 0..=`used` hold cursors, the rest are zero.
fn slots(rng: &mut Rng, used: usize) -> [u32; 10] {
    let mut s = [0u32; 10];
    for v in &mut s[..=used.min(9)] {
        // Nonzero; mostly heap addresses, sometimes anything.
        *v = match rng.next_u32() % 4 {
            0 => rng.next_u32() | 1,
            _ => 0x8014_D7E0 + rng.next_u32() % 0x0030_0000,
        };
    }
    s
}

fn run(name: &str, port: RecompFn, s: &State) -> State {
    compare(name, port, s).unwrap_or_else(|d| panic!("{d}"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn heap_cursor(seed in any::<u64>(), level in -0x3000i32..0x3000) {
        let s = setup(seed, level, &[], 0, 0);
        let after = compare("func_8002FAFC", heap::func_8002FAFC, &s).map_err(|d| TestCaseError::fail(d.to_string()))?;
        let want = s.rdram.as_words()[((CURSORS.wrapping_add((level as u32).wrapping_mul(4)) - 0x8000_0000) / 4) as usize];
        prop_assert_eq!(after.ctx.gpr[V0], sext(want));
    }

    #[test]
    fn heap_check(seed in any::<u64>(), used in 0usize..=9, top in any::<u32>(), a0 in any::<u32>()) {
        let mut rng = Rng::new(seed);
        let mut s = setup(seed, used as i32, &slots(&mut rng, used), 0, top);
        s.ctx.gpr[A0] = sext(a0);
        // The walk past slot 9 stops at the first zero in the texture cache.
        s.rdram.mem().write_u32(TEX_CACHE + 4 * (rng.next_u32() % 64), 0);
        compare("func_8002FC80", heap::func_8002FC80, &s).map_err(|d| TestCaseError::fail(d.to_string()))?;
    }

    #[test]
    fn heap_set_cursor(seed in any::<u64>(), level in 0i32..=9, used in 0usize..=9, a0 in any::<u32>(), top in any::<u32>()) {
        let mut rng = Rng::new(seed);
        let mut s = setup(seed, level, &slots(&mut rng, used), 0, top);
        s.ctx.gpr[A0] = sext(a0);
        s.rdram.mem().write_u32(TEX_CACHE + 4 * (rng.next_u32() % 64), 0);
        let mut after = compare("func_8002FAC4", heap::func_8002FAC4, &s).map_err(|d| TestCaseError::fail(d.to_string()))?;
        prop_assert_eq!(after.rdram.mem().read_u32(CURSORS + 4 * level as u32), a0);
        prop_assert_eq!(after.ctx.gpr[SP], sext(STACK));
    }

    #[test]
    fn heap_free(seed in any::<u64>(), level in 0i32..=9, end in any::<u32>()) {
        let mut rng = Rng::new(seed);
        let sl = slots(&mut rng, 9);
        let s = setup(seed, level, &sl, end, 0);
        let after = compare("func_8002FC58", heap::func_8002FC58, &s).map_err(|d| TestCaseError::fail(d.to_string()))?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(end.wrapping_sub(sl[level as usize])));
    }
}

#[test]
fn heap_check_walk_edges() {
    let mut rng = Rng::new(1);
    // Slot 1 zero: no walk, a0 survives, and slot 0 is the one compared.
    let mut s = setup(1, 0, &slots(&mut rng, 0), 0, 0x8030_0000);
    s.ctx.gpr[A0] = 0x1234_5678_9ABC_DEF0;
    let after = run("func_8002FC80", heap::func_8002FC80, &s);
    assert_eq!(after.ctx.gpr[A0], 0x1234_5678_9ABC_DEF0);
    assert_eq!(after.ctx.gpr[V1], 1);

    // Slot 1 zero but later slots set: still no walk.
    let mut sl = slots(&mut rng, 9);
    sl[1] = 0;
    run("func_8002FC80", heap::func_8002FC80, &setup(2, 0, &sl, 0, 0));

    // QUIRK: every slot in use, so the walk runs into the texture cache and
    // stops at its first zero word (here the 5th).
    let mut s = setup(3, 9, &slots(&mut rng, 9), 0, 0x8030_0000);
    let mut m = s.rdram.mem();
    for k in 0..4 {
        m.write_u32(TEX_CACHE + 4 * k, 0x8020_0000 + k);
    }
    m.write_u32(TEX_CACHE + 16, 0);
    let after = run("func_8002FC80", heap::func_8002FC80, &s);
    // v1 ends as the index of the first zero word counted from CURSORS:
    // slots 1..=9, texture cache words as slots 10..=13, the zero at 14.
    assert_eq!(after.ctx.gpr[V1], 14);
    assert_eq!(after.ctx.gpr[T0], sext(0x8020_0003));

    // Both outcomes of the compare, and equal values (sltu is false).
    for (last, top) in [(0x8020_0000, 0x8030_0000), (0x8030_0000, 0x8020_0000), (0x8020_0000, 0x8020_0000), (0x8020_0000, 0x0000_0001)] {
        let mut sl = [0u32; 10];
        sl[0] = 0x8014_D7E0;
        sl[1] = last;
        run("func_8002FC80", heap::func_8002FC80, &setup(4, 1, &sl, 0, top));
    }
}

#[test]
fn upper_register_halves() {
    // ra and a0 are stored with sw (low words), ra comes back sign-extended,
    // and sp passes through ADD32, so non-canonical upper halves are in the
    // domain for all three.
    let mut rng = Rng::new(5);
    let sl = slots(&mut rng, 3);
    for (name, port) in [
        ("func_8002FAC4", heap::func_8002FAC4 as RecompFn),
        ("func_8002FC58", heap::func_8002FC58),
        ("func_8002FAFC", heap::func_8002FAFC),
        ("func_8002FC80", heap::func_8002FC80),
    ] {
        let mut s = setup(6, 2, &sl, 0x8040_0000, 0x8040_0000);
        s.ctx.gpr[RA] = 0x0BAD_F00D_8003_0123;
        s.ctx.gpr[A0] = 0x7777_0000_8020_0000;
        s.ctx.gpr[SP] = 0x3333_0000_0000_0000 | u64::from(STACK);
        let after = run(name, port, &s);
        if name == "func_8002FAC4" || name == "func_8002FC58" {
            assert_eq!(after.ctx.gpr[RA], sext(0x8003_0123), "{name}");
            assert_eq!(after.ctx.gpr[SP], sext(STACK), "{name}");
        }
    }
}

#[test]
fn levels_outside_the_array() {
    // Levels past 9 or negative address other words; the game never sets
    // them (func_8002FA00 checks < 9), but the functions just index.
    for level in [-3, 10, 11, 0x40] {
        let mut s = setup(7, level, &[0x8014_D7E0, 0x8016_0000], 0x8040_0000, 0x8040_0000);
        s.rdram.mem().write_u32(TEX_CACHE, 0);
        run("func_8002FAFC", heap::func_8002FAFC, &s);
        run("func_8002FC58", heap::func_8002FC58, &s);
        run("func_8002FAC4", heap::func_8002FAC4, &s);
    }
}
