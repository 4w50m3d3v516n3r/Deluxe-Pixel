//! Colour Set > Load: after the file is chosen, a panel along the bottom (as on the person's Amiga
//! screenshots and in the manual, fig. 8.41) chooses which of its colours to take: "SELECTED: n",
//! SELECT ALL / NONE / BASE, the file's colours in rows of 16 (cells the file doesn't have are
//! hatched) with up/down arrows, the chosen colour with its number, and ADD / OVERWRITE / CANCEL
//! and GRID. While it is open the screen shows the file's colours.

use crate::app::App;
use crate::palette::{to_color32, Palette, Range};
use crate::ui_style::{bevel, BLACK, GREY, TEAL, WHITE};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

pub struct ColourSetChoice {
    pub colors: Vec<[u8; 3]>,
    pub ranges: Vec<Range>,
    pub selected: Vec<bool>,
    pub page: usize,
    pub chosen: usize,
    pub grid: bool,
}

impl ColourSetChoice {
    pub fn new(colors: Vec<[u8; 3]>, ranges: Vec<Range>) -> Self {
        let n = colors.len();
        ColourSetChoice { colors, ranges, selected: vec![false; n], page: 0, chosen: 0, grid: false }
    }

    /// The file's colours over the current palette (for showing while choosing).
    pub fn preview(&self, under: &Palette) -> Palette {
        let mut p = *under;
        for (i, c) in self.colors.iter().enumerate().take(256) {
            p[i] = *c;
        }
        p
    }
}

/// ADD: the selected colours go into the empty cells of the colour set (black cells after
/// colour 0), in order; returns how many found a place.
pub fn add_colours(pal: &mut Palette, colours: &[[u8; 3]]) -> usize {
    let free_cells: Vec<usize> = (1..256).filter(|&i| pal[i] == [0, 0, 0]).collect();
    let mut free = free_cells.into_iter();
    let mut n = 0;
    for c in colours {
        match free.next() {
            Some(i) => {
                pal[i] = *c;
                n += 1;
            }
            None => break,
        }
    }
    n
}

/// OVERWRITE: the selected colours replace the colour set from position 0 on.
pub fn overwrite_colours(pal: &mut Palette, colours: &[[u8; 3]]) {
    for (i, c) in colours.iter().enumerate().take(256) {
        pal[i] = *c;
    }
}

fn gadget(ui: &mut egui::Ui, rect: Rect, id: egui::Id, text: &str) -> egui::Response {
    let resp = ui.interact(rect, id, Sense::click());
    let down = resp.is_pointer_button_down_on();
    let p = ui.painter();
    p.rect_filled(rect, 0.0, if down { TEAL } else { GREY });
    bevel(p, rect, 1.5, down);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(13.0), if down { WHITE } else { BLACK });
    resp
}

