/// CUDA-accelerated batch Atari environment.

use cudarc::driver::*;
use cudarc::driver::result::DriverError;
use std::sync::Arc;

use crate::atari::{HeadlessAtari, BankScheme};

/// GPU-side AtariState must match the C struct layout exactly.
#[repr(C)]
#[derive(Clone)]
pub struct AtariStateGpu {
    pub a: u8, pub x: u8, pub y: u8, pub sp: u8,
    pub pc: u16, pub status: u8, pub _pad0: u8,
    pub ram: [u8; 128],
    pub port_a_output: u8, pub port_a_ddr: u8, pub port_a_input: u8,
    pub port_b_output: u8, pub port_b_ddr: u8, pub port_b_input: u8,
    pub timer_value: u16, pub timer_prescaler: u16, pub timer_prescaler_select: u16,
    pub timer_underflow: u8, pub _pad1: u8, pub _pad2: u8, pub _pad3: u8,
    pub tia_clock: u16, pub tia_scanline: u16,
    pub tia_wsync: u8, pub tia_frame_complete: u8, pub tia_vsync: u8, pub tia_vblank: u8,
    pub cxm0p: u8, pub cxm1p: u8, pub cxp0fb: u8, pub cxp1fb: u8,
    pub cxm0fb: u8, pub cxm1fb: u8, pub cxblpf: u8, pub cxppmm: u8,
    pub inpt4: u8, pub inpt5: u8,
    pub paddle0: u8, pub paddle1: u8, pub paddle_counter: u16,
    pub paddle_dumped: u8, pub _pad4: u8,
    pub pos_p0: u16, pub pos_p1: u16, pub pos_m0: u16, pub pos_m1: u16, pub pos_bl: u16,
    pub hmp0: u8, pub hmp1: u8, pub hmm0: u8, pub hmm1: u8, pub hmbl: u8, pub _pad5: u8,
    pub grp0: u8, pub grp1: u8, pub grp0_old: u8, pub grp1_old: u8,
    pub enam0: u8, pub enam1: u8, pub enabl: u8, pub enabl_old: u8,
    pub vdelp0: u8, pub vdelp1: u8, pub vdelbl: u8,
    pub resmp0: u8, pub resmp1: u8, pub nusiz0: u8, pub nusiz1: u8,
    pub colup0: u8, pub colup1: u8, pub colupf: u8, pub colubk: u8,
    pub ctrlpf: u8, pub refp0: u8, pub refp1: u8,
    pub pf0: u8, pub pf1: u8, pub pf2: u8,
    pub bank: u8, pub scheme: u8,
    pub _pad_end: [u8; 2],
}

unsafe impl DeviceRepr for AtariStateGpu {}
unsafe impl ValidAsZeroBits for AtariStateGpu {}

impl AtariStateGpu {
    pub fn from_headless(atari: &HeadlessAtari) -> Self {
        let cpu = &atari.cpu; let bus = &atari.bus;
        let tia = &bus.tia; let pia = &bus.pia;
        Self {
            a: cpu.a, x: cpu.x, y: cpu.y, sp: cpu.sp,
            pc: cpu.pc, status: cpu.status.0, _pad0: 0,
            ram: pia.ram,
            port_a_output: pia.port_a_output, port_a_ddr: pia.port_a_ddr,
            port_a_input: pia.port_a_input,
            port_b_output: pia.port_b_output, port_b_ddr: pia.port_b_ddr,
            port_b_input: pia.port_b_input,
            timer_value: pia.timer_value, timer_prescaler: pia.timer_prescaler,
            timer_prescaler_select: pia.timer_prescaler_select,
            timer_underflow: pia.timer_underflow as u8,
            _pad1: 0, _pad2: 0, _pad3: 0,
            tia_clock: tia.clock, tia_scanline: tia.scanline,
            tia_wsync: tia.wsync as u8, tia_frame_complete: tia.frame_complete as u8,
            tia_vsync: tia.vsync, tia_vblank: tia.vblank,
            cxm0p: tia.cxm0p, cxm1p: tia.cxm1p,
            cxp0fb: tia.cxp0fb, cxp1fb: tia.cxp1fb,
            cxm0fb: tia.cxm0fb, cxm1fb: tia.cxm1fb,
            cxblpf: tia.cxblpf, cxppmm: tia.cxppmm,
            inpt4: tia.inpt4 as u8, inpt5: tia.inpt5 as u8,
            paddle0: tia.paddle0, paddle1: tia.paddle1,
            paddle_counter: tia.paddle_counter, paddle_dumped: tia.paddle_dumped as u8,
            _pad4: 0,
            pos_p0: tia.pos_p0, pos_p1: tia.pos_p1,
            pos_m0: tia.pos_m0, pos_m1: tia.pos_m1, pos_bl: tia.pos_bl,
            hmp0: tia.hmp0, hmp1: tia.hmp1,
            hmm0: tia.hmm0, hmm1: tia.hmm1, hmbl: tia.hmbl, _pad5: 0,
            grp0: tia.grp0, grp1: tia.grp1,
            grp0_old: tia.grp0_old, grp1_old: tia.grp1_old,
            enam0: tia.enam0 as u8, enam1: tia.enam1 as u8,
            enabl: tia.enabl as u8, enabl_old: tia.enabl_old as u8,
            vdelp0: tia.vdelp0 as u8, vdelp1: tia.vdelp1 as u8,
            vdelbl: tia.vdelbl as u8,
            resmp0: tia.resmp0 as u8, resmp1: tia.resmp1 as u8,
            nusiz0: tia.nusiz0, nusiz1: tia.nusiz1,
            colup0: tia.colup0, colup1: tia.colup1,
            colupf: tia.colupf, colubk: tia.colubk, ctrlpf: tia.ctrlpf,
            refp0: tia.refp0 as u8, refp1: tia.refp1 as u8,
            pf0: tia.pf0, pf1: tia.pf1, pf2: tia.pf2,
            bank: bus.bank as u8,
            scheme: match bus.scheme {
                BankScheme::Fixed => 0, BankScheme::F8 => 1,
                BankScheme::F6 => 2, BankScheme::F4 => 3,
            },
            _pad_end: [0; 2],
        }
    }
}

