//! Application state, the frame loop, keyboard shortcuts and the drawing canvas.

use crate::convert::{ConvertOptions, Rgba};
use crate::doc::Document;
use crate::gallery::{Gallery, Show};
use crate::paint::{self, FillKind, Mode, PaintSettings, Rng, Stroke};
use crate::palette::{to_color32, Palette};
use crate::pix::{self, Brush, BuiltinShape, Image};
use crate::text::{FontEntry, TextStyle};
use egui::{Color32, Pos2, Rect, Sense, Vec2};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Dotted,
    Freehand,
    Line,
    Curve,
    Fill,
    Airbrush,
    Rect,
    Circle,
    Ellipse,
    Polygon,
    BrushPick,
    Lasso,
    Text,
    Eyedropper,
}

impl Tool {
    pub fn label(&self) -> &'static str {
        match self {
            Tool::Dotted => "Dotted freehand (s)",
            Tool::Freehand => "Continuous freehand (d)",
            Tool::Line => "Straight line (v)",
            Tool::Curve => "Curve (q)",
            Tool::Fill => "Fill (f)",
            Tool::Airbrush => "Airbrush (a)",
            Tool::Rect => "Rectangle (r / R filled)",
            Tool::Circle => "Circle (c / C filled)",
            Tool::Ellipse => "Ellipse (e / E filled)",
            Tool::Polygon => "Polygon (w / W filled)",
            Tool::BrushPick => "Brush selector (b)",
            Tool::Lasso => "Freehand brush selector (B with Shift)",
            Tool::Text => "Text (t)",
            Tool::Eyedropper => "Pick colour (,)",
        }
    }
    pub fn fillable(&self) -> bool {
        matches!(self, Tool::Rect | Tool::Circle | Tool::Ellipse | Tool::Polygon | Tool::Freehand)
    }
}

/// A drag in progress on the canvas.
pub struct Drag {
    pub start: (i32, i32),
    pub primary: bool,
    pub base: Image,
    pub points: Vec<(i32, i32)>,
    /// Ctrl trails: the picture with the lines drawn so far (later previews go on top of it).
    pub trails_base: Option<Image>,
}

pub struct CurveWait {
    pub a: (i32, i32),
    pub b: (i32, i32),
    pub primary: bool,
    pub base: Image,
}

pub struct TextEntry {
    pub pos: (i32, i32),
    pub buffer: String,
    pub base: Image,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pending {
    New,
    Open,
    Quit,
    Revert,
}

pub struct ConvertJob {
    pub rgba: Rgba,
    pub opts: ConvertOptions,
    pub path: Option<PathBuf>,
    /// Load into a new picture (true) or as a brush (false).
    pub as_picture: bool,
}

pub struct AnimBrush {
    pub cells: Vec<Image>,
    /// The cel the brush shows now.
    pub pos: usize,
    /// The cel the sequence starts from (AnimBrush Settings > Current).
    pub start: usize,
    /// 1 forward, -1 reverse, 0 ping-pong (AnimBrush Settings).
    pub dir: i32,
    /// Stamps (or frames) one run through all the cels takes (AnimBrush Settings > Duration):
    /// the number of cels plays one cel per stamp; more is slower, fewer skips cels.
    pub duration: u32,
    /// Stamps since the sequence started.
    pub ticks: u32,
    pub ping: i32,
}

impl AnimBrush {
    pub fn new(cells: Vec<Image>) -> AnimBrush {
        let n = cells.len() as u32;
        AnimBrush { cells, pos: 0, start: 0, dir: 1, duration: n.max(1), ticks: 0, ping: 1 }
    }

    /// The cel shown `step` stamps or frames after the start of the sequence, following the
    /// direction (forward, backward, ping-pong) and the duration (steps for a run through all cels).
    pub fn cell_at(&self, step: usize) -> usize {
        let n = self.cells.len().max(1);
        let d = if self.duration == 0 { n } else { self.duration as usize };
        let s = step * n / d.max(1);
        match self.dir {
            dd if dd > 0 => (self.start + s) % n,
            dd if dd < 0 => (self.start + n - s % n) % n,
            _ => {
                if n == 1 {
                    return 0;
                }
                // Ping-pong runs 0..n-1 and back: a period of 2n-2 positions.
                let period = 2 * n - 2;
                let i = (self.start + s) % period;
                if i < n {
                    i
                } else {
                    period - i
                }
            }
        }
    }

    /// Show the cel after one more stamp.
    pub fn advance(&mut self) {
        self.ticks = self.ticks.wrapping_add(1);
        self.pos = self.cell_at(self.ticks as usize);
    }

