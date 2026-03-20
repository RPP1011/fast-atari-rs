mod common;
use fast_atari_rs::atari::Atari;

#[test]
fn trace_positions() {
    let rom = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/Breakout.bin")).unwrap();
    let mut atari = Atari::new(rom);
    for _ in 0..300 { atari.run_frame(); }
    
    println!("pos_p0={} pos_p1={} nusiz0=${:02X} nusiz1=${:02X}",
        atari.bus.tia.pos_p0, atari.bus.tia.pos_p1,
        atari.bus.tia.nusiz0, atari.bus.tia.nusiz1);
    println!("grp0=${:02X} grp1=${:02X}", atari.bus.tia.grp0, atari.bus.tia.grp1);
    
    // Check what pixels P1 occupies with its current NUSIZ
    let nusiz = atari.bus.tia.nusiz1 & 0x07;
    println!("P1 NUSIZ mode={} copies:", nusiz);
    let offsets: &[u16] = match nusiz {
        0 => &[0], 1 => &[0,16], 2 => &[0,32], 3 => &[0,16,32],
        4 => &[0,64], 5 => &[0], 6 => &[0,32,64], 7 => &[0],
        _ => &[0],
    };
    for o in offsets {
        let pos = (atari.bus.tia.pos_p1 + o) % 160;
        println!("  copy at pixel {}-{}", pos, pos + 7);
    }
}
