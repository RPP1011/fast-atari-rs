/// rl4burn-compatible Atari 2600 environments.
///
/// Provides two environment variants:
/// - [`AtariEnv`]: Full rendering with framebuffer observations (160×192 grayscale).
/// - [`HeadlessAtariEnv`]: RAM-only observations (128 bytes) for maximum throughput.
///
/// Both implement [`rl4burn_core::env::Env`] with `Observation = Vec<f32>` and
/// `Action = usize` (discrete, 18 ALE actions).

use rl4burn_core::env::render::{Renderable, RgbFrame};
use rl4burn_core::env::space::Space;
use rl4burn_core::env::{Env, Step};

use crate::atari::{Action, Atari, HeadlessAtari};
use crate::tia::{FRAME_HEIGHT, FRAME_WIDTH};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of ALE-compatible joystick actions.
const NUM_ACTIONS: usize = 18;

/// Default max steps per episode before truncation (108_000 frames ≈ 30 min at 60fps).
const DEFAULT_MAX_STEPS: usize = 108_000;

/// Number of frames to skip per step (action repeat / frameskip).
const DEFAULT_FRAMESKIP: usize = 4;

// ---------------------------------------------------------------------------
// AtariEnv — full rendering, framebuffer observations
// ---------------------------------------------------------------------------

/// Atari 2600 environment with framebuffer observations.
///
/// Observations are the TIA framebuffer flattened to `Vec<f32>` with values in
/// `[0.0, 1.0]` (palette indices normalised to 128 colours).
///
/// # Example
/// ```ignore
/// let rom = std::fs::read("breakout.bin").unwrap();
/// let mut env = AtariEnv::new(rom);
/// let obs = env.reset();
/// let step = env.step(1); // Fire
/// ```
pub struct AtariEnv {
    console: Atari,
    rom: Vec<u8>,
    step_count: usize,
    max_steps: usize,
    frameskip: usize,
}

impl AtariEnv {
    /// Create a new environment from a ROM binary.
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            console: Atari::new(rom.clone()),
            rom,
            step_count: 0,
            max_steps: DEFAULT_MAX_STEPS,
            frameskip: DEFAULT_FRAMESKIP,
        }
    }

    /// Set maximum steps per episode (0 = unlimited).
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Set number of frames to skip per step (action repeat).
    pub fn with_frameskip(mut self, frameskip: usize) -> Self {
        self.frameskip = frameskip;
        self
    }

    /// Get the raw framebuffer as palette indices.
    pub fn framebuffer(&self) -> &[u8; FRAME_WIDTH * FRAME_HEIGHT] {
        self.console.framebuffer()
    }

    /// Convert framebuffer to normalised `Vec<f32>` observation.
    fn obs(&self) -> Vec<f32> {
        self.console
            .framebuffer()
            .iter()
            .map(|&px| (px >> 1) as f32 / 127.0)
            .collect()
    }
}

impl Env for AtariEnv {
    type Observation = Vec<f32>;
    type Action = usize;

    fn reset(&mut self) -> Vec<f32> {
        self.console = Atari::new(self.rom.clone());
        self.step_count = 0;
        // Run one frame to get initial observation
        self.console.run_frame();
        self.obs()
    }

    fn step(&mut self, action: usize) -> Step<Vec<f32>> {
        let act = Action::from_index(action);
        self.console.set_action(act);

        for _ in 0..self.frameskip {
            self.console.run_frame();
        }

        self.step_count += 1;

        let truncated = self.max_steps > 0 && self.step_count >= self.max_steps;

        Step {
            observation: self.obs(),
            reward: 0.0, // reward extraction is game-specific; use a wrapper
            terminated: false,
            truncated,
        }
    }

    fn observation_space(&self) -> Space {
        Space::Box {
            low: vec![0.0; FRAME_WIDTH * FRAME_HEIGHT],
            high: vec![1.0; FRAME_WIDTH * FRAME_HEIGHT],
        }
    }

    fn action_space(&self) -> Space {
        Space::Discrete(NUM_ACTIONS)
    }
}

impl Renderable for AtariEnv {
    fn render(&self) -> RgbFrame {
        let fb = self.console.framebuffer();
        let mut data = Vec::with_capacity(FRAME_WIDTH * FRAME_HEIGHT * 3);
        for &px in fb.iter() {
            let (r, g, b) = ntsc_palette(px);
            data.push(r);
            data.push(g);
            data.push(b);
        }
        RgbFrame {
            width: FRAME_WIDTH as u16,
            height: FRAME_HEIGHT as u16,
            data,
        }
    }
}

// ---------------------------------------------------------------------------
// HeadlessAtariEnv — RAM observations, maximum throughput
// ---------------------------------------------------------------------------

/// Atari 2600 environment with RAM-only observations (no rendering).
///
/// Observations are the 128-byte PIA RAM normalised to `[0.0, 1.0]`.
/// This variant is significantly faster than [`AtariEnv`] since it skips
/// all pixel rendering.
pub struct HeadlessAtariEnv {
    console: HeadlessAtari,
    rom: Vec<u8>,
    step_count: usize,
    max_steps: usize,
    frameskip: usize,
}

impl HeadlessAtariEnv {
    /// Create a new headless environment from a ROM binary.
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            console: HeadlessAtari::new(rom.clone()),
            rom,
            step_count: 0,
            max_steps: DEFAULT_MAX_STEPS,
            frameskip: DEFAULT_FRAMESKIP,
        }
    }

    /// Set maximum steps per episode (0 = unlimited).
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Set number of frames to skip per step (action repeat).
    pub fn with_frameskip(mut self, frameskip: usize) -> Self {
        self.frameskip = frameskip;
        self
    }

    /// Access the raw 128-byte PIA RAM.
    pub fn ram(&self) -> &[u8; 128] {
        self.console.ram()
    }

    fn obs(&self) -> Vec<f32> {
        self.console.ram().iter().map(|&b| b as f32 / 255.0).collect()
    }
}

impl Env for HeadlessAtariEnv {
    type Observation = Vec<f32>;
    type Action = usize;

    fn reset(&mut self) -> Vec<f32> {
        self.console = HeadlessAtari::new(self.rom.clone());
        self.step_count = 0;
        self.console.run_frame();
        self.obs()
    }

    fn step(&mut self, action: usize) -> Step<Vec<f32>> {
        let act = Action::from_index(action);
        self.console.set_action(act);

        for _ in 0..self.frameskip {
            self.console.run_frame();
        }

        self.step_count += 1;

        let truncated = self.max_steps > 0 && self.step_count >= self.max_steps;

        Step {
            observation: self.obs(),
            reward: 0.0,
            terminated: false,
            truncated,
        }
    }

    fn observation_space(&self) -> Space {
        Space::Box {
            low: vec![0.0; 128],
            high: vec![1.0; 128],
        }
    }

    fn action_space(&self) -> Space {
        Space::Discrete(NUM_ACTIONS)
    }
}

// ---------------------------------------------------------------------------
// NTSC Palette
// ---------------------------------------------------------------------------

/// NTSC color palette (128 colours). Returns (R, G, B) for a TIA palette index.
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
