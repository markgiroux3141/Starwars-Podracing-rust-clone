//! Differential testing harness.
//!
//! A [`State`] is a full machine as N64Recomp functions see it: 8 MB of RDRAM
//! and a `recomp_context`. [`compare`] clones one state twice, runs the
//! recompiled C (from `oracle`) on one copy and the Rust port (from `game`)
//! on the other, and reports every difference in the GPRs, `hi`/`lo`, FPRs,
//! the other context fields and the whole of RDRAM.
//!
//! RDRAM starts out filled with a fixed pseudo-random pattern rather than
//! zeros, so a store of zero (or of any value) is always visible.
//!
//! Calls from the function under test go to the same callee on both sides
//! (NOTES.md, "How ports call other functions"): recompiled C compiled into
//! the oracle, or a stub running a test double ([`oracle::doubles`], and
//! [`rom`] for reading the ROM). Every call through a stub is recorded, and
//! the two runs must make the same calls with the same registers.

pub mod rom;
pub mod world;

use game::recomp::{RecompContext, RecompFn};
use oracle::doubles::{self, Call};
use n64mem::{Rdram, KSEG0};
use std::fmt;
use std::sync::OnceLock;

/// o32 names, for reports.
pub const GPR_NAMES: [&str; 32] = [
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7", "s0", "s1", "s2",
    "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp", "fp", "ra",
];

/// Deterministic xorshift64*: fills memory quickly and reproducibly from a seed.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
}

/// RDRAM filled with the fixed background pattern (built once per process).
fn background() -> &'static Rdram {
    static BG: OnceLock<Rdram> = OnceLock::new();
    BG.get_or_init(|| {
        let mut r = Rdram::new();
        let mut rng = Rng::new(0x5241_4345_5231); // "RACER1"
        for w in r.as_words_mut() {
            *w = rng.next_u32();
        }
        r
    })
}

/// A machine state. The context is boxed so `f_odd` can point into it.
pub struct State {
    pub rdram: Rdram,
    pub ctx: Box<RecompContext>,
    /// Calls through oracle stubs (test doubles) during the last [`State::run`].
    pub calls: Vec<Call>,
}

