#pragma once
#include "state_layout.cuh"

#define SCANLINES_PER_FRAME 262

__device__ __forceinline__
uint16_t tia_apply_hmove_offset(uint16_t pos, uint8_t hm) {
    int8_t offset = ((int8_t)hm) >> 4;
    int16_t result = (int16_t)pos - (int16_t)offset;
    result = ((result % 160) + 160) % 160;
    return (uint16_t)result;
}

__device__ __forceinline__
void tia_end_scanline(ThreadCtx* c) {
    c->tia_clock -= 228;
    c->tia_scanline += 1;
    c->tia_wsync = 0;

    if (!c->paddle_dumped && c->paddle_counter < 256) {
        c->paddle_counter += 1;
    }

    if (c->tia_scanline >= SCANLINES_PER_FRAME) {
        c->tia_scanline = 0;
        if (!c->tia_frame_complete) {
            c->tia_frame_complete = 1;
        }
    }
}

__device__ __forceinline__
void tia_tick_n(ThreadCtx* c, uint8_t n) {
    c->tia_clock += (uint16_t)n * 3;
    if (c->tia_clock >= 228) {
        tia_end_scanline(c);
    }
}

__device__ __forceinline__
uint16_t tia_cycles_until_scanline_end(const ThreadCtx* c) {
    uint16_t tia_remaining = 228 - min(c->tia_clock, (uint16_t)228);
    return (tia_remaining + 2) / 3;
}

__device__ __forceinline__
uint16_t tia_skip_to_scanline_end(ThreadCtx* c) {
    uint16_t cpu_cycles = tia_cycles_until_scanline_end(c);
    if (cpu_cycles > 0) {
        c->tia_clock = 228;
        tia_end_scanline(c);
    }
    return cpu_cycles;
}

__device__ __forceinline__
uint8_t tia_read(const ThreadCtx* c, uint16_t addr) {
    switch (addr & 0x0F) {
        case 0x00: return c->cxm0p;
        case 0x01: return c->cxm1p;
        case 0x02: return c->cxp0fb;
        case 0x03: return c->cxp1fb;
        case 0x04: return c->cxm0fb;
        case 0x05: return c->cxm1fb;
        case 0x06: return c->cxblpf;
        case 0x07: return c->cxppmm;
        case 0x08: return (c->paddle_counter >= (uint16_t)c->paddle0) ? 0x80 : 0x00;
        case 0x09: return (c->paddle_counter >= (uint16_t)c->paddle1) ? 0x80 : 0x00;
        case 0x0A: return 0x80;
        case 0x0B: return 0x80;
        case 0x0C: return c->inpt4 ? 0x80 : 0x00;
        case 0x0D: return c->inpt5 ? 0x80 : 0x00;
        default:   return 0;
    }
}

__device__ __forceinline__
void tia_write(ThreadCtx* c, uint16_t addr, uint8_t val) {
    switch (addr & 0x3F) {
        case 0x00:
            if ((c->tia_vsync & 0x02) == 0 && (val & 0x02) != 0) {
                c->tia_scanline = 0;
                c->tia_frame_complete = 1;
            }
            c->tia_vsync = val;
            break;
        case 0x01:
            if (val & 0x80) {
                c->paddle_counter = 0;
                c->paddle_dumped = 1;
            } else if (c->paddle_dumped) {
                c->paddle_dumped = 0;
            }
            c->tia_vblank = val;
            break;
        case 0x02: c->tia_wsync = 1; break;
        case 0x03: break;
        case 0x04: c->nusiz0 = val; break;
        case 0x05: c->nusiz1 = val; break;
        case 0x06: c->colup0 = val; break;
        case 0x07: c->colup1 = val; break;
        case 0x08: c->colupf = val; break;
        case 0x09: c->colubk = val; break;
        case 0x0A: c->ctrlpf = val; break;
        case 0x0B: c->refp0 = (val & 0x08) != 0; break;
        case 0x0C: c->refp1 = (val & 0x08) != 0; break;
        case 0x0D: c->pf0 = val; break;
        case 0x0E: c->pf1 = val; break;
        case 0x0F: c->pf2 = val; break;
        case 0x10: c->pos_p0 = (uint16_t)(((int16_t)c->tia_clock - 68 + 5) % 160 + 160) % 160; break;
        case 0x11: c->pos_p1 = (uint16_t)(((int16_t)c->tia_clock - 68 + 5) % 160 + 160) % 160; break;
        case 0x12: c->pos_m0 = (uint16_t)(((int16_t)c->tia_clock - 68 + 5) % 160 + 160) % 160; break;
        case 0x13: c->pos_m1 = (uint16_t)(((int16_t)c->tia_clock - 68 + 5) % 160 + 160) % 160; break;
        case 0x14: c->pos_bl = (uint16_t)(((int16_t)c->tia_clock - 68 + 5) % 160 + 160) % 160; break;
        case 0x15: case 0x16: case 0x17: case 0x18: case 0x19: case 0x1A:
            break;
        case 0x1B: c->grp0_old = c->grp0; c->grp0 = val; break;
        case 0x1C: c->grp1_old = c->grp1; c->grp1 = val; c->enabl_old = c->enabl; break;
        case 0x1D: c->enam0 = (val & 0x02) != 0; break;
        case 0x1E: c->enam1 = (val & 0x02) != 0; break;
        case 0x1F: c->enabl = (val & 0x02) != 0; break;
        case 0x20: c->hmp0 = val; break;
        case 0x21: c->hmp1 = val; break;
        case 0x22: c->hmm0 = val; break;
        case 0x23: c->hmm1 = val; break;
        case 0x24: c->hmbl = val; break;
        case 0x25: c->vdelp0 = (val & 0x01) != 0; break;
        case 0x26: c->vdelp1 = (val & 0x01) != 0; break;
        case 0x27: c->vdelbl = (val & 0x01) != 0; break;
        case 0x28: c->resmp0 = (val & 0x02) != 0; break;
        case 0x29: c->resmp1 = (val & 0x02) != 0; break;
        case 0x2A:
            c->pos_p0 = tia_apply_hmove_offset(c->pos_p0, c->hmp0);
            c->pos_p1 = tia_apply_hmove_offset(c->pos_p1, c->hmp1);
            c->pos_m0 = tia_apply_hmove_offset(c->pos_m0, c->hmm0);
            c->pos_m1 = tia_apply_hmove_offset(c->pos_m1, c->hmm1);
            c->pos_bl = tia_apply_hmove_offset(c->pos_bl, c->hmbl);
            break;
        case 0x2B:
            c->hmp0 = 0; c->hmp1 = 0;
            c->hmm0 = 0; c->hmm1 = 0;
            c->hmbl = 0;
            break;
        case 0x2C:
            c->cxm0p = 0; c->cxm1p = 0;
            c->cxp0fb = 0; c->cxp1fb = 0;
            c->cxm0fb = 0; c->cxm1fb = 0;
            c->cxblpf = 0; c->cxppmm = 0;
            break;
        default: break;
    }
}
