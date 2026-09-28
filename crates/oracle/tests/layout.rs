//! SPEC §5.3 day-one test: values poked through N64Recomp's C macros read back
//! identically through n64mem, and vice versa.

use n64mem::Rdram;
use oracle::layout::*;

/// Addresses spanning every byte lane, the start and end of RDRAM, and the
/// Expansion Pak half.
const ADDRS: [u32; 6] = [0x8000_0000, 0x8000_0400, 0x8012_3450, 0x803F_FFF8, 0x8040_0000, 0x807F_FFF8];

#[test]
fn c_writes_rust_reads() {
    let mut r = Rdram::new();
    let p = r.as_mut_ptr();
    for &a in &ADDRS {
        unsafe {
            layout_write_w(p, a, 0x8899_AABBu32 as i32);
            layout_write_h(p, a + 4, 0xCCDDu16 as i16);
            layout_write_b(p, a + 6, 0xEEu8 as i8);
            layout_write_b(p, a + 7, 0x7F);
        }
        let m = r.mem();
        assert_eq!(m.read_u32(a), 0x8899_AABB);
        assert_eq!(m.read_u16(a + 4), 0xCCDD);
        assert_eq!(m.read_i16(a + 4), 0xCCDDu16 as i16);
        assert_eq!(m.read_u8(a + 6), 0xEE);
        assert_eq!(m.read_i8(a + 6), -0x12);
        assert_eq!(m.read_u8(a + 7), 0x7F);
        // Byte lanes in big-endian order
        assert_eq!([m.read_u8(a), m.read_u8(a + 1), m.read_u8(a + 2), m.read_u8(a + 3)], [0x88, 0x99, 0xAA, 0xBB]);
        assert_eq!(m.read_u32(a + 4), 0xCCDD_EE7F);
    }
}

#[test]
fn rust_writes_c_reads() {
    let mut r = Rdram::new();
    for &a in &ADDRS {
        {
            let mut m = r.mem();
            m.write_bytes(a, &[0x01, 0x82, 0x03, 0x84, 0xF5, 0x06, 0x07, 0x08]);
        }
        let p = r.as_mut_ptr();
        unsafe {
            assert_eq!(layout_read_w(p, a) as u32, 0x0182_0384);
            assert_eq!(layout_read_hu(p, a), 0x0182);
            assert_eq!(layout_read_hu(p, a + 2), 0x0384);
            assert_eq!(layout_read_h(p, a + 2), 0x0384);
            assert_eq!(layout_read_bu(p, a + 1), 0x82);
            assert_eq!(layout_read_b(p, a + 1), 0x82u8 as i8);
            assert_eq!(layout_read_b(p, a + 4), 0xF5u8 as i8);
            assert_eq!(layout_read_d(p, a), 0x0182_0384_F506_0708);
        }
    }
}

#[test]
fn doublewords_agree_both_ways() {
    let mut r = Rdram::new();
    let v = 0xFEDC_BA98_7654_3210u64;
    unsafe { layout_write_d(r.as_mut_ptr(), 0x8020_0000, v) };
    assert_eq!(r.mem().read_u64(0x8020_0000), v);
    r.mem().write_u64(0x8020_0008, !v);
    assert_eq!(unsafe { layout_read_d(r.as_mut_ptr(), 0x8020_0008) }, !v);
}

#[test]
fn every_byte_lane_and_halfword_matches() {
    // Exhaustive over one word's lanes with distinct values.
    let mut r = Rdram::new();
    let base = 0x8010_0000;
    for lane in 0..8u32 {
        unsafe { layout_write_b(r.as_mut_ptr(), base + lane, (0x10 + lane) as i8) };
    }
    for lane in 0..8u32 {
        assert_eq!(r.mem().read_u8(base + lane), (0x10 + lane) as u8, "byte lane {lane}");
    }
    for hw in (0..8u32).step_by(2) {
        let expect = u16::from_be_bytes([(0x10 + hw) as u8, (0x11 + hw) as u8]);
        assert_eq!(r.mem().read_u16(base + hw), expect, "halfword at +{hw}");
        assert_eq!(unsafe { layout_read_hu(r.as_mut_ptr(), base + hw) }, expect);
    }
}
