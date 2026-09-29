//! The glyph rectangle at 0x80011F38 (game::render), the last depth-0
//! float function: recompiled C vs Rust, checked against its statement by
//! simulating the stores (the text box, the three commands, the list head
//! and the argument spills) on a copy of the input and comparing all of
//! RDRAM. It is the first port of IDO's float → unsigned idiom
//! (`fpu::to_unsigned_s`), whose flag path is dead under the oracle.

// Tests are named after the functions (func_80011F38), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::recomp::{reg::*, RecompFn};
use game::render::{self, DL_HEAD, TEXT_BOX};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn sext(v: u32) -> u64 {
    v as i32 as i64 as u64
}

const SP_AT: u32 = 0x803F_0000;
const SCREEN: u32 = 0x8011_4470;
const DL: u32 = 0x8030_4000;

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Glyph {
    /// x, y, ox, oy in a0..a3 (any 64-bit value: only the low half counts),
    /// then the stack words w, h, s, t, tile.
    regs: [u64; 4],
    stack: [u32; 5],
    screen: (i16, i16),
    /// ymin, ymax, xmin, xmax.
    bbox: [i16; 4],
}

/// `f32(n / d)` in double, raised to 1.0 if below it.
fn scale(n: i16, d: f64) -> f32 {
    let v = (f64::from(n) / d) as f32;
    if f64::from(v) < 1.0 {
        1.0
    } else {
        v
    }
}

/// The idiom under the oracle: truncation toward zero, negative results
/// `0xFFFFFFFF` (every value here is far inside `i32`).
fn unsigned(v: f32) -> u32 {
    let t = v.trunc() as i32;
    if t < 0 {
        u32::MAX
    } else {
        t as u32
    }
}

/// The scaled edges `X * ws`, `Y * hs`, `X2 * ws`, `Y2 * hs`.
fn edges(g: &Glyph) -> ([f32; 4], (i16, i16, i16, i16), (f32, f32)) {
    let (x, y, ox, oy) = (g.regs[0] as i16, g.regs[1] as i16, g.regs[2] as i16, g.regs[3] as i16);
    let (w, h) = (g.stack[0] as i16, g.stack[1] as i16);
    let (x, y) = (x.wrapping_sub(ox), y.wrapping_sub(oy));
    let (x2, y2) = (x.wrapping_add(w), y.wrapping_add(h));
    let (ws, hs) = (scale(g.screen.0, 320.0), scale(g.screen.1, 240.0));
    ([x as f32 * ws, y as f32 * hs, x2 as f32 * ws, y2 as f32 * hs], (x, y, x2, y2), (ws, hs))
}

/// The box and the six command words of the statement.
fn glyph(g: &Glyph) -> ([i16; 4], [u32; 6]) {
    let ([px, py, px2, py2], (x, y, x2, y2), (ws, hs)) = edges(g);
    let t = |v: f32| v as i32 as i16;
    let [mut ymin, mut ymax, mut xmin, mut xmax] = g.bbox;
    if xmax < xmin {
        (xmin, ymin, xmax, ymax) = (t(px), t(py), t(px2), t(py2));
    } else {
        if px < xmin as f32 {
            xmin = t(px);
        }
        if py < ymin as f32 {
            ymin = t(py);
        }
        if (xmax as f32) < px2 {
            xmax = t(px2);
        }
        if (ymax as f32) < py2 {
            ymax = t(py2);
        }
    }
    let fixed = |v: i16, s: f32| unsigned(v.wrapping_mul(4) as f32 * s) & 0xFFF;
    let (s, tt, tile) = (g.stack[2] as i16, g.stack[3] as i16, g.stack[4] as i16);
    let w0 = 0xE400_0000 | fixed(x2, ws) << 12 | fixed(y2, hs);
    let w1 = (tile as u32 & 7) << 24 | fixed(x, ws) << 12 | fixed(y, hs);
    let w3 = (s as i32 as u32) << 21 | ((tt as i32 as u32) << 5 & 0xFFFF);
    let w5 = unsigned(1024.0 / ws) << 16 | (unsigned(1024.0 / hs) & 0xFFFF);
    ([ymin, ymax, xmin, xmax], [w0, w1, 0xE100_0000, w3, 0xF100_0000, w5])
}

fn check(seed: u64, g: &Glyph, junk: &[u32; 8]) -> Result<(), TestCaseError> {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    {
        let mut m = s.rdram.mem();
        m.write_u32(DL_HEAD, DL);
        for (i, w) in junk.iter().enumerate() {
            m.write_u32(DL + 4 * i as u32, *w);
        }
        m.write_u16(SCREEN, g.screen.0 as u16);
        m.write_u16(SCREEN + 2, g.screen.1 as u16);
        for (i, v) in g.bbox.iter().enumerate() {
            m.write_u16(TEXT_BOX + 2 * i as u32, *v as u16);
        }
        for (i, w) in g.stack.iter().enumerate() {
            m.write_u32(SP_AT + 0x10 + 4 * i as u32, *w);
        }
    }
    s.ctx.gpr[A0..=A3].copy_from_slice(&g.regs);
    let (bbox, words) = glyph(g);
    let mut want = s.clone();
    {
        let mut m = want.rdram.mem();
        for (i, r) in g.regs.iter().enumerate() {
            m.write_u32(SP_AT + 4 * i as u32, *r as u32);
        }
        for (i, v) in bbox.iter().enumerate() {
            m.write_u16(TEXT_BOX + 2 * i as u32, *v as u16);
        }
        for (i, w) in words.iter().enumerate() {
            m.write_u32(DL + 4 * i as u32, *w);
        }
        m.write_u32(DL_HEAD, DL + 24);
    }
    let after = run("func_80011F38", render::func_80011F38, &s)?;
    same_memory(&after, &want)
}

