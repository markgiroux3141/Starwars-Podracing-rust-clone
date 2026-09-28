/// The fields of the 64-byte cartridge header we care about. Input must be z64.
#[derive(Debug, Clone)]
pub struct Header {
    pub clock_rate: u32,
    pub entry_point: u32,
    pub libultra_version: u32,
    pub crc1: u32,
    pub crc2: u32,
    /// Internal name (0x20..0x34), trailing spaces/NULs trimmed.
    pub name: String,
    /// Media type + 2-char ID + region, e.g. `NEPE`.
    pub game_code: String,
    pub version: u8,
}

impl Header {
    pub fn parse(rom: &[u8]) -> Self {
        let be32 = |o: usize| u32::from_be_bytes(rom[o..o + 4].try_into().unwrap());
        let ascii = |r: std::ops::Range<usize>| -> String {
            rom[r].iter().map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { ' ' }).collect()
        };
        Self {
            clock_rate: be32(0x04),
            entry_point: be32(0x08),
            libultra_version: be32(0x0C),
            crc1: be32(0x10),
            crc2: be32(0x14),
            name: ascii(0x20..0x34).trim_end().to_string(),
            game_code: ascii(0x3B..0x3F),
            version: rom[0x3F],
        }
    }
}
