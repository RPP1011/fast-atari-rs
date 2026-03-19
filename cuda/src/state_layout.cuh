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
    uint8_t  a, x, y, sp;
    uint16_t pc;
    uint8_t  status;
    uint8_t  _pad0;
    uint8_t ram[128];
    uint8_t  port_a_output, port_a_ddr, port_a_input;
    uint8_t  port_b_output, port_b_ddr, port_b_input;
    uint16_t timer_value, timer_prescaler, timer_prescaler_select;
    uint8_t  timer_underflow;
    uint8_t  _pad1, _pad2, _pad3;
    uint16_t tia_clock, tia_scanline;
    uint8_t  tia_wsync, tia_frame_complete, tia_vsync, tia_vblank;
    uint8_t  cxm0p, cxm1p, cxp0fb, cxp1fb;
    uint8_t  cxm0fb, cxm1fb, cxblpf, cxppmm;
    uint8_t  inpt4, inpt5;
    uint8_t  paddle0, paddle1;
    uint16_t paddle_counter;
    uint8_t  paddle_dumped, _pad4;
    uint16_t pos_p0, pos_p1, pos_m0, pos_m1, pos_bl;
    uint8_t  hmp0, hmp1, hmm0, hmm1, hmbl, _pad5;
    uint8_t  grp0, grp1, grp0_old, grp1_old;
    uint8_t  enam0, enam1, enabl, enabl_old;
    uint8_t  vdelp0, vdelp1, vdelbl;
    uint8_t  resmp0, resmp1;
    uint8_t  nusiz0, nusiz1;
    uint8_t  colup0, colup1, colupf, colubk;
    uint8_t  ctrlpf;
    uint8_t  refp0, refp1;
    uint8_t  pf0, pf1, pf2;
    uint8_t  bank, scheme;
    uint8_t  _pad_end[2];
};

// ============================================================
// HotCtx: register-resident state — accessed every CPU cycle.
// ~20 bytes → ~6-8 registers. Stays in GPU registers.
// ============================================================
struct HotCtx {
    uint8_t  a, x, y, sp;
    uint16_t pc;
    uint8_t  status;
    // TIA timing
    uint16_t tia_clock, tia_scanline;
    uint8_t  tia_wsync, tia_frame_complete, tia_vsync;
    // PIA timer (accessed on every tick_n)
    uint16_t timer_value, timer_prescaler, timer_prescaler_select;
    uint8_t  timer_underflow;
    // Bankswitching (accessed on every ROM read)
    uint8_t  bank, scheme;
};

// ============================================================
// ColdCtx: infrequently-accessed TIA/PIA state.
// Stored in shared memory in SoA layout to avoid bank conflicts.
// Accessed only on specific memory-mapped reads/writes.
// ~50 bytes per instance.
// ============================================================
// SoA arrays in shared memory — each array is [BLOCK_SIZE] elements.
// Access pattern: cold_xxx[threadIdx.x]
// This ensures consecutive threads hit consecutive addresses (no bank conflicts).
struct ColdCtxPtrs {
    // PIA I/O
    uint8_t* port_a_output;
    uint8_t* port_a_ddr;
    uint8_t* port_a_input;
    uint8_t* port_b_output;
    uint8_t* port_b_ddr;
    uint8_t* port_b_input;
    // TIA vblank
    uint8_t* tia_vblank;
    // Collision latches
    uint8_t* cxm0p;   uint8_t* cxm1p;
    uint8_t* cxp0fb;  uint8_t* cxp1fb;
    uint8_t* cxm0fb;  uint8_t* cxm1fb;
    uint8_t* cxblpf;  uint8_t* cxppmm;
    // Input
    uint8_t* inpt4;    uint8_t* inpt5;
    // Paddle
    uint8_t* paddle0;  uint8_t* paddle1;
    uint16_t* paddle_counter;
    uint8_t* paddle_dumped;
    // Object positions
    uint16_t* pos_p0; uint16_t* pos_p1;
    uint16_t* pos_m0; uint16_t* pos_m1; uint16_t* pos_bl;
    uint8_t* hmp0; uint8_t* hmp1;
    uint8_t* hmm0; uint8_t* hmm1; uint8_t* hmbl;
    // Graphics
    uint8_t* grp0; uint8_t* grp1;
    uint8_t* grp0_old; uint8_t* grp1_old;
    uint8_t* enam0; uint8_t* enam1;
    uint8_t* enabl; uint8_t* enabl_old;
    uint8_t* vdelp0; uint8_t* vdelp1; uint8_t* vdelbl;
    uint8_t* resmp0; uint8_t* resmp1;
    uint8_t* nusiz0; uint8_t* nusiz1;
    // Color/PF
    uint8_t* colup0; uint8_t* colup1;
    uint8_t* colupf; uint8_t* colubk;
    uint8_t* ctrlpf;
    uint8_t* refp0; uint8_t* refp1;
    uint8_t* pf0; uint8_t* pf1; uint8_t* pf2;
};

