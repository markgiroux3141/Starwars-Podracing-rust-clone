//! The leaves at 0x80039A30..0x8003E54C (game::misc): framebuffers_init, a
//! framebuffer save/restore, spline point helpers and walker start, setters,
//! a display-list prologue and a 190-entry list. Recompiled C vs Rust on
//! random register files and memory, each checked against its statement.

// Tests are named after the functions (func_80039A30), capitals included.
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

const SP0: u32 = 0x800A_2800;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP0);
    s
}

// ---------------------------------------------------------------------------
// Spline helpers

const SPLINE: u32 = 0x8031_0000;
const POINTS: u32 = 0x8031_1000;
const W: u32 = 0x8031_4000;
const N: u32 = 16;

fn pt(i: u32) -> u32 {
    POINTS.wrapping_add(i.wrapping_mul(0x54))
}

fn spline_state(seed: u64, flag: i16, ids: &[i16], links: &[(i16, i16)]) -> State {
    let mut s = state(seed);
    s.randomise_memory(seed ^ 1, SPLINE, 0x5000);
    let mut m = s.rdram.mem();
    m.write_u16(SPLINE, flag as u16);
    m.write_u32(SPLINE + 4, N);
    m.write_u32(SPLINE + 0xC, POINTS);
    for k in 0..N {
        m.write_u16(pt(k) + 0x40, ids[k as usize] as u16);
        let (count, next) = links[k as usize];
        m.write_u16(pt(k), count as u16);
        m.write_u16(pt(k) + 4, next as u16);
    }
    m.write_u32(W, SPLINE);
    drop(m);
    s
}

fn ids() -> impl Strategy<Value = Vec<i16>> {
    proptest::collection::vec(prop_oneof![0i16..6, any::<i16>()], N as usize)
}

fn links() -> impl Strategy<Value = Vec<(i16, i16)>> {
    proptest::collection::vec((prop_oneof![Just(0i16), 1i16..3, Just(-1i16)], 0i16..N as i16), N as usize)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Point i's halfword +0x40.
    #[test]
    fn func_8003A4E8(seed: u64, ids in ids(), links in links(), i in 0u64..N as u64) {
        let mut s = spline_state(seed, 0, &ids, &links);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(SPLINE), i);
        let after = run("func_8003A4E8", misc::func_8003A4E8, &s)?;
        prop_assert_eq!(after.ctx.gpr[V0], ids[i as usize] as i64 as u64);
    }

    /// The first point from `from` whose +0x40 is `id`, or -1.
    #[test]
    fn func_8003A50C(seed: u64, ids in ids(), links in links(), id in prop_oneof![-1i64..6, Just(0x1_0000_0002i64)], from in prop_oneof![0u64..N as u64 + 2, Just(u64::MAX)]) {
        let mut s = spline_state(seed, 0, &ids, &links);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(SPLINE), id as u64, from);
        let after = run("func_8003A50C", misc::func_8003A50C, &s)?;
        let start = (from as i64).max(i64::MIN);
        let want = (start..N as i64).find(|&i| i >= 0 && i64::from(ids[i as usize]) == id);
        // A negative `from` would scan points before the array; only
        // nonnegative starts are checked against the statement.
        if (from as i64) >= 0 {
            prop_assert_eq!(after.ctx.gpr[V0], want.map_or(u64::MAX, |i| i as u64));
        }
    }

    /// The walker's point index, or a halfword of that point by the path bits.
    #[test]
    fn func_8003A568(seed: u64, ids in ids(), links in links(), k in 0u64..4, idx in [0u32..N, 0u32..N, 0u32..N, 0u32..N], bits in prop_oneof![Just(0u32), 1u32..16, any::<u32>().prop_map(|b| b & 0x7)]) {
        let mut s = spline_state(seed, 0, &ids, &links);
        for (j, &i) in idx.iter().enumerate() {
            s.rdram.mem().write_u32(W + 0x10 + 4 * j as u32, i);
        }
        s.rdram.mem().write_u32(W + 0x2C, bits);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(W), k);
        let after = run("func_8003A568", misc::func_8003A568, &s)?;
        let p = idx[k as usize];
        let want = if bits == 0 {
            sext(p)
        } else {
            let j = (bits as i32) >> k;
            half(&s, pt(p) + 0x42 + 2 * j as u32) as i16 as i64 as u64
        };
        prop_assert_eq!(after.ctx.gpr[V0], want);
    }

    /// Start the walker at p: follow first successors while they exist.
    #[test]
    fn func_8003B250(seed: u64, ids in ids(), links in links(), flag in prop_oneof![Just(0i16), any::<i16>()], p in 0u64..N as u64) {
        let mut s = spline_state(seed, flag, &ids, &links);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(W), p);
        let after = run("func_8003B250", misc::func_8003B250, &s)?;
        let mut want = [p as u32; 4];
        let next = |i: u32| {
            let (count, n) = links[i as usize];
            (count != 0).then_some(n as i32 as u32)
        };
        if let Some(a) = next(p as u32) {
            want[1] = a;
            if flag == 0 {
                if let Some(b) = next(a) {
                    want[2] = b;
                    if let Some(c) = next(b) {
                        want[3] = c;
                    }
                }
            }
        }
        for j in 0..4 {
            prop_assert_eq!(word(&after, W + 0x10 + 4 * j), want[j as usize], "+{:#x}", 0x10 + 4 * j);
        }
    }
}

