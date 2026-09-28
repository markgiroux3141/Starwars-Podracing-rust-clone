//! A tokenizer and expression parser for the subset of C that N64Recomp
//! emits. It knows nothing about instructions: [`crate::ir`] matches the
//! resulting trees against the shapes each instruction produces.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    Ident(String),
    Num(i64),
    Punct(&'static str),
}

const PUNCTS: &[&str] = &[
    "->", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "(", ")", ",", ";", "=", "+", "-", "*", "/", "%", "&", "|",
    "^", "~", "!", "<", ">", "?", ":", ".", "[", "]", "{", "}",
];

pub fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Tok::Ident(s[start..i].to_string()));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            let text = &s[start..i];
            let v = if let Some(h) = text.strip_prefix("0X").or_else(|| text.strip_prefix("0x")) {
                i64::from_str_radix(h, 16)
            } else {
                text.parse::<i64>()
            }
            .map_err(|_| format!("bad number `{text}`"))?;
            out.push(Tok::Num(v));
        } else if c == b'"' {
            return Err("string literal".into());
        } else {
            let p = PUNCTS
                .iter()
                .find(|p| s[i..].starts_with(**p))
                .ok_or_else(|| format!("unexpected character `{}`", c as char))?;
            out.push(Tok::Punct(p));
            i += p.len();
        }
    }
    Ok(out)
}

/// A C expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum E {
    /// `ctx->rN`.
    Reg(u8),
    Num(i64),
    Ident(String),
    /// `f(args)`, macros included.
    Call(String, Vec<E>),
    Bin(&'static str, Box<E>, Box<E>),
    Unary(&'static str, Box<E>),
    /// `(type)e`.
    Cast(String, Box<E>),
    Ternary(Box<E>, Box<E>, Box<E>),
    /// `e->name` or `e.name` other than `ctx->rN` (FPRs, `f_odd`).
    Member(Box<E>, String),
    Index(Box<E>, Box<E>),
}

impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            E::Reg(n) => write!(f, "ctx->r{n}"),
            E::Num(v) => write!(f, "{v:#X}"),
            E::Ident(s) => write!(f, "{s}"),
            E::Call(n, args) => {
                write!(f, "{n}(")?;
                for (k, a) in args.iter().enumerate() {
                    if k > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ")")
            }
            E::Bin(op, a, b) => write!(f, "({a} {op} {b})"),
            E::Unary(op, a) => write!(f, "{op}{a}"),
            E::Cast(t, a) => write!(f, "({t}){a}"),
            E::Ternary(c, a, b) => write!(f, "({c} ? {a} : {b})"),
            E::Member(a, n) => write!(f, "{a}.{n}"),
            E::Index(a, i) => write!(f, "{a}[{i}]"),
        }
    }
}

const TYPES: &[&str] = &["int8_t", "uint8_t", "int16_t", "uint16_t", "int32_t", "uint32_t", "int64_t", "uint64_t", "gpr", "float", "double", "int"];

