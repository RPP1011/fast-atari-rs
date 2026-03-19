#pragma once
#include "state_layout.cuh"

// PIA timer: tick by one CPU cycle.
__device__ __forceinline__
void pia_tick(AtariState* s) {
    if (s->timer_value == 0 && s->timer_underflow) {
        s->timer_value = 0xFF;
        return;
    }

    s->timer_prescaler -= 1;
    if (s->timer_prescaler == 0) {
        s->timer_prescaler = s->timer_underflow ? 1 : s->timer_prescaler_select;

        if (s->timer_value == 0) {
            s->timer_underflow = 1;
            s->timer_value = 0xFF;
        } else {
            s->timer_value -= 1;
        }
    }
}

// PIA timer: advance by n CPU cycles in bulk.
__device__ __forceinline__
void pia_tick_n(AtariState* s, uint16_t n) {
    if (n == 0) return;

    // Post-underflow mode
    if (s->timer_value == 0 && s->timer_underflow) {
        s->timer_value = (uint16_t)((0xFF - (n - 1)) & 0xFF);
        return;
    }

    uint16_t remaining = n;

    // Phase 1: Consume remaining prescaler ticks
    if (s->timer_prescaler > 0) {
        if (remaining < s->timer_prescaler) {
            s->timer_prescaler -= remaining;
            return;
        }
        remaining -= s->timer_prescaler;
        s->timer_prescaler = 0;

        if (s->timer_value == 0) {
            s->timer_underflow = 1;
            s->timer_value = 0xFF;
            if (remaining > 0) {
                s->timer_value = (uint16_t)((0xFF - (remaining - 1)) & 0xFF);
            }
            return;
        }
        s->timer_value -= 1;
    }

    if (remaining == 0) {
        s->timer_prescaler = s->timer_prescaler_select;
        return;
    }

    // Phase 2: Full prescaler periods
    uint16_t ps = s->timer_prescaler_select;
    uint16_t full_decrements = remaining / ps;
    uint16_t leftover = remaining % ps;

    if (full_decrements >= s->timer_value) {
        uint16_t cycles_to_zero = s->timer_value * ps;
        remaining -= cycles_to_zero;
        s->timer_value = 0;
        s->timer_underflow = 1;
        s->timer_value = 0xFF;
        if (remaining > 0) {
            s->timer_value = (uint16_t)((0xFF - (remaining - 1)) & 0xFF);
        }
    } else {
        s->timer_value -= full_decrements;
        s->timer_prescaler = ps - leftover;
    }
}

// PIA read register or RAM.
__device__ __forceinline__
uint8_t pia_read(AtariState* s, uint16_t addr) {
    if ((addr & 0x200) == 0) {
        // RAM
        return s->ram[addr & 0x7F];
    }
    // I/O registers
    switch (addr & 0x07) {
        case 0x00: // SWCHA
            return (s->port_a_input & ~s->port_a_ddr)
                 | (s->port_a_output & s->port_a_ddr);
        case 0x01: return s->port_a_ddr;
        case 0x02: // SWCHB
            return (s->port_b_input & ~s->port_b_ddr)
                 | (s->port_b_output & s->port_b_ddr);
        case 0x03: return s->port_b_ddr;
        case 0x04: // INTIM
            s->timer_underflow = 0;
            return (uint8_t)s->timer_value;
        case 0x05: // INSTAT
            return s->timer_underflow ? 0x80 : 0x00;
        default: return 0;
    }
}

// PIA write register or RAM.
__device__ __forceinline__
void pia_write(AtariState* s, uint16_t addr, uint8_t val) {
    if ((addr & 0x200) == 0) {
        s->ram[addr & 0x7F] = val;
    } else if ((addr & 0x10) != 0) {
        // Timer set: $294-$297
        s->timer_value = val;
        s->timer_underflow = 0;
        switch (addr & 0x03) {
            case 0x00: s->timer_prescaler_select = 1;    break;
            case 0x01: s->timer_prescaler_select = 8;    break;
            case 0x02: s->timer_prescaler_select = 64;   break;
            case 0x03: s->timer_prescaler_select = 1024; break;
        }
        s->timer_prescaler = s->timer_prescaler_select;
    } else {
        // I/O registers
        switch (addr & 0x03) {
            case 0x00: s->port_a_output = val; break;
            case 0x01: s->port_a_ddr    = val; break;
            case 0x02: s->port_b_output = val; break;
            case 0x03: s->port_b_ddr    = val; break;
        }
    }
}
