//! Requesters of the DeluxePaint V functions.

use crate::app::App;
use crate::doc::Document;
use crate::paint::{Dither, FillKind};
use crate::palette::to_color32;
use crate::pix::Brush;
use crate::amiga::{Chipset, MODES};
use crate::ui_style::{bevel, button, toggle, BLACK, GREY, WHITE};
use egui::{Color32, RichText, Vec2};
use std::sync::Arc;

fn req(title: &str) -> egui::Window<'static> {
    egui::Window::new(RichText::new(title.to_string()).color(Color32::WHITE)).collapsible(false).resizable(false)
}

const PAGE_KINDS: [&str; 4] = ["Screen", "Full Page", "Overscan", "Custom"];
const SCREEN_KINDS: [&str; 2] = ["Standard", "Overscan"];

impl App {
    pub fn dialogs5(&mut self, ctx: &egui::Context) {
        self.fill_v5(ctx);
        self.screen_format(ctx);
        self.about_v5(ctx);
        self.frame_requesters(ctx);
        self.camera_move_req(ctx);
        self.morph_req(ctx);
        self.animbrush_req(ctx);
        self.handle_req(ctx);
        self.lt_settings_req(ctx);
        self.pressure_req(ctx);
        self.media_req(ctx);
        self.arrange_req(ctx);
        self.macro_reqs(ctx);
        self.confirm_delete_req(ctx);
    }

    // ---------------------------------------------------------------- Fill Type

