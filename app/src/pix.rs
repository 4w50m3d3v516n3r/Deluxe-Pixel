//! Indexed 8-bit images, geometric primitives and brush transforms.

use crate::palette::{nearest, Palette};

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
}

impl Image {
    pub fn new(w: i32, h: i32, fill: u8) -> Self {
        let w = w.max(1);
        let h = h.max(1);
        Image { w, h, px: vec![fill; (w * h) as usize] }
    }
    #[inline]
    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if self.inside(x, y) {
            self.px[(y * self.w + x) as usize]
        } else {
            0
        }
    }
    #[inline]
    pub fn get_clamped(&self, x: i32, y: i32) -> u8 {
        let x = x.clamp(0, self.w - 1);
        let y = y.clamp(0, self.h - 1);
        self.px[(y * self.w + x) as usize]
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: u8) {
        if self.inside(x, y) {
            self.px[(y * self.w + x) as usize] = c;
        }
    }
    pub fn fill(&mut self, c: u8) {
        self.px.iter_mut().for_each(|p| *p = c);
    }
    pub fn crop(&self, x0: i32, y0: i32, w: i32, h: i32) -> Image {
        let mut out = Image::new(w, h, 0);
        for y in 0..out.h {
            for x in 0..out.w {
                out.px[(y * out.w + x) as usize] = self.get(x0 + x, y0 + y);
            }
        }
        out
    }
    /// Resize the page, keeping the top-left content (Page Size).
    pub fn resized_canvas(&self, w: i32, h: i32, fill: u8) -> Image {
        let mut out = Image::new(w, h, fill);
        for y in 0..h.min(self.h) {
            for x in 0..w.min(self.w) {
                out.px[(y * w + x) as usize] = self.px[(y * self.w + x) as usize];
            }
        }
        out
    }
    pub fn scaled(&self, w: i32, h: i32) -> Image {
        let w = w.max(1);
        let h = h.max(1);
        let mut out = Image::new(w, h, 0);
        for y in 0..h {
            let sy = ((y as i64 * self.h as i64) / h as i64) as i32;
            for x in 0..w {
                let sx = ((x as i64 * self.w as i64) / w as i64) as i32;
                out.px[(y * w + x) as usize] = self.px[(sy * self.w + sx) as usize];
            }
        }
        out
    }
    pub fn flip_x(&self) -> Image {
        let mut o = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                o.px[(y * self.w + x) as usize] = self.px[(y * self.w + (self.w - 1 - x)) as usize];
            }
        }
        o
    }
    pub fn flip_y(&self) -> Image {
        let mut o = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                o.px[(y * self.w + x) as usize] = self.px[((self.h - 1 - y) * self.w + x) as usize];
            }
        }
        o
    }
    pub fn rot90(&self) -> Image {
        // Clockwise quarter turn.
        let mut o = Image::new(self.h, self.w, 0);
        for y in 0..self.h {
            for x in 0..self.w {
                let nx = self.h - 1 - y;
                let ny = x;
                o.px[(ny * o.w + nx) as usize] = self.px[(y * self.w + x) as usize];
            }
        }
        o
    }
}

/// Bresenham line, calling `f` for every point.
pub fn line_points(x0: i32, y0: i32, x1: i32, y1: i32, mut f: impl FnMut(i32, i32)) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (x0, y0);
    loop {
        f(x, y);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Quadratic Bezier curve through a control point (Curve tool).
pub fn curve_points(p0: (i32, i32), ctrl: (i32, i32), p1: (i32, i32), mut f: impl FnMut(i32, i32)) {
    // The control is the point the curve passes through at t = 0.5.
    let cx = 2.0 * ctrl.0 as f32 - 0.5 * (p0.0 + p1.0) as f32;
    let cy = 2.0 * ctrl.1 as f32 - 0.5 * (p0.1 + p1.1) as f32;
    let len = ((p1.0 - p0.0).abs() + (p1.1 - p0.1).abs() + (ctrl.0 - p0.0).abs() + (ctrl.1 - p0.1).abs()) as usize;
    let steps = (len * 2).clamp(8, 4000);
    let mut last = p0;
    f(p0.0, p0.1);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        let x = u * u * p0.0 as f32 + 2.0 * u * t * cx + t * t * p1.0 as f32;
        let y = u * u * p0.1 as f32 + 2.0 * u * t * cy + t * t * p1.1 as f32;
        let p = (x.round() as i32, y.round() as i32);
        if p != last {
            line_points(last.0, last.1, p.0, p.1, |a, b| {
                if (a, b) != last {
                    f(a, b)
                }
            });
            last = p;
        }
    }
}

