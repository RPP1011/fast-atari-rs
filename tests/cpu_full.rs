//! Klaus Dormann's 6502 functional test suite.
//!
//! Downloads the test binary from the Klaus Dormann repository and runs it
//! against our CPU implementation. The test ROM is a self-contained 6502
//! program that exercises every documented instruction and addressing mode.
//!
//! Success: PC reaches $3469 (the success trap).
//! Failure: PC gets stuck in a loop (the same address for consecutive steps).
//!
//! Run with:  cargo test -- cpu_full

use stella_rs::cpu::{Cpu, FlatMemory};

const DORMANN_BIN_URL: &str =
    "https://raw.githubusercontent.com/Klaus2m5/6502_65C02_functional_tests/master/bin_files/6502_functional_test.bin";

/// The address the test ROM is assembled to start at.
const LOAD_ADDR: u16 = 0x0000;

/// Entry point of the functional test (start of code after vectors).
const START_ADDR: u16 = 0x0400;

/// The PC value that signals all tests passed.
const SUCCESS_ADDR: u16 = 0x3469;

/// Maximum instructions to execute before we declare a hang.
const MAX_STEPS: u64 = 100_000_000;

fn fetch_test_rom() -> Vec<u8> {
    // Try to use a cached copy first.
    let cache_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("roms")
        .join("6502_functional_test.bin");

    if cache_path.exists() {
        return std::fs::read(&cache_path).expect("failed to read cached ROM");
    }

    // Download at test time so the repo stays small.
    let output = std::process::Command::new("curl")
        .args(["-sL", DORMANN_BIN_URL, "-o"])
        .arg(&cache_path)
        .output()
        .expect("failed to run curl");

    assert!(
        output.status.success(),
        "curl failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::read(&cache_path).expect("failed to read downloaded ROM")
}

#[test]
fn cpu_full() {
    let rom = fetch_test_rom();

    let mut mem = FlatMemory::new();
    mem.load(LOAD_ADDR, &rom);

    let mut cpu = Cpu::new();
    cpu.pc = START_ADDR;

    let mut prev_pc = cpu.pc;
    let mut same_pc_count: u32 = 0;

    for _ in 0..MAX_STEPS {
        if cpu.pc == SUCCESS_ADDR {
            println!("All Klaus Dormann 6502 tests passed at PC=${:04X}", cpu.pc);
            return;
        }

        cpu.step(&mut mem);

        if cpu.pc == prev_pc {
            same_pc_count += 1;
            if same_pc_count > 2 {
                panic!(
                    "CPU trapped at PC=${:04X} — test failure (A=${:02X} X=${:02X} Y=${:02X})",
                    cpu.pc, cpu.a, cpu.x, cpu.y
                );
            }
        } else {
            same_pc_count = 0;
        }
        prev_pc = cpu.pc;
    }

    panic!(
        "Test did not complete within {} instructions (stuck at PC=${:04X})",
        MAX_STEPS, cpu.pc
    );
}
