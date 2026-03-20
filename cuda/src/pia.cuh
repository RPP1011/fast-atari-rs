#pragma once
#include "state_layout.cuh"

// PIA timer: advance by n CPU cycles in bulk.
__device__ __forceinline__
void pia_tick_n(ThreadCtx* c, uint16_t n) {
    if (n == 0) return;

    if (c->timer_value == 0 && c->timer_underflow) {
        c->timer_value = (uint16_t)((0xFF - (n - 1)) & 0xFF);
        return;
    }

    uint16_t remaining = n;

    if (c->timer_prescaler > 0) {
        if (remaining < c->timer_prescaler) {
            c->timer_prescaler -= remaining;
            return;
        }
        remaining -= c->timer_prescaler;
        c->timer_prescaler = 0;

        if (c->timer_value == 0) {
            c->timer_underflow = 1;
            c->timer_value = 0xFF;
            if (remaining > 0) {
                c->timer_value = (uint16_t)((0xFF - (remaining - 1)) & 0xFF);
            }
            return;
        }
        c->timer_value -= 1;
    }

    if (remaining == 0) {
        c->timer_prescaler = c->timer_prescaler_select;
        return;
    }

    uint16_t ps = c->timer_prescaler_select;
    uint16_t full_decrements = remaining / ps;
    uint16_t leftover = remaining % ps;

    if (full_decrements >= c->timer_value) {
        uint16_t cycles_to_zero = c->timer_value * ps;
        remaining -= cycles_to_zero;
        c->timer_value = 0;
        c->timer_underflow = 1;
        c->timer_value = 0xFF;
        if (remaining > 0) {
            c->timer_value = (uint16_t)((0xFF - (remaining - 1)) & 0xFF);
        }
    } else {
        c->timer_value -= full_decrements;
        c->timer_prescaler = ps - leftover;
    }
}

// PIA read register or RAM. RAM reads go to shared memory.
__device__ __forceinline__
uint8_t pia_read(ThreadCtx* c, uint8_t* my_ram, uint16_t addr) {
    if ((addr & 0x200) == 0) {
        return my_ram[addr & 0x7F];
    }
    switch (addr & 0x07) {
        case 0x00:
            return (c->port_a_input & ~c->port_a_ddr)
                 | (c->port_a_output & c->port_a_ddr);
        case 0x01: return c->port_a_ddr;
        case 0x02:
            return (c->port_b_input & ~c->port_b_ddr)
                 | (c->port_b_output & c->port_b_ddr);
        case 0x03: return c->port_b_ddr;
        case 0x04:
            c->timer_underflow = 0;
            return (uint8_t)c->timer_value;
        case 0x05:
            return c->timer_underflow ? 0x80 : 0x00;
        default: return 0;
    }
}

// PIA write register or RAM.
__device__ __forceinline__
void pia_write(ThreadCtx* c, uint8_t* my_ram, uint16_t addr, uint8_t val) {
    if ((addr & 0x200) == 0) {
        my_ram[addr & 0x7F] = val;
    } else if ((addr & 0x10) != 0) {
        c->timer_value = val;
        c->timer_underflow = 0;
        switch (addr & 0x03) {
            case 0x00: c->timer_prescaler_select = 1;    break;
            case 0x01: c->timer_prescaler_select = 8;    break;
            case 0x02: c->timer_prescaler_select = 64;   break;
            case 0x03: c->timer_prescaler_select = 1024; break;
        }
        c->timer_prescaler = c->timer_prescaler_select;
    } else {
        switch (addr & 0x03) {
            case 0x00: c->port_a_output = val; break;
            case 0x01: c->port_a_ddr    = val; break;
            case 0x02: c->port_b_output = val; break;
            case 0x03: c->port_b_ddr    = val; break;
        }
    }
}
