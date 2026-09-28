//! The game's asset blocks, read straight from the ROM.
//!
//! Layout facts are in NOTES.md ("ROM asset loading path"). Four blocks tile
//! ROM 0x0102ABB0..0x01FF30F0. Each starts with a `u32` count and an offset
//! table relative to the block, one past the last entry giving the end.
//! Texture and model entries are pairs; spline and sprite entries are single.
//!
//! This is tooling (extraction, inspection), not the game: it works on byte
//! slices, not RDRAM. Decompression is [`lzss`], which is differentially
//! tested against the original.

pub mod lzss;

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

/// A sprite from the sprite block (NOTES.md, "Sprites"): a header, a table of
/// pages that tile the image, an optional palette, then the pages' texels.
///
/// Header (0x14 bytes, big-endian): `+0` u16 width, `+2` u16 height, `+4` u8
/// `G_IM_FMT`, `+5` u8 `G_IM_SIZ`, `+6` u16 (0), `+8` u32 palette offset (0 =
/// none), `+0xC` s16 page count, `+0xE` u16 (always 32), `+0x10` u32 page
/// table offset (always 0x14). Offsets are from the start of the sprite. The
/// loader (`func_8002FF38`) reads only `+4`, `+8` and `+0xC`, and patches
/// `+8`, `+0x10` and each page's offset into pointers.
#[derive(Debug)]
pub struct Sprite<'a> {
    pub width: u16,
    pub height: u16,
    /// `(G_IM_FMT << 8) | G_IM_SIZ`, as in texture descriptors.
    pub format_code: u16,
    pub palette: Option<&'a [u8]>,
    pub pages: Vec<SpritePage<'a>>,
}

/// One page: 8 bytes in the table, u16 width, u16 height, u32 texel offset.
#[derive(Debug)]
pub struct SpritePage<'a> {
    pub width: u16,
    pub height: u16,
    pub texels: &'a [u8],
}

impl<'a> Sprite<'a> {
    pub fn parse(s: &'a [u8]) -> Result<Self> {
        if s.len() < 0x14 {
            return err(format!("sprite of {} bytes has no header", s.len()));
        }
        let h16 = |at: usize| be16(s, at).unwrap();
        let (width, height) = (h16(0), h16(2));
        let format_code = u16::from(s[4]) << 8 | u16::from(s[5]);
        let pal_off = be32(s, 8)? as usize;
        let count = h16(0xC) as i16;
        // The loader takes the table from +0x14 whatever +0x10 says.
        let entry = |k: usize| -> Result<(u16, u16, usize)> {
            let at = 0x14 + 8 * k;
            let (w, h) = (be16(s, at), be16(s, at + 2));
            Ok((w.ok_or_else(|| Error("page table truncated".into()))?, h.unwrap(), be32(s, at + 4)? as usize))
        };
        let n = count.max(0) as usize;
        let table = (0..n).map(entry).collect::<Result<Vec<_>>>()?;
        let mut pages = Vec::with_capacity(n);
        for (k, &(w, h, off)) in table.iter().enumerate() {
            // Like the loader: a page runs to the next page, the last to the
            // end of the sprite.
            let end = table.get(k + 1).map_or(s.len(), |t| t.2);
            let texels = s.get(off..end).ok_or_else(|| Error(format!("page {k}: {off:#x}..{end:#x} out of range")))?;
            pages.push(SpritePage { width: w, height: h, texels });
        }
        let palette = match pal_off {
            0 => None,
            p => {
                let end = table.first().map_or(s.len(), |t| t.2);
                Some(s.get(p..end).ok_or_else(|| Error(format!("palette {p:#x}..{end:#x} out of range")))?)
            }
        };
        Ok(Self { width, height, format_code, palette, pages })
    }

    pub fn format(&self) -> Option<Format> {
        Format::from_code(self.format_code)
    }

    /// Pages tile the image in rows of `page[0].height` pixels, left to right
    /// and top to bottom: `rows = ceil(height / page height)` and
    /// `columns = pages / rows`. (Sprite 145 declares width 193 but its pages
    /// cover 192; its last column stays transparent.)
    pub fn layout(&self) -> Result<Vec<(usize, usize)>> {
        let Some(p0) = self.pages.first() else { return Ok(Vec::new()) };
        let (tw, th) = (usize::from(p0.width), usize::from(p0.height));
        if th == 0 || tw == 0 {
            return err("zero-sized first page");
        }
        let rows = usize::from(self.height).div_ceil(th);
        if rows == 0 || self.pages.len() % rows != 0 {
            return err(format!("{} pages don't make {rows} rows", self.pages.len()));
        }
        let cols = self.pages.len() / rows;
        Ok((0..self.pages.len()).map(|k| (k % cols * tw, k / cols * th)).collect())
    }
}

