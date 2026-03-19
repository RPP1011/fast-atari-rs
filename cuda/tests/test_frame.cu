/// GPU vs CPU frame comparison test.
/// This is compiled and run separately from the Rust test harness.
/// The actual verification is done in Rust tests (tests/test_cuda.rs).
///
/// This file exists as a standalone CUDA compilation test.

#include "../src/atari_kernel.cu"

#include <cstdio>
#include <cstdlib>

int main() {
    printf("CUDA kernel compilation test passed.\n");
    printf("sizeof(AtariState) = %zu\n", sizeof(AtariState));

    // Verify struct layout expectations
    static_assert(sizeof(AtariState) < 512, "AtariState too large");

    printf("All static assertions passed.\n");
    return 0;
}