// Total shared memory per block for cold state SoA:
// 6×u8 (PIA IO) + 1×u8 (vblank) + 8×u8 (collision) + 2×u8 (input)
// + 2×u8 (paddle) + 1×u16 (paddle_counter) + 1×u8 (paddle_dumped)
// + 5×u16 (positions) + 5×u8 (hm) + 15×u8 (graphics) + 10×u8 (color/pf)
// = (6+1+8+2+2+1+5+15+10)×128 + (1+5)×128×2
// = 50×128 + 6×256 = 6400 + 1536 = 7936 bytes
// Plus 128×128 = 16384 for RAM = 24320 bytes total
// sm_89 has 100KB configurable shared → 4 blocks/SM at 24KB

#define MAX_ROM_SIZE 32768
#define BLOCK_SIZE 128

// ============================================================
// Shared memory layout declaration macro
// ============================================================
#define DECLARE_COLD_SHARED() \
    __shared__ uint8_t _cs_u8[50][BLOCK_SIZE]; \
    __shared__ uint16_t _cs_u16[6][BLOCK_SIZE];

// Build ColdCtxPtrs from the shared arrays. t = threadIdx.x.
__device__ __forceinline__
ColdCtxPtrs make_cold_ptrs(uint8_t _cs_u8[][BLOCK_SIZE], uint16_t _cs_u16[][BLOCK_SIZE]) {
    ColdCtxPtrs p;
    int i = 0;
    p.port_a_output = &_cs_u8[i++][0]; p.port_a_ddr = &_cs_u8[i++][0];
    p.port_a_input  = &_cs_u8[i++][0]; p.port_b_output = &_cs_u8[i++][0];
    p.port_b_ddr    = &_cs_u8[i++][0]; p.port_b_input  = &_cs_u8[i++][0];
    p.tia_vblank    = &_cs_u8[i++][0];
    p.cxm0p = &_cs_u8[i++][0]; p.cxm1p = &_cs_u8[i++][0];
    p.cxp0fb = &_cs_u8[i++][0]; p.cxp1fb = &_cs_u8[i++][0];
    p.cxm0fb = &_cs_u8[i++][0]; p.cxm1fb = &_cs_u8[i++][0];
    p.cxblpf = &_cs_u8[i++][0]; p.cxppmm = &_cs_u8[i++][0];
    p.inpt4 = &_cs_u8[i++][0]; p.inpt5 = &_cs_u8[i++][0];
    p.paddle0 = &_cs_u8[i++][0]; p.paddle1 = &_cs_u8[i++][0];
    p.paddle_dumped = &_cs_u8[i++][0];
    p.hmp0 = &_cs_u8[i++][0]; p.hmp1 = &_cs_u8[i++][0];
    p.hmm0 = &_cs_u8[i++][0]; p.hmm1 = &_cs_u8[i++][0]; p.hmbl = &_cs_u8[i++][0];
    p.grp0 = &_cs_u8[i++][0]; p.grp1 = &_cs_u8[i++][0];
    p.grp0_old = &_cs_u8[i++][0]; p.grp1_old = &_cs_u8[i++][0];
    p.enam0 = &_cs_u8[i++][0]; p.enam1 = &_cs_u8[i++][0];
    p.enabl = &_cs_u8[i++][0]; p.enabl_old = &_cs_u8[i++][0];
    p.vdelp0 = &_cs_u8[i++][0]; p.vdelp1 = &_cs_u8[i++][0]; p.vdelbl = &_cs_u8[i++][0];
    p.resmp0 = &_cs_u8[i++][0]; p.resmp1 = &_cs_u8[i++][0];
    p.nusiz0 = &_cs_u8[i++][0]; p.nusiz1 = &_cs_u8[i++][0];
    p.colup0 = &_cs_u8[i++][0]; p.colup1 = &_cs_u8[i++][0];
    p.colupf = &_cs_u8[i++][0]; p.colubk = &_cs_u8[i++][0];
    p.ctrlpf = &_cs_u8[i++][0];
    p.refp0 = &_cs_u8[i++][0]; p.refp1 = &_cs_u8[i++][0];
    p.pf0 = &_cs_u8[i++][0]; p.pf1 = &_cs_u8[i++][0]; p.pf2 = &_cs_u8[i++][0];
    // u16 arrays
    int j = 0;
    p.paddle_counter = &_cs_u16[j++][0];
    p.pos_p0 = &_cs_u16[j++][0]; p.pos_p1 = &_cs_u16[j++][0];
    p.pos_m0 = &_cs_u16[j++][0]; p.pos_m1 = &_cs_u16[j++][0]; p.pos_bl = &_cs_u16[j++][0];
    return p;
}

