//! Painting engine: drawing modes, Process (Tint/Hue/Value), translucency, textures,
//! stencil painting, symmetry, fill types with gradients and dithering, airbrush.

use crate::doc::{Document, Stencil};
use crate::palette::{hsv_to_rgb, nearest, rgb_to_hsv, Palette, Range};
use crate::pix::{Brush, Image};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Matte: the brush with its own colours (or the current colour for built-in brushes).
    Paint,
    /// Colour: the brush shape in the current colour.
    SingleColor,
    /// Repl: the whole brush rectangle, transparent colour included.
    Replace,
    Smear,
    Shade,
    Blend,
    Cycle,
    Smooth,
    Mix,
    /// Tint: the brush's colour (hue and saturation) on the brightness of what is there.
    Tint,
    HBrite,
    Rubthru,
}

impl Mode {
    /// The Mode menu, as in the DPaint 5.2 program: Matte, Colour, Replc, Smear, Shade, Blend,
    /// Cycle, Smooth, HBrite, Rubthru. (Mix stays for old macros but isn't offered.)
    pub const ALL: [Mode; 11] = [
        Mode::Paint,
        Mode::SingleColor,
        Mode::Replace,
        Mode::Smear,
        Mode::Shade,
        Mode::Blend,
        Mode::Cycle,
        Mode::Smooth,
        Mode::Tint,
        Mode::HBrite,
        Mode::Rubthru,
    ];
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Paint => "Matte",
            Mode::SingleColor => "Colour",
            Mode::Replace => "Replc",
            Mode::Smear => "Smear",
            Mode::Shade => "Shade",
            Mode::Blend => "Blend",
            Mode::Cycle => "Cycle",
            Mode::Smooth => "Smooth",
            Mode::Mix => "Mix",
            Mode::Tint => "Tint",
            Mode::HBrite => "HBrite",
            Mode::Rubthru => "Rubthru",
        }
    }
    /// The name in the Mode menu (the menu bar shows the short label).
    pub fn menu_name(&self) -> &'static str {
        match self {
            Mode::Replace => "Replace",
            Mode::Rubthru => "Rub Through",
            m => m.label(),
        }
    }
    /// What the mode does, shown when the pointer rests on it in the Mode menu.
    pub fn help(&self) -> &'static str {
        match self {
            Mode::Paint => "Paints the brush as it is; its background colour stays see-through.",
            Mode::SingleColor => "Paints the brush's shape in the foreground colour.",
            Mode::Replace => "Paints the whole brush rectangle, its background colour too (a custom brush covers what is under it).",
            Mode::Smear => "Drags the colours under the brush along the way you move it, like a finger in wet paint.",
            Mode::Shade => "Left button: each pass moves the colours under the brush one step along the foreground colour's range; right button: back. Only colours in that range change. Choose a foreground colour from a range (for example a grey) and paint over that range.",
            Mode::Blend => "Mixes the colours you drag over with the ones underneath (half each).",
            Mode::Cycle => "Paints with the colours of the foreground colour's range one after another.",
            Mode::Smooth => "Softens hard edges by averaging each pixel with its neighbours.",
            Mode::Mix => "Mixes the brush colour half with the colour underneath.",
            Mode::Tint => "Tints what is there with the brush's colour, keeping its brightness.",
            Mode::HBrite => "Extra-Half-Brite screens: left button darkens colours to their half-bright twins, right button brings them back.",
            Mode::Rubthru => "Shows the spare page through where you paint.",
        }
    }
    pub fn key(&self) -> &'static str {
        match self {
            Mode::Paint => "F1",
            Mode::SingleColor => "F2",
            Mode::Replace => "F3",
            Mode::Smear => "F4",
            Mode::Shade => "F5",
            Mode::Blend => "F6",
            Mode::Cycle => "F7",
            Mode::Smooth => "F8",
            _ => "",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillKind {
    Solid,
    /// The current brush, tiled.
    Brush,
    /// The current brush stretched over the shape.
    Wrap,
    /// The current brush tiled in perspective.
    Perspective,
    /// The spare page rubbed through.
    Rub,
    /// Darken (Halfbrite) the area.
    HBrite,
    /// The pattern made with "From brush".
    Pattern,
    Linear,
    Shaped,
    Circular,
    Contour,
    Highlight,
}

impl FillKind {
    pub const ALL: [FillKind; 12] = [
        FillKind::Solid,
        FillKind::Brush,
        FillKind::Wrap,
        FillKind::Perspective,
        FillKind::Rub,
        FillKind::HBrite,
        FillKind::Pattern,
        FillKind::Linear,
        FillKind::Shaped,
        FillKind::Circular,
        FillKind::Contour,
        FillKind::Highlight,
    ];
    pub fn is_gradient(&self) -> bool {
        matches!(self, FillKind::Linear | FillKind::Shaped | FillKind::Circular | FillKind::Contour | FillKind::Highlight)
    }
    pub fn label(&self) -> &'static str {
        match self {
            FillKind::Solid => "Solid",
            FillKind::Brush => "Brush",
            FillKind::Wrap => "Wrap",
            FillKind::Perspective => "Perspective",
            FillKind::Rub => "Rub",
            FillKind::HBrite => "HBrite",
            FillKind::Pattern => "Pattern",
            FillKind::Linear => "Line",
            FillKind::Shaped => "Shap",
            FillKind::Circular => "Cir",
            FillKind::Contour => "Cont",
            FillKind::Highlight => "Hi",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dither {
    /// Random mixing at colour borders, amount set by the dither slider.
    Random,
    /// No mixing: smooth bands.
    Smooth,
    /// Ordered (patterned) mixing at colour borders.
    Pattern,
}

impl Dither {
    pub fn label(&self) -> &'static str {
        match self {
            Dither::Random => "Random",
            Dither::Smooth => "Smooth",
            Dither::Pattern => "Pattern",
        }
    }
    pub fn next(&self) -> Dither {
        match self {
            Dither::Random => Dither::Smooth,
            Dither::Smooth => Dither::Pattern,
            Dither::Pattern => Dither::Random,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Process {
    Tint,
    Hue,
    Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymKind {
    Cyclic,
    Tile,
}

#[derive(Clone, Debug)]
pub struct Symmetry {
    pub on: bool,
    pub kind: SymKind,
    pub points: i32,
    pub mirror: bool,
    pub center: Option<(i32, i32)>,
    pub tile_w: i32,
    pub tile_h: i32,
}

impl Default for Symmetry {
    fn default() -> Self {
        Symmetry { on: false, kind: SymKind::Cyclic, points: 6, mirror: true, center: None, tile_w: 32, tile_h: 32 }
    }
}

impl Symmetry {
    pub fn expand(&self, x: i32, y: i32, w: i32, h: i32) -> Vec<(i32, i32)> {
        if !self.on {
            return vec![(x, y)];
        }
        match self.kind {
            SymKind::Cyclic => {
                let (cx, cy) = self.center.unwrap_or((w / 2, h / 2));
                let dx = (x - cx) as f32;
                let dy = (y - cy) as f32;
                let n = self.points.clamp(1, 40);
                let mut out = Vec::new();
                for i in 0..n {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    let (s, c) = a.sin_cos();
                    out.push((cx + (dx * c - dy * s).round() as i32, cy + (dx * s + dy * c).round() as i32));
                    if self.mirror {
                        let mdx = -dx;
                        out.push((cx + (mdx * c - dy * s).round() as i32, cy + (mdx * s + dy * c).round() as i32));
                    }
                }
                out.sort();
                out.dedup();
                out
            }
            SymKind::Tile => {
                let tw = self.tile_w.max(2);
                let th = self.tile_h.max(2);
                let bx = x.rem_euclid(tw);
                let by = y.rem_euclid(th);
                let mut out = Vec::new();
                let mut ty = by - th;
                while ty < h + th {
                    let mut tx = bx - tw;
                    while tx < w + tw {
                        out.push((tx, ty));
                        tx += tw;
                    }
                    ty += th;
                }
                out
            }
        }
    }
}

/// Simple xorshift random generator (for airbrush and gradient dither).
#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// A picture used by Rubthru: the spare page and its palette.
pub type RubSource = Arc<(Image, Palette)>;

/// A texture: painting only reaches pixels where the (tiled) texture is set.
#[derive(Clone)]
pub struct Texture {
    pub img: Arc<Image>,
    pub invert: bool,
}

/// A plane in perspective: screen → brush position (`inv`, relative to the brush handle), the
/// depth row of the forward mapping (to tell points in front of the eye), and the flat brush.
#[derive(Clone)]
pub struct PerspPlane {
    pub inv: [[f32; 3]; 3],
    pub depth_row: [f32; 3],
    pub eye: f32,
    pub brush: Brush,
}

impl PerspPlane {
    /// The brush pixel seen at screen point (x, y), the brush repeating over the whole plane;
    /// None beyond the horizon or where the brush is transparent.
    pub fn sample(&self, x: i32, y: i32) -> Option<u8> {
        let (sx, sy) = (x as f32 + 0.5, y as f32 + 0.5);
        let m = &self.inv;
        let w = m[2][0] * sx + m[2][1] * sy + m[2][2];
        if w.abs() < 1e-12 {
            return None;
        }
        let u = (m[0][0] * sx + m[0][1] * sy + m[0][2]) / w;
        let v = (m[1][0] * sx + m[1][1] * sy + m[1][2]) / w;
        // Only the part of the plane in front of the eye, and not so far that it is only noise.
        let d = self.depth_row[0] * u + self.depth_row[1] * v + self.depth_row[2];
        if d < 1.0 || d > self.eye * 60.0 {
            return None;
        }
        let b = &self.brush;
        let px = ((u + b.hx as f32).floor() as i32).rem_euclid(b.img.w.max(1));
        let py = ((v + b.hy as f32).floor() as i32).rem_euclid(b.img.h.max(1));
        let p = b.img.get(px, py);
        if Some(p) == b.transparent {
            None
        } else {
            Some(p)
        }
    }
}

/// Settings shared by all painting operations.
#[derive(Clone)]
pub struct PaintSettings {
    pub mode: Mode,
    /// Percentage of the existing colour that shows through (Translucency).
    pub translucent: u8,
    pub translucent_on: bool,
    pub process_on: bool,
    pub process: Process,
    pub range: usize,
    /// The range holding the foreground colour: Shade and Cycle work in it, as in the original
    /// (set by the program every frame; None when the colour is in no range).
    pub fg_range: Option<usize>,
    pub sym: Symmetry,
    pub fill: FillKind,
    /// Gradient direction in degrees (0 = left to right, 90 = top to bottom).
    pub fill_angle: f32,
    /// Gradient spans the whole page instead of the filled shape.
    pub fill_page: bool,
    pub dither: Dither,
    /// Dither amount 0..=100.
    pub spatter: u8,
    pub airbrush_radius: i32,
    pub airbrush_density: i32,
    /// Spacing > Airbrush: sprays for every pixel along the line.
    pub spacing_air_n: i32,
    /// Dot spacing for freehand: 0 = continuous, n = a stamp every n pixels.
    pub spacing: i32,
    pub grid_on: bool,
    pub grid: (i32, i32, i32, i32),
    pub rub: Option<RubSource>,
    pub texture: Option<Texture>,
    pub texture_on: bool,
    /// Stencil > Paint: strokes mark protected areas instead of painting.
    pub stencil_paint: bool,
    /// The fill pattern made with "From brush".
    pub pattern: Option<Arc<Brush>>,
    /// Perspective angles used by Perspective fills.
    pub persp: (f32, f32, f32),
    /// The brush's plane in 3-D for Perspective fills: the brush repeats over that plane, into
    /// the depth up to the horizon (set by the program from the perspective settings).
    pub persp_plane: Option<std::sync::Arc<PerspPlane>>,
    /// Pen pressure 0..1 (1 when no tablet).
    pub pressure: f32,
    pub pressure_translucent: bool,
    /// MultiCycle: Cycle mode cycles each brush colour that is in the range.
    pub multi_cycle: bool,
    /// Range FADE (0..=25): gradient translucency.
    pub range_fade: u8,
    /// Spacing: N total dots along a line or outline (0 = off).
    pub spacing_total: i32,
    /// Spacing: spray an airbrush burst at each point of the line.
    pub spacing_air: bool,
    /// Airbrush type: Old Style sprays single dots of the colour; Standard sprays the brush.
    pub airbrush_old: bool,
    /// Directional line for range fills: from the centre of the area to where the person pointed.
    pub grad_line: Option<((f32, f32), (f32, f32))>,
}

impl Default for PaintSettings {
    fn default() -> Self {
        PaintSettings {
            mode: Mode::Paint,
            translucent: 50,
            translucent_on: false,
            process_on: false,
            process: Process::Tint,
            range: 0,
            fg_range: None,
            sym: Symmetry::default(),
            fill: FillKind::Solid,
            fill_angle: 90.0,
            fill_page: false,
            dither: Dither::Random,
            spatter: 0,
            airbrush_radius: 16,
            airbrush_density: 12,
            spacing_air_n: 16,
            spacing: 0,
            grid_on: false,
            grid: (8, 8, 0, 0),
            rub: None,
            texture: None,
            texture_on: false,
            stencil_paint: false,
            pattern: None,
            persp: (60.0, 0.0, 0.0),
            persp_plane: None,
            pressure: 1.0,
            pressure_translucent: false,
            multi_cycle: false,
            range_fade: 0,
            spacing_total: 0,
            spacing_air: false,
            airbrush_old: true,
            grad_line: None,
        }
    }
}

impl PaintSettings {
    /// Percentage of the old colour that stays, from Translucency and pen pressure.
    fn keep_pct(&self) -> f32 {
        let mut keep = if self.translucent_on { self.translucent as f32 / 100.0 } else { 0.0 };
        if self.pressure_translucent && self.pressure < 1.0 {
            keep = 1.0 - (1.0 - keep) * self.pressure.clamp(0.05, 1.0);
        }
        keep
    }
    fn texture_allows(&self, x: i32, y: i32) -> bool {
        match (&self.texture, self.texture_on) {
            (Some(t), true) => {
                let set = t.img.get(x.rem_euclid(t.img.w), y.rem_euclid(t.img.h)) != 0;
                set != t.invert
            }
            _ => true,
        }
    }
}

/// Smear: chance per pixel and dab that the brush takes the colour underneath instead of the
/// dragged one (**estimated**; makes a smear fade over about ten brush-widths).
pub const SMEAR_PICKUP: f32 = 0.08;

/// State of one stroke.
pub struct Stroke {
    pub color: u8,
    /// Left button = true (paint with foreground / shade up).
    pub primary: bool,
    pub last: Option<(i32, i32)>,
    pub cycle_pos: usize,
    pub dist_acc: f32,
    pub rng: Rng,
    /// Pixels Shade already moved in this stroke: one step per pass over a pixel.
    pub shaded: std::collections::HashSet<(i32, i32)>,
}

impl Stroke {
    pub fn new(color: u8, primary: bool) -> Self {
        Stroke { color, primary, last: None, cycle_pos: 0, dist_acc: 0.0, rng: Rng::new(0x9E3779B97F4A7C15 ^ color as u64), shaded: Default::default() }
    }
}

pub fn blend(pal: &Palette, a: u8, b: u8, t_b: f32) -> u8 {
    let ca = pal[a as usize];
    let cb = pal[b as usize];
    let mix = |k: usize| (ca[k] as f32 * (1.0 - t_b) + cb[k] as f32 * t_b).round() as i32;
    nearest(pal, [mix(0), mix(1), mix(2)], 256)
}

/// Process: combine the painted colour with the colour underneath.
fn process(pal: &Palette, kind: Process, paint: u8, under: u8) -> u8 {
    let (ph, ps, pv) = rgb_to_hsv(pal[paint as usize]);
    let (uh, us, uv) = rgb_to_hsv(pal[under as usize]);
    let c = match kind {
        // Tint: towards the painted colour, keeping the brightness underneath.
        Process::Tint => hsv_to_rgb(ph, ps, uv),
        Process::Hue => hsv_to_rgb(ph, us, uv),
        Process::Value => hsv_to_rgb(uh, us, pv),
    };
    nearest(pal, [c[0] as i32, c[1] as i32, c[2] as i32], 256)
}

fn range_of(ranges: &[Range], idx: usize) -> Option<&Range> {
    ranges.get(idx).or_else(|| ranges.first())
}

/// Stamp the brush at canvas point (x,y), honouring symmetry.
pub fn stamp(doc: &mut Document, brush: &Brush, set: &PaintSettings, st: &mut Stroke, x: i32, y: i32) {
    let pts = set.sym.expand(x, y, doc.w, doc.h);
    let motion = match st.last {
        Some((lx, ly)) => (x - lx, y - ly),
        None => (0, 0),
    };
    for (px, py) in pts {
        stamp_one(doc, brush, set, st, px, py, motion);
    }
    if set.mode == Mode::Cycle {
        st.cycle_pos += 1;
    }
}

fn stamp_one(doc: &mut Document, brush: &Brush, set: &PaintSettings, st: &mut Stroke, x: i32, y: i32, motion: (i32, i32)) {
    let pal = doc.palette;
    // Shade and Cycle use the range of the foreground colour; otherwise the chosen range.
    let range = match set.mode {
        Mode::Shade | Mode::Cycle => range_of(&doc.ranges, set.fg_range.unwrap_or(set.range)).cloned(),
        _ => range_of(&doc.ranges, set.range).cloned(),
    };
    // The right mouse button paints the brush's shape in the background colour (Colour mode) in
    // every mode but Shade and HBrite, which use it for the other direction (manual p. 232).
    let mode = if !st.primary && !matches!(set.mode, Mode::Shade | Mode::HBrite) && !set.stencil_paint { Mode::SingleColor } else { set.mode };

    let bw = brush.img.w;
    let bh = brush.img.h;
    let ox = x - brush.hx;
    let oy = y - brush.hy;
    // Stencil > Paint: mark (left button) or clear (right button) protected pixels.
    if set.stencil_paint {
        if doc.stencil.is_none() {
            doc.stencil = Some(Stencil { mask: vec![false; (doc.w * doc.h) as usize], on: true });
        }
        let (w, h) = (doc.w, doc.h);
        let s = doc.stencil.as_mut().unwrap();
        for by in 0..bh {
            for bx in 0..bw {
                let (tx, ty) = (ox + bx, oy + by);
                if brush.is_solid(bx, by) && tx >= 0 && ty >= 0 && tx < w && ty < h {
                    s.mask[(ty * w + tx) as usize] = st.primary;
                }
            }
        }
        return;
    }
    // A copy of the area under the brush, for modes that read their neighbours.
    let needs_src = matches!(mode, Mode::Smear | Mode::Smooth | Mode::Blend);
    let src = if needs_src {
        let (mx, my) = (motion.0.abs() + 2, motion.1.abs() + 2);
        Some((doc.img().crop(ox - mx, oy - my, bw + 2 * mx, bh + 2 * my), ox - mx, oy - my))
    } else {
        None
    };
    let keep = set.keep_pct();
    for by in 0..bh {
        for bx in 0..bw {
            let solid = brush.is_solid(bx, by);
            if !solid && mode != Mode::Replace {
                continue;
            }
            let tx = ox + bx;
            let ty = oy + by;
            if !doc.img().inside(tx, ty) || doc.protected(tx, ty) || !set.texture_allows(tx, ty) {
                continue;
            }
            let existing = doc.img().get(tx, ty);
            let bpx = brush.img.get(bx, by);
            let paint_col = if brush.custom { bpx } else { st.color };
            let new = match mode {
                Mode::Paint => paint_col,
                Mode::SingleColor => st.color,
                Mode::Replace => {
                    if brush.custom {
                        bpx
                    } else if solid {
                        st.color
                    } else {
                        continue;
                    }
                }
                Mode::Smear => {
                    // The colours under the brush are dragged along the way it moves; now and
                    // then a pixel picks up the colour it passes over, so a smear fades out.
                    let (s, sx0, sy0) = src.as_ref().unwrap();
                    if motion == (0, 0) || st.rng.f() < SMEAR_PICKUP {
                        existing
                    } else {
                        s.get_clamped(tx - motion.0 - sx0, ty - motion.1 - sy0)
                    }
                }
                Mode::Blend => {
                    // Like Smear, but averaging the dragged colour with the one underneath.
                    let (s, sx0, sy0) = src.as_ref().unwrap();
                    let from = s.get_clamped(tx - motion.0 - sx0, ty - motion.1 - sy0);
                    blend(&pal, existing, from, 0.5)
                }
                Mode::Shade => {
                    let Some(r) = &range else { continue };
                    if !r.contains(existing) || !st.shaded.insert((tx, ty)) {
                        continue;
                    }
                    // One step along the range's colours (in bar order).
                    let regs = r.registers();
                    let i = regs.iter().position(|&c| c == existing).unwrap_or(0);
                    if st.primary {
                        regs[(i + 1).min(regs.len() - 1)]
                    } else {
                        regs[i.saturating_sub(1)]
                    }
                }
                Mode::Cycle => match &range {
                    Some(r) => {
                        let regs = r.registers();
                        let n = regs.len().max(1);
                        let k = st.cycle_pos % n;
                        let step = |base: u8| -> u8 {
                            let off = regs.iter().position(|&c| c == base).unwrap_or(0);
                            let i = if st.primary { (off + k) % n } else { (off + n - k % n) % n };
                            regs[i]
                        };
                        if brush.custom && set.multi_cycle {
                            // Each brush colour in the range cycles on its own.
                            if r.contains(paint_col) { step(paint_col) } else { paint_col }
                        } else if r.contains(st.color) {
                            step(st.color)
                        } else if brush.custom {
                            st.color
                        } else {
                            step(regs[0])
                        }
                    }
                    None => paint_col,
                },
                Mode::Smooth => {
                    let (s, sx0, sy0) = src.as_ref().unwrap();
                    let mut acc = [0i32; 3];
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let c = pal[s.get_clamped(tx + dx - sx0, ty + dy - sy0) as usize];
                            for k in 0..3 {
                                acc[k] += c[k] as i32;
                            }
                        }
                    }
                    nearest(&pal, [acc[0] / 9, acc[1] / 9, acc[2] / 9], 256)
                }
                Mode::Mix => blend(&pal, existing, paint_col, 0.5),
                Mode::Tint => process(&pal, Process::Tint, paint_col, existing),
                Mode::HBrite => {
                    let c = pal[existing as usize];
                    let f = if st.primary { 0.5 } else { 2.0 };
                    let v = |k: usize| ((c[k] as f32 * f).round() as i32).clamp(0, 255);
                    nearest(&pal, [v(0), v(1), v(2)], 256)
                }
                Mode::Rubthru => match &set.rub {
                    Some(r) => {
                        let (img, rp) = &**r;
                        if !img.inside(tx, ty) {
                            continue;
                        }
                        let c = rp[img.get(tx, ty) as usize];
                        nearest(&pal, [c[0] as i32, c[1] as i32, c[2] as i32], 256)
                    }
                    None => continue,
                },
            };
            let new = if set.process_on && !matches!(mode, Mode::Shade | Mode::Smear | Mode::Smooth | Mode::HBrite) {
                process(&pal, set.process, new, existing)
            } else {
                new
            };
            let new = if keep > 0.0 && !matches!(mode, Mode::Shade | Mode::Smear | Mode::Smooth) {
                blend(&pal, new, existing, keep)
            } else {
                new
            };
            doc.img_mut().set(tx, ty, new);
        }
    }
}

/// Anti-alias (Effect > AntiAlias Low / High): soften the edges of what was just drawn. `before`
/// is the picture before the stroke; pixels it changed are the stroke. Pixels just outside the
/// stroke take some of the stroke's colour by how much of their 3×3 neighbourhood it covers; with
/// High the stroke's own edge pixels also take some of the colour behind them. Colours are the
/// nearest the palette has; the stencil is respected.
pub fn antialias_edges(doc: &mut Document, before: &crate::pix::Image, high: bool) -> usize {
    let (w, h) = (doc.w, doc.h);
    if before.w != w || before.h != h {
        return 0;
    }
    let now = doc.img().clone();
    let changed: Vec<bool> = now.px.iter().zip(before.px.iter()).map(|(a, b)| a != b).collect();
    if !changed.iter().any(|&c| c) {
        return 0;
    }
    let pal = doc.palette;
    // How strongly a fringe pixel takes the stroke's colour per covered neighbour, and the most it
    // can take (estimated; strong enough that the palette has an in-between colour to pick).
    let (outside, out_max, inside, in_max) = if high { (2.2, 0.7, 1.2, 0.45) } else { (1.6, 0.55, 0.0, 0.0) };
    let mut out = now.clone();
    let mut n = 0;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let (mut covered, mut acc, mut total) = (0, [0i32; 3], 0);
            let mut behind = [0i32; 3];
            let mut nb = 0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let j = (ny * w + nx) as usize;
                    total += 1;
                    if changed[j] {
                        covered += 1;
                        let c = pal[now.px[j] as usize];
                        for k in 0..3 {
                            acc[k] += c[k] as i32;
                        }
                    } else {
                        nb += 1;
                        let c = pal[now.px[j] as usize];
                        for k in 0..3 {
                            behind[k] += c[k] as i32;
                        }
                    }
                }
            }
            if total == 0 || doc.protected(x, y) {
                continue;
            }
            let own = pal[now.px[i] as usize];
            let mix = |a: [u8; 3], b: [i32; 3], cnt: i32, t: f32| -> [i32; 3] {
                [0, 1, 2].map(|k| (a[k] as f32 * (1.0 - t) + (b[k] as f32 / cnt as f32) * t).round() as i32)
            };
            if !changed[i] && covered > 0 {
                // Just outside: a share of the stroke's colour.
                let t = (outside * covered as f32 / total as f32).min(out_max);
                out.px[i] = nearest(&pal, mix(own, acc, covered, t), 256);
                n += 1;
            } else if changed[i] && inside > 0.0 && nb > 0 {
                // The stroke's own edge (High): a share of what is behind it.
                let t = (inside * nb as f32 / total as f32).min(in_max);
                out.px[i] = nearest(&pal, mix(own, behind, nb, t), 256);
                n += 1;
            }
        }
    }
    *doc.img_mut() = out;
    n
}

