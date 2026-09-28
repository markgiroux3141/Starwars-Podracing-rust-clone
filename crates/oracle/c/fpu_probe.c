/* Probes of recomp.h's float conversions under each FCR31 rounding mode, so
 * game::recomp's helpers can be checked against exactly what the generated
 * code computes on this host (crates/difftest/tests/fpu.rs). Our code. */
#include "recomp.h"

int32_t fpu_probe_cvt_w_s(float x, uint32_t mode) {
    set_cop1_cs(mode);
    int32_t r = CVT_W_S(x);
    set_cop1_cs(0);
    return r;
}

int32_t fpu_probe_cvt_w_d(double x, uint32_t mode) {
    set_cop1_cs(mode);
    int32_t r = CVT_W_D(x);
    set_cop1_cs(0);
    return r;
}

int32_t fpu_probe_trunc_w_s(float x) { return TRUNC_W_S(x); }
int32_t fpu_probe_trunc_w_d(double x) { return TRUNC_W_D(x); }

float fpu_probe_cvt_s_w(int32_t x, uint32_t mode) {
    set_cop1_cs(mode);
    float r = CVT_S_W(x);
    set_cop1_cs(0);
    return r;
}

float fpu_probe_cvt_s_d(double x, uint32_t mode) {
    set_cop1_cs(mode);
    float r = CVT_S_D(x);
    set_cop1_cs(0);
    return r;
}

uint32_t fpu_probe_get_cop1_cs(void) { return get_cop1_cs(); }

/* 1 if NAN_CHECK (an assert) is compiled in. */
int fpu_probe_asserts_on(void) {
#ifdef NDEBUG
    return 0;
#else
    return 1;
#endif
}
