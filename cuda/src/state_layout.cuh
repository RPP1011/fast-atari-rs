#pragma once
#include <cstdint>

// Phase 1: AoS layout — one struct per Atari instance.
// All state the CPU/TIA/PIA need to run a frame.

// Bank switching schemes, matching Rust BankScheme enum.
enum BankScheme : uint8_t {
    BANK_FIXED = 0,
    BANK_F8    = 1,
    BANK_F6    = 2,
    BANK_F4    = 3,
};

struct AtariState {
    // CPU registers (8 bytes)
    uint8_t  a;
    uint8_t  x;
    uint8_t  y;
    uint8_t  sp;
    uint16_t pc;
    uint8_t  status;  // NV-BDIZC packed
    uint8_t  _pad0;

    // PIA RAM (128 bytes)
    uint8_t ram[128];

    // PIA I/O + timer (15 bytes)
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

    // HeadlessTia (timing + registers)
    uint16_t tia_clock;
    uint16_t tia_scanline;
    uint8_t  tia_wsync;
    uint8_t  tia_frame_complete;
    uint8_t  tia_vsync;
    uint8_t  tia_vblank;

    // Collision latches
    uint8_t  cxm0p;
    uint8_t  cxm1p;
    uint8_t  cxp0fb;
    uint8_t  cxp1fb;
    uint8_t  cxm0fb;
    uint8_t  cxm1fb;
    uint8_t  cxblpf;
    uint8_t  cxppmm;

    // Input latches
    uint8_t  inpt4;   // 1 = true, 0 = false
    uint8_t  inpt5;

    // Paddle
    uint8_t  paddle0;
    uint8_t  paddle1;
    uint16_t paddle_counter;
    uint8_t  paddle_dumped;
    uint8_t  _pad4;

    // Object positions
    uint16_t pos_p0;
    uint16_t pos_p1;
    uint16_t pos_m0;
    uint16_t pos_m1;
    uint16_t pos_bl;
    uint8_t  hmp0;
    uint8_t  hmp1;
    uint8_t  hmm0;
    uint8_t  hmm1;
    uint8_t  hmbl;
    uint8_t  _pad5;

    // Graphics registers
    uint8_t  grp0;
    uint8_t  grp1;
    uint8_t  grp0_old;
    uint8_t  grp1_old;
    uint8_t  enam0;
    uint8_t  enam1;
    uint8_t  enabl;
    uint8_t  enabl_old;
    uint8_t  vdelp0;
    uint8_t  vdelp1;
    uint8_t  vdelbl;
    uint8_t  resmp0;
    uint8_t  resmp1;
    uint8_t  nusiz0;
    uint8_t  nusiz1;

    // Color/playfield registers
    uint8_t  colup0;
    uint8_t  colup1;
    uint8_t  colupf;
    uint8_t  colubk;
    uint8_t  ctrlpf;
    uint8_t  refp0;
    uint8_t  refp1;
    uint8_t  pf0;
    uint8_t  pf1;
    uint8_t  pf2;

    // Bankswitching
    uint8_t  bank;
    uint8_t  scheme;  // BankScheme enum

    uint8_t  _pad_end[2]; // align to 4 bytes
};

// ROM is stored in __constant__ memory (shared across all instances).
// Max 32KB for constant memory.
#define MAX_ROM_SIZE 32768

