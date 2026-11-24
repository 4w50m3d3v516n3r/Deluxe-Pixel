//! The two panels along the bottom of the screen: the animation control panel
//! (Anim > Control > Panel) and the Range requester (Colour > Ranges).

use crate::actions::Cmd;
use crate::app::App;
use crate::paint::Dither;
use crate::palette::{to_color32, Range};
use crate::ui_style::{bevel, bitmap, BLACK, GREY, TEAL, WHITE};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

/// A bevelled panel button with a pixel icon or a short text.
fn gadget(ui: &mut egui::Ui, rect: Rect, id: egui::Id, icon: Option<&[&str]>, text: &str, lit: bool) -> egui::Response {
    let resp = ui.interact(rect, id, Sense::click());
    let down = resp.is_pointer_button_down_on() || lit;
    let p = ui.painter();
    p.rect_filled(rect, 0.0, if down { TEAL } else { GREY });
    bevel(p, rect, 1.5, down);
    let ink = if down { WHITE } else { BLACK };
    match icon {
        Some(rows) => bitmap(p, rect.center(), rows, 2.0, ink),
        None => {
            p.text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(13.0), ink);
        }
    }
    resp
}

const IC_LOOP_BACK: &[&str] = &["..####..", ".#....#.", "#......#", "#..#...#", "#.##....", "####....", ".##.....", "..#....."];
const IC_ONCE_BACK: &[&str] = &["....##", "...###", "..####", ".#####", "..####", "...###", "....##"];
const IC_PREV: &[&str] = &["#...##", "#..###", "#.####", "######", "#.####", "#..###", "#...##"];
const IC_NEXT: &[&str] = &["##...#", "###..#", "####.#", "######", "####.#", "###..#", "##...#"];
const IC_ONCE: &[&str] = &["##....", "###...", "####..", "#####.", "####..", "###...", "##...."];
const IC_LOOP: &[&str] = &["..####..", ".#....#.", "#......#", "#...#..#", "....##.#", "....####", ".....##.", ".....#.."];
const IC_PINGPONG: &[&str] = &["..#..#..", ".##..##.", "###..###", "###..###", ".##..##.", "..#..#.."];
const IC_GOTO: &[&str] = &["......##.", "...#.####", "...######", "########.", "...######", "...#.####", "......##."];
const IC_BULB: &[&str] = &["..###..", ".#...#.", "#.....#", "#.....#", ".#...#.", "..#.#..", "..###..", "..###.."];
const IC_FIRST: &[&str] = &["#..#", "#.##", "####", "#.##", "#..#"];
const IC_LAST: &[&str] = &["#..#", "##.#", "####", "##.#", "#..#"];

