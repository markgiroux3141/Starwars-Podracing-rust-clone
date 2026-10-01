//! Depth 1 at 0x80014568 (game::render): a glyph with an outline or a
//! shadow, drawn as offset copies through `func_80011F38` around the glyph
//! itself. Recompiled C vs Rust with the callee as C. The model replays the
//! callee's C on a copy of the state in the same order with the same
//! arguments, adding the function's own display-list appends, flag and
//! frame spills; whole RDRAM is compared.

// Tests are named after the functions (func_80014568), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::imports;
use game::recomp::{reg::*, RecompFn};
use game::render::{self, DL_HEAD, TEXT_BOX};
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

/// The s16 value of a word's low half, as a sign-extended word.
fn s16(v: u32) -> u32 {
    v as u16 as i16 as i32 as u32
}

const SP_AT: u32 = 0x803F_0000;
const SCREEN: u32 = 0x8011_4470;
const DL: u32 = 0x8030_4000;
/// `ox`, `oy`, `w`, `h`, `s`, `t`, `tile`: the glyph's words.
const GLYPH: u32 = 0x800D_691C;
const FLAG: u32 = 0x800D_6938;
const COLOUR: u32 = 0x800A_1CCC;

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
}

fn wr(s: &mut State, a: u32, v: u32) {
    s.rdram.mem().write_u32(a, v);
}

fn saves(w: &mut State, s: &State, frame: u32, regs: &[(u32, usize)]) {
    for &(off, r) in regs {
        wr(w, SP_AT - frame + off, s.ctx.gpr[r] as u32);
    }
}

/// Two words at the display-list head: the head first, advanced by 8.
fn append(w: &mut State, a: u32, b: u32) {
    let head = word(w, DL_HEAD);
    wr(w, DL_HEAD, head.wrapping_add(8));
    wr(w, head, a);
    wr(w, head.wrapping_add(4), b);
}

/// [`render::func_80011F38`]'s C at `(x, y)` (s16 words) with the glyph's
/// current words as arguments.
fn glyph_at(w: &mut State, x: u32, y: u32) {
    let sp = SP_AT - 0x90;
    for k in 0..5 {
        let v = word(w, GLYPH + 8 + 4 * k);
        wr(w, sp + 0x10 + 4 * k, v);
    }
    let (ox, oy) = (s16(word(w, GLYPH)), s16(word(w, GLYPH + 4)));
    w.ctx.gpr[SP] = sext(sp);
    (w.ctx.gpr[A0], w.ctx.gpr[A1], w.ctx.gpr[A2], w.ctx.gpr[A3]) = (sext(x), sext(y), sext(ox), sext(oy));
    w.run(imports::func_80011F38);
}

fn outline_model(s: &State) -> State {
    let mut w = s.clone();
    let sp = SP_AT - 0x90;
    saves(&mut w, s, 0x90, &[(0x34, S1), (0x38, S2), (0x98, A2), (0x54, RA), (0x50, FP), (0x4C, S7), (0x48, S6), (0x44, S5), (0x40, S4), (0x3C, S3), (0x30, S0)]);
    let (x, y) = (s.ctx.gpr[A0] as u32, s.ctx.gpr[A1] as u32);
    let half = |v: u32| ((v as i32) / 2) as u32;
    let bx = x.wrapping_sub(half(word(&w, GLYPH + 8)));
    let by = half(word(&w, GLYPH + 0xC)).wrapping_add(y);
    wr(&mut w, sp + 0x8C, bx);
    wr(&mut w, sp + 0x88, by);
    let at = |d: i32, v: u32| s16(v.wrapping_add(d as u32));
    let offsets: &[(i32, i32)] = match s.ctx.gpr[A2] as u8 {
        b'o' => {
            for (off, v) in [(0x68, at(-1, bx)), (0x64, at(-1, by)), (0x5C, at(1, by)), (0x60, at(1, bx)), (0x58, s16(by)), (0x68, s16(bx))] {
                wr(&mut w, sp + off, v);
            }
            &[(-1, -1), (1, 1), (1, -1), (-1, 1), (-1, 0), (1, 0), (0, -1), (0, 1)]
        }
        b's' => &[(1, 1)],
        b'f' => {
            wr(&mut w, sp + 0x58, s16(by));
            wr(&mut w, sp + 0x68, s16(bx));
            &[(-1, 0), (1, 0), (0, -1), (0, 1)]
        }
        _ => &[],
    };
    if !offsets.is_empty() {
        let alpha = word(&w, COLOUR) & 0xFF;
        append(&mut w, 0xFA00_0000, alpha);
        for &(dx, dy) in offsets {
            glyph_at(&mut w, at(dx, bx), at(dy, by));
        }
        let colour = word(&w, COLOUR);
        append(&mut w, 0xFA00_0000, colour);
        wr(&mut w, FLAG, 1);
    }
    if word(&w, FLAG) == 0 {
        let colour = word(&w, COLOUR);
        append(&mut w, 0xFA00_0000, colour);
        wr(&mut w, FLAG, 1);
    }
    glyph_at(&mut w, s16(bx), s16(by));
    w
}