// Macro for accessing cold field: cold.field[threadIdx.x]
#define C(field) cold.field[threadIdx.x]

// ============================================================
// Load/Store between global AtariState and HotCtx + cold shared
// ============================================================
__device__ __forceinline__
void load_hot(HotCtx* h, const AtariState* g) {
    h->a = g->a; h->x = g->x; h->y = g->y; h->sp = g->sp;
    h->pc = g->pc; h->status = g->status;
    h->tia_clock = g->tia_clock; h->tia_scanline = g->tia_scanline;
    h->tia_wsync = g->tia_wsync; h->tia_frame_complete = g->tia_frame_complete;
    h->tia_vsync = g->tia_vsync;
    h->timer_value = g->timer_value; h->timer_prescaler = g->timer_prescaler;
    h->timer_prescaler_select = g->timer_prescaler_select;
    h->timer_underflow = g->timer_underflow;
    h->bank = g->bank; h->scheme = g->scheme;
}

__device__ __forceinline__
void store_hot(AtariState* g, const HotCtx* h) {
    g->a = h->a; g->x = h->x; g->y = h->y; g->sp = h->sp;
    g->pc = h->pc; g->status = h->status;
    g->tia_clock = h->tia_clock; g->tia_scanline = h->tia_scanline;
    g->tia_wsync = h->tia_wsync; g->tia_frame_complete = h->tia_frame_complete;
    g->tia_vsync = h->tia_vsync;
    g->timer_value = h->timer_value; g->timer_prescaler = h->timer_prescaler;
    g->timer_prescaler_select = h->timer_prescaler_select;
    g->timer_underflow = h->timer_underflow;
    g->bank = h->bank; g->scheme = h->scheme;
}

