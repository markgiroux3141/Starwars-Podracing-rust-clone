//! Depth 1 at 0x800086F8..0x8000B254 (game::misc): the sound request and
//! its slot clear, mode and parameter setters, the record and entry resets,
//! the entry selection and a node's transform as a 4x4 matrix. Recompiled
//! C vs Rust with the callees as C, each checked against its statement by
//! simulating the stores (frames included) on a copy of the input and
//! comparing all of RDRAM, the callees' effects taken from their
//! statements.

// Tests are named after the functions (func_800086F8), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::misc::{self, ENTRIES, RECORDS};
use game::pools::POOLS;
use game::recomp::{fpu, reg::*, RecompFn};
use proptest::prelude::*;

fn run(name: &str, port: RecompFn, s: &State) -> Result<State, TestCaseError> {
    compare(name, port, s).map_err(|d| TestCaseError::fail(d.to_string()))
}

fn word(s: &State, a: u32) -> u32 {
    s.rdram.as_words()[((a - 0x8000_0000) / 4) as usize]
}

fn half(s: &State, a: u32) -> u16 {
    let w = word(s, a & !3);
    (if a & 2 == 0 { w >> 16 } else { w }) as u16
}

fn byte(s: &State, a: u32) -> u8 {
    (word(s, a & !3) >> (8 * (3 - (a & 3)))) as u8
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

fn rom_word(vaddr: u32) -> u32 {
    let o = (vaddr - 0x8000_0400 + 0x1000) as usize;
    u32::from_be_bytes(baserom()[o..o + 4].try_into().unwrap())
}

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

fn wh(s: &mut State, a: u32, v: u16) {
    s.rdram.mem().write_u16(a, v);
}

fn wb(s: &mut State, a: u32, v: u8) {
    s.rdram.mem().write_u8(a, v);
}

/// The caller-visible frame words: `(offset from the new sp, register)`.
fn saves(want: &mut State, s: &State, frame: u32, regs: &[(u32, usize)]) {
    for &(off, r) in regs {
        wr(want, SP_AT - frame + off, s.ctx.gpr[r] as u32);
    }
}

fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}
fn mul(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? * ok(b)?)
}
fn sub(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? - ok(b)?)
}
fn div(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? / ok(b)?)
}

fn float() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        -1.0e4f32..1.0e4f32,
        -1.0f32..1.0,
        Just(1.0e19f32),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

// ---- The sound slots (func_80007A44, func_80008760) ----

const SLOTS: u32 = 0x800D_2038;
const RUNNING: u32 = 0x8009_A2B8;

/// func_80007A44: word +0x18 of the eight slots zeroed if the flag is set.
fn slot_clear(want: &mut State, flag: u32) {
    if flag != 0 {
        for k in 0..8 {
            wr(want, SLOTS + 0x20 * k + 0x18, 0);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn func_800086F8(seed: u64, flag in prop_oneof![Just(0u32), any::<u32>()], junk in prop::collection::vec(any::<u32>(), 64)) {
        let mut s = state(seed);
        wr(&mut s, RUNNING, flag);
        for (i, w) in junk.iter().enumerate() {
            wr(&mut s, SLOTS + 4 * i as u32, *w);
        }
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA)]);
        slot_clear(&mut want, flag);
        let after = run("func_800086F8", misc::func_800086F8, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80009F6C(seed: u64, flag in prop_oneof![Just(0u32), any::<u32>()], junk in prop::collection::vec(any::<u32>(), 64)) {
        let mut s = state(seed);
        wr(&mut s, RUNNING, flag);
        for (i, w) in junk.iter().enumerate() {
            wr(&mut s, SLOTS + 4 * i as u32, *w);
        }
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA)]);
        slot_clear(&mut want, flag);
        wr(&mut want, 0x8009_AEF0, u32::MAX);
        wr(&mut want, 0x8009_ADE8, u32::MAX);
        let after = run("func_80009F6C", misc::func_80009F6C, &s)?;
        same_memory(&after, &want)?;
    }
}

