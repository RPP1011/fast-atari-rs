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
pub const VISIBLE_TOP: u16 = 40; // first visible scanline (after VBLANK)
pub const VISIBLE_HEIGHT: u16 = 192;
pub const FRAME_WIDTH: usize = 160;
pub const FRAME_HEIGHT: usize = VISIBLE_HEIGHT as usize;

#[derive(Clone, Debug)]
pub struct Tia {
    // Scanline tracking
    pub clock: u16,     // TIA clock within current scanline (0–227)
    pub scanline: u16,  // current scanline (0–261)
    pub wsync: bool,    // CPU halted until end of scanline

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

    // Input latches
    pub inpt4: bool, // P0 fire button (active low)
    pub inpt5: bool, // P1 fire button (active low)
}

impl Default for Tia {
    fn default() -> Self {
        Self {
            clock: 0,
            scanline: 0,
            wsync: false,
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

    /// Advance the TIA by one TIA clock (3 per CPU cycle).
    /// Renders pixels and tracks scanline/frame progress.
    pub fn tick(&mut self) {
        let pixel_x = self.clock.wrapping_sub(68); // visible area starts at TIA clock 68

        // Render pixel if in visible area
        if self.clock >= 68 && pixel_x < FRAME_WIDTH as u16 {
            let vis_line = self.scanline.wrapping_sub(VISIBLE_TOP);
            if vis_line < VISIBLE_HEIGHT {
                let idx = vis_line as usize * FRAME_WIDTH + pixel_x as usize;
                self.framebuffer[idx] = self.render_pixel(pixel_x as u8);
            }
        }

        self.clock += 1;
        if self.clock >= 228 {
            self.clock = 0;
            self.scanline += 1;
            self.wsync = false;

            if self.scanline >= SCANLINES_PER_FRAME {
                self.scanline = 0;
                self.frame_complete = true;
            }
        }
    }

    /// Determine the color index for the given visible pixel column.
    fn render_pixel(&self, x: u8) -> u8 {
        // Playfield
        let pf_bit = self.get_playfield_bit(x);

        // Priority: CTRLPF bit 2 controls whether PF/BL have priority over players
        let pf_priority = self.ctrlpf & 0x04 != 0;

        // Player graphics
        let p0 = self.get_player_pixel(0, x);
        let p1 = self.get_player_pixel(1, x);
        let bl = self.enabl && self.is_object_at(self.pos_bl, 1, x);

        if pf_priority {
            if pf_bit || bl { self.colupf }
            else if p0 { self.colup0 }
            else if p1 { self.colup1 }
            else { self.colubk }
        } else {
            if p0 { self.colup0 }
            else if p1 { self.colup1 }
            else if pf_bit || bl { self.colupf }
            else { self.colubk }
        }
    }

    fn get_playfield_bit(&self, x: u8) -> bool {
        // Playfield is 20 bits wide, mirrored or repeated for right half
        let pf_x = if x < 80 { x / 4 } else {
            if self.ctrlpf & 0x01 != 0 {
                // Reflected
                19 - ((x - 80) / 4)
            } else {
                (x - 80) / 4
            }
        };

        match pf_x {
            0..=3   => self.pf0 & (0x10 << (pf_x)) != 0,
            4..=11  => self.pf1 & (0x80 >> (pf_x - 4)) != 0,
            12..=19 => self.pf2 & (0x01 << (pf_x - 12)) != 0,
            _ => false,
        }
    }

    fn get_player_pixel(&self, player: u8, x: u8) -> bool {
        let (grp, pos, refp) = match player {
            0 => (if self.vdelp0 { self.grp0_old } else { self.grp0 },
                  self.pos_p0, self.refp0),
            _ => (if self.vdelp1 { self.grp1_old } else { self.grp1 },
                  self.pos_p1, self.refp1),
        };

        if grp == 0 { return false; }

        let offset = (x as u16).wrapping_sub(pos) % 160;
        if offset >= 8 { return false; }

        if refp {
            grp & (0x01 << offset) != 0
        } else {
            grp & (0x80 >> offset) != 0
        }
    }

    fn is_object_at(&self, obj_pos: u16, _size: u8, x: u8) -> bool {
        let offset = (x as u16).wrapping_sub(obj_pos) % 160;
        offset == 0
    }

    fn apply_hmove_offset(pos: u16, hm: u8) -> u16 {
        // HM values are 4-bit signed: -8 to +7 (stored in upper nibble)
        let offset = ((hm >> 4) as i8) >> 0; // sign-extend the upper nibble
        // Negate because HM convention: positive = left, negative = right
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
            0x08..=0x0B => 0x00, // paddle inputs (not implemented)
            0x0C => if self.inpt4 { 0x80 } else { 0x00 },
            0x0D => if self.inpt5 { 0x80 } else { 0x00 },
            _ => 0,
        }
    }

    /// Write a TIA register. `addr` is the raw CPU address ($00–$2C range).
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr & 0x3F {
            0x00 => self.vsync = val,
            0x01 => self.vblank = val,
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
            0x10 => self.pos_p0 = ((self.clock as i16 - 68).rem_euclid(160)) as u16,
            0x11 => self.pos_p1 = ((self.clock as i16 - 68).rem_euclid(160)) as u16,
            0x12 => self.pos_m0 = ((self.clock as i16 - 68).rem_euclid(160)) as u16,
            0x13 => self.pos_m1 = ((self.clock as i16 - 68).rem_euclid(160)) as u16,
            0x14 => self.pos_bl = ((self.clock as i16 - 68).rem_euclid(160)) as u16,
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
                // HMOVE — apply horizontal motion
                self.pos_p0 = Self::apply_hmove_offset(self.pos_p0, self.hmp0);
                self.pos_p1 = Self::apply_hmove_offset(self.pos_p1, self.hmp1);
                self.pos_m0 = Self::apply_hmove_offset(self.pos_m0, self.hmm0);
                self.pos_m1 = Self::apply_hmove_offset(self.pos_m1, self.hmm1);
                self.pos_bl = Self::apply_hmove_offset(self.pos_bl, self.hmbl);
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
