//! A control-flow graph from N64Recomp's C.
//!
//! N64Recomp writes a conditional branch as
//! `if (cond) { delay; goto L; } delay; ...`: the condition is evaluated
//! before the delay slot, which then runs on both paths. A branch-likely
//! has the delay slot only inside the `if`. Unconditional jumps and calls
//! are followed by a dead copy of their delay slot. The graph built here
//! gives each taken edge its own block for the delay slot, then hoists
//! delay slots that both paths share back above the branch, saving the
//! condition first if the delay slot overwrites one of its registers.

use crate::c::{classify, Line};
use crate::ir::{self, Alu, Cond, Op, Refusal, Val};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Term {
    Goto(usize),
    /// `if cond { goto taken } else { goto fall }`, with the branch's
    /// address and instruction for comments.
    Branch(Cond, usize, usize),
    /// A jump table: `match jr_addend_JR >> 2`, case `k` going to
    /// `cases[k]`, anything else to `default` (a block calling
    /// `switch_error`, then falling through to what follows the switch).
    Switch { jr: u32, cases: Vec<usize>, default: usize },
    Return,
}

#[derive(Clone, Debug)]
pub struct Block {
    /// Address of the block's first instruction.
    pub addr: u32,
    /// The C label this block starts at, if any (`L_...`, `after_N`, `skip_N`).
    pub label: Option<String>,
    pub ops: Vec<(Op, u32)>,
    pub term: Term,
    /// A block made for a taken edge's delay slot.
    synthetic: bool,
}

pub struct Cfg {
    pub name: String,
    pub blocks: Vec<Block>,
    /// Number of saved conditions (`c0`, `c1`, ...).
    pub saved: usize,
}

enum PTerm {
    Goto(String),
    Branch(Cond, String, String),
    Switch { jr: u32, cases: Vec<String>, default: String },
    Return,
}

struct PBlock {
    addr: u32,
    label: Option<String>,
    ops: Vec<(Op, u32)>,
    term: Option<PTerm>,
    synthetic: bool,
}

