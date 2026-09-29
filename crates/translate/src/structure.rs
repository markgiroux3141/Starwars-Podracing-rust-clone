//! Structured control flow from the graph.
//!
//! Reducible graphs go through Ramsey's algorithm ("Beyond Relooper:
//! recursive translation of unstructured control flow to structured control
//! flow", ICFP 2022), which maps onto Rust directly: a forward jump to a
//! merge point is `break 'b` out of a labelled block, a backward jump is
//! `continue 'l` of a labelled loop, and everything else nests along the
//! dominator tree. Simplification passes then remove jumps that only fall
//! through, labels nobody uses, and move loop exits after their loops.
//!
//! Irreducible graphs (none so far) get a `match` state machine instead.

use crate::cfg::{Cfg, Term};
use crate::ir::{Cond, Op};

/// Structured code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum S {
    Op(Op),
    If(Cond, Vec<S>, Vec<S>),
    Block(String, Vec<S>),
    Loop(String, Vec<S>),
    Break(String),
    Continue(String),
    /// A jump table: `match jr_addend_JR >> 2 { ks => arm, .. _ => default }`,
    /// cases with the same target sharing an arm.
    Match(u32, Vec<(Vec<u64>, Vec<S>)>, Vec<S>),
    Return,
    /// State machine fallback: `pc = n`.
    SetPc(usize),
    /// State machine fallback: `loop { match pc { n => { .. } } }`.
    Machine(usize, Vec<(usize, u32, Vec<S>)>),
}

pub struct Structured {
    pub body: Vec<S>,
    /// False when the graph was irreducible and the body is a state machine.
    pub reducible: bool,
}

struct Graph<'a> {
    cfg: &'a Cfg,
    /// Reverse postorder number of each block (usize::MAX if unreachable).
    rpo: Vec<usize>,
    idom: Vec<usize>,
    loop_header: Vec<bool>,
    merge: Vec<bool>,
    children: Vec<Vec<usize>>,
}

pub fn structure(cfg: &Cfg) -> Structured {
    let n = cfg.blocks.len();
    // Reverse postorder by iterative DFS.
    let mut post = Vec::new();
    let mut seen = vec![false; n];
    let mut stack = vec![(0usize, 0usize)];
    seen[0] = true;
    while let Some(&mut (b, ref mut k)) = stack.last_mut() {
        let succs = cfg.succs(b);
        if *k < succs.len() {
            let s = succs[*k];
            *k += 1;
            if !seen[s] {
                seen[s] = true;
                stack.push((s, 0));
            }
        } else {
            post.push(b);
            stack.pop();
        }
    }
    let order: Vec<usize> = post.iter().rev().copied().collect();
    let mut rpo = vec![usize::MAX; n];
    for (i, &b) in order.iter().enumerate() {
        rpo[b] = i;
    }
    let mut preds = vec![Vec::new(); n];
    for &b in &order {
        for s in cfg.succs(b) {
            preds[s].push(b);
        }
    }
    // Dominators (Cooper, Harvey, Kennedy).
    let mut idom = vec![usize::MAX; n];
    idom[0] = 0;
    let mut changed = true;
    while changed {
        changed = false;
        for &b in order.iter().skip(1) {
            let mut new = usize::MAX;
            for &p in &preds[b] {
                if idom[p] == usize::MAX {
                    continue;
                }
                new = if new == usize::MAX { p } else { intersect(&idom, &rpo, p, new) };
            }
            if idom[b] != new {
                idom[b] = new;
                changed = true;
            }
        }
    }
    let dominates = |a: usize, mut b: usize| loop {
        if a == b {
            return true;
        }
        if b == 0 {
            return false;
        }
        b = idom[b];
    };
    // Back edges; the graph is reducible if every retreating edge is one.
    let mut loop_header = vec![false; n];
    let mut forward_preds = vec![0usize; n];
    let mut reducible = true;
    for &b in &order {
        for s in cfg.succs(b) {
            if rpo[s] <= rpo[b] {
                if dominates(s, b) {
                    loop_header[s] = true;
                } else {
                    reducible = false;
                }
            } else {
                forward_preds[s] += 1;
            }
        }
    }
    if !reducible {
        return Structured { body: machine(cfg, &order), reducible: false };
    }
    let mut children = vec![Vec::new(); n];
    for &b in order.iter().skip(1) {
        children[idom[b]].push(b);
    }
    let g = Graph { cfg, rpo, idom, loop_header, merge: forward_preds.iter().map(|&k| k >= 2).collect(), children };
    let _ = &g.idom;
    let mut body = g.do_tree(0);
    simplify(&mut body);
    for n in 0..cfg.saved {
        polarity(&mut body, n);
    }
    Structured { body, reducible: true }
}