    /// Start the sequence again from cel `c` (0-based).
    pub fn restart_at(&mut self, c: usize) {
        self.start = c.min(self.cells.len().saturating_sub(1));
        self.pos = self.start;
        self.ticks = 0;
    }
}

pub struct App {
    pub doc: Document,
    pub spare: Document,
    pub brush: Brush,
    pub brush_original: Option<Brush>,
    pub builtin: (BuiltinShape, i32),
    pub anim_brush: Option<AnimBrush>,
    pub last_pick: Option<(i32, i32, i32, i32)>,
    pub fg: u8,
    pub bg: u8,
    pub set: PaintSettings,
    pub tool: Tool,
    pub filled: bool,
    pub zoom: f32,
    pub pan: Vec2,
    pub fit_pending: bool,
    pub magnify: bool,
    pub mag_zoom: f32,
    pub mag_center: (f32, f32),
    /// Last sizes of the main view and the magnified view, in screen pixels (for arrow-key steps).
    pub view_sizes: (Vec2, Vec2),
    pub drag: Option<Drag>,
    pub curve: Option<CurveWait>,
    pub stroke: Option<Stroke>,
    pub text_entry: Option<TextEntry>,
    pub cursor: Option<(i32, i32)>,
    pub texture: Option<egui::TextureHandle>,
    pub cycling: bool,
    pub multi_cycle: bool,
    pub cycle_phases: Vec<f32>,
    pub info_bar: bool,
    pub coords: bool,
    pub antialias: bool,
    pub aa_high: bool,
    pub light_table: bool,
    pub playing: bool,
    pub play_accum: f32,
    pub view_page: bool,
    pub brush_handle: bool,
    pub backup_pict: bool,
    pub rng: Rng,
    // dialogs
    pub show_palette: bool,
    pub show_fill: bool,
    pub show_symmetry: bool,
    pub show_grid: bool,
    pub show_spacing: bool,
    pub show_airbrush: bool,
    pub show_perspective: bool,
    pub show_page_size: bool,
    pub show_new: bool,
    pub show_anim: bool,
    pub show_move: bool,
    pub show_stencil: bool,
    pub show_font: bool,
    pub show_about: bool,
    pub show_keys: bool,
    pub show_translucent: bool,
    pub show_gallery: bool,
    pub show_camera: bool,
    pub show_rotate: bool,
    pub show_resize_brush: bool,
    pub message: Option<(String, String)>,
    pub pending: Option<Pending>,
    pub convert: Option<ConvertJob>,
    // dialog values
    pub new_size: (i32, i32),
    pub page_size: (i32, i32, bool),
    pub persp: (f32, f32, f32),
    pub rotate_deg: f32,
    pub shear_amt: i32,
    pub brush_size_edit: (i32, i32),
    pub frames_edit: usize,
    pub move_params: MoveParams,
    pub stencil_pick: [bool; 256],
    pub pal_action: Option<PalAction>,
    pub pal_undo: Option<Palette>,
    pub sym_place: bool,
    pub camera_delay: f32,
    pub camera_at: Option<f64>,
    // text
    pub fonts: Vec<FontEntry>,
    pub font_idx: usize,
    pub font: Option<crate::text::LoadedFont>,
    pub text_style: TextStyle,
    // gallery
    pub gallery: Gallery,
    pub show: Option<Show>,
    pub gallery_tex: Option<egui::TextureHandle>,
    // screenshot mode
    pub screenshot: Option<PathBuf>,
    pub frame_count: u64,
    pub shot_requested: bool,
    pub shot_result: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    pub status: String,
    pub time: f64,
    pub quit_now: bool,
    pub pal_page: usize,
    pub v5: crate::actions::V5,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PalAction {
    Spread,
    Copy,
    Swap,
    RangeFromTo,
}

#[derive(Clone, Debug)]
pub struct MoveParams {
    /// Number of frames to draw into (Count).
    pub frames: usize,
    /// Total distance moved along X, Y and Z (Z moves away from the viewer).
    pub dist: [i32; 3],
    /// Total rotation around X, Y and Z, in degrees.
    pub angle: [f32; 3],
    /// Frames to speed up over at the start / slow down over at the end.
    pub slow_in: usize,
    pub slow_out: usize,
    /// Wrap round to the first frame when the end of the animation is reached.
    pub cyclic: bool,
    pub start: (i32, i32),
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, open: Option<PathBuf>) -> Self {
        crate::ui_style::apply(&cc.egui_ctx);
        let fonts = crate::text::system_fonts();
        let font_idx = fonts.iter().position(|f| f.name.eq_ignore_ascii_case("arial")).unwrap_or(0);
        let font = fonts.get(font_idx).and_then(|f| crate::text::load_entry(f, 24));
        let (w, h) = (640, 480);
        let mut app = App {
            doc: Document::new(w, h, 0),
            spare: Document::new(w, h, 0),
            brush: Brush::builtin(BuiltinShape::Round, 1),
            brush_original: None,
            builtin: (BuiltinShape::Round, 1),
            anim_brush: None,
            last_pick: None,
            fg: 15,
            bg: 0,
            set: PaintSettings::default(),
            tool: Tool::Freehand,
            filled: false,
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            magnify: false,
            mag_zoom: 6.0,
            mag_center: (w as f32 / 2.0, h as f32 / 2.0),
            view_sizes: (Vec2::new(800.0, 600.0), Vec2::new(480.0, 600.0)),
            drag: None,
            curve: None,
            stroke: None,
            text_entry: None,
            cursor: None,
            texture: None,
            cycling: false,
            multi_cycle: true,
            cycle_phases: vec![0.0; 16],
            info_bar: true,
            coords: false,
            antialias: false,
            aa_high: false,
            light_table: false,
            playing: false,
            play_accum: 0.0,
            view_page: false,
            brush_handle: false,
            backup_pict: true,
            rng: Rng::new(0x5eed_1234),
            show_palette: false,
            show_fill: false,
            show_symmetry: false,
            show_grid: false,
            show_spacing: false,
            show_airbrush: false,
            show_perspective: false,
            show_page_size: false,
            show_new: false,
            show_anim: false,
            show_move: false,
            show_stencil: false,
            show_font: false,
            show_about: false,
            show_keys: false,
            show_translucent: false,
            show_gallery: false,
            show_camera: false,
            show_rotate: false,
            show_resize_brush: false,
            message: None,
            pending: None,
            convert: None,
            new_size: (640, 480),
            page_size: (640, 480, false),
            persp: (30.0, 0.0, 0.0),
            rotate_deg: 45.0,
            shear_amt: 16,
            brush_size_edit: (32, 32),
            frames_edit: 8,
            // As the manual's Move requester first shows: all distances and angles 0, Cyclic on.
            move_params: MoveParams { frames: 2, dist: [0, 0, 0], angle: [0.0, 0.0, 0.0], slow_in: 0, slow_out: 0, cyclic: true, start: (w / 2, h / 2) },
            stencil_pick: [false; 256],
            pal_action: None,
            pal_undo: None,
            sym_place: false,
            camera_delay: 3.0,
            camera_at: None,
            fonts,
            font_idx,
            font,
            text_style: TextStyle::default(),
            gallery: Gallery::default(),
            show: None,
            gallery_tex: None,
            screenshot,
            frame_count: 0,
            shot_requested: false,
            shot_result: Arc::new(Mutex::new(None)),
            status: String::new(),
            time: 0.0,
            quit_now: false,
            pal_page: 0,
            v5: crate::actions::V5::new(),
        };
        app.multi_cycle = app.v5.prefs.multi_cycle;
        if let Some(dir) = app.v5.prefs.font_dir.clone() {
            let fonts = crate::text::fonts_in(&dir);
            if !fonts.is_empty() {
                app.fonts = fonts;
                app.font_idx = 0;
                app.font = crate::text::load_entry(&app.fonts[0], app.text_style.size);
            }
        }
        if let Some(p) = open {
            app.open_path(p);
        }
        // Preview a requester at start-up (used to check the look in screenshots).
        match std::env::var("DPX_DIALOG").unwrap_or_default().as_str() {
            "move" => {
                app.show_move = true;
                if std::env::var("DPX_MOVE").is_ok() {
                    // A striped card brush stamped low on the page, then a 3-D move: up 200, 400 into
                    // the distance, a full turn about Z, 20 frames; drawn, then previewed.
                    let mut card = Image::new(80, 40, 0);
                    for y in 0..40 {
                        for x in 0..80 {
                            let edge = x < 3 || y < 3 || x > 76 || y > 36;
                            card.set(x, y, if edge { 15 } else if (x / 10 + y / 10) % 2 == 0 { 34 } else { 66 });
                        }
                    }
                    app.brush = Brush::from_image(card, Some(0));
                    app.doc.set_frame_count(20);
                    app.move_note_stamp((160, 400));
                    app.move_params.frames = 20;
                    app.move_params.dist = [0, 200, 400];
                    app.move_params.angle = [0.0, 0.0, 360.0];
                    app.move_params.cyclic = false;
                    app.do_move_3d(3);
                    app.light_table = true;
                    // Preview from a fresh stamp low on the right, so the wire frames show.
                    app.move_note_stamp((480, 420));
                    app.v5.x.move_preview_on = true;
                    if std::env::var("DPX_TRAILS").is_ok() {
                        // As the person did: Trails twice during the preview, then clear all frames.
                        app.do_move_3d(1);
                        app.do_move_3d(1);
                        for i in 0..app.doc.frames.len() {
                            app.doc.cur = i;
                            app.doc.clear(0);
                        }
                        app.doc.go_to(0);
                        app.move_forget_end();
                    }
                }
            }
            "spacing" => {
                // Every 17th dot, with Dotted (top) and Continuous freehand (bottom), through the
                // normal mouse handling; the Spacing requester open.
                app.fg = 15;
                app.set_builtin(BuiltinShape::Round, 9);
                if std::env::var("DPX_NTOTAL").is_ok() {
                    app.set.spacing_total = 20;
                } else {
                    app.set.spacing = 17;
                }
                app.show_spacing = true;
                for (k, tool) in [Tool::Dotted, Tool::Freehand].into_iter().enumerate() {
                    app.tool = tool;
                    let ev = |p: (i32, i32), pr: bool, down: bool, rel: bool| crate::macros::PtrIn { pos: p, hovered: true, pressed_p: pr, pressed_s: false, down_p: down, down_s: false, released: rel, dbl: false };
                    let y = |x: i32| 250 + k as i32 * 120 + ((x as f32 / 50.0).sin() * 40.0) as i32;
                    app.pointer_logic(&ev((60, y(60)), true, true, false));
                    for x in (62..560).step_by(2) {
                        app.pointer_logic(&ev((x, y(x)), false, true, false));
                    }
                    app.pointer_logic(&ev((560, y(560)), false, false, true));
                }
            }
            "outline" => {
                // Filled shapes with outline (key i): the fill in one colour, then the current brush
                // traces the edge in another; the edge uses Airbrush spacing.
                app.v5.x.fill_outline = true;
                app.set.spacing_air = true;
                app.set.airbrush_radius = 6;
                app.set.airbrush_density = 12;
                app.set_builtin(BuiltinShape::Round, 2);
                let ev = |p: (i32, i32), pr: bool, down: bool, rel: bool| crate::macros::PtrIn { pos: p, hovered: true, pressed_p: pr, pressed_s: false, down_p: down, down_s: false, released: rel, dbl: false };
                for (k, (tool, a, b)) in [(Tool::Rect, (60, 60), (200, 170)), (Tool::Circle, (330, 115), (390, 115)), (Tool::Ellipse, (520, 115), (590, 165))].into_iter().enumerate() {
                    app.tool = tool;
                    app.filled = true;
                    app.fg = [36, 98, 120][k];
                    // The fill colour; the outline follows in the colour below.
                    app.pointer_logic(&ev(a, true, true, false));
                    app.pointer_logic(&ev(b, false, true, false));
                    app.pointer_logic(&ev(b, false, false, true));
                }
                app.set.spacing_air = false;
                app.set_builtin(BuiltinShape::Round, 5);
                app.tool = Tool::Rect;
                app.fg = 66;
                for (a, b) in [((80, 260), (580, 420))] {
                    app.pointer_logic(&ev(a, true, true, false));
                    app.pointer_logic(&ev(b, false, true, false));
                    app.pointer_logic(&ev(b, false, false, true));
                }
            }
            // Requesters of the reference section, one per name.
            "rays" => {
                // A line dragged with Ctrl held (trails): a ray of lines from the start point; then a
                // rectangle the same way.
                app.fg = 15;
                let ev = |p: (i32, i32), pr: bool, down: bool, rel: bool| crate::macros::PtrIn { pos: p, hovered: true, pressed_p: pr, pressed_s: false, down_p: down, down_s: false, released: rel, dbl: false };
                for (tool, start, k0) in [(Tool::Line, (160, 300), 0), (Tool::Rect, (420, 120), 1)] {
                    app.tool = tool;
                    app.filled = false;
                    app.fg = if k0 == 0 { 15 } else { 36 };
                    app.pointer_logic(&ev(start, true, true, false));
                    app.v5.x.ctrl_down = true;
                    for k in 0..24 {
                        let a = k as f32 / 24.0 * std::f32::consts::PI;
                        let r = if k0 == 0 { 140.0 } else { 20.0 + k as f32 * 6.0 };
                        let p = if k0 == 0 { (start.0 + (a.cos() * r) as i32, start.1 - (a.sin() * r) as i32) } else { (start.0 + r as i32, start.1 + (r * 0.6) as i32) };
                        app.pointer_logic(&ev(p, false, true, false));
                    }
                    app.pointer_logic(&ev((start.0 - 140, start.1), false, false, true));
                    app.v5.x.ctrl_down = false;
                }
            }
            "fpal" | "fpalinfo" => {
                app.doc.set_frame_count(20);
                app.v5.show_anim_panel = true;
                app.open_frame_palette(std::env::var("DPX_DIALOG").unwrap_or_default() == "fpalinfo");
            }
            "colourset" => {
                // The person's media folder's Hues.set, or a made-up 16-colour set, being chosen from.
                let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("media").join("COLOURS").join("HUES.SET");
                let (cols, ranges) = match crate::amigafiles::read_palette_file(&path) {
                    Ok(pf) => (pf.colors, pf.ranges),
                    Err(_) => ((0..16u8).map(|k| [k * 16, 255 - k * 16, 128]).collect(), Vec::new()),
                };
                let mut cs = crate::colorset::ColourSetChoice::new(cols, ranges);
                for k in 0..12 {
                    cs.selected[k] = true;
                }
                app.v5.x.cset = Some(cs);
            }
            // What follows a colour set's OVERWRITE: Arrange, to put the colours where they belong.
            "arrange" => {
                let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("media").join("COLOURS").join("HUES.SET");
                if let Ok(pf) = crate::amigafiles::read_palette_file(&path) {
                    crate::colorset::overwrite_colours(&mut app.doc.palette, &pf.colors);
                }
                app.show_palette = true;
                app.v5.x.mixer.arrange = true;
                app.fg = 4;
            }
            // As in the person's screenshot: pinks side by side, then single colours further along.
            "rangecells" => {
                let mut cells: Vec<(u8, u8)> = (0..8).map(|k| (k as u8, 48 + k as u8)).collect();
                cells.extend([(18, 115), (29, 93), (35, 9)]);
                app.doc.ranges[0] = crate::palette::Range::from_cells(cells, 0);
                app.set.range = 0;
                app.v5.show_ranges = true;
                app.fg = 9;
            }
            "font" => app.show_font = true,
            "airbrush8" => app.show_airbrush = true,
            "grid8" => app.show_grid = true,
            "sym8" => app.show_symmetry = true,
            "page8" => app.show_page_size = true,
            "transl8" => app.show_translucent = true,
            "cam8" => {
                app.doc.set_frame_count(4);
                app.v5.x.show_cam = true;
            }
            "rate8" => {
                app.doc.set_frame_count(30);
                app.open_rate();
            }
            "copy8" => {
                app.doc.set_frame_count(10);
                app.open_copy();
            }
            "aa" => {
                // Circles and lines drawn through the normal mouse handling with AntiAlias off,
                // Low and High (left to right), magnified.
                app.fg = 4;
                app.set_builtin(BuiltinShape::Round, 3);
                for (k, aa) in [None, Some(false), Some(true)].into_iter().enumerate() {
                    app.antialias = aa.is_some();
                    app.aa_high = aa == Some(true);
                    let cx = 60 + k as i32 * 90;
                    for (tool, a, b) in [(Tool::Circle, (cx, 70), (cx + 30, 70)), (Tool::Line, (cx - 30, 140), (cx + 30, 170))] {
                        app.tool = tool;
                        app.filled = false;
                        let ev = |p: (i32, i32), pr: bool, down: bool, rel: bool| crate::macros::PtrIn { pos: p, hovered: true, pressed_p: pr, pressed_s: false, down_p: down, down_s: false, released: rel, dbl: false };
                        app.pointer_logic(&ev(a, true, true, false));
                        app.pointer_logic(&ev(b, false, true, false));
                        app.pointer_logic(&ev(b, false, false, true));
                    }
                }
                app.antialias = false;
                app.magnify = true;
                app.mag_zoom = 4.0;
                app.mag_center = (150.0, 110.0);
            }
            "animbrush" => {
                // An AnimBrush of 8 coloured cels and its settings requester.
                let cells: Vec<Image> = (0..8u8).map(|k| Image::new(24, 24, 32 + k * 2)).collect();
                app.brush = Brush::from_image(cells[0].clone(), Some(0));
                app.anim_brush = Some(AnimBrush::new(cells));
                app.v5.show_animbrush_settings = true;
            }
            "palette" => app.show_palette = true,
            "about" => app.show_about = true,
            "fill" => app.show_fill = true,
            "fill5" => {
                app.open_fill();
                if std::env::var("DPX_MSG").is_ok() {
                    // As after pressing From brush without a picked-up brush.
                    app.msg("Fill Type", "Must have custom brush.
Pick up a brush first (the brush tool, or b), then press From brush.");
                }
            }
            "frame6" => app.doc.go_to(5.min(app.doc.frames.len() - 1)),
            "chisel" => {
                // Amiga colour font from Font Dir: the project folder holding Chisel/.
                let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
                let fonts = crate::text::fonts_in(&dir);
                if let Some(i) = fonts.iter().position(|f| f.is_bitmap()) {
                    app.fonts = fonts;
                    app.font_idx = i;
                    app.text_style.size = 62;
                    app.font = crate::text::load_entry(&app.fonts[i], 62);
                    for (k, line) in ["Hello", "Amiga 1234"].iter().enumerate() {
                        if let Some(b) = app.text_brush(line) {
                            let mut st = Stroke::new(app.fg, true);
                            let set = app.set.clone();
                            paint::stamp(&mut app.doc, &b, &set, &mut st, 40, 60 + k as i32 * 80);
                        }
                    }
                    app.show_font = true;
                }
            }
            "modes" => {
                // Colour bands, then one mouse drag per mode across them, through the normal
                // mouse handling: Matte, Colour, Replc, Smear, Shade, Blend, Cycle, Smooth, HBrite.
                for y in 0..480 {
                    for x in 0..640 {
                        let c = match (x / 80) % 4 { 0 => 4, 1 => 15, 2 => 18 + (y / 40) as u8 % 10, _ => 2 };
                        app.doc.frames[0].set(x, y, c);
                    }
                }
                app.set_builtin(BuiltinShape::Round, 9);
                app.fg = 11;
                for (k, m) in crate::paint::Mode::ALL.iter().take(9).enumerate() {
                    app.set.mode = *m;
                    // Shade and Cycle work in the foreground colour's range: take a grey.
                    app.fg = if matches!(m, crate::paint::Mode::Shade | crate::paint::Mode::Cycle) { 20 } else { 11 };
                    app.set.fg_range = app.doc.ranges.iter().position(|r| r.contains(app.fg));
                    app.tool = if k % 2 == 0 { Tool::Freehand } else { Tool::Dotted };
                    let y = 30 + k as i32 * 50;
                    let ev = |x: i32, pressed: bool, down: bool, released: bool| crate::macros::PtrIn {
                        pos: (x, y + ((x as f32 / 40.0).sin() * 8.0) as i32),
                        hovered: true,
                        pressed_p: pressed,
                        pressed_s: false,
                        down_p: down,
                        down_s: false,
                        released,
                        dbl: false,
                    };
                    app.pointer_logic(&ev(20, true, true, false));
                    for x in (22..600).step_by(3) {
                        app.pointer_logic(&ev(x, false, true, false));
                    }
                    app.pointer_logic(&ev(600, false, false, true));
                }
                app.set.mode = crate::paint::Mode::Paint;
                app.tool = Tool::Freehand;
            }
            "persp3d" => {
                // A striped card brush stamped in perspective, and the Perspective requester.
                let mut card = Image::new(96, 64, 0);
                for y in 0..64 {
                    for x in 0..96 {
                        let edge = x < 4 || y < 4 || x > 91 || y > 59;
                        card.set(x, y, if edge { 15 } else if (x / 12 + y / 16) % 2 == 0 { 34 } else { 66 });
                    }
                }
                app.brush = Brush::from_image(card, Some(0));
                app.enter_persp();
                app.v5.persp_centre = Some((320, 120));
                for (k, (a, z, pos)) in [([0.0, 0.0, 0.0], 0.0, (90, 380)), ([-60.0, 0.0, 0.0], 0.0, (320, 400)), ([0.0, 50.0, 0.0], 150.0, (540, 330)), ([45.0, 0.0, 45.0], 300.0, (200, 200))].iter().enumerate() {
                    app.v5.x.p3.set_angles(*a);
                    app.v5.x.p3.z = *z;
                    app.cursor = Some(*pos);
                    app.v5.x.p3.last_key = None;
                    app.persp_update(&cc.egui_ctx);
                    let b = app.brush.clone();
                    let mut st = Stroke::new(app.fg, true);
                    let set = app.set.clone();
                    paint::stamp(&mut app.doc, &b, &set, &mut st, pos.0, pos.1);
                    let _ = k;
                }
                if std::env::var("DPX_FILL").is_ok() {
                    // As in the person's picture: brush tilted -76 degrees about X near the bottom,
                    // then Fill Screen: a floor running back to the horizon.
                    app.doc.clear(0);
                    app.v5.x.p3.set_angles([-76.0, 0.0, 0.0]);
                    app.v5.x.p3.z = 0.0;
                    app.cursor = Some((320, 430));
                    app.v5.x.p3.last_key = None;
                    app.persp_update(&cc.egui_ctx);
                    app.persp_char('-');
                }
                if std::env::var("DPX_ZSTEPS").is_ok() {
                    // Shift+" three times (each one further back), then Shift+: twice.
                    app.doc.clear(0);
                    app.v5.x.p3.set_angles([0.0, 0.0, 0.0]);
                    app.v5.x.p3.z = 0.0;
                    let keys = ['"', '"', '"', '"', ':', ':'];
                    for k in 0..=keys.len() {
                        let pos = (60 + k as i32 * 85, 380);
                        app.cursor = Some(pos);
                        app.v5.x.p3.last_key = None;
                        app.persp_update(&cc.egui_ctx);
                        let b = app.brush.clone();
                        let mut st = Stroke::new(app.fg, true);
                        let set = app.set.clone();
                        paint::stamp(&mut app.doc, &b, &set, &mut st, pos.0, pos.1);
                        if let Some(ch) = keys.get(k) {
                            app.persp_char(*ch);
                        }
                    }
                }
                app.v5.x.p3.set_angles([-45.0, 30.0, 0.0]);
                app.v5.x.p3.z = 60.0;
                app.cursor = Some((470, 200));
                app.show_perspective = std::env::var("DPX_SF").is_ok();
            }
            "ecs32" => {
                // A classic Amiga low-res screen: 32 colours, 4 bits per gun.
                let mut d = Document::new(320, 200, 0);
                for y in 0..200 {
                    for x in 0..320 {
                        d.frames[0].set(x, y, ((x / 10 + y / 25) % 32) as u8);
                    }
                }
                d.palette = crate::palette::palette_for(32);
                d.ranges = crate::palette::ranges_for(32);
                app.doc = d;
                app.v5.screen = (320, 200);
                app.format_from_file(5, Some(crate::amiga::MODES[0].mode_id()));
                app.v5.show_ranges = std::env::var("DPX_RANGES").is_ok();
                app.v5.chipset = crate::amiga::Chipset::Ecs;
                app.doc.gun_bits = 4;
                if std::env::var("DPX_SF").is_ok() {
                    app.open_screen_format();
                }
            }
            "lowres" => {
                // A 320 x 200 screen with a 320 x 200 picture.
                let mut d = Document::new(320, 200, 0);
                for y in 0..200 {
                    for x in 0..320 {
                        let c = if (x / 20 + y / 20) % 2 == 0 { 32 + (x / 20) as u8 } else { 16 + (y / 13) as u8 };
                        d.frames[0].set(x, y, c);
                    }
                }
                app.doc = d;
                app.v5.screen = (320, 200);
                app.fit_pending = true;
            }
            "gradline" | "gradfill" => {
                // A range fill with its directional line (and, for gradfill, the result).
                app.set.fill = crate::paint::FillKind::Linear;
                app.set.range = 1;
                app.set.dither = crate::paint::Dither::Random;
                app.set.spatter = 30;
                app.doc.checkpoint();
                let (w, h) = (app.doc.w, app.doc.h);
                let m = app.shape_mask(Tool::Ellipse, (w / 2, h / 2), (w / 2 - 160, h / 2 - 120));
                let base = app.doc.img().clone();
                let fg = app.fg;
                app.begin_grad_line(m, base, fg, false, (w / 2 + 150, h / 2 - 140));
                if std::env::var("DPX_DIALOG").unwrap_or_default() == "gradfill" {
                    app.finish_grad_line();
                }
            }
            "screenformat" => app.open_screen_format(),
            "about5" => app.v5.show_about_v5 = true,
            "panels" => {
                app.v5.show_ranges = true;
                app.v5.show_anim_panel = true;
                app.doc.set_frame_count(8);
            }
            "page" => app.show_page_size = true,
            "magnify" => {
                // As if "m" was pressed and the right arrow key three times.
                app.magnify = true;
                app.mag_center = (60.0, 60.0);
                for _ in 0..3 {
                    app.mag_center.0 += (app.view_sizes.1.x / app.mag_zoom * 0.25).max(1.0);
                }
            }
            _ => {}
        }
        app
    }

    pub fn msg(&mut self, title: &str, text: impl Into<String>) {
        self.message = Some((title.to_string(), text.into()));
    }

    // ------------------------------------------------------------ brushes

    pub fn set_builtin(&mut self, shape: BuiltinShape, size: i32) {
        self.builtin = (shape, size.clamp(1, 64));
        self.brush = Brush::builtin(shape, self.builtin.1);
        self.brush_original = None;
        self.anim_brush = None;
    }

    pub fn transform_brush(&mut self, f: impl FnOnce(&Brush) -> Brush) {
        if !self.brush.custom {
            self.msg("Brush", "Can't transform built-in brush. Must have custom brush.");
            return;
        }
        if self.brush_original.is_none() {
            self.brush_original = Some(self.brush.clone());
        }
        self.brush = f(&self.brush);
        if self.antialias && !matches!(self.tool, Tool::Text) {
            // Anti-alias applies to the transforms that resample the brush.
        }
    }

    pub fn restore_brush(&mut self) {
        if let Some(b) = self.brush_original.take() {
            self.brush = b;
        }
    }

    /// AnimPainting: go on to the next frame (round to the first) after a stamp.
    pub(crate) fn anim_paint_step(&mut self) {
        if self.v5.x.anim_paint && self.doc.frames.len() > 1 {
            self.doc.next_frame();
            self.doc.checkpoint();
        }
    }

    /// A freehand stroke with Spacing > N Total (not a filled freehand shape).
    pub(crate) fn n_total_stroke(&self) -> bool {
        self.set.spacing_total > 0 && (self.tool == Tool::Dotted || (self.tool == Tool::Freehand && !self.filled))
    }

    /// The colour picker buttons (Range panel, Mixer): on, or off again back to the tool before.
    pub(crate) fn toggle_picker(&mut self) {
        if self.tool == Tool::Eyedropper {
            self.tool = self.v5.x.picker_prev.take().unwrap_or(Tool::Dotted);
        } else {
            self.v5.x.picker_prev = Some(self.tool);
            self.tool = Tool::Eyedropper;
        }
    }

    pub(crate) fn current_brush(&mut self) -> Brush {
        if let Some(ab) = &mut self.anim_brush {
            let mut b = self.brush.clone();
            b.img = ab.cells[ab.pos % ab.cells.len()].clone();
            b
        } else if self.v5.pressure_on && self.v5.pressure_size && !self.brush.custom && self.set.pressure < 1.0 {
            // Pressure > Size: harder presses give a bigger built-in brush (or smaller with Reverse).
            let p = if self.v5.pressure_reverse { 1.0 - self.set.pressure } else { self.set.pressure };
            let extra = (self.v5.pressure_amount as f32 * p).round() as i32;
            Brush::builtin(self.builtin.0, (self.builtin.1 + extra - self.v5.pressure_amount / 2).max(1))
        } else {
            self.brush.clone()
        }
    }

    pub(crate) fn advance_anim_brush(&mut self) {
        if let Some(ab) = &mut self.anim_brush {
            ab.advance();
        }
    }

    // ------------------------------------------------------------ colours

    pub fn display_palette(&self) -> Palette {
        // While choosing colours from a colour set, the screen shows the file's colours.
        if let Some(cs) = &self.v5.x.cset {
            return cs.preview(&self.doc.palette);
        }
        if self.cycling {
            self.doc.cycled_palette(&self.cycle_phases)
        } else {
            self.doc.palette
        }
    }

    pub(crate) fn snap(&self, p: (i32, i32)) -> (i32, i32) {
        if !self.set.grid_on {
            return p;
        }
        let (gw, gh, ox, oy) = self.set.grid;
        let gw = gw.max(1);
        let gh = gh.max(1);
        let sx = ((p.0 - ox) as f32 / gw as f32).round() as i32 * gw + ox;
        let sy = ((p.1 - oy) as f32 / gh as f32).round() as i32 * gh + oy;
        (sx, sy)
    }

    // ------------------------------------------------------------ frame loop

    pub fn tick(&mut self, ctx: &egui::Context) {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.time += dt as f64;
        self.set.multi_cycle = self.v5.prefs.multi_cycle;
        self.v5.x.alt_down = ctx.input(|i| i.modifiers.alt);
        self.v5.x.shift_down = ctx.input(|i| i.modifiers.shift);
        self.v5.x.ctrl_down = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
        self.set.fg_range = self.doc.ranges.iter().position(|r| r.contains(self.fg));
        // The screen format's palette: only its colours may be used, at its precision.
        let n = self.doc.colors.clamp(2, 256);
        crate::amiga::set_active_colors(n);
        if n < 256 || self.doc.ehb || self.doc.gun_bits < 8 {
            self.doc.limit_colors();
        }
        if self.fg as usize >= n {
            self.fg = (n - 1) as u8;
        }
        if self.bg as usize >= n {
            self.bg = 0;
        }
        self.pal_page = self.pal_page.min(n.div_ceil(64).max(1) - 1);
        // Pen pressure from a tablet, when Windows reports it.
        if self.v5.pressure_on {
            let force = ctx.input(|i| {
                i.events.iter().rev().find_map(|e| match e {
                    egui::Event::Touch { force: Some(f), .. } => Some(*f),
                    _ => None,
                })
            });
            if let Some(f) = force {
                self.set.pressure = f.clamp(0.0, 1.0);
            }
        } else {
            self.set.pressure = 1.0;
        }
        if self.cycling {
            for (i, r) in self.doc.ranges.iter().enumerate() {
                if i >= self.cycle_phases.len() {
                    self.cycle_phases.resize(i + 1, 0.0);
                }
                self.cycle_phases[i] += r.steps_per_second() * dt;
            }
            ctx.request_repaint();
        }
        if self.playing {
            self.play_accum += dt;
            // Each frame stays for its own time when Set Frame Rate varies by frame.
            loop {
                let step = self.doc.frame_seconds(self.doc.cur);
                if self.play_accum < step || !self.playing {
                    break;
                }
                self.play_accum -= step;
                self.play_step();
            }
            ctx.request_repaint();
        }
        if let Some(at) = self.camera_at {
            if self.time >= at {
                self.camera_at = None;
                match crate::capture::grab_screen() {
                    Ok(rgba) => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        self.convert = Some(ConvertJob { opts: ConvertOptions { colors: 256, ..Default::default() }, rgba, path: None, as_picture: true });
                    }
                    Err(e) => self.msg("Camera", e),
                }
            } else {
                ctx.request_repaint();
            }
        }
        // Airbrush keeps spraying while held.
        if self.tool == Tool::Airbrush {
            if let (Some(st), Some(c)) = (&mut self.stroke, self.cursor) {
                let b = self.brush.clone();
                paint::airbrush(&mut self.doc, &b, &self.set, st, c.0, c.1);
                ctx.request_repaint();
            }
        }
    }