fn coord() -> BoxedStrategy<u32> {
    prop_oneof![
        5 => (0i32..320).prop_map(|v| v as u32),
        3 => (-64i32..400).prop_map(|v| v as u32),
        1 => prop::sample::select(vec![0x7FFFu32, 0x8000, 0xFFFF, 0x1_0000, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF]),
        1 => any::<i16>().prop_map(|v| v as i32 as u32),
        1 => any::<u32>(),
    ]
    .boxed()
}

fn size() -> BoxedStrategy<u32> {
    prop_oneof![4 => 0u32..32, 1 => (-33i32..0).prop_map(|v| v as u32), 1 => coord()].boxed()
}

fn screen() -> BoxedStrategy<(i16, i16)> {
    prop_oneof![
        3 => (prop::sample::select(vec![320i16, 640]), prop::sample::select(vec![240i16, 480])),
        2 => (1i16..1000, 1i16..1000),
        1 => (any::<i16>(), any::<i16>()),
    ]
    .boxed()
}

fn mode() -> BoxedStrategy<u64> {
    prop_oneof![
        3 => prop::sample::select(vec![b'o', b's', b'f']).prop_map(u64::from),
        1 => (prop::sample::select(vec![b'o', b's', b'f', b'n', 0xEF, 0xF3, 0xE6]), any::<u32>()).prop_map(|(c, hi)| sext(hi << 8 | u32::from(c))),
        1 => any::<u8>().prop_map(u64::from),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Every mode (and others; high bits set), the colour already sent or
    /// not, odd and negative glyph sizes, coordinates wrapping at 16 bits,
    /// any screen and box.
    #[test]
    fn func_80014568(seed: u64, x in coord(), y in coord(), mode in mode(), offs in prop::array::uniform2(any::<u32>()), wh in prop::array::uniform2(size()),
                     st in prop::array::uniform3(any::<u32>()), flag in prop_oneof![Just(0u32), Just(1u32), any::<u32>()], colour: u32,
                     screen in screen(), bbox in prop::array::uniform4(any::<i16>()), empty: bool) {
        let mut s = State::new();
        s.randomise_registers(seed);
        s.randomise_memory(seed ^ 0x5EED, DL, 0x400);
        s.ctx.gpr[SP] = sext(SP_AT);
        {
            let mut m = s.rdram.mem();
            m.write_u32(DL_HEAD, DL);
            m.write_u16(SCREEN, screen.0 as u16);
            m.write_u16(SCREEN + 2, screen.1 as u16);
            let bbox = if empty { [i16::MAX, i16::MIN, i16::MAX, i16::MIN] } else { bbox };
            for (i, v) in bbox.iter().enumerate() {
                m.write_u16(TEXT_BOX + 2 * i as u32, *v as u16);
            }
            for (k, v) in offs.iter().chain(&wh).chain(&st).enumerate() {
                m.write_u32(GLYPH + 4 * k as u32, *v);
            }
            m.write_u32(FLAG, flag);
            m.write_u32(COLOUR, colour);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (sext(x), sext(y), mode);
        let want = outline_model(&s);
        let after = run("func_80014568", render::func_80014568, &s)?;
        same_memory(&after, &want)?;
    }
}