/// Draw a brush line from a to b, stamping every `spacing` pixels (0 = continuous).
pub fn stroke_line(doc: &mut Document, brush: &Brush, set: &PaintSettings, st: &mut Stroke, a: (i32, i32), b: (i32, i32)) {
    let mut pts = Vec::new();
    crate::pix::line_points(a.0, a.1, b.0, b.1, |x, y| pts.push((x, y)));
    stamp_points(doc, brush, set, st, &pts);
}

/// Stamp the brush along points, following Spacing (continuous, every N pixels travelled, N total,
/// airbrush bursts). Returns how many times the brush was put down.
pub fn stamp_points(doc: &mut Document, brush: &Brush, set: &PaintSettings, st: &mut Stroke, pts: &[(i32, i32)]) -> usize {
    // N Total: that many dots spread evenly along the whole line or outline.
    if set.spacing_total > 0 && pts.len() > 1 {
        let n = set.spacing_total as usize;
        let mut chosen: Vec<(i32, i32)> = (0..n).map(|k| pts[if n == 1 { 0 } else { k * (pts.len() - 1) / (n - 1) }]).collect();
        chosen.dedup();
        let mut plain = set.clone();
        plain.spacing_total = 0;
        plain.spacing = 0;
        for p in chosen {
            st.last = None;
            if set.spacing_air {
                airbrush(doc, brush, &plain, st, p.0, p.1);
            } else {
                stamp(doc, brush, &plain, st, p.0, p.1);
            }
        }
        st.last = pts.last().copied();
        return 0;
    }
    let spacing = set.spacing.max(0);
    let mut stamped = 0;
    for &(x, y) in pts {
        if st.last == Some((x, y)) {
            continue;
        }
        if spacing > 1 {
            if let Some((lx, ly)) = st.last {
                // Count the distance actually travelled (from the previous point, stamped or not).
                st.dist_acc += (((x - lx).pow(2) + (y - ly).pow(2)) as f32).sqrt();
                if st.dist_acc < spacing as f32 {
                    st.last = Some((x, y));
                    continue;
                }
                st.dist_acc -= spacing as f32;
                if st.dist_acc >= spacing as f32 {
                    st.dist_acc = 0.0;
                }
            }
        }
        stamped += 1;
        if set.spacing_air {
            // Airbrush spacing: Spacing's number of sprays at each point.
            let saved = st.last;
            let mut burst = set.clone();
            burst.airbrush_density = set.spacing_air_n.max(1);
            airbrush(doc, brush, &burst, st, x, y);
            st.last = saved;
        } else {
            stamp(doc, brush, set, st, x, y);
        }
        st.last = Some((x, y));
    }
    stamped
}

