//! Tests for Amiga screen modes, palette limits and IFF depth/CAMG handling.

use crate::amiga::*;
use crate::fileio::{load, save_anim_fmt, save_ilbm_fmt, IffFormat, LoadResult};
use crate::palette::DEFAULT_PALETTE;
use crate::pix::Image;

fn mode(name: &str) -> DisplayMode {
    *MODES.iter().find(|m| m.name == name).unwrap()
}

#[test]
fn colour_limits_per_resolution_and_chip_set() {
    assert_eq!(mode("Low-res").max_colors(Chipset::Ecs), 32);
    assert_eq!(mode("Low-res EHB").max_colors(Chipset::Ecs), 64);
    assert_eq!(mode("Hi-res").max_colors(Chipset::Ecs), 16);
    assert_eq!(mode("Super-Hi-res").max_colors(Chipset::Ecs), 4);
    assert_eq!(mode("Hi-res").max_colors(Chipset::Aga), 256);
    assert_eq!(mode("VGA").max_colors(Chipset::Ecs), 256);
    assert_eq!(mode("Low-res").gun_bits(Chipset::Ecs), 4);
    assert_eq!(mode("Low-res").gun_bits(Chipset::Aga), 8);
    assert_eq!(mode("VGA").gun_bits(Chipset::Ecs), 8);
}

#[test]
fn mode_ids_round_trip() {
    for (i, m) in MODES.iter().enumerate().filter(|(_, m)| m.amiga()) {
        let d = if m.ehb { 6 } else { 5 };
        assert_eq!(mode_for(Some(m.mode_id()), m.w, m.h, d), i, "{}", m.name);
    }
    assert_eq!(mode("PAL Hi-res Laced").mode_id(), 0x0002_9004);
    // Old files with only resolution bits.
    assert_eq!(MODES[mode_for(Some(0x8004), 640, 400, 4)].name, "Hi-res Laced");
    assert_eq!(MODES[mode_for(None, 640, 480, 8)].name, "VGA");
}

#[test]
fn ecs_palette_has_16_levels_and_ehb_is_derived() {
    let mut p = DEFAULT_PALETTE;
    p[1] = [200, 9, 250];
    p[5] = [255, 136, 34];
    limit_palette(&mut p, 64, true, 4);
    assert_eq!(p[1], [204, 17, 255]);
    for c in p.iter().take(64) {
        assert!(c.iter().all(|v| v % 17 == 0));
    }
    // Colour 37 = colour 5 at half the 4-bit level: 15>>1=7, 8>>1=4, 2>>1=1.
    assert_eq!(p[37], [7 * 17, 4 * 17, 17]);
    // AGA EHB halves 8-bit values.
    let mut q = DEFAULT_PALETTE;
    q[3] = [201, 100, 51];
    limit_palette(&mut q, 64, true, 8);
    assert_eq!(q[35], [100, 50, 25]);
}