impl<'a> Blocks<'a> {
    pub fn sprite(&self, i: usize) -> Result<Sprite<'a>> {
        Sprite::parse(self.sprites.entry(i)[0].unwrap_or(&[])).map_err(|e| Error(format!("sprite {i}: {e}")))
    }
}

/// Decode a whole sprite to 8-bit RGBA (`width * height`), each page with
/// [`decode`] and its assumptions. Pixels no page covers are transparent.
pub fn decode_sprite(s: &Sprite) -> Result<Vec<u8>> {
    let format = s.format().ok_or_else(|| Error(format!("sprite format {:#x}", s.format_code)))?;
    let (w, h) = (usize::from(s.width), usize::from(s.height));
    let mut out = vec![0u8; w * h * 4];
    for (page, (x0, y0)) in s.pages.iter().zip(s.layout()?) {
        let (pw, ph) = (usize::from(page.width), usize::from(page.height));
        if x0 + pw > w || y0 + ph > h {
            return err(format!("page at ({x0}, {y0}) {pw}x{ph} is outside the {w}x{h} sprite"));
        }
        let rgba = decode(format, pw, ph, Texture { pixels: page.texels, palette: s.palette })?;
        for y in 0..ph {
            let n = pw.min(w - x0);
            let dst = ((y0 + y) * w + x0) * 4;
            out[dst..dst + n * 4].copy_from_slice(&rgba[y * pw * 4..(y * pw + n) * 4]);
        }
    }
    Ok(out)
}

/// Size of one spline point.
pub const SPLINE_POINT_SIZE: usize = 0x54;

/// A spline from the spline block (NOTES.md, "Splines"): a 16-byte header
/// and `count` points of [`SPLINE_POINT_SIZE`] bytes, big-endian.
///
/// Header: `+0` u32 unknown, `+4` u32 point count, `+8` u32 segment count
/// (points plus one per extra successor at a fork), `+0xC` u32 stale: the
/// loader (`func_80030174`) overwrites it with the address of the first
/// point.
#[derive(Debug, Clone, PartialEq)]
pub struct Spline {
    pub unknown: u32,
    pub segments: u32,
    pub points: Vec<SplinePoint>,
}

/// One point. `+0` u16 successor count and `+2` predecessor count; the
/// successors' indices from `+4` (two slots), the predecessors' from `+8`
/// (up to three used, so the list runs into `+0xC`). Slots past the counts
/// hold stale bytes (often ASCII text) and are not links. Then four f32
/// triples at `+0x10`, `+0x1C`, `+0x28`, `+0x34`: the position, an
/// often-(0, 0, 1) vector, and two points that look like Bézier handles
/// (**guess**). Then ten i16 at `+0x40`: usually the point's own index
/// twice, the ids of its extra segments (`next.len() - 1` of them, each in
/// `count..segments`), and -1 for none.
#[derive(Debug, Clone, PartialEq)]
pub struct SplinePoint {
    pub next: Vec<u16>,
    pub prev: Vec<u16>,
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub handles: [[f32; 3]; 2],
    pub ids: [i16; 10],
}

impl Spline {
    pub fn parse(s: &[u8]) -> Result<Self> {
        if s.len() < 0x10 {
            return err(format!("spline of {} bytes has no header", s.len()));
        }
        let count = be32(s, 4)? as usize;
        if s.len() != 0x10 + SPLINE_POINT_SIZE * count {
            return err(format!("{} bytes for {count} points", s.len()));
        }
        let h16 = |at: usize| be16(s, at).unwrap();
        let f = |at: usize| f32::from_bits(be32(s, at).unwrap());
        let v3 = |at: usize| [f(at), f(at + 4), f(at + 8)];
        let mut points = Vec::with_capacity(count);
        for k in 0..count {
            let p = 0x10 + SPLINE_POINT_SIZE * k;
            let (n_next, n_prev) = (usize::from(h16(p)), usize::from(h16(p + 2)));
            if n_next > 2 || n_prev > 4 {
                return err(format!("point {k}: {n_next} successors, {n_prev} predecessors"));
            }
            let next: Vec<u16> = (0..n_next).map(|j| h16(p + 4 + 2 * j)).collect();
            let prev: Vec<u16> = (0..n_prev).map(|j| h16(p + 8 + 2 * j)).collect();
            if let Some(j) = next.iter().chain(&prev).find(|&&j| usize::from(j) >= count) {
                return err(format!("point {k}: link to {j} of {count}"));
            }
            let ids = std::array::from_fn(|j| h16(p + 0x40 + 2 * j) as i16);
            points.push(SplinePoint {
                next,
                prev,
                position: v3(p + 0x10),
                normal: v3(p + 0x1C),
                handles: [v3(p + 0x28), v3(p + 0x34)],
                ids,
            });
        }
        Ok(Self { unknown: be32(s, 0)?, segments: be32(s, 8)?, points })
    }
}

