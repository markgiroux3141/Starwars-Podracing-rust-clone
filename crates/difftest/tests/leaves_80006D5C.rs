//! The leaves at 0x80006D5C..0x80007D44 (game::anim, game::misc): an object search, flag
//! set/clear, empty functions, the audio DMA "new" hook, a record clear and
//! a handle lookup. Recompiled C vs Rust on random register files and random
//! memory, each checked against its statement.

// Tests are named after the functions (func_80006D5C), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, Rng, State};
use game::anim::{self, OBJECTS};
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

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(0x800A_2800);
    s
}

/// A canonical, word-aligned address with `room` bytes after it in RDRAM.
fn addr(room: u32) -> impl Strategy<Value = u64> {
    (0x8000_0000u32..0x8080_0000 - room).prop_map(|v| sext(v & !3))
}

/// Where the test objects live.
const POOL: u32 = 0x8030_0000;
const OBJ: u32 = 0x130;
/// Entries past the 300-word table that a larger count can reach.
const SLACK: u32 = 16;

/// The object table as a game might have it: `count` in use, entries 0 or
/// pointers to objects with random flags (bit 31, kind nibble) and ids from
/// a small set, so matches happen. The words after the table get the same
/// treatment, so counts above 300 stay inside the domain.
fn objects(s: &mut State, seed: u64, count: i32) {
    let mut rng = Rng::new(seed);
    let mut m = s.rdram.mem();
    m.write_u32(0x8009_A2A0, count as u32);
    for k in 0..300 + SLACK {
        let entry = match rng.next_u32() % 4 {
            0 => 0,
            _ => POOL + OBJ * k,
        };
        m.write_u32(OBJECTS + 4 * k, entry);
        let flags = (rng.next_u32() & 0x8000_0000) | (rng.next_u32() % 4) | (rng.next_u32() & 0x0FF0);
        m.write_u32(POOL + OBJ * k + 0x100, flags);
        let id = [1, 2, 3, 0x8000_0001, 0][rng.next_u32() as usize % 5];
        m.write_u32(POOL + OBJ * k + 0x124, id);
    }
}

/// The search, stated directly.
fn find(s: &State, id: u64, kind: u64) -> u64 {
    if id == 0 {
        return 0;
    }
    let count = word(s, 0x8009_A2A0) as i32;
    for k in 0..count.max(0) as u32 {
        let o = word(s, OBJECTS + 4 * k);
        if o == 0 {
            continue;
        }
        let flags = sext(word(s, o + 0x100));
        if flags & 0x8000_0000 == 0 && flags & 0xF == kind && sext(word(s, o + 0x124)) == id {
            return sext(o);
        }
    }
    0
}

fn id() -> impl Strategy<Value = u64> {
    prop_oneof![Just(0u64), Just(1u64), Just(2u64), Just(3u64), Just(sext(0x8000_0001)), Just(0x8000_0001u64), any::<u64>()]
}

fn kind() -> impl Strategy<Value = u64> {
    prop_oneof![0u64..4, Just(0x1_0000_0001u64), any::<u64>()]
}

