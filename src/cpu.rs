/// Cycle-accurate 6502 CPU emulator.
///
/// Every CPU cycle produces exactly one bus operation: `memory.read()`,
/// `memory.write()`, or `memory.tick()`. This keeps external hardware
/// (TIA, PIA) advancing in lockstep with the CPU.

// ---------------------------------------------------------------------------
// Status flags
// ---------------------------------------------------------------------------

macro_rules! flags {
    ($($name:ident = $bit:expr),* $(,)?) => {
        $(pub const $name: u8 = 1 << $bit;)*
    };
}

flags! {
    CARRY     = 0,
    ZERO      = 1,
    INTERRUPT = 2,
    DECIMAL   = 3,
    BREAK     = 4,
    UNUSED    = 5,
    OVERFLOW  = 6,
    NEGATIVE  = 7,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Status(pub u8);

impl Status {
    /// Pack for push (always set BREAK and UNUSED bits).
    pub fn to_byte_with_break(self) -> u8 {
        self.0 | BREAK | UNUSED
    }

    /// Restore from a pulled byte (BREAK and UNUSED are ignored on pull).
    pub fn from_byte(byte: u8) -> Self {
        Self(byte & !(BREAK | UNUSED))
    }

    #[inline] pub fn set(&mut self, flag: u8, on: bool) {
        if on { self.0 |= flag; } else { self.0 &= !flag; }
    }
    #[inline] pub fn get(self, flag: u8) -> bool { self.0 & flag != 0 }

    /// Set ZERO and NEGATIVE from a result byte.
    #[inline] pub fn set_zn(&mut self, val: u8) {
        self.set(ZERO, val == 0);
        self.set(NEGATIVE, val & 0x80 != 0);
    }
}

// ---------------------------------------------------------------------------
// Memory trait
// ---------------------------------------------------------------------------

pub trait Memory {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn tick(&mut self) {}
    fn tick_count(&self) -> u32 { 0 }
}

// ---------------------------------------------------------------------------
// FlatMemory (test helper)
// ---------------------------------------------------------------------------

pub struct FlatMemory {
    pub ram: [u8; 0x10000],
}

impl Default for FlatMemory {
    fn default() -> Self { Self { ram: [0; 0x10000] } }
}

impl FlatMemory {
    pub fn new() -> Self { Self::default() }

    /// Load a binary blob at the given base address.
    pub fn load(&mut self, base: u16, data: &[u8]) {
        let start = base as usize;
        self.ram[start..start + data.len()].copy_from_slice(data);
    }
}

impl Memory for FlatMemory {
    fn read(&mut self, addr: u16) -> u8 { self.ram[addr as usize] }
    fn write(&mut self, addr: u16, val: u8) { self.ram[addr as usize] = val; }
}

// ---------------------------------------------------------------------------
// CPU
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Cpu {
    pub pc: u16,
    pub sp: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub status: Status,
    pub cycles: u64,
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            pc: 0,
            sp: 0xFD,
            a: 0,
            x: 0,
            y: 0,
            status: Status::default(),
            cycles: 0,
        }
    }
}

impl Cpu {
    pub fn new() -> Self { Self::default() }

    /// Reset the CPU, reading the reset vector from $FFFC/$FFFD.
    pub fn reset<M: Memory>(&mut self, memory: &mut M) {
        let lo = memory.read(0xFFFC) as u16;
        let hi = memory.read(0xFFFD) as u16;
        self.pc = (hi << 8) | lo;
        self.sp = 0xFD;
        self.status = Status::default();
        self.a = 0;
        self.x = 0;
        self.y = 0;
    }

    // -- helpers for stack operations --

    #[inline]
    fn push<M: Memory>(&mut self, mem: &mut M, val: u8) {
        mem.write(0x0100 | self.sp as u16, val);
        self.sp = self.sp.wrapping_sub(1);
    }

