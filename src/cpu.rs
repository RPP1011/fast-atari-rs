/// OpCode : http://www.6502.org/tutorials/6502opcodes.html


/// 6502 CPU status flags.
#[derive(Clone, Copy, Debug, Default)]
pub struct StatusFlags {
    pub carry: bool,
    pub zero: bool,
    pub interrupt_disable: bool,
    pub decimal: bool,
    pub break_command: bool,
    pub overflow: bool,
    pub negative: bool,
}

impl StatusFlags {
    pub fn to_Byte(self) -> u8 {
        (self.carry as u8)
            | ((self.zero as u8) << 1)
            | ((self.interrupt_disable as u8) << 2)
            | ((self.decimal as u8) << 3)
            | ((self.break_command as u8) << 4)
            | (1 << 5) // unused, always set
            | ((self.overflow as u8) << 6)
            | ((self.negative as u8) << 7)
    }

    pub fn from_Byte(Byte: u8) -> Self {
        Self {
            carry: Byte & 0x01 != 0,
            zero: Byte & 0x02 != 0,
            interrupt_disable: Byte & 0x04 != 0,
            decimal: Byte & 0x08 != 0,
            break_command: Byte & 0x10 != 0,
            overflow: Byte & 0x40 != 0,
            negative: Byte & 0x80 != 0,
        }
    }
}

pub struct OpCodeDetails {
    pub cycle_count: u8,
    pub extra_cycle_on_page_bound_cross: bool,
}

