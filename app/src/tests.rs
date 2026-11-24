//! Checks for the painting engine, brush transforms and file formats.

use crate::doc::Document;
use crate::paint::{self, FillKind, Mode, PaintSettings, Stroke};
use crate::palette::{Range, DEFAULT_PALETTE};
use crate::pix::{self, Brush, BuiltinShape, Image};

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join("deluxe-pixel-tests");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

#[test]
fn default_palette_matches_original_start() {
    assert_eq!(DEFAULT_PALETTE[0], [0, 0, 0]);
    assert_eq!(DEFAULT_PALETTE[1], [0, 0, 170]);
    assert_eq!(DEFAULT_PALETTE[15], [255, 255, 255]);
    assert_eq!(DEFAULT_PALETTE[16], [239, 239, 239]);
    assert_eq!(DEFAULT_PALETTE[255], [255, 255, 255]);
}

#[test]
fn freehand_line_paints_pixels() {
    let mut doc = Document::new(64, 64, 0);
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let set = PaintSettings::default();
    let mut st = Stroke::new(15, true);
    paint::stroke_line(&mut doc, &b, &set, &mut st, (0, 0), (63, 63));
    for i in 0..64 {
        assert_eq!(doc.img().get(i, i), 15);
    }
    assert_eq!(doc.img().get(1, 0), 0);
}

#[test]
fn flood_fill_respects_borders() {
    let mut img = Image::new(20, 20, 0);
    for i in 0..20 {
        img.set(10, i, 4);
    }
    let m = pix::flood_mask(&img, 2, 2, 1_000_000).unwrap();
    assert!(m[(5 * 20 + 5) as usize]);
    assert!(!m[(5 * 20 + 15) as usize]);
    assert!(!m[(5 * 20 + 10) as usize]);
}

#[test]
fn gradient_fill_uses_range() {
    let mut doc = Document::new(32, 8, 0);
    doc.ranges = vec![Range::new(16, 31, 0)];
    let mut set = PaintSettings::default();
    set.fill = FillKind::Linear;
    set.fill_angle = 0.0;
    let mask = vec![true; 32 * 8];
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut rng = paint::Rng::new(1);
    paint::fill_mask(&mut doc, &mask, &set, 1, &b, &mut rng);
    assert_eq!(doc.img().get(0, 0), 16);
    assert_eq!(doc.img().get(31, 0), 31);
    assert!(doc.img().get(16, 4) > 16 && doc.img().get(16, 4) < 31);
}

#[test]
fn shade_moves_within_range() {
    let mut doc = Document::new(8, 8, 20);
    doc.ranges = vec![Range::new(16, 31, 0)];
    let mut set = PaintSettings::default();
    set.mode = Mode::Shade;
    let b = Brush::builtin(BuiltinShape::Square, 1);
    let mut st = Stroke::new(15, true);
    paint::stamp(&mut doc, &b, &set, &mut st, 3, 3);
    assert_eq!(doc.img().get(3, 3), 21);
    let mut st = Stroke::new(0, false);
    paint::stamp(&mut doc, &b, &set, &mut st, 4, 4);
    assert_eq!(doc.img().get(4, 4), 19);
}

#[test]
fn stencil_protects() {
    let mut doc = Document::new(8, 8, 0);
    doc.img_mut().set(2, 2, 4);
    let mut protect = [false; 256];
    protect[4] = true;
    let mask = paint::stencil_from_colors(doc.img(), &protect);
    doc.stencil = Some(crate::doc::Stencil { mask, on: true });
    let b = Brush::builtin(BuiltinShape::Square, 5);
    let mut st = Stroke::new(9, true);
    paint::stamp(&mut doc, &b, &PaintSettings::default(), &mut st, 2, 2);
    assert_eq!(doc.img().get(2, 2), 4);
    assert_eq!(doc.img().get(1, 1), 9);
}

#[test]
fn symmetry_makes_copies() {
    let mut set = PaintSettings::default();
    set.sym.on = true;
    set.sym.points = 4;
    set.sym.mirror = false;
    let pts = set.sym.expand(60, 50, 100, 100);
    assert_eq!(pts.len(), 4);
}

