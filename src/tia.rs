/// Television Interface Adapter (TIA) — video/audio/input for the Atari 2600.
///
/// Address space: $00–$7F (active bits A0–A5, active at $00–$3F with mirrors).
///
/// Write registers ($00–$2C):
///   $00 VSYNC   — vertical sync set-clear
///   $01 VBLANK  — vertical blank set-clear
///   $02 WSYNC   — wait for horizontal sync (halts CPU until end of scanline)
///   $03 RSYNC   — reset horizontal sync counter
///   $04 NUSIZ0  — number-size player-missile 0
///   $05 NUSIZ1  — number-size player-missile 1
///   $06 COLUP0  — color-luminance player 0
///   $07 COLUP1  — color-luminance player 1
///   $08 COLUPF  — color-luminance playfield
///   $09 COLUBK  — color-luminance background
///   $0A CTRLPF  — control playfield ball size & collisions
///   $0B REFP0   — reflect player 0
///   $0C REFP1   — reflect player 1
///   $0D PF0     — playfield register byte 0
///   $0E PF1     — playfield register byte 1
///   $0F PF2     — playfield register byte 2
///   $10 RESP0   — reset player 0
///   $11 RESP1   — reset player 1
///   $12 RESM0   — reset missile 0
///   $13 RESM1   — reset missile 1
///   $14 RESBL   — reset ball
///   $15 AUDC0   — audio control 0
///   $16 AUDC1   — audio control 1
///   $17 AUDF0   — audio frequency 0
///   $18 AUDF1   — audio frequency 1
///   $19 AUDV0   — audio volume 0
///   $1A AUDV1   — audio volume 1
///   $1B GRP0    — graphics player 0
///   $1C GRP1    — graphics player 1
///   $1D ENAM0   — enable missile 0
///   $1E ENAM1   — enable missile 1
///   $1F ENABL   — enable ball
///   $20 HMP0    — horizontal motion player 0
///   $21 HMP1    — horizontal motion player 1
///   $22 HMM0    — horizontal motion missile 0
///   $23 HMM1    — horizontal motion missile 1
///   $24 HMBL    — horizontal motion ball
///   $25 VDELP0  — vertical delay player 0
///   $26 VDELP1  — vertical delay player 1
///   $27 VDELBL  — vertical delay ball
///   $28 RESMP0  — reset missile 0 to player 0
///   $29 RESMP1  — reset missile 1 to player 1
///   $2A HMOVE   — apply horizontal motion
///   $2B HMCLR   — clear horizontal motion registers
///   $2C CXCLR   — clear collision latches
///
/// Read registers ($00–$0D, active at $30–$3D):
///   $30 CXM0P   — collision M0-P1, M0-P0
///   $31 CXM1P   — collision M1-P0, M1-P1
///   $32 CXP0FB  — collision P0-PF, P0-BL
///   $33 CXP1FB  — collision P1-PF, P1-BL
///   $34 CXM0FB  — collision M0-PF, M0-BL
///   $35 CXM1FB  — collision M1-PF, M1-BL
///   $36 CXBLPF  — collision BL-PF
///   $37 CXPPMM  — collision P0-P1, M0-M1
///   $38 INPT0   — paddle 0 input
///   $39 INPT1   — paddle 1 input
///   $3A INPT2   — paddle 2 input
///   $3B INPT3   — paddle 3 input
///   $3C INPT4   — player 0 fire button
///   $3D INPT5   — player 1 fire button

/// NTSC constants.
pub const SCANLINE_CYCLES: u16 = 76; // CPU clocks per scanline (228 TIA clocks / 3)
pub const SCANLINES_PER_FRAME: u16 = 262;
pub const VISIBLE_HEIGHT: u16 = 192;
pub const FRAME_WIDTH: usize = 160;
pub const FRAME_HEIGHT: usize = VISIBLE_HEIGHT as usize;
/// Display width after correcting for non-square TIA pixels (2x horizontal).
pub const DISPLAY_WIDTH: usize = FRAME_WIDTH * 2;

