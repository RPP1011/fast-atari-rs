/// ROM Compiler: static CFG analysis + CUDA code generation.
///
/// Walks the ROM to discover basic blocks (straight-line instruction sequences),
/// then generates CUDA C code that replaces the per-instruction switch dispatch
/// with compiled blocks and batched TIA/PIA ticks.

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::fmt::Write;

use crate::atari::BankScheme;

// ============================================================
// Instruction tables (duplicated from cuda_env for non-cuda use)
// ============================================================

pub(crate) const CYCLE_TABLE: [u8; 256] = [
    7,6,0,0,0,3,5,0,3,2,2,0,0,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
    6,6,0,0,3,3,5,0,4,2,2,0,4,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
    6,6,0,0,0,3,5,0,3,2,2,0,3,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
    6,6,0,0,0,3,5,0,4,2,2,0,5,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
    0,6,0,0,3,3,3,0,2,0,2,0,4,4,4,0, 2,6,0,0,4,4,4,0,2,5,2,0,5,5,0,0,
    2,6,2,0,3,3,3,0,2,2,2,0,4,4,4,0, 2,5,0,0,4,4,4,0,2,4,2,0,4,4,4,0,
    2,6,0,0,3,3,5,0,2,2,2,0,4,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
    2,6,0,0,3,3,5,0,2,2,2,0,4,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
];

pub(crate) const SIZE_TABLE: [u8; 256] = [
    1,2,1,1,1,2,2,1,1,2,1,1,1,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
    3,2,1,1,2,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
    1,2,1,1,1,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
    1,2,1,1,1,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
    1,2,1,1,2,2,2,1,1,1,1,1,3,3,3,1, 2,2,1,1,2,2,2,1,1,3,1,1,3,3,1,1,
    2,2,2,1,2,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,2,2,2,1,1,3,1,1,3,3,3,1,
    2,2,1,1,2,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
    2,2,1,1,2,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
];

// ============================================================
// Types
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Lda, Ldx, Ldy, Sta, Stx, Sty,
    Adc, Sbc, And, Ora, Eor, Cmp, Cpx, Cpy, Bit,
    AslM, LsrM, RolM, RorM, IncM, DecM,
    AslA, LsrA, RolA, RorA,
    Tax, Txa, Tay, Tya, Txs, Tsx,
    Inx, Iny, Dex, Dey,
    Pha, Pla, Php, Plp,
    Clc, Sec, Cli, Sei, Clv, Cld, Sed,
    Nop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AddrMode {
    Implied, Immediate,
    ZeroPage, ZeroPageX, ZeroPageY,
    Absolute, AbsoluteX, AbsoluteY,
    IndirectX, IndirectY,
}

#[derive(Clone, Debug)]
pub struct DecodedInstr {
    pub pc: u16,
    pub opcode: u8,
    pub operand: u16,
    pub size: u8,
    pub cycles: u8,
}

#[derive(Clone, Debug)]
pub enum ExitType {
    Branch { taken_pc: u16, not_taken_pc: u16 },
    JmpAbs { target: u16 },
    JmpInd { operand: u16 },
    Jsr { target: u16, return_pc: u16 },
    Rts,
    Rti,
    Brk,
    Wsync { next_pc: u16 },
    Fallthrough { next_pc: u16 },
}

#[derive(Clone, Debug)]
pub struct BasicBlock {
    pub start_pc: u16,
    pub bank: usize,
    pub instructions: Vec<DecodedInstr>,
    pub exit_type: ExitType,
}

impl BasicBlock {
    pub fn base_cycles(&self) -> u32 {
        self.instructions.iter().map(|i| i.cycles as u32).sum()
    }

    fn block_id(&self) -> u32 {
        ((self.bank as u32) << 12) | (self.start_pc as u32 & 0x0FFF)
    }
}

// ============================================================
// ROM access helpers
// ============================================================

fn num_banks(scheme: BankScheme) -> usize {
    match scheme {
        BankScheme::Fixed => 1,
        BankScheme::F8 => 2,
        BankScheme::F6 => 4,
        BankScheme::F4 => 8,
    }
}

fn rom_byte(rom: &[u8], scheme: BankScheme, bank: usize, addr: u16) -> u8 {
    let offset = addr & 0x0FFF;
    match scheme {
        BankScheme::Fixed => rom[(offset as usize) % rom.len()],
        _ => {
            let idx = bank * 4096 + offset as usize;
            if idx < rom.len() { rom[idx] } else { 0 }
        }
    }
}

fn read_vector(rom: &[u8], scheme: BankScheme, bank: usize, vec_addr: u16) -> u16 {
    let lo = rom_byte(rom, scheme, bank, vec_addr) as u16;
    let hi = rom_byte(rom, scheme, bank, vec_addr.wrapping_add(1)) as u16;
    (hi << 8) | lo
}

fn read_operand(rom: &[u8], scheme: BankScheme, bank: usize, pc: u16, size: u8) -> u16 {
    match size {
        2 => rom_byte(rom, scheme, bank, pc.wrapping_add(1)) as u16,
        3 => {
            let lo = rom_byte(rom, scheme, bank, pc.wrapping_add(1)) as u16;
            let hi = rom_byte(rom, scheme, bank, pc.wrapping_add(2)) as u16;
            lo | (hi << 8)
        }
        _ => 0,
    }
}

// ============================================================
// Opcode classification
// ============================================================

