//! Requesters as the DeluxePaint V reference section (chapter 8) shows them: Page Size
//! (fig. 8.23), Symmetry (8.16), Grid (8.15), Spacing (8.4), Airbrush (8.13), Translucency (8.43),
//! Set # of frames (8.32) and Go to Frame (8.38). Each works on a copy: Ok takes it, Cancel drops it.

use crate::app::App;
use crate::doc::Document;
use crate::paint::{self, Stroke, SymKind, Symmetry};
use crate::ui_style::{button, toggle};
use egui::{Color32, RichText, Vec2};

fn req(title: &str) -> egui::Window<'static> {
    egui::Window::new(RichText::new(title.to_string()).color(Color32::WHITE)).collapsible(false).resizable(false)
}

/// Ok on the left, Cancel on the right, as in every requester of the original.
fn ok_cancel(ui: &mut egui::Ui, width: f32) -> Option<bool> {
    let mut r = None;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if button(ui, "Ok").clicked() {
            r = Some(true);
        }
        ui.add_space((width - 110.0).max(10.0));
        if button(ui, "Cancel").clicked() {
            r = Some(false);
        }
    });
    r
}

/// Working copies of the requesters' settings.
#[derive(Default)]
pub struct Reqs8 {
    pub page: Option<(i32, i32, u8)>,
    pub sym: Option<Symmetry>,
    pub grid: Option<(i32, i32)>,
    /// Grid > Adjust: dragging on the page sets a grid point and the spacing.
    pub grid_adjust: bool,
    pub grid_adjust_from: Option<(i32, i32)>,
    /// Spacing: (kind 0 N Total / 1 Every Nth / 2 Airbrush / 3 Continuous, N total, Nth, airbrush sprays).
    pub spacing: Option<(u8, i32, i32, i32)>,
    /// Airbrush: (old style, radius, flow) and its test area.
    pub air: Option<(bool, i32, i32)>,
    pub air_test: Option<Document>,
    pub air_size: bool,
    pub transl: Option<u8>,
    pub frames_n: Option<usize>,
    pub goto_n: Option<usize>,
}

impl App {
    pub fn reqs8(&mut self, ctx: &egui::Context) {
        self.page_req8(ctx);
        self.symmetry_req8(ctx);
        self.grid_req8(ctx);
        self.spacing_req8(ctx);
        self.airbrush_req8(ctx);
        self.transl_req8(ctx);
        self.frames_req8(ctx);
        self.goto_req8(ctx);
    }

    // ---------------------------------------------------------------- Page Size (fig. 8.23)

