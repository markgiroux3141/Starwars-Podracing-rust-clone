//! Instructions as N64Recomp writes them in C, matched shape by shape.
//!
//! Every accepted shape corresponds to one MIPS instruction and to one
//! helper in `game::recomp` with the same semantics. Anything else is
//! refused with a [`Refusal`] naming what it is, never guessed at.

use crate::c::E;

/// Why a function can't be translated (yet).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// A short category for surveys: "float", "FCR31", "jump table", ...
    pub kind: &'static str,
    pub detail: String,
}

impl Refusal {
    pub fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        Self { kind, detail: detail.into() }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.detail)
    }
}

/// A source operand: a GPR, or a constant (the zero register is 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Val {
    R(u8),
    /// The C literal's value. As a `gpr` operand it is sign-extended to 64
    /// bits, which is what C's usual arithmetic conversions do to a negative
    /// `int` literal meeting a `uint64_t`.
    I(i64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    W,
    H,
    Hu,
    B,
    Bu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    W,
    H,
    B,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alu {
    Addu,
    Subu,
    Or,
    And,
    Xor,
    Nor,
    Slt,
    Sltu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Sll,
    Sra,
    Srl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MulDiv {
    Mult,
    Multu,
    Div,
    Divu,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// `rt = mem[base + off]`.
    Load(Load, u8, u8, i32),
    /// `mem[base + off] = val`.
    Store(Store, u8, i32, Val),
    /// A 32-bit constant, sign-extended (`lui`, or `lui` + `addiu`/`ori`).
    Li(u8, u32),
    /// A small constant (`addiu rd, zero, n`, `or rd, zero, zero`).
    Const(u8, i64),
    /// `or rd, rs, zero`.
    Move(u8, u8),
    Alu(Alu, u8, Val, Val),
    Shift(Shift, u8, Val, u32),
    /// Variable shift by the low 5 bits of a register.
    ShiftV(Shift, u8, Val, u8),
    /// Writes the function's `lo`/`hi` locals (not `ctx->lo`/`ctx->hi`).
    MulDiv(MulDiv, Val, Val),
    MfLo(u8),
    MfHi(u8),
    Call(String),
    /// The runtime's `pause_self`, N64Recomp's translation of `b .`.
    PauseSelf,
    /// `let c<n> = cond;`: a branch condition read before its delay slot
    /// overwrites one of its registers.
    SaveCond(usize, Cond),
    /// A label of the C, kept as a comment where blocks were merged.
    Label(String),
}

impl Op {
    /// The GPR this op writes, if any.
    pub fn writes(&self) -> Option<u8> {
        match *self {
            Op::Load(_, d, ..)
            | Op::Li(d, _)
            | Op::Const(d, _)
            | Op::Move(d, _)
            | Op::Alu(_, d, ..)
            | Op::Shift(_, d, ..)
            | Op::ShiftV(_, d, ..)
            | Op::MfLo(d)
            | Op::MfHi(d) => Some(d),
            _ => None,
        }
    }

    pub fn is_call(&self) -> bool {
        matches!(self, Op::Call(_) | Op::PauseSelf)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Cmp {
    pub fn negate(self) -> Self {
        match self {
            Cmp::Eq => Cmp::Ne,
            Cmp::Ne => Cmp::Eq,
            Cmp::Lt => Cmp::Ge,
            Cmp::Ge => Cmp::Lt,
            Cmp::Le => Cmp::Gt,
            Cmp::Gt => Cmp::Le,
        }
    }
}

/// A branch condition, evaluated before the delay slot runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cond {
    /// `a cmp b`, as signed 64-bit values if `signed` (C's `SIGNED()`),
    /// else as the raw 64-bit registers.
    Cmp { cmp: Cmp, signed: bool, a: Val, b: Val },
    /// A condition saved by [`Op::SaveCond`], possibly negated.
    Saved(usize, bool),
    /// Both (nested ifs merged by the structurer; conditions have no side
    /// effects, so `&&`'s short circuit changes nothing).
    And(Box<Cond>, Box<Cond>),
    Not(Box<Cond>),
}

impl Cond {
    pub fn negate(&self) -> Self {
        match self {
            Cond::Cmp { cmp, signed, a, b } => Cond::Cmp { cmp: cmp.negate(), signed: *signed, a: *a, b: *b },
            Cond::Saved(n, neg) => Cond::Saved(*n, !neg),
            Cond::Not(c) => (**c).clone(),
            c => Cond::Not(Box::new(c.clone())),
        }
    }

    pub fn reads(&self, r: u8) -> bool {
        match self {
            Cond::Cmp { a, b, .. } => *a == Val::R(r) || *b == Val::R(r),
            Cond::Saved(..) => false,
            Cond::And(a, b) => a.reads(r) || b.reads(r),
            Cond::Not(c) => c.reads(r),
        }
    }
}

fn float(e: &E) -> Refusal {
    Refusal::new("float", format!("`{e}`"))
}

/// A GPR or constant operand.
fn val(e: &E) -> Option<Val> {
    match e {
        E::Reg(n) => Some(Val::R(*n)),
        E::Num(v) => Some(Val::I(*v)),
        _ => None,
    }
}

fn reg(e: &E) -> Option<u8> {
    match e {
        E::Reg(n) => Some(*n),
        _ => None,
    }
}

fn call<'a>(e: &'a E, name: &str) -> Option<&'a [E]> {
    match e {
        E::Call(n, a) if n == name => Some(a),
        _ => None,
    }
}

fn call1<'a>(e: &'a E, name: &str) -> Option<&'a E> {
    match call(e, name) {
        Some([a]) => Some(a),
        _ => None,
    }
}