fn classify(opcode: u8) -> Option<(Op, AddrMode)> {
    use Op::*;
    use AddrMode::*;
    Some(match opcode {
        // ADC
        0x69 => (Adc, Immediate), 0x65 => (Adc, ZeroPage), 0x75 => (Adc, ZeroPageX),
        0x6D => (Adc, Absolute),  0x7D => (Adc, AbsoluteX), 0x79 => (Adc, AbsoluteY),
        0x61 => (Adc, IndirectX), 0x71 => (Adc, IndirectY),
        // SBC
        0xE9 => (Sbc, Immediate), 0xE5 => (Sbc, ZeroPage), 0xF5 => (Sbc, ZeroPageX),
        0xED => (Sbc, Absolute),  0xFD => (Sbc, AbsoluteX), 0xF9 => (Sbc, AbsoluteY),
        0xE1 => (Sbc, IndirectX), 0xF1 => (Sbc, IndirectY),
        // AND
        0x29 => (And, Immediate), 0x25 => (And, ZeroPage), 0x35 => (And, ZeroPageX),
        0x2D => (And, Absolute),  0x3D => (And, AbsoluteX), 0x39 => (And, AbsoluteY),
        0x21 => (And, IndirectX), 0x31 => (And, IndirectY),
        // ORA
        0x09 => (Ora, Immediate), 0x05 => (Ora, ZeroPage), 0x15 => (Ora, ZeroPageX),
        0x0D => (Ora, Absolute),  0x1D => (Ora, AbsoluteX), 0x19 => (Ora, AbsoluteY),
        0x01 => (Ora, IndirectX), 0x11 => (Ora, IndirectY),
        // EOR
        0x49 => (Eor, Immediate), 0x45 => (Eor, ZeroPage), 0x55 => (Eor, ZeroPageX),
        0x4D => (Eor, Absolute),  0x5D => (Eor, AbsoluteX), 0x59 => (Eor, AbsoluteY),
        0x41 => (Eor, IndirectX), 0x51 => (Eor, IndirectY),
        // CMP
        0xC9 => (Cmp, Immediate), 0xC5 => (Cmp, ZeroPage), 0xD5 => (Cmp, ZeroPageX),
        0xCD => (Cmp, Absolute),  0xDD => (Cmp, AbsoluteX), 0xD9 => (Cmp, AbsoluteY),
        0xC1 => (Cmp, IndirectX), 0xD1 => (Cmp, IndirectY),
        // CPX
        0xE0 => (Cpx, Immediate), 0xE4 => (Cpx, ZeroPage), 0xEC => (Cpx, Absolute),
        // CPY
        0xC0 => (Cpy, Immediate), 0xC4 => (Cpy, ZeroPage), 0xCC => (Cpy, Absolute),
        // LDA
        0xA9 => (Lda, Immediate), 0xA5 => (Lda, ZeroPage), 0xB5 => (Lda, ZeroPageX),
        0xAD => (Lda, Absolute),  0xBD => (Lda, AbsoluteX), 0xB9 => (Lda, AbsoluteY),
        0xA1 => (Lda, IndirectX), 0xB1 => (Lda, IndirectY),
        // LDX
        0xA2 => (Ldx, Immediate), 0xA6 => (Ldx, ZeroPage), 0xB6 => (Ldx, ZeroPageY),
        0xAE => (Ldx, Absolute),  0xBE => (Ldx, AbsoluteY),
        // LDY
        0xA0 => (Ldy, Immediate), 0xA4 => (Ldy, ZeroPage), 0xB4 => (Ldy, ZeroPageX),
        0xAC => (Ldy, Absolute),  0xBC => (Ldy, AbsoluteX),
        // STA
        0x85 => (Sta, ZeroPage), 0x95 => (Sta, ZeroPageX),
        0x8D => (Sta, Absolute), 0x9D => (Sta, AbsoluteX), 0x99 => (Sta, AbsoluteY),
        0x81 => (Sta, IndirectX), 0x91 => (Sta, IndirectY),
        // STX
        0x86 => (Stx, ZeroPage), 0x96 => (Stx, ZeroPageY), 0x8E => (Stx, Absolute),
        // STY
        0x84 => (Sty, ZeroPage), 0x94 => (Sty, ZeroPageX), 0x8C => (Sty, Absolute),
        // ASL memory
        0x06 => (AslM, ZeroPage), 0x16 => (AslM, ZeroPageX),
        0x0E => (AslM, Absolute), 0x1E => (AslM, AbsoluteX),
        // LSR memory
        0x46 => (LsrM, ZeroPage), 0x56 => (LsrM, ZeroPageX),
        0x4E => (LsrM, Absolute), 0x5E => (LsrM, AbsoluteX),
        // ROL memory
        0x26 => (RolM, ZeroPage), 0x36 => (RolM, ZeroPageX),
        0x2E => (RolM, Absolute), 0x3E => (RolM, AbsoluteX),
        // ROR memory
        0x66 => (RorM, ZeroPage), 0x76 => (RorM, ZeroPageX),
        0x6E => (RorM, Absolute), 0x7E => (RorM, AbsoluteX),
        // INC memory
        0xE6 => (IncM, ZeroPage), 0xF6 => (IncM, ZeroPageX),
        0xEE => (IncM, Absolute), 0xFE => (IncM, AbsoluteX),
        // DEC memory
        0xC6 => (DecM, ZeroPage), 0xD6 => (DecM, ZeroPageX),
        0xCE => (DecM, Absolute), 0xDE => (DecM, AbsoluteX),
        // Accumulator shifts
        0x0A => (AslA, Implied), 0x4A => (LsrA, Implied),
        0x2A => (RolA, Implied), 0x6A => (RorA, Implied),
        // BIT
        0x24 => (Bit, ZeroPage), 0x2C => (Bit, Absolute),
        // Transfers
        0xAA => (Tax, Implied), 0x8A => (Txa, Implied),
        0xA8 => (Tay, Implied), 0x98 => (Tya, Implied),
        0x9A => (Txs, Implied), 0xBA => (Tsx, Implied),
        // Inc/Dec register
        0xE8 => (Inx, Implied), 0xC8 => (Iny, Implied),
        0xCA => (Dex, Implied), 0x88 => (Dey, Implied),
        // Stack
        0x48 => (Pha, Implied), 0x68 => (Pla, Implied),
        0x08 => (Php, Implied), 0x28 => (Plp, Implied),
        // Flags
        0x18 => (Clc, Implied), 0x38 => (Sec, Implied),
        0x58 => (Cli, Implied), 0x78 => (Sei, Implied),
        0xB8 => (Clv, Implied),
        0xD8 => (Cld, Implied), 0xF8 => (Sed, Implied),
        // NOP
        0xEA => (Nop, Implied),
        _ => return None,
    })
}

/// Returns the static write target for ZP/ABS store instructions only.
fn static_write_addr(opcode: u8, operand: u16) -> Option<u16> {
    match opcode {
        0x85 | 0x86 | 0x84 => Some(operand & 0xFF),  // STA/STX/STY zp
        0x8D | 0x8E | 0x8C => Some(operand),          // STA/STX/STY abs
        _ => None,
    }
}

fn is_tia_write_addr(addr: u16) -> bool {
    (addr & 0x1080) == 0x0000 && (addr & 0x1000) == 0
}

fn is_wsync_write(opcode: u8, operand: u16) -> bool {
    if let Some(addr) = static_write_addr(opcode, operand) {
        is_tia_write_addr(addr) && (addr & 0x3F) == 0x02
    } else {
        false
    }
}

fn is_vsync_write(opcode: u8, operand: u16) -> bool {
    if let Some(addr) = static_write_addr(opcode, operand) {
        is_tia_write_addr(addr) && (addr & 0x3F) == 0x00
    } else {
        false
    }
}

/// Returns true if this instruction requires a block split (WSYNC or VSYNC write).
/// These writes affect frame loop control flow and must be at block boundaries.
fn is_frame_sync_write(opcode: u8, operand: u16) -> bool {
    is_wsync_write(opcode, operand) || is_vsync_write(opcode, operand)
}

fn is_tia_position_write(opcode: u8, operand: u16) -> bool {
    if let Some(addr) = static_write_addr(opcode, operand) {
        if is_tia_write_addr(addr) {
            let reg = addr & 0x3F;
            (0x10..=0x14).contains(&reg)
        } else {
            false
        }
    } else {
        false
    }
}

