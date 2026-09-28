//! `cargo xtask next-function`: propose the next port target (SPEC §7).
//!
//! A function is ready when it is still `recomp`, N64Recomp translated it
//! (not `ignored` in recomp.toml), it has no indirect calls (their targets
//! are unknown) and every function it calls directly, tail-calls or falls
//! into is already `rust_verified` or `lifted`. Ready functions are ranked by
//! preferred subsystem, depth, then outside the OS range, integer before
//! float, and address.
//!
//! Inputs: symbols/functions.csv, symbols/callgraph.csv and
//! symbols/racer.syms.toml (all from `cargo xtask find-functions`),
//! recomp.toml, and generated/ (for the float / FCR31 flags, if present).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::Result;

/// libultra/OS code starts at or before this address (NOTES.md: cache ops at
/// 0x80087CC0). Not exact, so it only lowers the rank.
const OS_RANGE_GUESS: u32 = 0x8008_7CC0;

struct Func {
    vram: u32,
    name: String,
    depth: u32,
    subsystem: String,
    status: String,
    size: u32,
    callees: BTreeSet<u32>,
    indirect: usize,
    ignored: bool,
    uses_float: bool,
    reads_fcr31: bool,
    generated: bool,
}

impl Func {
    fn done(&self) -> bool {
        matches!(self.status.as_str(), "rust_verified" | "lifted")
    }
    fn in_os_range(&self) -> bool {
        self.vram >= OS_RANGE_GUESS
    }
}

/// Minimal CSV reader: comma-separated, fields optionally double-quoted
/// (with "" for a quote inside), as Python's csv module writes them.
fn parse_csv(text: &str) -> Vec<BTreeMap<String, String>> {
    fn fields(line: &str) -> Vec<String> {
        let mut out = vec![String::new()];
        let mut quoted = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match (c, quoted) {
                ('"', true) if chars.peek() == Some(&'"') => {
                    chars.next();
                    out.last_mut().unwrap().push('"');
                }
                ('"', _) => quoted = !quoted,
                (',', false) => out.push(String::new()),
                (c, _) => out.last_mut().unwrap().push(c),
            }
        }
        out
    }
    let mut lines = text.lines().filter(|l| !l.is_empty());
    let Some(header) = lines.next() else { return Vec::new() };
    let cols = fields(header);
    lines.map(|l| cols.iter().cloned().zip(fields(l)).collect()).collect()
}

fn hex(s: &str) -> Option<u32> {
    u32::from_str_radix(s.trim().trim_start_matches("0x").trim_start_matches("0X"), 16).ok()
}

/// `{ name = "...", vram = 0x..., size = 0x... }` lines of the symbol file.
fn sizes(syms: &str) -> BTreeMap<u32, u32> {
    let field = |line: &str, key: &str| {
        let i = line.find(key)? + key.len();
        let rest = line[i..].trim_start().strip_prefix('=')?.trim_start();
        hex(rest.split(|c: char| c == ',' || c == ' ' || c == '}').next()?)
    };
    syms.lines().filter_map(|l| Some((field(l, "vram")?, field(l, "size")?))).collect()
}

/// Names in recomp.toml's `ignored = [ ... ]`.
fn ignored(recomp_toml: &str) -> BTreeSet<String> {
    let Some(start) = recomp_toml.find("ignored") else { return BTreeSet::new() };
    let body = &recomp_toml[start..];
    let body = &body[..body.find(']').unwrap_or(body.len())];
    body.lines()
        .filter_map(|l| l.split('#').next())
        .flat_map(|l| l.split('"').skip(1).step_by(2))
        .map(str::to_string)
        .collect()
}

fn load(root: &Path) -> Result<BTreeMap<u32, Func>> {
    let read = |p: &str| {
        fs::read_to_string(root.join(p))
            .map_err(|e| format!("{p}: {e}. Run `cargo xtask find-functions` (and `cargo xtask recomp`) first."))
    };
    let csv = parse_csv(&read("symbols/functions.csv")?);
    let graph = parse_csv(&read("symbols/callgraph.csv")?);
    let sizes = sizes(&read("symbols/racer.syms.toml")?);
    let ignored = ignored(&read("recomp.toml")?);

    let mut funcs = BTreeMap::new();
    for row in &csv {
        let vram = hex(&row["vram"]).ok_or_else(|| format!("functions.csv: bad vram {:?}", row["vram"]))?;
        let name = row["name"].clone();
        // Flags from the generated C: the canonical file name is func_XXXXXXXX
        // (or recomp_entrypoint), whatever the CSV calls the function.
        let gen = root.join("generated").join(format!("func_{vram:08X}.c"));
        let c = fs::read_to_string(&gen).ok();
        funcs.insert(
            vram,
            Func {
                vram,
                depth: row["depth"].parse().map_err(|_| format!("functions.csv: {name} has no depth"))?,
                subsystem: row["subsystem"].clone(),
                status: row["status"].clone(),
                size: sizes.get(&vram).copied().unwrap_or(0),
                callees: BTreeSet::new(),
                indirect: 0,
                ignored: ignored.contains(&format!("func_{vram:08X}")) || ignored.contains(&name),
                uses_float: c.as_deref().is_some_and(|c| c.contains("ctx->f")),
                reads_fcr31: c.as_deref().is_some_and(|c| c.contains("get_cop1_cs")),
                generated: c.is_some(),
                name,
            },
        );
    }
    for row in &graph {
        let caller = hex(&row["caller"]).ok_or("callgraph.csv: bad caller")?;
        let f = funcs.get_mut(&caller).ok_or_else(|| format!("callgraph.csv: unknown caller {caller:#010x}"))?;
        match row["kind"].as_str() {
            "jalr" | "jr" => f.indirect += 1,
            _ => {
                let callee = hex(&row["callee"]).ok_or("callgraph.csv: bad callee")?;
                if callee != caller {
                    f.callees.insert(callee);
                }
            }
        }
    }
    Ok(funcs)
}