fn count() -> impl Strategy<Value = i32> {
    prop_oneof![Just(0), Just(-1), Just(i32::MIN), Just(1), Just(300), Just(300 + SLACK as i32), 0..300]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80006D5C(seed: u64, id in id(), kind in kind(), count in count()) {
        let mut s = state(seed);
        objects(&mut s, seed, count);
        s.ctx.gpr[A0] = id;
        s.ctx.gpr[A1] = kind;
        let after = run("func_80006D5C", anim::func_80006D5C, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], find(&s, id, kind));
        prop_assert_eq!(after.ctx.gpr[S0], sext(s.ctx.gpr[S0] as u32), "s0 restored from its low word");
    }

    #[test]
    fn func_80006E50(seed: u64, o in addr(0x104), bits: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = o;
        s.ctx.gpr[A1] = bits;
        let after = run("func_80006E50", anim::func_80006E50, &s)?;
        let at = o as u32 + 0x100;
        prop_assert_eq!(word(&after, at), word(&s, at) | bits as u32);
    }

    #[test]
    fn func_80006E60(seed: u64, o in addr(0x104), bits: u64) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = o;
        s.ctx.gpr[A1] = bits;
        let after = run("func_80006E60", anim::func_80006E60, &s)?;
        let at = o as u32 + 0x100;
        prop_assert_eq!(word(&after, at), word(&s, at) & !(bits as u32));
    }

    /// The empty functions: 6F34 spills a0; 6F3C spills a0 and a1 and
    /// returns 0; the rest do nothing.
    #[test]
    fn empty_functions(seed: u64, sp in addr(8), a0: u64, a1: u64) {
        let mut s = state(seed);
        s.ctx.gpr[SP] = sp;
        s.ctx.gpr[A0] = a0;
        s.ctx.gpr[A1] = a1;
        let sp = sp as u32;

        let after = run("func_80006F34", misc::func_80006F34, &s)?;
        prop_assert_eq!(word(&after, sp), a0 as u32);
        prop_assert_eq!(after.ctx.gpr, s.ctx.gpr);

        let after = run("func_80006F3C", misc::func_80006F3C, &s)?;
        prop_assert_eq!((word(&after, sp), word(&after, sp + 4)), (a0 as u32, a1 as u32));
        prop_assert_eq!(after.ctx.gpr[V0], 0);

        for (name, port) in [
            ("func_80006FD4", misc::func_80006FD4 as RecompFn),
            ("func_80006FDC", misc::func_80006FDC),
            ("func_8000758C", misc::func_8000758C),
        ] {
            let after = run(name, port, &s)?;
            prop_assert_eq!(after.ctx.gpr, s.ctx.gpr);
            prop_assert!(after.rdram.as_words() == s.rdram.as_words());
        }
    }

    #[test]
    fn func_80007710(seed: u64, state_ptr in addr(4)) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = state_ptr;
        let after = run("func_80007710", misc::func_80007710, &s)?;
        prop_assert_eq!(word(&after, state_ptr as u32), 0x800A_FAC0);
        prop_assert_eq!(after.ctx.gpr[V0], sext(0x8000_7594));
    }

    #[test]
    fn func_80007A44(seed: u64, flag in prop_oneof![Just(0u32), any::<u32>()]) {
        let mut s = state(seed);
        s.randomise_memory(seed, 0x800D_2000, 0x200);
        s.rdram.mem().write_u32(0x8009_A2B8, flag);
        let after = run("func_80007A44", misc::func_80007A44, &s)?;
        for a in (0x800D_2000..0x800D_2200).step_by(4) {
            let cleared = flag != 0 && (0x800D_2038..0x800D_2138).contains(&a) && (a - 0x800D_2038) % 0x20 == 0x18;
            prop_assert_eq!(word(&after, a), if cleared { 0 } else { word(&s, a) }, "{:#x}", a);
        }
    }

    /// Any handle, canonical or not, with random tables (and whatever lies
    /// past them).
    #[test]
    fn func_80007CE4(seed: u64, handle in prop_oneof![any::<u32>().prop_map(sext), any::<u64>(), (0u64..0x300).prop_map(|k| sext(0x8000 | (k as u32) << 16 | 1 << 24)), (0u64..0x100).prop_map(|k| sext(0x8000 | (k as u32) << 24))]) {
        let mut s = state(seed);
        s.randomise_memory(seed, 0x8009_A300, 0x800);
        s.randomise_memory(seed ^ 1, 0x800A_FA00, 0x100);
        s.ctx.gpr[A0] = handle;
        let after = run("func_80007CE4", misc::func_80007CE4, &s)?;
        // Byte `sh / 8` of the register as N64Recomp's sra sees it (a
        // 64-bit arithmetic shift).
        let byte = |sh: u32| ((handle as i64) >> sh) as u32 & 0xFF;
        let want = if handle & 0x8000 == 0 {
            word(&s, 0x800A_FA54)
        } else if byte(24) <= 1 {
            word(&s, 0x8009_A32C + 4 * byte(16))
        } else {
            word(&s, 0x8009_A388 + 4 * byte(24))
        };
        prop_assert_eq!(after.ctx.gpr[V0], sext(want));
    }
}