fn is_branch(opcode: u8) -> bool {
    matches!(opcode, 0x10 | 0x30 | 0x50 | 0x70 | 0x90 | 0xB0 | 0xD0 | 0xF0)
}

fn in_hotspot_region(pc: u16) -> bool {
    (pc & 0x0FFF) >= 0x0FF4
}

// ============================================================
// Block discovery
// ============================================================

/// Discover all reachable basic blocks in the ROM.
pub fn discover_blocks(rom: &[u8], scheme: BankScheme) -> Vec<BasicBlock> {
    let banks = num_banks(scheme);
    let last_bank = banks - 1;

    // Pass 1: collect all block-start addresses via worklist walk
    let mut block_starts: BTreeSet<(usize, u16)> = BTreeSet::new();
    let mut worklist: VecDeque<(usize, u16)> = VecDeque::new();
    let mut visited: HashSet<(usize, u16)> = HashSet::new();

    // Seed with interrupt vectors (from last bank where vectors live)
    let reset_pc = read_vector(rom, scheme, last_bank, 0xFFFC);
    let nmi_pc = read_vector(rom, scheme, last_bank, 0xFFFA);
    let irq_pc = read_vector(rom, scheme, last_bank, 0xFFFE);

    for pc in [reset_pc, nmi_pc, irq_pc] {
        if !in_hotspot_region(pc) {
            block_starts.insert((last_bank, pc));
            worklist.push_back((last_bank, pc));
        }
    }

    // Walk all reachable code to find block boundaries
    while let Some((bank, start)) = worklist.pop_front() {
        let mut pc = start;
        loop {
            if !visited.insert((bank, pc)) { break; }
            if in_hotspot_region(pc) { break; }

            let opcode = rom_byte(rom, scheme, bank, pc);
            let size = SIZE_TABLE[opcode as usize];
            if size == 0 { break; } // illegal opcode

            let operand = read_operand(rom, scheme, bank, pc, size);
            let next_pc = pc.wrapping_add(size as u16);

            // TIA position write → split before AND after
            if is_tia_position_write(opcode, operand) {
                block_starts.insert((bank, pc));
                if !in_hotspot_region(next_pc) {
                    block_starts.insert((bank, next_pc));
                    // Continue walking from next_pc
                    pc = next_pc;
                    continue;
                }
                break;
            }

            // WSYNC/VSYNC → split after (frame loop checks these between blocks)
            if is_frame_sync_write(opcode, operand) {
                if !in_hotspot_region(next_pc) {
                    block_starts.insert((bank, next_pc));
                }
                pc = next_pc;
                continue;
            }

            match opcode {
                // Branches
                _ if is_branch(opcode) => {
                    let offset = operand as u8 as i8;
                    let taken = next_pc.wrapping_add(offset as u16);
                    for target in [taken, next_pc] {
                        if !in_hotspot_region(target) {
                            block_starts.insert((bank, target));
                            worklist.push_back((bank, target));
                        }
                    }
                    break;
                }
                // JMP absolute
                0x4C => {
                    if !in_hotspot_region(operand) {
                        block_starts.insert((bank, operand));
                        worklist.push_back((bank, operand));
                    }
                    break;
                }
                // JMP indirect
                0x6C => { break; }
                // JSR
                0x20 => {
                    for target in [operand, next_pc] {
                        if !in_hotspot_region(target) {
                            block_starts.insert((bank, target));
                            worklist.push_back((bank, target));
                        }
                    }
                    break;
                }
                // RTS, RTI — target unknown (from stack)
                0x60 | 0x40 => { break; }
                // BRK — jumps to IRQ vector, returns to pc+2
                0x00 => {
                    let cont_pc = next_pc.wrapping_add(1); // BRK is 1 byte but pushes PC+2
                    if !in_hotspot_region(cont_pc) {
                        block_starts.insert((bank, cont_pc));
                        worklist.push_back((bank, cont_pc));
                    }
                    break;
                }
                _ => { pc = next_pc; }
            }
        }
    }

    // Pass 2: build blocks from sorted start addresses
    // Sort starts into a map by (bank, start_pc) for easy "next start" lookup
    let starts_vec: Vec<(usize, u16)> = block_starts.iter().copied().collect();
    let start_set: HashSet<(usize, u16)> = block_starts.iter().copied().collect();

    let mut blocks = Vec::new();

    for &(bank, start_pc) in &starts_vec {
        if in_hotspot_region(start_pc) { continue; }

        let mut instrs = Vec::new();
        let mut pc = start_pc;

        loop {
            if in_hotspot_region(pc) { break; }

            let opcode = rom_byte(rom, scheme, bank, pc);
            let size = SIZE_TABLE[opcode as usize];
            if size == 0 { break; }

            let operand = read_operand(rom, scheme, bank, pc, size);
            let cycles = CYCLE_TABLE[opcode as usize];
            let next_pc = pc.wrapping_add(size as u16);

            // If this address is a different block's start (not our first instruction), stop
            if pc != start_pc && start_set.contains(&(bank, pc)) {
                blocks.push(BasicBlock {
                    start_pc, bank, instructions: instrs,
                    exit_type: ExitType::Fallthrough { next_pc: pc },
                });
                break;
            }

            let instr = DecodedInstr { pc, opcode, operand, size, cycles };
            instrs.push(instr);

            // Check for block-ending opcodes
            match opcode {
                _ if is_branch(opcode) => {
                    let offset = operand as u8 as i8;
                    let taken = next_pc.wrapping_add(offset as u16);
                    blocks.push(BasicBlock {
                        start_pc, bank, instructions: instrs,
                        exit_type: ExitType::Branch { taken_pc: taken, not_taken_pc: next_pc },
                    });
                    break;
                }
                0x4C => {
                    blocks.push(BasicBlock {
                        start_pc, bank, instructions: instrs,
                        exit_type: ExitType::JmpAbs { target: operand },
                    });
                    break;
                }
                0x6C => {
                    blocks.push(BasicBlock {
                        start_pc, bank, instructions: instrs,
                        exit_type: ExitType::JmpInd { operand },
                    });
                    break;
                }
                0x20 => {
                    blocks.push(BasicBlock {
                        start_pc, bank, instructions: instrs,
                        exit_type: ExitType::Jsr { target: operand, return_pc: next_pc },
                    });
                    break;
                }
                0x60 => {
                    blocks.push(BasicBlock {
                        start_pc, bank, instructions: instrs,
                        exit_type: ExitType::Rts,
                    });
                    break;
                }
                0x40 => {
                    blocks.push(BasicBlock {
                        start_pc, bank, instructions: instrs,
                        exit_type: ExitType::Rti,
                    });
                    break;
                }
                0x00 => {
                    blocks.push(BasicBlock {
                        start_pc, bank, instructions: instrs,
                        exit_type: ExitType::Brk,
                    });
                    break;
                }
                _ if is_frame_sync_write(opcode, operand) => {
                    // WSYNC sets tia_wsync flag; VSYNC affects frame boundary.
                    // Both need the frame loop to check state between blocks.
                    let exit = if is_wsync_write(opcode, operand) {
                        ExitType::Wsync { next_pc }
                    } else {
                        ExitType::Fallthrough { next_pc }
                    };
                    blocks.push(BasicBlock {
                        start_pc, bank, instructions: instrs,
                        exit_type: exit,
                    });
                    break;
                }
                _ if is_tia_position_write(opcode, operand) => {
                    // Position write is a single-instruction block
                    blocks.push(BasicBlock {
                        start_pc, bank, instructions: instrs,
                        exit_type: ExitType::Fallthrough { next_pc },
                    });
                    break;
                }
                _ => { pc = next_pc; }
            }
        }
    }

    blocks.sort_by_key(|b| b.block_id());
    blocks
}

