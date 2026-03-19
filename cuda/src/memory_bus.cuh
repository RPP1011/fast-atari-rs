#pragma once
#include "state_layout.cuh"
#include "tia_headless.cuh"
#include "pia.cuh"

// ROM passed as a global memory pointer (shared across all instances).
// rom_ptr and rom_len are set per-kernel-launch.

// Read from ROM bank.
__device__ __forceinline__
uint8_t rom_read(const AtariState* s, uint16_t addr, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint16_t a = addr & 0x0FFF;
    if (s->scheme == BANK_FIXED) {
        return rom_ptr[a % rom_len];
    }
    uint32_t offset = (uint32_t)s->bank * 4096 + a;
    if (offset < rom_len) return rom_ptr[offset];
    return 0;
}

// Check and apply bankswitching hotspots.
__device__ __forceinline__
void check_bankswitch(AtariState* s, uint16_t addr) {
    uint16_t a = addr & 0x1FFF;
    switch ((BankScheme)s->scheme) {
        case BANK_FIXED: return;
        case BANK_F8:
            if (a == 0x1FF8)      s->bank = 0;
            else if (a == 0x1FF9) s->bank = 1;
            return;
        case BANK_F6:
            if      (a == 0x1FF6) s->bank = 0;
            else if (a == 0x1FF7) s->bank = 1;
            else if (a == 0x1FF8) s->bank = 2;
            else if (a == 0x1FF9) s->bank = 3;
            return;
        case BANK_F4:
            if      (a == 0x1FF4) s->bank = 0;
            else if (a == 0x1FF5) s->bank = 1;
            else if (a == 0x1FF6) s->bank = 2;
            else if (a == 0x1FF7) s->bank = 3;
            else if (a == 0x1FF8) s->bank = 4;
            else if (a == 0x1FF9) s->bank = 5;
            else if (a == 0x1FFA) s->bank = 6;
            else if (a == 0x1FFB) s->bank = 7;
            return;
    }
}

// Bus read — mirrors HeadlessBus::read from Rust.
__device__ __forceinline__
uint8_t bus_read(AtariState* s, uint16_t addr, const uint8_t* rom_ptr, uint32_t rom_len) {
    check_bankswitch(s, addr);
    uint16_t a = addr & 0x1FFF;

    // TIA read registers: A12=0, A7=0
    if ((a & 0x1080) == 0x0000) {
        return tia_read(s, a);
    }
    // PIA RAM: A12=0, A9=0, A7=1
    if ((a & 0x1280) == 0x0080) {
        return pia_read(s, a);
    }
    // PIA I/O: A12=0, A9=1
    if ((a & 0x1280) == 0x0280) {
        return pia_read(s, a);
    }
    // Cartridge ROM: A12=1
    if ((a & 0x1000) == 0x1000) {
        return rom_read(s, a, rom_ptr, rom_len);
    }
    return 0;
}

// Bus write — mirrors HeadlessBus::write from Rust.
__device__ __forceinline__
void bus_write(AtariState* s, uint16_t addr, uint8_t val) {
    check_bankswitch(s, addr);
    uint16_t a = addr & 0x1FFF;

    // ROM space — only bankswitch hotspots matter (already handled)
    if ((a & 0x1000) != 0) return;

    // PIA RAM: $80-$FF or $180-$1FF
    if ((a & 0x80) != 0 && (a & 0x200) == 0) {
        s->ram[a & 0x7F] = val;
        return;
    }

    // PIA I/O: $280-$29F
    if ((a & 0x200) != 0) {
        pia_write(s, a, val);
        return;
    }

    // TIA write registers
    tia_write(s, a, val);
}
