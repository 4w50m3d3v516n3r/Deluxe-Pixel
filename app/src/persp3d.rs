//! Perspective mode (Effect > Perspective > Do), after the DeluxePaint V manual, chapter 5:
//! the brush is a flat card in 3-D space. It turns about the X, Y and Z axes with the number
//! keys (7/8, 4/5, 1/2 by one degree, Shift for the angle step, 9/6/3 reset an axis, 0 resets
//! all), either about the screen's axes or about the brush's own axes, moves into the depth with
//! " and : (Ctrl + mouse slides it over the floor), and is drawn in perspective towards the
//! perspective centre. The menu bar shows its angles, or with \ its 3-D position.

use crate::pix::{Brush, Image};

pub type Mat = [[f32; 3]; 3];

pub const IDENTITY: Mat = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

pub fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut m = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

/// Rotation about one axis (0 = X, 1 = Y, 2 = Z) by `deg` degrees.
pub fn axis_rot(axis: usize, deg: f32) -> Mat {
    let (s, c) = deg.to_radians().sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        1 => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        _ => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

/// Screen angles → orientation: turned about Z first, then X, then Y (fixed screen axes), so the
/// same three angles always give the same orientation whatever order they were typed in.
pub fn from_angles(a: [f32; 3]) -> Mat {
    mul(&axis_rot(1, a[1]), &mul(&axis_rot(0, a[0]), &axis_rot(2, a[2])))
}

fn wrap(d: f32) -> f32 {
    let r = (d + 180.0).rem_euclid(360.0) - 180.0;
    if (r - -180.0).abs() < 1e-3 {
        180.0
    } else {
        r
    }
}

/// Orientation → the screen angles that give it (shown in the menu bar).
pub fn to_angles(m: &Mat) -> [f32; 3] {
    let sa = (-m[1][2]).clamp(-1.0, 1.0);
    let a = sa.asin();
    let ca = a.cos();
    let (b, c) = if ca.abs() > 1e-4 { (m[0][2].atan2(m[2][2]), m[1][0].atan2(m[1][1])) } else { ((-m[2][0]).atan2(m[0][0]), 0.0) };
    [wrap(a.to_degrees()), wrap(b.to_degrees()), wrap(c.to_degrees())].map(|v| (v * 100.0).round() / 100.0)
}

#[derive(Clone, Debug)]
pub struct Persp3 {
    /// Screen angles X, Y, Z in degrees.
    pub angles: [f32; 3],
    /// The brush's orientation (brush axes in screen axes).
    pub rot: Mat,
    /// Depth of the brush handle: 0 = the screen's plane, more = further away.
    pub z: f32,
    /// Angle step for Shift + number key (degrees).
    pub step: f32,
    /// Type: turn about the brush's own axes (true) or the screen's (false).
    pub brush_axes: bool,
    /// Display: 3-D position (true) or angles (false) in the menu bar.
    pub show_pos: bool,
    /// Grid in 3-D (X, Y, Z): positions snap to it when the grid is on; Z is the depth step.
    pub grid: [i32; 3],
    /// Eye distance in pixels: smaller = stronger perspective.
    pub eye: f32,
    /// Ctrl held: the brush slides over the floor (mouse up/down moves it in depth).
    pub floor: Option<(i32, f32)>,
    /// Fixed coordinates of the handle (FIXPERSP X Y Z): the brush stays there on that axis.
    pub fixed: [Option<f32>; 3],
    /// Skip the character that follows a Shift + number key.
    pub skip_char: bool,
    /// What the brush was last made from, so it is only redrawn when something changes.
    pub last_key: Option<(i32, i32, i32, [i32; 9], i32, i32, i32)>,
}

impl Default for Persp3 {
    fn default() -> Self {
        Persp3 { angles: [0.0; 3], rot: IDENTITY, z: 0.0, step: 90.0, brush_axes: false, show_pos: false, grid: [1, 1, 1], eye: 0.0, floor: None, fixed: [None; 3], skip_char: false, last_key: None }
    }
}

impl Persp3 {
    pub fn flat(&self) -> bool {
        self.z.abs() < 0.01 && self.angles.iter().all(|a| a.abs() < 0.01)
    }

    /// Turn about one axis by `deg` degrees (screen or brush axes, by the Type setting).
    pub fn turn(&mut self, axis: usize, deg: f32) {
        if self.brush_axes {
            self.rot = mul(&self.rot, &axis_rot(axis, deg));
            self.angles = to_angles(&self.rot);
        } else {
            self.angles[axis] = wrap(self.angles[axis] + deg);
            self.rot = from_angles(self.angles);
        }
    }

    /// Put one axis back to 0 (9, 6, 3 on the number pad).
    pub fn reset_axis(&mut self, axis: usize) {
        self.angles[axis] = 0.0;
        self.rot = from_angles(self.angles);
    }

    pub fn reset_angles(&mut self) {
        self.angles = [0.0; 3];
        self.rot = IDENTITY;
    }

    pub fn set_angles(&mut self, a: [f32; 3]) {
        self.angles = a.map(wrap);
        self.rot = from_angles(self.angles);
    }
}

/// The plane-to-screen mapping of the brush card: brush pixel (u, v) relative to its handle,
/// with the handle at `h` (x, y, depth), seen from `eye` pixels in front of the perspective
/// centre `c`. Returns a 3×3 homography (screen = H · (u, v, 1)).
pub fn homography(rot: &Mat, h: (f32, f32, f32), c: (f32, f32), eye: f32) -> Mat {
    let (cx, cy) = c;
    let d0 = eye + h.2;
    [
        [eye * rot[0][0] + cx * rot[2][0], eye * rot[0][1] + cx * rot[2][1], eye * (h.0 - cx) + cx * d0],
        [eye * rot[1][0] + cy * rot[2][0], eye * rot[1][1] + cy * rot[2][1], eye * (h.1 - cy) + cy * d0],
        [rot[2][0], rot[2][1], d0],
    ]
}

pub fn apply(m: &Mat, u: f32, v: f32) -> Option<(f32, f32)> {
    let w = m[2][0] * u + m[2][1] * v + m[2][2];
    if w <= 1.0 {
        return None;
    }
    Some(((m[0][0] * u + m[0][1] * v + m[0][2]) / w, (m[1][0] * u + m[1][1] * v + m[1][2]) / w))
}

pub fn invert(m: &Mat) -> Option<Mat> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-9 {
        return None;
    }
    let i = 1.0 / det;
    Some([
        [(m[1][1] * m[2][2] - m[1][2] * m[2][1]) * i, (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * i, (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * i],
        [(m[1][2] * m[2][0] - m[1][0] * m[2][2]) * i, (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * i, (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * i],
        [(m[1][0] * m[2][1] - m[1][1] * m[2][0]) * i, (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * i, (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * i],
    ])
}

/// The four corners of the brush card on the screen (None when part of it is behind the eye).
pub fn corners(base: &Brush, hm: &Mat) -> Option<[(f32, f32); 4]> {
    let (w, h) = (base.img.w as f32, base.img.h as f32);
    let (hx, hy) = (base.hx as f32, base.hy as f32);
    Some([apply(hm, -hx, -hy)?, apply(hm, w - hx, -hy)?, apply(hm, w - hx, h - hy)?, apply(hm, -hx, h - hy)?])
}

/// Draw the brush card in perspective. The result's handle sits where the pointer is (`at`),
/// so stamping it at the pointer puts every pixel where the 3-D brush shows it.
pub fn project(base: &Brush, hm: &Mat, at: (i32, i32), limit: (i32, i32)) -> Option<Brush> {
    let cs = corners(base, hm)?;
    let minx = cs.iter().map(|c| c.0).fold(f32::MAX, f32::min).floor().max(-limit.0 as f32) as i32;
    let maxx = cs.iter().map(|c| c.0).fold(f32::MIN, f32::max).ceil().min(2.0 * limit.0 as f32) as i32;
    let miny = cs.iter().map(|c| c.1).fold(f32::MAX, f32::min).floor().max(-limit.1 as f32) as i32;
    let maxy = cs.iter().map(|c| c.1).fold(f32::MIN, f32::max).ceil().min(2.0 * limit.1 as f32) as i32;
    let (w, h) = ((maxx - minx).max(1), (maxy - miny).max(1));
    let inv = invert(hm)?;
    let bg = base.transparent.unwrap_or(0);
    let mut img = Image::new(w, h, bg);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = ((minx + x) as f32 + 0.5, (miny + y) as f32 + 0.5);
            let ww = inv[2][0] * sx + inv[2][1] * sy + inv[2][2];
            if ww.abs() < 1e-9 {
                continue;
            }
            let u = (inv[0][0] * sx + inv[0][1] * sy + inv[0][2]) / ww + base.hx as f32;
            let v = (inv[1][0] * sx + inv[1][1] * sy + inv[1][2]) / ww + base.hy as f32;
            let (ui, vi) = (u.floor() as i32, v.floor() as i32);
            if base.img.inside(ui, vi) && base.is_solid(ui, vi) {
                img.set(x, y, base.img.get(ui, vi));
            }
        }
    }
    let mut b = base.clone();
    b.img = img;
    b.transparent = Some(bg);
    b.hx = at.0 - minx;
    b.hy = at.1 - miny;
    b.custom = true;
    Some(b)
}
