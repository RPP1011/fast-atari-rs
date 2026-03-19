/// Atari 2600 console — ties CPU, TIA, PIA, and cartridge ROM together.

use crate::cpu::{Cpu, Memory};
use crate::env::{Env, StepInfo};
use crate::headless_tia::HeadlessTia;
use crate::pia::Pia;
use crate::tia::{Tia, FRAME_WIDTH, FRAME_HEIGHT};

/// Atari 2600 joystick actions (mimics ALE's 18-action set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Action {
    Noop = 0,
    Fire,
    Up,
    Right,
    Left,
    Down,
    UpRight,
    UpLeft,
    DownRight,
    DownLeft,
    UpFire,
    RightFire,
    LeftFire,
    DownFire,
    UpRightFire,
    UpLeftFire,
    DownRightFire,
    DownLeftFire,
}

impl Action {
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Noop,
            1 => Self::Fire,
            2 => Self::Up,
            3 => Self::Right,
            4 => Self::Left,
            5 => Self::Down,
            6 => Self::UpRight,
            7 => Self::UpLeft,
            8 => Self::DownRight,
            9 => Self::DownLeft,
            10 => Self::UpFire,
            11 => Self::RightFire,
            12 => Self::LeftFire,
            13 => Self::DownFire,
            14 => Self::UpRightFire,
            15 => Self::UpLeftFire,
            16 => Self::DownRightFire,
            17 => Self::DownLeftFire,
            _ => Self::Noop,
        }
    }

    /// Decode into (up, down, left, right, fire).
    fn decode(self) -> (bool, bool, bool, bool, bool) {
        match self {
            Self::Noop          => (false, false, false, false, false),
            Self::Fire          => (false, false, false, false, true),
            Self::Up            => (true,  false, false, false, false),
            Self::Right         => (false, false, false, true,  false),
            Self::Left          => (false, false, true,  false, false),
            Self::Down          => (false, true,  false, false, false),
            Self::UpRight       => (true,  false, false, true,  false),
            Self::UpLeft        => (true,  false, true,  false, false),
            Self::DownRight     => (false, true,  false, true,  false),
            Self::DownLeft      => (false, true,  true,  false, false),
            Self::UpFire        => (true,  false, false, false, true),
            Self::RightFire     => (false, false, false, true,  true),
            Self::LeftFire      => (false, false, true,  false, true),
            Self::DownFire      => (false, true,  false, false, true),
            Self::UpRightFire   => (true,  false, false, true,  true),
            Self::UpLeftFire    => (true,  false, true,  false, true),
            Self::DownRightFire => (false, true,  false, true,  true),
            Self::DownLeftFire  => (false, true,  true,  false, true),
        }
    }
}

/// Bank switching scheme, auto-detected from ROM size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankScheme {
    /// 2K or 4K — no bankswitching, simple mirroring.
    Fixed,
    /// F8 — 8K, two 4K banks. Hotspots: $1FF8 (bank 0), $1FF9 (bank 1).
    F8,
    /// F6 — 16K, four 4K banks. Hotspots: $1FF6–$1FF9.
    F6,
    /// F4 — 32K, eight 4K banks. Hotspots: $1FF4–$1FFB.
    F4,
}

impl BankScheme {
    pub fn detect(rom_len: usize) -> Self {
        match rom_len {
            0..=4096 => Self::Fixed,
            4097..=8192 => Self::F8,
            8193..=16384 => Self::F6,
            _ => Self::F4,
        }
    }
}

/// The Atari 2600 memory bus, visible to the CPU.
pub struct Bus {
    pub tia: Tia,
    pub pia: Pia,
    pub rom: Vec<u8>,
    pub bank: usize,
    pub scheme: BankScheme,
}

impl Bus {
    fn new(rom: Vec<u8>) -> Self {
        let scheme = BankScheme::detect(rom.len());
        // Default to last bank (where reset vector lives)
        let bank = match scheme {
            BankScheme::Fixed => 0,
            BankScheme::F8 => 1,
            BankScheme::F6 => 3,
            BankScheme::F4 => 7,
        };
        Self {
            tia: Tia::new(),
            pia: Pia::new(),
            rom,
            bank,
            scheme,
        }
    }