fn bin<'a>(e: &'a E, op: &str) -> Option<(&'a E, &'a E)> {
    match e {
        E::Bin(o, a, b) if *o == op => Some((a, b)),
        _ => None,
    }
}

/// `MEM_x(a, b)`: base register and offset, in either order (N64Recomp
/// writes loads as `(reg, offset)` and stores as `(offset, reg)`).
fn mem(e: &E) -> Option<(&'static str, u8, i32)> {
    let E::Call(n, args) = e else { return None };
    let w = ["MEM_W", "MEM_H", "MEM_HU", "MEM_B", "MEM_BU"].into_iter().find(|m| m == n)?;
    let (base, off) = match args.as_slice() {
        [E::Reg(r), E::Num(o)] | [E::Num(o), E::Reg(r)] => (*r, *o),
        _ => return None,
    };
    Some((w, base, i32::try_from(off).ok()?))
}

/// What a function-like name in the C means, for refusals.
fn refusal_for_name(name: &str, e: &E) -> Refusal {
    match name {
        "CHECK_FR" | "NAN_CHECK" | "CVT_S_W" | "CVT_D_W" | "CVT_D_S" | "CVT_S_D" | "MUL_S" | "MUL_D" | "DIV_S" | "DIV_D"
        | "TRUNC_W_S" | "TRUNC_W_D" | "CVT_W_S" | "CVT_W_D" | "sqrtf" => float(e),
        "get_cop1_cs" | "set_cop1_cs" => Refusal::new("FCR31", format!("`{e}`")),
        "do_break" => Refusal::new("break", format!("`{e}`")),
        "do_lwl" | "do_lwr" | "do_swl" | "do_swr" => Refusal::new("unaligned access", format!("`{e}`")),
        "LD" | "SD" => Refusal::new("64-bit", format!("`{e}`")),
        "switch_error" => Refusal::new("jump table", format!("`{e}`")),
        n if n.starts_with("(LOOKUP_FUNC") => Refusal::new("indirect call", format!("`{e}`")),
        _ => Refusal::new("unknown", format!("`{e}`")),
    }
}

/// Any FPR, FCR31 or other unsupported name inside `e`, for a precise refusal.
fn find_unsupported(e: &E) -> Option<Refusal> {
    match e {
        E::Member(..) => Some(float(e)),
        E::Ident(n) if n == "c1cs" => Some(float(e)),
        E::Index(..) => Some(float(e)),
        E::Call(n, args) => {
            let r = refusal_for_name(n, e);
            if r.kind != "unknown" {
                return Some(r);
            }
            args.iter().find_map(find_unsupported)
        }
        E::Bin(_, a, b) => find_unsupported(a).or_else(|| find_unsupported(b)),
        E::Unary(_, a) | E::Cast(_, a) => find_unsupported(a),
        E::Ternary(a, b, c) => find_unsupported(a).or_else(|| find_unsupported(b)).or_else(|| find_unsupported(c)),
        _ => None,
    }
}

