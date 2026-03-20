use std::process::Command;
use std::path::Path;

fn main() {
    #[cfg(feature = "cuda")]
    {
        build_cuda();
    }
}

#[cfg(feature = "cuda")]
fn build_cuda() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let cuda_src = Path::new(&manifest_dir).join("cuda/src/atari_kernel.cu");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let ptx_path = Path::new(&out_dir).join("atari_kernel.ptx");

    // Compile CUDA kernel to PTX
    let status = Command::new("nvcc")
        .args(&[
            "--ptx",
            "-arch=sm_89",
            "-O3",
            "--use_fast_math",
            "-lineinfo",
            "-ccbin", "/usr/bin/g++-12",
            "-I", &Path::new(&manifest_dir).join("cuda/src").to_string_lossy(),
            "-o", &ptx_path.to_string_lossy(),
            &cuda_src.to_string_lossy(),
        ])
        .status()
        .expect("Failed to run nvcc. Is CUDA toolkit installed?");

    if !status.success() {
        panic!("nvcc compilation failed");
    }

    println!("cargo:rerun-if-changed=cuda/src/atari_kernel.cu");
    println!("cargo:rerun-if-changed=cuda/src/cpu_6502.cuh");
    println!("cargo:rerun-if-changed=cuda/src/memory_bus.cuh");
    println!("cargo:rerun-if-changed=cuda/src/tia_headless.cuh");
    println!("cargo:rerun-if-changed=cuda/src/pia.cuh");
    println!("cargo:rerun-if-changed=cuda/src/state_layout.cuh");
    println!("cargo:rerun-if-changed=cuda/src/opcode_sort.cuh");
    println!("cargo:rerun-if-changed=cuda/src/decoded_op.cuh");
    println!("cargo:rerun-if-changed=cuda/src/cpu_6502_aot.cuh");
    println!("cargo:rerun-if-changed=src/rom_compiler.rs");
}