pub struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(toks: &'a [Tok]) -> Self {
        Self { toks, pos: 0 }
    }

    pub fn at_end(&self) -> bool {
        self.pos == self.toks.len()
    }

    pub fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn peek_at(&self, k: usize) -> Option<&Tok> {
        self.toks.get(self.pos + k)
    }

    pub fn eat(&mut self, p: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Punct(q)) if *q == p) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    pub fn expect(&mut self, p: &str) -> Result<(), String> {
        if self.eat(p) {
            Ok(())
        } else {
            Err(format!("expected `{p}`, found {:?}", self.peek()))
        }
    }

    pub fn expr(&mut self) -> Result<E, String> {
        self.ternary()
    }

    fn ternary(&mut self) -> Result<E, String> {
        let c = self.binary(0)?;
        if self.eat("?") {
            let a = self.expr()?;
            self.expect(":")?;
            let b = self.expr()?;
            return Ok(E::Ternary(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }

    fn binary(&mut self, min: u8) -> Result<E, String> {
        const LEVELS: &[&[&str]] =
            &[&["||"], &["&&"], &["|"], &["^"], &["&"], &["==", "!="], &["<", "<=", ">", ">="], &["<<", ">>"], &["+", "-"], &["*", "/", "%"]];
        if min as usize == LEVELS.len() {
            return self.unary();
        }
        let mut lhs = self.binary(min + 1)?;
        loop {
            let op = match self.peek() {
                Some(Tok::Punct(p)) if LEVELS[min as usize].contains(p) => *p,
                _ => break,
            };
            self.pos += 1;
            let rhs = self.binary(min + 1)?;
            lhs = E::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<E, String> {
        // `&` only as address-of (`DMULTU(a, b, &lo, &hi)`).
        for op in ["-", "~", "!", "&"] {
            if self.eat(op) {
                let e = self.unary()?;
                return Ok(match (op, e) {
                    ("-", E::Num(v)) => E::Num(-v),
                    (op, e) => E::Unary(op, Box::new(e)),
                });
            }
        }
        // A cast: `(type)`.
        if matches!(self.peek(), Some(Tok::Punct("("))) {
            if let (Some(Tok::Ident(t)), Some(Tok::Punct(")"))) = (self.peek_at(1), self.peek_at(2)) {
                if TYPES.contains(&t.as_str()) {
                    let t = t.clone();
                    self.pos += 3;
                    let e = self.unary()?;
                    return Ok(E::Cast(t, Box::new(e)));
                }
            }
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<E, String> {
        let mut e = self.primary()?;
        loop {
            if self.eat("->") || self.eat(".") {
                let name = match self.peek() {
                    Some(Tok::Ident(n)) => n.clone(),
                    t => return Err(format!("expected a member name, found {t:?}")),
                };
                self.pos += 1;
                e = match (&e, name.strip_prefix('r').and_then(|n| n.parse::<u8>().ok())) {
                    (E::Ident(c), Some(n)) if c == "ctx" && n < 32 => E::Reg(n),
                    _ => E::Member(Box::new(e), name),
                };
            } else if self.eat("[") {
                let i = self.expr()?;
                self.expect("]")?;
                e = E::Index(Box::new(e), Box::new(i));
            } else if matches!(self.peek(), Some(Tok::Punct("("))) {
                // A call: only of a plain name (LOOKUP_FUNC(x)(...) is a call of a call).
                self.pos += 1;
                let mut args = Vec::new();
                if !self.eat(")") {
                    loop {
                        args.push(self.expr()?);
                        if self.eat(")") {
                            break;
                        }
                        self.expect(",")?;
                    }
                }
                e = match e {
                    E::Ident(n) => E::Call(n, args),
                    other => E::Call(format!("({other})"), args),
                };
            } else {
                return Ok(e);
            }
        }
    }

    fn primary(&mut self) -> Result<E, String> {
        match self.peek().cloned() {
            Some(Tok::Num(v)) => {
                self.pos += 1;
                Ok(E::Num(v))
            }
            Some(Tok::Ident(n)) => {
                self.pos += 1;
                Ok(E::Ident(n))
            }
            Some(Tok::Punct("(")) => {
                self.pos += 1;
                let e = self.expr()?;
                self.expect(")")?;
                Ok(e)
            }
            t => Err(format!("unexpected {t:?}")),
        }
    }
}

/// One line of a generated function body, classified.
#[derive(Debug)]
pub enum Line {
    /// `// 0xADDR: insn`.
    Comment { addr: u32 },
    Label(String),
    Goto(String),
    Return,
    /// `if (cond) {`.
    If(E),
    /// `}`.
    Close,
    /// One or more `;`-separated statements: `lhs = rhs` or an expression.
    Stmts(Vec<Stmt>),
    /// `gpr jr_addend_X = ctx->rN;`: a jump table's index register, saved
    /// at its `addu` (X is the `jr`'s address).
    JrAddend { jr: u32, reg: u8 },
    /// `switch (jr_addend_X >> 2) {`.
    Switch { jr: u32 },
    /// `case K: goto L; break;`.
    Case(u64, String),
    /// `default: switch_error(__func__, JR, TABLE);`.
    Default { jr: u32, table: u32 },
    /// Declarations and the closing `;}`: nothing to translate.
    Skip,
}

#[derive(Debug)]
pub enum Stmt {
    Assign(E, E),
    Expr(E),
}

pub fn classify(line: &str) -> Result<Line, String> {
    let s = line.trim();
    if s.is_empty() || s == ";}" || s == "uint64_t hi = 0, lo = 0, result = 0;" || s == "int c1cs = 0;" {
        return Ok(Line::Skip);
    }
    if let Some(c) = s.strip_prefix("// ") {
        // `0x80006D5C: addiu       $sp, $sp, -0x8`
        if let Some((a, _insn)) = c.split_once(": ") {
            if let Some(h) = a.strip_prefix("0x") {
                if let Ok(addr) = u32::from_str_radix(h, 16) {
                    return Ok(Line::Comment { addr });
                }
            }
        }
        return Ok(Line::Skip);
    }
    if s == "}" {
        return Ok(Line::Close);
    }
    if s == "return;" {
        return Ok(Line::Return);
    }
    if let Some(l) = s.strip_prefix("goto ").and_then(|l| l.strip_suffix(';')) {
        return Ok(Line::Goto(l.to_string()));
    }
    if let Some(l) = s.strip_suffix(':') {
        if l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') && l != "default" {
            return Ok(Line::Label(l.to_string()));
        }
    }
    if s.starts_with("case ") || s.starts_with("switch ") || s.starts_with("default:") || s.starts_with("gpr jr_addend") {
        return jump_table_line(s).ok_or_else(|| format!("jump table (switch): unexpected shape `{s}`"));
    }
    if let Some(c) = s.strip_prefix("if (").and_then(|c| c.strip_suffix(") {")) {
        let toks = tokenize(c)?;
        let mut p = Parser::new(&toks);
        let e = p.expr()?;
        if !p.at_end() {
            return Err(format!("trailing tokens in condition `{c}`"));
        }
        return Ok(Line::If(e));
    }
    let toks = tokenize(s)?;
    let mut out = Vec::new();
    let mut p = Parser::new(&toks);
    while !p.at_end() {
        let lhs = p.expr()?;
        let st = if p.eat("=") { Stmt::Assign(lhs, p.expr()?) } else { Stmt::Expr(lhs) };
        p.expect(";")?;
        out.push(st);
    }
    Ok(Line::Stmts(out))
}

fn hex_u32(s: &str) -> Option<u32> {
    u32::from_str_radix(s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?, 16).ok()
}

/// The four line shapes of N64Recomp's jump tables (`emit_switch` and
/// friends in its `cgenerator.cpp`), exactly as it prints them.
fn jump_table_line(s: &str) -> Option<Line> {
    if let Some(rest) = s.strip_prefix("gpr jr_addend_") {
        // gpr jr_addend_80008F98 = ctx->r14;
        let (jr, rhs) = rest.split_once(" = ctx->r")?;
        let reg = rhs.strip_suffix(';')?.parse::<u8>().ok().filter(|&r| r < 32)?;
        return Some(Line::JrAddend { jr: u32::from_str_radix(jr, 16).ok()?, reg });
    }
    if let Some(rest) = s.strip_prefix("switch (jr_addend_") {
        let jr = rest.strip_suffix(" >> 2) {")?;
        return Some(Line::Switch { jr: u32::from_str_radix(jr, 16).ok()? });
    }
    if let Some(rest) = s.strip_prefix("case ") {
        // case 0: goto L_80008FA0; break;
        let (k, rest) = rest.split_once(": goto ")?;
        let label = rest.strip_suffix("; break;")?;
        if !label.starts_with("L_") || !label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return None;
        }
        return Some(Line::Case(k.parse().ok()?, label.to_string()));
    }
    // default: switch_error(__func__, 0x80008F98, 0x800A8200);
    let args = s.strip_prefix("default: switch_error(__func__, ")?.strip_suffix(");")?;
    let (jr, table) = args.split_once(", ")?;
    Some(Line::Default { jr: hex_u32(jr)?, table: hex_u32(table)? })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> E {
        let t = tokenize(s).unwrap();
        let mut p = Parser::new(&t);
        let e = p.expr().unwrap();
        assert!(p.at_end());
        e
    }

    #[test]
    fn expressions() {
        assert_eq!(parse("ctx->r29"), E::Reg(29));
        assert_eq!(parse("-0X18"), E::Num(-0x18));
        assert_eq!(
            parse("ADD32(ctx->r29, -0X18)"),
            E::Call("ADD32".into(), vec![E::Reg(29), E::Num(-0x18)])
        );
        assert_eq!(
            parse("S32(SIGNED(ctx->r6) >> 2)"),
            E::Call(
                "S32".into(),
                vec![E::Bin(">>", Box::new(E::Call("SIGNED".into(), vec![E::Reg(6)])), Box::new(E::Num(2)))]
            )
        );
        assert!(matches!(parse("SIGNED(ctx->r4) < SIGNED(ctx->r5) ? 1 : 0"), E::Ternary(..)));
        assert!(matches!(parse("(int32_t)ctx->f8.u32l"), E::Cast(..)));
        assert!(matches!(parse("~(ctx->r5 | 0)"), E::Unary("~", _)));
    }

    #[test]
    fn lines() {
        assert!(matches!(classify("    // 0x80006D5C: addiu       $sp, $sp, -0x8").unwrap(), Line::Comment { addr: 0x80006D5C, .. }));
        assert!(matches!(classify("L_80006D74:").unwrap(), Line::Label(_)));
        assert!(matches!(classify("        goto L_80006D74;").unwrap(), Line::Goto(_)));
        assert!(matches!(classify("    if (SIGNED(ctx->r3) <= 0) {").unwrap(), Line::If(_)));
        match classify("result = U64(U32(ctx->r24)) * U64(U32(ctx->r2)); lo = S32(result >> 0); hi = S32(result >> 32);").unwrap() {
            Line::Stmts(v) => assert_eq!(v.len(), 3),
            l => panic!("{l:?}"),
        }
        assert!(matches!(
            classify("        default: switch_error(__func__, 0x8000107C, 0x800A80FC);").unwrap(),
            Line::Default { jr: 0x8000107C, table: 0x800A80FC }
        ));
        assert!(matches!(classify("    gpr jr_addend_80008F98 = ctx->r14;").unwrap(), Line::JrAddend { jr: 0x80008F98, reg: 14 }));
        assert!(matches!(classify("    switch (jr_addend_80008F98 >> 2) {").unwrap(), Line::Switch { jr: 0x80008F98 }));
        match classify("        case 12: goto L_80008FA0; break;").unwrap() {
            Line::Case(12, l) => assert_eq!(l, "L_80008FA0"),
            l => panic!("{l:?}"),
        }
        assert!(classify("        case 1: goto L_1; return;").is_err());
        assert!(classify("    switch (jr_addend_80008F98 >> 3) {").is_err());
    }
}
