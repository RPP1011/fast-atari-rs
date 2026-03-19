#pragma once
#include <cstdint>

// Bank switching schemes, matching Rust BankScheme enum.
enum BankScheme : uint8_t {
    BANK_FIXED = 0,
    BANK_F8    = 1,
    BANK_F6    = 2,
    BANK_F4    = 3,
};

// ============================================================
// AtariState: global memory layout (matches Rust AtariStateGpu).
// Used only for load/store at frame boundaries.
// ============================================================
struct AtariState {
    // CPU registers (8 bytes)
    uint8_t  a;
    uint8_t  x;
    uint8_t  y;
    uint8_t  sp;
    uint16_t pc;
    uint8_t  status;
    uint8_t  _pad0;

    // PIA RAM (128 bytes)
    uint8_t ram[128];

    // PIA I/O + timer
    uint8_t  port_a_output;
    uint8_t  port_a_ddr;
    uint8_t  port_a_input;
    uint8_t  port_b_output;
    uint8_t  port_b_ddr;
    uint8_t  port_b_input;
    uint16_t timer_value;
    uint16_t timer_prescaler;
    uint16_t timer_prescaler_select;
    uint8_t  timer_underflow;
    uint8_t  _pad1;
    uint8_t  _pad2;
    uint8_t  _pad3;

    // HeadlessTia
    uint16_t tia_clock;
    uint16_t tia_scanline;
    uint8_t  tia_wsync;
    uint8_t  tia_frame_complete;
    uint8_t  tia_vsync;
    uint8_t  tia_vblank;

    // Collision latches
    uint8_t  cxm0p, cxm1p, cxp0fb, cxp1fb;
    uint8_t  cxm0fb, cxm1fb, cxblpf, cxppmm;

    // Input latches
    uint8_t  inpt4, inpt5;

    // Paddle
    uint8_t  paddle0, paddle1;
    uint16_t paddle_counter;
    uint8_t  paddle_dumped;
    uint8_t  _pad4;

    // Object positions
    uint16_t pos_p0, pos_p1, pos_m0, pos_m1, pos_bl;
    uint8_t  hmp0, hmp1, hmm0, hmm1, hmbl;
    uint8_t  _pad5;

    // Graphics registers
    uint8_t  grp0, grp1, grp0_old, grp1_old;
    uint8_t  enam0, enam1, enabl, enabl_old;
    uint8_t  vdelp0, vdelp1, vdelbl;
    uint8_t  resmp0, resmp1;
    uint8_t  nusiz0, nusiz1;

    // Color/playfield registers
    uint8_t  colup0, colup1, colupf, colubk;
    uint8_t  ctrlpf;
    uint8_t  refp0, refp1;
    uint8_t  pf0, pf1, pf2;

    // Bankswitching
    uint8_t  bank;
    uint8_t  scheme;

    uint8_t  _pad_end[2];
};

// ============================================================
// ThreadCtx: register-resident state for the hot inner loop.
// Loaded from AtariState at frame start, stored back at frame end.
// RAM is NOT here — it lives in shared memory.
// ============================================================
struct ThreadCtx {
    // CPU registers
    uint8_t  a, x, y, sp;
    uint16_t pc;
    uint8_t  status;

    // PIA I/O + timer
    uint8_t  port_a_output, port_a_ddr, port_a_input;
    uint8_t  port_b_output, port_b_ddr, port_b_input;
    uint16_t timer_value, timer_prescaler, timer_prescaler_select;
    uint8_t  timer_underflow;

    // TIA timing (hot — accessed every cycle)
    uint16_t tia_clock, tia_scanline;
    uint8_t  tia_wsync, tia_frame_complete, tia_vsync, tia_vblank;

    // Collision latches
    uint8_t  cxm0p, cxm1p, cxp0fb, cxp1fb;
    uint8_t  cxm0fb, cxm1fb, cxblpf, cxppmm;

    // Input latches
    uint8_t  inpt4, inpt5;

    // Paddle
    uint8_t  paddle0, paddle1;
    uint16_t paddle_counter;
    uint8_t  paddle_dumped;

    // Object positions
    uint16_t pos_p0, pos_p1, pos_m0, pos_m1, pos_bl;
    uint8_t  hmp0, hmp1, hmm0, hmm1, hmbl;

    // Graphics registers
    uint8_t  grp0, grp1, grp0_old, grp1_old;
    uint8_t  enam0, enam1, enabl, enabl_old;
    uint8_t  vdelp0, vdelp1, vdelbl;
    uint8_t  resmp0, resmp1;
    uint8_t  nusiz0, nusiz1;

    // Color/playfield registers
    uint8_t  colup0, colup1, colupf, colubk;
    uint8_t  ctrlpf;
    uint8_t  refp0, refp1;
    uint8_t  pf0, pf1, pf2;

    // Bankswitching
    uint8_t  bank, scheme;
};

// ============================================================
// Load/Store between global AtariState and register ThreadCtx
// ============================================================