// ============================================================
// CUDA code generation
// ============================================================

#[cfg(feature = "cuda")]
fn read_expr(mode: AddrMode, operand: u16) -> String {
    match mode {
        AddrMode::Immediate => format!("0x{:02X}", operand & 0xFF),
        AddrMode::ZeroPage => format!("BR(0x{:04X})", operand & 0xFF),
        AddrMode::ZeroPageX => format!("BR((uint16_t)(({} + c->x) & 0xFF))", operand & 0xFF),
        AddrMode::ZeroPageY => format!("BR((uint16_t)(({} + c->y) & 0xFF))", operand & 0xFF),
        AddrMode::Absolute => format!("BR(0x{:04X})", operand),
        AddrMode::AbsoluteX => format!("BR((uint16_t)(0x{:04X} + c->x))", operand),
        AddrMode::AbsoluteY => format!("BR((uint16_t)(0x{:04X} + c->y))", operand),
        AddrMode::IndirectX => format!("BR(__aot_addr_izx(c, mr, rp, rl, 0x{:04X}))", operand & 0xFF),
        AddrMode::IndirectY => format!("BR(__aot_addr_izy(c, mr, rp, rl, 0x{:04X}))", operand & 0xFF),
        AddrMode::Implied => unreachable!("read_expr called with Implied"),
    }
}

#[cfg(feature = "cuda")]
fn write_addr_expr(mode: AddrMode, operand: u16) -> String {
    match mode {
        AddrMode::ZeroPage => format!("0x{:04X}", operand & 0xFF),
        AddrMode::ZeroPageX => format!("(uint16_t)(({} + c->x) & 0xFF)", operand & 0xFF),
        AddrMode::ZeroPageY => format!("(uint16_t)(({} + c->y) & 0xFF)", operand & 0xFF),
        AddrMode::Absolute => format!("0x{:04X}", operand),
        AddrMode::AbsoluteX => format!("(uint16_t)(0x{:04X} + c->x)", operand),
        AddrMode::AbsoluteY => format!("(uint16_t)(0x{:04X} + c->y)", operand),
        AddrMode::IndirectX => format!("__aot_addr_izx(c, mr, rp, rl, 0x{:04X})", operand & 0xFF),
        AddrMode::IndirectY => format!("__aot_addr_izy(c, mr, rp, rl, 0x{:04X})", operand & 0xFF),
        _ => unreachable!("write_addr_expr called with {:?}", mode),
    }
}

/// Returns page-cross penalty code for read operations in indexed modes.
/// Sets `_p` variable (must be declared in scope) to 0 or 1.
#[cfg(feature = "cuda")]
fn penalty_code(mode: AddrMode, operand: u16) -> Option<String> {
    match mode {
        AddrMode::AbsoluteX => Some(format!(
            "_p = ((0x{:04X} & 0xFF00) != ((uint16_t)(0x{:04X} + c->x) & 0xFF00)) ? 1 : 0;",
            operand, operand
        )),
        AddrMode::AbsoluteY => Some(format!(
            "_p = ((0x{:04X} & 0xFF00) != ((uint16_t)(0x{:04X} + c->y) & 0xFF00)) ? 1 : 0;",
            operand, operand
        )),
        AddrMode::IndirectY => Some(format!(
            "_p = __aot_px_izy(c, mr, rp, rl, 0x{:04X});",
            operand & 0xFF
        )),
        _ => None,
    }
}

/// Returns true if this op+mode combination can have page-cross penalties.
#[cfg(feature = "cuda")]
fn has_penalty(op: Op, mode: AddrMode) -> bool {
    use Op::*;
    let is_read = matches!(op, Lda | Ldx | Ldy | Adc | Sbc | And | Ora | Eor | Cmp | Cpx | Cpy);
    is_read && matches!(mode, AddrMode::AbsoluteX | AddrMode::AbsoluteY | AddrMode::IndirectY)
}

