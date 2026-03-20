#[cfg(feature = "cuda")]
mod compiled_tests {
    use fast_atari_rs::atari::{HeadlessAtari, Action};
    use fast_atari_rs::cuda_env::{BatchAtariGpu, AtariStateGpu, KernelVariant};
    use std::time::Instant;

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
    fn test_compiled_500_frames() {
        let rom = load_rom();
        let n = 10;
        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.set_variant(KernelVariant::Compiled).unwrap();
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
    fn test_compiled_varied_actions() {
        let rom = load_rom();
        let n = 18;
        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.set_variant(KernelVariant::Compiled).unwrap();
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

    #[test]
    fn test_compiled_1000_frames_all_actions() {
        let rom = load_rom();
        let n = 18; // one instance per action
        let frames = 1000;
        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.set_variant(KernelVariant::Compiled).unwrap();
        gpu.reset().unwrap();

        let mut cpus: Vec<HeadlessAtari> = (0..n)
            .map(|_| { let mut e = HeadlessAtari::new(rom.clone()); e.run_frame(); e })
            .collect();

        for frame in 0..frames {
            // Each instance gets its own action (0-17), cycling
            let actions: Vec<u8> = (0..n).map(|i| ((i + frame) % 18) as u8).collect();
            let obs = gpu.step(&actions).unwrap();
            for i in 0..n {
                cpus[i].set_action(Action::from_index(actions[i] as usize));
                cpus[i].run_frame();
                for j in 0..128 {
                    assert_eq!(cpus[i].ram()[j], obs[i*128+j],
                        "Inst {} frame {}: RAM[{:#04x}] mismatch (cpu={:#04x} gpu={:#04x})",
                        i, frame+1, j+0x80, cpus[i].ram()[j], obs[i*128+j]);
                }
            }

            if (frame + 1) % 250 == 0 {
                let gpu_states = gpu.download_states().unwrap();
                for i in 0..n {
                    compare_cpu_regs(&cpus[i], &gpu_states[i], frame + 1, i);
                }
                eprintln!("  Frame {} — all {} instances match", frame + 1, n);
            }
        }
    }

    #[test]
    fn test_compiled_vs_aot_1frame() {
        let rom = load_rom();
        let n = 1;

        // AOT reference
        let mut gpu_aot = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu_aot.set_variant(KernelVariant::Aot).unwrap();
        gpu_aot.reset().unwrap();
        let aot_obs = gpu_aot.step(&[0u8]).unwrap();
        let aot_state = gpu_aot.download_states().unwrap();

        // Compiled
        let mut gpu_comp = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu_comp.set_variant(KernelVariant::Compiled).unwrap();
        gpu_comp.reset().unwrap();
        let comp_obs = gpu_comp.step(&[0u8]).unwrap();
        let comp_state = gpu_comp.download_states().unwrap();

        // Compare
        let a = &aot_state[0];
        let c = &comp_state[0];
        println!("AOT:  A={:#04x} X={:#04x} Y={:#04x} SP={:#04x} PC={:#06x} ST={:#04x}",
            a.a, a.x, a.y, a.sp, a.pc, a.status);
        println!("COMP: A={:#04x} X={:#04x} Y={:#04x} SP={:#04x} PC={:#06x} ST={:#04x}",
            c.a, c.x, c.y, c.sp, c.pc, c.status);
        println!("AOT:  tia_clock={} scanline={} wsync={} vsync={:#04x} bank={}",
            a.tia_clock, a.tia_scanline, a.tia_wsync, a.tia_vsync, a.bank);
        println!("COMP: tia_clock={} scanline={} wsync={} vsync={:#04x} bank={}",
            c.tia_clock, c.tia_scanline, c.tia_wsync, c.tia_vsync, c.bank);

        let mut ram_diffs = 0;
        for i in 0..128 {
            if aot_obs[i] != comp_obs[i] {
                if ram_diffs < 10 {
                    println!("RAM[{:#04x}] AOT={:#04x} COMP={:#04x}", i + 0x80, aot_obs[i], comp_obs[i]);
                }
                ram_diffs += 1;
            }
        }
        println!("Total RAM diffs: {ram_diffs}");

        assert_eq!(a.a, c.a, "A register mismatch");
    }

    #[test]
    fn bench_compiled_vs_aot() {
        let rom = load_rom();

        for &(n, frames) in &[(1024, 100), (50_000, 1000)] {
            // AOT
            let mut gpu_aot = BatchAtariGpu::new(rom.clone(), n).unwrap();
            gpu_aot.set_variant(KernelVariant::Aot).unwrap();
            gpu_aot.reset().unwrap();
            let actions = vec![0u8; n];
            for _ in 0..10 { gpu_aot.step(&actions).unwrap(); }

            let t0 = Instant::now();
            for _ in 0..frames { gpu_aot.step(&actions).unwrap(); }
            let aot_ms = t0.elapsed().as_secs_f64() * 1000.0;
            let aot_fps = (n as f64 * frames as f64) / t0.elapsed().as_secs_f64();

            // Compiled
            let mut gpu_comp = BatchAtariGpu::new(rom.clone(), n).unwrap();
            gpu_comp.set_variant(KernelVariant::Compiled).unwrap();
            gpu_comp.reset().unwrap();
            for _ in 0..10 { gpu_comp.step(&actions).unwrap(); }

            let t0 = Instant::now();
            for _ in 0..frames { gpu_comp.step(&actions).unwrap(); }
            let comp_ms = t0.elapsed().as_secs_f64() * 1000.0;
            let comp_fps = (n as f64 * frames as f64) / t0.elapsed().as_secs_f64();

            println!("\n=== Compiled vs AOT ({n} instances, {frames} frames) ===");
            println!("  AOT:      {aot_ms:.1}ms  ({aot_fps:.0} fps)");
            println!("  Compiled: {comp_ms:.1}ms  ({comp_fps:.0} fps)");
            println!("  Speedup:  {:.2}x", aot_ms / comp_ms);
        }
    }
}
