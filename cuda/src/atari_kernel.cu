#include "cpu_6502.cuh"
#include "opcode_sort.cuh"

// NOTE: Opcode sorting (Phase 4) adds warp shuffle overhead per instruction.
// It helps on divergence-heavy games (no WSYNC) but hurts on WSYNC-heavy games
// like Breakout where most cycles are in the fast-forward path.
// Use set_sorted(true) selectively based on ROM characteristics.

// ============================================================
// Action dispatch
// ============================================================
__device__ __forceinline__
void apply_action(ThreadCtx* c, uint8_t action) {
    bool up=false, dn=false, lt=false, rt=false, fi=false;
    switch (action) {
        case 0: break; case 1: fi=true; break;
        case 2: up=true; break; case 3: rt=true; break;
        case 4: lt=true; break; case 5: dn=true; break;
        case 6: up=true;rt=true; break; case 7: up=true;lt=true; break;
        case 8: dn=true;rt=true; break; case 9: dn=true;lt=true; break;
        case 10: up=true;fi=true; break; case 11: rt=true;fi=true; break;
        case 12: lt=true;fi=true; break; case 13: dn=true;fi=true; break;
        case 14: up=true;rt=true;fi=true; break; case 15: up=true;lt=true;fi=true; break;
        case 16: dn=true;rt=true;fi=true; break; case 17: dn=true;lt=true;fi=true; break;
    }
    uint8_t swcha = 0xFF;
    if (up) swcha &= ~0x10; if (dn) swcha &= ~0x20;
    if (lt) swcha &= ~0x40; if (rt) swcha &= ~0x80;
    c->port_a_input = swcha;
    c->inpt4 = fi ? 0 : 1;
    c->paddle0 = lt ? 200 : (rt ? 50 : 128);
}

// ============================================================
// Run one cycle
// ============================================================
// Unsorted version (Phase 2 compatibility)
__device__ __forceinline__
void run_one_cycle(ThreadCtx* c, uint8_t* mr, uint64_t* cycles,
                   const uint8_t* rp, uint32_t rl) {
    if (c->tia_wsync) {
        uint16_t skipped = tia_skip_to_scanline_end(c);
        pia_tick_n(c, skipped);
        *cycles += skipped;
    } else {
        uint8_t cy = cpu_step(c, mr, rp, rl);
        *cycles += cy;
        tia_tick_n(c, cy);
        pia_tick_n(c, (uint16_t)cy);
    }
}

// Sorted version (Phase 4 — warp opcode compaction)
__device__ __forceinline__
void run_one_cycle_sorted(ThreadCtx* c, uint8_t* mr, uint64_t* cycles,
                          const uint8_t* rp, uint32_t rl) {
    if (c->tia_wsync) {
        uint16_t skipped = tia_skip_to_scanline_end(c);
        pia_tick_n(c, skipped);
        *cycles += skipped;
    } else {
        uint8_t cy = cpu_step_sorted(c, mr, rp, rl);
        *cycles += cy;
        tia_tick_n(c, cy);
        pia_tick_n(c, (uint16_t)cy);
    }
}

// ============================================================
// Run one frame
// ============================================================
__device__
void run_frame(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl) {
    c->tia_frame_complete = 0;
    uint64_t cycles = 0;

    while (c->tia_vsync & 0x02)
        run_one_cycle(c, mr, &cycles, rp, rl);

    while (!(c->tia_vsync & 0x02)) {
        run_one_cycle(c, mr, &cycles, rp, rl);
        if (cycles > 100000) break;
    }
}

// Sorted version (Phase 4)
__device__
void run_frame_sorted(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl) {
    c->tia_frame_complete = 0;
    uint64_t cycles = 0;

    while (c->tia_vsync & 0x02)
        run_one_cycle_sorted(c, mr, &cycles, rp, rl);

    while (!(c->tia_vsync & 0x02)) {
        run_one_cycle_sorted(c, mr, &cycles, rp, rl);
        if (cycles > 100000) break;
    }
}