/// Outline points of an axis-aligned ellipse centred on (cx,cy) with radii rx, ry.
pub fn ellipse_outline(cx: i32, cy: i32, rx: i32, ry: i32, mut f: impl FnMut(i32, i32)) {
    let rx = rx.abs();
    let ry = ry.abs();
    if rx == 0 || ry == 0 {
        line_points(cx - rx, cy - ry, cx + rx, cy + ry, f);
        return;
    }
    let mut pts: Vec<(i32, i32)> = Vec::new();
    let n = ((rx + ry) as f32 * 4.0).max(16.0) as i32;
    let mut last: Option<(i32, i32)> = None;
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let p = (cx + (a.cos() * rx as f32).round() as i32, cy + (a.sin() * ry as f32).round() as i32);
        if let Some(l) = last {
            if l != p {
                line_points(l.0, l.1, p.0, p.1, |x, y| pts.push((x, y)));
            }
        } else {
            pts.push(p);
        }
        last = Some(p);
    }
    pts.dedup();
    for (x, y) in pts {
        f(x, y);
    }
}

/// Spans (y, x0, x1) of a filled ellipse.
pub fn ellipse_spans(cx: i32, cy: i32, rx: i32, ry: i32) -> Vec<(i32, i32, i32)> {
    let rx = rx.abs().max(0);
    let ry = ry.abs().max(0);
    let mut spans = Vec::new();
    for dy in -ry..=ry {
        let t = if ry == 0 { 0.0 } else { dy as f32 / (ry as f32 + 0.5) };
        let w = ((1.0 - t * t).max(0.0)).sqrt() * (rx as f32 + 0.5);
        let w = w.floor() as i32;
        spans.push((cy + dy, cx - w, cx + w));
    }
    spans
}

/// Spans of a filled polygon (even-odd scanline).
pub fn polygon_spans(pts: &[(i32, i32)]) -> Vec<(i32, i32, i32)> {
    let mut spans = Vec::new();
    if pts.len() < 3 {
        return spans;
    }
    let ymin = pts.iter().map(|p| p.1).min().unwrap();
    let ymax = pts.iter().map(|p| p.1).max().unwrap();
    for y in ymin..=ymax {
        let fy = y as f32 + 0.5;
        let mut xs: Vec<f32> = Vec::new();
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            let (ay, by) = (a.1 as f32, b.1 as f32);
            if (ay <= fy && by > fy) || (by <= fy && ay > fy) {
                let t = (fy - ay) / (by - ay);
                xs.push(a.0 as f32 + t * (b.0 - a.0) as f32);
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.chunks(2) {
            if pair.len() == 2 {
                let x0 = pair[0].round() as i32;
                let x1 = (pair[1].round() as i32 - 1).max(x0);
                spans.push((y, x0, x1));
            }
        }
    }
    // Include the outline itself.
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        line_points(a.0, a.1, b.0, b.1, |x, y| spans.push((y, x, x)));
    }
    spans
}

/// Region mask of a flood fill from (x,y): pixels of the same colour, 4-connected.
/// Returns None if the region would exceed `limit` pixels (the original refused fills that were "too complex").
pub fn flood_mask(img: &Image, x: i32, y: i32, limit: usize) -> Option<Vec<bool>> {
    if !img.inside(x, y) {
        return None;
    }
    let target = img.get(x, y);
    let mut mask = vec![false; img.px.len()];
    let mut stack = vec![(x, y)];
    let mut count = 0usize;
    while let Some((sx, sy)) = stack.pop() {
        let mut x0 = sx;
        let row = (sy * img.w) as usize;
        if mask[row + sx as usize] {
            continue;
        }
        while x0 > 0 && img.px[row + (x0 - 1) as usize] == target && !mask[row + (x0 - 1) as usize] {
            x0 -= 1;
        }
        let mut x1 = sx;
        while x1 < img.w - 1 && img.px[row + (x1 + 1) as usize] == target && !mask[row + (x1 + 1) as usize] {
            x1 += 1;
        }
        for xx in x0..=x1 {
            mask[row + xx as usize] = true;
            count += 1;
        }
        if count > limit {
            return None;
        }
        for ny in [sy - 1, sy + 1] {
            if ny < 0 || ny >= img.h {
                continue;
            }
            let nrow = (ny * img.w) as usize;
            let mut xx = x0;
            while xx <= x1 {
                if img.px[nrow + xx as usize] == target && !mask[nrow + xx as usize] {
                    stack.push((xx, ny));
                    while xx <= x1 && img.px[nrow + xx as usize] == target {
                        xx += 1;
                    }
                } else {
                    xx += 1;
                }
            }
        }
    }
    Some(mask)
}

