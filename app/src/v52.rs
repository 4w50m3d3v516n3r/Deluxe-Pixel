//! Functions of the Amiga DeluxePaint 5.2 program (bin/DPaint) that the earlier rebuild lacked,
//! found in the program's own command list (its ARexx commands and requesters):
//! frame rate by frame, Copy/Delete Frames ranges, frame scopes for Flip and colour swaps,
//! Pickup AnimBrush, Camera Move over the spare page, the full Move requester (go from / come to,
//! record forward / in place / backward, fade, Preview / Trails / Fill / Draw, Load / Save),
//! Fill to background and fill through all frames, the airbrush and spacing types, interactive
//! perspective, load questions, picture sequences, print ranges, Title and Cursor, and a runner
//! for DeluxePaint ARexx command scripts.

use crate::app::{App, Tool};
use crate::doc::Document;
use crate::paint::{self, Mode, Rng, Stroke};
use crate::palette::nearest;
use crate::pix::{self, Brush, BuiltinShape, Image};
use crate::ui_style::{button, toggle};
use egui::{Color32, RichText};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Current,
    All,
    Range,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeOp {
    FlipH,
    FlipV,
    BgToFg,
    SwapBgFg,
    Print,
    SaveFrames,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyMode {
    /// Copy a range of frames and insert the copies before a frame.
    Range,
    /// Copy the current frame over a range of frames.
    CurrentOver,
    /// Copy the current frame over all frames.
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CamMode {
    /// A new animation of the camera's frames (the old one is dropped).
    New,
    /// The camera's frames inserted before the current frame.
    Before,
    /// The camera's frames inserted after the current frame.
    After,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CamParams {
    pub dx: i32,
    pub dy: i32,
    /// Distance moved away from (positive) or towards (negative) the page.
    pub dz: i32,
    pub count: usize,
    pub slow_in: usize,
    pub slow_out: usize,
    pub wrap: bool,
    pub mode: CamMode,
    /// Where on the spare page the camera starts (top-left of the view).
    pub start: (i32, i32),
    /// Cyclic: the last frame stops one step short, for a seamless loop.
    pub cyclic: bool,
    /// Move: come to here (the move ends at the start position) instead of go from here.
    pub come_to: bool,
    /// "Cam" box: distances along the camera's own axes (the same as the screen's while it doesn't turn).
    pub cam_axes: bool,
}

/// A picture being loaded that waits for an answer.
pub struct LoadAsk {
    pub path: PathBuf,
    pub loaded: crate::fileio::Loaded,
    pub stage: u8,
}

/// The Choose Font requester's working copy and the font loaded for its sample line.
pub struct FontEdit {
    pub idx: usize,
    pub size: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub preview: Option<crate::text::LoadedFont>,
    pub preview_key: Option<(usize, u32)>,
}

pub struct V52 {
    pub show_rate: bool,
    pub rate_by_frame: bool,
    pub rate_jiffies: bool,
    pub rate_value: f32,
    pub rate_scope: Scope,
    pub rate_ab: (usize, usize),
    pub show_copy: bool,
    pub copy_mode: CopyMode,
    pub copy_ab: (usize, usize),
    pub copy_before: usize,
    /// Copy Frames: the range the current frame is copied over.
    pub copy_over: (usize, usize),
    pub scope_op: Option<ScopeOp>,
    pub scope: Scope,
    pub scope_ab: (usize, usize),
    pub show_pickup: bool,
    pub pickup_count: usize,
    pub pickup_cut: bool,
    pub show_cam: bool,
    pub cam: CamParams,
    pub move_come_to: bool,
    /// Record: 1 forward, 0 in place, -1 backward.
    pub move_record: i32,
    pub move_fade: (u8, u8),
    /// Move > Preview: the wire frame of every step, and the running preview time.
    pub move_preview: Vec<[(f32, f32); 4]>,
    pub move_preview_t: f32,
    /// Move > Preview is switched on (it shows until clicked again or the requester closes).
    pub move_preview_on: bool,
    /// What the preview was last worked out from (it is only worked out again when this changes).
    pub move_preview_key: String,
    /// Move: distances along the brush's axes; angles about the brush's axes (manual defaults).
    pub mv_dist_brush: bool,
    pub mv_angle_brush: bool,
    /// Where moves start (the last stamp) and where the last move ended; Go Back.
    pub mv_origin: Option<crate::move3d::MovePose>,
    pub mv_cur: Option<crate::move3d::MovePose>,
    pub mv_go_back: bool,
    /// Start / End: which end Fade and Adjust work on.
    pub mv_fade_end: bool,
    /// Adjusting the start (false) or end (true) by hand, and what to put back afterwards.
    pub mv_adjust: Option<bool>,
    pub mv_adjust_saved: Option<(bool, Option<Brush>, crate::persp3d::Persp3, Brush)>,
    /// The Move settings when the requester opened (Cancel restores them), and whether it was
    /// opened before (the first time Count takes the number of frames).
    #[allow(clippy::type_complexity)]
    pub mv_backup: Option<(crate::app::MoveParams, bool, i32, (u8, u8), bool, bool, bool)>,
    pub mv_opened_once: bool,
    /// CLR (right button) requester: current frame, range or all frames.
    pub show_clear: bool,
    pub clear_scope: Scope,
    pub clear_ab: (usize, usize),
    /// AnimPainting: Alt held when the button went down; each stamp goes to the next frame.
    pub alt_down: bool,
    /// Spacing requester: the chosen option (0 Continuous, 1 N Total, 2 Every Nth Dot, 3 Airbrush),
    /// each option's own N, and whether the requester was open last frame.
    pub spacing_kind: u8,
    /// Requesters of the reference section (chapter 8).
    pub r8: crate::reqs8::Reqs8,
    /// The Mixer panel's state.
    pub mixer: crate::mixer::MixerState,
    /// Set Frame Palette and Frame Palette Info.
    pub fpal: crate::framepal::FramePalState,
    /// Colour Set > Load: the colours being chosen.
    pub cset: Option<crate::colorset::ColourSetChoice>,
    /// Choose Font being edited (OK takes it).
    pub font_edit: Option<FontEdit>,
    /// Range panel: a colour being dragged (colour, the bar position it came from) and which 64 of
    /// the bar's 256 positions are shown.
    pub range_drag: Option<(u8, Option<u8>)>,
    pub range_bar_off: u8,
    /// The tool in use before a colour picker button was pressed.
    pub picker_prev: Option<crate::app::Tool>,
    /// The last u was an undo (u again redoes); Shift held; the axis a Shift-drag keeps to.
    pub u_was_undo: bool,
    pub shift_down: bool,
    /// Ctrl held, trails drawn in this drag, and where the last trail line ended.
    pub ctrl_down: bool,
    pub trails_used: bool,
    pub trails_last: Option<(i32, i32)>,
    pub constrain_axis: Option<u8>,
    /// Filled shapes with an outline (key i): after the fill, the current brush traces the edge,
    /// following Spacing (airbrush, N total...). An outline waiting for a range fill to finish.
    pub fill_outline: bool,
    pub outline_pending: Option<(Vec<(i32, i32)>, bool)>,
    pub spacing_nth: i32,
    pub spacing_ntotal: i32,
    pub spacing_was_open: bool,
    pub anim_paint: bool,
    pub fill_to_bg: bool,
    pub anim_fill: bool,
    pub persp_mode: bool,
    pub persp_base: Option<Brush>,
    pub persp_step: f32,
    pub persp_depth: f32,
    /// Perspective mode's 3-D brush (angles, depth, step, axes type, read-out, grid).
    pub p3: crate::persp3d::Persp3,
    /// The Perspective requester's working copy (Cancel throws it away).
    pub p3_edit: Option<(crate::persp3d::Persp3, f32)>,
    pub ask: Option<LoadAsk>,
    pub load_append: bool,
    /// Change to the file's screen format: 0 ask, 1 always, 2 never.
    pub format_policy: u8,
    pub show_load_opts: bool,
    pub show_seq: bool,
    pub seq_first: Option<PathBuf>,
    pub seq_count: usize,
    pub title: bool,
    pub crosshair: bool,
    pub script_log: Vec<String>,
    pub show_script_log: bool,
}

impl Default for V52 {
    fn default() -> Self {
        V52 {
            show_rate: false,
            rate_by_frame: false,
            rate_jiffies: false,
            rate_value: 12.0,
            rate_scope: Scope::All,
            rate_ab: (1, 1),
            show_copy: false,
            copy_mode: CopyMode::Range,
            copy_ab: (1, 1),
            copy_before: 1,
            copy_over: (1, 1),
            scope_op: None,
            scope: Scope::Current,
            scope_ab: (1, 1),
            show_pickup: false,
            pickup_count: 8,
            pickup_cut: false,
            show_cam: false,
            cam: CamParams { dx: 0, dy: 0, dz: 0, count: 10, slow_in: 0, slow_out: 0, wrap: true, mode: CamMode::New, start: (0, 0), cyclic: true, come_to: false, cam_axes: false },
            move_come_to: false,
            move_record: 1,
            move_fade: (0, 0),
            move_preview: Vec::new(),
            move_preview_t: 0.0,
            move_preview_on: false,
            move_preview_key: String::new(),
            mv_dist_brush: false,
            mv_angle_brush: true,
            mv_origin: None,
            mv_cur: None,
            mv_go_back: false,
            mv_fade_end: false,
            mv_adjust: None,
            mv_adjust_saved: None,
            mv_backup: None,
            mv_opened_once: false,
            show_clear: false,
            clear_scope: Scope::Current,
            clear_ab: (1, 1),
            alt_down: false,
            spacing_kind: 0,
            r8: Default::default(),
            mixer: Default::default(),
            fpal: Default::default(),
            cset: None,
            font_edit: None,
            range_drag: None,
            range_bar_off: 0,
            picker_prev: None,
            u_was_undo: false,
            shift_down: false,
            ctrl_down: false,
            trails_used: false,
            trails_last: None,
            constrain_axis: None,
            fill_outline: false,
            outline_pending: None,
            spacing_nth: 8,
            spacing_ntotal: 8,
            spacing_was_open: false,
            anim_paint: false,
            fill_to_bg: false,
            anim_fill: false,
            persp_mode: false,
            persp_base: None,
            persp_step: 5.0,
            persp_depth: 2.0,
            p3: crate::persp3d::Persp3::default(),
            p3_edit: None,
            ask: None,
            load_append: false,
            format_policy: 0,
            show_load_opts: false,
            show_seq: false,
            seq_first: None,
            seq_count: 10,
            title: true,
            crosshair: true,
            script_log: Vec::new(),
            show_script_log: false,
        }
    }
}

fn req(title: &str) -> egui::Window<'static> {
    egui::Window::new(RichText::new(title.to_string()).color(Color32::WHITE)).collapsible(false).resizable(false)
}

/// Frames a scope covers (0-based).
pub fn scope_frames(scope: Scope, ab: (usize, usize), cur: usize, n: usize) -> Vec<usize> {
    match scope {
        Scope::Current => vec![cur],
        Scope::All => (0..n).collect(),
        Scope::Range => {
            let a = ab.0.min(ab.1).clamp(1, n) - 1;
            let b = ab.0.max(ab.1).clamp(1, n) - 1;
            (a..=b).collect()
        }
    }
}

fn scope_ui(ui: &mut egui::Ui, scope: &mut Scope, ab: &mut (usize, usize), n: usize) {
    ui.horizontal(|ui| {
        for (s, l) in [(Scope::Current, "Current Frame"), (Scope::All, "All Frames"), (Scope::Range, "Range")] {
            if toggle(ui, l, *scope == s).clicked() {
                *scope = s;
            }
        }
    });
    if *scope == Scope::Range {
        ui.horizontal(|ui| {
            ui.label("Frames");
            ui.add(egui::DragValue::new(&mut ab.0).range(1..=n.max(1)));
            ui.label("to");
            ui.add(egui::DragValue::new(&mut ab.1).range(1..=n.max(1)));
        });
    }
}

/// Connected pixels that are not the background colour (Fill to Background).
pub fn region_to_bg(img: &Image, x: i32, y: i32, bg: u8) -> Option<Vec<bool>> {
    if !img.inside(x, y) {
        return None;
    }
    if img.get(x, y) == bg {
        return pix::flood_mask(img, x, y, 64_000_000);
    }
    let mut mask = vec![false; img.px.len()];
    let mut stack = vec![(x, y)];
    while let Some((cx, cy)) = stack.pop() {
        if !img.inside(cx, cy) {
            continue;
        }
        let i = (cy * img.w + cx) as usize;
        if mask[i] || img.px[i] == bg {
            continue;
        }
        mask[i] = true;
        stack.extend_from_slice(&[(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)]);
    }
    Some(mask)
}

/// The camera's view of the spare page at step `t` (0..1).
pub fn camera_view(spare: &Image, w: i32, h: i32, cam: &CamParams, t: f32, bg: u8) -> Image {
    let ox = cam.start.0 as f32 + cam.dx as f32 * t;
    let oy = cam.start.1 as f32 + cam.dy as f32 * t;
    let eye = 300.0f32;
    let s = (eye / (eye + cam.dz as f32 * t)).clamp(0.05, 20.0);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let mut out = Image::new(w, h, bg);
    for y in 0..h {
        for x in 0..w {
            let sx = ((x as f32 - cx) / s + cx + ox).floor() as i32;
            let sy = ((y as f32 - cy) / s + cy + oy).floor() as i32;
            let p = if cam.wrap {
                Some(spare.get(sx.rem_euclid(spare.w), sy.rem_euclid(spare.h)))
            } else if spare.inside(sx, sy) {
                Some(spare.get(sx, sy))
            } else {
                None
            };
            if let Some(p) = p {
                out.set(x, y, p);
            }
        }
    }
    out
}

fn kv_text(pairs: &[(&str, String)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={v}\n")).collect()
}

fn kv_parse(t: &str) -> std::collections::HashMap<String, String> {
    t.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect()
}

impl App {
    // ---------------------------------------------------------------- requesters

    pub fn v52_dialogs(&mut self, ctx: &egui::Context) {
        self.rate_req(ctx);
        self.copy_req(ctx);
        self.scope_req(ctx);
        self.pickup_req(ctx);
        self.cam_req(ctx);
        self.move_req3(ctx);
        self.frame_palette_reqs(ctx);
        self.clear_req(ctx);
        self.load_ask_req(ctx);
        self.load_opts_req(ctx);
        self.seq_req(ctx);
        self.script_log_req(ctx);
    }

    /// Set Frame Rate, laid out as the manual's figure 8.34.
    fn rate_req(&mut self, ctx: &egui::Context) {
        if !self.v5.x.show_rate {
            return;
        }
        let n = self.doc.frames.len();
        let mut done = None;
        req("Frame Rate").show(ctx, |ui| {
            ui.set_width(330.0);
            let x = &mut self.v5.x;
            // rate_value is in frames per second here.
            if x.rate_jiffies {
                x.rate_value = (60.0 / x.rate_value.max(1.0)).max(0.1);
                x.rate_jiffies = false;
            }
            ui.vertical_centered(|ui| ui.label("Play rate:"));
            ui.horizontal(|ui| {
                if ui.radio(!x.rate_by_frame, "Global (fps)").clicked() {
                    x.rate_by_frame = false;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut v = x.rate_value.round() as i32;
                    if ui.add_enabled(!x.rate_by_frame, egui::DragValue::new(&mut v).range(1..=60)).changed() {
                        x.rate_value = v as f32;
                    }
                });
            });
            if ui.radio(x.rate_by_frame, "Vary by frame").clicked() {
                x.rate_by_frame = true;
            }
            ui.separator();
            ui.vertical_centered(|ui| ui.label("Specify rate in:"));
            ui.horizontal(|ui| {
                ui.label("Frames per second");
                ui.spacing_mut().slider_width = 110.0;
                ui.add(egui::Slider::new(&mut x.rate_value, 1.0..=60.0).show_value(false));
                let mut v = x.rate_value.round() as i32;
                if ui.add(egui::DragValue::new(&mut v).range(1..=60)).changed() {
                    x.rate_value = v as f32;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Jiffies (1/60)");
                let mut j = (60.0 / x.rate_value.max(0.1)).round() as i32;
                if ui.add(egui::DragValue::new(&mut j).range(1..=3600)).on_hover_text("How long one frame stays, in 1/60 s (180 = 3 seconds)").changed() {
                    x.rate_value = 60.0 / j.max(1) as f32;
                }
            });
            ui.separator();
            ui.add_enabled_ui(x.rate_by_frame, |ui| {
                if ui.radio(x.rate_scope == Scope::Current, "Current frame").clicked() {
                    x.rate_scope = Scope::Current;
                }
                ui.horizontal(|ui| {
                    if ui.radio(x.rate_scope == Scope::Range, "Range").clicked() {
                        x.rate_scope = Scope::Range;
                    }
                    ui.add_space(40.0);
                    ui.add(egui::DragValue::new(&mut x.rate_ab.0).range(1..=n));
                    ui.label("to");
                    ui.add(egui::DragValue::new(&mut x.rate_ab.1).range(1..=n));
                });
                if ui.radio(x.rate_scope == Scope::All, "All frames").clicked() {
                    x.rate_scope = Scope::All;
                }
            });
            ui.horizontal(|ui| {
                if button(ui, "Ok").clicked() {
                    done = Some(true);
                }
                ui.add_space(220.0);
                if button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            if ok {
                let (by_frame, fps, scope, ab) = {
                    let x = &self.v5.x;
                    (x.rate_by_frame, x.rate_value.max(0.1), x.rate_scope, x.rate_ab)
                };
                if by_frame {
                    let j = (60.0 / fps).round().max(1.0) as u32;
                    self.doc.sync_palettes();
                    for i in scope_frames(scope, ab, self.doc.cur, n) {
                        self.doc.frame_delays[i] = j;
                    }
                } else {
                    self.doc.fps = fps;
                    self.doc.frame_delays.iter_mut().for_each(|d| *d = 0);
                }
            }
            self.v5.x.show_rate = false;
        }
    }

    pub fn open_rate(&mut self) {
        let fps = self.doc.fps;
        let cur = self.doc.cur;
        let x = &mut self.v5.x;
        x.rate_value = if x.rate_jiffies { (60.0 / fps).round() } else { fps.round() };
        x.rate_ab = (cur + 1, cur + 1);
        x.show_rate = true;
    }

    /// Copy Frames, laid out as the manual's figure 8.33.
    fn copy_req(&mut self, ctx: &egui::Context) {
        if !self.v5.x.show_copy {
            return;
        }
        let n = self.doc.frames.len();
        let cur = self.doc.cur;
        let mut done = None;
        req("Copy Frames").show(ctx, |ui| {
            ui.set_width(320.0);
            let x = &mut self.v5.x;
            ui.label("Current frame over:");
            ui.horizontal(|ui| {
                if ui.radio(x.copy_mode == CopyMode::CurrentOver, "Range").clicked() {
                    x.copy_mode = CopyMode::CurrentOver;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled(x.copy_mode == CopyMode::CurrentOver, egui::DragValue::new(&mut x.copy_over.1).range(1..=n));
                    ui.label("to");
                    ui.add_enabled(x.copy_mode == CopyMode::CurrentOver, egui::DragValue::new(&mut x.copy_over.0).range(1..=n));
                });
            });
            if ui.radio(x.copy_mode == CopyMode::All, "All Frames").clicked() {
                x.copy_mode = CopyMode::All;
            }
            ui.horizontal(|ui| {
                if ui.radio(x.copy_mode == CopyMode::Range, "Copy Range").clicked() {
                    x.copy_mode = CopyMode::Range;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled(x.copy_mode == CopyMode::Range, egui::DragValue::new(&mut x.copy_ab.1).range(1..=n));
                    ui.label("to");
                    ui.add_enabled(x.copy_mode == CopyMode::Range, egui::DragValue::new(&mut x.copy_ab.0).range(1..=n));
                });
            });
            ui.horizontal(|ui| {
                ui.add_space(30.0);
                ui.label("Before frame:");
                ui.add_enabled(x.copy_mode == CopyMode::Range, egui::DragValue::new(&mut x.copy_before).range(1..=n + 1));
            });
            ui.horizontal(|ui| {
                if button(ui, "Ok").clicked() {
                    done = Some(true);
                }
                ui.add_space(210.0);
                if button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            if ok {
                match self.v5.x.copy_mode {
                    CopyMode::Range => {
                        let (a, b) = self.v5.x.copy_ab;
                        let before = self.v5.x.copy_before - 1;
                        self.doc.insert_copies(a.min(b) - 1, a.max(b) - 1, before);
                    }
                    CopyMode::CurrentOver => {
                        let (a, b) = self.v5.x.copy_over;
                        self.doc.copy_frames(cur, a.min(b) - 1, a.max(b) - 1);
                    }
                    CopyMode::All => self.doc.copy_frames(cur, 0, n - 1),
                }
            }
            self.v5.x.show_copy = false;
        }
    }

    pub fn open_copy(&mut self) {
        let n = self.doc.frames.len();
        let cur = self.doc.cur;
        let x = &mut self.v5.x;
        x.copy_ab = (cur + 1, cur + 1);
        x.copy_over = (1, n);
        x.copy_mode = CopyMode::CurrentOver;
        x.copy_before = n + 1;
        x.show_copy = true;
    }

    /// Ask which frames an operation applies to (only when there are several).
    pub fn with_scope(&mut self, op: ScopeOp) {
        if self.doc.frames.len() <= 1 && op != ScopeOp::SaveFrames {
            let cur = self.doc.cur;
            self.apply_scope(op, vec![cur]);
        } else {
            self.v5.x.scope_op = Some(op);
            self.v5.x.scope_ab = (1, self.doc.frames.len());
        }
    }

    fn scope_req(&mut self, ctx: &egui::Context) {
        let Some(op) = self.v5.x.scope_op else { return };
        let n = self.doc.frames.len();
        let title = match op {
            ScopeOp::FlipH => "Flip Horiz",
            ScopeOp::FlipV => "Flip Vert",
            ScopeOp::BgToFg => "Bg -> Fg",
            ScopeOp::SwapBgFg => "Bg <-> Fg",
            ScopeOp::Print => "Print Picture",
            ScopeOp::SaveFrames => "Save Frames",
        };
        let mut done = None;
        req(title).show(ctx, |ui| {
            ui.label("Frame Range:");
            let x = &mut self.v5.x;
            scope_ui(ui, &mut x.scope, &mut x.scope_ab, n);
            ui.horizontal(|ui| {
                if button(ui, "Ok").clicked() {
                    done = Some(true);
                }
                if button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            self.v5.x.scope_op = None;
            if ok {
                let frames = scope_frames(self.v5.x.scope, self.v5.x.scope_ab, self.doc.cur, n);
                self.apply_scope(op, frames);
            }
        }
    }

    pub fn apply_scope(&mut self, op: ScopeOp, frames: Vec<usize>) {
        let start = self.doc.cur;
        match op {
            ScopeOp::Print => {
                for i in frames {
                    let tmp = std::env::temp_dir().join(format!("deluxe-pixel-print-{}.png", i + 1));
                    let r = crate::fileio::save_png(&tmp, &self.doc.frames[i], &self.doc.palette, None).and_then(|_| crate::capture::print_file(&tmp));
                    if let Err(e) = r {
                        self.msg("Print", e);
                        break;
                    }
                }
                return;
            }
            ScopeOp::SaveFrames => {
                let Some(p) = rfd::FileDialog::new()
                    .set_title("Save Frames (a number is added to each name)")
                    .set_file_name("frame.lbm")
                    .add_filter("DeluxePaint picture", &["lbm"])
                    .add_filter("PNG", &["png"])
                    .save_file()
                else {
                    return;
                };
                let imgs: Vec<Image> = frames.iter().map(|&i| self.doc.frames[i].clone()).collect();
                match crate::fileio::save_frames(&p, &imgs, &self.doc.palette, &self.doc.ranges) {
                    Ok(n) => self.status = format!("Saved {n} pictures"),
                    Err(e) => self.msg("Save Frames", e),
                }
                return;
            }
            _ => {}
        }
        let (fg, bg) = (self.fg, self.bg);
        for i in frames {
            self.doc.cur = i;
            self.doc.checkpoint();
            let f = &mut self.doc.frames[i];
            match op {
                ScopeOp::FlipH => *f = f.flip_x(),
                ScopeOp::FlipV => *f = f.flip_y(),
                ScopeOp::BgToFg | ScopeOp::SwapBgFg => {
                    for p in f.px.iter_mut() {
                        if *p == bg {
                            *p = fg;
                        } else if op == ScopeOp::SwapBgFg && *p == fg {
                            *p = bg;
                        }
                    }
                }
                _ => {}
            }
        }
        self.doc.cur = start;
    }

    // ---------------------------------------------------------------- CLR with the right button

    /// CLR with the right button: clear the current frame, a range of frames or all frames.
    pub fn open_clear(&mut self) {
        if self.doc.frames.len() <= 1 {
            self.doc.clear(self.bg);
            self.move_forget_end();
            return;
        }
        let cur = self.doc.cur + 1;
        let x = &mut self.v5.x;
        x.clear_scope = Scope::Current;
        x.clear_ab = (cur, self.doc.frames.len());
        x.show_clear = true;
    }

    fn clear_req(&mut self, ctx: &egui::Context) {
        if !self.v5.x.show_clear {
            return;
        }
        let n = self.doc.frames.len();
        let mut done = None;
        req("Clear").show(ctx, |ui| {
            let x = &mut self.v5.x;
            for (s, l) in [(Scope::Current, "Current Frame"), (Scope::Range, "Range"), (Scope::All, "All Frames")] {
                ui.horizontal(|ui| {
                    if ui.radio(x.clear_scope == s, l).clicked() {
                        x.clear_scope = s;
                    }
                    if s == Scope::Range {
                        ui.add_enabled(x.clear_scope == Scope::Range, egui::DragValue::new(&mut x.clear_ab.0).range(1..=n));
                        ui.label("to");
                        ui.add_enabled(x.clear_scope == Scope::Range, egui::DragValue::new(&mut x.clear_ab.1).range(1..=n));
                    }
                });
            }
            ui.horizontal(|ui| {
                if button(ui, "Ok").clicked() {
                    done = Some(true);
                }
                ui.add_space(80.0);
                if button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            self.v5.x.show_clear = false;
            if ok {
                let x = &self.v5.x;
                let frames = scope_frames(x.clear_scope, x.clear_ab, self.doc.cur, n);
                let all = x.clear_scope == Scope::All;
                let start = self.doc.cur;
                let bg = self.bg;
                for i in frames {
                    self.doc.cur = i;
                    self.doc.clear(bg);
                }
                // The drawn moves are gone: the next move starts where the brush was stamped.
                self.move_forget_end();
                // After clearing all frames the first one is shown.
                self.doc.go_to(if all { 0 } else { start });
            }
        }
    }

    // ---------------------------------------------------------------- AnimBrush pickup and morph

    fn pickup_req(&mut self, ctx: &egui::Context) {
        if !self.v5.x.show_pickup {
            return;
        }
        let n = self.doc.frames.len();
        let cur = self.doc.cur;
        let pick = self.last_pick;
        let mut done = None;
        req("Pickup AnimBrush").show(ctx, |ui| {
            match pick {
                Some((x, y, w, h)) => {
                    ui.label(format!("Area: {w} x {h} at {x}, {y} (the last brush you picked up)"));
                }
                None => {
                    ui.colored_label(Color32::from_rgb(128, 0, 0), "Pick up a brush first (b): its area is taken from each frame.");
                }
            }
            ui.horizontal(|ui| {
                ui.label("Count:");
                ui.add(egui::DragValue::new(&mut self.v5.x.pickup_count).range(1..=n.max(1)));
                ui.label(format!("frames from frame {}", cur + 1));
            });
            ui.checkbox(&mut self.v5.x.pickup_cut, "Cut (clear the area in each frame)");
            ui.horizontal(|ui| {
                if button(ui, "Ok").clicked() {
                    done = Some(true);
                }
                if button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            self.v5.x.show_pickup = false;
            if ok {
                if let Some(r) = self.last_pick {
                    let (c, cut) = (self.v5.x.pickup_count, self.v5.x.pickup_cut);
                    self.pick_anim_brush_at(r, c, cut);
                }
            }
        }
    }

    pub fn pick_anim_brush_at(&mut self, (x, y, w, h): (i32, i32, i32, i32), count: usize, cut: bool) {
        let n = self.doc.frames.len();
        let count = count.clamp(1, n);
        let start = self.doc.cur;
        let mut cells = Vec::new();
        let bg = self.bg;
        for k in 0..count {
            let i = (start + k) % n;
            cells.push(self.doc.frames[i].crop(x, y, w, h));
            if cut {
                self.doc.cur = i;
                self.doc.checkpoint();
                for yy in y..y + h {
                    for xx in x..x + w {
                        self.doc.frames[i].set(xx, yy, bg);
                    }
                }
            }
        }
        self.doc.cur = start;
        self.brush = Brush::from_image(cells[0].clone(), Some(bg));
        self.anim_brush = Some(crate::app::AnimBrush::new(cells));
        self.last_pick = Some((x, y, w, h));
        self.tool = Tool::Dotted;
        self.status = format!("AnimBrush with {count} cells");
    }

    /// Morph: an AnimBrush of `cells` cells changing the brush into the spare brush.
    pub fn morph_cells(&mut self, cells: usize) -> bool {
        let Some(b2) = self.v5.spare_brush.clone() else { return false };
        let b1 = self.brush.clone();
        let n = cells.clamp(2, 999);
        let tr = self.bg;
        let mut out_cells = Vec::with_capacity(n);
        let mut maxw = 1;
        let mut maxh = 1;
        for k in 0..n {
            let t = k as f32 / (n - 1) as f32;
            let w = ((1.0 - t) * b1.img.w as f32 + t * b2.img.w as f32).round().max(1.0) as i32;
            let h = ((1.0 - t) * b1.img.h as f32 + t * b2.img.h as f32).round().max(1.0) as i32;
            maxw = maxw.max(w);
            maxh = maxh.max(h);
            let a = b1.img.scaled(w, h);
            let b = b2.img.scaled(w, h);
            let mut out = Image::new(w, h, tr);
            for yy in 0..h {
                for xx in 0..w {
                    let hsh = ((xx as u32).wrapping_mul(73856093) ^ (yy as u32).wrapping_mul(19349663)) % 1000;
                    let (img, trn) = if (hsh as f32 / 1000.0) < t { (&b, b2.transparent) } else { (&a, b1.transparent) };
                    let p = img.get(xx, yy);
                    if Some(p) != trn {
                        out.set(xx, yy, p);
                    }
                }
            }
            out_cells.push(out);
        }
        // All cells the same size and centred, so the brush stays in place.
        let cells: Vec<Image> = out_cells
            .into_iter()
            .map(|c| {
                let mut o = Image::new(maxw, maxh, tr);
                let (ox, oy) = ((maxw - c.w) / 2, (maxh - c.h) / 2);
                for y in 0..c.h {
                    for x in 0..c.w {
                        o.set(x + ox, y + oy, c.get(x, y));
                    }
                }
                o
            })
            .collect();
        self.brush = Brush::from_image(cells[0].clone(), Some(tr));
        self.anim_brush = Some(crate::app::AnimBrush::new(cells));
        self.tool = Tool::Dotted;
        self.status = format!("Morph AnimBrush with {n} cells");
        true
    }

    // ---------------------------------------------------------------- Camera Move

    /// Camera Move, laid out as the manual's figure 8.30.
    fn cam_req(&mut self, ctx: &egui::Context) {
        if !self.v5.x.show_cam {
            return;
        }
        let mut action = None;
        egui::Window::new(RichText::new("Camera Move").color(Color32::WHITE)).collapsible(false).resizable(false).show(ctx, |ui| {
            const W: f32 = 560.0;
            ui.set_width(W);
            let c = &mut self.v5.x.cam;
            egui::Grid::new("cam_xyz").num_columns(5).spacing([14.0, 6.0]).show(ui, |ui| {
                ui.label("");
                for h in ["X", "Y", "Z"] {
                    ui.vertical_centered(|ui| {
                        ui.set_width(100.0);
                        ui.label(RichText::new(h).strong());
                    });
                }
                ui.label("");
                ui.end_row();
                ui.label("Dist:");
                for v in [&mut c.dx, &mut c.dy] {
                    ui.add_sized([100.0, 22.0], egui::DragValue::new(v).range(-9999..=9999));
                }
                ui.add_sized([100.0, 22.0], egui::DragValue::new(&mut c.dz).range(-280..=9999))
                    .on_hover_text("Negative: the camera moves away (more is seen); positive: it zooms in");
                ui.checkbox(&mut c.cam_axes, "Cam").on_hover_text("Distances along the camera's own axes");
                ui.end_row();
            });
            ui.horizontal(|ui| {
                ui.add_space(170.0);
                if ui.add_sized([70.0, 22.0], egui::Button::new("Zero")).clicked() {
                    c.dx = 0;
                    c.dy = 0;
                    c.dz = 0;
                }
                ui.checkbox(&mut c.cyclic, "Cyclic");
            });
            ui.separator();
            ui.allocate_ui_with_layout(egui::vec2(W, 84.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.set_height(84.0);
                ui.vertical(|ui| {
                    ui.set_width(220.0);
                    egui::Grid::new("cam_counts").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                        for (l, v) in [("Slow In:", &mut c.slow_in), ("Slow Out:", &mut c.slow_out)] {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.set_width(100.0);
                                ui.label(l)
                            });
                            ui.add_sized([80.0, 20.0], egui::DragValue::new(v).range(0..=999));
                            ui.end_row();
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.set_width(100.0);
                            ui.label("Count:")
                        });
                        ui.add_sized([80.0, 20.0], egui::DragValue::new(&mut c.count).range(1..=999));
                        ui.end_row();
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(140.0);
                    ui.vertical_centered(|ui| ui.label(RichText::new("Move").strong()));
                    if ui.radio(!c.come_to, "•→").on_hover_text("Go from here: from the current frame on").clicked() {
                        c.come_to = false;
                    }
                    if ui.radio(c.come_to, "→•").on_hover_text("Come to here: ending at the current position").clicked() {
                        c.come_to = true;
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.add_space(24.0);
                    ui.checkbox(&mut c.wrap, "Wrap").on_hover_text("Seamless where the camera goes past the edge of the spare page");
                });
            });
            ui.separator();
            let bw = (W - 18.0) / 4.0;
            ui.horizontal(|ui| {
                ui.add_sized([bw, 24.0], egui::Button::new("Start").selected(true)).on_hover_text("The values are for the first frame");
                ui.add_enabled(false, egui::Button::new("End").min_size(egui::vec2(bw, 24.0))).on_disabled_hover_text("Not available yet");
                ui.add_space(bw);
                ui.add_enabled(false, egui::Button::new("Adjust").min_size(egui::vec2(bw, 24.0))).on_disabled_hover_text("Not available yet");
            });
            for row in [[(9u8, "Preview"), (0, "New"), (1, "Before"), (2, "After")], [(8, "Exit"), (4, "Load"), (5, "Save"), (3, "Cancel")]] {
                ui.horizontal(|ui| {
                    for (id, l) in row {
                        let enabled = id != 9;
                        let tip = match id {
                            0 => "A new animation (the old one is dropped; can't be undone)",
                            1 => "Inserted before the current frame",
                            2 => "Inserted after the current frame",
                            9 => "Not available yet",
                            _ => "",
                        };
                        if ui.add_enabled(enabled, egui::Button::new(l).min_size(egui::vec2(bw, 26.0))).on_hover_text(tip).on_disabled_hover_text(tip).clicked() {
                            action = Some(id);
                        }
                    }
                });
            }
        });
        match action {
            Some(m @ 0..=2) => {
                self.v5.x.cam.mode = [CamMode::New, CamMode::Before, CamMode::After][m as usize];
                self.record_camera();
                self.v5.x.show_cam = false;
            }
            Some(4) => self.load_cam(),
            Some(5) => self.save_cam(),
            Some(3) | Some(8) => self.v5.x.show_cam = false,
            _ => {}
        }
    }

    /// Record the camera's frames: New replaces the animation, Before / After insert them
    /// before / after the current frame (manual p. 241).
    pub fn record_camera(&mut self) {
        let cam = self.v5.x.cam.clone();
        let spare = self.spare.img().clone();
        let (w, h, bg) = (self.doc.w, self.doc.h, self.bg);
        let n = cam.count.max(1);
        let frames: Vec<Image> = crate::move3d::move_fractions(n, cam.cyclic, cam.slow_in, cam.slow_out)
            .into_iter()
            .map(|t| camera_view(&spare, w, h, &cam, if cam.come_to { t - 1.0 } else { t }, bg))
            .collect();
        self.doc.sync_palettes();
        match cam.mode {
            CamMode::New => {
                self.doc.frames = frames;
                self.doc.frame_palettes = vec![None; n];
                self.doc.frame_delays = vec![0; n];
                self.doc.cur = 0;
                self.doc.undo.clear();
                self.doc.redo.clear();
            }
            CamMode::Before | CamMode::After => {
                let at = if cam.mode == CamMode::Before { self.doc.cur } else { self.doc.cur + 1 };
                for (k, f) in frames.into_iter().enumerate() {
                    self.doc.frames.insert(at + k, f);
                    self.doc.frame_palettes.insert(at + k, None);
                    self.doc.frame_delays.insert(at + k, 0);
                }
                self.doc.cur = at;
                self.doc.undo.clear();
                self.doc.redo.clear();
            }
        }
        self.doc.dirty = true;
        self.doc.go_to(self.doc.cur);
        self.status = format!("Camera recorded {n} frames");
    }

    fn save_cam(&mut self) {
        let c = &self.v5.x.cam;
        let text = kv_text(&[
            ("dx", c.dx.to_string()),
            ("dy", c.dy.to_string()),
            ("dz", c.dz.to_string()),
            ("count", c.count.to_string()),
            ("slowin", c.slow_in.to_string()),
            ("slowout", c.slow_out.to_string()),
            ("wrap", (c.wrap as u8).to_string()),
            ("startx", c.start.0.to_string()),
            ("starty", c.start.1.to_string()),
        ]);
        if let Some(p) = rfd::FileDialog::new().set_title("Save Camera").set_file_name("camera.cam").add_filter("Camera", &["cam"]).save_file() {
            if let Err(e) = std::fs::write(&p, text) {
                self.msg("Save Camera", format!("Can't Save: {e}"));
            }
        }
    }

    fn load_cam(&mut self) {
        let Some(p) = rfd::FileDialog::new().set_title("Load Camera").add_filter("Camera", &["cam"]).pick_file() else { return };
        match std::fs::read_to_string(&p) {
            Ok(t) => {
                let m = kv_parse(&t);
                let g = |k: &str, d: i64| m.get(k).and_then(|v| v.parse::<i64>().ok()).unwrap_or(d);
                let c = &mut self.v5.x.cam;
                c.dx = g("dx", c.dx as i64) as i32;
                c.dy = g("dy", c.dy as i64) as i32;
                c.dz = g("dz", c.dz as i64) as i32;
                c.count = g("count", c.count as i64).max(1) as usize;
                c.slow_in = g("slowin", 0) as usize;
                c.slow_out = g("slowout", 0) as usize;
                c.wrap = g("wrap", 1) == 1;
                c.start = (g("startx", 0) as i32, g("starty", 0) as i32);
            }
            Err(e) => self.msg("Load Camera", format!("Can't Load: {e}")),
        }
    }

    // ---------------------------------------------------------------- Move

    /// Move: 1 = Trails (every step on this frame), 2 = Fill (add frames as needed), 3 = Draw.
    pub fn do_move_v5(&mut self, how: u8) {
        self.do_move_3d(how);
    }

    pub(crate) fn save_move(&mut self) {
        let mp = &self.move_params;
        let x = &self.v5.x;
        let text = kv_text(&[
            ("distx", mp.dist[0].to_string()),
            ("disty", mp.dist[1].to_string()),
            ("distz", mp.dist[2].to_string()),
            ("anglex", mp.angle[0].to_string()),
            ("angley", mp.angle[1].to_string()),
            ("anglez", mp.angle[2].to_string()),
            ("count", mp.frames.to_string()),
            ("slowin", mp.slow_in.to_string()),
            ("slowout", mp.slow_out.to_string()),
            ("cyclic", (mp.cyclic as u8).to_string()),
            ("cometo", (x.move_come_to as u8).to_string()),
            ("record", x.move_record.to_string()),
            ("fadestart", x.move_fade.0.to_string()),
            ("fadeend", x.move_fade.1.to_string()),
        ]);
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Save Move")
            .set_file_name("untitled.move")
            .add_filter("DeluxePaint Move (IFF)", &["move"])
            .add_filter("Move with fade settings (text)", &["mov"])
            .save_file()
        {
            // .move is DeluxePaint's own format (no fade); .mov is ours, with the fade too.
            let is_text = p.extension().map(|e| e.to_string_lossy().to_lowercase() == "mov").unwrap_or(false);
            let data = if is_text {
                text.into_bytes()
            } else {
                crate::amigafiles::write_move(&crate::amigafiles::MoveFile {
                    cyclic: mp.cyclic,
                    record: x.move_record,
                    go_from: !x.move_come_to,
                    dist: mp.dist,
                    angle: mp.angle.map(|a| a.round() as i32),
                    count: mp.frames,
                    slow_in: mp.slow_in,
                    slow_out: mp.slow_out,
                })
            };
            if let Err(e) = std::fs::write(&p, data) {
                self.msg("Save Move", format!("Can't Save: {e}"));
            }
        }
    }

    pub(crate) fn load_move(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .set_title("Load Move")
            .add_filter("Move settings", &["move", "mov"])
            .add_filter("All files", &["*"])
            .pick_file()
        else {
            return;
        };
        // DeluxePaint's .MOVE files (IFF form PRSP).
        if let Some(mf) = std::fs::read(&p).ok().and_then(|d| crate::amigafiles::read_move(&d)) {
            let mp = &mut self.move_params;
            mp.dist = mf.dist;
            mp.angle = mf.angle.map(|a| a as f32);
            mp.frames = mf.count;
            mp.slow_in = mf.slow_in;
            mp.slow_out = mf.slow_out;
            mp.cyclic = mf.cyclic;
            self.v5.x.move_come_to = !mf.go_from;
            self.v5.x.move_record = mf.record;
            return;
        }
        let Ok(t) = std::fs::read_to_string(&p) else {
            self.msg("Load Move", "Not a valid MOVE template");
            return;
        };
        let m = kv_parse(&t);
        if !m.contains_key("count") {
            self.msg("Load Move", "Not a valid MOVE template");
            return;
        }
        let f = |k: &str| m.get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        let mp = &mut self.move_params;
        mp.dist = [f("distx") as i32, f("disty") as i32, f("distz") as i32];
        mp.angle = [f("anglex"), f("angley"), f("anglez")];
        mp.frames = (f("count") as usize).max(1);
        mp.slow_in = f("slowin") as usize;
        mp.slow_out = f("slowout") as usize;
        mp.cyclic = f("cyclic") != 0.0;
        self.v5.x.move_come_to = f("cometo") != 0.0;
        self.v5.x.move_record = f("record") as i32;
        self.v5.x.move_fade = (f("fadestart") as u8, f("fadeend") as u8);
    }

    // ---------------------------------------------------------------- fill extras

    /// The Fill tool's area: same colour, or (Fill to Background) everything up to the background colour.
    pub fn fill_region(&self, img: &Image, x: i32, y: i32) -> Option<Vec<bool>> {
        if self.v5.x.fill_to_bg {
            region_to_bg(img, x, y, self.bg)
        } else {
            pix::flood_mask(img, x, y, 64_000_000)
        }
    }

    /// Fill the same point in every frame.
    pub fn anim_fill(&mut self, pos: (i32, i32), color: u8) {
        let start = self.doc.cur;
        let b = self.brush.clone();
        for i in 0..self.doc.frames.len() {
            self.doc.cur = i;
            let img = self.doc.frames[i].clone();
            if let Some(mask) = self.fill_region(&img, pos.0, pos.1) {
                self.doc.checkpoint();
                let mut rng = Rng::new(31 + i as u64);
                paint::fill_mask(&mut self.doc, &mask, &self.set, color, &b, &mut rng);
            }
        }
        self.doc.cur = start;
    }

    // ---------------------------------------------------------------- perspective mode

    pub fn enter_persp(&mut self) {
        if !self.brush.custom {
            self.msg("Perspective", "Must have a user-defined brush.");
            return;
        }
        self.v5.x.persp_base = Some(self.brush.clone());
        self.v5.x.persp_mode = true;
        self.v5.x.p3.last_key = None;
        self.v5.x.p3.floor = None;
        // AutoGrid (Prefs): the perspective grid takes the new brush's size; the brush keeps the
        // orientation it had last time, so a new brush lands on the same plane.
        if self.v5.prefs.auto_grid {
            let (w, h) = (self.brush.img.w.max(1), self.brush.img.h.max(1));
            self.v5.x.p3.grid = [w, h, w.max(h)];
        }
        if self.v5.persp_centre.is_none() {
            self.v5.persp_centre = Some((self.doc.w / 2, self.doc.h / 2));
        }
        self.tool = Tool::Dotted;
        self.status = "Perspective: 7/8 4/5 1/2 turn X Y Z (Shift: by the angle step), 9/6/3 reset an axis, 0 all; \" and : move in depth; . centre; Esc ends".into();
    }

    pub fn exit_persp(&mut self) {
        // The brush keeps its last perspective form, as after Perspective > Do.
        self.v5.x.persp_mode = false;
        self.v5.x.persp_base = None;
        self.v5.x.p3.floor = None;
        self.status.clear();
    }

    /// Eye distance in pixels (Perspective depth: 2 = the page's size).
    pub fn persp_eye(&self) -> f32 {
        self.v5.x.persp_depth.max(0.3) * self.doc.w.max(self.doc.h) as f32 / 2.0
    }

    pub fn persp_centre_f(&self) -> (f32, f32) {
        let (cx, cy) = self.v5.persp_centre.unwrap_or((self.doc.w / 2, self.doc.h / 2));
        (cx as f32, cy as f32)
    }

    /// Where the brush handle is in 3-D: the pointer, or (Ctrl held) on the floor.
    pub fn persp_handle(&self) -> Option<(f32, f32, f32)> {
        let (x, y) = self.cursor?;
        let p3 = &self.v5.x.p3;
        // The grid passes through the grid point (Shift+G sets it at the pointer).
        let snap = |v: i32, g: i32, o: i32| if self.set.grid_on && g > 1 { ((v - o) as f32 / g as f32).round() as i32 * g + o } else { v };
        let y = match p3.floor {
            Some((y0, _)) => y0,
            None => y,
        };
        let (ox, oy) = (self.set.grid.2, self.set.grid.3);
        let (fx, fy) = (p3.fixed[0].unwrap_or(snap(x, p3.grid[0], ox) as f32), p3.fixed[1].unwrap_or(snap(y, p3.grid[1], oy) as f32));
        Some((fx, fy, p3.fixed[2].unwrap_or(p3.z)))
    }

    /// Redraw the brush in perspective when the pointer or the 3-D settings changed (each frame).
    pub fn persp_update(&mut self, ctx: &egui::Context) {
        if !self.v5.x.persp_mode {
            return;
        }
        let Some(cur) = self.cursor else { return };
        // Ctrl + mouse: up moves the brush away, down brings it nearer, on the floor.
        let ctrl = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
        if ctrl {
            match self.v5.x.p3.floor {
                None => self.v5.x.p3.floor = Some((cur.1, self.v5.x.p3.z)),
                Some((y0, z0)) => {
                    let step = self.v5.x.p3.grid[2].max(1) as f32;
                    let z = z0 + ((y0 - cur.1) as f32 * 4.0 / step).round() * step;
                    self.v5.x.p3.z = z.max(-self.persp_eye() + 20.0);
                }
            }
        } else {
            self.v5.x.p3.floor = None;
        }
        let Some(h) = self.persp_handle() else { return };
        let (c, eye) = (self.persp_centre_f(), self.persp_eye());
        let p3 = &self.v5.x.p3;
        let rk: [i32; 9] = std::array::from_fn(|i| (p3.rot[i / 3][i % 3] * 10000.0) as i32);
        let key = (cur.0, cur.1, (h.2 * 10.0) as i32, rk, (c.0 * 10.0 + c.1 * 100000.0) as i32, eye as i32, h.1 as i32);
        if p3.last_key == Some(key) {
            return;
        }
        self.v5.x.p3.last_key = Some(key);
        let Some(base) = self.v5.x.persp_base.clone() else { return };
        // Keep Fill Screen and the old Perspective settings in step with the angles.
        let a = self.v5.x.p3.angles;
        self.persp = (a[0], a[1], a[2]);
        if self.v5.x.p3.flat() {
            self.brush = base;
            return;
        }
        let hm = crate::persp3d::homography(&self.v5.x.p3.rot, h, c, eye);
        if let Some(b) = crate::persp3d::project(&base, &hm, cur, (self.doc.w, self.doc.h)) {
            self.brush = if self.antialias { b.antialiased(&self.doc.palette) } else { b };
        }
    }

    /// The menu bar's read-out in perspective mode: the angles, or the 3-D position.
    pub fn persp_readout(&self) -> Option<String> {
        if !self.v5.x.persp_mode {
            return None;
        }
        let p3 = &self.v5.x.p3;
        Some(if p3.show_pos {
            let (x, y, z) = self.persp_handle().unwrap_or((0.0, 0.0, p3.z));
            let y = if self.v5.prefs.origin_ul { y } else { self.doc.h as f32 - 1.0 - y };
            format!("{:>5} {:>5} {:>5}", x as i32, y as i32, z as i32)
        } else {
            format!("{:>4} {:>4} {:>4}", p3.angles[0].round() as i32, p3.angles[1].round() as i32, p3.angles[2].round() as i32)
        })
    }

    /// Number keys in perspective mode (also with Shift). Returns true when used.
    pub fn persp_digit(&mut self, d: u8, shift: bool) -> bool {
        if !self.v5.x.persp_mode {
            return false;
        }
        // Shift + 9 / 6 / 3: fix the X / Y / Z axis (the brush can't move along it); Shift+0 resets all.
        if shift && matches!(d, 9 | 6 | 3 | 0) {
            if d == 0 {
                let centre = self.v5.persp_centre;
                let depth = crate::v52::V52::default().persp_depth;
                self.v5.x.p3 = crate::persp3d::Persp3::default();
                self.v5.x.persp_depth = depth;
                self.v5.persp_centre = centre;
                self.status = "Perspective: everything reset".into();
            } else {
                let axis = match d {
                    9 => 0,
                    6 => 1,
                    _ => 2,
                };
                let h = self.persp_handle().unwrap_or((0.0, 0.0, self.v5.x.p3.z));
                let v = [h.0, h.1, h.2][axis];
                let f = &mut self.v5.x.p3.fixed[axis];
                *f = if f.is_some() { None } else { Some(v) };
                self.status = format!("{} axis {}", ["X", "Y", "Z"][axis], if f.is_some() { "fixed" } else { "free" });
            }
            self.v5.x.p3.last_key = None;
            return true;
        }
        let step = if shift { self.v5.x.p3.step } else { 1.0 };
        let p3 = &mut self.v5.x.p3;
        match d {
            7 => p3.turn(0, -step),
            8 => p3.turn(0, step),
            9 => p3.reset_axis(0),
            4 => p3.turn(1, -step),
            5 => p3.turn(1, step),
            6 => p3.reset_axis(1),
            1 => p3.turn(2, -step),
            2 => p3.turn(2, step),
            3 => p3.reset_axis(2),
            // 0: angles back to 0; the distance stays.
            0 => p3.reset_angles(),
            _ => return false,
        }
        p3.skip_char = shift;
        true
    }

    /// Other keys while in perspective mode. Returns true when the key was used.
    pub fn persp_char(&mut self, ch: char) -> bool {
        if !self.v5.x.persp_mode {
            return false;
        }
        if std::mem::take(&mut self.v5.x.p3.skip_char) {
            return true;
        }
        let (big, small) = self.persp_z_steps();
        let min_z = -self.persp_eye() * 0.9;
        match ch {
            // Digits come as keys (persp_digit); their characters are used up here.
            '0'..='9' => {}
            // " (Shift) moves the brush back along the Z axis: smaller, towards the centre;
            // : (Shift) brings it forward again. ' and ; take small steps.
            '"' => self.v5.x.p3.z += big,
            '\'' => self.v5.x.p3.z += small,
            ':' => self.v5.x.p3.z = (self.v5.x.p3.z - big).max(min_z),
            ';' => self.v5.x.p3.z = (self.v5.x.p3.z - small).max(min_z),
            '\\' => self.v5.x.p3.show_pos = !self.v5.x.p3.show_pos,
            '.' | ',' => {
                if let Some(c) = self.cursor {
                    self.v5.persp_centre = Some(c);
                    self.status = format!("Perspective centre at {}, {}", c.0, c.1);
                }
            }
            '-' => {
                if let Some(base) = self.v5.x.persp_base.clone() {
                    let keep = std::mem::replace(&mut self.brush, base);
                    self.cmd_fill_screen();
                    self.brush = keep;
                }
            }
            '<' => self.v5.x.persp_depth = (self.v5.x.persp_depth * 1.25).min(20.0),
            '>' => self.v5.x.persp_depth = (self.v5.x.persp_depth / 1.25).max(0.3),
            _ => return false,
        }
        self.v5.x.p3.last_key = None;
        true
    }

    /// Depth steps of " / : and ' / ;: a tenth and a fiftieth of the eye distance (so each press
    /// visibly shrinks or grows the brush), in whole grid-Z steps when the grid's Z is set.
    pub fn persp_z_steps(&self) -> (f32, f32) {
        let eye = self.persp_eye();
        let g = self.v5.x.p3.grid[2].max(1) as f32;
        let snap = |v: f32| ((v / g).round() * g).max(g);
        (snap(eye * 0.1), snap(eye * 0.02))
    }

    /// The plane for Perspective fills: the brush card's plane (perspective mode: where the brush
    /// is; otherwise the Perspective angles with the handle low in the middle of the page).
    pub fn update_persp_plane(&mut self) {
        if self.set.fill != crate::paint::FillKind::Perspective && !self.v5.x.persp_mode {
            self.set.persp_plane = None;
            return;
        }
        let base = self.v5.x.persp_base.clone().unwrap_or_else(|| self.brush.clone());
        if !base.custom {
            self.set.persp_plane = None;
            return;
        }
        let rot = if self.v5.x.persp_mode { self.v5.x.p3.rot } else { crate::persp3d::from_angles([self.persp.0, self.persp.1, self.persp.2]) };
        let h = match (self.v5.x.persp_mode, self.persp_handle()) {
            (true, Some(h)) => h,
            _ => (self.doc.w as f32 / 2.0, self.doc.h as f32 * 0.75, self.v5.x.p3.z),
        };
        let (c, eye) = (self.persp_centre_f(), self.persp_eye());
        let hm = crate::persp3d::homography(&rot, h, c, eye);
        let Some(inv) = crate::persp3d::invert(&hm) else {
            self.set.persp_plane = None;
            return;
        };
        let same = self.set.persp_plane.as_ref().map(|p| p.inv == inv && p.brush.img == base.img && (p.brush.hx, p.brush.hy) == (base.hx, base.hy)).unwrap_or(false);
        if !same {
            self.set.persp_plane = Some(std::sync::Arc::new(crate::paint::PerspPlane { inv, depth_row: hm[2], eye, brush: base }));
        }
    }

    pub(crate) fn cmd_fill_screen(&mut self) {
        if self.v5.x.persp_mode {
            let a = self.v5.x.p3.angles;
            self.persp = (a[0], a[1], a[2]);
        }
        self.doc.checkpoint();
        let mut set = self.set.clone();
        set.fill = crate::paint::FillKind::Perspective;
        set.persp = self.persp;
        let keep = self.set.fill;
        self.set.fill = crate::paint::FillKind::Perspective;
        self.update_persp_plane();
        set.persp_plane = self.set.persp_plane.clone();
        self.set.fill = keep;
        let mask = vec![true; (self.doc.w * self.doc.h) as usize];
        let b = self.brush.clone();
        let mut rng = Rng::new(3);
        paint::fill_mask(&mut self.doc, &mask, &set, self.fg, &b, &mut rng);
    }

    // ---------------------------------------------------------------- loading with questions

    /// Load from the Load requester: may ask about the screen format, stretching and appending.
    pub fn open_path_ask(&mut self, p: PathBuf) {
        match crate::fileio::load(&p) {
            Ok(crate::fileio::LoadResult::Indexed(l)) => {
                let (w, h) = (l.frames[0].w, l.frames[0].h);
                let is_anim = l.frames.len() > 1;
                if is_anim && self.v5.x.load_append {
                    if (w, h) != (self.doc.w, self.doc.h) {
                        self.msg("Load Anim", "Can't append incompatible Anims.");
                    } else {
                        let pal = self.doc.palette;
                        let map: Vec<u8> = (0..256).map(|i| nearest(&pal, [l.palette[i][0] as i32, l.palette[i][1] as i32, l.palette[i][2] as i32], 256)).collect();
                        for f in l.frames {
                            let mut f = f;
                            f.px.iter_mut().for_each(|v| *v = map[*v as usize]);
                            self.doc.frames.push(f);
                        }
                        self.doc.sync_palettes();
                        self.doc.dirty = true;
                        self.status = format!("Appended; the animation now has {} frames", self.doc.frames.len());
                    }
                    return;
                }
                let same = (w, h) == self.v5.screen;
                match (same, self.v5.x.format_policy) {
                    (true, _) | (false, 1) => self.open_path(p),
                    (false, 2) => self.v5.x.ask = Some(LoadAsk { path: p, loaded: l, stage: 1 }),
                    _ => self.v5.x.ask = Some(LoadAsk { path: p, loaded: l, stage: 0 }),
                }
            }
            Ok(crate::fileio::LoadResult::TrueColor(_)) => self.open_path(p),
            Err(e) => self.msg("Can't Load", e),
        }
    }

    fn load_ask_req(&mut self, ctx: &egui::Context) {
        let Some(ask) = &self.v5.x.ask else { return };
        let (w, h) = (ask.loaded.frames[0].w, ask.loaded.frames[0].h);
        let stage = ask.stage;
        let bigger = w > self.doc.w || h > self.doc.h;
        let mut answer = None;
        req("System Message").show(ctx, |ui| {
            if stage == 0 {
                ui.label("Do you want to change format to");
                ui.label(format!("that of the file? ({w} x {h})"));
                ui.horizontal(|ui| {
                    if button(ui, "Change").clicked() {
                        answer = Some(0);
                    }
                    if button(ui, "Keep Same").clicked() {
                        answer = Some(1);
                    }
                    if button(ui, "Cancel").clicked() {
                        answer = Some(9);
                    }
                });
            } else {
                ui.label("Do you want the image");
                ui.label(if bigger { "shrunk to fit the page?" } else { "stretched to fit the page?" });
                ui.horizontal(|ui| {
                    if button(ui, "Yes").clicked() {
                        answer = Some(2);
                    }
                    if button(ui, "No").clicked() {
                        answer = Some(3);
                    }
                });
            }
        });
        match answer {
            Some(0) => {
                let a = self.v5.x.ask.take().unwrap();
                self.open_path(a.path);
            }
            Some(1) => {
                if let Some(a) = &mut self.v5.x.ask {
                    a.stage = 1;
                }
            }
            Some(2) | Some(3) => {
                let a = self.v5.x.ask.take().unwrap();
                let stretch = answer == Some(2);
                let (pw, ph) = (self.doc.w, self.doc.h);
                let mut d = Document::new(pw, ph, 0);
                d.frames = a.loaded.frames.iter().map(|f| if stretch { f.scaled(pw, ph) } else { f.resized_canvas(pw, ph, 0) }).collect();
                d.palette = a.loaded.palette;
                d.saved_palette = a.loaded.palette;
                if let Some(r) = a.loaded.ranges {
                    d.ranges = r;
                }
                if let Some(f) = a.loaded.fps {
                    d.fps = f;
                }
                d.sync_palettes();
                d.path = Some(a.path);
                d.colors = self.doc.colors;
                self.doc = d;
                self.fit_pending = true;
            }
            Some(9) => self.v5.x.ask = None,
            _ => {}
        }
    }

    fn load_opts_req(&mut self, ctx: &egui::Context) {
        if !self.v5.x.show_load_opts {
            return;
        }
        let mut done = false;
        req("Load Options").show(ctx, |ui| {
            let x = &mut self.v5.x;
            ui.label("Screen/Page Size when a file has another size:");
            ui.horizontal(|ui| {
                for (v, l) in [(0u8, "Ask"), (1, "Change to the file's"), (2, "Keep Same")] {
                    if toggle(ui, l, x.format_policy == v).clicked() {
                        x.format_policy = v;
                    }
                }
            });
            ui.checkbox(&mut x.load_append, "Append animations to the current one");
            if button(ui, "Ok").clicked() {
                done = true;
            }
        });
        if done {
            self.v5.x.show_load_opts = false;
        }
    }

    // ---------------------------------------------------------------- picture sequences

    pub fn open_sequence(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Load Sequence (choose the first picture)")
            .add_filter("Pictures", &["lbm", "ilbm", "iff", "pcx", "bmp", "png", "jpg", "jpeg"])
            .pick_file()
        {
            self.v5.x.seq_first = Some(p);
            self.v5.x.show_seq = true;
        }
    }

    fn seq_req(&mut self, ctx: &egui::Context) {
        if !self.v5.x.show_seq {
            return;
        }
        let mut done = None;
        let first = self.v5.x.seq_first.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        req("Load Picture Options").show(ctx, |ui| {
            ui.label(format!("First picture: {first}"));
            ui.horizontal(|ui| {
                ui.label("Load # of Frames:");
                ui.add(egui::DragValue::new(&mut self.v5.x.seq_count).range(1..=999));
            });
            ui.label("The number in the file name counts up (pic001, pic002, ...).");
            ui.horizontal(|ui| {
                if button(ui, "Load").clicked() {
                    done = Some(true);
                }
                if button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            self.v5.x.show_seq = false;
            if ok {
                if let Some(p) = self.v5.x.seq_first.clone() {
                    let n = self.v5.x.seq_count;
                    if let Err(e) = self.load_sequence(&p, n) {
                        self.msg("Load Sequence", e);
                    }
                }
            }
        }
    }

    /// Load numbered pictures (name0001.lbm, name0002.lbm, ...) into animation frames.
    pub fn load_sequence(&mut self, first: &Path, count: usize) -> Result<usize, String> {
        let stem = first.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = first.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
        let digits: String = stem.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
        if digits.is_empty() {
            return Err("The file name must end in a number (for example pic001).".into());
        }
        let base = &stem[..stem.len() - digits.len()];
        let start: usize = digits.parse().unwrap_or(0);
        let dir = first.parent().unwrap_or(Path::new("."));
        let mut frames: Vec<Image> = Vec::new();
        let mut pal = None;
        for k in 0..count {
            let name = format!("{base}{:0width$}.{ext}", start + k, width = digits.len());
            let p = dir.join(&name);
            if !p.exists() {
                break;
            }
            match crate::fileio::load(&p)? {
                crate::fileio::LoadResult::Indexed(l) => {
                    if let Some(f) = frames.first() {
                        if (l.frames[0].w, l.frames[0].h) != (f.w, f.h) {
                            return Err("Can't load different sized picture as anim frame".into());
                        }
                    }
                    if pal.is_none() {
                        pal = Some(l.palette);
                    }
                    frames.push(l.frames[0].clone());
                }
                crate::fileio::LoadResult::TrueColor(_) => return Err(format!("{name} is not a 256-colour picture.")),
            }
        }
        if frames.is_empty() {
            return Err("No pictures found.".into());
        }
        let n = frames.len();
        let mut d = Document::new(frames[0].w, frames[0].h, 0);
        d.frames = frames;
        d.palette = pal.unwrap();
        d.saved_palette = d.palette;
        d.sync_palettes();
        d.path = Some(first.to_path_buf());
        self.v5.screen = (d.w, d.h);
        self.doc = d;
        self.fit_pending = true;
        self.status = format!("Loaded {n} frames");
        Ok(n)
    }

    // ---------------------------------------------------------------- ARexx command scripts

    fn script_log_req(&mut self, ctx: &egui::Context) {
        if !self.v5.x.show_script_log {
            return;
        }
        let mut open = true;
        req("Macro Output").open(&mut open).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for l in &self.v5.x.script_log {
                    ui.monospace(l);
                }
            });
        });
        self.v5.x.show_script_log = open;
    }

    /// Run a DeluxePaint ARexx command script (one command per line; REXX statements are skipped).
    pub fn run_script(&mut self, ctx: &egui::Context, text: &str) -> Vec<String> {
        let mut log = Vec::new();
        let mut clean = String::new();
        let mut rest = text;
        while let Some(i) = rest.find("/*") {
            clean.push_str(&rest[..i]);
            rest = match rest[i + 2..].find("*/") {
                Some(j) => &rest[i + 2 + j + 2..],
                None => "",
            };
        }
        clean.push_str(rest);
        for (ln, raw) in clean.lines().enumerate() {
            let line = raw.trim().trim_end_matches(';').trim();
            if line.is_empty() {
                continue;
            }
            // A quoted command line ('SETFGCOLOR 3') is sent as it is.
            let line = line.trim_matches(|c| c == '\'' || c == '"');
            let toks = tokenize(line);
            if toks.is_empty() {
                continue;
            }
            let cmd = toks[0].to_uppercase();
            let rexx = matches!(cmd.as_str(), "ADDRESS" | "OPTIONS" | "SAY" | "IF" | "THEN" | "ELSE" | "DO" | "END" | "EXIT" | "SIGNAL" | "CALL" | "PARSE");
            if rexx || toks.get(1).map(|t| t == "=").unwrap_or(false) {
                continue;
            }
            match self.script_command(ctx, &cmd, &toks[1..]) {
                Ok(Some(result)) => log.push(format!("{:>3}: {} -> {}", ln + 1, cmd, result)),
                Ok(None) => {}
                Err(e) => log.push(format!("{:>3}: {} — {}", ln + 1, cmd, e)),
            }
        }
        if !log.is_empty() {
            self.v5.x.script_log = log.clone();
            self.v5.x.show_script_log = true;
        }
        log
    }

    pub fn script_command(&mut self, ctx: &egui::Context, cmd: &str, args: &[String]) -> Result<Option<String>, String> {
        use crate::actions::Cmd as C;
        let a = Args(args);
        let onoff = |cur: bool| -> bool {
            if a.has("ON") {
                true
            } else if a.has("OFF") {
                false
            } else {
                !cur
            }
        };
        let file = || -> Result<PathBuf, String> { a.string("FILE").or_else(|| a.pos(0)).map(PathBuf::from).ok_or("FILE is missing".to_string()) };
        let frames_arg = |app: &App| -> Vec<usize> {
            let n = app.doc.frames.len();
            if a.has("ALL") {
                (0..n).collect()
            } else if a.has("RANGE") || a.num("START").is_some() {
                let s = a.num("START").unwrap_or(1).max(1) as usize;
                let e = a.num("END").unwrap_or(n as i64).max(1) as usize;
                scope_frames(Scope::Range, (s, e), app.doc.cur, n)
            } else {
                vec![app.doc.cur]
            }
        };
        let simple: Option<C> = match cmd {
            "ABOUT" => Some(C::About),
            "BRUSHBGTOFG" => Some(C::BrushBgToFg),
            "SWAPBRUSHBGFG" => Some(C::BrushBgSwapFg),
            "CHANGEBRUSHTRANS" => Some(C::BrushTransparency),
            "CLOSEMEDIA" => Some(C::MediaClose),
            "COPYBRUSHTOSPARE" => Some(C::BrushToSpare),
            "SWAPBRUSHTOSPARE" => Some(C::BrushSwapSpare),
            "COPYPICTOSPARE" => Some(C::SpareCopy),
            "SWAPPICTOSPARE" => Some(C::SpareSwap),
            "DEFAULTPALETTE" => Some(C::DefaultPalette),
            "DELETEPAGE" => Some(C::SpareDelete),
            "DOUBLEBRUSH" => Some(C::BrushDouble),
            "DOUBLEBRUSHX" => Some(C::BrushDoubleH),
            "DOUBLEBRUSHY" => Some(C::BrushDoubleV),
            "HALFBRUSH" => Some(C::BrushHalve),
            "FIXBG" => Some(C::BgFix),
            "FREEBG" => Some(C::BgFree),
            "FLIPBRUSHX" => Some(C::BrushFlipH),
            "FLIPBRUSHY" => Some(C::BrushFlipV),
            "FREEANIMBRUSH" => Some(C::AnimBrushFree),
            "FREEBRUSH" => Some(C::BrushFree),
            "FREESTENCIL" => Some(C::StencilFree),
            "FREETEXTURE" => Some(C::TextureFree),
            "INVERTTEXTURE" => Some(C::TextureInvert),
            "LIGHTMERGE" => Some(C::LightTableMerge),
            "LOCKFG" => Some(C::LockFg),
            "MERGEINBACK" => Some(C::MergeBack),
            "MERGEINFRONT" => Some(C::MergeFront),
            "NEXTFRAME" => Some(C::NextFrame),
            "PREVFRAME" => Some(C::PrevFrame),
            "OUTLINEBRUSH" => Some(C::BrushOutline),
            "TRIMBRUSH" => Some(C::BrushTrim),
            "PASTEBRUSH" => Some(C::BrushPaste),
            "PERSPFILLSCREEN" => Some(C::PerspFillScreen),
            "RESETPERSP" => Some(C::PerspReset),
            "PLAYFORWARD" => Some(C::PlayOnce),
            "PLAYLOOPFWD" => Some(C::Play),
            "PLAYPING" => Some(C::PingPong),
            "STOPANIM" => Some(C::Stop),
            "REMAKESTENCIL" => Some(C::StencilRemake),
            "REVERSESTENCIL" => Some(C::StencilReverse),
            "SHOWSTENCIL" => Some(C::StencilShow),
            "REMAPBRUSH" => Some(C::BrushRemap),
            "RESTOREBRUSH" => Some(C::BrushRestore),
            "RESTOREPALETTE" => Some(C::RestorePalette),
            "ROTATEBRUSH90" => Some(C::BrushRot90),
            "ROTATEBRUSHANYANGLE" | "SHEARBRUSH" | "BENDBRUSHX" | "BENDBRUSHY" => Some(C::BrushRotAny),
            "ROTATEHANDLE" => Some(C::HandleRotate),
            "SHOWFULLPAGE" => Some(C::ShowPage),
            "UNDO" => Some(C::Undo),
            "USEANIMBRUSH" => Some(C::AnimBrushUse),
            "USEBRUSHPALETTE" => Some(C::UseBrushPalette),
            "RESETSETTINGS" => Some(C::PrefsDefault),
            "RESTORESETTINGS" => Some(C::PrefsLastSaved),
            "SAVESETTINGS" => Some(C::PrefsSave),
            "SETTINT" => Some(C::ProcessTint),
            "SETHUE" => Some(C::ProcessHue),
            "SETVALUE" => Some(C::ProcessValue),
            _ => None,
        };
        if let Some(c) = simple {
            self.cmd(ctx, c);
            return Ok(None);
        }
        let set_mode = match cmd {
            "MATTE" => Some(Mode::Paint),
            "COLOR" => Some(Mode::SingleColor),
            "REPLACE" => Some(Mode::Replace),
            "SMEAR" => Some(Mode::Smear),
            "SHADE" => Some(Mode::Shade),
            "BLEND" => Some(Mode::Blend),
            "CYCLE" => Some(Mode::Cycle),
            "SMOOTH" => Some(Mode::Smooth),
            "HBRITE" => Some(Mode::HBrite),
            "RUBTHRU" => Some(Mode::Rubthru),
            _ => None,
        };
        if let Some(m) = set_mode {
            self.set_mode(m);
            return Ok(None);
        }
        match cmd {
            "CAMERAMOVE" => self.v5.x.show_cam = true,
            "PRINT" => self.with_scope(ScopeOp::Print),
            // ---- switches
            "ANIMCONTROLS" | "PANEL" => self.v5.show_anim_panel = onoff(self.v5.show_anim_panel),
            "ANTIALIAS" => {
                self.antialias = !a.has("OFF");
                self.aa_high = a.has("HIGH");
            }
            "ARRANGEPALETTE" => self.v5.show_arrange = onoff(self.v5.show_arrange),
            "AUTOGRID" => self.v5.prefs.auto_grid = onoff(self.v5.prefs.auto_grid),
            "AUTOTRANSP" => self.v5.prefs.auto_transp = onoff(self.v5.prefs.auto_transp),
            "BESQUARE" => self.v5.prefs.be_square = onoff(self.v5.prefs.be_square),
            "COLORCYCLE" => self.cycling = onoff(self.cycling),
            "COLORMIXER" => self.show_palette = onoff(self.show_palette),
            "COORDS" => self.v5.prefs.coords = onoff(self.v5.prefs.coords),
            "CURSOR" => self.v5.x.crosshair = onoff(self.v5.x.crosshair),
            "EXCLBRUSH" => self.v5.prefs.excl_brush = onoff(self.v5.prefs.excl_brush),
            "FASTFB" => self.v5.prefs.fast_fb = onoff(self.v5.prefs.fast_fb),
            "FRAMEONLY" => self.v5.frame_only = onoff(self.v5.frame_only),
            "SOLIDBODY" => self.v5.frame_only = !onoff(!self.v5.frame_only),
            "MOUSETRACK" => self.v5.prefs.mouse_track = onoff(self.v5.prefs.mouse_track),
            "MULTICYCLE" => self.v5.prefs.multi_cycle = onoff(self.v5.prefs.multi_cycle),
            "ORIGINUL" => self.v5.prefs.origin_ul = onoff(self.v5.prefs.origin_ul),
            "PAINTSTENCIL" => self.set.stencil_paint = onoff(self.set.stencil_paint),
            "PRESSURE" => self.v5.pressure_on = onoff(self.v5.pressure_on),
            "PROCESS" => self.set.process_on = onoff(self.set.process_on),
            "STENCIL" => {
                let on = onoff(self.doc.stencil.as_ref().map(|s| s.on).unwrap_or(false));
                if let Some(s) = &mut self.doc.stencil {
                    s.on = on;
                }
            }
            "SYMMETRY" => self.set.sym.on = onoff(self.set.sym.on),
            "TEXTURE" => self.set.texture_on = onoff(self.set.texture_on) && self.set.texture.is_some(),
            "TITLE" => self.v5.x.title = onoff(self.v5.x.title),
            "TRANSLUCENCY" => self.set.translucent_on = onoff(self.set.translucent_on),
            "WORKBENCH" => {
                let on = onoff(self.v5.prefs.workbench);
                if on != self.v5.prefs.workbench {
                    self.cmd(ctx, C::PrefWorkbench);
                }
            }
            "FASTADJUST" | "ANIMCOMPRESSED" | "ANIMEXPANDED" | "LOCKGUI" | "UNLOCKGUI" | "SETICONS" | "MBUTTONS" | "RECOMPUTE" => {}
            "SCREENTOBACK" => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            "SCREENTOFRONT" => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            // ---- tools
            "DOTTED" => self.tool = Tool::Dotted,
            "AIRBRUSH" => self.tool = Tool::Airbrush,
            "FREEHAND" => {
                self.tool = Tool::Freehand;
                // OUTLINE: a filled shape whose edge the current brush traces (key i).
                self.filled = a.has("FILL") || a.has("OUTLINE");
                self.v5.x.fill_outline = a.has("OUTLINE");
            }
            "ENTERPERSP" => self.enter_persp(),
            "EXITPERSP" => self.exit_persp(),
            "ZOOM" => {
                if a.has("OUT") {
                    self.mag_zoom = (self.mag_zoom - 1.0).max(2.0);
                } else {
                    self.magnify = true;
                    self.mag_zoom = (self.mag_zoom + 1.0).min(24.0);
                }
            }
            // ---- colours and pens
            "SETFGCOLOR" => self.fg = a.num("COLNUM").or_else(|| a.posnum(0)).ok_or("COLNUM is missing")?.clamp(0, 255) as u8,
            "SETBGCOLOR" => self.bg = a.num("COLNUM").or_else(|| a.posnum(0)).ok_or("COLNUM is missing")?.clamp(0, 255) as u8,
            "GETFGCOLOR" => return Ok(Some(self.fg.to_string())),
            "GETBGCOLOR" => return Ok(Some(self.bg.to_string())),
            "CLOSESTCOLOR" => {
                let r = a.num("RED").or_else(|| a.posnum(0)).unwrap_or(0) as i32;
                let g = a.num("GREEN").or_else(|| a.posnum(1)).unwrap_or(0) as i32;
                let b = a.num("BLUE").or_else(|| a.posnum(2)).unwrap_or(0) as i32;
                return Ok(Some(nearest(&self.doc.palette, [r, g, b], 256).to_string()));
            }
            "SETPEN" => {
                let n = a.num("PENNUM").or_else(|| a.posnum(0)).unwrap_or(0);
                let shape = match n {
                    0..=3 => BuiltinShape::Round,
                    4..=7 => BuiltinShape::Square,
                    _ => BuiltinShape::Dither,
                };
                let size = self.builtin.1;
                self.set_builtin(shape, size);
            }
            "SETPENSIZE" => {
                let s = a.num("SIZE").or_else(|| a.posnum(0)).unwrap_or(1) as i32;
                let shape = self.builtin.0;
                self.set_builtin(shape, s);
            }
            "GETPENSIZE" => return Ok(Some(self.builtin.1.to_string())),
            "SETTRANSLUCENCY" => self.set.translucent = a.num("LEVEL").or_else(|| a.posnum(0)).unwrap_or(50).clamp(0, 99) as u8,
            "GETTRANSLUCENCY" => return Ok(Some(self.set.translucent.to_string())),
            "SETAIRBRUSH" => {
                if a.has("OLD") {
                    self.set.airbrush_old = true;
                }
                if a.has("STANDARD") {
                    self.set.airbrush_old = false;
                }
                if let Some(r) = a.num("RADIUS") {
                    self.set.airbrush_radius = r.clamp(1, 256) as i32;
                }
                if let Some(f) = a.num("FLOW") {
                    self.set.airbrush_density = f.clamp(1, 100) as i32;
                }
            }
            "GETAIRBRUSH" => return Ok(Some(format!("{} {} {}", if self.set.airbrush_old { "OLD" } else { "STANDARD" }, self.set.airbrush_radius, self.set.airbrush_density))),
            "SETSPACE" => {
                let v = a.num("VALUE").or_else(|| a.posnum(0)).unwrap_or(8) as i32;
                self.set.spacing = 0;
                self.set.spacing_total = 0;
                self.set.spacing_air = false;
                if a.has("NTOTAL") {
                    self.set.spacing_total = v.max(1);
                } else if a.has("NTHDOT") {
                    self.set.spacing = v.max(1);
                } else if a.has("AIRBRUSH") {
                    self.set.spacing_air = true;
                }
            }
            "SETFONT" => {
                if let Some(name) = a.string("NAME").or_else(|| a.pos(0)) {
                    if let Some(i) = self.fonts.iter().position(|f| f.name.eq_ignore_ascii_case(&name)) {
                        self.font_idx = i;
                        self.font = crate::text::load_entry(&self.fonts[i], a.num("SIZE").map(|v| v as u32).unwrap_or(self.text_style.size));
                    }
                }
                if let Some(s) = a.num("SIZE") {
                    self.text_style.size = s.clamp(4, 400) as u32;
                }
                if a.has("PLAIN") {
                    self.text_style.bold = false;
                    self.text_style.italic = false;
                    self.text_style.underline = false;
                }
                self.text_style.bold |= a.has("BOLD");
                self.text_style.italic |= a.has("ITALIC");
                self.text_style.underline |= a.has("UNDERLINE");
            }
            // ---- perspective (DPaint 5.2 command list)
            "SETFILL" => {
                use crate::paint::FillKind as F;
                for (k, f) in [("SOLID", F::Solid), ("PERSPECTIVE", F::Perspective), ("PATTERN", F::Pattern), ("RUB", F::Rub), ("BRUSH", F::Brush), ("WRAP", F::Wrap), ("HBRITE", F::HBrite), ("LINE", F::Linear), ("HLINE", F::Linear), ("VERT", F::Linear), ("HORIZ", F::Linear), ("SHAPE", F::Shaped), ("CIR", F::Circular), ("CONT", F::Contour), ("HI", F::Highlight)] {
                    if a.has(k) {
                        self.set.fill = f;
                    }
                }
                if a.has("VERT") {
                    self.set.fill_angle = 90.0;
                }
                if a.has("HORIZ") || a.has("HLINE") {
                    self.set.fill_angle = 0.0;
                }
                if let Some(r) = a.num("RANGE") {
                    self.set.range = (r.max(1) as usize - 1).min(7);
                }
            }
            "CENTERPERSP" => {
                let (x, y) = (a.num("X").or_else(|| a.posnum(0)), a.num("Y").or_else(|| a.posnum(1)));
                self.v5.persp_centre = match (x, y) {
                    (Some(x), Some(y)) => Some((x as i32, y as i32)),
                    _ => self.cursor.or(Some((self.doc.w / 2, self.doc.h / 2))),
                };
                self.v5.x.p3.last_key = None;
            }
            "FIXPERSP" => {
                let h = self.persp_handle().unwrap_or((self.doc.w as f32 / 2.0, self.doc.h as f32 / 2.0, self.v5.x.p3.z));
                let all = !a.has("X") && !a.has("Y") && !a.has("Z");
                let p3 = &mut self.v5.x.p3;
                for (k, name, v) in [(0, "X", h.0), (1, "Y", h.1), (2, "Z", h.2)] {
                    if all || a.has(name) {
                        p3.fixed[k] = if p3.fixed[k].is_some() { None } else { Some(v) };
                    }
                }
                p3.last_key = None;
            }
            "SETPERSP" => {
                if a.has("BRUSH") && !self.v5.x.persp_mode {
                    self.enter_persp();
                }
                if a.has("NOBRUSH") && self.v5.x.persp_mode {
                    self.exit_persp();
                }
                let p3 = &mut self.v5.x.p3;
                let mut ang = p3.angles;
                for (k, n) in ["AX", "AY", "AZ"].iter().enumerate() {
                    if let Some(v) = a.num(n) {
                        ang[k] = v as f32;
                    }
                }
                p3.set_angles(ang);
                if let Some(z) = a.num("DZ") {
                    p3.z = z as f32;
                }
                if a.num("DX").is_some() || a.num("DY").is_some() {
                    let (x, y) = self.cursor.unwrap_or((self.doc.w / 2, self.doc.h / 2));
                    self.cursor = Some((a.num("DX").map(|v| v as i32).unwrap_or(x), a.num("DY").map(|v| v as i32).unwrap_or(y)));
                }
                self.persp = (ang[0], ang[1], ang[2]);
                self.v5.x.p3.last_key = None;
            }
            "SETPERSPOPTS" => {
                let p3 = &mut self.v5.x.p3;
                for (k, n) in ["X", "Y", "Z"].iter().enumerate() {
                    if let Some(v) = a.num(n) {
                        p3.grid[k] = v.max(1) as i32;
                    }
                }
                if let Some(s) = a.num("ANGLESTEP") {
                    p3.step = (s as f32).clamp(1.0, 180.0);
                }
                if a.has("BRUSH") {
                    p3.brush_axes = true;
                }
                if a.has("SCREEN") {
                    p3.brush_axes = false;
                }
                if a.has("ANGLE") {
                    p3.show_pos = false;
                }
                if a.has("POS") {
                    p3.show_pos = true;
                }
                if a.has("FROMBRUSH") {
                    let b = self.v5.x.persp_base.as_ref().unwrap_or(&self.brush);
                    let (w, h) = (b.img.w, b.img.h);
                    let p3 = &mut self.v5.x.p3;
                    p3.grid = [w.max(1), h.max(1), w.max(h).max(1)];
                }
            }
            "SETPERSPDEPTH" => self.v5.x.persp_depth = (a.num("DEPTH").or_else(|| a.posnum(0)).unwrap_or(200) as f32 / 100.0).clamp(0.3, 20.0),
            // ---- pictures and frames
            "SETPAGESIZE" => {
                let w = a.num("WIDTH").ok_or("WIDTH is missing")? as i32;
                let h = a.num("HEIGHT").ok_or("HEIGHT is missing")? as i32;
                let bg = self.bg;
                self.doc.resize_page(w.clamp(8, 8192), h.clamp(8, 8192), bg, false);
                self.fit_pending = true;
            }
            "GETPAGESIZE" => return Ok(Some(format!("{} {}", self.doc.w, self.doc.h))),
            "SETSCREENFORMAT" => {
                let sw = a.num("SCREENW").unwrap_or(self.v5.screen.0 as i64) as i32;
                let sh = a.num("SCREENH").unwrap_or(self.v5.screen.1 as i64) as i32;
                let pw = a.num("PAGEW").unwrap_or(sw as i64) as i32;
                let ph = a.num("PAGEH").unwrap_or(sh as i64) as i32;
                self.v5.screen = (sw.max(16), sh.max(16));
                let bg = self.bg;
                if a.has("NORETAIN") {
                    let mut d = Document::new(pw.max(8), ph.max(8), bg);
                    d.palette = self.doc.palette;
                    self.doc = d;
                } else if (pw, ph) != (self.doc.w, self.doc.h) {
                    self.doc.resize_page(pw.max(8), ph.max(8), bg, false);
                }
                if let Some(c) = a.num("COLORS") {
                    self.doc.colors = (c as usize).clamp(2, 256);
                }
                self.fit_pending = true;
            }
            "GETSCREENFORMAT" => return Ok(Some(format!("{} {} {} {} {}", self.v5.screen.0, self.v5.screen.1, self.doc.w, self.doc.h, self.doc.colors))),
            "SETFRAMES" => self.doc.set_frame_count(a.num("NUMBER").or_else(|| a.posnum(0)).unwrap_or(1).max(1) as usize),
            "ADDFRAMES" => self.doc.add_frames(a.num("NUMBER").or_else(|| a.posnum(0)).unwrap_or(1).max(1) as usize),
            "GETFRAMES" => return Ok(Some(self.doc.frames.len().to_string())),
            "GETCURFRAME" => return Ok(Some((self.doc.cur + 1).to_string())),
            "GOTOFRAME" => {
                let n = self.doc.frames.len();
                let f = if a.has("START") {
                    0
                } else if a.has("END") {
                    n - 1
                } else {
                    (a.num("NUMBER").or_else(|| a.posnum(0)).unwrap_or(1).max(1) as usize - 1).min(n - 1)
                };
                self.doc.go_to(f);
            }
            "DELETEFRAMES" => {
                if a.has("ALL") {
                    let keep = self.doc.frames[self.doc.cur].clone();
                    self.doc.frames = vec![keep];
                    self.doc.cur = 0;
                    self.doc.sync_palettes();
                } else {
                    let fr = frames_arg(self);
                    if let (Some(&s), Some(&e)) = (fr.first(), fr.last()) {
                        if !self.doc.delete_frames(s, e) {
                            return Err("Can't delete only page.".into());
                        }
                    }
                }
            }
            "COPYFRAMES" => {
                let n = self.doc.frames.len();
                let cur = self.doc.cur;
                if a.has("INSERT") || a.num("BEFORE").is_some() {
                    let fr = frames_arg(self);
                    let before = a.num("BEFORE").unwrap_or(n as i64 + 1).max(1) as usize - 1;
                    self.doc.insert_copies(fr[0], *fr.last().unwrap(), before);
                } else if a.has("ALL") {
                    self.doc.copy_frames(cur, 0, n - 1);
                } else {
                    let fr = frames_arg(self);
                    self.doc.copy_frames(cur, fr[0], *fr.last().unwrap());
                }
            }
            "FLIPPICX" => {
                let fr = frames_arg(self);
                self.apply_scope(ScopeOp::FlipH, fr);
            }
            "FLIPPICY" => {
                let fr = frames_arg(self);
                self.apply_scope(ScopeOp::FlipV, fr);
            }
            "SWAPBGFG" => {
                let fr = frames_arg(self);
                self.apply_scope(ScopeOp::SwapBgFg, fr);
            }
            "SETANIMRANGE" => {
                let n = self.doc.frames.len();
                self.doc.play_range = if a.has("ALL") {
                    None
                } else {
                    let s = a.num("START").unwrap_or(1).max(1) as usize - 1;
                    let e = a.num("END").unwrap_or(n as i64).max(1) as usize - 1;
                    Some((s.min(e), s.max(e).min(n - 1)))
                };
            }
            "GETANIMRANGE" => {
                let (s, e) = self.doc.play_range.unwrap_or((0, self.doc.frames.len() - 1));
                return Ok(Some(format!("{} {}", s + 1, e + 1)));
            }
            "SETFRAMERATE" => {
                let fps = if let Some(j) = a.num("JIFFIES") { 60.0 / j.max(1) as f32 } else { a.num("FPS").or_else(|| a.posnum(0)).unwrap_or(12).max(1) as f32 };
                if a.has("BYFRAME") {
                    let j = (60.0 / fps).round().max(1.0) as u32;
                    self.doc.sync_palettes();
                    for i in frames_arg(self) {
                        self.doc.frame_delays[i] = j;
                    }
                } else {
                    self.doc.fps = fps;
                }
            }
            "GETFRAMERATE" => return Ok(Some(format!("{:.1}", self.doc.fps))),
            "SETFRAMEPALETTE" => self.cmd(ctx, C::FramePalette),
            "GETFRAMEPALETTE" => return Ok(Some(self.doc.frame_palettes.iter().filter(|p| p.is_some()).count().to_string())),
            "SETPALETTEINFO" => {
                if a.has("FREE") {
                    self.doc.frame_palettes.iter_mut().for_each(|p| *p = None);
                }
            }
            // ---- anim brushes, move and camera
            "PICKANIMBRUSH" => {
                let r = (
                    a.num("LEFT").unwrap_or(0) as i32,
                    a.num("TOP").unwrap_or(0) as i32,
                    a.num("WIDTH").unwrap_or(32).max(1) as i32,
                    a.num("HEIGHT").unwrap_or(32).max(1) as i32,
                );
                let count = a.num("COUNT").unwrap_or(self.doc.frames.len() as i64).max(1) as usize;
                self.pick_anim_brush_at(r, count, a.has("CUT"));
            }
            "SETANIMBRUSH" => {
                let ab = self.anim_brush.as_mut().ok_or("Must have an AnimBrush")?;
                if a.has("FORWARD") {
                    ab.dir = 1;
                }
                if a.has("REVERSE") {
                    ab.dir = -1;
                }
                if a.has("PING") {
                    ab.dir = 0;
                }
                if let Some(d) = a.num("DURATION") {
                    ab.duration = d.max(1) as u32;
                }
                if let Some(c) = a.num("CURRENT") {
                    ab.restart_at(c.max(1) as usize - 1);
                }
            }
            "GETANIMBRUSH" => {
                let ab = self.anim_brush.as_ref().ok_or("Must have an AnimBrush")?;
                return Ok(Some(format!("{} {} {}", ab.dir, ab.duration, ab.pos + 1)));
            }
            "MORPHBRUSH" => {
                let n = a.num("NUMCELS").or_else(|| a.posnum(0)).unwrap_or(8) as usize;
                if !self.morph_cells(n) {
                    return Err("Must have a spare brush.".into());
                }
            }
            "PLACEHANDLE" => {
                let (w, h) = (self.brush.img.w, self.brush.img.h);
                self.brush.hx = (a.num("OFFSETX").unwrap_or(0) as i32).clamp(0, w - 1);
                self.brush.hy = (a.num("OFFSETY").unwrap_or(0) as i32).clamp(0, h - 1);
            }
            "SETMOVEOPTS" => {
                if let Some(v) = a.num("SLOWIN") {
                    self.move_params.slow_in = v.max(0) as usize;
                }
                if let Some(v) = a.num("SLOWOUT") {
                    self.move_params.slow_out = v.max(0) as usize;
                }
                if let Some(v) = a.num("COUNT").or_else(|| a.posnum(0)) {
                    self.move_params.frames = v.max(1) as usize;
                }
                if a.has("GOFROM") {
                    self.v5.x.move_come_to = false;
                }
                if a.has("COMETO") {
                    self.v5.x.move_come_to = true;
                }
                if a.has("FORWARD") {
                    self.v5.x.move_record = 1;
                }
                if a.has("INPLACE") {
                    self.v5.x.move_record = 0;
                }
                if a.has("BACKWARD") {
                    self.v5.x.move_record = -1;
                }
            }
            "SETMOVEFADE" => {
                self.v5.x.move_fade = (a.num("START").unwrap_or(0).clamp(0, 255) as u8, a.num("END").unwrap_or(0).clamp(0, 255) as u8);
            }
            "RECORDMOVE" => {
                let how = if a.has("TRAILS") {
                    1
                } else if a.has("FILL") {
                    2
                } else if a.has("PREVIEW") {
                    0
                } else {
                    3
                };
                if how > 0 {
                    self.do_move_v5(how);
                }
            }
            "SETCAMOPTS" => {
                let c = &mut self.v5.x.cam;
                if let Some(v) = a.num("SLOWIN") {
                    c.slow_in = v.max(0) as usize;
                }
                if let Some(v) = a.num("SLOWOUT") {
                    c.slow_out = v.max(0) as usize;
                }
                if let Some(v) = a.num("COUNT").or_else(|| a.posnum(0)) {
                    c.count = v.max(1) as usize;
                }
                if a.has("WRAP") {
                    c.wrap = true;
                }
                if a.has("NOWRAP") {
                    c.wrap = false;
                }
            }
            "RECORDCAM" => {
                self.v5.x.cam.mode = if a.has("BEFORE") {
                    CamMode::Before
                } else if a.has("AFTER") {
                    CamMode::After
                } else {
                    CamMode::New
                };
                if !a.has("PREVIEW") {
                    self.record_camera();
                }
            }
            // ---- fill and mouse
            "FILL" => {
                let x = a.num("X").ok_or("X is missing")? as i32;
                let y = a.num("Y").ok_or("Y is missing")? as i32;
                let save = self.v5.x.fill_to_bg;
                self.v5.x.fill_to_bg = a.has("TOBGCOLOR");
                let color = self.fg;
                if a.has("ANIMFILL") {
                    self.anim_fill((x, y), color);
                } else {
                    self.doc.checkpoint();
                    let img = self.doc.img().clone();
                    if let Some(mask) = self.fill_region(&img, x, y) {
                        let b = self.brush.clone();
                        let mut rng = Rng::new(17);
                        paint::fill_mask(&mut self.doc, &mask, &self.set, color, &b, &mut rng);
                    }
                }
                self.v5.x.fill_to_bg = save;
            }
            "LEFTMOUSE" | "RIGHTMOUSE" | "MOUSEMOVE" => {
                let primary = cmd != "RIGHTMOUSE";
                let x = a.num("X").or_else(|| a.posnum(0)).unwrap_or(self.cursor.map(|c| c.0 as i64).unwrap_or(0)) as i32;
                let y = a.num("Y").or_else(|| a.posnum(1)).unwrap_or(self.cursor.map(|c| c.1 as i64).unwrap_or(0)) as i32;
                self.cursor = Some((x, y));
                let mk = |pp: bool, dn: bool, rl: bool| crate::macros::PtrIn {
                    pos: (x, y),
                    hovered: true,
                    pressed_p: pp && primary,
                    pressed_s: pp && !primary,
                    down_p: dn && primary,
                    down_s: dn && !primary,
                    released: rl,
                    dbl: false,
                };
                let mut events = Vec::new();
                if cmd == "MOUSEMOVE" {
                    let held = self.drag.is_some() || self.stroke.is_some();
                    events.push(crate::macros::PtrIn { pos: (x, y), hovered: true, pressed_p: false, pressed_s: false, down_p: held, down_s: false, released: false, dbl: false });
                } else if a.has("DOWN") {
                    events.push(mk(true, true, false));
                } else if a.has("UP") {
                    events.push(mk(false, true, false));
                    events.push(mk(false, false, true));
                } else {
                    events.push(mk(true, true, false));
                    events.push(mk(false, false, true));
                }
                for e in events {
                    self.pointer_logic(&e);
                }
            }
            // ---- files
            "LOADPIC" | "LOADANIM" => {
                let p = file()?;
                if let Some(c) = a.num("COUNT") {
                    self.load_sequence(&p, c.max(1) as usize)?;
                } else if cmd == "LOADANIM" && a.has("APPEND") {
                    let keep = self.v5.x.load_append;
                    self.v5.x.load_append = true;
                    self.open_path_ask(p);
                    self.v5.x.load_append = keep;
                } else {
                    self.open_path(p);
                }
            }
            "SAVEPIC" => {
                let p = file()?;
                if a.num("START").is_some() {
                    let fr = frames_arg(self);
                    let imgs: Vec<Image> = fr.iter().map(|&i| self.doc.frames[i].clone()).collect();
                    crate::fileio::save_frames(&p, &imgs, &self.doc.palette, &self.doc.ranges)?;
                } else {
                    self.doc.sync_palettes();
                    if p.extension().map(|e| e.to_string_lossy().to_lowercase() == "anim").unwrap_or(false) {
                        crate::fileio::save_anim_timed(&p, &self.doc.frames, &self.doc.palette, &self.doc.ranges, self.doc.fps, &self.doc.frame_palettes, &self.doc.frame_delays)?;
                    } else {
                        crate::fileio::save_any(&p, &self.doc.frames, self.doc.cur, &self.doc.palette, &self.doc.ranges, self.doc.fps)?;
                    }
                }
            }
            "LOADBRUSH" | "LOADANIMBRUSH" => {
                let p = file()?;
                match crate::fileio::load(&p)? {
                    crate::fileio::LoadResult::Indexed(l) => {
                        let with_pal = a.has("PALETTE");
                        let pal = self.doc.palette;
                        let map: Vec<u8> = (0..256).map(|i| nearest(&pal, [l.palette[i][0] as i32, l.palette[i][1] as i32, l.palette[i][2] as i32], 256)).collect();
                        let cells: Vec<Image> = l
                            .frames
                            .iter()
                            .map(|f| {
                                let mut f = f.clone();
                                if !with_pal {
                                    f.px.iter_mut().for_each(|v| *v = map[*v as usize]);
                                }
                                f
                            })
                            .collect();
                        if with_pal {
                            self.doc.palette = l.palette;
                        }
                        let t = l.transparent.map(|t| if with_pal { t } else { map[t as usize] }).unwrap_or(0);
                        let mut b = Brush::from_image(cells[0].clone(), Some(t));
                        if let Some((gx, gy)) = l.grab {
                            b.hx = (gx as i32).clamp(0, b.img.w - 1);
                            b.hy = (gy as i32).clamp(0, b.img.h - 1);
                        }
                        self.v5.brush_palette = Some(l.palette);
                        self.brush = b;
                        self.anim_brush = if cells.len() > 1 { Some(crate::app::AnimBrush::new(cells)) } else { None };
                        self.tool = Tool::Dotted;
                    }
                    _ => return Err("Not a 256-colour brush".into()),
                }
            }
            "SAVEBRUSH" => {
                let p = file()?;
                let t = if a.has("NOMASK") { None } else { self.brush.transparent };
                crate::fileio::save_ilbm(&p, &self.brush.img, &self.doc.palette, &[], t)?;
            }
            "SAVEANIMBRUSH" => {
                let p = file()?;
                let ab = self.anim_brush.as_ref().ok_or("Must have an AnimBrush")?;
                crate::fileio::save_anim_brush(&p, &ab.cells, &self.doc.palette, (self.brush.hx as i16, self.brush.hy as i16), self.brush.transparent, self.doc.fps)?;
            }
            "LOADPALETTE" | "LOADCOLORSET" => {
                let p = file()?;
                match crate::fileio::load(&p)? {
                    crate::fileio::LoadResult::Indexed(l) => {
                        self.doc.checkpoint();
                        self.doc.palette = l.palette;
                    }
                    _ => return Err("That file has no palette".into()),
                }
            }
            "SAVEPALETTE" | "SAVECOLORSET" => {
                let p = file()?;
                let mut s = String::from("JASC-PAL\n0100\n256\n");
                for c in self.doc.palette.iter() {
                    s += &format!("{} {} {}\n", c[0], c[1], c[2]);
                }
                std::fs::write(&p, s).map_err(|e| format!("Can't Save: {e}"))?;
            }
            "LOADSTENCIL" => {
                let p = file()?;
                match crate::fileio::load(&p)? {
                    crate::fileio::LoadResult::Indexed(l) if (l.frames[0].w, l.frames[0].h) == (self.doc.w, self.doc.h) => {
                        let mask = l.frames[0].px.iter().map(|&v| v != 0).collect();
                        self.doc.stencil = Some(crate::doc::Stencil { mask, on: true });
                    }
                    _ => return Err("Wrong size stencil.".into()),
                }
            }
            "SAVESTENCIL" => {
                let p = file()?;
                let st = self.doc.stencil.as_ref().ok_or("No stencil defined.")?;
                let mut img = Image::new(self.doc.w, self.doc.h, 0);
                for (i, m) in st.mask.iter().enumerate() {
                    img.px[i] = *m as u8;
                }
                crate::fileio::save_ilbm(&p, &img, &self.doc.palette, &[], None)?;
            }
            "LOADTEXTURE" => {
                let p = file()?;
                match crate::fileio::load(&p)? {
                    crate::fileio::LoadResult::Indexed(l) => {
                        let mut i = l.frames[0].clone();
                        i.px.iter_mut().for_each(|v| *v = (*v != 0) as u8);
                        self.set.texture = Some(paint::Texture { img: std::sync::Arc::new(i), invert: false });
                        self.set.texture_on = true;
                    }
                    _ => return Err("Not a 256-colour picture".into()),
                }
            }
            "OPENMEDIA" => {
                let name = file()?.to_string_lossy().to_lowercase();
                let i = crate::actions::MEDIA.iter().position(|m| name.contains(&m.0.to_lowercase().replace([' ', '-'], ""))).unwrap_or(0);
                self.set_media(i);
            }
            "LOADMOVE" | "SAVEMOVE" | "LOADCAM" | "SAVECAM" => return Err("Use the Move and Camera Move requesters to load and save these.".into()),
            "QUIT" => {
                if a.has("FORCE") {
                    self.quit_now = true;
                } else {
                    self.request(crate::app::Pending::Quit);
                }
            }
            "VERSION" => return Ok(Some("Deluxe Pixel 1.3 (DeluxePaint 5.2 commands)".into())),
            "HELP" => return Ok(Some("See Help > Keyboard shortcuts".into())),
            "REQUESTOK" | "REQUESTYESNO" | "REQUESTSTRING" | "REQUESTNUMBER" => {
                let prompt = a.pos(0).unwrap_or_default();
                self.msg("User Request", prompt);
                return Ok(Some("1".into()));
            }
            c if c.starts_with("GET") => return Ok(Some("(not available)".into())),
            _ => return Err("not understood".into()),
        }
        Ok(None)
    }
}

/// Split a command line into words, keeping "quoted strings" together.
pub fn tokenize(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in line.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => cur.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() || ch == ',' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Command arguments in the AmigaDOS template style: KEYWORD value, switches, or by position.
pub struct Args<'a>(pub &'a [String]);

const VALUE_KEYS: [&str; 33] = [
    "FILE", "NAME", "SIZE", "NUMBER", "COLNUM", "PENNUM", "LEVEL", "VALUE", "DEPTH", "NUMCELS", "FPS", "JIFFIES", "START", "END", "BEFORE", "COUNT", "X", "Y", "WIDTH", "HEIGHT", "LEFT", "TOP", "RADIUS",
    "FLOW", "DURATION", "CURRENT", "OFFSETX", "OFFSETY", "RED", "GREEN", "BLUE", "SLOWIN", "SLOWOUT",
];

impl Args<'_> {
    pub fn has(&self, k: &str) -> bool {
        self.0.iter().any(|t| t.eq_ignore_ascii_case(k))
    }
    pub fn string(&self, k: &str) -> Option<String> {
        for (i, t) in self.0.iter().enumerate() {
            if let Some((key, v)) = t.split_once('=') {
                if key.eq_ignore_ascii_case(k) {
                    return Some(v.to_string());
                }
            }
            if t.eq_ignore_ascii_case(k) {
                return self.0.get(i + 1).cloned();
            }
        }
        None
    }
    pub fn num(&self, k: &str) -> Option<i64> {
        self.string(k).and_then(|v| v.parse().ok())
    }
    /// The n-th argument that is not a keyword or a keyword's value.
    pub fn pos(&self, n: usize) -> Option<String> {
        let mut plain = Vec::new();
        let mut skip = false;
        for t in self.0 {
            if skip {
                skip = false;
                continue;
            }
            let upper = t.to_uppercase();
            if VALUE_KEYS.contains(&upper.as_str()) {
                skip = true;
                continue;
            }
            if t.len() > 1 && t.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
                continue;
            }
            if t.split_once('=').map(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphabetic())).unwrap_or(false) {
                continue;
            }
            plain.push(t.clone());
        }
        plain.get(n).cloned()
    }
    pub fn posnum(&self, n: usize) -> Option<i64> {
        self.pos(n).and_then(|v| v.parse().ok())
    }
}
