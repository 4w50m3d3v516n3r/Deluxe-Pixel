//! Gallery: a slideshow of pictures with transitions (the companion Gallery program).

use crate::fileio::{self, LoadResult};
use crate::palette::{Palette, Range};
use crate::pix::Image;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Random,
    Cut,
    Fade,
    Blend,
    HBlinds,
    VBlinds,
    ZoomIn,
    ZoomOut,
}

impl Transition {
    pub const ALL: [Transition; 8] = [
        Transition::Random,
        Transition::Cut,
        Transition::Fade,
        Transition::Blend,
        Transition::HBlinds,
        Transition::VBlinds,
        Transition::ZoomIn,
        Transition::ZoomOut,
    ];
    pub fn label(&self) -> &'static str {
        match self {
            Transition::Random => "Random",
            Transition::Cut => "Cut",
            Transition::Fade => "Fade",
            Transition::Blend => "Blend",
            Transition::HBlinds => "Horizontal Blinds",
            Transition::VBlinds => "Vertical Blinds",
            Transition::ZoomIn => "Zoom In",
            Transition::ZoomOut => "Zoom Out",
        }
    }
    fn code(&self) -> &'static str {
        match self {
            Transition::Random => "random",
            Transition::Cut => "cut",
            Transition::Fade => "fade",
            Transition::Blend => "blend",
            Transition::HBlinds => "hblinds",
            Transition::VBlinds => "vblinds",
            Transition::ZoomIn => "zoomin",
            Transition::ZoomOut => "zoomout",
        }
    }
    fn from_code(s: &str) -> Transition {
        Transition::ALL.iter().cloned().find(|t| t.code() == s).unwrap_or(Transition::Cut)
    }
}

#[derive(Clone)]
pub struct Slide {
    pub path: PathBuf,
    pub img: Image,
    pub palette: Palette,
    pub ranges: Vec<Range>,
    /// Seconds on screen, 1..=9 as in the original's Delay menu.
    pub delay: u8,
    pub transition: Transition,
}

pub struct Gallery {
    pub slides: Vec<Slide>,
    pub selected: usize,
    pub manual: bool,
    pub black_start: bool,
    pub black_end: bool,
    pub cycling: bool,
    pub common_palette: bool,
    pub path: Option<PathBuf>,
}

impl Default for Gallery {
    fn default() -> Self {
        Gallery {
            slides: Vec::new(),
            selected: 0,
            manual: false,
            black_start: true,
            black_end: true,
            cycling: true,
            common_palette: false,
            path: None,
        }
    }
}

/// The original refused to hold more than a fixed number of slides; we keep a generous cap.
pub const MAX_SLIDES: usize = 500;
pub const TRANSITION_SECONDS: f32 = 1.0;

pub fn load_slide(path: &Path) -> Result<Slide, String> {
    match fileio::load(path)? {
        LoadResult::Indexed(l) => Ok(Slide {
            path: path.to_path_buf(),
            img: l.frames[0].clone(),
            palette: l.palette,
            ranges: l.ranges.unwrap_or_default(),
            delay: 3,
            transition: Transition::Fade,
        }),
        LoadResult::TrueColor(rgb) => {
            let (img, pal) = crate::convert::reduce(&rgb, &Default::default(), &crate::palette::DEFAULT_PALETTE);
            Ok(Slide { path: path.to_path_buf(), img, palette: pal, ranges: Vec::new(), delay: 3, transition: Transition::Fade })
        }
    }
}