/// A brush: an image with a transparent colour and a handle point.
#[derive(Clone, Debug, PartialEq)]
pub struct Brush {
    pub img: Image,
    /// Pixels of this colour are not painted (the background colour when the brush was picked up).
    pub transparent: Option<u8>,
    pub hx: i32,
    pub hy: i32,
    /// Built-in brushes take the current colour; custom brushes keep their own colours.
    pub custom: bool,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinShape {
    Round,
    Square,
    Dither,
}

impl Brush {
    pub fn builtin(shape: BuiltinShape, size: i32) -> Brush {
        let size = size.clamp(1, 64);
        let mut img = Image::new(size, size, 0);
        let r = size as f32 / 2.0;
        for y in 0..size {
            for x in 0..size {
                let on = match shape {
                    BuiltinShape::Square => true,
                    BuiltinShape::Round => {
                        let dx = x as f32 + 0.5 - r;
                        let dy = y as f32 + 0.5 - r;
                        size <= 2 || dx * dx + dy * dy <= r * r + 0.25
                    }
                    BuiltinShape::Dither => (x + y) % 2 == 0 || size == 1,
                };
                if on {
                    img.set(x, y, 1);
                }
            }
        }
        Brush { hx: size / 2, hy: size / 2, img, transparent: Some(0), custom: false, name: format!("{:?} {}", shape, size) }
    }

    pub fn from_image(img: Image, transparent: Option<u8>) -> Brush {
        Brush { hx: img.w / 2, hy: img.h / 2, img, transparent, custom: true, name: "Custom brush".into() }
    }

    #[inline]
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        let p = self.img.get(x, y);
        match self.transparent {
            Some(t) => p != t,
            None => true,
        }
    }

    fn with(&self, img: Image) -> Brush {
        let mut b = self.clone();
        b.hx = img.w / 2;
        b.hy = img.h / 2;
        b.img = img;
        b
    }

    pub fn flip_x(&self) -> Brush {
        self.with(self.img.flip_x())
    }
    pub fn flip_y(&self) -> Brush {
        self.with(self.img.flip_y())
    }
    pub fn rot90(&self) -> Brush {
        self.with(self.img.rot90())
    }
    pub fn rot180(&self) -> Brush {
        self.with(self.img.rot90().rot90())
    }
    pub fn scaled(&self, w: i32, h: i32) -> Brush {
        self.with(self.img.scaled(w.clamp(1, 4096), h.clamp(1, 4096)))
    }

    fn bg(&self) -> u8 {
        self.transparent.unwrap_or(0)
    }

    /// Rotate by any angle (degrees), nearest-neighbour, enlarging the box.
    pub fn rotated(&self, deg: f32) -> Brush {
        let a = deg.to_radians();
        let (s, c) = a.sin_cos();
        let w = self.img.w as f32;
        let h = self.img.h as f32;
        let nw = (w * c.abs() + h * s.abs()).ceil() as i32;
        let nh = (w * s.abs() + h * c.abs()).ceil() as i32;
        let mut out = Image::new(nw, nh, self.bg());
        let (cx, cy) = (w / 2.0, h / 2.0);
        let (ncx, ncy) = (nw as f32 / 2.0, nh as f32 / 2.0);
        for y in 0..nh {
            for x in 0..nw {
                let dx = x as f32 + 0.5 - ncx;
                let dy = y as f32 + 0.5 - ncy;
                let sx = (c * dx + s * dy + cx).floor() as i32;
                let sy = (-s * dx + c * dy + cy).floor() as i32;
                if self.img.inside(sx, sy) {
                    out.set(x, y, self.img.get(sx, sy));
                }
            }
        }
        self.with(out)
    }

