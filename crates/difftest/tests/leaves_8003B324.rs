//! Leaves at 0x8003B324..0x8003FDCC: the rotated screen quad
//! (game::render) and the nearest-elements query over the pool registry
//! (game::pools). Recompiled C vs Rust, each checked against its statement
//! by a model: the quad's 14 commands and list pointer, the query's count
//! and its three arrays.
//!
//! NaN operands of arithmetic are outside the domain (NAN_CHECK in the
//! oracle); models return None for inputs that would reach one, skipped
//! before the C runs.

// Tests are named after the functions (func_8003B324), capitals included.
#![allow(non_snake_case)]

use difftest::{compare, rom::baserom, State};
use game::recomp::{reg::*, RecompFn};
use game::{pools, render};
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

fn operands(a: f32, b: f32) -> Option<(f32, f32)> {
    (!a.is_nan() && !b.is_nan()).then_some((a, b))
}
fn mul(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a * b)
}
fn add(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a + b)
}
fn sub(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a - b)
}
fn div(a: f32, b: f32) -> Option<f32> {
    operands(a, b).map(|(a, b)| a / b)
}
fn neg(a: f32) -> Option<f32> {
    (!a.is_nan()).then_some(-a)
}

/// `trunc.w.s` as the C cast does it on the host: `0x80000000` for NaN
/// and out of range.
fn trunc(v: f32) -> u32 {
    if v.is_nan() || v >= 2_147_483_648.0 || v < -2_147_483_648.0 {
        0x8000_0000
    } else {
        v as i32 as u32
    }
}

fn edge() -> BoxedStrategy<f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::from_bits(1)),
        (1u32..0x0080_0000).prop_map(f32::from_bits),
        Just(1.0f32),
        Just(-1.0f32),
        (-64i32..64).prop_map(|n| n as f32 + 0.5),
        Just(1.0e19f32),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        any::<u32>().prop_map(f32::from_bits).prop_filter("not NaN", |x| !x.is_nan()),
    ]
    .boxed()
}

fn ordinary() -> BoxedStrategy<f32> {
    prop_oneof![5 => -1.0e3f32..1.0e3f32, 3 => -1.0f32..1.0f32, 2 => (-8i32..8).prop_map(|n| n as f32), 1 => edge()].boxed()
}

// ---- func_8003B324: rotated screen quad ----

const DLP: u32 = 0x8030_3000;
const LIST: u32 = 0x8030_4000;
const SCREEN: u32 = 0x8011_4470;
const TOLERANCE: u32 = 0x800A_AB10;

/// The tolerance as the ROM has it (0.05), read at test time.
fn rom_tolerance() -> f32 {
    let at = (TOLERANCE - 0x8000_0400 + 0x1000) as usize;
    f32::from_bits(u32::from_be_bytes(baserom()[at..at + 4].try_into().unwrap()))
}

struct Quad {
    x: [i32; 4],
    st: [u32; 4],
    sn: f32,
    cs: f32,
    screen: (i16, i16),
    k: f32,
}

/// The eight coordinates before truncation (`+ 0.5` done), vertex by
/// vertex, and whether the path was the aspect-corrected one; optionally
/// with one corrected coordinate's scale-back fused into its `+ 0.5`
/// (`fused = (vertex, 0 for X or 1 for Y)`, the fused mutants' form).
fn coords(q: &Quad, fused: Option<(usize, usize)>) -> Option<(Vec<(f32, f32)>, bool)> {
    let [x0, x1, y0, y1] = q.x;
    let cx = mul(x0.wrapping_add(x1) as f32, 0.5)?;
    let cy = mul(y0.wrapping_add(y1) as f32, 0.5)?;
    let ws = div(q.screen.0 as f32, 320.0)?;
    let hs = div(q.screen.1 as f32, 240.0)?;
    let t = sub(div(hs, ws)?, 1.0)?;
    let correct = q.k < t || q.k < neg(t)?;
    let msn = neg(q.sn)?;
    let mut out = Vec::new();
    for (v, (x, y)) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].into_iter().enumerate() {
        let (dx, dy) = (sub(x as f32, cx)?, sub(y as f32, cy)?);
        out.push(if correct {
            let (ux, uy) = (div(dx, ws)?, div(dy, hs)?);
            let sx = add(add(mul(ux, q.cs)?, div(cx, ws)?)?, mul(uy, q.sn)?)?;
            let sy = add(add(mul(uy, q.cs)?, div(cy, hs)?)?, mul(ux, msn)?)?;
            let back = |s: f32, k: f32, axis: usize| if fused == Some((v, axis)) { Some(s.mul_add(k, 0.5)) } else { add(mul(s, k)?, 0.5) };
            (back(sx, ws, 0)?, back(sy, hs, 1)?)
        } else {
            let px = add(add(add(mul(dx, q.cs)?, cx)?, mul(dy, q.sn)?)?, 0.5)?;
            let py = add(add(add(mul(dy, q.cs)?, cy)?, mul(dx, msn)?)?, 0.5)?;
            (px, py)
        });
    }
    Some((out, correct))
}