    /// Read from the currently selected ROM bank.
    fn rom_read(&self, addr: u16) -> u8 {
        match self.scheme {
            BankScheme::Fixed => {
                let index = (addr & 0x0FFF) as usize % self.rom.len();
                self.rom[index]
            }
            _ => {
                let offset = self.bank * 4096 + (addr & 0x0FFF) as usize;
                *self.rom.get(offset).unwrap_or(&0)
            }
        }
    }

    /// Check for bankswitching hotspot access and switch bank if needed.
    fn check_bankswitch(&mut self, addr: u16) {
        let a = addr & 0x1FFF;
        match self.scheme {
            BankScheme::Fixed => {}
            BankScheme::F8 => match a {
                0x1FF8 => self.bank = 0,
                0x1FF9 => self.bank = 1,
                _ => {}
            },
            BankScheme::F6 => match a {
                0x1FF6 => self.bank = 0,
                0x1FF7 => self.bank = 1,
                0x1FF8 => self.bank = 2,
                0x1FF9 => self.bank = 3,
                _ => {}
            },
            BankScheme::F4 => match a {
                0x1FF4 => self.bank = 0,
                0x1FF5 => self.bank = 1,
                0x1FF6 => self.bank = 2,
                0x1FF7 => self.bank = 3,
                0x1FF8 => self.bank = 4,
                0x1FF9 => self.bank = 5,
                0x1FFA => self.bank = 6,
                0x1FFB => self.bank = 7,
                _ => {}
            },
        }
    }
}

impl Memory for Bus {
    fn read(&mut self, addr: u16) -> u8 {
        self.check_bankswitch(addr);
        match addr & 0x1FFF {
            // TIA read registers: $00–$0D (active when A12=0, A7=0)
            a if a & 0x1080 == 0x0000 => self.tia.read(a),
            // PIA RAM: $80–$FF (A12=0, A9=0, A7=1)
            a if a & 0x1280 == 0x0080 => self.pia.read(a),
            // PIA I/O: $280–$29F (A12=0, A9=1)
            a if a & 0x1280 == 0x0280 => self.pia.read(a),
            // Cartridge ROM: $1000–$1FFF (A12=1)
            a if a & 0x1000 == 0x1000 => self.rom_read(a),
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.check_bankswitch(addr);
        match addr & 0x1FFF {
            // TIA write registers: $00–$2C (A12=0, A7=0)
            a if a & 0x1080 == 0x0000 => self.tia.write(a, val),
            // PIA RAM: $80–$FF
            a if a & 0x1280 == 0x0080 => self.pia.write(a, val),
            // PIA I/O: $280–$29F
            a if a & 0x1280 == 0x0280 => self.pia.write(a, val),
            _ => {}
        }
    }
}

/// Atari 2600 console.
pub struct Atari {
    pub cpu: Cpu,
    pub bus: Bus,
    /// Pending TIA/PIA ticks from the previous CPU instruction.
    /// Flushed before the next instruction executes, so register
    /// writes land at the correct beam position.
    pending_cycles: u64,
}

impl Atari {
    pub fn new(rom: Vec<u8>) -> Self {
        let mut console = Self {
            cpu: Cpu::new(),
            bus: Bus::new(rom),
            pending_cycles: 0,
        };
        console.cpu.reset(&mut console.bus);
        console
    }

