//! Rust source from structured code, in the style of the hand-written ports:
//! `g[REG]` for `ctx.gpr`, `m` for RDRAM, `game::recomp` helpers.

use crate::ir::{Alu, Cmp, Cond, Load, MulDiv, Op, Shift, Store, Val};
use crate::structure::S;
use std::collections::BTreeSet;
use std::fmt::Write as _;

pub const REG: [&str; 32] = [
    "ZERO", "AT", "V0", "V1", "A0", "A1", "A2", "A3", "T0", "T1", "T2", "T3", "T4", "T5", "T6", "T7", "S0", "S1", "S2",
    "S3", "S4", "S5", "S6", "S7", "T8", "T9", "K0", "K1", "GP", "SP", "FP", "RA",
];

/// A number the way the ports write them: decimal below 10, else hex with
/// `_` every four digits past four.
pub fn hex(v: u64) -> String {
    if v < 10 {
        return v.to_string();
    }
    let h = format!("{v:X}");
    if h.len() <= 4 {
        return format!("0x{h}");
    }
    let mut out = String::new();
    for (k, c) in h.chars().enumerate() {
        if k > 0 && (h.len() - k) % 4 == 0 {
            out.push('_');
        }
        out.push(c);
    }
    format!("0x{out}")
}

fn off(v: i32) -> String {
    if v < 0 {
        format!("-{}", hex(u64::from(v.unsigned_abs())))
    } else {
        hex(v as u64)
    }
}

/// A `u64` register value.
fn val(v: Val) -> String {
    match v {
        Val::R(r) => format!("g[{}]", REG[r as usize]),
        Val::I(-1) => "u64::MAX".into(),
        Val::I(n) if n < 0 => format!("(-{}i64) as u64", hex(n.unsigned_abs())),
        Val::I(n) => hex(n as u64),
    }
}

fn r(d: u8) -> String {
    format!("g[{}]", REG[d as usize])
}

pub struct Uses {
    pub helpers: BTreeSet<&'static str>,
    pub callees: BTreeSet<String>,
    pub stores: bool,
    pub loads: bool,
    pub gprs: bool,
    pub lohi: bool,
    pub runtime: bool,
}

fn uses(code: &[S], u: &mut Uses) {
    for s in code {
        match s {
            S::Op(o) => op_uses(o, u),
            S::If(c, t, e) => {
                cond_uses(c, u);
                uses(t, u);
                uses(e, u);
            }
            S::Block(_, b) | S::Loop(_, b) => uses(b, u),
            S::Machine(_, arms) => {
                for (_, _, b) in arms {
                    uses(b, u);
                }
            }
            _ => {}
        }
    }
}

fn val_uses(v: Val, u: &mut Uses) {
    if let Val::R(_) = v {
        u.gprs = true;
    }
}

fn cond_uses(c: &Cond, u: &mut Uses) {
    match c {
        Cond::Cmp { a, b, .. } => {
            val_uses(*a, u);
            val_uses(*b, u);
        }
        Cond::And(a, b) => {
            cond_uses(a, u);
            cond_uses(b, u);
        }
        Cond::Not(c) => cond_uses(c, u),
        Cond::Saved(..) => {}
    }
}

fn op_uses(o: &Op, u: &mut Uses) {
    let h = &mut u.helpers;
    match o {
        Op::Load(w, ..) => {
            u.loads = true;
            u.gprs = true;
            h.insert(match w {
                Load::W => "lw",
                Load::H => "lh",
                Load::Hu => "lhu",
                Load::B => "lb",
                Load::Bu => "lbu",
            });
        }
        Op::Store(w, _, _, v) => {
            u.stores = true;
            u.gprs = true;
            val_uses(*v, u);
            u.helpers.insert(match w {
                Store::W => "sw",
                Store::H => "sh",
                Store::B => "sb",
            });
        }
        Op::Li(..) => {
            u.gprs = true;
            h.insert("li");
        }
        Op::Const(..) | Op::Move(..) | Op::MfLo(_) | Op::MfHi(_) => {
            u.gprs = true;
            if matches!(o, Op::MfLo(_) | Op::MfHi(_)) {
                u.lohi = true;
            }
        }
        Op::Alu(a, ..) => {
            u.gprs = true;
            match a {
                Alu::Addu => {
                    h.insert("addu");
                }
                Alu::Subu => {
                    h.insert("subu");
                }
                Alu::Slt => {
                    h.insert("slt");
                }
                Alu::Sltu => {
                    h.insert("sltu");
                }
                _ => {}
            }
        }
        Op::Shift(s, ..) => {
            u.gprs = true;
            h.insert(match s {
                Shift::Sll => "sll",
                Shift::Sra => "sra",
                Shift::Srl => "srl",
            });
        }
        Op::ShiftV(s, ..) => {
            u.gprs = true;
            h.insert(match s {
                Shift::Sll => "sllv",
                Shift::Sra => "srav",
                Shift::Srl => "srlv",
            });
        }
        Op::MulDiv(k, a, b) => {
            u.lohi = true;
            val_uses(*a, u);
            val_uses(*b, u);
            u.helpers.insert(match k {
                MulDiv::Mult => "mult",
                MulDiv::Multu => "multu",
                MulDiv::Div => "div",
                MulDiv::Divu => "divu",
            });
        }
        Op::Call(f) => {
            u.stores = true;
            h.insert("call");
            u.callees.insert(f.clone());
        }
        Op::PauseSelf => {
            u.stores = true;
            u.runtime = true;
        }
        Op::SaveCond(_, c) => cond_uses(c, u),
        Op::Label(_) => {}
    }
}

