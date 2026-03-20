#[cfg(feature = "cuda")]
mod sorted_tests {
    use fast_atari_rs::atari::{HeadlessAtari, Action};
    use fast_atari_rs::cuda_env::BatchAtariGpu;

    fn load_rom() -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Breakout.bin");
        std::fs::read(&path).expect("Place Breakout.bin in project root")
    }

    fn compare_ram(cpu_ram: &[u8], gpu_ram: &[u8], frame: usize, idx: usize) {
        for j in 0..128 {
            assert_eq!(cpu_ram[j], gpu_ram[j],
                "Instance {} frame {}: RAM[{:#04x}] mismatch (cpu={:#04x} gpu={:#04x})",
                idx, frame, j + 0x80, cpu_ram[j], gpu_ram[j]);
        }
    }

    #[test]
    fn test_sorted_500_frames() {
        let rom = load_rom();
        let n = 10;

        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.set_sorted(true);
        gpu.reset().unwrap();

        let mut cpus: Vec<HeadlessAtari> = (0..n)
            .map(|_| { let mut e = HeadlessAtari::new(rom.clone()); e.run_frame(); e })
            .collect();

        let action_seq = [0u8, 0, 2, 3, 4, 1, 0, 5, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8];

        for frame in 0..500 {
            let a = action_seq[frame % action_seq.len()];
            let actions = vec![a; n];
            let gpu_obs = gpu.step(&actions).unwrap();

            for i in 0..n {
                cpus[i].set_action(Action::from_index(a as usize));
                cpus[i].run_frame();
                compare_ram(cpus[i].ram(), &gpu_obs[i*128..(i+1)*128], frame+1, i);
            }
        }

        // Register check
        let gpu_states = gpu.download_states().unwrap();
        for i in 0..n {
            assert_eq!(cpus[i].cpu.pc, gpu_states[i].pc,
                "Instance {} PC mismatch after 500 frames", i);
        }
    }

    #[test]
    fn test_sorted_multi_frame_varied() {
        let rom = load_rom();
        let n = 18;
        let k = 100;

        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.set_sorted(true);
        gpu.reset().unwrap();

        let mut cpus: Vec<HeadlessAtari> = (0..n)
            .map(|_| { let mut e = HeadlessAtari::new(rom.clone()); e.run_frame(); e })
            .collect();

        let mut all_actions = Vec::with_capacity(k * n);
        for _frame in 0..k {
            for i in 0..n { all_actions.push(i as u8); }
        }

        let gpu_obs = gpu.step_multi(&all_actions, k).unwrap();

        for _frame in 0..k {
            for i in 0..n {
                cpus[i].set_action(Action::from_index(i));
                cpus[i].run_frame();
            }
        }

        for i in 0..n {
            compare_ram(cpus[i].ram(), &gpu_obs[i*128..(i+1)*128], k, i);
        }
    }
}