    /// Shear left-and-right (horizontal) or up-and-down (vertical) by `amount` pixels at the far edge.
    pub fn sheared(&self, horizontal: bool, amount: i32) -> Brush {
        let (w, h) = (self.img.w, self.img.h);
        let extra = amount.abs();
        let (nw, nh) = if horizontal { (w + extra, h) } else { (w, h + extra) };
        let mut out = Image::new(nw, nh, self.bg());
        for y in 0..h {
            for x in 0..w {
                let (nx, ny) = if horizontal {
                    let t = if h > 1 { (h - 1 - y) as f32 / (h - 1) as f32 } else { 0.0 };
                    let off = (t * amount as f32).round() as i32 + if amount < 0 { extra } else { 0 };
                    (x + off, y)
                } else {
                    let t = if w > 1 { x as f32 / (w - 1) as f32 } else { 0.0 };
                    let off = (t * amount as f32).round() as i32 + if amount < 0 { extra } else { 0 };
                    (x, y + off)
                };
                out.set(nx, ny, self.img.get(x, y));
            }
        }
        self.with(out)
    }

    /// Bend: curve the brush with a parabola, peak displacement `amount` in the middle.
    pub fn bent(&self, horizontal: bool, amount: i32) -> Brush {
        let (w, h) = (self.img.w, self.img.h);
        let extra = amount.abs();
        let (nw, nh) = if horizontal { (w + extra, h) } else { (w, h + extra) };
        let mut out = Image::new(nw, nh, self.bg());
        for y in 0..h {
            for x in 0..w {
                let (nx, ny) = if horizontal {
                    let t = if h > 1 { y as f32 / (h - 1) as f32 * 2.0 - 1.0 } else { 0.0 };
                    let off = ((1.0 - t * t) * amount as f32).round() as i32 + if amount < 0 { extra } else { 0 };
                    (x + off, y)
                } else {
                    let t = if w > 1 { x as f32 / (w - 1) as f32 * 2.0 - 1.0 } else { 0.0 };
                    let off = ((1.0 - t * t) * amount as f32).round() as i32 + if amount < 0 { extra } else { 0 };
                    (x, y + off)
                };
                out.set(nx, ny, self.img.get(x, y));
            }
        }
        self.with(out)
    }

    /// Outline: surround the brush shape with a one-pixel border of colour `c`.
    pub fn outlined(&self, c: u8) -> Brush {
        let (w, h) = (self.img.w + 2, self.img.h + 2);
        let bg = self.bg();
        let mut out = Image::new(w, h, bg);
        for y in 0..self.img.h {
            for x in 0..self.img.w {
                out.set(x + 1, y + 1, self.img.get(x, y));
            }
        }
        let src = out.clone();
        let solid = |x: i32, y: i32| src.inside(x, y) && match self.transparent { Some(t) => src.get(x, y) != t, None => true };
        for y in 0..h {
            for x in 0..w {
                if !solid(x, y) && (solid(x - 1, y) || solid(x + 1, y) || solid(x, y - 1) || solid(x, y + 1)) {
                    out.set(x, y, c);
                }
            }
        }
        let mut b = self.with(out);
        if b.transparent.is_none() {
            b.transparent = Some(bg);
        }
        b.custom = true;
        b
    }

    /// Perspective: rotate the brush around X, Y and Z axes (degrees) and project it.
    pub fn perspective(&self, ax: f32, ay: f32, az: f32, aa: bool, pal: &Palette) -> Brush {
        self.perspective_depth(ax, ay, az, aa, pal, 2.0)
    }

