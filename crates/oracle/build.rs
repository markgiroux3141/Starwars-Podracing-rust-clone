use std::env;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let recomp_dir = env::var_os("RACER_N64RECOMP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../third_party/N64Recomp"));
    let include = recomp_dir.join("include");
    if !include.join("recomp.h").exists() {
        panic!(
            "recomp.h not found under {}. Clone N64Recomp into third_party/ or set RACER_N64RECOMP_DIR.",
            include.display()
        );
    }

    cc::Build::new().file("c/layout_shim.c").include(&include).compile("oracle_layout");

    println!("cargo:rerun-if-changed=c");
    println!("cargo:rerun-if-changed={}", include.join("recomp.h").display());
    println!("cargo:rerun-if-env-changed=RACER_N64RECOMP_DIR");
}