macro_rules! opcodes {
    // Internal: construct variant value during decode (dispatched by optional operand type)
    (@decode $variant:ident, $mem:expr, $pc:expr) => {
        Self::$variant
    };
    (@decode $variant:ident, u8, $mem:expr, $pc:expr) => {
        Self::$variant($mem.read($pc + 1))
    };
    (@decode $variant:ident, u16, $mem:expr, $pc:expr) => {
        Self::$variant({
            let lo = $mem.read($pc + 1) as u16;
            let hi = $mem.read($pc + 2) as u16;
            (hi << 8) | lo
        })
    };

    // Internal: wildcard pattern for match arms
    (@pat $variant:ident) => { Self::$variant };
    (@pat $variant:ident, u8) => { Self::$variant(_) };
    (@pat $variant:ident, u16) => { Self::$variant(_) };

    // Internal: instruction size in bytes (opcode + operand)
    (@size) => { 1u16 };
    (@size u8) => { 2u16 };
    (@size u16) => { 3u16 };

    // Entry point
    // No operand:   `Foo         = 0xNN => (cycles, extra_cycle)`
    // Byte operand: `Foo(u8)     = 0xNN => (cycles, extra_cycle)`
    // Word operand: `Foo(u16)    = 0xNN => (cycles, extra_cycle)`
    ($( $(#[$meta:meta])* $variant:ident $(($operand:tt))? = $hex:expr => ($cc:expr, $ec:expr) ),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum OpCode {
            $( $(#[$meta])* $variant $(($operand))? ),*
        }

        impl OpCode {
            pub fn decode(memory: &mut dyn Memory, pc: u16) -> Option<Self> {
                let byte = memory.read(pc);
                match byte {
                    $( $hex => Some(opcodes!(@decode $variant, $($operand,)? memory, pc)), )*
                    _ => None,
                }
            }

            pub fn hex(&self) -> u8 {
                match self {
                    $( opcodes!(@pat $variant $(,$operand)?) => $hex, )*
                }
            }

            pub fn size(&self) -> u16 {
                match self {
                    $( opcodes!(@pat $variant $(,$operand)?) => opcodes!(@size $($operand)?), )*
                }
            }

            pub fn details(&self) -> OpCodeDetails {
                match self {
                    $( opcodes!(@pat $variant $(,$operand)?) => OpCodeDetails {
                        cycle_count: $cc,
                        extra_cycle_on_page_bound_cross: $ec,
                    }, )*
                }
            }
        }
    };
}

opcodes! {
    /// BRK (BReaK)
    /// Affects Flags: B
    ///
    /// BRK causes a non-maskable interrupt and increments the program counter
    /// by one. Therefore an RTI will go to the address of the BRK +2 so that
    /// BRK may be used to replace a two-Byte instruction for debugging and the
    /// subsequent RTI will be correct.
    BrkImplied     = 0x00 => (7, false),

    /// ORA (bitwise OR with Accumulator)
    /// Affects Flags: N Z
    OraIndirectX   (u8) = 0x01 => (6, false),
    OraZeroPage    (u8) = 0x05 => (3, false),
    OraImmediate   (u8) = 0x09 => (2, false),
    OraAbsolute    (u16) = 0x0D => (4, false),
    OraIndirectY   (u8) = 0x11 => (5, true),
    OraZeroPageX   (u8) = 0x15 => (4, false),
    OraAbsoluteY   (u16) = 0x19 => (4, true),
    OraAbsoluteX   (u16) = 0x1D => (4, true),

    /// ASL (Arithmetic Shift Left)
    /// Affects Flags: N Z C
    ///
    /// ASL shifts all bits left one position. 0 is shifted into bit 0 and the
    /// original bit 7 is shifted into the Carry.
    AslZeroPage    (u8) = 0x06 => (5, false),
    AslAccumulator = 0x0A => (2, false),
    AslAbsolute    (u16) = 0x0E => (6, false),
    AslZeroPageX   (u8) = 0x16 => (6, false),
    AslAbsoluteX   (u16) = 0x1E => (7, false),

    /// PHP (PusH Processor status) / PLP (PuLl Processor status)
    /// Stack Instructions — implied mode, one Byte.
    PhpImplied     = 0x08 => (3, false),
    PlpImplied     = 0x28 => (4, false),

    /// Branch Instructions
    /// Affect Flags: None
    ///
    /// All branches are relative mode and have a length of two Bytes.
    /// A branch not taken requires two machine cycles. Add one if the branch
    /// is taken and add one more if the branch crosses a page boundary.
    BplRelative    (u8) = 0x10 => (2, true),  // Branch on PLus
    BmiRelative    (u8) = 0x30 => (2, true),  // Branch on MInus

    /// Flag (Processor Status) Instructions
    /// These instructions are implied mode, have a length of one Byte and
    /// require two machine cycles.
    ClcImplied     = 0x18 => (2, false),  // CLear Carry
    SecImplied     = 0x38 => (2, false),  // SEt Carry

    /// JSR (Jump to SubRoutine)
    /// Affects Flags: None
    ///
    /// JSR pushes the address-1 of the next operation on to the stack before
    /// transferring program control to the following address. Subroutines are
    /// normally terminated by a RTS op code.
    JsrAbsolute    (u16) = 0x20 => (6, false),

    /// AND (bitwise AND with accumulator)
    /// Affects Flags: N Z
    AndIndirectX   (u8) = 0x21 => (6, false),
    AndZeroPage    (u8) = 0x25 => (3, false),
    AndImmediate   (u8) = 0x29 => (2, false),
    AndAbsolute    (u16) = 0x2D => (4, false),
    AndIndirectY   (u8) = 0x31 => (5, true),
    AndZeroPageX   (u8) = 0x35 => (4, false),
    AndAbsoluteY   (u16) = 0x39 => (4, true),
    AndAbsoluteX   (u16) = 0x3D => (4, true),

    /// BIT (test BITs)
    /// Affects Flags: N V Z
    ///
    /// BIT sets the Z flag as though the value in the address tested were ANDed
    /// with the accumulator. The N and V flags are set to match bits 7 and 6
    /// respectively in the value stored at the tested address.
    BitZeroPage    (u8) = 0x24 => (3, false),
    BitAbsolute    (u16) = 0x2C => (4, false),

    /// ROL (ROtate Left)
    /// Affects Flags: N Z C
    ///
    /// ROL shifts all bits left one position. The Carry is shifted into bit 0
    /// and the original bit 7 is shifted into the Carry.
    RolZeroPage    (u8) = 0x26 => (5, false),
    RolAccumulator = 0x2A => (2, false),
    RolAbsolute    (u16) = 0x2E => (6, false),
    RolZeroPageX   (u8) = 0x36 => (6, false),
    RolAbsoluteX   (u16) = 0x3E => (7, false),

    /// RTI (ReTurn from Interrupt)
    /// Affects Flags: all
    ///
    /// RTI retrieves the Processor Status Word (flags) and the Program Counter
    /// from the stack in that order (interrupts push the PC first and then the
    /// PSW). Note that unlike RTS, the return address on the stack is the
    /// actual address rather than the address-1.
    RtiImplied     = 0x40 => (6, false),

    /// EOR (bitwise Exclusive OR)
    /// Affects Flags: N Z
    EorIndirectX   (u8) = 0x41 => (6, false),
    EorZeroPage    (u8) = 0x45 => (3, false),
    EorImmediate   (u8) = 0x49 => (2, false),
    EorAbsolute    (u16) = 0x4D => (4, false),
    EorIndirectY   (u8) = 0x51 => (5, true),
    EorZeroPageX   (u8) = 0x55 => (4, false),
    EorAbsoluteY   (u16) = 0x59 => (4, true),
    EorAbsoluteX   (u16) = 0x5D => (4, true),

    /// LSR (Logical Shift Right)
    /// Affects Flags: N Z C
    ///
    /// LSR shifts all bits right one position. 0 is shifted into bit 7 and the
    /// original bit 0 is shifted into the Carry.
    LsrZeroPage    (u8) = 0x46 => (5, false),
    LsrAccumulator = 0x4A => (2, false),
    LsrAbsolute    (u16) = 0x4E => (6, false),
    LsrZeroPageX   (u8) = 0x56 => (6, false),
    LsrAbsoluteX   (u16) = 0x5E => (7, false),

    /// PHA (PusH Accumulator) / PLA (PuLl Accumulator)
    /// Stack Instructions — implied mode, one Byte.
    PhaImplied     = 0x48 => (3, false),
    PlaImplied     = 0x68 => (4, false),

    /// JMP (JuMP)
    /// Affects Flags: None
    ///
    /// JMP transfers program execution to the following address (absolute) or
    /// to the location contained in the following address (indirect). Note that
    /// there is no carry associated with the indirect jump so an indirect jump
    /// must never use a vector beginning on the last Byte of a page.
    JmpAbsolute    (u16) = 0x4C => (3, false),
    JmpIndirect    (u16) = 0x6C => (5, false),

    /// BVC/BVS — Branch on oVerflow Clear / Set
    BvcRelative    (u8) = 0x50 => (2, true),
    BvsRelative    (u8) = 0x70 => (2, true),

    /// CLI (CLear Interrupt) / SEI (SEt Interrupt)
    CliImplied     = 0x58 => (2, false),
    SeiImplied     = 0x78 => (2, false),

    /// RTS (ReTurn from Subroutine)
    /// Affects Flags: None
    ///
    /// RTS pulls the top two Bytes off the stack (low Byte first) and transfers
    /// program control to that address+1. It is used to exit a subroutine
    /// invoked via JSR which pushed the address-1.
    RtsImplied     = 0x60 => (6, false),

    /// ADC (ADd with Carry)
    /// Affects Flags: N V Z C
    ///
    /// ADC results are dependant on the setting of the decimal flag. In decimal
    /// mode, addition is carried out on the assumption that the values involved
    /// are packed BCD (Binary Coded Decimal).
    /// There is no way to add without carry.
    AdcIndirectX   (u8) = 0x61 => (6, false),
    AdcZeroPage    (u8) = 0x65 => (3, false),
    AdcImmediate   (u8) = 0x69 => (2, false),
    AdcAbsolute    (u16) = 0x6D => (4, false),
    AdcIndirectY   (u8) = 0x71 => (5, true),
    AdcZeroPageX   (u8) = 0x75 => (4, false),
    AdcAbsoluteY   (u16) = 0x79 => (4, true),
    AdcAbsoluteX   (u16) = 0x7D => (4, true),

    /// ROR (ROtate Right)
    /// Affects Flags: N Z C
    ///
    /// ROR shifts all bits right one position. The Carry is shifted into bit 7
    /// and the original bit 0 is shifted into the Carry.
    RorZeroPage    (u8) = 0x66 => (5, false),
    RorAccumulator = 0x6A => (2, false),
    RorAbsolute    (u16) = 0x6E => (6, false),
    RorZeroPageX   (u8) = 0x76 => (6, false),
    RorAbsoluteX   (u16) = 0x7E => (7, false),

    /// STA (STore Accumulator)
    /// Affects Flags: None
    StaIndirectX   (u8) = 0x81 => (6, false),
    StaZeroPage    (u8) = 0x85 => (3, false),
    StaAbsolute    (u16) = 0x8D => (4, false),
    StaIndirectY   (u8) = 0x91 => (6, false),
    StaZeroPageX   (u8) = 0x95 => (4, false),
    StaAbsoluteY   (u16) = 0x99 => (5, false),
    StaAbsoluteX   (u16) = 0x9D => (5, false),

    /// STX (STore X register)
    /// Affects Flags: None
    StxZeroPage    (u8) = 0x86 => (3, false),
    StxZeroPageY   (u8) = 0x96 => (4, false),
    StxAbsolute    (u16) = 0x8E => (4, false),

    /// STY (STore Y register)
    /// Affects Flags: None
    StyZeroPage    (u8) = 0x84 => (3, false),
    StyZeroPageX   (u8) = 0x94 => (4, false),
    StyAbsolute    (u16) = 0x8C => (4, false),

    /// Register Instructions
    /// Affect Flags: N Z
    ///
    /// These instructions are implied mode, have a length of one Byte and
    /// require two machine cycles.
    DeyImplied     = 0x88 => (2, false),  // DEcrement Y
    TayImplied     = 0xA8 => (2, false),  // Transfer A to Y
    TxaImplied     = 0x8A => (2, false),  // Transfer X to A
    TaxImplied     = 0xAA => (2, false),  // Transfer A to X

    /// BCC/BCS — Branch on Carry Clear / Set
    BccRelative    (u8) = 0x90 => (2, true),
    BcsRelative    (u8) = 0xB0 => (2, true),

    /// TYA (Transfer Y to A)
    TyaImplied     = 0x98 => (2, false),

    /// Stack Instructions
    ///
    /// These instructions are implied mode, have a length of one Byte.
    /// With the 6502, the stack is always on page one ($100-$1FF) and works
    /// top down.
    TxsImplied     = 0x9A => (2, false),  // Transfer X to Stack ptr
    TsxImplied     = 0xBA => (2, false),  // Transfer Stack ptr to X

    /// LDA (LoaD Accumulator)
    /// Affects Flags: N Z
    LdaIndirectX   (u8) = 0xA1 => (6, false),
    LdaZeroPage    (u8) = 0xA5 => (3, false),
    LdaImmediate   (u8) = 0xA9 => (2, false),
    LdaAbsolute    (u16) = 0xAD => (4, false),
    LdaIndirectY   (u8) = 0xB1 => (5, true),
    LdaZeroPageX   (u8) = 0xB5 => (4, false),
    LdaAbsoluteY   (u16) = 0xB9 => (4, true),
    LdaAbsoluteX   (u16) = 0xBD => (4, true),

    /// LDX (LoaD X register)
    /// Affects Flags: N Z
    LdxImmediate   (u8) = 0xA2 => (2, false),
    LdxZeroPage    (u8) = 0xA6 => (3, false),
    LdxAbsolute    (u16) = 0xAE => (4, false),
    LdxZeroPageY   (u8) = 0xB6 => (4, false),
    LdxAbsoluteY   (u16) = 0xBE => (4, true),

    /// LDY (LoaD Y register)
    /// Affects Flags: N Z
    LdyImmediate   (u8) = 0xA0 => (2, false),
    LdyZeroPage    (u8) = 0xA4 => (3, false),
    LdyAbsolute    (u16) = 0xAC => (4, false),
    LdyZeroPageX   (u8) = 0xB4 => (4, false),
    LdyAbsoluteX   (u16) = 0xBC => (4, true),

    /// CLV (CLear oVerflow)
    ClvImplied     = 0xB8 => (2, false),

    /// CPY (ComPare Y register)
    /// Affects Flags: N Z C
    ///
    /// Operation and flag results are identical to equivalent mode CMP ops.
    CpyImmediate   (u8) = 0xC0 => (2, false),
    CpyZeroPage    (u8) = 0xC4 => (3, false),
    CpyAbsolute    (u16) = 0xCC => (4, false),

    /// CMP (CoMPare accumulator)
    /// Affects Flags: N Z C
    ///
    /// Compare sets flags as if a subtraction had been carried out. If the value
    /// in the accumulator is equal or greater than the compared value, the Carry
    /// will be set. The equal (Z) and negative (N) flags will be set based on
    /// equality or lack thereof and the sign (i.e. A>=0x80) of the accumulator.
    CmpIndirectX   (u8) = 0xC1 => (6, false),
    CmpZeroPage    (u8) = 0xC5 => (3, false),
    CmpImmediate   (u8) = 0xC9 => (2, false),
    CmpAbsolute    (u16) = 0xCD => (4, false),
    CmpIndirectY   (u8) = 0xD1 => (5, true),
    CmpZeroPageX   (u8) = 0xD5 => (4, false),
    CmpAbsoluteY   (u16) = 0xD9 => (4, true),
    CmpAbsoluteX   (u16) = 0xDD => (4, true),

    /// DEC (DECrement memory)
    /// Affects Flags: N Z
    DecZeroPage    (u8) = 0xC6 => (5, false),
    DecAbsolute    (u16) = 0xCE => (6, false),
    DecZeroPageX   (u8) = 0xD6 => (6, false),
    DecAbsoluteX   (u16) = 0xDE => (7, false),

    /// INY (INcrement Y) / INX (INcrement X)
    InyImplied     = 0xC8 => (2, false),
    InxImplied     = 0xE8 => (2, false),

    /// DEX (DEcrement X)
    DexImplied     = 0xCA => (2, false),

    /// BNE/BEQ — Branch on Not Equal / EQual
    BneRelative    (u8) = 0xD0 => (2, true),
    BeqRelative    (u8) = 0xF0 => (2, true),

    /// CLD (CLear Decimal) / SED (SEt Decimal)
    CldImplied     = 0xD8 => (2, false),
    SedImplied     = 0xF8 => (2, false),

    /// CPX (ComPare X register)
    /// Affects Flags: N Z C
    ///
    /// Operation and flag results are identical to equivalent mode CMP ops.
    CpxImmediate   (u8) = 0xE0 => (2, false),
    CpxZeroPage    (u8) = 0xE4 => (3, false),
    CpxAbsolute    (u16) = 0xEC => (4, false),

    /// SBC (SuBtract with Carry)
    /// Affects Flags: N V Z C
    ///
    /// SBC results are dependant on the setting of the decimal flag. In decimal
    /// mode, subtraction is carried out on the assumption that the values
    /// involved are packed BCD (Binary Coded Decimal).
    /// There is no way to subtract without the carry which works as an inverse
    /// borrow. i.e, to subtract you set the carry before the operation. If the
    /// carry is cleared by the operation, it indicates a borrow occurred.
    SbcIndirectX   (u8) = 0xE1 => (6, false),
    SbcZeroPage    (u8) = 0xE5 => (3, false),
    SbcImmediate   (u8) = 0xE9 => (2, false),
    SbcAbsolute    (u16) = 0xED => (4, false),
    SbcIndirectY   (u8) = 0xF1 => (5, true),
    SbcZeroPageX   (u8) = 0xF5 => (4, false),
    SbcAbsoluteY   (u16) = 0xF9 => (4, true),
    SbcAbsoluteX   (u16) = 0xFD => (4, true),

    /// INC (INCrement memory)
    /// Affects Flags: N Z
    IncZeroPage    (u8) = 0xE6 => (5, false),
    IncAbsolute    (u16) = 0xEE => (6, false),
    IncZeroPageX   (u8) = 0xF6 => (6, false),
    IncAbsoluteX   (u16) = 0xFE => (7, false),

    /// NOP (No OPeration)
    /// Affects Flags: None
    NopImplied     = 0xEA => (2, false),
}

/// 6502 CPU state.
#[derive(Clone, Debug)]
pub struct Cpu {
    /// Accumulator
    pub a: u8,
    /// X index register
    pub x: u8,
    /// Y index register
    pub y: u8,
    /// Stack pointer
    pub sp: u8,
    /// Program counter
    pub pc: u16,
    /// Processor status flags
    pub status: StatusFlags,
    /// Total cycles executed
    pub cycles: u64,
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            status: StatusFlags::default(),
            cycles: 0,
        }
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the CPU, reading the reset vector from memory.
    pub fn reset(&mut self, memory: &mut dyn Memory) {
        let lo = memory.read(0xFFFC) as u16;
        let hi = memory.read(0xFFFD) as u16;
        self.pc = (hi << 8) | lo;
        self.sp = 0xFD;
        self.status = StatusFlags::default();
        self.a = 0;
        self.x = 0;
        self.y = 0;
    }

    /// Execute a single instruction. Returns the number of cycles consumed.
    pub fn step(&mut self, memory: &mut dyn Memory) -> u8 {
        let op = OpCode::decode(memory, self.pc)
            .unwrap_or_else(|| panic!("illegal opcode: 0x{:02X} at PC=0x{:04X}", memory.read(self.pc), self.pc));
        let details = op.details();

        match op {
            OpCode::AdcImmediate(val) => {
                self.a = self.a.wrapping_add(val);
            },
            OpCode::AdcAbsolute(val) => {
                self.a = self.a.wrapping_add(memory.read(self.pc));
            },
            OpCode::AdcAbsoluteX(val) => {
                self.a = self.a.wrapping_add(memory.read(self.pc + self.x as u16));
            },
            OpCode::AdcAbsoluteY(val) => {
                self.a = self.a.wrapping_add(memory.read(self.pc + self.y as u16));
            },
            _ => unimplemented!(),
        }

        self.pc += op.size();
        details.cycle_count
    }
}

/// Trait for memory-mapped bus access.
pub trait Memory {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
}

/// Flat 64KB RAM for testing purposes.
pub struct FlatMemory {
    pub ram: [u8; 0x10000],
}

impl Default for FlatMemory {
    fn default() -> Self {
        Self { ram: [0; 0x10000] }
    }
}

impl FlatMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a binary blob at the given base address.
    pub fn load(&mut self, base: u16, data: &[u8]) {
        let start = base as usize;
        self.ram[start..start + data.len()].copy_from_slice(data);
    }
}

impl Memory for FlatMemory {
    fn read(&mut self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.ram[addr as usize] = val;
    }
}
