//! Amiga bitmap fonts, as DeluxePaint used them: a font is a folder named after it (for example
//! `Chisel/`) holding one file per size (`50`, `62c`...). Each file is an AmigaDOS load file
//! with a DiskFontHeader and a TextFont: glyph bitmaps side by side in one bitplane (or several
//! for a ColorText font), a location/width table, and optional spacing and kerning tables.
//! ColorText fonts carry their own colour table (12-bit RGB).

use crate::palette::{nearest, Palette};
use crate::pix::{Brush, Image};
use std::path::{Path, PathBuf};

const HUNK_HEADER: u32 = 0x3F3;
const HUNK_CODE: u32 = 0x3E9;
const HUNK_DATA: u32 = 0x3EA;
const DFH_ID: u16 = 0x0F80;
const FSF_COLORFONT: u8 = 0x40;
const FPF_PROPORTIONAL: u8 = 0x20;
const CT_COLORFONT: u16 = 0x1;
const CT_GREYFONT: u16 = 0x2;

#[derive(Clone, Debug)]
pub struct AmigaFont {
    pub name: String,
    pub path: PathBuf,
    pub ysize: i32,
    pub xsize: i32,
    pub baseline: i32,
    pub bold_smear: i32,
    pub lo: u8,
    pub hi: u8,
    pub proportional: bool,
    /// ColorText font: pens come from the font's own colour table.
    pub color: bool,
    /// ColorText grey font: pens are levels between background and the foreground colour.
    pub grey: bool,
    pub depth: usize,
    pub modulo: usize,
    /// One bitmap per plane (modulo * ysize bytes); None = plane is all `plane_onoff`.
    pub planes: Vec<Option<Vec<u8>>>,
    pub plane_onoff: u8,
    /// (bit offset, bit width) per character, lo..=hi plus the "not defined" glyph.
    pub loc: Vec<(u32, u32)>,
    pub space: Vec<i32>,
    pub kern: Vec<i32>,
    /// The font's colours by pen number (ColorText fonts).
    pub colors: Vec<[u8; 3]>,
}

fn be16(d: &[u8], o: usize) -> Result<u16, String> {
    d.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]])).ok_or_else(|| "The font file is cut short.".to_string())
}
fn be32(d: &[u8], o: usize) -> Result<u32, String> {
    d.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "The font file is cut short.".to_string())
}

/// The first code or data hunk of an AmigaDOS load file. Pointers inside it are offsets from its
/// start (they are relocated against the hunk itself), so they can be used as they are.
pub fn first_hunk(d: &[u8]) -> Result<&[u8], String> {
    if be32(d, 0)? != HUNK_HEADER {
        return Err("Not an Amiga font file.".into());
    }
    let mut p = 4;
    // Resident library names (none in fonts), ended by 0.
    loop {
        let n = be32(d, p)? as usize;
        p += 4;
        if n == 0 {
            break;
        }
        p += n * 4;
    }
    let _table = be32(d, p)?;
    let first = be32(d, p + 4)?;
    let last = be32(d, p + 8)?;
    p += 12 + (last.saturating_sub(first) as usize + 1) * 4;
    loop {
        let t = be32(d, p)? & 0x3FFF_FFFF;
        p += 4;
        match t {
            HUNK_CODE | HUNK_DATA => {
                let n = be32(d, p)? as usize * 4;
                p += 4;
                return d.get(p..p + n).ok_or_else(|| "The font file is cut short.".to_string());
            }
            0x3E8 => {
                // HUNK_NAME
                let n = be32(d, p)? as usize * 4;
                p += 4 + n;
            }
            _ => return Err("Not an Amiga font file.".into()),
        }
    }
}

