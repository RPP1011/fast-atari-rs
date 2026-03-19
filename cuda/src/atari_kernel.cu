#include "cpu_6502.cuh"

// ============================================================
// Action dispatch — translate action byte to PIA/TIA inputs
// ============================================================
__device__ __forceinline__
void apply_action(AtariState* s, uint8_t action) {
    bool up = false, down = false, left = false, right = false, fire = false;
    switch (action) {
        case 0:  break;
        case 1:  fire = true; break;
        case 2:  up = true; break;
        case 3:  right = true; break;
        case 4:  left = true; break;
        case 5:  down = true; break;
        case 6:  up = true; right = true; break;
        case 7:  up = true; left = true; break;
        case 8:  down = true; right = true; break;
        case 9:  down = true; left = true; break;
        case 10: up = true; fire = true; break;
        case 11: right = true; fire = true; break;
        case 12: left = true; fire = true; break;
        case 13: down = true; fire = true; break;
        case 14: up = true; right = true; fire = true; break;
        case 15: up = true; left = true; fire = true; break;
        case 16: down = true; right = true; fire = true; break;
        case 17: down = true; left = true; fire = true; break;
    }

    uint8_t swcha = 0xFF;
    if (up)    swcha &= ~0x10;
    if (down)  swcha &= ~0x20;
    if (left)  swcha &= ~0x40;
    if (right) swcha &= ~0x80;
    s->port_a_input = swcha;

    s->inpt4 = fire ? 0 : 1;
    s->paddle0 = left ? 200 : (right ? 50 : 128);
}

// ============================================================
// Run one emulation cycle (WSYNC fast-forward or CPU step + tick)
// ============================================================
__device__ __forceinline__
void run_one_cycle(AtariState* s, uint64_t* cycles, const uint8_t* rom_ptr, uint32_t rom_len) {
    if (s->tia_wsync) {
        uint16_t skipped = tia_skip_to_scanline_end(s);
        pia_tick_n(s, skipped);
        *cycles += skipped;
    } else {
        uint8_t c = cpu_step(s, rom_ptr, rom_len);
        *cycles += c;
        tia_tick_n(s, c);
        pia_tick_n(s, (uint16_t)c);
    }
}

// ============================================================
// Run one frame (until VSYNC triggers)
// ============================================================
__device__
uint64_t run_frame(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    s->tia_frame_complete = 0;
    uint64_t cycles = 0;

    // Phase 1: If currently in VSYNC, run until it ends
    while (s->tia_vsync & 0x02) {
        run_one_cycle(s, &cycles, rom_ptr, rom_len);
    }

    // Phase 2: Run until next VSYNC starts
    while (!(s->tia_vsync & 0x02)) {
        run_one_cycle(s, &cycles, rom_ptr, rom_len);
        if (cycles > 100000) break;
    }

    return cycles;
}

// ============================================================
// Main kernel: one thread per Atari instance
// ============================================================
extern "C"
__global__
void atari_frame_kernel(
    AtariState* __restrict__ states,
    const uint8_t* __restrict__ actions,
    uint8_t* __restrict__ obs_out,
    const uint8_t* __restrict__ rom_ptr,
    uint32_t rom_len,
    int N
) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= N) return;

    AtariState* s = &states[idx];

    // Apply action
    apply_action(s, actions[idx]);

    // Run one frame
    run_frame(s, rom_ptr, rom_len);

    // Copy PIA RAM to observation buffer
    uint8_t* obs = &obs_out[idx * 128];
    for (int i = 0; i < 128; i++) {
        obs[i] = s->ram[i];
    }
}
