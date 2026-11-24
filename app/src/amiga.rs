//! Amiga screen modes and their colour limits, so pictures stay compatible with the Amiga.
//!
//! The original chip sets limit how many bitplanes (colours) each resolution may have and how
//! precise the palette is: OCS/ECS have 4 bits per colour gun (4096 colours), low-res up to
//! 5 planes (32 colours, or 64 with Extra-Half-Brite), high-res up to 4 (16) and super-high-res
//! up to 2 (4). AGA has 8 bits per gun and up to 8 planes (256 colours) in every resolution.

use crate::palette::Palette;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chipset {
    /// Original and Enhanced chip sets: 4 bits per gun, depth limited by resolution.
    Ecs,
    /// Advanced Graphics Architecture: 8 bits per gun, 8 planes everywhere.
    Aga,
}

impl Chipset {
    pub fn label(self) -> &'static str {
        match self {
            Chipset::Ecs => "OCS/ECS",
            Chipset::Aga => "AGA",
        }
    }
}

/// Horizontal resolution class of a mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Res {
    Lores,
    Hires,
    SuperHires,
    /// Not an Amiga mode (PC and modern screens): always 256 colours, 8 bits per gun.
    Pc,
}

#[derive(Clone, Copy, Debug)]
pub struct DisplayMode {
    pub name: &'static str,
    pub w: i32,
    pub h: i32,
    pub res: Res,
    pub lace: bool,
    pub pal: bool,
    /// Extra-Half-Brite: 6 planes, colours 32-63 are colours 0-31 at half brightness.
    pub ehb: bool,
}

const fn m(name: &'static str, w: i32, h: i32, res: Res, lace: bool, pal: bool, ehb: bool) -> DisplayMode {
    DisplayMode { name, w, h, res, lace, pal, ehb }
}

pub const MODES: [DisplayMode; 22] = [
    m("Low-res", 320, 200, Res::Lores, false, false, false),
    m("Low-res Laced", 320, 400, Res::Lores, true, false, false),
    m("Low-res EHB", 320, 200, Res::Lores, false, false, true),
    m("PAL Low-res", 320, 256, Res::Lores, false, true, false),
    m("PAL Low-res Laced", 320, 512, Res::Lores, true, true, false),
    m("PAL Low-res EHB", 320, 256, Res::Lores, false, true, true),
    m("Hi-res", 640, 200, Res::Hires, false, false, false),
    m("Hi-res Laced", 640, 400, Res::Hires, true, false, false),
    m("PAL Hi-res", 640, 256, Res::Hires, false, true, false),
    m("PAL Hi-res Laced", 640, 512, Res::Hires, true, true, false),
    m("Super-Hi-res", 1280, 200, Res::SuperHires, false, false, false),
    m("Super-Hi-res Laced", 1280, 400, Res::SuperHires, true, false, false),
    m("PAL Super-Hi-res", 1280, 256, Res::SuperHires, false, true, false),
    m("PAL Super-Hi-res Laced", 1280, 512, Res::SuperHires, true, true, false),
    m("VGA", 640, 480, Res::Pc, false, false, false),
    m("VGA Low-res", 320, 240, Res::Pc, false, false, false),
    m("SVGA", 800, 600, Res::Pc, false, false, false),
    m("XGA", 1024, 768, Res::Pc, false, false, false),
    m("HD", 1280, 720, Res::Pc, false, false, false),
    m("Full HD", 1920, 1080, Res::Pc, false, false, false),
    m("WQHD", 2560, 1440, Res::Pc, false, false, false),
    m("4K UHD", 3840, 2160, Res::Pc, false, false, false),
];

impl DisplayMode {
    pub fn amiga(&self) -> bool {
        self.res != Res::Pc
    }

    /// Most bitplanes this mode allows.
    pub fn max_depth(&self, cs: Chipset) -> u32 {
        if !self.amiga() || cs == Chipset::Aga {
            return if self.ehb { 6 } else { 8 };
        }
        match (self.res, self.ehb) {
            (Res::Lores, true) => 6,
            (Res::Lores, false) => 5,
            (Res::Hires, _) => 4,
            _ => 2,
        }
    }