fn flags(f: &Func) -> String {
    let mut v = Vec::new();
    if f.uses_float {
        v.push("float");
    }
    if f.reads_fcr31 {
        v.push("reads FCR31: oracle has rounding bits only, check for flag tests");
    }
    if f.in_os_range() {
        v.push("OS range?");
    }
    if !f.generated {
        v.push("no generated C (run `cargo xtask recomp`)");
    }
    v.join(", ")
}

fn blockers(funcs: &BTreeMap<u32, Func>, f: &Func) -> Vec<String> {
    let mut why = Vec::new();
    if f.ignored {
        why.push("not recompilable (ignored in recomp.toml)".to_string());
    }
    if f.indirect > 0 {
        why.push(format!("{} indirect call site(s)", f.indirect));
    }
    let open: Vec<String> =
        f.callees.iter().filter(|c| !funcs.get(c).is_some_and(Func::done)).map(|c| format!("func_{c:08X}")).collect();
    if !open.is_empty() {
        why.push(format!("unverified callees: {}", open.join(" ")));
    }
    why
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let mut count = 10usize;
    let mut subsystem: Option<String> = None;
    let mut toward: Option<u32> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-n" => count = it.next().and_then(|n| n.parse().ok()).ok_or("-n needs a number")?,
            "--subsystem" => subsystem = Some(it.next().ok_or("--subsystem needs a name")?.clone()),
            "--toward" => {
                let t = it.next().ok_or("--toward needs a function")?;
                toward = Some(hex(t.trim_start_matches("func_")).ok_or_else(|| format!("bad function {t:?}"))?);
            }
            other => return Err(format!("next-function: unknown argument {other:?}").into()),
        }
    }
    let funcs = load(root)?;

    // Restrict to the target and everything it reaches through direct edges.
    let scope: Option<BTreeSet<u32>> = match toward {
        None => None,
        Some(t) => {
            let target = funcs.get(&t).ok_or_else(|| format!("no function at {t:#010x}"))?;
            let mut seen = BTreeSet::from([t]);
            let mut work = vec![t];
            while let Some(v) = work.pop() {
                for &c in &funcs[&v].callees {
                    if funcs.contains_key(&c) && seen.insert(c) {
                        work.push(c);
                    }
                }
            }
            let open = seen.iter().filter(|v| !funcs[v].done()).count();
            println!(
                "toward {} (depth {}): reaches {} function(s), {} not yet verified",
                target.name,
                target.depth,
                seen.len(),
                open
            );
            if target.done() {
                println!("  already {}", target.status);
            }
            let stuck: Vec<&Func> =
                seen.iter().map(|v| &funcs[v]).filter(|f| !f.done() && (f.ignored || f.indirect > 0)).collect();
            for f in stuck {
                println!("  blocked below: {} ({})", f.name, blockers(&funcs, f).join("; "));
            }
            Some(seen)
        }
    };

    let mut ready: Vec<&Func> = funcs
        .values()
        .filter(|f| scope.as_ref().is_none_or(|s| s.contains(&f.vram)))
        .filter(|f| f.status == "recomp" && blockers(&funcs, f).is_empty())
        .collect();
    let pref = |f: &Func| subsystem.as_ref().is_some_and(|s| *s != f.subsystem);
    ready.sort_by_key(|f| (pref(f), f.depth, f.in_os_range(), f.uses_float, f.reads_fcr31, f.vram));

    let total = funcs.len();
    let done = funcs.values().filter(|f| f.done()).count();
    println!("{done}/{total} functions verified; {} ready to port\n", ready.len());
    println!("{:<16} {:>5} {:>7}  {:<10} flags", "function", "depth", "size", "subsystem");
    for f in ready.iter().take(count) {
        println!("{:<16} {:>5} {:>#7x}  {:<10} {}", f.name, f.depth, f.size, f.subsystem, flags(f));
    }
    if ready.len() > count {
        println!("... {} more (-n)", ready.len() - count);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quotes() {
        let rows = parse_csv("a,b,c\n1,\"x, \"\"y\"\"\",\n");
        assert_eq!(rows[0]["a"], "1");
        assert_eq!(rows[0]["b"], "x, \"y\"");
        assert_eq!(rows[0]["c"], "");
    }

    #[test]
    fn symbol_sizes_and_ignored() {
        let s = sizes("    { name = \"func_80000554\", vram = 0x80000554, size = 0x5C },\n");
        assert_eq!(s[&0x8000_0554], 0x5C);
        let i = ignored("[patches]\nignored = [\n    \"func_80087CC0\", # cache\n    \"func_8008C550\",\n]\n");
        assert_eq!(i, BTreeSet::from(["func_80087CC0".to_string(), "func_8008C550".to_string()]));
    }
}
