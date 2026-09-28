//! `cargo xtask translate`: draft Rust ports from N64Recomp's C (the
//! `translate` crate), and a survey of what it can translate.

use crate::next_function::parse_csv;
use crate::Result;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Where the OS/libultra range starts (NOTES.md, "Code segment").
const OS_START: u32 = 0x8008_7CC0;

fn generated(root: &Path, name: &str) -> Result<String> {
    let path = root.join("generated").join(format!("{name}.c"));
    fs::read_to_string(&path).map_err(|e| {
        format!("{}: {e}. Generate the C with `cargo xtask recomp` (generated/ is not committed).", path.display()).into()
    })
}

fn func_name(arg: &str) -> Result<String> {
    let hex = arg.strip_prefix("func_").or_else(|| arg.strip_prefix("0x")).unwrap_or(arg);
    let v = u32::from_str_radix(hex, 16).map_err(|_| format!("not a function name or address: {arg}"))?;
    Ok(format!("func_{v:08X}"))
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) == Some("--survey") {
        return survey(root, &args[1..]);
    }
    if args.is_empty() {
        return Err("usage: cargo xtask translate func_XXXXXXXX... | --survey [--depth N] [--list KIND]".into());
    }
    let mut failed = false;
    for a in args {
        let name = func_name(a)?;
        match translate::translate(&generated(root, &name)?) {
            Ok(d) => {
                println!("// ---- {name}: draft (restructure and verify before use) ----");
                if !d.structured {
                    println!("// NOTE: irreducible control flow; emitted as a state machine.");
                }
                if !d.callees.is_empty() {
                    println!("// Callees (must be in game::imports): {}", d.callees.join(", "));
                }
                print!("{}\n{}", d.uses, d.source);
            }
            Err(r) => {
                eprintln!("{name}: can't translate: {r}");
                failed = true;
            }
        }
    }
    if failed {
        Err("some functions were refused".into())
    } else {
        Ok(())
    }
}

/// Try every game-side function (below the OS range) and count outcomes.
fn survey(root: &Path, args: &[String]) -> Result<()> {
    let mut depth: Option<u32> = None;
    let mut list: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--depth" => depth = Some(it.next().ok_or("--depth needs a value")?.parse()?),
            "--list" => list = Some(it.next().ok_or("--list needs a kind")?.clone()),
            _ => return Err(format!("unknown argument {a}").into()),
        }
    }
    let csv = parse_csv(&fs::read_to_string(root.join("symbols/functions.csv"))?);
    let mut total = 0;
    let mut done = 0;
    let mut ok = 0;
    let mut machines = Vec::new();
    let mut refused: BTreeMap<&'static str, Vec<(String, String)>> = BTreeMap::new();
    for row in &csv {
        let vram = u32::from_str_radix(row["vram"].trim_start_matches("0x"), 16)?;
        if vram >= OS_START || row["depth"].is_empty() {
            continue;
        }
        if depth.is_some_and(|d| row["depth"].parse::<u32>().ok() != Some(d)) {
            continue;
        }
        let name = format!("func_{vram:08X}");
        let Ok(src) = generated(root, &name) else { continue }; // ignored in recomp.toml
        total += 1;
        if matches!(row["status"].as_str(), "rust_verified" | "lifted") {
            done += 1;
        }
        match translate::translate(&src) {
            Ok(d) => {
                ok += 1;
                if !d.structured {
                    machines.push(name);
                }
            }
            Err(r) => refused.entry(r.kind).or_default().push((name, r.detail)),
        }
    }
    let scope = depth.map_or("game-side".to_string(), |d| format!("depth-{d} game-side"));
    println!("{total} {scope} functions ({done} already verified)");
    println!("  translated: {ok} ({} as state machines{})", machines.len(), if machines.is_empty() { String::new() } else { format!(": {}", machines.join(" ")) });
    let mut kinds: Vec<_> = refused.iter().collect();
    kinds.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    for (kind, v) in kinds {
        println!("  refused, {kind}: {}", v.len());
        if list.as_deref() == Some(*kind) {
            for (n, d) in v {
                println!("      {n}: {d}");
            }
        }
    }
    Ok(())
}
