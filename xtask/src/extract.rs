//! `cargo xtask extract`: dump the asset blocks from baserom.z64 into
//! extracted/ (gitignored: everything here is ROM-derived) and decode the
//! textures the models reference to PNG.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::BufWriter;
use std::path::Path;

use assets::{decode, fit, Blocks, Fit, Format};

use crate::Result;

pub fn run(root: &Path, rom_path: &Path) -> Result<()> {
    // Never write ROM-derived files anywhere git would pick them up.
    let ignore = fs::read_to_string(root.join(".gitignore"))?;
    if !ignore.lines().any(|l| l.trim() == "/extracted/") {
        return Err("/extracted/ is not in .gitignore; refusing to write ROM-derived files".into());
    }
    let out = root.join("extracted");
    if out.exists() {
        fs::remove_dir_all(&out)?;
    }
    for d in ["model", "texture", "sprite", "spline", "png"] {
        fs::create_dir_all(out.join(d))?;
    }
    fs::write(out.join("README.txt"), "Extracted from the user's ROM by `cargo xtask extract`. Do not commit or share.\n")?;

    let rom = fs::read(rom_path)?;
    let b = Blocks::read(&rom)?;
    for (name, blk, next) in [
        ("texture", &b.textures, Some(b.splines.base)),
        ("spline", &b.splines, Some(b.sprites.base)),
        ("sprite", &b.sprites, Some(b.models.base)),
        ("model", &b.models, None),
    ] {
        println!(
            "{name:<8} block ROM {:#010x}-{:#010x}: {} entries{}",
            blk.base,
            blk.end(),
            blk.count,
            next.map_or(String::new(), |n| format!(", {} bytes of padding before the next", n - blk.end()))
        );
    }

    for i in 0..b.splines.count {
        fs::write(out.join(format!("spline/{i:03}.bin")), b.splines.entry(i)[0].unwrap_or(&[]))?;
    }
    for i in 0..b.sprites.count {
        fs::write(out.join(format!("sprite/{i:03}.bin")), b.sprites.entry(i)[0].unwrap_or(&[]))?;
    }
    for i in 0..b.textures.count {
        let t = b.texture(i);
        fs::write(out.join(format!("texture/{i:04}.pixels")), t.pixels)?;
        if let Some(p) = t.palette {
            fs::write(out.join(format!("texture/{i:04}.palette")), p)?;
        }
    }

    // Models: decompress, and gather texture descriptors per texture index.
    let mut descs: BTreeMap<usize, HashMap<(u16, u16, u16), usize>> = BTreeMap::new();
    let mut users: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let (mut compressed, mut dangling) = (0, 0);
    for i in 0..b.models.count {
        let m = b.model(i)?;
        let tag = String::from_utf8_lossy(m.tag()).into_owned();
        if !assets::MODEL_TAGS.iter().any(|t| m.tag() == *t) {
            return Err(format!("model {i}: unexpected tag {tag:?}").into());
        }
        compressed += usize::from(m.compressed);
        fs::write(out.join(format!("model/{i:03}_{tag}.bin")), &*m.data)?;
        fs::write(out.join(format!("model/{i:03}.mask")), m.mask)?;
        for r in m.texture_refs() {
            if r.index >= b.textures.count {
                // texture_get writes null pointers for these.
                dangling += 1;
                continue;
            }
            *descs.entry(r.index).or_default().entry((r.format, r.width, r.height)).or_default() += 1;
            let u = users.entry(r.index).or_default();
            if u.last() != Some(&i) {
                u.push(i);
            }
        }
    }
    println!(
        "models: {} ({compressed} compressed), {} texture references to {} textures, {dangling} past the end of the table",
        b.models.count,
        descs.values().flat_map(|d| d.values()).sum::<usize>(),
        descs.len()
    );

    // Textures: pick the descriptor that fits the data (exact beats mipmaps,
    // then the most used), decode its first level.
    let mut csv = String::from("index,format,width,height,pixel_bytes,palette_bytes,fit,descriptors,models,png\n");
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for i in 0..b.textures.count {
        let t = b.texture(i);
        let pal_len = t.palette.map_or(0, <[u8]>::len);
        let Some(d) = descs.get(&i) else {
            csv.push_str(&format!("{i},,,,{},{pal_len},unreferenced,0,,\n", t.pixels.len()));
            *tally.entry("unreferenced".into()).or_default() += 1;
            continue;
        };
        let mut cands: Vec<((u16, u16, u16), usize, Fit)> = d
            .iter()
            .map(|(&k, &n)| {
                let f = Format::from_code(k.0)
                    .map_or(Fit::Mismatch, |f| fit(f, usize::from(k.1), usize::from(k.2), t.pixels.len()));
                (k, n, f)
            })
            .collect();
        let rank = |f: &Fit| match f {
            Fit::Exact => 0,
            Fit::Mipmaps { .. } => 1,
            Fit::Mismatch => 2,
        };
        cands.sort_by_key(|&(k, n, f)| (rank(&f), std::cmp::Reverse(n), k));
        let ((code, w, h), _, f) = cands[0];
        let fit_s = match f {
            Fit::Exact => "exact".to_string(),
            Fit::Mipmaps { levels } => format!("mipmaps:{levels}"),
            Fit::Mismatch => "mismatch".to_string(),
        };
        *tally.entry(fit_s.split(':').next().unwrap().to_string()).or_default() += 1;
        let format = Format::from_code(code);
        let fname = format.map(|f| f.name()).unwrap_or("unknown");
        let mut png_name = String::new();
        if let (Some(format), false) = (format, f == Fit::Mismatch) {
            let rgba = decode(format, usize::from(w), usize::from(h), t)?;
            png_name = format!("tex_{i:04}_{fname}_{w}x{h}.png");
            write_png(&out.join("png").join(&png_name), u32::from(w), u32::from(h), &rgba)?;
        }
        let models: Vec<String> = users[&i].iter().map(usize::to_string).collect();
        csv.push_str(&format!(
            "{i},{code:#x},{w},{h},{},{pal_len},{fit_s},{},{},{png_name}\n",
            t.pixels.len(),
            d.len(),
            models.join(" ")
        ));
    }
    fs::write(out.join("textures.csv"), csv)?;
    let pngs = fs::read_dir(out.join("png"))?.count();
    println!(
        "textures: {} ({}); {pngs} PNGs in extracted/png, index in extracted/textures.csv",
        b.textures.count,
        tally.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")
    );
    Ok(())
}

fn write_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    let file = BufWriter::new(fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}
