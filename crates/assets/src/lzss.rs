//! The "Comp"/"Wolf" LZSS format over byte slices, for reading assets
//! straight from the ROM (NOTES.md, "Asset compression").
//!
//! This is the algorithm of `func_80011940`, whose register-exact port is
//! `game::asset::func_80011940`. `crates/difftest/tests/func_80011940.rs`
//! checks this version against the recompiled C as well.

/// Ring buffer size and initial write position (`func_80011940`, NOTES.md).
pub const WINDOW: usize = 0x1000;
const START: usize = 1;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The stream ended before its terminator.
    Truncated { at: usize },
    /// A reference read a window byte nothing had written yet. On the N64
    /// that is whatever RDRAM held below the input. No real asset does it
    /// (NOTES.md), so this refuses rather than guess.
    UnwrittenWindow { at: usize },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Truncated { at } => write!(f, "LZSS stream truncated at input byte {at:#x}"),
            Error::UnwrittenWindow { at } => {
                write!(f, "LZSS reference at input byte {at:#x} reads the uninitialised window")
            }
        }
    }
}

impl std::error::Error for Error {}

/// A decompressed stream and how many input bytes it used, terminator included.
#[derive(Debug)]
pub struct Decompressed {
    pub data: Vec<u8>,
    pub consumed: usize,
}

/// Decompress an LZSS stream (the bytes after the 12-byte "Comp" header).
pub fn decompress(input: &[u8]) -> Result<Decompressed, Error> {
    let mut window = [0u8; WINDOW];
    let mut written = [false; WINDOW];
    let mut pos = START;
    let mut out = Vec::new();
    let mut i = 0;
    let byte = |i: usize| input.get(i).copied().ok_or(Error::Truncated { at: i });
    loop {
        let flags = byte(i)?;
        i += 1;
        for bit in 0..8 {
            if flags >> bit & 1 != 0 {
                let b = byte(i)?;
                i += 1;
                out.push(b);
                window[pos] = b;
                written[pos] = true;
                pos = (pos + 1) % WINDOW;
            } else {
                let (b0, b1) = (byte(i)?, byte(i + 1)?);
                let at = i;
                i += 2;
                let offset = usize::from(b0 & 0xF) << 8 | usize::from(b1);
                if offset == 0 {
                    return Ok(Decompressed { data: out, consumed: i });
                }
                for k in 0..usize::from(b0 >> 4) + 2 {
                    let src = (offset + k) % WINDOW;
                    if !written[src] {
                        return Err(Error::UnwrittenWindow { at });
                    }
                    let b = window[src];
                    out.push(b);
                    window[pos] = b;
                    written[pos] = true;
                    pos = (pos + 1) % WINDOW;
                }
            }
        }
    }
}

/// A "Comp" block: the 12-byte header and its stream.
#[derive(Debug, PartialEq, Eq)]
pub struct Comp<'a> {
    /// Bytes 4..8. "Wolf" in every block of the USA ROM.
    pub tag: [u8; 4],
    /// Declared decompressed size.
    pub size: u32,
    pub stream: &'a [u8],
}

/// Recognise a "Comp" block, as the model loader does (first word only).
pub fn parse_comp(block: &[u8]) -> Option<Comp<'_>> {
    if block.len() < 12 || &block[..4] != b"Comp" {
        return None;
    }
    Some(Comp {
        tag: block[4..8].try_into().unwrap(),
        size: u32::from_be_bytes(block[8..12].try_into().unwrap()),
        stream: &block[12..],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_references_and_terminator() {
        // flags 0b0000_0111: three literals, then a reference to offset 1
        // (the first literal) of length 5 that overlaps itself, then the end.
        let s = [0x07, b'a', b'b', b'c', 0x30, 0x01, 0x00, 0x00];
        let d = decompress(&s).unwrap();
        assert_eq!(d.data, b"abcabcab");
        assert_eq!(d.consumed, s.len());
    }

    #[test]
    fn errors() {
        assert_eq!(decompress(&[0x01]).unwrap_err(), Error::Truncated { at: 1 });
        assert_eq!(decompress(&[0x00, 0x00, 0x05]).unwrap_err(), Error::UnwrittenWindow { at: 1 });
        assert!(parse_comp(b"Comp").is_none());
        let c = parse_comp(b"CompWolf\x00\x00\x01\x00xyz").unwrap();
        assert_eq!((c.tag, c.size, c.stream), (*b"Wolf", 0x100, &b"xyz"[..]));
    }
}
