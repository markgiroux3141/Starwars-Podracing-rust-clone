//! Project automation: `cargo xtask <command>`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::{env, fs};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  verify-rom [PATH]   Detect byte order, verify the ROM, write baserom.z64 and
                      record/check rom/EXPECTED.sha1. PATH defaults to
                      baserom.z64, else the single ROM found in rom/.
  find-functions      Regenerate symbols/racer.syms.toml and symbols/functions.csv
                      with tools/find_functions.py (needs the .venv).
  recomp              Run N64Recomp with recomp.toml into generated/ (replacing
                      it). N64Recomp.exe defaults to third_party/N64Recomp/build;
                      override with RACER_N64RECOMP_EXE.";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("verify-rom") => verify_rom(args.get(1).map(PathBuf::from)),
        Some("find-functions") => find_functions(),
        Some("recomp") => recomp(),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn find_rom(root: &Path) -> Result<PathBuf> {
    let base = root.join("baserom.z64");
    if base.exists() {
        return Ok(base);
    }
    let rom_dir = root.join("rom");
    let found: Vec<PathBuf> = fs::read_dir(&rom_dir)
        .map_err(|e| format!("no baserom.z64 and can't read {}: {e}", rom_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| ["z64", "v64", "n64"].contains(&x.to_ascii_lowercase().as_str()))
        })
        .collect();
    match found.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err("no ROM found: place it at baserom.z64 or in rom/, or pass a path".into()),
        _ => Err("several ROMs in rom/; pass the path explicitly".into()),
    }
}

fn verify_rom(path: Option<PathBuf>) -> Result<()> {
    let root = repo_root();
    let src = match path {
        Some(p) => p,
        None => find_rom(&root)?,
    };
    println!("ROM:          {}", src.display());

    let rom = racer_rom::verify(fs::read(&src)?)?;
    let h = &rom.header;
    println!("byte order:   {}", rom.original_order.name());
    println!("name:         {}", h.name);
    println!("game code:    {} (version {})", h.game_code, h.version);
    println!("entry point:  {:#010X}", h.entry_point);
    println!("header CRCs:  {:08X} {:08X}", h.crc1, h.crc2);
    println!("boot CIC:     {} (checksum recomputed from data: OK)", rom.cic.name());
    println!("SHA-1 (z64):  {}", rom.sha1);

    let base = root.join("baserom.z64");
    if fs::read(&base).ok().as_deref() == Some(&rom.data[..]) {
        println!("baserom.z64:  up to date");
    } else {
        fs::write(&base, &rom.data)?;
        println!("baserom.z64:  written");
    }

    let sha_path = root.join("rom").join("EXPECTED.sha1");
    match fs::read_to_string(&sha_path) {
        Ok(recorded) if recorded.trim() == rom.sha1 => println!("EXPECTED.sha1: matches"),
        Ok(recorded) => {
            return Err(format!(
                "SHA-1 {} differs from the one recorded in {} ({}). This is not the ROM earlier sessions used.",
                rom.sha1,
                sha_path.display(),
                recorded.trim()
            )
            .into())
        }
        Err(_) => {
            fs::create_dir_all(sha_path.parent().unwrap())?;
            fs::write(&sha_path, format!("{}\n", rom.sha1))?;
            println!("EXPECTED.sha1: recorded");
        }
    }
    println!("\nROM verified.");
    Ok(())
}

/// The ROM at baserom.z64 must be the verified one recorded in rom/EXPECTED.sha1.
fn check_baserom(root: &Path) -> Result<PathBuf> {
    let base = root.join("baserom.z64");
    let data = fs::read(&base)
        .map_err(|e| format!("can't read {}: {e}. Run `cargo xtask verify-rom` first.", base.display()))?;
    let rom = racer_rom::verify(data)?;
    let expected = fs::read_to_string(root.join("rom").join("EXPECTED.sha1"))
        .map_err(|_| "rom/EXPECTED.sha1 missing; run `cargo xtask verify-rom` first")?;
    if expected.trim() != rom.sha1 {
        return Err(format!("baserom.z64 SHA-1 {} is not the one in rom/EXPECTED.sha1", rom.sha1).into());
    }
    Ok(base)
}

fn find_functions() -> Result<()> {
    let root = repo_root();
    check_baserom(&root)?;
    let py = if cfg!(windows) { root.join(".venv/Scripts/python.exe") } else { root.join(".venv/bin/python") };
    if !py.exists() {
        return Err(format!("{} not found; create the venv (rabbitizer) first", py.display()).into());
    }
    let status = Command::new(py).arg(root.join("tools/find_functions.py")).current_dir(&root).status()?;
    if !status.success() {
        return Err(format!("find_functions.py failed ({status})").into());
    }
    Ok(())
}

fn n64recomp_exe(root: &Path) -> Result<PathBuf> {
    if let Some(p) = env::var_os("RACER_N64RECOMP_EXE") {
        return Ok(PathBuf::from(p));
    }
    let build = root.join("third_party/N64Recomp/build");
    let candidates = [
        build.join("Release/N64Recomp.exe"),
        build.join("RelWithDebInfo/N64Recomp.exe"),
        build.join("Debug/N64Recomp.exe"),
        build.join("N64Recomp"),
        build.join("N64Recomp.exe"),
    ];
    candidates.into_iter().find(|p| p.exists()).ok_or_else(|| {
        format!("N64Recomp not found under {}; build it (NOTES.md) or set RACER_N64RECOMP_EXE", build.display()).into()
    })
}

fn recomp() -> Result<()> {
    let root = repo_root();
    check_baserom(&root)?;
    let exe = n64recomp_exe(&root)?;
    let out = root.join("generated");
    // Start clean so files from an older symbol file can't linger.
    if out.exists() {
        fs::remove_dir_all(&out)?;
    }
    println!("running {} recomp.toml", exe.display());
    let output = Command::new(&exe).arg("recomp.toml").current_dir(&root).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stdout.lines().chain(stderr.lines()) {
        // Tail calls are expected and listed by find_functions.py; show the rest.
        if !line.contains("Tail call in") {
            println!("  {line}");
        }
    }
    if !output.status.success() {
        return Err(format!("N64Recomp failed ({}). Fix the analysis in tools/find_functions.py, not the output.", output.status).into());
    }

    // Our symbol file must account for every call target. If N64Recomp had to
    // invent static functions or found branches leaving a function, the
    // boundaries are wrong.
    let mut funcs = 0;
    let mut statics = Vec::new();
    for e in fs::read_dir(&out)? {
        let name = e?.file_name().to_string_lossy().into_owned();
        if name.ends_with(".c") {
            funcs += 1;
            if name.starts_with("static_") {
                statics.push(name);
            }
        }
    }
    let warned = stderr.lines().chain(stdout.lines()).filter(|l| l.contains("[Warn]")).count();
    if !statics.is_empty() || warned > 0 {
        return Err(format!(
            "N64Recomp created {} static function(s) {:?} and printed {warned} warning(s): the symbol file misses boundaries",
            statics.len(),
            statics
        )
        .into());
    }
    println!("generated/: {funcs} functions, no statics, no warnings");
    Ok(())
}
