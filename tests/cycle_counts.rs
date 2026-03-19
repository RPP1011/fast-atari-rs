use stella_rs::cpu::{Cpu, FlatMemory};

/// Helper: load code at $8000, set reset vector, run one instruction, return cycles.
fn run_one(code: &[u8]) -> (Cpu, u8) {
    let mut mem = FlatMemory::new();
    mem.load(0x8000, code);
    // Reset vector → $8000
    mem.ram[0xFFFC] = 0x00;
    mem.ram[0xFFFD] = 0x80;

    let mut cpu = Cpu::new();
    cpu.reset(&mut mem);
    assert_eq!(cpu.pc, 0x8000);

    let cycles = cpu.step(&mut mem);
    (cpu, cycles)
}

/// Helper: load code, run N instructions, return cpu and total cycles.
fn run_n(code: &[u8], n: usize) -> (Cpu, FlatMemory, u8) {
    let mut mem = FlatMemory::new();
    mem.load(0x8000, code);
    mem.ram[0xFFFC] = 0x00;
    mem.ram[0xFFFD] = 0x80;

    let mut cpu = Cpu::new();
    cpu.reset(&mut mem);

    let mut total = 0u8;
    for _ in 0..n {
        total += cpu.step(&mut mem);
    }
    (cpu, mem, total)
}

// --- Implied / Accumulator ---

#[test]
fn nop_cycles() {
    let (_, cycles) = run_one(&[0xEA]); // NOP
    assert_eq!(cycles, 2);
}

#[test]
fn tax_cycles() {
    let (_, cycles) = run_one(&[0xAA]); // TAX
    assert_eq!(cycles, 2);
}

#[test]
fn inx_cycles() {
    let (_, cycles) = run_one(&[0xE8]); // INX
    assert_eq!(cycles, 2);
}

#[test]
fn asl_accumulator_cycles() {
    let (_, cycles) = run_one(&[0x0A]); // ASL A
    assert_eq!(cycles, 2);
}

// --- Immediate ---

