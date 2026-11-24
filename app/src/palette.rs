//! Palette: 256 RGB entries, colour ranges for cycling/shading/gradients, HSV helpers.

pub type Rgb = [u8; 3];
pub type Palette = [Rgb; 256];

/// The original program's built-in 256-colour palette (see MECHANICS.md, "Default palette").
pub const DEFAULT_PALETTE: Palette = [
    [0,0,0],[0,0,170],[0,170,0],[0,170,170],[170,0,0],[170,0,170],[170,85,0],[170,170,170],
    [85,85,85],[85,85,255],[85,255,85],[85,255,255],[255,85,85],[255,85,255],[255,255,85],[255,255,255],
    [239,239,239],[223,223,223],[211,211,211],[195,195,195],[183,183,183],[171,171,171],[155,155,155],[143,143,143],
    [127,127,127],[115,115,115],[103,103,103],[87,87,87],[75,75,75],[59,59,59],[47,47,47],[35,35,35],
    [255,0,0],[239,0,0],[227,0,0],[215,0,0],[203,0,0],[191,0,0],[179,0,0],[167,0,0],
    [155,0,0],[139,0,0],[127,0,0],[115,0,0],[103,0,0],[91,0,0],[79,0,0],[64,0,0],
    [255,218,218],[255,186,186],[255,159,159],[255,127,127],[255,95,95],[255,64,64],[255,32,32],[255,0,0],
    [252,168,92],[252,152,64],[252,136,32],[252,120,0],[228,108,0],[204,96,0],[180,84,0],[156,76,0],
    [252,252,216],[252,252,184],[252,252,156],[252,252,124],[252,248,92],[252,244,64],[252,244,32],[252,244,0],
    [228,216,0],[204,196,0],[180,172,0],[156,156,0],[132,132,0],[112,108,0],[88,84,0],[64,64,0],
    [208,252,92],[196,252,64],[180,252,32],[160,252,0],[144,228,0],[128,204,0],[116,180,0],[96,156,0],
    [216,252,216],[188,252,184],[156,252,156],[128,252,124],[96,252,92],[64,252,64],[32,252,32],[0,252,0],
    [0,255,0],[0,239,0],[0,227,0],[0,215,0],[7,203,0],[7,191,0],[7,179,0],[7,167,0],
    [7,155,0],[7,139,0],[7,127,0],[7,115,0],[7,103,0],[7,91,0],[7,79,0],[4,64,0],
    [218,255,255],[184,252,252],[156,252,252],[124,252,248],[92,252,252],[64,252,252],[32,252,252],[0,252,252],
    [0,228,228],[0,204,204],[0,180,180],[0,156,156],[0,132,132],[0,112,112],[0,88,88],[0,64,64],
    [92,188,252],[64,176,252],[32,168,252],[0,156,252],[0,140,228],[0,124,204],[0,108,180],[0,92,156],
    [218,218,255],[186,191,255],[159,159,255],[127,128,255],[95,96,255],[64,64,255],[32,37,255],[0,5,255],
    [0,0,255],[0,0,239],[0,0,227],[0,0,215],[0,0,203],[0,0,191],[0,0,179],[0,0,167],
    [0,0,155],[0,0,139],[0,0,127],[0,0,115],[0,0,103],[0,0,91],[0,0,79],[0,0,64],
    [240,218,255],[229,186,255],[218,159,255],[208,127,255],[202,95,255],[191,64,255],[181,32,255],[170,0,255],
    [154,0,229],[128,0,207],[117,0,181],[96,0,159],[80,0,133],[69,0,112],[53,0,90],[42,0,64],
    [255,218,255],[255,186,255],[255,159,255],[255,127,255],[255,95,255],[255,64,255],[255,32,255],[255,0,255],
    [224,0,229],[202,0,207],[181,0,181],[159,0,159],[133,0,133],[111,0,112],[90,0,90],[64,0,64],
    [255,234,223],[255,224,208],[255,218,197],[255,213,191],[255,207,176],[255,197,165],[255,191,159],[255,186,144],
    [255,176,128],[255,165,112],[255,159,96],[240,149,95],[234,143,90],[223,138,85],[208,128,80],[202,127,79],
    [191,122,74],[181,112,69],[170,106,64],[160,101,63],[159,96,58],[144,95,53],[138,90,48],[128,80,47],
    [117,79,42],[111,74,37],[95,64,32],[85,63,31],[74,58,26],[64,48,26],[58,47,21],[42,32,15],
    [0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],[0,0,0],
    [199,43,43],[199,79,43],[199,119,43],[199,159,43],[199,199,43],[159,199,43],[119,199,43],[79,199,43],
    [43,199,51],[43,199,95],[43,199,139],[43,199,183],[43,171,199],[43,127,199],[43,83,199],[47,43,199],
    [91,43,199],[135,43,199],[179,43,199],[199,43,175],[199,43,131],[199,43,87],[199,43,43],[255,255,255],
];

