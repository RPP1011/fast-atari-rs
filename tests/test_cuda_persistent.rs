#[cfg(feature = "cuda")]
mod multi_frame_tests {
    use fast_atari_rs::atari::{HeadlessAtari, Action};
    use fast_atari_rs::cuda_env::BatchAtariGpu;

    fn load_rom() -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Breakout.bin");
        std::fs::read(&path).expect("Place Breakout.bin in project root")
    }

    #[test]
    fn test_multi_frame_500() {
        let rom = load_rom();
        let n = 10;
        let k = 500;

        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.reset().unwrap();

        let mut cpus: Vec<HeadlessAtari> = (0..n)
            .map(|_| { let mut e = HeadlessAtari::new(rom.clone()); e.run_frame(); e })
            .collect();

        let action_seq = [0u8, 0, 2, 3, 4, 1, 0, 5, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8];

        // Build K*N action buffer
        let mut all_actions = Vec::with_capacity(k * n);
        for frame in 0..k {
            let a = action_seq[frame % action_seq.len()];
            for _ in 0..n {
                all_actions.push(a);
            }
        }

        // Run K frames on GPU in one kernel launch
        let gpu_obs = gpu.step_multi(&all_actions, k).unwrap();

        // Run K frames on CPU
        for frame in 0..k {
            let a = action_seq[frame % action_seq.len()];
            for cpu in &mut cpus {
                cpu.set_action(Action::from_index(a as usize));
                cpu.run_frame();
            }
        }

        // Compare final RAM
        for i in 0..n {
            let gpu_ram = &gpu_obs[i * 128..(i + 1) * 128];
            let cpu_ram = cpus[i].ram();
            for j in 0..128 {
                assert_eq!(cpu_ram[j], gpu_ram[j],
                    "Instance {} after {} frames: RAM[{:#04x}] mismatch (cpu={:#04x} gpu={:#04x})",
                    i, k, j + 0x80, cpu_ram[j], gpu_ram[j]);
            }
        }

        // Also check CPU registers
        let gpu_states = gpu.download_states().unwrap();
        for i in 0..n {
            let gs = &gpu_states[i];
            assert_eq!(cpus[i].cpu.a, gs.a, "Instance {} A mismatch", i);
            assert_eq!(cpus[i].cpu.x, gs.x, "Instance {} X mismatch", i);
            assert_eq!(cpus[i].cpu.y, gs.y, "Instance {} Y mismatch", i);
            assert_eq!(cpus[i].cpu.sp, gs.sp, "Instance {} SP mismatch", i);
            assert_eq!(cpus[i].cpu.pc, gs.pc, "Instance {} PC mismatch", i);
            assert_eq!(cpus[i].cpu.status.0, gs.status, "Instance {} status mismatch", i);
        }
    }

    #[test]
    fn test_multi_frame_varied_actions() {
        let rom = load_rom();
        let n = 18;
        let k = 100;

        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.reset().unwrap();

        let mut cpus: Vec<HeadlessAtari> = (0..n)
            .map(|_| { let mut e = HeadlessAtari::new(rom.clone()); e.run_frame(); e })
            .collect();

        // Each instance gets its own action each frame
        let mut all_actions = Vec::with_capacity(k * n);
        for _frame in 0..k {
            for i in 0..n {
                all_actions.push(i as u8);
            }
        }

        let gpu_obs = gpu.step_multi(&all_actions, k).unwrap();

        for _frame in 0..k {
            for i in 0..n {
                cpus[i].set_action(Action::from_index(i));
                cpus[i].run_frame();
            }
        }

        for i in 0..n {
            let gpu_ram = &gpu_obs[i * 128..(i + 1) * 128];
            let cpu_ram = cpus[i].ram();
            for j in 0..128 {
                assert_eq!(cpu_ram[j], gpu_ram[j],
                    "Instance {} after {} frames: RAM[{:#04x}] mismatch", i, k, j + 0x80);
            }
        }
    }
}
