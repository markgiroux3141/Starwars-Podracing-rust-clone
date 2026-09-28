//! ROM handling for Star Wars Episode I: Racer (N64, USA).
//!
//! Detects and normalises byte order, parses the cartridge header, recomputes
//! the IPL3 boot checksum and checks everything against the one supported ROM.
//! See SPEC §2.

mod byteorder;
mod checksum;
mod header;

pub use byteorder::{to_z64, ByteOrder};
pub use checksum::{compute_crc, Cic};
pub use header::Header;

use sha1::{Digest, Sha1};
use std::fmt;

/// Facts about the one supported ROM (SPEC §2).
pub mod expected {
    pub const NAME: &str = "STAR WARS EP1 RACER";
    pub const GAME_CODE: &str = "NEPE";
    pub const CRC1: u32 = 0x72F7_0398;
    pub const CRC2: u32 = 0x6556_A98B;
    pub const SIZE: usize = 32 * 1024 * 1024;
    pub const ENTRY_POINT: u32 = 0x8000_0400;
}

#[derive(Debug)]
pub enum RomError {
    TooSmall(usize),
    UnknownByteOrder([u8; 4]),
    WrongSize(usize),
    WrongName(String),
    WrongGameCode(String),
    WrongEntryPoint(u32),
    HeaderCrcMismatch { crc1: u32, crc2: u32 },
    ChecksumMismatch,
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use expected::*;
        match self {
            Self::TooSmall(n) => write!(f, "file is only {n} bytes; not an N64 ROM"),
            Self::UnknownByteOrder(b) => write!(
                f,
                "unrecognised first word {:02X} {:02X} {:02X} {:02X}; expected 80 37 12 40 (z64), 37 80 40 12 (v64) or 40 12 37 80 (n64)",
                b[0], b[1], b[2], b[3]
            ),
            Self::WrongSize(n) => write!(f, "ROM is {n} bytes; expected {SIZE}"),
            Self::WrongName(n) => write!(f, "header name is {n:?}; expected {NAME:?}"),
            Self::WrongGameCode(c) => write!(f, "game code is {c:?}; expected {GAME_CODE:?} (USA). Only the USA ROM is supported"),
            Self::WrongEntryPoint(e) => write!(f, "entry point is {e:#010X}; expected {ENTRY_POINT:#010X}"),
            Self::HeaderCrcMismatch { crc1, crc2 } => write!(
                f,
                "header CRCs are {crc1:08X}/{crc2:08X}; expected {CRC1:08X}/{CRC2:08X}. Wrong revision or modified ROM"
            ),
            Self::ChecksumMismatch => {
                write!(f, "boot checksum does not match the header under any known CIC; ROM data is corrupt or modified")
            }
        }
    }
}

impl std::error::Error for RomError {}

/// A ROM that has passed every check, stored in z64 (big-endian) order.
pub struct VerifiedRom {
    pub data: Vec<u8>,
    pub original_order: ByteOrder,
    pub header: Header,
    pub cic: Cic,
    pub sha1: String,
}

/// Normalise `data` to z64 order and verify it is the supported ROM.
pub fn verify(mut data: Vec<u8>) -> Result<VerifiedRom, RomError> {
    if data.len() < 0x101000 {
        return Err(RomError::TooSmall(data.len()));
    }
    let first = [data[0], data[1], data[2], data[3]];
    let order = ByteOrder::detect(first).ok_or(RomError::UnknownByteOrder(first))?;
    to_z64(&mut data, order);

    let header = Header::parse(&data);
    if data.len() != expected::SIZE {
        return Err(RomError::WrongSize(data.len()));
    }
    if header.name != expected::NAME {
        return Err(RomError::WrongName(header.name));
    }
    if header.game_code != expected::GAME_CODE {
        return Err(RomError::WrongGameCode(header.game_code));
    }
    if header.entry_point != expected::ENTRY_POINT {
        return Err(RomError::WrongEntryPoint(header.entry_point));
    }
    if (header.crc1, header.crc2) != (expected::CRC1, expected::CRC2) {
        return Err(RomError::HeaderCrcMismatch { crc1: header.crc1, crc2: header.crc2 });
    }

    // The header CRCs could have been copied onto a bad dump; recompute them
    // from the data. Try every CIC and accept whichever reproduces the header.
    let cic = Cic::ALL
        .into_iter()
        .find(|&cic| compute_crc(&data, cic) == (header.crc1, header.crc2))
        .ok_or(RomError::ChecksumMismatch)?;

    let sha1 = hex(&Sha1::digest(&data));
    Ok(VerifiedRom { data, original_order: order, header, cic, sha1 })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
