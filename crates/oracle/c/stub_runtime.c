/* Minimal runtime for running single recompiled functions in tests.
 *
 * This is not N64ModernRuntime (GPL-3.0), which is deliberately not linked
 * into tests. It provides only the symbols N64Recomp's generated code and
 * recomp.h reference. A pure function never reaches any of them, so each one
 * traps: it reports what was hit and aborts the test process via
 * oracle_trap() on the Rust side. */

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

recomp_func_t* get_function(int32_t vram) {
    trap("LOOKUP_FUNC(0x%08X): indirect call; the oracle has no function table", (uint32_t)vram);
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

gpr cop0_status_read(recomp_context* ctx) {
    (void)ctx;
    trap("mfc0 Status: interrupt/OS code is not available in the oracle");
    return 0;
}

void cop0_status_write(recomp_context* ctx, gpr value) {
    (void)ctx;
    trap("mtc0 Status <- 0x%016llX: interrupt/OS code is not available in the oracle", (unsigned long long)value);
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