    /// Perspective with the eye `depth` brush-sizes away (smaller = stronger foreshortening).
    pub fn perspective_depth(&self, ax: f32, ay: f32, az: f32, aa: bool, pal: &Palette, depth: f32) -> Brush {
        let (w, h) = (self.img.w as f32, self.img.h as f32);
        let (sx_, cx_) = ax.to_radians().sin_cos();
        let (sy_, cy_) = ay.to_radians().sin_cos();
        let (sz_, cz_) = az.to_radians().sin_cos();
        let dist = (w.max(h)) * depth.max(0.3);
        // Forward map of a source point to screen.
        let fwd = |x: f32, y: f32| -> (f32, f32) {
            let (mut px, mut py, mut pz) = (x - w / 2.0, y - h / 2.0, 0.0f32);
            // Z
            let (nx, ny) = (px * cz_ - py * sz_, px * sz_ + py * cz_);
            px = nx;
            py = ny;
            // X
            let (ny2, nz2) = (py * cx_ - pz * sx_, py * sx_ + pz * cx_);
            py = ny2;
            pz = nz2;
            // Y
            let (nx3, nz3) = (px * cy_ + pz * sy_, -px * sy_ + pz * cy_);
            px = nx3;
            pz = nz3;
            let k = dist / (dist + pz).max(1.0);
            (px * k, py * k)
        };
        let corners = [fwd(0.0, 0.0), fwd(w, 0.0), fwd(0.0, h), fwd(w, h)];
        let minx = corners.iter().map(|c| c.0).fold(f32::MAX, f32::min);
        let maxx = corners.iter().map(|c| c.0).fold(f32::MIN, f32::max);
        let miny = corners.iter().map(|c| c.1).fold(f32::MAX, f32::min);
        let maxy = corners.iter().map(|c| c.1).fold(f32::MIN, f32::max);
        let nw = ((maxx - minx).ceil() as i32).clamp(1, 4096);
        let nh = ((maxy - miny).ceil() as i32).clamp(1, 4096);
        let mut out = Image::new(nw, nh, self.bg());
        let mut filled = vec![false; (nw * nh) as usize];
        // Forward-splat with sub-sampling (simple and robust).
        let ss = 3;
        for y in 0..self.img.h * ss {
            for x in 0..self.img.w * ss {
                let sxp = x / ss;
                let syp = y / ss;
                let p = self.img.get(sxp, syp);
                let (fx, fy) = fwd((x as f32 + 0.5) / ss as f32, (y as f32 + 0.5) / ss as f32);
                let ox = (fx - minx).floor() as i32;
                let oy = (fy - miny).floor() as i32;
                if out.inside(ox, oy) {
                    out.set(ox, oy, p);
                    filled[(oy * nw + ox) as usize] = true;
                }
            }
        }
        let mut b = self.with(out);
        if aa {
            b = b.antialiased(pal);
        }
        let _ = filled;
        b
    }

    /// Anti-alias: soften inner edges by averaging with neighbours (nearest palette colour).
    pub fn antialiased(&self, pal: &Palette) -> Brush {
        self.antialiased_level(pal, false)
    }

    /// Low keeps more of each pixel's own colour; High blends more with its neighbours.
    pub fn antialiased_level(&self, pal: &Palette, high: bool) -> Brush {
        let centre = if high { 1 } else { 4 };
        let src = &self.img;
        let mut out = src.clone();
        for y in 0..src.h {
            for x in 0..src.w {
                if !self.is_solid(x, y) {
                    continue;
                }
                let mut acc = [0i32; 3];
                let mut n = 0;
                for (dx, dy, wgt) in [(0, 0, centre), (-1, 0, 1), (1, 0, 1), (0, -1, 1), (0, 1, 1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if self.is_solid(nx, ny) && src.inside(nx, ny) {
                        let c = pal[src.get(nx, ny) as usize];
                        for k in 0..3 {
                            acc[k] += c[k] as i32 * wgt;
                        }
                        n += wgt;
                    }
                }
                if n > 0 {
                    out.set(x, y, nearest(pal, [acc[0] / n, acc[1] / n, acc[2] / n], 256));
                }
            }
        }
        self.with(out)
    }

    /// Remap brush colours from its own palette to the picture's palette.
    pub fn remapped(&self, from: &Palette, to: &Palette) -> Brush {
        let mut b = self.clone();
        let mut map = [0u8; 256];
        for i in 0..256 {
            map[i] = nearest(to, [from[i][0] as i32, from[i][1] as i32, from[i][2] as i32], 256);
        }
        for p in b.img.px.iter_mut() {
            if Some(*p) != self.transparent {
                *p = map[*p as usize];
            }
        }
        b
    }
}
