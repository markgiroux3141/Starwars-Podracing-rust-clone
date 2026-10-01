//! Depth 2 at 0x80029494..0x80031B70 (game::misc, heap, pools, channels):
//! three boxes of an object's models, aiming the camera at a target, a
//! heap allocation and an aligned carve, placing a pool on the heap, the
//! HUD entries' setup, and two channel wrappers. Recompiled C vs Rust with
//! the callees as C. The models replay the callees' C on a copy of the
//! state in the same order with the same arguments (and the s registers
//! the port holds), adding the functions' own stores; whole RDRAM (and
//! `v0` where there is a result) is compared.

// Tests are named after the functions (func_80029494), capitals included.
#![allow(non_snake_case)]

use difftest::rom::baserom;
use difftest::world::{world, HEAP_8MB};
use difftest::{compare, State};
use game::channels::{self, CHANNELS, CURRENT, ENTRIES as CH_ENTRIES};
use game::heap::{self, CURSORS, HEAP_END, LEVEL};
use game::imports;
use game::pools::{self, POOLS};
use game::recomp::{reg::*, RecompFn};
use game::misc;
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
    let mut s = world(seed, HEAP_8MB);
    s.ctx.gpr[SP] = sext(SP_AT);
    load_data(&mut s);
    s
}

/// The game's data segment (`0x80098000..0x800AE8B0`) as the ROM image has
/// it, leaving the heap words `world` set.
fn load_data(s: &mut State) {
    let keep: Vec<(u32, u32)> = [LEVEL, CURSORS, HEAP_END].iter().flat_map(|&a| (0..10).map(move |k| a + 4 * k)).map(|a| (a, word(s, a))).collect();
    let rom = baserom();
    let mut m = s.rdram.mem();
    for va in (0x8009_8000u32..0x800A_E8B0).step_by(4) {
        let o = (va - 0x8000_0400 + 0x1000) as usize;
        m.write_u32(va, u32::from_be_bytes(rom[o..o + 4].try_into().unwrap()));
    }
    for (a, v) in keep {
        m.write_u32(a, v);
    }
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

fn wf(s: &mut State, a: u32, v: f32) {
    wr(s, a, v.to_bits());
}

fn rf(s: &State, a: u32) -> f32 {
    f32::from_bits(word(s, a))
}

/// Runs a callee's C on `w` with `sp` at `SP_AT - frame` and `args` set.
fn call_c(w: &mut State, frame: u32, args: &[(usize, u64)], f: RecompFn) -> u64 {
    for &(r, v) in args {
        w.ctx.gpr[r] = v;
    }
    w.ctx.gpr[SP] = sext(SP_AT - frame);
    w.run(f);
    w.ctx.gpr[V0]
}

fn saves(w: &mut State, s: &State, frame: u32, regs: &[(u32, usize)]) {
    for &(off, r) in regs {
        wr(w, SP_AT - frame + off, s.ctx.gpr[r] as u32);
    }
}

fn save_double(w: &mut State, a: u32, v: u64) {
    wr(w, a, (v >> 32) as u32);
    wr(w, a + 4, v as u32);
}

fn ok(x: f32) -> Option<f32> {
    (!x.is_nan()).then_some(x)
}
fn mul(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? * ok(b)?)
}
fn add(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? + ok(b)?)
}
fn div(a: f32, b: f32) -> Option<f32> {
    Some(ok(a)? / ok(b)?)
}

fn fin() -> BoxedStrategy<f32> {
    prop_oneof![
        4 => -100.0f32..100.0,
        2 => -1.0f32..1.0,
        2 => (-4i32..=4).prop_map(|n| n as f32),
        1 => Just(-0.0f32),
        1 => Just(-157.0f32),
        1 => (1u32..0x0080_0000).prop_map(f32::from_bits),
    ]
    .boxed()
}

// ---- func_8002FEE4 / func_80030C08: the heap ----

