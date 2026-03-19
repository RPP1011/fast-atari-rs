mod common;

use fast_atari_rs::atari::{Atari, Action};
use fast_atari_rs::tia::{FRAME_WIDTH, FRAME_HEIGHT};

#[test]
fn breakout_runs_and_renders() {
    let rom_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Breakout.bin");
    let rom = std::fs::read(rom_path).expect("Breakout.bin not found in project root");

    let mut atari = Atari::new(rom);

    // Run 300 frames to get past title screen
    for _ in 0..300 {
        atari.run_frame();
    }

    // Snapshot of title screen
    common::save_png("breakout_title.png", atari.framebuffer());

    // Press fire to start
    atari.set_action(Action::Fire);
    for _ in 0..30 {
        atari.run_frame();
    }
    atari.set_action(Action::Noop);
    for _ in 0..30 {
        atari.run_frame();
    }

    let fb = atari.framebuffer();
    let nonzero = fb.iter().filter(|&&p| p != 0).count();
    println!("Non-zero pixels: {} / {}", nonzero, FRAME_WIDTH * FRAME_HEIGHT);
    assert!(nonzero > 100, "framebuffer is mostly empty");

    common::save_png("breakout_game.png", fb);
    println!("Wrote breakout_title.png and breakout_game.png");
}

#[test]
fn breakout_gif() {
    let rom_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Breakout.bin");
    let rom = std::fs::read(rom_path).expect("Breakout.bin not found in project root");

    let mut atari = Atari::new(rom);

    // Skip 120 frames, then capture every 3rd frame for 180 frames (~3 seconds at 60fps)
    for _ in 0..120 {
        atari.run_frame();
    }

    // Press fire
    atari.set_action(Action::Fire);
    let frames = common::capture_frames(&mut atari, 180, 3);
    atari.set_action(Action::Noop);

    common::save_gif("breakout.gif", &frames, 50); // 50ms per frame = ~20fps playback
    println!("Wrote breakout.gif ({} frames)", frames.len());
}