fn contains_call(code: &[S]) -> bool {
    code.iter().any(|s| match s {
        S::Op(o) => o.is_call() && !matches!(o, Op::PauseSelf),
        S::If(_, t, e) => contains_call(t) || contains_call(e),
        S::Block(_, b) | S::Loop(_, b) => contains_call(b),
        S::Machine(_, arms) => arms.iter().any(|(_, _, b)| contains_call(b)),
        _ => false,
    })
}

/// Whether `s` reads or writes `g`.
fn uses_g(s: &S) -> bool {
    match s {
        S::Op(Op::Call(_) | Op::PauseSelf | Op::Label(_)) => false,
        S::Op(Op::MulDiv(_, a, b)) => matches!(a, Val::R(_)) || matches!(b, Val::R(_)),
        S::Op(_) => true,
        S::Break(_) | S::Continue(_) | S::Return | S::SetPc(_) => false,
        _ => true,
    }
}

/// Marker op for `let g = &mut ctx.gpr;` (never produced by the parser).
fn rebind() -> S {
    S::Op(Op::Label("\u{0}rebind".into()))
}

fn is_rebind(s: &S) -> bool {
    matches!(s, S::Op(Op::Label(l)) if l == "\u{0}rebind")
}

/// Re-borrow `g` after anything that passed `ctx` to a callee, since the
/// callee needs the whole context: after each call, after each construct
/// containing one, and at the top of loop bodies and machine arms with one.
fn insert_rebinds(code: &mut Vec<S>) {
    let mut out = Vec::with_capacity(code.len());
    for mut s in code.drain(..) {
        let mut after = false;
        match &mut s {
            S::Op(o) if matches!(o, Op::Call(_)) => after = true,
            S::If(_, t, e) => {
                insert_rebinds(t);
                insert_rebinds(e);
                after = contains_call(t) || contains_call(e);
            }
            S::Block(_, b) => {
                insert_rebinds(b);
                after = contains_call(b);
            }
            S::Loop(_, b) => {
                insert_rebinds(b);
                if contains_call(b) {
                    b.insert(0, rebind());
                    after = true;
                }
            }
            S::Machine(_, arms) => {
                for (_, _, b) in arms.iter_mut() {
                    insert_rebinds(b);
                    if contains_call(b) {
                        b.insert(0, rebind());
                    }
                }
            }
            _ => {}
        }
        out.push(s);
        if after {
            out.push(rebind());
        }
    }
    // Drop re-borrows nothing uses before the next one or the end.
    let mut k = 0;
    while k < out.len() {
        if is_rebind(&out[k]) {
            let used = out[k + 1..].iter().take_while(|s| !is_rebind(s)).any(uses_g);
            if !used {
                out.remove(k);
                continue;
            }
        }
        k += 1;
    }
    *code = out;
}

/// Labels that must be written: a jump needs its label unless it targets
/// the innermost loop with no labelled block in between.
fn needed_labels(code: &[S], stack: &mut Vec<(bool, String)>, out: &mut BTreeSet<String>) {
    for s in code {
        match s {
            S::Break(l) | S::Continue(l) => {
                if !simple_jump(stack, l) {
                    out.insert(l.clone());
                }
            }
            S::If(_, t, e) => {
                needed_labels(t, stack, out);
                needed_labels(e, stack, out);
            }
            S::Block(l, b) => {
                stack.push((false, l.clone()));
                out.insert(l.clone());
                needed_labels(b, stack, out);
                stack.pop();
            }
            S::Loop(l, b) => {
                stack.push((true, l.clone()));
                needed_labels(b, stack, out);
                stack.pop();
            }
            _ => {}
        }
    }
}

fn simple_jump(stack: &[(bool, String)], l: &str) -> bool {
    matches!(stack.last(), Some((true, top)) if top == l)
}

