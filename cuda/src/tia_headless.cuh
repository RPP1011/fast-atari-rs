#pragma once
#include "state_layout.cuh"

#define SCANLINES_PER_FRAME 262

// Apply HMOVE offset to a position register.
__device__ __forceinline__
uint16_t tia_apply_hmove_offset(uint16_t pos, uint8_t hm) {
    int8_t offset = ((int8_t)hm) >> 4;
    int16_t result = (int16_t)pos - (int16_t)offset;
    // rem_euclid(160)
    result = ((result % 160) + 160) % 160;
    return (uint16_t)result;
}

// End of scanline processing.
__device__ __forceinline__
void tia_end_scanline(AtariState* s) {
    s->tia_clock -= 228;
    s->tia_scanline += 1;
    s->tia_wsync = 0;

    if (!s->paddle_dumped && s->paddle_counter < 256) {
        s->paddle_counter += 1;
    }

    if (s->tia_scanline >= SCANLINES_PER_FRAME) {
        s->tia_scanline = 0;
        if (!s->tia_frame_complete) {
            s->tia_frame_complete = 1;
        }
    }
}

// Advance by n CPU cycles (3*n TIA clocks).
__device__ __forceinline__
void tia_tick_n(AtariState* s, uint8_t n) {
    s->tia_clock += (uint16_t)n * 3;
    if (s->tia_clock >= 228) {
        tia_end_scanline(s);
    }
}

// How many CPU cycles remain until end of current scanline.
__device__ __forceinline__
uint16_t tia_cycles_until_scanline_end(const AtariState* s) {
    uint16_t tia_remaining = 228 - min(s->tia_clock, (uint16_t)228);
    return (tia_remaining + 2) / 3;
}

// Fast-forward to end of current scanline (WSYNC). Returns CPU cycles skipped.
__device__ __forceinline__
uint16_t tia_skip_to_scanline_end(AtariState* s) {
    uint16_t cpu_cycles = tia_cycles_until_scanline_end(s);
    if (cpu_cycles > 0) {
        s->tia_clock = 228; // Set to exactly 228 so end_scanline produces 0
        tia_end_scanline(s);
    }
    return cpu_cycles;
}

// TIA read register.
__device__ __forceinline__
uint8_t tia_read(const AtariState* s, uint16_t addr) {
    switch (addr & 0x0F) {
        case 0x00: return s->cxm0p;
        case 0x01: return s->cxm1p;
        case 0x02: return s->cxp0fb;
        case 0x03: return s->cxp1fb;
        case 0x04: return s->cxm0fb;
        case 0x05: return s->cxm1fb;
        case 0x06: return s->cxblpf;
        case 0x07: return s->cxppmm;
        case 0x08: return (s->paddle_counter >= (uint16_t)s->paddle0) ? 0x80 : 0x00;
        case 0x09: return (s->paddle_counter >= (uint16_t)s->paddle1) ? 0x80 : 0x00;
        case 0x0A: return 0x80;
        case 0x0B: return 0x80;
        case 0x0C: return s->inpt4 ? 0x80 : 0x00;
        case 0x0D: return s->inpt5 ? 0x80 : 0x00;
        default:   return 0;
    }
}

// TIA write register.
__device__ __forceinline__
void tia_write(AtariState* s, uint16_t addr, uint8_t val) {
    switch (addr & 0x3F) {
        case 0x00: // VSYNC
            if ((s->tia_vsync & 0x02) == 0 && (val & 0x02) != 0) {
                s->tia_scanline = 0;
                s->tia_frame_complete = 1;
            }
            s->tia_vsync = val;
            break;
        case 0x01: // VBLANK
            if (val & 0x80) {
                s->paddle_counter = 0;
                s->paddle_dumped = 1;
            } else if (s->paddle_dumped) {
                s->paddle_dumped = 0;
            }
            s->tia_vblank = val;
            break;
        case 0x02: s->tia_wsync = 1; break;
        case 0x03: break; // RSYNC
        case 0x04: s->nusiz0 = val; break;
        case 0x05: s->nusiz1 = val; break;
        case 0x06: s->colup0 = val; break;
        case 0x07: s->colup1 = val; break;
        case 0x08: s->colupf = val; break;
        case 0x09: s->colubk = val; break;
        case 0x0A: s->ctrlpf = val; break;
        case 0x0B: s->refp0 = (val & 0x08) != 0; break;
        case 0x0C: s->refp1 = (val & 0x08) != 0; break;
        case 0x0D: s->pf0 = val; break;
        case 0x0E: s->pf1 = val; break;
        case 0x0F: s->pf2 = val; break;
        case 0x10: s->pos_p0 = (uint16_t)(((int16_t)s->tia_clock - 68 + 5) % 160 + 160) % 160; break;
        case 0x11: s->pos_p1 = (uint16_t)(((int16_t)s->tia_clock - 68 + 5) % 160 + 160) % 160; break;
        case 0x12: s->pos_m0 = (uint16_t)(((int16_t)s->tia_clock - 68 + 5) % 160 + 160) % 160; break;
        case 0x13: s->pos_m1 = (uint16_t)(((int16_t)s->tia_clock - 68 + 5) % 160 + 160) % 160; break;
        case 0x14: s->pos_bl = (uint16_t)(((int16_t)s->tia_clock - 68 + 5) % 160 + 160) % 160; break;
        case 0x15: case 0x16: case 0x17: case 0x18: case 0x19: case 0x1A:
            break; // Audio — ignored
        case 0x1B: s->grp0_old = s->grp0; s->grp0 = val; break;
        case 0x1C: s->grp1_old = s->grp1; s->grp1 = val; s->enabl_old = s->enabl; break;
        case 0x1D: s->enam0 = (val & 0x02) != 0; break;
        case 0x1E: s->enam1 = (val & 0x02) != 0; break;
        case 0x1F: s->enabl = (val & 0x02) != 0; break;
        case 0x20: s->hmp0 = val; break;
        case 0x21: s->hmp1 = val; break;
        case 0x22: s->hmm0 = val; break;
        case 0x23: s->hmm1 = val; break;
        case 0x24: s->hmbl = val; break;
        case 0x25: s->vdelp0 = (val & 0x01) != 0; break;
        case 0x26: s->vdelp1 = (val & 0x01) != 0; break;
        case 0x27: s->vdelbl = (val & 0x01) != 0; break;
        case 0x28: s->resmp0 = (val & 0x02) != 0; break;
        case 0x29: s->resmp1 = (val & 0x02) != 0; break;
        case 0x2A: // HMOVE
            s->pos_p0 = tia_apply_hmove_offset(s->pos_p0, s->hmp0);
            s->pos_p1 = tia_apply_hmove_offset(s->pos_p1, s->hmp1);
            s->pos_m0 = tia_apply_hmove_offset(s->pos_m0, s->hmm0);
            s->pos_m1 = tia_apply_hmove_offset(s->pos_m1, s->hmm1);
            s->pos_bl = tia_apply_hmove_offset(s->pos_bl, s->hmbl);
            break;
        case 0x2B: // HMCLR
            s->hmp0 = 0; s->hmp1 = 0;
            s->hmm0 = 0; s->hmm1 = 0;
            s->hmbl = 0;
            break;
        case 0x2C: // CXCLR
            s->cxm0p = 0; s->cxm1p = 0;
            s->cxp0fb = 0; s->cxp1fb = 0;
            s->cxm0fb = 0; s->cxm1fb = 0;
            s->cxblpf = 0; s->cxppmm = 0;
            break;
        default: break;
    }
}
