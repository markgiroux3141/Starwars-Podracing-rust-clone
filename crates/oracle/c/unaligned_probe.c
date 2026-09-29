/* Probes of recomp.h's unaligned word accesses (do_lwl, do_lwr, do_swl,
 * do_swr) on a caller's RDRAM buffer, so game::recomp's helpers can be
 * checked against exactly what the generated code does
 * (crates/difftest/tests/unaligned.rs). Our code. */
#include "recomp.h"

uint64_t unaligned_probe_lwl(uint8_t* rdram, uint64_t initial, uint64_t base, uint64_t offset) {
    return do_lwl(rdram, initial, offset, base);
}

uint64_t unaligned_probe_lwr(uint8_t* rdram, uint64_t initial, uint64_t base, uint64_t offset) {
    return do_lwr(rdram, initial, offset, base);
}

void unaligned_probe_swl(uint8_t* rdram, uint64_t base, uint64_t offset, uint64_t value) {
    do_swl(rdram, offset, base, value);
}

void unaligned_probe_swr(uint8_t* rdram, uint64_t base, uint64_t offset, uint64_t value) {
    do_swr(rdram, offset, base, value);
}