const FADE: u32 = 0x8009_B77F;
const VOLS: u32 = 0x8011_3685;
const STEREO: u32 = 0x8011_3688;
const FRAME: u32 = 0x8012_0BE8;
const K_AT: u32 = 0x800A_81E0;

#[derive(Clone, Debug)]
struct Sound {
    id: u64,
    a1: u64,
    pitch: f32,
    vol: f32,
    pan: u32,
    keep: u32,
    fade: u8,
    vols: [u8; 2],
    stereo: u32,
    frame: u32,
    k: f32,
    /// word 0, id, +8, +0xC, priority, pitch, volume, pan word per slot.
    slots: [[u32; 8]; 8],
}

/// func_80008718.
fn special(c: u64) -> bool {
    let c = c as i64;
    (0x8E..0x9E).contains(&c) || c == 0x22
}

/// The statement's stores on a copy of `s`, or None outside the domain.
fn sound(s: &State, x: &Sound) -> Option<State> {
    let mut w = s.clone();
    saves(&mut w, s, 0x38, &[(0x14, S0), (0x18, S1), (0x1C, RA), (0x3C, A1)]);
    let f = if x.keep == 0 { 1.0 } else { sub(1.0, div(f32::from(x.fade), 255.0)?)? };
    let sp = special(x.id);
    let sv = f32::from(if sp { x.vols[1] } else { x.vols[0] });
    let v0 = mul(x.vol, div(sv, 255.0)?)?;
    wr(&mut w, SP_AT - 0x38 + 0x24, f.to_bits());
    wr(&mut w, SP_AT + 8, x.pitch.to_bits());
    wr(&mut w, SP_AT + 0xC, v0.to_bits());
    let v = mul(v0, f)?;
    if v == 0.0 {
        return Some(w);
    }
    let prio = if sp { 8 } else { x.a1 as u16 as i16 as i32 };
    let pitch2 = mul(x.pitch, 2.0)?;
    let vk = mul(v, x.k)?;
    let pan = if x.stereo & 1 == 0 {
        wh(&mut w, SP_AT + 0x12, 0x40);
        0x40
    } else {
        x.pan as u16
    };
    if (x.id as i64) < 0 {
        return Some(w);
    }
    let slot = |k: usize| SLOTS + 0x20 * k as u32;
    let fill = |w: &mut State, k: usize, fresh: bool| {
        let r = slot(k);
        if fresh {
            wr(w, r + 4, x.id as u32);
            wr(w, r, (-2i32) as u32);
            wr(w, r + 8, x.keep);
        }
        wr(w, r + 0x10, prio as u32);
        wr(w, r + 0x14, pitch2.to_bits());
        wh(w, r + 0x1C, pan);
        wr(w, r + 0xC, x.frame);
        wr(w, r + 0x18, fpu::trunc_w_s(vk));
    };
    if x.keep != 0 {
        if let Some(k) = (0..8).find(|&k| x.slots[k][2] != 0 && sext(x.slots[k][1]) == x.id) {
            let louder = !((x.slots[k][6] as i32) as f32 <= vk);
            if !(louder && x.frame == x.slots[k][3]) {
                fill(&mut w, k, false);
            }
            return Some(w);
        }
    }
    let k = (0..8).find(|&k| x.slots[k][0] == u32::MAX).or_else(|| {
        let (mut best, mut at) = (prio, None);
        for k in 0..8 {
            if (x.slots[k][4] as i32) < best {
                (best, at) = (x.slots[k][4] as i32, Some(k));
            }
        }
        at
    });
    if let Some(k) = k {
        fill(&mut w, k, true);
    }
    Some(w)
}