fn cond(c: &Cond) -> String {
    match c {
        Cond::Saved(n, false) => format!("c{n}"),
        Cond::Saved(n, true) => format!("!c{n}"),
        Cond::And(a, b) => format!("{} && {}", cond(a), cond(b)),
        Cond::Not(c) => format!("!({})", cond(c)),
        Cond::Cmp { cmp, signed, a, b } => {
            let op = match cmp {
                Cmp::Eq => "==",
                Cmp::Ne => "!=",
                Cmp::Lt => "<",
                Cmp::Le => "<=",
                Cmp::Gt => ">",
                Cmp::Ge => ">=",
            };
            let side = |v: &Val| match (signed, v) {
                (true, Val::I(n)) => n.to_string(),
                (true, v) => format!("({} as i64)", val(*v)),
                (false, v) => val(*v),
            };
            format!("{} {op} {}", side(a), side(b))
        }
    }
}

fn op(o: &Op) -> String {
    match o {
        Op::Load(w, d, base, o) => {
            let f = match w {
                Load::W => "lw",
                Load::H => "lh",
                Load::Hu => "lhu",
                Load::B => "lb",
                Load::Bu => "lbu",
            };
            format!("{} = {f}(m, {}, {});", r(*d), r(*base), off(*o))
        }
        Op::Store(w, base, o, v) => {
            let f = match w {
                Store::W => "sw",
                Store::H => "sh",
                Store::B => "sb",
            };
            format!("{f}(m, {}, {}, {});", r(*base), off(*o), val(*v))
        }
        Op::Li(d, v) => format!("{} = li({});", r(*d), hex(u64::from(*v))),
        Op::Const(d, n) => format!("{} = {};", r(*d), val(Val::I(*n))),
        Op::Move(d, s) => format!("{} = {};", r(*d), r(*s)),
        Op::Alu(a, d, x, y) => {
            let rhs = match a {
                Alu::Addu => format!("addu({}, {})", val(*x), val(*y)),
                Alu::Subu => format!("subu({}, {})", val(*x), val(*y)),
                Alu::Or => format!("{} | {}", val(*x), val(*y)),
                Alu::And => format!("{} & {}", val(*x), val(*y)),
                Alu::Xor => format!("{} ^ {}", val(*x), val(*y)),
                Alu::Nor => match y {
                    Val::I(0) => format!("!{}", val(*x)),
                    _ => format!("!({} | {})", val(*x), val(*y)),
                },
                Alu::Slt => format!("slt({}, {})", val(*x), val(*y)),
                Alu::Sltu => format!("sltu({}, {})", val(*x), val(*y)),
            };
            format!("{} = {rhs};", r(*d))
        }
        Op::Shift(s, d, x, sa) => {
            let f = match s {
                Shift::Sll => "sll",
                Shift::Sra => "sra",
                Shift::Srl => "srl",
            };
            format!("{} = {f}({}, {sa});", r(*d), val(*x))
        }
        Op::ShiftV(s, d, x, rs) => {
            let f = match s {
                Shift::Sll => "sllv",
                Shift::Sra => "srav",
                Shift::Srl => "srlv",
            };
            format!("{} = {f}({}, {});", r(*d), val(*x), r(*rs))
        }
        Op::MulDiv(k, a, b) => {
            let f = match k {
                MulDiv::Mult => "mult",
                MulDiv::Multu => "multu",
                MulDiv::Div => "div",
                MulDiv::Divu => "divu",
            };
            format!("(lo, hi) = {f}({}, {});", val(*a), val(*b))
        }
        Op::MfLo(d) => format!("{} = lo;", r(*d)),
        Op::MfHi(d) => format!("{} = hi;", r(*d)),
        Op::Call(f) => format!("call(imports::{f}, m, ctx);"),
        Op::PauseSelf => "imports::runtime::pause_self(m.as_mut_ptr());".into(),
        Op::SaveCond(n, c) => format!("let c{n} = {};", cond(c)),
        Op::Label(l) if l == "\u{0}rebind" => "let g = &mut ctx.gpr;".into(),
        Op::Label(l) => format!("// {l}"),
    }
}

struct Writer<'a> {
    out: String,
    labels: &'a BTreeSet<String>,
    stack: Vec<(bool, String)>,
}

