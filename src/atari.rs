/// Atari 2600 console — ties CPU, TIA, PIA, and cartridge ROM together.

use crate::cpu::{Cpu, Memory};
use crate::env::{Env, StepInfo};
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

/// The Atari 2600 memory bus, visible to the CPU.
pub struct Bus {
    pub tia: Tia,
    pub pia: Pia,
    pub rom: Vec<u8>,
}

impl Bus {
    fn new(rom: Vec<u8>) -> Self {
        Self {
            tia: Tia::new(),
            pia: Pia::new(),
            rom,
        }
    }

    /// Map a ROM address to the correct byte, handling 2K/4K mirroring.
    fn rom_read(&self, addr: u16) -> u8 {
        let index = (addr & 0x0FFF) as usize % self.rom.len();
        self.rom[index]
    }
}

impl Memory for Bus {
    fn read(&mut self, addr: u16) -> u8 {
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
        match addr & 0x1FFF {
            // TIA write registers: $00–$2C (A12=0, A7=0)
            a if a & 0x1080 == 0x0000 => self.tia.write(a, val),
            // PIA RAM: $80–$FF
            a if a & 0x1280 == 0x0080 => self.pia.write(a, val),
            // PIA I/O: $280–$29F
            a if a & 0x1280 == 0x0280 => self.pia.write(a, val),
            // ROM writes (bank switching would go here)
            _ => {}
        }
    }
}

/// Atari 2600 console.
pub struct Atari {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Atari {
    pub fn new(rom: Vec<u8>) -> Self {
        let mut console = Self {
            cpu: Cpu::new(),
            bus: Bus::new(rom),
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
    }

    /// Run the console until the next frame is complete.
    /// Returns the number of CPU cycles executed.
    pub fn run_frame(&mut self) -> u64 {
        self.bus.tia.frame_complete = false;
        let mut cycles: u64 = 0;

        while !self.bus.tia.frame_complete {
            // If WSYNC is active, skip CPU but keep TIA/PIA ticking
            if !self.bus.tia.wsync {
                let c = self.cpu.step(&mut self.bus) as u64;
                cycles += c;

                // Each CPU cycle = 3 TIA clocks, 1 PIA clock
                for _ in 0..c {
                    self.bus.tia.tick();
                    self.bus.tia.tick();
                    self.bus.tia.tick();
                    self.bus.pia.tick();
                }
            } else {
                // WSYNC: run TIA clocks until end of scanline
                while self.bus.tia.wsync {
                    self.bus.tia.tick();
                    self.bus.tia.tick();
                    self.bus.tia.tick();
                    self.bus.pia.tick();
                    cycles += 1;
                }
            }
        }

        cycles
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
