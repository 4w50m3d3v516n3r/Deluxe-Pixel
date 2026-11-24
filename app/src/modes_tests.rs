//! Each paint mode of the Mode menu, checked on a small picture.

use crate::doc::Document;
use crate::paint::*;
use crate::pix::{Brush, BuiltinShape, Image};

/// Left half colour 4 (red), right half colour 1 (blue).
fn two_halves() -> Document {
    let mut d = Document::new(40, 11, 1);
    for y in 0..11 {
        for x in 0..20 {
            d.frames[0].set(x, y, 4);
        }
    }
    d
}

fn settings(mode: Mode) -> PaintSettings {
    let mut s = PaintSettings::default();
    s.mode = mode;
    s
}

#[test]
fn smear_drags_colour_with_freehand_and_dotted() {
    let b = Brush::builtin(BuiltinShape::Round, 3);
    // Freehand: a line from the red side well into the blue side.
    let mut d = two_halves();
    let set = settings(Mode::Smear);
    let mut st = Stroke::new(15, true);
    stamp_points(&mut d, &b, &set, &mut st, &[(10, 5)]);
    stroke_line(&mut d, &b, &set, &mut st, (10, 5), (32, 5));
    let red_in_blue = (21..33).filter(|&x| d.img().get(x, 5) == 4).count();
    assert!(red_in_blue >= 4, "red is smeared into the blue half: {red_in_blue}");
    assert_eq!(d.img().get(22, 5), 4, "right after the edge the smear is still red");
    assert_eq!(d.img().get(30, 0), 1, "away from the stroke nothing changes");
    // Dotted: separate dabs, each remembering the one before.
    let mut d = two_halves();
    let mut st = Stroke::new(15, true);
    for x in (12..=30).step_by(2) {
        stamp(&mut d, &b, &set, &mut st, x, 5);
        st.last = Some((x, 5));
    }
    let red_in_blue = (21..31).flat_map(|x| (4..7).map(move |y| (x, y))).filter(|&(x, y)| d.img().get(x, y) == 4).count();
    assert!(red_in_blue >= 6, "dotted smear drags too: {red_in_blue}");
}

#[test]
fn blend_mixes_dragged_and_underlying_colours() {
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut d = two_halves();
    let set = settings(Mode::Blend);
    let mut st = Stroke::new(15, true);
    stamp_points(&mut d, &b, &set, &mut st, &[(18, 5)]);
    stroke_line(&mut d, &b, &set, &mut st, (18, 5), (24, 5));
    let c = d.img().get(20, 5);
    assert!(c != 1 && c != 4, "a colour between red and blue, got {c}");
}

#[test]
fn shade_moves_colours_within_the_range_only() {
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut d = Document::new(10, 1, 20);
    d.frames[0].set(0, 0, 4); // outside the grey range 16-31
    let set = settings(Mode::Shade);
    let mut st = Stroke::new(15, true);
    stamp(&mut d, &b, &set, &mut st, 3, 0);
    stamp(&mut d, &b, &set, &mut st, 0, 0);
    assert_eq!(d.img().get(3, 0), 21);
    assert_eq!(d.img().get(0, 0), 4);
    // One step per pass: going over the same pixel again in one stroke changes nothing more.
    let mut st = Stroke::new(15, false);
    stamp(&mut d, &b, &set, &mut st, 3, 0);
    stamp(&mut d, &b, &set, &mut st, 3, 0);
    assert_eq!(d.img().get(3, 0), 20, "right button shades the other way, one step per stroke");
    let mut st = Stroke::new(15, false);
    stamp(&mut d, &b, &set, &mut st, 3, 0);
    assert_eq!(d.img().get(3, 0), 19, "a second pass takes it one step further");
}

