#[cfg(feature = "cuda")]
mod ldg_tests {
    use fast_atari_rs::atari::{HeadlessAtari, Action};
    use fast_atari_rs::cuda_env::{BatchAtariGpu, KernelVariant};

    fn load_rom() -> Vec<u8> {
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Breakout.bin")).unwrap()
    }

    #[test]
    fn test_ldg_500_frames() {
        let rom = load_rom();
        let n = 10;
        let mut gpu = BatchAtariGpu::new(rom.clone(), n).unwrap();
        gpu.set_variant(KernelVariant::Ldg);
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
                        "Inst {} frame {}: RAM[{:#04x}] mismatch", i, frame+1, j+0x80);
                }
            }
        }
    }
}