    pub fn compose_rgba(&self, pal: &Palette) -> egui::ColorImage {
        let img = self.doc.img();
        let (w, h) = (img.w as usize, img.h as usize);
        let mut px: Vec<Color32> = img.px.iter().map(|&p| to_color32(pal[p as usize])).collect();
        // Light table: chosen frames (or the spare page) show through the background colour, dimmed.
        if !self.playing {
            let bgc = self.bg;
            for (layer, dim) in self.light_layers().iter().rev() {
                for i in 0..px.len() {
                    if img.px[i] == bgc {
                        let lp = layer.px.get(i).copied().unwrap_or(bgc);
                        if lp != bgc {
                            let c = pal[lp as usize];
                            let d = |v: u8| (v as i32 - dim).clamp(0, 255) as u8;
                            px[i] = Color32::from_rgb(d(c[0]), d(c[1]), d(c[2]));
                        }
                    }
                }
            }
        }
        // Stencil Show (and Stencil Paint): dim the picture, protected parts lighter.
        if let Some(st) = &self.doc.stencil {
            if self.v5.stencil_show || self.set.stencil_paint {
                for i in 0..px.len() {
                    let c = px[i];
                    let prot = st.mask.get(i).copied().unwrap_or(false);
                    px[i] = if prot {
                        Color32::from_rgb(c.r() / 3 + 150, c.g() / 3 + 150, c.b() / 3 + 150)
                    } else {
                        Color32::from_rgb(c.r() / 3, c.g() / 3, c.b() / 3)
                    };
                }
            }
        }
        // Brush preview under the cursor (as the original showed the brush following the pointer).
        if self.drag.is_none() && self.curve.is_none() && self.text_entry.is_none() && !self.playing {
            if let Some((cx, cy)) = self.cursor {
                let stamping = matches!(self.tool, Tool::Dotted | Tool::Freehand | Tool::Line | Tool::Curve | Tool::Rect | Tool::Circle | Tool::Ellipse | Tool::Polygon);
                if stamping && !self.v5.frame_only && matches!(self.set.mode, Mode::Paint | Mode::SingleColor | Mode::Replace) {
                    let b = &self.brush;
                    let (cx, cy) = self.snap((cx, cy));
                    for by in 0..b.img.h {
                        for bx in 0..b.img.w {
                            if !b.is_solid(bx, by) {
                                continue;
                            }
                            let x = cx - b.hx + bx;
                            let y = cy - b.hy + by;
                            if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                                let c = if b.custom && self.set.mode != Mode::SingleColor { b.img.get(bx, by) } else { self.fg };
                                px[y as usize * w + x as usize] = to_color32(pal[c as usize]);
                            }
                        }
                    }
                }
            }
        }
        egui::ColorImage::new([w, h], px)
    }

