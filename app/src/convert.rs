//! Picture conversion (the companion converter): resize by stretch or pad, and reduce
//! true-colour pictures to a palette by Floyd-Steinberg dither, closest colour or truncation.

use crate::palette::{cga_palette, ega_palette, grey_palette, mono_palette, nearest, Palette, Rgb, DEFAULT_PALETTE};
use crate::pix::Image;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteChoice {
    /// The most common colours of the picture (median cut).
    MostCommon,
    /// The current picture's palette.
    Current,
    /// The original program's default palette.
    Default,
    Ega,
    Cga,
    Mono,
    Grey,
}

impl PaletteChoice {
    pub const ALL: [PaletteChoice; 7] = [
        PaletteChoice::MostCommon,
        PaletteChoice::Current,
        PaletteChoice::Default,
        PaletteChoice::Ega,
        PaletteChoice::Cga,
        PaletteChoice::Mono,
        PaletteChoice::Grey,
    ];
    pub fn label(&self) -> &'static str {
        match self {
            PaletteChoice::MostCommon => "Most common colours",
            PaletteChoice::Current => "Current picture's palette",
            PaletteChoice::Default => "Default 256-colour palette",
            PaletteChoice::Ega => "EGA 16 colours",
            PaletteChoice::Cga => "CGA 4 colours",
            PaletteChoice::Mono => "Black & white",
            PaletteChoice::Grey => "Shades of grey",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    FloydSteinberg,
    Closest,
}

#[derive(Clone, Debug)]
pub struct ConvertOptions {
    pub colors: usize,
    pub palette: PaletteChoice,
    pub method: Method,
    pub resize: Option<(i32, i32)>,
    pub stretch: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        ConvertOptions { colors: 256, palette: PaletteChoice::MostCommon, method: Method::FloydSteinberg, resize: None, stretch: true }
    }
}

pub struct Rgba {
    pub w: i32,
    pub h: i32,
    pub px: Vec<[u8; 3]>,
}

impl Rgba {
    pub fn stretched(&self, w: i32, h: i32) -> Rgba {
        let mut px = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            let sy = (y as i64 * self.h as i64 / h as i64) as i32;
            for x in 0..w {
                let sx = (x as i64 * self.w as i64 / w as i64) as i32;
                px.push(self.px[(sy * self.w + sx) as usize]);
            }
        }
        Rgba { w, h, px }
    }
    pub fn padded(&self, w: i32, h: i32) -> Rgba {
        let mut px = vec![[0u8; 3]; (w * h) as usize];
        for y in 0..h.min(self.h) {
            for x in 0..w.min(self.w) {
                px[(y * w + x) as usize] = self.px[(y * self.w + x) as usize];
            }
        }
        Rgba { w, h, px }
    }
}

/// Median-cut palette of up to `n` colours.
pub fn median_cut(px: &[[u8; 3]], n: usize) -> Vec<Rgb> {
    let n = n.clamp(2, 256);
    // Sample to keep it quick on large pictures.
    let step = (px.len() / 200_000).max(1);
    let sample: Vec<[u8; 3]> = px.iter().step_by(step).cloned().collect();
    // If the picture already has few colours, keep them exactly.
    let mut uniq: Vec<[u8; 3]> = sample.clone();
    uniq.sort();
    uniq.dedup();
    if uniq.len() <= n {
        let mut all: Vec<[u8; 3]> = px.to_vec();
        all.sort();
        all.dedup();
        if all.len() <= n {
            return all;
        }
    }
    let mut boxes: Vec<Vec<[u8; 3]>> = vec![sample];
    while boxes.len() < n {
        // Split the box with the widest channel range.
        let mut best = None;
        let mut best_range = 0i32;
        for (i, b) in boxes.iter().enumerate() {
            if b.len() < 2 {
                continue;
            }
            for k in 0..3 {
                let mn = b.iter().map(|c| c[k]).min().unwrap() as i32;
                let mx = b.iter().map(|c| c[k]).max().unwrap() as i32;
                if mx - mn > best_range {
                    best_range = mx - mn;
                    best = Some((i, k));
                }
            }
        }
        let Some((i, k)) = best else { break };
        let mut b = boxes.swap_remove(i);
        b.sort_by_key(|c| c[k]);
        let half = b.split_off(b.len() / 2);
        boxes.push(b);
        boxes.push(half);
    }
    boxes
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let mut s = [0u64; 3];
            for c in b {
                for k in 0..3 {
                    s[k] += c[k] as u64;
                }
            }
            let l = b.len() as u64;
            [(s[0] / l) as u8, (s[1] / l) as u8, (s[2] / l) as u8]
        })
        .collect()
}

/// Convert a true-colour picture into an indexed image and palette.
pub fn reduce(src: &Rgba, opts: &ConvertOptions, current: &Palette) -> (Image, Palette) {
    let src_resized;
    let src = match opts.resize {
        Some((w, h)) if w > 0 && h > 0 && (w != src.w || h != src.h) => {
            src_resized = if opts.stretch { src.stretched(w, h) } else { src.padded(w, h) };
            &src_resized
        }
        _ => src,
    };
    let colors: Vec<Rgb> = match opts.palette {
        PaletteChoice::MostCommon => median_cut(&src.px, opts.colors),
        PaletteChoice::Current => current.to_vec(),
        PaletteChoice::Default => DEFAULT_PALETTE.to_vec(),
        PaletteChoice::Ega => ega_palette(),
        PaletteChoice::Cga => cga_palette(),
        PaletteChoice::Mono => mono_palette(),
        PaletteChoice::Grey => grey_palette(opts.colors.min(256)),
    };
    let count = colors.len().clamp(1, 256);
    let mut pal: Palette = if opts.palette == PaletteChoice::Current { *current } else { DEFAULT_PALETTE };
    for (i, c) in colors.iter().enumerate().take(256) {
        pal[i] = *c;
    }
    let mut out = Image::new(src.w, src.h, 0);
    match opts.method {
        Method::Closest => {
            let mut cache = std::collections::HashMap::new();
            for (i, c) in src.px.iter().enumerate() {
                let idx = *cache.entry(*c).or_insert_with(|| nearest(&pal, [c[0] as i32, c[1] as i32, c[2] as i32], count));
                out.px[i] = idx;
            }
        }
        Method::FloydSteinberg => {
            let w = src.w as usize;
            let mut err_cur = vec![[0i32; 3]; w + 2];
            let mut err_next = vec![[0i32; 3]; w + 2];
            for y in 0..src.h as usize {
                for x in 0..w {
                    let c = src.px[y * w + x];
                    let e = err_cur[x + 1];
                    let want = [
                        (c[0] as i32 + e[0] / 16).clamp(0, 255),
                        (c[1] as i32 + e[1] / 16).clamp(0, 255),
                        (c[2] as i32 + e[2] / 16).clamp(0, 255),
                    ];
                    let idx = nearest(&pal, want, count);
                    out.px[y * w + x] = idx;
                    let got = pal[idx as usize];
                    for k in 0..3 {
                        let d = want[k] - got[k] as i32;
                        err_cur[x + 2][k] += d * 7;
                        err_next[x][k] += d * 3;
                        err_next[x + 1][k] += d * 5;
                        err_next[x + 2][k] += d;
                    }
                }
                std::mem::swap(&mut err_cur, &mut err_next);
                err_next.iter_mut().for_each(|e| *e = [0; 3]);
            }
        }
    }
    (out, pal)
}