    /// Apply a joystick action to the PIA ports.
    pub fn set_action(&mut self, action: Action) {
        let (up, down, left, right, fire) = action.decode();

        // SWCHA: P0 = upper nibble (active low)
        // Bit 4 = P0 up, bit 5 = P0 down, bit 6 = P0 left, bit 7 = P0 right
        // (inverted: 0 = pressed)
        let mut swcha = 0xFF;
        if up    { swcha &= !0x10; }
        if down  { swcha &= !0x20; }
        if left  { swcha &= !0x40; }
        if right { swcha &= !0x80; }
        self.bus.pia.set_port_a_input(swcha);

        // Fire button via TIA input latch (active low: false = pressed)
        self.bus.tia.set_input(0, !fire);

        // Map left/right to paddle position for paddle-based games
        let paddle = if left { 200u8 } else if right { 50u8 } else { 128u8 };
        self.bus.tia.set_paddle(0, paddle);
    }

    /// Tick TIA and PIA for one CPU cycle (3 TIA clocks, 1 PIA clock).
    #[inline]
    fn tick_components(&mut self) {
        self.bus.tia.tick();
        self.bus.tia.tick();
        self.bus.tia.tick();
        self.bus.pia.tick();
    }

    /// Run the console until the next frame is complete.
    /// Returns the number of CPU cycles executed.
    pub fn run_frame(&mut self) -> u64 {
        // Wait for VSYNC to signal frame complete, but ignore if it's
        // already set from the previous frame.
        self.bus.tia.frame_complete = false;
        let mut cycles: u64 = 0;

        // Phase 1: If we're currently in VSYNC, run until VSYNC ends
        while self.bus.tia.vsync & 0x02 != 0 {
            self.run_one_cycle(&mut cycles);
        }

        // Phase 2: Run until the next VSYNC starts (= end of this frame)
        while self.bus.tia.vsync & 0x02 == 0 {
            self.run_one_cycle(&mut cycles);
            // Safety: break if we've run way too many cycles (broken ROM)
            if cycles > 100_000 { break; }
        }

        // Flush any remaining pending ticks so the framebuffer is complete
        for _ in 0..self.pending_cycles {
            self.tick_components();
        }
        self.pending_cycles = 0;

        cycles
    }

    /// Execute one CPU step with interleaved TIA/PIA ticking.
    ///
    /// On real hardware the TIA beam advances continuously during CPU
    /// execution, and register writes take effect on the last cycle.
    /// We approximate this by ticking TIA for (N-1) cycles before the
    /// write, then 1 cycle after — so the write lands near the end of
    /// the instruction, matching real hardware behavior.
    #[inline]
    fn run_one_cycle(&mut self, cycles: &mut u64) {
        if self.bus.tia.wsync {
            self.tick_components();
            *cycles += 1;
        } else {
            // Peek at opcode to get cycle count before executing
            let op = crate::cpu::OpCode::decode(&mut self.bus, self.cpu.pc);
            let c = match &op {
                Some(op) => {
                    let d = op.details();
                    d.cycle_count as u64
                }
                None => 2, // fallback
            };

            // Tick TIA for (c-1) cycles — beam advances to just before the write
            for _ in 0..c.saturating_sub(1) {
                self.tick_components();
            }

            // Execute the instruction — register writes happen here
            let actual_c = self.cpu.step(&mut self.bus) as u64;
            *cycles += actual_c;

            // Tick the remaining cycle(s) after the write
            let remaining = actual_c.saturating_sub(c.saturating_sub(1));
            for _ in 0..remaining {
                self.tick_components();
            }
        }
    }

    /// Get the current framebuffer as NTSC palette indices (160 x 192).
    pub fn framebuffer(&self) -> &[u8; FRAME_WIDTH * FRAME_HEIGHT] {
        &self.bus.tia.framebuffer
    }
}

impl Env for Atari {
    type Obs = Vec<u8>;
    type Act = Action;

    fn reset(&mut self, _seed: Option<u64>) -> Self::Obs {
        self.cpu = Cpu::new();
        self.bus.tia = Tia::new();
        self.bus.pia = Pia::new();
        self.pending_cycles = 0;
        self.cpu.reset(&mut self.bus);

        // Run one frame to get initial observation
        self.run_frame();
        self.bus.tia.framebuffer.to_vec()
    }