/// The cursor and end moved (the end below the cursor too), at level 0 or
/// 1.
fn put_heap(s: &mut State, cursor: u32, end: u32, level: u32) {
    wr(s, LEVEL, level);
    wr(s, CURSORS + 4 * level, cursor);
    wr(s, HEAP_END, end);
}

fn heap_state() -> BoxedStrategy<(u32, u32, u32)> {
    (prop_oneof![Just(0x8019_8820u32), 0x8019_0000u32..0x8030_0000], prop_oneof![Just(0x8063_A400u32), 0x8019_0000u32..0x8070_0000], prop_oneof![3 => Just(0u32), 1 => Just(1u32)])
        .prop_map(|(c, e, l)| (c & !3, e & !3, l))
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Sizes below, at and above the bytes left, negative ones, and a
    /// negative free count (the end below the cursor; unsigned compare).
    #[test]
    fn func_8002FEE4(seed: u64, (cursor, end, level) in heap_state(), n in prop_oneof![0u32..0x10_0000, any::<u32>()], at_free in prop_oneof![3 => Just(None), 1 => (-1i32..=1).prop_map(Some)]) {
        let mut s = state(seed);
        put_heap(&mut s, cursor, end, level);
        let n = at_free.map_or(n, |d| end.wrapping_sub(cursor).wrapping_add(d as u32));
        s.ctx.gpr[A0] = sext(n);
        let mut w = s.clone();
        saves(&mut w, &s, 0x20, &[(0x14, RA), (0x20, A0)]);
        let free = call_c(&mut w, 0x20, &[], imports::func_8002FC58);
        let v0 = if sext(n) < free {
            let p = call_c(&mut w, 0x20, &[], imports::func_8002FAFC);
            wr(&mut w, SP_AT - 0x20 + 0x1C, p as u32);
            call_c(&mut w, 0x20, &[(A0, sext((p as u32).wrapping_add(n)))], imports::func_8002FAC4);
            sext(p as u32)
        } else {
            0
        };
        let after = run("func_8002FEE4", heap::func_8002FEE4, &s)?;
        same_memory(&after, &w)?;
        prop_assert_eq!(after.ctx.gpr[V0], v0);
    }

    #[test]
    fn func_80030C08(seed: u64, (cursor, end, level) in heap_state(), mem in prop_oneof![Just(0x80_0000u32), Just(0x40_0000u32), Just(0x7F_FFFFu32), any::<u32>()], off in 0u32..64) {
        let mut s = state(seed);
        put_heap(&mut s, cursor + off, end, level);
        wr(&mut s, 0x8000_0318, mem);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        let p = call_c(&mut w, 0x18, &[], imports::func_8002FAFC) as u32;
        wr(&mut w, 0x800D_B894, p);
        wr(&mut w, 0x800D_B894, p.wrapping_add(0x3F) & !0x3F);
        let size = if mem < 0x80_0000 { 0x4000u32 } else { 0x1_0000 };
        let v = size.wrapping_add(word(&w, 0x800D_B894));
        wr(&mut w, 0x800D_B898, v);
        call_c(&mut w, 0x18, &[(A0, sext(size.wrapping_add(p).wrapping_add(0x40)))], imports::func_8002FAC4);
        let after = run("func_80030C08", heap::func_80030C08, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- func_80030298: a pool on the heap ----

const LIST: u32 = 0x8030_0000;
const DESCS: u32 = 0x8030_0100;
const MARK: u32 = 0x8005_F31C;
const STOP: u32 = 0x8006_FED0;

#[derive(Clone, Debug)]
struct Pool {
    id: u32,
    count: i32,
    size: u32,
    flags: u16,
    callback: u32,
}

fn pool_list() -> BoxedStrategy<Vec<Pool>> {
    prop::collection::vec(
        (1u32..3, prop_oneof![0i32..4, Just(-1i32)], prop_oneof![Just(0x68u32), 0u32..0x100], prop_oneof![Just(0u16), Just(0x100u16), any::<u16>()],
         prop_oneof![2 => Just(0u32), 2 => Just(MARK), 1 => Just(STOP)])
            .prop_map(|(id, count, size, flags, callback)| Pool { id, count, size: size & !3, flags, callback }),
        0..4,
    )
    .boxed()
}

fn put_pools(s: &mut State, pools: &[Pool]) {
    s.randomise_memory(0x9001, LIST, 0x200);
    wr(s, POOLS, LIST);
    for (k, p) in pools.iter().enumerate() {
        let d = DESCS + 0x40 * k as u32;
        wr(s, LIST + 4 * k as u32, d);
        wr(s, d, p.id);
        wh(s, d + 4, p.flags);
        wr(s, d + 8, p.count as u32);
        wr(s, d + 0xC, p.size);
        wr(s, d + 0x10, 0x8033_0000 + 0x1000 * k as u32);
        wr(s, d + 0x24, p.callback);
    }
    wr(s, LIST + 4 * pools.len() as u32, 0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Pools with and without the id, several with it (the first wins the
    /// placement, all are initialised and messaged), callbacks that mark
    /// or stop the message.
    #[test]
    fn func_80030298(seed: u64, pools in pool_list(), id in 1u32..4, count in prop_oneof![0u32..5, Just(u32::MAX)], (cursor, end, level) in heap_state()) {
        let mut s = state(seed);
        put_heap(&mut s, cursor, end, level);
        put_pools(&mut s, &pools);
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(id), sext(count));
        let mut w = s.clone();
        let fr = SP_AT - 0x28;
        saves(&mut w, &s, 0x28, &[(0x14, RA), (0x28, A0), (0x2C, A1)]);
        let p = call_c(&mut w, 0x28, &[], imports::func_8002FAFC);
        wr(&mut w, fr + 0x20, p as u32);
        let size = call_c(&mut w, 0x28, &[(A0, sext(id)), (A1, sext(count)), (A2, p)], imports::func_8003FB78);
        wr(&mut w, fr + 0x24, size as u32);
        call_c(&mut w, 0x28, &[(A0, sext(id))], imports::func_8003F300);
        call_c(&mut w, 0x28, &[(A0, sext((size as u32).wrapping_add(p as u32)))], imports::func_8002FAC4);
        wr(&mut w, fr + 0x1C, 0x4C6F_6164);
        call_c(&mut w, 0x28, &[(A0, sext(id)), (A1, sext(fr + 0x1C))], imports::func_8003FA24);
        let after = run("func_80030298", pools::func_80030298, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- func_80031924 / func_80031B70: channels ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80031924(seed: u64, i in -1i32..4) {
        let mut s = state(seed);
        s.ctx.gpr[A0] = sext(i as u32);
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        call_c(&mut w, 0x18, &[(A0, sext(i as u32))], imports::func_80031BBC);
        let after = run("func_80031924", channels::func_80031924, &s)?;
        same_memory(&after, &w)?;
    }

    /// Bit 26 of the flags either way (other bits random, or all zero
    /// below it: `flags << 5` then 0), channels 0..3,
    /// entries ranking below, at or above the current one (13 and up hang:
    /// not started here).
    #[test]
    fn func_80031B70(seed: u64, flags in prop_oneof![3 => any::<u32>(), 1 => Just(0u32), 1 => Just(0xF800_0000u32)], bit: bool, c in 0u8..4, k in -2i32..13, d in -1i32..=1, e4 in prop_oneof![Just(0u32), any::<u32>()], stopped: bool) {
        let mut s = state(seed);
        let (o, link) = (0x8030_4000u32, 0x8030_6000u32);
        wr(&mut s, o + 0x64, if bit { flags | 1 << 26 } else { flags & !(1 << 26) });
        wr(&mut s, o + 0x1E70, link);
        s.rdram.mem().write_u8(link + 0x10, c);
        let c = u32::from(c);
        wr(&mut s, CURRENT + 4 * c, k.wrapping_add(d) as u32);
        wr(&mut s, CH_ENTRIES.wrapping_add((12 * k) as u32) + 4, e4);
        wh(&mut s, CHANNELS + 28 * c + 8, if stopped { 0 } else { 1 + (seed as u16 & 0x7FFE) });
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(o), sext(k as u32));
        let mut w = s.clone();
        saves(&mut w, &s, 0x18, &[(0x14, RA)]);
        if !bit {
            call_c(&mut w, 0x18, &[(A0, u64::from(c)), (A1, sext(k as u32)), (A2, sext(o))], imports::func_800319F4);
        } else {
            call_c(&mut w, 0x18, &[(A0, u64::from(c)), (A2, sext(o))], imports::func_80031BBC);
        }
        let after = run("func_80031B70", channels::func_80031B70, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- func_80031134: the HUD entries ----

const CMAN: u32 = 0x634D_616E;
const ENTRY_BASE: u32 = 0x8031_0000;
const CELEMS: u32 = 0x8032_0000;
const CSIZE: u32 = 0x200;

/// One "cMan" pool (`elems`: tag and flags per element, in pool order)
/// after another pool, and the entry array at `ENTRY_BASE`.
fn put_cman(s: &mut State, count: i32, elems: &[(u32, u16)]) {
    s.randomise_memory(0x7777, LIST, 0x200);
    wr(s, POOLS, LIST);
    wr(s, LIST, DESCS);
    wr(s, LIST + 4, DESCS + 0x40);
    wr(s, LIST + 8, 0);
    wr(s, DESCS, 0x6F74_6872);
    wr(s, DESCS + 8, 0);
    let d = DESCS + 0x40;
    wr(s, d, CMAN);
    wr(s, d + 8, count as u32);
    wr(s, d + 0xC, CSIZE);
    wr(s, d + 0x10, CELEMS);
    for (i, &(tag, flags)) in elems.iter().enumerate() {
        let e = CELEMS + CSIZE * i as u32;
        wh(s, e + 4, tag as u16);
        wh(s, e + 6, flags);
    }
    wr(s, 0x8009_B790, ENTRY_BASE);
}

fn entries_model(s: &State) -> State {
    let mut w = s.clone();
    saves(&mut w, s, 0x40, &[(0x3C, RA), (0x34, S1), (0x38, S2), (0x30, S0)]);
    save_double(&mut w, SP_AT - 0x40 + 0x28, s.ctx.fpr[20].u64);
    w.ctx.gpr[S1] = 7;
    call_c(&mut w, 0x40, &[], imports::func_8000ACC0);
    for i in 0..10u32 {
        call_c(&mut w, 0x40, &[(A2, sext(0x800D_69A0 + 0x40 * i)), (A0, u64::from(i)), (A1, 1), (A3, 0)], imports::func_8000AEFC);
    }
    w.ctx.gpr[S2] = sext(CMAN);
    let mut n = call_c(&mut w, 0x40, &[(A0, sext(CMAN))], imports::func_8003F7B8) as i64;
    if n > 0 {
        w.ctx.fpr[20].set_u32l(0);
        let mut j: i16 = 7;
        loop {
            let e = call_c(&mut w, 0x40, &[(A0, sext(CMAN)), (A1, (i64::from(j) - 7) as u64)], imports::func_8003F714);
            w.ctx.gpr[S0] = e;
            let jw = i64::from(j) as u64;
            call_c(&mut w, 0x40, &[(A0, jw), (A1, 3), (A2, sext((e as u32).wrapping_add(0x20))), (A3, 0)], imports::func_8000AEFC);
            call_c(&mut w, 0x40, &[(A0, jw), (A1, sext((e as u32).wrapping_add(0x108))), (A2, 0)], imports::func_8000B02C);
            for off in [0x10, 0x14, 0x18] {
                wr(&mut w, SP_AT - 0x40 + off, 0);
            }
            call_c(&mut w, 0x40, &[(A0, jw), (A1, 0), (A2, 0), (A3, 0)], imports::func_8000AF4C);
            wr(&mut w, (e as u32).wrapping_add(0x78), j as i32 as u32);
            j = j.wrapping_add(1);
            w.ctx.gpr[S1] = i64::from(j) as u64;
            if j >= 32 {
                break;
            }
            n = call_c(&mut w, 0x40, &[(A0, sext(CMAN))], imports::func_8003F7B8) as i64;
            if !(i64::from(j) - 7 < n) {
                break;
            }
        }
    }
    call_c(&mut w, 0x40, &[(A0, 8)], imports::func_8000B1B0);
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Pools of 0 to 25 elements (the loop stops at entry 31, 25 lookups),
    /// tags in any order; with 25, flagged duplicates in front (the lookup
    /// skips bit 8) so the count goes past the elements looked up; a
    /// negative count.
    #[test]
    fn func_80031134(seed: u64, n in prop_oneof![0i32..26, Just(25i32), Just(-1i32)], perm in Just(()).prop_perturb(|_, mut r| { let mut v: Vec<u32> = (0..25).collect(); for i in (1..v.len()).rev() { let j = (r.next_u32() as usize) % (i + 1); v.swap(i, j); } v }),
                     dups in prop::collection::vec((0u32..25, any::<u16>()), 0..3), sel in 0u32..40, other: u32) {
        let mut s = state(seed);
        let mut elems: Vec<(u32, u16)> = Vec::new();
        if n == 25 {
            elems.extend(dups.iter().map(|&(t, f)| (t, f | 0x100)));
        }
        elems.extend(perm.into_iter().filter(|&t| (t as i32) < n).map(|t| (t, 0u16)));
        put_cman(&mut s, if n < 0 { n } else { elems.len() as i32 }, &elems);
        wr(&mut s, 0x8009_B798, sel);
        wr(&mut s, 0x800D_4B20, other);
        let w = entries_model(&s);
        let after = run("func_80031134", misc::func_80031134, &s)?;
        same_memory(&after, &w)?;
    }
}

// ---- func_80029494: three boxes ----

const NODES: u32 = 0x8034_0000;
const BOXES: u32 = 0x8035_0000;
const OBJ: u32 = 0x8030_8000;

/// Three trees: none (null), a box node (0x3064) whose first child holds
/// the six floats, a node of another type, or a transform (0xD065) above a
/// box node.
fn put_tree(s: &mut State, t: usize, kind: u8, bx: [f32; 6], m: [f32; 12]) -> u32 {
    let n = NODES + 0x400 * t as u32;
    let leaf = n + 0x200;
    let bxa = BOXES + 0x40 * t as u32;
    for (k, &x) in bx.iter().enumerate() {
        wf(s, bxa + 8 + 4 * k as u32, x);
    }
    let boxnode = |s: &mut State, at: u32| {
        wr(s, at, 0x3064);
        wr(s, at + 0x14, 1);
        wr(s, at + 0x18, at + 0xC0);
        wr(s, at + 0xC0, bxa);
    };
    match kind {
        0 => 0,
        1 => {
            boxnode(s, n);
            n
        }
        2 => {
            wr(s, n, 0x8001);
            wr(s, n + 0x14, 0);
            n
        }
        _ => {
            wr(s, n, 0xD065);
            wr(s, n + 0x14, 1);
            wr(s, n + 0x18, n + 0xC0);
            wr(s, n + 0xC0, leaf);
            for (k, &x) in m.iter().enumerate() {
                wf(s, n + 0x1C + 4 * k as u32, x);
            }
            boxnode(s, leaf);
            n
        }
    }
}

fn boxes_model(s: &State) -> Option<State> {
    let mut w = s.clone();
    let (b0, b1, b2) = (0x800D_6D90u32, 0x800D_6DA8u32, 0x800D_6DC0u32);
    saves(&mut w, s, 0x28, &[(0x18, S0), (0x1C, RA)]);
    let o = word(&w, 0x8011_A544);
    wr(&mut w, SP_AT - 0x28 + 0x20, o);
    w.ctx.gpr[S0] = sext(b0);
    let tree = word(&w, o + 0x100);
    let found = call_c(&mut w, 0x28, &[(A0, sext(tree)), (A1, sext(b0)), (A2, 0)], imports::func_80083D80);
    if found != 0 {
        let (py, qx, qy) = (rf(&w, b0 + 4), rf(&w, b0 + 0xC), rf(&w, b0 + 0x10));
        let y = add(qy, py)?;
        let x = add(qx, 100.0)?;
        let yy = (f64::from(y) * 0.5) as f32;
        call_c(&mut w, 0x28, &[(A0, sext(0x800A_4FCC)), (A1, sext(x.to_bits())), (A2, sext(yy.to_bits())), (A3, sext(0xC31D_0000))], imports::func_80015268);
    } else {
        call_c(&mut w, 0x28, &[(A0, sext(b0)), (A1, 0), (A2, 0), (A3, 0)], imports::func_80015268);
        let ten = sext((-10.0f32).to_bits());
        call_c(&mut w, 0x28, &[(A0, sext(0x800D_6D9C)), (A1, ten), (A2, ten), (A3, ten)], imports::func_80015268);
    }
    for (off, b, last) in [(0xF8u32, b1, false), (0xFC, b2, true)] {
        let o = word(&w, SP_AT - 0x28 + 0x20);
        let tree = word(&w, o + off);
        let found = call_c(&mut w, 0x28, &[(A0, sext(tree)), (A1, sext(b)), (A2, 0)], imports::func_80083D80);
        if found == 0 {
            call_c(&mut w, 0x28, &[(A0, sext(b)), (A1, sext(b0))], imports::func_80015288);
            call_c(&mut w, 0x28, &[(A0, sext(b + 0xC)), (A1, sext(0x800D_6D9C))], imports::func_80015288);
            if last {
                let (qy, py) = (rf(&w, b0 + 0x10), rf(&w, b0 + 4));
                let h = mul(add(qy, py)?, 0.5)?;
                wf(&mut w, 0x800D_6DD0, h);
                wf(&mut w, 0x800D_6DB8, h);
                let v = div(add(mul(qy, 5.0)?, py)?, 6.0)?;
                wf(&mut w, b0 + 4, v);
            }
        }
    }
    Some(w)
}

fn small() -> BoxedStrategy<f32> {
    prop_oneof![4 => -50.0f32..50.0, 1 => (-4i32..=4).prop_map(|n| n as f32), 1 => Just(-0.0f32)].boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_80029494(seed: u64, kinds in prop::array::uniform3(0u8..4), bxs in prop::array::uniform3(prop::array::uniform6(small())),
                     ms in prop::array::uniform3(prop::array::uniform12(-1.0f32..1.0)), junk in prop::array::uniform6(small())) {
        let mut s = state(seed);
        for (k, x) in junk.iter().enumerate() {
            wf(&mut s, 0x800D_6D90 + 4 * k as u32, *x);
        }
        for (t, off) in [0x100u32, 0xF8, 0xFC].iter().enumerate() {
            let n = put_tree(&mut s, t, kinds[t], bxs[t], ms[t]);
            wr(&mut s, OBJ + off, n);
        }
        wr(&mut s, 0x8011_A544, OBJ);
        let want = boxes_model(&s);
        prop_assume!(want.is_some());
        let after = run("func_80029494", misc::func_80029494, &s)?;
        same_memory(&after, &want.unwrap())?;
    }
}

// ---- func_8002B3C8: aim the camera ----

const PAIRS: u32 = 0x8030_A000;
const PNODES: u32 = 0x8030_B000;

/// A node pair at `PAIRS + 8 j` (`a`, `b` at `+0`/`+4`, either null), the
/// nodes' 3x4 transforms bounded.
fn put_pair(s: &mut State, j: u32, null_a: bool, null_b: bool, ta: [f32; 12], tb: [f32; 12]) -> u32 {
    let (pair, a, b) = (PAIRS + 8 * j, PNODES + 0x200 * j, PNODES + 0x200 * j + 0x100);
    wr(s, pair, if null_a { 0 } else { a });
    wr(s, pair + 4, if null_b { 0 } else { b });
    for (k, (&x, &y)) in ta.iter().zip(&tb).enumerate() {
        wf(s, a + 0x1C + 4 * k as u32, x);
        wf(s, b + 0x1C + 4 * k as u32, y);
    }
    pair
}

fn aim_model(s: &State, k: u32) -> Option<State> {
    let mut w = s.clone();
    let fr = SP_AT - 0x40;
    saves(&mut w, s, 0x40, &[(0x1C, RA), (0x40, A0)]);
    if k.wrapping_sub(0x1A) >= 5 {
        return Some(w);
    }
    let o = s.ctx.gpr[A0] as u32;
    if k == 0x1E {
        let pair = word(&w, 0x8011_A578);
        call_c(&mut w, 0x40, &[(A0, sext(pair)), (A1, sext(fr + 0x34))], imports::func_80033590);
        let i = ((word(&w, o + 0x70) >> 8) as u8 as i8) as i32;
        let t = rf(&w, 0x800A_3200u32.wrapping_add((52 * i) as u32));
        ok(t)?;
        let d1 = f64::from_bits(u64::from(word(&w, 0x800A_9E80)) << 32 | u64::from(word(&w, 0x800A_9E84)));
        let d2 = f64::from_bits(u64::from(word(&w, 0x800A_9E88)) << 32 | u64::from(word(&w, 0x800A_9E8C)));
        let z = game::recomp::fpu::cvt_s_d(f64::from(t) * d1 + d2, game::recomp::fpu::NEAREST);
        wf(&mut w, fr + 0x3C, z);
    } else {
        let pair = word(&w, 0x8011_A570u32.wrapping_add(k.wrapping_mul(4)));
        call_c(&mut w, 0x40, &[(A0, sext(pair)), (A1, sext(fr + 0x34))], imports::func_80033590);
        let z = rf(&w, fr + 0x3C);
        let z = add(z, if z == -157.0 { 60.0 } else { 30.0 })?;
        wf(&mut w, fr + 0x3C, z);
    }
    let mode = word(&w, 0x800A_4BC0) >> 16;
    match mode as u16 as i16 {
        1 => {
            wr(&mut w, fr + 0x10, 1);
            call_c(&mut w, 0x40, &[(A0, sext(0x8011_8E10)), (A1, sext(fr + 0x34)), (A2, 1), (A3, 0)], imports::func_8005058C);
        }
        3 => {
            wr(&mut w, fr + 0x10, 1);
            call_c(&mut w, 0x40, &[(A0, sext(0x8011_8D90)), (A1, sext(fr + 0x34)), (A2, 3), (A3, 1)], imports::func_8005058C);
        }
        _ => {
            call_c(&mut w, 0x40, &[(A0, sext(fr + 0x28)), (A1, sext(0x8011_8D90)), (A2, sext(0x8011_8E50))], imports::func_8001535C);
            call_c(&mut w, 0x40, &[(A0, sext(0x8011_8E50)), (A1, sext(fr + 0x34))], imports::func_80015288);
            call_c(&mut w, 0x40, &[(A0, sext(0x8011_8D90)), (A1, sext(0x8011_8E50)), (A2, sext(fr + 0x28))], imports::func_80015328);
        }
    }
    Some(w)
}

fn tf() -> BoxedStrategy<[f32; 12]> {
    prop::array::uniform12(prop_oneof![3 => -10.0f32..10.0, 1 => Just(-157.0f32), 1 => (-2i32..=2).prop_map(|n| n as f32)]).boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(768))]

    /// Every target kind (and some outside the table), null pairs and
    /// nodes, `t.z` exactly -157 (the pair's translation z), the modes 1, 3
    /// and others, table rows of the racer byte's whole range.
    #[test]
    fn func_8002B3C8(seed: u64, k in prop_oneof![4 => 0x1Au32..0x1F, 1 => 0x18u32..0x22, 1 => any::<u32>()], nulls: [bool; 2], ta in tf(), tb in tf(),
                     mode in prop_oneof![Just(1u16), Just(3u16), Just(0u16), any::<u16>()], racer: i8, eye in prop::array::uniform6(fin()), z157: bool) {
        let mut s = state(seed);
        let ta = if z157 { let mut t = ta; t[11] = -157.0; t } else { ta };
        let pair = put_pair(&mut s, 0, nulls[0], nulls[1], ta, tb);
        let slot = if k == 0x1E { 0x8011_A578 } else { 0x8011_A570u32.wrapping_add(k.wrapping_mul(4)) };
        if (0x8000_0000..0x8080_0000).contains(&slot) {
            wr(&mut s, slot, pair);
        }
        let o = 0x8030_C000u32;
        s.rdram.mem().write_u8(o + 0x72, racer as u8);
        wh(&mut s, 0x800A_4BC0, mode);
        for (j, x) in eye.iter().enumerate() {
            wf(&mut s, if j < 3 { 0x8011_8D90 } else { 0x8011_8E50 } + 4 * (j as u32 % 3), *x);
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(o), sext(k));
        let want = aim_model(&s, k);
        prop_assume!(want.is_some());
        let after = run("func_8002B3C8", misc::func_8002B3C8, &s)?;
        same_memory(&after, &want.unwrap())?;
    }
}

/// Separators for the double multiply-add `f64(T[i]) * D1 + D2` rounded to
/// f32 (k = 0x1E): with `p = fl(t * D1)` and `D2 = m - p` for `m` the
/// midpoint between two neighbouring floats in a lower binade than `p` (a
/// cancellation), the unfused sum is exactly the tie (rounded to the even
/// one), while the fused one is `m` plus the product's rounding error, more
/// than half an ulp of `m`, so it lies on that error's side; searched here
/// over `t` and `D1` (written into RAM with `D2`) for cases where that side
/// is the odd neighbour.
#[test]
fn func_8002B3C8_separators() {
    let mut found = 0;
    'search: for a in 0..200u64 {
        let d1 = f64::from_bits(0x3FF1_2345_6789_ABCD + 0x1_0001_0001 * a);
        for b in 0..50u32 {
            let t = 1.5f32 + b as f32 * 0.37;
            let p = f64::from(t) * d1;
            let near = (p / 5.0) as f32;
            let m = (f64::from(near) + f64::from(f32::from_bits(near.to_bits() + 1))) / 2.0;
            let d2 = m - p;
            if p + d2 != m {
                continue;
            }
            let unfused = (p + d2) as f32;
            let fused = f64::from(t).mul_add(d1, d2) as f32;
            if unfused == fused {
                continue;
            }
            let mut s = state(0xB3C8 + found);
            let o = 0x8030_C000u32;
            s.rdram.mem().write_u8(o + 0x72, 3);
            wf(&mut s, 0x800A_3200 + 52 * 3, t);
            for (at, v) in [(0x800A_9E80u32, d1), (0x800A_9E88, d2)] {
                wr(&mut s, at, (v.to_bits() >> 32) as u32);
                wr(&mut s, at + 4, v.to_bits() as u32);
            }
            wr(&mut s, 0x8011_A578, 0);
            wh(&mut s, 0x800A_4BC0, 0);
            (s.ctx.gpr[A0], s.ctx.gpr[A1]) = (sext(o), 0x1E);
            let want = aim_model(&s, 0x1E).unwrap();
            let after = compare("func_8002B3C8", misc::func_8002B3C8, &s).unwrap_or_else(|e| panic!("{e}"));
            same_memory(&after, &want).unwrap();
            found += 1;
            if found == 3 {
                break 'search;
            }
        }
    }
    assert_eq!(found, 3, "too few separators for the double multiply-add");
}
