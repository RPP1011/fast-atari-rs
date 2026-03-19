/// Shared test utilities for frame capture and image export.

use fast_atari_rs::atari::Atari;
use fast_atari_rs::tia::{FRAME_WIDTH, FRAME_HEIGHT, DISPLAY_WIDTH};

/// NTSC color palette — maps TIA palette index to (R, G, B).
pub fn ntsc_color(idx: u8) -> (u8, u8, u8) {
    #[rustfmt::skip]
    const PAL: [(u8, u8, u8); 128] = [
        (0,0,0),(0,0,0),(68,68,0),(68,68,0),(112,40,0),(112,40,0),(132,24,0),(132,24,0),
        (136,0,0),(136,0,0),(120,0,92),(120,0,92),(72,0,120),(72,0,120),(20,0,132),(20,0,132),
        (0,0,136),(0,0,136),(0,0,120),(0,0,120),(0,0,96),(0,0,96),(0,20,44),(0,20,44),
        (0,40,0),(0,40,0),(0,44,0),(0,44,0),(0,44,0),(0,44,0),(0,24,0),(0,24,0),
        (40,40,40),(40,40,40),(100,100,16),(100,100,16),(144,72,16),(144,72,16),(168,56,0),(168,56,0),
        (184,20,0),(184,20,0),(160,0,116),(160,0,116),(100,0,164),(100,0,164),(48,0,176),(48,0,176),
        (0,0,184),(0,0,184),(0,24,168),(0,24,168),(0,44,128),(0,44,128),(0,60,76),(0,60,76),
        (0,68,0),(0,68,0),(0,76,0),(0,76,0),(0,68,0),(0,68,0),(0,56,0),(0,56,0),
        (84,84,84),(84,84,84),(132,132,36),(132,132,36),(176,100,36),(176,100,36),(200,80,16),(200,80,16),
        (220,56,20),(220,56,20),(196,20,144),(196,20,144),(132,0,200),(132,0,200),(80,0,212),(80,0,212),
        (24,0,220),(24,0,220),(0,48,212),(0,48,212),(0,76,164),(0,76,164),(0,92,108),(0,92,108),
        (0,100,0),(0,100,0),(0,108,0),(0,108,0),(0,100,16),(0,100,16),(0,84,0),(0,84,0),
        (120,120,120),(120,120,120),(168,168,56),(168,168,56),(204,136,56),(204,136,56),(232,112,36),(232,112,36),
        (252,92,40),(252,92,40),(232,48,172),(232,48,172),(168,0,232),(168,0,232),(112,0,244),(112,0,244),
        (56,0,252),(56,0,252),(12,76,248),(12,76,248),(0,108,196),(0,108,196),(0,128,140),(0,128,140),
        (0,136,0),(0,136,0),(0,144,0),(0,144,0),(0,136,40),(0,136,40),(0,116,0),(0,116,0),
    ];
    PAL[(idx >> 1) as usize & 0x7F]
}

/// Convert a TIA palette-indexed framebuffer to RGB bytes at display resolution
/// (320x192 — each TIA pixel doubled horizontally for correct aspect ratio).
pub fn fb_to_rgb(fb: &[u8; FRAME_WIDTH * FRAME_HEIGHT]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(DISPLAY_WIDTH * FRAME_HEIGHT * 3);
    for row in 0..FRAME_HEIGHT {
        for col in 0..FRAME_WIDTH {
            let (r, g, b) = ntsc_color(fb[row * FRAME_WIDTH + col]);
            // Double each pixel horizontally
            rgb.push(r); rgb.push(g); rgb.push(b);
            rgb.push(r); rgb.push(g); rgb.push(b);
        }
    }
    rgb
}

/// Save a single framebuffer as a PNG file at correct display resolution (320x192).
pub fn save_png(path: &str, fb: &[u8; FRAME_WIDTH * FRAME_HEIGHT]) {
    let file = std::fs::File::create(path).unwrap();
    let w = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, DISPLAY_WIDTH as u32, FRAME_HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&fb_to_rgb(fb)).unwrap();
}

/// Capture frames from an Atari console at a given interval.
/// Runs `total_frames` frames, capturing a snapshot every `every` frames.
/// Returns the captured RGB frame data.
pub fn capture_frames(
    atari: &mut Atari,
    total_frames: usize,
    every: usize,
) -> Vec<Vec<u8>> {
    let mut captures = Vec::new();
    for i in 0..total_frames {
        atari.run_frame();
        if i % every == 0 {
            captures.push(fb_to_rgb(atari.framebuffer()));
        }
    }
    captures
}