/// One airbrush burst at (x,y).
pub fn airbrush(doc: &mut Document, brush: &Brush, set: &PaintSettings, st: &mut Stroke, x: i32, y: i32) {
    let r = set.airbrush_radius.max(1) as f32;
    let dot;
    let brush = if set.airbrush_old {
        // Old Style: single dots of the colour.
        dot = Brush::builtin(crate::pix::BuiltinShape::Round, 1);
        &dot
    } else {
        brush
    };
    let mut set = set.clone();
    set.spacing_air = false;
    let set = &set;
    for _ in 0..set.airbrush_density.max(1) {
        let a = st.rng.f() * std::f32::consts::TAU;
        let d = st.rng.f().sqrt() * r;
        let px = x + (a.cos() * d).round() as i32;
        let py = y + (a.sin() * d).round() as i32;
        let saved = st.last;
        st.last = None;
        stamp(doc, brush, set, st, px, py);
        st.last = saved;
    }
}

const BAYER4: [[f32; 4]; 4] = [[0.0, 8.0, 2.0, 10.0], [12.0, 4.0, 14.0, 6.0], [3.0, 11.0, 1.0, 9.0], [15.0, 7.0, 13.0, 5.0]];

/// Pick the gradient colour for position t (0..1) in a range, with the chosen dither.
/// `seq` is the range's colour sequence (Range::sequence), worked out once per fill.
pub fn gradient_color(seq: &[u8], t: f32, set: &PaintSettings, x: i32, y: i32, rng: &mut Rng) -> u8 {
    if seq.is_empty() {
        return 0;
    }
    let n = seq.len() as f32;
    let t = t.clamp(0.0, 0.9999);
    let amt = set.spatter as f32 / 100.0;
    let jitter = match set.dither {
        Dither::Smooth => 0.0,
        Dither::Random => (rng.f() - 0.5) * (0.35 + amt * 1.65),
        Dither::Pattern => (BAYER4[(y & 3) as usize][(x & 3) as usize] / 16.0 - 0.47) * (0.5 + amt),
    };
    let k = ((t * n) + jitter).floor().clamp(0.0, n - 1.0) as usize;
    seq[k]
}

