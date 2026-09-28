//! The game's asset blocks, read straight from the ROM.
//!
//! Layout facts are in NOTES.md ("ROM asset loading path"). Four blocks tile
//! ROM 0x0102ABB0..0x01FF30F0. Each starts with a `u32` count and an offset
//! table relative to the block, one past the last entry giving the end.
//! Texture and model entries are pairs; spline and sprite entries are single.
//!
//! This is tooling (extraction, inspection), not the game: it works on byte
//! slices, not RDRAM. Decompression uses [`game::asset::lzss`], which is
//! differentially tested against the original.

use game::asset::lzss;
use std::borrow::Cow;
use std::fmt;

pub const TEXTURE_BLOCK: usize = 0x0102_ABB0;
pub const SPLINE_BLOCK: usize = 0x012C_7F30;
pub const SPRITE_BLOCK: usize = 0x0133_07F0;
pub const MODEL_BLOCK: usize = 0x0141_E200;

/// Top byte of a model word that the loader resolves as a texture reference
/// (`func_800305E8` → `func_800304AC`); the low 24 bits are the index.
pub const TEXTURE_REF_TAG: u8 = 0x0A;
/// Model tags `func_800305E8` accepts.
pub const MODEL_TAGS: [&[u8; 4]; 7] = [b"Modl", b"Trak", b"Podd", b"Part", b"Scen", b"MAlt", b"Pupp"];

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error(msg.into()))
}

fn be32(b: &[u8], at: usize) -> Result<u32> {
    b.get(at..at + 4)
        .map(|w| u32::from_be_bytes(w.try_into().unwrap()))
        .ok_or_else(|| Error(format!("read past the end at {at:#x}")))
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|w| u16::from_be_bytes(w.try_into().unwrap()))
}

/// One block's offset table.
#[derive(Debug)]
pub struct Block<'a> {
    pub base: usize,
    pub count: usize,
    /// Offsets per entry (2 for textures and models).
    pub per_entry: usize,
    offsets: Vec<u32>,
    rom: &'a [u8],
}

impl<'a> Block<'a> {
    pub fn read(rom: &'a [u8], base: usize, per_entry: usize) -> Result<Self> {
        let count = be32(rom, base)? as usize;
        if count > 0x10000 {
            return err(format!("block at {base:#x}: implausible count {count}"));
        }
        let offsets = (0..count * per_entry + 1).map(|k| be32(rom, base + 4 + 4 * k)).collect::<Result<Vec<_>>>()?;
        let end = *offsets.last().unwrap() as usize;
        if base + end > rom.len() {
            return err(format!("block at {base:#x} ends past the ROM ({end:#x})"));
        }
        Ok(Self { base, count, per_entry, offsets, rom })
    }

    /// The bytes from offset `k` of the table to the next nonzero offset
    /// (a zero offset means the part is absent, e.g. a texture without palette).
    fn part(&self, k: usize) -> Option<&'a [u8]> {
        let start = self.offsets[k];
        if start == 0 {
            return None;
        }
        let end = self.offsets[k + 1..].iter().copied().find(|&o| o != 0).unwrap();
        Some(&self.rom[self.base + start as usize..self.base + end as usize])
    }

    /// The end of this block in the ROM.
    pub fn end(&self) -> usize {
        self.base + *self.offsets.last().unwrap() as usize
    }

    /// All parts of entry `i`.
    pub fn entry(&self, i: usize) -> Vec<Option<&'a [u8]>> {
        (0..self.per_entry).map(|p| self.part(i * self.per_entry + p)).collect()
    }
}

pub struct Blocks<'a> {
    pub textures: Block<'a>,
    pub splines: Block<'a>,
    pub sprites: Block<'a>,
    pub models: Block<'a>,
}

impl<'a> Blocks<'a> {
    pub fn read(rom: &'a [u8]) -> Result<Self> {
        Ok(Self {
            textures: Block::read(rom, TEXTURE_BLOCK, 2)?,
            splines: Block::read(rom, SPLINE_BLOCK, 1)?,
            sprites: Block::read(rom, SPRITE_BLOCK, 1)?,
            models: Block::read(rom, MODEL_BLOCK, 2)?,
        })
    }

    pub fn texture(&self, i: usize) -> Texture<'a> {
        let e = self.textures.entry(i);
        Texture { pixels: e[0].unwrap_or(&[]), palette: e[1] }
    }

    pub fn model(&self, i: usize) -> Result<Model<'a>> {
        let e = self.models.entry(i);
        let (mask, raw) = (e[0].unwrap_or(&[]), e[1].unwrap_or(&[]));
        let data = match lzss::parse_comp(raw) {
            Some(c) => {
                let d = lzss::decompress(c.stream).map_err(|e| Error(format!("model {i}: {e}")))?;
                if d.data.len() != c.size as usize {
                    return err(format!("model {i}: decompressed {} bytes, header says {}", d.data.len(), c.size));
                }
                Cow::Owned(d.data)
            }
            None => Cow::Borrowed(raw),
        };
        Ok(Model { mask, data, compressed: raw.starts_with(b"Comp") })
    }
}