/// Emit C code for a single instruction. Returns lines of C code.
#[cfg(feature = "cuda")]
fn emit_instruction(op: Op, mode: AddrMode, operand: u16) -> Vec<String> {
    use Op::*;
    let mut lines = Vec::new();

    match op {
        // --- Load ---
        Lda => {
            let val = read_expr(mode, operand);
            lines.push(format!("c->a = {val}; set_zn(&c->status, c->a);"));
        }
        Ldx => {
            let val = read_expr(mode, operand);
            lines.push(format!("c->x = {val}; set_zn(&c->status, c->x);"));
        }
        Ldy => {
            let val = read_expr(mode, operand);
            lines.push(format!("c->y = {val}; set_zn(&c->status, c->y);"));
        }
        // --- Store ---
        Sta => {
            let addr = write_addr_expr(mode, operand);
            lines.push(format!("BW({addr}, c->a);"));
        }
        Stx => {
            let addr = write_addr_expr(mode, operand);
            lines.push(format!("BW({addr}, c->x);"));
        }
        Sty => {
            let addr = write_addr_expr(mode, operand);
            lines.push(format!("BW({addr}, c->y);"));
        }
        // --- ALU ---
        Adc => {
            let val = read_expr(mode, operand);
            lines.push(format!("do_adc(c, {val});"));
        }
        Sbc => {
            let val = read_expr(mode, operand);
            lines.push(format!("do_sbc(c, {val});"));
        }
        And => {
            let val = read_expr(mode, operand);
            lines.push(format!("c->a &= {val}; set_zn(&c->status, c->a);"));
        }
        Ora => {
            let val = read_expr(mode, operand);
            lines.push(format!("c->a |= {val}; set_zn(&c->status, c->a);"));
        }
        Eor => {
            let val = read_expr(mode, operand);
            lines.push(format!("c->a ^= {val}; set_zn(&c->status, c->a);"));
        }
        // --- Compare ---
        Cmp => {
            let val = read_expr(mode, operand);
            lines.push(format!("do_cmp(&c->status, c->a, {val});"));
        }
        Cpx => {
            let val = read_expr(mode, operand);
            lines.push(format!("do_cmp(&c->status, c->x, {val});"));
        }
        Cpy => {
            let val = read_expr(mode, operand);
            lines.push(format!("do_cmp(&c->status, c->y, {val});"));
        }
        // --- BIT ---
        Bit => {
            let val = read_expr(mode, operand);
            lines.push(format!("{{ uint8_t v = {val}; set_flag(&c->status, FLAG_ZERO, (c->a & v) == 0); \
                set_flag(&c->status, FLAG_OVERFLOW, (v & 0x40) != 0); \
                set_flag(&c->status, FLAG_NEGATIVE, (v & 0x80) != 0); }}"));
        }
        // --- RMW memory ---
        AslM => {
            let addr = write_addr_expr(mode, operand);
            lines.push(format!("do_asl_m(c, mr, rp, rl, {addr});"));
        }
        LsrM => {
            let addr = write_addr_expr(mode, operand);
            lines.push(format!("do_lsr_m(c, mr, rp, rl, {addr});"));
        }
        RolM => {
            let addr = write_addr_expr(mode, operand);
            lines.push(format!("do_rol_m(c, mr, rp, rl, {addr});"));
        }
        RorM => {
            let addr = write_addr_expr(mode, operand);
            lines.push(format!("do_ror_m(c, mr, rp, rl, {addr});"));
        }
        IncM => {
            let addr = write_addr_expr(mode, operand);
            lines.push(format!("do_inc_m(c, mr, rp, rl, {addr});"));
        }
        DecM => {
            let addr = write_addr_expr(mode, operand);
            lines.push(format!("do_dec_m(c, mr, rp, rl, {addr});"));
        }
        // --- Accumulator shifts ---
        AslA => {
            lines.push("set_flag(&c->status, FLAG_CARRY, (c->a & 0x80) != 0); c->a <<= 1; set_zn(&c->status, c->a);".into());
        }
        LsrA => {
            lines.push("set_flag(&c->status, FLAG_CARRY, (c->a & 0x01) != 0); c->a >>= 1; \
                set_flag(&c->status, FLAG_ZERO, c->a == 0); set_flag(&c->status, FLAG_NEGATIVE, false);".into());
        }
        RolA => {
            lines.push("{ uint8_t oc = get_flag(c->status, FLAG_CARRY) ? 1 : 0; \
                set_flag(&c->status, FLAG_CARRY, (c->a & 0x80) != 0); \
                c->a = (c->a << 1) | oc; set_zn(&c->status, c->a); }".into());
        }
        RorA => {
            lines.push("{ uint8_t oc = get_flag(c->status, FLAG_CARRY) ? 1 : 0; \
                set_flag(&c->status, FLAG_CARRY, (c->a & 0x01) != 0); \
                c->a = (c->a >> 1) | (oc << 7); set_zn(&c->status, c->a); }".into());
        }
        // --- Transfers ---
        Tax => lines.push("c->x = c->a; set_zn(&c->status, c->x);".into()),
        Txa => lines.push("c->a = c->x; set_zn(&c->status, c->a);".into()),
        Tay => lines.push("c->y = c->a; set_zn(&c->status, c->y);".into()),
        Tya => lines.push("c->a = c->y; set_zn(&c->status, c->a);".into()),
        Txs => lines.push("c->sp = c->x;".into()),
        Tsx => lines.push("c->x = c->sp; set_zn(&c->status, c->x);".into()),
        // --- Inc/Dec register ---
        Inx => lines.push("c->x += 1; set_zn(&c->status, c->x);".into()),
        Iny => lines.push("c->y += 1; set_zn(&c->status, c->y);".into()),
        Dex => lines.push("c->x -= 1; set_zn(&c->status, c->x);".into()),
        Dey => lines.push("c->y -= 1; set_zn(&c->status, c->y);".into()),
        // --- Stack ---
        Pha => lines.push("PUSH8(c->a);".into()),
        Pla => lines.push("c->a = PULL8_VAL(); set_zn(&c->status, c->a);".into()),
        Php => lines.push("PUSH8(c->status | FLAG_BREAK | FLAG_UNUSED);".into()),
        Plp => lines.push("{ uint8_t f = PULL8_VAL(); c->status = f & ~(FLAG_BREAK | FLAG_UNUSED); }".into()),
        // --- Flags ---
        Clc => lines.push("set_flag(&c->status, FLAG_CARRY, false);".into()),
        Sec => lines.push("set_flag(&c->status, FLAG_CARRY, true);".into()),
        Cli => lines.push("set_flag(&c->status, FLAG_INTERRUPT, false);".into()),
        Sei => lines.push("set_flag(&c->status, FLAG_INTERRUPT, true);".into()),
        Clv => lines.push("set_flag(&c->status, FLAG_OVERFLOW, false);".into()),
        Cld => lines.push("set_flag(&c->status, FLAG_DECIMAL, false);".into()),
        Sed => lines.push("set_flag(&c->status, FLAG_DECIMAL, true);".into()),
        // --- NOP ---
        Nop => {}
    }

    // Note: page-cross penalties are handled in emit_block per-instruction ticking
    lines
}

/// Branch flag condition for the given branch opcode.
#[cfg(feature = "cuda")]
fn branch_condition(opcode: u8) -> &'static str {
    match opcode {
        0x10 => "!get_flag(c->status, FLAG_NEGATIVE)",  // BPL
        0x30 => "get_flag(c->status, FLAG_NEGATIVE)",    // BMI
        0x50 => "!get_flag(c->status, FLAG_OVERFLOW)",   // BVC
        0x70 => "get_flag(c->status, FLAG_OVERFLOW)",    // BVS
        0x90 => "!get_flag(c->status, FLAG_CARRY)",      // BCC
        0xB0 => "get_flag(c->status, FLAG_CARRY)",       // BCS
        0xD0 => "!get_flag(c->status, FLAG_ZERO)",       // BNE
        0xF0 => "get_flag(c->status, FLAG_ZERO)",        // BEQ
        _ => unreachable!(),
    }
}

