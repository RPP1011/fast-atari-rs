/// Verify dispatch table produces identical results to Cpu::step.
///
/// Runs both the original Atari (using Cpu::step) and HeadlessAtari (using
/// dispatch table) on the same ROM with the same inputs, comparing CPU state
/// after every frame.

use fast_atari_rs::atari::{Action, Atari, HeadlessAtari};

#[test]
fn dispatch_matches_step_breakout() {
    let rom = std::fs::read("Breakout.bin").expect("Breakout.bin not found");

    let mut original = Atari::new(rom.clone());
    let mut dispatch = HeadlessAtari::new(rom);

    let actions = [Action::Noop, Action::Fire, Action::Left, Action::Right,
                   Action::Up, Action::Down, Action::UpFire, Action::DownFire];

    for frame in 0..500 {
        let action = actions[frame % actions.len()];
        original.set_action(action);
        dispatch.set_action(action);

        let cycles_orig = original.run_frame();
        let cycles_disp = dispatch.run_frame();

        // Compare CPU state
        assert_eq!(original.cpu.a, dispatch.cpu.a,
            "frame {}: A register mismatch (orig={:#04x}, disp={:#04x})",
            frame, original.cpu.a, dispatch.cpu.a);
        assert_eq!(original.cpu.x, dispatch.cpu.x,
            "frame {}: X register mismatch", frame);
        assert_eq!(original.cpu.y, dispatch.cpu.y,
            "frame {}: Y register mismatch", frame);
        assert_eq!(original.cpu.sp, dispatch.cpu.sp,
            "frame {}: SP mismatch", frame);
        assert_eq!(original.cpu.pc, dispatch.cpu.pc,
            "frame {}: PC mismatch (orig={:#06x}, disp={:#06x})",
            frame, original.cpu.pc, dispatch.cpu.pc);
        assert_eq!(original.cpu.status.0, dispatch.cpu.status.0,
            "frame {}: status flags mismatch (orig={:#04x}, disp={:#04x})",
            frame, original.cpu.status.0, dispatch.cpu.status.0);

        // Compare PIA RAM
        let orig_ram = &original.bus.pia.ram;
        let disp_ram = dispatch.ram();
        assert_eq!(orig_ram.as_slice(), disp_ram,
            "frame {}: PIA RAM mismatch", frame);

        // Cycle counts may differ slightly due to TIA timing differences
        // (rendering TIA ticks per-clock, headless batches), so we check
        // they're within a small tolerance
        let diff = (cycles_orig as i64 - cycles_disp as i64).unsigned_abs();
        assert!(diff < 100,
            "frame {}: cycle count diverged too much (orig={}, disp={}, diff={})",
            frame, cycles_orig, cycles_disp, diff);
    }
}