// ============================================================
// Batch GPU Atari environment (per-frame kernel, Phase 2)
// ============================================================
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KernelVariant {
    Default,
    Sorted,
    Ldg,
}

pub struct BatchAtariGpu {
    dev: Arc<CudaDevice>,
    n: usize,
    rom: Vec<u8>,
    d_states: CudaSlice<AtariStateGpu>,
    d_actions: CudaSlice<u8>,
    d_obs: CudaSlice<u8>,
    d_rom: CudaSlice<u8>,
    rom_len: u32,
    variant: KernelVariant,
}

impl BatchAtariGpu {
    pub fn new(rom: Vec<u8>, n: usize) -> Result<Self, DriverError> {
        let dev = CudaDevice::new(0)?;
        let ptx = include_str!(concat!(env!("OUT_DIR"), "/atari_kernel.ptx"));
        dev.load_ptx(
            cudarc::nvrtc::Ptx::from_src(ptx), "atari",
            &["atari_frame_kernel", "atari_multi_frame_kernel",
              "atari_frame_kernel_sorted", "atari_multi_frame_kernel_sorted",
              "atari_frame_kernel_ldg"],
        )?;
        let rom_len = rom.len() as u32;
        let d_rom = dev.htod_copy(rom.clone())?;
        let d_states = dev.alloc_zeros::<AtariStateGpu>(n)?;
        let d_actions = dev.alloc_zeros::<u8>(n)?;
        let d_obs = dev.alloc_zeros::<u8>(n * 128)?;
        Ok(Self { dev, n, rom, d_states, d_actions, d_obs, d_rom, rom_len, variant: KernelVariant::Default })
    }

    /// Set kernel variant.
    pub fn set_variant(&mut self, v: KernelVariant) { self.variant = v; }

    /// Convenience: enable sorted mode.
    pub fn set_sorted(&mut self, sorted: bool) {
        self.variant = if sorted { KernelVariant::Sorted } else { KernelVariant::Default };
    }

    pub fn reset(&mut self) -> Result<Vec<Vec<u8>>, DriverError> {
        let mut gpu_states = Vec::with_capacity(self.n);
        let mut obs = Vec::with_capacity(self.n);
        for _ in 0..self.n {
            let mut emu = HeadlessAtari::new(self.rom.clone());
            emu.run_frame();
            obs.push(emu.ram().to_vec());
            gpu_states.push(AtariStateGpu::from_headless(&emu));
        }
        self.dev.htod_copy_into(gpu_states, &mut self.d_states)?;
        Ok(obs)
    }

    pub fn step(&mut self, actions: &[u8]) -> Result<Vec<u8>, DriverError> {
        assert_eq!(actions.len(), self.n);
        self.dev.htod_copy_into(actions.to_vec(), &mut self.d_actions)?;
        let block_size = 128u32;
        let grid_size = ((self.n as u32) + block_size - 1) / block_size;
        let cfg = LaunchConfig {
            grid_dim: (grid_size, 1, 1), block_dim: (block_size, 1, 1), shared_mem_bytes: 0,
        };
        let name = match self.variant {
            KernelVariant::Default => "atari_frame_kernel",
            KernelVariant::Sorted => "atari_frame_kernel_sorted",
            KernelVariant::Ldg => "atari_frame_kernel_ldg",
        };
        let func = self.dev.get_func("atari", name).unwrap();
        unsafe {
            func.launch(cfg, (
                &mut self.d_states, &self.d_actions, &mut self.d_obs,
                &self.d_rom, self.rom_len, self.n as i32,
            ))?;
        }
        self.dev.dtoh_sync_copy(&self.d_obs)
    }

    /// Step K frames at once. State stays in GPU registers between frames.
    /// actions: K * N bytes, laid out as [frame0_action0..frame0_actionN, frame1_action0..].
    /// Returns obs from the LAST frame only (N * 128 bytes).
    pub fn step_multi(&mut self, actions: &[u8], k: usize) -> Result<Vec<u8>, DriverError> {
        assert_eq!(actions.len(), k * self.n);
        // Allocate (or reuse) actions buffer for K*N
        let d_actions_multi = self.dev.htod_copy(actions.to_vec())?;
        let block_size = 128u32;
        let grid_size = ((self.n as u32) + block_size - 1) / block_size;
        let cfg = LaunchConfig {
            grid_dim: (grid_size, 1, 1), block_dim: (block_size, 1, 1), shared_mem_bytes: 0,
        };
        let name = match self.variant {
            KernelVariant::Sorted => "atari_multi_frame_kernel_sorted",
            _ => "atari_multi_frame_kernel",
        };
        let func = self.dev.get_func("atari", name).unwrap();
        unsafe {
            func.launch(cfg, (
                &mut self.d_states, &d_actions_multi, &mut self.d_obs,
                &self.d_rom, self.rom_len, k as i32, self.n as i32,
            ))?;
        }
        self.dev.dtoh_sync_copy(&self.d_obs)
    }

    pub fn download_states(&self) -> Result<Vec<AtariStateGpu>, DriverError> {
        self.dev.dtoh_sync_copy(&self.d_states)
    }

    pub fn num_envs(&self) -> usize { self.n }
}