fn intersect(idom: &[usize], rpo: &[usize], mut a: usize, mut b: usize) -> usize {
    while a != b {
        while rpo[a] > rpo[b] {
            a = idom[a];
        }
        while rpo[b] > rpo[a] {
            b = idom[b];
        }
    }
    a
}

fn block_label(cfg: &Cfg, b: usize) -> String {
    format!("b_{:08X}", cfg.blocks[b].addr)
}

fn loop_label(cfg: &Cfg, b: usize) -> String {
    format!("l_{:08X}", cfg.blocks[b].addr)
}

impl Graph<'_> {
    fn do_tree(&self, x: usize) -> Vec<S> {
        let mut ys: Vec<usize> = self.children[x].iter().copied().filter(|&c| self.merge[c]).collect();
        // Highest reverse postorder number first: it is the outermost block.
        ys.sort_by_key(|&y| std::cmp::Reverse(self.rpo[y]));
        let code = self.node_within(x, &ys);
        if self.loop_header[x] {
            vec![S::Loop(loop_label(self.cfg, x), code)]
        } else {
            code
        }
    }

    fn node_within(&self, x: usize, ys: &[usize]) -> Vec<S> {
        match ys.split_first() {
            None => {
                let b = &self.cfg.blocks[x];
                let mut out = Vec::new();
                if let Some(l) = b.label.as_ref().filter(|l| l.starts_with("L_")) {
                    out.push(S::Op(Op::Label(l.clone())));
                }
                out.extend(b.ops.iter().map(|(o, _)| S::Op(o.clone())));
                match b.term {
                    Term::Goto(t) => out.extend(self.do_branch(x, t)),
                    Term::Branch(ref c, t, f) => out.push(S::If(c.clone(), self.do_branch(x, t), self.do_branch(x, f))),
                    Term::Switch { jr, ref cases, default } => {
                        let arms = group_cases(cases).into_iter().map(|(ks, t)| (ks, self.do_branch(x, t))).collect();
                        out.push(S::Match(jr, arms, self.do_branch(x, default)));
                    }
                    Term::Return => out.push(S::Return),
                }
                out
            }
            Some((&y, rest)) => {
                let mut out = vec![S::Block(block_label(self.cfg, y), self.node_within(x, rest))];
                out.extend(self.do_tree(y));
                out
            }
        }
    }

    fn do_branch(&self, from: usize, to: usize) -> Vec<S> {
        if self.rpo[to] <= self.rpo[from] {
            vec![S::Continue(loop_label(self.cfg, to))]
        } else if self.merge[to] {
            vec![S::Break(block_label(self.cfg, to))]
        } else {
            self.do_tree(to)
        }
    }
}

/// Case numbers by target, in order of first appearance.
fn group_cases(cases: &[usize]) -> Vec<(Vec<u64>, usize)> {
    let mut arms: Vec<(Vec<u64>, usize)> = Vec::new();
    for (k, &t) in cases.iter().enumerate() {
        match arms.iter_mut().find(|(_, x)| *x == t) {
            Some((ks, _)) => ks.push(k as u64),
            None => arms.push((vec![k as u64], t)),
        }
    }
    arms
}

