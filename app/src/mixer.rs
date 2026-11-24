//! The Mixer (Colour > Palette > Mixer, p), as a panel along the bottom of the screen like the
//! original (manual fig. 8.40 and the person's Amiga screenshot): R G B H S V sliders with their
//! values; OK, REVERT, UNDO, SCALE and GRID; two rows of 16 colours with up/down arrows; COPY,
//! SPREAD, EX, SELECT and DELETE; a mixing area to paint and mix colours in; the chosen colour
//! with its number and an eyedropper.

use crate::app::{App, PalAction, Tool};
use crate::palette::{hsv_to_rgb, rgb_to_hsv, to_color32};
use crate::ui_style::{bevel, BLACK, GREY, TEAL, WHITE};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

pub const MIX_W: usize = 96;
pub const MIX_H: usize = 24;

/// The Mixer's own state.
pub struct MixerState {
    /// The mixing area, in true colour.
    pub area: Vec<[u8; 3]>,
    /// The palette when the Mixer opened (REVERT).
    pub backup: Option<crate::palette::Palette>,
    /// The first row of 16 colours shown.
    pub page: usize,
    /// Arrange (Shift+P, and after loading a colour set): seven rows of the colour set in place of
    /// the mixing area, to put colours where they are wanted.
    pub arrange: bool,
    /// SELECT: the first colour, waiting for the second; and the selected colours.
    pub select_from: bool,
    pub selected: Vec<u8>,
    /// The eyedropper: the next click in the mixing area takes its colour into the chosen cell.
    pub pick: bool,
    pub grid: bool,
}

impl Default for MixerState {
    fn default() -> Self {
        MixerState { area: vec![[0, 0, 0]; MIX_W * MIX_H], backup: None, page: 0, arrange: false, select_from: false, selected: Vec::new(), pick: false, grid: false }
    }
}

fn gadget(ui: &mut egui::Ui, rect: Rect, id: egui::Id, text: &str, lit: bool) -> egui::Response {
    let resp = ui.interact(rect, id, Sense::click());
    let down = resp.is_pointer_button_down_on() || lit;
    let p = ui.painter();
    p.rect_filled(rect, 0.0, if down { TEAL } else { GREY });
    bevel(p, rect, 1.5, down);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(13.0), if down { WHITE } else { BLACK });
    resp
}