    pub fn fit_view(&mut self, avail: Vec2) {
        let z = (avail.x / self.doc.w as f32).min(avail.y / self.doc.h as f32);
        self.zoom = if z >= 1.0 { z.floor().max(1.0) } else { z };
        self.pan = Vec2::ZERO;
    }

    // ------------------------------------------------------------ canvas

    pub fn canvas_ui(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        // The screen format fills the window: keep the zoom percentage when the window changes size.
        let sz = self.screen_zoom(ui.ctx());
        if self.v5.last_screen_zoom > 0.0 && (sz - self.v5.last_screen_zoom).abs() > 0.0001 {
            self.zoom *= sz / self.v5.last_screen_zoom;
            self.pan *= sz / self.v5.last_screen_zoom;
        }
        self.v5.last_screen_zoom = sz;
        ui.painter().rect_filled(full, 0.0, Color32::BLACK);
        let tex = self.texture.as_ref().map(|t| t.id());
        let (main_rect, mag_rect) = if self.magnify {
            let split = full.width() * 0.4;
            (
                Rect::from_min_max(full.min, Pos2::new(full.min.x + split - 2.0, full.max.y)),
                Rect::from_min_max(Pos2::new(full.min.x + split + 2.0, full.min.y), full.max),
            )
        } else {
            (full, Rect::NOTHING)
        };
        self.view_sizes = (main_rect.size(), mag_rect.size());
        let pic = Vec2::new(self.doc.w as f32, self.doc.h as f32);
        if self.magnify {
            // The zoom area must stay clearly smaller than the picture, or it could no longer be moved.
            let min_mag = (mag_rect.width() / (pic.x * 0.8)).max(mag_rect.height() / (pic.y * 0.8)).max(2.0).ceil();
            self.mag_zoom = self.mag_zoom.clamp(min_mag, 24.0f32.max(min_mag));
            // Keep the magnified area inside the picture.
            let half = mag_rect.size() / (2.0 * self.mag_zoom);
            self.mag_center.0 = clamp_center(self.mag_center.0, half.x, pic.x);
            self.mag_center.1 = clamp_center(self.mag_center.1, half.y, pic.y);
            // Scroll the normal view so the magnified area stays in sight.
            let lo = main_rect.center() - pic * self.zoom / 2.0 + self.pan;
            let area = Rect::from_min_max(
                lo + (Vec2::new(self.mag_center.0, self.mag_center.1) - half) * self.zoom,
                lo + (Vec2::new(self.mag_center.0, self.mag_center.1) + half) * self.zoom,
            );
            if area.width() < main_rect.width() {
                if area.min.x < main_rect.min.x {
                    self.pan.x += main_rect.min.x - area.min.x;
                } else if area.max.x > main_rect.max.x {
                    self.pan.x -= area.max.x - main_rect.max.x;
                }
            }
            if area.height() < main_rect.height() {
                if area.min.y < main_rect.min.y {
                    self.pan.y += main_rect.min.y - area.min.y;
                } else if area.max.y > main_rect.max.y {
                    self.pan.y -= area.max.y - main_rect.max.y;
                }
            }
        }
        // Never scroll past the picture's edges when it is bigger than the view.
        let img = pic * self.zoom;
        let lim = |img: f32, view: f32, v: f32| if img > view { v.clamp(-(img - view) / 2.0, (img - view) / 2.0) } else { v.clamp(-(view - img) / 2.0, (view - img) / 2.0) };
        self.pan.x = lim(img.x, main_rect.width(), self.pan.x);
        self.pan.y = lim(img.y, main_rect.height(), self.pan.y);
        if self.fit_pending {
            // 100%: one picture pixel per screen pixel, the picture's top-left corner at the top-left
            // of the page, as the original showed it.
            self.zoom = sz;
            let img = pic * self.zoom;
            self.pan = Vec2::new(
                if img.x > main_rect.width() { (img.x - main_rect.width()) / 2.0 } else { 0.0 },
                if img.y > main_rect.height() { (img.y - main_rect.height()) / 2.0 } else { 0.0 },
            );
            self.fit_pending = false;
        }
        let lo = main_rect.center() - pic * self.zoom / 2.0 + self.pan;
        let panes: Vec<(Rect, f32, Pos2)> = if self.magnify {
            let ro = mag_rect.center() - Vec2::new(self.mag_center.0, self.mag_center.1) * self.mag_zoom;
            vec![(main_rect, self.zoom, lo), (mag_rect, self.mag_zoom, ro)]
        } else {
            vec![(main_rect, self.zoom, lo)]
        };
        let mut hovered_any = false;
        for (pi, (rect, zoom, origin)) in panes.iter().enumerate() {
            let resp = ui.interact(*rect, ui.scope_id().with(("canvas", pi)), Sense::click_and_drag());
            let painter = ui.painter_at(*rect);
            let img_rect = Rect::from_min_size(*origin, Vec2::new(self.doc.w as f32, self.doc.h as f32) * *zoom);
            painter.rect_filled(img_rect.expand(1.0), 0.0, Color32::from_rgb(10, 10, 12));
            if let Some(t) = tex {
                painter.image(t, img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            self.draw_overlays(&painter, *origin, *zoom, img_rect);
            if pi == 1 {
                painter.rect_stroke(*rect, 0.0, egui::Stroke::new(2.0, crate::ui_style::GREY), egui::StrokeKind::Inside);
            } else if self.magnify {
                // Frame on the normal view showing which part is magnified.
                let half = mag_rect.size() / (2.0 * self.mag_zoom);
                let c = Vec2::new(self.mag_center.0, self.mag_center.1);
                let r = Rect::from_min_max(*origin + (c - half) * *zoom, *origin + (c + half) * *zoom).intersect(img_rect);
                painter.rect_stroke(r, 0.0, egui::Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Outside);
                painter.rect_stroke(r.expand(1.0), 0.0, egui::Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Outside);
            }
            if let Some(pos) = resp.hover_pos().or_else(|| if resp.dragged() { resp.interact_pointer_pos() } else { None }) {
                let cp = ((pos - *origin) / *zoom).floor();
                // With the zoom on, the unzoomed view only steers the zoom area; nothing is painted there.
                let navigate_only = self.magnify && pi == 0;
                if !navigate_only || self.drag.is_none() && self.stroke.is_none() && self.curve.is_none() {
                    if navigate_only {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                    } else {
                        self.cursor = Some((cp.x as i32, cp.y as i32));
                        hovered_any = true;
                        if self.v5.x.crosshair {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                        }
                    }
                }
                // Wheel zoom: one step per wheel notch (smoothed scrolling would repeat it every frame).
                let notches = wheel_notches(ui);
                if notches != 0.0 && !self.view_page {
                    self.v5.wheel_acc += notches;
                    let steps = self.v5.wheel_acc.trunc() as i32;
                    self.v5.wheel_acc -= steps as f32;
                    if steps != 0 {
                        if self.magnify {
                            // With the zoom tool on, the wheel changes the magnification only, so the
                            // normal view and the zoom area stay where they are.
                            self.mag_zoom = (self.mag_zoom + steps as f32).clamp(2.0, 24.0);
                        } else {
                            let old = self.zoom;
                            let nz = (old * 1.25f32.powi(steps)).clamp(0.1, 32.0);
                            // Keep the point under the mouse in place.
                            let before = (pos - *origin) / old;
                            let new_origin = pos - before * nz;
                            let base = rect.center() - Vec2::new(self.doc.w as f32, self.doc.h as f32) * nz / 2.0;
                            self.pan = new_origin - base;
                            self.zoom = nz;
                        }
                    }
                }
            }
            // Middle button pans.
            if resp.dragged_by(egui::PointerButton::Middle) {
                let d = resp.drag_delta();
                if pi == 0 {
                    self.pan += d;
                } else {
                    self.mag_center.0 -= d.x / self.mag_zoom;
                    self.mag_center.1 -= d.y / self.mag_zoom;
                }
            }
            if self.magnify && pi == 0 {
                // Clicking (or holding) in the unzoomed view moves the zoom area to the mouse.
                let busy = self.drag.is_some() || self.stroke.is_some() || self.curve.is_some() || self.text_entry.is_some();
                let held = ui.input(|i| i.pointer.primary_down() || i.pointer.secondary_down());
                if !busy && held && resp.is_pointer_button_down_on() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let cp = (pos - *origin) / *zoom;
                        self.mag_center = (cp.x, cp.y);
                    }
                }
                continue;
            }
            self.handle_canvas_input(ui, &resp, pi);
        }
        if !hovered_any && self.drag.is_none() && self.stroke.is_none() {
            self.cursor = None;
        }
    }

