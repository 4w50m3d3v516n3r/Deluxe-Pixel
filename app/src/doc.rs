//! A picture document: one or more animation frames, palette, colour ranges, stencil and undo.

use crate::palette::{default_ranges, Palette, Range, DEFAULT_PALETTE};
use crate::pix::Image;
use std::path::PathBuf;

pub const UNDO_LIMIT: usize = 40;

#[derive(Clone)]
pub struct Snapshot {
    pub frame: usize,
    pub img: Image,
    pub palette: Palette,
}

#[derive(Clone)]
pub struct Stencil {
    /// true = protected (cannot be painted).
    pub mask: Vec<bool>,
    pub on: bool,
}

#[derive(Clone)]
pub struct Document {
    pub w: i32,
    pub h: i32,
    pub frames: Vec<Image>,
    pub cur: usize,
    pub palette: Palette,
    pub ranges: Vec<Range>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    pub stencil: Option<Stencil>,
    /// Fixed background ("Background Fix"): Clear restores this instead of a plain colour.
    pub background: Option<Image>,
    /// Frames per second for animation playback.
    pub fps: f32,
    /// Saved (last loaded/saved) state for Revert.
    pub saved_frames: Option<Vec<Image>>,
    /// Palette as last loaded or saved (Colour > Palette > Restore Palette).
    pub saved_palette: Palette,
    /// Palette size chosen in Screen Format (2..=256): pixels only use colours below it.
    pub colors: usize,
    /// Extra-Half-Brite: colours 32-63 are always colours 0-31 at half brightness.
    pub ehb: bool,
    /// Bits per colour gun the screen can show (4 on OCS/ECS Amiga screens, 8 otherwise).
    pub gun_bits: u32,
    /// Amiga display mode id written to saved IFF files (0 = not an Amiga mode).
    pub mode_id: u32,
    /// Optional palette per animation frame (Anim > Frames > Set Frame Palette).
    pub frame_palettes: Vec<Option<Palette>>,
    /// Display time of each frame in 1/60 s (0 = use the global rate): Set Frame Rate > Vary By Frame.
    pub frame_delays: Vec<u32>,
    /// Frames played by Play (Anim > Control > Set Range), inclusive, 0-based.
    pub play_range: Option<(usize, usize)>,
}

impl Document {
    pub fn new(w: i32, h: i32, bg: u8) -> Self {
        Document {
            w,
            h,
            frames: vec![Image::new(w, h, bg)],
            cur: 0,
            palette: DEFAULT_PALETTE,
            ranges: default_ranges(),
            path: None,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            stencil: None,
            background: None,
            fps: 12.0,
            saved_frames: None,
            saved_palette: DEFAULT_PALETTE,
            colors: 256,
            ehb: false,
            gun_bits: 8,
            mode_id: 0,
            frame_palettes: vec![None],
            frame_delays: vec![0],
            play_range: None,
        }
    }

    pub fn from_image(img: Image, palette: Palette) -> Self {
        let mut d = Document::new(img.w, img.h, 0);
        d.frames = vec![img];
        d.palette = palette;
        d.saved_palette = palette;
        d
    }

