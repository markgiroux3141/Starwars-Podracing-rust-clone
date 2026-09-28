//! The IPL3 boot checksum stored at header 0x10/0x14.
//!
//! The boot code sums the first 1 MiB of game data after the header and boot
//! code (ROM 0x1000..0x101000). The seed and final mix depend on which CIC
//! lockout chip the cartridge uses.

/// CIC lockout chip variants (NTSC names; PAL 71xx equivalents share seeds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cic {
    C6101,
    C6102,
    C6103,
    C6105,
    C6106,
}

impl Cic {
    /// Order matters only for display; 6101 and 6102 share a seed and algorithm.
    pub const ALL: [Cic; 5] = [Cic::C6102, Cic::C6101, Cic::C6103, Cic::C6105, Cic::C6106];

    fn seed(self) -> u32 {
        match self {
            Cic::C6101 | Cic::C6102 => 0xF8CA_4DDC,
            Cic::C6103 => 0xA388_6759,
            Cic::C6105 => 0xDF26_F436,
            Cic::C6106 => 0x1FEA_617A,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Cic::C6101 => "CIC-6101",
            Cic::C6102 => "CIC-6102",
            Cic::C6103 => "CIC-6103",
            Cic::C6105 => "CIC-6105",
            Cic::C6106 => "CIC-6106",
        }
    }
}

const START: usize = 0x1000;
const LEN: usize = 0x10_0000;

/// Compute (crc1, crc2) over a z64 ROM as the IPL3 for `cic` would.
pub fn compute_crc(rom: &[u8], cic: Cic) -> (u32, u32) {
    let be32 = |o: usize| u32::from_be_bytes(rom[o..o + 4].try_into().unwrap());
    let seed = cic.seed();
    let (mut t1, mut t2, mut t3, mut t4, mut t5, mut t6) = (seed, seed, seed, seed, seed, seed);

    for i in (START..START + LEN).step_by(4) {
        let d = be32(i);
        let (sum, carry) = t6.overflowing_add(d);
        if carry {
            t4 = t4.wrapping_add(1);
        }
        t6 = sum;
        t3 ^= d;
        let r = d.rotate_left(d & 0x1F);
        t5 = t5.wrapping_add(r);
        t2 ^= if t2 > d { r } else { t6 ^ d };
        t1 = t1.wrapping_add(match cic {
            // 6105 mixes in words from its own boot code region.
            Cic::C6105 => be32(0x0750 + (i & 0xFF)) ^ d,
            _ => t5 ^ d,
        });
    }

    match cic {
        Cic::C6103 => ((t6 ^ t4).wrapping_add(t3), (t5 ^ t2).wrapping_add(t1)),
        Cic::C6106 => (
            t6.wrapping_mul(t4).wrapping_add(t3),
            t5.wrapping_mul(t2).wrapping_add(t1),
        ),
        _ => (t6 ^ t4 ^ t3, t5 ^ t2 ^ t1),
    }
}