/// IFF CRNG rate units: a rate of 16384 means 60 steps per second.
pub const CYCLE_RATE_ONE_STEP_PER_TICK: f32 = 16384.0;
pub const CYCLE_TICKS_PER_SECOND: f32 = 60.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Range {
    pub low: u8,
    pub high: u8,
    /// CRNG rate (0..=16384).
    pub rate: u16,
    pub active: bool,
    pub reverse: bool,
    /// Colours placed on the range bar (DeluxePaint IV/V ranges): (bar position 0..=255, palette
    /// colour), sorted by position. Empty: the colours low..=high side by side.
    pub cells: Vec<(u8, u8)>,
}

impl Range {
    pub fn new(low: u8, high: u8, rate: u16) -> Self {
        Range { low: low.min(high), high: low.max(high), rate, active: true, reverse: false, cells: Vec::new() }
    }
    /// A range of colours placed on the bar at the given positions.
    pub fn from_cells(mut cells: Vec<(u8, u8)>, rate: u16) -> Self {
        cells.sort_by_key(|c| c.0);
        cells.dedup_by_key(|c| c.0);
        let mut r = Range::new(0, 0, rate);
        r.set_cells(cells);
        r
    }
    /// Replace the placed colours (low / high follow the lowest and highest colour).
    pub fn set_cells(&mut self, mut cells: Vec<(u8, u8)>) {
        cells.sort_by_key(|c| c.0);
        cells.dedup_by_key(|c| c.0);
        if let (Some(lo), Some(hi)) = (cells.iter().map(|c| c.1).min(), cells.iter().map(|c| c.1).max()) {
            self.low = lo;
            self.high = hi;
        }
        self.cells = cells;
    }
    /// The placed colours as (bar position, colour): the cells, or low..=high side by side.
    pub fn bar(&self) -> Vec<(u8, u8)> {
        if self.cells.is_empty() {
            if self.high <= self.low && !self.active && self.rate == 0 {
                return Vec::new();
            }
            (self.low..=self.high).enumerate().map(|(k, c)| (k as u8, c)).collect()
        } else {
            self.cells.clone()
        }
    }
    /// The range's palette colours in bar order (what Shade, Cycle and colour cycling step through).
    pub fn registers(&self) -> Vec<u8> {
        if self.cells.is_empty() {
            (self.low..=self.high).collect()
        } else {
            self.cells.iter().map(|c| c.1).collect()
        }
    }
    pub fn len(&self) -> usize {
        if self.cells.is_empty() {
            self.high as usize - self.low as usize + 1
        } else {
            self.cells.len()
        }
    }
    pub fn contains(&self, c: u8) -> bool {
        if self.cells.is_empty() {
            c >= self.low && c <= self.high
        } else {
            self.cells.iter().any(|x| x.1 == c)
        }
    }
    /// The colours a gradient runs through, in order (REV turns it round): across gaps on the bar
    /// the colours are blended and the nearest palette colours used, as the original does.
    pub fn sequence(&self, pal: &Palette) -> Vec<u8> {
        let mut out: Vec<u8> = if self.cells.len() < 2 {
            self.registers()
        } else {
            let mut v = Vec::new();
            for w in self.cells.windows(2) {
                let ((pa, ca), (pb, cb)) = (w[0], w[1]);
                let span = (pb - pa).max(1) as f32;
                for k in 0..(pb - pa) {
                    if k == 0 {
                        v.push(ca);
                    } else {
                        let t = k as f32 / span;
                        let (a, b) = (pal[ca as usize], pal[cb as usize]);
                        let mix = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as i32;
                        v.push(nearest(pal, [mix(0), mix(1), mix(2)], 256));
                    }
                }
            }
            v.push(self.cells.last().unwrap().1);
            v
        };
        if self.reverse {
            out.reverse();
        }
        out
    }
    pub fn steps_per_second(&self) -> f32 {
        self.rate as f32 * CYCLE_TICKS_PER_SECOND / CYCLE_RATE_ONE_STEP_PER_TICK
    }
}

/// Default ranges: the grey ramp and the red ramp of the default palette (estimated).
pub fn default_ranges() -> Vec<Range> {
    vec![
        Range::new(16, 31, 2560),
        Range::new(32, 47, 2560),
        Range::new(232, 254, 4096),
        Range::new(80, 95, 2560),
        Range::new(112, 127, 2560),
        Range::new(144, 159, 2560),
    ]
}

