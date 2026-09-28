//! Project automation: `cargo xtask <command>`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  verify-rom [PATH]   Detect byte order, verify the ROM, write baserom.z64 and
                      record/check rom/EXPECTED.sha1. PATH defaults to
                      baserom.z64, else the single ROM found in rom/.";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("verify-rom") => verify_rom(args.get(1).map(PathBuf::from)),
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
