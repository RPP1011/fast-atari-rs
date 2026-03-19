#pragma once
#include "cpu_6502.cuh"

// ============================================================
// Warp-level opcode compaction
//
// Before executing cpu_step, we group threads in the warp by their
// current opcode. Each iteration of the loop picks one unique opcode
// (from the "leader" thread), and all threads with that opcode execute
// cpu_step together — fully converged, no switch divergence.
//
// A warp typically has 8-12 unique opcodes per cycle. Without sorting,
// the hardware serializes all divergent switch paths. With sorting,
// each path runs with all matching threads active.
// ============================================================

__device__ __forceinline__
uint8_t cpu_step_sorted(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl) {
    // Pre-read the opcode this thread will execute
    uint8_t my_op = bus_read(c, mr, c->pc, rp, rl);
    uint8_t my_cycles = 0;

    uint32_t remaining = __activemask();
    while (remaining) {
        // Pick the first active thread as leader
        int leader = __ffs(remaining) - 1;
        // Broadcast the leader's opcode to all active threads
        uint8_t target_op = __shfl_sync(remaining, (unsigned)my_op, leader);
        // Find all threads with the same opcode
        uint32_t match = __ballot_sync(remaining, my_op == target_op);
        // Execute: only matching threads enter cpu_step (fully converged)
        if (my_op == target_op) {
            my_cycles = cpu_step(c, mr, rp, rl);
        }
        // Remove processed threads
        remaining &= ~match;
    }
    return my_cycles;
}
