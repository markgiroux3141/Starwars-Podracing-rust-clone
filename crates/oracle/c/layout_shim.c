/* Exposes N64Recomp's memory macros to Rust so n64mem's layout can be tested
 * against the real thing (SPEC §5.3). Addresses arrive as 32-bit vaddrs and
 * are sign-extended into a gpr exactly as a MIPS register would hold them. */

#include "recomp.h"

#define REG(vaddr) ((gpr)(int64_t)(int32_t)(vaddr))

void layout_write_w(uint8_t* rdram, uint32_t vaddr, int32_t v)  { MEM_W(0, REG(vaddr)) = v; }
void layout_write_h(uint8_t* rdram, uint32_t vaddr, int16_t v)  { MEM_H(0, REG(vaddr)) = v; }
void layout_write_b(uint8_t* rdram, uint32_t vaddr, int8_t v)   { MEM_B(0, REG(vaddr)) = v; }
void layout_write_d(uint8_t* rdram, uint32_t vaddr, uint64_t v) { SD(v, 0, REG(vaddr)); }

int32_t  layout_read_w(uint8_t* rdram, uint32_t vaddr)  { return MEM_W(0, REG(vaddr)); }
int16_t  layout_read_h(uint8_t* rdram, uint32_t vaddr)  { return MEM_H(0, REG(vaddr)); }
uint16_t layout_read_hu(uint8_t* rdram, uint32_t vaddr) { return MEM_HU(0, REG(vaddr)); }
int8_t   layout_read_b(uint8_t* rdram, uint32_t vaddr)  { return MEM_B(0, REG(vaddr)); }
uint8_t  layout_read_bu(uint8_t* rdram, uint32_t vaddr) { return MEM_BU(0, REG(vaddr)); }
uint64_t layout_read_d(uint8_t* rdram, uint32_t vaddr)  { return LD(0, REG(vaddr)); }