/// Save captured frames as an animated GIF.
/// Each frame is `FRAME_WIDTH` x `FRAME_HEIGHT` RGB data.
/// `delay_ms` is the delay between frames in milliseconds.
pub fn save_gif(path: &str, frames: &[Vec<u8>], delay_ms: u16) {
    use std::io::Write;

    let w = DISPLAY_WIDTH as u16;
    let h = FRAME_HEIGHT as u16;

    let mut file = std::fs::File::create(path).unwrap();

    // GIF89a header
    file.write_all(b"GIF89a").unwrap();

    // Logical screen descriptor (no global color table)
    file.write_all(&w.to_le_bytes()).unwrap();
    file.write_all(&h.to_le_bytes()).unwrap();
    file.write_all(&[0x00, 0x00, 0x00]).unwrap(); // no GCT, bg=0, aspect=0

    // Netscape looping extension
    file.write_all(&[0x21, 0xFF, 0x0B]).unwrap();
    file.write_all(b"NETSCAPE2.0").unwrap();
    file.write_all(&[0x03, 0x01, 0x00, 0x00, 0x00]).unwrap(); // loop forever

    for frame_rgb in frames {
        // Build a local color table from this frame's unique colors (max 256)
        let mut palette: Vec<(u8, u8, u8)> = Vec::new();
        let mut color_map: std::collections::HashMap<(u8, u8, u8), u8> =
            std::collections::HashMap::new();

        let pixels: Vec<(u8, u8, u8)> = frame_rgb
            .chunks_exact(3)
            .map(|c| (c[0], c[1], c[2]))
            .collect();

        for &color in &pixels {
            if !color_map.contains_key(&color) {
                let idx = palette.len();
                if idx < 256 {
                    color_map.insert(color, idx as u8);
                    palette.push(color);
                }
            }
        }

        // Pad palette to power of 2
        let ct_size_bits = if palette.len() <= 2 {
            1
        } else {
            (palette.len() as f64).log2().ceil() as u8
        };
        let ct_len = 1usize << ct_size_bits;
        while palette.len() < ct_len {
            palette.push((0, 0, 0));
        }

        // Graphic control extension (delay)
        let delay_centis = delay_ms / 10;
        file.write_all(&[0x21, 0xF9, 0x04, 0x00]).unwrap();
        file.write_all(&delay_centis.to_le_bytes()).unwrap();
        file.write_all(&[0x00, 0x00]).unwrap(); // no transparent color, block terminator

        // Image descriptor with local color table
        file.write_all(&[0x2C]).unwrap();
        file.write_all(&0u16.to_le_bytes()).unwrap(); // left
        file.write_all(&0u16.to_le_bytes()).unwrap(); // top
        file.write_all(&w.to_le_bytes()).unwrap();
        file.write_all(&h.to_le_bytes()).unwrap();
        file.write_all(&[0x80 | (ct_size_bits - 1)]).unwrap(); // local CT flag + size

        // Local color table
        for &(r, g, b) in &palette {
            file.write_all(&[r, g, b]).unwrap();
        }

        // LZW-compressed image data
        let indices: Vec<u8> = pixels
            .iter()
            .map(|c| *color_map.get(c).unwrap_or(&0))
            .collect();

        let min_code_size = ct_size_bits.max(2);
        file.write_all(&[min_code_size]).unwrap();
        let compressed = lzw_compress(&indices, min_code_size);

        // Write in sub-blocks of up to 255 bytes
        for chunk in compressed.chunks(255) {
            file.write_all(&[chunk.len() as u8]).unwrap();
            file.write_all(chunk).unwrap();
        }
        file.write_all(&[0x00]).unwrap(); // block terminator
    }

    // GIF trailer
    file.write_all(&[0x3B]).unwrap();
}

/// Minimal LZW compressor for GIF.
fn lzw_compress(data: &[u8], min_code_size: u8) -> Vec<u8> {
    let clear_code = 1u16 << min_code_size;
    let eoi_code = clear_code + 1;

    let mut dictionary: std::collections::HashMap<Vec<u8>, u16> =
        std::collections::HashMap::new();
    for i in 0..clear_code {
        dictionary.insert(vec![i as u8], i);
    }

    let mut next_code = eoi_code + 1;
    let mut code_size = min_code_size as u32 + 1;
    let mut max_code = (1u16 << code_size) - 1;

    let mut output_bits: Vec<bool> = Vec::new();

    let emit = |bits: &mut Vec<bool>, code: u16, size: u32| {
        for i in 0..size {
            bits.push((code >> i) & 1 != 0);
        }
    };

    emit(&mut output_bits, clear_code, code_size);

    let mut current = Vec::new();
    for &byte in data {
        current.push(byte);
        if !dictionary.contains_key(&current) {
            // Output code for current minus last byte
            let prev = &current[..current.len() - 1];
            let code = dictionary[prev];
            emit(&mut output_bits, code, code_size);

            // Add new entry
            if next_code <= 4095 {
                dictionary.insert(current.clone(), next_code);
                next_code += 1;

                if next_code > max_code + 1 && code_size < 12 {
                    code_size += 1;
                    max_code = (1u16 << code_size) - 1;
                }
            }

            current.clear();
            current.push(byte);
        }
    }

    // Output remaining
    if !current.is_empty() {
        let code = dictionary[&current];
        emit(&mut output_bits, code, code_size);
    }

    emit(&mut output_bits, eoi_code, code_size);

    // Pack bits into bytes
    let mut bytes = Vec::new();
    for chunk in output_bits.chunks(8) {
        let mut byte = 0u8;
        for (i, &bit) in chunk.iter().enumerate() {
            if bit {
                byte |= 1 << i;
            }
        }
        bytes.push(byte);
    }

    bytes
}