fn sound_case() -> BoxedStrategy<Sound> {
    let id = prop_oneof![3 => (0u32..0x40).prop_map(u64::from), 2 => (0x8Cu32..0xA0).prop_map(u64::from), 1 => Just(0x22u64), 1 => any::<i32>().prop_map(|v| v as i64 as u64)];
    let slot = (prop_oneof![1 => Just(u32::MAX), 1 => Just((-2i32) as u32), 1 => any::<u32>()], 0u32..0x40, prop_oneof![Just(0u32), Just(1u32)], 0u32..3, -2i32..12, any::<u32>(), 0i32..300, any::<u32>())
        .prop_map(|(h, id, keep, frame, prio, pitch, vol, pan)| [h, id, keep, frame, prio as u32, pitch, vol as u32, pan]);
    let vol = prop_oneof![3 => 0.0f32..2.0, 1 => Just(0.0f32), 1 => float()];
    (id, any::<u64>(), float(), vol, any::<u32>(), prop_oneof![Just(0u32), Just(1u32), any::<u32>()], any::<u8>(), any::<[u8; 2]>(), any::<u32>(), 0u32..3,
     prop_oneof![3 => Just(None), 1 => float().prop_map(Some)], prop::array::uniform8(slot))
        .prop_map(|(id, a1, pitch, vol, pan, keep, fade, vols, stereo, frame, k, slots)| Sound {
            id,
            a1: if a1 & 1 == 0 { sext(a1 as u32 & 0xF) } else { a1 },
            pitch,
            vol,
            pan,
            keep,
            fade,
            vols,
            stereo,
            frame,
            k: k.unwrap_or(f32::from_bits(rom_word(K_AT))),
            slots,
        })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn func_80008760(seed: u64, x in sound_case(), kept in 0u8..3, level in 1u8..40) {
        let mut x = x;
        if kept == 1 {
            // A kept slot playing the id, set this frame, whose volume is
            // trunc(v') or one louder.
            let sv = f32::from(if special(x.id) { x.vols[1] } else { x.vols[0] });
            let f = if x.keep == 0 { 1.0 } else { 1.0 - f32::from(x.fade) / 255.0 };
            let vk = x.vol * (sv / 255.0) * f * x.k;
            if vk.is_finite() && vk.abs() < 1.0e6 {
                x.slots[0] = [0, x.id as u32, 1, x.frame, 5, 0, (vk as i32).wrapping_add(i32::from(seed & 1 == 0)) as u32, 0];
            }
        } else if kept == 2 {
            // An exact tie, f32(old) == v': f = 1, S / 255 = 1, K = 1 and an
            // integer volume.
            (x.keep, x.fade, x.vols, x.k, x.vol) = (1, 0, [255, 255], 1.0, f32::from(level));
            x.slots[0] = [0, x.id as u32, 1, x.frame, 5, 0, u32::from(level), 0];
        }
        let mut s = state(seed);
        for (k, sl) in x.slots.iter().enumerate() {
            for (j, v) in sl.iter().enumerate() {
                wr(&mut s, SLOTS + 0x20 * k as u32 + 4 * j as u32, *v);
            }
        }
        wb(&mut s, FADE, x.fade);
        wb(&mut s, VOLS, x.vols[0]);
        wb(&mut s, VOLS + 1, x.vols[1]);
        wr(&mut s, STEREO, x.stereo);
        wr(&mut s, FRAME, x.frame);
        wr(&mut s, K_AT, x.k.to_bits());
        wr(&mut s, SP_AT + 0x10, x.pan);
        wr(&mut s, SP_AT + 0x14, x.keep);
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (x.id, x.a1, sext(x.pitch.to_bits()), sext(x.vol.to_bits()));
        let want = sound(&s, &x);
        prop_assume!(want.is_some());
        let after = run("func_80008760", misc::func_80008760, &s)?;
        same_memory(&after, &want.unwrap())?;
        prop_assert_eq!(byte(&after, FADE), x.fade);
    }
}