    pub fn name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string())
    }

    pub fn img(&self) -> &Image {
        &self.frames[self.cur]
    }
    pub fn img_mut(&mut self) -> &mut Image {
        &mut self.frames[self.cur]
    }

    /// Remember the current frame before a change.
    pub fn checkpoint(&mut self) {
        let snap = Snapshot { frame: self.cur, img: self.frames[self.cur].clone(), palette: self.palette };
        self.undo.push(snap);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    pub fn undo(&mut self) -> bool {
        if let Some(s) = self.undo.pop() {
            let f = s.frame.min(self.frames.len() - 1);
            let cur = Snapshot { frame: f, img: self.frames[f].clone(), palette: self.palette };
            self.redo.push(cur);
            if s.img.w == self.w && s.img.h == self.h {
                self.frames[f] = s.img;
            }
            self.palette = s.palette;
            self.cur = f;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(s) = self.redo.pop() {
            let f = s.frame.min(self.frames.len() - 1);
            let cur = Snapshot { frame: f, img: self.frames[f].clone(), palette: self.palette };
            self.undo.push(cur);
            if s.img.w == self.w && s.img.h == self.h {
                self.frames[f] = s.img;
            }
            self.palette = s.palette;
            self.cur = f;
            true
        } else {
            false
        }
    }

    /// Clear the current frame to colour `bg`, or to the fixed background.
    pub fn clear(&mut self, bg: u8) {
        self.checkpoint();
        if let Some(b) = &self.background {
            self.frames[self.cur] = b.clone();
        } else {
            self.frames[self.cur].fill(bg);
        }
    }

    pub fn protected(&self, x: i32, y: i32) -> bool {
        if let Some(s) = &self.stencil {
            if s.on && x >= 0 && y >= 0 && x < self.w && y < self.h {
                return s.mask[(y * self.w + x) as usize];
            }
        }
        false
    }

    /// Page Size: change the canvas of every frame.
    pub fn resize_page(&mut self, w: i32, h: i32, bg: u8, stretch: bool) {
        self.undo.clear();
        self.redo.clear();
        for f in self.frames.iter_mut() {
            *f = if stretch { f.scaled(w, h) } else { f.resized_canvas(w, h, bg) };
        }
        self.w = w;
        self.h = h;
        self.stencil = None;
        self.background = None;
        self.dirty = true;
    }

    // ---- Animation frames ----
    pub fn set_frame_count(&mut self, n: usize) {
        let n = n.clamp(1, 999);
        while self.frames.len() < n {
            let copy = self.frames[self.frames.len() - 1].clone();
            self.frames.push(copy);
        }
        self.frames.truncate(n);
        self.frame_palettes.resize(n, None);
        self.frame_delays.resize(n, 0);
        self.cur = self.cur.min(n - 1);
        self.dirty = true;
    }
    pub fn add_frame_after(&mut self) {
        let copy = self.frames[self.cur].clone();
        self.frames.insert(self.cur + 1, copy);
        self.sync_palettes();
        self.frame_palettes.insert(self.cur + 1, None);
        self.frame_delays.insert((self.cur + 1).min(self.frame_delays.len()), 0);
        self.cur += 1;
        self.dirty = true;
    }
    pub fn delete_frame(&mut self) -> bool {
        if self.frames.len() <= 1 {
            return false;
        }
        self.frames.remove(self.cur);
        self.sync_palettes();
        if self.cur < self.frame_palettes.len() {
            self.frame_palettes.remove(self.cur);
            if self.cur < self.frame_delays.len() {
                self.frame_delays.remove(self.cur);
            }
        }
        self.sync_palettes();
        self.cur = self.cur.min(self.frames.len() - 1);
        self.undo.clear();
        self.redo.clear();
        self.dirty = true;
        true
    }
    pub fn copy_to_all(&mut self) {
        let c = self.frames[self.cur].clone();
        for f in self.frames.iter_mut() {
            *f = c.clone();
        }
        self.dirty = true;
    }
    pub fn next_frame(&mut self) {
        let n = (self.cur + 1) % self.frames.len();
        self.go_to(n);
    }
    pub fn prev_frame(&mut self) {
        let n = (self.cur + self.frames.len() - 1) % self.frames.len();
        self.go_to(n);
    }
    /// Show frame `n`, switching to its own palette if it has one.
    pub fn go_to(&mut self, n: usize) {
        self.cur = n.min(self.frames.len() - 1);
        self.sync_palettes();
        if let Some(p) = self.frame_palettes[self.cur] {
            self.palette = p;
        }
    }
    pub fn sync_palettes(&mut self) {
        self.frame_palettes.resize(self.frames.len(), None);
        self.frame_delays.resize(self.frames.len(), 0);
    }
    /// Insert `n` frames after the current one (copies of it).
    pub fn add_frames(&mut self, n: usize) {
        let copy = self.frames[self.cur].clone();
        for _ in 0..n.min(999 - self.frames.len().min(999)) {
            self.frames.insert(self.cur + 1, copy.clone());
        }
        self.sync_palettes();
        self.dirty = true;
    }
    /// Copy frame `from` over frames `a..=b` (1-based numbers shown to the person are converted by the caller).
    pub fn copy_frames(&mut self, from: usize, a: usize, b: usize) {
        let src = self.frames[from.min(self.frames.len() - 1)].clone();
        for i in a..=b.min(self.frames.len() - 1) {
            self.frames[i] = src.clone();
        }
        self.dirty = true;
    }
    /// Delete frames a..=b (at least one frame always stays).
    pub fn delete_frames(&mut self, a: usize, b: usize) -> bool {
        let b = b.min(self.frames.len() - 1);
        if a > b || b - a + 1 >= self.frames.len() {
            return false;
        }
        self.sync_palettes();
        self.frames.drain(a..=b);
        self.frame_palettes.drain(a..=b);
        self.frame_delays.drain(a..=b);
        self.cur = self.cur.min(self.frames.len() - 1);
        self.undo.clear();
        self.redo.clear();
        self.dirty = true;
        true
    }
    /// Seconds frame `i` stays on screen.
    pub fn frame_seconds(&self, i: usize) -> f32 {
        match self.frame_delays.get(i).copied().unwrap_or(0) {
            0 => 1.0 / self.fps.max(0.5),
            j => j as f32 / 60.0,
        }
    }

    /// Copy frames a..=b and insert the copies before frame `before` (0-based; frames.len() = at the end).
    pub fn insert_copies(&mut self, a: usize, b: usize, before: usize) {
        self.sync_palettes();
        let b = b.min(self.frames.len() - 1);
        let copies: Vec<Image> = self.frames[a..=b].to_vec();
        let pals: Vec<Option<Palette>> = self.frame_palettes[a..=b].to_vec();
        let dels: Vec<u32> = self.frame_delays[a..=b].to_vec();
        let at = before.min(self.frames.len());
        for (k, f) in copies.into_iter().enumerate() {
            self.frames.insert(at + k, f);
            self.frame_palettes.insert(at + k, pals[k]);
            self.frame_delays.insert(at + k, dels[k]);
        }
        self.undo.clear();
        self.redo.clear();
        self.dirty = true;
    }

    /// Scroll frame `i`'s picture by (dx,dy), wrapping round (Camera Move).
    pub fn scroll_frame(&mut self, i: usize, dx: i32, dy: i32) {
        let f = &self.frames[i];
        let mut out = f.clone();
        for y in 0..f.h {
            for x in 0..f.w {
                out.px[(y * f.w + x) as usize] = f.get((x - dx).rem_euclid(f.w), (y - dy).rem_euclid(f.h));
            }
        }
        self.frames[i] = out;
    }

    /// Make the palette obey the screen format (gun precision, Extra-Half-Brite).
    pub fn limit_colors(&mut self) {
        let (n, e, g) = (self.colors, self.ehb, self.gun_bits);
        crate::amiga::limit_palette(&mut self.palette, n, e, g);
        for p in self.frame_palettes.iter_mut().flatten() {
            crate::amiga::limit_palette(p, n, e, g);
        }
    }

    /// Repaint pixels that use colours the screen doesn't have with the nearest colour it has.
    pub fn fit_pixels(&mut self) -> usize {
        let n = self.colors.clamp(2, 256);
        if n >= 256 {
            return 0;
        }
        let pal = self.palette;
        let map: Vec<u8> = (0..256).map(|i| if i < n { i as u8 } else { crate::palette::nearest(&pal, [pal[i][0] as i32, pal[i][1] as i32, pal[i][2] as i32], n) }).collect();
        let mut changed = 0;
        for f in self.frames.iter_mut() {
            for p in f.px.iter_mut() {
                if *p as usize >= n {
                    *p = map[*p as usize];
                    changed += 1;
                }
            }
        }
        self.fit_ranges();
        if let Some(b) = &mut self.background {
            for p in b.px.iter_mut() {
                *p = map[*p as usize];
            }
        }
        if changed > 0 {
            self.dirty = true;
        }
        changed
    }

    /// Keep the colour ranges that fit the palette size; when fewer than 5 are left, add the
    /// ramps found in the palette (or the stock ranges for this size).
    pub fn fit_ranges(&mut self) {
        let n = self.colors.clamp(2, 256);
        self.ranges.retain(|r| (r.high as usize) < n);
        if self.ranges.len() >= 5 {
            return;
        }
        let extra = if crate::palette::is_stock(&self.palette, n) { crate::palette::ranges_for(n) } else { crate::palette::find_ramps(&self.palette, n, 8) };
        for r in extra {
            if self.ranges.len() >= 8 {
                break;
            }
            if !self.ranges.iter().any(|e| e.low <= r.high && r.low <= e.high) {
                self.ranges.push(r);
            }
        }
        self.ranges.sort_by_key(|r| r.low);
    }

    /// A new palette size: a stock palette is swapped for the one made for this size (the
    /// picture is matched to it) and gets that size's ranges; an own palette keeps its colours.
    pub fn adopt_palette_size(&mut self, old_colors: usize) {
        let n = self.colors.clamp(2, 256);
        if n != old_colors && crate::palette::is_stock(&self.palette, old_colors.min(n)) {
            let old = self.palette;
            let new = crate::palette::palette_for(n);
            let map: Vec<u8> = (0..256).map(|i| crate::palette::nearest(&new, [old[i][0] as i32, old[i][1] as i32, old[i][2] as i32], n)).collect();
            for f in self.frames.iter_mut() {
                for p in f.px.iter_mut() {
                    *p = map[*p as usize];
                }
            }
            self.palette = new;
            self.saved_palette = new;
            self.ranges = crate::palette::ranges_for(n);
        }
        self.fit_ranges();
    }

    /// How IFF files of this picture are written (bitplanes and display mode).
    pub fn iff_format(&self) -> crate::fileio::IffFormat {
        crate::fileio::IffFormat { planes: crate::amiga::depth_of(self.colors), camg: self.mode_id }
    }

    /// Colour-cycle the palette: rotate each active range by `steps[i]` positions.
    pub fn cycled_palette(&self, phases: &[f32]) -> Palette {
        let mut pal = self.palette;
        for (i, r) in self.ranges.iter().enumerate() {
            let regs = r.registers();
            if !r.active || r.rate == 0 || regs.len() < 2 {
                continue;
            }
            // The colours move along the range's colours, in bar order.
            let n = regs.len();
            let shift = phases.get(i).copied().unwrap_or(0.0).floor() as i64;
            let shift = shift.rem_euclid(n as i64) as usize;
            for k in 0..n {
                let src = regs[k] as usize;
                let dst = if r.reverse { regs[(k + n - shift) % n] } else { regs[(k + shift) % n] } as usize;
                pal[dst] = self.palette[src];
            }
        }
        pal
    }
}