pub fn parse(data: &[u8], name: &str, path: &Path) -> Result<AmigaFont, String> {
    let h = first_hunk(data)?;
    // DiskFontHeader: 4 bytes of code, a Node (14), FileID, Revision, Segment, Name[32], TextFont.
    if be16(h, 18)? != DFH_ID {
        return Err("Not an Amiga font file.".into());
    }
    let tf = 58;
    let ysize = be16(h, tf + 20)? as i32;
    let style = *h.get(tf + 22).ok_or("The font file is cut short.")?;
    let flags = h[tf + 23];
    let xsize = be16(h, tf + 24)? as i32;
    let baseline = be16(h, tf + 26)? as i32;
    let bold_smear = be16(h, tf + 28)? as i32;
    let lo = h[tf + 32];
    let hi = h[tf + 33];
    let chardata = be32(h, tf + 34)? as usize;
    let modulo = be16(h, tf + 38)? as usize;
    let charloc = be32(h, tf + 40)? as usize;
    let charspace = be32(h, tf + 44)? as usize;
    let charkern = be32(h, tf + 48)? as usize;
    if ysize == 0 || hi < lo || modulo == 0 {
        return Err("This font has no characters.".into());
    }
    let n = (hi - lo) as usize + 2;
    let plane_len = modulo * ysize as usize;
    let take = |off: usize| -> Result<Vec<u8>, String> { h.get(off..off + plane_len).map(|s| s.to_vec()).ok_or_else(|| "The font file is cut short.".to_string()) };
    let mut planes = Vec::new();
    let (mut color, mut grey, mut plane_onoff, mut colors) = (false, false, 0u8, Vec::new());
    if style & FSF_COLORFONT != 0 {
        let ct = tf + 52;
        let ctf_flags = be16(h, ct)?;
        let depth = (h[ct + 2] as usize).clamp(1, 8);
        let pick = h[ct + 6];
        plane_onoff = h[ct + 7];
        color = ctf_flags & CT_COLORFONT != 0;
        grey = ctf_flags & CT_GREYFONT != 0;
        let cfc = be32(h, ct + 8)? as usize;
        if cfc != 0 {
            let count = be16(h, cfc + 2)? as usize;
            let table = be32(h, cfc + 4)? as usize;
            for i in 0..count.min(256) {
                let v = be16(h, table + i * 2)?;
                colors.push([((v >> 8) & 15) as u8 * 17, ((v >> 4) & 15) as u8 * 17, (v & 15) as u8 * 17]);
            }
        }
        for i in 0..depth {
            let ptr = be32(h, ct + 12 + i * 4)? as usize;
            planes.push(if pick & (1 << i) != 0 && ptr != 0 { Some(take(ptr)?) } else { None });
        }
    } else {
        planes.push(Some(take(chardata)?));
    }
    let mut loc = Vec::with_capacity(n);
    for i in 0..n {
        let v = be32(h, charloc + i * 4)?;
        loc.push((v >> 16, v & 0xFFFF));
    }
    let proportional = flags & FPF_PROPORTIONAL != 0;
    let mut space = vec![xsize; n];
    if charspace != 0 {
        for (i, s) in space.iter_mut().enumerate() {
            *s = be16(h, charspace + i * 2)? as i16 as i32;
        }
    }
    let mut kern = vec![0; n];
    if charkern != 0 {
        for (i, k) in kern.iter_mut().enumerate() {
            *k = be16(h, charkern + i * 2)? as i16 as i32;
        }
    }
    Ok(AmigaFont {
        name: name.to_string(),
        path: path.to_path_buf(),
        ysize,
        xsize,
        baseline,
        bold_smear: bold_smear.max(1),
        lo,
        hi,
        proportional,
        color,
        grey,
        depth: planes.len(),
        modulo,
        planes,
        plane_onoff,
        loc,
        space,
        kern,
        colors,
    })
}

pub fn load(path: &Path, name: &str) -> Result<AmigaFont, String> {
    let data = std::fs::read(path).map_err(|e| format!("Can't load font: {e}"))?;
    parse(&data, name, path)
}

/// The size a font file is for, from its name ("50", "62c" → 50, 62).
pub fn size_of_name(file: &str) -> Option<u32> {
    let digits: String = file.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest = &file[digits.len()..];
    if digits.is_empty() || !rest.chars().all(|c| c.is_ascii_alphabetic()) || rest.len() > 2 {
        return None;
    }
    digits.parse().ok()
}