impl<'a> Blocks<'a> {
    pub fn spline(&self, i: usize) -> Result<Spline> {
        Spline::parse(self.splines.entry(i)[0].unwrap_or(&[])).map_err(|e| Error(format!("spline {i}: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spline_links_and_sizes() {
        // Two points in a loop, synthetic.
        let mut s = vec![0u8; 0x10 + 2 * SPLINE_POINT_SIZE];
        s[4..8].copy_from_slice(&2u32.to_be_bytes());
        s[8..12].copy_from_slice(&2u32.to_be_bytes());
        for (k, other) in [(0usize, 1u16), (1, 0)] {
            let p = 0x10 + SPLINE_POINT_SIZE * k;
            s[p..p + 2].copy_from_slice(&1u16.to_be_bytes());
            s[p + 2..p + 4].copy_from_slice(&1u16.to_be_bytes());
            s[p + 4..p + 6].copy_from_slice(&other.to_be_bytes());
            s[p + 6..p + 8].copy_from_slice(b"  "); // stale bytes past the count
            s[p + 8..p + 10].copy_from_slice(&other.to_be_bytes());
            s[p + 0x10..p + 0x14].copy_from_slice(&1.5f32.to_bits().to_be_bytes());
        }
        let sp = Spline::parse(&s).unwrap();
        assert_eq!(sp.points[0].next, [1]);
        assert_eq!(sp.points[1].prev, [0]);
        assert_eq!(sp.points[0].position[0], 1.5);
        assert!(Spline::parse(&s[..s.len() - 1]).is_err());
        s[0x10 + 4..0x10 + 6].copy_from_slice(&2u16.to_be_bytes()); // link out of range
        assert!(Spline::parse(&s).is_err());
    }

    #[test]
    fn sprite_pages_tile_rows_first() {
        // 3x3 I8 sprite in 2x2 pages: 4 pages, rows of 2.
        let mut s = vec![0u8; 0x14];
        s[..4].copy_from_slice(&[0, 3, 0, 3]);
        s[4..6].copy_from_slice(&[4, 1]); // I8
        s[0xC..0xE].copy_from_slice(&4i16.to_be_bytes());
        let pages = [(2u16, 2u16), (1, 2), (2, 1), (1, 1)];
        let mut data = Vec::new();
        let mut off = 0x14 + 8 * pages.len();
        for (k, &(pw, ph)) in pages.iter().enumerate() {
            s.extend_from_slice(&pw.to_be_bytes());
            s.extend_from_slice(&ph.to_be_bytes());
            s.extend_from_slice(&(off as u32).to_be_bytes());
            // I8 rows padded to 8 bytes; texel value = page number * 10 + x.
            for _ in 0..ph {
                let mut row = [0u8; 8];
                for (x, r) in row.iter_mut().enumerate().take(usize::from(pw)) {
                    *r = (k * 10 + x) as u8;
                }
                data.extend_from_slice(&row);
            }
            off += 8 * usize::from(ph);
        }
        s.extend_from_slice(&data);
        let sp = Sprite::parse(&s).unwrap();
        assert_eq!(sp.layout().unwrap(), [(0, 0), (2, 0), (0, 2), (2, 2)]);
        let rgba = decode_sprite(&sp).unwrap();
        let px = |x: usize, y: usize| rgba[(y * 3 + x) * 4];
        assert_eq!([px(0, 0), px(1, 0), px(2, 0)], [0, 1, 10]);
        assert_eq!([px(0, 2), px(1, 2), px(2, 2)], [20, 21, 30]);
    }

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
