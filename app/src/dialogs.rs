//! Dialog windows and the actions behind them.

use crate::app::{AnimBrush, App, ConvertJob, PalAction, Pending};
use crate::convert::{ConvertOptions, Method, PaletteChoice};
use crate::doc::{Document, Stencil};
use crate::gallery::{self, Gallery, Show, Transition};
use crate::paint::{self, FillKind, Stroke, SymKind};
use crate::palette::{hsv_to_rgb, rgb_to_hsv, to_color32, Range};
use crate::pix::{Brush, Image};
use egui::{Color32, Vec2};

/// Screen formats from the original's start-up list, offered as page-size presets.
pub const SCREEN_FORMATS: [(&str, i32, i32); 14] = [
    ("320 × 200 (MCGA / VGA low-res)", 320, 200),
    ("640 × 200 (EGA / VGA)", 640, 200),
    ("640 × 350 (EGA)", 640, 350),
    ("640 × 400 (Extended VGA)", 640, 400),
    ("640 × 480 (VGA)", 640, 480),
    ("720 × 348 (Hercules)", 720, 348),
    ("800 × 600 (Super VGA)", 800, 600),
    ("1024 × 768 (Super VGA)", 1024, 768),
    ("320 × 256 (Amiga PAL low-res)", 320, 256),
    ("640 × 512 (Amiga PAL hi-res interlace)", 640, 512),
    ("640 × 870 (Full Page)", 640, 870),
    ("736 × 566 (Overscan)", 736, 566),
    ("1280 × 720 (HD)", 1280, 720),
    ("1920 × 1080 (Full HD)", 1920, 1080),
];

fn win(title: &str) -> egui::Window<'static> {
    egui::Window::new(egui::RichText::new(title.to_string()).color(Color32::WHITE)).collapsible(false).resizable(false)
}

impl App {
    pub fn dialogs(&mut self, ctx: &egui::Context) {
        self.message_dialog(ctx);
        self.pending_dialog(ctx);
        self.new_dialog(ctx);
        self.fill_dialog(ctx);
        self.reqs8(ctx);
        self.rotate_dialog(ctx);
        self.resize_brush_dialog(ctx);
        self.perspective_dialog(ctx);
        self.anim_dialog(ctx);
        self.stencil_dialog(ctx);
        self.font_dialog(ctx);
        self.convert_dialog(ctx);
        self.gallery_dialog(ctx);
        self.camera_dialog(ctx);
        self.about_dialog(ctx);
        self.keys_dialog(ctx);
    }