#[test]
fn lda_immediate_cycles() {
    let (cpu, cycles) = run_one(&[0xA9, 0x42]); // LDA #$42
    assert_eq!(cycles, 2);
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn adc_immediate_cycles() {
    let (_, cycles) = run_one(&[0x69, 0x10]); // ADC #$10
    assert_eq!(cycles, 2);
}

// --- Zero Page ---

#[test]
fn lda_zero_page_cycles() {
    let mut mem = FlatMemory::new();
    mem.ram[0x42] = 0xBE;
    mem.load(0x8000, &[0xA5, 0x42]); // LDA $42
    mem.ram[0xFFFC] = 0x00;
    mem.ram[0xFFFD] = 0x80;
    let mut cpu = Cpu::new();
    cpu.reset(&mut mem);
    let cycles = cpu.step(&mut mem);
    assert_eq!(cycles, 3);
    assert_eq!(cpu.a, 0xBE);
}

#[test]
fn inc_zero_page_cycles() {
    let (_, _, cycles) = run_n(&[0xE6, 0x50], 1); // INC $50
    assert_eq!(cycles, 5);
}

// --- Zero Page,X ---

#[test]
fn lda_zero_page_x_cycles() {
    let (_, cycles) = run_one(&[0xB5, 0x10]); // LDA $10,X
    assert_eq!(cycles, 4);
}

// --- Absolute ---

#[test]
fn lda_absolute_cycles() {
    let (_, cycles) = run_one(&[0xAD, 0x00, 0x90]); // LDA $9000
    assert_eq!(cycles, 4);
}

#[test]
fn jmp_absolute_cycles() {
    let mut mem = FlatMemory::new();
    mem.load(0x8000, &[0x4C, 0x00, 0x90]); // JMP $9000
    mem.ram[0xFFFC] = 0x00;
    mem.ram[0xFFFD] = 0x80;
    let mut cpu = Cpu::new();
    cpu.reset(&mut mem);
    let cycles = cpu.step(&mut mem);
    assert_eq!(cycles, 3);
    assert_eq!(cpu.pc, 0x9000);
}

#[test]
fn jsr_rts_cycles() {
    let mut mem = FlatMemory::new();
    // $8000: JSR $8010
    // $8010: RTS
    mem.load(0x8000, &[0x20, 0x10, 0x80]);
    mem.ram[0x8010] = 0x60; // RTS
    mem.ram[0xFFFC] = 0x00;
    mem.ram[0xFFFD] = 0x80;
    let mut cpu = Cpu::new();
    cpu.reset(&mut mem);

    let c1 = cpu.step(&mut mem); // JSR
    assert_eq!(c1, 6);
    assert_eq!(cpu.pc, 0x8010);

    let c2 = cpu.step(&mut mem); // RTS
    assert_eq!(c2, 6);
    assert_eq!(cpu.pc, 0x8003); // JSR was 3 bytes, RTS returns to addr+1
}

#[test]
fn inc_absolute_cycles() {
    let (_, _, cycles) = run_n(&[0xEE, 0x00, 0x90], 1); // INC $9000
    assert_eq!(cycles, 6);
}

// --- Absolute,X (no page cross) ---

#[test]
fn lda_absolute_x_no_cross_cycles() {
    // LDX #$01; LDA $9000,X — no page cross
    let (_, _, cycles) = run_n(&[0xA2, 0x01, 0xBD, 0x00, 0x90], 2);
    // LDX #imm = 2, LDA abs,X = 4 (no page cross)
    assert_eq!(cycles, 2 + 4);
}

// --- Absolute,X (with page cross) ---

#[test]
fn lda_absolute_x_page_cross_cycles() {
    // LDX #$FF; LDA $90FF,X — crosses from page $90 to $91
    let (_, _, cycles) = run_n(&[0xA2, 0xFF, 0xBD, 0xFF, 0x90], 2);
    // LDX #imm = 2, LDA abs,X = 4+1 (page cross)
    assert_eq!(cycles, 2 + 5);
}

// --- Absolute,Y (no page cross) ---

#[test]
fn lda_absolute_y_no_cross_cycles() {
    // LDY #$01; LDA $9000,Y
    let (_, _, cycles) = run_n(&[0xA0, 0x01, 0xB9, 0x00, 0x90], 2);
    assert_eq!(cycles, 2 + 4);
}

// --- Absolute,Y (with page cross) ---

#[test]
fn lda_absolute_y_page_cross_cycles() {
    // LDY #$FF; LDA $90FF,Y
    let (_, _, cycles) = run_n(&[0xA0, 0xFF, 0xB9, 0xFF, 0x90], 2);
    assert_eq!(cycles, 2 + 5);
}

// --- STA Absolute,X always takes 5 cycles (no +1 distinction) ---

#[test]
fn sta_absolute_x_always_5_cycles() {
    // LDX #$FF; STA $90FF,X — store always takes 5, even with page cross
    let (_, _, cycles) = run_n(&[0xA2, 0xFF, 0x9D, 0xFF, 0x90], 2);
    assert_eq!(cycles, 2 + 5);
}

// --- Indirect,Y (no page cross) ---

#[test]
fn lda_indirect_y_no_cross_cycles() {
    let mut mem = FlatMemory::new();
    // ZP $20 points to $9000
    mem.ram[0x20] = 0x00;
    mem.ram[0x21] = 0x90;
    // LDY #$01; LDA ($20),Y
    mem.load(0x8000, &[0xA0, 0x01, 0xB1, 0x20]);
    mem.ram[0xFFFC] = 0x00;
    mem.ram[0xFFFD] = 0x80;
    let mut cpu = Cpu::new();
    cpu.reset(&mut mem);
    let c1 = cpu.step(&mut mem); // LDY
    let c2 = cpu.step(&mut mem); // LDA ($20),Y
    assert_eq!(c1, 2);
    assert_eq!(c2, 5); // no page cross
}

#[test]
fn lda_indirect_y_page_cross_cycles() {
    let mut mem = FlatMemory::new();
    // ZP $20 points to $90FF
    mem.ram[0x20] = 0xFF;
    mem.ram[0x21] = 0x90;
    // LDY #$01; LDA ($20),Y — target is $9100, page cross
    mem.load(0x8000, &[0xA0, 0x01, 0xB1, 0x20]);
    mem.ram[0xFFFC] = 0x00;
    mem.ram[0xFFFD] = 0x80;
    let mut cpu = Cpu::new();
    cpu.reset(&mut mem);
    let c1 = cpu.step(&mut mem);
    let c2 = cpu.step(&mut mem);
    assert_eq!(c1, 2);
    assert_eq!(c2, 6); // +1 for page cross
}

// --- Indirect,X (always 6, no page cross penalty) ---

#[test]
fn lda_indirect_x_cycles() {
    let mut mem = FlatMemory::new();
    mem.ram[0x21] = 0x00;
    mem.ram[0x22] = 0x90;
    // LDX #$01; LDA ($20,X)
    mem.load(0x8000, &[0xA2, 0x01, 0xA1, 0x20]);
    mem.ram[0xFFFC] = 0x00;
    mem.ram[0xFFFD] = 0x80;
    let mut cpu = Cpu::new();
    cpu.reset(&mut mem);
    cpu.step(&mut mem); // LDX
    let c = cpu.step(&mut mem); // LDA ($20,X)
    assert_eq!(c, 6);
}

// --- Branch: not taken ---

#[test]
fn branch_not_taken_cycles() {
    // CLC; BCS +$10 — carry is clear so BCS not taken
    let (_, _, cycles) = run_n(&[0x18, 0xB0, 0x10], 2);
    assert_eq!(cycles, 2 + 2); // CLC=2, BCS not taken=2
}

// --- Branch: taken, no page cross ---

#[test]
fn branch_taken_no_cross_cycles() {
    // SEC; BCS +$02 — carry is set, branch forward 2 bytes (same page)
    let (cpu, _, cycles) = run_n(&[0x38, 0xB0, 0x02], 2);
    assert_eq!(cycles, 2 + 3); // SEC=2, BCS taken same page=3
    assert_eq!(cpu.pc, 0x8005); // $8002 (after SEC) + 2 (branch size) + 2 (offset)
}

// --- Branch: taken, page cross ---

#[test]
fn branch_taken_page_cross_cycles() {
    let mut mem = FlatMemory::new();
    // Place SEC at $80FD, BCS +$05 at $80FE
    // After BCS: next_pc = $8100, target = $8105 — same page actually
    // We need: next_pc on one page, target on another.
    // Place SEC at $80FD, BCS +$7F at $80FE (offset = 127)
    // next_pc = $8100, target = $8100 + 127 = $817F — different page!
    // Actually next_pc for the branch = $80FE + 2 = $8100
    // target = $8100 + 0x7F = $817F — but $8100 and $817F are both page $81.
    //
    // Better: SEC at $80FD, BCS -$05 at $80FE
    // next_pc = $8100, target = $8100 + (-5) = $80FB — page $80 vs $81!
    mem.ram[0x80FD] = 0x38; // SEC
    mem.ram[0x80FE] = 0xB0; // BCS
    mem.ram[0x80FF] = 0xFB_u8; // offset = -5 (signed)
    mem.ram[0xFFFC] = 0xFD;
    mem.ram[0xFFFD] = 0x80;

    let mut cpu = Cpu::new();
    cpu.reset(&mut mem);

    let c1 = cpu.step(&mut mem); // SEC
    assert_eq!(c1, 2);
    assert_eq!(cpu.pc, 0x80FE);

    let c2 = cpu.step(&mut mem); // BCS -5
    assert_eq!(c2, 4); // taken + page cross
    assert_eq!(cpu.pc, 0x80FB);
}

// --- Stack ---

#[test]
fn pha_pla_cycles() {
    // LDA #$42; PHA; LDA #$00; PLA
    let (cpu, _, cycles) = run_n(&[0xA9, 0x42, 0x48, 0xA9, 0x00, 0x68], 4);
    assert_eq!(cycles, 2 + 3 + 2 + 4); // LDA=2, PHA=3, LDA=2, PLA=4
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn php_plp_cycles() {
    let (_, _, cycles) = run_n(&[0x08, 0x28], 2); // PHP; PLP
    assert_eq!(cycles, 3 + 4);
}

// --- ASL/LSR/ROL/ROR absolute: always 6 cycles ---

#[test]
fn asl_absolute_cycles() {
    let (_, _, cycles) = run_n(&[0x0E, 0x00, 0x90], 1);
    assert_eq!(cycles, 6);
}

// --- ASL/LSR absolute,X: always 7 cycles (no +1 for page cross) ---

#[test]
fn asl_absolute_x_cycles() {
    // LDX #$FF; ASL $90FF,X — RMW always 7 regardless of page cross
    let (_, _, cycles) = run_n(&[0xA2, 0xFF, 0x1E, 0xFF, 0x90], 2);
    assert_eq!(cycles, 2 + 7);
}

// --- BRK/RTI ---

#[test]
fn brk_cycles() {
    let mut mem = FlatMemory::new();
    mem.load(0x8000, &[0x00]); // BRK
    // IRQ vector → $9000
    mem.ram[0xFFFE] = 0x00;
    mem.ram[0xFFFF] = 0x90;
    mem.ram[0x9000] = 0x40; // RTI at handler
    mem.ram[0xFFFC] = 0x00;
    mem.ram[0xFFFD] = 0x80;

    let mut cpu = Cpu::new();
    cpu.reset(&mut mem);

    let c1 = cpu.step(&mut mem); // BRK
    assert_eq!(c1, 7);
    assert_eq!(cpu.pc, 0x9000);

    let c2 = cpu.step(&mut mem); // RTI
    assert_eq!(c2, 6);
    assert_eq!(cpu.pc, 0x8002); // BRK pushes PC+2
}