#[test]
fn brush_transforms() {
    let mut img = Image::new(4, 2, 0);
    img.set(0, 0, 5);
    let b = Brush::from_image(img, Some(0));
    assert_eq!(b.flip_x().img.get(3, 0), 5);
    let r = b.rot90();
    assert_eq!((r.img.w, r.img.h), (2, 4));
    assert_eq!(b.scaled(8, 4).img.w, 8);
    let o = b.outlined(9);
    assert_eq!((o.img.w, o.img.h), (6, 4));
    let rr = b.rotated(45.0);
    assert!(rr.img.w >= 4);
    let p = b.perspective(30.0, 0.0, 0.0, false, &DEFAULT_PALETTE);
    assert!(p.img.w >= 1);
}

#[test]
fn ilbm_round_trip() {
    let mut img = Image::new(37, 11, 0);
    for (i, p) in img.px.iter_mut().enumerate() {
        *p = (i * 7 % 256) as u8;
    }
    let ranges = vec![Range::new(16, 31, 4096)];
    let path = tmp("rt.lbm");
    crate::fileio::save_ilbm(&path, &img, &DEFAULT_PALETTE, &ranges, None).unwrap();
    match crate::fileio::load(&path).unwrap() {
        crate::fileio::LoadResult::Indexed(l) => {
            assert_eq!(l.frames[0], img);
            assert_eq!(l.palette, DEFAULT_PALETTE);
            assert_eq!(l.ranges.unwrap()[0], ranges[0]);
        }
        _ => panic!("not indexed"),
    }
}

#[test]
fn pcx_bmp_png_round_trip() {
    let mut img = Image::new(33, 9, 0);
    for (i, p) in img.px.iter_mut().enumerate() {
        *p = (i * 13 % 256) as u8;
    }
    for name in ["rt.pcx", "rt.bmp", "rt.png"] {
        let path = tmp(name);
        crate::fileio::save_any(&path, std::slice::from_ref(&img), 0, &DEFAULT_PALETTE, &[], 12.0).unwrap();
        match crate::fileio::load(&path).unwrap() {
            crate::fileio::LoadResult::Indexed(l) => assert_eq!(l.frames[0], img, "{name}"),
            _ => panic!("{name} not indexed"),
        }
    }
}

#[test]
fn convert_reduces_colours() {
    let px: Vec<[u8; 3]> = (0..64 * 64).map(|i| [(i % 64 * 4) as u8, (i / 64 * 4) as u8, 128]).collect();
    let rgba = crate::convert::Rgba { w: 64, h: 64, px };
    let opts = crate::convert::ConvertOptions { colors: 16, ..Default::default() };
    let (img, _pal) = crate::convert::reduce(&rgba, &opts, &DEFAULT_PALETTE);
    assert!(img.px.iter().all(|&p| p < 16));
}

#[test]
fn cycling_rotates_range() {
    let mut doc = Document::new(4, 4, 0);
    doc.ranges = vec![Range::new(16, 19, 16384)];
    let p = doc.cycled_palette(&[1.0]);
    assert_eq!(p[17], DEFAULT_PALETTE[16]);
    assert_eq!(p[16], DEFAULT_PALETTE[19]);
}

#[test]
fn gallery_transitions_finish_on_target() {
    use crate::gallery::{compose, Transition};
    let a = vec![[10u8, 10, 10]; 16 * 16];
    let b = vec![[200u8, 100, 50]; 16 * 16];
    for t in Transition::ALL {
        let out = compose(t, &a, &b, 16, 16, 1.0);
        assert_eq!(out[8 * 16 + 8], [200, 100, 50], "{:?}", t);
    }
}

#[test]
fn text_renders_when_font_available() {
    let fonts = crate::text::system_fonts();
    if let Some(f) = fonts.iter().find(|f| f.name.eq_ignore_ascii_case("arial")) {
        let font = crate::text::load_font(&f.path).unwrap();
        let b = crate::text::render(&font, "Hi", &Default::default(), 15, 0, &DEFAULT_PALETTE).unwrap();
        assert!(b.img.px.iter().any(|&p| p == 15));
    }
}

