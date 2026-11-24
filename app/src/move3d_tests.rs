//! The Move requester's 3-D path (DeluxePaint V manual, chapter 6).

use crate::move3d::*;
use crate::persp3d::{from_angles, to_angles, IDENTITY};

fn start() -> MovePose {
    MovePose { x: 100.0, y: 150.0, z: 0.0, rot: IDENTITY }
}

#[test]
fn y_is_up_and_z_goes_into_the_distance() {
    let p = pose_at(&start(), [200, 0, 0], [0.0; 3], false, true, 1.0);
    assert_eq!((p.x, p.y), (300.0, 150.0), "positive X goes right");
    let p = pose_at(&start(), [0, 200, 400], [0.0; 3], false, true, 0.5);
    assert_eq!((p.x, p.y, p.z), (100.0, 50.0, 200.0), "positive Y goes up, Z away");
    let p = pose_at(&start(), [-200, -100, -50], [0.0; 3], false, true, 1.0);
    assert_eq!((p.x, p.y, p.z), (-100.0, 250.0, -50.0));
}

#[test]
fn brush_axes_for_distance_and_angle() {
    // The brush turned 90 degrees about Z: its own X axis points down the screen.
    let s = MovePose { rot: from_angles([0.0, 0.0, 90.0]), ..start() };
    let screen = pose_at(&s, [100, 0, 0], [0.0; 3], false, true, 1.0);
    let brush = pose_at(&s, [100, 0, 0], [0.0; 3], true, true, 1.0);
    assert!((screen.x - 200.0).abs() < 1e-3 && (screen.y - 150.0).abs() < 1e-3);
    assert!((brush.x - 100.0).abs() < 1e-3 && (brush.y - 250.0).abs() < 1e-3, "{brush:?}");
    // Turning about X: about the brush's (turned) X or the screen's X give different results.
    let a = pose_at(&s, [0; 3], [45.0, 0.0, 0.0], false, true, 1.0);
    let b = pose_at(&s, [0; 3], [45.0, 0.0, 0.0], false, false, 1.0);
    assert_ne!(to_angles(&a.rot), to_angles(&b.rot));
}

#[test]
fn cyclic_moves_stop_one_step_short() {
    assert_eq!(move_fractions(4, true, 0, 0), vec![0.0, 0.25, 0.5, 0.75]);
    let open = move_fractions(4, false, 0, 0);
    assert!((open[3] - 1.0).abs() < 1e-6 && (open[1] - 1.0 / 3.0).abs() < 1e-6);
    // A full 360 degree turn: the cyclic last frame is not the same as the first.
    let last = pose_at(&start(), [0; 3], [0.0, 0.0, 360.0], false, true, move_fractions(20, true, 0, 0)[19]);
    assert!((to_angles(&last.rot)[2] - (-18.0)).abs() < 0.01, "{:?}", to_angles(&last.rot));
}

#[test]
fn slow_in_eases_the_start() {
    let f = move_fractions(20, false, 10, 0);
    assert!(f[1] < 1.0 / 19.0, "accelerates: {}", f[1]);
    assert!((f[19] - 1.0).abs() < 1e-4);
}

#[test]
fn animbrush_cels_follow_direction_and_duration() {
    use crate::app::AnimBrush;
    use crate::pix::Image;
    let mut ab = AnimBrush::new((0..4).map(|k| Image::new(1, 1, k)).collect());
    let seq = |ab: &AnimBrush| (0..8).map(|k| ab.cell_at(k)).collect::<Vec<_>>();
    assert_eq!(seq(&ab), vec![0, 1, 2, 3, 0, 1, 2, 3]);
    ab.dir = -1;
    ab.restart_at(2);
    assert_eq!(seq(&ab), vec![2, 1, 0, 3, 2, 1, 0, 3]);
    ab.dir = 0;
    ab.restart_at(0);
    assert_eq!(seq(&ab), vec![0, 1, 2, 3, 2, 1, 0, 1]);
    // Duration = frames for one run through the cels: 8 for 4 cels is half the speed.
    ab.dir = 1;
    ab.duration = 8;
    assert_eq!(seq(&ab), vec![0, 0, 1, 1, 2, 2, 3, 3]);
    // The person's case: 11 cels, Duration 20 -- AnimPainting 20 frames still goes through them all.
    let mut slow = AnimBrush::new((0..11).map(|k| Image::new(1, 1, k)).collect());
    assert_eq!(slow.duration, 11, "a new AnimBrush plays one cel per frame");
    slow.duration = 20;
    let mut seen = Vec::new();
    for _ in 0..20 {
        seen.push(slow.pos);
        slow.advance();
    }
    assert_eq!(seen.first(), Some(&0));
    assert_eq!(*seen.iter().max().unwrap(), 10, "all 11 cels show within the 20 frames: {seen:?}");
    assert_eq!(slow.pos, 0, "and after 20 frames it starts again");
    // Stamping by hand follows the same order.
    let mut by_hand = AnimBrush::new((0..4).map(|k| Image::new(1, 1, k)).collect());
    by_hand.dir = 0;
    let expect: Vec<usize> = (0..8).map(|k| by_hand.cell_at(k)).collect();
    let mut got = Vec::new();
    for _ in 0..8 {
        got.push(by_hand.pos);
        by_hand.advance();
    }
    assert_eq!(got, expect);
}
