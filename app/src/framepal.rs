//! Numbered frame palettes, as in the person's Amiga screenshots and the manual (fig. 8.35, 8.36):
//! Set Frame Palette (Palette Number with its colours; Current Frame / Range a to b / All Frames;
//! Make Palette; Ok / Cancel) and Frame Palette Info (Palette # with its colours, Free, the frames
//! using the palette, Ok).
//!
//! A frame's palette is the last palette set at or before it (a palette change in an animation
//! lasts until the next one); the distinct palettes are numbered from 1 in the order the frames
//! first use them.

use crate::app::App;
use crate::doc::Document;
use crate::palette::{to_color32, Palette};
use crate::ui_style::button;
use egui::{Color32, RichText, Vec2};

/// Each frame's palette, with frame 1's taken from `base` when it has none of its own.
pub fn effective(doc: &Document, base: &Palette) -> Vec<Palette> {
    let mut cur = *base;
    (0..doc.frames.len())
        .map(|i| {
            if let Some(Some(p)) = doc.frame_palettes.get(i) {
                cur = *p;
            }
            cur
        })
        .collect()
}

/// The numbered palettes (in order of first use) and each frame's palette number (from 0).
pub fn numbered(eff: &[Palette]) -> (Vec<Palette>, Vec<usize>) {
    let mut list: Vec<Palette> = Vec::new();
    let nums = eff
        .iter()
        .map(|p| match list.iter().position(|q| q == p) {
            Some(i) => i,
            None => {
                list.push(*p);
                list.len() - 1
            }
        })
        .collect();
    (list, nums)
}

/// Frame numbers as ranges, "1 - 20", "23".
pub fn ranges_text(frames: &[usize]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < frames.len() {
        let mut j = i;
        while j + 1 < frames.len() && frames[j + 1] == frames[j] + 1 {
            j += 1;
        }
        out.push(if i == j { format!("{}", frames[i] + 1) } else { format!("{} - {}", frames[i] + 1, frames[j] + 1) });
        i = j + 1;
    }
    out
}

/// Store the effective palettes back as palette changes (only where a frame's palette differs
/// from the one before it).
pub fn store(doc: &mut Document, eff: &[Palette]) {
    doc.sync_palettes();
    for i in 0..eff.len() {
        doc.frame_palettes[i] = if i == 0 || eff[i] != eff[i - 1] { Some(eff[i]) } else { None };
    }
    doc.dirty = true;
}

#[derive(Default)]
pub struct FramePalState {
    pub show_set: bool,
    pub show_info: bool,
    pub number: usize,
    pub scope: u8,
    pub ab: (usize, usize),
    pub make: bool,
}

fn req(title: &str) -> egui::Window<'static> {
    egui::Window::new(RichText::new(title.to_string()).color(Color32::WHITE)).collapsible(false).resizable(false)
}

/// The palette as a row of colours.
fn palette_bar(ui: &mut egui::Ui, pal: &Palette, n: usize, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, Color32::BLACK);
    let inner = rect.shrink(2.0);
    let cw = inner.width() / n as f32;
    for k in 0..n {
        let r = egui::Rect::from_min_size(inner.min + Vec2::new(k as f32 * cw, 0.0), Vec2::new(cw + 0.5, inner.height()));
        p.rect_filled(r, 0.0, to_color32(pal[k]));
    }
    crate::ui_style::bevel(p, rect, 1.5, true);
}

impl App {
    /// The palette frame 1 starts with: the current palette when no palette change comes before
    /// the current frame, else the palette as last loaded or saved.
    fn base_palette(&self) -> Palette {
        let before = (0..=self.doc.cur).any(|i| matches!(self.doc.frame_palettes.get(i), Some(Some(_))));
        if before {
            self.doc.frame_palettes.iter().flatten().next().copied().unwrap_or(self.doc.saved_palette)
        } else {
            self.doc.palette
        }
    }

    pub fn open_frame_palette(&mut self, info: bool) {
        self.doc.sync_palettes();
        let base = self.base_palette();
        let eff = effective(&self.doc, &base);
        let (_, nums) = numbered(&eff);
        let n = self.doc.frames.len();
        let s = &mut self.v5.x.fpal;
        s.number = nums.get(self.doc.cur).copied().unwrap_or(0) + 1;
        s.scope = 0;
        s.ab = ((self.doc.cur + 2).min(n), n);
        s.make = false;
        if info {
            s.show_info = true;
        } else {
            s.show_set = true;
        }
    }