#[test]
fn smooth_softens_a_hard_edge() {
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut d = Document::new(9, 3, 16);
    for y in 0..3 {
        for x in 5..9 {
            d.frames[0].set(x, y, 31);
        }
    }
    let set = settings(Mode::Smooth);
    let mut st = Stroke::new(15, true);
    stamp(&mut d, &b, &set, &mut st, 5, 1);
    let c = d.img().get(5, 1);
    assert!(c > 16 && c < 31, "edge pixel becomes an in-between grey, got {c}");
}

#[test]
fn colour_replace_cycle_and_hbrite() {
    // A custom brush: colour 34 with a transparent (0) corner.
    let mut img = Image::new(3, 3, 34);
    img.set(0, 0, 0);
    let custom = Brush::from_image(img, Some(0));
    let mut d = Document::new(10, 10, 7);
    let mut st = Stroke::new(15, true);
    stamp(&mut d, &custom, &settings(Mode::SingleColor), &mut st, 2, 2);
    assert_eq!(d.img().get(2, 2), 15, "Colour paints the brush shape in the foreground colour");
    assert_eq!(d.img().get(1, 1), 7, "its transparent part stays");
    stamp(&mut d, &custom, &settings(Mode::Replace), &mut st, 6, 6);
    assert_eq!(d.img().get(5, 5), 0, "Replace paints the brush's background colour too");
    // Cycle: a line goes through the colours of the range.
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut d = Document::new(30, 1, 0);
    let mut st = Stroke::new(16, true);
    let set = settings(Mode::Cycle);
    stamp_points(&mut d, &b, &set, &mut st, &[(0, 0)]);
    stroke_line(&mut d, &b, &set, &mut st, (0, 0), (20, 0));
    let used: std::collections::BTreeSet<u8> = d.img().px.iter().cloned().filter(|&p| p != 0).collect();
    assert!(used.len() >= 5 && used.iter().all(|&c| (16..=31).contains(&c)), "{used:?}");
    // HBrite: half as bright.
    let mut d = Document::new(4, 1, 15);
    let mut st = Stroke::new(15, true);
    stamp(&mut d, &b, &settings(Mode::HBrite), &mut st, 1, 0);
    let (o, n) = (d.palette[15], d.palette[d.img().get(1, 0) as usize]);
    assert!(n[0] < o[0] && n[1] < o[1], "{o:?} -> {n:?}");
}

#[test]
fn mode_menu_matches_the_original() {
    let names: Vec<&str> = Mode::ALL.iter().map(|m| m.label()).collect();
    assert_eq!(names, ["Matte", "Colour", "Replc", "Smear", "Shade", "Blend", "Cycle", "Smooth", "Tint", "HBrite", "Rubthru"]);
}

#[test]
fn shade_works_in_the_foreground_colours_range() {
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut d = Document::new(10, 1, 40); // reds 32-47 (range 2 of the default ranges)
    d.frames[0].set(0, 0, 20); // a grey
    let mut set = settings(Mode::Shade);
    set.range = 0; // the Range panel shows the greys...
    set.fg_range = d.ranges.iter().position(|r| r.contains(35)); // ...but the foreground is a red
    let mut st = Stroke::new(35, true);
    stamp(&mut d, &b, &set, &mut st, 5, 0);
    stamp(&mut d, &b, &set, &mut st, 0, 0);
    assert_eq!(d.img().get(5, 0), 41, "the red under the brush shades along the reds");
    assert_eq!(d.img().get(0, 0), 20, "a grey is not in the foreground's range");
}