#[derive(Clone, Debug)]
pub struct Tia {
    // Scanline tracking
    pub clock: u16,       // TIA clock within current scanline (0–227)
    pub scanline: u16,    // current scanline (0–261)
    pub render_line: u16, // current framebuffer row being written (0–191)
    pub wsync: bool,      // CPU halted until end of scanline
    pub hmove_pending: bool, // HMOVE was written this scanline — extend HBLANK by 8 clocks

    // Framebuffer: 160 x 192, one byte per pixel (NTSC palette index)
    pub framebuffer: Box<[u8; FRAME_WIDTH * FRAME_HEIGHT]>,
    pub frame_complete: bool,

    // Registers
    pub vsync: u8,
    pub vblank: u8,
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
    pub grp0: u8,
    pub grp1: u8,
    pub grp0_old: u8,
    pub grp1_old: u8,
    pub enam0: bool,
    pub enam1: bool,
    pub enabl: bool,
    pub enabl_old: bool,
    pub hmp0: u8,
    pub hmp1: u8,
    pub hmm0: u8,
    pub hmm1: u8,
    pub hmbl: u8,
    pub nusiz0: u8,
    pub nusiz1: u8,
    pub vdelp0: bool,
    pub vdelp1: bool,
    pub vdelbl: bool,
    pub resmp0: bool,
    pub resmp1: bool,

    // Object positions (in TIA pixel coordinates, 0–159)
    pub pos_p0: u16,
    pub pos_p1: u16,
    pub pos_m0: u16,
    pub pos_m1: u16,
    pub pos_bl: u16,

    // Collision latches (bits 7 and 6 of each register)
    pub cxm0p: u8,
    pub cxm1p: u8,
    pub cxp0fb: u8,
    pub cxp1fb: u8,
    pub cxm0fb: u8,
    pub cxm1fb: u8,
    pub cxblpf: u8,
    pub cxppmm: u8,

    // Paddle inputs (0-255 position, converted to charge timing)
    pub paddle0: u8,
    pub paddle1: u8,
    pub paddle_counter: u16, // counts up each scanline after VBLANK dump
    pub paddle_dumped: bool,

    // Input latches
    pub inpt4: bool, // P0 fire button (active low)
    pub inpt5: bool, // P1 fire button (active low)
}

impl Default for Tia {
    fn default() -> Self {
        Self {
            clock: 0,
            scanline: 0,
            render_line: 0,
            wsync: false,
            hmove_pending: false,
            framebuffer: Box::new([0; FRAME_WIDTH * FRAME_HEIGHT]),
            frame_complete: false,
            vsync: 0,
            vblank: 0,
            colup0: 0, colup1: 0, colupf: 0, colubk: 0,
            ctrlpf: 0,
            refp0: false, refp1: false,
            pf0: 0, pf1: 0, pf2: 0,
            grp0: 0, grp1: 0,
            grp0_old: 0, grp1_old: 0,
            enam0: false, enam1: false,
            enabl: false, enabl_old: false,
            hmp0: 0, hmp1: 0, hmm0: 0, hmm1: 0, hmbl: 0,
            nusiz0: 0, nusiz1: 0,
            vdelp0: false, vdelp1: false, vdelbl: false,
            resmp0: false, resmp1: false,
            pos_p0: 0, pos_p1: 0,
            pos_m0: 0, pos_m1: 0, pos_bl: 0,
            cxm0p: 0, cxm1p: 0, cxp0fb: 0, cxp1fb: 0,
            cxm0fb: 0, cxm1fb: 0, cxblpf: 0, cxppmm: 0,
            paddle0: 128, paddle1: 128,
            paddle_counter: 0, paddle_dumped: false,
            inpt4: true, inpt5: true,
        }
    }
}