    #[inline]
    fn pull<M: Memory>(&mut self, mem: &mut M) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        mem.read(0x0100 | self.sp as u16)
    }

    // -- ALU helpers --

    #[inline]
    fn adc(&mut self, val: u8) {
        let carry = self.status.get(CARRY) as u8;
        if self.status.get(DECIMAL) {
            let mut lo = (self.a & 0x0F) + (val & 0x0F) + carry;
            if lo > 9 { lo += 6; }
            let mut hi = (self.a >> 4) + (val >> 4) + if lo > 0x0F { 1 } else { 0 };
            let bin_sum = (self.a as u16) + (val as u16) + (carry as u16);
            self.status.set(ZERO, (bin_sum as u8) == 0);
            self.status.set(NEGATIVE, (hi & 0x08) != 0);
            self.status.set(OVERFLOW,
                (!(self.a ^ val) & (self.a ^ ((hi << 4) | (lo & 0x0F))) & 0x80) != 0);
            if hi > 9 { hi += 6; }
            self.status.set(CARRY, hi > 0x0F);
            self.a = ((hi & 0x0F) << 4) | (lo & 0x0F);
        } else {
            let (sum1, c1) = self.a.overflowing_add(val);
            let (sum2, c2) = sum1.overflowing_add(carry);
            self.status.set(CARRY, c1 || c2);
            self.status.set(OVERFLOW, (!(self.a ^ val) & (self.a ^ sum2) & 0x80) != 0);
            self.a = sum2;
            self.status.set_zn(self.a);
        }
    }

    #[inline]
    fn sbc(&mut self, val: u8) {
        let borrow = !self.status.get(CARRY) as u8;
        if self.status.get(DECIMAL) {
            let mut lo = (self.a & 0x0F).wrapping_sub(val & 0x0F).wrapping_sub(borrow);
            let lo_borrow = if (lo as i8) < 0 { lo = lo.wrapping_sub(6); 1u8 } else { 0 };
            let mut hi = (self.a >> 4).wrapping_sub(val >> 4).wrapping_sub(lo_borrow);
            if (hi as i8) < 0 { hi = hi.wrapping_sub(6); }
            let bin_diff = (self.a as i16) - (val as i16) - (borrow as i16);
            self.status.set(CARRY, bin_diff >= 0);
            self.status.set(ZERO, (bin_diff as u8) == 0);
            self.status.set(NEGATIVE, (bin_diff as u8) & 0x80 != 0);
            self.status.set(OVERFLOW,
                ((self.a ^ val) & (self.a ^ (bin_diff as u8)) & 0x80) != 0);
            self.a = ((hi & 0x0F) << 4) | (lo & 0x0F);
        } else {
            let (diff1, b1) = self.a.overflowing_sub(val);
            let (diff2, b2) = diff1.overflowing_sub(borrow);
            self.status.set(CARRY, !(b1 || b2));
            self.status.set(OVERFLOW, ((self.a ^ val) & (self.a ^ diff2) & 0x80) != 0);
            self.a = diff2;
            self.status.set_zn(self.a);
        }
    }

    #[inline]
    fn compare(&mut self, reg: u8, val: u8) {
        let result = reg.wrapping_sub(val);
        self.status.set(CARRY, reg >= val);
        self.status.set_zn(result);
    }

    #[inline]
    fn asl_val(&mut self, val: u8) -> u8 {
        self.status.set(CARRY, val & 0x80 != 0);
        let r = val << 1;
        self.status.set_zn(r);
        r
    }

    #[inline]
    fn lsr_val(&mut self, val: u8) -> u8 {
        self.status.set(CARRY, val & 0x01 != 0);
        let r = val >> 1;
        self.status.set_zn(r);
        r
    }

    #[inline]
    fn rol_val(&mut self, val: u8) -> u8 {
        let old_carry = self.status.get(CARRY) as u8;
        self.status.set(CARRY, val & 0x80 != 0);
        let r = (val << 1) | old_carry;
        self.status.set_zn(r);
        r
    }

    #[inline]
    fn ror_val(&mut self, val: u8) -> u8 {
        let old_carry = self.status.get(CARRY) as u8;
        self.status.set(CARRY, val & 0x01 != 0);
        let r = (val >> 1) | (old_carry << 7);
        self.status.set_zn(r);
        r
    }

    /// Execute a single instruction with cycle-accurate bus accesses.
    /// Returns the number of cycles consumed.
    pub fn step<M: Memory>(&mut self, memory: &mut M) -> u8 {
        let mut cycles: u8 = 0;

        // Cycle 1: fetch opcode
        let opcode = memory.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        cycles += 1;

        match opcode {
            // ==================================================================
            // NOP (implied, 2 cycles)
            // ==================================================================
            0xEA => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
            }

            // ==================================================================
            // Flag instructions (implied, 2 cycles)
            // ==================================================================
            0x18 | 0x38 | 0x58 | 0x78 | 0xB8 | 0xD8 | 0xF8 => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
                match opcode {
                    0x18 => self.status.set(CARRY, false),     // CLC
                    0x38 => self.status.set(CARRY, true),      // SEC
                    0x58 => self.status.set(INTERRUPT, false),  // CLI
                    0x78 => self.status.set(INTERRUPT, true),   // SEI
                    0xB8 => self.status.set(OVERFLOW, false),   // CLV
                    0xD8 => self.status.set(DECIMAL, false),    // CLD
                    0xF8 => self.status.set(DECIMAL, true),     // SED
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // Register transfers (implied, 2 cycles)
            // ==================================================================
            0xAA | 0xA8 | 0x8A | 0x98 | 0xBA | 0x9A => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
                match opcode {
                    0xAA => { self.x = self.a; self.status.set_zn(self.x); }        // TAX
                    0xA8 => { self.y = self.a; self.status.set_zn(self.y); }        // TAY
                    0x8A => { self.a = self.x; self.status.set_zn(self.a); }        // TXA
                    0x98 => { self.a = self.y; self.status.set_zn(self.a); }        // TYA
                    0xBA => { self.x = self.sp; self.status.set_zn(self.x); }       // TSX
                    0x9A => { self.sp = self.x; }                                    // TXS
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // INX/INY/DEX/DEY (implied, 2 cycles)
            // ==================================================================
            0xE8 | 0xC8 | 0xCA | 0x88 => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
                match opcode {
                    0xE8 => { self.x = self.x.wrapping_add(1); self.status.set_zn(self.x); } // INX
                    0xC8 => { self.y = self.y.wrapping_add(1); self.status.set_zn(self.y); } // INY
                    0xCA => { self.x = self.x.wrapping_sub(1); self.status.set_zn(self.x); } // DEX
                    0x88 => { self.y = self.y.wrapping_sub(1); self.status.set_zn(self.y); } // DEY
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // ASL/LSR/ROL/ROR accumulator (implied, 2 cycles)
            // ==================================================================
            0x0A | 0x4A | 0x2A | 0x6A => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
                match opcode {
                    0x0A => { self.a = self.asl_val(self.a); }
                    0x4A => { self.a = self.lsr_val(self.a); }
                    0x2A => { self.a = self.rol_val(self.a); }
                    0x6A => { self.a = self.ror_val(self.a); }
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // Immediate mode (2 cycles): ADC, AND, CMP, CPX, CPY, EOR, LDA,
            //   LDX, LDY, ORA, SBC
            // ==================================================================
            0x69 | 0x29 | 0xC9 | 0xE0 | 0xC0 | 0x49 | 0xA9 |
            0xA2 | 0xA0 | 0x09 | 0xE9 => {
                // Cycle 2: fetch operand
                let val = memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                match opcode {
                    0x69 => self.adc(val),
                    0xE9 => self.sbc(val),
                    0x29 => { self.a &= val; self.status.set_zn(self.a); }
                    0x09 => { self.a |= val; self.status.set_zn(self.a); }
                    0x49 => { self.a ^= val; self.status.set_zn(self.a); }
                    0xC9 => self.compare(self.a, val),
                    0xE0 => self.compare(self.x, val),
                    0xC0 => self.compare(self.y, val),
                    0xA9 => { self.a = val; self.status.set_zn(self.a); }
                    0xA2 => { self.x = val; self.status.set_zn(self.x); }
                    0xA0 => { self.y = val; self.status.set_zn(self.y); }
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // Zero Page read (3 cycles): LDA, LDX, LDY, ADC, AND, CMP, CPX,
            //   CPY, EOR, ORA, SBC, BIT
            // ==================================================================
            0xA5 | 0xA6 | 0xA4 | 0x65 | 0x25 | 0xC5 | 0xE4 |
            0xC4 | 0x45 | 0x05 | 0xE5 | 0x24 => {
                // Cycle 2: fetch ZP address
                let addr = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: read from ZP
                let val = memory.read(addr);
                cycles += 1;
                match opcode {
                    0xA5 => { self.a = val; self.status.set_zn(self.a); }
                    0xA6 => { self.x = val; self.status.set_zn(self.x); }
                    0xA4 => { self.y = val; self.status.set_zn(self.y); }
                    0x65 => self.adc(val),
                    0xE5 => self.sbc(val),
                    0x25 => { self.a &= val; self.status.set_zn(self.a); }
                    0x05 => { self.a |= val; self.status.set_zn(self.a); }
                    0x45 => { self.a ^= val; self.status.set_zn(self.a); }
                    0xC5 => self.compare(self.a, val),
                    0xE4 => self.compare(self.x, val),
                    0xC4 => self.compare(self.y, val),
                    0x24 => {
                        // BIT
                        self.status.set(ZERO, (self.a & val) == 0);
                        self.status.set(OVERFLOW, val & 0x40 != 0);
                        self.status.set(NEGATIVE, val & 0x80 != 0);
                    }
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // Zero Page write (3 cycles): STA, STX, STY
            // ==================================================================
            0x85 | 0x86 | 0x84 => {
                // Cycle 2: fetch ZP address
                let addr = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: write to ZP
                let val = match opcode {
                    0x85 => self.a,
                    0x86 => self.x,
                    0x84 => self.y,
                    _ => unreachable!(),
                };
                memory.write(addr, val);
                cycles += 1;
            }

            // ==================================================================
            // Zero Page,X read (4 cycles): LDA, LDY, ADC, AND, CMP, EOR, ORA, SBC
            // ==================================================================
            0xB5 | 0xB4 | 0x75 | 0x35 | 0xD5 | 0x55 | 0x15 | 0xF5 => {
                // Cycle 2: fetch ZP base
                let base = memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: phantom read of base (before adding X)
                memory.read(base as u16);
                cycles += 1;
                // Cycle 4: read from (base+X) & 0xFF
                let addr = base.wrapping_add(self.x) as u16;
                let val = memory.read(addr);
                cycles += 1;
                match opcode {
                    0xB5 => { self.a = val; self.status.set_zn(self.a); }
                    0xB4 => { self.y = val; self.status.set_zn(self.y); }
                    0x75 => self.adc(val),
                    0xF5 => self.sbc(val),
                    0x35 => { self.a &= val; self.status.set_zn(self.a); }
                    0x15 => { self.a |= val; self.status.set_zn(self.a); }
                    0x55 => { self.a ^= val; self.status.set_zn(self.a); }
                    0xD5 => self.compare(self.a, val),
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // Zero Page,X write (4 cycles): STA, STY
            // ==================================================================
            0x95 | 0x94 => {
                // Cycle 2: fetch ZP base
                let base = memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: phantom read of base
                memory.read(base as u16);
                cycles += 1;
                // Cycle 4: write to (base+X) & 0xFF
                let addr = base.wrapping_add(self.x) as u16;
                let val = match opcode {
                    0x95 => self.a,
                    0x94 => self.y,
                    _ => unreachable!(),
                };
                memory.write(addr, val);
                cycles += 1;
            }

            // ==================================================================
            // Zero Page,Y read (4 cycles): LDX
            // ==================================================================
            0xB6 => {
                // Cycle 2: fetch ZP base
                let base = memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: phantom read of base
                memory.read(base as u16);
                cycles += 1;
                // Cycle 4: read from (base+Y) & 0xFF
                let addr = base.wrapping_add(self.y) as u16;
                let val = memory.read(addr);
                cycles += 1;
                self.x = val;
                self.status.set_zn(self.x);
            }

            // ==================================================================
            // Zero Page,Y write (4 cycles): STX
            // ==================================================================
            0x96 => {
                let base = memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                memory.read(base as u16);
                cycles += 1;
                let addr = base.wrapping_add(self.y) as u16;
                memory.write(addr, self.x);
                cycles += 1;
            }

            // ==================================================================
            // Zero Page RMW (5 cycles): ASL, LSR, ROL, ROR, INC, DEC
            // ==================================================================
            0x06 | 0x46 | 0x26 | 0x66 | 0xE6 | 0xC6 => {
                // Cycle 2: fetch ZP address
                let addr = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: read value
                let val = memory.read(addr);
                cycles += 1;
                // Cycle 4: phantom write of original value
                memory.write(addr, val);
                cycles += 1;
                // Cycle 5: write new value
                let new_val = match opcode {
                    0x06 => self.asl_val(val),
                    0x46 => self.lsr_val(val),
                    0x26 => self.rol_val(val),
                    0x66 => self.ror_val(val),
                    0xE6 => { let r = val.wrapping_add(1); self.status.set_zn(r); r }
                    0xC6 => { let r = val.wrapping_sub(1); self.status.set_zn(r); r }
                    _ => unreachable!(),
                };
                memory.write(addr, new_val);
                cycles += 1;
            }

            // ==================================================================
            // Zero Page,X RMW (6 cycles): ASL, LSR, ROL, ROR, INC, DEC
            // ==================================================================
            0x16 | 0x56 | 0x36 | 0x76 | 0xF6 | 0xD6 => {
                // Cycle 2: fetch ZP base
                let base = memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: phantom read of base
                memory.read(base as u16);
                cycles += 1;
                // Cycle 4: read value from (base+X)&FF
                let addr = base.wrapping_add(self.x) as u16;
                let val = memory.read(addr);
                cycles += 1;
                // Cycle 5: phantom write of original value
                memory.write(addr, val);
                cycles += 1;
                // Cycle 6: write new value
                let new_val = match opcode {
                    0x16 => self.asl_val(val),
                    0x56 => self.lsr_val(val),
                    0x36 => self.rol_val(val),
                    0x76 => self.ror_val(val),
                    0xF6 => { let r = val.wrapping_add(1); self.status.set_zn(r); r }
                    0xD6 => { let r = val.wrapping_sub(1); self.status.set_zn(r); r }
                    _ => unreachable!(),
                };
                memory.write(addr, new_val);
                cycles += 1;
            }

            // ==================================================================
            // Absolute read (4 cycles): LDA, LDX, LDY, ADC, AND, CMP, CPX,
            //   CPY, EOR, ORA, SBC, BIT
            // ==================================================================
            0xAD | 0xAE | 0xAC | 0x6D | 0x2D | 0xCD | 0xEC |
            0xCC | 0x4D | 0x0D | 0xED | 0x2C => {
                // Cycle 2: fetch addr_lo
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: fetch addr_hi
                let hi = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let addr = (hi << 8) | lo;
                // Cycle 4: read from addr
                let val = memory.read(addr);
                cycles += 1;
                match opcode {
                    0xAD => { self.a = val; self.status.set_zn(self.a); }
                    0xAE => { self.x = val; self.status.set_zn(self.x); }
                    0xAC => { self.y = val; self.status.set_zn(self.y); }
                    0x6D => self.adc(val),
                    0xED => self.sbc(val),
                    0x2D => { self.a &= val; self.status.set_zn(self.a); }
                    0x0D => { self.a |= val; self.status.set_zn(self.a); }
                    0x4D => { self.a ^= val; self.status.set_zn(self.a); }
                    0xCD => self.compare(self.a, val),
                    0xEC => self.compare(self.x, val),
                    0xCC => self.compare(self.y, val),
                    0x2C => {
                        self.status.set(ZERO, (self.a & val) == 0);
                        self.status.set(OVERFLOW, val & 0x40 != 0);
                        self.status.set(NEGATIVE, val & 0x80 != 0);
                    }
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // Absolute write (4 cycles): STA, STX, STY
            // ==================================================================
            0x8D | 0x8E | 0x8C => {
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let hi = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let addr = (hi << 8) | lo;
                let val = match opcode {
                    0x8D => self.a,
                    0x8E => self.x,
                    0x8C => self.y,
                    _ => unreachable!(),
                };
                memory.write(addr, val);
                cycles += 1;
            }

            // ==================================================================
            // Absolute RMW (6 cycles): ASL, LSR, ROL, ROR, INC, DEC
            // ==================================================================
            0x0E | 0x4E | 0x2E | 0x6E | 0xEE | 0xCE => {
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let hi = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let addr = (hi << 8) | lo;
                // Cycle 4: read value
                let val = memory.read(addr);
                cycles += 1;
                // Cycle 5: phantom write original
                memory.write(addr, val);
                cycles += 1;
                // Cycle 6: write new
                let new_val = match opcode {
                    0x0E => self.asl_val(val),
                    0x4E => self.lsr_val(val),
                    0x2E => self.rol_val(val),
                    0x6E => self.ror_val(val),
                    0xEE => { let r = val.wrapping_add(1); self.status.set_zn(r); r }
                    0xCE => { let r = val.wrapping_sub(1); self.status.set_zn(r); r }
                    _ => unreachable!(),
                };
                memory.write(addr, new_val);
                cycles += 1;
            }

            // ==================================================================
            // Absolute,X read (4+1 cycles): LDA, LDY, ADC, AND, CMP, EOR, ORA, SBC
            // ==================================================================
            0xBD | 0xBC | 0x7D | 0x3D | 0xDD | 0x5D | 0x1D | 0xFD => {
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let hi = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let base = (hi << 8) | lo;
                let effective = base.wrapping_add(self.x as u16);
                let page_crossed = (base & 0xFF00) != (effective & 0xFF00);
                if page_crossed {
                    // Cycle 4: phantom read with wrong high byte
                    let wrong_addr = (base & 0xFF00) | (effective & 0x00FF);
                    memory.read(wrong_addr);
                    cycles += 1;
                }
                // Cycle 4 or 5: real read
                let val = memory.read(effective);
                cycles += 1;
                match opcode {
                    0xBD => { self.a = val; self.status.set_zn(self.a); }
                    0xBC => { self.y = val; self.status.set_zn(self.y); }
                    0x7D => self.adc(val),
                    0xFD => self.sbc(val),
                    0x3D => { self.a &= val; self.status.set_zn(self.a); }
                    0x1D => { self.a |= val; self.status.set_zn(self.a); }
                    0x5D => { self.a ^= val; self.status.set_zn(self.a); }
                    0xDD => self.compare(self.a, val),
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // Absolute,X write (5 cycles always): STA
            // ==================================================================
            0x9D => {
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let hi = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let base = (hi << 8) | lo;
                let effective = base.wrapping_add(self.x as u16);
                // Cycle 4: phantom read (fixup high byte)
                let wrong_addr = (base & 0xFF00) | (effective & 0x00FF);
                memory.read(wrong_addr);
                cycles += 1;
                // Cycle 5: write
                memory.write(effective, self.a);
                cycles += 1;
            }

            // ==================================================================
            // Absolute,X RMW (7 cycles): ASL, LSR, ROL, ROR, INC, DEC
            // ==================================================================
            0x1E | 0x5E | 0x3E | 0x7E | 0xFE | 0xDE => {
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let hi = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let base = (hi << 8) | lo;
                let effective = base.wrapping_add(self.x as u16);
                // Cycle 4: phantom read (fixup) - always happens for RMW
                let wrong_addr = (base & 0xFF00) | (effective & 0x00FF);
                memory.read(wrong_addr);
                cycles += 1;
                // Cycle 5: read value
                let val = memory.read(effective);
                cycles += 1;
                // Cycle 6: phantom write original
                memory.write(effective, val);
                cycles += 1;
                // Cycle 7: write new
                let new_val = match opcode {
                    0x1E => self.asl_val(val),
                    0x5E => self.lsr_val(val),
                    0x3E => self.rol_val(val),
                    0x7E => self.ror_val(val),
                    0xFE => { let r = val.wrapping_add(1); self.status.set_zn(r); r }
                    0xDE => { let r = val.wrapping_sub(1); self.status.set_zn(r); r }
                    _ => unreachable!(),
                };
                memory.write(effective, new_val);
                cycles += 1;
            }

            // ==================================================================
            // Absolute,Y read (4+1 cycles): LDA, LDX, ADC, AND, CMP, EOR, ORA, SBC
            // ==================================================================
            0xB9 | 0xBE | 0x79 | 0x39 | 0xD9 | 0x59 | 0x19 | 0xF9 => {
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let hi = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let base = (hi << 8) | lo;
                let effective = base.wrapping_add(self.y as u16);
                let page_crossed = (base & 0xFF00) != (effective & 0xFF00);
                if page_crossed {
                    let wrong_addr = (base & 0xFF00) | (effective & 0x00FF);
                    memory.read(wrong_addr);
                    cycles += 1;
                }
                let val = memory.read(effective);
                cycles += 1;
                match opcode {
                    0xB9 => { self.a = val; self.status.set_zn(self.a); }
                    0xBE => { self.x = val; self.status.set_zn(self.x); }
                    0x79 => self.adc(val),
                    0xF9 => self.sbc(val),
                    0x39 => { self.a &= val; self.status.set_zn(self.a); }
                    0x19 => { self.a |= val; self.status.set_zn(self.a); }
                    0x59 => { self.a ^= val; self.status.set_zn(self.a); }
                    0xD9 => self.compare(self.a, val),
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // Absolute,Y write (5 cycles always): STA
            // ==================================================================
            0x99 => {
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let hi = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let base = (hi << 8) | lo;
                let effective = base.wrapping_add(self.y as u16);
                let wrong_addr = (base & 0xFF00) | (effective & 0x00FF);
                memory.read(wrong_addr);
                cycles += 1;
                memory.write(effective, self.a);
                cycles += 1;
            }

            // ==================================================================
            // Indirect,X read (6 cycles): LDA, ADC, AND, CMP, EOR, ORA, SBC
            // ==================================================================
            0xA1 | 0x61 | 0x21 | 0xC1 | 0x41 | 0x01 | 0xE1 => {
                // Cycle 2: fetch ZP base
                let base = memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: phantom read of ZP base (before adding X)
                memory.read(base as u16);
                cycles += 1;
                // Cycle 4: read ptr_lo from (base+X) & FF
                let ptr = base.wrapping_add(self.x);
                let lo = memory.read(ptr as u16) as u16;
                cycles += 1;
                // Cycle 5: read ptr_hi from (base+X+1) & FF
                let hi = memory.read(ptr.wrapping_add(1) as u16) as u16;
                cycles += 1;
                let addr = (hi << 8) | lo;
                // Cycle 6: read from target
                let val = memory.read(addr);
                cycles += 1;
                match opcode {
                    0xA1 => { self.a = val; self.status.set_zn(self.a); }
                    0x61 => self.adc(val),
                    0xE1 => self.sbc(val),
                    0x21 => { self.a &= val; self.status.set_zn(self.a); }
                    0x01 => { self.a |= val; self.status.set_zn(self.a); }
                    0x41 => { self.a ^= val; self.status.set_zn(self.a); }
                    0xC1 => self.compare(self.a, val),
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // Indirect,X write (6 cycles): STA
            // ==================================================================
            0x81 => {
                let base = memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                memory.read(base as u16);
                cycles += 1;
                let ptr = base.wrapping_add(self.x);
                let lo = memory.read(ptr as u16) as u16;
                cycles += 1;
                let hi = memory.read(ptr.wrapping_add(1) as u16) as u16;
                cycles += 1;
                let addr = (hi << 8) | lo;
                memory.write(addr, self.a);
                cycles += 1;
            }

            // ==================================================================
            // Indirect,Y read (5+1 cycles): LDA, ADC, AND, CMP, EOR, ORA, SBC
            // ==================================================================
            0xB1 | 0x71 | 0x31 | 0xD1 | 0x51 | 0x11 | 0xF1 => {
                // Cycle 2: fetch ZP pointer
                let zp = memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: read ptr_lo
                let lo = memory.read(zp as u16) as u16;
                cycles += 1;
                // Cycle 4: read ptr_hi
                let hi = memory.read(zp.wrapping_add(1) as u16) as u16;
                cycles += 1;
                let base = (hi << 8) | lo;
                let effective = base.wrapping_add(self.y as u16);
                let page_crossed = (base & 0xFF00) != (effective & 0xFF00);
                if page_crossed {
                    // Cycle 5: phantom read with wrong high byte
                    let wrong_addr = (base & 0xFF00) | (effective & 0x00FF);
                    memory.read(wrong_addr);
                    cycles += 1;
                }
                // Cycle 5 or 6: real read
                let val = memory.read(effective);
                cycles += 1;
                match opcode {
                    0xB1 => { self.a = val; self.status.set_zn(self.a); }
                    0x71 => self.adc(val),
                    0xF1 => self.sbc(val),
                    0x31 => { self.a &= val; self.status.set_zn(self.a); }
                    0x11 => { self.a |= val; self.status.set_zn(self.a); }
                    0x51 => { self.a ^= val; self.status.set_zn(self.a); }
                    0xD1 => self.compare(self.a, val),
                    _ => unreachable!(),
                }
            }

            // ==================================================================
            // Indirect,Y write (6 cycles always): STA
            // ==================================================================
            0x91 => {
                let zp = memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let lo = memory.read(zp as u16) as u16;
                cycles += 1;
                let hi = memory.read(zp.wrapping_add(1) as u16) as u16;
                cycles += 1;
                let base = (hi << 8) | lo;
                let effective = base.wrapping_add(self.y as u16);
                // Cycle 5: phantom read (fixup) - always for write
                let wrong_addr = (base & 0xFF00) | (effective & 0x00FF);
                memory.read(wrong_addr);
                cycles += 1;
                // Cycle 6: write
                memory.write(effective, self.a);
                cycles += 1;
            }

            // ==================================================================
            // Branch instructions (2/3/4 cycles)
            // ==================================================================
            0x90 | 0xB0 | 0xF0 | 0x30 | 0xD0 | 0x10 | 0x50 | 0x70 => {
                // Cycle 2: fetch offset
                let offset = memory.read(self.pc) as i8;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;

                let taken = match opcode {
                    0x90 => !self.status.get(CARRY),     // BCC
                    0xB0 =>  self.status.get(CARRY),     // BCS
                    0xF0 =>  self.status.get(ZERO),      // BEQ
                    0x30 =>  self.status.get(NEGATIVE),   // BMI
                    0xD0 => !self.status.get(ZERO),      // BNE
                    0x10 => !self.status.get(NEGATIVE),   // BPL
                    0x50 => !self.status.get(OVERFLOW),   // BVC
                    0x70 =>  self.status.get(OVERFLOW),   // BVS
                    _ => unreachable!(),
                };

                if taken {
                    // Cycle 3: phantom read of PC (branch taken)
                    memory.read(self.pc);
                    cycles += 1;

                    let old_pc = self.pc;
                    // Add offset to low byte only first
                    let new_pc = self.pc.wrapping_add(offset as u16);
                    let page_crossed = (old_pc & 0xFF00) != (new_pc & 0xFF00);

                    if page_crossed {
                        // Cycle 4: phantom read with partially-fixed PC
                        // The CPU has fixed the low byte but not the high byte yet
                        let partial_pc = (old_pc & 0xFF00) | (new_pc & 0x00FF);
                        memory.read(partial_pc);
                        cycles += 1;
                    }

                    self.pc = new_pc;
                }
            }

            // ==================================================================
            // JMP absolute (3 cycles)
            // ==================================================================
            0x4C => {
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let hi = memory.read(self.pc) as u16;
                cycles += 1;
                self.pc = (hi << 8) | lo;
            }

            // ==================================================================
            // JMP indirect (5 cycles, with page-boundary bug)
            // ==================================================================
            0x6C => {
                // Cycle 2-3: fetch pointer address
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let hi = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                let ptr = (hi << 8) | lo;
                // Cycle 4: read target_lo from pointer
                let target_lo = memory.read(ptr) as u16;
                cycles += 1;
                // Cycle 5: read target_hi (with page boundary bug)
                let ptr_hi = (ptr & 0xFF00) | ((ptr.wrapping_add(1)) & 0x00FF);
                let target_hi = memory.read(ptr_hi) as u16;
                cycles += 1;
                self.pc = (target_hi << 8) | target_lo;
            }

            // ==================================================================
            // JSR (6 cycles)
            // ==================================================================
            0x20 => {
                // Cycle 2: fetch addr_lo
                let lo = memory.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: phantom read of stack pointer
                memory.read(0x0100 | self.sp as u16);
                cycles += 1;
                // Cycle 4: push return address high byte
                // PC currently points to the hi byte of the JSR operand.
                // We push PC (which is addr of hi byte = last byte of JSR).
                // RTS will add 1 to the pulled address.
                let ret = self.pc;
                self.push(memory, (ret >> 8) as u8);
                cycles += 1;
                // Cycle 5: push return address low byte
                self.push(memory, ret as u8);
                cycles += 1;
                // Cycle 6: fetch addr_hi
                let hi = memory.read(self.pc) as u16;
                cycles += 1;
                self.pc = (hi << 8) | lo;
            }

            // ==================================================================
            // RTS (6 cycles)
            // ==================================================================
            0x60 => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
                // Cycle 3: phantom read of current SP
                memory.read(0x0100 | self.sp as u16);
                cycles += 1;
                // Cycle 4: pull return address low byte
                let ret_lo = self.pull(memory) as u16;
                cycles += 1;
                // Cycle 5: pull return address high byte
                let ret_hi = self.pull(memory) as u16;
                cycles += 1;
                // Cycle 6: phantom read / increment PC
                let ret_addr = (ret_hi << 8) | ret_lo;
                memory.read(ret_addr);
                cycles += 1;
                self.pc = ret_addr.wrapping_add(1);
            }

            // ==================================================================
            // RTI (6 cycles)
            // ==================================================================
            0x40 => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
                // Cycle 3: phantom read of SP
                memory.read(0x0100 | self.sp as u16);
                cycles += 1;
                // Cycle 4: pull status
                let flags = self.pull(memory);
                self.status = Status::from_byte(flags);
                cycles += 1;
                // Cycle 5: pull PC low
                let pc_lo = self.pull(memory) as u16;
                cycles += 1;
                // Cycle 6: pull PC high
                let pc_hi = self.pull(memory) as u16;
                cycles += 1;
                self.pc = (pc_hi << 8) | pc_lo;
            }

            // ==================================================================
            // BRK (7 cycles)
            // ==================================================================
            0x00 => {
                // Cycle 2: phantom read of PC (and advance PC past padding byte)
                memory.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                cycles += 1;
                // Cycle 3: push PC high
                self.push(memory, (self.pc >> 8) as u8);
                cycles += 1;
                // Cycle 4: push PC low
                self.push(memory, self.pc as u8);
                cycles += 1;
                // Cycle 5: push status (with break flag set)
                let flags = self.status.to_byte_with_break();
                self.push(memory, flags);
                cycles += 1;
                // Set interrupt disable
                self.status.set(INTERRUPT, true);
                // Cycle 6: read IRQ vector low
                let vec_lo = memory.read(0xFFFE) as u16;
                cycles += 1;
                // Cycle 7: read IRQ vector high
                let vec_hi = memory.read(0xFFFF) as u16;
                cycles += 1;
                self.pc = (vec_hi << 8) | vec_lo;
            }

            // ==================================================================
            // PHA (3 cycles)
            // ==================================================================
            0x48 => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
                // Cycle 3: write A to stack
                self.push(memory, self.a);
                cycles += 1;
            }

            // ==================================================================
            // PHP (3 cycles)
            // ==================================================================
            0x08 => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
                // Cycle 3: write flags to stack
                let flags = self.status.to_byte_with_break();
                self.push(memory, flags);
                cycles += 1;
            }

            // ==================================================================
            // PLA (4 cycles)
            // ==================================================================
            0x68 => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
                // Cycle 3: phantom read of SP (before increment)
                memory.read(0x0100 | self.sp as u16);
                cycles += 1;
                // Cycle 4: pull A from stack
                self.a = self.pull(memory);
                self.status.set_zn(self.a);
                cycles += 1;
            }

            // ==================================================================
            // PLP (4 cycles)
            // ==================================================================
            0x28 => {
                // Cycle 2: phantom read of PC
                memory.read(self.pc);
                cycles += 1;
                // Cycle 3: phantom read of SP (before increment)
                memory.read(0x0100 | self.sp as u16);
                cycles += 1;
                // Cycle 4: pull flags from stack
                let flags = self.pull(memory);
                self.status = Status::from_byte(flags);
                cycles += 1;
            }

            _ => {
                panic!("illegal opcode: 0x{:02X} at PC=0x{:04X}", opcode, self.pc.wrapping_sub(1));
            }
        }

        self.cycles += cycles as u64;
        cycles
    }
}
