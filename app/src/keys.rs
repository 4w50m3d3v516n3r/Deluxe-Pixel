//! Keyboard: the DeluxePaint V keys. The original's Amiga key is Ctrl here and its Ctrl key
//! is Ctrl+Shift.

use crate::actions::Cmd;
use crate::app::{App, Tool};
use crate::macros::MacroEv;
use crate::paint::Mode;
use crate::pix::BuiltinShape;

impl App {
    pub fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            // Text entry swallows typing.
            if self.text_entry.is_some() {
                match &ev {
                    egui::Event::Text(t) => {
                        self.text_entry.as_mut().unwrap().buffer.push_str(t);
                        self.preview_text();
                    }
                    egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                        self.text_entry.as_mut().unwrap().buffer.pop();
                        self.preview_text();
                    }
                    egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                        self.text_entry.as_mut().unwrap().buffer.push('\n');
                        self.preview_text();
                    }
                    egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => self.commit_text(),
                    _ => {}
                }
                continue;
            }
            match ev {
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    let is_macro_key = (modifiers.ctrl || modifiers.command) && !modifiers.shift && key == egui::Key::M;
                    if !is_macro_key && (modifiers.ctrl || modifiers.alt || !is_plain_char_key(key)) {
                        self.record(MacroEv::Key(key, modifiers.ctrl || modifiers.command, modifiers.alt, modifiers.shift));
                    }
                    self.on_key(ctx, key, modifiers);
                }
                egui::Event::Text(t) => {
                    for ch in t.chars() {
                        self.record(MacroEv::Char(ch));
                        self.on_char(ctx, ch);
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn on_key(&mut self, ctx: &egui::Context, key: egui::Key, m: egui::Modifiers) {
        use egui::Key::*;
        let ctrl = m.ctrl || m.command;
        // Move > Adjust: Enter (or Return) takes the brush's place, Esc goes back unchanged.
        if self.v5.x.mv_adjust.is_some() && matches!(key, Enter | Escape) {
            self.finish_move_adjust(key == Enter);
            return;
        }
        // Enter (the number pad's Enter) switches perspective mode on and off.
        if key == Enter && !ctrl && !m.alt && !m.shift {
            self.cmd(ctx, Cmd::PerspDo);
            return;
        }
        if self.v5.x.persp_mode {
            if key == Escape {
                self.exit_persp();
                return;
            }
            // The number keys turn the brush (Shift: by the angle step).
            let digit = match key {
                Num0 => Some(0),
                Num1 => Some(1),
                Num2 => Some(2),
                Num3 => Some(3),
                Num4 => Some(4),
                Num5 => Some(5),
                Num6 => Some(6),
                Num7 => Some(7),
                Num8 => Some(8),
                Num9 => Some(9),
                _ => None,
            };
            if let Some(d) = digit {
                if !ctrl && !m.alt {
                    // Shift + a main-row digit types a character (Shift+2 is " on German
                    // keyboards, the depth key), so only plain digits turn the brush here.
                    if !m.shift && self.persp_digit(d, false) {
                        return;
                    }
                    if m.shift {
                        return;
                    }
                }
            }
            // The number pad with Shift (or with Num Lock off) arrives as navigation keys:
            // they turn the brush as the pad's digits do (Shift: by the angle step).
            let pad = match key {
                Home => Some(7),
                ArrowUp => Some(8),
                PageUp => Some(9),
                ArrowLeft => Some(4),
                ArrowRight => Some(6),
                End => Some(1),
                ArrowDown => Some(2),
                PageDown => Some(3),
                Insert => Some(0),
                _ => None,
            };
            // Shift + pad 0: Perspective > Reset.
            if pad == Some(0) && m.shift && !ctrl && !m.alt {
                self.cmd(ctx, Cmd::PerspReset);
                return;
            }
            if let Some(d) = pad {
                if !ctrl && !m.alt && self.persp_digit(d, m.shift) {
                    self.v5.x.p3.skip_char = false;
                    return;
                }
            }
        }
        if ctrl && m.shift {
            let c = match key {
                J => Some(Cmd::SpareCopy),
                F => Some(Cmd::FramesCopy),
                R => Some(Cmd::Ranges),
                S => Some(Cmd::StencilPaint),
                L => Some(Cmd::LightTableSettings),
                T => Some(Cmd::TranslucentSettings),
                P => Some(Cmd::PressureSettings),
                Num1 => Some(Cmd::Layer1),
                Num2 => Some(Cmd::Layer2),
                Num3 => Some(Cmd::Layer3),
                Num4 => Some(Cmd::Layer4),
                Z => Some(Cmd::Redo),
                _ => None,
            };
            if let Some(c) = c {
                self.cmd(ctx, c);
            }
            return;
        }
        if ctrl {
            let slot = match key {
                Num1 => Some(0),
                Num2 => Some(1),
                Num3 => Some(2),
                Num4 => Some(3),
                Num5 => Some(4),
                Num6 => Some(5),
                Num7 => Some(6),
                Num8 => Some(7),
                Num9 => Some(8),
                Num0 => Some(9),
                _ => None,
            };
            if let Some(i) = slot {
                self.run_user_macro(i);
                return;
            }
            let c = match key {
                L => Some(Cmd::Load),
                S => Some(Cmd::Save),
                D => Some(Cmd::DeleteFile),
                P => Some(Cmd::Print),
                Q => Some(Cmd::Quit),
                B => Some(Cmd::BrushLoad),
                C => Some(Cmd::BrushCopy),
                V => Some(Cmd::BrushPaste),
                O => Some(Cmd::AnimLoad),
                A => Some(Cmd::AnimSave),
                R => Some(Cmd::FrameRate),
                T => Some(Cmd::TextureToggle),
                I => Some(Cmd::TextureInvert),
                M => Some(Cmd::MacroRecord),
                Z => Some(Cmd::Undo),
                Y => Some(Cmd::Redo),
                N => Some(Cmd::ScreenFormat),
                Minus => Some(Cmd::SpareDelete),
                _ => None,
            };
            if let Some(c) = c {
                self.cmd(ctx, c);
            }
            return;
        }
        if m.alt {
            // Alt+Shift+A: Airbrush requester; Alt+=: one frame more.
            if m.shift && key == A {
                self.show_airbrush = true;
                return;
            }
            if key == Equals || key == Plus {
                self.doc.add_frame_after();
                return;
            }
            let c = match key {
                N => Some(Cmd::BrushToSpare),
                B => Some(Cmd::BrushSwapSpare),
                X => Some(Cmd::HandleRotate),
                Z => Some(Cmd::HandlePlace),
                F => Some(Cmd::FrameOnly),
                A => Some(Cmd::Panel),
                R => Some(Cmd::RangeRev),
                CloseBracket => Some(Cmd::RangeNext),
                OpenBracket => Some(Cmd::RangePrev),
                L => Some(Cmd::LightTableDim),
                Slash => Some(Cmd::AaCycle),
                P => Some(Cmd::ProcessToggle),
                T => Some(Cmd::TranslucentToggle),
                C => Some(Cmd::Camera),
                Enter => Some(Cmd::Fullscreen),
                _ => None,
            };
            if let Some(c) = c {
                self.cmd(ctx, c);
            }
            return;
        }
        match key {
            F1 => self.set_mode(Mode::Paint),
            F2 => self.set_mode(Mode::SingleColor),
            F3 => self.set_mode(Mode::Replace),
            F4 => self.set_mode(Mode::Smear),
            F5 => self.set_mode(Mode::Shade),
            F6 => self.set_mode(Mode::Blend),
            F7 => self.set_mode(Mode::Cycle),
            F8 => self.set_mode(Mode::Smooth),
            F9 => self.info_bar = !self.info_bar,
            F10 => self.view_page = !self.view_page,
            F11 => self.toggle_fullscreen(ctx),
            Tab => self.cycling = !self.cycling,
            Escape => {
                let busy = self.view_page || self.playing || self.drag.is_some() || self.curve.is_some() || self.v5.stencil_show;
                if !busy && self.is_fullscreen(ctx) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                }
                self.cancel_grad_line();
                self.view_page = false;
                self.playing = false;
                self.v5.stencil_show = false;
                if let Some(d) = self.drag.take() {
                    *self.doc.img_mut() = d.base;
                    self.doc.undo.pop();
                }
                if let Some(c) = self.curve.take() {
                    *self.doc.img_mut() = c.base;
                }
                self.stroke = None;
            }
            ArrowLeft | ArrowRight | ArrowUp | ArrowDown => {
                let (dx, dy) = match key {
                    ArrowLeft => (-1.0, 0.0),
                    ArrowRight => (1.0, 0.0),
                    ArrowUp => (0.0, -1.0),
                    _ => (0.0, 1.0),
                };
                // A quarter of the view per press; with Shift a whole view.
                let frac = if m.shift { 1.0 } else { 0.25 };
                if self.magnify {
                    let area = self.view_sizes.1 / self.mag_zoom;
                    self.mag_center.0 += dx * (area.x * frac).max(1.0);
                    self.mag_center.1 += dy * (area.y * frac).max(1.0);
                } else {
                    self.pan.x -= dx * self.view_sizes.0.x * frac;
                    self.pan.y -= dy * self.view_sizes.0.y * frac;
                }
            }
            Delete => {
                self.doc.clear(self.bg);
                self.move_forget_end();
            },
            _ => {}
        }
    }

    pub fn set_mode(&mut self, m: Mode) {
        self.set.mode = m;
        self.record(MacroEv::Mode(m));
    }

    pub(crate) fn on_char(&mut self, ctx: &egui::Context, ch: char) {
        if self.persp_char(ch) {
            return;
        }
        let tool = |app: &mut App, t: Tool, filled: bool| {
            app.tool = t;
            app.filled = filled;
        };
        match ch {
            's' => tool(self, Tool::Dotted, false),
            'd' => tool(self, Tool::Freehand, false),
            'D' => tool(self, Tool::Freehand, true),
            'v' => tool(self, Tool::Line, false),
            'q' => tool(self, Tool::Curve, false),
            'f' => tool(self, Tool::Fill, false),
            'F' => self.cmd(ctx, Cmd::ToolFill),
            'a' => tool(self, Tool::Airbrush, false),
            // Shift+A: airbrush type Random <-> Standard (Alt+Shift+A opens its requester).
            'A' => {
                self.set.airbrush_old = !self.set.airbrush_old;
                self.status = format!("Airbrush: {}", if self.set.airbrush_old { "Random" } else { "Standard" });
            }
            // r while playing: the other direction.
            'r' if self.playing => self.v5.play_dir = -self.v5.play_dir,
            'r' | 'R' => tool(self, Tool::Rect, ch == 'R'),
            'c' | 'C' => tool(self, Tool::Circle, ch == 'C'),
            'e' | 'E' => tool(self, Tool::Ellipse, ch == 'E'),
            'w' | 'W' => tool(self, Tool::Polygon, ch == 'W'),
            'b' => tool(self, Tool::BrushPick, false),
            'B' => self.cmd(ctx, Cmd::BrushRestore),
            't' => tool(self, Tool::Text, false),
            'T' => self.cmd(ctx, Cmd::ToolFonts),
            'V' => self.cmd(ctx, Cmd::ToolSpacing),
            'g' => self.set.grid_on = !self.set.grid_on,
            // i: filled shapes get their outline traced with the current brush.
            'i' => {
                self.v5.x.fill_outline = !self.v5.x.fill_outline;
                self.status = if self.v5.x.fill_outline {
                    "Filled shapes with outline: the current brush traces the edge (Spacing applies, e.g. Airbrush)".into()
                } else {
                    "Filled shapes without outline".into()
                };
            }
            // Shift+G: the grid on, with the pointer as one of its points.
            'G' => {
                if let Some((x, y)) = self.cursor {
                    let (gw, gh, _, _) = self.set.grid;
                    self.set.grid = (gw, gh, x.rem_euclid(gw.max(1)), y.rem_euclid(gh.max(1)));
                    self.v5.x.p3.last_key = None;
                }
                self.set.grid_on = true;
                self.status = format!("Grid on through {}, {}", self.cursor.map(|c| c.0).unwrap_or(0), self.cursor.map(|c| c.1).unwrap_or(0));
            }
            '/' => self.set.sym.on = !self.set.sym.on,
            '?' => self.cmd(ctx, Cmd::About),
            'm' => self.cmd(ctx, Cmd::Magnify),
            'n' => {
                if let Some(c) = self.cursor {
                    self.mag_center = (c.0 as f32, c.1 as f32);
                    self.magnify = true;
                }
            }
            '+' => self.mag_zoom = (self.mag_zoom + 1.0).min(24.0),
            '_' => self.mag_zoom = (self.mag_zoom - 1.0).max(2.0),
            // UNDO undoes the last operation, an undo too: u twice in a row brings it back.
            'u' | 'U' => {
                if self.v5.x.u_was_undo && !self.doc.redo.is_empty() {
                    self.doc.redo();
                    self.v5.x.u_was_undo = false;
                } else {
                    self.doc.undo();
                    self.v5.x.u_was_undo = true;
                }
            }
            // 7 / 8 (main keyboard): the AnimBrush's previous / next cel.
            '7' | '8' if self.anim_brush.is_some() => {
                if let Some(ab) = &mut self.anim_brush {
                    let n = ab.cells.len();
                    let c = if ch == '7' { (ab.pos + n - 1) % n } else { (ab.pos + 1) % n };
                    ab.restart_at(c);
                    self.brush.img = ab.cells[c].clone();
                    self.status = format!("AnimBrush cel {} of {}", c + 1, n);
                }
            }
            // Shift+3: the last Go To again.
            '#' | '§' => {
                let n = self.doc.frames.len();
                self.doc.go_to(self.v5.goto.clamp(1, n) - 1);
            }
            'K' => {
                self.doc.clear(self.bg);
                self.move_forget_end();
            },
            ',' => tool(self, Tool::Eyedropper, false),
            '.' => self.set_builtin(BuiltinShape::Round, 1),
            '=' => {
                let (s, n) = self.builtin;
                self.set_builtin(s, n + 1)
            }
            '-' => {
                let (s, n) = self.builtin;
                self.set_builtin(s, n - 1)
            }
            '[' => self.fg = self.fg.wrapping_sub(1),
            ']' => self.fg = self.fg.wrapping_add(1),
            '{' => self.bg = self.bg.wrapping_sub(1),
            '}' => self.bg = self.bg.wrapping_add(1),
            'j' => self.cmd(ctx, Cmd::SpareSwap),
            'S' => self.cmd(ctx, Cmd::ShowPage),
            'o' => self.cmd(ctx, Cmd::BrushOutline),
            'O' => self.cmd(ctx, Cmd::BrushTrim),
            'h' => self.cmd(ctx, Cmd::BrushHalve),
            'H' => self.cmd(ctx, Cmd::BrushDouble),
            'X' => self.cmd(ctx, Cmd::BrushDoubleH),
            'Y' => self.cmd(ctx, Cmd::BrushDoubleV),
            'Z' => self.cmd(ctx, Cmd::BrushStretch),
            'x' => self.cmd(ctx, Cmd::BrushFlipH),
            'y' => self.cmd(ctx, Cmd::BrushFlipV),
            'z' => self.cmd(ctx, Cmd::BrushRot90),
            'p' => self.cmd(ctx, Cmd::Mixer),
            'P' => self.cmd(ctx, Cmd::Arrange),
            'M' => self.cmd(ctx, Cmd::Move),
            'N' => self.cmd(ctx, Cmd::CameraMove),
            '~' => self.cmd(ctx, Cmd::StencilMake),
            '`' => self.cmd(ctx, Cmd::StencilToggle),
            '|' => self.cmd(ctx, Cmd::LightTableToggle),
            '\\' => self.cmd(ctx, Cmd::PrefCoords),
            '1' => self.cmd(ctx, Cmd::PrevFrame),
            '2' => self.cmd(ctx, Cmd::NextFrame),
            '3' => self.cmd(ctx, Cmd::GoTo),
            '4' => {
                if self.playing {
                    self.playing = false
                } else {
                    self.cmd(ctx, Cmd::Play)
                }
            }
            '5' => self.cmd(ctx, Cmd::PlayOnce),
            '6' => self.cmd(ctx, Cmd::PingPong),
            // Shift+1 / Shift+2: first / last frame (Shift+2 is @ on US, " on German keyboards).
            '!' => self.doc.go_to(0),
            '@' | '"' => {
                let n = self.doc.frames.len();
                self.doc.go_to(n - 1);
            }
            // Space stops a playing animation.
            ' ' if self.playing => self.playing = false,
            _ => {}
        }
    }

    pub fn run_user_macro(&mut self, i: usize) {
        match self.v5.slots.get(i).cloned().flatten() {
            Some(m) => {
                self.status = format!("Running {}", m.name);
                self.run_macro(&m);
            }
            None => self.msg("User", format!("Macro #{} is empty. Record one with Macro > Record Macro, or load one.", i + 1)),
        }
    }
}

/// Keys that also produce a typed character (recorded as characters instead).
fn is_plain_char_key(k: egui::Key) -> bool {
    use egui::Key::*;
    !matches!(
        k,
        F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12 | Tab | Escape | ArrowLeft | ArrowRight | ArrowUp | ArrowDown | Delete | Enter | Backspace
    )
}