impl Clone for State {
    fn clone(&self) -> Self {
        let mut s = Self { rdram: self.rdram.clone(), ctx: self.ctx.clone(), calls: self.calls.clone() };
        s.ctx.fix_f_odd();
        s
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    /// Background-pattern RDRAM and an all-zero context.
    pub fn new() -> Self {
        let mut s = Self { rdram: background().clone(), ctx: Box::default(), calls: Vec::new() };
        s.ctx.fix_f_odd();
        s
    }

    /// Randomise every GPR (except `r0`), `hi`, `lo` and every FPR from `seed`.
    /// GPRs are sign-extended 32-bit values, as they are for 32-bit code on
    /// hardware; callers override the argument registers they care about.
    pub fn randomise_registers(&mut self, seed: u64) {
        let mut rng = Rng::new(seed);
        for r in &mut self.ctx.gpr[1..] {
            *r = rng.next_u32() as i32 as i64 as u64;
        }
        self.ctx.hi = rng.next_u32() as i32 as i64 as u64;
        self.ctx.lo = rng.next_u32() as i32 as i64 as u64;
        for f in &mut self.ctx.fpr {
            f.u64 = rng.next_u64();
        }
    }

    /// Fill `len` bytes of RDRAM from `vaddr` (both word-aligned) with values
    /// from `seed`.
    pub fn randomise_memory(&mut self, seed: u64, vaddr: u32, len: u32) {
        assert!(vaddr % 4 == 0 && len % 4 == 0);
        let start = ((vaddr - KSEG0) / 4) as usize;
        let mut rng = Rng::new(seed);
        for w in &mut self.rdram.as_words_mut()[start..start + (len / 4) as usize] {
            *w = rng.next_u32();
        }
    }

    /// Run a recompiled-function-shaped entry point on this state, recording
    /// its calls to test doubles in [`State::calls`].
    pub fn run(&mut self, f: RecompFn) {
        self.ctx.fix_f_odd();
        doubles::start_trace();
        // SAFETY: rdram is a full-size N64Recomp buffer and ctx is exclusive.
        unsafe { f(self.rdram.as_mut_ptr(), &mut *self.ctx) }
        self.calls = doubles::take_trace();
    }
}

/// One observable difference between the C and Rust runs.
#[derive(Debug, PartialEq, Eq)]
pub enum Diff {
    Gpr { reg: usize, c: u64, rust: u64 },
    Hi { c: u64, rust: u64 },
    Lo { c: u64, rust: u64 },
    Fpr { reg: usize, c: u64, rust: u64 },
    StatusReg { c: u32, rust: u32 },
    Mips3FloatMode { c: u8, rust: u8 },
    FOdd,
    /// The first call to a test double that differs (or is missing on one side).
    Call { index: usize, c: Option<Call>, rust: Option<Call> },
    Word { vaddr: u32, c: u32, rust: u32 },
}

impl fmt::Display for Diff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Diff::Gpr { reg, c, rust } => write!(f, "${:<4} C {c:#018x}  Rust {rust:#018x}", GPR_NAMES[reg]),
            Diff::Hi { c, rust } => write!(f, "hi    C {c:#018x}  Rust {rust:#018x}"),
            Diff::Lo { c, rust } => write!(f, "lo    C {c:#018x}  Rust {rust:#018x}"),
            Diff::Fpr { reg, c, rust } => write!(f, "$f{reg:<3} C {c:#018x}  Rust {rust:#018x}"),
            Diff::StatusReg { c, rust } => write!(f, "status_reg C {c:#010x}  Rust {rust:#010x}"),
            Diff::Mips3FloatMode { c, rust } => write!(f, "mips3_float_mode C {c}  Rust {rust}"),
            Diff::FOdd => write!(f, "f_odd no longer points at the context's own f0 high word"),
            Diff::Call { index, ref c, ref rust } => {
                write!(f, "call #{index} to a test double: C {c:?}  Rust {rust:?}")?;
                if let (Some(c), Some(r)) = (c, rust) {
                    for reg in (0..32).filter(|&k| c.gpr[k] != r.gpr[k]) {
                        write!(f, "
      ${:<4} C {:#018x}  Rust {:#018x}", GPR_NAMES[reg], c.gpr[reg], r.gpr[reg])?;
                    }
                }
                Ok(())
            }
            Diff::Word { vaddr, c, rust } => write!(f, "[{vaddr:#010x}] C {c:#010x}  Rust {rust:#010x}"),
        }
    }
}

/// The result of a divergent run: every difference, memory in address order.
#[derive(Debug)]
pub struct Divergence {
    pub name: String,
    pub diffs: Vec<Diff>,
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const SHOW: usize = 40;
        writeln!(f, "{}: C and Rust diverge in {} place(s)", self.name, self.diffs.len())?;
        for d in self.diffs.iter().take(SHOW) {
            writeln!(f, "  {d}")?;
        }
        if self.diffs.len() > SHOW {
            writeln!(f, "  ... {} more", self.diffs.len() - SHOW)?;
        }
        Ok(())
    }
}

impl std::error::Error for Divergence {}

fn f_odd_ok(s: &State) -> bool {
    let expect = (&s.ctx.fpr[0] as *const game::recomp::Fpr).cast::<u32>().wrapping_add(1);
    std::ptr::eq(s.ctx.f_odd.cast_const(), expect)
}

