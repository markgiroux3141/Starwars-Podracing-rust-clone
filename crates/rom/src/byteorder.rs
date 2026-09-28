/// On-disk byte order of an N64 ROM dump, identified by its first word.
///
/// File extensions are unreliable (the dump this project was started with is
/// named `.n64` but is actually v64), so always detect from the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    /// Native big-endian: `80 37 12 40`.
    Z64,
    /// Byte-swapped within each 16-bit halfword: `37 80 40 12`.
    V64,
    /// Little-endian 32-bit words: `40 12 37 80`.
    N64,
}

impl ByteOrder {
    pub fn detect(first: [u8; 4]) -> Option<Self> {
        match first {
            [0x80, 0x37, 0x12, 0x40] => Some(Self::Z64),
            [0x37, 0x80, 0x40, 0x12] => Some(Self::V64),
            [0x40, 0x12, 0x37, 0x80] => Some(Self::N64),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Z64 => "z64 (big-endian)",
            Self::V64 => "v64 (byte-swapped)",
            Self::N64 => "n64 (little-endian)",
        }
    }
}

/// Convert `data` in place from `order` to z64. Trailing bytes that don't
/// fill a whole word are left alone (real ROMs are always word-aligned).
pub fn to_z64(data: &mut [u8], order: ByteOrder) {
    match order {
        ByteOrder::Z64 => {}
        ByteOrder::V64 => data.chunks_exact_mut(2).for_each(|c| c.swap(0, 1)),
        ByteOrder::N64 => data.chunks_exact_mut(4).for_each(|c| c.reverse()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const Z64: [u8; 8] = [0x80, 0x37, 0x12, 0x40, 0x01, 0x02, 0x03, 0x04];

    #[test]
    fn detects_all_orders() {
        assert_eq!(ByteOrder::detect([0x80, 0x37, 0x12, 0x40]), Some(ByteOrder::Z64));
        assert_eq!(ByteOrder::detect([0x37, 0x80, 0x40, 0x12]), Some(ByteOrder::V64));
        assert_eq!(ByteOrder::detect([0x40, 0x12, 0x37, 0x80]), Some(ByteOrder::N64));
        assert_eq!(ByteOrder::detect([0, 0, 0, 0]), None);
    }

    #[test]
    fn converts_v64() {
        let mut d = [0x37, 0x80, 0x40, 0x12, 0x02, 0x01, 0x04, 0x03];
        to_z64(&mut d, ByteOrder::V64);
        assert_eq!(d, Z64);
    }

    #[test]
    fn converts_n64() {
        let mut d = [0x40, 0x12, 0x37, 0x80, 0x04, 0x03, 0x02, 0x01];
        to_z64(&mut d, ByteOrder::N64);
        assert_eq!(d, Z64);
    }
}
