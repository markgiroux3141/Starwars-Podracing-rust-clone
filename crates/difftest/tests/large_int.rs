//! The large integer functions at depth 0 (game::render): the depth-buffer
//! probes (func_8000F5A0) and the texture load (func_800125E4). Recompiled
//! C vs Rust, each checked against its statement by simulating the stores
//! on a copy of the input and comparing all of RDRAM.

// Tests are named after the functions (func_8000F5A0), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, State};
use game::recomp::{reg::*, RecompFn};
use game::render;
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

const SP_AT: u32 = 0x803F_0000;

fn state(seed: u64) -> State {
    let mut s = State::new();
    s.randomise_registers(seed);
    s.ctx.gpr[SP] = sext(SP_AT);
    s
}

fn same_memory(after: &State, want: &State) -> Result<(), TestCaseError> {
    let (a, w) = (after.rdram.as_words(), want.rdram.as_words());
    if let Some(i) = (0..a.len()).find(|&i| a[i] != w[i]) {
        return Err(TestCaseError::fail(format!("word {:#010X}: {:#010X}, model {:#010X}", 0x8000_0000 + 4 * i as u32, a[i], w[i])));
    }
    Ok(())
}

fn half(s: &State, a: u32) -> u16 {
    let w = word(s, a & !3);
    (if a & 2 == 0 { w >> 16 } else { w }) as u16
}

// ---- func_8000F5A0 ----

const SCREEN: u32 = 0x8011_4470;
const ZPTR: u32 = 0x8011_4528;
const Z: u32 = 0x8040_0000;

#[derive(Clone, Debug)]
struct Probes {
    w: i16,
    h: i16,
    entries: [(i32, i32, i32); 2],
    lists: Vec<(i32, i32)>,
    flags: Vec<u8>,
    n: i32,
}

/// The statement's stores, on a copy of `s`.
fn probes(s: &State, p: &Probes) -> State {
    let mut sim = s.clone();
    let (w, h) = (p.w as i32, p.h as i32);
    let end = Z.wrapping_add((w.wrapping_mul(h) as u32).wrapping_mul(2));
    let depth = |x: i32, y: i32| half(s, Z.wrapping_add((y.wrapping_mul(w).wrapping_add(x) as u32).wrapping_mul(2)));
    let mut m = sim.rdram.mem();
    for (k, &(e, x, y)) in p.entries.iter().enumerate() {
        if e < 0 {
            continue;
        }
        let out = 0x800D_57C0 + 4 * k as u32;
        if x < -500 {
            m.write_u32(out, 50);
            continue;
        }
        let mut count = 0u32;
        for r in 0..8 {
            for c in 0..8 {
                let edge = c < 12 - x || c >= w - 8 - x || r < 12 - y || r >= h - 8 - y;
                let a = Z.wrapping_add(((y - 4 + r).wrapping_mul(w).wrapping_add(x - 4 + c) as u32).wrapping_mul(2));
                if edge || a < Z || (a < end && half(s, a) < 0xFFDC) {
                    count += 1;
                }
            }
        }
        m.write_u32(out, count);
    }
    let sample = |m: &mut n64mem::Mem, out: u32, x: i32, y: i32| {
        m.write_u32(out, (-1000i32) as u32);
        if x >= 0 {
            m.write_u32(out, u32::from(depth(x, y)));
        }
    };
    for i in 0..20u32 {
        let (x, y) = p.lists[i as usize];
        sample(&mut m, 0x800D_5FD0 + 4 * i, x, y);
        let (x, y) = p.lists[20 + i as usize];
        sample(&mut m, 0x800D_6020 + 4 * i, x, y);
    }
    for j in 0..40u32 {
        if p.flags[j as usize] != 0 {
            let (x, y) = p.lists[40 + j as usize];
            sample(&mut m, 0x800D_60A0 + 4 * j, x, y);
        }
    }
    for j in 0..p.n.max(0) as u32 {
        let (x, y) = p.lists[80 + j as usize];
        sample(&mut m, 0x800D_6070 + 4 * j, x, y);
    }
    drop(m);
    sim
}