    pub fn frame_palette_reqs(&mut self, ctx: &egui::Context) {
        if !self.v5.x.fpal.show_set && !self.v5.x.fpal.show_info {
            return;
        }
        self.doc.sync_palettes();
        let base = self.base_palette();
        let mut eff = effective(&self.doc, &base);
        // The palette on screen is the current frame's (it may have been edited in the Mixer).
        let cur = self.doc.cur;
        eff[cur] = self.doc.palette;
        let (list, nums) = numbered(&eff);
        let count = list.len();
        let colors = self.doc.colors.clamp(2, 256);
        let n = self.doc.frames.len();

        if self.v5.x.fpal.show_set {
            let mut done = None;
            req("Set Frame Palette").show(ctx, |ui| {
                ui.set_width(300.0);
                let s = &mut self.v5.x.fpal;
                ui.horizontal(|ui| {
                    ui.label("Palette Number:");
                    ui.add_enabled(!s.make, egui::DragValue::new(&mut s.number).range(1..=count));
                    let pal = list[(s.number.clamp(1, count)) - 1];
                    palette_bar(ui, &pal, colors, 120.0);
                });
                ui.separator();
                if ui.radio(s.scope == 0, "Current Frame").clicked() {
                    s.scope = 0;
                }
                ui.horizontal(|ui| {
                    if ui.radio(s.scope == 1, "Range").clicked() {
                        s.scope = 1;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(egui::DragValue::new(&mut s.ab.1).range(1..=n));
                        ui.label("to");
                        ui.add(egui::DragValue::new(&mut s.ab.0).range(1..=n));
                    });
                });
                if ui.radio(s.scope == 2, "All Frames").clicked() {
                    s.scope = 2;
                }
                ui.separator();
                ui.vertical_centered(|ui| {
                    ui.checkbox(&mut s.make, "Make Palette").on_hover_text("A new palette (a copy of the current one) for these frames, to change on its own");
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if button(ui, "Ok").clicked() {
                        done = Some(true);
                    }
                    ui.add_space(190.0);
                    if button(ui, "Cancel").clicked() {
                        done = Some(false);
                    }
                });
            });
            if let Some(ok) = done {
                if ok {
                    let s = &self.v5.x.fpal;
                    let frames: Vec<usize> = match s.scope {
                        0 => vec![cur],
                        1 => (s.ab.0.min(s.ab.1).clamp(1, n) - 1..=s.ab.0.max(s.ab.1).clamp(1, n) - 1).collect(),
                        _ => (0..n).collect(),
                    };
                    let pal = if s.make { self.doc.palette } else { list[s.number.clamp(1, count) - 1] };
                    // A new palette must differ from the others to get a number of its own: it does as
                    // soon as it is changed; until then it shares the number of the same colours.
                    let mut new_eff = eff.clone();
                    // Frames after the range keep the palette they had.
                    let last = *frames.last().unwrap();
                    for &i in &frames {
                        new_eff[i] = pal;
                    }
                    let _ = last;
                    store(&mut self.doc, &new_eff);
                    self.doc.go_to(self.doc.cur);
                    self.status = format!("Palette set for {} frame(s)", frames.len());
                }
                self.v5.x.fpal.show_set = false;
            }
        }

        if self.v5.x.fpal.show_info {
            let mut done = false;
            let mut free = false;
            req("Frame Palette Info").show(ctx, |ui| {
                ui.set_width(240.0);
                let s = &mut self.v5.x.fpal;
                s.number = s.number.clamp(1, count);
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(60.0);
                        ui.label("Palette #:");
                        ui.add(egui::DragValue::new(&mut s.number).range(1..=count));
                    });
                    palette_bar(ui, &list[s.number - 1], colors, 220.0);
                });
                ui.separator();
                ui.vertical_centered(|ui| {
                    if ui.add_enabled(count > 1, egui::Button::new("Free")).on_hover_text("Remove this palette: its frames take the palette of the frame before them").on_disabled_hover_text("The only palette can't be freed").clicked() {
                        free = true;
                    }
                });
                ui.separator();
                ui.vertical_centered(|ui| ui.label("Frames Using\nCurrent Palette:"));
                let frames: Vec<usize> = (0..n).filter(|&i| nums[i] == s.number - 1).collect();
                egui::Frame::new().stroke(egui::Stroke::new(1.5, Color32::BLACK)).inner_margin(6).show(ui, |ui| {
                    ui.set_min_size(Vec2::new(226.0, 110.0));
                    egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                        for t in ranges_text(&frames) {
                            ui.monospace(t);
                        }
                    });
                });
                ui.vertical_centered(|ui| {
                    if button(ui, "Ok").clicked() {
                        done = true;
                    }
                });
            });
            if free {
                let k = self.v5.x.fpal.number - 1;
                let frames: Vec<usize> = (0..n).filter(|&i| nums[i] == k).collect();
                let first = frames[0];
                // The frame before the first one using it; for frame 1, the next other palette.
                let repl = if first > 0 { eff[first - 1] } else { eff.iter().copied().find(|p| *p != list[k]).unwrap_or(list[k]) };
                for i in frames {
                    eff[i] = repl;
                }
                store(&mut self.doc, &eff);
                self.doc.go_to(self.doc.cur);
                self.v5.x.fpal.number = 1;
            }
            if done {
                self.v5.x.fpal.show_info = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pix::Image;

    #[test]
    fn palettes_are_numbered_and_listed_as_ranges() {
        let mut d = Document::new(2, 1, 0);
        d.frames = (0..6).map(|_| Image::new(2, 1, 0)).collect();
        d.sync_palettes();
        let a = d.palette;
        let mut b = a;
        b[1] = [9, 9, 9];
        d.frame_palettes[3] = Some(b);
        d.frame_palettes[5] = Some(a);
        let eff = effective(&d, &a);
        let (list, nums) = numbered(&eff);
        assert_eq!(list.len(), 2);
        assert_eq!(nums, vec![0, 0, 0, 1, 1, 0]);
        let f0: Vec<usize> = (0..6).filter(|&i| nums[i] == 0).collect();
        assert_eq!(ranges_text(&f0), vec!["1 - 3", "6"]);
        store(&mut d, &eff);
        assert!(d.frame_palettes[0].is_some() && d.frame_palettes[1].is_none() && d.frame_palettes[3] == Some(b));
    }
}
