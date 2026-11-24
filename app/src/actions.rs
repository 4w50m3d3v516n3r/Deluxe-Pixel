//! Every menu command, shared by the menus, the keyboard and macros, and the state the
//! DeluxePaint V-style functions need.

use crate::app::{App, Pending, Tool};
use crate::convert::ConvertOptions;
use crate::doc::{Document, Stencil};
use crate::macros::{Macro, MacroEv};
use crate::paint::{self, Dither, FillKind, Process, Rng, Stroke, Texture};
use crate::palette::{nearest, Palette, Range};
use crate::pix::{Brush, BuiltinShape, Image};
use crate::prefs::Prefs;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

macro_rules! commands {
    ($($n:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Cmd { $($n),* }
        impl Cmd {
            pub const ALL: &'static [Cmd] = &[$(Cmd::$n),*];
            pub fn name(&self) -> &'static str { match self { $(Cmd::$n => stringify!($n)),* } }
            pub fn from_name(s: &str) -> Option<Cmd> { Cmd::ALL.iter().copied().find(|c| c.name() == s) }
        }
    };
}

commands!(
    // Picture
    Load, Save, SaveAs, DeleteFile, Print, FlipPicH, FlipPicV, SpareSwap, SpareCopy, MergeFront, MergeBack, SpareDelete,
    PageSize, ShowPage, ScreenFormat, About, Quit, Undo, Redo, Clear,
    // Brush
    BrushLoad, BrushSave, BrushCopy, BrushPaste, BrushRestore, BrushToSpare, BrushSwapSpare, BrushMorph, BrushStretch,
    BrushHalve, BrushDouble, BrushDoubleH, BrushDoubleV, BrushFlipH, BrushFlipV, BrushOutline, BrushTrim, BrushRot90,
    BrushRotAny, BrushShear, BrushBendH, BrushBendV, HandleRotate, HandlePlace, SolidBody, FrameOnly, BrushFree,
    // Anim
    AnimLoad, AnimSave, Move, CameraMove, AnimBrushLoad, AnimBrushSave, AnimBrushPick, AnimBrushSettings, AnimBrushUse,
    AnimBrushFree, FramesSet, FramesAdd, FramesCopy, FramesDelete, FrameRate, FramePalette, FramePaletteInfo, Panel,
    SetRange, PrevFrame, NextFrame, GoTo, Play, PlayOnce, PingPong, PlayBackLoop, PlayBackOnce, Stop, AddFrame, DeleteFrame,
    // Colour
    Ranges, Cycle, Mixer, Arrange, UseBrushPalette, RestorePalette, DefaultPalette, PaletteLoad, PaletteSave, ColourSetLoad,
    ColourSetSave, BgToFg, BgSwapFg, Remap, BrushBgToFg, BrushBgSwapFg, BrushRemap, BrushTransparency, RangeNext, RangePrev,
    RangeRev,
    // Effect
    StencilShow, StencilMake, StencilRemake, StencilPaint, StencilReverse, StencilToggle, StencilFree, StencilLoad,
    StencilSave, LightTableToggle, LightTableMerge, LightTableDim, Layer1, Layer2, Layer3, Layer4, LightTableSettings,
    BgFix, BgFree, LockFg, AaNone, AaLow, AaHigh, AaCycle, ProcessToggle, ProcessTint, ProcessHue, ProcessValue,
    TranslucentToggle, TranslucentSettings, PerspDo, PerspFillScreen, PerspReset, PerspCentre, PerspSettings,
    PressureToggle, PressureSettings, MediaOpen, MediaClose, TextureToggle, TextureLoad, TextureInvert, TextureFree,
    // Macro
    MacroRecord, MacroDetails, MacroExecute, MacroLoad, MacroSaveAs,
    // Prefs
    PrefCoords, PrefFastFb, PrefMultiCycle, PrefBeSquare, PrefWorkbench, PrefExclBrush, PrefAutoTransp, PrefAutoGrid,
    PrefOriginUl, PrefMouseTrack, ToolAirbrush, ToolGrid, ToolSymmetry, ToolFill, ToolSpacing, ToolFonts, FontDir,
    PrefsDefault, PrefsLastSaved, PrefsSave,
    // Extras
    Fullscreen, Gallery, Camera, Convert, Keys, InfoBar, Magnify,
);

impl Cmd {
    /// Keyboard equivalent shown in the menus. The original's Amiga key is Ctrl here,
    /// and its Ctrl key is Ctrl+Shift.
    pub fn shortcut(&self) -> &'static str {
        use Cmd::*;
        match self {
            Load => "Ctrl+L",
            Save => "Ctrl+S",
            DeleteFile => "Ctrl+D",
            Print => "Ctrl+P",
            SpareSwap => "j",
            SpareCopy => "Ctrl+Shift+J",
            SpareDelete => "Ctrl+-",
            ShowPage => "S",
            About => "?",
            Quit => "Ctrl+Q",
            Undo => "u",
            Clear => "K",
            BrushLoad => "Ctrl+B",
            BrushCopy => "Ctrl+C",
            BrushPaste => "Ctrl+V",
            BrushRestore => "B",
            BrushToSpare => "Alt+N",
            BrushSwapSpare => "Alt+B",
            BrushStretch => "Z",
            BrushHalve => "h",
            BrushDouble => "H",
            BrushDoubleH => "X",
            BrushDoubleV => "Y",
            BrushFlipH => "x",
            BrushFlipV => "y",
            BrushOutline => "o",
            BrushTrim => "O",
            BrushRot90 => "z",
            HandleRotate => "Alt+X",
            HandlePlace => "Alt+Z",
            FrameOnly => "Alt+F",
            AnimLoad => "Ctrl+O",
            AnimSave => "Ctrl+A",
            Move => "M",
            CameraMove => "N",
            FramesCopy => "Ctrl+Shift+F",
            FrameRate => "Ctrl+R",
            Panel => "Alt+A",
            PrevFrame => "1",
            NextFrame => "2",
            GoTo => "3",
            Play => "4",
            PlayOnce => "5",
            PingPong => "6",
            Ranges => "Ctrl+Shift+R",
            Cycle => "Tab",
            Mixer => "p",
            Arrange => "P",
            RangeRev => "Alt+R",
            RangeNext => "Alt+]",
            RangePrev => "Alt+[",
            StencilMake => "~",
            StencilPaint => "Ctrl+Shift+S",
            StencilToggle => "`",
            LightTableToggle => "|",
            LightTableDim => "Alt+L",
            Layer1 => "Ctrl+Shift+1",
            Layer2 => "Ctrl+Shift+2",
            Layer3 => "Ctrl+Shift+3",
            Layer4 => "Ctrl+Shift+4",
            LightTableSettings => "Ctrl+Shift+L",
            AaCycle => "Alt+/",
            ProcessToggle => "Alt+P",
            TranslucentToggle => "Alt+T",
            TranslucentSettings => "Ctrl+Shift+T",
            PressureSettings => "Ctrl+Shift+P",
            TextureToggle => "Ctrl+T",
            TextureInvert => "Ctrl+I",
            MacroRecord => "Ctrl+M",
            PrefCoords => "\\",
            ToolAirbrush => "Alt+A",
            ToolFill => "F",
            ToolSpacing => "V",
            ToolFonts => "T",
            Fullscreen => "F11",
            Camera => "Alt+C",
            InfoBar => "F9",
            Magnify => "m",
            _ => "",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    Loop,
    Once,
    PingPong,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LtLayer {
    pub on: bool,
    /// 0 = spare page, -1 = previous frame, 1 = next frame...
    pub offset: i32,
    /// Amount of black added (-255..=255).
    pub dim: i32,
}

/// State for the DeluxePaint V functions.
pub struct V5 {
    pub prefs: Prefs,
    pub spare_brush: Option<Brush>,
    pub brush_palette: Option<Palette>,
    pub frame_only: bool,
    pub show_handle: bool,
    pub lt_layers: [LtLayer; 4],
    pub lt_dim: bool,
    pub lt_anim_lock: bool,
    pub show_lt_settings: bool,
    pub stencil_show: bool,
    pub stencil_tolerance: u8,
    pub stencil_colors: Vec<bool>,
    pub show_ranges: bool,
    pub ranges_backup: Option<Vec<Range>>,
    pub ranges_undo: Option<Vec<Range>>,
    pub range_select_from: Option<u8>,
    pub range_grid: bool,
    pub range_fade: [u8; 8],
    pub range_page: usize,
    pub show_anim_panel: bool,
    pub play_mode: PlayMode,
    pub play_dir: i32,
    pub show_goto: bool,
    pub goto: usize,
    pub show_set_range: bool,
    pub range_ab: (usize, usize),
    pub show_frames_set: bool,
    pub frames_n: usize,
    pub show_frames_add: bool,
    pub add_n: usize,
    pub show_frames_copy: bool,
    pub copy_abc: (usize, usize, usize),
    pub show_frames_delete: bool,
    pub del_ab: (usize, usize),
    pub show_frame_rate: bool,
    pub show_camera_move: bool,
    pub cam: (i32, i32, usize, bool),
    pub show_morph: bool,
    pub morph_frames: usize,
    pub show_animbrush_settings: bool,
    /// Anim brush cell order: 1 forward, -1 backward.
    /// AnimBrush Settings being edited: (duration, current cel from 1, direction).
    pub ab_edit: Option<(u32, usize, i32)>,
    pub media: Option<&'static str>,
    pub show_media: bool,
    pub pressure_on: bool,
    pub pressure_size: bool,
    pub pressure_reverse: bool,
    pub pressure_amount: i32,
    pub show_pressure: bool,
    pub recording: Option<Macro>,
    pub record_details: bool,
    pub slots: Vec<Option<Macro>>,
    pub assign: Option<Macro>,
    pub assign_slot: usize,
    pub last_macro: Option<Macro>,
    pub replay: VecDeque<MacroEv>,
    pub show_macro_load: bool,
    pub macro_load_slot: usize,
    pub show_screen_format: bool,
    pub sf_mode: usize,
    pub sf_screen_kind: usize,
    pub sf_screen: (i32, i32),
    pub sf_page_kind: usize,
    pub sf_page: (i32, i32),
    pub sf_colors: usize,
    /// Amiga chip set the screen format follows (colour limits per resolution, palette precision).
    pub chipset: crate::amiga::Chipset,
    pub sf_chipset: crate::amiga::Chipset,
    pub sf_retain: bool,
    pub show_arrange: bool,
    pub arrange_sel: Option<u8>,
    pub fill_backup: Option<(FillKind, f32, bool, Dither, u8, usize)>,
    pub show_fill_v5: bool,
    pub coord_origin: Option<(i32, i32)>,
    pub persp_place: bool,
    pub persp_centre: Option<(i32, i32)>,
    pub track: Option<(f32, f32)>,
    pub confirm_delete: Option<PathBuf>,
    pub show_about_v5: bool,
    pub clipboard_brush: Option<Brush>,
    /// Mouse-wheel movement not yet turned into a zoom step.
    pub wheel_acc: f32,
    /// A range fill waiting for its direction line.
    pub grad_line: Option<GradLine>,
    /// DeluxePaint 5.2 functions (frame scopes, camera, move options, scripts...).
    pub x: crate::v52::V52,
    /// The screen format: the resolution the whole window stands for. At 100% one picture pixel
    /// is one screen pixel, as on the original's screen.
    pub screen: (i32, i32),
    /// Screen-pixel size (in points) last frame, to keep the zoom percentage when the window changes.
    pub last_screen_zoom: f32,
}

/// A range fill whose direction is being set with the directional line.
pub struct GradLine {
    pub mask: Vec<bool>,
    pub base: Image,
    pub color: u8,
    /// The fixed end of the line: the centre of the area.
    pub centre: (i32, i32),
    /// The moving end of the line: follows the pointer.
    pub end: (i32, i32),
    /// A button has been pressed for this line; releasing it does the fill.
    pub armed: bool,
}

impl V5 {
    pub fn new() -> V5 {
        let prefs = Prefs::load_saved();
        let mut slots: Vec<Option<Macro>> = vec![None; 10];
        for (i, p) in prefs.macro_paths.iter().enumerate() {
            if let Some(p) = p {
                slots[i] = Macro::load(p).ok();
            }
        }
        V5 {
            prefs,
            spare_brush: None,
            brush_palette: None,
            frame_only: false,
            show_handle: false,
            lt_layers: [
                LtLayer { on: true, offset: -1, dim: 96 },
                LtLayer { on: false, offset: 1, dim: 128 },
                LtLayer { on: false, offset: -2, dim: 160 },
                LtLayer { on: false, offset: 0, dim: 128 },
            ],
            lt_dim: true,
            lt_anim_lock: false,
            show_lt_settings: false,
            stencil_show: false,
            stencil_tolerance: 0,
            stencil_colors: vec![false; 256],
            show_ranges: false,
            ranges_backup: None,
            ranges_undo: None,
            range_select_from: None,
            range_grid: false,
            range_fade: [0; 8],
            range_page: 0,
            show_anim_panel: false,
            play_mode: PlayMode::Loop,
            play_dir: 1,
            show_goto: false,
            goto: 1,
            show_set_range: false,
            range_ab: (1, 1),
            show_frames_set: false,
            frames_n: 8,
            show_frames_add: false,
            add_n: 1,
            show_frames_copy: false,
            copy_abc: (1, 1, 1),
            show_frames_delete: false,
            del_ab: (1, 1),
            show_frame_rate: false,
            show_camera_move: false,
            cam: (8, 0, 8, true),
            show_morph: false,
            morph_frames: 8,
            show_animbrush_settings: false,
            ab_edit: None,
            media: None,
            show_media: false,
            pressure_on: false,
            pressure_size: true,
            pressure_reverse: false,
            pressure_amount: 8,
            show_pressure: false,
            recording: None,
            record_details: true,
            slots,
            assign: None,
            assign_slot: 0,
            last_macro: None,
            replay: VecDeque::new(),
            show_macro_load: false,
            macro_load_slot: 0,
            show_screen_format: false,
            sf_mode: 14,
            sf_screen_kind: 0,
            sf_screen: (640, 480),
            sf_page_kind: 0,
            sf_page: (640, 480),
            sf_colors: 256,
            chipset: crate::amiga::Chipset::Aga,
            sf_chipset: crate::amiga::Chipset::Aga,
            sf_retain: true,
            show_arrange: false,
            arrange_sel: None,
            fill_backup: None,
            show_fill_v5: false,
            coord_origin: None,
            persp_place: false,
            persp_centre: None,
            track: None,
            confirm_delete: None,
            show_about_v5: false,
            clipboard_brush: None,
            wheel_acc: 0.0,
            grad_line: None,
            x: crate::v52::V52::default(),
            screen: (640, 480),
            last_screen_zoom: 0.0,
        }
    }
}

/// Media library presets: a paper texture with a matching translucency.
pub const MEDIA: [(&str, u8, u32, u8); 4] = [("Chalk", 0, 3, 45), ("Felt tip", 30, 0, 0), ("Oil pastel", 10, 2, 70), ("Water-colour", 60, 1, 85)];

fn media_texture(grain: u32, density: u8, seed: u64) -> Image {
    let mut rng = Rng::new(seed);
    let mut img = Image::new(64, 64, 1);
    if grain == 0 {
        return img;
    }
    for y in 0..64 {
        for x in 0..64 {
            // Grainy paper: random with some horizontal fibre.
            let r = rng.f() * 100.0 + ((x / grain as i32 + y) % 7) as f32 * 2.0;
            img.set(x, y, if r < density as f32 { 1 } else { 0 });
        }
    }
    img
}

impl App {
    /// Does the current fill need a directional line? Every range (gradient) fill does.
    pub fn needs_grad_line(&self) -> bool {
        self.set.fill.is_gradient()
    }

    /// Begin a range fill of `mask`: the directional line runs from the centre of the area to the
    /// pointer, and the fill is made when the mouse button is released. With `armed` the button is
    /// already down (Fill tool); otherwise the next click sets the direction (filled shapes).
    pub fn begin_grad_line(&mut self, mask: Vec<bool>, base: Image, color: u8, armed: bool, at: (i32, i32)) {
        let (w, h) = (self.doc.w, self.doc.h);
        let (mut sx, mut sy, mut n) = (0i64, 0i64, 0i64);
        for y in 0..h {
            for x in 0..w {
                if mask[(y * w + x) as usize] {
                    sx += x as i64;
                    sy += y as i64;
                    n += 1;
                }
            }
        }
        if n == 0 {
            self.doc.undo.pop();
            return;
        }
        let centre = ((sx / n) as i32, (sy / n) as i32);
        // Show the outline of the area while the direction is chosen (nothing is filled yet).
        let mut shown = base.clone();
        if !armed {
            let fg = self.fg;
            for y in 0..h {
                for x in 0..w {
                    let i = (y * w + x) as usize;
                    if mask[i] {
                        let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1 || !mask[i - 1] || !mask[i + 1] || !mask[i - w as usize] || !mask[i + w as usize];
                        if edge {
                            shown.px[i] = fg;
                        }
                    }
                }
            }
        }
        *self.doc.img_mut() = shown;
        self.v5.grad_line = Some(GradLine { mask, base, color, centre, end: at, armed });
        self.status = if armed {
            "Move to set the direction of the range, then release the button to fill".into()
        } else {
            "Point to set the direction of the range, then click to fill".into()
        };
    }

    /// Make the fill with the directional line as it is now.
    pub fn finish_grad_line(&mut self) {
        let Some(gl) = self.v5.grad_line.take() else { return };
        let mut set = self.set.clone();
        let c = (gl.centre.0 as f32 + 0.5, gl.centre.1 as f32 + 0.5);
        let e = (gl.end.0 as f32 + 0.5, gl.end.1 as f32 + 0.5);
        set.grad_line = Some((c, e));
        *self.doc.img_mut() = gl.base;
        let b = self.brush.clone();
        let mut rng = Rng::new(21);
        paint::fill_mask(&mut self.doc, &gl.mask, &set, gl.color, &b, &mut rng);
        self.status.clear();
        // A filled shape with an outline: the edge is traced after the range fill.
        if let Some((pts, primary)) = self.v5.x.outline_pending.take() {
            self.trace_outline(&pts, primary);
        }
    }

    /// Pointer handling while the directional line is shown. Returns true when it used the event.
    pub fn grad_line_pointer(&mut self, pos: (i32, i32), pressed: bool, down: bool, released: bool, hovered: bool) -> bool {
        let (w, h) = (self.doc.w, self.doc.h);
        let Some(gl) = &mut self.v5.grad_line else { return false };
        // The line follows the pointer and may reach the edge of the picture.
        gl.end = (pos.0.clamp(0, w - 1), pos.1.clamp(0, h - 1));
        if pressed && hovered {
            gl.armed = true;
        }
        if gl.armed && released && !down {
            self.finish_grad_line();
        }
        true
    }

    /// Esc: leave the picture as it was before the fill.
    pub fn cancel_grad_line(&mut self) {
        if let Some(gl) = self.v5.grad_line.take() {
            *self.doc.img_mut() = gl.base;
            self.doc.undo.pop();
            self.status.clear();
        }
        self.v5.x.outline_pending = None;
    }

    pub fn record(&mut self, ev: MacroEv) {
        if let Some(m) = &mut self.v5.recording {
            m.events.push(ev);
        }
    }

    pub fn cmd(&mut self, ctx: &egui::Context, c: Cmd) {
        if !matches!(c, Cmd::MacroRecord | Cmd::MacroDetails | Cmd::MacroExecute | Cmd::MacroLoad | Cmd::MacroSaveAs) {
            self.record(MacroEv::Cmd(c));
        }
        use Cmd::*;
        match c {
            // ---------------------------------------------------------------- Picture
            Load => self.request(Pending::Open),
            Save => self.save(false),
            SaveAs => self.save(true),
            DeleteFile => {
                if let Some(p) = rfd::FileDialog::new().set_title("Delete Disk File").pick_file() {
                    self.v5.confirm_delete = Some(p);
                }
            }
            Print => self.with_scope(crate::v52::ScopeOp::Print),
            FlipPicH => self.with_scope(crate::v52::ScopeOp::FlipH),
            FlipPicV => self.with_scope(crate::v52::ScopeOp::FlipV),
            SpareSwap => {
                std::mem::swap(&mut self.doc, &mut self.spare);
                self.fit_pending = true;
            }
            SpareCopy => {
                self.spare = self.doc.clone();
                self.spare.path = None;
                self.status = "Copied the picture to the spare page".into();
            }
            MergeFront | MergeBack => self.merge_spare(c == MergeFront),
            SpareDelete => {
                let mut d = Document::new(self.doc.w, self.doc.h, self.bg);
                d.palette = self.doc.palette;
                self.spare = d;
                self.status = "Spare page cleared".into();
            }
            PageSize => {
                self.page_size = (self.doc.w, self.doc.h, false);
                self.show_page_size = true;
            }
            ShowPage => self.view_page = !self.view_page,
            ScreenFormat => self.open_screen_format(),
            About => self.v5.show_about_v5 = true,
            Quit => self.request(Pending::Quit),
            Undo => {
                self.doc.undo();
            }
            Redo => {
                self.doc.redo();
            }
            Clear => {
                self.doc.clear(self.bg);
                self.move_forget_end();
            },
            // ---------------------------------------------------------------- Brush
            BrushLoad => self.load_brush(),
            BrushSave => self.save_brush(),
            BrushCopy => self.brush_copy(),
            BrushPaste => self.brush_paste(),
            BrushRestore => self.restore_brush(),
            BrushToSpare => {
                self.v5.spare_brush = Some(self.brush.clone());
                self.status = "Brush copied to the spare brush".into();
            }
            BrushSwapSpare => {
                if let Some(sb) = self.v5.spare_brush.take() {
                    let cur = std::mem::replace(&mut self.brush, sb);
                    self.v5.spare_brush = Some(cur);
                } else {
                    self.msg("Brush", "There is no spare brush yet. Use Brush->Spare first.");
                }
            }
            BrushMorph => {
                if self.v5.spare_brush.is_none() || !self.brush.custom {
                    self.msg("Morph", "Pick up a brush, put it on the spare with Brush->Spare, then pick up the brush to morph into.");
                } else {
                    self.v5.show_morph = true;
                }
            }
            BrushStretch => {
                if self.brush.custom {
                    self.brush_size_edit = (self.brush.img.w, self.brush.img.h);
                    self.show_resize_brush = true;
                }
            }
            BrushHalve => self.transform_brush(|b| b.scaled(b.img.w / 2, b.img.h / 2)),
            BrushDouble => self.transform_brush(|b| b.scaled(b.img.w * 2, b.img.h * 2)),
            BrushDoubleH => self.transform_brush(|b| b.scaled(b.img.w * 2, b.img.h)),
            BrushDoubleV => self.transform_brush(|b| b.scaled(b.img.w, b.img.h * 2)),
            BrushFlipH => self.transform_brush(|b| b.flip_x()),
            BrushFlipV => self.transform_brush(|b| b.flip_y()),
            BrushOutline => {
                let fg = self.fg;
                self.transform_brush(|b| b.outlined(fg))
            }
            BrushTrim => self.transform_brush(trim_brush),
            BrushRot90 => self.transform_brush(|b| b.rot90()),
            BrushRotAny | BrushShear | BrushBendH | BrushBendV => {
                if self.brush.custom {
                    self.show_rotate = true;
                } else {
                    self.msg("Brush", "Can't transform built-in brush. Must have custom brush.");
                }
            }
            HandleRotate => {
                // In perspective mode the brush turns about its handle: move the handle of the flat brush.
                let in_persp = self.v5.x.persp_mode && self.v5.x.persp_base.is_some();
                if in_persp {
                    std::mem::swap(&mut self.brush, self.v5.x.persp_base.as_mut().unwrap());
                }
                let b = &mut self.brush;
                let (w, h) = (b.img.w, b.img.h);
                let next = if (b.hx, b.hy) == (w / 2, h / 2) {
                    (0, 0)
                } else if (b.hx, b.hy) == (0, 0) {
                    (w - 1, 0)
                } else if (b.hx, b.hy) == (w - 1, 0) {
                    (w - 1, h - 1)
                } else if (b.hx, b.hy) == (w - 1, h - 1) {
                    (0, h - 1)
                } else {
                    (w / 2, h / 2)
                };
                b.hx = next.0;
                b.hy = next.1;
                if in_persp {
                    std::mem::swap(&mut self.brush, self.v5.x.persp_base.as_mut().unwrap());
                    self.v5.x.p3.last_key = None;
                }
            }
            HandlePlace => self.v5.show_handle = true,
            // Solid Body: see-through pixels enclosed by colour become solid (the edge stays see-through).
            SolidBody => {
                self.v5.frame_only = false;
                if self.brush.custom {
                    self.transform_brush(crate::reqs8::solid_body);
                } else {
                    self.msg("Brush", "Must have custom brush.");
                }
            }
            FrameOnly => self.v5.frame_only = !self.v5.frame_only,
            BrushFree => self.set_builtin(BuiltinShape::Round, 1),
            // ---------------------------------------------------------------- Anim
            AnimLoad => self.request(Pending::Open),
            AnimSave => self.save_anim(),
            Move => self.show_move = true,
            CameraMove => {
                if self.spare.frames.len() == 1 && self.spare.img().px.iter().all(|&p| p == self.spare.img().px[0]) {
                    self.msg("Camera Move", "You must have a spare page to do a camera move.");
                } else {
                    self.v5.x.show_cam = true;
                }
            }
            AnimBrushLoad => self.anim_brush_load(),
            AnimBrushSave => self.anim_brush_save(),
            AnimBrushPick => {
                if self.last_pick.is_none() {
                    self.msg("AnimBrush", "Pick up a brush (b) first; the same area is then taken from every frame.");
                } else {
                    self.v5.x.pickup_count = self.doc.frames.len();
                    self.v5.x.show_pickup = true;
                }
            }
            AnimBrushSettings => self.v5.show_animbrush_settings = true,
            AnimBrushUse => {
                if self.anim_brush.is_none() {
                    self.pick_anim_brush();
                }
            }
            AnimBrushFree => self.anim_brush = None,
            FramesSet => {
                self.v5.frames_n = self.doc.frames.len();
                self.v5.show_frames_set = true;
            }
            FramesAdd => self.v5.show_frames_add = true,
            FramesCopy => self.open_copy(),
            FramesDelete => {
                let n = self.doc.cur + 1;
                self.v5.del_ab = (n, n);
                self.v5.show_frames_delete = true;
            }
            FrameRate => self.open_rate(),
            FramePalette => self.open_frame_palette(false),
            FramePaletteInfo => self.open_frame_palette(true),
            Panel => self.v5.show_anim_panel = !self.v5.show_anim_panel,
            SetRange => {
                let (a, b) = self.doc.play_range.unwrap_or((0, self.doc.frames.len() - 1));
                self.v5.range_ab = (a + 1, b + 1);
                self.v5.show_set_range = true;
            }
            PrevFrame => self.doc.prev_frame(),
            NextFrame => self.doc.next_frame(),
            GoTo => {
                self.v5.goto = self.doc.cur + 1;
                self.v5.show_goto = true;
            }
            Play => self.start_play(PlayMode::Loop, 1),
            PlayOnce => self.start_play(PlayMode::Once, 1),
            PingPong => self.start_play(PlayMode::PingPong, 1),
            PlayBackLoop => self.start_play(PlayMode::Loop, -1),
            PlayBackOnce => self.start_play(PlayMode::Once, -1),
            Stop => self.playing = false,
            AddFrame => self.doc.add_frame_after(),
            DeleteFrame => {
                if !self.doc.delete_frame() {
                    self.msg("Anim", "Can't delete only page.");
                }
            }
            // ---------------------------------------------------------------- Colour
            Ranges => {
                self.v5.show_ranges = !self.v5.show_ranges;
                if self.v5.show_ranges {
                    self.v5.ranges_backup = Some(self.doc.ranges.clone());
                }
            }
            Cycle => self.cycling = !self.cycling,
            // Arrange (Shift+P): the Mixer panel with seven rows of the colour set.
            Arrange => {
                let on = !(self.show_palette && self.v5.x.mixer.arrange);
                self.show_palette = on;
                self.v5.x.mixer.arrange = on;
            }
            Mixer => {
                self.show_palette = !(self.show_palette && !self.v5.x.mixer.arrange);
                self.v5.x.mixer.arrange = false;
            }
            UseBrushPalette => match self.v5.brush_palette {
                Some(p) => {
                    self.doc.checkpoint();
                    self.doc.palette = p;
                }
                None => self.msg("Palette", "The brush has no palette of its own (load a brush from a file first)."),
            },
            RestorePalette => {
                self.doc.checkpoint();
                self.doc.palette = self.doc.saved_palette;
            }
            DefaultPalette => {
                self.doc.checkpoint();
                self.doc.palette = crate::palette::DEFAULT_PALETTE;
            }
            PaletteLoad => self.palette_load(false),
            PaletteSave => self.palette_save(false),
            ColourSetLoad => self.palette_load(true),
            ColourSetSave => self.palette_save(true),
            BgToFg => self.with_scope(crate::v52::ScopeOp::BgToFg),
            BgSwapFg => self.with_scope(crate::v52::ScopeOp::SwapBgFg),
            Remap => match self.pal_undo {
                Some(old) => {
                    self.doc.checkpoint();
                    let new = self.doc.palette;
                    let map: Vec<u8> = (0..256).map(|i| nearest(&new, [old[i][0] as i32, old[i][1] as i32, old[i][2] as i32], 256)).collect();
                    for f in self.doc.frames.iter_mut() {
                        for p in f.px.iter_mut() {
                            *p = map[*p as usize];
                        }
                    }
                    self.pal_undo = None;
                }
                None => self.msg("Remap", "Remap matches the picture to the palette after you change it in the Mixer. Nothing to remap yet."),
            },
            BrushBgToFg | BrushBgSwapFg => {
                let (fg, bg) = (self.fg, self.bg);
                let swap = c == BrushBgSwapFg;
                self.transform_brush(|b| {
                    let mut n = b.clone();
                    for p in n.img.px.iter_mut() {
                        if *p == bg {
                            *p = fg;
                        } else if swap && *p == fg {
                            *p = bg;
                        }
                    }
                    n
                });
            }
            BrushRemap => {
                let from = self.v5.brush_palette.unwrap_or(self.spare.palette);
                let to = self.doc.palette;
                self.transform_brush(|b| b.remapped(&from, &to));
            }
            BrushTransparency => {
                self.brush.transparent = Some(self.bg);
            }
            RangeNext => self.set.range = (self.set.range + 1).min(7),
            RangePrev => self.set.range = self.set.range.saturating_sub(1),
            RangeRev => {
                let i = self.set.range;
                if let Some(r) = self.doc.ranges.get_mut(i) {
                    r.reverse = !r.reverse;
                }
            }
            // ---------------------------------------------------------------- Effect
            StencilShow => self.v5.stencil_show = !self.v5.stencil_show,
            StencilMake => self.show_stencil = true,
            StencilRemake => {
                let picked: [bool; 256] = std::array::from_fn(|i| self.v5.stencil_colors[i]);
                let locked = paint::colors_within(&self.doc.palette, &picked, self.v5.stencil_tolerance);
                let mask = paint::stencil_from_colors(self.doc.img(), &locked);
                self.doc.stencil = Some(Stencil { mask, on: true });
            }
            StencilPaint => {
                self.set.stencil_paint = !self.set.stencil_paint;
                self.status = if self.set.stencil_paint {
                    "Stencil Paint: paint the areas to protect (right button removes); choose Paint again to finish".into()
                } else {
                    "Stencil made from the painted areas".into()
                };
            }
            StencilReverse => {
                if let Some(s) = &mut self.doc.stencil {
                    s.mask.iter_mut().for_each(|m| *m = !*m);
                }
            }
            StencilToggle => match &mut self.doc.stencil {
                Some(s) => s.on = !s.on,
                None => self.show_stencil = true,
            },
            StencilFree => {
                self.doc.stencil = None;
                self.set.stencil_paint = false;
            }
            StencilLoad => self.load_stencil(),
            StencilSave => self.save_stencil(),
            LightTableToggle => self.light_table = !self.light_table,
            LightTableMerge => self.light_table_merge(),
            LightTableDim => self.v5.lt_dim = !self.v5.lt_dim,
            Layer1 | Layer2 | Layer3 | Layer4 => {
                let i = match c {
                    Layer1 => 0,
                    Layer2 => 1,
                    Layer3 => 2,
                    _ => 3,
                };
                self.v5.lt_layers[i].on = !self.v5.lt_layers[i].on;
                self.light_table = true;
            }
            LightTableSettings => self.v5.show_lt_settings = true,
            BgFix => {
                self.doc.background = Some(self.doc.img().clone());
                self.status = "Background fixed: CLR now returns to this picture".into();
            }
            BgFree => self.doc.background = None,
            LockFg => match &self.doc.background {
                Some(bgimg) => {
                    let mask: Vec<bool> = self.doc.img().px.iter().zip(bgimg.px.iter()).map(|(a, b)| a != b).collect();
                    self.doc.stencil = Some(Stencil { mask, on: true });
                }
                None => self.lock_foreground(),
            },
            AaNone => self.antialias = false,
            AaLow => {
                self.antialias = true;
                self.aa_high = false;
            }
            AaHigh => {
                self.antialias = true;
                self.aa_high = true;
            }
            AaCycle => {
                if !self.antialias {
                    self.antialias = true;
                    self.aa_high = false;
                } else if !self.aa_high {
                    self.aa_high = true;
                } else {
                    self.antialias = false;
                }
            }
            ProcessToggle => self.set.process_on = !self.set.process_on,
            ProcessTint => self.set.process = Process::Tint,
            ProcessHue => self.set.process = Process::Hue,
            ProcessValue => self.set.process = Process::Value,
            TranslucentToggle => self.set.translucent_on = !self.set.translucent_on,
            TranslucentSettings => self.show_translucent = true,
            PerspDo => {
                if self.v5.x.persp_mode {
                    self.exit_persp();
                } else {
                    self.enter_persp();
                }
            }
            PerspFillScreen => {
                if self.brush.custom || self.v5.x.persp_base.is_some() {
                    // The brush repeats over its plane in 3-D, into the depth up to the horizon.
                    self.cmd_fill_screen();
                } else {
                    self.msg("Perspective", "Must have custom brush.");
                }
            }
            PerspReset => {
                // Every perspective setting back to its default, the angle type too.
                let depth_default = crate::v52::V52::default().persp_depth;
                self.v5.x.p3 = crate::persp3d::Persp3::default();
                self.v5.x.persp_depth = depth_default;
                self.v5.persp_centre = Some((self.doc.w / 2, self.doc.h / 2));
                self.persp = (0.0, 0.0, 0.0);
                if let Some(b) = self.v5.x.persp_base.clone() {
                    self.brush = b;
                } else {
                    self.restore_brush();
                }
            }
            PerspCentre => {
                self.v5.persp_place = true;
                self.status = "Click on the picture to place the perspective centre".into();
            }
            PerspSettings => self.show_perspective = true,
            PressureToggle => self.v5.pressure_on = !self.v5.pressure_on,
            PressureSettings => self.v5.show_pressure = true,
            MediaOpen => self.v5.show_media = true,
            MediaClose => {
                self.v5.media = None;
                self.set.texture = None;
                self.set.texture_on = false;
            }
            TextureToggle => {
                if self.set.texture.is_none() {
                    self.texture_load();
                } else {
                    self.set.texture_on = !self.set.texture_on;
                }
            }
            TextureLoad => self.texture_load(),
            TextureInvert => {
                if let Some(t) = &mut self.set.texture {
                    t.invert = !t.invert;
                }
            }
            TextureFree => {
                self.set.texture = None;
                self.set.texture_on = false;
            }
            // ---------------------------------------------------------------- Macro
            MacroRecord => {
                if let Some(m) = self.v5.recording.take() {
                    self.v5.assign_slot = self.v5.slots.iter().position(|s| s.is_none()).unwrap_or(0);
                    self.v5.assign = Some(m);
                } else {
                    let n = self.v5.slots.iter().filter(|s| s.is_some()).count() + 1;
                    self.v5.recording = Some(Macro { name: format!("Macro {n}"), events: Vec::new() });
                    self.status = "Recording a macro — choose Record Macro (Ctrl+M) again to stop".into();
                }
            }
            MacroDetails => self.v5.record_details = !self.v5.record_details,
            MacroExecute => {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Execute Macro")
                    .add_filter("Macro or ARexx script", &["dpm", "rexx", "dprx", "txt"])
                    .pick_file()
                {
                    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                    if ext == "dpm" {
                        match Macro::load(&p) {
                            Ok(m) => self.run_macro(&m),
                            Err(e) => self.msg("Execute Macro", e),
                        }
                    } else {
                        match std::fs::read_to_string(&p) {
                            Ok(t) => {
                                self.run_script(ctx, &t);
                            }
                            Err(e) => self.msg("Execute Macro", format!("Can't Load: {e}")),
                        }
                    }
                }
            }
            MacroLoad => self.v5.show_macro_load = true,
            MacroSaveAs => match self.v5.last_macro.clone() {
                Some(m) => {
                    if let Some(p) = rfd::FileDialog::new().set_title("Save Macro").set_file_name("macro.dpm").add_filter("Macro", &["dpm"]).save_file() {
                        if let Err(e) = m.save(&p) {
                            self.msg("Save Macro", e);
                        }
                    }
                }
                None => self.msg("Save Macro", "Record a macro first (Macro > Record Macro)."),
            },
            // ---------------------------------------------------------------- Prefs
            PrefCoords => self.v5.prefs.coords = !self.v5.prefs.coords,
            PrefFastFb => self.v5.prefs.fast_fb = !self.v5.prefs.fast_fb,
            PrefMultiCycle => {
                self.v5.prefs.multi_cycle = !self.v5.prefs.multi_cycle;
                self.multi_cycle = self.v5.prefs.multi_cycle;
            }
            PrefBeSquare => self.v5.prefs.be_square = !self.v5.prefs.be_square,
            PrefWorkbench => {
                self.v5.prefs.workbench = !self.v5.prefs.workbench;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!self.v5.prefs.workbench));
                self.fit_pending = true;
            }
            PrefExclBrush => self.v5.prefs.excl_brush = !self.v5.prefs.excl_brush,
            PrefAutoTransp => self.v5.prefs.auto_transp = !self.v5.prefs.auto_transp,
            PrefAutoGrid => self.v5.prefs.auto_grid = !self.v5.prefs.auto_grid,
            PrefOriginUl => self.v5.prefs.origin_ul = !self.v5.prefs.origin_ul,
            PrefMouseTrack => self.v5.prefs.mouse_track = !self.v5.prefs.mouse_track,
            ToolAirbrush => self.show_airbrush = true,
            ToolGrid => self.show_grid = true,
            ToolSymmetry => self.show_symmetry = true,
            ToolFill => self.open_fill(),
            ToolSpacing => self.show_spacing = true,
            ToolFonts => self.show_font = true,
            FontDir => {
                if let Some(dir) = rfd::FileDialog::new().set_title("Font Directory").pick_folder() {
                    let mut fonts = crate::text::fonts_in(&dir);
                    if fonts.is_empty() {
                        self.msg("Font Dir", "No fonts were found in that folder (Amiga fonts in their own folders, or TrueType/OpenType files).");
                    } else {
                        fonts.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                        let n = fonts.len();
                        let amiga = fonts.iter().filter(|f| f.is_bitmap()).count();
                        self.fonts = fonts;
                        self.font_idx = 0;
                        if let Some((s, _)) = self.fonts[0].nearest_size(self.text_style.size) {
                            self.text_style.size = *s;
                        }
                        self.font = crate::text::load_entry(&self.fonts[0], self.text_style.size);
                        self.status = format!("{n} fonts ({amiga} Amiga bitmap fonts)");
                        self.show_font = true;
                        self.v5.prefs.font_dir = Some(dir);
                    }
                }
            }
            PrefsDefault => {
                let macros = self.v5.prefs.macro_paths.clone();
                self.v5.prefs = Prefs::default();
                self.v5.prefs.macro_paths = macros;
                self.multi_cycle = false;
            }
            PrefsLastSaved => {
                self.v5.prefs = Prefs::load_saved();
                self.multi_cycle = self.v5.prefs.multi_cycle;
            }
            PrefsSave => {
                if let Err(e) = self.v5.prefs.save() {
                    self.msg("Prefs", e);
                } else {
                    self.status = "Preferences saved".into();
                }
            }
            // ---------------------------------------------------------------- Extras
            Fullscreen => self.toggle_fullscreen(ctx),
            Gallery => self.show_gallery = true,
            Camera => self.show_camera = true,
            Convert => self.import_convert(),
            Keys => self.show_keys = true,
            InfoBar => self.info_bar = !self.info_bar,
            Magnify => {
                self.magnify = !self.magnify;
                if let Some(c) = self.cursor {
                    self.mag_center = (c.0 as f32, c.1 as f32);
                }
            }
        }
    }

    /// Is a check-mark command currently on (for the menus)?
    pub fn cmd_checked(&self, c: Cmd) -> Option<bool> {
        use Cmd::*;
        Some(match c {
            SolidBody => false,
            FrameOnly => self.v5.frame_only,
            Panel => self.v5.show_anim_panel,
            Ranges => self.v5.show_ranges,
            Cycle => self.cycling,
            StencilShow => self.v5.stencil_show,
            StencilPaint => self.set.stencil_paint,
            StencilToggle => self.doc.stencil.as_ref().map(|s| s.on).unwrap_or(false),
            LightTableToggle => self.light_table,
            LightTableDim => self.v5.lt_dim,
            Layer1 => self.v5.lt_layers[0].on,
            Layer2 => self.v5.lt_layers[1].on,
            Layer3 => self.v5.lt_layers[2].on,
            Layer4 => self.v5.lt_layers[3].on,
            AaNone => !self.antialias,
            AaLow => self.antialias && !self.aa_high,
            AaHigh => self.antialias && self.aa_high,
            ProcessToggle => self.set.process_on,
            ProcessTint => self.set.process == Process::Tint,
            ProcessHue => self.set.process == Process::Hue,
            ProcessValue => self.set.process == Process::Value,
            TranslucentToggle => self.set.translucent_on,
            PressureToggle => self.v5.pressure_on,
            TextureToggle => self.set.texture_on,
            MacroRecord => self.v5.recording.is_some(),
            MacroDetails => self.v5.record_details,
            PrefCoords => self.v5.prefs.coords,
            PrefFastFb => self.v5.prefs.fast_fb,
            PrefMultiCycle => self.v5.prefs.multi_cycle,
            PrefBeSquare => self.v5.prefs.be_square,
            PrefWorkbench => self.v5.prefs.workbench,
            PrefExclBrush => self.v5.prefs.excl_brush,
            PrefAutoTransp => self.v5.prefs.auto_transp,
            PrefAutoGrid => self.v5.prefs.auto_grid,
            PrefOriginUl => self.v5.prefs.origin_ul,
            PrefMouseTrack => self.v5.prefs.mouse_track,
            AnimBrushUse => self.anim_brush.is_some(),
            InfoBar => self.info_bar,
            _ => return None,
        })
    }

    /// Can the command be used right now (for greying out menu items)?
    pub fn cmd_enabled(&self, c: Cmd) -> bool {
        use Cmd::*;
        let custom = self.brush.custom;
        match c {
            BrushSave | BrushCopy | BrushToSpare | BrushStretch | BrushHalve | BrushDouble | BrushDoubleH | BrushDoubleV | BrushFlipH
            | BrushFlipV | BrushOutline | BrushTrim | BrushRot90 | BrushRotAny | BrushShear | BrushBendH | BrushBendV | BrushBgToFg
            | BrushBgSwapFg | BrushRemap | BrushTransparency | PerspDo | PerspFillScreen => custom,
            BrushRestore => self.brush_original.is_some(),
            BrushSwapSpare => self.v5.spare_brush.is_some(),
            StencilReverse | StencilFree | StencilSave => self.doc.stencil.is_some(),
            BgFree => self.doc.background.is_some(),
            AnimBrushSave | AnimBrushFree => self.anim_brush.is_some(),
            MacroSaveAs => self.v5.last_macro.is_some(),
            MediaClose => self.v5.media.is_some(),
            TextureInvert | TextureFree => self.set.texture.is_some(),
            _ => true,
        }
    }

    pub fn start_play(&mut self, mode: PlayMode, dir: i32) {
        self.v5.play_mode = mode;
        self.v5.play_dir = dir;
        self.play_accum = 0.0;
        self.playing = self.doc.frames.len() > 1;
        if !self.playing {
            self.status = "Add frames first (Anim > Frames)".into();
        }
    }

    /// Advance playback by one frame, following the play mode and range.
    pub fn play_step(&mut self) {
        let n = self.doc.frames.len();
        let (a, b) = self.doc.play_range.map(|(a, b)| (a.min(n - 1), b.min(n - 1))).unwrap_or((0, n - 1));
        let cur = self.doc.cur.clamp(a, b) as i64;
        let mut next = cur + self.v5.play_dir as i64;
        if next > b as i64 || next < a as i64 {
            match self.v5.play_mode {
                PlayMode::Loop => next = if self.v5.play_dir > 0 { a as i64 } else { b as i64 },
                PlayMode::Once => {
                    self.playing = false;
                    return;
                }
                PlayMode::PingPong => {
                    self.v5.play_dir = -self.v5.play_dir;
                    next = cur + self.v5.play_dir as i64;
                    if next < a as i64 || next > b as i64 {
                        next = cur;
                    }
                }
            }
        }
        self.doc.go_to(next as usize);
    }

    pub fn merge_spare(&mut self, front: bool) {
        if self.spare.w != self.doc.w || self.spare.h != self.doc.h {
            self.msg("Merge", "The spare page must be the same size as the picture.");
            return;
        }
        self.doc.checkpoint();
        let sp = self.spare.img().clone();
        let spal = self.spare.palette;
        let pal = self.doc.palette;
        let bg = self.bg;
        let map: Vec<u8> = (0..256).map(|i| nearest(&pal, [spal[i][0] as i32, spal[i][1] as i32, spal[i][2] as i32], 256)).collect();
        let img = self.doc.img_mut();
        for (i, p) in img.px.iter_mut().enumerate() {
            let s = sp.px[i];
            if front {
                if s != bg {
                    *p = map[s as usize];
                }
            } else if *p == bg {
                *p = map[s as usize];
            }
        }
    }

    pub fn light_table_merge(&mut self) {
        self.doc.checkpoint();
        let layers = self.light_layers();
        let bg = self.bg;
        let cur = self.doc.cur;
        let mut img = self.doc.frames[cur].clone();
        for (src, _dim) in layers.iter().rev() {
            for (i, p) in img.px.iter_mut().enumerate() {
                if *p == bg && src.px.get(i).copied().unwrap_or(bg) != bg {
                    *p = src.px[i];
                }
            }
        }
        self.doc.frames[cur] = img;
        self.light_table = false;
        self.doc.undo.clear();
    }

    /// The pictures shown on the light table, topmost first, with their dim amounts.
    pub fn light_layers(&self) -> Vec<(Image, i32)> {
        let mut out = Vec::new();
        if !self.light_table {
            return out;
        }
        let n = self.doc.frames.len() as i64;
        for l in self.v5.lt_layers.iter().filter(|l| l.on) {
            let img = if l.offset == 0 {
                if self.spare.w == self.doc.w && self.spare.h == self.doc.h {
                    Some(self.spare.img().clone())
                } else {
                    None
                }
            } else {
                let idx = self.doc.cur as i64 + l.offset as i64;
                if n <= 1 {
                    None
                } else if (0..n).contains(&idx) {
                    Some(self.doc.frames[idx as usize].clone())
                } else if self.v5.lt_anim_lock {
                    None
                } else {
                    Some(self.doc.frames[idx.rem_euclid(n) as usize].clone())
                }
            };
            if let Some(i) = img {
                out.push((i, if self.v5.lt_dim { l.dim } else { 0 }));
            }
        }
        out
    }

    pub fn brush_copy(&mut self) {
        self.v5.clipboard_brush = Some(self.brush.clone());
        let pal = self.doc.palette;
        let b = &self.brush;
        let mut bytes = Vec::with_capacity((b.img.w * b.img.h * 4) as usize);
        for &p in &b.img.px {
            let c = pal[p as usize];
            let a = if Some(p) == b.transparent { 0 } else { 255 };
            bytes.extend_from_slice(&[c[0], c[1], c[2], a]);
        }
        match arboard::Clipboard::new().and_then(|mut cb| {
            cb.set_image(arboard::ImageData { width: b.img.w as usize, height: b.img.h as usize, bytes: std::borrow::Cow::Owned(bytes) })
        }) {
            Ok(()) => self.status = "Brush copied (also to the Windows clipboard)".into(),
            Err(_) => self.status = "Brush copied".into(),
        }
    }

    pub fn brush_paste(&mut self) {
        let from_clip = arboard::Clipboard::new().and_then(|mut cb| cb.get_image());
        match from_clip {
            Ok(img) => {
                let (w, h) = (img.width as i32, img.height as i32);
                let px: Vec<[u8; 3]> = img.bytes.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
                let alpha: Vec<u8> = img.bytes.chunks_exact(4).map(|p| p[3]).collect();
                let rgba = crate::convert::Rgba { w, h, px };
                let opts = ConvertOptions { palette: crate::convert::PaletteChoice::Current, ..Default::default() };
                let (mut idx, _) = crate::convert::reduce(&rgba, &opts, &self.doc.palette);
                let bg = self.bg;
                for (i, a) in alpha.iter().enumerate() {
                    if *a < 128 {
                        idx.px[i] = bg;
                    }
                }
                self.brush = Brush::from_image(idx, Some(bg));
                self.brush_original = None;
                self.anim_brush = None;
                self.tool = Tool::Dotted;
            }
            Err(_) => match self.v5.clipboard_brush.clone() {
                Some(b) => {
                    self.brush = b;
                    self.tool = Tool::Dotted;
                }
                None => self.msg("Paste", "There is no picture on the clipboard."),
            },
        }
    }

    pub fn anim_brush_load(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .set_title("Load AnimBrush")
            .add_filter("AnimBrush (IFF ANIM)", &["anbr", "anim", "brush"])
            .add_filter("All files", &["*"])
            .pick_file()
        else {
            return;
        };
        match crate::fileio::load(&p) {
            Ok(crate::fileio::LoadResult::Indexed(l)) => {
                let pal = self.doc.palette;
                let map: Vec<u8> = (0..256).map(|i| nearest(&pal, [l.palette[i][0] as i32, l.palette[i][1] as i32, l.palette[i][2] as i32], 256)).collect();
                let cells: Vec<Image> = l
                    .frames
                    .iter()
                    .map(|f| {
                        let mut f = f.clone();
                        f.px.iter_mut().for_each(|p| *p = map[*p as usize]);
                        f
                    })
                    .collect();
                let t = map[l.transparent.unwrap_or(0) as usize];
                let mut b = Brush::from_image(cells[0].clone(), Some(t));
                if let Some((gx, gy)) = l.grab {
                    b.hx = (gx as i32).clamp(0, b.img.w - 1);
                    b.hy = (gy as i32).clamp(0, b.img.h - 1);
                }
                self.v5.brush_palette = Some(l.palette);
                self.brush = b;
                self.anim_brush = Some(crate::app::AnimBrush::new(cells));
                self.tool = Tool::Dotted;
            }
            Ok(_) => self.msg("AnimBrush", "That file isn't an indexed animation."),
            Err(e) => self.msg("AnimBrush", e),
        }
    }

    pub fn anim_brush_save(&mut self) {
        let Some(ab) = &self.anim_brush else { return };
        let cells = ab.cells.clone();
        let grab = (self.brush.hx as i16, self.brush.hy as i16);
        let transparent = self.brush.transparent;
        if let Some(p) = rfd::FileDialog::new().set_title("Save AnimBrush").set_file_name("untitled.anbr").add_filter("AnimBrush (IFF ANIM)", &["anbr"]).save_file() {
            if let Err(e) = crate::fileio::save_anim_brush(&p, &cells, &self.doc.palette, grab, transparent, self.doc.fps) {
                self.msg("AnimBrush", e);
            }
        }
    }

    pub fn palette_load(&mut self, only_range: bool) {
        let Some(p) = rfd::FileDialog::new()
            .set_title(if only_range { "Load Colour Set" } else { "Load Palette" })
            .add_filter("Palettes, colour sets and pictures", &["pal", "set", "lbm", "bbm", "brush", "iff", "ilbm", "png", "gif", "pcx", "bmp"])
            .add_filter("All files", &["*"])
            .pick_file()
        else {
            return;
        };
        // DeluxePaint's .PAL and .SET files are IFF with colours and ranges but no picture;
        // JASC-PAL text and any picture's palette work too.
        match crate::amigafiles::read_palette_file(&p) {
            // Colour Set > Load: choose which of the file's colours to take first (bottom panel).
            Ok(pf) if only_range => {
                self.v5.x.cset = Some(crate::colorset::ColourSetChoice::new(pf.colors.clone(), pf.ranges.clone()));
                self.show_palette = false;
                self.status = "Choose the colours to load: click them (or ALL / BASE), then ADD or OVERWRITE".into();
            }
            Ok(pf) => {
                self.doc.checkpoint();
                self.pal_undo = Some(self.doc.palette);
                for (i, c) in pf.colors.iter().enumerate().take(256) {
                    self.doc.palette[i] = *c;
                }
                // A colour set also brings its colour ranges.
                if only_range && !pf.ranges.is_empty() {
                    self.doc.ranges = pf.ranges.clone();
                    self.doc.fit_ranges();
                }
                self.status = format!("{} colours{}", pf.colors.len(), if only_range && !pf.ranges.is_empty() { format!(" and {} ranges", pf.ranges.len()) } else { String::new() });
            }
            Err(e) => self.msg("Palette", e),
        }
    }

    pub fn palette_save(&mut self, only_range: bool) {
        let Some(p) = rfd::FileDialog::new()
            .set_title(if only_range { "Save Colour Set" } else { "Save Palette" })
            .set_file_name(if only_range { "colours.set" } else { "palette.pal" })
            .add_filter("DeluxePaint palette / colour set (IFF)", &["pal", "set"])
            .add_filter("JASC-PAL text palette", &["jasc", "txt"])
            .save_file()
        else {
            return;
        };
        // The screen's colours; a colour set also keeps the ranges.
        let n = self.doc.colors.clamp(2, 256);
        let cols: Vec<[u8; 3]> = self.doc.palette[..n].to_vec();
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let r = if ext == "jasc" || ext == "txt" {
            let mut s = format!("JASC-PAL\n0100\n{}\n", cols.len());
            for c in &cols {
                s += &format!("{} {} {}\n", c[0], c[1], c[2]);
            }
            std::fs::write(&p, s).map_err(|e| format!("Can't save file: {e}"))
        } else {
            let ranges = if only_range { self.doc.ranges.clone() } else { Vec::new() };
            crate::amigafiles::write_palette_iff(&p, &cols, &ranges, self.doc.mode_id)
        };
        if let Err(e) = r {
            self.msg("Palette", e);
        }
    }

    pub fn texture_load(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .set_title("Load Texture")
            .add_filter("Pictures", &["lbm", "bbm", "brush", "pat", "iff", "ilbm", "png", "gif", "pcx", "bmp", "jpg", "jpeg"])
            .add_filter("All files (DeluxePaint textures have no extension)", &["*"])
            .pick_file()
        else {
            return;
        };
        let img = match crate::fileio::load(&p) {
            Ok(crate::fileio::LoadResult::Indexed(l)) => {
                let mut i = l.frames[0].clone();
                i.px.iter_mut().for_each(|v| *v = (*v != 0) as u8);
                Ok(i)
            }
            Ok(crate::fileio::LoadResult::TrueColor(t)) => {
                let mut i = Image::new(t.w, t.h, 0);
                for (k, c) in t.px.iter().enumerate() {
                    i.px[k] = (crate::palette::luminance(*c) > 127.0) as u8;
                }
                Ok(i)
            }
            Err(e) => Err(e),
        };
        match img {
            Ok(i) => {
                self.set.texture = Some(Texture { img: Arc::new(i), invert: false });
                self.set.texture_on = true;
            }
            Err(e) => self.msg("Texture", e),
        }
    }

    /// Fill Type > Pattern > Load: a DeluxePaint .PAT file (an IFF brush) or any brush or picture.
    pub fn load_pattern(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .set_title("Load Pattern")
            .add_filter("Patterns", &["pat", "brush", "bbm", "lbm", "iff", "ilbm", "lores", "png", "pcx", "bmp"])
            .add_filter("All files", &["*"])
            .pick_file()
        else {
            return;
        };
        match crate::fileio::load(&p) {
            Ok(crate::fileio::LoadResult::Indexed(l)) => {
                let mut b = Brush::from_image(l.frames[0].clone(), None).remapped(&l.palette, &self.doc.palette);
                b.transparent = l.transparent.map(|t| {
                    let c = l.palette[t as usize];
                    nearest(&self.doc.palette, [c[0] as i32, c[1] as i32, c[2] as i32], 256)
                });
                b.custom = true;
                self.set.pattern = Some(Arc::new(b));
                self.set.fill = FillKind::Pattern;
            }
            Ok(_) => self.msg("Load Pattern", "Not a 256-colour picture."),
            Err(e) => self.msg("Load Pattern", e),
        }
    }

    pub fn set_media(&mut self, i: usize) {
        let (name, transl, grain, density) = MEDIA[i];
        self.v5.media = Some(name);
        self.set.translucent = transl.max(1);
        self.set.translucent_on = transl > 0;
        self.set.texture = Some(Texture { img: Arc::new(media_texture(grain, density, 77 + i as u64)), invert: false });
        self.set.texture_on = grain > 0;
    }

    pub fn open_fill(&mut self) {
        let s = &self.set;
        self.v5.fill_backup = Some((s.fill, s.fill_angle, s.fill_page, s.dither, s.spatter, s.range));
        self.v5.show_fill_v5 = true;
    }

    /// Points per screen pixel: the screen format stretched over the whole window.
    pub fn screen_zoom(&self, ctx: &egui::Context) -> f32 {
        let r = ctx.content_rect();
        let (sw, sh) = self.v5.screen;
        (r.width() / sw.max(1) as f32).min(r.height() / sh.max(1) as f32).max(0.01)
    }

    /// The zoom shown to the person: 100% = one picture pixel per screen pixel.
    pub fn zoom_percent(&self, ctx: &egui::Context) -> f32 {
        self.zoom / self.screen_zoom(ctx) * 100.0
    }

    pub fn open_screen_format(&mut self) {
        self.v5.sf_screen = self.v5.screen;
        let cur = crate::amiga::MODES[self.v5.sf_mode.min(crate::amiga::MODES.len() - 1)];
        if (cur.w, cur.h) != self.v5.screen {
            self.v5.sf_mode = crate::amiga::MODES.iter().position(|m| (m.w, m.h) == self.v5.screen).unwrap_or(self.v5.sf_mode);
        }
        self.v5.sf_page = (self.doc.w, self.doc.h);
        self.v5.sf_colors = self.doc.colors;
        self.v5.sf_chipset = self.v5.chipset;
        self.v5.sf_page_kind = 0;
        self.v5.show_screen_format = true;
    }

    /// Queue a macro's events for playback.
    pub fn run_macro(&mut self, m: &Macro) {
        self.v5.replay.extend(m.events.iter().cloned());
    }

    /// Play back queued macro events (a batch per frame so the drawing is visible).
    pub fn run_replay(&mut self, ctx: &egui::Context) {
        let mut budget = 400;
        while budget > 0 {
            let Some(ev) = self.v5.replay.pop_front() else { break };
            budget -= 1;
            match ev {
                MacroEv::Char(c) => self.on_char(ctx, c),
                MacroEv::Key(k, ctrl, alt, shift) => {
                    let m = egui::Modifiers { ctrl, alt, shift, command: ctrl, ..Default::default() };
                    self.on_key(ctx, k, m);
                }
                MacroEv::Ptr(p) => {
                    self.cursor = Some(p.pos);
                    self.pointer_logic(&p);
                }
                MacroEv::Fg(c) => self.fg = c,
                MacroEv::Bg(c) => self.bg = c,
                MacroEv::Tool(t, f) => {
                    self.tool = t;
                    self.filled = f;
                }
                MacroEv::Mode(m) => self.set.mode = m,
                MacroEv::Builtin(s, n) => self.set_builtin(s, n),
                MacroEv::Cmd(c) => self.cmd(ctx, c),
            }
        }
        if !self.v5.replay.is_empty() {
            ctx.request_repaint();
        }
    }

    /// Morph: draw the brush changing into the spare brush across frames.
    pub fn do_morph(&mut self) {
        let Some(b2) = self.v5.spare_brush.clone() else { return };
        let b1 = self.brush.clone();
        let n = self.v5.morph_frames.clamp(2, 999);
        let total = self.doc.frames.len();
        let start = self.doc.cur;
        let (x, y) = self.move_params.start;
        for k in 0..n {
            let t = k as f32 / (n - 1) as f32;
            let w = ((1.0 - t) * b1.img.w as f32 + t * b2.img.w as f32).round().max(1.0) as i32;
            let h = ((1.0 - t) * b1.img.h as f32 + t * b2.img.h as f32).round().max(1.0) as i32;
            let a = b1.img.scaled(w, h);
            let b = b2.img.scaled(w, h);
            let tr = self.bg;
            let mut out = Image::new(w, h, tr);
            for yy in 0..h {
                for xx in 0..w {
                    // Each pixel switches over at its own moment, so the picture dissolves.
                    let hsh = ((xx as u32).wrapping_mul(73856093) ^ (yy as u32).wrapping_mul(19349663)) % 1000;
                    let (img, trn) = if (hsh as f32 / 1000.0) < t { (&b, b2.transparent) } else { (&a, b1.transparent) };
                    let p = img.get(xx, yy);
                    if Some(p) != trn {
                        out.set(xx, yy, p);
                    }
                }
            }
            let br = Brush::from_image(out, Some(tr));
            self.doc.cur = (start + k) % total;
            self.doc.checkpoint();
            let mut st = Stroke::new(self.fg, true);
            paint::stamp(&mut self.doc, &br, &self.set, &mut st, x, y);
        }
        self.doc.cur = start;
        self.status = format!("Morph drawn into {n} frames");
    }

    pub fn do_camera_move(&mut self) {
        let (dx, dy, n, _wrap) = self.v5.cam;
        let total = self.doc.frames.len();
        let start = self.doc.cur;
        for k in 0..n.min(total) {
            let i = (start + k) % total;
            self.doc.cur = i;
            self.doc.checkpoint();
            let t = if n > 1 { k as f32 / (n - 1) as f32 } else { 0.0 };
            self.doc.scroll_frame(i, (dx as f32 * t).round() as i32, (dy as f32 * t).round() as i32);
        }
        self.doc.cur = start;
    }
}

/// Trim: cut the brush down to its painted part.
pub fn trim_brush(b: &Brush) -> Brush {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, -1, -1);
    for y in 0..b.img.h {
        for x in 0..b.img.w {
            if b.is_solid(x, y) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < 0 {
        return b.clone();
    }
    let mut n = b.clone();
    n.img = b.img.crop(x0, y0, x1 - x0 + 1, y1 - y0 + 1);
    n.hx = n.img.w / 2;
    n.hy = n.img.h / 2;
    n
}

