//! game::recomp::fpu against recomp.h's own conversion macros run on this
//! host under each FCR31 rounding mode (oracle::fpu_probe). These pin what
//! ports compute for float-to-int and narrowing conversions, including the
//! host-specific out-of-range and NaN results (NOTES.md, "Floats").

use game::recomp::fpu;
use oracle::fpu_probe::*;
use proptest::prelude::*;

/// f32 bit patterns: zeros, subnormals, halfway cases, the 2^31 and 2^32
/// boundaries, infinities, NaN payloads (quiet and signalling), anything.
fn f32_bits() -> impl Strategy<Value = u32> {
    prop_oneof![
        Just(0u32),
        Just(0x8000_0000),
        Just(0x0000_0001),
        Just(0x007F_FFFF),
        Just(0x8000_0001),
        0u32..0x0080_0000,
        (-40i32..40).prop_map(|n| (n as f32 + 0.5).to_bits()),
        (-40i32..40).prop_map(|n| (n as f32).to_bits()),
        Just(2_147_483_520.0f32.to_bits()),
        Just(2_147_483_648.0f32.to_bits()),
        Just(2_147_483_904.0f32.to_bits()),
        Just((-2_147_483_648.0f32).to_bits()),
        Just((-2_147_483_904.0f32).to_bits()),
        Just(4_294_967_296.0f32.to_bits()),
        Just(16_777_216.5f32.to_bits()),
        Just(f32::INFINITY.to_bits()),
        Just(f32::NEG_INFINITY.to_bits()),
        (0u32..0x0080_0000).prop_map(|p| 0x7F80_0000 | p.max(1)),
        (0u32..0x0080_0000).prop_map(|p| 0xFF80_0000 | p.max(1)),
        any::<u32>(),
    ]
}

/// f64 values: halfway cases, the i32 boundaries (with halves), infinities,
/// NaN, and anything.
fn f64_bits() -> impl Strategy<Value = u64> {
    prop_oneof![
        (-40i64..40).prop_map(|n| (n as f64 + 0.5).to_bits()),
        Just(2_147_483_647.5f64.to_bits()),
        Just(2_147_483_647.0f64.to_bits()),
        Just(2_147_483_648.0f64.to_bits()),
        Just((-2_147_483_648.5f64).to_bits()),
        Just((-2_147_483_649.0f64).to_bits()),
        Just((-2_147_483_648.0f64).to_bits()),
        Just(f64::INFINITY.to_bits()),
        Just(f64::NAN.to_bits()),
        Just(0x7FF0_0000_0000_0001),
        Just(0x0000_0000_0000_0001),
        Just(f64::from(f32::MAX).to_bits()),
        Just((f64::from(f32::MAX) * (1.0 + f64::EPSILON)).to_bits()),
        Just(1e-50f64.to_bits()),
        Just((-1e-50f64).to_bits()),
        any::<u64>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn cvt_w_s(bits in f32_bits(), mode in 0u32..4) {
        let x = f32::from_bits(bits);
        prop_assert_eq!(fpu::cvt_w_s(x, mode), unsafe { fpu_probe_cvt_w_s(x, mode) } as u32, "{:e} mode {}", x, mode);
    }

    #[test]
    fn trunc_w_s(bits in f32_bits()) {
        let x = f32::from_bits(bits);
        prop_assert_eq!(fpu::trunc_w_s(x), unsafe { fpu_probe_trunc_w_s(x) } as u32, "{:e}", x);
    }

    #[test]
    fn cvt_w_d(bits in f64_bits(), mode in 0u32..4) {
        let x = f64::from_bits(bits);
        prop_assert_eq!(fpu::cvt_w_d(x, mode), unsafe { fpu_probe_cvt_w_d(x, mode) } as u32, "{:e} mode {}", x, mode);
    }

    #[test]
    fn trunc_w_d(bits in f64_bits()) {
        let x = f64::from_bits(bits);
        prop_assert_eq!(fpu::trunc_w_d(x), unsafe { fpu_probe_trunc_w_d(x) } as u32, "{:e}", x);
    }

    #[test]
    fn cvt_s_w(w in prop_oneof![any::<u32>(), (0u32..64).prop_map(|k| (1u32 << 24) + k), Just(0x7FFF_FFFF), Just(0x8000_0001)], mode in 0u32..4) {
        let want = unsafe { fpu_probe_cvt_s_w(w as i32, mode) };
        prop_assert_eq!(fpu::cvt_s_w(w, mode).to_bits(), want.to_bits(), "{} mode {}", w as i32, mode);
    }

    /// Narrowing, NaN excluded: NAN_CHECK guards CVT_S_D (NOTES.md, "Floats").
    #[test]
    fn cvt_s_d(bits in f64_bits().prop_filter("NaN aborts the oracle", |b| !f64::from_bits(*b).is_nan()), mode in 0u32..4) {
        let x = f64::from_bits(bits);
        let want = unsafe { fpu_probe_cvt_s_d(x, mode) };
        prop_assert_eq!(fpu::cvt_s_d(x, mode).to_bits(), want.to_bits(), "{:e} mode {}", x, mode);
    }
}

/// The oracle's environment: asserts (NAN_CHECK) on, round-to-nearest at entry.
#[test]
fn oracle_environment() {
    unsafe {
        assert_eq!(fpu_probe_asserts_on(), 1);
        assert_eq!(fpu_probe_get_cop1_cs(), fpu::NEAREST);
    }
}