/// The 28 words of the statement, or None.
fn quad(q: &Quad) -> Option<Vec<u32>> {
    let xy: Vec<u32> = coords(q, None)?.0.iter().map(|&(px, py)| trunc(px) << 16 | (trunc(py) & 0xFFFF)).collect();
    let [sa, sb, ta, tb] = q.st;
    let st = |s: u32, t: u32| s << 16 | (t & 0xFFFF);
    let mut w = vec![0x0100_4008, 0x800A_4920];
    for k in 0..4 {
        w.extend([0x021C_0000 | 2 * k, 0]);
    }
    for k in 0..4 {
        w.extend([0x0218_0000 | 2 * k as u32, xy[k]]);
    }
    for (k, v) in [st(sa, ta), st(sb, ta), st(sb, tb), st(sa, tb)].into_iter().enumerate() {
        w.extend([0x0214_0000 | 2 * k as u32, v]);
    }
    w.extend([0x0600_0402, 0x0000_0604]);
    Some(w)
}

/// A quad on which fusing coordinate `(vertex, axis)`'s scale-back into
/// its `+ 0.5` changes the truncated value. That needs `fl(S * k)` to round
/// to `-(m + 0.5)` with `m` a power of two while the exact product lies
/// above it by more than half the float spacing just above `-m` (finer
/// than at `-(m + 0.5)`): then `fl(S * k) + 0.5 = -m` truncates to `-m` but
/// the fused sum, just above, to `-(m - 1)`. Random inputs essentially
/// never do that. Here the screen makes `ws`, `hs` full-mantissa (a
/// factor like 1.25 has too few bits for the product to land in that
/// window) and the path corrected, `sn = 0`, and a scan over consecutive
/// floats of `cs` walks the coordinate through `-m`.
fn quad_separator(vertex: usize, axis: usize) -> Quad {
    for screen in [(373i16, 250i16), (333, 280), (401, 250), (365, 262)] {
        for x in [[-8, 0, -8, 0], [-9, 1, -9, 1], [-7, 3, -7, 3], [-10, -2, -10, -2]] {
            for m in [4.0f32, 2.0, 8.0] {
                if let Some(q) = quad_separator_at(vertex, axis, screen, x, m) {
                    return q;
                }
            }
        }
    }
    panic!("no separator for vertex {vertex}, axis {axis}");
}