impl Tia {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set fire button state (active low: false = pressed).
    pub fn set_input(&mut self, player: u8, fire: bool) {
        match player {
            0 => self.inpt4 = fire,
            1 => self.inpt5 = fire,
            _ => {}
        }
    }

    /// Set paddle position (0 = full left, 255 = full right).
    pub fn set_paddle(&mut self, player: u8, position: u8) {
        match player {
            0 => self.paddle0 = position,
            1 => self.paddle1 = position,
            _ => {}
        }
    }

    /// Advance the TIA by one TIA clock (3 per CPU cycle).
    /// Renders pixels, detects collisions, and tracks scanline/frame progress.
    pub fn tick(&mut self) {
        // HBLANK boundary: normally clock 68, extended to 76 after HMOVE
        let hblank_end = if self.hmove_pending { 76 } else { 68 };
        let pixel_x = self.clock.wrapping_sub(hblank_end);

        // Render pixel if in visible area and not blanked
        if self.clock >= hblank_end && pixel_x < FRAME_WIDTH as u16
            && self.vblank & 0x02 == 0
            && self.render_line < FRAME_HEIGHT as u16
        {
            let x = pixel_x as u8;
            let (color, p0, p1, m0, m1, bl, pf) = self.compute_pixel(x);

            let idx = self.render_line as usize * FRAME_WIDTH + pixel_x as usize;
            self.framebuffer[idx] = color;

            // Collision detection — latch bits (only during visible area)
            if m0 && p1 { self.cxm0p |= 0x80; }
            if m0 && p0 { self.cxm0p |= 0x40; }
            if m1 && p0 { self.cxm1p |= 0x80; }
            if m1 && p1 { self.cxm1p |= 0x40; }
            if p0 && pf { self.cxp0fb |= 0x80; }
            if p0 && bl { self.cxp0fb |= 0x40; }
            if p1 && pf { self.cxp1fb |= 0x80; }
            if p1 && bl { self.cxp1fb |= 0x40; }
            if m0 && pf { self.cxm0fb |= 0x80; }
            if m0 && bl { self.cxm0fb |= 0x40; }
            if m1 && pf { self.cxm1fb |= 0x80; }
            if m1 && bl { self.cxm1fb |= 0x40; }
            if bl && pf { self.cxblpf |= 0x80; }
            if p0 && p1 { self.cxppmm |= 0x80; }
            if m0 && m1 { self.cxppmm |= 0x40; }
        }

        self.clock += 1;
        if self.clock >= 228 {
            self.clock = 0;
            self.scanline += 1;
            self.wsync = false;
            self.hmove_pending = false;

            if self.vblank & 0x02 == 0 && self.render_line < FRAME_HEIGHT as u16 {
                self.render_line += 1;
            }

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
    }

    /// Compute pixel color and per-object hit flags at visible column `x`.
    /// Returns (color, p0_hit, p1_hit, m0_hit, m1_hit, bl_hit, pf_hit).
    fn compute_pixel(&self, x: u8) -> (u8, bool, bool, bool, bool, bool, bool) {
        let pf = self.get_playfield_bit(x);
        let p0 = self.get_player_pixel(0, x);
        let p1 = self.get_player_pixel(1, x);
        let m0 = self.get_missile_pixel(0, x);
        let m1 = self.get_missile_pixel(1, x);
        let bl = self.get_ball_pixel(x);

        let pf_priority = self.ctrlpf & 0x04 != 0;
        let score_mode = self.ctrlpf & 0x02 != 0;

        let pf_color = if score_mode {
            if x < 80 { self.colup0 } else { self.colup1 }
        } else {
            self.colupf
        };

        let color = if pf_priority {
            if pf || bl { pf_color }
            else if p0 || m0 { self.colup0 }
            else if p1 || m1 { self.colup1 }
            else { self.colubk }
        } else {
            if p0 || m0 { self.colup0 }
            else if p1 || m1 { self.colup1 }
            else if pf || bl { pf_color }
            else { self.colubk }
        };

        (color, p0, p1, m0, m1, bl, pf)
    }

    fn get_playfield_bit(&self, x: u8) -> bool {
        let pf_x = if x < 80 { x / 4 } else {
            if self.ctrlpf & 0x01 != 0 {
                19 - ((x - 80) / 4)
            } else {
                (x - 80) / 4
            }
        };

        match pf_x {
            0..=3   => self.pf0 & (0x10 << pf_x) != 0,
            4..=11  => self.pf1 & (0x80 >> (pf_x - 4)) != 0,
            12..=19 => self.pf2 & (0x01 << (pf_x - 12)) != 0,
            _ => false,
        }
    }

    /// Player pixel test with NUSIZ support (copies and sizing).
    fn get_player_pixel(&self, player: u8, x: u8) -> bool {
        let (grp, pos, refp, nusiz) = match player {
            0 => (if self.vdelp0 { self.grp0_old } else { self.grp0 },
                  self.pos_p0, self.refp0, self.nusiz0),
            _ => (if self.vdelp1 { self.grp1_old } else { self.grp1 },
                  self.pos_p1, self.refp1, self.nusiz1),
        };

        if grp == 0 { return false; }

        let size_mode = nusiz & 0x07;
        // Player pixel width: 1 for normal, 2 for double, 4 for quad
        let pixel_width: u16 = match size_mode {
            5 => 2, // double-size
            7 => 4, // quad-size
            _ => 1, // normal
        };

        // Copy positions relative to base position
        let copy_offsets: &[u16] = match size_mode {
            0 => &[0],                // one copy
            1 => &[0, 16],            // two copies, close
            2 => &[0, 32],            // two copies, medium
            3 => &[0, 16, 32],        // three copies, close
            4 => &[0, 64],            // two copies, wide
            5 => &[0],                // one copy, double-size
            6 => &[0, 32, 64],        // three copies, medium
            7 => &[0],                // one copy, quad-size
            _ => &[0],
        };

        let pixel_span = 8 * pixel_width;
        for &copy_off in copy_offsets {
            let copy_pos = (pos + copy_off) % 160;
            let offset = ((x as u16) + 160 - copy_pos) % 160;
            if offset < pixel_span && offset < 80 {
                let bit_index = offset / pixel_width;
                let hit = if refp {
                    grp & (0x01 << bit_index) != 0
                } else {
                    grp & (0x80 >> bit_index) != 0
                };
                if hit { return true; }
            }
        }

        false
    }

    /// Missile pixel test with NUSIZ support (size and copies).
    fn get_missile_pixel(&self, missile: u8, x: u8) -> bool {
        let (enabled, pos, nusiz, resmp) = match missile {
            0 => (self.enam0, self.pos_m0, self.nusiz0, self.resmp0),
            _ => (self.enam1, self.pos_m1, self.nusiz1, self.resmp1),
        };

        if !enabled || resmp { return false; }

        // Missile width from NUSIZ bits 4-5
        let width: u16 = 1 << ((nusiz >> 4) & 0x03);

        // Missile copies follow the same pattern as its player
        let copy_offsets: &[u16] = match nusiz & 0x07 {
            0 | 5 | 7 => &[0],
            1 => &[0, 16],
            2 => &[0, 32],
            3 => &[0, 16, 32],
            4 => &[0, 64],
            6 => &[0, 32, 64],
            _ => &[0],
        };

        for &copy_off in copy_offsets {
            let copy_pos = (pos + copy_off) % 160;
            let offset = ((x as u16) + 160 - copy_pos) % 160;
            if offset < width && offset < 80 {
                return true;
            }
        }

        false
    }

    /// Ball pixel test with CTRLPF size.
    fn get_ball_pixel(&self, x: u8) -> bool {
        let enabled = if self.vdelbl { self.enabl_old } else { self.enabl };
        if !enabled { return false; }

        // Ball width from CTRLPF bits 4-5
        let width: u16 = 1 << ((self.ctrlpf >> 4) & 0x03);
        let offset = ((x as u16) + 160 - self.pos_bl) % 160;
        offset < width && offset < 80
    }

    fn apply_hmove_offset(pos: u16, hm: u8) -> u16 {
        // HM values are 4-bit signed in the upper nibble: -8 to +7
        // Arithmetic right shift of the signed byte gives proper sign extension
        let offset = (hm as i8) >> 4;
        // Positive = move left (subtract), negative = move right (add)
        ((pos as i16 - offset as i16).rem_euclid(160)) as u16
    }

    /// Read a TIA register. `addr` is the raw CPU address ($00–$0D range).
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
            // Paddle inputs: bit 7 goes high when paddle_counter >= paddle position
            0x08 => if self.paddle_counter >= self.paddle0 as u16 { 0x80 } else { 0x00 },
            0x09 => if self.paddle_counter >= self.paddle1 as u16 { 0x80 } else { 0x00 },
            0x0A => 0x80, // INPT2 — unused paddle, always charged
            0x0B => 0x80, // INPT3 — unused paddle, always charged
            0x0C => if self.inpt4 { 0x80 } else { 0x00 },
            0x0D => if self.inpt5 { 0x80 } else { 0x00 },
            _ => 0,
        }
    }

    /// Write a TIA register. `addr` is the raw CPU address ($00–$2C range).
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr & 0x3F {
            0x00 => {
                // When VSYNC is turned on (bit 1: 0→1), signal start of new frame
                if self.vsync & 0x02 == 0 && val & 0x02 != 0 {
                    self.scanline = 0;
                    self.frame_complete = true;
                }
                self.vsync = val;
            }
            0x01 => {
                // When VBLANK is turned off (bit 1: 1→0), start rendering visible area
                if self.vblank & 0x02 != 0 && val & 0x02 == 0 {
                    self.render_line = 0;
                    self.framebuffer.fill(0);
                }
                // Bit 7: dump paddle capacitors (reset charge timer)
                if val & 0x80 != 0 {
                    self.paddle_counter = 0;
                    self.paddle_dumped = true;
                } else if self.paddle_dumped {
                    self.paddle_dumped = false;
                    // Start charging — counter will increment each scanline
                }
                self.vblank = val;
            }
            0x02 => self.wsync = true,
            0x03 => {} // RSYNC — rarely used
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
            0x15 => {} // AUDC0
            0x16 => {} // AUDC1
            0x17 => {} // AUDF0
            0x18 => {} // AUDF1
            0x19 => {} // AUDV0
            0x1A => {} // AUDV1
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
                // HMOVE — apply horizontal motion and extend HBLANK by 8 clocks
                self.pos_p0 = Self::apply_hmove_offset(self.pos_p0, self.hmp0);
                self.pos_p1 = Self::apply_hmove_offset(self.pos_p1, self.hmp1);
                self.pos_m0 = Self::apply_hmove_offset(self.pos_m0, self.hmm0);
                self.pos_m1 = Self::apply_hmove_offset(self.pos_m1, self.hmm1);
                self.pos_bl = Self::apply_hmove_offset(self.pos_bl, self.hmbl);
                self.hmove_pending = true;
            }
            0x2B => {
                // HMCLR
                self.hmp0 = 0; self.hmp1 = 0;
                self.hmm0 = 0; self.hmm1 = 0;
                self.hmbl = 0;
            }
            0x2C => {
                // CXCLR
                self.cxm0p = 0; self.cxm1p = 0;
                self.cxp0fb = 0; self.cxp1fb = 0;
                self.cxm0fb = 0; self.cxm1fb = 0;
                self.cxblpf = 0; self.cxppmm = 0;
            }
            _ => {}
        }
    }
}
