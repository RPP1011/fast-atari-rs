#pragma once
#include <cstdint>

// Pre-decoded instruction: 8 bytes (power-of-2 for aligned access).
// Built on host per ROM bank, uploaded once to GPU.
struct DecodedOp {
    uint8_t  opcode;   // raw opcode byte
    uint8_t  size;     // instruction size (1-3)
    uint8_t  cycles;   // base cycle count
    uint8_t  _pad0;
    uint16_t operand;  // pre-read: lo | (hi << 8) for 2/3-byte ops
    uint16_t _pad1;
};

// Fetch a DecodedOp from the pre-decoded table via read-only cache.
// table layout: table[bank * 4096 + (pc & 0x0FFF)]
__device__ __forceinline__
DecodedOp decode_fetch(const DecodedOp* __restrict__ table, uint8_t bank, uint16_t pc) {
    uint32_t idx = (uint32_t)bank * 4096 + (pc & 0x0FFF);
    // Load as two uint32_t via __ldg for coalesced read-only cache access
    const uint32_t* p = (const uint32_t*)&table[idx];
    DecodedOp d;
    uint32_t w0 = __ldg(&p[0]);
    uint32_t w1 = __ldg(&p[1]);
    // Unpack
    d.opcode = (uint8_t)(w0);
    d.size   = (uint8_t)(w0 >> 8);
    d.cycles = (uint8_t)(w0 >> 16);
    d._pad0  = 0;
    d.operand = (uint16_t)(w1);
    d._pad1  = 0;
    return d;
}