fn quad_separator_at(vertex: usize, axis: usize, screen: (i16, i16), x: [i32; 4], target: f32) -> Option<Quad> {
    let at = |cs: f32| Quad { x, st: [0; 4], sn: 0.0, cs, screen, k: 0.05 };
    let value = |cs: f32| -> Option<f32> {
        let (c, correct) = coords(&at(cs), None)?;
        correct.then_some(if axis == 0 { c[vertex].0 } else { c[vertex].1 } + target)
    };
    let trunc_of = |q: &Quad, fused| coords(q, fused).map(|(c, _)| trunc(if axis == 0 { c[vertex].0 } else { c[vertex].1 }));
    // Coarse: brackets where the coordinate crosses -target; bisect each
    // to adjacent floats, then scan consecutive floats of cs around it.
    let mut lo = -4.0f32;
    while lo < 4.0 {
        let hi = lo + 1.0 / 64.0;
        let (vl, vh) = (value(lo)?, value(hi)?);
        if (vl < 0.0) != (vh < 0.0) {
            let (mut a, mut b) = (lo, hi);
            for _ in 0..64 {
                let mid = 0.5 * (a + b);
                if mid == a || mid == b {
                    break;
                }
                if (value(mid)? < 0.0) == (vl < 0.0) {
                    a = mid;
                } else {
                    b = mid;
                }
            }
            for d in -20_000i32..20_000 {
                let q = at(f32::from_bits((a.to_bits() as i32 + d) as u32));
                if let (Some(p), Some(f)) = (trunc_of(&q, None), trunc_of(&q, Some((vertex, axis)))) {
                    if p != f && coords(&q, None).is_some_and(|c| c.1) {
                        return Some(q);
                    }
                }
            }
        }
        lo = hi;
    }
    None
}

/// The separators for all eight coordinates, C vs Rust.
#[test]
fn func_8003B324_fused_separators() {
    for vertex in 0..4 {
        for axis in 0..2 {
            let q = quad_separator(vertex, axis);
            let mut s = state((vertex * 2 + axis) as u64);
            {
                let mut m = s.rdram.mem();
                m.write_u32(DLP, LIST);
                m.write_u16(SCREEN, q.screen.0 as u16);
                m.write_u16(SCREEN + 2, q.screen.1 as u16);
                m.write_f32(TOLERANCE, q.k);
                for (i, w) in [q.x[3] as u32, 0, 0, 0, 0, q.sn.to_bits(), q.cs.to_bits()].iter().enumerate() {
                    m.write_u32(SP_AT + 0x10 + 4 * i as u32, *w);
                }
            }
            (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(DLP), sext(q.x[0] as u32), sext(q.x[1] as u32), sext(q.x[2] as u32));
            let after = compare("func_8003B324", render::func_8003B324, &s).unwrap_or_else(|d| panic!("vertex {vertex}, axis {axis}: {d}"));
            let got: Vec<u32> = (0..28).map(|i| word(&after, LIST + 4 * i)).collect();
            assert_eq!(got, quad(&q).unwrap(), "vertex {vertex}, axis {axis}");
        }
    }
}

fn coord() -> BoxedStrategy<i32> {
    prop_oneof![6 => 0i32..640, 2 => -64i32..720, 1 => any::<i32>()].boxed()
}

fn angle() -> BoxedStrategy<(f32, f32)> {
    prop_oneof![
        4 => (0.0f32..std::f32::consts::TAU).prop_map(|a| (a.sin(), a.cos())),
        1 => Just((0.0f32, 1.0f32)),
        1 => Just((1.0f32, 0.0f32)),
        1 => (ordinary(), ordinary()),
    ]
    .boxed()
}

/// Screen sizes: the game's, ones that give r - 1 = ±0.5 exactly (a tie
/// with a tolerance of 0.5), and arbitrary ones.
fn screen() -> BoxedStrategy<(i16, i16)> {
    prop_oneof![
        3 => (prop::sample::select(vec![320i16, 640]), prop::sample::select(vec![240i16, 480])),
        1 => prop::sample::select(vec![(320i16, 360i16), (320, 120), (640, 720)]),
        2 => (1i16..1000, 1i16..1000),
        1 => (any::<i16>(), any::<i16>()),
    ]
    .boxed()
}

fn tolerance() -> BoxedStrategy<f32> {
    prop_oneof![4 => Just(rom_tolerance()), 1 => Just(0.5f32), 1 => Just(0.0f32), 1 => ordinary()].boxed()
}

// ---- func_8003FDCC: nearest elements ----

const REGISTRY: u32 = 0x8030_5000;
const DESCS: u32 = 0x8030_5100;
const ELEMS: u32 = 0x8030_6000;
const DIST: u32 = 0x8030_A000;
const DELTA: u32 = 0x8030_A100;
const FOUND: u32 = 0x8030_A300;
const POS: u32 = 0x8030_A400;
const ALL: u32 = 0x416C_6C21;
const IDS: [u32; 3] = [0x5465_7374, 0x506F_6473, 0x4E6F_6E65];