// ---- func_80009744 / func_80009B8C: modes and parameters ----

const MODE: u32 = 0x8009_AEE0;
const TEST: u32 = 0x5465_7374;
const REGISTRY: u32 = 0x8030_5000;
const DESCS: u32 = 0x8030_5100;
const WORDS: u32 = 0x8011_B1BC;

/// The pool registry with `pools` as (id, count), and the first "Test"
/// pool's count as func_8003F7B8 returns it.
fn put_pools(s: &mut State, pools: &[(u32, u32)]) -> i32 {
    wr(s, POOLS, REGISTRY);
    for (i, &(id, n)) in pools.iter().enumerate() {
        let d = DESCS + 0x40 * i as u32;
        wr(s, REGISTRY + 4 * i as u32, d);
        wr(s, d, id);
        wr(s, d + 8, n);
    }
    wr(s, REGISTRY + 4 * pools.len() as u32, 0);
    pools.iter().find(|p| p.0 == TEST).map_or(0, |p| p.1 as i32)
}

fn pools() -> BoxedStrategy<Vec<(u32, u32)>> {
    prop::collection::vec((prop_oneof![Just(TEST), Just(0x506F_6473u32), any::<u32>()], prop_oneof![Just(0u32), 1u32..5, any::<u32>()]), 0..4).boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80009744(seed: u64, mode in prop_oneof![4 => 0u64..5, 1 => any::<u64>()], k: u64, table: [u16; 5], ps in pools(), kf in float()) {
        let mut s = state(seed);
        for (i, v) in table.iter().enumerate() {
            wh(&mut s, 0x8009_AEFC + 2 * i as u32, *v);
        }
        wr(&mut s, 0x800A_8220, kf.to_bits());
        let count = put_pools(&mut s, &ps);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (mode, k);
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA)]);
        match mode {
            0 => {
                wr(&mut want, MODE, 0);
                wr(&mut want, 0x8009_AEEC, u32::MAX);
            }
            1 => {
                wr(&mut want, MODE, 1);
                let v = table[(k as u32 % 5) as usize] as i16 as i32 as u32;
                wr(&mut want, 0x8009_AEEC, if count <= 0 { 0x8F } else { v });
            }
            2 => {
                wr(&mut want, MODE, 2);
                wr(&mut want, 0x8009_AEE8, kf.to_bits());
            }
            3 => {
                wr(&mut want, MODE, 3);
                wr(&mut want, 0x8009_AEE8, (-2.0f32).to_bits());
            }
            _ => {}
        }
        let after = run("func_80009744", misc::func_80009744, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_80009B8C(seed: u64, k: u64, table: [u16; 12], ps in pools(), words in prop::array::uniform4(prop_oneof![Just(0u32), any::<u32>()]), kf in float()) {
        let mut s = state(seed);
        for (i, v) in table.iter().enumerate() {
            wh(&mut s, 0x8009_AF08 + 2 * i as u32, *v);
        }
        for (i, v) in words.iter().enumerate() {
            wr(&mut s, WORDS + 4 * i as u32, *v);
        }
        wr(&mut s, 0x800A_822C, kf.to_bits());
        let count = put_pools(&mut s, &ps);
        s.ctx.gpr[A0] = k;
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA), (0x18, A0)]);
        let free = words.iter().position(|&w| w == 0).unwrap_or(4);
        if !(count > 0 && free == 0) {
            wr(&mut want, 0x8009_AF24, 0);
            wr(&mut want, 0x8009_AEF0, table[(k as u32 % 12) as usize] as i16 as i32 as u32);
            wr(&mut want, 0x8009_AF20, kf.to_bits());
        }
        let after = run("func_80009B8C", misc::func_80009B8C, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_8000A4D8: record reset ----

const COUNT: u32 = 0x8009_B770;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn func_8000A4D8(seed: u64, count in prop_oneof![3 => 0i32..12, 1 => 195i32..206, 1 => -3i32..0], junk in prop::collection::vec(any::<u32>(), 8 * 205)) {
        let mut s = state(seed);
        wr(&mut s, COUNT, count as u32);
        for (i, w) in junk.iter().enumerate() {
            wr(&mut s, RECORDS + 4 * i as u32, *w);
        }
        let mut want = s.clone();
        saves(&mut want, &s, 0x20, &[(0x1C, RA), (0x18, S1), (0x14, S0)]);
        for i in 0..count.max(0) as u32 {
            // func_8000A44C(i, 0): its spill, then record i if i < 200.
            wr(&mut want, SP_AT - 0x20, i);
            if i < 200 {
                let r = RECORDS + 32 * i;
                wh(&mut want, r, 0);
                wh(&mut want, r + 2, 0);
                wr(&mut want, r + 0x14, 1);
                wr(&mut want, r + 0x18, u32::MAX);
                wr(&mut want, r + 0x1C, 0);
                wr(&mut want, r + 8, 1.0f32.to_bits());
                wr(&mut want, r + 0xC, 1.0f32.to_bits());
                wr(&mut want, r + 0x10, 0);
            }
        }
        wr(&mut want, COUNT, 0);
        let after = run("func_8000A4D8", misc::func_8000A4D8, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_8000AC90, func_8000ACC0, func_8000B0E0: entries ----

const ENTRY_BASE: u32 = 0x8030_6000;
const SEL: u32 = 0x8009_B798;
const MAIN: u32 = 0x800D_4B20;

fn identity(want: &mut State, at: u32) {
    for i in 0..16 {
        wr(want, at + 4 * i, if i % 5 == 0 { 1.0f32.to_bits() } else { 0 });
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn func_8000AC90(seed: u64, v: u64, old: u32) {
        let mut s = state(seed);
        wr(&mut s, 0x800A_48D4, old);
        s.ctx.gpr[A0] = v;
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA), (0x18, A0)]);
        wr(&mut want, SP_AT - 0x18, v as u32 & 0xFFFF);
        wr(&mut want, 0x800A_48D4, v as u32 & 0xFFFF);
        let after = run("func_8000AC90", misc::func_8000AC90, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8000ACC0(seed: u64, junk in prop::collection::vec(any::<u32>(), 31 * 33 + 34)) {
        let mut s = state(seed);
        wr(&mut s, ENTRIES, ENTRY_BASE);
        for (i, w) in junk[..31 * 33].iter().enumerate() {
            wr(&mut s, ENTRY_BASE + 4 * i as u32, *w);
        }
        for (i, w) in junk[31 * 33..].iter().enumerate() {
            wr(&mut s, MAIN + 4 * i as u32, *w);
        }
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA)]);
        for i in 0..32 {
            let e = ENTRY_BASE + 0x7C * i;
            identity(&mut want, e + 0x14);
            wr(&mut want, e + 8, 0);
            wh(&mut want, e + 4, 0);
            for j in 0..3 {
                wr(&mut want, e + 0x6C + 4 * j, 0);
            }
        }
        wr(&mut want, 0x8009_B794, ENTRY_BASE);
        wr(&mut want, SEL, 0);
        let w0 = word(&s, MAIN) & !1;
        wr(&mut want, MAIN, w0);
        for o in [4, 6, 0x80] {
            wh(&mut want, MAIN + o, 0);
        }
        wr(&mut want, MAIN + 0x20, 0);
        wr(&mut want, MAIN + 0x84, 0);
        identity(&mut want, MAIN + 0x28);
        for j in 0..3 {
            wr(&mut want, MAIN + 0x68 + 4 * j, 0);
        }
        wr(&mut want, MAIN + 0x24, 10.0f32.to_bits());
        // func_8000AED4(1, 4): entry 1's flags |= 4.
        let f1 = word(&want, ENTRY_BASE + 0x7C);
        wr(&mut want, ENTRY_BASE + 0x7C, f1 | 4);
        let after = run("func_8000ACC0", misc::func_8000ACC0, &s)?;
        same_memory(&after, &want)?;
    }

    #[test]
    fn func_8000B0E0(seed: u64, id in prop_oneof![0u32..32, Just(0xFFFFu32), any::<u32>().prop_map(|v| v | 0xFFFF), any::<u32>()], o_set: bool,
                     cur in prop_oneof![0i32..32, Just(-1i32)], sel in 0u32..32, flags in prop::collection::vec(any::<u32>(), 32), hi: u32, tail: u32) {
        let mut s = state(seed);
        let o = 0x8030_7000;
        wr(&mut s, ENTRIES, ENTRY_BASE);
        for (i, f) in flags.iter().enumerate() {
            wr(&mut s, ENTRY_BASE + 0x7C * i as u32, *f);
        }
        wr(&mut s, o + 4, cur as u32);
        wr(&mut s, SEL, sel);
        wr(&mut s, 0x8009_B79C, tail);
        let a0 = u64::from(hi) << 32 | u64::from(id);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (a0, if o_set { sext(o) } else { 0 });
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA), (0x18, A0)]);
        if o_set {
            wr(&mut want, SP_AT + 4, o);
            let sid = id as u16 as i16;
            wh(&mut want, SP_AT + 2, sid as u16);
            if cur != -1 {
                let e = ENTRY_BASE + 0x7C * cur as u32;
                let f = word(&want, e) & !1;
                wr(&mut want, e, f);
            }
            if sid != -1 {
                wr(&mut want, o + 4, sid as i32 as u32);
                let e = ENTRY_BASE + 0x7C * sel;
                let f = word(&want, e) | 1;
                wr(&mut want, e, f);
                wr(&mut want, 0x8009_B79C, 0);
            } else {
                wr(&mut want, SEL, u32::MAX);
            }
        }
        let after = run("func_8000B0E0", misc::func_8000B0E0, &s)?;
        same_memory(&after, &want)?;
    }
}

