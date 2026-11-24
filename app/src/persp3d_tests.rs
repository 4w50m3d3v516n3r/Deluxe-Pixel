//! Tests for perspective mode's 3-D brush (DeluxePaint V manual, chapter 5).

use crate::persp3d::*;
use crate::pix::{Brush, Image};

fn close(a: &Mat, b: &Mat) -> bool {
    (0..3).all(|i| (0..3).all(|j| (a[i][j] - b[i][j]).abs() < 1e-3))
}

#[test]
fn screen_angles_do_not_depend_on_order() {
    let mut a = Persp3::default();
    a.turn(2, 45.0);
    a.turn(0, -45.0);
    let mut b = Persp3::default();
    b.turn(0, -45.0);
    b.turn(2, 45.0);
    assert_eq!(a.angles, b.angles);
    assert!(close(&a.rot, &b.rot));
}

#[test]
fn brush_axes_turn_about_the_brush_and_show_screen_angles() {
    let mut s = Persp3::default();
    s.turn(2, 45.0);
    s.turn(0, -45.0);
    let mut b = Persp3 { brush_axes: true, ..Persp3::default() };
    b.turn(2, 45.0);
    b.turn(0, -45.0);
    assert!(!close(&s.rot, &b.rot), "brush axes give another result (manual figs. 5.9 / 5.10)");
    // The shown angles reproduce the brush orientation in the screen system.
    assert!(close(&from_angles(b.angles), &b.rot), "{:?}", b.angles);
    for a in [[10.0, 20.0, 30.0], [-70.0, 45.0, 170.0], [0.0, 90.0, 0.0]] {
        assert!(close(&from_angles(to_angles(&from_angles(a))), &from_angles(a)));
    }
}

#[test]
fn keys_reset_one_axis_or_all() {
    let mut p = Persp3::default();
    p.turn(0, 30.0);
    p.turn(1, 90.0);
    p.reset_axis(0);
    assert_eq!(p.angles, [0.0, 90.0, 0.0]);
    p.reset_angles();
    assert!(p.flat());
    p.turn(2, 270.0);
    assert_eq!(p.angles[2], -90.0, "angles wrap to -180..180");
}

#[test]
fn depth_moves_towards_the_perspective_centre() {
    let c = (160.0, 100.0);
    let flat = homography(&IDENTITY, (40.0, 150.0, 0.0), c, 320.0);
    let p = apply(&flat, 10.0, -5.0).unwrap();
    assert!((p.0 - 50.0).abs() < 1e-3 && (p.1 - 145.0).abs() < 1e-3, "flat card is drawn as it is");
    let far = homography(&IDENTITY, (40.0, 150.0, 320.0), c, 320.0);
    let q = apply(&far, 0.0, 0.0).unwrap();
    assert!((q.0 - 100.0).abs() < 1e-3 && (q.1 - 125.0).abs() < 1e-3, "half way to the centre at twice the distance: {q:?}");
    let inv = invert(&far).unwrap();
    let w = inv[2][0] * q.0 + inv[2][1] * q.1 + inv[2][2];
    let u = (inv[0][0] * q.0 + inv[0][1] * q.1 + inv[0][2]) / w;
    let v = (inv[1][0] * q.0 + inv[1][1] * q.1 + inv[1][2]) / w;
    assert!(u.abs() < 1e-2 && v.abs() < 1e-2, "{u} {v}");
}

#[test]
fn projected_brush_stamps_where_the_card_is() {
    let mut img = Image::new(100, 60, 0);
    for y in 0..60 {
        for x in 0..100 {
            img.set(x, y, 5);
        }
    }
    let base = Brush::from_image(img, Some(0));
    // Tilted 60 degrees about X below the centre: seen from above, the top edge comes nearer
    // (wider) and the bottom edge goes away (narrower).
    let rot = from_angles([60.0, 0.0, 0.0]);
    let hm = homography(&rot, (100.0, 100.0, 0.0), (100.0, 40.0), 150.0);
    let b = project(&base, &hm, (100, 100), (320, 200)).unwrap();
    assert!(b.img.h < 30 && b.img.h >= 5, "foreshortened height {}", b.img.h);
    let row = |y: i32| (0..b.img.w).filter(|&x| b.img.get(x, y) == 5).count();
    assert!(row(1) > row(b.img.h - 2) + 10, "the far edge is narrower or wider than the near one");
    // The handle is where the pointer is.
    let (ox, oy) = (100 - b.hx, 100 - b.hy);
    let cs = corners(&base, &hm).unwrap();
    assert!((ox as f32 - cs.iter().map(|c| c.0).fold(f32::MAX, f32::min).floor()).abs() <= 1.0);
    assert!((oy as f32 - cs.iter().map(|c| c.1).fold(f32::MAX, f32::min).floor()).abs() <= 1.0);
}

#[test]
fn perspective_fill_repeats_the_brush_over_its_plane_into_the_depth() {
    let mut img = Image::new(16, 16, 0);
    for y in 0..16 {
        for x in 0..16 {
            img.set(x, y, if (x < 8) ^ (y < 8) { 1 } else { 2 });
        }
    }
    let brush = Brush::from_image(img, Some(0));
    // A floor: the brush laid flat (turned -90 degrees about X) below the centre.
    let rot = from_angles([-90.0, 0.0, 0.0]);
    let (c, eye) = ((160.0, 100.0), 300.0);
    let hm = homography(&rot, (160.0, 180.0, 0.0), c, eye);
    let plane = crate::paint::PerspPlane { inv: invert(&hm).unwrap(), depth_row: hm[2], eye, brush };
    // Above the horizon (the centre's height) there is no floor.
    assert_eq!(plane.sample(160, 60), None);
    // Below it the brush repeats, and the squares get smaller towards the horizon.
    let run = |y: i32| {
        let first = plane.sample(160, y);
        (160..400).take_while(|&x| plane.sample(x, y) == first).count()
    };
    assert!(plane.sample(160, 190).is_some() && plane.sample(10, 190).is_some());
    assert!(run(195) > run(110) + 2, "near rows show bigger squares: {} vs {}", run(195), run(110));
}