fn put_probes(s: &mut State, p: &Probes) {
    let mut m = s.rdram.mem();
    m.write_u16(SCREEN, p.w as u16);
    m.write_u16(SCREEN + 2, p.h as u16);
    m.write_u32(ZPTR, Z);
    for (k, &(e, x, y)) in p.entries.iter().enumerate() {
        m.write_u32(0x8009_B814 + 4 * k as u32, e as u32);
        m.write_u32(0x800D_57A0 + 4 * k as u32, x as u32);
        m.write_u32(0x800D_57A8 + 4 * k as u32, y as u32);
    }
    let lists = [(0x800D_5AF8u32, 0x800D_5B48u32, 0usize, 20usize), (0x800D_5B98, 0x800D_5BE8, 20, 20), (0x800D_5E40, 0x800D_5EE0, 40, 40), (0x800D_5958, 0x800D_5988, 80, 12)];
    for (xs, ys, from, len) in lists {
        for i in 0..len {
            let (x, y) = p.lists[from + i];
            m.write_u32(xs + 4 * i as u32, x as u32);
            m.write_u32(ys + 4 * i as u32, y as u32);
        }
    }
    for (j, f) in p.flags.iter().enumerate() {
        m.write_u8(0x800D_5C38 + j as u32, *f);
    }
    m.write_u32(0x8009_B86C, p.n as u32);
}

fn probe_strategy() -> BoxedStrategy<Probes> {
    (8i16..48, 8i16..40)
        .prop_flat_map(|(w, h)| {
            let wi = w as i32;
            let hi = h as i32;
            let coord = move |n: i32| prop_oneof![3 => -2i32..n + 2, 1 => Just(-1i32), 1 => -600i32..-400, 1 => prop::sample::select(vec![-500i32, -501])];
            let entry = (prop_oneof![3 => Just(0i32), 1 => Just(-1i32), 1 => any::<i32>()], coord(wi), -2i32..hi + 2);
            (
                Just((w, h)),
                prop::array::uniform2(entry),
                prop::collection::vec((prop_oneof![3 => 0i32..wi, 1 => Just(-1i32), 1 => i32::MIN..0], 0i32..hi), 92),
                prop::collection::vec(prop_oneof![Just(0u8), any::<u8>()], 40),
                prop_oneof![Just(0i32), 1i32..12, Just(-3i32)],
            )
        })
        .prop_map(|((w, h), entries, lists, flags, n)| Probes { w, h, entries, lists, flags, n })
        .boxed()
}

// ---- func_800125E4 ----

const DL_HEAD: u32 = 0x8012_17B0;
const DL: u32 = 0x8030_6000;
const TEX: u32 = 0x8030_2000;