#[derive(Clone, Debug)]
struct Pool {
    id: u32,
    flags: u32,
    stride: u32,
    elems: Vec<(u16, [f32; 3])>,
}

fn elem_at(p: usize, k: usize, stride: u32) -> u32 {
    ELEMS + 0x800 * p as u32 + k as u32 * stride
}

/// The count and the three arrays (`cap` entries each, or none), or None.
fn nearest(pools: &[Pool], id: u32, pos: [f32; 3], max: f32, skip: u32, cap: i32, junk: &[u32]) -> Option<(u32, Vec<u32>, Vec<u32>, Vec<u32>)> {
    let c = cap.max(0) as usize;
    let mut dist: Vec<u32> = junk[..c].to_vec();
    let mut delta: Vec<u32> = junk[8..8 + 3 * c].to_vec();
    let mut found: Vec<u32> = junk[32..32 + c].to_vec();
    let mut n = 0i32;
    for (p, pool) in pools.iter().enumerate() {
        if !(pool.id == id || id == ALL) || pool.flags & 1 == 0 {
            continue;
        }
        for (k, &(fl, e)) in pool.elems.iter().enumerate() {
            let at = elem_at(p, k, pool.stride);
            if fl & 0x100 != 0 || at == skip {
                continue;
            }
            let (dx, dy, dz) = (sub(e[0], pos[0])?, sub(e[1], pos[1])?, sub(e[2], pos[2])?);
            let q = add(mul(dz, dz)?, add(mul(dx, dx)?, mul(dy, dy)?)?)?;
            if !(q < max) {
                continue;
            }
            let k = (0..n as usize).find(|&i| !(f32::from_bits(dist[i]) < q)).unwrap_or(n as usize) as i32;
            if k >= cap {
                continue;
            }
            let last = if n < cap {
                n += 1;
                n - 1
            } else {
                cap - 1
            } as usize;
            let k = k as usize;
            for j in (k + 1..=last).rev() {
                found[j] = found[j - 1];
                dist[j] = dist[j - 1];
                for a in (0..3).rev() {
                    delta[3 * j + a] = delta[3 * (j - 1) + a];
                }
            }
            dist[k] = q.to_bits();
            found[k] = at;
            delta[3 * k..3 * k + 3].copy_from_slice(&[dx.to_bits(), dy.to_bits(), dz.to_bits()]);
        }
    }
    Some((n as u32, dist, delta, found))
}

/// Positions: small integers, unit vectors (all at distance 1 from the
/// origin, so distances tie), or anything.
fn position() -> BoxedStrategy<[f32; 3]> {
    prop_oneof![
        2 => prop::array::uniform3((-4i32..=4).prop_map(|n| n as f32)),
        2 => prop::sample::select(vec![[1.0f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 0.0]]),
        3 => prop::array::uniform3(ordinary()),
    ]
    .boxed()
}