__device__ __forceinline__
void load_cold(ColdCtxPtrs& cold, const AtariState* g, int t) {
    cold.port_a_output[t] = g->port_a_output; cold.port_a_ddr[t] = g->port_a_ddr;
    cold.port_a_input[t] = g->port_a_input;
    cold.port_b_output[t] = g->port_b_output; cold.port_b_ddr[t] = g->port_b_ddr;
    cold.port_b_input[t] = g->port_b_input;
    cold.tia_vblank[t] = g->tia_vblank;
    cold.cxm0p[t] = g->cxm0p; cold.cxm1p[t] = g->cxm1p;
    cold.cxp0fb[t] = g->cxp0fb; cold.cxp1fb[t] = g->cxp1fb;
    cold.cxm0fb[t] = g->cxm0fb; cold.cxm1fb[t] = g->cxm1fb;
    cold.cxblpf[t] = g->cxblpf; cold.cxppmm[t] = g->cxppmm;
    cold.inpt4[t] = g->inpt4; cold.inpt5[t] = g->inpt5;
    cold.paddle0[t] = g->paddle0; cold.paddle1[t] = g->paddle1;
    cold.paddle_counter[t] = g->paddle_counter; cold.paddle_dumped[t] = g->paddle_dumped;
    cold.pos_p0[t] = g->pos_p0; cold.pos_p1[t] = g->pos_p1;
    cold.pos_m0[t] = g->pos_m0; cold.pos_m1[t] = g->pos_m1; cold.pos_bl[t] = g->pos_bl;
    cold.hmp0[t] = g->hmp0; cold.hmp1[t] = g->hmp1;
    cold.hmm0[t] = g->hmm0; cold.hmm1[t] = g->hmm1; cold.hmbl[t] = g->hmbl;
    cold.grp0[t] = g->grp0; cold.grp1[t] = g->grp1;
    cold.grp0_old[t] = g->grp0_old; cold.grp1_old[t] = g->grp1_old;
    cold.enam0[t] = g->enam0; cold.enam1[t] = g->enam1;
    cold.enabl[t] = g->enabl; cold.enabl_old[t] = g->enabl_old;
    cold.vdelp0[t] = g->vdelp0; cold.vdelp1[t] = g->vdelp1; cold.vdelbl[t] = g->vdelbl;
    cold.resmp0[t] = g->resmp0; cold.resmp1[t] = g->resmp1;
    cold.nusiz0[t] = g->nusiz0; cold.nusiz1[t] = g->nusiz1;
    cold.colup0[t] = g->colup0; cold.colup1[t] = g->colup1;
    cold.colupf[t] = g->colupf; cold.colubk[t] = g->colubk;
    cold.ctrlpf[t] = g->ctrlpf;
    cold.refp0[t] = g->refp0; cold.refp1[t] = g->refp1;
    cold.pf0[t] = g->pf0; cold.pf1[t] = g->pf1; cold.pf2[t] = g->pf2;
}

__device__ __forceinline__
void store_cold(AtariState* g, const ColdCtxPtrs& cold, int t) {
    g->port_a_output = cold.port_a_output[t]; g->port_a_ddr = cold.port_a_ddr[t];
    g->port_a_input = cold.port_a_input[t];
    g->port_b_output = cold.port_b_output[t]; g->port_b_ddr = cold.port_b_ddr[t];
    g->port_b_input = cold.port_b_input[t];
    g->tia_vblank = cold.tia_vblank[t];
    g->cxm0p = cold.cxm0p[t]; g->cxm1p = cold.cxm1p[t];
    g->cxp0fb = cold.cxp0fb[t]; g->cxp1fb = cold.cxp1fb[t];
    g->cxm0fb = cold.cxm0fb[t]; g->cxm1fb = cold.cxm1fb[t];
    g->cxblpf = cold.cxblpf[t]; g->cxppmm = cold.cxppmm[t];
    g->inpt4 = cold.inpt4[t]; g->inpt5 = cold.inpt5[t];
    g->paddle0 = cold.paddle0[t]; g->paddle1 = cold.paddle1[t];
    g->paddle_counter = cold.paddle_counter[t]; g->paddle_dumped = cold.paddle_dumped[t];
    g->pos_p0 = cold.pos_p0[t]; g->pos_p1 = cold.pos_p1[t];
    g->pos_m0 = cold.pos_m0[t]; g->pos_m1 = cold.pos_m1[t]; g->pos_bl = cold.pos_bl[t];
    g->hmp0 = cold.hmp0[t]; g->hmp1 = cold.hmp1[t];
    g->hmm0 = cold.hmm0[t]; g->hmm1 = cold.hmm1[t]; g->hmbl = cold.hmbl[t];
    g->grp0 = cold.grp0[t]; g->grp1 = cold.grp1[t];
    g->grp0_old = cold.grp0_old[t]; g->grp1_old = cold.grp1_old[t];
    g->enam0 = cold.enam0[t]; g->enam1 = cold.enam1[t];
    g->enabl = cold.enabl[t]; g->enabl_old = cold.enabl_old[t];
    g->vdelp0 = cold.vdelp0[t]; g->vdelp1 = cold.vdelp1[t]; g->vdelbl = cold.vdelbl[t];
    g->resmp0 = cold.resmp0[t]; g->resmp1 = cold.resmp1[t];
    g->nusiz0 = cold.nusiz0[t]; g->nusiz1 = cold.nusiz1[t];
    g->colup0 = cold.colup0[t]; g->colup1 = cold.colup1[t];
    g->colupf = cold.colupf[t]; g->colubk = cold.colubk[t];
    g->ctrlpf = cold.ctrlpf[t];
    g->refp0 = cold.refp0[t]; g->refp1 = cold.refp1[t];
    g->pf0 = cold.pf0[t]; g->pf1 = cold.pf1[t]; g->pf2 = cold.pf2[t];
}

