/// MOS 6532 RIOT (RAM-I/O-Timer) — the "PIA" of the Atari 2600.
///
/// Provides:
///   - 128 bytes of RAM ($80–$FF, mirrored at $180–$1FF)
///   - Two 8-bit I/O ports (A and B) with data direction registers
///   - An interval timer with selectable prescaler
///
/// In the 2600:
///   - Port A ($280 SWCHA) = joystick inputs (active low)
///   - Port B ($282 SWCHB) = console switches (active low)
///   - Timer is used for game timing
///
/// Address decoding (active bits A0–A4, active at $280–$29F with mirrors):
///   $280 SWCHA   — Port A data (read: pins & ~DDR | output & DDR)
///   $281 SWACNT  — Port A data direction (0=input, 1=output)
///   $282 SWCHB   — Port B data
///   $283 SWBCNT  — Port B data direction
///   $284 INTIM   — Timer output (read)
///   $285 INSTAT  — Timer interrupt status (read, bit 7 = underflow)
///   $294 TIM1T   — Set 1-clock interval (write)
///   $295 TIM8T   — Set 8-clock interval (write)
///   $296 TIM64T  — Set 64-clock interval (write)
///   $297 T1024T  — Set 1024-clock interval (write)

#[derive(Clone, Debug)]
pub struct Pia {
    /// 128 bytes of internal RAM.
    pub ram: [u8; 128],

    // I/O ports
    port_a_output: u8,
    port_a_ddr: u8,
    port_a_input: u8,

    port_b_output: u8,
    port_b_ddr: u8,
    port_b_input: u8,

    // Timer
    timer_value: u16,
    timer_prescaler: u16,
    timer_prescaler_select: u16,
    timer_underflow: bool,
}

impl Default for Pia {
    fn default() -> Self {
        Self {
            ram: [0; 128],
            port_a_output: 0,
            port_a_ddr: 0,
            port_a_input: 0xFF, // all high (no joystick pressed)
            port_b_output: 0,
            port_b_ddr: 0,
            port_b_input: 0x3F, // default console switches
            timer_value: 0,
            timer_prescaler: 1,
            timer_prescaler_select: 1,
            timer_underflow: false,
        }
    }
}

impl Pia {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the external input pins for port A (joystick directions).
    pub fn set_port_a_input(&mut self, val: u8) {
        self.port_a_input = val;
    }

    /// Set the external input pins for port B (console switches).
    pub fn set_port_b_input(&mut self, val: u8) {
        self.port_b_input = val;
    }

    /// Tick the timer by one CPU cycle.
    pub fn tick(&mut self) {
        if self.timer_value == 0 && self.timer_underflow {
            // After underflow, timer counts down at 1-clock rate
            // (wrapping through $FF, $FE, ...)
            self.timer_value = 0xFF;
            return;
        }

        self.timer_prescaler -= 1;
        if self.timer_prescaler == 0 {
            self.timer_prescaler = if self.timer_underflow {
                1
            } else {
                self.timer_prescaler_select
            };

            if self.timer_value == 0 {
                self.timer_underflow = true;
                self.timer_value = 0xFF;
            } else {
                self.timer_value -= 1;
            }
        }
    }

    /// Advance the timer by `n` CPU cycles in bulk.
    /// Equivalent to calling `tick()` n times, but avoids the per-cycle loop.
    pub fn tick_n(&mut self, n: u16) {
        if n == 0 { return; }

        // Post-underflow mode: timer just wraps through 0xFF each cycle
        if self.timer_value == 0 && self.timer_underflow {
            // Each tick sets timer_value = 0xFF, then next tick decrements.
            // After n cycles: value = 0xFF - (n-1) wrapped to u8
            self.timer_value = (0xFF_u16.wrapping_sub(n - 1)) & 0xFF;
            return;
        }

        let mut remaining = n;

        // Phase 1: Consume remaining prescaler ticks before first timer decrement
        if self.timer_prescaler > 0 {
            if remaining < self.timer_prescaler {
                self.timer_prescaler -= remaining;
                return;
            }
            remaining -= self.timer_prescaler;
            self.timer_prescaler = 0;
            // One timer tick happens now
            if self.timer_value == 0 {
                self.timer_underflow = true;
                self.timer_value = 0xFF;
                // Switch to 1-clock mode for remaining cycles
                if remaining > 0 {
                    self.timer_value = (0xFF_u16.wrapping_sub(remaining - 1)) & 0xFF;
                }
                return;
            }
            self.timer_value -= 1;
        }

        if remaining == 0 {
            self.timer_prescaler = self.timer_prescaler_select;
            return;
        }

        // Phase 2: Full prescaler periods
        let ps = self.timer_prescaler_select;
        let full_decrements = remaining / ps;
        let leftover = remaining % ps;

        if full_decrements >= self.timer_value as u16 {
            // Will underflow during this batch
            let cycles_to_zero = self.timer_value as u16 * ps;
            remaining -= cycles_to_zero;
            self.timer_value = 0;
            self.timer_underflow = true;
            self.timer_value = 0xFF;
            // Remaining cycles in 1-clock post-underflow mode
            if remaining > 0 {
                self.timer_value = (0xFF_u16.wrapping_sub(remaining - 1)) & 0xFF;
            }
        } else {
            self.timer_value -= full_decrements as u16;
            self.timer_prescaler = ps - leftover;
        }
    }

    /// Read a PIA register or RAM. `addr` is the CPU address ($80–$2FF range).
    pub fn read(&mut self, addr: u16) -> u8 {
        if addr & 0x200 == 0 {
            // RAM: $80–$FF (bit 9 clear, bit 7 set)
            self.ram[(addr & 0x7F) as usize]
        } else {
            // I/O registers: $280–$29F
            match addr & 0x07 {
                // SWCHA — Port A data
                0x00 => (self.port_a_input & !self.port_a_ddr)
                      | (self.port_a_output & self.port_a_ddr),
                // SWACNT — Port A DDR
                0x01 => self.port_a_ddr,
                // SWCHB — Port B data
                0x02 => (self.port_b_input & !self.port_b_ddr)
                      | (self.port_b_output & self.port_b_ddr),
                // SWBCNT — Port B DDR
                0x03 => self.port_b_ddr,
                // INTIM — Timer value
                0x04 => {
                    self.timer_underflow = false;
                    self.timer_value as u8
                }
                // INSTAT — Timer status (bit 7 = underflow occurred)
                0x05 => {
                    let val = if self.timer_underflow { 0x80 } else { 0x00 };
                    val
                }
                _ => 0,
            }
        }
    }

    /// Write a PIA register or RAM. `addr` is the CPU address ($80–$2FF range).
    pub fn write(&mut self, addr: u16, val: u8) {
        if addr & 0x200 == 0 {
            // RAM
            self.ram[(addr & 0x7F) as usize] = val;
        } else if addr & 0x10 != 0 {
            // Timer set: $294–$297 (bit 4 set on writes)
            self.timer_value = val as u16;
            self.timer_underflow = false;
            self.timer_prescaler_select = match addr & 0x03 {
                0x00 => 1,    // TIM1T
                0x01 => 8,    // TIM8T
                0x02 => 64,   // TIM64T
                0x03 => 1024, // T1024T
                _ => unreachable!(),
            };
            self.timer_prescaler = self.timer_prescaler_select;
        } else {
            // I/O registers: $280–$283
            match addr & 0x03 {
                0x00 => self.port_a_output = val,
                0x01 => self.port_a_ddr = val,
                0x02 => self.port_b_output = val,
                0x03 => self.port_b_ddr = val,
                _ => unreachable!(),
            }
        }
    }
}