/// Parse a generated function into a graph. Refuses constructs the
/// translator doesn't handle yet.
pub fn build(src: &str) -> Result<Cfg, Refusal> {
    let mut lines = src.lines();
    let name = loop {
        let l = lines.next().ok_or_else(|| Refusal::new("parse", "no RECOMP_FUNC line"))?;
        if let Some(rest) = l.strip_prefix("RECOMP_FUNC void ") {
            break rest.split('(').next().unwrap().to_string();
        }
    };

    let mut blocks: Vec<PBlock> = Vec::new();
    let mut anon = 0;
    let mut fresh = |blocks: &mut Vec<PBlock>, addr: u32, label: Option<String>, synthetic: bool| {
        let label = label.or_else(|| {
            anon += 1;
            Some(format!("#{anon}"))
        });
        blocks.push(PBlock { addr, label, ops: Vec::new(), term: None, synthetic });
        blocks.len() - 1
    };
    let mut cur = fresh(&mut blocks, 0, Some("#entry".into()), false);
    let mut addr = 0u32;
    let mut first_addr = None;
    // Inside `if (...) {`: the condition, and the ops and target so far.
    let mut in_if: Option<(Cond, Vec<(Op, u32)>, Option<String>, u32)> = None;
    // Inside `switch (...) {`: the `jr`'s address, the case labels so far,
    // and the default's `switch_error` arguments once seen.
    let mut in_switch: Option<(u32, Vec<String>, Option<(u32, u32)>)> = None;
    let jt = |n: usize, what: &str| Refusal::new("jump table", format!("line {}: {what}", n + 2));

    // N64Recomp prints `cop0_status_write(ctx, rN);` without a newline, so
    // the next instruction's `// 0x...` comment ends that line: split it
    // off, since it carries the instruction's address.
    let lines: Vec<(usize, &str)> = lines
        .enumerate()
        .flat_map(|(n, l)| match l.find("    // 0x") {
            Some(i) if !l.trim_start().starts_with("//") => vec![(n, &l[..i]), (n, &l[i..])],
            _ => vec![(n, l)],
        })
        .collect();
    for (n, line) in lines {
        let parsed = classify(line).map_err(|e| {
            let kind = if e.starts_with("jump table") { "jump table" } else { "parse" };
            Refusal::new(kind, format!("line {}: {e}: `{}`", n + 2, line.trim()))
        })?;
        if in_switch.is_some() && !matches!(parsed, Line::Skip | Line::Comment { .. } | Line::Case(..) | Line::Default { .. } | Line::Close) {
            return Err(jt(n, "unexpected line inside a switch"));
        }
        match parsed {
            Line::Skip => {}
            Line::Comment { addr: a, .. } => {
                addr = a;
                first_addr.get_or_insert(a);
            }
            Line::JrAddend { jr, reg } => {
                if in_if.is_some() || in_switch.is_some() {
                    return Err(jt(n, "jr_addend inside an if or switch"));
                }
                blocks[cur].ops.push((Op::JrAddend(jr, reg), addr));
            }
            Line::Switch { jr } => {
                if in_if.is_some() || in_switch.is_some() {
                    return Err(jt(n, "switch inside an if or switch"));
                }
                // The addend's `let` must be in scope: in the same block.
                if !blocks[cur].ops.iter().any(|(o, _)| matches!(o, Op::JrAddend(j, _) if *j == jr)) {
                    return Err(jt(n, "switch without its jr_addend in the same block"));
                }
                in_switch = Some((jr, Vec::new(), None));
            }
            Line::Case(k, l) => {
                let Some((_, cases, None)) = &mut in_switch else { return Err(jt(n, "case outside a switch, or after default")) };
                if k != cases.len() as u64 {
                    return Err(jt(n, "cases out of order"));
                }
                cases.push(l);
            }
            Line::Default { jr, table } => {
                let Some((j, _, d @ None)) = &mut in_switch else { return Err(jt(n, "default outside a switch")) };
                if *j != jr {
                    return Err(jt(n, "default for another jr"));
                }
                *d = Some((jr, table));
            }
            Line::Close if in_switch.is_some() => {
                let (jr, cases, d) = in_switch.take().unwrap();
                let (_, table) = d.ok_or_else(|| jt(n, "switch without default"))?;
                // If switch_error returns, the C carries on after the switch:
                // the jr's dead delay-slot copy, then whatever follows.
                let def = fresh(&mut blocks, jr, None, false);
                blocks[def].ops.push((Op::SwitchError { func: name.clone(), jr, table }, jr));
                let after = fresh(&mut blocks, addr, None, false);
                let after_label = blocks[after].label.clone().unwrap();
                blocks[def].term = Some(PTerm::Goto(after_label));
                let default = blocks[def].label.clone().unwrap();
                end(&mut blocks[cur], PTerm::Switch { jr, cases, default });
                cur = after;
            }
            Line::If(e) => {
                if in_if.is_some() {
                    return Err(Refusal::new("parse", "nested if"));
                }
                // A block that starts with its branch (a label right before
                // it) starts at the branch; its ops stay empty.
                let b = &mut blocks[cur];
                if b.ops.is_empty() && b.term.is_none() {
                    b.addr = addr;
                }
                in_if = Some((ir::cond(&e)?, Vec::new(), None, addr));
            }
            Line::Close => {
                let (cond, ops, target, _) =
                    in_if.take().ok_or_else(|| Refusal::new("parse", format!("line {}: unmatched `}}`", n + 2)))?;
                let target = target.ok_or_else(|| Refusal::new("parse", "if without goto"))?;
                // The taken edge: a block for the delay slot (maybe empty).
                let delay_addr = ops.first().map_or(addr, |o| o.1);
                let t = fresh(&mut blocks, delay_addr, None, true);
                blocks[t].ops = ops;
                blocks[t].term = Some(PTerm::Goto(target));
                let f = fresh(&mut blocks, addr, None, false);
                let tl = blocks[t].label.clone().unwrap();
                let fl = blocks[f].label.clone().unwrap();
                end(&mut blocks[cur], PTerm::Branch(cond, tl, fl));
                cur = f;
            }
            Line::Goto(l) => match &mut in_if {
                Some((_, _, target, _)) => *target = Some(l),
                None => {
                    end(&mut blocks[cur], PTerm::Goto(l));
                    cur = fresh(&mut blocks, addr, None, false);
                }
            },
            Line::Return => {
                if in_if.is_some() {
                    return Err(Refusal::new("parse", "return inside if"));
                }
                end(&mut blocks[cur], PTerm::Return);
                cur = fresh(&mut blocks, addr, None, false);
            }
            Line::Label(l) => {
                let next = fresh(&mut blocks, addr, Some(l.clone()), false);
                end(&mut blocks[cur], PTerm::Goto(l));
                cur = next;
            }
            Line::Stmts(s) => {
                let ops = ir::ops_of_line(&s)?;
                match &mut in_if {
                    Some((_, body, _, _)) => body.extend(ops.into_iter().map(|o| (o, addr))),
                    None => {
                        let b = &mut blocks[cur];
                        if b.ops.is_empty() && b.term.is_none() {
                            b.addr = addr;
                        }
                        b.ops.extend(ops.into_iter().map(|o| (o, addr)));
                    }
                }
            }
        }
    }
    // Falling off the end of the C returns.
    end(&mut blocks[cur], PTerm::Return);
    blocks[0].addr = first_addr.unwrap_or(0);

    // Resolve labels.
    let index: HashMap<String, usize> =
        blocks.iter().enumerate().map(|(i, b)| (b.label.clone().unwrap(), i)).collect();
    let find = |l: &str| index.get(l).copied().ok_or_else(|| Refusal::new("parse", format!("goto to unknown label {l}")));
    let mut out = Vec::new();
    for b in &blocks {
        let term = match b.term.as_ref().unwrap() {
            PTerm::Goto(l) => Term::Goto(find(l)?),
            PTerm::Branch(c, t, f) => Term::Branch(c.clone(), find(t)?, find(f)?),
            PTerm::Switch { jr, cases, default } => Term::Switch {
                jr: *jr,
                cases: cases.iter().map(|l| find(l)).collect::<Result<_, _>>()?,
                default: find(default)?,
            },
            PTerm::Return => Term::Return,
        };
        let label = b.label.clone().filter(|l| !l.starts_with('#'));
        out.push(Block { addr: b.addr, label, ops: b.ops.clone(), term, synthetic: b.synthetic });
    }
    let mut cfg = Cfg { name, blocks: out, saved: 0 };
    cfg.hoist_delay_slots();
    cfg.simplify();
    Ok(cfg)
}