#[test]
fn antialias_softens_the_edges_of_a_stroke() {
    let draw = |high: Option<bool>| {
        let mut d = Document::new(40, 20, 0);
        let before = d.img().clone();
        let b = Brush::builtin(BuiltinShape::Round, 3);
        let set = settings(Mode::Paint);
        let mut st = Stroke::new(4, true);
        stamp_points(&mut d, &b, &set, &mut st, &[(3, 3)]);
        stroke_line(&mut d, &b, &set, &mut st, (3, 3), (36, 16));
        if let Some(h) = high {
            antialias_edges(&mut d, &before, h);
        }
        d
    };
    let between = |d: &Document| d.img().px.iter().filter(|&&p| p != 0 && p != 4).count();
    let red = |d: &Document| d.img().px.iter().filter(|&&p| p == 4).count();
    let (none, low, high) = (draw(None), draw(Some(false)), draw(Some(true)));
    assert_eq!(between(&none), 0, "without anti-alias only red and black");
    assert!(between(&low) > 20, "Low adds in-between colours along the edge: {}", between(&low));
    assert_eq!(red(&low), red(&none), "Low keeps the stroke itself");
    assert!(red(&high) < red(&none), "High also softens the stroke's own edge");
    // The in-between colours are darker reds, between red and black.
    let p = low.palette;
    for &c in low.img().px.iter().filter(|&&c| c != 0 && c != 4) {
        let v = p[c as usize];
        assert!(v[0] as i32 >= v[1] as i32 && v[0] as i32 >= v[2] as i32, "reddish: {v:?}");
    }
}

#[test]
fn spacing_every_nth_pixel_for_lines_freehand_and_dotted() {
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut set = settings(Mode::Paint);
    set.spacing = 17;
    let count = |d: &Document| d.img().px.iter().filter(|&&p| p == 15).count();
    // Continuous freehand / line: one line of 100 pixels in small pieces, as the mouse moves.
    let mut d = Document::new(120, 3, 0);
    let mut st = Stroke::new(15, true);
    stamp_points(&mut d, &b, &set, &mut st, &[(0, 1)]);
    let mut last = (0, 1);
    for x in (5..=100).step_by(5) {
        stroke_line(&mut d, &b, &set, &mut st, last, (x, 1));
        last = (x, 1);
    }
    let n = count(&d);
    assert!((6..=7).contains(&n), "a dot every 17 pixels over 100: {n}");
    // The dots really are 17 apart.
    let xs: Vec<i32> = (0..120).filter(|&x| d.img().get(x, 1) == 15).collect();
    assert!(xs.windows(2).all(|w| w[1] - w[0] == 17), "{xs:?}");
    // Dotted freehand: the brush put down at mouse positions 3 pixels apart, also every 17 pixels.
    let mut d = Document::new(120, 3, 0);
    let mut st = Stroke::new(15, true);
    let mut put = 0;
    for x in (0..=99).step_by(3) {
        put += stamp_points(&mut d, &b, &set, &mut st, &[(x, 1)]);
    }
    assert!((5..=7).contains(&put), "dotted: {put}");
    // Continuous (spacing 0) leaves no gaps.
    set.spacing = 0;
    let mut d = Document::new(120, 3, 0);
    let mut st = Stroke::new(15, true);
    stamp_points(&mut d, &b, &set, &mut st, &[(0, 1)]);
    stroke_line(&mut d, &b, &set, &mut st, (0, 1), (100, 1));
    assert_eq!(count(&d), 101);
}

#[test]
fn n_total_spreads_exactly_n_dots_over_the_whole_path() {
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut set = settings(Mode::Paint);
    set.spacing_total = 20;
    // A long wavy path, as recorded from the mouse, joined up.
    let pts: Vec<(i32, i32)> = (0..300).map(|x| (x + 5, 50 + ((x as f32 / 30.0).sin() * 30.0) as i32)).collect();
    let mut all = Vec::new();
    for w in pts.windows(2) {
        crate::pix::line_points(w[0].0, w[0].1, w[1].0, w[1].1, |x, y| all.push((x, y)));
    }
    all.dedup();
    let mut d = Document::new(320, 100, 0);
    let mut st = Stroke::new(15, true);
    stamp_points(&mut d, &b, &set, &mut st, &all);
    let dots = d.img().px.iter().filter(|&&p| p == 15).count();
    assert_eq!(dots, 20, "exactly N dots");
    assert_eq!(d.img().get(5, 50), 15, "one at the start");
    let last = all.last().unwrap();
    assert_eq!(d.img().get(last.0, last.1), 15, "one at the end");
}