fn texture_load(format: u32, img: u32) -> Vec<u32> {
    let head = |fd: u32, f5: u32, f3: u32, f5b: u32| vec![fd, img, f5, 0x0708_0200, 0xE600_0000, 0, 0xF300_0000, f3, f5b, 0x0008_0200];
    match format {
        1 | 3 => [head(0xFD90_0000, 0xF590_0000, 0x077F_F200, 0xF580_0800), vec![0xF200_0000, 0x000F_C1FC, 0xE200_001C, 0x0040_4240]].concat(),
        2 => [head(0xFD50_0000, 0xF550_0000, 0x073F_F200, 0xF540_0800), vec![0xF200_0000, 0x000F_C0FC, 0xE200_001C, 0x0040_4240]].concat(),
        0 => {
            let mut w = head(0xFD50_0000, 0xF550_0000, 0x073F_F200, 0xF540_0800);
            for t in [0x0118_0200u32, 0x0228_0200, 0x0338_0200] {
                w.extend([0xF540_0800, t]);
            }
            for t in [0x000F_C0FCu32, 0x010F_C0FC, 0x020F_C0FC, 0x030F_C0FC] {
                w.extend([0xF200_0000, t]);
            }
            w.extend([0xE200_001C, 0x0040_4240]);
            w
        }
        _ => vec![],
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    /// Depths mostly far (so the edge rules decide), with values at the
    /// threshold and near ones.
    fn func_8000F5A0(seed: u64, p in probe_strategy(), regs: [u32; 9],
                     depths in prop::collection::vec(prop_oneof![6 => Just(0xFFFFu16), 1 => prop::sample::select(vec![0xFFDBu16, 0xFFDC, 0xFFDD]), 1 => any::<u16>()], 2048)) {
        let mut s = state(seed);
        s.randomise_memory(seed, Z - 0x100, 2 * 48 * 40 + 0x200);
        {
            let mut m = s.rdram.mem();
            for (k, d) in depths.iter().enumerate() {
                m.write_u16(Z + 2 * k as u32, *d);
            }
        }
        put_probes(&mut s, &p);
        for (r, v) in [S0, S1, S2, S3, S4, S5, S6, S7, FP].iter().zip(regs) {
            s.ctx.gpr[*r] = sext(v);
        }
        let mut want = probes(&s, &p);
        {
            let frame = SP_AT - 0x68;
            let mut m = want.rdram.mem();
            for (off, r) in [(4u32, S0), (8, S1), (0xC, S2), (0x10, S3), (0x14, S4), (0x18, S5), (0x1C, S6), (0x20, S7), (0x24, FP)] {
                m.write_u32(frame + off, s.ctx.gpr[r] as u32);
            }
            m.write_u32(frame + 0x2C, p.h as i32 as u32);
        }
        let after = run("func_8000F5A0", render::func_8000F5A0, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_800125E4(seed: u64, format in prop_oneof![0u32..4, any::<u32>()], count in prop_oneof![0i32..6, any::<i32>()],
                     i in prop_oneof![0i32..6, -2i32..0], imgs: [u32; 6], s0: u32) {
        let mut s = state(seed);
        s.ctx.gpr[S0] = sext(s0);
        {
            let mut m = s.rdram.mem();
            m.write_u32(DL_HEAD, DL);
            m.write_u32(TEX, format);
            m.write_u32(TEX + 4, count as u32);
            for (k, v) in imgs.iter().enumerate() {
                m.write_u32(TEX + 8 + 4 * k as u32, *v);
            }
            // Images before the table, for negative indices.
            m.write_u32(TEX + 4, count as u32);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(TEX), sext(i as u32));
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            m.write_u32(SP_AT - 4, s0);
            m.write_u32(SP_AT + 4, i as u32);
            if i < count {
                let img = word(&s, TEX.wrapping_add(8).wrapping_add((4 * i) as u32));
                let words = texture_load(format, img);
                for (k, w) in words.iter().enumerate() {
                    m.write_u32(DL + 4 * k as u32, *w);
                }
                m.write_u32(DL_HEAD, DL + 4 * words.len() as u32);
            }
        }
        let after = run("func_800125E4", render::func_800125E4, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_8003D110 / func_8003B860 ----

const DLP: u32 = 0x8030_3000;

fn quad_words(corners: [(u32, u32); 4], st: [u32; 4]) -> Vec<u32> {
    let mut w = vec![0x0100_4008, 0x800A_4920];
    for k in 0..4 {
        w.extend([0x021C_0000 | 2 * k, 0]);
    }
    for (k, (x, y)) in corners.iter().enumerate() {
        w.extend([0x0218_0000 | 2 * k as u32, x << 16 | (y & 0xFFFF)]);
    }
    let [sa, sb, ta, tb] = st;
    for (k, (s, t)) in [(sa, ta), (sb, ta), (sb, tb), (sa, tb)].iter().enumerate() {
        w.extend([0x0214_0000 | 2 * k as u32, s << 16 | (t & 0xFFFF)]);
    }
    w.extend([0x0600_0402, 0x0000_0604]);
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_8003D110(seed: u64, settings in prop_oneof![Just(0x800u32), Just(0u32), any::<u32>()], h: u16) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(DL_HEAD, DL);
            m.write_u32(0x800D_697C, settings);
            m.write_u16(0x8011_447C, h);
        }
        let mut words = vec![
            0xE700_0000, 0, 0xE300_0A01, 0, 0xD700_0002, 0x8000_8000, 0xE200_1E01, 0, 0xE300_0C00, 0, 0xE300_1201, 0,
            0xE300_1402, 0xC00, 0xE300_0D01, 0, 0xE300_0F00, 0, 0xE300_1001, 0, 0xE200_001C, 0x0F0A_7008,
            0xDC38_000E, 0x800A_3C80, 0xDB0C_0000, 0x0001_0000,
        ];
        words.extend([0xE300_1801, if settings & 0x800 != 0 { 0xC0 } else { h as i16 as i32 as u32 }]);
        words.extend([0xE300_1A01, 0x30]);
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            for (a, v) in [(0x800A_4960u32, 0u32), (0x8011_4548, 0), (0x8011_454C, 0), (0x8011_4540, 320), (0x8011_4544, 240), (0x800A_48D0, 0)] {
                m.write_u32(a, v);
            }
            for (k, w) in words.iter().enumerate() {
                m.write_u32(DL + 4 * k as u32, *w);
            }
            m.write_u32(DL_HEAD, DL + 4 * words.len() as u32);
        }
        let after = run("func_8003D110", render::func_8003D110, &s)?;
        same_memory(&after, &want)?;
    }

    /// The moved corner by the point's quadrant; ties on either side.
    #[test]
    fn func_8003B860(seed: u64, x in prop::array::uniform2(prop_oneof![0i32..640, any::<i32>()]), y in prop::array::uniform2(prop_oneof![0i32..480, any::<i32>()]),
                     p in prop::array::uniform2(prop_oneof![0i32..640, any::<i32>()]), same: [bool; 2], st: [u32; 4]) {
        let (x0, x1, y0, y1) = (x[0], x[1], y[0], y[1]);
        let px = if same[0] { x0 } else { p[0] };
        let py = if same[1] { y0 } else { p[1] };
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(DLP, DL);
            for (k, v) in [y1 as u32, px as u32, py as u32, st[0], st[1], st[2], st[3]].iter().enumerate() {
                m.write_u32(SP_AT + 0x10 + 4 * k as u32, *v);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(DLP), sext(x0 as u32), sext(x1 as u32), sext(y0 as u32));
        let mut c = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        let k = match (px < x0, py < y0) {
            (true, true) => 0,
            (false, true) => 1,
            (false, false) => 2,
            (true, false) => 3,
        };
        c[k] = (px, py);
        let words = quad_words(c.map(|(a, b)| (a as u32, b as u32)), [st[0], st[1], st[2], st[3]]);
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            for (k, w) in words.iter().enumerate() {
                m.write_u32(DL + 4 * k as u32, *w);
            }
            m.write_u32(DLP, DL + 4 * words.len() as u32);
            m.write_u32(SP_AT, DLP);
        }
        let after = run("func_8003B860", render::func_8003B860, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_8003594C / func_8003609C ----

const MAT: u32 = 0x8030_7000;
const QQ: u32 = 0x8030_7100;
const MODE_DL: u32 = 0x8011_2C90;

#[derive(Clone, Debug)]
struct Material {
    word0: u32,
    kind: i16,
    blocks: [u8; 16],
    w18: u32,
    w1c: u32,
    tail: [u8; 4],
}

fn selector() -> BoxedStrategy<u8> {
    prop_oneof![3 => prop::sample::select(vec![1u8, 2, 8, 9]), 2 => prop::sample::select(vec![0u8, 3, 4, 6]), 1 => any::<u8>()].boxed()
}

fn material() -> BoxedStrategy<Material> {
    (any::<u32>(), prop_oneof![Just(2i16), any::<i16>()], prop::array::uniform16(selector()),
     prop_oneof![any::<u32>().prop_map(|w| 0xC800_0000 | (w & 0xFFFF)), any::<u32>()], any::<u32>(), any::<[u8; 4]>())
        .prop_map(|(word0, kind, blocks, w18, w1c, tail)| Material { word0, kind, blocks, w18, w1c, tail })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_8003594C(seed: u64, mat in material(), gate in prop_oneof![Just(0u32), any::<u32>()],
                     settings in prop_oneof![Just(0x50u32), Just(0x40u32), Just(0x10u32), Just(0u32), any::<u32>()],
                     q_null: bool, q8 in prop_oneof![Just(0u32), any::<u32>()],
                     clear in prop_oneof![Just(0u32), any::<u32>()], replace in prop_oneof![Just(0u32), any::<u32>()],
                     w in prop_oneof![Just(0xF550_4040u32), any::<u32>()], w2: u32) {
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(MAT, mat.word0);
            m.write_u16(MAT + 4, mat.kind as u16);
            for (k, b) in mat.blocks.iter().enumerate() {
                m.write_u8(MAT + 6 + k as u32, *b);
            }
            m.write_u32(MAT + 0x18, mat.w18);
            m.write_u32(MAT + 0x1C, mat.w1c);
            for (k, b) in mat.tail.iter().enumerate() {
                m.write_u8(MAT + 0x2A + k as u32, *b);
            }
            m.write_u32(QQ + 8, q8);
            m.write_u32(0x800A_4740, gate);
            m.write_u32(0x800D_697C, settings);
            m.write_u32(0x800A_3DA8, clear);
            m.write_u32(0x800A_3D9C, replace);
            m.write_u32(0x800A_3DA0, w);
            m.write_u32(0x800A_3DA4, w2);
        }
        let q = if q_null { 0 } else { QQ };
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(MAT), sext(q));
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            let (mut w18, mut w1c) = (mat.w18, mat.w1c);
            if gate == 0 {
                w18 &= !0x30;
                w1c &= !0x30;
            }
            if settings & 0x40 != 0 && mat.kind == 2 && w18 & 0xFFFF_0000 == 0xC800_0000 {
                w18 = (w18 & 0xFFFF) | 0x0C08_0000;
            }
            if !(settings & 0x10 == 0 && q != 0 && q8 != 0) {
                m.write_u32(SP_AT - 0x4C, MAT + 6);
                m.write_u32(SP_AT - 0x48, MAT + 0xE);
                for blk in [MAT + 6, MAT + 0xE] {
                    for k in 0..8u32 {
                        let v = m.read_u8(blk + k);
                        let wide = k < 4;
                        if v == 1 || v == 2 || (wide && (v == 8 || v == 9)) {
                            m.write_u8(blk + k, if [1, 2, 6].contains(&k) { 4 } else { 6 });
                        }
                    }
                }
            }
            if clear != 0 {
                w18 &= !0x10;
                w1c &= !0x10;
            }
            if replace != 0 {
                w18 = w;
                w1c = w2;
                if w == 0xF550_4040 {
                    for (k, b) in [0x80u8, 0, 0xFF, 0x28].iter().enumerate() {
                        m.write_u8(MAT + 0x2A + k as u32, *b);
                    }
                    m.write_u32(MAT, mat.word0 | 4);
                }
            }
            m.write_u32(MAT + 0x18, w18);
            m.write_u32(MAT + 0x1C, w1c);
        }
        let after = run("func_8003594C", render::func_8003594C, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8003609C(seed: u64, mode in prop_oneof![0u32..0x100, any::<u32>()], cur in prop_oneof![0u32..0x100, any::<u32>()], same: bool) {
        let cur = if same { mode } else { cur };
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(MODE_DL, DL);
            m.write_u32(QQ, cur);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(mode), sext(QQ));
        let mut words = Vec::new();
        let mut c = cur;
        if mode != cur {
            let fields: [(u32, [(u32, u32); 4]); 4] = [
                (0x3, [(0, 0), (1, 0x8000), (2, 0xC000), (3, u32::MAX)]),
                (0xC, [(0, 0), (4, 0x2_0000), (8, 0x4_0000), (0xC, u32::MAX)]),
                (0x30, [(0, 0x2000), (0x10, 0), (0x20, 0x3000), (0x30, u32::MAX)]),
                (0xC0, [(0, 0), (0x40, 0x1_0000), (0x80, u32::MAX), (0xC0, u32::MAX)]),
            ];
            let cmds = [0xE300_1001u32, 0xE300_0D01, 0xE300_1201, 0xE300_0F00];
            for ((mask, vals), cmd) in fields.iter().zip(cmds) {
                if mode & mask != c & mask {
                    c = (c & !mask) | (mode & mask);
                    let arg = vals.iter().find(|(v, _)| *v == mode & mask).unwrap().1;
                    if arg != u32::MAX {
                        words.extend([cmd, arg]);
                    }
                }
            }
        }
        let mut want = s.clone();
        {
            let mut m = want.rdram.mem();
            m.write_u32(QQ, c);
            for (k, w) in words.iter().enumerate() {
                m.write_u32(DL + 4 * k as u32, *w);
            }
            m.write_u32(MODE_DL, DL + 4 * words.len() as u32);
        }
        let after = run("func_8003609C", render::func_8003609C, &s)?;
        same_memory(&after, &want)?;
    }
}