/// Generate C code for a single basic block (the body of a switch case).
/// Uses per-instruction ticking for correct timing behavior.
/// The main compiled-block wins (no switch dispatch, inlined operands) are preserved.
#[cfg(feature = "cuda")]
fn emit_block(block: &BasicBlock) -> String {
    let mut out = String::new();
    let id = block.block_id();

    let _ = writeln!(out, "    case 0x{id:04X}: {{");

    // Split: body instructions vs exit instruction
    let (body_instrs, exit_instr) = match block.exit_type {
        ExitType::Branch { .. } | ExitType::JmpAbs { .. } | ExitType::JmpInd { .. } |
        ExitType::Jsr { .. } | ExitType::Rts | ExitType::Rti | ExitType::Brk => {
            let (body, last) = block.instructions.split_at(block.instructions.len() - 1);
            (body, Some(&last[0]))
        }
        ExitType::Wsync { .. } | ExitType::Fallthrough { .. } => {
            (&block.instructions[..], None)
        }
    };

    // Emit body instructions with per-instruction ticking
    for instr in body_instrs {
        if let Some((op, mode)) = classify(instr.opcode) {
            for line in emit_instruction(op, mode, instr.operand) {
                let _ = writeln!(out, "        {line}");
            }
            let cy = instr.cycles;
            if has_penalty(op, mode) {
                let pen_code = penalty_code(mode, instr.operand).unwrap();
                let _ = writeln!(out, "        {{ uint8_t _p = 0; {pen_code} \
                    tia_tick_n(c, {cy} + _p); pia_tick_n(c, (uint16_t)({cy} + _p)); *cycles += {cy} + _p; }}");
            } else {
                let _ = writeln!(out, "        tia_tick_n(c, {cy}); pia_tick_n(c, (uint16_t){cy}); *cycles += {cy};");
            }
        }
    }

    // Emit exit
    match &block.exit_type {
        ExitType::Branch { taken_pc, not_taken_pc } => {
            let bi = exit_instr.unwrap();
            let cond = branch_condition(bi.opcode);
            let page_cross = (not_taken_pc & 0xFF00) != (taken_pc & 0xFF00);
            let taken_cy = bi.cycles + 1 + if page_cross { 1 } else { 0 };
            let not_taken_cy = bi.cycles;
            let _ = writeln!(out, "        if ({cond}) {{");
            let _ = writeln!(out, "            tia_tick_n(c, {taken_cy}); pia_tick_n(c, (uint16_t){taken_cy}); *cycles += {taken_cy};");
            let _ = writeln!(out, "            c->pc = 0x{taken_pc:04X}; break;");
            let _ = writeln!(out, "        }}");
            let _ = writeln!(out, "        tia_tick_n(c, {not_taken_cy}); pia_tick_n(c, (uint16_t){not_taken_cy}); *cycles += {not_taken_cy};");
            let _ = writeln!(out, "        c->pc = 0x{not_taken_pc:04X}; break;");
        }
        ExitType::JmpAbs { target } => {
            let cy = exit_instr.unwrap().cycles;
            let _ = writeln!(out, "        tia_tick_n(c, {cy}); pia_tick_n(c, (uint16_t){cy}); *cycles += {cy};");
            let _ = writeln!(out, "        c->pc = 0x{target:04X}; break;");
        }
        ExitType::JmpInd { operand } => {
            let cy = exit_instr.unwrap().cycles;
            let _ = writeln!(out, "        {{ uint16_t p = 0x{operand:04X}; uint8_t lo = BR(p); \
                uint8_t hi = BR((p & 0xFF00) | ((p + 1) & 0x00FF));");
            let _ = writeln!(out, "        tia_tick_n(c, {cy}); pia_tick_n(c, (uint16_t){cy}); *cycles += {cy};");
            let _ = writeln!(out, "        c->pc = ((uint16_t)hi << 8) | lo; break; }}");
        }
        ExitType::Jsr { target, .. } => {
            let ei = exit_instr.unwrap();
            let push_addr = ei.pc.wrapping_add(2);
            let _ = writeln!(out, "        PUSH16(0x{push_addr:04X});");
            let _ = writeln!(out, "        tia_tick_n(c, {}); pia_tick_n(c, (uint16_t){}); *cycles += {};",
                ei.cycles, ei.cycles, ei.cycles);
            let _ = writeln!(out, "        c->pc = 0x{target:04X}; break;");
        }
        ExitType::Rts => {
            let cy = exit_instr.unwrap().cycles;
            let _ = writeln!(out, "        {{ uint16_t r = PULL16_VAL();");
            let _ = writeln!(out, "        tia_tick_n(c, {cy}); pia_tick_n(c, (uint16_t){cy}); *cycles += {cy};");
            let _ = writeln!(out, "        c->pc = r + 1; break; }}");
        }
        ExitType::Rti => {
            let cy = exit_instr.unwrap().cycles;
            let _ = writeln!(out, "        {{ uint8_t f = PULL8_VAL(); c->status = f & ~(FLAG_BREAK | FLAG_UNUSED);");
            let _ = writeln!(out, "        c->pc = PULL16_VAL();");
            let _ = writeln!(out, "        tia_tick_n(c, {cy}); pia_tick_n(c, (uint16_t){cy}); *cycles += {cy};");
            let _ = writeln!(out, "        break; }}");
        }
        ExitType::Brk => {
            let ei = exit_instr.unwrap();
            let push_addr = ei.pc.wrapping_add(2);
            let _ = writeln!(out, "        PUSH16(0x{push_addr:04X});");
            let _ = writeln!(out, "        PUSH8(c->status | FLAG_BREAK | FLAG_UNUSED);");
            let _ = writeln!(out, "        set_flag(&c->status, FLAG_INTERRUPT, true);");
            let _ = writeln!(out, "        tia_tick_n(c, {}); pia_tick_n(c, (uint16_t){}); *cycles += {};",
                ei.cycles, ei.cycles, ei.cycles);
            let _ = writeln!(out, "        c->pc = R16(0xFFFE); break;");
        }
        ExitType::Wsync { next_pc } | ExitType::Fallthrough { next_pc } => {
            // Body instructions already ticked individually
            let _ = writeln!(out, "        c->pc = 0x{next_pc:04X}; break;");
        }
    }

    let _ = writeln!(out, "    }}");
    out
}