// Keep the old ThreadCtx and its load/store for Phase 2/3/4 compatibility
struct ThreadCtx {
    uint8_t  a, x, y, sp;
    uint16_t pc;
    uint8_t  status;
    uint8_t  port_a_output, port_a_ddr, port_a_input;
    uint8_t  port_b_output, port_b_ddr, port_b_input;
    uint16_t timer_value, timer_prescaler, timer_prescaler_select;
    uint8_t  timer_underflow;
    uint16_t tia_clock, tia_scanline;
    uint8_t  tia_wsync, tia_frame_complete, tia_vsync, tia_vblank;
    uint8_t  cxm0p, cxm1p, cxp0fb, cxp1fb;
    uint8_t  cxm0fb, cxm1fb, cxblpf, cxppmm;
    uint8_t  inpt4, inpt5;
    uint8_t  paddle0, paddle1;
    uint16_t paddle_counter;
    uint8_t  paddle_dumped;
    uint16_t pos_p0, pos_p1, pos_m0, pos_m1, pos_bl;
    uint8_t  hmp0, hmp1, hmm0, hmm1, hmbl;
    uint8_t  grp0, grp1, grp0_old, grp1_old;
    uint8_t  enam0, enam1, enabl, enabl_old;
    uint8_t  vdelp0, vdelp1, vdelbl;
    uint8_t  resmp0, resmp1;
    uint8_t  nusiz0, nusiz1;
    uint8_t  colup0, colup1, colupf, colubk;
    uint8_t  ctrlpf;
    uint8_t  refp0, refp1;
    uint8_t  pf0, pf1, pf2;
    uint8_t  bank, scheme;
};

__device__ __forceinline__
void load_ctx(ThreadCtx* c, const AtariState* g) {
    c->a=g->a; c->x=g->x; c->y=g->y; c->sp=g->sp; c->pc=g->pc; c->status=g->status;
    c->port_a_output=g->port_a_output; c->port_a_ddr=g->port_a_ddr; c->port_a_input=g->port_a_input;
    c->port_b_output=g->port_b_output; c->port_b_ddr=g->port_b_ddr; c->port_b_input=g->port_b_input;
    c->timer_value=g->timer_value; c->timer_prescaler=g->timer_prescaler;
    c->timer_prescaler_select=g->timer_prescaler_select; c->timer_underflow=g->timer_underflow;
    c->tia_clock=g->tia_clock; c->tia_scanline=g->tia_scanline;
    c->tia_wsync=g->tia_wsync; c->tia_frame_complete=g->tia_frame_complete;
    c->tia_vsync=g->tia_vsync; c->tia_vblank=g->tia_vblank;
    c->cxm0p=g->cxm0p; c->cxm1p=g->cxm1p; c->cxp0fb=g->cxp0fb; c->cxp1fb=g->cxp1fb;
    c->cxm0fb=g->cxm0fb; c->cxm1fb=g->cxm1fb; c->cxblpf=g->cxblpf; c->cxppmm=g->cxppmm;
    c->inpt4=g->inpt4; c->inpt5=g->inpt5;
    c->paddle0=g->paddle0; c->paddle1=g->paddle1; c->paddle_counter=g->paddle_counter; c->paddle_dumped=g->paddle_dumped;
    c->pos_p0=g->pos_p0; c->pos_p1=g->pos_p1; c->pos_m0=g->pos_m0; c->pos_m1=g->pos_m1; c->pos_bl=g->pos_bl;
    c->hmp0=g->hmp0; c->hmp1=g->hmp1; c->hmm0=g->hmm0; c->hmm1=g->hmm1; c->hmbl=g->hmbl;
    c->grp0=g->grp0; c->grp1=g->grp1; c->grp0_old=g->grp0_old; c->grp1_old=g->grp1_old;
    c->enam0=g->enam0; c->enam1=g->enam1; c->enabl=g->enabl; c->enabl_old=g->enabl_old;
    c->vdelp0=g->vdelp0; c->vdelp1=g->vdelp1; c->vdelbl=g->vdelbl;
    c->resmp0=g->resmp0; c->resmp1=g->resmp1; c->nusiz0=g->nusiz0; c->nusiz1=g->nusiz1;
    c->colup0=g->colup0; c->colup1=g->colup1; c->colupf=g->colupf; c->colubk=g->colubk;
    c->ctrlpf=g->ctrlpf; c->refp0=g->refp0; c->refp1=g->refp1;
    c->pf0=g->pf0; c->pf1=g->pf1; c->pf2=g->pf2;
    c->bank=g->bank; c->scheme=g->scheme;
}