impl Gallery {
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut s = String::from("GALLERY 1\n");
        s += &format!(
            "options manual={} black_start={} black_end={} cycling={} common_palette={}\n",
            self.manual as u8, self.black_start as u8, self.black_end as u8, self.cycling as u8, self.common_palette as u8
        );
        for sl in &self.slides {
            s += &format!("slide {} {} {}\n", sl.delay, sl.transition.code(), sl.path.display());
        }
        std::fs::write(path, s).map_err(|e| format!("Can't save file: {e}"))
    }

    pub fn load(path: &Path) -> Result<(Gallery, Vec<String>), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("Can't load file: {e}"))?;
        let mut g = Gallery::default();
        let mut problems = Vec::new();
        let mut lines = text.lines();
        if lines.next().map(|l| l.starts_with("GALLERY")) != Some(true) {
            return Err("Mangled Gallery file.".into());
        }
        for l in lines {
            if let Some(rest) = l.strip_prefix("options ") {
                for kv in rest.split_whitespace() {
                    let (k, v) = kv.split_once('=').unwrap_or((kv, "0"));
                    let on = v == "1";
                    match k {
                        "manual" => g.manual = on,
                        "black_start" => g.black_start = on,
                        "black_end" => g.black_end = on,
                        "cycling" => g.cycling = on,
                        "common_palette" => g.common_palette = on,
                        _ => {}
                    }
                }
            } else if let Some(rest) = l.strip_prefix("slide ") {
                let mut parts = rest.splitn(3, ' ');
                let delay = parts.next().and_then(|d| d.parse().ok()).unwrap_or(3u8).clamp(1, 9);
                let tr = Transition::from_code(parts.next().unwrap_or("cut"));
                let p = PathBuf::from(parts.next().unwrap_or(""));
                match load_slide(&p) {
                    Ok(mut s) => {
                        s.delay = delay;
                        s.transition = tr;
                        g.slides.push(s);
                    }
                    Err(e) => problems.push(format!("{}: {e}", p.display())),
                }
            }
        }
        g.path = Some(path.to_path_buf());
        Ok((g, problems))
    }

    pub fn frame_size(&self) -> (i32, i32) {
        let w = self.slides.iter().map(|s| s.img.w).max().unwrap_or(320);
        let h = self.slides.iter().map(|s| s.img.h).max().unwrap_or(200);
        (w, h)
    }

    /// RGB picture of a slide, centred on a black frame of the gallery's size, with optional cycling.
    pub fn slide_rgb(&self, i: usize, t: f32) -> Vec<[u8; 3]> {
        let (w, h) = self.frame_size();
        let mut out = vec![[0u8; 3]; (w * h) as usize];
        let Some(s) = self.slides.get(i) else { return out };
        let mut pal = if self.common_palette { self.slides[0].palette } else { s.palette };
        if self.cycling && !s.ranges.is_empty() {
            let mut doc = crate::doc::Document::new(1, 1, 0);
            doc.palette = pal;
            doc.ranges = s.ranges.clone();
            let phases: Vec<f32> = s.ranges.iter().map(|r| r.steps_per_second() * t).collect();
            pal = doc.cycled_palette(&phases);
        }
        let ox = (w - s.img.w) / 2;
        let oy = (h - s.img.h) / 2;
        for y in 0..s.img.h {
            for x in 0..s.img.w {
                let c = if self.common_palette {
                    let src = s.palette[s.img.get(x, y) as usize];
                    pal[crate::palette::nearest(&pal, [src[0] as i32, src[1] as i32, src[2] as i32], 256) as usize]
                } else {
                    pal[s.img.get(x, y) as usize]
                };
                out[((y + oy) * w + x + ox) as usize] = c;
            }
        }
        out
    }
}

/// Combine the outgoing and incoming pictures for transition progress `p` (0..1).
pub fn compose(tr: Transition, from: &[[u8; 3]], to: &[[u8; 3]], w: i32, h: i32, p: f32) -> Vec<[u8; 3]> {
    let p = p.clamp(0.0, 1.0);
    let mut out = vec![[0u8; 3]; (w * h) as usize];
    let mix = |a: [u8; 3], b: [u8; 3], t: f32| -> [u8; 3] {
        [
            (a[0] as f32 * (1.0 - t) + b[0] as f32 * t) as u8,
            (a[1] as f32 * (1.0 - t) + b[1] as f32 * t) as u8,
            (a[2] as f32 * (1.0 - t) + b[2] as f32 * t) as u8,
        ]
    };
    let band = 16;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            out[i] = match tr {
                Transition::Cut | Transition::Random => to[i],
                Transition::Blend => mix(from[i], to[i], p),
                Transition::Fade => {
                    if p < 0.5 {
                        mix(from[i], [0, 0, 0], p * 2.0)
                    } else {
                        mix([0, 0, 0], to[i], (p - 0.5) * 2.0)
                    }
                }
                Transition::HBlinds => {
                    if ((y % band) as f32) < p * band as f32 { to[i] } else { from[i] }
                }
                Transition::VBlinds => {
                    if ((x % band) as f32) < p * band as f32 { to[i] } else { from[i] }
                }
                Transition::ZoomIn => {
                    let dx = (x - w / 2).abs() as f32 / (w as f32 / 2.0);
                    let dy = (y - h / 2).abs() as f32 / (h as f32 / 2.0);
                    if dx <= p && dy <= p {
                        let sx = (w as f32 / 2.0 + (x - w / 2) as f32 / p.max(0.01)) as i32;
                        let sy = (h as f32 / 2.0 + (y - h / 2) as f32 / p.max(0.01)) as i32;
                        to[(sy.clamp(0, h - 1) * w + sx.clamp(0, w - 1)) as usize]
                    } else {
                        from[i]
                    }
                }
                Transition::ZoomOut => {
                    let q = 1.0 - p;
                    let dx = (x - w / 2).abs() as f32 / (w as f32 / 2.0);
                    let dy = (y - h / 2).abs() as f32 / (h as f32 / 2.0);
                    if dx <= q && dy <= q && q > 0.01 {
                        let sx = (w as f32 / 2.0 + (x - w / 2) as f32 / q) as i32;
                        let sy = (h as f32 / 2.0 + (y - h / 2) as f32 / q) as i32;
                        from[(sy.clamp(0, h - 1) * w + sx.clamp(0, w - 1)) as usize]
                    } else {
                        to[i]
                    }
                }
            };
        }
    }
    out
}

/// Slideshow player state.
pub struct Show {
    pub index: usize,
    pub phase_start: f64,
    pub in_transition: bool,
    pub from: Vec<[u8; 3]>,
    pub transition: Transition,
    pub finished: bool,
    pub start_time: f64,
    pub ending: bool,
}
