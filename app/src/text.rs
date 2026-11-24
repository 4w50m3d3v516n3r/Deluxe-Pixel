//! Text tool: renders typed text with a system font into a brush.

use crate::palette::{nearest, Palette};
use crate::pix::{Brush, Image};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::path::PathBuf;

/// Font sizes offered by the original Font menu.
pub const FONT_SIZES: [u32; 9] = [8, 12, 18, 24, 36, 48, 56, 72, 96];

pub struct FontEntry {
    pub name: String,
    pub path: PathBuf,
    /// Amiga bitmap font: its sizes and their files (empty for TrueType/OpenType fonts).
    pub sizes: Vec<(u32, PathBuf)>,
}

impl FontEntry {
    pub fn is_bitmap(&self) -> bool {
        !self.sizes.is_empty()
    }
    /// The bitmap size closest to `size`.
    pub fn nearest_size(&self, size: u32) -> Option<&(u32, PathBuf)> {
        self.sizes.iter().min_by_key(|(s, _)| (*s as i64 - size as i64).abs())
    }
}

/// A font ready to draw with.
pub enum LoadedFont {
    Outline(FontVec),
    Bitmap(crate::amigafont::AmigaFont),
}

/// Load a font from the list; bitmap fonts at the size nearest `size`.
pub fn load_entry(e: &FontEntry, size: u32) -> Option<LoadedFont> {
    if let Some((_, p)) = e.nearest_size(size) {
        crate::amigafont::load(p, &e.name).ok().map(LoadedFont::Bitmap)
    } else {
        load_font(&e.path).map(LoadedFont::Outline)
    }
}

/// Render with either kind of font.
pub fn render_any(font: &LoadedFont, text: &str, style: &TextStyle, fg: u8, bg: u8, pal: &Palette) -> Option<Brush> {
    match font {
        LoadedFont::Outline(f) => render(f, text, style, fg, bg, pal),
        LoadedFont::Bitmap(f) => f.render(text, &crate::amigafont::Style { bold: style.bold, italic: style.italic, underline: style.underline }, fg, bg, pal),
    }
}

pub fn system_fonts() -> Vec<FontEntry> {
    let mut out = Vec::new();
    let dirs = [
        std::env::var("WINDIR").map(|w| PathBuf::from(w).join("Fonts")).unwrap_or_else(|_| PathBuf::from("C:\\Windows\\Fonts")),
        dirs_local_fonts(),
    ];
    for d in dirs.iter() {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
                if ext == "ttf" || ext == "otf" {
                    let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    out.push(FontEntry { name, path: p, sizes: Vec::new() });
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    out
}

/// Fonts in one folder (Prefs > Font Dir): TrueType and OpenType files, and Amiga bitmap fonts,
/// each in a folder of its own named after the font with one file per size (Chisel/50c...).
/// The chosen folder may also be one Amiga font's own folder.
pub fn fonts_in(dir: &std::path::Path) -> Vec<FontEntry> {
    let mut out = Vec::new();
    let folder_name = |p: &std::path::Path| p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let own = crate::amigafont::sizes_in(dir);
    if !own.is_empty() {
        out.push(FontEntry { name: folder_name(dir), path: dir.to_path_buf(), sizes: own });
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                let sizes = crate::amigafont::sizes_in(&p);
                if !sizes.is_empty() {
                    out.push(FontEntry { name: folder_name(&p), path: p, sizes });
                }
                continue;
            }
            let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext == "ttf" || ext == "otf" {
                let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                out.push(FontEntry { name, path: p, sizes: Vec::new() });
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn dirs_local_fonts() -> PathBuf {
    std::env::var("LOCALAPPDATA").map(|l| PathBuf::from(l).join("Microsoft\\Windows\\Fonts")).unwrap_or_default()
}

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub size: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub antialias: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle { size: 24, bold: false, italic: false, underline: false, antialias: false }
    }
}

pub fn load_font(path: &PathBuf) -> Option<FontVec> {
    let data = std::fs::read(path).ok()?;
    FontVec::try_from_vec(data).ok()
}

/// Render `text` into a custom brush in colour `fg` (background transparent).
pub fn render(font: &FontVec, text: &str, style: &TextStyle, fg: u8, bg: u8, pal: &Palette) -> Option<Brush> {
    if text.is_empty() {
        return None;
    }
    let scale = PxScale::from(style.size as f32);
    let sf = font.as_scaled(scale);
    let lines: Vec<&str> = text.split('\n').collect();
    let line_h = (sf.ascent() - sf.descent() + sf.line_gap()).ceil() as i32;
    let mut width = 1i32;
    for l in &lines {
        let mut x = 0.0f32;
        let mut prev = None;
        for ch in l.chars() {
            let id = font.glyph_id(ch);
            if let Some(p) = prev {
                x += sf.kern(p, id);
            }
            x += sf.h_advance(id);
            prev = Some(id);
        }
        width = width.max(x.ceil() as i32);
    }
    let slant = if style.italic { (style.size as f32 * 0.25).ceil() as i32 } else { 0 };
    let extra = if style.bold { 1 + style.size as i32 / 24 } else { 0 };
    let w = width + slant + extra + 2;
    let h = line_h * lines.len() as i32 + 2;
    let mut cov = vec![0f32; (w * h) as usize];
    for (li, l) in lines.iter().enumerate() {
        let base = sf.ascent() + (li as i32 * line_h) as f32;
        let mut x = 1.0f32;
        let mut prev = None;
        for ch in l.chars() {
            let id = font.glyph_id(ch);
            if let Some(p) = prev {
                x += sf.kern(p, id);
            }
            let g = id.with_scale_and_position(scale, ab_glyph::point(x, base));
            if let Some(og) = font.outline_glyph(g) {
                let bb = og.px_bounds();
                og.draw(|gx, gy, c| {
                    let py = bb.min.y as i32 + gy as i32;
                    let shear = if style.italic { ((h - py) as f32 * slant as f32 / h as f32).round() as i32 } else { 0 };
                    for b in 0..=extra {
                        let px = bb.min.x as i32 + gx as i32 + shear + b;
                        if px >= 0 && py >= 0 && px < w && py < h {
                            let i = (py * w + px) as usize;
                            cov[i] = cov[i].max(c);
                        }
                    }
                });
            }
            x += sf.h_advance(id);
            prev = Some(id);
        }
        if style.underline {
            let uy = (base + 2.0 + style.size as f32 / 16.0) as i32;
            let thick = (style.size as i32 / 16).max(1);
            for t in 0..thick {
                for ux in 1..(x.ceil() as i32 + slant).min(w) {
                    if uy + t < h {
                        cov[((uy + t) * w + ux) as usize] = 1.0;
                    }
                }
            }
        }
    }
    let mut img = Image::new(w, h, bg);
    for i in 0..cov.len() {
        let c = cov[i];
        if style.antialias {
            if c > 0.08 {
                let a = pal[fg as usize];
                let b = pal[bg as usize];
                let mix = |k: usize| (b[k] as f32 * (1.0 - c) + a[k] as f32 * c).round() as i32;
                img.px[i] = if c > 0.92 { fg } else { nearest(pal, [mix(0), mix(1), mix(2)], 256) };
            }
        } else if c >= 0.5 {
            img.px[i] = fg;
        }
    }
    let mut b = Brush::from_image(img, Some(bg));
    b.hx = 0;
    b.hy = 0;
    b.name = "Text".into();
    Some(b)
}