fn unknown(what: &str, e: &str) -> Refusal {
    Refusal::new("unknown", format!("{what} `{e}`"))
}

/// `rd = rhs`.
fn assign_reg(d: u8, rhs: &E) -> Option<Op> {
    if let Some((w, base, off)) = mem(rhs) {
        let w = match w {
            "MEM_W" => Load::W,
            "MEM_H" => Load::H,
            "MEM_HU" => Load::Hu,
            "MEM_B" => Load::B,
            _ => Load::Bu,
        };
        return Some(Op::Load(w, d, base, off));
    }
    if let Some(inner) = call1(rhs, "S32") {
        // lui: S32(0XHHHH << 16)
        if let Some((E::Num(h), E::Num(16))) = bin(inner, "<<") {
            if (0..=0xFFFF).contains(h) {
                return Some(Op::Li(d, (*h as u32) << 16));
            }
        }
        // sll / sllv: S32(x << sa), S32(x << (rs & 31))
        if let Some((x, s)) = bin(inner, "<<") {
            let x = val(x)?;
            return shift(Shift::Sll, d, x, s);
        }
        // sra / srav / srl / srlv
        if let Some((x, s)) = bin(inner, ">>") {
            if let Some(x) = call1(x, "SIGNED") {
                return shift(Shift::Sra, d, val(x)?, s);
            }
            if let Some(x) = call1(x, "U32") {
                return shift(Shift::Srl, d, val(x)?, s);
            }
        }
        return None;
    }
    if let Some([a, b]) = call(rhs, "ADD32") {
        return Some(match (val(a)?, val(b)?) {
            (Val::I(0), Val::I(n)) if i32::try_from(n).is_ok() => Op::Const(d, n),
            (a, b) => Op::Alu(Alu::Addu, d, a, b),
        });
    }
    if let Some([a, b]) = call(rhs, "SUB32") {
        return Some(Op::Alu(Alu::Subu, d, val(a)?, val(b)?));
    }
    if let Some((a, b)) = bin(rhs, "|") {
        return Some(match (val(a)?, val(b)?) {
            (Val::I(0), Val::I(n)) | (Val::I(n), Val::I(0)) => Op::Const(d, n),
            (Val::R(s), Val::I(0)) | (Val::I(0), Val::R(s)) => Op::Move(d, s),
            (a, b) => Op::Alu(Alu::Or, d, a, b),
        });
    }
    if let Some((a, b)) = bin(rhs, "&") {
        return Some(Op::Alu(Alu::And, d, val(a)?, val(b)?));
    }
    if let Some((a, b)) = bin(rhs, "^") {
        return Some(Op::Alu(Alu::Xor, d, val(a)?, val(b)?));
    }
    if let E::Unary("~", inner) = rhs {
        let (a, b) = bin(inner, "|")?;
        return Some(Op::Alu(Alu::Nor, d, val(a)?, val(b)?));
    }
    // slt / slti / sltu / sltiu: `a < b ? 1 : 0`
    if let E::Ternary(c, one, zero) = rhs {
        if **one != E::Num(1) || **zero != E::Num(0) {
            return None;
        }
        let (a, b) = bin(c, "<")?;
        return Some(match (call1(a, "SIGNED"), call1(b, "SIGNED")) {
            (Some(a), Some(b)) => Op::Alu(Alu::Slt, d, val(a)?, val(b)?),
            (Some(a), None) => Op::Alu(Alu::Slt, d, val(a)?, Val::I(num(b)?)),
            (None, None) => Op::Alu(Alu::Sltu, d, val(a)?, val(b)?),
            (None, Some(_)) => return None,
        });
    }
    match rhs {
        E::Ident(n) if n == "lo" => Some(Op::MfLo(d)),
        E::Ident(n) if n == "hi" => Some(Op::MfHi(d)),
        _ => None,
    }
}

