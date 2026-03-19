cmake_minimum_required(VERSION 3.18)
project(atari_cuda CUDA)

set(CMAKE_CUDA_STANDARD 17)
set(CMAKE_CUDA_ARCHITECTURES 89)

# Build PTX from the kernel source
add_library(atari_kernel OBJECT
    src/atari_kernel.cu
)

target_include_directories(atari_kernel PRIVATE src/)

# Generate PTX output
set_target_properties(atari_kernel PROPERTIES
    CUDA_PTX_COMPILATION ON
)

target_compile_options(atari_kernel PRIVATE
    $<$<COMPILE_LANGUAGE:CUDA>:
        -O3
        --use_fast_math
        -lineinfo
    >
)

# Custom target to copy PTX to a known location
add_custom_command(
    TARGET atari_kernel POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        $<TARGET_OBJECTS:atari_kernel>
        ${CMAKE_BINARY_DIR}/atari_kernel.ptx
    COMMENT "Copying PTX to build directory"
)
