/* Exposes recomp_context's layout, as the C compiler sees it, so the Rust
 * mirror (game::recomp::RecompContext) can be checked against it. */

#include <stddef.h>
#include "recomp.h"

enum {
    CTX_SIZE, CTX_ALIGN, OFF_R0, OFF_R1, OFF_R31, OFF_F0, OFF_F1, OFF_F31,
    OFF_HI, OFF_LO, OFF_F_ODD, OFF_STATUS_REG, OFF_MIPS3_FLOAT_MODE,
    FPR_SIZE, FPR_ALIGN, OFF_FPR_FL, OFF_FPR_FH, OFF_FPR_U32L, OFF_FPR_U32H,
    CTX_LAYOUT_COUNT
};

size_t ctx_layout_count(void) { return CTX_LAYOUT_COUNT; }

void ctx_layout(size_t* out) {
    out[CTX_SIZE] = sizeof(recomp_context);
    out[CTX_ALIGN] = _Alignof(recomp_context);
    out[OFF_R0] = offsetof(recomp_context, r0);
    out[OFF_R1] = offsetof(recomp_context, r1);
    out[OFF_R31] = offsetof(recomp_context, r31);
    out[OFF_F0] = offsetof(recomp_context, f0);
    out[OFF_F1] = offsetof(recomp_context, f1);
    out[OFF_F31] = offsetof(recomp_context, f31);
    out[OFF_HI] = offsetof(recomp_context, hi);
    out[OFF_LO] = offsetof(recomp_context, lo);
    out[OFF_F_ODD] = offsetof(recomp_context, f_odd);
    out[OFF_STATUS_REG] = offsetof(recomp_context, status_reg);
    out[OFF_MIPS3_FLOAT_MODE] = offsetof(recomp_context, mips3_float_mode);
    out[FPR_SIZE] = sizeof(fpr);
    out[FPR_ALIGN] = _Alignof(fpr);
    out[OFF_FPR_FL] = offsetof(fpr, fl);
    out[OFF_FPR_FH] = offsetof(fpr, fh);
    out[OFF_FPR_U32L] = offsetof(fpr, u32l);
    out[OFF_FPR_U32H] = offsetof(fpr, u32h);
}

/* Writes through the C field names, for checking the Rust accessors. */
void ctx_poke(recomp_context* ctx) {
    ctx->r1 = 0x1111111111111111ull;
    ctx->r31 = 0x3131313131313131ull;
    ctx->f0.fl = 1.5f;
    ctx->f0.fh = -2.0f;
    ctx->f1.u32l = 0xAABBCCDDu;
    ctx->f1.u32h = 0x11223344u;
    ctx->f31.d = 0.25;
    ctx->hi = 0x4849u;
    ctx->lo = 0x4C4Fu;
    ctx->status_reg = 0x3400FF01u;
    ctx->mips3_float_mode = 1;
}

/* Writes an odd FPU register the way generated code does (FR=0 mode). */
void ctx_write_f_odd(recomp_context* ctx, int n, uint32_t v) {
    ctx->f_odd[(n - 1) * 2] = v;
}