    pub(crate) fn message_dialog(&mut self, ctx: &egui::Context) {
        let Some((title, text)) = self.message.clone() else { return };
        let mut close = false;
        // A message has its own window id: its title is often a requester's title ("Fill Type"),
        // and two windows with one id clash ("Double use of widget ID").
        win(&title).id(egui::Id::new("message-window")).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.label(text);
            ui.add_space(6.0);
            if crate::ui_style::button(ui, "OK").clicked() {
                close = true;
            }
        });
        if close {
            self.message = None;
        }
    }

    pub(crate) fn pending_dialog(&mut self, ctx: &egui::Context) {
        let Some(p) = self.pending else { return };
        let what = match p {
            Pending::New => "creating new picture?",
            Pending::Open => "loading another picture?",
            Pending::Quit => "quitting?",
            Pending::Revert => "reverting to the last saved version?",
        };
        let mut choice = None;
        win("Unsaved changes").anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            ui.label(format!("Save changes to picture \"{}\" before {}", self.doc.name(), what));
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "Save").clicked() {
                    choice = Some(0);
                }
                if crate::ui_style::button(ui, "Discard").clicked() {
                    choice = Some(1);
                }
                if crate::ui_style::button(ui, "Cancel").clicked() {
                    choice = Some(2);
                }
            });
        });
        match choice {
            Some(0) => {
                self.pending = None;
                self.save(false);
                if !self.doc.dirty {
                    self.do_pending(p);
                }
            }
            Some(1) => {
                self.pending = None;
                self.doc.dirty = false;
                self.do_pending(p);
            }
            Some(2) => self.pending = None,
            _ => {}
        }
    }

    pub(crate) fn new_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_new {
            return;
        }
        let mut open = true;
        let mut create = false;
        win("New picture").open(&mut open).show(ctx, |ui| {
            ui.label("Choose a page size (the original's screen formats are listed):");
            for (name, w, h) in SCREEN_FORMATS {
                if ui.selectable_label(self.new_size == (w, h), name).clicked() {
                    self.new_size = (w, h);
                }
            }
            ui.horizontal(|ui| {
                ui.label("Width:");
                ui.add(egui::DragValue::new(&mut self.new_size.0).range(8..=8192));
                ui.label("Height:");
                ui.add(egui::DragValue::new(&mut self.new_size.1).range(8..=8192));
            });
            if crate::ui_style::button(ui, "Create").clicked() {
                create = true;
            }
        });
        if create {
            let mut d = Document::new(self.new_size.0, self.new_size.1, self.bg);
            d.palette = self.doc.palette;
            d.ranges = self.doc.ranges.clone();
            self.doc = d;
            self.fit_pending = true;
            open = false;
        }
        self.show_new = open;
    }

    pub(crate) fn page_size_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_page_size {
            return;
        }
        let mut open = true;
        let mut apply = false;
        win("Page Size").open(&mut open).show(ctx, |ui| {
            for (name, w, h) in SCREEN_FORMATS {
                if ui.selectable_label((self.page_size.0, self.page_size.1) == (w, h), name).clicked() {
                    self.page_size.0 = w;
                    self.page_size.1 = h;
                }
            }
            ui.horizontal(|ui| {
                ui.label("Width:");
                ui.add(egui::DragValue::new(&mut self.page_size.0).range(8..=8192));
                ui.label("Height:");
                ui.add(egui::DragValue::new(&mut self.page_size.1).range(8..=8192));
            });
            ui.checkbox(&mut self.page_size.2, "Stretch the picture to the new size (otherwise pad or crop)");
            ui.colored_label(Color32::from_rgb(128, 0, 0), "Warning: This change cannot be undone.");
            if crate::ui_style::button(ui, "Apply").clicked() {
                apply = true;
            }
        });
        if apply {
            let (w, h, st) = self.page_size;
            self.doc.resize_page(w, h, self.bg, st);
            self.fit_pending = true;
            open = false;
        }
        self.show_page_size = open;
    }

    pub(crate) fn palette_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_palette {
            return;
        }
        let mut open = true;
        win("Palette").open(&mut open).default_pos([200.0, 120.0]).show(ctx, |ui| {
            let i = self.fg as usize;
            let mut c = self.doc.palette[i];
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(64.0, 64.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 6.0, to_color32(c));
                ui.vertical(|ui| {
                    ui.label(format!("Edit colour #{i}"));
                    ui.label(format!("Hex #{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
                });
            });
            let before = c;
            if self.doc.ehb && (32..64).contains(&i) {
                ui.label(format!("Extra-Half-Brite: this colour is always colour {} at half brightness.", i - 32));
            }
            if self.doc.gun_bits < 8 {
                // Amiga OCS/ECS palette: 16 levels per gun (4096 colours).
                for (k, name) in ["Red", "Green", "Blue"].iter().enumerate() {
                    let mut v = c[k] / 17;
                    if ui.add(egui::Slider::new(&mut v, 0..=15).text(*name)).changed() {
                        c[k] = v * 17;
                    }
                }
            } else {
                ui.add(egui::Slider::new(&mut c[0], 0..=255).text("Red"));
                ui.add(egui::Slider::new(&mut c[1], 0..=255).text("Green"));
                ui.add(egui::Slider::new(&mut c[2], 0..=255).text("Blue"));
            }
            let (mut h, mut s, mut v) = rgb_to_hsv(c);
            let hsv_before = (h, s, v);
            ui.add(egui::Slider::new(&mut h, 0.0..=359.0).text("Hue"));
            ui.add(egui::Slider::new(&mut s, 0.0..=1.0).text("Saturation"));
            ui.add(egui::Slider::new(&mut v, 0.0..=1.0).text("Value"));
            if (h, s, v) != hsv_before {
                c = hsv_to_rgb(h, s, v);
            }
            if c != before {
                if self.pal_undo.is_none() {
                    self.pal_undo = Some(self.doc.palette);
                }
                self.doc.palette[i] = c;
                self.doc.dirty = true;
            }
            ui.separator();
            ui.label("Then click a second colour in the palette strip:");
            ui.horizontal(|ui| {
                for (a, name) in [(PalAction::Spread, "Spread"), (PalAction::Copy, "Copy"), (PalAction::Swap, "Swap"), (PalAction::RangeFromTo, "Set range")] {
                    if crate::ui_style::toggle(ui, name, self.pal_action == Some(a)).clicked() {
                        self.pal_action = if self.pal_action == Some(a) { None } else { Some(a) };
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled_ui(self.pal_undo.is_some(), |ui| crate::ui_style::button(ui, "Undo")).inner.clicked() {
                    if let Some(p) = self.pal_undo.take() {
                        self.doc.palette = p;
                    }
                }
                if crate::ui_style::button(ui, "Default").clicked() {
                    self.pal_undo = Some(self.doc.palette);
                    self.doc.palette = crate::palette::DEFAULT_PALETTE;
                }
            });
            ui.separator();
            ui.label("Colour ranges (for Shade, Cycle, gradients and colour cycling):");
            let mut remove = None;
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                for (ri, r) in self.doc.ranges.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.radio(self.set.range == ri, "").clicked() {
                            self.set.range = ri;
                        }
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(96.0, 14.0), egui::Sense::hover());
                        let n = r.len();
                        for k in 0..n {
                            let x0 = rect.min.x + rect.width() * k as f32 / n as f32;
                            let x1 = rect.min.x + rect.width() * (k + 1) as f32 / n as f32;
                            let cr = egui::Rect::from_min_max(egui::pos2(x0, rect.min.y), egui::pos2(x1, rect.max.y));
                            ui.painter().rect_filled(cr, 0.0, to_color32(self.doc.palette[r.low as usize + k]));
                        }
                        let top = (self.doc.colors.clamp(2, 256) - 1) as u8;
                        ui.add(egui::DragValue::new(&mut r.low).range(0..=top).prefix("from "));
                        ui.add(egui::DragValue::new(&mut r.high).range(0..=top).prefix("to "));
                        if r.high < r.low {
                            std::mem::swap(&mut r.high, &mut r.low);
                        }
                        let mut sps = r.steps_per_second();
                        if ui.add(egui::Slider::new(&mut sps, 0.0..=60.0).text("steps/s").max_decimals(1)).changed() {
                            r.rate = (sps * 16384.0 / 60.0).round().clamp(0.0, 16384.0) as u16;
                        }
                        ui.checkbox(&mut r.active, "on");
                        ui.checkbox(&mut r.reverse, "reverse");
                        if crate::ui_style::button(ui, "Del").clicked() {
                            remove = Some(ri);
                        }
                    });
                }
            });
            if let Some(ri) = remove {
                self.doc.ranges.remove(ri);
                self.set.range = self.set.range.min(self.doc.ranges.len().saturating_sub(1));
            }
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "Add range").clicked() {
                    let lo = self.fg;
                    self.doc.ranges.push(Range::new(lo, lo.saturating_add(15), 2560));
                    self.set.range = self.doc.ranges.len() - 1;
                }
                ui.checkbox(&mut self.cycling, "Cycle now (Tab)");
            });
        });
        self.show_palette = open;
    }

    pub(crate) fn fill_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_fill {
            return;
        }
        let mut open = true;
        win("Fill Type").open(&mut open).show(ctx, |ui| {
            for k in FillKind::ALL {
                ui.radio_value(&mut self.set.fill, k, k.label());
            }
            ui.separator();
            ui.add(egui::Slider::new(&mut self.set.fill_angle, 0.0..=359.0).text("Gradient direction (degrees)"));
            ui.add(egui::Slider::new(&mut self.set.spatter, 0..=100).text("Grad. Spatter %"));
            ui.label("Gradients use the current colour range (choose it in the Palette).");
            let r = self.doc.ranges.get(self.set.range).cloned();
            if let Some(r) = r {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(220.0, 16.0), egui::Sense::hover());
                let n = r.len();
                for k in 0..n {
                    let x0 = rect.min.x + rect.width() * k as f32 / n as f32;
                    let x1 = rect.min.x + rect.width() * (k + 1) as f32 / n as f32;
                    let cr = egui::Rect::from_min_max(egui::pos2(x0, rect.min.y), egui::pos2(x1, rect.max.y));
                    ui.painter().rect_filled(cr, 0.0, to_color32(self.doc.palette[r.low as usize + k]));
                }
            }
        });
        self.show_fill = open;
    }

    pub(crate) fn symmetry_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_symmetry {
            return;
        }
        let mut open = true;
        win("Symmetry Options").open(&mut open).show(ctx, |ui| {
            ui.checkbox(&mut self.set.sym.on, "Symmetry on  (/)");
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.set.sym.kind, SymKind::Cyclic, "Cyclic");
                ui.radio_value(&mut self.set.sym.kind, SymKind::Tile, "Tile");
            });
            if self.set.sym.kind == SymKind::Cyclic {
                ui.add(egui::Slider::new(&mut self.set.sym.points, 1..=40).text("Points"));
                ui.checkbox(&mut self.set.sym.mirror, "Mirror");
                ui.horizontal(|ui| {
                    if crate::ui_style::button(ui, "Place center…").clicked() {
                        self.sym_place = true;
                    }
                    if crate::ui_style::button(ui, "Center of page").clicked() {
                        self.set.sym.center = None;
                    }
                });
                if self.sym_place {
                    ui.colored_label(Color32::from_rgb(128, 0, 0), "Click on the picture to place the center.");
                }
            } else {
                ui.add(egui::Slider::new(&mut self.set.sym.tile_w, 2..=512).text("Tile width"));
                ui.add(egui::Slider::new(&mut self.set.sym.tile_h, 2..=512).text("Tile height"));
            }
        });
        self.show_symmetry = open;
    }

    pub(crate) fn grid_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_grid {
            return;
        }
        let mut open = true;
        win("Grid").open(&mut open).show(ctx, |ui| {
            ui.checkbox(&mut self.set.grid_on, "Grid on  (g)");
            ui.add(egui::Slider::new(&mut self.set.grid.0, 1..=256).text("X Spacing"));
            ui.add(egui::Slider::new(&mut self.set.grid.1, 1..=256).text("Y Spacing"));
            ui.add(egui::Slider::new(&mut self.set.grid.2, 0..=255).text("X Offset"));
            ui.add(egui::Slider::new(&mut self.set.grid.3, 0..=255).text("Y Offset"));
            if ui.add_enabled_ui(self.brush.custom, |ui| crate::ui_style::button(ui, "From Brush")).inner.clicked() {
                self.set.grid.0 = self.brush.img.w.max(1);
                self.set.grid.1 = self.brush.img.h.max(1);
            }
        });
        self.show_grid = open;
    }

    pub(crate) fn spacing_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_spacing {
            self.v5.x.spacing_was_open = false;
            return;
        }
        // On opening, read the current spacing (a script or macro may have changed it); after that
        // the requester keeps the chosen option itself, and each option keeps its own N.
        if !self.v5.x.spacing_was_open {
            let st = &self.set;
            let x = &mut self.v5.x;
            x.spacing_kind = if st.spacing_air {
                3
            } else if st.spacing_total > 0 {
                x.spacing_ntotal = st.spacing_total;
                1
            } else if st.spacing > 1 {
                x.spacing_nth = st.spacing;
                2
            } else {
                0
            };
            x.spacing_was_open = true;
        }
        let mut open = true;
        win("Spacing").open(&mut open).resizable(false).show(ctx, |ui| {
            // A fixed width, so nothing moves under the pointer when the option changes.
            ui.set_width(300.0);
            let x = &mut self.v5.x;
            for (k, l, tip) in [
                (0u8, "Continuous", "The brush is put down at every point of the line"),
                (1, "N Total", "Exactly N dots, spread evenly over the whole line, shape or stroke"),
                (2, "Every Nth Dot", "A dot every N pixels along the line"),
                (3, "Airbrush", "Little airbrush bursts along the line (Airbrush settings)"),
            ] {
                ui.radio_value(&mut x.spacing_kind, k, l).on_hover_text(tip);
            }
            ui.add_space(4.0);
            // Always the same row: the N of the chosen option (greyed when it has none).
            ui.horizontal(|ui| {
                let label = match x.spacing_kind {
                    1 => "N (dots in all)",
                    2 => "N (pixels apart)",
                    _ => "N",
                };
                match x.spacing_kind {
                    1 => {
                        ui.add(egui::Slider::new(&mut x.spacing_ntotal, 2..=500).text(label));
                    }
                    2 => {
                        ui.add(egui::Slider::new(&mut x.spacing_nth, 2..=200).text(label));
                    }
                    _ => {
                        let mut dummy = 0;
                        ui.add_enabled(false, egui::Slider::new(&mut dummy, 0..=1).text(label));
                    }
                }
            });
            // The painting settings follow the requester.
            let st = &mut self.set;
            st.spacing = if x.spacing_kind == 2 { x.spacing_nth.max(2) } else { 0 };
            st.spacing_total = if x.spacing_kind == 1 { x.spacing_ntotal.max(2) } else { 0 };
            st.spacing_air = x.spacing_kind == 3;
        });
        self.show_spacing = open;
    }

    pub(crate) fn airbrush_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_airbrush {
            return;
        }
        let mut open = true;
        win("Airbrush").open(&mut open).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.radio(self.set.airbrush_old, "Old Style").on_hover_text("Single pixels, as in older versions").clicked() {
                    self.set.airbrush_old = true;
                }
                if ui.radio(!self.set.airbrush_old, "Standard").on_hover_text("Sprays the current brush").clicked() {
                    self.set.airbrush_old = false;
                }
            });
            ui.add(egui::Slider::new(&mut self.set.airbrush_radius, 2..=128).text("Spray radius"));
            ui.add(egui::Slider::new(&mut self.set.airbrush_density, 1..=100).text("Flow"));
        });
        self.show_airbrush = open;
    }

    pub(crate) fn translucent_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_translucent {
            return;
        }
        let mut open = true;
        win("Translucent").open(&mut open).show(ctx, |ui| {
            ui.checkbox(&mut self.set.translucent_on, "Translucent on  (Alt+T)");
            ui.add(egui::Slider::new(&mut self.set.translucent, 0..=99).text("Translucent %"));
        });
        self.show_translucent = open;
    }

    pub(crate) fn rotate_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_rotate {
            return;
        }
        let mut open = true;
        let mut action = None;
        win("Rotate / Bend / Shear brush").open(&mut open).show(ctx, |ui| {
            ui.add(egui::Slider::new(&mut self.rotate_deg, -180.0..=180.0).text("Angle (degrees)"));
            if crate::ui_style::button(ui, "Rotate by angle").clicked() {
                action = Some(0);
            }
            ui.separator();
            ui.add(egui::Slider::new(&mut self.shear_amt, -256..=256).text("Amount (pixels)"));
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "Shear left-and-right").clicked() {
                    action = Some(1);
                }
                if crate::ui_style::button(ui, "Shear up-and-down").clicked() {
                    action = Some(2);
                }
            });
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "Bend left-and-right").clicked() {
                    action = Some(3);
                }
                if crate::ui_style::button(ui, "Bend up-and-down").clicked() {
                    action = Some(4);
                }
            });
            ui.checkbox(&mut self.antialias, "Anti-alias the result");
        });
        let (deg, amt, aa, pal, high) = (self.rotate_deg, self.shear_amt, self.antialias, self.doc.palette, self.aa_high);
        match action {
            Some(0) => self.transform_brush(|b| {
                let r = b.rotated(deg);
                if aa { r.antialiased_level(&pal, high) } else { r }
            }),
            Some(1) => self.transform_brush(|b| b.sheared(true, amt)),
            Some(2) => self.transform_brush(|b| b.sheared(false, amt)),
            Some(3) => self.transform_brush(|b| b.bent(true, amt)),
            Some(4) => self.transform_brush(|b| b.bent(false, amt)),
            _ => {}
        }
        self.show_rotate = open;
    }

    pub(crate) fn resize_brush_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_resize_brush {
            return;
        }
        let mut open = true;
        let mut apply = false;
        win("Stretch brush").open(&mut open).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Width:");
                ui.add(egui::DragValue::new(&mut self.brush_size_edit.0).range(1..=4096));
                ui.label("Height:");
                ui.add(egui::DragValue::new(&mut self.brush_size_edit.1).range(1..=4096));
            });
            if crate::ui_style::button(ui, "Stretch").clicked() {
                apply = true;
            }
        });
        if apply {
            let (w, h) = self.brush_size_edit;
            self.transform_brush(|b| b.scaled(w, h));
            open = false;
        }
        self.show_resize_brush = open;
    }

    /// The Perspective requester (Effect > Perspective > Settings, or right-click the grid tool in
    /// perspective mode): grid X Y Z (From Brush), angle step, Type (Screen / Brush axes),
    /// Display (Angle / Pos); also the angles and the perspective depth.
    pub(crate) fn perspective_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_perspective {
            self.v5.x.p3_edit = None;
            return;
        }
        if self.v5.x.p3_edit.is_none() {
            let mut p = self.v5.x.p3.clone();
            if !self.v5.x.persp_mode {
                p.set_angles([self.persp.0, self.persp.1, self.persp.2]);
            }
            self.v5.x.p3_edit = Some((p, self.v5.x.persp_depth));
        }
        let (bw, bh) = (self.v5.x.persp_base.as_ref().unwrap_or(&self.brush).img.w, self.v5.x.persp_base.as_ref().unwrap_or(&self.brush).img.h);
        let mut done: Option<bool> = None;
        let (p, depth) = self.v5.x.p3_edit.as_mut().unwrap();
        win("Perspective").collapsible(false).resizable(false).show(ctx, |ui| {
            egui::Grid::new("persp_grid").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
                ui.label("");
                ui.label("X");
                ui.label("Y");
                ui.label("Z");
                ui.end_row();
                ui.label("Grid:");
                for k in 0..3 {
                    ui.add(egui::DragValue::new(&mut p.grid[k]).range(1..=999));
                }
                ui.end_row();
            });
            ui.horizontal(|ui| {
                ui.add_space(52.0);
                if crate::ui_style::button(ui, "From Brush").on_hover_text("Grid = the brush's width and height").clicked() {
                    p.grid[0] = bw.max(1);
                    p.grid[1] = bh.max(1);
                    p.grid[2] = bw.max(bh).max(1);
                }
            });
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.label("Step:");
                    ui.add(egui::DragValue::new(&mut p.step).range(1.0..=180.0).suffix("°"));
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.label("Type:");
                    if ui.radio(!p.brush_axes, "Screen").on_hover_text("Turn about the screen's axes: the same angles always give the same result").clicked() {
                        p.brush_axes = false;
                    }
                    if ui.radio(p.brush_axes, "Brush").on_hover_text("Turn about the brush's own axes").clicked() {
                        p.brush_axes = true;
                    }
                });
            });
            ui.separator();
            ui.vertical_centered(|ui| ui.label("Display:"));
            ui.horizontal(|ui| {
                ui.add_space(60.0);
                ui.vertical(|ui| {
                    if ui.radio(!p.show_pos, "Angle").clicked() {
                        p.show_pos = false;
                    }
                    if ui.radio(p.show_pos, "Pos").clicked() {
                        p.show_pos = true;
                    }
                });
            });
            ui.separator();
            let mut a = p.angles;
            ui.horizontal(|ui| {
                ui.label("Angles:");
                for v in a.iter_mut() {
                    ui.add(egui::DragValue::new(v).range(-180.0..=180.0).speed(1.0).suffix("°"));
                }
            });
            if a != p.angles {
                p.set_angles(a);
            }
            ui.horizontal(|ui| {
                ui.label("Depth:");
                ui.add(egui::DragValue::new(&mut p.z).speed(1.0));
                ui.label("Perspective:");
                ui.add(egui::Slider::new(depth, 0.3..=20.0).logarithmic(true).show_value(false)).on_hover_text("How far away the eye is: left = strong perspective");
            });
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "Ok").clicked() {
                    done = Some(true);
                }
                ui.add_space(120.0);
                if crate::ui_style::button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            if let Some((p, d)) = self.v5.x.p3_edit.take() {
                if ok {
                    self.persp = (p.angles[0], p.angles[1], p.angles[2]);
                    self.v5.x.p3 = p;
                    self.v5.x.persp_depth = d;
                    self.v5.x.p3.last_key = None;
                }
            }
            self.show_perspective = false;
        }
    }

    pub(crate) fn anim_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_anim {
            return;
        }
        let mut open = true;
        win("Animation").open(&mut open).show(ctx, |ui| {
            ui.label(format!("Frame {} of {}", self.doc.cur + 1, self.doc.frames.len()));
            let mut f = self.doc.cur + 1;
            if ui.add(egui::Slider::new(&mut f, 1..=self.doc.frames.len()).text("Go to frame")).changed() {
                self.doc.cur = f - 1;
            }
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.frames_edit).range(1..=999).prefix("Frames: "));
                if crate::ui_style::button(ui, "Set number of frames").clicked() {
                    self.doc.set_frame_count(self.frames_edit);
                }
            });
            ui.add(egui::Slider::new(&mut self.doc.fps, 1.0..=60.0).text("Frames per second"));
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "⏮").clicked() {
                    self.doc.prev_frame();
                }
                if crate::ui_style::button(ui, if self.playing { "⏹ Stop" } else { "▶ Play" }).clicked() {
                    self.playing = !self.playing;
                }
                if crate::ui_style::button(ui, "⏭").clicked() {
                    self.doc.next_frame();
                }
            });
            ui.checkbox(&mut self.light_table, "Light table");
            // Film strip.
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    let pal = self.doc.palette;
                    for (i, fr) in self.doc.frames.iter().enumerate().take(200) {
                        let th = 40.0;
                        let tw = th * fr.w as f32 / fr.h as f32;
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(tw, th), egui::Sense::click());
                        // Coarse thumbnail: sample a 12×8 grid.
                        let gx = 12;
                        let gy = 8;
                        for yy in 0..gy {
                            for xx in 0..gx {
                                let c = fr.get(xx * fr.w / gx, yy * fr.h / gy);
                                let r = egui::Rect::from_min_size(
                                    rect.min + Vec2::new(xx as f32 * tw / gx as f32, yy as f32 * th / gy as f32),
                                    Vec2::new(tw / gx as f32 + 0.5, th / gy as f32 + 0.5),
                                );
                                ui.painter().rect_filled(r, 0.0, to_color32(pal[c as usize]));
                            }
                        }
                        let border = if i == self.doc.cur { crate::ui_style::ACCENT } else { Color32::from_gray(90) };
                        ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(2.0, border), egui::StrokeKind::Outside);
                        if resp.clicked() {
                            self.doc.cur = i;
                        }
                    }
                });
            });
        });
        self.show_anim = open;
    }

    pub(crate) fn move_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_move {
            return;
        }
        let mut open = true;
        let mut go = false;
        win("Move").open(&mut open).show(ctx, |ui| {
            ui.label("Draws the brush into a run of frames, moving and turning it a little each time.");
            egui::Grid::new("movegrid").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("");
                ui.label("X");
                ui.label("Y");
                ui.label("Z");
                ui.end_row();
                ui.label("Dist:");
                for k in 0..3 {
                    ui.add(egui::DragValue::new(&mut self.move_params.dist[k]).range(-4000..=4000));
                }
                ui.end_row();
                ui.label("Angle:");
                for k in 0..3 {
                    ui.add(egui::DragValue::new(&mut self.move_params.angle[k]).range(-1440.0..=1440.0).suffix("°"));
                }
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "Zero").clicked() {
                    self.move_params.dist = [0; 3];
                    self.move_params.angle = [0.0; 3];
                }
                ui.checkbox(&mut self.move_params.cyclic, "Cyclic");
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Slow In:");
                ui.add(egui::DragValue::new(&mut self.move_params.slow_in).range(0..=999));
                ui.label("Slow Out:");
                ui.add(egui::DragValue::new(&mut self.move_params.slow_out).range(0..=999));
                ui.label("Count:");
                ui.add(egui::DragValue::new(&mut self.move_params.frames).range(1..=999));
            });
            ui.horizontal(|ui| {
                ui.label("Start at x/y:");
                ui.add(egui::DragValue::new(&mut self.move_params.start.0));
                ui.add(egui::DragValue::new(&mut self.move_params.start.1));
                if crate::ui_style::button(ui, "Centre").clicked() {
                    self.move_params.start = (self.doc.w / 2, self.doc.h / 2);
                }
            });
            ui.label(format!("Starts at frame {} of {}.", self.doc.cur + 1, self.doc.frames.len()));
            if !self.brush.custom {
                ui.colored_label(Color32::from_rgb(128, 0, 0), "Tip: pick up a brush first (b) — Move works best with a custom brush.");
            }
            if crate::ui_style::button(ui, "Draw").clicked() {
                go = true;
            }
        });
        if go {
            self.do_move();
        }
        self.show_move = open;
    }

    /// Position along the move (0..1) for step `k` of `n`, with slow-in and slow-out easing.
    pub(crate) fn move_progress(k: usize, n: usize, slow_in: usize, slow_out: usize) -> f32 {
        if n <= 1 {
            return 0.0;
        }
        let t = k as f32 / (n - 1) as f32;
        let si = (slow_in as f32 / (n - 1) as f32).clamp(0.0, 0.5);
        let so = (slow_out as f32 / (n - 1) as f32).clamp(0.0, 0.5);
        if si == 0.0 && so == 0.0 {
            return t;
        }
        // Trapezoidal speed profile: accelerate over `si`, cruise, decelerate over `so`.
        let v = 1.0 / (1.0 - si / 2.0 - so / 2.0);
        if t < si {
            v * t * t / (2.0 * si)
        } else if t > 1.0 - so {
            let r = 1.0 - t;
            1.0 - v * r * r / (2.0 * so)
        } else {
            v * (t - si / 2.0)
        }
    }

    #[allow(dead_code)]
    pub fn move_progress_pub(k: usize, n: usize, si: usize, so: usize) -> f32 {
        Self::move_progress(k, n, si, so)
    }

    pub(crate) fn do_move(&mut self) {
        let mp = self.move_params.clone();
        let total = self.doc.frames.len();
        let n = if mp.cyclic { mp.frames } else { mp.frames.min(total - self.doc.cur) }.max(1);
        let start_frame = self.doc.cur;
        let base_brush = self.brush.clone();
        let pal = self.doc.palette;
        let eye = 300.0f32;
        for k in 0..n {
            let t = Self::move_progress(k, n, mp.slow_in, mp.slow_out);
            self.doc.cur = (start_frame + k) % total;
            self.doc.checkpoint();
            let mut b = base_brush.clone();
            if let Some(ab) = &self.anim_brush {
                b.img = ab.cells[k % ab.cells.len()].clone();
            }
            let (ax, ay, az) = (mp.angle[0] * t, mp.angle[1] * t, mp.angle[2] * t);
            if ax != 0.0 || ay != 0.0 {
                b = b.perspective(ax, ay, az, self.antialias, &pal);
            } else if az != 0.0 {
                b = b.rotated(az);
            }
            let z = mp.dist[2] as f32 * t;
            if z != 0.0 {
                let s = (eye / (eye + z)).clamp(0.02, 50.0);
                b = b.scaled(((b.img.w as f32 * s).round() as i32).max(1), ((b.img.h as f32 * s).round() as i32).max(1));
            }
            let x = mp.start.0 + (mp.dist[0] as f32 * t).round() as i32;
            let y = mp.start.1 + (mp.dist[1] as f32 * t).round() as i32;
            let mut st = Stroke::new(self.fg, true);
            paint::stamp(&mut self.doc, &b, &self.set, &mut st, x, y);
        }
        self.doc.cur = start_frame;
        self.status = format!("Move drew the brush into {n} frames");
    }

    pub fn pick_anim_brush(&mut self) {
        let Some((x, y, w, h)) = self.last_pick else { return };
        let cells: Vec<Image> = self.doc.frames.iter().map(|f| f.crop(x, y, w, h)).collect();
        self.brush = Brush::from_image(cells[0].clone(), Some(self.bg));
        self.anim_brush = Some(AnimBrush::new(cells));
        self.status = format!("Animated brush with {} cells", self.doc.frames.len());
    }

    pub(crate) fn stencil_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_stencil {
            return;
        }
        let mut open = true;
        let mut make = false;
        win("Make Stencil").open(&mut open).show(ctx, |ui| {
            ui.label("Pick the colours to protect (click them here or in the palette strip):");
            let pal = self.doc.palette;
            let cell = 14.0;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(cell * 32.0, cell * 8.0), egui::Sense::click());
            for i in 0..256usize {
                let r = egui::Rect::from_min_size(rect.min + Vec2::new((i % 32) as f32 * cell, (i / 32) as f32 * cell), Vec2::splat(cell - 1.0));
                ui.painter().rect_filled(r, 1.0, to_color32(pal[i]));
                if self.stencil_pick[i] {
                    ui.painter().rect_stroke(r, 1.0, egui::Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Inside);
                    ui.painter().line_segment([r.left_top(), r.right_bottom()], egui::Stroke::new(1.5, Color32::BLACK));
                }
            }
            if resp.clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let v = p - rect.min;
                    let i = (v.y / cell) as usize * 32 + (v.x / cell) as usize;
                    if i < 256 {
                        self.stencil_pick[i] = !self.stencil_pick[i];
                    }
                }
            }
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "Clear").clicked() {
                    self.stencil_pick = [false; 256];
                }
                if crate::ui_style::button(ui, "Invert").clicked() {
                    self.stencil_pick.iter_mut().for_each(|b| *b = !*b);
                }
                if crate::ui_style::button(ui, "Make").clicked() {
                    make = true;
                }
            });
        });
        if make {
            let mask = paint::stencil_from_colors(self.doc.img(), &self.stencil_pick);
            self.doc.stencil = Some(Stencil { mask, on: true });
            self.status = "Stencil on: protected colours can't be painted".into();
            open = false;
        }
        self.show_stencil = open;
    }

    pub fn lock_foreground(&mut self) {
        let bg = self.bg;
        let mask: Vec<bool> = self.doc.img().px.iter().map(|&p| p != bg).collect();
        self.doc.stencil = Some(Stencil { mask, on: true });
        self.status = "Foreground locked: paint goes behind what is drawn".into();
    }

    /// Choose Font, laid out like the original's (the person's Amiga screenshot): the font list
    /// and the size list side by side, a name field and a size field under them, Plain / Bold /
    /// Italic / Underlined, a sample line in the chosen font, OK / Cancel.
    pub(crate) fn font_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_font {
            self.v5.x.font_edit = None;
            return;
        }
        if self.fonts.is_empty() {
            let mut close = false;
            win("Choose Font").show(ctx, |ui| {
                ui.label("No fonts were found on this computer; the text tool is unavailable.");
                close = crate::ui_style::button(ui, "OK").clicked();
            });
            if close {
                self.show_font = false;
            }
            return;
        }
        let ts = &self.text_style;
        let mut e = self.v5.x.font_edit.take().unwrap_or(crate::v52::FontEdit {
            idx: self.font_idx.min(self.fonts.len() - 1),
            size: ts.size,
            bold: ts.bold,
            italic: ts.italic,
            underline: ts.underline,
            preview: None,
            preview_key: None,
        });
        let sizes: Vec<u32> = {
            let f = &self.fonts[e.idx];
            if f.is_bitmap() { f.sizes.iter().map(|(s, _)| *s).collect() } else { crate::text::FONT_SIZES.to_vec() }
        };
        let mut done: Option<bool> = None;
        win("Choose Font").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.horizontal_top(|ui| {
                // The font list.
                ui.vertical(|ui| {
                    egui::Frame::new().fill(Color32::from_gray(170)).stroke(egui::Stroke::new(1.5, Color32::BLACK)).show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("font_names").max_height(150.0).min_scrolled_height(150.0).show(ui, |ui| {
                            ui.set_width(270.0);
                            for (i, f) in self.fonts.iter().enumerate() {
                                let sel = i == e.idx;
                                let t = egui::RichText::new(&f.name).color(if sel { Color32::WHITE } else { Color32::BLACK });
                                let t = if sel { t.background_color(crate::ui_style::TEAL) } else { t };
                                let r = ui.add(egui::Label::new(t).sense(egui::Sense::click()));
                                if sel && e.preview_key.is_none() {
                                    r.scroll_to_me(Some(egui::Align::Center));
                                }
                                if r.clicked() {
                                    e.idx = i;
                                    if let Some((s, _)) = f.nearest_size(e.size) {
                                        e.size = *s;
                                    }
                                }
                            }
                        });
                    });
                    let mut name = self.fonts[e.idx].name.clone();
                    ui.add_enabled(false, egui::TextEdit::singleline(&mut name).desired_width(270.0));
                });
                // The size list.
                ui.vertical(|ui| {
                    egui::Frame::new().fill(Color32::from_gray(170)).stroke(egui::Stroke::new(1.5, Color32::BLACK)).show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("font_sizes").max_height(150.0).min_scrolled_height(150.0).show(ui, |ui| {
                            ui.set_width(50.0);
                            for s in &sizes {
                                let sel = *s == e.size;
                                let t = egui::RichText::new(s.to_string()).color(if sel { Color32::WHITE } else { Color32::BLACK });
                                let t = if sel { t.background_color(crate::ui_style::TEAL) } else { t };
                                if ui.add(egui::Label::new(t).sense(egui::Sense::click())).clicked() {
                                    e.size = *s;
                                }
                            }
                        });
                    });
                    let bitmap = self.fonts[e.idx].is_bitmap();
                    if ui.add_enabled(!bitmap, egui::DragValue::new(&mut e.size).range(4..=400)).changed() {
                        e.size = e.size.clamp(4, 400);
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(200.0);
                    let mut plain = !e.bold && !e.italic && !e.underline;
                    if ui.checkbox(&mut plain, "Plain").clicked() && plain {
                        e.bold = false;
                        e.italic = false;
                        e.underline = false;
                    }
                    ui.checkbox(&mut e.bold, "Bold");
                });
                ui.vertical(|ui| {
                    ui.checkbox(&mut e.italic, "Italic");
                    ui.checkbox(&mut e.underline, "Underlined");
                });
            });
            ui.add_space(4.0);
            // The sample line in the chosen font, size and style.
            let key = (e.idx, e.size);
            if e.preview_key != Some(key) {
                e.preview = crate::text::load_entry(&self.fonts[e.idx], e.size);
                e.preview_key = Some(key);
            }
            let (rect, _) = ui.allocate_exact_size(egui::vec2(360.0, 44.0), egui::Sense::hover());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 0.0, Color32::from_gray(160));
            crate::ui_style::bevel(&p, rect, 1.5, true);
            if let Some(font) = &e.preview {
                let style = crate::text::TextStyle { size: e.size, bold: e.bold, italic: e.italic, underline: e.underline, antialias: false };
                let sample = "123 AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQq";
                if let Some(b) = crate::text::render_any(font, sample, &style, 1, 0, &self.doc.palette) {
                    let s = (36.0 / b.img.h.max(1) as f32).min(1.0);
                    let o = rect.min + egui::vec2(6.0, (rect.height() - b.img.h as f32 * s) / 2.0);
                    for y in 0..b.img.h {
                        for x in 0..b.img.w {
                            let c = b.img.get(x, y);
                            if Some(c) != b.transparent && o.x + x as f32 * s < rect.max.x - 4.0 {
                                let col = if c == 1 { Color32::BLACK } else { crate::palette::to_color32(self.doc.palette[c as usize]) };
                                p.rect_filled(egui::Rect::from_min_size(o + egui::vec2(x as f32 * s, y as f32 * s), egui::Vec2::splat(s.max(1.0))), 0.0, col);
                            }
                        }
                    }
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "OK").clicked() {
                    done = Some(true);
                }
                ui.add_space(250.0);
                if crate::ui_style::button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        match done {
            Some(ok) => {
                if ok {
                    self.font_idx = e.idx;
                    self.text_style.size = e.size;
                    self.text_style.bold = e.bold;
                    self.text_style.italic = e.italic;
                    self.text_style.underline = e.underline;
                    self.font = e.preview.take().or_else(|| crate::text::load_entry(&self.fonts[e.idx], e.size));
                    if self.font.is_none() {
                        self.msg("Font", format!("Couldn't load font: {}", self.fonts[e.idx].name));
                    }
                }
                self.show_font = false;
            }
            None => self.v5.x.font_edit = Some(e),
        }
    }

    // ------------------------------------------------------------ convert

    pub fn import_convert(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Convert a picture")
            .add_filter("Pictures", &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "tga", "lbm", "pcx"])
            .pick_file()
        {
            match crate::fileio::load(&p) {
                Ok(crate::fileio::LoadResult::TrueColor(rgba)) => {
                    self.convert = Some(ConvertJob { rgba, opts: ConvertOptions::default(), path: Some(p), as_picture: true });
                }
                Ok(crate::fileio::LoadResult::Indexed(l)) => {
                    // Already indexed: offer conversion anyway (resize / new palette).
                    let img = &l.frames[0];
                    let px = img.px.iter().map(|&i| l.palette[i as usize]).collect();
                    let rgba = crate::convert::Rgba { w: img.w, h: img.h, px };
                    self.convert = Some(ConvertJob { rgba, opts: ConvertOptions::default(), path: Some(p), as_picture: true });
                }
                Err(e) => self.msg("Convert", e),
            }
        }
    }

    pub(crate) fn convert_dialog(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.convert else { return };
        let mut apply = false;
        let mut cancel = false;
        win("Convert picture").show(ctx, |ui| {
            ui.label(format!("The picture is {} × {} in full colour.", job.rgba.w, job.rgba.h));
            ui.separator();
            ui.label("Size:");
            let mut resize = job.opts.resize.is_some();
            ui.checkbox(&mut resize, "Resize");
            if resize {
                let mut wh = job.opts.resize.unwrap_or((job.rgba.w, job.rgba.h));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut wh.0).range(8..=8192).prefix("W "));
                    ui.add(egui::DragValue::new(&mut wh.1).range(8..=8192).prefix("H "));
                    ui.radio_value(&mut job.opts.stretch, true, "Stretch");
                    ui.radio_value(&mut job.opts.stretch, false, "Pad");
                });
                job.opts.resize = Some(wh);
            } else {
                job.opts.resize = None;
            }
            ui.separator();
            ui.label("Colours:");
            egui::ComboBox::from_id_salt("palchoice").selected_text(job.opts.palette.label()).show_ui(ui, |ui| {
                for p in PaletteChoice::ALL {
                    ui.selectable_value(&mut job.opts.palette, p, p.label());
                }
            });
            if matches!(job.opts.palette, PaletteChoice::MostCommon | PaletteChoice::Grey) {
                egui::ComboBox::from_label("Number of colours").selected_text(job.opts.colors.to_string()).show_ui(ui, |ui| {
                    for n in [2usize, 4, 8, 16, 32, 64, 128, 256] {
                        ui.selectable_value(&mut job.opts.colors, n, n.to_string());
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.radio_value(&mut job.opts.method, Method::FloydSteinberg, "Floyd-Steinberg dither");
                ui.radio_value(&mut job.opts.method, Method::Closest, "Closest colour");
            });
            ui.radio_value(&mut job.as_picture, true, "Open as a new picture");
            ui.radio_value(&mut job.as_picture, false, "Use as a brush");
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "Convert").clicked() {
                    apply = true;
                }
                if crate::ui_style::button(ui, "Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if apply {
            self.apply_convert();
        } else if cancel {
            self.convert = None;
        }
    }

    // ------------------------------------------------------------ camera

    pub(crate) fn camera_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_camera {
            return;
        }
        let mut open = true;
        let mut go = false;
        win("Camera").open(&mut open).show(ctx, |ui| {
            ui.label("Grabs the whole screen into a new picture.");
            ui.add(egui::Slider::new(&mut self.camera_delay, 1.0..=10.0).text("Seconds to wait (this window hides meanwhile)"));
            if crate::ui_style::button(ui, "Grab screen").clicked() {
                go = true;
            }
        });
        if go {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            self.camera_at = Some(self.time + self.camera_delay as f64);
            open = false;
        }
        self.show_camera = open;
    }

    // ------------------------------------------------------------ gallery

    pub(crate) fn gallery_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_gallery {
            return;
        }
        let mut open = true;
        let mut run = false;
        win("Gallery").open(&mut open).resizable(true).default_size([520.0, 420.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if crate::ui_style::button(ui, "New").clicked() {
                    self.gallery = Gallery::default();
                }
                if crate::ui_style::button(ui, "Load…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("Gallery", &["gal"]).pick_file() {
                        match Gallery::load(&p) {
                            Ok((g, problems)) => {
                                self.gallery = g;
                                if !problems.is_empty() {
                                    self.msg("Gallery", format!("Some slides couldn't be loaded:\n{}", problems.join("\n")));
                                }
                            }
                            Err(e) => self.msg("Gallery", e),
                        }
                    }
                }
                if crate::ui_style::button(ui, "Save…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("Gallery", &["gal"]).set_file_name("untitled.gal").save_file() {
                        if let Err(e) = self.gallery.save(&p) {
                            self.msg("Gallery", e);
                        }
                    }
                }
                if crate::ui_style::button(ui, "Insert picture…").clicked() {
                    if let Some(ps) = rfd::FileDialog::new()
                        .add_filter("Pictures", &["lbm", "bbm", "pcx", "bmp", "png", "gif", "jpg", "jpeg"])
                        .pick_files()
                    {
                        self.insert_slides(ps);
                    }
                }
                if crate::ui_style::button(ui, "Insert all from folder…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        let mut ps: Vec<_> = std::fs::read_dir(dir)
                            .map(|rd| rd.flatten().map(|e| e.path()).collect())
                            .unwrap_or_default();
                        ps.retain(|p: &std::path::PathBuf| {
                            let e = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                            ["lbm", "bbm", "pcx", "bmp", "png", "gif", "jpg", "jpeg"].contains(&e.as_str())
                        });
                        ps.sort();
                        self.insert_slides(ps);
                    }
                }
                if crate::ui_style::button(ui, "Insert this picture").clicked() {
                    self.gallery.slides.push(gallery::Slide {
                        path: self.doc.path.clone().unwrap_or_else(|| "untitled".into()),
                        img: self.doc.img().clone(),
                        palette: self.doc.palette,
                        ranges: self.doc.ranges.clone(),
                        delay: 3,
                        transition: Transition::Fade,
                    });
                }
            });
            ui.separator();
            let mut remove = None;
            let mut up = None;
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for (i, s) in self.gallery.slides.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{:>3}.", i + 1));
                        let name = s.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or("picture".into());
                        if ui.selectable_label(self.gallery.selected == i, name).clicked() {
                            self.gallery.selected = i;
                        }
                        egui::ComboBox::from_id_salt(("delay", i)).width(60.0).selected_text(format!("{} sec", s.delay)).show_ui(ui, |ui| {
                            for d in 1..=9u8 {
                                ui.selectable_value(&mut s.delay, d, format!("{d} sec"));
                            }
                        });
                        egui::ComboBox::from_id_salt(("tr", i)).width(130.0).selected_text(s.transition.label()).show_ui(ui, |ui| {
                            for t in Transition::ALL {
                                ui.selectable_value(&mut s.transition, t, t.label());
                            }
                        });
                        if crate::ui_style::button(ui, "▲").clicked() && i > 0 {
                            up = Some(i);
                        }
                        if crate::ui_style::button(ui, "Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                }
            });
            if let Some(i) = up {
                self.gallery.slides.swap(i, i - 1);
            }
            if let Some(i) = remove {
                self.gallery.slides.remove(i);
            }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.gallery.manual, "Manual switch");
                ui.checkbox(&mut self.gallery.black_start, "Black screen at start");
                ui.checkbox(&mut self.gallery.black_end, "Black screen at end");
                ui.checkbox(&mut self.gallery.cycling, "Colour cycling");
                ui.checkbox(&mut self.gallery.common_palette, "Common palette");
            });
            ui.horizontal(|ui| {
                ui.label("Set all:");
                for t in Transition::ALL {
                    if crate::ui_style::button(ui, t.label()).clicked() {
                        self.gallery.slides.iter_mut().for_each(|s| s.transition = t);
                    }
                }
            });
            if ui.add_enabled_ui(!self.gallery.slides.is_empty(), |ui| crate::ui_style::button(ui, "▶ Run Slideshow")).inner.clicked() {
                run = true;
            }
            ui.label(egui::RichText::new("During the show: Space / click = next, Backspace = previous, Esc = stop.").small());
        });
        if run {
            self.start_show();
        }
        self.show_gallery = open;
    }

    pub(crate) fn insert_slides(&mut self, paths: Vec<std::path::PathBuf>) {
        let mut problems = Vec::new();
        for p in paths {
            if self.gallery.slides.len() >= gallery::MAX_SLIDES {
                problems.push("Gallery cannot hold any more slides.".to_string());
                break;
            }
            match gallery::load_slide(&p) {
                Ok(s) => self.gallery.slides.push(s),
                Err(e) => problems.push(format!("{}: {e}", p.display())),
            }
        }
        if !problems.is_empty() {
            self.msg("Gallery", problems.join("\n"));
        }
    }

    pub(crate) fn start_show(&mut self) {
        let (w, h) = self.gallery.frame_size();
        let black = vec![[0u8; 3]; (w * h) as usize];
        let first_tr = self.gallery.slides[0].transition;
        self.show = Some(Show {
            index: 0,
            phase_start: self.time,
            in_transition: self.gallery.black_start,
            from: black,
            transition: first_tr,
            finished: false,
            start_time: self.time,
            ending: false,
        });
        if !self.gallery.black_start {
            if let Some(s) = &mut self.show {
                s.in_transition = false;
            }
        }
    }

    pub fn slideshow_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ctx.request_repaint();
        let (w, h) = self.gallery.frame_size();
        let n = self.gallery.slides.len();
        let (next, prev, stop) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Space) || i.key_pressed(egui::Key::ArrowRight) || i.pointer.primary_clicked(),
                i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::Escape) || i.pointer.secondary_clicked(),
            )
        });
        let now = self.time;
        let manual = self.gallery.manual;
        let black_end = self.gallery.black_end;
        let Some(show) = &mut self.show else { return };
        if stop || show.finished {
            self.show = None;
            return;
        }
        let elapsed = (now - show.phase_start) as f32;
        let t_show = (now - show.start_time) as f32;
        let mut tr = show.transition;
        if tr == Transition::Random {
            let k = (show.index * 7 + 3) % (Transition::ALL.len() - 1);
            tr = Transition::ALL[k + 1];
        }
        let cur_rgb = if show.ending { vec![[0u8; 3]; (w * h) as usize] } else { self.gallery.slide_rgb(show.index, t_show) };
        let frame = if show.in_transition && tr != Transition::Cut {
            let p = elapsed / gallery::TRANSITION_SECONDS;
            if p >= 1.0 {
                show.in_transition = false;
                show.phase_start = now;
                if show.ending {
                    show.finished = true;
                }
                cur_rgb.clone()
            } else {
                gallery::compose(tr, &show.from, &cur_rgb, w, h, p)
            }
        } else {
            if show.in_transition {
                show.in_transition = false;
                show.phase_start = now;
                if show.ending {
                    show.finished = true;
                }
            }
            cur_rgb.clone()
        };
        // Advance?
        let delay = self.gallery.slides.get(show.index).map(|s| s.delay as f32).unwrap_or(3.0);
        let auto = !manual && !show.in_transition && !show.ending && (now - show.phase_start) as f32 >= delay;
        if (next || auto) && !show.ending {
            show.from = cur_rgb.clone();
            show.phase_start = now;
            show.in_transition = true;
            if show.index + 1 < n {
                show.index += 1;
                show.transition = self.gallery.slides[show.index].transition;
            } else if black_end {
                show.ending = true;
                show.transition = Transition::Fade;
            } else {
                show.finished = true;
            }
        } else if prev && show.index > 0 && !show.ending {
            show.index -= 1;
            show.in_transition = false;
            show.phase_start = now;
        }
        let px: Vec<Color32> = frame.iter().map(|c| Color32::from_rgb(c[0], c[1], c[2])).collect();
        let img = egui::ColorImage::new([w as usize, h as usize], px);
        match &mut self.gallery_tex {
            Some(t) => t.set(img, egui::TextureOptions::NEAREST),
            None => self.gallery_tex = Some(ctx.load_texture("gallery", img, egui::TextureOptions::NEAREST)),
        }
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let full = ui.available_rect_before_wrap();
            ui.painter().rect_filled(full, 0.0, Color32::BLACK);
            let z = (full.width() / w as f32).min(full.height() / h as f32);
            let r = egui::Rect::from_center_size(full.center(), Vec2::new(w as f32, h as f32) * z);
            if let Some(t) = &self.gallery_tex {
                ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            }
        });
    }

    // ------------------------------------------------------------ misc dialogs

    pub(crate) fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        win("About").open(&mut open).show(ctx, |ui| {
            ui.heading("Deluxe Pixel");
            ui.label("A modern 256-colour pixel paint program for Windows,");
            ui.label("modelled on the classic DOS and Amiga paint programs:");
            ui.label("painting modes, brushes, stencils, gradients, colour cycling,");
            ui.label("animation, slideshows, conversion and screen capture.");
        });
        self.show_about = open;
    }

    pub(crate) fn keys_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_keys {
            return;
        }
        let mut open = true;
        win("Keyboard shortcuts").open(&mut open).resizable(true).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                egui::Grid::new("keys").striped(true).show(ui, |ui| {
                    for (k, v) in KEYS {
                        ui.monospace(*k);
                        ui.label(*v);
                        ui.end_row();
                    }
                });
            });
        });
        self.show_keys = open;
    }

    // ------------------------------------------------------------ file helpers

    pub fn print(&mut self) {
        let tmp = std::env::temp_dir().join(format!("{}-print.png", self.doc.name().replace('.', "_")));
        let pal = self.doc.palette;
        match crate::fileio::save_png(&tmp, self.doc.img(), &pal, None).and_then(|_| crate::capture::print_file(&tmp)) {
            Ok(()) => self.status = "Sent to the printer".into(),
            Err(e) => self.msg("Print", e),
        }
    }

    pub fn load_brush(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .set_title("Load Brush")
            .add_filter("Brushes and pictures", &["bbm", "lbm", "brush", "pat", "lores", "hires", "iff", "ilbm", "png", "pcx", "bmp", "gif", "jpg"])
            .add_filter("All files (Amiga brushes often have no extension)", &["*"])
            .pick_file()
        else {
            return;
        };
        match crate::fileio::load(&p) {
            Ok(crate::fileio::LoadResult::Indexed(l)) => {
                // The file's see-through colour (BMHD masking 2) and handle (GRAB), as DeluxePaint
                // saves them; colour 0 when the file doesn't say.
                let t = l.transparent.unwrap_or(0);
                let mut b = Brush::from_image(l.frames[0].clone(), Some(t));
                if let Some((gx, gy)) = l.grab {
                    b.hx = (gx as i32).clamp(0, b.img.w - 1);
                    b.hy = (gy as i32).clamp(0, b.img.h - 1);
                }
                let (hx, hy) = (b.hx, b.hy);
                self.brush = b.remapped(&l.palette, &self.doc.palette);
                self.brush.hx = hx;
                self.brush.hy = hy;
                let tc = l.palette[t as usize];
                self.brush.transparent = Some(crate::palette::nearest(&self.doc.palette, [tc[0] as i32, tc[1] as i32, tc[2] as i32], 256));
                self.brush.name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.v5.brush_palette = Some(l.palette);
                self.brush_original = None;
                self.anim_brush = None;
                self.tool = crate::app::Tool::Dotted;
            }
            Ok(crate::fileio::LoadResult::TrueColor(rgba)) => {
                self.convert = Some(ConvertJob {
                    rgba,
                    opts: ConvertOptions { palette: PaletteChoice::Current, ..Default::default() },
                    path: Some(p),
                    as_picture: false,
                });
            }
            Err(e) => self.msg("Load Brush", e),
        }
    }

    pub fn save_brush(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .set_title("Save Brush")
            .set_file_name("untitled.bbm")
            .add_filter("Brush (IFF .bbm)", &["bbm"])
            .add_filter("PNG with transparency", &["png"])
            .save_file()
        else {
            return;
        };
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let r = if ext == "png" {
            crate::fileio::save_png(&p, &self.brush.img, &self.doc.palette, self.brush.transparent)
        } else {
            crate::fileio::save_ilbm(&p, &self.brush.img, &self.doc.palette, &[], self.brush.transparent)
        };
        if let Err(e) = r {
            self.msg("Save Brush", e);
        }
    }

    pub fn save_anim(&mut self) {
        let name = format!("{}.anim", self.doc.name().split('.').next().unwrap_or("untitled"));
        let Some(p) = rfd::FileDialog::new().set_title("Save Anim").set_file_name(&name).add_filter("Deluxe Paint animation (IFF ANIM)", &["anim"]).save_file() else {
            return;
        };
        self.doc.sync_palettes();
        if let Err(e) = crate::fileio::save_anim_timed(&p, &self.doc.frames, &self.doc.palette, &self.doc.ranges, self.doc.fps, &self.doc.frame_palettes, &self.doc.frame_delays) {
            self.msg("Save animation", e);
        } else {
            self.status = format!("Saved {} frames", self.doc.frames.len());
        }
    }

    pub fn save_frames(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .set_title("Save frames (a number is added to each name)")
            .set_file_name("frame.lbm")
            .add_filter("DeluxePaint picture", &["lbm"])
            .add_filter("PNG", &["png"])
            .save_file()
        else {
            return;
        };
        match crate::fileio::save_frames(&p, &self.doc.frames, &self.doc.palette, &self.doc.ranges) {
            Ok(n) => self.status = format!("Saved {n} pictures"),
            Err(e) => self.msg("Save frames", e),
        }
    }

    pub fn load_stencil(&mut self) {
        let Some(p) = rfd::FileDialog::new().set_title("Load Stencil").add_filter("Stencil", &["sbm", "lbm", "png"]).pick_file() else {
            return;
        };
        match crate::fileio::load(&p) {
            Ok(crate::fileio::LoadResult::Indexed(l)) => {
                let img = &l.frames[0];
                if img.w != self.doc.w || img.h != self.doc.h {
                    self.msg("Load Stencil", "Wrong size Stencil.");
                    return;
                }
                let mask = img.px.iter().map(|&p| p != 0).collect();
                self.doc.stencil = Some(Stencil { mask, on: true });
            }
            Ok(_) => self.msg("Load Stencil", "Not a Stencil file."),
            Err(e) => self.msg("Load Stencil", e),
        }
    }

    pub fn save_stencil(&mut self) {
        let Some(st) = &self.doc.stencil else {
            self.msg("Save Stencil", "No Stencil is defined.");
            return;
        };
        let Some(p) = rfd::FileDialog::new().set_title("Save Stencil").set_file_name("untitled.sbm").add_filter("Stencil", &["sbm"]).save_file() else {
            return;
        };
        let mut img = Image::new(self.doc.w, self.doc.h, 0);
        for (i, &m) in st.mask.iter().enumerate() {
            img.px[i] = m as u8;
        }
        let mut pal = crate::palette::DEFAULT_PALETTE;
        pal[0] = [0, 0, 0];
        pal[1] = [255, 255, 255];
        if let Err(e) = crate::fileio::save_ilbm(&p, &img, &pal, &[], None) {
            self.msg("Save Stencil", e);
        }
    }

    pub fn remap_to_spare(&mut self) {
        self.doc.checkpoint();
        let from = self.doc.palette;
        let to = self.spare.palette;
        let mut map = [0u8; 256];
        for i in 0..256 {
            map[i] = crate::palette::nearest(&to, [from[i][0] as i32, from[i][1] as i32, from[i][2] as i32], 256);
        }
        for f in self.doc.frames.iter_mut() {
            for p in f.px.iter_mut() {
                *p = map[*p as usize];
            }
        }
        self.doc.palette = to;
    }
}