#[test]
fn ilbm_keeps_depth_and_display_mode() {
    let mut img = Image::new(21, 5, 0);
    for x in 0..21 {
        img.set(x, x % 5, (x % 3) as u8 + 1);
    }
    let dir = std::env::temp_dir();
    let path = dir.join(format!("dp-amiga-{}.lbm", std::process::id()));
    let m = mode("PAL Low-res");
    save_ilbm_fmt(&path, &img, &DEFAULT_PALETTE, &[], None, IffFormat { planes: 5, camg: m.mode_id() }).unwrap();
    let data = std::fs::read(&path).unwrap();
    // BMHD nPlanes is byte 8 of the BMHD data (after "FORM....ILBMBMHD....").
    assert_eq!(data[20 + 8], 5);
    match load(&path).unwrap() {
        LoadResult::Indexed(l) => {
            assert_eq!(l.depth, 5);
            assert_eq!(l.camg, Some(m.mode_id()));
            assert_eq!(l.frames[0], img);
        }
        _ => panic!("not indexed"),
    }
    // Without a format, the pixels decide: colours 0..3 need 2 planes (no longer padded to 8).
    save_ilbm_fmt(&path, &img, &DEFAULT_PALETTE, &[], None, IffFormat::default()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap()[28], 2);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn ehb_ilbm_loads_64_colours() {
    let mut img = Image::new(16, 2, 0);
    img.set(0, 0, 40);
    img.set(1, 0, 8);
    let mut pal = DEFAULT_PALETTE;
    pal[8] = [204, 102, 68];
    let path = std::env::temp_dir().join(format!("dp-ehb-{}.lbm", std::process::id()));
    save_ilbm_fmt(&path, &img, &pal, &[], None, IffFormat { planes: 6, camg: mode("Low-res EHB").mode_id() }).unwrap();
    match load(&path).unwrap() {
        LoadResult::Indexed(l) => {
            assert_eq!(l.depth, 6);
            assert_eq!(l.palette[40], [102, 51, 34]);
            assert_eq!(l.frames[0].get(0, 0), 40);
        }
        _ => panic!("not indexed"),
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn anim_keeps_depth_and_display_mode() {
    let frames: Vec<Image> = (0..3)
        .map(|k| {
            let mut i = Image::new(32, 4, 0);
            i.set(k, 1, 3);
            i
        })
        .collect();
    let path = std::env::temp_dir().join(format!("dp-amiga-{}.anim", std::process::id()));
    let id = mode("Hi-res").mode_id();
    save_anim_fmt(&path, &frames, &DEFAULT_PALETTE, &[], 10.0, &[], &[], IffFormat { planes: 4, camg: id }).unwrap();
    match load(&path).unwrap() {
        LoadResult::Indexed(l) => {
            assert_eq!(l.depth, 4);
            assert_eq!(l.camg, Some(id));
            assert_eq!(l.frames, frames);
        }
        _ => panic!("not indexed"),
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn fewer_colours_move_pixels_to_the_nearest_kept_colour() {
    let mut d = crate::doc::Document::new(4, 1, 0);
    d.palette[1] = [255, 0, 0];
    d.palette[100] = [250, 5, 5];
    d.frames[0].set(2, 0, 100);
    d.colors = 32;
    assert_eq!(d.fit_pixels(), 1);
    assert_eq!(d.frames[0].get(2, 0), 1);
    assert_eq!(depth_of(32), 5);
    assert_eq!(depth_of(2), 1);
    assert_eq!(d.iff_format().planes, 5);
}

#[test]
fn every_palette_size_has_ranges_on_real_ramps() {
    use crate::palette::{find_ramps, palette_for, ranges_for};
    for n in [16usize, 32, 64, 128, 256] {
        let rs = ranges_for(n);
        assert!(rs.len() >= 5, "{n} colours: {} ranges", rs.len());
        let pal = palette_for(n);
        for r in &rs {
            assert!((r.high as usize) < n, "{n}: range {}-{}", r.low, r.high);
            assert!(r.len() >= 3, "{n}: range {}-{} too short", r.low, r.high);
            // Each range steps evenly in brightness one way.
            let lum = |c: [u8; 3]| c[0] as i32 * 3 + c[1] as i32 * 6 + c[2] as i32;
            let up = lum(pal[r.high as usize]) > lum(pal[r.low as usize]);
            // (256 colours keeps the default palette's rainbow range for colour cycling.)
            for i in (r.low..r.high).filter(|_| n < 256) {
                let (a, b) = (lum(pal[i as usize]), lum(pal[i as usize + 1]));
                assert!(if up { b > a } else { b < a }, "{n}: not a ramp at {i}");
            }
        }
        // The ramps can also be found in the palette itself (used for pictures without ranges).
        assert!(find_ramps(&pal, n, 8).len() >= 5, "{n}: ramps found {:?}", find_ramps(&pal, n, 8).iter().map(|r| (r.low, r.high)).collect::<Vec<_>>());
    }
    assert_eq!(ranges_for(8).len(), 3);
}

#[test]
fn fewer_colours_bring_their_own_ranges() {
    let mut d = crate::doc::Document::new(8, 2, 0);
    d.frames[0].set(1, 0, 15); // white in the default palette
    d.colors = 32;
    d.adopt_palette_size(256);
    assert_eq!(d.palette, crate::palette::palette_for(32));
    assert!(d.ranges.len() >= 5);
    assert!(d.ranges.iter().all(|r| r.high < 32));
    assert_eq!(d.frames[0].get(1, 0), 7, "white moves to the white of the new palette");
    // An own palette keeps its colours; its ramps become ranges.
    let mut e = crate::doc::Document::new(8, 2, 0);
    for i in 0..16 {
        e.palette[i] = [(i * 16) as u8, 0, (255 - i * 16) as u8];
    }
    e.palette[3] = [1, 2, 3];
    e.ranges.clear();
    e.colors = 16;
    e.adopt_palette_size(256);
    assert_eq!(e.palette[3], [1, 2, 3]);
    assert!(!e.ranges.is_empty());
}