// ---- func_8000B254: a node's transform as a 4x4 matrix ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn func_8000B254(seed: u64, mode in prop_oneof![Just(2u64), Just(3u64), 0u64..5, any::<u64>()], ty in prop_oneof![Just(0xD065u32), Just(0xD064u32), Just(0xD066u32), any::<u32>()],
                     t in prop::collection::vec(any::<u32>(), 12), junk in prop::collection::vec(any::<u32>(), 16)) {
        let (n, m) = (0x8030_8000u32, 0x8030_8100u32);
        let mut s = state(seed);
        wr(&mut s, n, ty);
        for (i, w) in t.iter().enumerate() {
            wr(&mut s, n + 0x1C + 4 * i as u32, *w);
        }
        for (i, w) in junk.iter().enumerate() {
            wr(&mut s, m + 4 * i as u32, *w);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2]) = (mode, sext(n), sext(m));
        let mut want = s.clone();
        saves(&mut want, &s, 0x18, &[(0x14, RA)]);
        let transform = mode == 2 || mode == 3 || sext(ty) == 0xD065;
        if mode != 2 && mode != 3 {
            wr(&mut want, SP_AT + 4, n);
            wr(&mut want, SP_AT + 8, m);
        }
        if transform {
            for i in 0..4u32 {
                for j in 0..3u32 {
                    wr(&mut want, m + 16 * i + 4 * j, t[(3 * i + j) as usize]);
                }
                wr(&mut want, m + 16 * i + 12, if i == 3 { 1.0f32.to_bits() } else { 0 });
            }
        } else {
            identity(&mut want, m);
        }
        let after = run("func_8000B254", misc::func_8000B254, &s)?;
        same_memory(&after, &want)?;
        prop_assert_eq!(half(&after, n), half(&s, n));
    }
}
