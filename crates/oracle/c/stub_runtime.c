/* Minimal runtime for running single recompiled functions in tests.
 *
 * This is not N64ModernRuntime (GPL-3.0), which is deliberately not linked
 * into tests. It provides only the symbols N64Recomp's generated code and
 * recomp.h reference. A pure function never reaches any of them, so most
 * trap: they report what was hit and abort the test process via
 * oracle_trap() on the Rust side. The exception is CP0 Status, which lives in
 * the context's status_reg (NOTES.md, "CP0 Status"). */

#include <stdarg.h>
#include <stdio.h>
#include "recomp.h"

/* Rust side (crates/oracle/src/lib.rs): prints the message and aborts. */
extern void oracle_trap(const char* msg);

static void trap(const char* fmt, ...) {
    char buf[512];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof buf, fmt, ap);
    va_end(ap);
    oracle_trap(buf);
}

/* Every recompiled function by start address (build.rs: the compiled-in
 * C, or the stub that runs a test double or traps), sorted. */
struct oracle_function { uint32_t vram; recomp_func_t* func; };
extern const struct oracle_function oracle_functions[];
extern const size_t oracle_function_count;

/* LOOKUP_FUNC: resolves an indirect call the way a direct call to the same
 * function resolves. Only function starts are valid targets. */
recomp_func_t* get_function(int32_t vram) {
    uint32_t v = (uint32_t)vram;
    size_t lo = 0, hi = oracle_function_count;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2;
        if (oracle_functions[mid].vram < v) lo = mid + 1;
        else hi = mid;
    }
    if (lo < oracle_function_count && oracle_functions[lo].vram == v) {
        return oracle_functions[lo].func;
    }
    trap("LOOKUP_FUNC(0x%08X): indirect call to an address where no function starts", v);
    return NULL;
}

void switch_error(const char* func, uint32_t vram, uint32_t jtbl) {
    trap("%s: jump table at 0x%08X (jr at 0x%08X) got an out-of-range index", func, jtbl, vram);
}

void do_break(uint32_t vram) {
    trap("break instruction at 0x%08X (e.g. division by zero trap)", vram);
}

void recomp_syscall_handler(uint8_t* rdram, recomp_context* ctx, int32_t instruction_vram) {
    (void)rdram; (void)ctx;
    trap("syscall at 0x%08X", (uint32_t)instruction_vram);
}

/* CP0 Status is the context's status_reg, a 32-bit register: mfc0 reads it
 * sign-extended, mtc0 writes the low word. Changing FR (bit 26) would switch
 * the FPU register mode (mips3_float_mode, f_odd), which the oracle doesn't
 * model, so that traps. */
#define STATUS_FR (1u << 26)

gpr cop0_status_read(recomp_context* ctx) {
    return (gpr)(int64_t)(int32_t)ctx->status_reg;
}

void cop0_status_write(recomp_context* ctx, gpr value) {
    uint32_t v = (uint32_t)value;
    if ((v ^ ctx->status_reg) & STATUS_FR) {
        trap("mtc0 Status <- 0x%08X changes FR (from 0x%08X): FPU mode switches are not modelled", v, ctx->status_reg);
    }
    ctx->status_reg = v;
}

void pause_self(uint8_t* rdram) {
    (void)rdram;
    trap("pause_self: branch-to-self idle loop");
}

/* Only used by RELOC_HI16/RELOC_LO16, which N64Recomp emits for relocatable
 * sections. Ours has none. NULL so that any use faults immediately. */
int32_t* section_addresses = NULL;

/* Called (via oracle_callee on the Rust side) when a generated stub for a
 * recompiled function that is not compiled into the oracle is reached and the
 * test installed no double for it. */
void oracle_unexpected_call(const char* name) {
    trap("call to %s, which is not compiled into the oracle (add it to crates/oracle/functions.txt, "
         "or install a test double with oracle::doubles::install)", name);
}
