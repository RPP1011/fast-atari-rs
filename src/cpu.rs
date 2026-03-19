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