fn num(e: &E) -> Option<i64> {
    match e {
        E::Num(v) => Some(*v),
        _ => None,
    }
}

fn shift(kind: Shift, d: u8, x: Val, s: &E) -> Option<Op> {
    match s {
        E::Num(sa) if (0..32).contains(sa) => Some(Op::Shift(kind, d, x, *sa as u32)),
        _ => {
            let (r, mask) = bin(s, "&")?;
            if *mask != E::Num(31) {
                return None;
            }
            Some(Op::ShiftV(kind, d, x, reg(r)?))
        }
    }
}

/// Statements of one C line, as ops. `mult`/`div` span several statements.
pub fn ops_of_line(stmts: &[crate::c::Stmt]) -> Result<Vec<Op>, Refusal> {
    use crate::c::Stmt;
    let mut out = Vec::new();
    let mut k = 0;
    while k < stmts.len() {
        match &stmts[k] {
            Stmt::Assign(lhs, rhs) => {
                if let Some(r) = find_unsupported(lhs).or_else(|| find_unsupported(rhs)) {
                    return Err(r);
                }
                match lhs {
                    E::Reg(d) => {
                        out.push(assign_reg(*d, rhs).ok_or_else(|| unknown("assignment", &format!("{lhs} = {rhs}")))?)
                    }
                    E::Ident(n) if n == "result" || n == "lo" => {
                        let (op, used) = muldiv(&stmts[k..]).ok_or_else(|| unknown("mult/div", &format!("{lhs} = {rhs}")))?;
                        out.push(op);
                        k += used;
                        continue;
                    }
                    _ => {
                        let (w, base, off) = mem(lhs).ok_or_else(|| unknown("store", &format!("{lhs} = {rhs}")))?;
                        let w = match w {
                            "MEM_W" => Store::W,
                            "MEM_H" => Store::H,
                            "MEM_B" => Store::B,
                            _ => return Err(unknown("store", &format!("{lhs} = {rhs}"))),
                        };
                        let v = val(rhs).ok_or_else(|| unknown("stored value", &format!("{rhs}")))?;
                        out.push(Op::Store(w, base, off, v));
                    }
                }
            }
            Stmt::Expr(e) => match e {
                E::Call(n, args)
                    if n.starts_with("func_")
                        && args.as_slice() == [E::Ident("rdram".into()), E::Ident("ctx".into())] =>
                {
                    out.push(Op::Call(n.clone()))
                }
                E::Call(n, args) if n == "pause_self" && args.as_slice() == [E::Ident("rdram".into())] => {
                    out.push(Op::PauseSelf)
                }
                E::Call(n, _) => return Err(refusal_for_name(n, e)),
                _ => return Err(unknown("statement", &format!("{e}"))),
            },
        }
        k += 1;
    }
    Ok(out)
}

/// `result = a * b; lo = S32(result >> 0); hi = S32(result >> 32);` or
/// `lo = S32(a / b); hi = S32(a % b);`, with the operand casts that make it
/// signed or unsigned. Returns the op and how many statements it used.
fn muldiv(stmts: &[crate::c::Stmt]) -> Option<(Op, usize)> {
    use crate::c::Stmt;
    let assign = |k: usize, name: &str| -> Option<&E> {
        match stmts.get(k)? {
            Stmt::Assign(E::Ident(n), rhs) if n == name => Some(rhs),
            _ => None,
        }
    };
    // Operand forms: S64(S32(x)) signed, U64(U32(x)) unsigned for mult;
    // S64(S32(x)) signed, U32(x) unsigned for div.
    let signed = |e: &E| call1(e, "S64").and_then(|e| call1(e, "S32")).and_then(val);
    let unsigned64 = |e: &E| call1(e, "U64").and_then(|e| call1(e, "U32")).and_then(val);
    let unsigned32 = |e: &E| call1(e, "U32").and_then(val);
    if let Some(rhs) = assign(0, "result") {
        let (a, b) = bin(rhs, "*")?;
        let op = match (signed(a), signed(b), unsigned64(a), unsigned64(b)) {
            (Some(a), Some(b), ..) => Op::MulDiv(MulDiv::Mult, a, b),
            (_, _, Some(a), Some(b)) => Op::MulDiv(MulDiv::Multu, a, b),
            _ => return None,
        };
        let lo = call1(assign(1, "lo")?, "S32")?;
        let hi = call1(assign(2, "hi")?, "S32")?;
        if bin(lo, ">>")? != (&E::Ident("result".into()), &E::Num(0))
            || bin(hi, ">>")? != (&E::Ident("result".into()), &E::Num(32))
        {
            return None;
        }
        return Some((op, 3));
    }
    let lo = call1(assign(0, "lo")?, "S32")?;
    let hi = call1(assign(1, "hi")?, "S32")?;
    let (a, b) = bin(lo, "/")?;
    let (a2, b2) = bin(hi, "%")?;
    if (a, b) != (a2, b2) {
        return None;
    }
    let op = match (signed(a), signed(b), unsigned32(a), unsigned32(b)) {
        (Some(a), Some(b), ..) => Op::MulDiv(MulDiv::Div, a, b),
        (_, _, Some(a), Some(b)) => Op::MulDiv(MulDiv::Divu, a, b),
        _ => return None,
    };
    Some((op, 2))
}

