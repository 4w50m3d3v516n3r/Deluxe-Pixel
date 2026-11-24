//! The person's Amiga DeluxePaint media folder (../media), when present: every file opens with
//! the loader DeluxePaint would use for it.

use crate::amigafiles::*;
use crate::fileio::{load, LoadResult};
use std::path::PathBuf;

fn media() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("media")
}

fn files(dir: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(media().join(dir))
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| !p.to_string_lossy().to_uppercase().ends_with(".INFO")).collect())
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn brushes_patterns_and_textures_open_with_handle_and_see_through_colour() {
    let mut n = 0;
    for dir in ["BRUSH", "PATTERNS", "TEXTURES"] {
        for p in files(dir) {
            match load(&p) {
                Ok(LoadResult::Indexed(l)) => {
                    let img = &l.frames[0];
                    assert!(img.w > 0 && img.h > 0, "{}", p.display());
                    if !p.to_string_lossy().to_uppercase().contains("PATTERNSET") {
                        assert!(l.grab.is_some(), "{} has a handle", p.display());
                        assert!(l.transparent.is_some(), "{} has a see-through colour", p.display());
                    }
                    n += 1;
                }
                Ok(_) => panic!("{} opened as true colour", p.display()),
                Err(e) => panic!("{}: {e}", p.display()),
            }
        }
    }
    if n > 0 {
        assert!(n >= 80, "{n} files");
    }
}

#[test]
fn palettes_and_colour_sets_open() {
    let list = files("COLOURS");
    for p in &list {
        let pf = read_palette_file(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert!(pf.colors.len() >= 16, "{}", p.display());
        // As a picture they explain themselves instead of failing silently.
        assert!(load(p).is_err());
    }
    if let Some(p) = list.iter().find(|p| p.ends_with("WB256.PAL")) {
        assert_eq!(read_palette_file(p).unwrap().colors.len(), 256);
    }
}

#[test]
fn drng_ranges_become_palette_ranges() {
    // Cells 0-8 fixed colours, cells 12-23 registers 27 down to 16 (from BIRDS.BRUSH).
    let mut c = vec![0x00, 0x17, 0x06, 0x80, 0x00, 0x01, 0x04, 0x0c];
    c.extend_from_slice(&[0x00, 0x22, 0x66, 0xcc, 0x04, 0xff, 0xaa, 0x00, 0x06, 0xff, 0xff, 0x00, 0x08, 0xff, 0xaa, 0x00]);
    for k in 0..12u8 {
        c.extend_from_slice(&[0x0c + k, 0x1b - k]);
    }
    let r = parse_drng(&c).unwrap();
    assert_eq!((r.low, r.high, r.rate, r.active), (16, 27, 0x680, true));
    // The placed colours in bar order: 27 down to 16.
    assert_eq!(r.registers(), (16..=27).rev().collect::<Vec<u8>>());
    assert_eq!(r.bar()[0], (12, 27));
    // Written back as DRNG it reads the same.
    assert_eq!(parse_drng(&drng_bytes(&r)).unwrap().cells, r.cells);
    // Fixed colours only: nothing to cycle in the palette.
    assert!(parse_drng(&[1, 9, 3, 0x80, 0, 0, 2, 0, 1, 0, 0x77, 0xff, 2, 0xee, 0, 0]).is_none());
}

#[test]
fn move_files_read_and_write() {
    for p in files("MOVE") {
        let d = std::fs::read(&p).unwrap();
        let m = read_move(&d).unwrap_or_else(|| panic!("{}", p.display()));
        assert_eq!(m.count, 20);
        assert_eq!(write_move(&m), d, "{} writes back the same bytes", p.display());
    }
    let m = MoveFile { cyclic: true, record: -1, go_from: false, dist: [10, -20, 300], angle: [0, 90, -45], count: 12, slow_in: 2, slow_out: 3 };
    assert_eq!(read_move(&write_move(&m)), Some(m));
}

#[test]
fn media_libraries_are_known_by_name() {
    for (name, i) in [("CHALK", 0), ("FELTTIP", 1), ("OIL", 2), ("WATERCOLOR", 3)] {
        assert_eq!(media_for_file(&PathBuf::from(name)), Some(i));
    }
    assert_eq!(media_for_file(&PathBuf::from("CRAYON")), None);
    for p in files("MEDIA") {
        assert!(is_amiga_program(&std::fs::read(&p).unwrap()));
    }
}