/// Every difference between two states after a run.
pub fn diff_states(c: &State, rust: &State) -> Vec<Diff> {
    let mut out = Vec::new();
    for reg in 0..32 {
        if c.ctx.gpr[reg] != rust.ctx.gpr[reg] {
            out.push(Diff::Gpr { reg, c: c.ctx.gpr[reg], rust: rust.ctx.gpr[reg] });
        }
    }
    if c.ctx.hi != rust.ctx.hi {
        out.push(Diff::Hi { c: c.ctx.hi, rust: rust.ctx.hi });
    }
    if c.ctx.lo != rust.ctx.lo {
        out.push(Diff::Lo { c: c.ctx.lo, rust: rust.ctx.lo });
    }
    for reg in 0..32 {
        // Compare bit patterns: NaN payloads and -0.0 matter.
        if c.ctx.fpr[reg].u64 != rust.ctx.fpr[reg].u64 {
            out.push(Diff::Fpr { reg, c: c.ctx.fpr[reg].u64, rust: rust.ctx.fpr[reg].u64 });
        }
    }
    if c.ctx.status_reg != rust.ctx.status_reg {
        out.push(Diff::StatusReg { c: c.ctx.status_reg, rust: rust.ctx.status_reg });
    }
    if c.ctx.mips3_float_mode != rust.ctx.mips3_float_mode {
        out.push(Diff::Mips3FloatMode { c: c.ctx.mips3_float_mode, rust: rust.ctx.mips3_float_mode });
    }
    if !f_odd_ok(c) || !f_odd_ok(rust) {
        out.push(Diff::FOdd);
    }
    if c.calls != rust.calls {
        let index = (0..).find(|&i| c.calls.get(i) != rust.calls.get(i)).unwrap();
        out.push(Diff::Call { index, c: c.calls.get(index).cloned(), rust: rust.calls.get(index).cloned() });
    }
    let (cw, rw) = (c.rdram.as_words(), rust.rdram.as_words());
    if cw != rw {
        for (i, (&a, &b)) in cw.iter().zip(rw).enumerate() {
            if a != b {
                out.push(Diff::Word { vaddr: KSEG0 + 4 * i as u32, c: a, rust: b });
            }
        }
    }
    out
}

/// The Rust side a test runs for `name`: `port`, or with the `translated`
/// feature the translate crate's draft of `name` (validating the translator
/// against the existing tests). A function the translator refused fails
/// the test with the reason.
pub fn port_under_test(name: &str, port: RecompFn) -> RecompFn {
    #[cfg(feature = "translated")]
    {
        let _ = port;
        if let Some((_, why)) = game::translated::REFUSED.iter().find(|(n, _)| *n == name) {
            panic!("translated: {name} was refused: {why}");
        }
        game::translated::by_name(name).unwrap_or_else(|| panic!("translated: no draft of {name}"))
    }
    #[cfg(not(feature = "translated"))]
    {
        let _ = name;
        port
    }
}

/// Run the oracle's `name` and `port` on copies of `input`; Ok with the
/// post-run state if they agree on everything.
pub fn compare(name: &str, port: RecompFn, input: &State) -> Result<State, Divergence> {
    let port = port_under_test(name, port);
    let c_fn = oracle::recomp::by_name(name)
        .unwrap_or_else(|| panic!("{name} is not compiled into the oracle; add it to crates/oracle/functions.txt"));
    let mut c = input.clone();
    let mut rust = input.clone();
    c.run(c_fn);
    rust.run(port);
    let diffs = diff_states(&c, &rust);
    if diffs.is_empty() {
        Ok(c)
    } else {
        Err(Divergence { name: name.to_string(), diffs })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_reports_registers_and_memory() {
        let a = State::new();
        let mut b = a.clone();
        b.ctx.gpr[2] = 7;
        b.ctx.fpr[3].u64 = 1;
        b.rdram.mem().write_u32(0x8012_3450, !a.rdram.as_words()[(0x12_3450) / 4]);
        let d = diff_states(&a, &b);
        assert_eq!(d.len(), 3);
        assert!(matches!(d[0], Diff::Gpr { reg: 2, .. }));
        assert!(matches!(d[1], Diff::Fpr { reg: 3, .. }));
        assert!(matches!(d[2], Diff::Word { vaddr: 0x8012_3450, .. }));
    }

    #[test]
    fn every_port_is_compiled_into_the_oracle() {
        for p in game::PORTED {
            assert_eq!(p.name, format!("func_{:08X}", p.vram));
            assert!(oracle::recomp::by_name(p.name).is_some(), "{} is ported but not in crates/oracle/functions.txt", p.name);
        }
    }

    #[test]
    fn f_odd_follows_clones() {
        let a = State::new();
        let b = a.clone();
        assert!(f_odd_ok(&a) && f_odd_ok(&b));
    }
}