#[test]
fn move_easing_starts_and_ends() {
    for (si, so) in [(0, 0), (3, 0), (0, 3), (3, 3)] {
        assert!((crate::app::App::move_progress_pub(0, 10, si, so) - 0.0).abs() < 1e-4);
        assert!((crate::app::App::move_progress_pub(9, 10, si, so) - 1.0).abs() < 1e-3, "{si} {so}");
    }
}

#[test]
fn zoom_area_stays_inside_picture() {
    use crate::app::clamp_center;
    // A 100-pixel-wide zoom area on a 640-pixel picture.
    assert_eq!(clamp_center(10.0, 50.0, 640.0), 50.0);
    assert_eq!(clamp_center(300.0, 50.0, 640.0), 300.0);
    assert_eq!(clamp_center(700.0, 50.0, 640.0), 590.0);
    // Zoom area wider than the picture: centred.
    assert_eq!(clamp_center(10.0, 400.0, 640.0), 320.0);
}

#[test]
fn new_modes_and_effects() {
    use crate::paint::{Process, Texture};
    use std::sync::Arc;
    // Mix: halfway between brush colour and picture.
    let mut doc = Document::new(8, 8, 0);
    let mut set = PaintSettings::default();
    set.mode = Mode::Mix;
    let b = Brush::builtin(BuiltinShape::Square, 1);
    let mut st = Stroke::new(15, true);
    paint::stamp(&mut doc, &b, &set, &mut st, 2, 2);
    let c = DEFAULT_PALETTE[doc.img().get(2, 2) as usize];
    assert!(c[0] > 90 && c[0] < 170, "{c:?}");
    // HBrite darkens with the left button.
    let mut doc = Document::new(8, 8, 15);
    set.mode = Mode::HBrite;
    paint::stamp(&mut doc, &b, &set, &mut Stroke::new(1, true), 3, 3);
    assert!(DEFAULT_PALETTE[doc.img().get(3, 3) as usize][0] < 200);
    // Rubthru copies the spare page.
    let mut doc = Document::new(8, 8, 0);
    let spare = Image::new(8, 8, 4);
    set.mode = Mode::Rubthru;
    set.rub = Some(Arc::new((spare, DEFAULT_PALETTE)));
    paint::stamp(&mut doc, &b, &set, &mut Stroke::new(1, true), 1, 1);
    assert_eq!(doc.img().get(1, 1), 4);
    // Process Value keeps the hue underneath.
    let mut doc = Document::new(8, 8, 4);
    set.mode = Mode::Paint;
    set.rub = None;
    set.process_on = true;
    set.process = Process::Value;
    paint::stamp(&mut doc, &b, &set, &mut Stroke::new(15, true), 1, 1);
    let c = DEFAULT_PALETTE[doc.img().get(1, 1) as usize];
    assert!(c[0] > c[1] && c[0] > c[2], "still red: {c:?}");
    // A texture blocks painting where it is empty.
    let mut doc = Document::new(8, 8, 0);
    let mut tex = Image::new(2, 1, 0);
    tex.set(0, 0, 1);
    set.process_on = false;
    set.texture = Some(Texture { img: Arc::new(tex), invert: false });
    set.texture_on = true;
    let big = Brush::builtin(BuiltinShape::Square, 4);
    paint::stamp(&mut doc, &big, &set, &mut Stroke::new(9, true), 3, 3);
    assert_eq!(doc.img().get(2, 2), 9);
    assert_eq!(doc.img().get(3, 2), 0);
    // Stencil Paint marks protected pixels instead of painting.
    let mut doc = Document::new(8, 8, 0);
    let mut set = PaintSettings::default();
    set.stencil_paint = true;
    paint::stamp(&mut doc, &big, &set, &mut Stroke::new(9, true), 3, 3);
    assert_eq!(doc.img().get(3, 3), 0);
    assert!(doc.protected(3, 3));
}