/// Fill the masked pixels of the current frame with the chosen fill type.
pub fn fill_mask(doc: &mut Document, mask: &[bool], set: &PaintSettings, color: u8, brush: &Brush, rng: &mut Rng) {
    let w = doc.w;
    let h = doc.h;
    let (mut minx, mut miny, mut maxx, mut maxy) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for y in 0..h {
        for x in 0..w {
            if mask[(y * w + x) as usize] {
                minx = minx.min(x);
                maxx = maxx.max(x);
                miny = miny.min(y);
                maxy = maxy.max(y);
            }
        }
    }
    if minx > maxx {
        return;
    }
    if set.fill_page && set.fill.is_gradient() {
        minx = 0;
        miny = 0;
        maxx = w - 1;
        maxy = h - 1;
    }
    let range = range_of(&doc.ranges, set.range).cloned().unwrap_or(Range::new(color, color, 0));
    // The colours the gradient runs through, worked out once for the whole fill.
    let seq = range.sequence(&doc.palette);
    let bwid = (maxx - minx + 1) as f32;
    let bhei = (maxy - miny + 1) as f32;
    let (cx, cy) = ((minx + maxx) as f32 / 2.0, (miny + maxy) as f32 / 2.0);
    let edge = if matches!(set.fill, FillKind::Contour | FillKind::Shaped) { Some(edge_distance(mask, w, h)) } else { None };
    let maxd = edge.as_ref().map(|d| d.iter().cloned().max().unwrap_or(1).max(1) as f32).unwrap_or(1.0);
    // With a directional line, every range fill is laid out from the line.
    let t_map = match set.grad_line {
        Some((c, e)) if set.fill.is_gradient() => Some(line_t_map(set.fill, mask, w, h, (minx, miny, maxx, maxy), c, e)),
        _ => None,
    };
    let ang = set.fill_angle.to_radians();
    let (dirx, diry) = (ang.cos(), ang.sin());
    let corners = [(minx, miny), (maxx, miny), (minx, maxy), (maxx, maxy)];
    let projs: Vec<f32> = corners.iter().map(|c| c.0 as f32 * dirx + c.1 as f32 * diry).collect();
    let pmin = projs.iter().cloned().fold(f32::MAX, f32::min);
    let pmax = projs.iter().cloned().fold(f32::MIN, f32::max);
    let rmax = ((bwid / 2.0).powi(2) + (bhei / 2.0).powi(2)).sqrt().max(1.0);
    let (hx, hy) = (minx as f32 + bwid * 0.33, miny as f32 + bhei * 0.33);
    let hmax = ((bwid * 0.67).powi(2) + (bhei * 0.67).powi(2)).sqrt().max(1.0);
    let wrap = if set.fill == FillKind::Wrap { Some(brush.img.scaled(bwid as i32, bhei as i32)) } else { None };
    let plane = if set.fill == FillKind::Perspective { set.persp_plane.clone() } else { None };
    let persp = if plane.is_none() && set.fill == FillKind::Perspective && brush.custom {
        Some(brush.perspective(set.persp.0, set.persp.1, set.persp.2, false, &doc.palette))
    } else {
        None
    };
    let keep = set.keep_pct();
    let pal = doc.palette;
    for y in miny..=maxy {
        for x in minx..=maxx {
            if !doc.img().inside(x, y) {
                continue;
            }
            let i = (y * w + x) as usize;
            if !mask[i] || doc.protected(x, y) || !set.texture_allows(x, y) {
                continue;
            }
            let existing = doc.img().get(x, y);
            let c = match set.fill {
                FillKind::Solid => color,
                FillKind::Perspective if plane.is_some() => match plane.as_ref().unwrap().sample(x, y) {
                    Some(p) => p,
                    None => continue,
                },
                FillKind::Brush | FillKind::Pattern | FillKind::Perspective => {
                    let b: &Brush = match set.fill {
                        FillKind::Pattern => match &set.pattern {
                            Some(p) => p,
                            None => brush,
                        },
                        FillKind::Perspective => persp.as_ref().unwrap_or(brush),
                        _ => brush,
                    };
                    if !b.custom {
                        color
                    } else {
                        let p = b.img.get((x - minx).rem_euclid(b.img.w), (y - miny).rem_euclid(b.img.h));
                        if Some(p) == b.transparent {
                            continue;
                        }
                        p
                    }
                }
                FillKind::Wrap => {
                    if !brush.custom {
                        color
                    } else {
                        let p = wrap.as_ref().unwrap().get(x - minx, y - miny);
                        if Some(p) == brush.transparent {
                            continue;
                        }
                        p
                    }
                }
                FillKind::Rub => match &set.rub {
                    Some(r) => {
                        let (img, rp) = &**r;
                        let c = rp[img.get(x, y) as usize];
                        nearest(&pal, [c[0] as i32, c[1] as i32, c[2] as i32], 256)
                    }
                    None => color,
                },
                FillKind::HBrite => {
                    let c = pal[existing as usize];
                    nearest(&pal, [c[0] as i32 / 2, c[1] as i32 / 2, c[2] as i32 / 2], 256)
                }
                _ => {
                    let t = match (&t_map, set.fill) {
                        (Some(tm), _) => tm[i],
                        (None, FillKind::Linear) => (x as f32 * dirx + y as f32 * diry - pmin) / (pmax - pmin).max(1.0),
                        (None, FillKind::Circular) => ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / rmax,
                        (None, FillKind::Highlight) => ((x as f32 - hx).powi(2) + (y as f32 - hy).powi(2)).sqrt() / hmax,
                        (None, FillKind::Contour) => 1.0 - edge.as_ref().unwrap()[i] as f32 / maxd,
                        (None, _) => {
                            // Shaped: rings that follow the outline, from the edge to the centre.
                            let d_edge = edge.as_ref().unwrap()[i] as f32;
                            let d_c = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                            d_c / (d_c + d_edge).max(1.0)
                        }
                    };
                    let g = gradient_color(&seq, t, set, x, y, rng);
                    if set.range_fade > 0 {
                        blend(&pal, g, existing, set.range_fade as f32 / 25.0 * t.clamp(0.0, 1.0))
                    } else {
                        g
                    }
                }
            };
            let c = if set.process_on { process(&pal, set.process, c, existing) } else { c };
            let c = if keep > 0.0 { blend(&pal, c, existing, keep) } else { c };
            doc.img_mut().set(x, y, c);
        }
    }
}