/// The fallback for irreducible graphs: one arm per block.
fn machine(cfg: &Cfg, order: &[usize]) -> Vec<S> {
    let mut arms = Vec::new();
    for &b in order {
        let blk = &cfg.blocks[b];
        let mut code: Vec<S> = blk.ops.iter().map(|(o, _)| S::Op(o.clone())).collect();
        match blk.term {
            Term::Goto(t) => code.push(S::SetPc(t)),
            Term::Branch(ref c, t, f) => code.push(S::If(c.clone(), vec![S::SetPc(t)], vec![S::SetPc(f)])),
            Term::Switch { jr, ref cases, default } => code.push(S::Match(
                jr,
                group_cases(cases).into_iter().map(|(ks, t)| (ks, vec![S::SetPc(t)])).collect(),
                vec![S::SetPc(default)],
            )),
            Term::Return => code.push(S::Return),
        }
        arms.push((b, blk.addr, code));
    }
    vec![S::Machine(0, arms)]
}

// ---------------------------------------------------------------------------
// Simplification

/// Whether control can reach the end of `code` (it doesn't end in a jump).
fn falls_through(code: &[S]) -> bool {
    match code.last() {
        None => true,
        Some(S::Break(_) | S::Continue(_) | S::Return | S::SetPc(_)) => false,
        Some(S::If(_, t, e)) => falls_through(t) || falls_through(e),
        Some(S::Match(_, arms, d)) => arms.iter().any(|(_, a)| falls_through(a)) || falls_through(d),
        // A loop only ends through a `break` to its own label; blocks can end
        // by breaking out, which falls through to what follows.
        Some(S::Loop(l, body)) => mentions_break(body, l),
        Some(S::Block(..)) | Some(S::Op(_)) => true,
        Some(S::Machine(..)) => false,
    }
}

fn mentions_break(code: &[S], label: &str) -> bool {
    code.iter().any(|s| match s {
        S::Break(l) => l == label,
        S::If(_, t, e) => mentions_break(t, label) || mentions_break(e, label),
        S::Match(_, arms, d) => arms.iter().any(|(_, a)| mentions_break(a, label)) || mentions_break(d, label),
        S::Block(_, b) | S::Loop(_, b) => mentions_break(b, label),
        _ => false,
    })
}

fn mentions(code: &[S], label: &str) -> bool {
    code.iter().any(|s| match s {
        S::Break(l) | S::Continue(l) => l == label,
        S::If(_, t, e) => mentions(t, label) || mentions(e, label),
        S::Match(_, arms, d) => arms.iter().any(|(_, a)| mentions(a, label)) || mentions(d, label),
        S::Block(_, b) | S::Loop(_, b) => mentions(b, label),
        _ => false,
    })
}

fn simplify(code: &mut Vec<S>) {
    for _ in 0..100 {
        let before = code.clone();
        hoist_loop_exits(code);
        strip_tails(code, &[S::Return]);
        tidy(code);
        if *code == before {
            return;
        }
    }
}

/// Remove jumps at the end of `code` that go where falling off the end
/// would go anyway (`equiv`).
fn strip_tails(code: &mut Vec<S>, equiv: &[S]) {
    // Inner sequences first, each with its own fall-through.
    let n = code.len();
    for (k, s) in code.iter_mut().enumerate() {
        let last = k + 1 == n;
        match s {
            S::If(_, t, e) => {
                let eq: &[S] = if last { equiv } else { &[] };
                strip_tails(t, eq);
                strip_tails(e, eq);
            }
            S::Match(_, arms, d) => {
                let eq: &[S] = if last { equiv } else { &[] };
                for (_, a) in arms.iter_mut() {
                    strip_tails(a, eq);
                }
                strip_tails(d, eq);
            }
            S::Block(l, body) => {
                let mut eq = vec![S::Break(l.clone())];
                if last {
                    eq.extend(equiv.iter().cloned());
                }
                strip_tails(body, &eq);
            }
            S::Loop(l, body) => strip_tails(body, &[S::Continue(l.clone())]),
            _ => {}
        }
    }
    while matches!(code.last(), Some(s) if equiv.contains(s)) {
        code.pop();
    }
}

