//! Invariants of the USA ROM's asset blocks (NOTES.md), read from
//! baserom.z64 at test time. Nothing ROM-derived is stored in the repo.
//!
//! This crate builds without the ROM, so the test is skipped (loudly) when
//! baserom.z64 is absent. The oracle, and so `cargo test` as a whole, needs
//! the ROM anyway.

use assets::{fit, Blocks, Fit, Format, MODEL_TAGS};
use std::collections::HashMap;
use std::path::PathBuf;

fn rom() -> Option<Vec<u8>> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baserom.z64");
    match std::fs::read(&p) {
        Ok(r) => {
            assert_eq!(&r[0x20..0x33], b"STAR WARS EP1 RACER", "{} is not the USA z64 ROM", p.display());
            Some(r)
        }
        Err(_) => {
            eprintln!("SKIPPED: {} not found (run `cargo xtask verify-rom`)", p.display());
            None
        }
    }
}

#[test]
fn blocks_tile_and_models_load() {
    let Some(rom) = rom() else { return };
    let b = Blocks::read(&rom).unwrap();
    assert_eq!((b.textures.count, b.splines.count, b.sprites.count, b.models.count), (1648, 91, 173, 307));
    // Each block ends where the next starts, up to 16-byte alignment.
    for (blk, next) in [(&b.textures, b.splines.base), (&b.splines, b.sprites.base), (&b.sprites, b.models.base)] {
        assert!(next >= blk.end() && next - blk.end() < 16, "block {:#x} ends at {:#x}, next at {next:#x}", blk.base, blk.end());
    }
    assert_eq!(b.models.end(), 0x01FF_30F0);

    let mut compressed = 0;
    let mut descs: HashMap<usize, Vec<(u16, u16, u16)>> = HashMap::new();
    for i in 0..b.models.count {
        let m = b.model(i).unwrap();
        assert!(MODEL_TAGS.iter().any(|t| m.tag() == *t), "model {i}: tag {:?}", m.tag());
        assert!(m.mask.len() * 8 >= m.data.len() / 4, "model {i}: mask shorter than the model");
        compressed += usize::from(m.compressed);
        for r in m.texture_refs().into_iter().filter(|r| r.index < b.textures.count) {
            descs.entry(r.index).or_default().push((r.format, r.width, r.height));
        }
    }
    assert_eq!(compressed, 92);

    // Every referenced texture has a descriptor that fits its data, with rows
    // padded to 8 bytes and, for some, a mipmap chain after the first level.
    // Palettes: 16 colours for CI4, 256 for CI8, none otherwise.
    let (mut exact, mut mips) = (0, 0);
    for (&i, ds) in &descs {
        let t = b.texture(i);
        let fits: Vec<(Format, Fit)> = ds
            .iter()
            .filter_map(|&(c, w, h)| Format::from_code(c).map(|f| (f, fit(f, w.into(), h.into(), t.pixels.len()))))
            .filter(|(_, f)| *f != Fit::Mismatch)
            .collect();
        let Some(&(format, f)) = fits.first() else { panic!("texture {i}: no descriptor fits {ds:?}") };
        match f {
            Fit::Exact => exact += 1,
            _ => mips += 1,
        }
        let pal = t.palette.map_or(0, <[u8]>::len);
        let want = match format {
            Format::Ci4 => 32,
            Format::Ci8 => 512,
            _ => 0,
        };
        assert_eq!(pal, want, "texture {i} ({}): palette bytes", format.name());
    }
    assert_eq!((descs.len(), exact, mips), (1603, 1568, 35));
}
