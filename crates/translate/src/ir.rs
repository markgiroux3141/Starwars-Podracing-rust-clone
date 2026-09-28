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
    /// `ld`: a doubleword, high word first.
    D,
    H,
    Hu,
    B,
    Bu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    W,
    /// `sd`: a doubleword (N64Recomp stores the low word first).
    D,
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
    /// The doubleword forms, on the full registers.
    DMult,
    DMultu,
    DDiv,
    DDivu,
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
    /// A doubleword shift of the full register by 0..63 (`dsll`, `dsll32`, ...).
    DShift(Shift, u8, Val, u32),
    /// A doubleword shift by the low 6 bits of a register.
    DShiftV(Shift, u8, Val, u8),
    /// `daddu`/`daddiu`: the full 64-bit sum, wrapping.
    DAdd(u8, Val, Val),
    /// Writes the function's `lo`/`hi` locals (not `ctx->lo`/`ctx->hi`).
    MulDiv(MulDiv, Val, Val),
    MfLo(u8),
    MfHi(u8),
    Call(String),
    /// `LOOKUP_FUNC(v)(rdram, ctx)`: an indirect call (`jalr`) to the
    /// function the runtime's `get_function` finds at `v`'s low word.
    CallIndirect(Val),
    /// The runtime's `pause_self`, N64Recomp's translation of `b .`.
    PauseSelf,
    /// `let jr_addend_JR = rN;`: a jump table's index register (byte
    /// offset into the table), saved at the table's `addu`.
    JrAddend(u32, u8),
    /// The runtime's `switch_error(func, jr, table)`: a jump table's
    /// index matched no case. The C carries on after the switch if it
    /// returns.
    SwitchError { func: String, jr: u32, table: u32 },
    /// The runtime's `do_break(vram)`, N64Recomp's translation of `break`
    /// (IDO's guards after `div`: `break 7` for a zero divisor, `break 6`
    /// for `INT_MIN / -1`). The C carries on after it if it returns.
    Break(u32),
    /// `let c<n> = cond;`: a branch condition read before its delay slot
    /// overwrites one of its registers.
    SaveCond(usize, Cond),
    /// A label of the C, kept as a comment where blocks were merged.
    Label(String),
    // Floats. FPRs by number; in the game's 32-bit FPU mode singles and
    // words live in the low half (`fl`/`u32l`) of an even register, doubles
    // in the whole register, and odd registers in `f_odd`, the high half of
    // the even register below.
    /// `lwc1`: `f.u32l = mem[base + off]`.
    FLoad(u8, u8, i32),
    /// `swc1`: `mem[base + off] = f.u32l`.
    FStore(u8, u8, i32),
    /// `ldc1`: `f.u64 = mem64[base + off]`.
    FLoadD(u8, u8, i32),
    /// `sdc1`: `mem64[base + off] = f.u64`.
    FStoreD(u8, u8, i32),
    /// `mtc1` to an even register: `f.u32l = low word`.
    Mtc1(u8, Val),
    /// `mtc1` to an odd register `n`: the high half of `f(n - 1)`.
    Mtc1Odd(u8, Val),
    /// `mfc1` from an even register: `rd = sext(f.u32l)`.
    Mfc1(u8, u8),
    /// `mfc1` from an odd register `n`: `rd = sext(f(n - 1).u32h)`.
    Mfc1Odd(u8, u8),
    /// `dmtc1`: `f.u64 = rs`.
    DMtc1(u8, Val),
    /// `dmfc1`: `rd = f.u64`.
    DMfc1(u8, u8),
    FArith(FArith, Prec, u8, u8, u8),
    FUn(FUn, Prec, u8, u8),
    Cvt(Cvt, u8, u8),
    /// `c.cond.fmt`: sets the function's `c1cs` local.
    FCmp(Cmp, Prec, u8, u8),
    /// `cfc1 rd, $31`: FCR31 as N64Recomp reads it, the rounding bits only.
    Cfc1(u8),
    /// `ctc1 rs, $31`: sets the rounding mode.
    Ctc1(Val),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prec {
    S,
    D,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FArith {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FUn {
    Neg,
    Mov,
    Sqrt,
}

/// Conversions: `Xy` converts format `y` to `X` (`W` word, `S` single, `D`
/// double), as MIPS names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cvt {
    SW,
    DW,
    DS,
    SD,
    WS,
    WD,
    TruncWS,
    TruncWD,
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
            | Op::DShift(_, d, ..)
            | Op::DShiftV(_, d, ..)
            | Op::DAdd(d, ..)
            | Op::DMfc1(d, _)
            | Op::MfLo(d)
            | Op::MfHi(d)
            | Op::Mfc1(d, _)
            | Op::Mfc1Odd(d, _)
            | Op::Cfc1(d) => Some(d),
            _ => None,
        }
    }

    /// Whether this op writes `c1cs` (a float compare).
    pub fn writes_c1(&self) -> bool {
        matches!(self, Op::FCmp(..))
    }

    pub fn is_call(&self) -> bool {
        matches!(self, Op::Call(_) | Op::CallIndirect(_) | Op::PauseSelf)
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
    /// `c1cs` (true) or `!c1cs` (false): the last float compare, `bc1t`/`bc1f`.
    C1(bool),
}