    fn page_req8(&mut self, ctx: &egui::Context) {
        if !self.show_page_size {
            self.v5.x.r8.page = None;
            return;
        }
        // An animation's frames keep their size (the original only says so).
        if self.doc.frames.len() > 1 {
            let mut done = false;
            req("Anim Page Size").show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(format!("{} x {}", self.doc.w, self.doc.h)).monospace());
                    if button(ui, "Continue").clicked() {
                        done = true;
                    }
                });
            });
            if done {
                self.show_page_size = false;
            }
            return;
        }
        let (sw, sh) = self.v5.screen;
        let choices = [("Screen", sw, sh), ("Full Page", sw, sh * 17 / 10), ("Overscan", sw * 23 / 20, sh * 23 / 20)];
        let (dw, dh) = (self.doc.w, self.doc.h);
        let (mut w, mut h, mut kind) = self.v5.x.r8.page.unwrap_or_else(|| {
            let k = choices.iter().position(|c| (c.1, c.2) == (dw, dh)).unwrap_or(3) as u8;
            (dw, dh, k)
        });
        let mut done = None;
        req("Page Size").show(ctx, |ui| {
            ui.set_width(300.0);
            ui.label("Enter size:");
            ui.horizontal(|ui| {
                ui.label("Width:");
                if ui.add(egui::DragValue::new(&mut w).range(8..=8192)).changed() {
                    kind = 3;
                }
                ui.add_space(20.0);
                ui.label("Height:");
                if ui.add(egui::DragValue::new(&mut h).range(8..=8192)).changed() {
                    kind = 3;
                }
            });
            ui.separator();
            ui.label("or choose:");
            for (k, (name, cw, ch)) in choices.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.radio(kind == k as u8, *name).clicked() {
                        kind = k as u8;
                        w = *cw;
                        h = *ch;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.monospace(format!("{cw} x {ch}")));
                });
            }
            if ui.radio(kind == 3, "Custom").clicked() {
                kind = 3;
            }
            done = ok_cancel(ui, 300.0);
        });
        self.v5.x.r8.page = Some((w, h, kind));
        if let Some(ok) = done {
            if ok && (w, h) != (self.doc.w, self.doc.h) {
                self.doc.resize_page(w, h, self.bg, false);
                self.fit_pending = true;
            }
            self.show_page_size = false;
        }
    }

    // ---------------------------------------------------------------- Symmetry (fig. 8.16)

    fn symmetry_req8(&mut self, ctx: &egui::Context) {
        if !self.show_symmetry {
            self.v5.x.r8.sym = None;
            return;
        }
        let mut s = self.v5.x.r8.sym.clone().unwrap_or_else(|| self.set.sym.clone());
        let mut done = None;
        let mut place = false;
        req("Symmetry").show(ctx, |ui| {
            ui.set_width(260.0);
            ui.horizontal(|ui| {
                if toggle(ui, "Point", s.kind == SymKind::Cyclic).clicked() {
                    s.kind = SymKind::Cyclic;
                }
                if toggle(ui, "Tile", s.kind == SymKind::Tile).clicked() {
                    s.kind = SymKind::Tile;
                }
            });
            ui.separator();
            ui.add_enabled_ui(s.kind == SymKind::Cyclic, |ui| {
                ui.horizontal(|ui| {
                    if ui.radio(!s.mirror, "Cyclic").clicked() {
                        s.mirror = false;
                    }
                    if ui.radio(s.mirror, "Mirror").clicked() {
                        s.mirror = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Order:");
                    ui.add(egui::DragValue::new(&mut s.points).range(1..=40));
                    ui.add_space(20.0);
                    if button(ui, "Place").on_hover_text("Click on the picture to set the centre").clicked() {
                        place = true;
                    }
                });
            });
            ui.separator();
            ui.add_enabled_ui(s.kind == SymKind::Tile, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Width:");
                    ui.add(egui::DragValue::new(&mut s.tile_w).range(2..=1024));
                    ui.label("Height:");
                    ui.add(egui::DragValue::new(&mut s.tile_h).range(2..=1024));
                });
            });
            done = ok_cancel(ui, 260.0);
        });
        if place {
            s.on = true;
            self.set.sym = s.clone();
            self.sym_place = true;
            self.status = "Click on the picture to place the symmetry centre".into();
            self.show_symmetry = false;
            return;
        }
        self.v5.x.r8.sym = Some(s.clone());
        if let Some(ok) = done {
            if ok {
                s.on = true;
                self.set.sym = s;
            }
            self.show_symmetry = false;
        }
    }

    // ---------------------------------------------------------------- Grid (fig. 8.15)

    fn grid_req8(&mut self, ctx: &egui::Context) {
        if !self.show_grid {
            self.v5.x.r8.grid = None;
            return;
        }
        let (mut gx, mut gy) = self.v5.x.r8.grid.unwrap_or((self.set.grid.0, self.set.grid.1));
        let mut done = None;
        let mut adjust = false;
        req("Grid").show(ctx, |ui| {
            ui.set_width(230.0);
            egui::Grid::new("grid8").num_columns(2).show(ui, |ui| {
                ui.label("X Spacing:");
                ui.add(egui::DragValue::new(&mut gx).range(1..=512));
                ui.end_row();
                ui.label("Y Spacing:");
                ui.add(egui::DragValue::new(&mut gy).range(1..=512));
                ui.end_row();
            });
            ui.vertical_centered(|ui| {
                if button(ui, "Adjust").on_hover_text("Drag on the picture: where you press is a grid point, the drag sets the spacing").clicked() {
                    adjust = true;
                }
            });
            done = ok_cancel(ui, 230.0);
        });
        self.v5.x.r8.grid = Some((gx, gy));
        if adjust {
            self.v5.x.r8.grid_adjust = true;
            self.v5.x.r8.grid_adjust_from = None;
            self.show_grid = false;
            self.status = "Grid: press at a grid point and drag to the next one".into();
            return;
        }
        if let Some(ok) = done {
            if ok {
                self.set.grid.0 = gx;
                self.set.grid.1 = gy;
                self.set.grid_on = true;
            }
            self.show_grid = false;
        }
    }

    /// Grid > Adjust on the page. Returns true when it used the pointer.
    pub fn grid_adjust_pointer(&mut self, pos: (i32, i32), pressed: bool, released: bool) -> bool {
        if !self.v5.x.r8.grid_adjust {
            return false;
        }
        if pressed {
            self.v5.x.r8.grid_adjust_from = Some(pos);
        }
        if released {
            if let Some(a) = self.v5.x.r8.grid_adjust_from.take() {
                let gw = (pos.0 - a.0).abs().max(1);
                let gh = (pos.1 - a.1).abs().max(1);
                self.set.grid = (gw, gh, a.0.rem_euclid(gw), a.1.rem_euclid(gh));
                self.set.grid_on = true;
                self.v5.x.r8.grid = Some((gw, gh));
                self.v5.x.r8.grid_adjust = false;
                self.show_grid = true;
                self.status.clear();
            }
        }
        true
    }

    // ---------------------------------------------------------------- Spacing (fig. 8.4)

    fn spacing_req8(&mut self, ctx: &egui::Context) {
        if !self.show_spacing {
            self.v5.x.r8.spacing = None;
            return;
        }
        let st = &self.set;
        let (mut kind, mut nt, mut nth, mut air) = self.v5.x.r8.spacing.unwrap_or_else(|| {
            let k = if st.spacing_total > 0 {
                0
            } else if st.spacing > 1 {
                1
            } else if st.spacing_air {
                2
            } else {
                3
            };
            let nt = if st.spacing_total > 0 { st.spacing_total } else { 20 };
            let nth = if st.spacing > 1 { st.spacing } else { 8 };
            (k, nt, nth, st.spacing_air_n.max(1))
        });
        let mut done = None;
        req("Spacing").show(ctx, |ui| {
            ui.set_width(250.0);
            egui::Grid::new("spacing8").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                if toggle(ui, "N Total", kind == 0).on_hover_text("Exactly N dots spread evenly over the whole line or stroke").clicked() {
                    kind = 0;
                }
                ui.add(egui::DragValue::new(&mut nt).range(2..=999));
                ui.end_row();
                if toggle(ui, "Every Nth Dot", kind == 1).on_hover_text("A dot every N pixels along the line").clicked() {
                    kind = 1;
                }
                ui.add(egui::DragValue::new(&mut nth).range(2..=999));
                ui.end_row();
                if toggle(ui, "Airbrush", kind == 2).on_hover_text("N airbrush sprays for every pixel along the line").clicked() {
                    kind = 2;
                }
                ui.add(egui::DragValue::new(&mut air).range(1..=999));
                ui.end_row();
                if toggle(ui, "Continuous", kind == 3).on_hover_text("The brush at every point of the line").clicked() {
                    kind = 3;
                }
                ui.end_row();
            });
            done = ok_cancel(ui, 250.0);
        });
        self.v5.x.r8.spacing = Some((kind, nt, nth, air));
        if let Some(ok) = done {
            if ok {
                let st = &mut self.set;
                st.spacing_total = if kind == 0 { nt.max(2) } else { 0 };
                st.spacing = if kind == 1 { nth.max(2) } else { 0 };
                st.spacing_air = kind == 2;
                st.spacing_air_n = air.max(1);
            }
            self.show_spacing = false;
        }
    }

    // ---------------------------------------------------------------- Airbrush (fig. 8.13)

    fn airbrush_req8(&mut self, ctx: &egui::Context) {
        if !self.show_airbrush {
            self.v5.x.r8.air = None;
            self.v5.x.r8.air_test = None;
            return;
        }
        let (mut old, mut radius, mut flow) = self.v5.x.r8.air.unwrap_or((self.set.airbrush_old, self.set.airbrush_radius, self.set.airbrush_density));
        const TW: i32 = 120;
        const TH: i32 = 90;
        let bg = self.bg;
        let mut test = self.v5.x.r8.air_test.take().unwrap_or_else(|| {
            let mut d = Document::new(TW, TH, bg);
            d.palette = self.doc.palette;
            d
        });
        test.palette = self.doc.palette;
        let mut done = None;
        let mut clear = false;
        let mut size = self.v5.x.r8.air_size;
        let mut spray_at: Option<(i32, i32)> = None;
        req("Airbrush").show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(220.0);
                    ui.label("Airbrush Type");
                    ui.horizontal(|ui| {
                        if toggle(ui, "Random", old).on_hover_text("Sprays pixels or whole brush prints at random").clicked() {
                            old = true;
                        }
                        let can_std = !self.brush.custom;
                        if ui.add_enabled_ui(can_std, |ui| toggle(ui, "Standard", !old)).inner.on_hover_text("Builds the colour up gradually (round built-in brushes)").clicked() {
                            old = false;
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Radius");
                        ui.add(egui::Slider::new(&mut radius, 1..=100).show_value(false));
                        ui.add(egui::DragValue::new(&mut radius).range(1..=100));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Flow  ");
                        ui.add(egui::Slider::new(&mut flow, 1..=100).show_value(false));
                        ui.add(egui::DragValue::new(&mut flow).range(1..=100));
                    });
                    ui.horizontal(|ui| {
                        if toggle(ui, "Size", size).on_hover_text("Drag on the picture to size the nozzle").clicked() {
                            size = !size;
                        }
                        if button(ui, "Clear Area").clicked() {
                            clear = true;
                        }
                    });
                });
                // The test area: try the airbrush here.
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(TW as f32 * 1.5, TH as f32 * 1.5), egui::Sense::drag());
                let p = ui.painter_at(rect);
                let s = 1.5f32;
                for y in 0..TH {
                    for x in 0..TW {
                        let c = test.palette[test.frames[0].get(x, y) as usize];
                        p.rect_filled(egui::Rect::from_min_size(rect.min + Vec2::new(x as f32 * s, y as f32 * s), Vec2::splat(s)), 0.0, Color32::from_rgb(c[0], c[1], c[2]));
                    }
                }
                crate::ui_style::bevel(&p, rect, 1.5, true);
                if resp.is_pointer_button_down_on() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let v = (pos - rect.min) / s;
                        spray_at = Some((v.x as i32, v.y as i32));
                        ctx.request_repaint();
                    }
                }
            });
            done = ok_cancel(ui, 420.0);
        });
        if clear {
            test.frames[0].fill(bg);
        }
        if let Some((x, y)) = spray_at {
            let mut set = self.set.clone();
            set.airbrush_old = old;
            set.airbrush_radius = radius;
            set.airbrush_density = flow;
            set.sym.on = false;
            set.texture_on = false;
            set.persp_plane = None;
            let mut st = Stroke::new(self.fg, true);
            st.rng = paint::Rng::new(self.rng.next());
            let b = self.brush.clone();
            paint::airbrush(&mut test, &b, &set, &mut st, x, y);
        }
        self.v5.x.r8.air = Some((old, radius, flow));
        self.v5.x.r8.air_test = Some(test);
        self.v5.x.r8.air_size = size;
        if let Some(ok) = done {
            if ok {
                self.set.airbrush_old = old;
                self.set.airbrush_radius = radius;
                self.set.airbrush_density = flow;
                if size {
                    self.v5.x.r8.air_size = true;
                    self.status = "Airbrush size: drag on the picture from the centre outwards".into();
                }
            }
            self.show_airbrush = false;
        }
    }

    /// Airbrush > Size: a drag on the page sets the nozzle radius. Returns true when it used the pointer.
    pub fn air_size_pointer(&mut self, pos: (i32, i32), pressed: bool, released: bool) -> bool {
        if !self.v5.x.r8.air_size || self.show_airbrush {
            return false;
        }
        if pressed {
            self.v5.x.r8.grid_adjust_from = Some(pos);
        }
        if let Some(a) = self.v5.x.r8.grid_adjust_from {
            let r = ((((pos.0 - a.0).pow(2) + (pos.1 - a.1).pow(2)) as f32).sqrt()).round() as i32;
            self.set.airbrush_radius = r.clamp(1, 100);
            self.status = format!("Airbrush radius {}", self.set.airbrush_radius);
            if released {
                self.v5.x.r8.grid_adjust_from = None;
                self.v5.x.r8.air_size = false;
            }
        }
        true
    }

    // ---------------------------------------------------------------- Translucency (fig. 8.43)

    fn transl_req8(&mut self, ctx: &egui::Context) {
        if !self.show_translucent {
            self.v5.x.r8.transl = None;
            return;
        }
        let mut t = self.v5.x.r8.transl.unwrap_or(self.set.translucent);
        let mut done = None;
        req("Translucency").show(ctx, |ui| {
            ui.set_width(240.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().slider_width = 180.0;
                ui.add(egui::Slider::new(&mut t, 0..=99).show_value(false));
                ui.label(format!("{t}%"));
            });
            done = ok_cancel(ui, 240.0);
        });
        self.v5.x.r8.transl = Some(t);
        if let Some(ok) = done {
            if ok {
                self.set.translucent = t;
            }
            self.show_translucent = false;
        }
    }

    // ---------------------------------------------------------------- Set # (fig. 8.32), Go to (fig. 8.38)

    fn frames_req8(&mut self, ctx: &egui::Context) {
        if !self.v5.show_frames_set {
            self.v5.x.r8.frames_n = None;
            return;
        }
        let mut n = self.v5.x.r8.frames_n.unwrap_or(self.doc.frames.len());
        let mut done = None;
        req("Set # of Frames").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Count:");
                ui.add(egui::DragValue::new(&mut n).range(1..=999));
            });
            done = ok_cancel(ui, 200.0);
        });
        self.v5.x.r8.frames_n = Some(n);
        if let Some(ok) = done {
            if ok {
                set_frames_after_current(&mut self.doc, n);
            }
            self.v5.show_frames_set = false;
        }
    }

    fn goto_req8(&mut self, ctx: &egui::Context) {
        if !self.v5.show_goto {
            self.v5.x.r8.goto_n = None;
            return;
        }
        let total = self.doc.frames.len();
        let mut n = self.v5.x.r8.goto_n.unwrap_or(self.doc.cur + 1);
        let mut done = None;
        req("Go to Frame").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("No.:");
                ui.add(egui::DragValue::new(&mut n).range(1..=total));
            });
            done = ok_cancel(ui, 200.0);
        });
        self.v5.x.r8.goto_n = Some(n);
        if let Some(ok) = done {
            if ok {
                self.v5.goto = n.clamp(1, total);
                self.doc.go_to(self.v5.goto - 1);
            }
            self.v5.show_goto = false;
        }
    }
}

