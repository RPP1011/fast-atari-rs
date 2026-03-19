/// Headless TIA — timing-only variant for maximum throughput.
///
/// Maintains just enough state for the CPU to execute correctly:
/// - Clock/scanline counting (WSYNC release, frame boundaries)
/// - VSYNC/VBLANK frame detection
/// - Register reads (collision latches, input ports)
/// - Register writes (all game-visible state)
///
/// Drops: framebuffer, pixel rendering, playfield/player/missile/ball graphics.
/// This eliminates the per-pixel `render_pixel()` call — the dominant cost in
/// the rendering TIA (called 160× per visible scanline, 192 scanlines/frame).

use crate::tia::SCANLINES_PER_FRAME;

#[derive(Clone, Debug)]
pub struct HeadlessTia {
    // Timing
    pub clock: u16,
    pub scanline: u16,
    pub wsync: bool,
    pub frame_complete: bool,

    // Registers the CPU can read back or that affect timing
    pub vsync: u8,
    pub vblank: u8,

    // Collision latches (games read these to detect hits)
    pub cxm0p: u8,
    pub cxm1p: u8,
    pub cxp0fb: u8,
    pub cxp1fb: u8,
    pub cxm0fb: u8,
    pub cxm1fb: u8,
    pub cxblpf: u8,
    pub cxppmm: u8,

    // Input latches
    pub inpt4: bool,
    pub inpt5: bool,

    // Paddle
    pub paddle0: u8,
    pub paddle1: u8,
    pub paddle_counter: u16,
    pub paddle_dumped: bool,

    // Object positions (needed for RESPx/HMOVE register writes —
    // games may read these back indirectly via collision detection)
    pub pos_p0: u16,
    pub pos_p1: u16,
    pub pos_m0: u16,
    pub pos_m1: u16,
    pub pos_bl: u16,
    pub hmp0: u8,
    pub hmp1: u8,
    pub hmm0: u8,
    pub hmm1: u8,
    pub hmbl: u8,

    // Graphics registers (games write/read-back via VDEL)
    pub grp0: u8,
    pub grp1: u8,
    pub grp0_old: u8,
    pub grp1_old: u8,
    pub enam0: bool,
    pub enam1: bool,
    pub enabl: bool,
    pub enabl_old: bool,
    pub vdelp0: bool,
    pub vdelp1: bool,
    pub vdelbl: bool,
    pub resmp0: bool,
    pub resmp1: bool,
    pub nusiz0: u8,
    pub nusiz1: u8,

    // Color/playfield registers (writes are accepted, never rendered)
    pub colup0: u8,
    pub colup1: u8,
    pub colupf: u8,
    pub colubk: u8,
    pub ctrlpf: u8,
    pub refp0: bool,
    pub refp1: bool,
    pub pf0: u8,
    pub pf1: u8,
    pub pf2: u8,
}

impl Default for HeadlessTia {
    fn default() -> Self {
        Self {
            clock: 0,
            scanline: 0,
            wsync: false,
            frame_complete: false,
            vsync: 0,
            vblank: 0,
            cxm0p: 0, cxm1p: 0, cxp0fb: 0, cxp1fb: 0,
            cxm0fb: 0, cxm1fb: 0, cxblpf: 0, cxppmm: 0,
            inpt4: true, inpt5: true,
            paddle0: 0, paddle1: 0,
            paddle_counter: 0, paddle_dumped: false,
            pos_p0: 0, pos_p1: 0,
            pos_m0: 0, pos_m1: 0, pos_bl: 0,
            hmp0: 0, hmp1: 0, hmm0: 0, hmm1: 0, hmbl: 0,
            grp0: 0, grp1: 0, grp0_old: 0, grp1_old: 0,
            enam0: false, enam1: false,
            enabl: false, enabl_old: false,
            vdelp0: false, vdelp1: false, vdelbl: false,
            resmp0: false, resmp1: false,
            nusiz0: 0, nusiz1: 0,
            colup0: 0, colup1: 0, colupf: 0, colubk: 0,
            ctrlpf: 0, refp0: false, refp1: false,
            pf0: 0, pf1: 0, pf2: 0,
        }
    }
}

impl HeadlessTia {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_input(&mut self, player: u8, fire: bool) {
        match player {
            0 => self.inpt4 = fire,
            1 => self.inpt5 = fire,
            _ => {}
        }
    }

    pub fn set_paddle(&mut self, player: u8, position: u8) {
        match player {
            0 => self.paddle0 = position,
            1 => self.paddle1 = position,
            _ => {}
        }
    }

    /// Advance by 3 TIA clocks (1 CPU cycle). No pixel rendering.
    #[inline]
    pub fn tick3(&mut self) {
        self.clock += 3;
        if self.clock >= 228 {
            self.end_scanline();
        }
    }

    /// Advance by `n` CPU cycles (3*n TIA clocks) in one call.
    /// Handles at most one scanline boundary crossing (safe for n <= 76).
    #[inline]
    pub fn tick_n(&mut self, n: u8) {
        self.clock += n as u16 * 3;
        if self.clock >= 228 {
            self.end_scanline();
        }
    }

    /// How many CPU cycles remain until end of current scanline.
    /// Returns 0 if we're exactly at a scanline boundary.
    #[inline]
    pub fn cycles_until_scanline_end(&self) -> u16 {
        // 228 TIA clocks per scanline, 3 TIA clocks per CPU cycle = 76 CPU cycles
        // Current position in CPU cycles: clock / 3 (rounded up)
        let tia_remaining = 228u16.saturating_sub(self.clock);
        (tia_remaining + 2) / 3 // ceiling division
    }