/// A loop body ending in `if c { continue } else { X }` (or with an empty
/// branch in place of the `continue`, after stripping) becomes
/// `if !c { break }` with `X` after the loop, so exit code isn't nested
/// inside it.
fn hoist_loop_exits(code: &mut Vec<S>) {
    let mut k = 0;
    while k < code.len() {
        let last = k + 1 == code.len();
        let mut moved = None;
        match &mut code[k] {
            S::If(_, t, e) => {
                hoist_loop_exits(t);
                hoist_loop_exits(e);
            }
            S::Match(_, arms, d) => {
                for (_, a) in arms.iter_mut() {
                    hoist_loop_exits(a);
                }
                hoist_loop_exits(d);
            }
            S::Block(_, b) => hoist_loop_exits(b),
            S::Loop(l, body) => {
                hoist_loop_exits(body);
                let l = l.clone();
                let is_cont = |b: &[S]| b.is_empty() || b == [S::Continue(l.clone())];
                // Only if nothing follows the loop (it never falls through),
                // and one exit per loop.
                if !last || mentions_break(body, &l) {
                    k += 1;
                    continue;
                }
                // The exit branch must not fall through: inside the loop,
                // falling off it goes round again (a trailing `continue` may
                // already have been stripped), after the loop it would leave.
                let mut found = None;
                if let Some(S::If(c, t, e)) = body.last() {
                    if is_cont(t) && !e.is_empty() && !mentions(e, &l) && !falls_through(e) {
                        found = Some((body.len() - 1, c.negate(), e.clone()));
                    } else if is_cont(e) && !t.is_empty() && !mentions(t, &l) && !falls_through(t) {
                        found = Some((body.len() - 1, c.clone(), t.clone()));
                    }
                }
                // Or an early exit anywhere at the top of the body:
                // `if c { X }` where X never falls through.
                if found.is_none() {
                    found = body.iter().enumerate().find_map(|(i, s)| match s {
                        S::If(c, t, e) if e.is_empty() && !t.is_empty() && !falls_through(t) && !mentions(t, &l) => {
                            Some((i, c.clone(), t.clone()))
                        }
                        _ => None,
                    });
                }
                if let Some((i, cond, exit)) = found {
                    body[i] = S::If(cond, vec![S::Break(l.clone())], vec![]);
                    moved = Some(exit);
                }
            }
            _ => {}
        }
        if let Some(exit) = moved {
            code.splice(k + 1..k + 1, exit);
        }
        k += 1;
    }
}

/// Local rewrites: drop empty ifs, flip ifs with an empty then-branch, pull
/// the rest out of an else after a branch that jumps away, merge a block
/// ending in another block, and splice blocks nobody breaks to.
fn tidy(code: &mut Vec<S>) {
    let mut k = 0;
    while k < code.len() {
        match &mut code[k] {
            S::If(_, t, e) => {
                tidy(t);
                tidy(e);
            }
            S::Match(_, arms, d) => {
                for (_, a) in arms.iter_mut() {
                    tidy(a);
                }
                tidy(d);
            }
            S::Block(_, b) | S::Loop(_, b) => tidy(b),
            _ => {}
        }
        // Branches holding only label comments count as empty.
        if let S::If(_, t, e) = &mut code[k] {
            for b in [t, e] {
                if !b.is_empty() && b.iter().all(|s| matches!(s, S::Op(Op::Label(_)))) {
                    b.clear();
                }
            }
        }
        let s = code[k].clone();
        match s {
            S::If(c, mut t, e) if e.is_empty() && t.len() == 1 && matches!(&t[0], S::If(_, _, e2) if e2.is_empty()) => {
                // if a { if b { .. } }: if a && b { .. }
                let Some(S::If(c2, t2, _)) = t.pop() else { unreachable!() };
                code[k] = S::If(Cond::And(Box::new(c), Box::new(c2)), t2, vec![]);
                continue;
            }
            S::If(c, t, e) if t.is_empty() && e.is_empty() => {
                // Conditions have no side effects.
                let _ = c;
                code.remove(k);
                continue;
            }
            S::If(c, t, e) if t.is_empty() => {
                code[k] = S::If(c.negate(), e, t);
                continue;
            }
            S::If(c, t, e) if !e.is_empty() && !falls_through(&t) => {
                code[k] = S::If(c, t, vec![]);
                code.splice(k + 1..k + 1, e);
            }
            S::If(c, t, e) if !e.is_empty() && !falls_through(&e) && falls_through(&t) => {
                code[k] = S::If(c.negate(), e, vec![]);
                code.splice(k + 1..k + 1, t);
            }
            S::Block(outer, mut body) if matches!(body.last(), Some(S::Block(..))) => {
                // 'a: { ..; 'b: { .. } }: breaking to 'b goes where 'a's end goes.
                let Some(S::Block(inner, ib)) = body.pop() else { unreachable!() };
                let ib = rename(ib, &inner, &outer);
                body.extend(ib);
                code[k] = S::Block(outer, body);
                continue;
            }
            S::Block(l, body) if !mentions(&body, &l) => {
                code.splice(k..k + 1, body);
                continue;
            }
            _ => {}
        }
        k += 1;
    }
}