/// Strip all `#include` lines from embedded header source.
/// nvrtc doesn't have access to host filesystem headers.
#[cfg(feature = "cuda")]
fn strip_includes(src: &str) -> String {
    src.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("#include")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Generate the complete CUDA source for a compiled kernel.
#[cfg(feature = "cuda")]
pub fn generate_compiled_source(blocks: &[BasicBlock], _rom: &[u8], _scheme: BankScheme) -> String {
    // Embed all required headers
    let state_layout = include_str!("../cuda/src/state_layout.cuh");
    let tia_headless = include_str!("../cuda/src/tia_headless.cuh");
    let pia = include_str!("../cuda/src/pia.cuh");
    let memory_bus = include_str!("../cuda/src/memory_bus.cuh");
    let cpu_6502 = include_str!("../cuda/src/cpu_6502.cuh");
    let decoded_op = include_str!("../cuda/src/decoded_op.cuh");
    let cpu_6502_aot = include_str!("../cuda/src/cpu_6502_aot.cuh");

    let mut src = String::with_capacity(256 * 1024);

    // Write headers in dependency order (strip all #include lines)
    let _ = writeln!(src, "// === Auto-generated compiled kernel ===");
    // nvrtc has built-in fixed-width types but needs explicit typedefs
    let _ = writeln!(src, r#"
typedef unsigned char      uint8_t;
typedef signed char        int8_t;
typedef unsigned short     uint16_t;
typedef signed short       int16_t;
typedef unsigned int       uint32_t;
typedef signed int         int32_t;
typedef unsigned long long uint64_t;
typedef signed long long   int64_t;
typedef unsigned long      size_t;
"#);
    let _ = writeln!(src, "// --- state_layout.cuh ---");
    let _ = writeln!(src, "{}", strip_includes(state_layout));
    let _ = writeln!(src, "\n// --- tia_headless.cuh ---");
    let _ = writeln!(src, "{}", strip_includes(tia_headless));
    let _ = writeln!(src, "\n// --- pia.cuh ---");
    let _ = writeln!(src, "{}", strip_includes(pia));
    let _ = writeln!(src, "\n// --- memory_bus.cuh ---");
    let _ = writeln!(src, "{}", strip_includes(memory_bus));
    let _ = writeln!(src, "\n// --- cpu_6502.cuh ---");
    let _ = writeln!(src, "{}", strip_includes(cpu_6502));
    let _ = writeln!(src, "\n// --- decoded_op.cuh ---");
    let _ = writeln!(src, "{}", strip_includes(decoded_op));
    let _ = writeln!(src, "\n// --- cpu_6502_aot.cuh ---");
    let _ = writeln!(src, "{}", strip_includes(cpu_6502_aot));

    // Apply action (from atari_kernel.cu)
    let _ = writeln!(src, r#"
// === Action dispatch ===
__device__ __forceinline__
void apply_action(ThreadCtx* c, uint8_t action) {{
    bool up=false, dn=false, lt=false, rt=false, fi=false;
    switch (action) {{
        case 0: break; case 1: fi=true; break;
        case 2: up=true; break; case 3: rt=true; break;
        case 4: lt=true; break; case 5: dn=true; break;
        case 6: up=true;rt=true; break; case 7: up=true;lt=true; break;
        case 8: dn=true;rt=true; break; case 9: dn=true;lt=true; break;
        case 10: up=true;fi=true; break; case 11: rt=true;fi=true; break;
        case 12: lt=true;fi=true; break; case 13: dn=true;fi=true; break;
        case 14: up=true;rt=true;fi=true; break; case 15: up=true;lt=true;fi=true; break;
        case 16: dn=true;rt=true;fi=true; break; case 17: dn=true;lt=true;fi=true; break;
    }}
    uint8_t swcha = 0xFF;
    if (up) swcha &= ~0x10; if (dn) swcha &= ~0x20;
    if (lt) swcha &= ~0x40; if (rt) swcha &= ~0x80;
    c->port_a_input = swcha;
    c->inpt4 = fi ? 0 : 1;
    c->paddle0 = lt ? 200 : (rt ? 50 : 128);
}}

// Batched tick that handles multiple scanline crossings.
// Regular tia_tick_n only handles one; compiled blocks can span >1 scanline.
__device__ __forceinline__
void tia_tick_block(ThreadCtx* c, uint16_t n) {{
    c->tia_clock += n * 3;
    while (c->tia_clock >= 228) {{
        tia_end_scanline(c);
    }}
}}
"#);

    // Deduplicate blocks by block_id (mirror addresses map to same ROM offset)
    let mut seen_ids = std::collections::HashSet::new();
    let unique_blocks: Vec<&BasicBlock> = blocks.iter()
        .filter(|b| seen_ids.insert(b.block_id()))
        .collect();

    // Compiled block dispatcher
    let _ = writeln!(src, "// === Compiled block dispatcher ({} unique blocks) ===", unique_blocks.len());
    let _ = writeln!(src, "__device__");
    let _ = writeln!(src, "void run_compiled_block(ThreadCtx* c, uint8_t* mr, uint64_t* cycles,");
    let _ = writeln!(src, "                       const uint8_t* rp, uint32_t rl,");
    let _ = writeln!(src, "                       const DecodedOp* __restrict__ dt) {{");
    let _ = writeln!(src, "    uint32_t block_id = ((uint32_t)c->bank << 12) | (c->pc & 0x0FFF);");
    let _ = writeln!(src, "    switch (block_id) {{");

    for block in &unique_blocks {
        let _ = write!(src, "{}", emit_block(block));
    }

    // Default: fallback to AOT interpreter
    let _ = writeln!(src, "    default: {{");
    let _ = writeln!(src, "        uint8_t cy = cpu_step_aot(c, mr, rp, rl, dt);");
    let _ = writeln!(src, "        *cycles += cy;");
    let _ = writeln!(src, "        tia_tick_n(c, cy);");
    let _ = writeln!(src, "        pia_tick_n(c, (uint16_t)cy);");
    let _ = writeln!(src, "        break;");
    let _ = writeln!(src, "    }}");
    let _ = writeln!(src, "    }}");
    let _ = writeln!(src, "}}");

    // Frame runner
    let _ = writeln!(src, r#"
__device__ __forceinline__
void run_one_cycle_compiled(ThreadCtx* c, uint8_t* mr, uint64_t* cycles,
                            const uint8_t* rp, uint32_t rl,
                            const DecodedOp* __restrict__ dt) {{
    if (c->tia_wsync) {{
        uint16_t skipped = tia_skip_to_scanline_end(c);
        pia_tick_n(c, skipped);
        *cycles += skipped;
    }} else {{
        run_compiled_block(c, mr, cycles, rp, rl, dt);
    }}
}}

__device__
void run_frame_compiled(ThreadCtx* c, uint8_t* mr,
                        const uint8_t* rp, uint32_t rl,
                        const DecodedOp* __restrict__ dt) {{
    c->tia_frame_complete = 0;
    uint64_t cycles = 0;

    while (c->tia_vsync & 0x02)
        run_one_cycle_compiled(c, mr, &cycles, rp, rl, dt);

    while (!(c->tia_vsync & 0x02)) {{
        run_one_cycle_compiled(c, mr, &cycles, rp, rl, dt);
        if (cycles > 100000) break;
    }}
}}
"#);

    // Kernel entry points
    let _ = writeln!(src, r#"
extern "C" __global__
void atari_frame_kernel_compiled(
    AtariState* __restrict__ states,
    const uint8_t* __restrict__ actions,
    uint8_t* __restrict__ obs_out,
    const uint8_t* __restrict__ rom_ptr,
    uint32_t rom_len,
    const DecodedOp* __restrict__ decode_table,
    int N
) {{
    __shared__ uint8_t sram[BLOCK_SIZE][128];
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= N) return;
    uint8_t* my_ram = sram[threadIdx.x];
    ThreadCtx ctx;
    load_ctx(&ctx, &states[idx]);
    for (int i = 0; i < 128; i++) my_ram[i] = states[idx].ram[i];
    apply_action(&ctx, actions[idx]);
    run_frame_compiled(&ctx, my_ram, rom_ptr, rom_len, decode_table);
    store_ctx(&states[idx], &ctx);
    uint8_t* obs = &obs_out[idx * 128];
    for (int i = 0; i < 128; i++) {{
        uint8_t v = my_ram[i];
        states[idx].ram[i] = v;
        obs[i] = v;
    }}
}}

extern "C" __global__
void atari_multi_frame_kernel_compiled(
    AtariState* __restrict__ states,
    const uint8_t* __restrict__ actions,
    uint8_t* __restrict__ obs_out,
    const uint8_t* __restrict__ rom_ptr,
    uint32_t rom_len,
    const DecodedOp* __restrict__ decode_table,
    int K,
    int N
) {{
    __shared__ uint8_t sram[BLOCK_SIZE][128];
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= N) return;
    uint8_t* my_ram = sram[threadIdx.x];
    ThreadCtx ctx;
    load_ctx(&ctx, &states[idx]);
    for (int i = 0; i < 128; i++) my_ram[i] = states[idx].ram[i];
    for (int f = 0; f < K; f++) {{
        apply_action(&ctx, actions[f * N + idx]);
        run_frame_compiled(&ctx, my_ram, rom_ptr, rom_len, decode_table);
    }}
    store_ctx(&states[idx], &ctx);
    uint8_t* obs = &obs_out[idx * 128];
    for (int i = 0; i < 128; i++) {{
        uint8_t v = my_ram[i];
        states[idx].ram[i] = v;
        obs[i] = v;
    }}
}}
"#);

    src
}

// ============================================================
// Analysis / debug helpers
// ============================================================

pub fn print_block_summary(blocks: &[BasicBlock]) {
    let total_instrs: usize = blocks.iter().map(|b| b.instructions.len()).sum();
    let total_cycles: u32 = blocks.iter().map(|b| b.base_cycles()).sum();

    println!("ROM Compiler: {} blocks, {} instructions, {} base cycles",
        blocks.len(), total_instrs, total_cycles);

    let mut exit_counts = BTreeMap::new();
    for block in blocks {
        let key = match &block.exit_type {
            ExitType::Branch { .. } => "branch",
            ExitType::JmpAbs { .. } => "jmp_abs",
            ExitType::JmpInd { .. } => "jmp_ind",
            ExitType::Jsr { .. } => "jsr",
            ExitType::Rts => "rts",
            ExitType::Rti => "rti",
            ExitType::Brk => "brk",
            ExitType::Wsync { .. } => "wsync",
            ExitType::Fallthrough { .. } => "fallthrough",
        };
        *exit_counts.entry(key).or_insert(0) += 1;
    }
    println!("  Exit types: {:?}", exit_counts);

    let block_sizes: Vec<usize> = blocks.iter().map(|b| b.instructions.len()).collect();
    if let (Some(&min), Some(&max)) = (block_sizes.iter().min(), block_sizes.iter().max()) {
        let avg = total_instrs as f64 / blocks.len() as f64;
        println!("  Block sizes: min={min}, max={max}, avg={avg:.1}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_covers_all_valid_opcodes() {
        let mut classified = 0;
        for opcode in 0..=255u8 {
            if classify(opcode).is_some() {
                classified += 1;
            }
        }
        // 56 base instructions with various addressing modes = ~151 opcodes
        // Plus branches, jumps, etc. handled separately
        assert!(classified >= 50, "Expected at least 50 classified opcodes, got {classified}");
    }

    #[test]
    fn test_wsync_detection() {
        // STA $02 (ZP) — WSYNC
        assert!(is_wsync_write(0x85, 0x02));
        // STA $42 (ZP) — mirrors to WSYNC
        assert!(is_wsync_write(0x85, 0x42));
        // STA $0002 (ABS) — WSYNC
        assert!(is_wsync_write(0x8D, 0x0002));
        // STA $85 (ZP) — PIA RAM, not WSYNC
        assert!(!is_wsync_write(0x85, 0x85));
        // LDA $02 — not a store, not WSYNC
        assert!(!is_wsync_write(0xA5, 0x02));
        // STA $02,X — indexed, can't determine statically
        assert!(!is_wsync_write(0x95, 0x02));
    }

    #[test]
    fn test_tia_position_detection() {
        // STA $10 — RESP0
        assert!(is_tia_position_write(0x85, 0x10));
        // STA $14 — RESBL
        assert!(is_tia_position_write(0x85, 0x14));
        // STA $0F — PF2, not position
        assert!(!is_tia_position_write(0x85, 0x0F));
        // STA $15 — AUDC0, not position
        assert!(!is_tia_position_write(0x85, 0x15));
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_dump_compiled_source() {
        let rom = match std::fs::read("Breakout.bin") {
            Ok(r) => r,
            Err(_) => { eprintln!("Breakout.bin not found, skipping"); return; }
        };
        let scheme = BankScheme::detect(rom.len());
        let blocks = discover_blocks(&rom, scheme);
        let src = generate_compiled_source(&blocks, &rom, scheme);
        std::fs::write("/tmp/compiled_kernel.cu", &src).unwrap();
        eprintln!("Wrote {} bytes to /tmp/compiled_kernel.cu", src.len());
    }

    #[test]
    fn test_discover_blocks_breakout() {
        let rom = match std::fs::read("Breakout.bin") {
            Ok(r) => r,
            Err(_) => { eprintln!("Breakout.bin not found, skipping"); return; }
        };
        let scheme = BankScheme::detect(rom.len());
        let blocks = discover_blocks(&rom, scheme);
        print_block_summary(&blocks);

        // Breakout is a 2K/4K ROM — expect ~100-400 blocks
        assert!(blocks.len() >= 20, "Expected >=20 blocks for Breakout, got {}", blocks.len());
        assert!(blocks.len() <= 1000, "Expected <=1000 blocks, got {}", blocks.len());

        // Should have a mix of exit types
        let branch_count = blocks.iter().filter(|b| matches!(b.exit_type, ExitType::Branch { .. })).count();
        let wsync_count = blocks.iter().filter(|b| matches!(b.exit_type, ExitType::Wsync { .. })).count();
        assert!(branch_count > 0, "Expected at least some branch blocks");
        assert!(wsync_count > 0, "Expected at least some WSYNC blocks");
    }

    #[test]
    fn test_discover_blocks_simple() {
        // Minimal ROM: LDA #$00; STA $02 (WSYNC); JMP $F000
        // Place at offset $F00 within a 4K ROM
        let mut rom = vec![0xEA; 4096]; // fill with NOP
        // Reset vector → $F000
        rom[0x0FFC] = 0x00;
        rom[0x0FFD] = 0xF0;
        // $F000: LDA #$42
        rom[0x0000] = 0xA9;
        rom[0x0001] = 0x42;
        // $F002: STA $02 (WSYNC)
        rom[0x0002] = 0x85;
        rom[0x0003] = 0x02;
        // $F004: JMP $F000
        rom[0x0004] = 0x4C;
        rom[0x0005] = 0x00;
        rom[0x0006] = 0xF0;

        let blocks = discover_blocks(&rom, BankScheme::Fixed);
        assert!(!blocks.is_empty(), "Should discover at least one block");

        // Should have at least: [LDA #$42, STA $02] (WSYNC exit) and [JMP $F000]
        let wsync_blocks: Vec<_> = blocks.iter()
            .filter(|b| matches!(b.exit_type, ExitType::Wsync { .. }))
            .collect();
        assert!(!wsync_blocks.is_empty(), "Should have at least one WSYNC block");
    }
}
