#pragma once
#include "state_layout.cuh"
#include "tia_headless.cuh"
#include "pia.cuh"

// Read from ROM bank. Uses __ldg() to route through read-only texture cache,
// avoiding coherency overhead and enabling L1/L2 caching of hot ROM regions.
__device__ __forceinline__
uint8_t rom_read(const ThreadCtx* c, uint16_t addr,
                 const uint8_t* __restrict__ rom_ptr, uint32_t rom_len) {
    uint16_t a = addr & 0x0FFF;
    if (c->scheme == BANK_FIXED) {
        return __ldg(&rom_ptr[a % rom_len]);
    }
    uint32_t offset = (uint32_t)c->bank * 4096 + a;
    if (offset < rom_len) return __ldg(&rom_ptr[offset]);
    return 0;
}

// Check and apply bankswitching hotspots.
__device__ __forceinline__
void check_bankswitch(ThreadCtx* c, uint16_t addr) {
    uint16_t a = addr & 0x1FFF;
    switch ((BankScheme)c->scheme) {
        case BANK_FIXED: return;
        case BANK_F8:
            if (a == 0x1FF8)      c->bank = 0;
            else if (a == 0x1FF9) c->bank = 1;
            return;
        case BANK_F6:
            if      (a == 0x1FF6) c->bank = 0;
            else if (a == 0x1FF7) c->bank = 1;
            else if (a == 0x1FF8) c->bank = 2;
            else if (a == 0x1FF9) c->bank = 3;
            return;
        case BANK_F4:
            if      (a == 0x1FF4) c->bank = 0;
            else if (a == 0x1FF5) c->bank = 1;
            else if (a == 0x1FF6) c->bank = 2;
            else if (a == 0x1FF7) c->bank = 3;
            else if (a == 0x1FF8) c->bank = 4;
            else if (a == 0x1FF9) c->bank = 5;
            else if (a == 0x1FFA) c->bank = 6;
            else if (a == 0x1FFB) c->bank = 7;
            return;
    }
}

// Bus read. my_ram points to this thread's 128-byte shared memory region.
__device__ __forceinline__
uint8_t bus_read(ThreadCtx* c, uint8_t* my_ram, uint16_t addr,
                 const uint8_t* rom_ptr, uint32_t rom_len) {
    check_bankswitch(c, addr);
    uint16_t a = addr & 0x1FFF;

    if ((a & 0x1080) == 0x0000) {
        return tia_read(c, a);
    }
    if ((a & 0x1280) == 0x0080) {
        return pia_read(c, my_ram, a);
    }
    if ((a & 0x1280) == 0x0280) {
        return pia_read(c, my_ram, a);
    }
    if ((a & 0x1000) == 0x1000) {
        return rom_read(c, a, rom_ptr, rom_len);
    }
    return 0;
}

// Bus write.
__device__ __forceinline__
void bus_write(ThreadCtx* c, uint8_t* my_ram, uint16_t addr, uint8_t val) {
    check_bankswitch(c, addr);
    uint16_t a = addr & 0x1FFF;

    if ((a & 0x1000) != 0) return;

    if ((a & 0x80) != 0 && (a & 0x200) == 0) {
        my_ram[a & 0x7F] = val;
        return;
    }

    if ((a & 0x200) != 0) {
        pia_write(c, my_ram, a, val);
        return;
    }

    tia_write(c, a, val);
}
