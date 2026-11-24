//! Tests for Amiga bitmap fonts (the person's sample font folder Chisel/, when present).

use crate::amigafont::*;
use crate::palette::DEFAULT_PALETTE;
use std::path::PathBuf;

fn sample_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("Chisel")
}

#[test]
fn size_names() {
    assert_eq!(size_of_name("50c"), Some(50));
    assert_eq!(size_of_name("8"), Some(8));
    assert_eq!(size_of_name("readme"), None);
    assert_eq!(size_of_name("12.font"), None);
}

#[test]
fn chisel_colour_font_reads_and_draws() {
    let dir = sample_dir();
    let sizes = sizes_in(&dir);
    if sizes.is_empty() {
        return;
    }
    assert_eq!(sizes.iter().map(|s| s.0).collect::<Vec<_>>(), vec![50, 62, 76, 98]);
    let f = load(&sizes[0].1, "Chisel").unwrap();
    assert_eq!((f.ysize, f.baseline, f.lo, f.hi), (50, 40, 0x20, 0xDC));
    assert!(f.color && f.depth == 3 && f.colors.len() == 8);
    assert!(f.proportional);
    let b = f.render("Ab", &Style::default(), 1, 0, &DEFAULT_PALETTE).unwrap();
    assert_eq!(b.img.h, 51);
    let used: std::collections::BTreeSet<u8> = b.img.px.iter().cloned().filter(|&p| p != 0).collect();
    assert!(used.len() >= 2, "a colour font draws in several colours: {used:?}");
    // Each size loads.
    for (s, p) in &sizes {
        assert_eq!(load(p, "Chisel").unwrap().ysize as u32, *s);
    }
    // Font Dir: the folder holding Chisel/ lists it as one font with its four sizes.
    let parent = dir.parent().unwrap().to_path_buf();
    let fonts = crate::text::fonts_in(&parent);
    let c = fonts.iter().find(|e| e.name == "Chisel").unwrap();
    assert_eq!(c.sizes.len(), 4);
    assert_eq!(c.nearest_size(60).unwrap().0, 62);
    // Choosing the Chisel folder itself works too.
    assert!(crate::text::fonts_in(&dir).iter().any(|e| e.name == "Chisel" && e.is_bitmap()));
}
