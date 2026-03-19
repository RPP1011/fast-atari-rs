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
    hex: u8,
    param_count: u8,
    cycle_count: u8,
    extra_cycle_on_page_bound_cross: bool
}

impl OpCodeDetails {
    fn new(hex:u8, pc:u8, cc:u8, ec:bool) -> Self {
        Self { hex:hex, param_count : pc, cycle_count: cc, extra_cycle_on_page_bound_cross: ec }
    }

}

enum OpCode {
    /*
     * ADC (ADd with Carry)
     * Affects Flags: N V Z C
     *
     * ADC results are dependant on the setting of the decimal flag. In decimal
     * mode, addition is carried out on the assumption that the values involved
     * are packed BCD (Binary Coded Decimal).
     * There is no way to add without carry.
     */
    AdcImmediate,
    AdcZeroPage,
    AdcZeroPageX,
    AdcAbsolute,
    AdcAbsoluteX,
    AdcAbsoluteY,
    AdcIndirectX,
    AdcIndirectY,

    /*
     * AND (bitwise AND with accumulator)
     * Affects Flags: N Z
     */
    AndImmediate,
    AndZeroPage,
    AndZeroPageX,
    AndAbsolute,
    AndAbsoluteX,
    AndAbsoluteY,
    AndIndirectX,
    AndIndirectY,

    /*
     * ASL (Arithmetic Shift Left)
     * Affects Flags: N Z C
     *
     * ASL shifts all bits left one position. 0 is shifted into bit 0 and the
     * original bit 7 is shifted into the Carry.
     */
    AslAccumulator,
    AslZeroPage,
    AslZeroPageX,
    AslAbsolute,
    AslAbsoluteX,

    /*
     * BIT (test BITs)
     * Affects Flags: N V Z
     *
     * BIT sets the Z flag as though the value in the address tested were ANDed
     * with the accumulator. The N and V flags are set to match bits 7 and 6
     * respectively in the value stored at the tested address.
     */
    BitZeroPage,
    BitAbsolute,

    /*
     * Branch Instructions
     * Affect Flags: none
     *
     * All branches are relative mode and have a length of two bytes.
     * Branches are dependant on the status of the flag bits when the op code
     * is encountered. A branch not taken requires two machine cycles. Add one
     * if the branch is taken and add one more if the branch crosses a page
     * boundary.
     */
    BplRelative,  // Branch on PLus
    BmiRelative,  // Branch on MInus
    BvcRelative,  // Branch on oVerflow Clear
    BvsRelative,  // Branch on oVerflow Set
    BccRelative,  // Branch on Carry Clear
    BcsRelative,  // Branch on Carry Set
    BneRelative,  // Branch on Not Equal
    BeqRelative,  // Branch on EQual

    /*
     * BRK (BReaK)
     * Affects Flags: B
     *
     * BRK causes a non-maskable interrupt and increments the program counter
     * by one. Therefore an RTI will go to the address of the BRK +2 so that
     * BRK may be used to replace a two-byte instruction for debugging and the
     * subsequent RTI will be correct.
     */
    BrkImplied,

    /*
     * CMP (CoMPare accumulator)
     * Affects Flags: N Z C
     *
     * Compare sets flags as if a subtraction had been carried out. If the value
     * in the accumulator is equal or greater than the compared value, the Carry
     * will be set. The equal (Z) and negative (N) flags will be set based on
     * equality or lack thereof and the sign (i.e. A>=0x80) of the accumulator.
     */
    CmpImmediate,
    CmpZeroPage,
    CmpZeroPageX,
    CmpAbsolute,
    CmpAbsoluteX,
    CmpAbsoluteY,
    CmpIndirectX,
    CmpIndirectY,

    /*
     * CPX (ComPare X register)
     * Affects Flags: N Z C
     *
     * Operation and flag results are identical to equivalent mode CMP ops.
     */
    CpxImmediate,
    CpxZeroPage,
    CpxAbsolute,