    fn fill_v5(&mut self, ctx: &egui::Context) {
        if !self.v5.show_fill_v5 {
            return;
        }
        let mut close: Option<bool> = None;
        req("Fill Type").show(ctx, |ui| {
            ui.set_width(330.0);
            let has_spare = self.spare.w == self.doc.w && self.spare.h == self.doc.h;
            ui.horizontal(|ui| {
                for k in [FillKind::Solid, FillKind::Brush, FillKind::Wrap] {
                    if toggle(ui, k.label(), self.set.fill == k).clicked() {
                        self.set.fill = k;
                    }
                }
            });
            ui.horizontal(|ui| {
                if toggle(ui, "Perspective", self.set.fill == FillKind::Perspective).clicked() {
                    self.set.fill = FillKind::Perspective;
                }
                ui.add_enabled_ui(has_spare, |ui| {
                    if toggle(ui, "Rub", self.set.fill == FillKind::Rub).on_hover_text("Rub the spare page through").clicked() {
                        self.set.fill = FillKind::Rub;
                    }
                });
                if toggle(ui, "HBrite", self.set.fill == FillKind::HBrite).on_hover_text("Darken to half brightness").clicked() {
                    self.set.fill = FillKind::HBrite;
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if toggle(ui, "Pattern", self.set.fill == FillKind::Pattern).clicked() {
                        self.set.fill = FillKind::Pattern;
                    }
                    ui.add_space(4.0);
                    if button(ui, "Load..").on_hover_text("A pattern file (DeluxePaint .PAT, or any brush or picture)").clicked() {
                        self.load_pattern();
                    }
                    if button(ui, "From brush").on_hover_text("Make the fill pattern from the current brush").clicked() {
                        if self.brush.custom {
                            self.set.pattern = Some(Arc::new(self.brush.clone()));
                            self.set.fill = FillKind::Pattern;
                        } else {
                            self.msg("Fill Type", "Must have custom brush.
Pick up a brush first (the brush tool, or b), then press From brush.");
                        }
                    }
                });
                // Pattern preview box.
                let (r, _) = ui.allocate_exact_size(Vec2::new(150.0, 60.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 0.0, GREY);
                bevel(ui.painter(), r, 1.5, true);
                if let Some(pb) = &self.set.pattern {
                    let pal = self.doc.palette;
                    let s = (56.0 / pb.img.h.max(1) as f32).min(146.0 / pb.img.w.max(1) as f32).min(4.0);
                    let o = r.center() - Vec2::new(pb.img.w as f32, pb.img.h as f32) * s / 2.0;
                    for y in 0..pb.img.h.min(200) {
                        for x in 0..pb.img.w.min(300) {
                            if pb.is_solid(x, y) {
                                let c = to_color32(pal[pb.img.get(x, y) as usize]);
                                ui.painter().rect_filled(egui::Rect::from_min_size(o + Vec2::new(x as f32, y as f32) * s, Vec2::splat(s.max(1.0))), 0.0, c);
                            }
                        }
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Range:");
                let mut n = self.set.range + 1;
                if ui.add(egui::DragValue::new(&mut n).range(1..=8)).changed() {
                    self.set.range = n - 1;
                }
                if toggle(ui, "↔", self.set.fill_angle == 0.0 && !self.set.fill_page).on_hover_text("Left to right").clicked() {
                    self.set.fill_angle = 0.0;
                    self.set.fill_page = false;
                }
                if toggle(ui, "↕", self.set.fill_angle == 90.0 && !self.set.fill_page).on_hover_text("Top to bottom").clicked() {
                    self.set.fill_angle = 90.0;
                    self.set.fill_page = false;
                }
                if toggle(ui, "▭", self.set.fill_page).on_hover_text("Spread the gradient over the whole page").clicked() {
                    self.set.fill_page = !self.set.fill_page;
                }
            });
            ui.horizontal(|ui| {
                for k in [FillKind::Linear, FillKind::Shaped, FillKind::Circular, FillKind::Contour, FillKind::Highlight] {
                    if toggle(ui, k.label(), self.set.fill == k).clicked() {
                        self.set.fill = k;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.v5.x.fill_to_bg, "Fill to Background").on_hover_text("Fill everything up to the background colour");
                ui.checkbox(&mut self.v5.x.anim_fill, "All Frames").on_hover_text("Fill the same spot in every frame");
            });
            ui.horizontal(|ui| {
                ui.label("Dither:");
                if button(ui, format!("⟳  {}", self.set.dither.label())).clicked() {
                    self.set.dither = self.set.dither.next();
                }
            });
            ui.horizontal(|ui| {
                let mut amt = self.set.spatter as f32;
                if ui.add(egui::Slider::new(&mut amt, 0.0..=100.0).show_value(false)).changed() {
                    self.set.spatter = amt as u8;
                    if self.set.dither == Dither::Smooth {
                        self.set.dither = Dither::Random;
                    }
                }
                // Gradient sample.
                let (r, _) = ui.allocate_exact_size(Vec2::new(120.0, 16.0), egui::Sense::hover());
                if let Some(rg) = self.doc.ranges.get(self.set.range).cloned() {
                    let mut rng = crate::paint::Rng::new(9);
                    let seq = rg.sequence(&self.doc.palette);
                    for k in 0..60 {
                        for yy in 0..8 {
                            let c = crate::paint::gradient_color(&seq, k as f32 / 60.0, &self.set, k, yy, &mut rng);
                            let cell = egui::Rect::from_min_size(r.min + Vec2::new(k as f32 * 2.0, yy as f32 * 2.0), Vec2::splat(2.0));
                            ui.painter().rect_filled(cell, 0.0, to_color32(self.doc.palette[c as usize]));
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                if button(ui, "Ok").clicked() {
                    close = Some(true);
                }
                ui.add_space(170.0);
                if button(ui, "Cancel").clicked() {
                    close = Some(false);
                }
            });
        });
        match close {
            Some(true) => {
                self.v5.show_fill_v5 = false;
                self.tool = crate::app::Tool::Fill;
            }
            Some(false) => {
                if let Some((f, a, pg, d, sp, r)) = self.v5.fill_backup.take() {
                    self.set.fill = f;
                    self.set.fill_angle = a;
                    self.set.fill_page = pg;
                    self.set.dither = d;
                    self.set.spatter = sp;
                    self.set.range = r;
                }
                self.v5.show_fill_v5 = false;
            }
            None => {}
        }
    }

    // ---------------------------------------------------------------- Screen Format

    fn screen_format(&mut self, ctx: &egui::Context) {
        if !self.v5.show_screen_format {
            return;
        }
        // This requester opens on its own screen in the original, with a pink title bar.
        let saved = ctx.global_style();
        ctx.global_style_mut(|s| {
            s.visuals.widgets.open.weak_bg_fill = Color32::from_rgb(220, 80, 112);
            s.visuals.window_fill = Color32::from_rgb(170, 170, 170);
        });
        let mut action: Option<bool> = None;
        let label = |t: &str| RichText::new(t).color(Color32::from_rgb(235, 235, 235)).strong();
        egui::Window::new(RichText::new("Deluxe Pixel - Screen Format").color(BLACK)).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(330.0);
                    ui.vertical_centered(|ui| ui.label(label("Choose Display Mode")));
                    egui::Frame::new().fill(Color32::from_rgb(170, 170, 170)).stroke(egui::Stroke::new(1.5, BLACK)).show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("sf_modes").max_height(230.0).show(ui, |ui| {
                            ui.set_width(318.0);
                            for (i, m) in MODES.iter().enumerate() {
                                let sel = self.v5.sf_mode == i;
                                let text = RichText::new(format!("{:<22}{:>5}x{:<4}", m.name, m.w, m.h)).monospace();
                                let text = if sel { text.color(WHITE).background_color(Color32::from_rgb(220, 80, 112)) } else { text.color(BLACK) };
                                if ui.add(egui::Label::new(text).sense(egui::Sense::click())).clicked() {
                                    self.v5.sf_mode = i;
                                    self.v5.sf_screen = (m.w, m.h);
                                    if self.v5.sf_page_kind == 0 {
                                        self.v5.sf_page = (m.w, m.h);
                                    }
                                    self.v5.sf_colors = if m.ehb { 64 } else { self.v5.sf_colors.min(m.max_colors(self.v5.sf_chipset)) };
                                }
                            }
                        });
                    });
                    let m = MODES[self.v5.sf_mode];
                    egui::Frame::new().stroke(egui::Stroke::new(1.5, BLACK)).inner_margin(4).show(ui, |ui| {
                        ui.set_width(318.0);
                        ui.label(RichText::new(format!("{:<22}{:>5}x{:<4}", m.name, m.w, m.h)).monospace().color(BLACK));
                    });
                    ui.horizontal(|ui| {
                        ui.label(label("Chip Set"));
                        for cs in [Chipset::Ecs, Chipset::Aga] {
                            if toggle(ui, cs.label(), self.v5.sf_chipset == cs)
                                .on_hover_text(if cs == Chipset::Ecs { "Classic Amiga: 4096 colours, depth limited by resolution" } else { "AGA Amiga: 16.7 million colours, 256 in every mode" })
                                .clicked()
                            {
                                self.v5.sf_chipset = cs;
                                let max = m.max_colors(cs);
                                self.v5.sf_colors = if m.ehb { 64 } else { self.v5.sf_colors.min(max) };
                            }
                        }
                    });
                });
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.set_width(260.0);
                    ui.vertical_centered(|ui| ui.label(label("Display Information")));
                    let m = MODES[self.v5.sf_mode];
                    let (n, w, h) = (m.name, m.w, m.h);
                    let cs = self.v5.sf_chipset;
                    let gb = m.gun_bits(cs);
                    let total: u64 = 1u64 << (3 * gb);
                    let g = gcd(w, h);
                    egui::Frame::new().stroke(egui::Stroke::new(1.5, BLACK)).inner_margin(6).show(ui, |ui| {
                        ui.set_width(246.0);
                        for line in [
                            format!("Type = {}", n),
                            format!("Max Colours = {} of {}", if m.ehb { 64 } else { m.max_colors(cs) }, total),
                            format!("Max Size = {}x{}", w * 2, h * 2),
                            format!("Aspect Ratio = {}:{}", w / g, h / g),
                            if m.ehb { "Extra-Half-Brite: colours 32-63 are half as bright as 0-31".to_string() } else if m.amiga() { format!("Amiga mode id = {:08X}", m.mode_id()) } else { "Supports double-buffering".to_string() },
                            String::new(),
                            "« Advanced Information »".to_string(),
                            format!("Max Depth = {}", m.max_depth(cs)),
                            format!("Bits Per RGB = [{gb}:{gb}:{gb}]"),
                            format!("Pixels = {}", w * h),
                        ] {
                            ui.label(RichText::new(line).monospace().color(BLACK));
                        }
                    });
                });
            });
            ui.add_space(8.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(330.0);
                    ui.vertical_centered(|ui| ui.label(label("Screen Size")));
                    ui.horizontal(|ui| {
                        if button(ui, format!("⟳  {:<10}", SCREEN_KINDS[self.v5.sf_screen_kind])).clicked() {
                            self.v5.sf_screen_kind = (self.v5.sf_screen_kind + 1) % 2;
                            let (w, h) = (MODES[self.v5.sf_mode].w, MODES[self.v5.sf_mode].h);
                            self.v5.sf_screen = if self.v5.sf_screen_kind == 1 { (w * 23 / 20, h * 23 / 20) } else { (w, h) };
                        }
                        ui.add(egui::DragValue::new(&mut self.v5.sf_screen.0).range(16..=8192));
                        ui.add(egui::DragValue::new(&mut self.v5.sf_screen.1).range(16..=8192));
                    });
                    ui.vertical_centered(|ui| ui.label(label("Page Size")));
                    ui.horizontal(|ui| {
                        if button(ui, format!("⟳  {:<10}", PAGE_KINDS[self.v5.sf_page_kind])).clicked() {
                            self.v5.sf_page_kind = (self.v5.sf_page_kind + 1) % 4;
                            let (w, h) = self.v5.sf_screen;
                            match self.v5.sf_page_kind {
                                0 => self.v5.sf_page = (w, h),
                                1 => self.v5.sf_page = (w, h * 17 / 10),
                                2 => self.v5.sf_page = (w * 23 / 20, h * 23 / 20),
                                _ => {}
                            }
                        }
                        let cust = self.v5.sf_page_kind == 3;
                        ui.add_enabled(cust, egui::DragValue::new(&mut self.v5.sf_page.0).range(8..=8192));
                        ui.add_enabled(cust, egui::DragValue::new(&mut self.v5.sf_page.1).range(8..=8192));
                    });
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(label("Palette Size"));
                            let m = MODES[self.v5.sf_mode];
                            let maxd = m.max_depth(self.v5.sf_chipset);
                            let mut bits = crate::amiga::depth_of(self.v5.sf_colors).min(maxd);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("{:>3}", 1u32 << bits)).color(BLACK));
                                if m.ehb {
                                    ui.label(RichText::new("(EHB)").color(BLACK));
                                } else if ui.add(egui::Slider::new(&mut bits, 1..=maxd).show_value(false)).changed() {
                                    self.v5.sf_colors = 1usize << bits;
                                }
                            });
                            if !m.ehb {
                                self.v5.sf_colors = self.v5.sf_colors.min(1 << maxd);
                            }
                        });
                        ui.vertical(|ui| {
                            ui.label(label("Backing Store"));
                            let _ = button(ui, "⟳  None").on_hover_text("Pictures always keep their 256-colour palette here");
                        });
                    });
                });
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.set_width(260.0);
                    ui.vertical_centered(|ui| ui.label(label("Credits")));
                    egui::Frame::new().stroke(egui::Stroke::new(1.5, BLACK)).inner_margin(6).show(ui, |ui| {
                        ui.set_width(246.0);
                        ui.vertical_centered(|ui| {
                            for line in ["Deluxe Pixel", "A modern paint program", "for Windows,", "in the spirit of the", "classic Amiga and DOS", "paint programs."] {
                                ui.label(RichText::new(line).color(BLACK));
                            }
                        });
                    });
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if button(ui, "Use").clicked() {
                    action = Some(true);
                }
                ui.add_space(180.0);
                ui.checkbox(&mut self.v5.sf_retain, RichText::new("Retain Picture").color(BLACK));
                ui.add_space(150.0);
                if button(ui, "Cancel").clicked() {
                    action = Some(false);
                }
            });
        });
        ctx.set_global_style(saved);
        match action {
            Some(true) => {
                let (w, h) = self.v5.sf_page;
                if self.v5.sf_retain {
                    if (w, h) != (self.doc.w, self.doc.h) {
                        self.doc.resize_page(w, h, self.bg, false);
                    }
                } else {
                    let mut d = Document::new(w, h, self.bg);
                    d.palette = self.doc.palette;
                    d.ranges = self.doc.ranges.clone();
                    self.doc = d;
                }
                let m = MODES[self.v5.sf_mode];
                let cs = self.v5.sf_chipset;
                self.v5.chipset = cs;
                let old_colors = self.doc.colors;
                self.doc.colors = if m.ehb { 64 } else { self.v5.sf_colors.min(m.max_colors(cs)) };
                crate::amiga::set_active_colors(self.doc.colors);
                self.doc.adopt_palette_size(old_colors);
                self.doc.ehb = m.ehb;
                self.doc.gun_bits = m.gun_bits(cs);
                self.doc.mode_id = m.mode_id();
                self.doc.limit_colors();
                let changed = self.doc.fit_pixels();
                if changed > 0 {
                    self.status = format!("{} pixels moved to the nearest of the {} colours", changed, self.doc.colors);
                }
                self.v5.screen = self.v5.sf_screen;
                self.pal_page = 0;
                self.fit_pending = true;
                self.v5.show_screen_format = false;
            }
            Some(false) => self.v5.show_screen_format = false,
            None => {}
        }
    }

    // ---------------------------------------------------------------- About

    fn about_v5(&mut self, ctx: &egui::Context) {
        if self.show_about {
            self.show_about = false;
            self.v5.show_about_v5 = true;
        }
        if !self.v5.show_about_v5 {
            return;
        }
        let mut ok = false;
        let (total, free) = memory_kb();
        let frames = self.doc.frames.len();
        let pals = self.doc.frame_palettes.iter().filter(|p| p.is_some()).count();
        let anim_size = frames as i64 * self.doc.w as i64 * self.doc.h as i64;
        let ab_size: i64 = self.anim_brush.as_ref().map(|a| a.cells.iter().map(|c| (c.w * c.h) as i64).sum()).unwrap_or(0);
        let br = (self.brush.img.w * self.brush.img.h) as i64;
        req("About Deluxe Pixel").show(ctx, |ui| {
            ui.set_width(260.0);
            ui.vertical_centered(|ui| {
                ui.label("Version 1.2");
                ui.label("A modern paint program");
                ui.label("for Windows");
            });
            ui.add_space(6.0);
            egui::Grid::new("about_mem").num_columns(3).show(ui, |ui| {
                ui.label("Memory");
                ui.label("Available:");
                ui.label("Total:");
                ui.end_row();
                ui.label("KB");
                ui.label(free.to_string());
                ui.label(total.to_string());
                ui.end_row();
            });
            ui.add_space(6.0);
            egui::Grid::new("about_anim").num_columns(2).show(ui, |ui| {
                for (k, v) in [("Anim Frames:", (if frames > 1 { frames } else { 0 }) as i64), ("Anim Palettes:", pals as i64), ("Anim Size:", if frames > 1 { anim_size } else { 0 }), ("AnimBrush Size:", ab_size), ("Brush Size:", br)] {
                    ui.label(k);
                    ui.label(v.to_string());
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                ui.label(self.doc.name());
                if button(ui, "Ok").clicked() {
                    ok = true;
                }
            });
        });
        if ok {
            self.v5.show_about_v5 = false;
        }
    }

    // ---------------------------------------------------------------- Frames

    fn frame_requesters(&mut self, ctx: &egui::Context) {
        let n = self.doc.frames.len();
        if self.v5.show_frames_add {
            let mut done = None;
            req("Add Frames").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Add");
                    ui.add(egui::DragValue::new(&mut self.v5.add_n).range(1..=500));
                    ui.label(format!("frames after frame {}", self.doc.cur + 1));
                });
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
                if ok {
                    self.doc.add_frames(self.v5.add_n);
                }
                self.v5.show_frames_add = false;
            }
        }
        if self.v5.show_frames_copy {
            let mut done = None;
            req("Copy Frames").show(ctx, |ui| {
                let (a, b, c) = &mut self.v5.copy_abc;
                ui.horizontal(|ui| {
                    ui.label("Copy frame");
                    ui.add(egui::DragValue::new(a).range(1..=n));
                });
                ui.horizontal(|ui| {
                    ui.label("to frames");
                    ui.add(egui::DragValue::new(b).range(1..=n));
                    ui.label("through");
                    ui.add(egui::DragValue::new(c).range(1..=n));
                });
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
                if ok {
                    let (a, b, c) = self.v5.copy_abc;
                    self.doc.copy_frames(a - 1, b.min(c) - 1, b.max(c) - 1);
                }
                self.v5.show_frames_copy = false;
            }
        }
        if self.v5.show_frames_delete {
            let mut done = None;
            req("Delete Frames").show(ctx, |ui| {
                let (a, b) = &mut self.v5.del_ab;
                ui.horizontal(|ui| {
                    ui.label("Delete frames");
                    ui.add(egui::DragValue::new(a).range(1..=n));
                    ui.label("through");
                    ui.add(egui::DragValue::new(b).range(1..=n));
                });
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
                if ok {
                    let (a, b) = self.v5.del_ab;
                    if !self.doc.delete_frames(a.min(b) - 1, a.max(b) - 1) {
                        self.msg("Delete Frames", "Can't delete every frame: at least one must stay.");
                    }
                }
                self.v5.show_frames_delete = false;
            }
        }
        if self.v5.show_frame_rate {
            let mut done = false;
            req("Set Frame Rate").show(ctx, |ui| {
                ui.add(egui::Slider::new(&mut self.doc.fps, 1.0..=60.0).text("frames per second"));
                if button(ui, "Ok").clicked() {
                    done = true;
                }
            });
            if done {
                self.v5.show_frame_rate = false;
            }
        }
        if self.v5.show_set_range {
            let mut done = None;
            req("Set Range").show(ctx, |ui| {
                let (a, b) = &mut self.v5.range_ab;
                ui.horizontal(|ui| {
                    ui.label("Play frames");
                    ui.add(egui::DragValue::new(a).range(1..=n));
                    ui.label("through");
                    ui.add(egui::DragValue::new(b).range(1..=n));
                });
                ui.horizontal(|ui| {
                    if button(ui, "Ok").clicked() {
                        done = Some(true);
                    }
                    if button(ui, "All").clicked() {
                        done = Some(false);
                    }
                });
            });
            if let Some(ok) = done {
                let (a, b) = self.v5.range_ab;
                self.doc.play_range = if ok { Some((a.min(b) - 1, a.max(b) - 1)) } else { None };
                self.v5.show_set_range = false;
            }
        }
    }

    fn camera_move_req(&mut self, ctx: &egui::Context) {
        if !self.v5.show_camera_move {
            return;
        }
        let mut done = None;
        let n = self.doc.frames.len();
        req("Camera Move").show(ctx, |ui| {
            ui.label("Scrolls the whole picture a little more in each frame, like a moving camera.");
            let (dx, dy, cnt, _) = &mut self.v5.cam;
            ui.horizontal(|ui| {
                ui.label("Distance X");
                ui.add(egui::DragValue::new(dx).range(-4000..=4000));
                ui.label("Y");
                ui.add(egui::DragValue::new(dy).range(-4000..=4000));
            });
            ui.horizontal(|ui| {
                ui.label("Count");
                ui.add(egui::DragValue::new(cnt).range(1..=n.max(1)));
                ui.label(format!("frames from frame {}", self.doc.cur + 1));
            });
            ui.horizontal(|ui| {
                if button(ui, "Draw").clicked() {
                    done = Some(true);
                }
                if button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            if ok {
                self.do_camera_move();
            }
            self.v5.show_camera_move = false;
        }
    }

    fn morph_req(&mut self, ctx: &egui::Context) {
        if !self.v5.show_morph {
            return;
        }
        let mut done = None;
        req("Morph").show(ctx, |ui| {
            ui.label("Draws the brush changing into the spare brush, one step per frame.");
            ui.horizontal(|ui| {
                ui.label("Frames:");
                ui.add(egui::DragValue::new(&mut self.v5.morph_frames).range(2..=999));
            });
            ui.horizontal(|ui| {
                ui.label("Position x/y:");
                ui.add(egui::DragValue::new(&mut self.move_params.start.0));
                ui.add(egui::DragValue::new(&mut self.move_params.start.1));
            });
            ui.horizontal(|ui| {
                if button(ui, "Draw").clicked() {
                    done = Some(true);
                }
                if button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            if ok {
                if self.doc.frames.len() < self.v5.morph_frames {
                    let need = self.v5.morph_frames;
                    self.doc.set_frame_count(need.max(self.doc.frames.len()));
                }
                self.do_morph();
            }
            self.v5.show_morph = false;
        }
    }

    fn animbrush_req(&mut self, ctx: &egui::Context) {
        if !self.v5.show_animbrush_settings {
            return;
        }
        // As in the original: Number of cels, Duration, Current, Direction (forward, backward,
        // ping-pong), Ok / Cancel. The fields edit a copy; Ok gives it to the AnimBrush.
        let cels = self.anim_brush.as_ref().map(|ab| ab.cells.len()).unwrap_or(0);
        let edit = *self.v5.ab_edit.get_or_insert_with(|| match &self.anim_brush {
            Some(ab) => (ab.duration, ab.pos + 1, ab.dir),
            None => (0, 1, 1),
        });
        let (mut duration, mut current, mut dir) = edit;
        let mut done: Option<bool> = None;
        req("AnimBrush Settings").show(ctx, |ui| {
            ui.label(format!("Number of cels: {cels}"));
            ui.add_space(2.0);
            egui::Grid::new("ab_settings").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Duration:");
                ui.add_enabled(cels > 0, egui::DragValue::new(&mut duration).range(1..=9999))
                    .on_hover_text("How many stamps (or frames) one run through all the cels takes: the number of cels shows one cel per frame, more is slower");
                ui.end_row();
                ui.label("Current:");
                ui.add_enabled(cels > 0, egui::DragValue::new(&mut current).range(1..=cels.max(1)))
                    .on_hover_text("The cel the brush shows now");
                ui.end_row();
                ui.label("Direction:");
                ui.horizontal(|ui| {
                    use crate::ui_style::{toggle_icon, ICON_BACK, ICON_FWD, ICON_PING};
                    for (d, icon, tip) in [(1, &ICON_FWD[..], "Forward"), (-1, &ICON_BACK[..], "Backward"), (0, &ICON_PING[..], "Ping-pong: forward, then back")] {
                        if toggle_icon(ui, icon, dir == d).on_hover_text(tip).clicked() {
                            dir = d;
                        }
                    }
                });
                ui.end_row();
            });
            if cels == 0 {
                ui.label(RichText::new("No AnimBrush yet: pick one up with AnimBrush > Pick up.").small());
            }
            ui.horizontal(|ui| {
                if button(ui, "Ok").clicked() {
                    done = Some(true);
                }
                ui.add_space(110.0);
                if button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        self.v5.ab_edit = Some((duration, current.clamp(1, cels.max(1)), dir));
        if let Some(ok) = done {
            if ok {
                if let Some(ab) = &mut self.anim_brush {
                    ab.duration = duration.max(1);
                    ab.dir = dir;
                    ab.restart_at(current.max(1) - 1);
                }
                // The brush shows the chosen cel.
                if let Some(ab) = &self.anim_brush {
                    self.brush.img = ab.cells[ab.pos].clone();
                }
            }
            self.v5.ab_edit = None;
            self.v5.show_animbrush_settings = false;
        }
    }

    fn handle_req(&mut self, ctx: &egui::Context) {
        if !self.v5.show_handle {
            return;
        }
        let mut done = false;
        let (w, h) = (self.brush.img.w, self.brush.img.h);
        req("Place Handle").show(ctx, |ui| {
            ui.label("The point of the brush that follows the pointer:");
            ui.horizontal(|ui| {
                ui.label("X");
                ui.add(egui::DragValue::new(&mut self.brush.hx).range(0..=w - 1));
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut self.brush.hy).range(0..=h - 1));
            });
            ui.horizontal(|ui| {
                for (n, x, y) in [("Top-left", 0, 0), ("Centre", w / 2, h / 2), ("Bottom-right", w - 1, h - 1)] {
                    if button(ui, n).clicked() {
                        self.brush.hx = x;
                        self.brush.hy = y;
                    }
                }
            });
            if button(ui, "Ok").clicked() {
                done = true;
            }
        });
        if done {
            self.v5.show_handle = false;
        }
    }

    fn lt_settings_req(&mut self, ctx: &egui::Context) {
        if !self.v5.show_lt_settings {
            return;
        }
        let mut done = false;
        req("LightTable Settings").show(ctx, |ui| {
            egui::Grid::new("ltgrid").num_columns(3).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Layer");
                ui.label("Frame");
                ui.label("Dim");
                ui.end_row();
                for (i, l) in self.v5.lt_layers.iter_mut().enumerate() {
                    ui.checkbox(&mut l.on, format!("{}", i + 1));
                    ui.add(egui::DragValue::new(&mut l.offset).range(-99..=99)).on_hover_text("0 = spare page, -1 = previous frame, 1 = next frame");
                    ui.add(egui::Slider::new(&mut l.dim, -255..=255));
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.v5.lt_dim, "Dim");
                if button(ui, "<").clicked() {
                    self.v5.lt_layers.iter_mut().for_each(|l| l.dim = (l.dim - 5).max(-255));
                }
                if button(ui, ">").clicked() {
                    self.v5.lt_layers.iter_mut().for_each(|l| l.dim = (l.dim + 5).min(255));
                }
                ui.checkbox(&mut self.v5.lt_anim_lock, "Anim Lock");
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.light_table, "LightTable on");
                if button(ui, "Ok").clicked() {
                    done = true;
                }
            });
        });
        if done {
            self.v5.show_lt_settings = false;
        }
    }

    fn pressure_req(&mut self, ctx: &egui::Context) {
        if !self.v5.show_pressure {
            return;
        }
        let mut done = None;
        req("Pressure").show(ctx, |ui| {
            ui.label("For pens and tablets that report pressure.");
            ui.checkbox(&mut self.set.pressure_translucent, "Translucency (light to full colour)");
            ui.checkbox(&mut self.v5.pressure_size, "Size");
            ui.add_enabled(self.v5.pressure_size, egui::Slider::new(&mut self.v5.pressure_amount, 1..=32).text("pixels"));
            ui.checkbox(&mut self.v5.pressure_reverse, "Reverse Size");
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
            if ok {
                self.v5.pressure_on = true;
            }
            self.v5.show_pressure = false;
        }
    }

    fn media_req(&mut self, ctx: &egui::Context) {
        if !self.v5.show_media {
            return;
        }
        let mut pick = None;
        let mut cancel = false;
        req("Open Media").show(ctx, |ui| {
            ui.label("Natural media: a paper texture and a matching translucency.");
            for (i, (name, ..)) in crate::actions::MEDIA.iter().enumerate() {
                if toggle(ui, *name, self.v5.media == Some(*name)).clicked() {
                    pick = Some(i);
                }
            }
            ui.horizontal(|ui| {
                if button(ui, "Load file..").on_hover_text("A DeluxePaint media library (Chalk, FeltTip, Oil, Watercolor)").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Open Media").add_filter("All files", &["*"]).pick_file() {
                        match crate::amigafiles::media_for_file(&p) {
                            Some(i) => pick = Some(i),
                            None => self.msg(
                                "Open Media",
                                "DeluxePaint media libraries are Amiga program code, which can't run here. Chalk, FeltTip, Oil and Watercolor are known by their names; this one isn't.",
                            ),
                        }
                    }
                }
                if button(ui, "Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if let Some(i) = pick {
            self.set_media(i);
            self.v5.show_media = false;
        } else if cancel {
            self.v5.show_media = false;
        }
    }

    // ---------------------------------------------------------------- Arrange

    fn arrange_req(&mut self, ctx: &egui::Context) {
        if !self.v5.show_arrange {
            return;
        }
        let mut open = true;
        req("Arrange Palette").open(&mut open).show(ctx, |ui| {
            ui.label("Click a colour, then click where it should go. The picture keeps its look.");
            let pal = self.doc.palette;
            let cell = 16.0;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(cell * 32.0, cell * 8.0), egui::Sense::click());
            for i in 0..256usize {
                let r = egui::Rect::from_min_size(rect.min + Vec2::new((i % 32) as f32 * cell, (i / 32) as f32 * cell), Vec2::splat(cell - 1.0));
                ui.painter().rect_filled(r, 0.0, to_color32(pal[i]));
                if Some(i as u8) == self.v5.arrange_sel {
                    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(2.0, WHITE), egui::StrokeKind::Inside);
                }
            }
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
                let v = p - rect.min;
                let i = ((v.y / cell) as usize * 32 + (v.x / cell) as usize).min(255) as u8;
                match self.v5.arrange_sel.take() {
                    None => self.v5.arrange_sel = Some(i),
                    Some(a) if a != i => self.swap_colours(a, i),
                    _ => {}
                }
            }
        });
        self.v5.show_arrange = open;
    }

    /// Exchange two palette entries and the pixels that use them, so nothing changes on screen.
    pub fn swap_colours(&mut self, a: u8, b: u8) {
        self.doc.checkpoint();
        self.doc.palette.swap(a as usize, b as usize);
        for f in self.doc.frames.iter_mut() {
            for p in f.px.iter_mut() {
                if *p == a {
                    *p = b;
                } else if *p == b {
                    *p = a;
                }
            }
        }
    }

    // ---------------------------------------------------------------- Macros

    fn macro_reqs(&mut self, ctx: &egui::Context) {
        if self.v5.assign.is_some() {
            let mut done = None;
            req("Assign Macro").show(ctx, |ui| {
                let m = self.v5.assign.as_mut().unwrap();
                ui.label(format!("{} steps recorded.", m.events.len()));
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut m.name);
                });
                ui.label("Put it in slot:");
                egui::Grid::new("slots").num_columns(5).show(ui, |ui| {
                    for i in 0..10 {
                        let taken = self.v5.slots[i].as_ref().map(|m| m.name.clone()).unwrap_or_else(|| "empty".into());
                        if toggle(ui, format!("#{} {}", i + 1, taken), self.v5.assign_slot == i).clicked() {
                            self.v5.assign_slot = i;
                        }
                        if i % 5 == 4 {
                            ui.end_row();
                        }
                    }
                });
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
                let m = self.v5.assign.take().unwrap();
                self.v5.last_macro = Some(m.clone());
                if ok {
                    let i = self.v5.assign_slot;
                    self.status = format!("{} is now User > Macro #{} (Ctrl+{})", m.name, i + 1, (i + 1) % 10);
                    self.v5.slots[i] = Some(m);
                }
            }
        }
        if self.v5.show_macro_load {
            let mut done = false;
            let mut load_into = None;
            req("Load Macro").show(ctx, |ui| {
                ui.label("Click ? next to a slot to load a macro file into it.");
                for i in 0..10 {
                    ui.horizontal(|ui| {
                        let name = self.v5.slots[i].as_ref().map(|m| m.name.clone()).unwrap_or_else(|| "—".into());
                        ui.label(format!("Macro #{:<2}", i + 1));
                        if button(ui, "?").clicked() {
                            load_into = Some(i);
                        }
                        ui.label(name);
                    });
                }
                if button(ui, "Ok").clicked() {
                    done = true;
                }
            });
            if let Some(i) = load_into {
                if let Some(p) = rfd::FileDialog::new().set_title("Load Macro").add_filter("Macro", &["dpm"]).pick_file() {
                    match crate::macros::Macro::load(&p) {
                        Ok(m) => {
                            self.v5.slots[i] = Some(m);
                            self.v5.prefs.macro_paths[i] = Some(p);
                        }
                        Err(e) => self.msg("Load Macro", e),
                    }
                }
            }
            if done {
                self.v5.show_macro_load = false;
            }
        }
    }

    fn confirm_delete_req(&mut self, ctx: &egui::Context) {
        let Some(p) = self.v5.confirm_delete.clone() else { return };
        let mut done = None;
        req("Delete Disk File").show(ctx, |ui| {
            ui.label("Permanently delete from your disk");
            ui.label(format!("the file \"{}\"?", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
            ui.horizontal(|ui| {
                if button(ui, "Delete").clicked() {
                    done = Some(true);
                }
                if button(ui, "Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(ok) = done {
            if ok {
                if let Err(e) = std::fs::remove_file(&p) {
                    self.msg("Delete", format!("Couldn't delete the file: {e}"));
                }
            }
            self.v5.confirm_delete = None;
        }
    }
}

fn gcd(a: i32, b: i32) -> i32 {
    if b == 0 {
        a.max(1)
    } else {
        gcd(b, a % b)
    }
}

/// Total and available memory in KB.
#[cfg(windows)]
fn memory_kb() -> (u64, u64) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    unsafe {
        let mut m: MEMORYSTATUSEX = std::mem::zeroed();
        m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut m) != 0 {
            return (m.ullTotalPhys / 1024, m.ullAvailPhys / 1024);
        }
    }
    (0, 0)
}

#[cfg(not(windows))]
fn memory_kb() -> (u64, u64) {
    (0, 0)
}

#[allow(dead_code)]
fn _unused(_: Brush) {}