#[derive(Clone, Copy)]
pub struct Texture<'a> {
    pub pixels: &'a [u8],
    pub palette: Option<&'a [u8]>,
}

pub struct Model<'a> {
    /// One bit per model word, MSB first: set = the loader relocates it.
    pub mask: &'a [u8],
    /// Decompressed model, starting with its tag.
    pub data: Cow<'a, [u8]>,
    pub compressed: bool,
}

/// A texture reference in a model with the material descriptor around it.
///
/// Offsets relative to the reference word are from the N64 data (NOTES.md):
/// the PC MaterialTexture layout (blender-swe1r facts) with the reference
/// 4 bytes earlier. Checked against every texture's data length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureRef {
    /// Byte offset of the reference word in the model.
    pub at: usize,
    pub index: usize,
    pub format: u16,
    pub width: u16,
    pub height: u16,
}

impl Model<'_> {
    pub fn tag(&self) -> &[u8] {
        &self.data[..4.min(self.data.len())]
    }

    /// Words the loader relocates: (byte offset, value).
    pub fn relocations(&self) -> impl Iterator<Item = (usize, u32)> + '_ {
        let words = self.data.len() / 4;
        (0..words).filter_map(move |i| {
            let bit = self.mask.get(i / 8).is_some_and(|b| b >> (7 - i % 8) & 1 != 0);
            bit.then(|| (4 * i, u32::from_be_bytes(self.data[4 * i..4 * i + 4].try_into().unwrap())))
        })
    }

    pub fn texture_refs(&self) -> Vec<TextureRef> {
        self.relocations()
            .filter(|&(at, w)| (w >> 24) as u8 == TEXTURE_REF_TAG && at >= 0x34)
            .filter_map(|(at, w)| {
                Some(TextureRef {
                    at,
                    index: (w & 0xFF_FFFF) as usize,
                    format: be16(&self.data, at - 0x2C)?,
                    width: be16(&self.data, at - 0x28)?,
                    height: be16(&self.data, at - 0x26)?,
                })
            })
            .collect()
    }
}

/// N64 texel formats as the descriptor encodes them: `(G_IM_FMT << 8) | G_IM_SIZ`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Rgba16,
    Rgba32,
    Ci4,
    Ci8,
    Ia4,
    Ia8,
    Ia16,
    I4,
    I8,
}

impl Format {
    pub fn from_code(code: u16) -> Option<Self> {
        Some(match code {
            0x002 => Format::Rgba16,
            0x003 => Format::Rgba32,
            0x200 => Format::Ci4,
            0x201 => Format::Ci8,
            0x300 => Format::Ia4,
            0x301 => Format::Ia8,
            0x302 => Format::Ia16,
            0x400 => Format::I4,
            0x401 => Format::I8,
            _ => return None,
        })
    }

    pub fn bits(self) -> usize {
        match self {
            Format::Ci4 | Format::Ia4 | Format::I4 => 4,
            Format::Ci8 | Format::Ia8 | Format::I8 => 8,
            Format::Rgba16 | Format::Ia16 => 16,
            Format::Rgba32 => 32,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Format::Rgba16 => "rgba16",
            Format::Rgba32 => "rgba32",
            Format::Ci4 => "ci4",
            Format::Ci8 => "ci8",
            Format::Ia4 => "ia4",
            Format::Ia8 => "ia8",
            Format::Ia16 => "ia16",
            Format::I4 => "i4",
            Format::I8 => "i8",
        }
    }

    /// Bytes per row: rows are padded to 8 bytes (one TMEM line). This fits
    /// every referenced texture in the USA ROM (NOTES.md).
    pub fn stride(self, width: usize) -> usize {
        (width * self.bits()).div_ceil(64) * 8
    }

    pub fn level_size(self, width: usize, height: usize) -> usize {
        self.stride(width) * height
    }
}

/// How a texture's data length compares with its descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Exactly one level.
    Exact,
    /// A chain of levels, each half the size (rounded down, at least 1), rows
    /// padded; `levels` including the first.
    Mipmaps { levels: usize },
    /// Neither; the descriptor doesn't describe this data.
    Mismatch,
}

pub fn fit(format: Format, width: usize, height: usize, len: usize) -> Fit {
    let (mut w, mut h, mut total, mut levels) = (width, height, 0, 0);
    while total < len && w > 0 && h > 0 {
        total += format.level_size(w, h);
        levels += 1;
        if total == len {
            return if levels == 1 { Fit::Exact } else { Fit::Mipmaps { levels } };
        }
        if w == 1 && h == 1 {
            break;
        }
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
    }
    Fit::Mismatch
}

fn rgba5551(c: u16) -> [u8; 4] {
    let x5 = |v: u16| ((v << 3) | (v >> 2)) as u8;
    [x5(c >> 11 & 31), x5(c >> 6 & 31), x5(c >> 1 & 31), if c & 1 != 0 { 255 } else { 0 }]
}