    fn step(&mut self, action: Self::Act) -> (Self::Obs, StepInfo) {
        self.set_action(action);
        self.run_frame();

        let obs = self.bus.tia.framebuffer.to_vec();
        let info = StepInfo {
            reward: 0.0, // reward extraction is game-specific
            terminated: false,
            truncated: false,
            info: vec![],
        };

        (obs, info)
    }

    fn render(&self) -> Vec<u8> {
        // Convert palette indices to RGBA
        let fb = self.framebuffer();
        let mut rgba = vec![0u8; FRAME_WIDTH * FRAME_HEIGHT * 4];
        for (i, &palette_idx) in fb.iter().enumerate() {
            let (r, g, b) = ntsc_palette(palette_idx);
            rgba[i * 4]     = r;
            rgba[i * 4 + 1] = g;
            rgba[i * 4 + 2] = b;
            rgba[i * 4 + 3] = 255;
        }
        rgba
    }

    fn action_space(&self) -> usize { 18 }

    fn observation_shape(&self) -> (usize, usize, usize) {
        (FRAME_HEIGHT, FRAME_WIDTH, 1) // palette indices
    }

    fn close(&mut self) {}
}

// ---------------------------------------------------------------------------
// Headless Atari — no framebuffer, no pixel rendering, maximum throughput.
// ---------------------------------------------------------------------------

/// Flat memory bus for headless mode.
///
/// The 2600 has a 13-bit address space (8KB). We flatten the entire thing
/// into a single `[u8; 8192]` array:
///   - `$0080–$00FF` → PIA RAM (128 bytes), mirrored at `$0180–$01FF` (stack)
///   - `$1000–$1FFF` → ROM (current 4K bank, copied in at load / bank switch)
///   - Everything else in the array is unused padding
///
/// Reads hit the flat array first. Only addresses that need special handling
/// (TIA registers, PIA I/O) fall through to the slow path. Since ~90% of
/// reads are instruction/operand fetches from ROM, this eliminates address
/// decoding, bankswitch checks, and bounds checks from the hot path.
///
/// Writes always go through dispatch (ROM is never written).
pub struct HeadlessBus {
    pub tia: HeadlessTia,
    pub pia: Pia,
    /// Flat 8KB address space. ROM and PIA RAM live at their real addresses.
    pub mem: Box<[u8; 8192]>,
    /// Full ROM image (for bankswitched games, all banks concatenated).
    pub rom: Vec<u8>,
    pub bank: usize,
    pub scheme: BankScheme,
}

impl HeadlessBus {
    fn new(rom: Vec<u8>) -> Self {
        let scheme = BankScheme::detect(rom.len());
        let bank = match scheme {
            BankScheme::Fixed => 0,
            BankScheme::F8 => 1,
            BankScheme::F6 => 3,
            BankScheme::F4 => 7,
        };

        let mut bus = Self {
            tia: HeadlessTia::new(),
            pia: Pia::new(),
            mem: Box::new([0u8; 8192]),
            rom,
            bank,
            scheme,
        };
        bus.load_bank(bank);
        bus.sync_ram_to_mem();
        bus
    }

    /// Copy a ROM bank into the flat address space at $1000–$1FFF.
    fn load_bank(&mut self, bank: usize) {
        let src = match self.scheme {
            BankScheme::Fixed => {
                // 2K/4K ROM: mirror into the 4K window
                let rom_len = self.rom.len();
                for i in 0..4096 {
                    self.mem[0x1000 + i] = self.rom[i % rom_len];
                }
                return;
            }
            _ => {
                let offset = bank * 4096;
                &self.rom[offset..offset + 4096]
            }
        };
        self.mem[0x1000..0x2000].copy_from_slice(src);
    }

    /// Sync PIA RAM into the flat array (call before reads that might hit RAM).
    /// PIA RAM at $0080–$00FF, mirrored at $0180–$01FF (stack page).
    #[inline]
    fn sync_ram_to_mem(&mut self) {
        self.mem[0x80..0x100].copy_from_slice(&self.pia.ram);
        self.mem[0x180..0x200].copy_from_slice(&self.pia.ram);
    }