/// The condition of `if (cond) {`.
pub fn cond(e: &E) -> Result<Cond, Refusal> {
    if let Some(r) = find_unsupported(e) {
        return Err(r);
    }
    let bad = || unknown("condition", &format!("{e}"));
    let E::Bin(op, a, b) = e else { return Err(bad()) };
    let cmp = match *op {
        "==" => Cmp::Eq,
        "!=" => Cmp::Ne,
        "<" => Cmp::Lt,
        "<=" => Cmp::Le,
        ">" => Cmp::Gt,
        ">=" => Cmp::Ge,
        _ => return Err(bad()),
    };
    match (cmp, call1(a, "SIGNED")) {
        (Cmp::Eq | Cmp::Ne, None) => Ok(Cond::Cmp { cmp, signed: false, a: val(a).ok_or_else(bad)?, b: val(b).ok_or_else(bad)? }),
        (Cmp::Eq | Cmp::Ne, Some(_)) => Err(bad()),
        (_, Some(x)) => Ok(Cond::Cmp { cmp, signed: true, a: val(x).ok_or_else(bad)?, b: Val::I(num(b).ok_or_else(bad)?) }),
        (_, None) => Err(bad()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c::{classify, Line};

    fn ops(line: &str) -> Result<Vec<Op>, Refusal> {
        match classify(line).unwrap() {
            Line::Stmts(s) => ops_of_line(&s),
            l => panic!("{l:?}"),
        }
    }

    #[test]
    fn shapes() {
        assert_eq!(ops("ctx->r29 = ADD32(ctx->r29, -0X8);").unwrap(), [Op::Alu(Alu::Addu, 29, Val::R(29), Val::I(-8))]);
        assert_eq!(ops("MEM_W(0X4, ctx->r29) = ctx->r16;").unwrap(), [Op::Store(Store::W, 29, 4, Val::R(16))]);
        assert_eq!(ops("ctx->r3 = MEM_W(ctx->r3, -0X5D60);").unwrap(), [Op::Load(Load::W, 3, 3, -0x5D60)]);
        assert_eq!(ops("ctx->r3 = S32(0X800A << 16);").unwrap(), [Op::Li(3, 0x800A_0000)]);
        assert_eq!(ops("ctx->r16 = ctx->r5 | 0;").unwrap(), [Op::Move(16, 5)]);
        assert_eq!(ops("ctx->r2 = 0 | 0;").unwrap(), [Op::Const(2, 0)]);
        assert_eq!(ops("ctx->r1 = ADD32(0, 0X3);").unwrap(), [Op::Const(1, 3)]);
        assert_eq!(ops("ctx->r15 = S32(ctx->r2 << 2);").unwrap(), [Op::Shift(Shift::Sll, 15, Val::R(2), 2)]);
        assert_eq!(ops("ctx->r25 = S32(SIGNED(ctx->r6) >> 2);").unwrap(), [Op::Shift(Shift::Sra, 25, Val::R(6), 2)]);
        assert_eq!(ops("ctx->r25 = S32(U32(ctx->r2) >> 2);").unwrap(), [Op::Shift(Shift::Srl, 25, Val::R(2), 2)]);
        assert_eq!(
            ops("ctx->r11 = S32(ctx->r9 << (ctx->r10 & 31));").unwrap(),
            [Op::ShiftV(Shift::Sll, 11, Val::R(9), 10)]
        );
        assert_eq!(
            ops("ctx->r1 = SIGNED(ctx->r4) < SIGNED(ctx->r5) ? 1 : 0;").unwrap(),
            [Op::Alu(Alu::Slt, 1, Val::R(4), Val::R(5))]
        );
        assert_eq!(ops("ctx->r1 = SIGNED(ctx->r3) < 0X3F ? 1 : 0;").unwrap(), [Op::Alu(Alu::Slt, 1, Val::R(3), Val::I(0x3F))]);
        assert_eq!(ops("ctx->r1 = ctx->r25 < -0X1 ? 1 : 0;").unwrap(), [Op::Alu(Alu::Sltu, 1, Val::R(25), Val::I(-1))]);
        assert_eq!(ops("ctx->r2 = 0 < ctx->r7 ? 1 : 0;").unwrap(), [Op::Alu(Alu::Sltu, 2, Val::I(0), Val::R(7))]);
        assert_eq!(ops("ctx->r15 = ~(ctx->r5 | 0);").unwrap(), [Op::Alu(Alu::Nor, 15, Val::R(5), Val::I(0))]);
        assert_eq!(
            ops("result = U64(U32(ctx->r24)) * U64(U32(ctx->r2)); lo = S32(result >> 0); hi = S32(result >> 32);").unwrap(),
            [Op::MulDiv(MulDiv::Multu, Val::R(24), Val::R(2))]
        );
        assert_eq!(
            ops("lo = S32(S64(S32(ctx->r13)) / S64(S32(ctx->r1))); hi = S32(S64(S32(ctx->r13)) % S64(S32(ctx->r1)));").unwrap(),
            [Op::MulDiv(MulDiv::Div, Val::R(13), Val::R(1))]
        );
        assert_eq!(
            ops("lo = S32(U32(ctx->r5) / U32(ctx->r1)); hi = S32(U32(ctx->r5) % U32(ctx->r1));").unwrap(),
            [Op::MulDiv(MulDiv::Divu, Val::R(5), Val::R(1))]
        );
        assert_eq!(ops("func_8002FAFC(rdram, ctx);").unwrap(), [Op::Call("func_8002FAFC".into())]);
    }

    #[test]
    fn refusals() {
        assert_eq!(ops("CHECK_FR(ctx, 8);").unwrap_err().kind, "float");
        assert_eq!(ops("ctx->f6.u32l = MEM_W(ctx->r4, 0X0);").unwrap_err().kind, "float");
        assert_eq!(ops("ctx->r14 = get_cop1_cs();").unwrap_err().kind, "FCR31");
        assert_eq!(ops("LOOKUP_FUNC(ctx->r25)(rdram, ctx);").unwrap_err().kind, "indirect call");
        assert_eq!(ops("do_break(2147521760);").unwrap_err().kind, "break");
        assert_eq!(ops("ctx->r1 = do_lwr(rdram, ctx->r1, ctx->r14, 0X12);").unwrap_err().kind, "unaligned access");
    }

    #[test]
    fn conditions() {
        let c = |s: &str| match classify(s).unwrap() {
            Line::If(e) => cond(&e),
            l => panic!("{l:?}"),
        };
        assert_eq!(
            c("if (ctx->r4 != ctx->r1) {").unwrap(),
            Cond::Cmp { cmp: Cmp::Ne, signed: false, a: Val::R(4), b: Val::R(1) }
        );
        assert_eq!(
            c("if (SIGNED(ctx->r3) <= 0) {").unwrap(),
            Cond::Cmp { cmp: Cmp::Le, signed: true, a: Val::R(3), b: Val::I(0) }
        );
        assert_eq!(c("if (!c1cs) {").unwrap_err().kind, "float");
    }
}