/// Decode the first level to 8-bit RGBA, row-major, top row first.
///
/// Assumptions (NOTES.md, "still guessed"): palettes are RGBA5551 (the TLUT
/// type is set by render state we haven't read); I texels are intensity
/// in all four channels, as TEXEL0 is in the combiner; IA splits as
/// intensity/alpha.
pub fn decode(format: Format, width: usize, height: usize, tex: Texture) -> Result<Vec<u8>> {
    let stride = format.stride(width);
    if tex.pixels.len() < stride * height {
        return err(format!("{} {width}x{height} needs {} bytes, have {}", format.name(), stride * height, tex.pixels.len()));
    }
    let palette = |n: usize| -> Result<Vec<[u8; 4]>> {
        let p = tex.palette.ok_or_else(|| Error(format!("{} texture without a palette", format.name())))?;
        if p.len() < 2 * n {
            return err(format!("palette has {} bytes, {} needs {}", p.len(), format.name(), 2 * n));
        }
        Ok((0..n).map(|k| rgba5551(u16::from_be_bytes([p[2 * k], p[2 * k + 1]]))).collect())
    };
    let pal = match format {
        Format::Ci4 => palette(16)?,
        Format::Ci8 => palette(256)?,
        _ => Vec::new(),
    };
    let mut out = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        let row = &tex.pixels[y * stride..(y + 1) * stride];
        for x in 0..width {
            // 4-bit texels: high nibble first.
            let nib = || if x % 2 == 0 { row[x / 2] >> 4 } else { row[x / 2] & 0xF };
            let px: [u8; 4] = match format {
                Format::Ci4 => pal[usize::from(nib())],
                Format::Ci8 => pal[usize::from(row[x])],
                Format::I4 => [nib() * 17; 4],
                Format::I8 => [row[x]; 4],
                Format::Ia4 => {
                    let v = nib();
                    let i = ((v >> 1) << 5) | ((v >> 1) << 2) | ((v >> 1) >> 1);
                    [i, i, i, if v & 1 != 0 { 255 } else { 0 }]
                }
                Format::Ia8 => {
                    let (i, a) = (row[x] >> 4, row[x] & 0xF);
                    [i * 17, i * 17, i * 17, a * 17]
                }
                Format::Ia16 => [row[2 * x], row[2 * x], row[2 * x], row[2 * x + 1]],
                Format::Rgba16 => rgba5551(u16::from_be_bytes([row[2 * x], row[2 * x + 1]])),
                Format::Rgba32 => row[4 * x..4 * x + 4].try_into().unwrap(),
            };
            out.extend_from_slice(&px);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strides_and_fits() {
        // 74x47 CI4: 37-byte rows padded to 40 (seen in the ROM).
        assert_eq!(Format::Ci4.stride(74), 40);
        assert_eq!(fit(Format::Ci4, 74, 47, 1880), Fit::Exact);
        // 8x8 CI4: 4-byte rows padded to 8.
        assert_eq!(fit(Format::Ci4, 8, 8, 64), Fit::Exact);
        // 32x64 CI4 plus 16x32, 8x16, 4x8, 2x4 levels.
        assert_eq!(fit(Format::Ci4, 32, 64, 1504), Fit::Mipmaps { levels: 5 });
        assert_eq!(fit(Format::Ci4, 32, 64, 1000), Fit::Mismatch);
        assert_eq!(fit(Format::Rgba32, 2, 2, 16), Fit::Exact);
    }

    #[test]
    fn decode_ci4_and_i4() {
        // 2x1 CI4 texels 1 and 0; palette: 0 = transparent black, 1 = opaque white.
        let pixels = [0x10, 0, 0, 0, 0, 0, 0, 0];
        let palette = [0x00, 0x00, 0xFF, 0xFF];
        let t = Texture { pixels: &pixels, palette: Some(&palette) };
        let err = decode(Format::Ci4, 2, 1, t).unwrap_err();
        assert!(err.0.contains("palette"), "{err}"); // CI4 needs all 16 entries
        let mut pal16 = [0u8; 32];
        pal16[2..4].copy_from_slice(&[0xFF, 0xFF]);
        let t = Texture { pixels: &pixels, palette: Some(&pal16) };
        assert_eq!(decode(Format::Ci4, 2, 1, t).unwrap(), [255, 255, 255, 255, 0, 0, 0, 0]);
        let t = Texture { pixels: &[0xF0, 0, 0, 0, 0, 0, 0, 0], palette: None };
        assert_eq!(decode(Format::I4, 2, 1, t).unwrap(), [255, 255, 255, 255, 0, 0, 0, 0]);
    }

    #[test]
    fn format_codes() {
        for (c, f) in [(3, Format::Rgba32), (0x200, Format::Ci4), (0x201, Format::Ci8), (0x400, Format::I4), (0x401, Format::I8)] {
            assert_eq!(Format::from_code(c), Some(f));
        }
        assert_eq!(Format::from_code(0x100), None);
    }
}