    #[inline]
    fn check_bankswitch(&mut self, addr: u16) {
        let a = addr & 0x1FFF;
        let new_bank = match self.scheme {
            BankScheme::Fixed => return,
            BankScheme::F8 => match a {
                0x1FF8 => 0,
                0x1FF9 => 1,
                _ => return,
            },
            BankScheme::F6 => match a {
                0x1FF6 => 0,
                0x1FF7 => 1,
                0x1FF8 => 2,
                0x1FF9 => 3,
                _ => return,
            },
            BankScheme::F4 => match a {
                0x1FF4 => 0,
                0x1FF5 => 1,
                0x1FF6 => 2,
                0x1FF7 => 3,
                0x1FF8 => 4,
                0x1FF9 => 5,
                0x1FFA => 6,
                0x1FFB => 7,
                _ => return,
            },
        };
        if new_bank != self.bank {
            self.bank = new_bank;
            self.load_bank(new_bank);
        }
    }
}

impl Memory for HeadlessBus {
    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        let a = addr & 0x1FFF;

        // Fast path: bit 12 or (bit 7 set AND bit 9 clear) → ROM or RAM
        // ROM: $1000–$1FFF, RAM: $80–$FF / $180–$1FF (stack)
        // Excludes PIA I/O ($0280–$029F) which also has bit 7 set
        if a & 0x1080 != 0 && (a & 0x1280) != 0x0280 {
            if a & 0x1000 != 0 && self.scheme != BankScheme::Fixed {
                self.check_bankswitch(addr);
            }
            return self.mem[a as usize];
        }

        // PIA I/O: $280–$29F (A12=0, A9=1)
        if a & 0x1280 == 0x0280 {
            return self.pia.read(a);
        }

        // TIA read registers or fallback
        if a & 0x1080 == 0x0000 {
            return self.tia.read(a);
        }
        0
    }

    #[inline]
    fn write(&mut self, addr: u16, val: u8) {
        let a = addr & 0x1FFF;

        // ROM space — only bankswitch hotspots matter
        if a & 0x1000 != 0 {
            if self.scheme != BankScheme::Fixed {
                self.check_bankswitch(addr);
            }
            return;
        }

        // PIA RAM: $80–$FF or $180–$1FF
        if a & 0x80 != 0 && a & 0x200 == 0 {
            // Write to both PIA and flat array (keep in sync)
            self.pia.ram[(a & 0x7F) as usize] = val;
            self.mem[(a | 0x80) as usize & 0x1FF] = val;
            // Mirror
            self.mem[((a | 0x80) as usize & 0x1FF) ^ 0x100] = val;
            return;
        }

        // PIA I/O: $280–$29F
        if a & 0x200 != 0 {
            self.pia.write(a, val);
            return;
        }

        // TIA write registers
        self.tia.write(a, val);
    }
}

/// Headless Atari 2600 — skips all pixel rendering for maximum throughput.
pub struct HeadlessAtari {
    pub cpu: Cpu,
    pub bus: HeadlessBus,
}

impl HeadlessAtari {
    pub fn new(rom: Vec<u8>) -> Self {
        let mut console = Self {
            cpu: Cpu::new(),
            bus: HeadlessBus::new(rom),
        };
        console.cpu.reset(&mut console.bus);
        console
    }

    pub fn set_action(&mut self, action: Action) {
        let (up, down, left, right, fire) = action.decode();
        let mut swcha = 0xFF;
        if up    { swcha &= !0x10; }
        if down  { swcha &= !0x20; }
        if left  { swcha &= !0x40; }
        if right { swcha &= !0x80; }
        self.bus.pia.set_port_a_input(swcha);
        self.bus.tia.set_input(0, !fire);
        let paddle = if left { 200u8 } else if right { 50u8 } else { 128u8 };
        self.bus.tia.set_paddle(0, paddle);
    }