fn end(b: &mut PBlock, t: PTerm) {
    if b.term.is_none() {
        b.term = Some(t);
    }
}

impl Cfg {
    /// Successors, one per edge (a switch lists a target once per case).
    pub fn succs(&self, b: usize) -> Vec<usize> {
        match &self.blocks[b].term {
            Term::Goto(t) => vec![*t],
            Term::Branch(_, t, f) => vec![*t, *f],
            Term::Switch { cases, default, .. } => cases.iter().copied().chain([*default]).collect(),
            Term::Return => vec![],
        }
    }

    fn preds_count(&self) -> Vec<usize> {
        let mut n = vec![0; self.blocks.len()];
        for b in self.reachable() {
            for s in self.succs(b) {
                n[s] += 1;
            }
        }
        n
    }

    pub fn reachable(&self) -> Vec<usize> {
        let mut seen = vec![false; self.blocks.len()];
        let mut stack = vec![0];
        let mut out = Vec::new();
        while let Some(b) = stack.pop() {
            if std::mem::replace(&mut seen[b], true) {
                continue;
            }
            out.push(b);
            stack.extend(self.succs(b));
        }
        out.sort_unstable();
        out
    }

    /// Move a non-likely branch's delay slot above it: the taken edge's
    /// block and the start of the fall-through are the same instructions.
    fn hoist_delay_slots(&mut self) {
        for b in 0..self.blocks.len() {
            let Term::Branch(ref cond, t, f) = self.blocks[b].term else { continue };
            let tb = &self.blocks[t];
            if !tb.synthetic || tb.ops.is_empty() {
                continue;
            }
            let delay = tb.ops.clone();
            let fb = &self.blocks[f];
            if fb.ops.len() < delay.len() || fb.ops[..delay.len()] != delay[..] {
                continue; // a branch-likely: the delay slot runs only when taken
            }
            let Term::Goto(target) = tb.term else { unreachable!() };
            let mut cond = cond.clone();
            let clobbers = delay
                .iter()
                .any(|(o, _)| o.writes().is_some_and(|r| cond.reads(r)) || (o.writes_c1() && cond.reads_c1()));
            if clobbers {
                let n = self.saved;
                self.saved += 1;
                let a = delay[0].1;
                self.blocks[b].ops.push((Op::SaveCond(n, cond), a));
                cond = Cond::Saved(n, false);
            }
            self.blocks[b].ops.extend(delay.iter().cloned());
            self.blocks[f].ops.drain(..delay.len());
            self.blocks[t].ops.clear();
            self.blocks[b].term = Term::Branch(cond, target, f);
        }
    }