fn saved_uses(code: &[S], n: usize, pos: &mut usize, neg: &mut usize) {
    fn walk(c: &Cond, n: usize, pos: &mut usize, neg: &mut usize) {
        match c {
            Cond::Saved(m, false) if *m == n => *pos += 1,
            Cond::Saved(m, true) if *m == n => *neg += 1,
            Cond::And(a, b) => {
                walk(a, n, pos, neg);
                walk(b, n, pos, neg);
            }
            Cond::Not(c) => walk(c, n, pos, neg),
            _ => {}
        }
    }
    for s in code {
        match s {
            S::If(c, t, e) => {
                walk(c, n, pos, neg);
                saved_uses(t, n, pos, neg);
                saved_uses(e, n, pos, neg);
            }
            S::Match(_, arms, d) => {
                for (_, a) in arms {
                    saved_uses(a, n, pos, neg);
                }
                saved_uses(d, n, pos, neg);
            }
            S::Block(_, b) | S::Loop(_, b) => saved_uses(b, n, pos, neg),
            _ => {}
        }
    }
}

fn flip_saved(code: &mut [S], n: usize) {
    fn walk(c: &mut Cond, n: usize) {
        match c {
            Cond::Saved(m, neg) if *m == n => *neg = !*neg,
            Cond::And(a, b) => {
                walk(a, n);
                walk(b, n);
            }
            Cond::Not(c) => walk(c, n),
            _ => {}
        }
    }
    for s in code {
        match s {
            S::Op(Op::SaveCond(m, c)) if *m == n => *c = c.negate(),
            S::If(c, t, e) => {
                walk(c, n);
                flip_saved(t, n);
                flip_saved(e, n);
            }
            S::Match(_, arms, d) => {
                for (_, a) in arms.iter_mut() {
                    flip_saved(a, n);
                }
                flip_saved(d, n);
            }
            S::Block(_, b) | S::Loop(_, b) => flip_saved(b, n),
            _ => {}
        }
    }
}

/// Save condition `n` the way round its uses read it: `let c0 = a == b;
/// if c0` rather than `let c0 = a != b; if !c0`.
fn polarity(code: &mut [S], n: usize) {
    let (mut pos, mut neg) = (0, 0);
    saved_uses(code, n, &mut pos, &mut neg);
    if neg > pos {
        flip_saved(code, n);
    }
}

fn rename(code: Vec<S>, from: &str, to: &str) -> Vec<S> {
    code.into_iter()
        .map(|s| match s {
            S::Break(l) if l == from => S::Break(to.to_string()),
            S::If(c, t, e) => S::If(c, rename(t, from, to), rename(e, from, to)),
            S::Match(jr, arms, d) => {
                S::Match(jr, arms.into_iter().map(|(ks, a)| (ks, rename(a, from, to))).collect(), rename(d, from, to))
            }
            S::Block(l, b) => S::Block(l, rename(b, from, to)),
            S::Loop(l, b) => S::Loop(l, rename(b, from, to)),
            s => s,
        })
        .collect()
}