    pub fn run_frame(&mut self) -> u64 {
        self.bus.tia.frame_complete = false;
        let mut cycles: u64 = 0;

        while self.bus.tia.vsync & 0x02 != 0 {
            self.run_one_cycle(&mut cycles);
        }

        while self.bus.tia.vsync & 0x02 == 0 {
            self.run_one_cycle(&mut cycles);
            if cycles > 100_000 { break; }
        }

        cycles
    }

    #[inline]
    fn run_one_cycle(&mut self, cycles: &mut u64) {
        if self.bus.tia.wsync {
            // Fast-forward to end of scanline instead of ticking one cycle at a time
            let skipped = self.bus.tia.skip_to_scanline_end();
            self.bus.pia.tick_n(skipped);
            *cycles += skipped as u64;
        } else {
            let c = self.cpu.step(&mut self.bus);
            *cycles += c as u64;
            // Batch-advance TIA and PIA instead of per-cycle loop
            self.bus.tia.tick_n(c);
            self.bus.pia.tick_n(c as u16);
        }
    }

    /// Access PIA RAM for observation (128 bytes at $0080–$00FF in flat memory).
    pub fn ram(&self) -> &[u8] {
        &self.bus.mem[0x80..0x100]
    }
}

/// NTSC color palette (128 colors, indexed by the TIA color register value).
/// Returns (R, G, B) for a given palette index (upper 7 bits used, bit 0 ignored).
fn ntsc_palette(idx: u8) -> (u8, u8, u8) {
    #[rustfmt::skip]
    const PALETTE: [(u8, u8, u8); 128] = [
        (0,0,0),(0,0,0),(68,68,0),(68,68,0),(112,40,0),(112,40,0),(132,24,0),(132,24,0),
        (136,0,0),(136,0,0),(120,0,92),(120,0,92),(72,0,120),(72,0,120),(20,0,132),(20,0,132),
        (0,0,136),(0,0,136),(0,0,120),(0,0,120),(0,0,96),(0,0,96),(0,20,44),(0,20,44),
        (0,40,0),(0,40,0),(0,44,0),(0,44,0),(0,44,0),(0,44,0),(0,24,0),(0,24,0),
        (40,40,40),(40,40,40),(100,100,16),(100,100,16),(144,72,16),(144,72,16),(168,56,0),(168,56,0),
        (184,20,0),(184,20,0),(160,0,116),(160,0,116),(100,0,164),(100,0,164),(48,0,176),(48,0,176),
        (0,0,184),(0,0,184),(0,24,168),(0,24,168),(0,44,128),(0,44,128),(0,60,76),(0,60,76),
        (0,68,0),(0,68,0),(0,76,0),(0,76,0),(0,68,0),(0,68,0),(0,56,0),(0,56,0),
        (84,84,84),(84,84,84),(132,132,36),(132,132,36),(176,100,36),(176,100,36),(200,80,16),(200,80,16),
        (220,56,20),(220,56,20),(196,20,144),(196,20,144),(132,0,200),(132,0,200),(80,0,212),(80,0,212),
        (24,0,220),(24,0,220),(0,48,212),(0,48,212),(0,76,164),(0,76,164),(0,92,108),(0,92,108),
        (0,100,0),(0,100,0),(0,108,0),(0,108,0),(0,100,16),(0,100,16),(0,84,0),(0,84,0),
        (120,120,120),(120,120,120),(168,168,56),(168,168,56),(204,136,56),(204,136,56),(232,112,36),(232,112,36),
        (252,92,40),(252,92,40),(232,48,172),(232,48,172),(168,0,232),(168,0,232),(112,0,244),(112,0,244),
        (56,0,252),(56,0,252),(12,76,248),(12,76,248),(0,108,196),(0,108,196),(0,128,140),(0,128,140),
        (0,136,0),(0,136,0),(0,144,0),(0,144,0),(0,136,40),(0,136,40),(0,116,0),(0,116,0),
    ];
    PALETTE[(idx >> 1) as usize & 0x7F]
}
