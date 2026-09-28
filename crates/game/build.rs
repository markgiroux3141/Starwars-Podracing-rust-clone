//! With the `translated` feature: draft every ported function with the
//! translate crate into `$OUT_DIR/translated.rs` (`game::translated`), so
//! difftest can run the existing difftests against the drafts. The drafts
//! come from generated/ (ROM-derived) and are never committed.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(feature = "translated")]
    translated();
}

#[cfg(feature = "translated")]
fn translated() {
    use std::path::PathBuf;
    use std::{env, fs};

    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let generated = env::var_os("RACER_GENERATED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../generated"));
    let lib = manifest.join("src/lib.rs");
    println!("cargo:rerun-if-changed={}", lib.display());
    println!("cargo:rerun-if-env-changed=RACER_GENERATED_DIR");

    // Every entry of PORTED: `name: "func_XXXXXXXX"`.
    let text = fs::read_to_string(&lib).unwrap();
    let mut sources = Vec::new();
    for part in text.split("name: \"").skip(1) {
        let name = part.split('"').next().unwrap().to_string();
        let path = generated.join(format!("{name}.c"));
        println!("cargo:rerun-if-changed={}", path.display());
        let src = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e}; run `cargo xtask recomp`", path.display()));
        sources.push((name, src));
    }
    assert!(!sources.is_empty(), "found no PORTED entries in src/lib.rs");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("translated.rs");
    fs::write(out, translate::module(&sources)).unwrap();
}