    /*
     * CPY (ComPare Y register)
     * Affects Flags: N Z C
     *
     * Operation and flag results are identical to equivalent mode CMP ops.
     */
    CpyImmediate,
    CpyZeroPage,
    CpyAbsolute,

    /*
     * DEC (DECrement memory)
     * Affects Flags: N Z
     */
    DecZeroPage,
    DecZeroPageX,
    DecAbsolute,
    DecAbsoluteX,

    /*
     * EOR (bitwise Exclusive OR)
     * Affects Flags: N Z
     */
    EorImmediate,
    EorZeroPage,
    EorZeroPageX,
    EorAbsolute,
    EorAbsoluteX,
    EorAbsoluteY,
    EorIndirectX,
    EorIndirectY,

    /*
     * Flag (Processor Status) Instructions
     * Affect Flags: as noted
     *
     * These instructions are implied mode, have a length of one byte and
     * require two machine cycles.
     */
    ClcImplied,  // CLear Carry
    SecImplied,  // SEt Carry
    CliImplied,  // CLear Interrupt
    SeiImplied,  // SEt Interrupt
    ClvImplied,  // CLear oVerflow
    CldImplied,  // CLear Decimal
    SedImplied,  // SEt Decimal

    /*
     * INC (INCrement memory)
     * Affects Flags: N Z
     */
    IncZeroPage,
    IncZeroPageX,
    IncAbsolute,
    IncAbsoluteX,

    /*
     * JMP (JuMP)
     * Affects Flags: none
     *
     * JMP transfers program execution to the following address (absolute) or
     * to the location contained in the following address (indirect). Note that
     * there is no carry associated with the indirect jump so an indirect jump
     * must never use a vector beginning on the last byte of a page.
     */
    JmpAbsolute,
    JmpIndirect,

    /*
     * JSR (Jump to SubRoutine)
     * Affects Flags: none
     *
     * JSR pushes the address-1 of the next operation on to the stack before
     * transferring program control to the following address. Subroutines are
     * normally terminated by a RTS op code.
     */
    JsrAbsolute,

    /*
     * LDA (LoaD Accumulator)
     * Affects Flags: N Z
     */
    LdaImmediate,
    LdaZeroPage,
    LdaZeroPageX,
    LdaAbsolute,
    LdaAbsoluteX,
    LdaAbsoluteY,
    LdaIndirectX,
    LdaIndirectY,

    /*
     * LDX (LoaD X register)
     * Affects Flags: N Z
     */
    LdxImmediate,
    LdxZeroPage,
    LdxZeroPageY,
    LdxAbsolute,
    LdxAbsoluteY,

    /*
     * LDY (LoaD Y register)
     * Affects Flags: N Z
     */
    LdyImmediate,
    LdyZeroPage,
    LdyZeroPageX,
    LdyAbsolute,
    LdyAbsoluteX,

    /*
     * LSR (Logical Shift Right)
     * Affects Flags: N Z C
     *
     * LSR shifts all bits right one position. 0 is shifted into bit 7 and the
     * original bit 0 is shifted into the Carry.
     */
    LsrAccumulator,
    LsrZeroPage,
    LsrZeroPageX,
    LsrAbsolute,
    LsrAbsoluteX,

    /*
     * NOP (No OPeration)
     * Affects Flags: none
     *
     * NOP is used to reserve space for future modifications or effectively REM
     * out existing code.
     */
    NopImplied,

    /*
     * ORA (bitwise OR with Accumulator)
     * Affects Flags: N Z
     */
    OraImmediate,
    OraZeroPage,
    OraZeroPageX,
    OraAbsolute,
    OraAbsoluteX,
    OraAbsoluteY,
    OraIndirectX,
    OraIndirectY,

