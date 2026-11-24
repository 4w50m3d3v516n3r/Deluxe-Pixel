//! Tests for the functions added from the DeluxePaint 5.2 program's command list.

use crate::anim::{decode, encode, EncodeOptions};
use crate::doc::Document;
use crate::palette::DEFAULT_PALETTE;
use crate::pix::Image;
use crate::v52::*;

fn numbered(n: u8) -> Image {
    Image::new(4, 3, n)
}

#[test]
fn tokenize_keeps_quoted_words() {
    assert_eq!(tokenize("LOADPIC \"My Pic.lbm\" KEEPFMT"), vec!["LOADPIC", "My Pic.lbm", "KEEPFMT"]);
    assert_eq!(tokenize("  FILL X 10, Y 20 TOBGCOLOR "), vec!["FILL", "X", "10", "Y", "20", "TOBGCOLOR"]);
    assert_eq!(tokenize("SAY 'hello there'"), vec!["SAY", "hello there"]);
}

#[test]
fn args_by_keyword_switch_and_position() {
    let t: Vec<String> = tokenize("FILE=a.lbm START 2 END 5 ALL 7");
    let a = Args(&t);
    assert_eq!(a.string("FILE").as_deref(), Some("a.lbm"));
    assert_eq!(a.num("START"), Some(2));
    assert_eq!(a.num("end"), Some(5));
    assert!(a.has("ALL"));
    assert!(!a.has("RANGE"));
    assert_eq!(a.posnum(0), Some(7));
    let t: Vec<String> = tokenize("12");
    assert_eq!(Args(&t).posnum(0), Some(12));
}

#[test]
fn scopes_cover_the_right_frames() {
    assert_eq!(scope_frames(Scope::Current, (1, 1), 3, 8), vec![3]);
    assert_eq!(scope_frames(Scope::All, (1, 1), 3, 4), vec![0, 1, 2, 3]);
    assert_eq!(scope_frames(Scope::Range, (5, 2), 0, 8), vec![1, 2, 3, 4]);
    assert_eq!(scope_frames(Scope::Range, (6, 40), 0, 8), vec![5, 6, 7]);
}

#[test]
fn fill_to_background_stops_only_at_background() {
    // 0 = background; a ring of colours 1 and 2 around a hole of 0.
    let mut img = Image::new(7, 7, 0);
    for y in 1..6 {
        for x in 1..6 {
            img.set(x, y, if (x + y) % 2 == 0 { 1 } else { 2 });
        }
    }
    img.set(3, 3, 0);
    let m = region_to_bg(&img, 1, 1, 0).unwrap();
    let count = m.iter().filter(|&&b| b).count();
    assert_eq!(count, 24, "every non-background pixel of the ring, not the hole");
    assert!(!m[(3 * 7 + 3) as usize]);
    assert!(!m[0]);
    // Starting on the background fills the background area like a normal fill.
    let m = region_to_bg(&img, 0, 0, 0).unwrap();
    assert!(m[0] && !m[(3 * 7 + 3) as usize] && !m[(1 * 7 + 1) as usize]);
}

#[test]
fn camera_view_pans_and_wraps() {
    let mut spare = Image::new(8, 4, 0);
    for y in 0..4 {
        for x in 0..8 {
            spare.set(x, y, x as u8);
        }
    }
    let cam = CamParams { dx: 3, dy: 0, dz: 0, count: 2, slow_in: 0, slow_out: 0, wrap: true, mode: CamMode::New, start: (0, 0), cyclic: false, come_to: false, cam_axes: false };
    let v0 = camera_view(&spare, 8, 4, &cam, 0.0, 9);
    assert_eq!(v0, spare);
    let v1 = camera_view(&spare, 8, 4, &cam, 1.0, 9);
    assert_eq!(v1.get(0, 0), 3);
    assert_eq!(v1.get(6, 2), 1, "wraps round");
    let nowrap = CamParams { wrap: false, ..cam.clone() };
    let v2 = camera_view(&spare, 8, 4, &nowrap, 1.0, 9);
    assert_eq!(v2.get(6, 2), 9, "background past the edge");
    // Moving away (Z) shrinks the picture towards the centre.
    let away = CamParams { dx: 0, dz: 300, ..cam };
    let v3 = camera_view(&spare, 8, 4, &away, 1.0, 9);
    assert_eq!(v3.get(4, 2), 4);
    assert_eq!(v3.get(5, 2), 6);
}

#[test]
fn copy_range_inserts_before_a_frame() {
    let mut d = Document::new(4, 3, 0);
    d.frames = (0..4).map(numbered).collect();
    d.sync_palettes();
    d.frame_delays[1] = 7;
    d.insert_copies(1, 2, 0);
    let v: Vec<u8> = d.frames.iter().map(|f| f.get(0, 0)).collect();
    assert_eq!(v, vec![1, 2, 0, 1, 2, 3]);
    assert_eq!(d.frame_delays, vec![7, 0, 0, 7, 0, 0]);
    d.insert_copies(0, 0, 6);
    assert_eq!(d.frames.len(), 7);
    assert_eq!(d.frames[6].get(0, 0), 1);
    assert_eq!(d.frame_palettes.len(), 7);
}

#[test]
fn per_frame_rate_plays_and_survives_saving() {
    let mut d = Document::new(4, 3, 0);
    d.frames = (0..3).map(numbered).collect();
    d.fps = 10.0;
    d.sync_palettes();
    d.frame_delays[1] = 30;
    assert!((d.frame_seconds(0) - 0.1).abs() < 1e-4);
    assert!((d.frame_seconds(1) - 0.5).abs() < 1e-4);
    let frames: Vec<Image> = (0..3).map(|k| {
        let mut i = Image::new(16, 4, 0);
        i.set(k, 1, 5);
        i
    }).collect();
    let o = EncodeOptions {
        brush: false,
        grab: (0, 0),
        transparent: None,
        fps: 10.0,
        ranges: Vec::new(),
        frame_palettes: Vec::new(),
        frame_delays: vec![0, 30, 0],
        planes: 0,
        camg: 0,
    };
    let back = decode(&encode(&frames, &DEFAULT_PALETTE, &o)).unwrap();
    assert_eq!(back.frames, frames);
    assert_eq!(back.frame_delays.get(1).copied(), Some(30), "{:?}", back.frame_delays);
}