impl App {
    pub fn anim_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(full, 0.0, BLACK);
        let n = self.doc.frames.len();
        // Row 1: the frame slider between "first" and "last" buttons.
        let r1 = Rect::from_min_max(full.min + Vec2::new(2.0, 2.0), Pos2::new(full.max.x - 2.0, full.min.y + 20.0));
        let first = Rect::from_min_size(r1.min, Vec2::new(18.0, r1.height()));
        let last = Rect::from_min_max(Pos2::new(r1.max.x - 18.0, r1.min.y), r1.max);
        if gadget(ui, first, ui.scope_id().with("ap_first"), Some(IC_FIRST), "", false).on_hover_text("First frame").clicked() {
            self.doc.go_to(0);
        }
        if gadget(ui, last, ui.scope_id().with("ap_last"), Some(IC_LAST), "", false).on_hover_text("Last frame").clicked() {
            self.doc.go_to(n - 1);
        }
        let track = Rect::from_min_max(Pos2::new(first.max.x + 3.0, r1.min.y), Pos2::new(last.min.x - 3.0, r1.max.y));
        p.rect_filled(track, 0.0, GREY);
        bevel(&p, track, 1.5, true);
        let knob_w = (track.width() / n as f32).max(12.0);
        let kx = if n > 1 { track.min.x + (track.width() - knob_w) * self.doc.cur as f32 / (n - 1) as f32 } else { track.min.x };
        let knob = Rect::from_min_size(Pos2::new(kx, track.min.y + 2.0), Vec2::new(knob_w, track.height() - 4.0));
        p.rect_filled(knob, 0.0, GREY);
        bevel(&p, knob, 1.5, false);
        p.text(knob.center(), egui::Align2::CENTER_CENTER, format!("{}", self.doc.cur + 1), egui::FontId::proportional(11.0), BLACK);
        let tr = ui.interact(track, ui.scope_id().with("ap_track"), Sense::click_and_drag());
        if let Some(pos) = tr.interact_pointer_pos() {
            if tr.dragged() || tr.clicked() {
                let t = ((pos.x - track.min.x - knob_w / 2.0) / (track.width() - knob_w).max(1.0)).clamp(0.0, 1.0);
                let f = (t * (n - 1) as f32).round() as usize;
                if f != self.doc.cur {
                    self.doc.go_to(f);
                }
            }
        }
        tr.on_hover_text(format!("Frame {} of {}", self.doc.cur + 1, n));
        // Row 2: playback and frame buttons, then the light table and its layers.
        let r2 = Rect::from_min_max(Pos2::new(full.min.x + 2.0, r1.max.y + 3.0), Pos2::new(full.max.x - 2.0, full.max.y - 2.0));
        let left_count = 10.0;
        let right_count = 5.0;
        let gap = 8.0;
        let bw = (r2.width() - gap) / (left_count + right_count);
        let playing = self.playing;
        let mode = self.v5.play_mode;
        let dir = self.v5.play_dir;
        let buttons: [(&[&str], &str, Cmd, bool); 10] = [
            (IC_LOOP_BACK, "Play backwards (loop)", Cmd::PlayBackLoop, playing && mode == crate::actions::PlayMode::Loop && dir < 0),
            (IC_ONCE_BACK, "Play backwards once", Cmd::PlayBackOnce, playing && mode == crate::actions::PlayMode::Once && dir < 0),
            (IC_PREV, "Previous frame (1)", Cmd::PrevFrame, false),
            (IC_NEXT, "Next frame (2)", Cmd::NextFrame, false),
            (IC_ONCE, "Play once (5)", Cmd::PlayOnce, playing && mode == crate::actions::PlayMode::Once && dir > 0),
            (IC_LOOP, "Play (4)", Cmd::Play, playing && mode == crate::actions::PlayMode::Loop && dir > 0),
            (IC_PINGPONG, "Ping-pong (6)", Cmd::PingPong, playing && mode == crate::actions::PlayMode::PingPong),
            (&[], "+", Cmd::AddFrame, false),
            (&[], "-", Cmd::DeleteFrame, false),
            (IC_GOTO, "Go to frame (3)", Cmd::GoTo, false),
        ];
        for (i, (icon, tip, cmd, lit)) in buttons.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(r2.min.x + i as f32 * bw, r2.min.y), Vec2::new(bw - 2.0, r2.height()));
            let ic = if icon.is_empty() { None } else { Some(*icon) };
            let tip = match cmd {
                Cmd::AddFrame => "Add a frame after this one",
                Cmd::DeleteFrame => "Delete this frame",
                _ => tip,
            };
            let label = match cmd {
                Cmd::AddFrame => "+",
                Cmd::DeleteFrame => "-",
                _ => "",
            };
            if gadget(ui, r, ui.scope_id().with(("ap", i)), ic, label, *lit).on_hover_text(tip).clicked() {
                if *lit {
                    self.playing = false;
                } else {
                    self.cmd(&ctx, *cmd);
                }
            }
        }
        let rx = r2.min.x + left_count * bw + gap;
        let bulb = Rect::from_min_size(Pos2::new(rx, r2.min.y), Vec2::new(bw - 2.0, r2.height()));
        if gadget(ui, bulb, ui.scope_id().with("ap_lt"), Some(IC_BULB), "", self.light_table).on_hover_text("Light table on/off (|)").clicked() {
            self.light_table = !self.light_table;
        }
        for k in 0..4 {
            let r = Rect::from_min_size(Pos2::new(rx + (k + 1) as f32 * bw, r2.min.y), Vec2::new(bw - 2.0, r2.height()));
            let on = self.v5.lt_layers[k].on;
            let tip = format!("Light table layer {} (frame {:+})", k + 1, self.v5.lt_layers[k].offset);
            if gadget(ui, r, ui.scope_id().with(("ap_l", k)), None, &(k + 1).to_string(), on).on_hover_text(tip).clicked() {
                self.v5.lt_layers[k].on = !on;
                self.light_table = true;
            }
        }
    }

    /// The Range requester along the bottom of the screen.
    pub fn range_panel(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(full, 0.0, GREY);
        bevel(&p, full, 2.0, false);
        while self.doc.ranges.len() < 8 {
            let mut r = Range::new(0, 0, 0);
            r.active = false;
            self.doc.ranges.push(r);
        }
        let ri = self.set.range.min(7);
        let font = egui::FontId::proportional(13.0);
        let x0 = full.min.x + 8.0;
        let row = |k: f32| full.min.y + 6.0 + k * 18.0;
        // RANGE, RATE and FADE sliders on the left.
        let slider = |ui: &mut egui::Ui, label: &str, y: f32, value: f32, max: f32, id: &str| -> Option<f32> {
            p.text(Pos2::new(x0 + 54.0, y + 7.0), egui::Align2::RIGHT_CENTER, label, font.clone(), BLACK);
            let track = Rect::from_min_size(Pos2::new(x0 + 60.0, y), Vec2::new(150.0, 14.0));
            p.rect_filled(track, 0.0, BLACK);
            let kx = track.min.x + (track.width() - 18.0) * (value / max).clamp(0.0, 1.0);
            let knob = Rect::from_min_size(Pos2::new(kx, track.min.y + 2.0), Vec2::new(18.0, 10.0));
            p.rect_filled(knob, 0.0, GREY);
            p.text(Pos2::new(track.max.x + 6.0, y + 7.0), egui::Align2::LEFT_CENTER, format!("{:03}", value.round() as i32), font.clone(), BLACK);
            let r = ui.interact(track, ui.scope_id().with(id), Sense::click_and_drag());
            r.interact_pointer_pos().filter(|_| r.dragged() || r.clicked()).map(|pos| ((pos.x - track.min.x - 9.0) / (track.width() - 18.0)).clamp(0.0, 1.0) * max)
        };
        if let Some(v) = slider(ui, "RANGE:", row(0.0), (ri + 1) as f32, 8.0, "rp_range") {
            self.set.range = (v.round() as usize).clamp(1, 8) - 1;
        }
        let ri = self.set.range.min(7);
        // RATE: 0..63 on a curve, as the original's numbers are not evenly spaced.
        let rate63 = (self.doc.ranges[ri].rate as f32 / 16384.0).sqrt() * 63.0;
        if let Some(v) = slider(ui, "RATE:", row(1.0), rate63, 63.0, "rp_rate") {
            self.v5.ranges_undo.get_or_insert_with(|| self.doc.ranges.clone());
            let r = &mut self.doc.ranges[ri];
            r.rate = (((v.round() / 63.0).powi(2)) * 16384.0).round() as u16;
            r.active = r.rate > 0;
        }
        if let Some(v) = slider(ui, "FADE:", row(2.0), self.v5.range_fade[ri] as f32, 25.0, "rp_fade") {
            self.v5.range_fade[ri] = v.round() as u8;
        }
        self.set.range_fade = self.v5.range_fade[ri];
        // DITHER choice and amount.
        p.text(Pos2::new(x0, row(3.0) + 8.0), egui::Align2::LEFT_CENTER, "DITHER:", font.clone(), BLACK);
        let db = Rect::from_min_size(Pos2::new(x0 + 64.0, row(3.0)), Vec2::new(80.0, 16.0));
        if gadget(ui, db, ui.scope_id().with("rp_dither"), None, &self.set.dither.label().to_uppercase(), false).on_hover_text("Random, Smooth or Pattern").clicked() {
            self.set.dither = self.set.dither.next();
        }
        let dt = Rect::from_min_size(Pos2::new(x0, row(4.0) + 2.0), Vec2::new(210.0, 12.0));
        p.rect_filled(dt, 0.0, BLACK);
        let kx = dt.min.x + (dt.width() - 18.0) * self.set.spatter as f32 / 100.0;
        p.rect_filled(Rect::from_min_size(Pos2::new(kx, dt.min.y + 1.0), Vec2::new(18.0, 10.0)), 0.0, GREY);
        let dr = ui.interact(dt, ui.scope_id().with("rp_dither_amt"), Sense::click_and_drag());
        if let Some(pos) = dr.interact_pointer_pos().filter(|_| dr.dragged() || dr.clicked()) {
            self.set.spatter = (((pos.x - dt.min.x - 9.0) / (dt.width() - 18.0)).clamp(0.0, 1.0) * 100.0) as u8;
            if self.set.dither == Dither::Smooth {
                self.set.dither = Dither::Random;
            }
        }
        // OK / REVERT / UNDO and GRID.
        let yb = full.max.y - 22.0;
        let mut bx = x0;
        for (label, id) in [("OK", 0), ("REVERT", 1), ("UNDO", 2)] {
            let w = if id == 0 { 34.0 } else { 70.0 };
            let r = Rect::from_min_size(Pos2::new(bx, yb), Vec2::new(w, 18.0));
            if gadget(ui, r, ui.scope_id().with(("rp_btn", id)), None, label, false).clicked() {
                match id {
                    0 => {
                        self.v5.show_ranges = false;
                        self.v5.ranges_backup = None;
                    }
                    1 => {
                        if let Some(b) = self.v5.ranges_backup.clone() {
                            self.doc.ranges = b;
                        }
                    }
                    _ => {
                        if let Some(u) = self.v5.ranges_undo.take() {
                            self.doc.ranges = u;
                        }
                    }
                }
            }
            bx += w + 2.0;
        }
        let gb = Rect::from_min_size(Pos2::new(bx + 10.0, yb), Vec2::new(18.0, 18.0));
        if gadget(ui, gb, ui.scope_id().with("rp_grid"), None, if self.v5.range_grid { "✓" } else { "" }, false).clicked() {
            self.v5.range_grid = !self.v5.range_grid;
        }
        p.text(Pos2::new(gb.max.x + 6.0, gb.center().y), egui::Align2::LEFT_CENTER, "GRID", font.clone(), BLACK);
        // SHOW / SELECT / CLEAR / REV.
        let mx = x0 + 260.0;
        let actions = ["SHOW", "SELECT", "CLEAR", "REV"];
        for (k, a) in actions.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(mx, row(k as f32 + if k == 3 { 0.4 } else { 0.0 })), Vec2::new(78.0, 16.0));
            let lit = (k == 1 && self.v5.range_select_from.is_some()) || (k == 3 && self.doc.ranges[ri].reverse);
            if gadget(ui, r, ui.scope_id().with(("rp_act", k)), None, a, lit).clicked() {
                self.v5.ranges_undo = Some(self.doc.ranges.clone());
                match k {
                    0 => {
                        self.cycling = !self.cycling;
                    }
                    1 => {
                        self.v5.range_select_from = if self.v5.range_select_from.is_some() { None } else { Some(self.fg) };
                        self.status = "SELECT: now click the last colour of the range".into();
                    }
                    2 => {
                        let r = &mut self.doc.ranges[ri];
                        r.active = false;
                        r.rate = 0;
                        r.low = 0;
                        r.high = 0;
                        r.cells.clear();
                    }
                    _ => {
                        let r = &mut self.doc.ranges[ri];
                        r.reverse = !r.reverse;
                    }
                }
            }
        }
        // Palette strip: two rows of 16 colours, with page arrows.
        let px0 = mx + 92.0;
        let pal = self.display_palette();
        let avail_w = full.max.x - px0 - 90.0;
        let cw = (avail_w / 16.0).clamp(10.0, 40.0);
        // range_page is the first row of 16 shown (two rows at a time, of 16).
        self.v5.range_page = self.v5.range_page.min(14);
        let base = self.v5.range_page * 16;
        let strip = Rect::from_min_size(Pos2::new(px0, full.min.y + 6.0), Vec2::new(cw * 16.0, 30.0));
        for k in 0..32 {
            let i = base + k;
            let r = Rect::from_min_size(strip.min + Vec2::new((k % 16) as f32 * cw, (k / 16) as f32 * 15.0), Vec2::new(cw, 15.0));
            p.rect_filled(r, 0.0, to_color32(pal[i]));
            if self.v5.range_grid {
                p.rect_stroke(r, 0.0, Stroke::new(1.0, WHITE), egui::StrokeKind::Inside);
            }
            if i == self.fg as usize {
                p.rect_stroke(r.shrink(1.0), 0.0, Stroke::new(2.0, WHITE), egui::StrokeKind::Inside);
            }
        }
        let sr = ui.interact(strip, ui.scope_id().with("rp_strip"), Sense::click_and_drag());
        if let Some(pos) = sr.interact_pointer_pos().filter(|_| sr.clicked()) {
            let v = pos - strip.min;
            let k = (v.y / 15.0) as usize * 16 + (v.x / cw) as usize;
            let c = (base + k.min(31)) as u8;
            if let Some(from) = self.v5.range_select_from.take() {
                self.v5.ranges_undo = Some(self.doc.ranges.clone());
                let r = &mut self.doc.ranges[ri];
                // SELECT: the colours side by side from the first to the second.
                r.cells.clear();
                r.low = from.min(c);
                r.high = from.max(c);
                if r.rate == 0 {
                    r.rate = 2560;
                }
                r.active = true;
                self.status.clear();
            }
            self.fg = c;
        }
        let up = Rect::from_min_size(Pos2::new(strip.max.x + 2.0, strip.min.y), Vec2::new(16.0, 14.0));
        let down = Rect::from_min_size(Pos2::new(strip.max.x + 2.0, strip.min.y + 16.0), Vec2::new(16.0, 14.0));
        // One row of colours at a time (Shift: the first / last rows).
        let shift = ui.input(|i| i.modifiers.shift);
        if gadget(ui, up, ui.scope_id().with("rp_up"), None, "↑", false).on_hover_text("Colours one row up (Shift: the first row)").clicked() {
            self.v5.range_page = if shift { 0 } else { self.v5.range_page.saturating_sub(1) };
        }
        if gadget(ui, down, ui.scope_id().with("rp_down"), None, "↓", false).on_hover_text("Colours one row down (Shift: the last row)").clicked() {
            self.v5.range_page = if shift { 14 } else { (self.v5.range_page + 1).min(14) };
        }
        // Sample square with the current colour.
        let sample = Rect::from_min_size(Pos2::new(full.max.x - 64.0, full.min.y + 6.0), Vec2::new(56.0, 30.0));
        p.rect_filled(sample, 0.0, BLACK);
        p.rect_filled(sample.shrink(5.0), 0.0, to_color32(pal[self.fg as usize]));
        p.text(Pos2::new(sample.center().x, sample.max.y + 7.0), egui::Align2::CENTER_CENTER, format!("{}", self.fg), egui::FontId::proportional(11.0), BLACK);
        // ---- the range bar: 64 positions at a time (of 256), each may hold a colour
        let bar = Rect::from_min_size(Pos2::new(px0, strip.max.y + 6.0), Vec2::new(cw * 16.0, 18.0));
        let slots = 64usize;
        let sw = bar.width() / slots as f32;
        let off = self.v5.x.range_bar_off as usize;
        let r = self.doc.ranges[ri].clone();
        let cells = r.bar();
        for k in 0..slots {
            let pos = off + k;
            let cell = Rect::from_min_size(Pos2::new(bar.min.x + k as f32 * sw, bar.min.y), Vec2::new(sw, bar.height()));
            match cells.iter().find(|c| c.0 as usize == pos) {
                Some(&(_, ci)) => {
                    p.rect_filled(cell.shrink2(Vec2::new(0.5, 1.0)), 0.0, to_color32(pal[ci as usize]));
                    p.rect_stroke(cell.shrink2(Vec2::new(0.5, 1.0)), 0.0, Stroke::new(1.0, BLACK), egui::StrokeKind::Inside);
                }
                None => {
                    // An empty position: a notch on the line.
                    // An empty position: a thick dot on the line, easy to aim at.
                    p.line_segment([Pos2::new(cell.min.x, cell.center().y), Pos2::new(cell.max.x, cell.center().y)], Stroke::new(1.5, BLACK));
                    p.circle_filled(cell.center(), (sw * 0.32).clamp(2.5, 4.0), BLACK);
                }
            }
        }
        let br = ui.interact(bar, ui.scope_id().with("rp_bar"), Sense::click_and_drag());
        let slot_at = |pos: Pos2| -> Option<u8> {
            bar.contains(pos).then(|| (off + ((pos.x - bar.min.x) / sw).floor().clamp(0.0, slots as f32 - 1.0) as usize).min(255) as u8)
        };
        // Dragging: a colour from the palette rows onto the bar, a colour along the bar, or off it.
        if sr.drag_started() {
            if let Some(pos) = sr.interact_pointer_pos() {
                let v = pos - strip.min;
                let k = (v.y / 15.0) as usize * 16 + (v.x / cw) as usize;
                self.v5.x.range_drag = Some(((base + k.min(31)) as u8, None));
            }
        }
        if br.drag_started() {
            if let Some(slot) = br.interact_pointer_pos().and_then(slot_at) {
                if let Some(&(_, c)) = cells.iter().find(|c| c.0 == slot) {
                    self.v5.x.range_drag = Some((c, Some(slot)));
                }
            }
        }
        let place = |app: &mut App, from: Option<u8>, to: Option<(u8, u8)>| {
            app.v5.ranges_undo = Some(app.doc.ranges.clone());
            let r = &mut app.doc.ranges[ri];
            let mut cells = r.bar();
            if let Some(f) = from {
                cells.retain(|c| c.0 != f);
            }
            if let Some((slot, c)) = to {
                cells.retain(|x| x.0 != slot);
                cells.push((slot, c));
            }
            if cells.is_empty() {
                r.cells.clear();
                r.low = 0;
                r.high = 0;
                r.active = false;
            } else {
                r.set_cells(cells);
                if r.rate == 0 && !r.active {
                    r.active = true;
                }
            }
        };
        if let Some((c, from)) = self.v5.x.range_drag {
            let pointer = ui.input(|i| i.pointer.latest_pos());
            if let Some(pp) = pointer {
                // The colour being dragged follows the pointer.
                let sq = Rect::from_center_size(pp + Vec2::new(10.0, 10.0), Vec2::splat(12.0));
                ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, ui.scope_id().with("rp_drag"))).rect_filled(sq, 0.0, to_color32(pal[c as usize]));
            }
            if ui.input(|i| i.pointer.any_released()) {
                match pointer.and_then(slot_at) {
                    Some(slot) => place(self, from, Some((slot, c))),
                    // Dragged off the bar: taken out of the range.
                    None => {
                        if from.is_some() {
                            place(self, from, None);
                        }
                    }
                }
                self.v5.x.range_drag = None;
            }
        } else if br.clicked() {
            // A click on the bar puts the chosen (foreground) colour there.
            if let Some(slot) = br.interact_pointer_pos().and_then(slot_at) {
                place(self, None, Some((slot, self.fg)));
            }
        }
        br.on_hover_text("Drag colours here from the rows above (or click to put the chosen colour); drag them off to take them out");
        // ---- the scroller under the bar: which 64 of the 256 positions are shown
        let sc = Rect::from_min_size(Pos2::new(px0, bar.max.y + 4.0), Vec2::new(cw * 16.0, 10.0));
        p.rect_filled(sc, 0.0, BLACK);
        let kw = sc.width() * slots as f32 / 256.0;
        let kx = sc.min.x + (sc.width() - kw) * off as f32 / (256 - slots) as f32;
        p.rect_filled(Rect::from_min_size(Pos2::new(kx, sc.min.y + 1.0), Vec2::new(kw, sc.height() - 2.0)), 0.0, GREY);
        let scr = ui.interact(sc, ui.scope_id().with("rp_scroll"), Sense::click_and_drag());
        if let Some(pos) = scr.interact_pointer_pos().filter(|_| scr.dragged() || scr.clicked()) {
            let t = ((pos.x - sc.min.x - kw / 2.0) / (sc.width() - kw)).clamp(0.0, 1.0);
            self.v5.x.range_bar_off = (t * (256 - slots) as f32).round() as u8;
        }
        // ---- the gradient the range gives with the current dither
        let band = Rect::from_min_size(Pos2::new(px0, sc.max.y + 4.0), Vec2::new(cw * 16.0, 14.0));
        p.rect_filled(band, 0.0, Color32::from_gray(140));
        let r = self.doc.ranges[ri].clone();
        let seq = r.sequence(&self.doc.palette);
        if !r.bar().is_empty() {
            let mut rng = crate::paint::Rng::new(5);
            let steps = (band.width() as i32 / 2).max(2);
            // Where the range's colours end on the bar, the gradient ends too.
            let span = r.bar().last().map(|c| c.0 as f32 + 1.0).unwrap_or(1.0);
            let used = (span / 256.0 * 4.0).clamp(0.05, 1.0);
            for k in 0..steps {
                let t = k as f32 / steps as f32 / used;
                if t >= 1.0 {
                    break;
                }
                for yy in 0..7 {
                    let c = crate::paint::gradient_color(&seq, t, &self.set, k, yy, &mut rng);
                    let cell = Rect::from_min_size(Pos2::new(band.min.x + k as f32 * 2.0, band.min.y + yy as f32 * 2.0), Vec2::splat(2.0));
                    p.rect_filled(cell, 0.0, to_color32(pal[c as usize]));
                }
            }
        }
        bevel(&p, band, 1.0, true);
        // ---- the colour picker: takes the chosen colour from the picture
        let dropper = Rect::from_min_size(Pos2::new(full.max.x - 46.0, full.max.y - 40.0), Vec2::new(34.0, 32.0));
        let lit = self.tool == crate::app::Tool::Eyedropper;
        let dr = ui.interact(dropper, ui.scope_id().with("rp_dropper"), Sense::click());
        p.rect_filled(dropper, 0.0, if lit { TEAL } else { GREY });
        bevel(&p, dropper, 1.5, lit);
        crate::ui_style::pipette_icon(&p, dropper.shrink(3.0), if lit { WHITE } else { BLACK }, to_color32(pal[self.fg as usize]));
        if dr.on_hover_text("Colour picker: click a colour in the picture; then click the bar to put it there. Click again to switch it off").clicked() {
            self.toggle_picker();
        }
    }
}