impl Writer<'_> {
    fn line(&mut self, depth: usize, s: &str) {
        for _ in 0..depth {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn seq(&mut self, code: &[S], depth: usize) {
        for s in code {
            self.stmt(s, depth);
        }
    }

    fn stmt(&mut self, s: &S, d: usize) {
        match s {
            S::Op(o) => {
                let t = op(o);
                self.line(d, &t);
            }
            S::If(c, t, e) => {
                self.line(d, &format!("if {} {{", cond(c)));
                self.seq(t, d + 1);
                if e.is_empty() {
                    self.line(d, "}");
                } else if let [S::If(..)] = e.as_slice() {
                    // else if
                    let mut inner = Writer { out: String::new(), labels: self.labels, stack: self.stack.clone() };
                    inner.stmt(&e[0], d);
                    let text = inner.out.trim_start().to_string();
                    for _ in 0..d {
                        self.out.push_str("    ");
                    }
                    self.out.push_str("} else ");
                    self.out.push_str(&text);
                } else {
                    self.line(d, "} else {");
                    self.seq(e, d + 1);
                    self.line(d, "}");
                }
            }
            S::Block(l, b) => {
                self.line(d, &format!("'{l}: {{"));
                self.stack.push((false, l.clone()));
                self.seq(b, d + 1);
                self.stack.pop();
                self.line(d, "}");
            }
            S::Loop(l, b) => {
                if self.labels.contains(l) {
                    self.line(d, &format!("'{l}: loop {{"));
                } else {
                    self.line(d, "loop {");
                }
                self.stack.push((true, l.clone()));
                self.seq(b, d + 1);
                self.stack.pop();
                self.line(d, "}");
            }
            S::Break(l) => {
                let t = if simple_jump(&self.stack, l) { "break;".into() } else { format!("break '{l};") };
                self.line(d, &t);
            }
            S::Continue(l) => {
                let t = if simple_jump(&self.stack, l) { "continue;".into() } else { format!("continue '{l};") };
                self.line(d, &t);
            }
            S::Return => self.line(d, "return;"),
            S::SetPc(n) => self.line(d, &format!("pc = {n};")),
            S::Machine(entry, arms) => {
                self.line(d, "// TODO(restructure): irreducible control flow, translated as a state machine.");
                self.line(d, &format!("let mut pc: usize = {entry};"));
                self.line(d, "loop {");
                self.line(d + 1, "match pc {");
                for (n, addr, b) in arms {
                    self.line(d + 2, &format!("{n} => {{ // {addr:#010X}"));
                    self.seq(b, d + 3);
                    self.line(d + 2, "}");
                }
                self.line(d + 2, "_ => unreachable!(),");
                self.line(d + 1, "}");
                self.line(d, "}");
            }
        }
    }
}

/// The function's Rust source, and what it uses.
pub fn function(name: &str, body: &[S], doc: &str) -> (String, Uses) {
    let mut body = body.to_vec();
    let mut u = Uses {
        helpers: BTreeSet::new(),
        callees: BTreeSet::new(),
        stores: false,
        loads: false,
        gprs: false,
        lohi: false,
        runtime: false,
    };
    uses(&body, &mut u);
    insert_rebinds(&mut body);
    let mut labels = BTreeSet::new();
    needed_labels(&body, &mut Vec::new(), &mut labels);

    let mut w = Writer { out: String::new(), labels: &labels, stack: Vec::new() };
    for l in doc.lines() {
        w.line(0, &format!("///{}{l}", if l.is_empty() { "" } else { " " }));
    }
    let empty = body.is_empty() || body == [S::Return];
    let needs_mem = u.loads || u.stores;
    if empty || (!needs_mem && !u.gprs && !u.lohi) {
        w.line(0, &format!("pub unsafe extern \"C\" fn {name}(_rdram: *mut u8, _ctx: *mut RecompContext) {{}}"));
        return (w.out, u);
    }
    u.helpers.insert("enter");
    w.line(0, &format!("pub unsafe extern \"C\" fn {name}(rdram: *mut u8, ctx: *mut RecompContext) {{"));
    let mem = if u.stores { "mut mem" } else if needs_mem { "mem" } else { "_mem" };
    w.line(1, &format!("let ({mem}, ctx) = enter(rdram, ctx);"));
    if u.stores {
        w.line(1, "let m = &mut mem;");
    } else if needs_mem {
        w.line(1, "let m = &mem;");
    }
    if u.gprs || u.lohi {
        let first_uses_g = body.iter().take_while(|s| !is_rebind(s)).any(uses_g);
        if first_uses_g {
            w.line(1, "let g = &mut ctx.gpr;");
        }
    }
    if u.lohi {
        // N64Recomp's hi/lo are locals of the generated function, starting at 0.
        w.line(1, "let (mut lo, mut hi) = (0u64, 0u64);");
    }
    // A trailing `return` is implied.
    if body.last() == Some(&S::Return) {
        body.pop();
    }
    w.seq(&body, 1);
    w.line(0, "}");
    (w.out, u)
}

/// `use` lines for a draft.
pub fn use_lines(u: &Uses) -> String {
    let mut s = String::new();
    if !u.callees.is_empty() || u.runtime {
        writeln!(s, "use crate::imports;").unwrap();
    }
    let mut items: Vec<&str> = u.helpers.iter().copied().collect();
    if u.gprs {
        items.push("reg::*");
    }
    items.push("RecompContext");
    items.sort_by_key(|i| i.to_ascii_lowercase());
    writeln!(s, "use crate::recomp::{{{}}};", items.join(", ")).unwrap();
    s
}