pub const KEYS: &[(&str, &str)] = &[
    ("Ctrl", "stands for the original's Amiga key; Ctrl+Shift for its Ctrl key"),
    ("s  d / D", "Dotted freehand, continuous freehand / filled freehand"),
    ("v  q", "Straight line, curve (drag, then bend and click)"),
    ("f / F", "Fill / Fill Type requester"),
    ("a / A", "Airbrush / airbrush settings"),
    ("r R  c C  e E  w W", "Rectangle, circle, ellipse, polygon (capital = filled)"),
    ("b / B", "Brush selector (right button cuts) / Restore brush"),
    ("t / T", "Text / Fonts requester"),
    (",", "Pick a colour from the picture"),
    (". = -", "1-pixel brush, bigger, smaller built-in brush"),
    ("[ ]  { }", "Previous/next foreground colour, background colour"),
    ("g G  / ", "Grid on/off, grid settings, symmetry on/off"),
    ("m  n", "Magnify on/off, magnify here; arrows move the zoom area"),
    ("u, Ctrl+Z / Ctrl+Y", "Undo / redo"),
    ("K", "Clear (CLR)"),
    ("Ctrl+L  Ctrl+S  Ctrl+D  Ctrl+P  Ctrl+Q", "Load, Save, Delete file, Print, Quit"),
    ("j  Ctrl+Shift+J", "Swap with spare page, copy to spare page"),
    ("S  ?", "Show Page, About"),
    ("Ctrl+B  Ctrl+C  Ctrl+V", "Load brush, copy brush, paste brush"),
    ("Alt+N  Alt+B", "Brush->Spare, Brush<->Spare"),
    ("Z h H X Y", "Stretch, halve, double, double horizontally, double vertically"),
    ("x y z  o O", "Flip horiz/vert, rotate 90°, outline, trim"),
    ("Alt+X  Alt+Z  Alt+F", "Rotate handle, place handle, Frame Only"),
    ("F1…F8", "Matte, Colour, Repl, Smear, Shade, Blend, Cycle, Smooth"),
    ("Ctrl+O  Ctrl+A", "Load / save animation"),
    ("M  N", "Move, Camera Move"),
    ("1 2 3 4 5 6", "Previous, next, go to, play, play once, ping-pong"),
    ("Alt+A", "Animation control panel"),
    ("Ctrl+R  Ctrl+Shift+F", "Frame rate, copy frames"),
    ("Ctrl+Shift+R  Tab  p  P", "Ranges, cycle colours, Mixer, Arrange"),
    ("Alt+[  Alt+]  Alt+R", "Previous / next range, reverse range"),
    ("~  `  Ctrl+Shift+S", "Make stencil, stencil on/off, stencil paint"),
    ("|  Alt+L  Ctrl+Shift+1…4", "Light table on/off, dim, layers"),
    ("Alt+/  Alt+P  Alt+T", "Anti-alias, Process, Translucency"),
    ("Ctrl+T  Ctrl+I", "Texture on/off, invert texture"),
    ("Ctrl+M  Ctrl+1…0", "Record macro, run User macros 1-10"),
    ("\\  V", "Coordinates, Spacing requester"),
    ("F9  F11  Alt+Enter", "Info bar, fullscreen"),
    ("Alt+C", "Camera (grab the screen)"),
    ("Mouse wheel, middle drag", "Zoom, scroll the page"),
];

#[allow(dead_code)]
fn _unused(_: FillKind) {}
