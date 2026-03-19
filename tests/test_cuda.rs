#[cfg(feature = "cuda")]
mod cuda_tests {
    use fast_atari_rs::atari::{HeadlessAtari, Action};
    use fast_atari_rs::cuda_env::{BatchAtariGpu, AtariStateGpu};

    fn load_rom() -> Vec<u8> {
        // Try to load Breakout ROM from the project root
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Breakout.bin");
        std::fs::read(&path).unwrap_or_else(|_| {
            panic!("Cannot find ROM at {:?}. Place Breakout.bin in the project root.", path);
        })
    }

    /// Compare CPU register state between CPU and GPU emulators.
    fn compare_cpu_regs(cpu: &HeadlessAtari, gpu: &AtariStateGpu, frame: usize, idx: usize) {
        assert_eq!(cpu.cpu.a, gpu.a,
            "Instance {} frame {}: A mismatch (cpu={:#04x} gpu={:#04x})", idx, frame, cpu.cpu.a, gpu.a);
        assert_eq!(cpu.cpu.x, gpu.x,
            "Instance {} frame {}: X mismatch (cpu={:#04x} gpu={:#04x})", idx, frame, cpu.cpu.x, gpu.x);
        assert_eq!(cpu.cpu.y, gpu.y,
            "Instance {} frame {}: Y mismatch (cpu={:#04x} gpu={:#04x})", idx, frame, cpu.cpu.y, gpu.y);
        assert_eq!(cpu.cpu.sp, gpu.sp,
            "Instance {} frame {}: SP mismatch (cpu={:#04x} gpu={:#04x})", idx, frame, cpu.cpu.sp, gpu.sp);
        assert_eq!(cpu.cpu.pc, gpu.pc,
            "Instance {} frame {}: PC mismatch (cpu={:#06x} gpu={:#06x})", idx, frame, cpu.cpu.pc, gpu.pc);
        assert_eq!(cpu.cpu.status.0, gpu.status,
            "Instance {} frame {}: Status mismatch (cpu={:#04x} gpu={:#04x})", idx, frame, cpu.cpu.status.0, gpu.status);
    }

    /// Compare PIA RAM between CPU and GPU.
    fn compare_ram(cpu_ram: &[u8], gpu_ram: &[u8], frame: usize, idx: usize) {
        for i in 0..128 {
            assert_eq!(cpu_ram[i], gpu_ram[i],
                "Instance {} frame {}: RAM[{:#04x}] mismatch (cpu={:#04x} gpu={:#04x})",
                idx, frame, i + 0x80, cpu_ram[i], gpu_ram[i]);
        }
    }

    #[test]
    fn test_gpu_cpu_parity_single_frame() {
        let rom = load_rom();
        let n = 4;

        // Initialize GPU
        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.reset().unwrap();

        // Initialize CPU emulators identically
        let mut cpus: Vec<HeadlessAtari> = (0..n)
            .map(|_| {
                let mut e = HeadlessAtari::new(rom.clone());
                e.run_frame();
                e
            })
            .collect();

        // Step both with Noop
        let actions = vec![0u8; n]; // Noop
        let gpu_obs = gpu.step(&actions).unwrap();
        let gpu_states = gpu.download_states().unwrap();

        for i in 0..n {
            cpus[i].set_action(Action::Noop);
            cpus[i].run_frame();

            // Compare RAM from observation buffer
            let gpu_ram = &gpu_obs[i * 128..(i + 1) * 128];
            compare_ram(cpus[i].ram(), gpu_ram, 1, i);

            // Compare CPU registers
            compare_cpu_regs(&cpus[i], &gpu_states[i], 1, i);
        }
    }

    #[test]
    fn test_gpu_cpu_parity_500_frames() {
        let rom = load_rom();
        let n = 10;

        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.reset().unwrap();

        let mut cpus: Vec<HeadlessAtari> = (0..n)
            .map(|_| {
                let mut e = HeadlessAtari::new(rom.clone());
                e.run_frame();
                e
            })
            .collect();

        // Action sequence: cycle through different actions
        let action_sequence = [0u8, 0, 2, 3, 4, 1, 0, 5, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8];

        for frame in 0..500 {
            let action_idx = action_sequence[frame % action_sequence.len()];
            let actions = vec![action_idx; n];

            let gpu_obs = gpu.step(&actions).unwrap();

            for i in 0..n {
                cpus[i].set_action(Action::from_index(action_idx as usize));
                cpus[i].run_frame();

                let gpu_ram = &gpu_obs[i * 128..(i + 1) * 128];
                compare_ram(cpus[i].ram(), gpu_ram, frame + 1, i);
            }

            // Spot-check CPU registers at key frames
            if frame == 0 || frame == 49 || frame == 99 || frame == 249 || frame == 499 {
                let gpu_states = gpu.download_states().unwrap();
                for i in 0..n {
                    compare_cpu_regs(&cpus[i], &gpu_states[i], frame + 1, i);
                }
            }
        }
    }

    #[test]
    fn test_gpu_cpu_parity_varied_actions() {
        let rom = load_rom();
        let n = 18; // One instance per action type

        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.reset().unwrap();

        let mut cpus: Vec<HeadlessAtari> = (0..n)
            .map(|_| {
                let mut e = HeadlessAtari::new(rom.clone());
                e.run_frame();
                e
            })
            .collect();

        // Each instance gets a different action
        for frame in 0..100 {
            let actions: Vec<u8> = (0..n).map(|i| i as u8).collect();
            let gpu_obs = gpu.step(&actions).unwrap();

            for i in 0..n {
                cpus[i].set_action(Action::from_index(i));
                cpus[i].run_frame();

                let gpu_ram = &gpu_obs[i * 128..(i + 1) * 128];
                compare_ram(cpus[i].ram(), gpu_ram, frame + 1, i);
            }
        }

        // Final register check
        let gpu_states = gpu.download_states().unwrap();
        for i in 0..n {
            compare_cpu_regs(&cpus[i], &gpu_states[i], 100, i);
        }
    }
}
