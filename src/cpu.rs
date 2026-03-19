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
    pub fn to_byte(self) -> u8 {
        (self.carry as u8)
            | ((self.zero as u8) << 1)
            | ((self.interrupt_disable as u8) << 2)
            | ((self.decimal as u8) << 3)
            | ((self.break_command as u8) << 4)
            | (1 << 5) // unused, always set
            | ((self.overflow as u8) << 6)
            | ((self.negative as u8) << 7)
    }

    pub fn from_byte(byte: u8) -> Self {
        Self {
            carry: byte & 0x01 != 0,
            zero: byte & 0x02 != 0,
            interrupt_disable: byte & 0x04 != 0,
            decimal: byte & 0x08 != 0,
            break_command: byte & 0x10 != 0,
            overflow: byte & 0x40 != 0,
            negative: byte & 0x80 != 0,
        }
    }
}

pub struct OpCodeDetails {
    pub param_count: u8,
    pub cycle_count: u8,
    pub extra_cycle_on_page_bound_cross: bool,
}

macro_rules! opcodes {
    ($( $(#[$meta:meta])* $variant:ident = $hex:expr => ($pc:expr, $cc:expr, $ec:expr) ),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum OpCode {
            $( $(#[$meta])* $variant = $hex ),*
        }

        impl OpCode {
            pub fn from_u8(byte: u8) -> Option<Self> {
                match byte {
                    $( $hex => Some(Self::$variant), )*
                    _ => None,
                }
            }

            pub fn hex(self) -> u8 {
                self as u8
            }

            pub fn details(self) -> OpCodeDetails {
                match self {
                    $( Self::$variant => OpCodeDetails {
                        param_count: $pc,
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
    /// BRK may be used to replace a two-byte instruction for debugging and the
    /// subsequent RTI will be correct.
    BrkImplied     = 0x00 => (0, 7, false),

    /// ORA (bitwise OR with Accumulator)
    /// Affects Flags: N Z
    OraIndirectX   = 0x01 => (1, 6, false),
    OraZeroPage    = 0x05 => (1, 3, false),
    OraImmediate   = 0x09 => (1, 2, false),
    OraAbsolute    = 0x0D => (2, 4, false),
    OraIndirectY   = 0x11 => (1, 5, true),
    OraZeroPageX   = 0x15 => (1, 4, false),
    OraAbsoluteY   = 0x19 => (2, 4, true),
    OraAbsoluteX   = 0x1D => (2, 4, true),

    /// ASL (Arithmetic Shift Left)
    /// Affects Flags: N Z C
    ///
    /// ASL shifts all bits left one position. 0 is shifted into bit 0 and the
    /// original bit 7 is shifted into the Carry.
    AslZeroPage    = 0x06 => (1, 5, false),
    AslAccumulator = 0x0A => (0, 2, false),
    AslAbsolute    = 0x0E => (2, 6, false),
    AslZeroPageX   = 0x16 => (1, 6, false),
    AslAbsoluteX   = 0x1E => (2, 7, false),

    /// PHP (PusH Processor status) / PLP (PuLl Processor status)
    /// Stack Instructions — implied mode, one byte.
    PhpImplied     = 0x08 => (0, 3, false),
    PlpImplied     = 0x28 => (0, 4, false),

    /// Branch Instructions
    /// Affect Flags: none
    ///
    /// All branches are relative mode and have a length of two bytes.
    /// A branch not taken requires two machine cycles. Add one if the branch
    /// is taken and add one more if the branch crosses a page boundary.
    BplRelative    = 0x10 => (1, 2, true),  // Branch on PLus
    BmiRelative    = 0x30 => (1, 2, true),  // Branch on MInus

    /// Flag (Processor Status) Instructions
    /// These instructions are implied mode, have a length of one byte and
    /// require two machine cycles.
    ClcImplied     = 0x18 => (0, 2, false),  // CLear Carry
    SecImplied     = 0x38 => (0, 2, false),  // SEt Carry

    /// JSR (Jump to SubRoutine)
    /// Affects Flags: none
    ///
    /// JSR pushes the address-1 of the next operation on to the stack before
    /// transferring program control to the following address. Subroutines are
    /// normally terminated by a RTS op code.
    JsrAbsolute    = 0x20 => (2, 6, false),

    /// AND (bitwise AND with accumulator)
    /// Affects Flags: N Z
    AndIndirectX   = 0x21 => (1, 6, false),
    AndZeroPage    = 0x25 => (1, 3, false),
    AndImmediate   = 0x29 => (1, 2, false),
    AndAbsolute    = 0x2D => (2, 4, false),
    AndIndirectY   = 0x31 => (1, 5, true),
    AndZeroPageX   = 0x35 => (1, 4, false),
    AndAbsoluteY   = 0x39 => (2, 4, true),
    AndAbsoluteX   = 0x3D => (2, 4, true),

    /// BIT (test BITs)
    /// Affects Flags: N V Z
    ///
    /// BIT sets the Z flag as though the value in the address tested were ANDed
    /// with the accumulator. The N and V flags are set to match bits 7 and 6
    /// respectively in the value stored at the tested address.
    BitZeroPage    = 0x24 => (1, 3, false),
    BitAbsolute    = 0x2C => (2, 4, false),

    /// ROL (ROtate Left)
    /// Affects Flags: N Z C
    ///
    /// ROL shifts all bits left one position. The Carry is shifted into bit 0
    /// and the original bit 7 is shifted into the Carry.
    RolZeroPage    = 0x26 => (1, 5, false),
    RolAccumulator = 0x2A => (0, 2, false),
    RolAbsolute    = 0x2E => (2, 6, false),
    RolZeroPageX   = 0x36 => (1, 6, false),
    RolAbsoluteX   = 0x3E => (2, 7, false),

    /// RTI (ReTurn from Interrupt)
    /// Affects Flags: all
    ///
    /// RTI retrieves the Processor Status Word (flags) and the Program Counter
    /// from the stack in that order (interrupts push the PC first and then the
    /// PSW). Note that unlike RTS, the return address on the stack is the
    /// actual address rather than the address-1.
    RtiImplied     = 0x40 => (0, 6, false),

    /// EOR (bitwise Exclusive OR)
    /// Affects Flags: N Z
    EorIndirectX   = 0x41 => (1, 6, false),
    EorZeroPage    = 0x45 => (1, 3, false),
    EorImmediate   = 0x49 => (1, 2, false),
    EorAbsolute    = 0x4D => (2, 4, false),
    EorIndirectY   = 0x51 => (1, 5, true),
    EorZeroPageX   = 0x55 => (1, 4, false),
    EorAbsoluteY   = 0x59 => (2, 4, true),
    EorAbsoluteX   = 0x5D => (2, 4, true),

    /// LSR (Logical Shift Right)
    /// Affects Flags: N Z C
    ///
    /// LSR shifts all bits right one position. 0 is shifted into bit 7 and the
    /// original bit 0 is shifted into the Carry.
    LsrZeroPage    = 0x46 => (1, 5, false),
    LsrAccumulator = 0x4A => (0, 2, false),
    LsrAbsolute    = 0x4E => (2, 6, false),
    LsrZeroPageX   = 0x56 => (1, 6, false),
    LsrAbsoluteX   = 0x5E => (2, 7, false),

    /// PHA (PusH Accumulator) / PLA (PuLl Accumulator)
    /// Stack Instructions — implied mode, one byte.
    PhaImplied     = 0x48 => (0, 3, false),
    PlaImplied     = 0x68 => (0, 4, false),

    /// JMP (JuMP)
    /// Affects Flags: none
    ///
    /// JMP transfers program execution to the following address (absolute) or
    /// to the location contained in the following address (indirect). Note that
    /// there is no carry associated with the indirect jump so an indirect jump
    /// must never use a vector beginning on the last byte of a page.
    JmpAbsolute    = 0x4C => (2, 3, false),
    JmpIndirect    = 0x6C => (2, 5, false),

    /// BVC/BVS — Branch on oVerflow Clear / Set
    BvcRelative    = 0x50 => (1, 2, true),
    BvsRelative    = 0x70 => (1, 2, true),

    /// CLI (CLear Interrupt) / SEI (SEt Interrupt)
    CliImplied     = 0x58 => (0, 2, false),
    SeiImplied     = 0x78 => (0, 2, false),

    /// RTS (ReTurn from Subroutine)
    /// Affects Flags: none
    ///
    /// RTS pulls the top two bytes off the stack (low byte first) and transfers
    /// program control to that address+1. It is used to exit a subroutine
    /// invoked via JSR which pushed the address-1.
    RtsImplied     = 0x60 => (0, 6, false),

    /// ADC (ADd with Carry)
    /// Affects Flags: N V Z C
    ///
    /// ADC results are dependant on the setting of the decimal flag. In decimal
    /// mode, addition is carried out on the assumption that the values involved
    /// are packed BCD (Binary Coded Decimal).
    /// There is no way to add without carry.
    AdcIndirectX   = 0x61 => (1, 6, false),
    AdcZeroPage    = 0x65 => (1, 3, false),
    AdcImmediate   = 0x69 => (1, 2, false),
    AdcAbsolute    = 0x6D => (2, 4, false),
    AdcIndirectY   = 0x71 => (1, 5, true),
    AdcZeroPageX   = 0x75 => (1, 4, false),
    AdcAbsoluteY   = 0x79 => (2, 4, true),
    AdcAbsoluteX   = 0x7D => (2, 4, true),

    /// ROR (ROtate Right)
    /// Affects Flags: N Z C
    ///
    /// ROR shifts all bits right one position. The Carry is shifted into bit 7
    /// and the original bit 0 is shifted into the Carry.
    RorZeroPage    = 0x66 => (1, 5, false),
    RorAccumulator = 0x6A => (0, 2, false),
    RorAbsolute    = 0x6E => (2, 6, false),
    RorZeroPageX   = 0x76 => (1, 6, false),
    RorAbsoluteX   = 0x7E => (2, 7, false),

    /// STA (STore Accumulator)
    /// Affects Flags: none
    StaIndirectX   = 0x81 => (1, 6, false),
    StaZeroPage    = 0x85 => (1, 3, false),
    StaAbsolute    = 0x8D => (2, 4, false),
    StaIndirectY   = 0x91 => (1, 6, false),
    StaZeroPageX   = 0x95 => (1, 4, false),
    StaAbsoluteY   = 0x99 => (2, 5, false),
    StaAbsoluteX   = 0x9D => (2, 5, false),

    /// STX (STore X register)
    /// Affects Flags: none
    StxZeroPage    = 0x86 => (1, 3, false),
    StxZeroPageY   = 0x96 => (1, 4, false),
    StxAbsolute    = 0x8E => (2, 4, false),

    /// STY (STore Y register)
    /// Affects Flags: none
    StyZeroPage    = 0x84 => (1, 3, false),
    StyZeroPageX   = 0x94 => (1, 4, false),
    StyAbsolute    = 0x8C => (2, 4, false),

    /// Register Instructions
    /// Affect Flags: N Z
    ///
    /// These instructions are implied mode, have a length of one byte and
    /// require two machine cycles.
    DeyImplied     = 0x88 => (0, 2, false),  // DEcrement Y
    TayImplied     = 0xA8 => (0, 2, false),  // Transfer A to Y
    TxaImplied     = 0x8A => (0, 2, false),  // Transfer X to A
    TaxImplied     = 0xAA => (0, 2, false),  // Transfer A to X

    /// BCC/BCS — Branch on Carry Clear / Set
    BccRelative    = 0x90 => (1, 2, true),
    BcsRelative    = 0xB0 => (1, 2, true),

    /// TYA (Transfer Y to A)
    TyaImplied     = 0x98 => (0, 2, false),

    /// Stack Instructions
    ///
    /// These instructions are implied mode, have a length of one byte.
    /// With the 6502, the stack is always on page one ($100-$1FF) and works
    /// top down.
    TxsImplied     = 0x9A => (0, 2, false),  // Transfer X to Stack ptr
    TsxImplied     = 0xBA => (0, 2, false),  // Transfer Stack ptr to X

    /// LDA (LoaD Accumulator)
    /// Affects Flags: N Z
    LdaIndirectX   = 0xA1 => (1, 6, false),
    LdaZeroPage    = 0xA5 => (1, 3, false),
    LdaImmediate   = 0xA9 => (1, 2, false),
    LdaAbsolute    = 0xAD => (2, 4, false),
    LdaIndirectY   = 0xB1 => (1, 5, true),
    LdaZeroPageX   = 0xB5 => (1, 4, false),
    LdaAbsoluteY   = 0xB9 => (2, 4, true),
    LdaAbsoluteX   = 0xBD => (2, 4, true),

    /// LDX (LoaD X register)
    /// Affects Flags: N Z
    LdxImmediate   = 0xA2 => (1, 2, false),
    LdxZeroPage    = 0xA6 => (1, 3, false),
    LdxAbsolute    = 0xAE => (2, 4, false),
    LdxZeroPageY   = 0xB6 => (1, 4, false),
    LdxAbsoluteY   = 0xBE => (2, 4, true),

    /// LDY (LoaD Y register)
    /// Affects Flags: N Z
    LdyImmediate   = 0xA0 => (1, 2, false),
    LdyZeroPage    = 0xA4 => (1, 3, false),
    LdyAbsolute    = 0xAC => (2, 4, false),
    LdyZeroPageX   = 0xB4 => (1, 4, false),
    LdyAbsoluteX   = 0xBC => (2, 4, true),

    /// CLV (CLear oVerflow)
    ClvImplied     = 0xB8 => (0, 2, false),

    /// CPY (ComPare Y register)
    /// Affects Flags: N Z C
    ///
    /// Operation and flag results are identical to equivalent mode CMP ops.
    CpyImmediate   = 0xC0 => (1, 2, false),
    CpyZeroPage    = 0xC4 => (1, 3, false),
    CpyAbsolute    = 0xCC => (2, 4, false),

    /// CMP (CoMPare accumulator)
    /// Affects Flags: N Z C
    ///
    /// Compare sets flags as if a subtraction had been carried out. If the value
    /// in the accumulator is equal or greater than the compared value, the Carry
    /// will be set. The equal (Z) and negative (N) flags will be set based on
    /// equality or lack thereof and the sign (i.e. A>=0x80) of the accumulator.
    CmpIndirectX   = 0xC1 => (1, 6, false),
    CmpZeroPage    = 0xC5 => (1, 3, false),
    CmpImmediate   = 0xC9 => (1, 2, false),
    CmpAbsolute    = 0xCD => (2, 4, false),
    CmpIndirectY   = 0xD1 => (1, 5, true),
    CmpZeroPageX   = 0xD5 => (1, 4, false),
    CmpAbsoluteY   = 0xD9 => (2, 4, true),
    CmpAbsoluteX   = 0xDD => (2, 4, true),

    /// DEC (DECrement memory)
    /// Affects Flags: N Z
    DecZeroPage    = 0xC6 => (1, 5, false),
    DecAbsolute    = 0xCE => (2, 6, false),
    DecZeroPageX   = 0xD6 => (1, 6, false),
    DecAbsoluteX   = 0xDE => (2, 7, false),

    /// INY (INcrement Y) / INX (INcrement X)
    InyImplied     = 0xC8 => (0, 2, false),
    InxImplied     = 0xE8 => (0, 2, false),

    /// DEX (DEcrement X)
    DexImplied     = 0xCA => (0, 2, false),

    /// BNE/BEQ — Branch on Not Equal / EQual
    BneRelative    = 0xD0 => (1, 2, true),
    BeqRelative    = 0xF0 => (1, 2, true),

    /// CLD (CLear Decimal) / SED (SEt Decimal)
    CldImplied     = 0xD8 => (0, 2, false),
    SedImplied     = 0xF8 => (0, 2, false),

    /// CPX (ComPare X register)
    /// Affects Flags: N Z C
    ///
    /// Operation and flag results are identical to equivalent mode CMP ops.
    CpxImmediate   = 0xE0 => (1, 2, false),
    CpxZeroPage    = 0xE4 => (1, 3, false),
    CpxAbsolute    = 0xEC => (2, 4, false),

    /// SBC (SuBtract with Carry)
    /// Affects Flags: N V Z C
    ///
    /// SBC results are dependant on the setting of the decimal flag. In decimal
    /// mode, subtraction is carried out on the assumption that the values
    /// involved are packed BCD (Binary Coded Decimal).
    /// There is no way to subtract without the carry which works as an inverse
    /// borrow. i.e, to subtract you set the carry before the operation. If the
    /// carry is cleared by the operation, it indicates a borrow occurred.
    SbcIndirectX   = 0xE1 => (1, 6, false),
    SbcZeroPage    = 0xE5 => (1, 3, false),
    SbcImmediate   = 0xE9 => (1, 2, false),
    SbcAbsolute    = 0xED => (2, 4, false),
    SbcIndirectY   = 0xF1 => (1, 5, true),
    SbcZeroPageX   = 0xF5 => (1, 4, false),
    SbcAbsoluteY   = 0xF9 => (2, 4, true),
    SbcAbsoluteX   = 0xFD => (2, 4, true),

    /// INC (INCrement memory)
    /// Affects Flags: N Z
    IncZeroPage    = 0xE6 => (1, 5, false),
    IncAbsolute    = 0xEE => (2, 6, false),
    IncZeroPageX   = 0xF6 => (1, 6, false),
    IncAbsoluteX   = 0xFE => (2, 7, false),

    /// NOP (No OPeration)
    /// Affects Flags: none
    NopImplied     = 0xEA => (0, 2, false),
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
        let byte = memory.read(self.pc);
        let op = OpCode::from_u8(byte)
            .unwrap_or_else(|| panic!("illegal opcode: 0x{:02X} at PC=0x{:04X}", byte, self.pc));
        let details = op.details();

        // TODO: implement instruction execution
        self.pc += 1 + details.param_count as u16;
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
