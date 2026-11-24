//! The frame loop, toolbox, palette strip, info bar and menus.

use crate::app::{App, PalAction, Pending, Tool};
use crate::paint::{FillKind, Mode};
use crate::palette::to_color32;
use crate::pix::BuiltinShape;
use crate::ui_style::ACCENT;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame_count += 1;
        if self.screenshot.is_some() {
            ctx.request_repaint();
        }
        // The whole screen (menus, toolbox, palette) scales with the window, like the original's
        // screen; it was laid out for a window 800 points high.
        let zf = ctx.zoom_factor();
        let target = (ctx.content_rect().height() * zf / 800.0).clamp(0.75, 3.0);
        if (target - zf).abs() > 0.02 {
            ctx.set_zoom_factor(target);
        }
        self.tick(&ctx);
        self.run_replay(&ctx);
        self.keyboard(&ctx);
        self.persp_update(&ctx);
        self.update_persp_plane();

        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_now && self.screenshot.is_none() && self.doc.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }

        if self.show.is_some() {
            self.slideshow_ui(ui);
        } else {
            let near_top = ctx.input(|i| i.pointer.hover_pos().map(|p| p.y < 28.0).unwrap_or(false));
            if !self.view_page && (self.v5.x.title || near_top || ctx.any_popup_open()) {
                egui::Panel::top("menu")
                    .frame(egui::Frame::new().fill(crate::ui_style::GREY).inner_margin(egui::Margin::symmetric(4, 1)).stroke(egui::Stroke::new(1.0, crate::ui_style::BLACK)))
                    .show(ui, |ui| self.menu_bar(ui));
            }
            if !self.view_page {
                if self.info_bar {
                    egui::Panel::bottom("info")
                        .exact_size(24.0)
                        .frame(egui::Frame::new().fill(crate::ui_style::GREY).inner_margin(egui::Margin::symmetric(6, 2)).stroke(egui::Stroke::new(1.0, crate::ui_style::BLACK)))
                        .show(ui, |ui| self.info_bar_ui(ui));
                }
                egui::Panel::right("tools")
                    .exact_size(112.0)
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(egui::Frame::new().fill(crate::ui_style::BLACK))
                    .show(ui, |ui| self.toolbox(ui));
                // Colour Set > Load: choosing the colours, along the bottom.
                if self.v5.x.cset.is_some() {
                    egui::Panel::bottom("colourset")
                        .exact_size(124.0)
                        .resizable(false)
                        .show_separator_line(false)
                        .frame(egui::Frame::new().fill(crate::ui_style::BLACK))
                        .show(ui, |ui| self.colour_set_panel(ui));
                }
                // The Mixer along the bottom, as in the original (Colour > Palette > Mixer, p).
                if self.show_palette && self.v5.x.cset.is_none() {
                    egui::Panel::bottom("mixer")
                        .exact_size(124.0)
                        .resizable(false)
                        .show_separator_line(false)
                        .frame(egui::Frame::new().fill(crate::ui_style::BLACK))
                        .show(ui, |ui| self.mixer_panel(ui));
                }
                if self.v5.show_ranges {
                    egui::Panel::bottom("ranges")
                        .exact_size(124.0)
                        .resizable(false)
                        .show_separator_line(false)
                        .frame(egui::Frame::new().fill(crate::ui_style::BLACK))
                        .show(ui, |ui| self.range_panel(ui));
                }
                if self.v5.show_anim_panel {
                    egui::Panel::bottom("animpanel")
                        .exact_size(48.0)
                        .resizable(false)
                        .show_separator_line(false)
                        .frame(egui::Frame::new().fill(crate::ui_style::BLACK))
                        .show(ui, |ui| self.anim_panel(ui));
                }
            }
            self.upload_texture(&ctx);
            egui::CentralPanel::no_frame().show(ui, |ui| self.canvas_ui(ui));
            if self.show_fill {
                self.show_fill = false;
                self.open_fill();
            }
            self.dialogs(&ctx);
            self.dialogs5(&ctx);
            self.v52_dialogs(&ctx);
        }

        if self.drag.is_some() || self.stroke.is_some() || self.curve.is_some() || self.text_entry.is_some() {
            ctx.request_repaint();
        }

        // --screenshot: draw about 120 frames, save the window picture and exit.
        if let Some(path) = self.screenshot.clone() {
            if self.frame_count >= 120 && !self.shot_requested {
                self.shot_requested = true;
                let slot = self.shot_result.clone();
                ctx.request_screenshot(move |img| {
                    *slot.lock().unwrap() = Some(img);
                });
            }
            let got = self.shot_result.lock().unwrap().take();
            if let Some(img) = got {
                let [w, h] = img.size;
                let mut buf = image::RgbaImage::new(w as u32, h as u32);
                for (i, p) in buf.pixels_mut().enumerate() {
                    let c = img.pixels[i];
                    *p = image::Rgba([c.r(), c.g(), c.b(), 255]);
                }
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = buf.save(&path) {
                    eprintln!("Couldn't save screenshot: {e}");
                }
                self.quit_now = true;
            }
        }
        if self.quit_now {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl App {
    pub fn upload_texture(&mut self, ctx: &egui::Context) {
        let pal = self.display_palette();
        let img = self.compose_rgba(&pal);
        let opts = egui::TextureOptions::NEAREST;
        match &mut self.texture {
            Some(t) => t.set(img, opts),
            None => self.texture = Some(ctx.load_texture("canvas", img, opts)),
        }
    }

    // ------------------------------------------------------------ toolbox

    pub fn toolbox(&mut self, ui: &mut egui::Ui) {
        use crate::ui_style::*;
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(full, 0.0, BLACK);
        let m = 3.0;
        let (x0, x1) = (full.min.x + m, full.max.x - m);
        let cw = ((x1 - x0) - 3.0) / 2.0;
        let mut y = full.min.y + 3.0;

        // Built-in brushes block.
        let block = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + 56.0));
        self.brush_block(ui, &p, block);
        y = block.max.y + 3.0;

        // Tool tiles, two per row, in the original's order.
        let rows: [(&str, &str); 9] = [
            ("dotted", "freehand"),
            ("line", "curve"),
            ("fill", "airbrush"),
            ("rect", "circle"),
            ("ellipse", "polygon"),
            ("brush", "text"),
            ("grid", "symmetry"),
            ("magnify", "zoom"),
            ("undo", "clr"),
        ];
        let ch = 33.0;
        for (a, b) in rows {
            for (i, id) in [a, b].into_iter().enumerate() {
                let rx = x0 + i as f32 * (cw + 3.0);
                let rect = Rect::from_min_size(Pos2::new(rx, y), Vec2::new(cw, ch));
                self.tool_tile(ui, &p, rect, id);
            }
            y += ch + 3.0;
        }
        y += 3.0;

        // Colour indicator: the foreground colour as a bar on the background colour.
        let pal = self.display_palette();
        let ind = Rect::from_min_max(Pos2::new(x0 + 10.0, y), Pos2::new(x1 - 10.0, y + 20.0));
        p.rect_filled(ind, 0.0, to_color32(pal[self.bg as usize]));
        p.rect_filled(ind.shrink2(Vec2::new(ind.width() * 0.2, 5.0)), 0.0, to_color32(pal[self.fg as usize]));
        p.rect_stroke(ind, 0.0, Stroke::new(1.0, GREY), egui::StrokeKind::Outside);
        let r = ui.interact(ind, ui.scope_id().with("indicator"), Sense::click());
        // Left click: pick a colour from the picture (eyedropper); right click: the Mixer.
        let r = r.on_hover_text(format!("Foreground {} on background {} — click: pick a colour from the picture (,); right click: Mixer (p)", self.fg, self.bg));
        if r.clicked() {
            self.tool = Tool::Eyedropper;
        }
        if r.secondary_clicked() {
            self.show_palette = true;
        }
        y = ind.max.y + 6.0;

        // Palette page selector at the bottom.
        let sel = Rect::from_min_max(Pos2::new(x0, full.max.y - 24.0), Pos2::new(x1, full.max.y - 3.0));
        p.rect_filled(sel, 0.0, GREY);
        bevel(&p, sel, 2.0, false);
        let third = sel.width() / 4.0;
        bitmap(&p, Pos2::new(sel.min.x + third / 2.0, sel.center().y), &["...#", "..##", ".###", "####", ".###", "..##", "...#"], 2.0, BLACK);
        bitmap(&p, Pos2::new(sel.max.x - third / 2.0, sel.center().y), &["#...", "##..", "###.", "####", "###.", "##..", "#..."], 2.0, BLACK);
        let pages = self.doc.colors.div_ceil(64).max(1);
        let letter = if pages > 1 { ((b'a' + self.pal_page as u8) as char).to_string() } else { format!("{}", self.doc.colors) };
        p.text(sel.center(), egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional(16.0), BLACK);
        let r = ui.interact(sel, ui.scope_id().with("palpage"), Sense::click());
        if r.clicked() {
            if let Some(pos) = r.interact_pointer_pos() {
                if pos.x < sel.center().x {
                    self.pal_page = (self.pal_page + pages - 1) % pages;
                } else {
                    self.pal_page = (self.pal_page + 1) % pages;
                }
            }
        }
        r.on_hover_text(if pages > 1 { "Palette page (64 colours each): click left or right half" } else { "This screen has this many colours (Picture > Screen Format)" });

        // Palette: 4 columns of 16, colours running down each column.
        let area = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, sel.min.y - 4.0));
        self.palette_page(ui, &p, area, &pal);
    }

    pub(crate) fn brush_block(&mut self, ui: &mut egui::Ui, p: &egui::Painter, block: Rect) {
        use crate::ui_style::*;
        p.rect_filled(block, 0.0, GREY);
        bevel(p, block, 2.0, false);
        let items: [(BuiltinShape, i32, f32, f32); 10] = [
            (BuiltinShape::Round, 1, 0.125, 0.22),
            (BuiltinShape::Round, 3, 0.375, 0.22),
            (BuiltinShape::Round, 5, 0.625, 0.22),
            (BuiltinShape::Round, 9, 0.875, 0.22),
            (BuiltinShape::Square, 9, 0.125, 0.55),
            (BuiltinShape::Square, 7, 0.375, 0.55),
            (BuiltinShape::Square, 5, 0.625, 0.55),
            (BuiltinShape::Square, 3, 0.875, 0.55),
            (BuiltinShape::Dither, 5, 0.3, 0.83),
            (BuiltinShape::Dither, 9, 0.7, 0.83),
        ];
        let inner = block.shrink(3.0);
        let centre = |fx: f32, fy: f32| Pos2::new(inner.min.x + fx * inner.width(), inner.min.y + fy * inner.height());
        for (shape, size, fx, fy) in items {
            let c = centre(fx, fy);
            if !self.brush.custom && self.builtin == (shape, size) {
                p.rect_filled(Rect::from_center_size(c, Vec2::new(inner.width() / 4.0 - 1.0, inner.height() / 3.0 - 1.0)), 0.0, TEAL);
            }
            let s = (size as f32 * 1.3).max(2.0);
            match shape {
                BuiltinShape::Round => {
                    p.circle_filled(c, s / 2.0, BLACK);
                }
                BuiltinShape::Square => {
                    p.rect_filled(Rect::from_center_size(c, Vec2::splat(s)), 0.0, BLACK);
                }
                BuiltinShape::Dither => {
                    let n = size / 2 + 1;
                    for yy in 0..n {
                        for xx in 0..n {
                            if (xx + yy) % 2 == 0 {
                                let o = Vec2::new((xx as f32 - n as f32 / 2.0) * 2.5, (yy as f32 - n as f32 / 2.0) * 2.5);
                                p.rect_filled(Rect::from_min_size(c + o, Vec2::splat(2.0)), 0.0, BLACK);
                            }
                        }
                    }
                }
            }
        }
        let r = ui.interact(block, ui.scope_id().with("brushblock"), Sense::click());
        if let Some(pos) = r.hover_pos() {
            let best = items
                .iter()
                .min_by(|a, b| {
                    let da = (centre(a.2, a.3) - pos).length();
                    let db = (centre(b.2, b.3) - pos).length();
                    da.partial_cmp(&db).unwrap()
                })
                .cloned()
                .unwrap();
            if r.clicked() {
                self.set_builtin(best.0, best.1);
            }
            r.on_hover_text(format!("{:?} brush, {} px", best.0, best.1));
        }
    }

    pub(crate) fn tool_tile(&mut self, ui: &mut egui::Ui, p: &egui::Painter, rect: Rect, id: &str) {
        use crate::ui_style::*;
        let tool = match id {
            "dotted" => Some(Tool::Dotted),
            "freehand" => Some(Tool::Freehand),
            "line" => Some(Tool::Line),
            "curve" => Some(Tool::Curve),
            "fill" => Some(Tool::Fill),
            "airbrush" => Some(Tool::Airbrush),
            "rect" => Some(Tool::Rect),
            "circle" => Some(Tool::Circle),
            "ellipse" => Some(Tool::Ellipse),
            "polygon" => Some(Tool::Polygon),
            "brush" => Some(Tool::BrushPick),
            "text" => Some(Tool::Text),
            _ => None,
        };
        let selected = match id {
            "brush" => matches!(self.tool, Tool::BrushPick | Tool::Lasso),
            "grid" => self.set.grid_on,
            "symmetry" => self.set.sym.on,
            "magnify" => self.magnify,
            _ => tool == Some(self.tool),
        };
        let resp = ui.interact(rect, ui.scope_id().with(("tile", id)), Sense::click());
        let pressed = resp.is_pointer_button_down_on();
        let inverted = selected ^ pressed;
        p.rect_filled(rect, 0.0, if inverted { BLACK } else { GREY });
        if !inverted {
            // White highlight on the top and left edges, as in the original's tiles.
            p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width(), 2.0)), 0.0, WHITE);
            p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())), 0.0, WHITE);
        }
        let ink = if inverted { WHITE } else { BLACK };
        let icon = match id {
            "dotted" => ICON_DOTTED,
            "freehand" => ICON_FREEHAND,
            "line" => ICON_LINE,
            "curve" => ICON_CURVE,
            "fill" => ICON_FILL,
            "airbrush" => ICON_AIRBRUSH,
            "rect" => ICON_RECT,
            "circle" => ICON_CIRCLE,
            "ellipse" => ICON_ELLIPSE,
            "polygon" => ICON_POLYGON,
            "brush" => ICON_BRUSH,
            "text" => ICON_TEXT,
            "grid" => ICON_GRID,
            "symmetry" => ICON_SYMMETRY,
            "magnify" => ICON_MAGNIFY,
            "zoom" => ICON_ZOOM,
            "undo" => ICON_UNDO,
            _ => ICON_CLR,
        };
        let px = ((rect.height() - 6.0) / 13.0).floor().clamp(1.0, 3.0);
        bitmap(p, rect.center() + Vec2::new(1.0, 1.0), icon, px, ink);
        // Split tiles: the lower-right half chooses the filled shape (or freehand brush selection).
        let split = matches!(id, "freehand" | "rect" | "circle" | "ellipse" | "polygon" | "brush");
        if split && selected {
            let filled = if id == "brush" { self.tool == Tool::Lasso } else { self.filled };
            let mark = if filled { rect.right_bottom() - Vec2::new(6.0, 6.0) } else { rect.left_top() + Vec2::new(6.0, 6.0) };
            p.circle_filled(mark, 2.5, TEAL);
        }
        let tip = match tool {
            Some(_) if id == "brush" => "Brush selector (b)
Upper-left half: rectangle · lower-right half: freehand outline".to_string(),
            Some(t) if split => format!("{}\nUpper-left half: outline · lower-right half: filled\nRight-click: settings", t.label()),
            Some(t) => format!("{}\nRight-click: settings", t.label()),
            None => match id {
                "grid" => "Grid (g) — right-click: grid settings".into(),
                "symmetry" => "Symmetry (/) — right-click: symmetry settings".into(),
                "magnify" => "Magnify (m) — the mouse wheel changes the magnification".into(),
                "zoom" => "Zoom: left-click in, right-click out".into(),
                "undo" => "Undo (u)".into(),
                _ => "Clear (K)".into(),
            },
        };
        let lower_right = resp
            .interact_pointer_pos()
            .map(|pos| {
                let v = pos - rect.min;
                v.x / rect.width() + v.y / rect.height() > 1.0
            })
            .unwrap_or(false);
        let clicked = resp.clicked();
        let right = resp.secondary_clicked();
        resp.on_hover_text(tip);
        if clicked {
            match id {
                "grid" => self.set.grid_on = !self.set.grid_on,
                "symmetry" => self.set.sym.on = !self.set.sym.on,
                "magnify" => {
                    self.magnify = !self.magnify;
                    self.mag_center = (self.doc.w as f32 / 2.0, self.doc.h as f32 / 2.0);
                }
                "zoom" => self.zoom = (self.zoom * 2.0).min(32.0),
                "undo" => {
                    self.doc.undo();
                }
                "clr" => {
                    self.doc.clear(self.bg);
                    self.move_forget_end();
                },
                // (right button: see below)
                "brush" => self.tool = if lower_right { Tool::Lasso } else { Tool::BrushPick },
                _ => {
                    if let Some(t) = tool {
                        self.tool = t;
                        self.filled = split && lower_right;
                    }
                }
            }
        }
        if right {
            match id {
                "fill" => self.show_fill = true,
                "airbrush" => self.show_airbrush = true,
                "dotted" | "freehand" => self.show_spacing = true,
                "text" => self.show_font = true,
                // In perspective mode the grid tool's requester is the Perspective requester.
                "grid" if self.v5.x.persp_mode => self.show_perspective = true,
                "grid" => self.show_grid = true,
                "symmetry" => self.show_symmetry = true,
                "zoom" => self.zoom = (self.zoom / 2.0).max(0.125),
                // CLR with the right button: clear this frame, a range of frames or all frames.
                "clr" => self.open_clear(),
                "rect" | "circle" | "ellipse" | "polygon" => {
                    self.tool = tool.unwrap();
                    self.filled = !self.filled;
                }
                _ => {}
            }
        }
    }

    pub(crate) fn palette_page(&mut self, ui: &mut egui::Ui, p: &egui::Painter, area: Rect, pal: &crate::palette::Palette) {
        // As many cells as the screen has colours: 64 per page in 4 columns; 32 or fewer in 2 columns,
        // as on the original's 32-colour low-res screen.
        let shown = self.doc.colors.clamp(2, 64);
        let cols = if shown >= 64 { 4usize } else if shown >= 4 { 2 } else { 1 };
        let rows = shown / cols;
        let cw = area.width() / cols as f32;
        let chh = (area.height() / rows as f32).max(4.0);
        let base = self.pal_page * 64;
        let cell = |k: usize| -> Rect {
            Rect::from_min_size(area.min + Vec2::new((k / rows) as f32 * cw, (k % rows) as f32 * chh), Vec2::new(cw, chh))
        };
        for k in 0..shown {
            let i = base + k;
            p.rect_filled(cell(k), 0.0, to_color32(pal[i]));
            if self.show_stencil && self.stencil_pick[i] {
                let r = cell(k);
                p.line_segment([r.left_top(), r.right_bottom()], Stroke::new(1.0, Color32::WHITE));
            }
        }
        if let Some(rg) = self.doc.ranges.get(self.set.range) {
            for k in 0..shown {
                if rg.contains((base + k) as u8) {
                    let r = cell(k);
                    p.line_segment([r.left_top(), r.left_bottom()], Stroke::new(2.0, Color32::from_rgb(255, 210, 60)));
                }
            }
        }
        for (c, col) in [(self.bg as usize, Color32::from_rgb(255, 60, 60)), (self.fg as usize, Color32::WHITE)] {
            if c >= base && c < base + shown {
                p.rect_stroke(cell(c - base).shrink(1.0), 0.0, Stroke::new(2.0, col), egui::StrokeKind::Inside);
            }
        }
        let resp = ui.interact(area, ui.scope_id().with("palette"), Sense::click());
        let hit = |pos: Pos2| -> Option<u8> {
            let v = pos - area.min;
            let c = (v.x / cw) as i32;
            let r = (v.y / chh) as i32;
            if c >= 0 && r >= 0 && (c as usize) < cols && (r as usize) < rows {
                Some((base + c as usize * rows + r as usize) as u8)
            } else {
                None
            }
        };
        if let Some(i) = resp.hover_pos().and_then(hit) {
            let c = self.doc.palette[i as usize];
            resp.clone().on_hover_text(format!("Colour {}  R{} G{} B{}\nLeft: foreground, right: background, double-click: edit", i, c[0], c[1], c[2]));
        }
        if resp.double_clicked() {
            self.show_palette = true;
        } else if resp.clicked() {
            if let Some(i) = resp.interact_pointer_pos().and_then(hit) {
                if let Some(action) = self.pal_action.take() {
                    self.palette_action(action, i);
                } else if self.show_stencil {
                    self.stencil_pick[i as usize] = !self.stencil_pick[i as usize];
                } else {
                    self.fg = i;
                }
            }
        } else if resp.secondary_clicked() {
            if let Some(i) = resp.interact_pointer_pos().and_then(hit) {
                self.bg = i;
            }
        }
    }

    pub fn palette_action(&mut self, action: PalAction, target: u8) {
        self.pal_undo = Some(self.doc.palette);
        self.doc.checkpoint();
        let a = self.fg;
        match action {
            PalAction::Spread => crate::palette::spread(&mut self.doc.palette, a, target),
            PalAction::Copy => self.doc.palette[target as usize] = self.doc.palette[a as usize],
            PalAction::Swap => self.doc.palette.swap(a as usize, target as usize),
            PalAction::RangeFromTo => {
                if let Some(r) = self.doc.ranges.get_mut(self.set.range) {
                    r.low = a.min(target);
                    r.high = a.max(target);
                }
            }
        }
        self.fg = target;
    }

    // ------------------------------------------------------------ info bar

    pub fn info_bar_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            let small = |s: String| egui::RichText::new(s).small().monospace();
            ui.label(small(format!("{}{}", self.doc.name(), if self.doc.dirty { " *" } else { "" })));
            ui.separator();
            ui.label(small(format!("{}×{}", self.doc.w, self.doc.h)));
            ui.separator();
            if let Some((x, y)) = self.cursor {
                ui.label(small(format!("x {:4}  y {:4}", x, y)));
                if let Some(d) = &self.drag {
                    let (w, h) = ((x - d.start.0).abs() + 1, (y - d.start.1).abs() + 1);
                    ui.label(small(format!("  {}×{}", w, h)));
                }
                if self.doc.img().inside(x, y) {
                    let c = self.doc.img().get(x, y);
                    ui.label(small(format!("  colour {}", c)));
                }
                ui.separator();
            }
            ui.label(small(format!("{} · {}", self.tool.label().split(" (").next().unwrap_or(""), self.set.mode.label())));
            if self.filled {
                ui.label(small(format!("· filled: {}", self.set.fill.label())));
            }
            ui.separator();
            ui.label(small(format!("frame {}/{}", self.doc.cur + 1, self.doc.frames.len())));
            ui.separator();
            ui.label(small(format!("zoom {:.0}%", self.zoom_percent(ui.ctx()))));
            let mut flags = Vec::new();
            if self.cycling {
                flags.push("CYCLE");
            }
            if self.set.translucent_on {
                flags.push("TRANSL");
            }
            if self.antialias {
                flags.push("AA");
            }
            if self.doc.stencil.as_ref().map(|s| s.on).unwrap_or(false) {
                flags.push("STENCIL");
            }
            if self.set.sym.on {
                flags.push("SYM");
            }
            if self.set.grid_on {
                flags.push("GRID");
            }
            if self.v5.x.fill_outline {
                flags.push("OUTLINE");
            }
            if !flags.is_empty() {
                ui.separator();
                ui.label(small(flags.join(" ")).color(ACCENT));
            }
            if !self.status.is_empty() {
                ui.separator();
                ui.label(small(self.status.clone()).color(crate::ui_style::DIM_TEXT));
            }
        });
    }

    // ------------------------------------------------------------ menus

    pub fn menu_bar(&mut self, ui: &mut egui::Ui) {
        use crate::actions::Cmd::*;
        let ctx = ui.ctx().clone();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Picture", |ui| {
                self.mi(ui, &ctx, "Load..", Load);
                self.mi(ui, &ctx, "Save..", Save);
                self.mi(ui, &ctx, "Save As..", SaveAs);
                self.mi(ui, &ctx, "Delete..", DeleteFile);
                self.mi(ui, &ctx, "Print..", Print);
                ui.menu_button("Flip", |ui| {
                    self.mi(ui, &ctx, "Horiz", FlipPicH);
                    self.mi(ui, &ctx, "Vert", FlipPicV);
                });
                ui.menu_button("Spare", |ui| {
                    self.mi(ui, &ctx, "Swap", SpareSwap);
                    self.mi(ui, &ctx, "Copy to Spare", SpareCopy);
                    self.mi(ui, &ctx, "Merge in front", MergeFront);
                    self.mi(ui, &ctx, "Merge in back", MergeBack);
                    self.mi(ui, &ctx, "Delete this Page", SpareDelete);
                });
                self.mi(ui, &ctx, "Page Size..", PageSize);
                self.mi(ui, &ctx, "Show Page", ShowPage);
                self.mi(ui, &ctx, "Screen Format..", ScreenFormat);
                self.mi(ui, &ctx, "About", About);
                self.mi(ui, &ctx, "Quit", Quit);
            });
            ui.menu_button("Brush", |ui| {
                self.mi(ui, &ctx, "Load..", BrushLoad);
                self.mi(ui, &ctx, "Save..", BrushSave);
                self.mi(ui, &ctx, "Copy", BrushCopy);
                self.mi(ui, &ctx, "Paste", BrushPaste);
                self.mi(ui, &ctx, "Restore", BrushRestore);
                ui.menu_button("Spare", |ui| {
                    self.mi(ui, &ctx, "Brush->Spare", BrushToSpare);
                    self.mi(ui, &ctx, "Brush<->Spare", BrushSwapSpare);
                    self.mi(ui, &ctx, "Morph->Spare..", BrushMorph);
                });
                ui.menu_button("Size", |ui| {
                    self.mi(ui, &ctx, "Stretch", BrushStretch);
                    self.mi(ui, &ctx, "Halve", BrushHalve);
                    self.mi(ui, &ctx, "Double", BrushDouble);
                    self.mi(ui, &ctx, "Double Horiz", BrushDoubleH);
                    self.mi(ui, &ctx, "Double Vert", BrushDoubleV);
                });
                ui.menu_button("Flip", |ui| {
                    self.mi(ui, &ctx, "Horiz", BrushFlipH);
                    self.mi(ui, &ctx, "Vert", BrushFlipV);
                });
                ui.menu_button("Edge", |ui| {
                    self.mi(ui, &ctx, "Outline", BrushOutline);
                    self.mi(ui, &ctx, "Trim", BrushTrim);
                });
                ui.menu_button("Rotate", |ui| {
                    self.mi(ui, &ctx, "90 Degrees", BrushRot90);
                    self.mi(ui, &ctx, "Any Angle", BrushRotAny);
                    self.mi(ui, &ctx, "Shear", BrushShear);
                });
                ui.menu_button("Bend", |ui| {
                    self.mi(ui, &ctx, "Horiz", BrushBendH);
                    self.mi(ui, &ctx, "Vert", BrushBendV);
                });
                ui.menu_button("Handle", |ui| {
                    self.mi(ui, &ctx, "Rotate", HandleRotate);
                    self.mi(ui, &ctx, "Place", HandlePlace);
                });
                self.mi(ui, &ctx, "  Solid Body", SolidBody);
                self.mi(ui, &ctx, "  Frame Only", FrameOnly);
                self.mi(ui, &ctx, "Free", BrushFree);
            });
            ui.menu_button("Mode", |ui| {
                for m in Mode::ALL {
                    let text = format!("{} {}", if self.set.mode == m { "✓" } else { "   " }, m.menu_name());
                    // HBrite only on Extra-Half-Brite screens (Screen Format).
                    let enabled = m != Mode::HBrite || self.doc.ehb;
                    if ui.add_enabled(enabled, egui::Button::new(text).shortcut_text(m.key())).on_hover_text(m.help()).on_disabled_hover_text(m.help()).clicked() {
                        self.set_mode(m);
                    }
                }
            });
            ui.menu_button("Anim", |ui| {
                self.mi(ui, &ctx, "Load..", AnimLoad);
                self.mi(ui, &ctx, "Save..", AnimSave);
                if ui.button("Save Frames..").clicked() {
                    self.with_scope(crate::v52::ScopeOp::SaveFrames);
                    ui.close();
                }
                if ui.button("Load Sequence..").clicked() {
                    self.open_sequence();
                    ui.close();
                }
                if ui.button("Load Options..").clicked() {
                    self.v5.x.show_load_opts = true;
                    ui.close();
                }
                ui.separator();
                self.mi(ui, &ctx, "Move..", Move);
                self.mi(ui, &ctx, "Camera Move..", CameraMove);
                ui.separator();
                ui.menu_button("AnimBrush", |ui| {
                    self.mi(ui, &ctx, "Load..", AnimBrushLoad);
                    self.mi(ui, &ctx, "Save..", AnimBrushSave);
                    self.mi(ui, &ctx, "Pick up..", AnimBrushPick);
                    self.mi(ui, &ctx, "Settings..", AnimBrushSettings);
                    self.mi(ui, &ctx, "Use", AnimBrushUse);
                    self.mi(ui, &ctx, "Free", AnimBrushFree);
                });
                ui.separator();
                ui.menu_button("Frames", |ui| {
                    self.mi(ui, &ctx, "Set # ..", FramesSet);
                    self.mi(ui, &ctx, "Add Frames..", FramesAdd);
                    self.mi(ui, &ctx, "Copy Frames..", FramesCopy);
                    self.mi(ui, &ctx, "Delete Frames..", FramesDelete);
                    self.mi(ui, &ctx, "Set Frame Rate..", FrameRate);
                    self.mi(ui, &ctx, "Set Frame Palette..", FramePalette);
                    self.mi(ui, &ctx, "Frame Palette Info..", FramePaletteInfo);
                });
                ui.menu_button("Control", |ui| {
                    self.mi(ui, &ctx, "Panel", Panel);
                    self.mi(ui, &ctx, "Set Range..", SetRange);
                    self.mi(ui, &ctx, "Previous", PrevFrame);
                    self.mi(ui, &ctx, "Next", NextFrame);
                    self.mi(ui, &ctx, "Go to..", GoTo);
                    self.mi(ui, &ctx, "Play", Play);
                    self.mi(ui, &ctx, "Play once", PlayOnce);
                    self.mi(ui, &ctx, "Ping-pong", PingPong);
                });
                ui.menu_button("Method", |ui| {
                    let _ = ui.add_enabled(false, egui::Button::new("✓ Compressed"));
                    let _ = ui.add_enabled(false, egui::Button::new("   Expanded"));
                    ui.label(egui::RichText::new("(memory is no longer limited)").small());
                });
            });
            ui.menu_button("Colour", |ui| {
                self.mi(ui, &ctx, "Ranges", Ranges);
                self.mi(ui, &ctx, "Cycle", Cycle);
                ui.menu_button("Palette", |ui| {
                    self.mi(ui, &ctx, "Mixer", Mixer);
                    self.mi(ui, &ctx, "Arrange", Arrange);
                    self.mi(ui, &ctx, "Use Brush Palette", UseBrushPalette);
                    self.mi(ui, &ctx, "Restore Palette", RestorePalette);
                    self.mi(ui, &ctx, "Default Palette", DefaultPalette);
                    self.mi(ui, &ctx, "Load..", PaletteLoad);
                    self.mi(ui, &ctx, "Save..", PaletteSave);
                });
                ui.menu_button("Colour Set", |ui| {
                    self.mi(ui, &ctx, "Load..", ColourSetLoad);
                    self.mi(ui, &ctx, "Save..", ColourSetSave);
                });
                self.mi(ui, &ctx, "Bg -> Fg", BgToFg);
                self.mi(ui, &ctx, "Bg <-> Fg", BgSwapFg);
                self.mi(ui, &ctx, "Remap", Remap);
                let _ = ui.add_enabled(false, egui::Button::new("Recompute"));
                ui.menu_button("Brush", |ui| {
                    self.mi(ui, &ctx, "Bg -> Fg", BrushBgToFg);
                    self.mi(ui, &ctx, "Bg <-> Fg", BrushBgSwapFg);
                    self.mi(ui, &ctx, "Remap", BrushRemap);
                    self.mi(ui, &ctx, "Change Transparency", BrushTransparency);
                });
            });
            ui.menu_button("Effect", |ui| {
                ui.menu_button("Stencil", |ui| {
                    self.mi(ui, &ctx, "Show", StencilShow);
                    self.mi(ui, &ctx, "Make", StencilMake);
                    self.mi(ui, &ctx, "Remake", StencilRemake);
                    self.mi(ui, &ctx, "  Paint", StencilPaint);
                    self.mi(ui, &ctx, "Reverse", StencilReverse);
                    self.mi(ui, &ctx, "  On/Off", StencilToggle);
                    self.mi(ui, &ctx, "Free", StencilFree);
                    self.mi(ui, &ctx, "Load..", StencilLoad);
                    self.mi(ui, &ctx, "Save..", StencilSave);
                });
                ui.menu_button("LightTable", |ui| {
                    self.mi(ui, &ctx, "On/Off", LightTableToggle);
                    self.mi(ui, &ctx, "Merge", LightTableMerge);
                    self.mi(ui, &ctx, "Dim", LightTableDim);
                    self.mi(ui, &ctx, "Layer 1", Layer1);
                    self.mi(ui, &ctx, "Layer 2", Layer2);
                    self.mi(ui, &ctx, "Layer 3", Layer3);
                    self.mi(ui, &ctx, "Layer 4", Layer4);
                    self.mi(ui, &ctx, "Settings..", LightTableSettings);
                });
                ui.menu_button("Background", |ui| {
                    self.mi(ui, &ctx, "Fix", BgFix);
                    self.mi(ui, &ctx, "Free", BgFree);
                    self.mi(ui, &ctx, "Lock FG", LockFg);
                });
                ui.menu_button("AntiAlias", |ui| {
                    self.mi(ui, &ctx, "None", AaNone);
                    self.mi(ui, &ctx, "Low", AaLow);
                    self.mi(ui, &ctx, "High", AaHigh);
                });
                ui.menu_button("Process", |ui| {
                    self.mi(ui, &ctx, "On/Off", ProcessToggle);
                    self.mi(ui, &ctx, "Tint", ProcessTint);
                    self.mi(ui, &ctx, "Hue", ProcessHue);
                    self.mi(ui, &ctx, "Value", ProcessValue);
                });
                ui.menu_button("Translucency", |ui| {
                    self.mi(ui, &ctx, "On/Off", TranslucentToggle);
                    self.mi(ui, &ctx, "Settings..", TranslucentSettings);
                });
                ui.menu_button("Perspective", |ui| {
                    self.mi(ui, &ctx, "Do", PerspDo);
                    self.mi(ui, &ctx, "Fill Screen", PerspFillScreen);
                    self.mi(ui, &ctx, "Reset", PerspReset);
                    self.mi(ui, &ctx, "Centre", PerspCentre);
                    self.mi(ui, &ctx, "Settings..", PerspSettings);
                });
                ui.menu_button("Pressure", |ui| {
                    self.mi(ui, &ctx, "On/Off", PressureToggle);
                    self.mi(ui, &ctx, "Settings..", PressureSettings);
                });
                ui.menu_button("Media", |ui| {
                    self.mi(ui, &ctx, "Open..", MediaOpen);
                    self.mi(ui, &ctx, "Close", MediaClose);
                });
                ui.menu_button("Texture", |ui| {
                    self.mi(ui, &ctx, "On/Off", TextureToggle);
                    self.mi(ui, &ctx, "Load..", TextureLoad);
                    self.mi(ui, &ctx, "Invert", TextureInvert);
                    self.mi(ui, &ctx, "Free", TextureFree);
                });
            });
            ui.menu_button("Macro", |ui| {
                self.mi(ui, &ctx, "Record Macro", MacroRecord);
                self.mi(ui, &ctx, "Record Details", MacroDetails);
                ui.separator();
                self.mi(ui, &ctx, "Execute Macro..", MacroExecute);
                ui.separator();
                self.mi(ui, &ctx, "Load..", MacroLoad);
                self.mi(ui, &ctx, "Save As..", MacroSaveAs);
            });
            ui.menu_button("Prefs", |ui| {
                self.mi(ui, &ctx, "Coords", PrefCoords);
                self.mi(ui, &ctx, "Fast FB", PrefFastFb);
                self.mi(ui, &ctx, "MultiCycle", PrefMultiCycle);
                self.mi(ui, &ctx, "Be Square", PrefBeSquare);
                self.mi(ui, &ctx, "Workbench", PrefWorkbench);
                self.mi(ui, &ctx, "ExclBrush", PrefExclBrush);
                self.mi(ui, &ctx, "AutoTransp", PrefAutoTransp);
                self.mi(ui, &ctx, "AutoGrid", PrefAutoGrid);
                self.mi(ui, &ctx, "OriginUL", PrefOriginUl);
                let _ = ui.add_enabled(false, egui::Button::new("   FastAdjust"));
                self.mi(ui, &ctx, "MouseTrack", PrefMouseTrack);
                let t = format!("{} Title", if self.v5.x.title { "✓" } else { "   " });
                if ui.button(t).on_hover_text("Show the menu bar all the time (off: it appears when the pointer is at the top)").clicked() {
                    self.v5.x.title = !self.v5.x.title;
                    ui.close();
                }
                let t = format!("{} Cursor", if self.v5.x.crosshair { "✓" } else { "   " });
                if ui.button(t).on_hover_text("Cross-hair pointer over the picture").clicked() {
                    self.v5.x.crosshair = !self.v5.x.crosshair;
                    ui.close();
                }
                ui.separator();
                ui.menu_button("Toolbox", |ui| {
                    self.mi(ui, &ctx, "Airbrush..", ToolAirbrush);
                    self.mi(ui, &ctx, "Grid..", ToolGrid);
                    self.mi(ui, &ctx, "Symmetry..", ToolSymmetry);
                    self.mi(ui, &ctx, "Fill..", ToolFill);
                    self.mi(ui, &ctx, "Spacing..", ToolSpacing);
                    self.mi(ui, &ctx, "Fonts..", ToolFonts);
                });
                let _ = ui.add_enabled(false, egui::Button::new("Icons.."));
                self.mi(ui, &ctx, "Font Dir..", FontDir);
                ui.separator();
                self.mi(ui, &ctx, "Default", PrefsDefault);
                self.mi(ui, &ctx, "Last Saved", PrefsLastSaved);
                self.mi(ui, &ctx, "Save", PrefsSave);
                ui.separator();
                self.mi(ui, &ctx, "Info Bar", InfoBar);
                self.mi(ui, &ctx, "Fullscreen", Fullscreen);
                self.mi(ui, &ctx, "Gallery (slideshow)..", Gallery);
                self.mi(ui, &ctx, "Camera (grab screen)..", Camera);
                self.mi(ui, &ctx, "Convert picture..", Convert);
                self.mi(ui, &ctx, "Keyboard shortcuts..", Keys);
            });
            ui.menu_button("User", |ui| {
                for i in 0..10 {
                    let name = self.v5.slots[i].as_ref().map(|m| m.name.clone()).unwrap_or_else(|| format!("Macro #{}", i + 1));
                    let key = format!("Ctrl+{}", (i + 1) % 10);
                    if ui.add(egui::Button::new(name).shortcut_text(key)).clicked() {
                        self.run_user_macro(i);
                    }
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Screen depth gadget, as in the top-right corner of the original's screen.
                let (r, resp) = ui.allocate_exact_size(Vec2::new(30.0, 20.0), Sense::click());
                ui.painter().rect_filled(r, 0.0, crate::ui_style::GREY);
                crate::ui_style::bevel(ui.painter(), r, 1.5, resp.is_pointer_button_down_on());
                let back = Rect::from_min_size(r.center() + Vec2::new(-10.0, -7.0), Vec2::new(13.0, 9.0));
                let front = Rect::from_min_size(r.center() + Vec2::new(-3.0, -2.0), Vec2::new(13.0, 9.0));
                ui.painter().rect_stroke(back, 0.0, Stroke::new(1.5, crate::ui_style::BLACK), egui::StrokeKind::Inside);
                ui.painter().rect_filled(front, 0.0, crate::ui_style::WHITE);
                ui.painter().rect_stroke(front, 0.0, Stroke::new(1.5, crate::ui_style::BLACK), egui::StrokeKind::Inside);
                if resp.on_hover_text("Show Page (S): hide menus and tools").clicked() {
                    self.view_page = true;
                }
                ui.add_space(8.0);
                // Coordinates (Prefs > Coords): from the lower-left, or upper-left with OriginUL;
                // while a button is held, the distance from where it was pressed.
                if self.v5.prefs.coords {
                    if let Some((x, y)) = self.cursor {
                        let text = match self.v5.coord_origin {
                            Some((ox, oy)) => format!("{:>5} {:>5}", x - ox, if self.v5.prefs.origin_ul { y - oy } else { oy - y }),
                            None => format!("{:>5} {:>5}", x, if self.v5.prefs.origin_ul { y } else { self.doc.h - 1 - y }),
                        };
                        ui.label(egui::RichText::new(text).monospace().color(crate::ui_style::BLACK));
                    }
                } else if (self.show_perspective || self.v5.persp_place) && !self.v5.x.persp_mode {
                    let (ax, ay, az) = self.persp;
                    ui.label(egui::RichText::new(format!("{:>4.0} {:>4.0} {:>4.0}", ax, ay, az)).monospace().color(crate::ui_style::BLACK));
                }
                ui.add_space(8.0);
                // Status letters, as in the original's title bar.
                let mut flags = String::new();
                if self.v5.recording.is_some() {
                    flags.push_str("REC ");
                }
                if self.doc.stencil.as_ref().map(|s| s.on).unwrap_or(false) {
                    flags.push(if self.v5.stencil_show { 's' } else { 'S' });
                }
                if self.set.process_on {
                    flags.push('P');
                }
                if self.set.translucent_on {
                    flags.push('T');
                }
                if self.set.texture_on {
                    flags.push('X');
                }
                if !flags.is_empty() {
                    ui.label(egui::RichText::new(flags).monospace().color(crate::ui_style::TEAL));
                    ui.add_space(8.0);
                }
                ui.label(egui::RichText::new(self.set.mode.label()).color(crate::ui_style::BLACK));
                // Frame counter, as in the original's title bar ("1/10").
                if self.doc.frames.len() > 1 {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(format!("{}/{}", self.doc.cur + 1, self.doc.frames.len())).monospace().color(crate::ui_style::BLACK));
                }
                // Perspective mode: the brush's angles (or its 3-D position, toggled with \).
                if let Some(t) = self.persp_readout() {
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(t).monospace().color(crate::ui_style::BLACK))
                        .on_hover_text(if self.v5.x.p3.show_pos { "Brush position X Y Z (the backslash key shows the angles)" } else { "Brush angles X Y Z (the backslash key shows the position)" });
                }
            });
        });
    }

    /// One menu item for a command: check mark, keyboard equivalent, greyed when unusable.
    pub fn mi(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, label: &str, c: crate::actions::Cmd) {
        let text = match self.cmd_checked(c) {
            Some(true) => format!("✓ {}", label.trim_start()),
            Some(false) => format!("   {}", label.trim_start()),
            None => label.to_string(),
        };
        let btn = egui::Button::new(text).shortcut_text(c.shortcut());
        if ui.add_enabled(self.cmd_enabled(c), btn).clicked() {
            self.cmd(ctx, c);
        }
    }

}

#[allow(dead_code)]
fn unused(_: FillKind) {}