impl App {
    pub fn mixer_panel(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(full, 0.0, GREY);
        bevel(&p, full, 2.0, false);
        if self.v5.x.mixer.backup.is_none() {
            self.v5.x.mixer.backup = Some(self.doc.palette);
        }
        let font = egui::FontId::proportional(13.0);
        let x0 = full.min.x + 6.0;
        let fg = self.fg as usize;
        let ncol = self.doc.colors.clamp(2, 256);
        let ehb_locked = self.doc.ehb && (32..64).contains(&fg);
        let gun8 = self.doc.gun_bits >= 8;
        // ---- R G B H S V sliders
        let c = self.doc.palette[fg];
        let (h, s, v) = rgb_to_hsv(c);
        let vals = [c[0] as f32, c[1] as f32, c[2] as f32, h, s * 100.0, v * 100.0];
        let maxs = [255.0, 255.0, 255.0, 360.0, 100.0, 100.0];
        let mut changed: Option<(usize, f32)> = None;
        for (k, label) in ["R", "G", "B", "H", "S", "V"].iter().enumerate() {
            let y = full.min.y + 4.0 + k as f32 * 15.0 + if k >= 3 { 3.0 } else { 0.0 };
            p.text(Pos2::new(x0, y + 6.0), egui::Align2::LEFT_CENTER, *label, font.clone(), BLACK);
            let track = Rect::from_min_size(Pos2::new(x0 + 14.0, y), Vec2::new(210.0, 12.0));
            p.rect_filled(track, 0.0, BLACK);
            let kx = track.min.x + (track.width() - 18.0) * (vals[k] / maxs[k]).clamp(0.0, 1.0);
            p.rect_filled(Rect::from_min_size(Pos2::new(kx, track.min.y + 2.0), Vec2::new(18.0, 8.0)), 0.0, GREY);
            p.text(Pos2::new(track.max.x + 6.0, y + 6.0), egui::Align2::LEFT_CENTER, format!("{:03}", vals[k].round() as i32), font.clone(), BLACK);
            let r = ui.interact(track, ui.scope_id().with(("mx_sl", k)), Sense::click_and_drag());
            if let Some(pos) = r.interact_pointer_pos().filter(|_| r.dragged() || r.clicked()) {
                changed = Some((k, ((pos.x - track.min.x - 9.0) / (track.width() - 18.0)).clamp(0.0, 1.0) * maxs[k]));
            }
        }
        // The number of the first colour of the page, as the original shows above the strip.
        p.text(Pos2::new(x0 + 262.0, full.min.y + 13.0), egui::Align2::LEFT_CENTER, format!("{}", self.v5.x.mixer.page * 16), font.clone(), BLACK);
        if let Some((k, val)) = changed {
            if !ehb_locked {
                if self.pal_undo.is_none() {
                    self.pal_undo = Some(self.doc.palette);
                }
                let mut nc = c;
                match k {
                    0..=2 => nc[k] = val.round() as u8,
                    3 => nc = hsv_to_rgb(val, s, v),
                    4 => nc = hsv_to_rgb(h, val / 100.0, v),
                    _ => nc = hsv_to_rgb(h, s, val / 100.0),
                }
                if !gun8 {
                    nc = nc.map(|x| crate::amiga::gun(x, 4));
                }
                self.doc.palette[fg] = nc;
                self.doc.dirty = true;
            } else {
                self.status = "Extra-Half-Brite: colours 32-63 follow colours 0-31".into();
            }
        }
        // ---- OK / REVERT / UNDO / SCALE and GRID
        let yb = full.max.y - 21.0;
        let mut bx = x0;
        for (k, (label, w)) in [("OK", 34.0), ("REVERT", 70.0), ("UNDO", 52.0), ("SCALE", 58.0)].iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(bx, yb), Vec2::new(*w, 18.0));
            let tip = ["Take the changes and close (the mixing area is cleared)", "The palette as it was when the Mixer opened", "Undo the last change", "Old 4-bit colour values (0-15) to 8-bit (0-255)"][k];
            if gadget(ui, r, ui.scope_id().with(("mx_b", k)), label, false).on_hover_text(tip).clicked() {
                match k {
                    0 => {
                        self.show_palette = false;
                        self.v5.x.mixer.backup = None;
                        self.v5.x.mixer.area.iter_mut().for_each(|c| *c = [0, 0, 0]);
                    }
                    1 => {
                        if let Some(b) = self.v5.x.mixer.backup {
                            self.pal_undo = Some(self.doc.palette);
                            self.doc.palette = b;
                        }
                    }
                    2 => {
                        if let Some(u) = self.pal_undo.take() {
                            self.doc.palette = u;
                        }
                    }
                    _ => {
                        self.pal_undo = Some(self.doc.palette);
                        if self.doc.palette.iter().all(|c| c.iter().all(|v| *v <= 15)) {
                            self.doc.palette.iter_mut().for_each(|c| c.iter_mut().for_each(|v| *v *= 17));
                        } else {
                            self.status = "SCALE: the colours are already 8-bit values".into();
                        }
                    }
                }
            }
            bx += w + 2.0;
        }
        let gb = Rect::from_min_size(Pos2::new(bx + 8.0, yb), Vec2::new(18.0, 18.0));
        if gadget(ui, gb, ui.scope_id().with("mx_grid"), if self.v5.x.mixer.grid { "✓" } else { "" }, false).clicked() {
            self.v5.x.mixer.grid = !self.v5.x.mixer.grid;
        }
        p.text(Pos2::new(gb.max.x + 6.0, gb.center().y), egui::Align2::LEFT_CENTER, "GRID", font.clone(), BLACK);
        // ---- palette strip: two rows of 16 with arrows
        let px0 = x0 + 290.0;
        let pal = self.display_palette();
        let avail_w = full.max.x - px0 - 90.0;
        let cw = (avail_w / 16.0).clamp(10.0, 40.0);
        // Two rows in the Mixer, seven in Arrange; the whole colour set (16 rows) can be scrolled.
        let arrange = self.v5.x.mixer.arrange;
        let rows = if arrange { 7usize } else { 2 };
        let rh = if arrange { ((full.height() - 34.0) / rows as f32).floor() } else { 15.0 };
        let _ = ncol;
        self.v5.x.mixer.page = self.v5.x.mixer.page.min(16 - rows);
        let base = self.v5.x.mixer.page * 16;
        let shown = rows * 16;
        let strip = Rect::from_min_size(Pos2::new(px0, full.min.y + 6.0), Vec2::new(cw * 16.0, rh * rows as f32));
        for k in 0..shown {
            let i = base + k;
            let r = Rect::from_min_size(strip.min + Vec2::new((k % 16) as f32 * cw, (k / 16) as f32 * rh), Vec2::new(cw, rh));
            p.rect_filled(r, 0.0, to_color32(pal[i.min(255)]));
            if self.v5.x.mixer.grid {
                // Screen colours white, the rest of the colour set blue (the original's GRID).
                let c = if i < ncol { WHITE } else { Color32::from_rgb(120, 160, 255) };
                p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, c));
                p.line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0, c));
            }
            if self.v5.x.mixer.selected.contains(&(i as u8)) {
                p.rect_stroke(r.shrink(2.0), 0.0, Stroke::new(1.0, BLACK), egui::StrokeKind::Inside);
            }
            if i == fg {
                p.rect_stroke(r.shrink(1.0), 0.0, Stroke::new(2.0, WHITE), egui::StrokeKind::Inside);
            }
        }
        let sr = ui.interact(strip, ui.scope_id().with("mx_strip"), Sense::click());
        if let Some(pos) = sr.interact_pointer_pos().filter(|_| sr.clicked() || sr.secondary_clicked()) {
            let v = pos - strip.min;
            let k = (v.y / rh) as usize * 16 + (v.x / cw) as usize;
            let c = (base + k.min(shown - 1)).min(255) as u8;
            let (alt, shift) = ui.input(|i| (i.modifiers.alt, i.modifiers.shift));
            if sr.secondary_clicked() {
                self.bg = c;
            } else if let Some(a) = self.pal_action.take() {
                self.palette_action(a, c);
            } else if self.v5.x.mixer.select_from || alt {
                // SELECT (or Alt+click): the colours from the chosen one to this one.
                let (lo, hi) = (self.fg.min(c), self.fg.max(c));
                self.v5.x.mixer.selected = (lo..=hi).collect();
                self.v5.x.mixer.select_from = false;
            } else if shift {
                if !self.v5.x.mixer.selected.contains(&c) {
                    self.v5.x.mixer.selected.push(c);
                }
            } else {
                self.v5.x.mixer.selected = vec![c];
                self.fg = c;
            }
        }
        let half = strip.height() / 2.0 - 1.0;
        let up = Rect::from_min_size(Pos2::new(strip.max.x + 2.0, strip.min.y), Vec2::new(16.0, half));
        let down = Rect::from_min_size(Pos2::new(strip.max.x + 2.0, strip.min.y + half + 2.0), Vec2::new(16.0, half));
        let shift = ui.input(|i| i.modifiers.shift);
        let last = 16 - rows;
        // One row at a time (Shift: the first / last rows).
        if gadget(ui, up, ui.scope_id().with("mx_up"), "↑", false).on_hover_text("One row up (Shift: the first row)").clicked() {
            self.v5.x.mixer.page = if shift { 0 } else { self.v5.x.mixer.page.saturating_sub(1) };
        }
        if gadget(ui, down, ui.scope_id().with("mx_down"), "↓", false).on_hover_text("One row down (Shift: the last row)").clicked() {
            self.v5.x.mixer.page = if shift { last } else { (self.v5.x.mixer.page + 1).min(last) };
        }
        // ---- COPY / SPREAD / EX / SELECT / DELETE (in Arrange along the bottom, after GRID)
        let ab_y = if arrange { full.max.y - 21.0 } else { strip.max.y + 4.0 };
        let mut ax = if arrange { px0 + 14.0 } else { px0 };
        for (k, (label, w)) in [("COPY", 52.0), ("SPREAD", 70.0), ("EX", 30.0), ("SELECT", 66.0), ("DELETE", 66.0)].iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(ax, ab_y), Vec2::new(*w, 16.0));
            let lit = match k {
                0 => self.pal_action == Some(PalAction::Copy),
                1 => self.pal_action == Some(PalAction::Spread),
                2 => self.pal_action == Some(PalAction::Swap),
                3 => self.v5.x.mixer.select_from,
                _ => false,
            };
            let tip = [
                "Copy the chosen colour into the cell you click next",
                "A gradient from the chosen colour to the one you click next",
                "Exchange the chosen colour with the one you click next",
                "Select the colours from the chosen one to the one you click next (Alt+click too; Shift+click adds one)",
                "Empty the selected colours (black)",
            ][k];
            if gadget(ui, r, ui.scope_id().with(("mx_a", k)), label, lit).on_hover_text(tip).clicked() {
                match k {
                    0 => self.pal_action = Some(PalAction::Copy),
                    1 => self.pal_action = Some(PalAction::Spread),
                    2 => self.pal_action = Some(PalAction::Swap),
                    3 => self.v5.x.mixer.select_from = !self.v5.x.mixer.select_from,
                    _ => {
                        self.pal_undo = Some(self.doc.palette);
                        let sel = if self.v5.x.mixer.selected.is_empty() { vec![self.fg] } else { self.v5.x.mixer.selected.clone() };
                        for i in sel {
                            if !(self.doc.ehb && (32..64).contains(&(i as usize))) {
                                self.doc.palette[i as usize] = [0, 0, 0];
                            }
                        }
                    }
                }
            }
            ax += w + 2.0;
        }
        // ---- the mixing area (not in Arrange)
        if arrange {
            self.mixer_swatch(ui, &p, full, &pal, fg);
            return;
        }
        let area = Rect::from_min_max(Pos2::new(px0, ab_y + 20.0), Pos2::new(strip.max.x, full.max.y - 6.0));
        p.rect_filled(area, 0.0, BLACK);
        let (cwm, chm) = (area.width() / MIX_W as f32, area.height() / MIX_H as f32);
        for y in 0..MIX_H {
            for x in 0..MIX_W {
                let c = self.v5.x.mixer.area[y * MIX_W + x];
                if c != [0, 0, 0] {
                    let r = Rect::from_min_size(area.min + Vec2::new(x as f32 * cwm, y as f32 * chm), Vec2::new(cwm + 0.5, chm + 0.5));
                    p.rect_filled(r, 0.0, Color32::from_rgb(c[0], c[1], c[2]));
                }
            }
        }
        p.rect_stroke(area, 0.0, Stroke::new(2.0, WHITE), egui::StrokeKind::Inside);
        let mr = ui.interact(area, ui.scope_id().with("mx_area"), Sense::click_and_drag());
        if let Some(pos) = mr.interact_pointer_pos().filter(|_| mr.dragged() || mr.clicked() || mr.is_pointer_button_down_on()) {
            let v = pos - area.min;
            let (mx, my) = ((v.x / cwm) as i32, (v.y / chm) as i32);
            if self.v5.x.mixer.pick {
                // The eyedropper: the mixed colour goes into the chosen cell.
                if mr.clicked() && mx >= 0 && my >= 0 && (mx as usize) < MIX_W && (my as usize) < MIX_H {
                    self.pal_undo = Some(self.doc.palette);
                    self.doc.palette[fg] = self.v5.x.mixer.area[my as usize * MIX_W + mx as usize];
                    self.v5.x.mixer.pick = false;
                }
            } else {
                mix_paint(&mut self.v5.x.mixer.area, mx, my, self.doc.palette[fg]);
                ui.ctx().request_repaint();
            }
        }
        self.mixer_swatch(ui, &p, full, &pal, fg);
    }

    /// The chosen colour with its number, and the eyedropper (Mixer and Arrange).
    fn mixer_swatch(&mut self, ui: &mut egui::Ui, p: &egui::Painter, full: Rect, pal: &crate::palette::Palette, fg: usize) {
        // ---- the chosen colour and the eyedropper
        let sample = Rect::from_min_size(Pos2::new(full.max.x - 64.0, full.min.y + 6.0), Vec2::new(56.0, 30.0));
        p.rect_filled(sample, 0.0, BLACK);
        p.rect_filled(sample.shrink(5.0), 0.0, to_color32(pal[fg]));
        p.text(Pos2::new(sample.center().x, sample.max.y + 7.0), egui::Align2::CENTER_CENTER, format!("{}", self.fg), egui::FontId::proportional(11.0), BLACK);
        let dropper = Rect::from_min_size(Pos2::new(full.max.x - 46.0, full.max.y - 38.0), Vec2::new(34.0, 32.0));
        let dr = ui.interact(dropper, ui.scope_id().with("mx_dropper"), Sense::click());
        let lit = self.v5.x.mixer.pick || self.tool == Tool::Eyedropper;
        p.rect_filled(dropper, 0.0, if lit { TEAL } else { GREY });
        bevel(p, dropper, 1.5, lit);
        crate::ui_style::pipette_icon(p, dropper.shrink(3.0), if lit { WHITE } else { BLACK }, to_color32(pal[fg]));
        if dr.on_hover_text("Eyedropper: click the mixing area to take a mixed colour, or the picture (left: foreground, right: background)").clicked() {
            // A second click switches it off again (back to the tool before).
            self.v5.x.mixer.pick = !self.v5.x.mixer.pick;
            if self.v5.x.mixer.pick != (self.tool == Tool::Eyedropper) {
                self.toggle_picker();
            }
        }
    }
}

/// Paint in the mixing area: a soft round dab that mixes the colour with what is there.
pub fn mix_paint(area: &mut [[u8; 3]], x: i32, y: i32, c: [u8; 3]) {
    for dy in -2..=2i32 {
        for dx in -3..=3i32 {
            let (px, py) = (x + dx, y + dy);
            if px < 0 || py < 0 || px as usize >= MIX_W || py as usize >= MIX_H {
                continue;
            }
            let d = ((dx * dx) as f32 / 9.0 + (dy * dy) as f32 / 4.0).sqrt();
            if d > 1.0 {
                continue;
            }
            let a = 0.35 * (1.0 - d * 0.6);
            let o = &mut area[py as usize * MIX_W + px as usize];
            if *o == [0, 0, 0] {
                *o = c;
            } else {
                for k in 0..3 {
                    o[k] = (o[k] as f32 * (1.0 - a) + c[k] as f32 * a).round() as u8;
                }
            }
        }
    }
}
