//! The Move requester, after the DeluxePaint V manual (chapter 6, "Bewegen"): the brush travels
//! through 3-D space over a number of frames, using the same 3-D model as perspective mode.
//! * Distance X (right), Y (up) and Z (into the distance, towards the perspective centre);
//!   angles about X, Y and Z. "Brush" boxes: distance along the brush's own axes (default off),
//!   angles about the brush's own axes (default on).
//! * The brush starts where it was last stamped; after a move DeluxePaint remembers where it
//!   ended, and the next move starts there. Go Back starts the next move from the original place.
//! * Cyclic: the moves loops seamlessly (the last frame stops one step short of the full move;
//!   afterwards the frame after the move is shown). Not cyclic: the last frame reaches the end.
//! * Fade (0–255, 255 = invisible) at the start and at the end; Start / End choose which end the
//!   Fade slider and Adjust work on. Adjust places the brush for that end interactively (perspective
//!   keys turn it, Ctrl + mouse moves it in depth, Enter takes it, Esc cancels) and fills in the
//!   distances and angles.

use crate::app::App;
use crate::paint::{self, Stroke};
use crate::persp3d::{self, Mat, IDENTITY};
use crate::pix::Brush;
use crate::ui_style::{button, toggle};
use egui::{Color32, RichText};

/// Where the brush is in 3-D: handle position (screen x, y; depth z) and orientation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovePose {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot: Mat,
}

pub fn transpose(m: &Mat) -> Mat {
    [[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]]
}