__device__ __forceinline__
void store_ctx(AtariState* g, const ThreadCtx* c) {
    g->a=c->a; g->x=c->x; g->y=c->y; g->sp=c->sp; g->pc=c->pc; g->status=c->status;
    g->port_a_output=c->port_a_output; g->port_a_ddr=c->port_a_ddr; g->port_a_input=c->port_a_input;
    g->port_b_output=c->port_b_output; g->port_b_ddr=c->port_b_ddr; g->port_b_input=c->port_b_input;
    g->timer_value=c->timer_value; g->timer_prescaler=c->timer_prescaler;
    g->timer_prescaler_select=c->timer_prescaler_select; g->timer_underflow=c->timer_underflow;
    g->tia_clock=c->tia_clock; g->tia_scanline=c->tia_scanline;
    g->tia_wsync=c->tia_wsync; g->tia_frame_complete=c->tia_frame_complete;
    g->tia_vsync=c->tia_vsync; g->tia_vblank=c->tia_vblank;
    g->cxm0p=c->cxm0p; g->cxm1p=c->cxm1p; g->cxp0fb=c->cxp0fb; g->cxp1fb=c->cxp1fb;
    g->cxm0fb=c->cxm0fb; g->cxm1fb=c->cxm1fb; g->cxblpf=c->cxblpf; g->cxppmm=c->cxppmm;
    g->inpt4=c->inpt4; g->inpt5=c->inpt5;
    g->paddle0=c->paddle0; g->paddle1=c->paddle1; g->paddle_counter=c->paddle_counter; g->paddle_dumped=c->paddle_dumped;
    g->pos_p0=c->pos_p0; g->pos_p1=c->pos_p1; g->pos_m0=c->pos_m0; g->pos_m1=c->pos_m1; g->pos_bl=c->pos_bl;
    g->hmp0=c->hmp0; g->hmp1=c->hmp1; g->hmm0=c->hmm0; g->hmm1=c->hmm1; g->hmbl=c->hmbl;
    g->grp0=c->grp0; g->grp1=c->grp1; g->grp0_old=c->grp0_old; g->grp1_old=c->grp1_old;
    g->enam0=c->enam0; g->enam1=c->enam1; g->enabl=c->enabl; g->enabl_old=c->enabl_old;
    g->vdelp0=c->vdelp0; g->vdelp1=c->vdelp1; g->vdelbl=c->vdelbl;
    g->resmp0=c->resmp0; g->resmp1=c->resmp1; g->nusiz0=c->nusiz0; g->nusiz1=c->nusiz1;
    g->colup0=c->colup0; g->colup1=c->colup1; g->colupf=c->colupf; g->colubk=c->colubk;
    g->ctrlpf=c->ctrlpf; g->refp0=c->refp0; g->refp1=c->refp1;
    g->pf0=c->pf0; g->pf1=c->pf1; g->pf2=c->pf2;
    g->bank=c->bank; g->scheme=c->scheme;
}

// ============================================================
// SoA global memory layout — coalesced access at frame boundaries.
// Each field is a contiguous array of N elements.
// Packed into a single allocation with computed offsets.
// ============================================================
struct AtariSoA {
    // Base pointer + N (number of instances)
    uint8_t* base;
    int N;

    // Field offsets (in bytes from base). Computed on host, passed to kernel.
    // u8 fields: 50 fields × N bytes each
    // u16 fields: 6 fields × N×2 bytes each
    // ram: N × 128 bytes
    // Total: 50N + 12N + 128N = 190N bytes

    // Accessors via compile-time field IDs would be cleanest,
    // but for simplicity we store the full AtariState AoS and add
    // a coalesced SoA load/store that transposes on the fly.
};