    pub(crate) fn draw_overlays(&self, painter: &egui::Painter, origin: Pos2, zoom: f32, img_rect: Rect) {
        let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y) * zoom;
        // Pixel grid when zoomed far in.
        if zoom >= 8.0 {
            let c = Color32::from_rgba_unmultiplied(0, 0, 0, 60);
            let clip = painter.clip_rect().intersect(img_rect);
            let x0 = ((clip.min.x - origin.x) / zoom).floor().max(0.0) as i32;
            let x1 = ((clip.max.x - origin.x) / zoom).ceil().min(self.doc.w as f32) as i32;
            let y0 = ((clip.min.y - origin.y) / zoom).floor().max(0.0) as i32;
            let y1 = ((clip.max.y - origin.y) / zoom).ceil().min(self.doc.h as f32) as i32;
            for x in x0..=x1 {
                painter.line_segment([to_screen(x as f32, y0 as f32), to_screen(x as f32, y1 as f32)], (1.0, c));
            }
            for y in y0..=y1 {
                painter.line_segment([to_screen(x0 as f32, y as f32), to_screen(x1 as f32, y as f32)], (1.0, c));
            }
        }
        // Snap grid.
        if self.set.grid_on {
            let (gw, gh, ox, oy) = self.set.grid;
            if gw as f32 * zoom >= 4.0 && gh as f32 * zoom >= 4.0 {
                let c = Color32::from_rgba_unmultiplied(120, 180, 255, 70);
                let mut x = ox.rem_euclid(gw.max(1));
                while x <= self.doc.w {
                    painter.line_segment([to_screen(x as f32, 0.0), to_screen(x as f32, self.doc.h as f32)], (1.0, c));
                    x += gw.max(1);
                }
                let mut y = oy.rem_euclid(gh.max(1));
                while y <= self.doc.h {
                    painter.line_segment([to_screen(0.0, y as f32), to_screen(self.doc.w as f32, y as f32)], (1.0, c));
                    y += gh.max(1);
                }
            }
        }
        // Symmetry centre.
        if self.set.sym.on && self.set.sym.kind == paint::SymKind::Cyclic {
            let (cx, cy) = self.set.sym.center.unwrap_or((self.doc.w / 2, self.doc.h / 2));
            let p = to_screen(cx as f32 + 0.5, cy as f32 + 0.5);
            let c = Color32::from_rgb(255, 200, 60);
            painter.line_segment([p - Vec2::new(8.0, 0.0), p + Vec2::new(8.0, 0.0)], (1.5, c));
            painter.line_segment([p - Vec2::new(0.0, 8.0), p + Vec2::new(0.0, 8.0)], (1.5, c));
        }
        // Rubber bands.
        let band = egui::Stroke::new(1.0, Color32::from_rgb(255, 255, 255));
        if let Some(d) = &self.drag {
            if matches!(self.tool, Tool::BrushPick) {
                if let Some(c) = self.cursor {
                    let r = Rect::from_two_pos(to_screen(d.start.0 as f32, d.start.1 as f32), to_screen(c.0 as f32 + 1.0, c.1 as f32 + 1.0));
                    painter.rect_stroke(r, 0.0, band, egui::StrokeKind::Outside);
                }
            }
            // N Total freehand: the path so far, as a thin line (the dots come on release).
            if self.n_total_stroke() && d.points.len() > 1 {
                let pts: Vec<Pos2> = d.points.iter().map(|p| to_screen(p.0 as f32 + 0.5, p.1 as f32 + 0.5)).collect();
                painter.add(egui::Shape::line(pts, egui::Stroke::new(1.0, Color32::from_rgb(255, 255, 120))));
            }
            if matches!(self.tool, Tool::Lasso) && d.points.len() > 1 {
                let pts: Vec<Pos2> = d.points.iter().map(|p| to_screen(p.0 as f32 + 0.5, p.1 as f32 + 0.5)).collect();
                painter.add(egui::Shape::line(pts, band));
            }
            if matches!(self.tool, Tool::Polygon) && !d.points.is_empty() {
                let mut pts: Vec<Pos2> = d.points.iter().map(|p| to_screen(p.0 as f32 + 0.5, p.1 as f32 + 0.5)).collect();
                if let Some(c) = self.cursor {
                    pts.push(to_screen(c.0 as f32 + 0.5, c.1 as f32 + 0.5));
                }
                painter.add(egui::Shape::line(pts, band));
            }
        }
        if let Some(te) = &self.text_entry {
            let p = to_screen(te.pos.0 as f32, te.pos.1 as f32);
            let blink = (self.time * 2.0) as i64 % 2 == 0;
            if blink {
                painter.line_segment([p, p + Vec2::new(0.0, self.text_style.size as f32 * zoom)], (2.0, Color32::from_rgb(255, 255, 0)));
            }
        }
        // Direction line of a range fill.
        if let Some(gl) = &self.v5.grad_line {
            let a = gl.centre;
            let pa = to_screen(a.0 as f32 + 0.5, a.1 as f32 + 0.5);
            let pb = to_screen(gl.end.0 as f32 + 0.5, gl.end.1 as f32 + 0.5);
            painter.line_segment([pa, pb], egui::Stroke::new(3.0, Color32::BLACK));
            painter.line_segment([pa, pb], egui::Stroke::new(1.0, Color32::WHITE));
            painter.circle_filled(pa, 3.5, Color32::WHITE);
            painter.circle_stroke(pa, 3.5, egui::Stroke::new(1.0, Color32::BLACK));
            // Cross-hair at the moving end, as in the original.
            let c = Color32::from_rgb(255, 80, 80);
            painter.line_segment([pb - Vec2::new(8.0, 0.0), pb + Vec2::new(8.0, 0.0)], egui::Stroke::new(1.0, c));
            painter.line_segment([pb - Vec2::new(0.0, 8.0), pb + Vec2::new(0.0, 8.0)], egui::Stroke::new(1.0, c));
        }
        // Perspective mode: the brush card as a frame with its own cross-hair (its X and Y axes),
        // and a small cross-hair at the perspective centre.
        if self.v5.x.persp_mode {
            let (cx, cy) = self.persp_centre_f();
            let pc = to_screen(cx + 0.5, cy + 0.5);
            let c = Color32::from_rgb(255, 255, 255);
            painter.line_segment([pc - Vec2::new(5.0, 0.0), pc + Vec2::new(5.0, 0.0)], egui::Stroke::new(1.0, c));
            painter.line_segment([pc - Vec2::new(0.0, 5.0), pc + Vec2::new(0.0, 5.0)], egui::Stroke::new(1.0, c));
            if let (Some(base), Some(h)) = (&self.v5.x.persp_base, self.persp_handle()) {
                let hm = crate::persp3d::homography(&self.v5.x.p3.rot, h, (cx, cy), self.persp_eye());
                if let Some(cs) = crate::persp3d::corners(base, &hm) {
                    let pts: Vec<Pos2> = cs.iter().map(|p| to_screen(p.0, p.1)).collect();
                    let frame = egui::Stroke::new(1.0, Color32::WHITE);
                    let shadow = egui::Stroke::new(3.0, Color32::from_rgba_unmultiplied(0, 0, 0, 160));
                    for k in 0..4 {
                        painter.line_segment([pts[k], pts[(k + 1) % 4]], shadow);
                        painter.line_segment([pts[k], pts[(k + 1) % 4]], frame);
                    }
                    let (w, hh) = (base.img.w as f32, base.img.h as f32);
                    let (hx, hy) = (base.hx as f32, base.hy as f32);
                    for (a, b) in [((-hx, 0.0), (w - hx, 0.0)), ((0.0, -hy), (0.0, hh - hy))] {
                        if let (Some(pa), Some(pb)) = (crate::persp3d::apply(&hm, a.0, a.1), crate::persp3d::apply(&hm, b.0, b.1)) {
                            painter.line_segment([to_screen(pa.0, pa.1), to_screen(pb.0, pb.1)], egui::Stroke::new(1.0, Color32::from_rgb(255, 255, 120)));
                        }
                    }
                }
            }
        }
        // Move > Preview: the brush as a wire frame running through the move.
        self.draw_move_preview(painter, to_screen);
        // Brush > Frame Only: show the brush as a frame instead of its pixels.
        if self.v5.frame_only && self.drag.is_none() {
            if let Some(c) = self.cursor {
                let b = &self.brush;
                let r = Rect::from_min_size(to_screen((c.0 - b.hx) as f32, (c.1 - b.hy) as f32), Vec2::new(b.img.w as f32, b.img.h as f32) * zoom);
                painter.rect_stroke(r, 0.0, egui::Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Outside);
            }
        }
        if self.brush_handle && self.brush.custom {
            if let Some(c) = self.cursor {
                let p = to_screen(c.0 as f32 + 0.5, c.1 as f32 + 0.5);
                painter.circle_stroke(p, 3.0, (1.0, Color32::from_rgb(255, 0, 255)));
            }
        }
    }

    pub(crate) fn handle_canvas_input(&mut self, ui: &egui::Ui, resp: &egui::Response, pane: usize) {
        if self.playing || self.show.is_some() || !self.v5.replay.is_empty() {
            return;
        }
        let Some(cur) = self.cursor else { return };
        let (pressed_p, pressed_s, down_p, down_s, released, dbl) = ui.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.button_pressed(egui::PointerButton::Secondary),
                i.pointer.button_down(egui::PointerButton::Primary),
                i.pointer.button_down(egui::PointerButton::Secondary),
                i.pointer.any_released(),
                i.pointer.button_double_clicked(egui::PointerButton::Primary),
            )
        });
        let hovered = resp.hovered() || resp.dragged();
        let _ = pane;
        let p = crate::macros::PtrIn { pos: cur, hovered, pressed_p, pressed_s, down_p, down_s, released, dbl };
        // Stencil > Show: a click returns to the picture.
        if self.v5.stencil_show {
            if hovered && (pressed_p || pressed_s) {
                self.v5.stencil_show = false;
            }
            return;
        }
        let active = pressed_p || pressed_s || released || ((down_p || down_s) && self.drag.is_some());
        if self.v5.recording.is_some() && (hovered || self.drag.is_some()) && active {
            let detailed = self.v5.record_details
                || pressed_p
                || pressed_s
                || released
                || matches!(self.tool, Tool::Dotted | Tool::Freehand | Tool::Airbrush | Tool::Lasso);
            if detailed {
                self.record(crate::macros::MacroEv::Ptr(p));
            }
        }
        self.pointer_logic(&p);
    }

    /// What a pointer sample does on the picture (shared by the mouse and macro playback).
    pub(crate) fn pointer_logic(&mut self, p: &crate::macros::PtrIn) {
        let crate::macros::PtrIn { pos: cur, hovered, pressed_p, pressed_s, down_p, down_s, released, dbl } = *p;
        let pos = self.snap(cur);
        // Coordinates measured from the press point while a button is held.
        if pressed_p || pressed_s {
            self.v5.coord_origin = Some(pos);
        } else if released && !(down_p || down_s) {
            self.v5.coord_origin = None;
        }
        // A range fill waiting for its direction line takes the pointer.
        if self.v5.grad_line.is_some() {
            self.grad_line_pointer(pos, pressed_p || pressed_s, down_p || down_s, released, hovered);
            return;
        }
        // Placing the perspective centre.
        // Grid > Adjust and Airbrush > Size work by dragging on the page.
        if hovered || released {
            let pr = hovered && (pressed_p || pressed_s);
            let rel = released && !(down_p || down_s);
            if self.grid_adjust_pointer(pos, pr, rel) || self.air_size_pointer(pos, pr, rel) {
                return;
            }
        }
        if self.v5.persp_place && hovered && (pressed_p || pressed_s) {
            self.v5.persp_centre = Some(pos);
            self.v5.persp_place = false;
            self.status = format!("Perspective centre at {}, {}", pos.0, pos.1);
            return;
        }
        // Rubthru reads the spare page.
        if (pressed_p || pressed_s) && (self.set.mode == crate::paint::Mode::Rubthru || self.set.fill == crate::paint::FillKind::Rub) {
            self.set.rub = Some(std::sync::Arc::new((self.spare.img().clone(), self.spare.palette)));
        }

        // Placing the symmetry centre.
        if self.sym_place && hovered && (pressed_p || pressed_s) {
            self.set.sym.center = Some(pos);
            self.sym_place = false;
            self.show_symmetry = true;
            return;
        }

        // Curve: second stage, bend with the mouse and click to finish.
        if let Some(cw) = &self.curve {
            let base = cw.base.clone();
            let (a, b, primary) = (cw.a, cw.b, cw.primary);
            *self.doc.img_mut() = base;
            let mut pts = Vec::new();
            pix::curve_points(a, pos, b, |x, y| pts.push((x, y)));
            let color = if primary { self.fg } else { self.bg };
            let mut st = Stroke::new(color, primary);
            let br = self.brush.clone();
            paint::stamp_points(&mut self.doc, &br, &self.set, &mut st, &pts);
            if hovered && (pressed_p || pressed_s) {
                if let Some(cw) = self.curve.take() {
                    self.antialias_since(&cw.base);
                }
            }
            return;
        }

        // Text entry: a click elsewhere commits the text.
        if self.text_entry.is_some() && hovered && (pressed_p || pressed_s) {
            self.commit_text();
            if self.tool != Tool::Text {
                return;
            }
        }

        // Polygon: clicks add corners; double-click or right-click closes.
        if self.tool == Tool::Polygon {
            if hovered && pressed_p {
                match &mut self.drag {
                    None => {
                        self.doc.checkpoint();
                        let base = self.doc.img().clone();
                        self.drag = Some(Drag { start: pos, primary: true, base, points: vec![pos], trails_base: None });
                    }
                    Some(d) => {
                        let first = d.points[0];
                        let close = d.points.len() > 2 && (pos.0 - first.0).abs() <= 2 && (pos.1 - first.1).abs() <= 2;
                        if close || dbl {
                            self.finish_polygon();
                        } else {
                            d.points.push(pos);
                        }
                    }
                }
            } else if hovered && pressed_s && self.drag.is_some() {
                self.finish_polygon();
            }
            return;
        }

        let pressed = hovered && (pressed_p || pressed_s);
        if pressed && self.drag.is_none() {
            let primary = pressed_p;
            let color = if primary { self.fg } else { self.bg };
            match self.tool {
                Tool::Eyedropper => {
                    let c = self.doc.img().get(pos.0, pos.1);
                    if primary {
                        self.fg = c
                    } else {
                        self.bg = c
                    }
                    return;
                }
                Tool::Fill => {
                    if self.v5.x.anim_fill && self.doc.frames.len() > 1 && !self.needs_grad_line() {
                        self.anim_fill(pos, color);
                        return;
                    }
                    self.doc.checkpoint();
                    let img = self.doc.img().clone();
                    match self.fill_region(&img, pos.0, pos.1) {
                        Some(mask) if self.needs_grad_line() => {
                            // Range fill: the direction line starts here and follows the mouse.
                            let base = self.doc.img().clone();
                            self.begin_grad_line(mask, base, color, true, pos);
                        }
                        Some(mask) => {
                            let br = self.brush.clone();
                            let mut rng = self.rng.clone();
                            paint::fill_mask(&mut self.doc, &mask, &self.set, color, &br, &mut rng);
                            self.rng = rng;
                        }
                        None => self.msg("Fill", "Area is too complex to fill."),
                    }
                    return;
                }
                Tool::Text => {
                    if self.text_entry.is_none() {
                        self.doc.checkpoint();
                        let base = self.doc.img().clone();
                        self.text_entry = Some(TextEntry { pos, buffer: String::new(), base });
                    }
                    return;
                }
                _ => {}
            }
            self.doc.checkpoint();
            let base = self.doc.img().clone();
            self.drag = Some(Drag { start: pos, primary, base, points: vec![pos], trails_base: None });
            let mut st = Stroke::new(color, primary);
            st.rng = Rng::new(self.rng.next());
            self.v5.x.constrain_axis = None;
            self.v5.x.trails_used = false;
            self.v5.x.trails_last = None;
            // AnimPainting: with Alt held when the button goes down, each stamp lands on the next frame.
            self.v5.x.anim_paint = self.v5.x.alt_down && self.doc.frames.len() > 1 && matches!(self.tool, Tool::Dotted | Tool::Freehand);
            // N Total: the whole stroke is needed first; the dots are put down on release.
            let n_total = self.n_total_stroke();
            if matches!(self.tool, Tool::Dotted | Tool::Freehand) && !n_total {
                let b = self.current_brush();
                paint::stamp_points(&mut self.doc, &b, &self.set, &mut st, &[pos]);
                self.advance_anim_brush();
                // Moves (Anim > Move) start where the brush was last stamped.
                self.move_note_stamp(pos);
                self.anim_paint_step();
            }
            if self.tool == Tool::Airbrush {
                let b = self.brush.clone();
                paint::airbrush(&mut self.doc, &b, &self.set, &mut st, pos.0, pos.1);
            }
            self.stroke = Some(st);
            return;
        }

        let still_down = down_p || down_s;
        if self.drag.is_some() && still_down {
            self.drag_update(pos);
        }
        if self.drag.is_some() && (released && !still_down) {
            self.drag_finish(pos);
        }
    }

    pub(crate) fn drag_update(&mut self, pos: (i32, i32)) {
        let tool = self.tool;
        // Shift: dotted, freehand, line, curve and airbrush keep to the horizontal or vertical,
        // whichever way the pointer first moved.
        let pos = match (&self.drag, self.v5.x.shift_down) {
            (Some(d), true) if matches!(tool, Tool::Dotted | Tool::Freehand | Tool::Line | Tool::Curve | Tool::Airbrush) => {
                let (dx, dy) = (pos.0 - d.start.0, pos.1 - d.start.1);
                if self.v5.x.constrain_axis.is_none() && (dx.abs() > 2 || dy.abs() > 2) {
                    self.v5.x.constrain_axis = Some(if dx.abs() >= dy.abs() { 0 } else { 1 });
                }
                match self.v5.x.constrain_axis {
                    Some(0) => (pos.0, d.start.1),
                    Some(_) => (d.start.0, pos.1),
                    None => d.start,
                }
            }
            _ => pos,
        };
        let filled = self.filled;
        let Some(d) = &mut self.drag else { return };
        let start = d.start;
        let primary = d.primary;
        match tool {
            Tool::Dotted if self.n_total_stroke() => {
                if let Some(d) = &mut self.drag {
                    if d.points.last() != Some(&pos) {
                        d.points.push(pos);
                    }
                }
            }
            Tool::Dotted => {
                let b = self.current_brush();
                if let Some(st) = &mut self.stroke {
                    if st.last != Some(pos) {
                        // Dotted freehand follows Spacing too (every N pixels travelled, airbrush
                        // bursts); the last position stays known, so Smear and Blend drag the colours
                        // along the way the brush moved.
                        if paint::stamp_points(&mut self.doc, &b, &self.set, st, &[pos]) > 0 {
                            self.advance_anim_brush();
                            self.anim_paint_step();
                        }
                    }
                }
            }
            Tool::Freehand => {
                // MouseTrack: smooth the line by averaging the mouse movement.
                let pos = if self.v5.prefs.mouse_track {
                    let t = self.v5.track.get_or_insert((pos.0 as f32, pos.1 as f32));
                    t.0 += (pos.0 as f32 - t.0) * 0.35;
                    t.1 += (pos.1 as f32 - t.1) * 0.35;
                    (t.0.round() as i32, t.1.round() as i32)
                } else {
                    pos
                };
                if let Some(d) = &mut self.drag {
                    if d.points.last() != Some(&pos) {
                        d.points.push(pos);
                    }
                }
                let b = self.current_brush();
                if self.n_total_stroke() {
                    // N Total: only the path is recorded now (shown as a thin line).
                    return;
                }
                if let Some(st) = &mut self.stroke {
                    let last = st.last.unwrap_or(pos);
                    if last != pos {
                        paint::stroke_line(&mut self.doc, &b, &self.set, st, last, pos);
                        if self.anim_brush.is_some() {
                            self.advance_anim_brush();
                        }
                        self.anim_paint_step();
                    }
                }
            }
            Tool::Airbrush => {}
            Tool::Lasso => {
                if let Some(d) = &mut self.drag {
                    if d.points.last() != Some(&pos) {
                        d.points.push(pos);
                    }
                }
            }
            Tool::BrushPick => {}
            // Ctrl held: trails - every line (or shape) drawn while moving stays, e.g. a ray of lines
            // from the start point (manual p. 193 / 201).
            Tool::Line | Tool::Rect | Tool::Circle | Tool::Ellipse | Tool::Curve if self.v5.x.ctrl_down => {
                if !self.v5.x.trails_used {
                    // The preview drawn before Ctrl went down goes.
                    let base = self.drag.as_ref().unwrap().base.clone();
                    *self.doc.img_mut() = base;
                    self.v5.x.trails_used = true;
                }
                if self.v5.x.trails_last != Some(pos) {
                    let t = if tool == Tool::Curve { Tool::Line } else { tool };
                    self.draw_shape(t, start, pos, primary, filled && tool != Tool::Curve);
                    self.v5.x.trails_last = Some(pos);
                }
                // What is drawn now is kept: later moves draw on top of it.
                let now = self.doc.img().clone();
                if let Some(d) = &mut self.drag {
                    d.trails_base = Some(now);
                }
            }
            Tool::Line | Tool::Rect | Tool::Circle | Tool::Ellipse => {
                let d = self.drag.as_ref().unwrap();
                let base = d.trails_base.clone().unwrap_or_else(|| d.base.clone());
                *self.doc.img_mut() = base;
                self.draw_shape_fb(tool, start, pos, primary, filled);
            }
            Tool::Curve => {
                let d = self.drag.as_ref().unwrap();
                let base = d.trails_base.clone().unwrap_or_else(|| d.base.clone());
                *self.doc.img_mut() = base;
                self.draw_shape_fb(Tool::Line, start, pos, primary, false);
            }
            _ => {}
        }
    }

    /// Draw a shape while dragging; with Fast FB, outlines are previewed with a one-pixel brush.
    pub(crate) fn draw_shape_fb(&mut self, tool: Tool, a: (i32, i32), b: (i32, i32), primary: bool, filled: bool) {
        if self.v5.prefs.fast_fb && !filled {
            let real = std::mem::replace(&mut self.brush, Brush::builtin(BuiltinShape::Round, 1));
            self.draw_shape(tool, a, b, primary, filled);
            self.brush = real;
        } else {
            self.draw_shape(tool, a, b, primary, filled);
        }
    }

    /// The pixels a filled rectangle, circle or ellipse covers.
    pub(crate) fn shape_mask(&self, tool: Tool, a: (i32, i32), b: (i32, i32)) -> Vec<bool> {
        let b = if self.v5.prefs.be_square && matches!(tool, Tool::Rect | Tool::Ellipse) {
            let d = (b.0 - a.0).abs().max((b.1 - a.1).abs());
            (a.0 + d * if b.0 >= a.0 { 1 } else { -1 }, a.1 + d * if b.1 >= a.1 { 1 } else { -1 })
        } else {
            b
        };
        let (w, h) = (self.doc.w, self.doc.h);
        match tool {
            Tool::Rect => {
                let spans: Vec<(i32, i32, i32)> = (a.1.min(b.1)..=a.1.max(b.1)).map(|y| (y, a.0.min(b.0), a.0.max(b.0))).collect();
                paint::spans_mask(&spans, w, h)
            }
            Tool::Circle => {
                let r = ((((b.0 - a.0).pow(2) + (b.1 - a.1).pow(2)) as f32).sqrt()).round() as i32;
                paint::spans_mask(&pix::ellipse_spans(a.0, a.1, r, r), w, h)
            }
            _ => paint::spans_mask(&pix::ellipse_spans(a.0, a.1, (b.0 - a.0).abs(), (b.1 - a.1).abs()), w, h),
        }
    }

    pub(crate) fn draw_shape(&mut self, tool: Tool, a: (i32, i32), b: (i32, i32), primary: bool, filled: bool) {
        // Be Square: rectangles become squares and ellipses circles.
        let b = if self.v5.prefs.be_square && matches!(tool, Tool::Rect | Tool::Ellipse) {
            let d = (b.0 - a.0).abs().max((b.1 - a.1).abs());
            let sx = if b.0 >= a.0 { 1 } else { -1 };
            let sy = if b.1 >= a.1 { 1 } else { -1 };
            (a.0 + d * sx, a.1 + d * sy)
        } else {
            b
        };
        let color = if primary { self.fg } else { self.bg };
        let br = self.brush.clone();
        let mut st = Stroke::new(color, primary);
        let (w, h) = (self.doc.w, self.doc.h);
        match tool {
            Tool::Line => paint::stroke_line(&mut self.doc, &br, &self.set, &mut st, a, b),
            Tool::Rect => {
                if filled {
                    let spans: Vec<(i32, i32, i32)> = (a.1.min(b.1)..=a.1.max(b.1)).map(|y| (y, a.0.min(b.0), a.0.max(b.0))).collect();
                    let m = paint::spans_mask(&spans, w, h);
                    let mut rng = Rng::new(7);
                    paint::fill_mask(&mut self.doc, &m, &self.set, color, &br, &mut rng);
                } else {
                    let pts = [(a.0, a.1), (b.0, a.1), (b.0, b.1), (a.0, b.1), (a.0, a.1)];
                    let mut all = Vec::new();
                    for i in 0..4 {
                        pix::line_points(pts[i].0, pts[i].1, pts[i + 1].0, pts[i + 1].1, |x, y| all.push((x, y)));
                    }
                    all.dedup();
                    paint::stamp_points(&mut self.doc, &br, &self.set, &mut st, &all);
                }
            }
            Tool::Circle | Tool::Ellipse => {
                let (rx, ry) = if tool == Tool::Circle {
                    let r = ((((b.0 - a.0).pow(2) + (b.1 - a.1).pow(2)) as f32).sqrt()).round() as i32;
                    (r, r)
                } else {
                    ((b.0 - a.0).abs(), (b.1 - a.1).abs())
                };
                if filled {
                    let m = paint::spans_mask(&pix::ellipse_spans(a.0, a.1, rx, ry), w, h);
                    let mut rng = Rng::new(7);
                    paint::fill_mask(&mut self.doc, &m, &self.set, color, &br, &mut rng);
                } else {
                    let mut pts = Vec::new();
                    pix::ellipse_outline(a.0, a.1, rx, ry, |x, y| pts.push((x, y)));
                    paint::stamp_points(&mut self.doc, &br, &self.set, &mut st, &pts);
                }
            }
            _ => {}
        }
    }

    /// The outline of a shape as points along its edge (Be Square applied like the shape itself).
    pub(crate) fn shape_outline_points(&self, tool: Tool, a: (i32, i32), b: (i32, i32), points: &[(i32, i32)]) -> Vec<(i32, i32)> {
        let b = if self.v5.prefs.be_square && matches!(tool, Tool::Rect | Tool::Ellipse) {
            let d = (b.0 - a.0).abs().max((b.1 - a.1).abs());
            (a.0 + d * if b.0 >= a.0 { 1 } else { -1 }, a.1 + d * if b.1 >= a.1 { 1 } else { -1 })
        } else {
            b
        };
        let mut all = Vec::new();
        match tool {
            Tool::Rect => {
                let pts = [(a.0, a.1), (b.0, a.1), (b.0, b.1), (a.0, b.1), (a.0, a.1)];
                for i in 0..4 {
                    pix::line_points(pts[i].0, pts[i].1, pts[i + 1].0, pts[i + 1].1, |x, y| all.push((x, y)));
                }
            }
            Tool::Circle | Tool::Ellipse => {
                let (rx, ry) = if tool == Tool::Circle {
                    let r = ((((b.0 - a.0).pow(2) + (b.1 - a.1).pow(2)) as f32).sqrt()).round() as i32;
                    (r, r)
                } else {
                    ((b.0 - a.0).abs(), (b.1 - a.1).abs())
                };
                pix::ellipse_outline(a.0, a.1, rx, ry, |x, y| all.push((x, y)));
            }
            _ => {
                // Freehand and polygon: the drawn points, closed.
                for i in 0..points.len() {
                    let (p, q) = (points[i], points[(i + 1) % points.len()]);
                    pix::line_points(p.0, p.1, q.0, q.1, |x, y| all.push((x, y)));
                }
            }
        }
        all.dedup();
        all
    }

    /// Trace an outline with the current brush in the current mode, following Spacing.
    pub(crate) fn trace_outline(&mut self, pts: &[(i32, i32)], primary: bool) {
        if pts.is_empty() {
            return;
        }
        let color = if primary { self.fg } else { self.bg };
        let mut st = Stroke::new(color, primary);
        st.rng = Rng::new(self.rng.next());
        let b = self.current_brush();
        paint::stamp_points(&mut self.doc, &b, &self.set, &mut st, pts);
    }

    /// Filled shape with outline: trace its edge now, or after the range fill when one is waiting.
    fn outline_filled_shape(&mut self, tool: Tool, a: (i32, i32), b: (i32, i32), points: &[(i32, i32)], primary: bool) {
        if !(self.v5.x.fill_outline && self.filled) {
            return;
        }
        let pts = self.shape_outline_points(tool, a, b, points);
        if self.v5.grad_line.is_some() {
            self.v5.x.outline_pending = Some((pts, primary));
        } else {
            self.trace_outline(&pts, primary);
        }
    }

    pub(crate) fn drag_finish(&mut self, pos: (i32, i32)) {
        let Some(d) = self.drag.take() else { return };
        // Shade only changes colours of the foreground colour's range: say so when nothing changed.
        if self.set.mode == crate::paint::Mode::Shade && *self.doc.img() == d.base {
            self.status = match self.set.fg_range {
                Some(i) => {
                    let r = &self.doc.ranges[i];
                    format!("Shade changes only colours {}–{} (the foreground colour's range); nothing under the brush was in it", r.low, r.high)
                }
                None => "Shade: the foreground colour is in no range; pick a colour from a range (Colour > Ranges) first".into(),
            };
        }
        let tool = self.tool;
        self.v5.track = None;
        // N Total with freehand: now that the whole stroke is known, N dots spread evenly over it.
        if self.n_total_stroke() && matches!(tool, Tool::Dotted | Tool::Freehand) {
            let mut all: Vec<(i32, i32)> = Vec::new();
            for w in d.points.windows(2) {
                pix::line_points(w[0].0, w[0].1, w[1].0, w[1].1, |x, y| all.push((x, y)));
            }
            if all.is_empty() {
                all.push(d.start);
            }
            all.dedup();
            let color = if d.primary { self.fg } else { self.bg };
            let mut st = Stroke::new(color, d.primary);
            let b = self.current_brush();
            paint::stamp_points(&mut self.doc, &b, &self.set, &mut st, &all);
            self.move_note_stamp(*all.last().unwrap());
        }
        match tool {
            Tool::Line | Tool::Rect | Tool::Circle | Tool::Ellipse if self.v5.prefs.fast_fb && !self.filled => {
                // Fast FB: the final shape is drawn with the real brush (on top of any Ctrl trails).
                *self.doc.img_mut() = d.trails_base.clone().unwrap_or_else(|| d.base.clone());
                let (filled, primary) = (self.filled, d.primary);
                self.draw_shape(tool, d.start, pos, primary, filled);
            }
            // Curve with Ctrl trails: the straight lines stay; no bending stage.
            Tool::Curve if d.trails_base.is_some() => {}
            Tool::Curve => {
                *self.doc.img_mut() = d.base.clone();
                self.curve = Some(CurveWait { a: d.start, b: pos, primary: d.primary, base: d.base.clone() });
            }
            Tool::Freehand if self.filled => {
                // Filled freehand: close the drawn outline and fill it.
                *self.doc.img_mut() = d.base.clone();
                let color = if d.primary { self.fg } else { self.bg };
                let m = paint::spans_mask(&pix::polygon_spans(&d.points), self.doc.w, self.doc.h);
                if self.needs_grad_line() {
                    self.begin_grad_line(m, d.base.clone(), color, false, pos);
                } else {
                    let br = self.brush.clone();
                    let mut rng = Rng::new(9);
                    paint::fill_mask(&mut self.doc, &m, &self.set, color, &br, &mut rng);
                }
            }
            Tool::Rect | Tool::Circle | Tool::Ellipse if self.filled && self.needs_grad_line() => {
                // Filled shape with a range: draw the direction line next.
                let color = if d.primary { self.fg } else { self.bg };
                let m = self.shape_mask(tool, d.start, pos);
                self.begin_grad_line(m, d.base.clone(), color, false, pos);
            }
            Tool::BrushPick => {
                let (x0, y0) = (d.start.0.min(pos.0), d.start.1.min(pos.1));
                let (x1, y1) = (d.start.0.max(pos.0), d.start.1.max(pos.1));
                let (mut w, mut h) = (x1 - x0 + 1, y1 - y0 + 1);
                // ExclBrush: with the grid on, leave out the right and bottom edge pixels.
                if self.v5.prefs.excl_brush && self.set.grid_on && w > 1 && h > 1 {
                    w -= 1;
                    h -= 1;
                }
                *self.doc.img_mut() = d.base.clone();
                let img = self.doc.img().crop(x0, y0, w, h);
                // AutoTransp: four matching corners give the transparent colour.
                let corners = [img.get(0, 0), img.get(w - 1, 0), img.get(0, h - 1), img.get(w - 1, h - 1)];
                let transparent = if self.v5.prefs.auto_transp && corners.iter().all(|c| *c == corners[0]) { corners[0] } else { self.bg };
                self.brush = Brush::from_image(img, Some(transparent));
                if self.v5.prefs.auto_grid && self.set.grid_on {
                    self.set.grid.0 = w.max(1);
                    self.set.grid.1 = h.max(1);
                }
                self.brush_original = None;
                self.anim_brush = None;
                self.last_pick = Some((x0, y0, w, h));
                if !d.primary {
                    // Right button cuts: the picked area is cleared to the background colour.
                    for y in y0..=y1 {
                        for x in x0..=x1 {
                            if !self.doc.protected(x, y) {
                                let bg = self.bg;
                                self.doc.img_mut().set(x, y, bg);
                            }
                        }
                    }
                } else {
                    self.doc.undo.pop();
                }
                // The chosen mode stays (it used to fall back to Matte here, and only for the
                // rectangle pick-up, which looked like the mode cancelling itself).
                self.tool = Tool::Dotted;
            }
            Tool::Lasso => {
                *self.doc.img_mut() = d.base.clone();
                // (picking up a brush: nothing drawn, nothing to anti-alias)
                let pts = d.points.clone();
                if pts.len() > 2 {
                    let spans = pix::polygon_spans(&pts);
                    let m = paint::spans_mask(&spans, self.doc.w, self.doc.h);
                    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
                    for &(y, a, b) in &spans {
                        x0 = x0.min(a);
                        x1 = x1.max(b);
                        y0 = y0.min(y);
                        y1 = y1.max(y);
                    }
                    x0 = x0.max(0);
                    y0 = y0.max(0);
                    x1 = x1.min(self.doc.w - 1);
                    y1 = y1.min(self.doc.h - 1);
                    if x1 >= x0 && y1 >= y0 {
                        let mut img = Image::new(x1 - x0 + 1, y1 - y0 + 1, self.bg);
                        for y in y0..=y1 {
                            for x in x0..=x1 {
                                if m[(y * self.doc.w + x) as usize] {
                                    img.set(x - x0, y - y0, self.doc.img().get(x, y));
                                    if !d.primary {
                                        let bg = self.bg;
                                        self.doc.img_mut().set(x, y, bg);
                                    }
                                }
                            }
                        }
                        self.brush = Brush::from_image(img, Some(self.bg));
                        self.brush_original = None;
                        self.anim_brush = None;
                        self.last_pick = Some((x0, y0, x1 - x0 + 1, y1 - y0 + 1));
                        self.tool = Tool::Dotted;
                    }
                }
                if d.primary {
                    self.doc.undo.pop();
                }
            }
            _ => {}
        }
        // Filled shapes with outline (key i): the current brush traces the edge.
        if matches!(tool, Tool::Rect | Tool::Circle | Tool::Ellipse | Tool::Freehand) {
            self.outline_filled_shape(tool, d.start, pos, &d.points, d.primary);
        }
        // Effect > AntiAlias: soften the edges of what the stroke or shape drew.
        if matches!(tool, Tool::Freehand | Tool::Dotted | Tool::Line | Tool::Rect | Tool::Circle | Tool::Ellipse) && self.v5.grad_line.is_none() && !self.v5.x.anim_paint {
            self.antialias_since(&d.base);
        }
        self.stroke = None;
    }

    /// Anti-alias what was drawn since `before`, when Effect > AntiAlias is Low or High.
    pub(crate) fn antialias_since(&mut self, before: &Image) {
        if self.antialias && self.set.mode != crate::paint::Mode::Smooth {
            crate::amiga::set_active_colors(self.doc.colors);
            paint::antialias_edges(&mut self.doc, before, self.aa_high);
        }
    }

    pub(crate) fn finish_polygon(&mut self) {
        let Some(d) = self.drag.take() else { return };
        *self.doc.img_mut() = d.base.clone();
        let pts = d.points;
        let color = if d.primary { self.fg } else { self.bg };
        let br = self.brush.clone();
        if self.filled && pts.len() > 2 {
            let m = paint::spans_mask(&pix::polygon_spans(&pts), self.doc.w, self.doc.h);
            if self.needs_grad_line() {
                self.begin_grad_line(m, d.base.clone(), color, false, pts[pts.len() - 1]);
                self.outline_filled_shape(Tool::Polygon, pts[0], pts[0], &pts, d.primary);
                return;
            }
            let mut rng = Rng::new(11);
            paint::fill_mask(&mut self.doc, &m, &self.set, color, &br, &mut rng);
            self.outline_filled_shape(Tool::Polygon, pts[0], pts[0], &pts, d.primary);
        } else {
            let mut all = Vec::new();
            for i in 0..pts.len() {
                let a = pts[i];
                let b = pts[(i + 1) % pts.len()];
                if i + 1 == pts.len() && pts.len() < 3 {
                    break;
                }
                pix::line_points(a.0, a.1, b.0, b.1, |x, y| all.push((x, y)));
            }
            all.dedup();
            let mut st = Stroke::new(color, d.primary);
            paint::stamp_points(&mut self.doc, &br, &self.set, &mut st, &all);
        }
        self.antialias_since(&d.base);
    }

    pub fn commit_text(&mut self) {
        if let Some(te) = self.text_entry.take() {
            *self.doc.img_mut() = te.base;
            if let Some(b) = self.text_brush(&te.buffer) {
                let mut st = Stroke::new(self.fg, true);
                let mut set = self.set.clone();
                set.sym.on = false;
                if set.mode != Mode::SingleColor {
                    set.mode = Mode::Paint;
                }
                paint::stamp(&mut self.doc, &b, &set, &mut st, te.pos.0, te.pos.1);
            } else {
                self.doc.undo.pop();
            }
        }
    }

    pub(crate) fn text_brush(&self, s: &str) -> Option<Brush> {
        let font = self.font.as_ref()?;
        let mut style = self.text_style.clone();
        style.antialias = self.antialias;
        let bg = if self.fg == self.bg { (self.fg as u16 + 1) as u8 } else { self.bg };
        crate::text::render_any(font, s, &style, self.fg, bg, &self.doc.palette)
    }

    pub(crate) fn preview_text(&mut self) {
        if let Some(te) = &self.text_entry {
            let base = te.base.clone();
            let pos = te.pos;
            let buf = te.buffer.clone();
            *self.doc.img_mut() = base;
            if let Some(b) = self.text_brush(&buf) {
                let mut st = Stroke::new(self.fg, true);
                let mut set = self.set.clone();
                set.sym.on = false;
                set.mode = Mode::Paint;
                paint::stamp(&mut self.doc, &b, &set, &mut st, pos.0, pos.1);
            }
        }
    }

    // ------------------------------------------------------------ keyboard

    // ------------------------------------------------------------ fullscreen

    pub fn is_fullscreen(&self, ctx: &egui::Context) -> bool {
        ctx.input(|i| i.viewport().fullscreen.unwrap_or(false))
    }

    /// Fill the whole monitor with the program (F11 or Alt+Enter; Esc leaves it).
    pub fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        let on = !self.is_fullscreen(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
        self.fit_pending = true;
        self.status = if on { "Fullscreen — F11, Alt+Enter or Esc to leave".into() } else { String::new() };
    }

    // ------------------------------------------------------------ files

    pub fn request(&mut self, p: Pending) {
        if self.doc.dirty {
            self.pending = Some(p);
        } else {
            self.do_pending(p);
        }
    }

    pub fn do_pending(&mut self, p: Pending) {
        match p {
            Pending::New => {
                self.new_size = (self.doc.w, self.doc.h);
                self.show_new = true;
            }
            Pending::Open => self.open_dialog(),
            Pending::Quit => self.quit_now = true,
            Pending::Revert => {
                if let Some(path) = self.doc.path.clone() {
                    self.open_path(path);
                }
            }
        }
    }

    pub fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Load Picture")
            .add_filter("Pictures and animations", &["lbm", "ilbm", "iff", "bbm", "anim", "pcx", "pcc", "bmp", "png", "gif", "jpg", "jpeg", "tif", "tiff", "tga"])
            .add_filter("Deluxe Paint animation (IFF ANIM)", &["anim"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.open_path_ask(p);
        }
    }

    /// Take the screen format (colours, Amiga mode, palette precision) from a loaded file.
    pub fn format_from_file(&mut self, depth: u32, camg: Option<u32>) {
        use crate::amiga::{Chipset, MODES};
        let depth = if depth == 0 { 8 } else { depth.min(8) };
        let colors = 1usize << depth;
        let mode = crate::amiga::mode_for(camg, self.doc.w, self.doc.h, depth);
        let m = MODES[mode];
        let ehb = m.ehb && depth == 6;
        // A file needing more planes than OCS/ECS allow, or a palette finer than 4 bits per gun, is AGA.
        let n = colors.min(256);
        let four_bit = self.doc.palette[..n].iter().all(|c| c.iter().all(|v| v % 17 == 0));
        let cs = if m.amiga() && four_bit && depth <= m.max_depth(Chipset::Ecs) { Chipset::Ecs } else { Chipset::Aga };
        self.v5.chipset = cs;
        self.v5.sf_mode = mode;
        self.doc.colors = colors;
        self.doc.ehb = ehb;
        self.doc.gun_bits = m.gun_bits(cs);
        self.doc.mode_id = if m.amiga() { camg.filter(|c| *c != 0).unwrap_or(m.mode_id()) } else { 0 };
        self.doc.limit_colors();
        self.doc.saved_palette = self.doc.palette;
        self.pal_page = 0;
    }

    pub fn open_path(&mut self, p: PathBuf) {
        match crate::fileio::load(&p) {
            Ok(crate::fileio::LoadResult::Indexed(l)) => {
                let mut d = Document::new(l.frames[0].w, l.frames[0].h, 0);
                d.frames = l.frames;
                d.palette = l.palette;
                // A file without ranges gets the ramps found in its own palette.
                d.ranges = l.ranges.unwrap_or_default();
                if let Some(f) = l.fps {
                    d.fps = f;
                }
                d.frame_palettes = if l.frame_palettes.len() == d.frames.len() { l.frame_palettes } else { vec![None; d.frames.len()] };
                d.frame_delays = if l.frame_delays.len() == d.frames.len() { l.frame_delays } else { vec![0; d.frames.len()] };
                d.path = Some(p);
                d.saved_frames = Some(d.frames.clone());
                if (d.w, d.h) != self.v5.screen {
                    self.v5.screen = (d.w, d.h);
                }
                self.doc = d;
                self.format_from_file(l.depth, l.camg);
                crate::amiga::set_active_colors(self.doc.colors);
                self.doc.fit_ranges();
                self.status = format!("Screen format {} x {}, {} colours", self.doc.w, self.doc.h, self.doc.colors);
                self.fit_pending = true;
            }
            Ok(crate::fileio::LoadResult::TrueColor(rgba)) => {
                self.convert = Some(ConvertJob { rgba, opts: ConvertOptions::default(), path: Some(p), as_picture: true });
            }
            Err(e) => self.msg("Load Picture", e),
        }
    }

    pub fn save(&mut self, save_as: bool) {
        let path = if save_as || self.doc.path.is_none() {
            rfd::FileDialog::new()
                .set_title("Save Picture")
                .set_file_name(&format!("{}.lbm", self.doc.name().trim_end_matches(".lbm")))
                .add_filter("DeluxePaint picture (IFF .lbm)", &["lbm"])
                .add_filter("PNG", &["png"])
                .add_filter("PCX", &["pcx"])
                .add_filter("BMP (256 colours)", &["bmp"])
                .add_filter("Deluxe Paint animation (IFF ANIM)", &["anim"])
                .add_filter("JPEG", &["jpg"])
                .save_file()
        } else {
            self.doc.path.clone()
        };
        let Some(path) = path else { return };
        if self.backup_pict && path.exists() {
            let backup = path.with_file_name("_backup_.lbm");
            if let Ok(crate::fileio::LoadResult::Indexed(_)) = crate::fileio::load(&path) {
                let _ = std::fs::copy(&path, backup);
            }
        }
        let is_anim = path.extension().map(|e| e.to_string_lossy().to_lowercase() == "anim").unwrap_or(false);
        self.doc.sync_palettes();
        let fmt = self.doc.iff_format();
        let result = if is_anim {
            crate::fileio::save_anim_fmt(&path, &self.doc.frames, &self.doc.palette, &self.doc.ranges, self.doc.fps, &self.doc.frame_palettes, &self.doc.frame_delays, fmt)
        } else {
            crate::fileio::save_any_fmt(&path, &self.doc.frames, self.doc.cur, &self.doc.palette, &self.doc.ranges, self.doc.fps, fmt)
        };
        match result {
            Ok(()) => {
                self.doc.path = Some(path.clone());
                self.doc.dirty = false;
                self.doc.saved_frames = Some(self.doc.frames.clone());
                self.status = format!("Saved {}", path.display());
            }
            Err(e) => self.msg("Save Picture", e),
        }
    }

    pub fn apply_convert(&mut self) {
        let Some(job) = self.convert.take() else { return };
        let (img, pal) = crate::convert::reduce(&job.rgba, &job.opts, &self.doc.palette);
        if job.as_picture {
            let mut d = Document::from_image(img, pal);
            d.path = job.path.map(|p| p.with_extension("lbm"));
            d.dirty = d.path.is_none();
            self.doc = d;
            self.fit_pending = true;
        } else {
            let b = Brush::from_image(img, Some(0));
            self.brush = b.remapped(&pal, &self.doc.palette);
            self.brush_original = None;
            self.tool = Tool::Dotted;
        }
    }
}

/// Keep a view of half-size `half` centred inside a picture of length `len`.
pub(crate) fn clamp_center(c: f32, half: f32, len: f32) -> f32 {
    if 2.0 * half >= len {
        len / 2.0
    } else {
        c.clamp(half, len - half)
    }
}

/// Wheel movement this frame in notches (lines), from the raw wheel events.
fn wheel_notches(ui: &egui::Ui) -> f32 {
    ui.input(|i| {
        i.events
            .iter()
            .map(|e| match e {
                egui::Event::MouseWheel { unit, delta, .. } => match unit {
                    egui::MouseWheelUnit::Line => delta.y,
                    egui::MouseWheelUnit::Page => delta.y * 3.0,
                    egui::MouseWheelUnit::Point => delta.y / 50.0,
                },
                _ => 0.0,
            })
            .sum()
    })
}
