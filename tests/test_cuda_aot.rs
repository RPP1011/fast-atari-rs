#[cfg(feature = "cuda")]
mod aot_tests {
    use fast_atari_rs::atari::{HeadlessAtari, Action};
    use fast_atari_rs::cuda_env::{BatchAtariGpu, AtariStateGpu, KernelVariant};

    fn load_rom() -> Vec<u8> {
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Breakout.bin")).unwrap()
    }

    fn compare_cpu_regs(cpu: &HeadlessAtari, gpu: &AtariStateGpu, frame: usize, idx: usize) {
        assert_eq!(cpu.cpu.a, gpu.a,
            "Inst {} frame {}: A mismatch (cpu={:#04x} gpu={:#04x})", idx, frame, cpu.cpu.a, gpu.a);
        assert_eq!(cpu.cpu.x, gpu.x,
            "Inst {} frame {}: X mismatch (cpu={:#04x} gpu={:#04x})", idx, frame, cpu.cpu.x, gpu.x);
        assert_eq!(cpu.cpu.y, gpu.y,
            "Inst {} frame {}: Y mismatch (cpu={:#04x} gpu={:#04x})", idx, frame, cpu.cpu.y, gpu.y);
        assert_eq!(cpu.cpu.sp, gpu.sp,
            "Inst {} frame {}: SP mismatch (cpu={:#04x} gpu={:#04x})", idx, frame, cpu.cpu.sp, gpu.sp);
        assert_eq!(cpu.cpu.pc, gpu.pc,
            "Inst {} frame {}: PC mismatch (cpu={:#06x} gpu={:#06x})", idx, frame, cpu.cpu.pc, gpu.pc);
        assert_eq!(cpu.cpu.status.0, gpu.status,
            "Inst {} frame {}: Status mismatch (cpu={:#04x} gpu={:#04x})", idx, frame, cpu.cpu.status.0, gpu.status);
    }

    #[test]
    fn test_aot_500_frames() {
        let rom = load_rom();
        let n = 10;
        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.set_variant(KernelVariant::Aot).unwrap();
        gpu.reset().unwrap();

        let mut cpus: Vec<HeadlessAtari> = (0..n)
            .map(|_| { let mut e = HeadlessAtari::new(rom.clone()); e.run_frame(); e })
            .collect();

        let seq = [0u8, 0, 2, 3, 4, 1, 0, 5, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8];
        for frame in 0..500 {
            let a = seq[frame % seq.len()];
            let obs = gpu.step(&vec![a; n]).unwrap();
            for i in 0..n {
                cpus[i].set_action(Action::from_index(a as usize));
                cpus[i].run_frame();
                for j in 0..128 {
                    assert_eq!(cpus[i].ram()[j], obs[i*128+j],
                        "Inst {} frame {}: RAM[{:#04x}] mismatch (cpu={:#04x} gpu={:#04x})",
                        i, frame+1, j+0x80, cpus[i].ram()[j], obs[i*128+j]);
                }
            }

            if frame == 0 || frame == 49 || frame == 99 || frame == 249 || frame == 499 {
                let gpu_states = gpu.download_states().unwrap();
                for i in 0..n {
                    compare_cpu_regs(&cpus[i], &gpu_states[i], frame + 1, i);
                }
            }
        }
    }

    #[test]
    fn test_aot_varied_actions() {
        let rom = load_rom();
        let n = 18;
        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.set_variant(KernelVariant::Aot).unwrap();
        gpu.reset().unwrap();

        let mut cpus: Vec<HeadlessAtari> = (0..n)
            .map(|_| { let mut e = HeadlessAtari::new(rom.clone()); e.run_frame(); e })
            .collect();

        for frame in 0..100 {
            let actions: Vec<u8> = (0..n).map(|i| i as u8).collect();
            let obs = gpu.step(&actions).unwrap();
            for i in 0..n {
                cpus[i].set_action(Action::from_index(i));
                cpus[i].run_frame();
                for j in 0..128 {
                    assert_eq!(cpus[i].ram()[j], obs[i*128+j],
                        "Inst {} frame {}: RAM[{:#04x}] mismatch (cpu={:#04x} gpu={:#04x})",
                        i, frame+1, j+0x80, cpus[i].ram()[j], obs[i*128+j]);
                }
            }
        }

        let gpu_states = gpu.download_states().unwrap();
        for i in 0..n {
            compare_cpu_regs(&cpus[i], &gpu_states[i], 100, i);
        }
    }
}