#[test]
fn macro_and_prefs_round_trip() {
    use crate::macros::{Macro, MacroEv, PtrIn};
    let m = Macro {
        name: "Test".into(),
        events: vec![
            MacroEv::Char('r'),
            MacroEv::Fg(12),
            MacroEv::Cmd(crate::actions::Cmd::BrushFlipH),
            MacroEv::Ptr(PtrIn { pos: (5, 6), hovered: true, pressed_p: true, pressed_s: false, down_p: true, down_s: false, released: false, dbl: false }),
            MacroEv::Mode(Mode::Smear),
            MacroEv::Key(egui::Key::F5, false, false, false),
        ],
    };
    let back = Macro::from_text(&m.to_text()).unwrap();
    assert_eq!(back.events, m.events);
    assert_eq!(back.name, "Test");
    let mut p = crate::prefs::Prefs::default();
    p.coords = true;
    p.be_square = true;
    p.macro_paths[2] = Some("C:/x.dpm".into());
    let q = crate::prefs::Prefs::from_text(&p.to_text());
    assert_eq!(p, q);
}

#[test]
fn trim_and_frames() {
    let mut img = Image::new(10, 10, 0);
    img.set(3, 4, 7);
    img.set(5, 6, 7);
    let b = Brush::from_image(img, Some(0));
    let t = crate::actions::trim_brush(&b);
    assert_eq!((t.img.w, t.img.h), (3, 3));
    let mut doc = Document::new(4, 4, 0);
    doc.set_frame_count(5);
    doc.add_frames(2);
    assert_eq!(doc.frames.len(), 7);
    assert!(doc.delete_frames(1, 2));
    assert_eq!(doc.frames.len(), 5);
    assert!(!doc.delete_frames(0, 4));
    doc.frames[0].set(0, 0, 3);
    doc.scroll_frame(0, 1, 0);
    assert_eq!(doc.frames[0].get(1, 0), 3);
}

#[test]
fn range_fill_follows_drawn_line() {
    let mut doc = Document::new(32, 32, 0);
    doc.ranges = vec![Range::new(16, 31, 0)];
    let mut set = PaintSettings::default();
    set.dither = crate::paint::Dither::Smooth;
    let mask = vec![true; 32 * 32];
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let fill = |doc: &mut Document, set: &PaintSettings| paint::fill_mask(doc, &mask, set, 1, &b, &mut paint::Rng::new(1));
    // Line: from the centre towards the left; the range starts at the end of the line (left)
    // and runs across the whole area, however short the line is.
    set.fill = FillKind::Linear;
    set.grad_line = Some(((16.0, 16.0), (10.0, 16.0)));
    fill(&mut doc, &set);
    assert_eq!(doc.img().get(0, 5), 16);
    assert_eq!(doc.img().get(31, 5), 31);
    // Turned 90 degrees: pointing up, the first colour is at the top.
    set.grad_line = Some(((16.0, 16.0), (16.0, 2.0)));
    fill(&mut doc, &set);
    assert_eq!(doc.img().get(5, 0), 16);
    assert_eq!(doc.img().get(5, 31), 31);
    // Diagonal: the corners along the line get the two ends of the range.
    set.grad_line = Some(((16.0, 16.0), (30.0, 2.0)));
    fill(&mut doc, &set);
    assert_eq!(doc.img().get(31, 0), 16);
    assert_eq!(doc.img().get(0, 31), 31);
    // Radial fills start at the end of the line.
    for kind in [FillKind::Circular, FillKind::Contour, FillKind::Highlight] {
        set.fill = kind;
        set.grad_line = Some(((16.0, 16.0), (8.5, 8.5)));
        fill(&mut doc, &set);
        assert_eq!(doc.img().get(8, 8), 16, "{kind:?}");
        assert!(doc.img().get(31, 31) >= 29, "{kind:?}");
    }
    // Shap follows the outline too.
    set.fill = FillKind::Shaped;
    set.grad_line = Some(((16.0, 16.0), (16.0, 2.0)));
    fill(&mut doc, &set);
    assert_eq!(doc.img().get(16, 0), 16);
    assert_eq!(doc.img().get(16, 31), 31);
}