/// Gradient position (0 = first colour, 1 = last) of every pixel for a fill laid out by a
/// directional line from `c` (centre of the area) to `e` (where the person pointed).
/// Line and Shap run at the line's angle across the whole area, starting at the end of the line;
/// Cir, Cont and Hi radiate from the end of the line out to the edge of the area.
pub fn line_t_map(kind: FillKind, mask: &[bool], w: i32, h: i32, bbox: (i32, i32, i32, i32), c: (f32, f32), e: (f32, f32)) -> Vec<f32> {
    let (minx, miny, maxx, maxy) = bbox;
    let mut t = vec![0f32; mask.len()];
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && mask[(y * w + x) as usize];
    let (mut ux, mut uy) = (e.0 - c.0, e.1 - c.1);
    let len = (ux * ux + uy * uy).sqrt();
    if len < 0.5 {
        // No direction yet: from the top down.
        ux = 0.0;
        uy = -1.0;
    } else {
        ux /= len;
        uy /= len;
    }
    match kind {
        FillKind::Linear | FillKind::Shaped => {
            let along = |x: i32, y: i32| (x as f32 + 0.5) * ux + (y as f32 + 0.5) * uy;
            let across = |x: i32, y: i32| ((x as f32 + 0.5) * -uy + (y as f32 + 0.5) * ux).floor() as i64;
            let (mut smin, mut smax) = (f32::MAX, f32::MIN);
            let mut bins: std::collections::HashMap<i64, (f32, f32)> = std::collections::HashMap::new();
            for y in miny..=maxy {
                for x in minx..=maxx {
                    if !inside(x, y) {
                        continue;
                    }
                    let s = along(x, y);
                    smin = smin.min(s);
                    smax = smax.max(s);
                    if kind == FillKind::Shaped {
                        let b = bins.entry(across(x, y)).or_insert((f32::MAX, f32::MIN));
                        b.0 = b.0.min(s);
                        b.1 = b.1.max(s);
                    }
                }
            }
            for y in miny..=maxy {
                for x in minx..=maxx {
                    if !inside(x, y) {
                        continue;
                    }
                    let s = along(x, y);
                    // Shap measures each strip across the shape on its own, so the bands follow its outline.
                    let (lo, hi) = if kind == FillKind::Shaped { bins[&across(x, y)] } else { (smin, smax) };
                    t[(y * w + x) as usize] = (hi - s) / (hi - lo).max(1.0);
                }
            }
        }
        _ => {
            let edge = if kind == FillKind::Circular { None } else { Some(edge_distance(mask, w, h)) };
            let dist = |x: i32, y: i32| ((x as f32 + 0.5 - e.0).powi(2) + (y as f32 + 0.5 - e.1).powi(2)).sqrt();
            let mut dmax = 1f32;
            for y in miny..=maxy {
                for x in minx..=maxx {
                    if inside(x, y) {
                        dmax = dmax.max(dist(x, y));
                    }
                }
            }
            for y in miny..=maxy {
                for x in minx..=maxx {
                    if !inside(x, y) {
                        continue;
                    }
                    let i = (y * w + x) as usize;
                    let dp = dist(x, y);
                    t[i] = match &edge {
                        None => dp / dmax,
                        Some(ed) => {
                            // Contours: rings that shrink towards the outline of the area.
                            let v = dp / (dp + ed[i] as f32).max(1.0);
                            if kind == FillKind::Highlight { v.sqrt() } else { v }
                        }
                    };
                }
            }
        }
    }
    t
}

