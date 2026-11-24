//! Prefs menu settings, saved to and restored from a small text file.

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    /// Show the cursor position in the title bar.
    pub coords: bool,
    /// Fast feedback: preview lines and outlines with a one-pixel brush while dragging.
    pub fast_fb: bool,
    /// Cycle mode cycles each colour of a multicoloured brush through its own range.
    pub multi_cycle: bool,
    /// Rectangles become squares and ellipses circles.
    pub be_square: bool,
    /// Run in a window (on) rather than filling the screen (off).
    pub workbench: bool,
    /// With the grid on, picked-up brushes leave out their right and bottom edge pixels.
    pub excl_brush: bool,
    /// Picked-up brushes take their transparent colour from matching corners.
    pub auto_transp: bool,
    /// Picking up a brush sets the grid spacing to the brush size.
    pub auto_grid: bool,
    /// Coordinates counted from the upper-left corner instead of the lower-left.
    pub origin_ul: bool,
    /// Smooth freehand lines by averaging mouse movements.
    pub mouse_track: bool,
    pub font_dir: Option<PathBuf>,
    pub macro_paths: Vec<Option<PathBuf>>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            coords: false,
            fast_fb: false,
            multi_cycle: false,
            be_square: false,
            workbench: true,
            excl_brush: false,
            auto_transp: true,
            auto_grid: true,
            origin_ul: false,
            mouse_track: false,
            font_dir: None,
            macro_paths: vec![None; 10],
        }
    }
}

pub fn prefs_dir() -> PathBuf {
    std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir()).join("DeluxePixel")
}

fn prefs_file() -> PathBuf {
    prefs_dir().join("prefs.txt")
}

impl Prefs {
    pub fn to_text(&self) -> String {
        let b = |v: bool| if v { "1" } else { "0" };
        let mut s = format!(
            "coords={}\nfast_fb={}\nmulti_cycle={}\nbe_square={}\nworkbench={}\nexcl_brush={}\nauto_transp={}\nauto_grid={}\norigin_ul={}\nmouse_track={}\n",
            b(self.coords),
            b(self.fast_fb),
            b(self.multi_cycle),
            b(self.be_square),
            b(self.workbench),
            b(self.excl_brush),
            b(self.auto_transp),
            b(self.auto_grid),
            b(self.origin_ul),
            b(self.mouse_track)
        );
        if let Some(d) = &self.font_dir {
            s += &format!("font_dir={}\n", d.display());
        }
        for (i, p) in self.macro_paths.iter().enumerate() {
            if let Some(p) = p {
                s += &format!("macro{}={}\n", i + 1, p.display());
            }
        }
        s
    }

    pub fn from_text(t: &str) -> Prefs {
        let mut p = Prefs::default();
        for l in t.lines() {
            let Some((k, v)) = l.split_once('=') else { continue };
            let on = v.trim() == "1";
            match k.trim() {
                "coords" => p.coords = on,
                "fast_fb" => p.fast_fb = on,
                "multi_cycle" => p.multi_cycle = on,
                "be_square" => p.be_square = on,
                "workbench" => p.workbench = on,
                "excl_brush" => p.excl_brush = on,
                "auto_transp" => p.auto_transp = on,
                "auto_grid" => p.auto_grid = on,
                "origin_ul" => p.origin_ul = on,
                "mouse_track" => p.mouse_track = on,
                "font_dir" => p.font_dir = Some(PathBuf::from(v.trim())),
                k if k.starts_with("macro") => {
                    if let Ok(n) = k[5..].parse::<usize>() {
                        if (1..=10).contains(&n) {
                            p.macro_paths[n - 1] = Some(PathBuf::from(v.trim()));
                        }
                    }
                }
                _ => {}
            }
        }
        p
    }

    /// The settings saved with Prefs > Save, or the defaults.
    pub fn load_saved() -> Prefs {
        std::fs::read_to_string(prefs_file()).map(|t| Prefs::from_text(&t)).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        std::fs::create_dir_all(prefs_dir()).map_err(|e| e.to_string())?;
        std::fs::write(prefs_file(), self.to_text()).map_err(|e| format!("Can't save the preferences: {e}"))
    }
}