pub fn to_color32(c: Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

pub fn dist2(a: Rgb, b: [i32; 3]) -> i32 {
    // Weighted RGB distance (green counts most, as the eye does).
    let dr = a[0] as i32 - b[0];
    let dg = a[1] as i32 - b[1];
    let db = a[2] as i32 - b[2];
    2 * dr * dr + 4 * dg * dg + 3 * db * db
}

/// Nearest palette index to an RGB colour, searching only `count` first entries.
pub fn nearest(pal: &Palette, rgb: [i32; 3], count: usize) -> u8 {
    let mut best = 0usize;
    let mut bd = i32::MAX;
    // Never pick a colour the screen doesn't have (Screen Format > Palette Size).
    let count = count.min(crate::amiga::active_colors());
    for (i, c) in pal.iter().take(count.max(1)).enumerate() {
        let d = dist2(*c, rgb);
        if d < bd {
            bd = d;
            best = i;
            if d == 0 {
                break;
            }
        }
    }
    best as u8
}

pub fn rgb_to_hsv(c: Rgb) -> (f32, f32, f32) {
    let r = c[0] as f32 / 255.0;
    let g = c[1] as f32 / 255.0;
    let b = c[2] as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Rgb {
    let c = v * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [
        ((r + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

pub fn luminance(c: Rgb) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

/// Spread: fill colours between `a` and `b` with an even RGB blend (Palette "Spread").
pub fn spread(pal: &mut Palette, a: u8, b: u8) {
    let (lo, hi) = (a.min(b) as usize, a.max(b) as usize);
    if hi <= lo + 1 {
        return;
    }
    let ca = pal[lo];
    let cb = pal[hi];
    let n = (hi - lo) as f32;
    for i in lo + 1..hi {
        let t = (i - lo) as f32 / n;
        for k in 0..3 {
            pal[i][k] = (ca[k] as f32 + (cb[k] as f32 - ca[k] as f32) * t).round() as u8;
        }
    }
}

/// Build the 16-colour EGA, 4-colour CGA, mono or grey palettes used by the converter.
pub fn ega_palette() -> Vec<Rgb> {
    DEFAULT_PALETTE[..16].to_vec()
}
pub fn cga_palette() -> Vec<Rgb> {
    vec![[0, 0, 0], [85, 255, 255], [255, 85, 255], [255, 255, 255]]
}
pub fn mono_palette() -> Vec<Rgb> {
    vec![[0, 0, 0], [255, 255, 255]]
}
pub fn grey_palette(n: usize) -> Vec<Rgb> {
    let n = n.max(2);
    (0..n)
        .map(|i| {
            let v = (i * 255 / (n - 1)) as u8;
            [v, v, v]
        })
        .collect()
}

// ---------------------------------------------------------------- palettes and ranges per screen size
//
// Our own designed palettes for small screens (2–64 colours): ramps from dark to light so that
// Shade, Blend, gradient fills and colour cycling have something to work with, and the ranges
// that go with them. 128 and 256 colours use the default palette and ranges on its ramps.

fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [0, 1, 2].map(|k| (a[k] as f32 + (b[k] as f32 - a[k] as f32) * t).round().clamp(0.0, 255.0) as u8)
}

/// `k` colours from dark through the full colour to a light tint.
fn ramp3(dark: Rgb, mid: Rgb, light: Rgb, k: usize) -> Vec<Rgb> {
    (0..k)
        .map(|i| {
            let t = if k > 1 { i as f32 / (k - 1) as f32 } else { 0.5 };
            if t <= 0.55 {
                lerp(dark, mid, t / 0.55)
            } else {
                lerp(mid, light, (t - 0.55) / 0.45)
            }
        })
        .collect()
}

/// The colour families of the designed palettes: dark, full and light colour.
const FAMILIES: [(Rgb, Rgb, Rgb); 7] = [
    ([48, 0, 0], [220, 16, 16], [255, 200, 190]),    // red
    ([44, 22, 4], [196, 104, 32], [255, 216, 160]),  // brown / skin
    ([44, 40, 0], [228, 204, 0], [255, 255, 200]),   // yellow
    ([0, 36, 4], [16, 176, 32], [200, 255, 196]),    // green
    ([0, 0, 56], [40, 72, 232], [200, 212, 255]),    // blue
    ([36, 0, 48], [164, 36, 204], [240, 200, 255]),  // purple
    ([0, 32, 40], [0, 168, 196], [200, 255, 255]),   // cyan
];

/// Ramp layout for a palette size: (grey length, families used in order, length of each).
fn layout(colors: usize) -> (usize, usize, usize) {
    match colors {
        0..=2 => (2, 0, 0),
        3..=4 => (2, 2, 1),
        5..=8 => (4, 2, 2),
        9..=16 => (4, 4, 3),
        17..=32 => (8, 6, 4),
        _ => (8, 7, 8),
    }
}

/// The palette a new screen of this many colours starts with.
pub fn palette_for(colors: usize) -> Palette {
    if colors > 64 {
        return DEFAULT_PALETTE;
    }
    let (grey, fams, len) = layout(colors);
    let mut cols: Vec<Rgb> = (0..grey).map(|i| lerp([0, 0, 0], [255, 255, 255], i as f32 / (grey - 1).max(1) as f32)).collect();
    for f in FAMILIES.iter().take(fams) {
        if len == 1 {
            cols.push(f.1);
        } else {
            cols.extend(ramp3(f.0, f.1, f.2, len));
        }
    }
    let mut pal = DEFAULT_PALETTE;
    for (i, c) in cols.into_iter().take(colors.max(2)).enumerate() {
        pal[i] = c;
    }
    pal
}

/// The ranges that go with `palette_for(colors)`.
pub fn ranges_for(colors: usize) -> Vec<Range> {
    if colors >= 256 {
        return default_ranges();
    }
    if colors > 64 {
        // Ramps of the default palette below 128: greys, reds, oranges, yellows, greens, cyans.
        return vec![
            Range::new(16, 31, 2560),
            Range::new(32, 47, 2560),
            Range::new(56, 63, 2560),
            Range::new(64, 79, 2560),
            Range::new(96, 111, 2560),
            Range::new(112, 127, 2560),
        ];
    }
    let (grey, fams, len) = layout(colors);
    let mut out = vec![Range::new(0, (grey - 1) as u8, 2560)];
    if len >= 2 {
        for k in 0..fams {
            let lo = grey + k * len;
            out.push(Range::new(lo as u8, (lo + len - 1) as u8, 2560));
        }
    }
    out
}

/// Whether the first `colors` entries are a stock palette (the default one or a designed one),
/// allowing for 4-bit rounding: then the palette may be swapped for the one fitting a new size.
pub fn is_stock(pal: &Palette, colors: usize) -> bool {
    let n = colors.clamp(2, 256);
    let near = |a: &Rgb, b: &Rgb| a.iter().zip(b.iter()).all(|(x, y)| (*x as i32 - *y as i32).abs() <= 9);
    let mut stocks = vec![DEFAULT_PALETTE];
    for s in [2usize, 4, 8, 16, 32, 64] {
        stocks.push(palette_for(s));
    }
    stocks.iter().any(|s| {
        // A designed palette is stock for the size it was made for; the default one for any size.
        pal[..n].iter().zip(s[..n].iter()).all(|(a, b)| near(a, b))
    })
}

/// Ranges found in a palette: runs of neighbouring colours that step evenly from dark to light
/// (or back) in one colour family. Longest first, at most `max`.
pub fn find_ramps(pal: &Palette, colors: usize, max: usize) -> Vec<Range> {
    let n = colors.clamp(2, 256);
    let lum = |c: Rgb| 0.3 * c[0] as f32 + 0.59 * c[1] as f32 + 0.11 * c[2] as f32;
    let hue_close = |a: Rgb, b: Rgb| {
        let (ha, sa, _) = rgb_to_hsv(a);
        let (hb, sb, _) = rgb_to_hsv(b);
        if sa < 0.12 && sb < 0.12 {
            return true;
        }
        if sa < 0.12 || sb < 0.12 {
            return false;
        }
        let d = (ha - hb).abs();
        d.min(360.0 - d) < 28.0
    };
    let step_ok = |a: Rgb, b: Rgb| {
        let d = (lum(b) - lum(a)).abs();
        d > 2.0 && d < 90.0 && hue_close(a, b)
    };
    let mut runs = Vec::new();
    let mut i = 0;
    while i + 1 < n {
        let mut j = i;
        let dir = (lum(pal[i + 1]) - lum(pal[i])).signum();
        while j + 1 < n && step_ok(pal[j], pal[j + 1]) && (lum(pal[j + 1]) - lum(pal[j])).signum() == dir {
            j += 1;
        }
        if j - i + 1 >= 3 || (n <= 16 && j > i) {
            runs.push(Range::new(i as u8, j as u8, 2560));
            i = j + 1;
        } else {
            i += 1;
        }
    }
    runs.sort_by(|a, b| b.len().cmp(&a.len()).then(a.low.cmp(&b.low)));
    runs.truncate(max);
    runs.sort_by_key(|r| r.low);
    runs
}