__device__ __forceinline__
void load_ctx(ThreadCtx* c, const AtariState* g) {
    c->a = g->a; c->x = g->x; c->y = g->y; c->sp = g->sp;
    c->pc = g->pc; c->status = g->status;

    c->port_a_output = g->port_a_output; c->port_a_ddr = g->port_a_ddr;
    c->port_a_input = g->port_a_input;
    c->port_b_output = g->port_b_output; c->port_b_ddr = g->port_b_ddr;
    c->port_b_input = g->port_b_input;
    c->timer_value = g->timer_value; c->timer_prescaler = g->timer_prescaler;
    c->timer_prescaler_select = g->timer_prescaler_select;
    c->timer_underflow = g->timer_underflow;

    c->tia_clock = g->tia_clock; c->tia_scanline = g->tia_scanline;
    c->tia_wsync = g->tia_wsync; c->tia_frame_complete = g->tia_frame_complete;
    c->tia_vsync = g->tia_vsync; c->tia_vblank = g->tia_vblank;

    c->cxm0p = g->cxm0p; c->cxm1p = g->cxm1p;
    c->cxp0fb = g->cxp0fb; c->cxp1fb = g->cxp1fb;
    c->cxm0fb = g->cxm0fb; c->cxm1fb = g->cxm1fb;
    c->cxblpf = g->cxblpf; c->cxppmm = g->cxppmm;

    c->inpt4 = g->inpt4; c->inpt5 = g->inpt5;
    c->paddle0 = g->paddle0; c->paddle1 = g->paddle1;
    c->paddle_counter = g->paddle_counter; c->paddle_dumped = g->paddle_dumped;

    c->pos_p0 = g->pos_p0; c->pos_p1 = g->pos_p1;
    c->pos_m0 = g->pos_m0; c->pos_m1 = g->pos_m1; c->pos_bl = g->pos_bl;
    c->hmp0 = g->hmp0; c->hmp1 = g->hmp1;
    c->hmm0 = g->hmm0; c->hmm1 = g->hmm1; c->hmbl = g->hmbl;

    c->grp0 = g->grp0; c->grp1 = g->grp1;
    c->grp0_old = g->grp0_old; c->grp1_old = g->grp1_old;
    c->enam0 = g->enam0; c->enam1 = g->enam1;
    c->enabl = g->enabl; c->enabl_old = g->enabl_old;
    c->vdelp0 = g->vdelp0; c->vdelp1 = g->vdelp1; c->vdelbl = g->vdelbl;
    c->resmp0 = g->resmp0; c->resmp1 = g->resmp1;
    c->nusiz0 = g->nusiz0; c->nusiz1 = g->nusiz1;

    c->colup0 = g->colup0; c->colup1 = g->colup1;
    c->colupf = g->colupf; c->colubk = g->colubk;
    c->ctrlpf = g->ctrlpf;
    c->refp0 = g->refp0; c->refp1 = g->refp1;
    c->pf0 = g->pf0; c->pf1 = g->pf1; c->pf2 = g->pf2;

    c->bank = g->bank; c->scheme = g->scheme;
}

__device__ __forceinline__
void store_ctx(AtariState* g, const ThreadCtx* c) {
    g->a = c->a; g->x = c->x; g->y = c->y; g->sp = c->sp;
    g->pc = c->pc; g->status = c->status;

    g->port_a_output = c->port_a_output; g->port_a_ddr = c->port_a_ddr;
    g->port_a_input = c->port_a_input;
    g->port_b_output = c->port_b_output; g->port_b_ddr = c->port_b_ddr;
    g->port_b_input = c->port_b_input;
    g->timer_value = c->timer_value; g->timer_prescaler = c->timer_prescaler;
    g->timer_prescaler_select = c->timer_prescaler_select;
    g->timer_underflow = c->timer_underflow;

    g->tia_clock = c->tia_clock; g->tia_scanline = c->tia_scanline;
    g->tia_wsync = c->tia_wsync; g->tia_frame_complete = c->tia_frame_complete;
    g->tia_vsync = c->tia_vsync; g->tia_vblank = c->tia_vblank;

    g->cxm0p = c->cxm0p; g->cxm1p = c->cxm1p;
    g->cxp0fb = c->cxp0fb; g->cxp1fb = c->cxp1fb;
    g->cxm0fb = c->cxm0fb; g->cxm1fb = c->cxm1fb;
    g->cxblpf = c->cxblpf; g->cxppmm = c->cxppmm;

    g->inpt4 = c->inpt4; g->inpt5 = c->inpt5;
    g->paddle0 = c->paddle0; g->paddle1 = c->paddle1;
    g->paddle_counter = c->paddle_counter; g->paddle_dumped = c->paddle_dumped;

    g->pos_p0 = c->pos_p0; g->pos_p1 = c->pos_p1;
    g->pos_m0 = c->pos_m0; g->pos_m1 = c->pos_m1; g->pos_bl = c->pos_bl;
    g->hmp0 = c->hmp0; g->hmp1 = c->hmp1;
    g->hmm0 = c->hmm0; g->hmm1 = c->hmm1; g->hmbl = c->hmbl;

    g->grp0 = c->grp0; g->grp1 = c->grp1;
    g->grp0_old = c->grp0_old; g->grp1_old = c->grp1_old;
    g->enam0 = c->enam0; g->enam1 = c->enam1;
    g->enabl = c->enabl; g->enabl_old = c->enabl_old;
    g->vdelp0 = c->vdelp0; g->vdelp1 = c->vdelp1; g->vdelbl = c->vdelbl;
    g->resmp0 = c->resmp0; g->resmp1 = c->resmp1;
    g->nusiz0 = c->nusiz0; g->nusiz1 = c->nusiz1;

    g->colup0 = c->colup0; g->colup1 = c->colup1;
    g->colupf = c->colupf; g->colubk = c->colubk;
    g->ctrlpf = c->ctrlpf;
    g->refp0 = c->refp0; g->refp1 = c->refp1;
    g->pf0 = c->pf0; g->pf1 = c->pf1; g->pf2 = c->pf2;

    g->bank = c->bank; g->scheme = c->scheme;
}

#define MAX_ROM_SIZE 32768
#define BLOCK_SIZE 128