    /// Thread jumps through empty blocks, merge straight-line chains, fold
    /// branches whose targets agree and constant pairs.
    fn simplify(&mut self) {
        loop {
            let mut changed = false;
            // Jump threading: an empty block that just jumps elsewhere.
            let forward: Vec<usize> = (0..self.blocks.len())
                .map(|b| {
                    let mut x = b;
                    for _ in 0..self.blocks.len() {
                        match self.blocks[x].term {
                            Term::Goto(t) if self.blocks[x].ops.is_empty() && t != x && x != 0 => x = t,
                            _ => break,
                        }
                    }
                    x
                })
                .collect();
            for b in &mut self.blocks {
                match &mut b.term {
                    Term::Goto(t) => {
                        changed |= forward[*t] != *t;
                        *t = forward[*t];
                    }
                    Term::Branch(_, t, f) => {
                        changed |= forward[*t] != *t || forward[*f] != *f;
                        *t = forward[*t];
                        *f = forward[*f];
                    }
                    Term::Switch { cases, default, .. } => {
                        for t in cases.iter_mut().chain([default]) {
                            changed |= forward[*t] != *t;
                            *t = forward[*t];
                        }
                    }
                    Term::Return => {}
                }
                if let Term::Branch(_, t, f) = b.term {
                    if t == f {
                        b.term = Term::Goto(t);
                        changed = true;
                    }
                }
            }
            // Merge a block into its only predecessor, when that ends in a goto.
            let preds = self.preds_count();
            for b in self.reachable() {
                let Term::Goto(t) = self.blocks[b].term else { continue };
                if t == b || t == 0 || preds[t] != 1 {
                    continue;
                }
                let next = std::mem::replace(&mut self.blocks[t].ops, Vec::new());
                let term = self.blocks[t].term.clone();
                if let Some(l) = self.blocks[t].label.clone().filter(|l| l.starts_with("L_")) {
                    let a = self.blocks[t].addr;
                    self.blocks[b].ops.push((Op::Label(l), a));
                }
                self.blocks[b].ops.extend(next);
                self.blocks[b].term = term;
                // Leave `t` unreachable.
                self.blocks[t].term = Term::Return;
                changed = true;
                break;
            }
            if !changed {
                break;
            }
        }
        for b in &mut self.blocks {
            fold_constants(&mut b.ops);
        }
    }
}

/// `lui r, hi` then `addiu r, r, lo` or `ori r, r, lo`: one constant.
fn fold_constants(ops: &mut Vec<(Op, u32)>) {
    let mut k = 0;
    while k + 1 < ops.len() {
        if let (Op::Li(d, hi), Op::Alu(alu, d2, Val::R(s), Val::I(lo))) = (&ops[k].0, &ops[k + 1].0) {
            if d == d2 && d == s {
                let v = match alu {
                    Alu::Addu => Some(hi.wrapping_add(*lo as i32 as u32)),
                    Alu::Or if (0..=0xFFFF).contains(lo) => Some(hi | *lo as u32),
                    _ => None,
                };
                if let Some(v) = v {
                    ops[k].0 = Op::Li(*d, v);
                    ops.remove(k + 1);
                    continue;
                }
            }
        }
        k += 1;
    }
}