    /*
     * Register Instructions
     * Affect Flags: N Z
     *
     * These instructions are implied mode, have a length of one byte and
     * require two machine cycles.
     */
    TaxImplied,  // Transfer A to X
    TxaImplied,  // Transfer X to A
    DexImplied,  // DEcrement X
    InxImplied,  // INcrement X
    TayImplied,  // Transfer A to Y
    TyaImplied,  // Transfer Y to A
    DeyImplied,  // DEcrement Y
    InyImplied,  // INcrement Y

    /*
     * ROL (ROtate Left)
     * Affects Flags: N Z C
     *
     * ROL shifts all bits left one position. The Carry is shifted into bit 0
     * and the original bit 7 is shifted into the Carry.
     */
    RolAccumulator,
    RolZeroPage,
    RolZeroPageX,
    RolAbsolute,
    RolAbsoluteX,

    /*
     * ROR (ROtate Right)
     * Affects Flags: N Z C
     *
     * ROR shifts all bits right one position. The Carry is shifted into bit 7
     * and the original bit 0 is shifted into the Carry.
     */
    RorAccumulator,
    RorZeroPage,
    RorZeroPageX,
    RorAbsolute,
    RorAbsoluteX,

    /*
     * RTI (ReTurn from Interrupt)
     * Affects Flags: all
     *
     * RTI retrieves the Processor Status Word (flags) and the Program Counter
     * from the stack in that order (interrupts push the PC first and then the
     * PSW). Note that unlike RTS, the return address on the stack is the
     * actual address rather than the address-1.
     */
    RtiImplied,

    /*
     * RTS (ReTurn from Subroutine)
     * Affects Flags: none
     *
     * RTS pulls the top two bytes off the stack (low byte first) and transfers
     * program control to that address+1. It is used to exit a subroutine
     * invoked via JSR which pushed the address-1.
     */
    RtsImplied,

    /*
     * SBC (SuBtract with Carry)
     * Affects Flags: N V Z C
     *
     * SBC results are dependant on the setting of the decimal flag. In decimal
     * mode, subtraction is carried out on the assumption that the values
     * involved are packed BCD (Binary Coded Decimal).
     * There is no way to subtract without the carry which works as an inverse
     * borrow. i.e, to subtract you set the carry before the operation. If the
     * carry is cleared by the operation, it indicates a borrow occurred.
     */
    SbcImmediate,
    SbcZeroPage,
    SbcZeroPageX,
    SbcAbsolute,
    SbcAbsoluteX,
    SbcAbsoluteY,
    SbcIndirectX,
    SbcIndirectY,

    /*
     * STA (STore Accumulator)
     * Affects Flags: none
     */
    StaZeroPage,
    StaZeroPageX,
    StaAbsolute,
    StaAbsoluteX,
    StaAbsoluteY,
    StaIndirectX,
    StaIndirectY,

    /*
     * Stack Instructions
     *
     * These instructions are implied mode, have a length of one byte.
     * With the 6502, the stack is always on page one ($100-$1FF) and works
     * top down.
     */
    TxsImplied,  // Transfer X to Stack ptr
    TsxImplied,  // Transfer Stack ptr to X
    PhaImplied,  // PusH Accumulator
    PlaImplied,  // PuLl Accumulator
    PhpImplied,  // PusH Processor status
    PlpImplied,  // PuLl Processor status

    /*
     * STX (STore X register)
     * Affects Flags: none
     */
    StxZeroPage,
    StxZeroPageY,
    StxAbsolute,

    /*
     * STY (STore Y register)
     * Affects Flags: none
     */
    StyZeroPage,
    StyZeroPageX,
    StyAbsolute,
}