fn apply_vec(m: &Mat, v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

/// The pose at fraction `t` of a move from `start` (t may be negative for "Come to here").
pub fn pose_at(start: &MovePose, dist: [i32; 3], angle: [f32; 3], dist_brush: bool, angle_brush: bool, t: f32) -> MovePose {
    // Screen y grows downwards; the requester's Y is up.
    let mut d = [dist[0] as f32 * t, -(dist[1] as f32) * t, dist[2] as f32 * t];
    if dist_brush {
        d = apply_vec(&start.rot, d);
    }
    let r = persp3d::from_angles([angle[0] * t, angle[1] * t, angle[2] * t]);
    let rot = if angle_brush { persp3d::mul(&start.rot, &r) } else { persp3d::mul(&r, &start.rot) };
    MovePose { x: start.x + d[0], y: start.y + d[1], z: start.z + d[2], rot }
}

/// Fractions of the move for each of `n` frames: cyclic moves stop one step short of the end so
/// the loop has no repeated frame.
pub fn move_fractions(n: usize, cyclic: bool, slow_in: usize, slow_out: usize) -> Vec<f32> {
    let n = n.max(1);
    (0..n).map(|k| if cyclic { App::move_progress_pub(k, n + 1, slow_in, slow_out) } else { App::move_progress_pub(k, n, slow_in, slow_out) }).collect()
}

fn flat(p: &MovePose) -> bool {
    p.z.abs() < 0.01 && (0..3).all(|i| (0..3).all(|j| (p.rot[i][j] - IDENTITY[i][j]).abs() < 1e-4))
}

impl App {
    /// The flat brush the move draws (in perspective mode: the brush before it was turned).
    fn move_base_brush(&self) -> Brush {
        self.v5.x.persp_base.clone().unwrap_or_else(|| self.brush.clone())
    }

    /// The pose a move starts from: where the last move ended, unless Go Back was pressed or the
    /// brush was stamped since; then the original place.
    pub fn move_start_pose(&self) -> MovePose {
        let x = &self.v5.x;
        if !x.mv_go_back {
            if let Some(p) = x.mv_cur {
                return p;
            }
        }
        if let Some(p) = x.mv_origin {
            return p;
        }
        let (sx, sy) = self.move_params.start;
        let (z, rot) = if x.persp_mode { (x.p3.z, x.p3.rot) } else { (0.0, IDENTITY) };
        MovePose { x: sx as f32, y: sy as f32, z, rot }
    }

    /// Forget where the last move ended (the frames were cleared): the next move starts where the
    /// brush was last stamped.
    pub fn move_forget_end(&mut self) {
        self.v5.x.mv_cur = None;
        self.v5.x.mv_go_back = false;
        self.v5.x.move_preview_key.clear();
    }

    /// The brush stamped somewhere by hand: that is the new starting place of moves.
    pub fn move_note_stamp(&mut self, pos: (i32, i32)) {
        self.move_params.start = pos;
        let (z, rot) = if self.v5.x.persp_mode { (self.v5.x.p3.z, self.v5.x.p3.rot) } else { (0.0, IDENTITY) };
        self.v5.x.mv_origin = Some(MovePose { x: pos.0 as f32, y: pos.1 as f32, z, rot });
        self.v5.x.mv_cur = None;
        self.v5.x.mv_go_back = false;
    }

    /// The brush drawn at a pose: (stamp position, brush, wire-frame corners).
    fn brush_at_pose(&self, base: &Brush, p: &MovePose) -> Option<((i32, i32), Brush, [(f32, f32); 4])> {
        let at = (p.x.round() as i32, p.y.round() as i32);
        let hm = persp3d::homography(&p.rot, (p.x, p.y, p.z), self.persp_centre_f(), self.persp_eye());
        let quad = persp3d::corners(base, &hm)?;
        if flat(p) {
            return Some((at, base.clone(), quad));
        }
        let b = persp3d::project(base, &hm, at, (self.doc.w, self.doc.h))?;
        Some((at, if self.antialias { b.antialiased(&self.doc.palette) } else { b }, quad))
    }

    /// The steps of the move: (frame offset, stamp position, brush, wire frame).
    pub fn move_steps(&self) -> Vec<(i64, (i32, i32), Brush, [(f32, f32); 4])> {
        let mp = &self.move_params;
        let x = &self.v5.x;
        let n = mp.frames.max(1);
        let start = self.move_start_pose();
        let base = self.move_base_brush();
        let mut out = Vec::with_capacity(n);
        for (k, t) in move_fractions(n, mp.cyclic, mp.slow_in, mp.slow_out).into_iter().enumerate() {
            // Come to here: the move ends where the brush is.
            let t = if x.move_come_to { t - 1.0 } else { t };
            let p = pose_at(&start, mp.dist, mp.angle, x.mv_dist_brush, x.mv_angle_brush, t);
            let mut b = base.clone();
            if let Some(ab) = &self.anim_brush {
                // The AnimBrush's own direction, duration and current cel decide each step's cel.
                b.img = ab.cells[ab.cell_at(ab.ticks as usize + k)].clone();
            }
            let off = match x.move_record {
                1 => k as i64,
                -1 => -(k as i64),
                _ => 0,
            };
            if let Some((at, br, quad)) = self.brush_at_pose(&b, &p) {
                out.push((off, at, br, quad));
            }
        }
        out
    }

    /// Only the wire frames of the steps (cheap enough to work out every frame for the Preview).
    pub fn move_quads(&self) -> Vec<[(f32, f32); 4]> {
        let mp = &self.move_params;
        let x = &self.v5.x;
        let start = self.move_start_pose();
        let base = self.move_base_brush();
        let (c, eye) = (self.persp_centre_f(), self.persp_eye());
        move_fractions(mp.frames.max(1), mp.cyclic, mp.slow_in, mp.slow_out)
            .into_iter()
            .filter_map(|t| {
                let t = if x.move_come_to { t - 1.0 } else { t };
                let p = pose_at(&start, mp.dist, mp.angle, x.mv_dist_brush, x.mv_angle_brush, t);
                persp3d::corners(&base, &persp3d::homography(&p.rot, (p.x, p.y, p.z), c, eye))
            })
            .collect()
    }

    /// Draw the move: 1 = Trails (every step on this frame), 2 = Fill (add frames as needed),
    /// 3 = Draw.
    pub fn do_move_3d(&mut self, how: u8) {
        let steps = self.move_steps();
        let n = steps.len();
        if n == 0 {
            self.msg("Move", "The brush would be behind the eye: nothing to draw.");
            return;
        }
        let start = self.doc.cur as i64;
        if how == 2 && self.v5.x.move_record == 1 {
            let need = start as usize + self.move_params.frames;
            if need > self.doc.frames.len() {
                self.doc.set_frame_count(need);
            }
        }
        let total = self.doc.frames.len() as i64;
        let (f0, f1) = self.v5.x.move_fade;
        if how == 1 {
            self.doc.checkpoint();
        }
        let mut last = start;
        for (k, (off, at, b, _)) in steps.into_iter().enumerate() {
            let fi = if how == 1 { start } else { start + off };
            let fi = if self.move_params.cyclic {
                fi.rem_euclid(total)
            } else if fi < 0 || fi >= total {
                continue;
            } else {
                fi
            };
            last = fi;
            self.doc.cur = fi as usize;
            if how != 1 {
                self.doc.checkpoint();
            }
            let mut set = self.set.clone();
            let t = if n > 1 { k as f32 / (n - 1) as f32 } else { 0.0 };
            // Fade 0-255 (255 = invisible) as translucency.
            let fade = (f0 as f32 + (f1 as f32 - f0 as f32) * t) * 100.0 / 255.0;
            if fade > 0.5 {
                set.translucent_on = true;
                set.translucent = fade.round().min(99.0) as u8;
            }
            let mut st = Stroke::new(self.fg, true);
            paint::stamp(&mut self.doc, &b, &set, &mut st, at.0, at.1);
        }
        // Remember where the brush ended (Draw and Fill; Trails only traces the path on this frame
        // and leaves the starting place as it was); show the frame after a cyclic move, or the last.
        if how != 1 {
            let start_pose = self.move_start_pose();
            let (mp, x) = (&self.move_params, &self.v5.x);
            let end_t = if x.move_come_to { 0.0 } else { 1.0 };
            self.v5.x.mv_cur = Some(pose_at(&start_pose, mp.dist, mp.angle, x.mv_dist_brush, x.mv_angle_brush, end_t));
            self.v5.x.mv_go_back = false;
        }
        self.v5.x.move_preview_key.clear();
        let frame = if how == 1 {
            start
        } else if self.move_params.cyclic {
            (start + self.move_params.frames as i64).rem_euclid(total)
        } else {
            last
        };
        self.doc.go_to(frame as usize);
        self.v5.x.move_preview.clear();
        self.status = format!("Move drew {n} steps");
    }

    // ---------------------------------------------------------------- Adjust

    /// Adjust: place the brush for the start (false) or the end (true) of the move by hand.
    pub fn begin_move_adjust(&mut self, end: bool) {
        let base = self.move_base_brush();
        if !base.custom {
            self.msg("Move", "Adjust needs a picked-up brush.");
            return;
        }
        let start = self.move_start_pose();
        let pose = if end {
            let (mp, x) = (&self.move_params, &self.v5.x);
            pose_at(&start, mp.dist, mp.angle, x.mv_dist_brush, x.mv_angle_brush, 1.0)
        } else {
            start
        };
        self.v5.x.mv_adjust_saved = Some((self.v5.x.persp_mode, self.v5.x.persp_base.clone(), self.v5.x.p3.clone(), self.brush.clone()));
        self.brush = base.clone();
        self.v5.x.persp_base = Some(base);
        self.v5.x.persp_mode = true;
        self.v5.x.p3.rot = pose.rot;
        self.v5.x.p3.angles = persp3d::to_angles(&pose.rot);
        self.v5.x.p3.z = pose.z;
        self.v5.x.p3.last_key = None;
        self.v5.x.mv_adjust = Some(end);
        self.show_move = false;
        self.status = format!(
            "Adjust the {}: move the brush, turn it with the number keys, Ctrl + mouse for depth; Enter takes it, Esc cancels",
            if end { "end" } else { "start" }
        );
    }

    pub fn finish_move_adjust(&mut self, take: bool) {
        let Some(end) = self.v5.x.mv_adjust.take() else { return };
        if take {
            if let Some(h) = self.persp_handle() {
                let pose = MovePose { x: h.0, y: h.1, z: h.2, rot: self.v5.x.p3.rot };
                if end {
                    let s = self.move_start_pose();
                    let mut d = [pose.x - s.x, pose.y - s.y, pose.z - s.z];
                    if self.v5.x.mv_dist_brush {
                        d = apply_vec(&transpose(&s.rot), d);
                    }
                    self.move_params.dist = [d[0].round() as i32, -(d[1].round() as i32), d[2].round() as i32];
                    let r = if self.v5.x.mv_angle_brush { persp3d::mul(&transpose(&s.rot), &pose.rot) } else { persp3d::mul(&pose.rot, &transpose(&s.rot)) };
                    let a = persp3d::to_angles(&r);
                    self.move_params.angle = a;
                } else {
                    self.move_params.start = (pose.x.round() as i32, pose.y.round() as i32);
                    self.v5.x.mv_origin = Some(pose);
                    self.v5.x.mv_cur = None;
                    self.v5.x.mv_go_back = false;
                }
            }
        }
        if let Some((mode, base, p3, brush)) = self.v5.x.mv_adjust_saved.take() {
            self.v5.x.persp_mode = mode;
            self.v5.x.persp_base = base;
            self.v5.x.p3 = p3;
            self.v5.x.p3.last_key = None;
            self.brush = brush;
        }
        self.show_move = true;
        self.status.clear();
    }

    // ---------------------------------------------------------------- the requester

    pub(crate) fn move_req3(&mut self, ctx: &egui::Context) {
        if !self.show_move {
            self.v5.x.move_preview.clear();
            self.v5.x.move_preview_on = false;
            self.v5.x.mv_backup = None;
            return;
        }
        // Preview on: the wire frames follow every change and keep running without the mouse moving.
        if self.v5.x.move_preview_on {
            // Worked out again only when something it depends on changed.
            let x = &self.v5.x;
            let b = self.move_base_brush();
            let key = format!(
                "{:?}{:?}{:?}{:?}{}{}{}{}{}{:?}{:?}{}{}",
                self.move_params, self.move_start_pose(), self.v5.persp_centre, x.persp_depth, x.move_come_to, x.mv_dist_brush, x.mv_angle_brush, b.img.w, b.img.h, (b.hx, b.hy), self.doc.w, x.move_record, self.doc.h
            );
            if key != self.v5.x.move_preview_key {
                self.v5.x.move_preview = self.move_quads();
                self.v5.x.move_preview_key = key;
                if self.v5.x.move_preview.is_empty() {
                    self.status = "Preview: the move would go behind the eye. Zero, Go Back or a smaller Z distance bring it back.".into();
                }
            }
            ctx.request_repaint();
        } else {
            self.v5.x.move_preview.clear();
            self.v5.x.move_preview_key.clear();
        }
        // What Cancel goes back to: the settings as they were when the requester opened.
        if self.v5.x.mv_backup.is_none() {
            if !self.v5.x.mv_opened_once && self.doc.frames.len() > 1 {
                self.move_params.frames = self.doc.frames.len();
            }
            self.v5.x.mv_opened_once = true;
            let x = &self.v5.x;
            self.v5.x.mv_backup = Some((self.move_params.clone(), x.move_come_to, x.move_record, x.move_fade, x.mv_dist_brush, x.mv_angle_brush, x.mv_fade_end));
        }
        let mut action: Option<u8> = None;
        // Layout of the manual's Move requester (fig. 6.6), labels from the DPaint 5.2 program.
        let field = |ui: &mut egui::Ui, w: f32, dv: egui::DragValue| ui.add_sized([w, 22.0], dv);
        let rule = |ui: &mut egui::Ui| {
            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);
        };
        egui::Window::new(RichText::new("Move").color(Color32::WHITE)).collapsible(false).resizable(false).show(ctx, |ui| {
            const W: f32 = 600.0;
            ui.set_width(W);
            ui.set_max_width(W);
            let mp = &mut self.move_params;
            let x = &mut self.v5.x;
            // X / Y / Z headings, Dist and Angle rows, each with its Brush box.
            egui::Grid::new("move_xyz").num_columns(5).spacing([14.0, 6.0]).min_col_width(0.0).show(ui, |ui| {
                ui.allocate_space(egui::vec2(80.0, 0.0));
                for h in ["X", "Y", "Z"] {
                    ui.vertical_centered(|ui| {
                        ui.set_width(110.0);
                        ui.label(RichText::new(h).strong());
                    });
                }
                ui.label("");
                ui.end_row();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label("Dist:"));
                for k in 0..3 {
                    field(ui, 110.0, egui::DragValue::new(&mut mp.dist[k]).range(-9999..=9999));
                }
                ui.checkbox(&mut x.mv_dist_brush, "Brush").on_hover_text("Move along the brush's own axes (off: the screen's axes)");
                ui.end_row();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label("Angle:"));
                for k in 0..3 {
                    field(ui, 110.0, egui::DragValue::new(&mut mp.angle[k]).range(-3600.0..=3600.0).speed(1.0));
                }
                ui.checkbox(&mut x.mv_angle_brush, "Brush").on_hover_text("Turn about the brush's own axes (off: the screen's axes)");
                ui.end_row();
            });
            rule(ui);
            // Zero, Go Back, Cyclic.
            ui.horizontal(|ui| {
                ui.add_space(120.0);
                if ui.add_sized([90.0, 24.0], egui::Button::new("Zero")).on_hover_text("Distances and angles to 0").clicked() {
                    mp.dist = [0; 3];
                    mp.angle = [0.0; 3];
                }
                ui.add_space(30.0);
                if ui.add_sized([110.0, 24.0], egui::Button::new("Go Back")).on_hover_text("The next move starts from where the brush was before the last move, not where it ended").clicked() {
                    x.mv_go_back = true;
                }
                ui.add_space(30.0);
                ui.checkbox(&mut mp.cyclic, "Cyclic").on_hover_text("The move loops: the last frame stops one step before the end");
            });
            rule(ui);
            // Slow In / Slow Out / Count | Move | Record, in a panel of fixed height (so its
            // dividers don't stretch the window).
            ui.allocate_ui_with_layout(egui::vec2(W, 96.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.set_height(96.0);
                ui.vertical(|ui| {
                    ui.set_width(250.0);
                    egui::Grid::new("move_counts").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                        for (label, v) in [("Slow In:", &mut mp.slow_in), ("Slow Out:", &mut mp.slow_out)] {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.set_width(120.0);
                                ui.label(label)
                            });
                            field(ui, 80.0, egui::DragValue::new(v).range(0..=999));
                            ui.end_row();
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.set_width(120.0);
                            ui.label("Count:")
                        });
                        field(ui, 80.0, egui::DragValue::new(&mut mp.frames).range(1..=999));
                        ui.end_row();
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(140.0);
                    ui.vertical_centered(|ui| ui.label(RichText::new("Move").strong()));
                    ui.horizontal(|ui| {
                        ui.add_space(30.0);
                        ui.vertical(|ui| {
                            if ui.radio(!x.move_come_to, "•→").on_hover_text("Go from here: the move starts where the brush is").clicked() {
                                x.move_come_to = false;
                            }
                            if ui.radio(x.move_come_to, "→•").on_hover_text("Come to here: the move ends where the brush is").clicked() {
                                x.move_come_to = true;
                            }
                        });
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(150.0);
                    ui.vertical_centered(|ui| ui.label(RichText::new("Record").strong()));
                    ui.horizontal(|ui| {
                        ui.add_space(50.0);
                        ui.vertical(|ui| {
                            for (v, l, tip) in [(1, "»", "Forwards, one frame after another"), (0, "||", "All on this frame"), (-1, "«", "Backwards")] {
                                if ui.radio(x.move_record == v, l).on_hover_text(tip).clicked() {
                                    x.move_record = v;
                                }
                            }
                        });
                    });
                });
            });
            rule(ui);
            // Start / End, Fade, Adjust.
            ui.horizontal(|ui| {
                if toggle(ui, "Start", !x.mv_fade_end).on_hover_text("Fade and Adjust work on the start of the move").clicked() {
                    x.mv_fade_end = false;
                }
                if toggle(ui, "End", x.mv_fade_end).on_hover_text("Fade and Adjust work on the end of the move").clicked() {
                    x.mv_fade_end = true;
                }
                ui.add_space(60.0);
                ui.label("Fade:");
                let v = if x.mv_fade_end { &mut x.move_fade.1 } else { &mut x.move_fade.0 };
                let value = *v;
                ui.spacing_mut().slider_width = 200.0;
                ui.add(egui::Slider::new(v, 0..=255).show_value(false)).on_hover_text("0 = solid, 255 = invisible");
                ui.add_sized([40.0, 20.0], egui::Label::new(value.to_string()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if button(ui, "Adjust").on_hover_text("Place the brush for the start or end by hand; Enter takes it").clicked() {
                        action = Some(6);
                    }
                });
            });
            // Two rows of four equal buttons.
            let bw = (W - 3.0 * 6.0) / 4.0;
            let preview_on = x.move_preview_on;
            for row in [[(0u8, "Preview"), (1, "Trails"), (2, "Fill"), (3, "Draw")], [(9, "Exit"), (4, "Load"), (5, "Save"), (8, "Cancel")]] {
                ui.horizontal(|ui| {
                    for (id, l) in row {
                        // Preview is a switch: lit while it runs.
                        let lit = id == 0 && preview_on;
                        if ui.add_sized([bw, 26.0], egui::Button::new(l).selected(lit)).on_hover_text(if id == 0 { "Preview on / off" } else { "" }).clicked() {
                            action = Some(id);
                        }
                    }
                });
            }
        });
        match action {
            Some(0) => {
                self.v5.x.move_preview_on = !self.v5.x.move_preview_on;
                self.v5.x.move_preview_key.clear();
                self.v5.x.move_preview = if self.v5.x.move_preview_on { self.move_quads() } else { Vec::new() };
                self.v5.x.move_preview_t = self.time as f32;
            }
            Some(1) => self.do_move_3d(1),
            Some(2) => self.do_move_3d(2),
            Some(3) => self.do_move_3d(3),
            Some(4) => self.load_move(),
            Some(5) => self.save_move(),
            Some(6) => {
                let end = self.v5.x.mv_fade_end;
                self.begin_move_adjust(end);
            }
            // Exit keeps the settings; Cancel puts back what they were when the requester opened.
            Some(9) => self.show_move = false,
            Some(8) => {
                if let Some((mp, come, rec, fade, db, ab, fe)) = self.v5.x.mv_backup.take() {
                    self.move_params = mp;
                    let x = &mut self.v5.x;
                    x.move_come_to = come;
                    x.move_record = rec;
                    x.move_fade = fade;
                    x.mv_dist_brush = db;
                    x.mv_angle_brush = ab;
                    x.mv_fade_end = fe;
                }
                self.show_move = false;
            }
            _ => {}
        }
    }

    /// The Preview: the brush as a wire frame running through the move, over and over.
    pub(crate) fn draw_move_preview(&self, painter: &egui::Painter, to_screen: impl Fn(f32, f32) -> egui::Pos2) {
        let n = self.v5.x.move_preview.len();
        if n == 0 {
            return;
        }
        let cur = ((self.time as f32 - self.v5.x.move_preview_t) * self.doc.fps.max(1.0)).max(0.0) as usize % n;
        for (k, q) in self.v5.x.move_preview.iter().enumerate() {
            let pts: Vec<egui::Pos2> = q.iter().map(|p| to_screen(p.0, p.1)).collect();
            let c = if k == cur { Color32::WHITE } else { Color32::from_rgba_unmultiplied(255, 255, 255, 50) };
            for i in 0..4 {
                painter.line_segment([pts[i], pts[(i + 1) % 4]], egui::Stroke::new(if k == cur { 1.5 } else { 1.0 }, c));
            }
        }
    }
}