fn pool() -> BoxedStrategy<Pool> {
    (
        prop_oneof![Just(IDS[0]), Just(IDS[1]), Just(IDS[2])],
        prop_oneof![3 => Just(1u32), 1 => Just(0u32), 1 => any::<u32>()],
        prop_oneof![Just(0x5Cu32), Just(0x60u32), Just(0x80u32)],
        prop::collection::vec((prop_oneof![3 => Just(0u16), 1 => Just(0x100u16), 1 => any::<u16>()], position()), 0..6),
    )
        .prop_map(|(id, flags, stride, elems)| Pool { id, flags, stride, elems })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn func_8003B324(seed: u64, x in prop::array::uniform4(coord()), st: [u32; 4], (sn, cs) in angle(),
                     scr in screen(), k in tolerance(), junk in prop::array::uniform32(any::<u32>())) {
        let q = Quad { x, st, sn, cs, screen: scr, k };
        let want = quad(&q);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(DLP, LIST);
            for (i, w) in junk.iter().enumerate() {
                m.write_u32(LIST + 4 * i as u32, *w);
            }
            m.write_u16(SCREEN, scr.0 as u16);
            m.write_u16(SCREEN + 2, scr.1 as u16);
            m.write_f32(TOLERANCE, k);
            for (i, w) in [x[3] as u32, st[0], st[1], st[2], st[3], sn.to_bits(), cs.to_bits()].iter().enumerate() {
                m.write_u32(SP_AT + 0x10 + 4 * i as u32, *w);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(DLP), sext(x[0] as u32), sext(x[1] as u32), sext(x[2] as u32));
        let after = run("func_8003B324", render::func_8003B324, &s)?;
        let got: Vec<u32> = (0..28).map(|i| word(&after, LIST + 4 * i)).collect();
        prop_assert_eq!(got, want.unwrap());
        prop_assert_eq!(word(&after, DLP), LIST + 0x70);
        prop_assert_eq!(word(&after, LIST + 0x70), junk[28]);
    }

    #[test]
    fn func_8003FDCC(seed: u64, pools in prop::collection::vec(pool(), 0..4),
                     id in prop_oneof![Just(IDS[0]), Just(IDS[1]), Just(ALL), Just(0u32)],
                     pos in position(), max in prop_oneof![Just(f32::INFINITY), Just(1.0e30f32), Just(2.0f32), Just(8.0f32), 0.0f32..1.0e6, ordinary()],
                     skip_pick in prop::option::of((0usize..4, 0usize..6)), cap in -1i32..=6,
                     junk in prop::array::uniform32(any::<u32>()), more in prop::array::uniform8(any::<u32>())) {
        let junk: Vec<u32> = junk.iter().chain(more.iter()).copied().collect();
        let skip = skip_pick
            .filter(|&(p, k)| p < pools.len() && k < pools[p].elems.len())
            .map_or(0, |(p, k)| elem_at(p, k, pools[p].stride));
        let want = nearest(&pools, id, pos, max, skip, cap, &junk);
        prop_assume!(want.is_some());
        let mut s = state(seed);
        {
            let mut m = s.rdram.mem();
            m.write_u32(pools::POOLS, REGISTRY);
            for (p, pool) in pools.iter().enumerate() {
                let d = DESCS + 0x40 * p as u32;
                m.write_u32(REGISTRY + 4 * p as u32, d);
                m.write_u32(d, pool.id);
                m.write_u32(d + 4, pool.flags);
                m.write_u32(d + 8, pool.elems.len() as u32);
                m.write_u32(d + 0xC, pool.stride);
                m.write_u32(d + 0x10, elem_at(p, 0, pool.stride));
                for (k, (fl, e)) in pool.elems.iter().enumerate() {
                    let at = elem_at(p, k, pool.stride);
                    m.write_u16(at + 6, *fl);
                    for a in 0..3 {
                        m.write_f32(at + 0x50 + 4 * a as u32, e[a]);
                    }
                }
            }
            m.write_u32(REGISTRY + 4 * pools.len() as u32, 0);
            for (i, w) in junk.iter().enumerate() {
                let (base, k) = match i {
                    0..=7 => (DIST, i),
                    8..=31 => (DELTA, i - 8),
                    _ => (FOUND, i - 32),
                };
                m.write_u32(base + 4 * k as u32, *w);
            }
            for a in 0..3 {
                m.write_f32(POS + 4 * a as u32, pos[a]);
            }
            for (i, w) in [cap as u32, DIST, DELTA, FOUND].iter().enumerate() {
                m.write_u32(SP_AT + 0x10 + 4 * i as u32, *w);
            }
        }
        (s.ctx.gpr[A0], s.ctx.gpr[A1], s.ctx.gpr[A2], s.ctx.gpr[A3]) = (sext(id), sext(POS), sext(max.to_bits()), sext(skip));
        let after = run("func_8003FDCC", pools::func_8003FDCC, &s)?;
        let (n, dist, delta, found) = want.unwrap();
        prop_assert_eq!(after.ctx.gpr[V0], u64::from(n));
        let read = |base: u32, len: usize| (0..len as u32).map(|i| word(&after, base + 4 * i)).collect::<Vec<u32>>();
        prop_assert_eq!(read(DIST, dist.len()), dist);
        prop_assert_eq!(read(DELTA, delta.len()), delta);
        prop_assert_eq!(read(FOUND, found.len()), found);
    }
}