impl OpCode {
    fn details(&self) -> OpCodeDetails {
        match self {
            // ADC
            OpCode::AdcImmediate  => OpCodeDetails::new(0x69, 1, 2, false),
            OpCode::AdcZeroPage   => OpCodeDetails::new(0x65, 1, 3, false),
            OpCode::AdcZeroPageX  => OpCodeDetails::new(0x75, 1, 4, false),
            OpCode::AdcAbsolute   => OpCodeDetails::new(0x6D, 2, 4, false),
            OpCode::AdcAbsoluteX  => OpCodeDetails::new(0x7D, 2, 4, true),
            OpCode::AdcAbsoluteY  => OpCodeDetails::new(0x79, 2, 4, true),
            OpCode::AdcIndirectX  => OpCodeDetails::new(0x61, 1, 6, false),
            OpCode::AdcIndirectY  => OpCodeDetails::new(0x71, 1, 5, true),

            // AND
            OpCode::AndImmediate  => OpCodeDetails::new(0x29, 1, 2, false),
            OpCode::AndZeroPage   => OpCodeDetails::new(0x25, 1, 3, false),
            OpCode::AndZeroPageX  => OpCodeDetails::new(0x35, 1, 4, false),
            OpCode::AndAbsolute   => OpCodeDetails::new(0x2D, 2, 4, false),
            OpCode::AndAbsoluteX  => OpCodeDetails::new(0x3D, 2, 4, true),
            OpCode::AndAbsoluteY  => OpCodeDetails::new(0x39, 2, 4, true),
            OpCode::AndIndirectX  => OpCodeDetails::new(0x21, 1, 6, false),
            OpCode::AndIndirectY  => OpCodeDetails::new(0x31, 1, 5, true),

            // ASL
            OpCode::AslAccumulator => OpCodeDetails::new(0x0A, 0, 2, false),
            OpCode::AslZeroPage    => OpCodeDetails::new(0x06, 1, 5, false),
            OpCode::AslZeroPageX   => OpCodeDetails::new(0x16, 1, 6, false),
            OpCode::AslAbsolute    => OpCodeDetails::new(0x0E, 2, 6, false),
            OpCode::AslAbsoluteX   => OpCodeDetails::new(0x1E, 2, 7, false),

            // BIT
            OpCode::BitZeroPage   => OpCodeDetails::new(0x24, 1, 3, false),
            OpCode::BitAbsolute   => OpCodeDetails::new(0x2C, 2, 4, false),

            // Branches (all relative: 1 param byte, 2 cycles base + 1 if taken + 1 if page cross)
            OpCode::BplRelative   => OpCodeDetails::new(0x10, 1, 2, true),
            OpCode::BmiRelative   => OpCodeDetails::new(0x30, 1, 2, true),
            OpCode::BvcRelative   => OpCodeDetails::new(0x50, 1, 2, true),
            OpCode::BvsRelative   => OpCodeDetails::new(0x70, 1, 2, true),
            OpCode::BccRelative   => OpCodeDetails::new(0x90, 1, 2, true),
            OpCode::BcsRelative   => OpCodeDetails::new(0xB0, 1, 2, true),
            OpCode::BneRelative   => OpCodeDetails::new(0xD0, 1, 2, true),
            OpCode::BeqRelative   => OpCodeDetails::new(0xF0, 1, 2, true),

            // BRK
            OpCode::BrkImplied    => OpCodeDetails::new(0x00, 0, 7, false),

            // CMP
            OpCode::CmpImmediate  => OpCodeDetails::new(0xC9, 1, 2, false),
            OpCode::CmpZeroPage   => OpCodeDetails::new(0xC5, 1, 3, false),
            OpCode::CmpZeroPageX  => OpCodeDetails::new(0xD5, 1, 4, false),
            OpCode::CmpAbsolute   => OpCodeDetails::new(0xCD, 2, 4, false),
            OpCode::CmpAbsoluteX  => OpCodeDetails::new(0xDD, 2, 4, true),
            OpCode::CmpAbsoluteY  => OpCodeDetails::new(0xD9, 2, 4, true),
            OpCode::CmpIndirectX  => OpCodeDetails::new(0xC1, 1, 6, false),
            OpCode::CmpIndirectY  => OpCodeDetails::new(0xD1, 1, 5, true),

            // CPX
            OpCode::CpxImmediate  => OpCodeDetails::new(0xE0, 1, 2, false),
            OpCode::CpxZeroPage   => OpCodeDetails::new(0xE4, 1, 3, false),
            OpCode::CpxAbsolute   => OpCodeDetails::new(0xEC, 2, 4, false),

            // CPY
            OpCode::CpyImmediate  => OpCodeDetails::new(0xC0, 1, 2, false),
            OpCode::CpyZeroPage   => OpCodeDetails::new(0xC4, 1, 3, false),
            OpCode::CpyAbsolute   => OpCodeDetails::new(0xCC, 2, 4, false),

            // DEC
            OpCode::DecZeroPage   => OpCodeDetails::new(0xC6, 1, 5, false),
            OpCode::DecZeroPageX  => OpCodeDetails::new(0xD6, 1, 6, false),
            OpCode::DecAbsolute   => OpCodeDetails::new(0xCE, 2, 6, false),
            OpCode::DecAbsoluteX  => OpCodeDetails::new(0xDE, 2, 7, false),

            // EOR
            OpCode::EorImmediate  => OpCodeDetails::new(0x49, 1, 2, false),
            OpCode::EorZeroPage   => OpCodeDetails::new(0x45, 1, 3, false),
            OpCode::EorZeroPageX  => OpCodeDetails::new(0x55, 1, 4, false),
            OpCode::EorAbsolute   => OpCodeDetails::new(0x4D, 2, 4, false),
            OpCode::EorAbsoluteX  => OpCodeDetails::new(0x5D, 2, 4, true),
            OpCode::EorAbsoluteY  => OpCodeDetails::new(0x59, 2, 4, true),
            OpCode::EorIndirectX  => OpCodeDetails::new(0x41, 1, 6, false),
            OpCode::EorIndirectY  => OpCodeDetails::new(0x51, 1, 5, true),

            // Flag instructions
            OpCode::ClcImplied    => OpCodeDetails::new(0x18, 0, 2, false),
            OpCode::SecImplied    => OpCodeDetails::new(0x38, 0, 2, false),
            OpCode::CliImplied    => OpCodeDetails::new(0x58, 0, 2, false),
            OpCode::SeiImplied    => OpCodeDetails::new(0x78, 0, 2, false),
            OpCode::ClvImplied    => OpCodeDetails::new(0xB8, 0, 2, false),
            OpCode::CldImplied    => OpCodeDetails::new(0xD8, 0, 2, false),
            OpCode::SedImplied    => OpCodeDetails::new(0xF8, 0, 2, false),

            // INC
            OpCode::IncZeroPage   => OpCodeDetails::new(0xE6, 1, 5, false),
            OpCode::IncZeroPageX  => OpCodeDetails::new(0xF6, 1, 6, false),
            OpCode::IncAbsolute   => OpCodeDetails::new(0xEE, 2, 6, false),
            OpCode::IncAbsoluteX  => OpCodeDetails::new(0xFE, 2, 7, false),

            // JMP
            OpCode::JmpAbsolute   => OpCodeDetails::new(0x4C, 2, 3, false),
            OpCode::JmpIndirect   => OpCodeDetails::new(0x6C, 2, 5, false),

            // JSR
            OpCode::JsrAbsolute   => OpCodeDetails::new(0x20, 2, 6, false),

            // LDA
            OpCode::LdaImmediate  => OpCodeDetails::new(0xA9, 1, 2, false),
            OpCode::LdaZeroPage   => OpCodeDetails::new(0xA5, 1, 3, false),
            OpCode::LdaZeroPageX  => OpCodeDetails::new(0xB5, 1, 4, false),
            OpCode::LdaAbsolute   => OpCodeDetails::new(0xAD, 2, 4, false),
            OpCode::LdaAbsoluteX  => OpCodeDetails::new(0xBD, 2, 4, true),
            OpCode::LdaAbsoluteY  => OpCodeDetails::new(0xB9, 2, 4, true),
            OpCode::LdaIndirectX  => OpCodeDetails::new(0xA1, 1, 6, false),
            OpCode::LdaIndirectY  => OpCodeDetails::new(0xB1, 1, 5, true),

            // LDX
            OpCode::LdxImmediate  => OpCodeDetails::new(0xA2, 1, 2, false),
            OpCode::LdxZeroPage   => OpCodeDetails::new(0xA6, 1, 3, false),
            OpCode::LdxZeroPageY  => OpCodeDetails::new(0xB6, 1, 4, false),
            OpCode::LdxAbsolute   => OpCodeDetails::new(0xAE, 2, 4, false),
            OpCode::LdxAbsoluteY  => OpCodeDetails::new(0xBE, 2, 4, true),

            // LDY
            OpCode::LdyImmediate  => OpCodeDetails::new(0xA0, 1, 2, false),
            OpCode::LdyZeroPage   => OpCodeDetails::new(0xA4, 1, 3, false),
            OpCode::LdyZeroPageX  => OpCodeDetails::new(0xB4, 1, 4, false),
            OpCode::LdyAbsolute   => OpCodeDetails::new(0xAC, 2, 4, false),
            OpCode::LdyAbsoluteX  => OpCodeDetails::new(0xBC, 2, 4, true),

            // LSR
            OpCode::LsrAccumulator => OpCodeDetails::new(0x4A, 0, 2, false),
            OpCode::LsrZeroPage    => OpCodeDetails::new(0x46, 1, 5, false),
            OpCode::LsrZeroPageX   => OpCodeDetails::new(0x56, 1, 6, false),
            OpCode::LsrAbsolute    => OpCodeDetails::new(0x4E, 2, 6, false),
            OpCode::LsrAbsoluteX   => OpCodeDetails::new(0x5E, 2, 7, false),

            // NOP
            OpCode::NopImplied    => OpCodeDetails::new(0xEA, 0, 2, false),

            // ORA
            OpCode::OraImmediate  => OpCodeDetails::new(0x09, 1, 2, false),
            OpCode::OraZeroPage   => OpCodeDetails::new(0x05, 1, 3, false),
            OpCode::OraZeroPageX  => OpCodeDetails::new(0x15, 1, 4, false),
            OpCode::OraAbsolute   => OpCodeDetails::new(0x0D, 2, 4, false),
            OpCode::OraAbsoluteX  => OpCodeDetails::new(0x1D, 2, 4, true),
            OpCode::OraAbsoluteY  => OpCodeDetails::new(0x19, 2, 4, true),
            OpCode::OraIndirectX  => OpCodeDetails::new(0x01, 1, 6, false),
            OpCode::OraIndirectY  => OpCodeDetails::new(0x11, 1, 5, true),

            // Register instructions
            OpCode::TaxImplied    => OpCodeDetails::new(0xAA, 0, 2, false),
            OpCode::TxaImplied    => OpCodeDetails::new(0x8A, 0, 2, false),
            OpCode::DexImplied    => OpCodeDetails::new(0xCA, 0, 2, false),
            OpCode::InxImplied    => OpCodeDetails::new(0xE8, 0, 2, false),
            OpCode::TayImplied    => OpCodeDetails::new(0xA8, 0, 2, false),
            OpCode::TyaImplied    => OpCodeDetails::new(0x98, 0, 2, false),
            OpCode::DeyImplied    => OpCodeDetails::new(0x88, 0, 2, false),
            OpCode::InyImplied    => OpCodeDetails::new(0xC8, 0, 2, false),

            // ROL
            OpCode::RolAccumulator => OpCodeDetails::new(0x2A, 0, 2, false),
            OpCode::RolZeroPage    => OpCodeDetails::new(0x26, 1, 5, false),
            OpCode::RolZeroPageX   => OpCodeDetails::new(0x36, 1, 6, false),
            OpCode::RolAbsolute    => OpCodeDetails::new(0x2E, 2, 6, false),
            OpCode::RolAbsoluteX   => OpCodeDetails::new(0x3E, 2, 7, false),

            // ROR
            OpCode::RorAccumulator => OpCodeDetails::new(0x6A, 0, 2, false),
            OpCode::RorZeroPage    => OpCodeDetails::new(0x66, 1, 5, false),
            OpCode::RorZeroPageX   => OpCodeDetails::new(0x76, 1, 6, false),
            OpCode::RorAbsolute    => OpCodeDetails::new(0x6E, 2, 6, false),
            OpCode::RorAbsoluteX   => OpCodeDetails::new(0x7E, 2, 7, false),

            // RTI
            OpCode::RtiImplied    => OpCodeDetails::new(0x40, 0, 6, false),

            // RTS
            OpCode::RtsImplied    => OpCodeDetails::new(0x60, 0, 6, false),

            // SBC
            OpCode::SbcImmediate  => OpCodeDetails::new(0xE9, 1, 2, false),
            OpCode::SbcZeroPage   => OpCodeDetails::new(0xE5, 1, 3, false),
            OpCode::SbcZeroPageX  => OpCodeDetails::new(0xF5, 1, 4, false),
            OpCode::SbcAbsolute   => OpCodeDetails::new(0xED, 2, 4, false),
            OpCode::SbcAbsoluteX  => OpCodeDetails::new(0xFD, 2, 4, true),
            OpCode::SbcAbsoluteY  => OpCodeDetails::new(0xF9, 2, 4, true),
            OpCode::SbcIndirectX  => OpCodeDetails::new(0xE1, 1, 6, false),
            OpCode::SbcIndirectY  => OpCodeDetails::new(0xF1, 1, 5, true),

            // STA
            OpCode::StaZeroPage   => OpCodeDetails::new(0x85, 1, 3, false),
            OpCode::StaZeroPageX  => OpCodeDetails::new(0x95, 1, 4, false),
            OpCode::StaAbsolute   => OpCodeDetails::new(0x8D, 2, 4, false),
            OpCode::StaAbsoluteX  => OpCodeDetails::new(0x9D, 2, 5, false),
            OpCode::StaAbsoluteY  => OpCodeDetails::new(0x99, 2, 5, false),
            OpCode::StaIndirectX  => OpCodeDetails::new(0x81, 1, 6, false),
            OpCode::StaIndirectY  => OpCodeDetails::new(0x91, 1, 6, false),

            // Stack instructions
            OpCode::TxsImplied   => OpCodeDetails::new(0x9A, 0, 2, false),
            OpCode::TsxImplied   => OpCodeDetails::new(0xBA, 0, 2, false),
            OpCode::PhaImplied   => OpCodeDetails::new(0x48, 0, 3, false),
            OpCode::PlaImplied   => OpCodeDetails::new(0x68, 0, 4, false),
            OpCode::PhpImplied   => OpCodeDetails::new(0x08, 0, 3, false),
            OpCode::PlpImplied   => OpCodeDetails::new(0x28, 0, 4, false),

            // STX
            OpCode::StxZeroPage   => OpCodeDetails::new(0x86, 1, 3, false),
            OpCode::StxZeroPageY  => OpCodeDetails::new(0x96, 1, 4, false),
            OpCode::StxAbsolute   => OpCodeDetails::new(0x8E, 2, 4, false),

            // STY
            OpCode::StyZeroPage   => OpCodeDetails::new(0x84, 1, 3, false),
            OpCode::StyZeroPageX  => OpCodeDetails::new(0x94, 1, 4, false),
            OpCode::StyAbsolute   => OpCodeDetails::new(0x8C, 2, 4, false),
        }
    }
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
    pub fn step(&mut self, _memory: &mut dyn Memory) -> u8 {
        // TODO: implement instruction decoding and execution
        todo!("6502 instruction execution not yet implemented")
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