    pub fn max_colors(&self, cs: Chipset) -> usize {
        1 << self.max_depth(cs)
    }

    /// Bits per colour gun of the palette.
    pub fn gun_bits(&self, cs: Chipset) -> u32 {
        if self.amiga() && cs == Chipset::Ecs {
            4
        } else {
            8
        }
    }

    /// The Amiga display mode id stored in a picture's CAMG chunk.
    pub fn mode_id(&self) -> u32 {
        if !self.amiga() {
            return 0;
        }
        let monitor: u32 = if self.pal { 0x0002_1000 } else { 0x0001_1000 };
        let res: u32 = match self.res {
            Res::Hires => 0x8000,
            Res::SuperHires => 0x8020,
            _ => 0,
        };
        monitor | res | if self.lace { 0x4 } else { 0 } | if self.ehb { 0x80 } else { 0 }
    }
}

/// The mode a picture was made for, from its CAMG mode id (or its size when it has none).
pub fn mode_for(camg: Option<u32>, w: i32, h: i32, depth: u32) -> usize {
    if let Some(id) = camg.filter(|c| *c != 0) {
        let res = if id & 0x8000 != 0 {
            if id & 0x20 != 0 {
                Res::SuperHires
            } else {
                Res::Hires
            }
        } else {
            Res::Lores
        };
        let lace = id & 0x4 != 0;
        let ehb = id & 0x80 != 0 && depth == 6;
        let pal = id & 0xFFFF_1000 == 0x0002_1000 || (h > 200 && !lace && h <= 256) || (lace && h > 400);
        if let Some(i) = MODES.iter().position(|m| m.res == res && m.lace == lace && m.ehb == ehb && m.pal == pal) {
            return i;
        }
        if let Some(i) = MODES.iter().position(|m| m.res == res && m.lace == lace && m.ehb == ehb) {
            return i;
        }
    }
    if let Some(i) = MODES.iter().position(|m| (m.w, m.h) == (w, h) && (m.ehb == (depth == 6))) {
        return i;
    }
    MODES.iter().position(|m| (m.w, m.h) == (w, h)).unwrap_or(14)
}

/// Round a palette value to what a gun with `bits` bits can show.
pub fn gun(v: u8, bits: u32) -> u8 {
    if bits >= 8 {
        v
    } else {
        (((v as u32 + 8) / 17).min(15) * 17) as u8
    }
}

/// Make a palette obey the screen: gun precision, and Extra-Half-Brite's derived colours.
pub fn limit_palette(pal: &mut Palette, colors: usize, ehb: bool, gun_bits: u32) {
    let n = colors.min(256);
    for c in pal.iter_mut().take(n) {
        for v in c.iter_mut() {
            *v = gun(*v, gun_bits);
        }
    }
    if ehb {
        for i in 0..32 {
            let c = pal[i];
            pal[32 + i] = if gun_bits < 8 {
                // Half brightness of the 4-bit value, as the hardware does it.
                [(c[0] / 17 >> 1) * 17, (c[1] / 17 >> 1) * 17, (c[2] / 17 >> 1) * 17]
            } else {
                [c[0] >> 1, c[1] >> 1, c[2] >> 1]
            };
        }
    }
}

static ACTIVE: AtomicUsize = AtomicUsize::new(256);

/// How many palette colours painting may use (the screen's palette size).
pub fn active_colors() -> usize {
    ACTIVE.load(Ordering::Relaxed).clamp(2, 256)
}

pub fn set_active_colors(n: usize) {
    ACTIVE.store(n.clamp(2, 256), Ordering::Relaxed);
}

/// Bitplanes needed for `colors` colours.
pub fn depth_of(colors: usize) -> u32 {
    let mut d = 1;
    while (1usize << d) < colors && d < 8 {
        d += 1;
    }
    d
}
