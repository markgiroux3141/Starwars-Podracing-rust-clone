//! Builds the oracle: N64Recomp's generated C for the functions listed in
//! `functions.txt`, a minimal stub runtime, and the layout shims.

use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../..");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    let recomp_dir = env::var_os("RACER_N64RECOMP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("third_party/N64Recomp"));
    let include = recomp_dir.join("include");
    if !include.join("recomp.h").exists() {
        panic!(
            "recomp.h not found under {}. Clone N64Recomp into third_party/ or set RACER_N64RECOMP_DIR.",
            include.display()
        );
    }

    let generated = env::var_os("RACER_GENERATED_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("generated"));
    if !generated.join("funcs.h").exists() {
        panic!(
            "\n\nNo recompiled C found in {}.\n\
             The oracle compiles N64Recomp's output, which is ROM-derived and not committed.\n\
             Generate it with `cargo xtask recomp` (needs baserom.z64, see `cargo xtask verify-rom`,\n\
             and the N64Recomp build in third_party/), or set RACER_GENERATED_DIR.\n\n",
            generated.display()
        );
    }

    let list_path = manifest.join("functions.txt");
    let selected = read_list(&list_path);

    let mut sources = Vec::new();
    let mut callees = BTreeSet::new();
    for name in &selected {
        let path = generated.join(format!("{name}.c"));
        let text = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "{} ({e}). Is {name} in symbols/racer.syms.toml and not ignored in recomp.toml? \
                 Re-run `cargo xtask recomp` if the symbol file changed.",
                path.display()
            )
        });
        callees.extend(direct_callees(&text));
        println!("cargo:rerun-if-changed={}", path.display());
        sources.push(path);
    }

    // Callees that are not compiled in get a stub that traps, so a test that
    // wanders into one fails loudly instead of failing to link.
    let selected_set: BTreeSet<&str> = selected.iter().map(String::as_str).collect();
    let mut stubs = String::from("#include \"recomp.h\"\nvoid oracle_unexpected_call(const char* name);\n");
    for c in callees.iter().filter(|c| !selected_set.contains(c.as_str())) {
        writeln!(
            stubs,
            "RECOMP_FUNC void {c}(uint8_t* rdram, recomp_context* ctx) {{ (void)rdram; (void)ctx; oracle_unexpected_call(\"{c}\"); }}"
        )
        .unwrap();
    }
    let stubs_path = out.join("oracle_callee_stubs.c");
    fs::write(&stubs_path, stubs).unwrap();

    // Rust declarations for the selected functions.
    let mut rs = String::from("extern \"C\" {\n");
    for name in &selected {
        writeln!(rs, "    pub fn {name}(rdram: *mut u8, ctx: *mut game::recomp::RecompContext);").unwrap();
    }
    rs.push_str("}\n\n/// Every function compiled into the oracle, by name.\npub const FUNCTIONS: &[(&str, game::recomp::RecompFn)] = &[\n");
    for name in &selected {
        writeln!(rs, "    (\"{name}\", {name}),").unwrap();
    }
    rs.push_str("];\n");
    fs::write(out.join("oracle_funcs.rs"), rs).unwrap();

    // Shims and stub runtime: our code, warnings on.
    let mut shims = base_build(&include);
    shims.files(["c/layout_shim.c", "c/ctx_shim.c", "c/stub_runtime.c"]).file(&stubs_path);
    shims.compile("oracle_shims");

    // Generated code: warnings off (unused locals in every function).
    if !sources.is_empty() {
        let mut gen = base_build(&include);
        gen.warnings(false).files(&sources);
        gen.compile("oracle_recomp");
    }

    println!("cargo:rerun-if-changed=c");
    println!("cargo:rerun-if-changed={}", list_path.display());
    println!("cargo:rerun-if-changed={}", generated.join("funcs.h").display());
    println!("cargo:rerun-if-changed={}", include.join("recomp.h").display());
    println!("cargo:rerun-if-env-changed=RACER_N64RECOMP_DIR");
    println!("cargo:rerun-if-env-changed=RACER_GENERATED_DIR");
}

fn base_build(include: &Path) -> cc::Build {
    let mut b = cc::Build::new();
    b.include(include).std("c11");
    // Honour the rounding-mode switches recomp.h makes with fesetround (the
    // game's FCR31 idiom, NOTES.md) instead of letting the compiler assume
    // round-to-nearest.
    if b.get_compiler().is_like_msvc() {
        b.flag("/fp:strict");
    } else {
        b.flag("-frounding-math");
    }
    b
}

fn read_list(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        assert!(
            line.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "{}: bad function name {line:?}",
            path.display()
        );
        if !names.iter().any(|n| n == line) {
            names.push(line.to_string());
        }
    }
    names
}

/// Names called as `name(rdram, ctx);`, which is how N64Recomp emits direct
/// calls and tail calls (indirect ones go through LOOKUP_FUNC).
fn direct_callees(text: &str) -> BTreeSet<String> {
    const CALL: &str = "(rdram, ctx);";
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(i) = rest.find(CALL) {
        let before = &rest[..i];
        let start = before.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(0, |p| p + 1);
        let ident = &before[start..];
        if !ident.is_empty() && !ident.starts_with(|c: char| c.is_ascii_digit()) {
            out.insert(ident.to_string());
        }
        rest = &rest[i + CALL.len()..];
    }
    out
}