    /// Fast-forward to end of current scanline. Used for WSYNC.
    /// Returns the number of CPU cycles skipped.
    #[inline]
    pub fn skip_to_scanline_end(&mut self) -> u16 {
        let cpu_cycles = self.cycles_until_scanline_end();
        if cpu_cycles > 0 {
            self.clock = 228; // set to boundary so end_scanline wraps to 0
            self.end_scanline();
        }
        cpu_cycles
    }

    #[inline]
    fn end_scanline(&mut self) {
        self.clock = self.clock.wrapping_sub(228);
        self.scanline += 1;
        self.wsync = false;

        if !self.paddle_dumped && self.paddle_counter < 256 {
            self.paddle_counter += 1;
        }

        if self.scanline >= SCANLINES_PER_FRAME {
            self.scanline = 0;
            if !self.frame_complete {
                self.frame_complete = true;
            }
        }
    }

    fn apply_hmove_offset(pos: u16, hm: u8) -> u16 {
        let offset = (hm as i8) >> 4;
        ((pos as i16 - offset as i16).rem_euclid(160)) as u16
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr & 0x0F {
            0x00 => self.cxm0p,
            0x01 => self.cxm1p,
            0x02 => self.cxp0fb,
            0x03 => self.cxp1fb,
            0x04 => self.cxm0fb,
            0x05 => self.cxm1fb,
            0x06 => self.cxblpf,
            0x07 => self.cxppmm,
            0x08 => if self.paddle_counter >= self.paddle0 as u16 { 0x80 } else { 0x00 },
            0x09 => if self.paddle_counter >= self.paddle1 as u16 { 0x80 } else { 0x00 },
            0x0A => 0x80,
            0x0B => 0x80,
            0x0C => if self.inpt4 { 0x80 } else { 0x00 },
            0x0D => if self.inpt5 { 0x80 } else { 0x00 },
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr & 0x3F {
            0x00 => {
                if self.vsync & 0x02 == 0 && val & 0x02 != 0 {
                    self.scanline = 0;
                    self.frame_complete = true;
                }
                self.vsync = val;
            }
            0x01 => {
                if val & 0x80 != 0 {
                    self.paddle_counter = 0;
                    self.paddle_dumped = true;
                } else if self.paddle_dumped {
                    self.paddle_dumped = false;
                }
                self.vblank = val;
            }
            0x02 => self.wsync = true,
            0x03 => {}
            0x04 => self.nusiz0 = val,
            0x05 => self.nusiz1 = val,
            0x06 => self.colup0 = val,
            0x07 => self.colup1 = val,
            0x08 => self.colupf = val,
            0x09 => self.colubk = val,
            0x0A => self.ctrlpf = val,
            0x0B => self.refp0 = val & 0x08 != 0,
            0x0C => self.refp1 = val & 0x08 != 0,
            0x0D => self.pf0 = val,
            0x0E => self.pf1 = val,
            0x0F => self.pf2 = val,
            0x10 => self.pos_p0 = ((self.clock as i16 - 68 + 5).rem_euclid(160)) as u16,
            0x11 => self.pos_p1 = ((self.clock as i16 - 68 + 5).rem_euclid(160)) as u16,
            0x12 => self.pos_m0 = ((self.clock as i16 - 68 + 5).rem_euclid(160)) as u16,
            0x13 => self.pos_m1 = ((self.clock as i16 - 68 + 5).rem_euclid(160)) as u16,
            0x14 => self.pos_bl = ((self.clock as i16 - 68 + 5).rem_euclid(160)) as u16,
            0x15..=0x1A => {} // Audio — ignored
            0x1B => { self.grp0_old = self.grp0; self.grp0 = val; }
            0x1C => { self.grp1_old = self.grp1; self.grp1 = val; self.enabl_old = self.enabl; }
            0x1D => self.enam0 = val & 0x02 != 0,
            0x1E => self.enam1 = val & 0x02 != 0,
            0x1F => self.enabl = val & 0x02 != 0,
            0x20 => self.hmp0 = val,
            0x21 => self.hmp1 = val,
            0x22 => self.hmm0 = val,
            0x23 => self.hmm1 = val,
            0x24 => self.hmbl = val,
            0x25 => self.vdelp0 = val & 0x01 != 0,
            0x26 => self.vdelp1 = val & 0x01 != 0,
            0x27 => self.vdelbl = val & 0x01 != 0,
            0x28 => self.resmp0 = val & 0x02 != 0,
            0x29 => self.resmp1 = val & 0x02 != 0,
            0x2A => {
                self.pos_p0 = Self::apply_hmove_offset(self.pos_p0, self.hmp0);
                self.pos_p1 = Self::apply_hmove_offset(self.pos_p1, self.hmp1);
                self.pos_m0 = Self::apply_hmove_offset(self.pos_m0, self.hmm0);
                self.pos_m1 = Self::apply_hmove_offset(self.pos_m1, self.hmm1);
                self.pos_bl = Self::apply_hmove_offset(self.pos_bl, self.hmbl);
            }
            0x2B => {
                self.hmp0 = 0; self.hmp1 = 0;
                self.hmm0 = 0; self.hmm1 = 0;
                self.hmbl = 0;
            }
            0x2C => {
                self.cxm0p = 0; self.cxm1p = 0;
                self.cxp0fb = 0; self.cxp1fb = 0;
                self.cxm0fb = 0; self.cxm1fb = 0;
                self.cxblpf = 0; self.cxppmm = 0;
            }
            _ => {}
        }
    }
}
