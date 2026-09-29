//! func_800129E4 (game::misc), the text width: recompiled C vs Rust on
//! strings with `~` escapes, lowercase, NULs and high characters, over fonts
//! with and without lowercase and extended tables, each checked against a
//! model of the statement that reads the same RDRAM.

// Tests are named after the functions (func_800129E4), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::misc;
use game::recomp::reg::*;
use n64mem::Mem;
use proptest::prelude::*;

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;
const S: u32 = 0x8030_0000;
const FONT: u32 = 0x8030_1000;
const GLYPHS: u32 = 0x8030_2000;
const EXTENDED: u32 = 0x8030_4000;
const MAP: u32 = 0x800A_1C86;
const PAIRS: u32 = 0x800A_1CD8;

/// The statement, reading the font, tables and string from memory.
fn width(m: &Mem, s: u32, font: u32) -> u32 {
    let (first, last) = (u32::from(m.read_u8(font + 0x5A)), u32::from(m.read_u8(font + 0x5B)));
    let (table, ext) = (m.read_u32(font + 0x5C), m.read_u32(font + 0x60));
    let (mut p, mut w) = (s, 0u32);
    loop {
        let c = u32::from(m.read_u8(p));
        let mut done = c == 0;
        let (mut code, mut has) = (c, c);
        if c == 0x7E {
            let c2 = m.read_u8(p + 1);
            p += 1;
            match c2 {
                b'n' => done = true,
                b'~' => (code, has) = (0x7E, 0x7E),
                _ => (code, has) = (0, 0),
            }
        }
        if has != 0 && !done {
            if (0x61..0x7B).contains(&has) && last < 0x61 {
                code = has - 0x20;
            }
            let mut glyph = 0u32;
            if code >= 0x97 && ext != 0 {
                let k = u32::from(m.read_u8(MAP + code));
                if k != 0xFF {
                    let (g0, g1) = (u32::from(m.read_u8(PAIRS + 2 * k)), u32::from(m.read_u8(PAIRS + 2 * k + 1)));
                    if g1 == 0xFF {
                        glyph = ext.wrapping_add(16 * g0);
                        code = 0;
                    } else {
                        code = g1;
                    }
                }
            }
            if table != 0 && first <= code && code <= last {
                glyph = table.wrapping_add(16 * (code - first));
            }
            if glyph != 0 {
                w = w.wrapping_add(m.read_i16(glyph + 2) as i32 as u32);
            }
        }
        p += 1;
        if done {
            return w;
        }
    }
}

/// String bytes: plain printable characters, lowercase, `~` escapes (`~n`,
/// `~~`, `~X`), high characters, and a NUL at the end.
fn text() -> impl Strategy<Value = Vec<u8>> {
    let byte = prop_oneof![
        4 => 0x20u8..0x7F,
        2 => b'a'..=b'z',
        1 => Just(b'~'),
        1 => prop::sample::select(vec![b'n', b'~', b'x', b'1']),
        2 => 0x97u8..=0xFF,
        1 => 0x80u8..0x97,
    ];
    prop::collection::vec(byte, 0..40).prop_map(|mut v| {
        v.push(0);
        v
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// `last` below or above `'a'`; `first` 0 (the QUIRK) or not; tables
    /// present or null; map and pair tables partly structured.
    #[test]
    fn func_800129E4(seed: u64, s in text(), first in prop_oneof![Just(0u8), 0x20u8..0x41], last in prop_oneof![0x5Au8..=0x62, 0x7Au8..=0xFF],
                     table: bool, ext: bool, map in prop::collection::vec(prop_oneof![Just(0xFFu8), 0u8..40, any::<u8>()], 0x69),
                     pairs in prop::collection::vec((0u8..64, prop_oneof![Just(0xFFu8), 0x20u8..0x7F]), 64)) {
        let mut st = State::new();
        st.randomise_registers(seed);
        st.ctx.gpr[SP] = sext(SP_AT);
        st.randomise_memory(seed ^ 3, GLYPHS, 0x1000);
        st.randomise_memory(seed ^ 5, EXTENDED, 0x1000);
        {
            let mut m = st.rdram.mem();
            m.write_bytes(S, &s);
            m.write_u8(FONT + 0x5A, first);
            m.write_u8(FONT + 0x5B, last);
            m.write_u32(FONT + 0x5C, if table { GLYPHS } else { 0 });
            m.write_u32(FONT + 0x60, if ext { EXTENDED } else { 0 });
            for (k, (g0, g1)) in pairs.iter().enumerate() {
                m.write_u8(PAIRS + 2 * k as u32, *g0);
                m.write_u8(PAIRS + 2 * k as u32 + 1, *g1);
            }
            // The map is written last: it lies inside the pair table's span.
            for (k, b) in map.iter().enumerate() {
                m.write_u8(MAP + 0x97 + k as u32, *b);
            }
        }
        (st.ctx.gpr[A0], st.ctx.gpr[A1]) = (sext(S), sext(FONT));
        let want = width(&st.rdram.clone().mem(), S, FONT);
        let after = compare("func_800129E4", misc::func_800129E4, &st).map_err(|d| TestCaseError::fail(d.to_string()))?;
        prop_assert_eq!(after.ctx.gpr[V0], sext(want));
        // Callee-saved registers come back, sign-extended.
        for r in [S0, S1, S2, S3] {
            prop_assert_eq!(after.ctx.gpr[r], sext(st.ctx.gpr[r] as u32));
        }
    }
}

/// The escapes and the lowercase fold on a font whose glyph c has advance c
/// (so the width is the sum of the codes counted).
#[test]
fn escapes() {
    // (string, last character of the font, width)
    let cases: [(&[u8], u8, u32); 7] = [
        (b"AB\0", 0x7E, 0x41 + 0x42),
        (b"A~nB\0", 0x7E, 0x41),
        (b"A~~B\0", 0x7E, 0x41 + 0x7E + 0x42),
        (b"A~xB\0", 0x7E, 0x41 + 0x42),
        (b"ab\0", 0x7E, 0x61 + 0x62),
        (b"ab\0", 0x5A, 0x41 + 0x42),
        (b"\0", 0x7E, 0),
    ];
    for (k, (s, last, want)) in cases.into_iter().enumerate() {
        let mut st = State::new();
        st.randomise_registers(k as u64);
        st.ctx.gpr[SP] = sext(SP_AT);
        {
            let mut m = st.rdram.mem();
            m.write_bytes(S, s);
            m.write_u8(FONT + 0x5A, 0x20);
            m.write_u8(FONT + 0x5B, last);
            m.write_u32(FONT + 0x5C, GLYPHS);
            m.write_u32(FONT + 0x60, 0);
            for c in 0x20u32..=0x7E {
                m.write_u16(GLYPHS + 16 * (c - 0x20) + 2, c as u16);
            }
        }
        (st.ctx.gpr[A0], st.ctx.gpr[A1]) = (sext(S), sext(FONT));
        let after = compare("func_800129E4", misc::func_800129E4, &st).unwrap_or_else(|d| panic!("{d}"));
        assert_eq!(after.ctx.gpr[V0], u64::from(want), "{:?}", String::from_utf8_lossy(s));
    }
}