fn edge_distance(mask: &[bool], w: i32, h: i32) -> Vec<i32> {
    let mut d = vec![i32::MAX; mask.len()];
    let mut q = std::collections::VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if !mask[i] {
                continue;
            }
            let edge = x == 0
                || y == 0
                || x == w - 1
                || y == h - 1
                || !mask[i - 1]
                || !mask[i + 1]
                || !mask[i - w as usize]
                || !mask[i + w as usize];
            if edge {
                d[i] = 0;
                q.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = q.pop_front() {
        let i = (y * w + x) as usize;
        for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                continue;
            }
            let j = (ny * w + nx) as usize;
            if mask[j] && d[j] == i32::MAX {
                d[j] = d[i] + 1;
                q.push_back((nx, ny));
            }
        }
    }
    d.iter().map(|&v| if v == i32::MAX { 0 } else { v }).collect()
}

/// Mask from spans (filled shapes).
pub fn spans_mask(spans: &[(i32, i32, i32)], w: i32, h: i32) -> Vec<bool> {
    let mut m = vec![false; (w * h) as usize];
    for &(y, x0, x1) in spans {
        if y < 0 || y >= h {
            continue;
        }
        for x in x0.max(0)..=x1.min(w - 1) {
            m[(y * w + x) as usize] = true;
        }
    }
    m
}

/// Make a stencil mask from a set of selected colours (true = protected).
pub fn stencil_from_colors(img: &Image, protect: &[bool; 256]) -> Vec<bool> {
    img.px.iter().map(|&p| protect[p as usize]).collect()
}

/// Lock colours similar to the picked ones, within a tolerance (0..=48) as in Make Stencil.
pub fn colors_within(pal: &Palette, picked: &[bool; 256], tolerance: u8) -> [bool; 256] {
    let mut out = *picked;
    if tolerance == 0 {
        return out;
    }
    let tol = (tolerance as i32 * 16).pow(2);
    for i in 0..256 {
        if picked[i] {
            for j in 0..256 {
                let d: i32 = (0..3).map(|k| (pal[i][k] as i32 - pal[j][k] as i32).pow(2)).sum();
                if d <= tol {
                    out[j] = true;
                }
            }
        }
    }
    out
}