impl App {
    pub fn colour_set_panel(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(full, 0.0, GREY);
        bevel(&p, full, 2.0, false);
        let Some(cs) = &mut self.v5.x.cset else { return };
        let font = egui::FontId::proportional(13.0);
        let x0 = full.min.x + 6.0;
        let nsel = cs.selected.iter().filter(|s| **s).count();
        p.text(Pos2::new(x0, full.min.y + 10.0), egui::Align2::LEFT_CENTER, format!("SELECTED: {nsel}"), font.clone(), BLACK);
        p.text(Pos2::new(x0, full.min.y + 28.0), egui::Align2::LEFT_CENTER, "SELECT", font.clone(), BLACK);
        let base_n = self.doc.colors.clamp(2, 256).min(cs.colors.len());
        let mut bx = x0 + 56.0;
        for (k, (label, w)) in [("ALL", 36.0), ("NONE", 46.0), ("BASE", 46.0)].iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(bx, full.min.y + 20.0), Vec2::new(*w, 16.0));
            let tip = ["Every colour of the file", "No colour", "Only the screen's colours (the first ones)"][k];
            if gadget(ui, r, ui.scope_id().with(("cs_sel", k)), label).on_hover_text(tip).clicked() {
                for (i, s) in cs.selected.iter_mut().enumerate() {
                    *s = match k {
                        0 => true,
                        1 => false,
                        _ => i < base_n,
                    };
                }
            }
            bx += w + 2.0;
        }
        // The first colour number of the rows shown.
        p.text(Pos2::new(x0 + 270.0, full.min.y + 10.0), egui::Align2::LEFT_CENTER, format!("{}", cs.page * 16), font.clone(), BLACK);
        // ---- the colours: rows of 16, hatched where the file has none
        let px0 = x0 + 290.0;
        let avail_w = full.max.x - px0 - 90.0;
        let cw = (avail_w / 16.0).clamp(10.0, 40.0);
        let rows = 7usize;
        let ch = (full.height() - 12.0) / rows as f32;
        let total_rows = 16usize;
        cs.page = cs.page.min(total_rows - rows);
        let grid = Rect::from_min_size(Pos2::new(px0, full.min.y + 6.0), Vec2::new(cw * 16.0, ch * rows as f32));
        for row in 0..rows {
            for col in 0..16 {
                let i = (cs.page + row) * 16 + col;
                let r = Rect::from_min_size(grid.min + Vec2::new(col as f32 * cw, row as f32 * ch), Vec2::new(cw, ch));
                if i < cs.colors.len() {
                    let c = cs.colors[i];
                    p.rect_filled(r.shrink(1.0), 0.0, Color32::from_rgb(c[0], c[1], c[2]));
                    if cs.selected[i] {
                        // Selected colours: a marker in the corner.
                        p.rect_filled(Rect::from_min_size(r.min + Vec2::new(1.0, 1.0), Vec2::splat(5.0)), 0.0, WHITE);
                        p.rect_stroke(Rect::from_min_size(r.min + Vec2::new(1.0, 1.0), Vec2::splat(5.0)), 0.0, Stroke::new(1.0, BLACK), egui::StrokeKind::Inside);
                    }
                    if cs.grid {
                        let gc = if i < self.doc.colors { WHITE } else { Color32::from_rgb(120, 160, 255) };
                        p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, gc));
                        p.line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0, gc));
                    }
                    if i == cs.chosen {
                        p.rect_stroke(r.shrink(1.0), 0.0, Stroke::new(2.0, WHITE), egui::StrokeKind::Inside);
                    }
                } else {
                    // A cell the file has no colour for: hatched.
                    let c = r.shrink(2.0);
                    p.rect_filled(c, 0.0, Color32::from_gray(110));
                    let mut y = c.min.y;
                    while y < c.max.y {
                        let mut x = c.min.x + if ((y - c.min.y) as i32 / 2) % 2 == 0 { 0.0 } else { 2.0 };
                        while x < c.max.x {
                            p.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(1.0, 1.0)), 0.0, BLACK);
                            x += 4.0;
                        }
                        y += 2.0;
                    }
                }
            }
        }
        let gr = ui.interact(grid, ui.scope_id().with("cs_grid"), Sense::click());
        if let Some(pos) = gr.interact_pointer_pos().filter(|_| gr.clicked()) {
            let v = pos - grid.min;
            let i = (cs.page + (v.y / ch) as usize) * 16 + (v.x / cw) as usize;
            if i < cs.colors.len() {
                // A click selects a colour, a second click deselects it.
                cs.selected[i] = !cs.selected[i];
                cs.chosen = i;
            }
        }
        let up = Rect::from_min_size(Pos2::new(grid.max.x + 2.0, grid.min.y), Vec2::new(18.0, grid.height() / 2.0 - 1.0));
        let down = Rect::from_min_size(Pos2::new(grid.max.x + 2.0, grid.center().y + 1.0), Vec2::new(18.0, grid.height() / 2.0 - 1.0));
        if gadget(ui, up, ui.scope_id().with("cs_up"), "↑").clicked() {
            cs.page = cs.page.saturating_sub(1);
        }
        if gadget(ui, down, ui.scope_id().with("cs_down"), "↓").clicked() {
            cs.page = (cs.page + 1).min(total_rows - rows);
        }
        // ---- the chosen colour and its number
        let sample = Rect::from_min_size(Pos2::new(full.max.x - 64.0, full.min.y + 6.0), Vec2::new(56.0, 30.0));
        p.rect_filled(sample, 0.0, BLACK);
        let sc = cs.colors.get(cs.chosen).copied().unwrap_or([0, 0, 0]);
        p.rect_filled(sample.shrink(5.0), 0.0, Color32::from_rgb(sc[0], sc[1], sc[2]));
        p.text(Pos2::new(sample.center().x, sample.max.y + 7.0), egui::Align2::CENTER_CENTER, format!("{}", cs.chosen), egui::FontId::proportional(11.0), BLACK);
        // ---- ADD / OVERWRITE / CANCEL and GRID
        let yb = full.max.y - 21.0;
        let mut bx = x0;
        let mut action = None;
        for (k, (label, w)) in [("ADD", 40.0), ("OVERWRITE", 96.0), ("CANCEL", 70.0)].iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(bx, yb), Vec2::new(*w, 18.0));
            let tip = ["The selected colours into the empty (black) cells of the colour set", "The selected colours over the colour set, from position 0", "Load no colours"][k];
            if gadget(ui, r, ui.scope_id().with(("cs_act", k)), label).on_hover_text(tip).clicked() {
                action = Some(k);
            }
            bx += w + 2.0;
        }
        let gb = Rect::from_min_size(Pos2::new(bx + 8.0, yb), Vec2::new(18.0, 18.0));
        if gadget(ui, gb, ui.scope_id().with("cs_gridbox"), if cs.grid { "✓" } else { "" }).clicked() {
            cs.grid = !cs.grid;
        }
        p.text(Pos2::new(gb.max.x + 6.0, gb.center().y), egui::Align2::LEFT_CENTER, "GRID", font, BLACK);
        if let Some(k) = action {
            let cs = self.v5.x.cset.take().unwrap();
            // Then Arrange opens, to put the new colours where they are wanted.
            if k < 2 {
                self.show_palette = true;
                self.v5.x.mixer.arrange = true;
                self.v5.x.mixer.backup = None;
            }
            if k < 2 {
                let chosen: Vec<[u8; 3]> = cs.colors.iter().zip(cs.selected.iter()).filter(|(_, s)| **s).map(|(c, _)| *c).collect();
                if chosen.is_empty() {
                    self.status = "No colours selected: nothing loaded".into();
                } else {
                    self.doc.checkpoint();
                    self.pal_undo = Some(self.doc.palette);
                    if k == 0 {
                        let n = add_colours(&mut self.doc.palette, &chosen);
                        self.status = if n < chosen.len() { format!("{n} of {} colours added: no more empty cells", chosen.len()) } else { format!("{n} colours added") };
                    } else {
                        overwrite_colours(&mut self.doc.palette, &chosen);
                        // Every colour taken: the file's ranges come too.
                        if chosen.len() == cs.colors.len() && !cs.ranges.is_empty() {
                            self.doc.ranges = cs.ranges.clone();
                            self.doc.fit_ranges();
                        }
                        self.status = format!("{} colours loaded", chosen.len());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::DEFAULT_PALETTE;

    #[test]
    fn add_fills_empty_cells_and_overwrite_starts_at_zero() {
        let mut pal = DEFAULT_PALETTE;
        pal[5] = [0, 0, 0];
        pal[9] = [0, 0, 0];
        let n = add_colours(&mut pal, &[[1, 2, 3], [4, 5, 6]]);
        assert_eq!(n, 2);
        assert_eq!((pal[5], pal[9]), ([1, 2, 3], [4, 5, 6]));
        assert_eq!(pal[0], [0, 0, 0], "colour 0 is not an empty cell");
        overwrite_colours(&mut pal, &[[7, 7, 7], [8, 8, 8]]);
        assert_eq!((pal[0], pal[1], pal[2]), ([7, 7, 7], [8, 8, 8], DEFAULT_PALETTE[2]));
        let cs = ColourSetChoice::new(vec![[200, 0, 0]; 16], Vec::new());
        assert_eq!(cs.preview(&DEFAULT_PALETTE)[15], [200, 0, 0]);
        assert_eq!(cs.preview(&DEFAULT_PALETTE)[16], DEFAULT_PALETTE[16]);
    }
}
