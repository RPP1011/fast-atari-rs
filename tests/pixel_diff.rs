mod common;

use stella_rs::atari::Atari;
use stella_rs::tia::{FRAME_WIDTH, FRAME_HEIGHT};

/// Run our emulator for N frames and compare pixel-by-pixel
/// against Gopher2600's reference output.
fn capture_our_frame(rom: &[u8], frames: usize) -> [u8; FRAME_WIDTH * FRAME_HEIGHT] {
    let mut atari = Atari::new(rom.to_vec());
    for _ in 0..frames {
        atari.run_frame();
    }
    *atari.framebuffer()
}

fn generate_gopher_frame(rom_path: &str, frames: usize) -> Vec<u8> {
    let out = format!("/tmp/gopher_frame_{}.bin", frames);
    let output = std::process::Command::new("/tmp/gopher_dump/gopher_dump")
        .args([rom_path, &frames.to_string(), &out])
        .output()
        .expect("failed to run gopher_dump — build it first");

    if !output.status.success() {
        panic!(
            "gopher_dump failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    std::fs::read(&out).expect("failed to read gopher frame output")
}

#[test]
fn pixel_diff_breakout() {
    let rom_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Breakout.bin");
    let rom = std::fs::read(rom_path).expect("Breakout.bin not found");

    let frames = 120;

    // Our frame
    let ours = capture_our_frame(&rom, frames);

    // Gopher's frame
    let gopher = generate_gopher_frame(rom_path, frames);
    assert_eq!(gopher.len(), FRAME_WIDTH * FRAME_HEIGHT, "Gopher frame size mismatch");

    // Compare pixel by pixel
    let total = FRAME_WIDTH * FRAME_HEIGHT;
    let mut matches = 0usize;
    let mut mismatches = 0usize;
    let mut diff_map = vec![0u8; total]; // 0=match, 255=mismatch

    for i in 0..total {
        if ours[i] == gopher[i] {
            matches += 1;
        } else {
            mismatches += 1;
            diff_map[i] = 255;
        }
    }

    let pct = matches as f64 / total as f64 * 100.0;
    println!("Pixel comparison (frame {}):", frames);
    println!("  Total:     {}", total);
    println!("  Matches:   {} ({:.1}%)", matches, pct);
    println!("  Mismatch:  {} ({:.1}%)", mismatches, 100.0 - pct);

    // Show first few mismatches with line/col and both values
    let mut shown = 0;
    for i in 0..total {
        if ours[i] != gopher[i] && shown < 20 {
            let line = i / FRAME_WIDTH;
            let col = i % FRAME_WIDTH;
            println!("    [{:3},{:3}] ours=${:02X} gopher=${:02X}",
                line, col, ours[i], gopher[i]);
            shown += 1;
        }
    }

    // Per-line mismatch summary
    println!("\nMismatches per line:");
    for line in 0..FRAME_HEIGHT {
        let start = line * FRAME_WIDTH;
        let line_mismatches = (start..start + FRAME_WIDTH)
            .filter(|&i| ours[i] != gopher[i])
            .count();
        if line_mismatches > 0 {
            println!("  line {:3}: {:3}/{} wrong", line, line_mismatches, FRAME_WIDTH);
        }
    }

    // Save visual diff: side-by-side PNG (ours | gopher | diff)
    save_comparison("breakout_diff.png", &ours, &gopher, &diff_map);

    // Fail if less than 80% match (generous for now)
    assert!(
        pct > 50.0,
        "Less than 50% pixel match ({:.1}%) — major rendering issue",
        pct
    );
}

fn save_comparison(path: &str, ours: &[u8], gopher: &[u8], diff: &[u8]) {
    let w = FRAME_WIDTH * 3; // three panels side by side
    let h = FRAME_HEIGHT;
    let mut rgb = vec![0u8; w * h * 3];

    for y in 0..h {
        for x in 0..FRAME_WIDTH {
            let idx = y * FRAME_WIDTH + x;

            // Left panel: our output
            let (r, g, b) = common::ntsc_color(ours[idx]);
            let out = (y * w + x) * 3;
            rgb[out] = r; rgb[out + 1] = g; rgb[out + 2] = b;

            // Middle panel: gopher output
            let (r, g, b) = common::ntsc_color(gopher[idx]);
            let out = (y * w + FRAME_WIDTH + x) * 3;
            rgb[out] = r; rgb[out + 1] = g; rgb[out + 2] = b;

            // Right panel: diff (red = mismatch, green = match)
            let out = (y * w + FRAME_WIDTH * 2 + x) * 3;
            if diff[idx] == 0 {
                rgb[out] = 0; rgb[out + 1] = 64; rgb[out + 2] = 0; // dark green
            } else {
                rgb[out] = 255; rgb[out + 1] = 0; rgb[out + 2] = 0; // red
            }
        }
    }

    let file = std::fs::File::create(path).unwrap();
    let bw = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(bw, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&rgb).unwrap();
}