// ---------------------------------------------------------------------------
// The rest

fn fb(size: u32) -> [u32; 3] {
    let end = size | 0x8000_0000;
    let big = size >= 0x80_0000;
    let (w, bpp, x) = if big { (640u32, 4u32, 2 * 4 * 640u32) } else { (320, 2, 0) };
    [1, 2, 3].map(|k| end.wrapping_sub(k * (w * 240 * bpp + x)).wrapping_add(x))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_80039A30(seed: u64, size in prop_oneof![Just(0x40_0000u32), Just(0x80_0000u32), Just(0x7F_FFFFu32), any::<u32>()]) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x8000_0318, size);
        let after = run("func_80039A30", misc::func_80039A30, &s)?;
        let want = fb(size);
        for k in 0..3 {
            prop_assert_eq!(word(&after, 0x8011_4530 + 4 * k), want[k as usize]);
        }
        prop_assert_eq!(after.ctx.gpr[S0], s.ctx.gpr[S0]);
        prop_assert_eq!(after.ctx.gpr[SP], sext(SP0));
    }

    /// Save three words, or restore the first if it was saved.
    #[test]
    fn func_80039CD8(seed: u64, restore in prop_oneof![Just(0u64), any::<u64>()], saved in prop_oneof![Just(0u32), any::<u32>()]) {
        let mut s = state(seed);
        s.randomise_memory(seed ^ 3, 0x8011_4480, 0x100);
        s.rdram.mem().write_u32(0x8011_44A4, saved);
        s.ctx.gpr[A0] = restore;
        let after = run("func_80039CD8", misc::func_80039CD8, &s)?;
        if restore == 0 {
            prop_assert_eq!(word(&after, 0x8011_44A4), word(&s, 0x8011_453C));
            prop_assert_eq!(word(&after, 0x8011_44B4), word(&s, 0x8011_4504));
            prop_assert_eq!(word(&after, 0x8011_44C8), word(&s, 0x8011_4518));
        } else if saved != 0 {
            prop_assert_eq!(word(&after, 0x8011_453C), saved);
            prop_assert_eq!(word(&after, 0x8011_44A4), 0);
        } else {
            prop_assert_eq!(word(&after, 0x8011_453C), word(&s, 0x8011_453C));
        }
    }

    #[test]
    fn func_8003B300(seed: u64, a: u64, b: u64, c: u64, d: u64) {
        let mut s = state(seed);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (a, b, c, d);
        let after = run("func_8003B300", misc::func_8003B300, &s)?;
        prop_assert_eq!(
            [0x8011_4540, 0x8011_4544, 0x8011_4548, 0x8011_454C].map(|x| word(&after, x)),
            [b as u32, d as u32, a as u32, c as u32]
        );
    }

    /// Texture, combine, othermode, then the optional two by the flags.
    #[test]
    fn func_8003D370(seed: u64, flags: u32) {
        let mut s = state(seed);
        let dl = 0x8030_0000;
        s.rdram.mem().write_u32(0x8012_17B0, dl);
        s.rdram.mem().write_u32(0x800A_4960, flags);
        let after = run("func_8003D370", misc::func_8003D370, &s)?;
        let mut want = vec![0xD700_0000, 0x8000_8000, 0xFCFF_FFFF, 0xFFFE_793C, 0xE200_1D00, 0];
        if flags & 1 != 0 {
            want.extend([0xE200_001C, 0x0F0A_4000]);
        }
        if flags & 4 != 0 {
            want.extend([0xE200_1E01, 0]);
        }
        for (k, w) in want.iter().enumerate() {
            prop_assert_eq!(word(&after, dl + 4 * k as u32), *w, "word {}", k);
        }
        prop_assert_eq!(word(&after, 0x8012_17B0), dl + 4 * want.len() as u32);
    }

    #[test]
    fn small(seed: u64, v: u64, which: bool) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = v;
        if which {
            let after = run("func_8003D488", misc::func_8003D488, &s)?;
            prop_assert_eq!(word(&after, 0x800A_48D4), v as u32 & 0xFFFF);
            prop_assert_eq!(word(&after, SP0), v as u32);
        } else {
            let after = run("func_8003E1D0", misc::func_8003E1D0, &s)?;
            prop_assert_eq!([0x800A_4984, 0x800A_4970, 0x800A_4978].map(|x| word(&after, x)), [0, 0, 0]);
        }
    }

    /// Append (x, y, b) while fewer than 190.
    #[test]
    fn func_8003E54C(seed: u64, n in prop_oneof![0i32..4, 186i32..192, Just(-1i32)], b: u64, x: u64, y: u64) {
        let mut s = state(seed);
        s.rdram.mem().write_u32(0x800A_4984, n as u32);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (b, x, y);
        let after = run("func_8003E54C", misc::func_8003E54C, &s)?;
        if n < 190 {
            let e = 0x8011_8958u32.wrapping_add((n as u32).wrapping_mul(4));
            prop_assert_eq!((half(&after, e), half(&after, e + 2)), (x as u16, y as u16));
            prop_assert_eq!(byte(&after, 0x8011_8C50u32.wrapping_add(n as u32)), b as u8);
            prop_assert_eq!(word(&after, 0x800A_4984), (n + 1) as u32);
        } else {
            prop_assert_eq!(word(&after, 0x800A_4984), n as u32);
        }
        prop_assert_eq!(word(&after, SP0), b as u32);
    }
}

/// The flags are read again after the render-mode command: with the list
/// placed so that command's second word lands on the flags (0x0F0A4000, bit
/// 2 clear), the last command isn't appended.
#[test]
fn func_8003D370_rereads_flags() {
    let mut s = state(9);
    let dl = 0x800A_4960 - 3 * 8 - 4;
    s.rdram.mem().write_u32(0x8012_17B0, dl);
    s.rdram.mem().write_u32(0x800A_4960, 5);
    let after = compare("func_8003D370", misc::func_8003D370, &s).unwrap_or_else(|d| panic!("{d}"));
    assert_eq!(word(&after, 0x800A_4960), 0x0F0A_4000);
    assert_eq!(word(&after, 0x8012_17B0), dl + 4 * 8);
}