// ============================================================
// Per-frame kernel (Phase 2 compatibility — still used for tests)
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
    __shared__ uint8_t sram[BLOCK_SIZE][128];

    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= N) return;

    uint8_t* my_ram = sram[threadIdx.x];

    ThreadCtx ctx;
    load_ctx(&ctx, &states[idx]);

    const uint8_t* gram = states[idx].ram;
    for (int i = 0; i < 128; i++) my_ram[i] = gram[i];

    apply_action(&ctx, actions[idx]);
    run_frame(&ctx, my_ram, rom_ptr, rom_len);

    store_ctx(&states[idx], &ctx);
    uint8_t* gram_out = states[idx].ram;
    uint8_t* obs = &obs_out[idx * 128];
    for (int i = 0; i < 128; i++) {
        uint8_t v = my_ram[i];
        gram_out[i] = v;
        obs[i] = v;
    }
}

// ============================================================
// Multi-frame kernel (Phase 3)
//
// Runs K frames in a single kernel launch. State stays in registers
// and shared memory across all K frames — no load/store between frames.
// Actions: actions[frame * N + idx] for each frame.
// Obs: only the LAST frame's obs is written to obs_out[idx * 128].
// ============================================================
extern "C"
__global__
void atari_multi_frame_kernel(
    AtariState* __restrict__ states,
    const uint8_t* __restrict__ actions,  // K * N actions
    uint8_t* __restrict__ obs_out,        // N * 128 obs (last frame only)
    const uint8_t* __restrict__ rom_ptr,
    uint32_t rom_len,
    int K,  // number of frames
    int N
) {
    __shared__ uint8_t sram[BLOCK_SIZE][128];

    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= N) return;

    uint8_t* my_ram = sram[threadIdx.x];

    // One-time load from global → registers + shared
    ThreadCtx ctx;
    load_ctx(&ctx, &states[idx]);
    const uint8_t* gram = states[idx].ram;
    for (int i = 0; i < 128; i++) my_ram[i] = gram[i];

    // Run K frames
    for (int f = 0; f < K; f++) {
        apply_action(&ctx, actions[f * N + idx]);
        run_frame(&ctx, my_ram, rom_ptr, rom_len);
    }

    // Store state back to global
    store_ctx(&states[idx], &ctx);

    // Store RAM → global + obs
    uint8_t* gram_out = states[idx].ram;
    uint8_t* obs = &obs_out[idx * 128];
    for (int i = 0; i < 128; i++) {
        uint8_t v = my_ram[i];
        gram_out[i] = v;
        obs[i] = v;
    }
}

// ============================================================
// Phase 4: Sorted variants (warp opcode compaction)
// ============================================================
extern "C"
__global__
void atari_frame_kernel_sorted(
    AtariState* __restrict__ states,
    const uint8_t* __restrict__ actions,
    uint8_t* __restrict__ obs_out,
    const uint8_t* __restrict__ rom_ptr,
    uint32_t rom_len,
    int N
) {
    __shared__ uint8_t sram[BLOCK_SIZE][128];
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= N) return;
    uint8_t* my_ram = sram[threadIdx.x];
    ThreadCtx ctx;
    load_ctx(&ctx, &states[idx]);
    for (int i = 0; i < 128; i++) my_ram[i] = states[idx].ram[i];
    apply_action(&ctx, actions[idx]);
    run_frame_sorted(&ctx, my_ram, rom_ptr, rom_len);
    store_ctx(&states[idx], &ctx);
    uint8_t* obs = &obs_out[idx * 128];
    for (int i = 0; i < 128; i++) {
        uint8_t v = my_ram[i];
        states[idx].ram[i] = v;
        obs[i] = v;
    }
}

extern "C"
__global__
void atari_multi_frame_kernel_sorted(
    AtariState* __restrict__ states,
    const uint8_t* __restrict__ actions,
    uint8_t* __restrict__ obs_out,
    const uint8_t* __restrict__ rom_ptr,
    uint32_t rom_len,
    int K,
    int N
) {
    __shared__ uint8_t sram[BLOCK_SIZE][128];
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= N) return;
    uint8_t* my_ram = sram[threadIdx.x];
    ThreadCtx ctx;
    load_ctx(&ctx, &states[idx]);
    for (int i = 0; i < 128; i++) my_ram[i] = states[idx].ram[i];
    for (int f = 0; f < K; f++) {
        apply_action(&ctx, actions[f * N + idx]);
        run_frame_sorted(&ctx, my_ram, rom_ptr, rom_len);
    }
    store_ctx(&states[idx], &ctx);
    uint8_t* obs = &obs_out[idx * 128];
    for (int i = 0; i < 128; i++) {
        uint8_t v = my_ram[i];
        states[idx].ram[i] = v;
        obs[i] = v;
    }
}