#[test]
fn right_button_paints_the_background_colour_except_in_shade() {
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut d = Document::new(6, 1, 20);
    // Blend with the right button: the brush shape in the background colour (here 2).
    let mut st = Stroke::new(2, false);
    stamp(&mut d, &b, &settings(Mode::Blend), &mut st, 1, 0);
    assert_eq!(d.img().get(1, 0), 2);
    // Shade with the right button still shades down.
    let mut st = Stroke::new(2, false);
    stamp(&mut d, &b, &settings(Mode::Shade), &mut st, 3, 0);
    assert_eq!(d.img().get(3, 0), 19);
}

#[test]
fn tint_keeps_brightness_and_takes_the_colour() {
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut d = Document::new(4, 1, 7); // light grey
    let mut st = Stroke::new(4, true); // red
    stamp(&mut d, &b, &settings(Mode::Tint), &mut st, 1, 0);
    let c = d.palette[d.img().get(1, 0) as usize];
    assert!(c[0] > c[1] && c[0] > c[2], "reddish: {c:?}");
}

#[test]
fn solid_body_fills_enclosed_holes_only() {
    let mut img = Image::new(5, 5, 0);
    for y in 0..5 {
        for x in 0..5 {
            if x == 0 || y == 0 || x == 4 || y == 4 {
                img.set(x, y, 3);
            }
        }
    }
    img.set(0, 2, 0); // a gap in the ring: the inside is not enclosed...
    let open = Brush::from_image(img.clone(), Some(0));
    assert_eq!(crate::reqs8::solid_body(&open).img.get(2, 2), 0);
    img.set(0, 2, 3); // ...closed: filled
    let closed = Brush::from_image(img, Some(0));
    let s = crate::reqs8::solid_body(&closed);
    assert_eq!(s.img.get(2, 2), 3);
}

#[test]
fn set_frames_adds_and_removes_after_the_current_frame() {
    let mut d = Document::new(2, 1, 0);
    d.frames = (0..10).map(|k| Image::new(2, 1, k)).collect();
    d.sync_palettes();
    d.cur = 3;
    crate::reqs8::set_frames_after_current(&mut d, 7);
    let v: Vec<u8> = d.frames.iter().map(|f| f.get(0, 0)).collect();
    assert_eq!(v, vec![0, 1, 2, 3, 7, 8, 9], "frames 5-7 removed after the current one");
    crate::reqs8::set_frames_after_current(&mut d, 9);
    assert_eq!(d.frames.len(), 9);
    assert_eq!(d.frames[4].get(0, 0), 3, "copies of the current frame inserted after it");
}

#[test]
fn ranges_with_colours_placed_on_the_bar() {
    use crate::palette::Range;
    let d = Document::new(4, 1, 0);
    // Default palette: 1 dark blue, 15 white, 4 red at positions 0, 4 and 8 of the bar.
    let r = Range::from_cells(vec![(8, 4), (0, 1), (4, 15)], 2560);
    assert_eq!(r.registers(), vec![1, 15, 4], "bar order, not colour number order");
    assert!(r.contains(15) && !r.contains(2));
    let seq = r.sequence(&d.palette);
    assert_eq!(seq.len(), 9, "positions 0..=8");
    assert_eq!((seq[0], seq[4], seq[8]), (1, 15, 4));
    assert!(seq[2] != 1 && seq[2] != 15, "an in-between palette colour across the gap: {}", seq[2]);
    // Shade steps along the placed colours: blue -> white -> red.
    let b = Brush::builtin(BuiltinShape::Round, 1);
    let mut doc = Document::new(3, 1, 15);
    doc.ranges = vec![r];
    let mut set = settings(Mode::Shade);
    set.fg_range = Some(0);
    let mut st = Stroke::new(1, true);
    stamp(&mut doc, &b, &set, &mut st, 1, 0);
    assert_eq!(doc.img().get(1, 0), 4);
}