/// Coordinates: on screen, near it, any s16, or any word.
fn coord() -> BoxedStrategy<u32> {
    prop_oneof![
        5 => (0i32..320).prop_map(|v| v as u32),
        3 => (-64i32..400).prop_map(|v| v as u32),
        1 => any::<i16>().prop_map(|v| v as i32 as u32),
        1 => any::<u32>(),
    ]
    .boxed()
}

/// A register holding a coordinate, sometimes with junk in the upper half.
fn reg() -> BoxedStrategy<u64> {
    prop_oneof![4 => coord().prop_map(sext), 1 => (coord(), any::<u32>()).prop_map(|(v, hi)| u64::from(hi) << 32 | u64::from(v))].boxed()
}

/// Screen sizes: the game's, 1:1 and just below it (the clamp at 1.0),
/// arbitrary ones, and anything (negative clamps too).
fn screen() -> BoxedStrategy<(i16, i16)> {
    prop_oneof![
        3 => (prop::sample::select(vec![320i16, 640]), prop::sample::select(vec![240i16, 480])),
        1 => prop::sample::select(vec![(319i16, 239i16), (321, 241), (0, 0), (320, 480), (640, 240)]),
        2 => (1i16..1000, 1i16..1000),
        1 => (any::<i16>(), any::<i16>()),
    ]
    .boxed()
}

fn glyph_input() -> BoxedStrategy<Glyph> {
    (prop::array::uniform4(reg()), (coord(), coord(), any::<u32>(), any::<u32>(), any::<u32>()), screen(), 0u8..6, prop::array::uniform4(any::<i16>()), prop::array::uniform4(-1i16..=1))
        .prop_map(|(regs, (w, h, s, t, tile), screen, kind, raw, nudge)| {
            let mut g = Glyph { regs, stack: [w, h, s, t, tile], screen, bbox: raw };
            // The box: arbitrary, empty, reset-like, or its edges at (or one
            // off) the truncated products, where the compares tie.
            let (p, _, _) = edges(&g);
            g.bbox = match kind {
                0 | 1 => raw,
                2 => [i16::MAX, i16::MIN, i16::MAX, i16::MIN],
                3 => [raw[0], raw[1], raw[3].saturating_add(1).max(raw[2]), raw[3].min(raw[2].saturating_sub(1))],
                _ => {
                    let at = |v: f32, k: usize| (v as i32 as i16).wrapping_add(nudge[k]);
                    [at(p[1], 0), at(p[3], 1), at(p[0], 2), at(p[2], 3)]
                }
            };
            g
        })
        .boxed()
}

/// Edge cases: the 1.0 clamp's tie (320 x 240), sizes below it, 0 and
/// negative sizes, coordinates wrapping at 16 bits and in `4 * X`, negative
/// scaled coordinates (0xFFF), products in (-1, 0), and ties with the box.
#[test]
fn func_80011F38_edges() {
    let screens = [(320i16, 240i16), (640, 480), (100, 100), (0, 0), (-320, -240), (i16::MAX, i16::MAX), (321, 239)];
    let coords: [(u32, u32); 7] = [(0, 0), (0x7FFF, 0x7FFF), (0x8000, 0xFFFF_8000), (2047, 8191), (8192, 0xFFFF_E000), (u32::MAX, 1), (0x1_0005, 0xFFFF_0003)];
    let boxes = [[0i16, 0, 0, 0], [i16::MAX, i16::MIN, i16::MAX, i16::MIN], [10, 20, 10, 20], [-5, 500, -5, 700], [0, 0, 1, 0]];
    let mut seed = 0;
    for &screen in &screens {
        for &(x, y) in &coords {
            for &(ox, oy) in &[(0u32, 0u32), (1, 1), (0x8000, 0x7FFF), (320, 240)] {
                for &bbox in &boxes {
                    seed += 1;
                    let g = Glyph { regs: [sext(x), sext(y), sext(ox), sext(oy)], stack: [8, 0xFFFF_FFF0, 0x7FFF, 0xFFFF_8001, 0xFFFF_FFFF], screen, bbox };
                    check(seed, &g, &[0xDEAD_BEEF; 8]).unwrap_or_else(|e| panic!("{g:?}: {e}"));
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn func_80011F38(seed: u64, g in glyph_input(), junk in prop::array::uniform8(any::<u32>())) {
        check(seed, &g, &junk)?;
    }
}