// SoA load: reads from AoS with __ldg() hints for read-only fields
__device__ __forceinline__
void load_ctx_ldg(ThreadCtx* c, const AtariState* __restrict__ g) {
    c->a=__ldg(&g->a); c->x=__ldg(&g->x); c->y=__ldg(&g->y); c->sp=__ldg(&g->sp);
    c->pc=__ldg(&g->pc); c->status=__ldg(&g->status);
    c->port_a_output=__ldg(&g->port_a_output); c->port_a_ddr=__ldg(&g->port_a_ddr);
    c->port_a_input=__ldg(&g->port_a_input);
    c->port_b_output=__ldg(&g->port_b_output); c->port_b_ddr=__ldg(&g->port_b_ddr);
    c->port_b_input=__ldg(&g->port_b_input);
    c->timer_value=__ldg(&g->timer_value); c->timer_prescaler=__ldg(&g->timer_prescaler);
    c->timer_prescaler_select=__ldg(&g->timer_prescaler_select);
    c->timer_underflow=__ldg(&g->timer_underflow);
    c->tia_clock=__ldg(&g->tia_clock); c->tia_scanline=__ldg(&g->tia_scanline);
    c->tia_wsync=__ldg(&g->tia_wsync); c->tia_frame_complete=__ldg(&g->tia_frame_complete);
    c->tia_vsync=__ldg(&g->tia_vsync); c->tia_vblank=__ldg(&g->tia_vblank);
    c->cxm0p=__ldg(&g->cxm0p); c->cxm1p=__ldg(&g->cxm1p);
    c->cxp0fb=__ldg(&g->cxp0fb); c->cxp1fb=__ldg(&g->cxp1fb);
    c->cxm0fb=__ldg(&g->cxm0fb); c->cxm1fb=__ldg(&g->cxm1fb);
    c->cxblpf=__ldg(&g->cxblpf); c->cxppmm=__ldg(&g->cxppmm);
    c->inpt4=__ldg(&g->inpt4); c->inpt5=__ldg(&g->inpt5);
    c->paddle0=__ldg(&g->paddle0); c->paddle1=__ldg(&g->paddle1);
    c->paddle_counter=__ldg(&g->paddle_counter); c->paddle_dumped=__ldg(&g->paddle_dumped);
    c->pos_p0=__ldg(&g->pos_p0); c->pos_p1=__ldg(&g->pos_p1);
    c->pos_m0=__ldg(&g->pos_m0); c->pos_m1=__ldg(&g->pos_m1); c->pos_bl=__ldg(&g->pos_bl);
    c->hmp0=__ldg(&g->hmp0); c->hmp1=__ldg(&g->hmp1);
    c->hmm0=__ldg(&g->hmm0); c->hmm1=__ldg(&g->hmm1); c->hmbl=__ldg(&g->hmbl);
    c->grp0=__ldg(&g->grp0); c->grp1=__ldg(&g->grp1);
    c->grp0_old=__ldg(&g->grp0_old); c->grp1_old=__ldg(&g->grp1_old);
    c->enam0=__ldg(&g->enam0); c->enam1=__ldg(&g->enam1);
    c->enabl=__ldg(&g->enabl); c->enabl_old=__ldg(&g->enabl_old);
    c->vdelp0=__ldg(&g->vdelp0); c->vdelp1=__ldg(&g->vdelp1); c->vdelbl=__ldg(&g->vdelbl);
    c->resmp0=__ldg(&g->resmp0); c->resmp1=__ldg(&g->resmp1);
    c->nusiz0=__ldg(&g->nusiz0); c->nusiz1=__ldg(&g->nusiz1);
    c->colup0=__ldg(&g->colup0); c->colup1=__ldg(&g->colup1);
    c->colupf=__ldg(&g->colupf); c->colubk=__ldg(&g->colubk);
    c->ctrlpf=__ldg(&g->ctrlpf);
    c->refp0=__ldg(&g->refp0); c->refp1=__ldg(&g->refp1);
    c->pf0=__ldg(&g->pf0); c->pf1=__ldg(&g->pf1); c->pf2=__ldg(&g->pf2);
    c->bank=__ldg(&g->bank); c->scheme=__ldg(&g->scheme);
}