/// Set # of Frames: frames are added (copies of the current one) or removed after the current
/// frame, so frames can be inserted or deleted in the middle (manual p. 244).
pub fn set_frames_after_current(doc: &mut Document, n: usize) {
    let n = n.clamp(1, 999);
    let have = doc.frames.len();
    doc.sync_palettes();
    if n > have {
        doc.add_frames(n - have);
    } else if n < have {
        let remove = have - n;
        let from = (doc.cur + 1).min(have);
        let to = (from + remove).min(have);
        if to > from {
            doc.delete_frames(from, to - 1);
        }
        // Not enough frames after the current one: the rest from the end.
        let left = doc.frames.len().saturating_sub(n);
        if left > 0 {
            doc.set_frame_count(n);
        }
    }
    doc.dirty = true;
}

/// Brush > Solid Body: see-through pixels completely enclosed by coloured pixels take the colour
/// of the nearest coloured pixel to their left (the see-through edge of the brush stays).
pub fn solid_body(b: &crate::pix::Brush) -> crate::pix::Brush {
    let (w, h) = (b.img.w, b.img.h);
    let mut outside = vec![false; (w * h) as usize];
    let mut stack: Vec<(i32, i32)> = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= w || y >= h {
            continue;
        }
        let i = (y * w + x) as usize;
        if outside[i] || b.is_solid(x, y) {
            continue;
        }
        outside[i] = true;
        stack.extend_from_slice(&[(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
    }
    let mut out = b.clone();
    for y in 0..h {
        let mut last = None;
        for x in 0..w {
            let i = (y * w + x) as usize;
            if b.is_solid(x, y) {
                last = Some(b.img.get(x, y));
            } else if !outside[i] {
                if let Some(c) = last {
                    out.img.set(x, y, c);
                }
            }
        }
    }
    out
}