impl Cond {
    pub fn negate(&self) -> Self {
        match self {
            Cond::Cmp { cmp, signed, a, b } => Cond::Cmp { cmp: cmp.negate(), signed: *signed, a: *a, b: *b },
            Cond::Saved(n, neg) => Cond::Saved(*n, !neg),
            Cond::Not(c) => (**c).clone(),
            Cond::C1(t) => Cond::C1(!t),
            c => Cond::Not(Box::new(c.clone())),
        }
    }

    pub fn reads(&self, r: u8) -> bool {
        match self {
            Cond::Cmp { a, b, .. } => *a == Val::R(r) || *b == Val::R(r),
            Cond::Saved(..) | Cond::C1(_) => false,
            Cond::And(a, b) => a.reads(r) || b.reads(r),
            Cond::Not(c) => c.reads(r),
        }
    }

    pub fn reads_c1(&self) -> bool {
        match self {
            Cond::C1(_) => true,
            Cond::And(a, b) => a.reads_c1() || b.reads_c1(),
            Cond::Not(c) => c.reads_c1(),
            _ => false,
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

/// `ctx->fN.field`.
fn fpr<'a>(e: &'a E, field: &str) -> Option<u8> {
    let E::Member(inner, f) = e else { return None };
    if f != field {
        return None;
    }
    let E::Member(ctx, name) = &**inner else { return None };
    if **ctx != E::Ident("ctx".into()) {
        return None;
    }
    name.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()).filter(|&n| n < 32)
}

/// `ctx->f_odd[(N - 1) * 2]`: odd register N.
fn f_odd(e: &E) -> Option<u8> {
    let E::Index(base, idx) = e else { return None };
    let E::Member(ctx, name) = &**base else { return None };
    if **ctx != E::Ident("ctx".into()) || name != "f_odd" {
        return None;
    }
    let (m, two) = bin(idx, "*")?;
    let (n, one) = bin(m, "-")?;
    match (n, one, two) {
        (E::Num(n), E::Num(1), E::Num(2)) if n % 2 == 1 && (1..32).contains(n) => Some(*n as u8),
        _ => None,
    }
}

/// A float statement: `Some(Ok(None))` for checks that translate to
/// nothing, `Some(Ok(Some(op)))`, `Some(Err)` for a float shape that isn't
/// supported, `None` if it isn't a float statement.
fn float_stmt(st: &crate::c::Stmt) -> Option<Result<Option<Op>, Refusal>> {
    use crate::c::Stmt;
    let ok = |op| Some(Ok(Some(op)));
    match st {
        Stmt::Expr(e @ E::Call(n, args)) => match (n.as_str(), args.as_slice()) {
            ("NAN_CHECK", [_]) => Some(Ok(None)),
            ("CHECK_FR", [E::Ident(c), E::Num(k)]) if c == "ctx" => {
                if k % 2 == 0 {
                    Some(Ok(None))
                } else {
                    Some(Err(Refusal::new("float", format!("odd FPR check `{e}`"))))
                }
            }
            ("SD", [v, a, b]) => {
                let f = fpr(v, "u64")?;
                let (base, off) = base_off(a, b)?;
                ok(Op::FStoreD(f, base, off))
            }
            ("set_cop1_cs", [v]) => ok(Op::Ctc1(val(v)?)),
            _ => None,
        },
        Stmt::Assign(lhs, rhs) => {
            // Stores of FPR words.
            if let Some(("MEM_W", base, off)) = mem(lhs) {
                return fpr(rhs, "u32l").map(|f| Ok(Some(Op::FStore(f, base, off))));
            }
            if let E::Reg(d) = lhs {
                if let E::Cast(t, inner) = rhs {
                    if t == "int32_t" {
                        if let Some(f) = fpr(inner, "u32l") {
                            return ok(Op::Mfc1(*d, f));
                        }
                        if let Some(n) = f_odd(inner) {
                            return ok(Op::Mfc1Odd(*d, n));
                        }
                    }
                }
                if call(rhs, "get_cop1_cs") == Some(&[]) {
                    return ok(Op::Cfc1(*d));
                }
                if let Some(f) = fpr(rhs, "u64") {
                    return ok(Op::DMfc1(*d, f));
                }
                return None;
            }
            if let Some(n) = f_odd(lhs) {
                return ok(Op::Mtc1Odd(n, val(rhs)?));
            }
            if let E::Ident(c) = lhs {
                if c == "c1cs" {
                    let E::Bin(op, a, b) = rhs else { return None };
                    let cmp = match *op {
                        "<" => Cmp::Lt,
                        "<=" => Cmp::Le,
                        "==" => Cmp::Eq,
                        _ => return None,
                    };
                    if let (Some(a), Some(b)) = (fpr(a, "fl"), fpr(b, "fl")) {
                        return ok(Op::FCmp(cmp, Prec::S, a, b));
                    }
                    if let (Some(a), Some(b)) = (fpr(a, "d"), fpr(b, "d")) {
                        return ok(Op::FCmp(cmp, Prec::D, a, b));
                    }
                    return None;
                }
            }
            if let Some(d) = fpr(lhs, "u32l") {
                if let Some(("MEM_W", base, off)) = mem(rhs) {
                    return ok(Op::FLoad(d, base, off));
                }
                if let Some(v) = val(rhs) {
                    return ok(Op::Mtc1(d, v));
                }
                let cvt = |name, from: &str| call1(rhs, name).and_then(|a| fpr(a, from));
                return [("CVT_W_S", "fl", Cvt::WS), ("CVT_W_D", "d", Cvt::WD), ("TRUNC_W_S", "fl", Cvt::TruncWS), ("TRUNC_W_D", "d", Cvt::TruncWD)]
                    .into_iter()
                    .find_map(|(n, from, k)| cvt(n, from).map(|a| Op::Cvt(k, d, a)))
                    .map(|op| Ok(Some(op)));
            }
            if let Some(d) = fpr(lhs, "u64") {
                if let Some(v) = val(rhs) {
                    return ok(Op::DMtc1(d, v));
                }
                let [a, b] = call(rhs, "LD")? else { return None };
                let (base, off) = base_off(a, b)?;
                return ok(Op::FLoadD(d, base, off));
            }
            for (field, prec) in [("fl", Prec::S), ("d", Prec::D)] {
                let Some(d) = fpr(lhs, field) else { continue };
                let arg = |e: &E| fpr(e, field);
                let (mul, div) = if prec == Prec::S { ("MUL_S", "DIV_S") } else { ("MUL_D", "DIV_D") };
                let two = |e: &E, k| -> Option<Op> {
                    match e {
                        E::Bin(o, a, b) if (*o == "+" && k == FArith::Add) || (*o == "-" && k == FArith::Sub) => {
                            Some(Op::FArith(k, prec, d, arg(a)?, arg(b)?))
                        }
                        E::Call(n, v) if (n == mul && k == FArith::Mul) || (n == div && k == FArith::Div) => match v.as_slice() {
                            [a, b] => Some(Op::FArith(k, prec, d, arg(a)?, arg(b)?)),
                            _ => None,
                        },
                        _ => None,
                    }
                };
                if let Some(op) = [FArith::Add, FArith::Sub, FArith::Mul, FArith::Div].into_iter().find_map(|k| two(rhs, k)) {
                    return ok(op);
                }
                if let E::Unary("-", a) = rhs {
                    return arg(a).map(|a| Ok(Some(Op::FUn(FUn::Neg, prec, d, a))));
                }
                if let Some(a) = arg(rhs) {
                    return ok(Op::FUn(FUn::Mov, prec, d, a));
                }
                if prec == Prec::S {
                    if let Some(a) = call1(rhs, "sqrtf").and_then(arg) {
                        return ok(Op::FUn(FUn::Sqrt, prec, d, a));
                    }
                    if let Some(a) = call1(rhs, "CVT_S_W").and_then(|a| fpr(a, "u32l")) {
                        return ok(Op::Cvt(Cvt::SW, d, a));
                    }
                    if let Some(a) = call1(rhs, "CVT_S_D").and_then(|a| fpr(a, "d")) {
                        return ok(Op::Cvt(Cvt::SD, d, a));
                    }
                } else {
                    if let Some(a) = call1(rhs, "CVT_D_W").and_then(|a| fpr(a, "u32l")) {
                        return ok(Op::Cvt(Cvt::DW, d, a));
                    }
                    if let Some(a) = call1(rhs, "CVT_D_S").and_then(|a| fpr(a, "fl")) {
                        return ok(Op::Cvt(Cvt::DS, d, a));
                    }
                }
                return None;
            }
            None
        }
        _ => None,
    }
}

fn base_off(a: &E, b: &E) -> Option<(u8, i32)> {
    match (a, b) {
        (E::Reg(r), E::Num(o)) | (E::Num(o), E::Reg(r)) => Some((*r, i32::try_from(*o).ok()?)),
        _ => None,
    }
}

/// A doubleword (64-bit) statement, if `st` is one of their shapes:
/// `LD`/`SD` of a GPR, `x << (N + 32)`, `SIGNED(x) >> (N + 32)`, `x >> (N +
/// 32)` and the plain `<<`/`>>` by a constant, the variable forms with `(rs
/// & 63)`, `a + b` without `ADD32`, and `DMULT`/`DMULTU`/`DDIV`/`DDIVU` on
/// `&lo, &hi`.
fn dword_stmt(st: &crate::c::Stmt) -> Option<Op> {
    use crate::c::Stmt;
    match st {
        Stmt::Expr(E::Call(n, args)) if n == "SD" => match args.as_slice() {
            [v, E::Num(o), E::Reg(b)] => Some(Op::Store(Store::D, *b, i32::try_from(*o).ok()?, val(v)?)),
            _ => None,
        },
        Stmt::Expr(E::Call(n, args)) => {
            let kind = match n.as_str() {
                "DMULT" => MulDiv::DMult,
                "DMULTU" => MulDiv::DMultu,
                "DDIV" => MulDiv::DDiv,
                "DDIVU" => MulDiv::DDivu,
                _ => return None,
            };
            let wrap = if matches!(kind, MulDiv::DMult | MulDiv::DDiv) { "S64" } else { "U64" };
            let [a, b, E::Unary("&", lo), E::Unary("&", hi)] = args.as_slice() else { return None };
            if **lo != E::Ident("lo".into()) || **hi != E::Ident("hi".into()) {
                return None;
            }
            Some(Op::MulDiv(kind, val(call1(a, wrap)?)?, val(call1(b, wrap)?)?))
        }
        Stmt::Assign(E::Reg(d), rhs) => {
            let d = *d;
            if let Some([E::Reg(b), E::Num(o)]) = call(rhs, "LD") {
                return Some(Op::Load(Load::D, d, *b, i32::try_from(*o).ok()?));
            }
            // The shift amount: `N`, `(N + 32)` or `(rs & 63)`.
            enum Amount {
                Fixed(u32),
                Reg(u8),
            }
            let amount = |s: &E| -> Option<Amount> {
                match s {
                    E::Num(n) if (0..32).contains(n) => Some(Amount::Fixed(*n as u32)),
                    E::Bin("+", a, b) => match (&**a, &**b) {
                        (E::Num(n), E::Num(32)) if (0..32).contains(n) => Some(Amount::Fixed(*n as u32 + 32)),
                        _ => None,
                    },
                    E::Bin("&", r, m) if **m == E::Num(63) => Some(Amount::Reg(reg(r)?)),
                    _ => None,
                }
            };
            let shift = |kind, x: &E, s: &E| -> Option<Op> {
                let x = val(x)?;
                Some(match amount(s)? {
                    Amount::Fixed(sa) => Op::DShift(kind, d, x, sa),
                    Amount::Reg(r) => Op::DShiftV(kind, d, x, r),
                })
            };
            match rhs {
                E::Bin("<<", x, s) => shift(Shift::Sll, x, s),
                E::Bin(">>", x, s) => match call1(x, "SIGNED") {
                    Some(x) => shift(Shift::Sra, x, s),
                    None => shift(Shift::Srl, x, s),
                },
                E::Bin("+", a, b) => Some(match (val(a)?, val(b)?) {
                    (Val::I(x), Val::I(y)) => Op::Const(d, x.checked_add(y)?),
                    (a, b) => Op::DAdd(d, a, b),
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Statements of one C line, as ops. `mult`/`div` span several statements.
pub fn ops_of_line(stmts: &[crate::c::Stmt]) -> Result<Vec<Op>, Refusal> {
    use crate::c::Stmt;
    let mut out = Vec::new();
    let mut k = 0;
    while k < stmts.len() {
        if let Some(op) = dword_stmt(&stmts[k]) {
            out.push(op);
            k += 1;
            continue;
        }
        match float_stmt(&stmts[k]) {
            Some(Ok(op)) => {
                out.extend(op);
                k += 1;
                continue;
            }
            Some(Err(r)) => return Err(r),
            None => {}
        }
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
                E::Call(n, args)
                    if n.starts_with("(LOOKUP_FUNC(")
                        && args.as_slice() == [E::Ident("rdram".into()), E::Ident("ctx".into())] =>
                {
                    // The parser names a call of a call by the callee's text.
                    let inner = crate::c::tokenize(&n[1..n.len() - 1]).map_err(|e| Refusal::new("indirect call", e))?;
                    let mut p = crate::c::Parser::new(&inner);
                    let target = p.expr().map_err(|e| Refusal::new("indirect call", e))?;
                    match (call1(&target, "LOOKUP_FUNC").and_then(val), p.at_end()) {
                        (Some(v), true) => out.push(Op::CallIndirect(v)),
                        _ => return Err(refusal_for_name(n, e)),
                    }
                }
                E::Call(n, args) if n == "do_break" => match args.as_slice() {
                    [E::Num(v)] if u32::try_from(*v).is_ok() => out.push(Op::Break(*v as u32)),
                    _ => return Err(refusal_for_name(n, e)),
                },
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
    match e {
        E::Ident(c) if c == "c1cs" => return Ok(Cond::C1(true)),
        E::Unary("!", c) if **c == E::Ident("c1cs".into()) => return Ok(Cond::C1(false)),
        _ => {}
    }
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
        assert_eq!(ops("do_break(2147688548);").unwrap(), [Op::Break(0x8003_2064)]);
    }

    #[test]
    fn float_shapes() {
        assert_eq!(ops("CHECK_FR(ctx, 8);").unwrap(), []);
        assert_eq!(ops("NAN_CHECK(ctx->f4.fl); NAN_CHECK(ctx->f6.fl);").unwrap(), []);
        assert_eq!(ops("ctx->f6.u32l = MEM_W(ctx->r4, 0X0);").unwrap(), [Op::FLoad(6, 4, 0)]);
        assert_eq!(ops("MEM_W(0X6C, ctx->r29) = ctx->f8.u32l;").unwrap(), [Op::FStore(8, 29, 0x6C)]);
        assert_eq!(ops("ctx->f20.u64 = LD(ctx->r29, 0X18);").unwrap(), [Op::FLoadD(20, 29, 0x18)]);
        assert_eq!(ops("SD(ctx->f20.u64, 0X18, ctx->r29);").unwrap(), [Op::FStoreD(20, 29, 0x18)]);
        assert_eq!(ops("ctx->f0.u32l = ctx->r1;").unwrap(), [Op::Mtc1(0, Val::R(1))]);
        assert_eq!(ops("ctx->f_odd[(19 - 1) * 2] = ctx->r1;").unwrap(), [Op::Mtc1Odd(19, Val::R(1))]);
        assert_eq!(ops("ctx->r5 = (int32_t)ctx->f8.u32l;").unwrap(), [Op::Mfc1(5, 8)]);
        assert_eq!(ops("ctx->r6 = (int32_t)ctx->f_odd[(17 - 1) * 2];").unwrap(), [Op::Mfc1Odd(6, 17)]);
        assert_eq!(ops("ctx->f8.fl = MUL_S(ctx->f4.fl, ctx->f6.fl);").unwrap(), [Op::FArith(FArith::Mul, Prec::S, 8, 4, 6)]);
        assert_eq!(ops("ctx->f10.fl = ctx->f12.fl + ctx->f2.fl;").unwrap(), [Op::FArith(FArith::Add, Prec::S, 10, 12, 2)]);
        assert_eq!(ops("ctx->f10.d = ctx->f6.d - ctx->f8.d;").unwrap(), [Op::FArith(FArith::Sub, Prec::D, 10, 6, 8)]);
        assert_eq!(ops("ctx->f4.d = DIV_D(ctx->f10.d, ctx->f18.d);").unwrap(), [Op::FArith(FArith::Div, Prec::D, 4, 10, 18)]);
        assert_eq!(ops("ctx->f12.fl = -ctx->f0.fl;").unwrap(), [Op::FUn(FUn::Neg, Prec::S, 12, 0)]);
        assert_eq!(ops("ctx->f12.fl = ctx->f0.fl;").unwrap(), [Op::FUn(FUn::Mov, Prec::S, 12, 0)]);
        assert_eq!(ops("ctx->f0.fl = sqrtf(ctx->f0.fl);").unwrap(), [Op::FUn(FUn::Sqrt, Prec::S, 0, 0)]);
        assert_eq!(ops("ctx->f6.fl = CVT_S_W(ctx->f4.u32l);").unwrap(), [Op::Cvt(Cvt::SW, 6, 4)]);
        assert_eq!(ops("ctx->f6.d = CVT_D_S(ctx->f4.fl);").unwrap(), [Op::Cvt(Cvt::DS, 6, 4)]);
        assert_eq!(ops("ctx->f10.u32l = TRUNC_W_S(ctx->f8.fl);").unwrap(), [Op::Cvt(Cvt::TruncWS, 10, 8)]);
        assert_eq!(ops("ctx->f10.u32l = CVT_W_S(ctx->f20.fl);").unwrap(), [Op::Cvt(Cvt::WS, 10, 20)]);
        assert_eq!(ops("c1cs = ctx->f0.fl < ctx->f16.fl;").unwrap(), [Op::FCmp(Cmp::Lt, Prec::S, 0, 16)]);
        assert_eq!(ops("c1cs = ctx->f6.d <= ctx->f20.d;").unwrap(), [Op::FCmp(Cmp::Le, Prec::D, 6, 20)]);
        assert_eq!(ops("ctx->r14 = get_cop1_cs();").unwrap(), [Op::Cfc1(14)]);
        assert_eq!(ops("set_cop1_cs(ctx->r6);").unwrap(), [Op::Ctc1(Val::R(6))]);
    }

    #[test]
    fn doubleword_shapes() {
        assert_eq!(ops("ctx->r15 = LD(ctx->r29, 0X8);").unwrap(), [Op::Load(Load::D, 15, 29, 8)]);
        assert_eq!(ops("SD(ctx->r4, 0X10, ctx->r29);").unwrap(), [Op::Store(Store::D, 29, 0x10, Val::R(4))]);
        assert_eq!(ops("ctx->r3 = ctx->r2 << (0 + 32);").unwrap(), [Op::DShift(Shift::Sll, 3, Val::R(2), 32)]);
        assert_eq!(ops("ctx->r3 = ctx->r2 << (31 + 32);").unwrap(), [Op::DShift(Shift::Sll, 3, Val::R(2), 63)]);
        assert_eq!(ops("ctx->r2 = SIGNED(ctx->r2) >> (0 + 32);").unwrap(), [Op::DShift(Shift::Sra, 2, Val::R(2), 32)]);
        assert_eq!(ops("ctx->r2 = ctx->r2 >> (4 + 32);").unwrap(), [Op::DShift(Shift::Srl, 2, Val::R(2), 36)]);
        assert_eq!(ops("ctx->r2 = ctx->r14 << (ctx->r15 & 63);").unwrap(), [Op::DShiftV(Shift::Sll, 2, Val::R(14), 15)]);
        assert_eq!(ops("ctx->r2 = SIGNED(ctx->r14) >> (ctx->r15 & 63);").unwrap(), [Op::DShiftV(Shift::Sra, 2, Val::R(14), 15)]);
        assert_eq!(ops("ctx->r2 = ctx->r14 >> (ctx->r15 & 63);").unwrap(), [Op::DShiftV(Shift::Srl, 2, Val::R(14), 15)]);
        assert_eq!(ops("ctx->r2 = ctx->r4 + ctx->r5;").unwrap(), [Op::DAdd(2, Val::R(4), Val::R(5))]);
        assert_eq!(ops("ctx->r2 = 0 + -0X1;").unwrap(), [Op::Const(2, -1)]);
        assert_eq!(
            ops("DMULTU(U64(ctx->r14), U64(ctx->r15), &lo, &hi);").unwrap(),
            [Op::MulDiv(MulDiv::DMultu, Val::R(14), Val::R(15))]
        );
        assert_eq!(ops("DDIV(S64(ctx->r14), S64(ctx->r15), &lo, &hi);").unwrap(), [Op::MulDiv(MulDiv::DDiv, Val::R(14), Val::R(15))]);
        assert_eq!(ops("ctx->f4.u64 = ctx->r5;").unwrap(), [Op::DMtc1(4, Val::R(5))]);
        assert_eq!(ops("ctx->r5 = ctx->f4.u64;").unwrap(), [Op::DMfc1(5, 4)]);
        // 32-bit shifts are still their own shapes.
        assert_eq!(ops("ctx->r15 = S32(ctx->r2 << 2);").unwrap(), [Op::Shift(Shift::Sll, 15, Val::R(2), 2)]);
    }

    #[test]
    fn refusals() {
        assert_eq!(ops("CHECK_FR(ctx, 9);").unwrap_err().kind, "float");
        assert_eq!(ops("ctx->f0.u64 = CVT_L_S(ctx->f2.fl);").unwrap_err().kind, "float");
        assert_eq!(ops("LOOKUP_FUNC(ctx->r25)(rdram, ctx);").unwrap(), [Op::CallIndirect(Val::R(25))]);
        assert_eq!(ops("LOOKUP_FUNC(0X80012340)(rdram, ctx);").unwrap(), [Op::CallIndirect(Val::I(0x8001_2340))]);
        assert_eq!(ops("LOOKUP_FUNC(ctx->r25 + 4)(rdram, ctx);").unwrap_err().kind, "indirect call");
        assert_eq!(ops("do_break(ctx->r4);").unwrap_err().kind, "break");
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
        assert_eq!(c("if (!c1cs) {").unwrap(), Cond::C1(false));
    }
}