/// The sizes of an Amiga font folder: (size, file), smallest first. Only real font files count.
pub fn sizes_in(dir: &Path) -> Vec<(u32, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_file() {
                continue;
            }
            let Some(size) = p.file_name().and_then(|f| size_of_name(&f.to_string_lossy())) else { continue };
            let mut head = [0u8; 4];
            let ok = std::fs::File::open(&p).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok();
            if ok && u32::from_be_bytes(head) == HUNK_HEADER {
                out.push((size, p));
            }
        }
    }
    out.sort();
    out
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl AmigaFont {
    fn index(&self, ch: char) -> usize {
        let i = self.raw_index(ch);
        // Headline fonts often have capitals only: a small letter drawn with the font's
        // "missing" glyph uses its capital instead.
        let missing = self.loc.len() - 1;
        if ch.is_ascii_lowercase() && (i == missing || self.loc[i] == self.loc[missing]) {
            let u = self.raw_index(ch.to_ascii_uppercase());
            if u != missing && self.loc[u] != self.loc[missing] {
                return u;
            }
        }
        i
    }

    fn raw_index(&self, ch: char) -> usize {
        let c = ch as u32;
        if c >= self.lo as u32 && c <= self.hi as u32 {
            (c - self.lo as u32) as usize
        } else {
            self.loc.len() - 1
        }
    }

    /// Pen number of glyph pixel (bit `x` of the font bitmap, row `y`).
    fn pen(&self, x: u32, y: i32) -> u8 {
        let byte = y as usize * self.modulo + (x / 8) as usize;
        let mask = 0x80u8 >> (x % 8);
        let mut v = 0u8;
        for (p, plane) in self.planes.iter().enumerate() {
            let bit = match plane {
                Some(d) => d.get(byte).map(|b| b & mask != 0).unwrap_or(false),
                None => self.plane_onoff & (1 << p) != 0,
            };
            if bit {
                v |= 1 << p;
            }
        }
        v
    }

    /// Width in pixels of one line of text.
    pub fn line_width(&self, line: &str) -> (i32, i32) {
        let (mut x, mut minx, mut maxx) = (0i32, 0i32, 0i32);
        for ch in line.chars() {
            let i = self.index(ch);
            let gx = x + self.kern[i];
            minx = minx.min(gx);
            maxx = maxx.max(gx + self.loc[i].1 as i32).max(x + self.space[i]);
            x += self.kern[i] + self.space[i];
        }
        (minx, maxx.max(x))
    }

    /// Render text into a brush. Plain fonts draw in `fg`; ColorText fonts in their own colours
    /// (matched to the palette); grey fonts blend from `bg` to `fg`.
    pub fn render(&self, text: &str, st: &Style, fg: u8, bg: u8, pal: &Palette) -> Option<Brush> {
        if text.is_empty() {
            return None;
        }
        let lines: Vec<&str> = text.split('\n').collect();
        let widths: Vec<(i32, i32)> = lines.iter().map(|l| self.line_width(l)).collect();
        let left = -widths.iter().map(|w| w.0).min().unwrap_or(0);
        let slant = if st.italic { self.ysize / 4 } else { 0 };
        let smear = if st.bold { self.bold_smear } else { 0 };
        let w = (left + widths.iter().map(|w| w.1).max().unwrap_or(1) + slant + smear).max(1);
        let line_h = self.ysize + 1;
        let h = line_h * lines.len() as i32;
        let pens = 1usize << self.depth.min(8);
        let pen_color: Vec<u8> = (0..pens)
            .map(|pen| {
                if pen == 0 {
                    bg
                } else if self.color && pen < self.colors.len() {
                    let c = self.colors[pen];
                    nearest(pal, [c[0] as i32, c[1] as i32, c[2] as i32], 256)
                } else if self.grey && pens > 2 {
                    let t = pen as f32 / (pens - 1) as f32;
                    let (a, b) = (pal[bg as usize], pal[fg as usize]);
                    let mix = |k: usize| (a[k] as f32 + (b[k] as f32 - a[k] as f32) * t).round() as i32;
                    nearest(pal, [mix(0), mix(1), mix(2)], 256)
                } else {
                    fg
                }
            })
            .collect();
        let mut img = Image::new(w, h, bg);
        for (li, line) in lines.iter().enumerate() {
            let top = li as i32 * line_h;
            let mut x = left;
            for ch in line.chars() {
                let i = self.index(ch);
                let (off, gw) = self.loc[i];
                let gx = x + self.kern[i];
                for gy in 0..self.ysize {
                    let shear = if st.italic { ((self.baseline - gy) * slant / self.ysize.max(1)).max(0) } else { 0 };
                    for bx in 0..gw {
                        let pen = self.pen(off + bx, gy);
                        if pen != 0 {
                            for s in 0..=smear {
                                img.set(gx + bx as i32 + shear + s, top + gy, pen_color[pen as usize]);
                            }
                        }
                    }
                }
                x += self.kern[i] + self.space[i];
            }
            if st.underline {
                let uy = top + (self.baseline + 2).min(self.ysize);
                for ux in left..x + slant {
                    img.set(ux, uy, fg);
                }
            }
        }
        let mut b = Brush::from_image(img, Some(bg));
        b.hx = 0;
        b.hy = 0;
        b.name = "Text".into();
        Some(b)
    }
}
