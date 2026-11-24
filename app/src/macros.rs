//! Macros: record keyboard commands, tool and colour choices and drawing on the picture,
//! then play them back (Macro menu, User menu slots 1-10).

use crate::app::Tool;
use crate::paint::Mode;
use crate::pix::BuiltinShape;
use std::path::Path;

/// One pointer sample on the picture, in picture pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PtrIn {
    pub pos: (i32, i32),
    pub hovered: bool,
    pub pressed_p: bool,
    pub pressed_s: bool,
    pub down_p: bool,
    pub down_s: bool,
    pub released: bool,
    pub dbl: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MacroEv {
    Char(char),
    Key(egui::Key, bool, bool, bool),
    Ptr(PtrIn),
    Fg(u8),
    Bg(u8),
    Tool(Tool, bool),
    Mode(Mode),
    Builtin(BuiltinShape, i32),
    Cmd(crate::actions::Cmd),
}

#[derive(Clone, Debug, Default)]
pub struct Macro {
    pub name: String,
    pub events: Vec<MacroEv>,
}

fn tool_name(t: Tool) -> &'static str {
    match t {
        Tool::Dotted => "dotted",
        Tool::Freehand => "freehand",
        Tool::Line => "line",
        Tool::Curve => "curve",
        Tool::Fill => "fill",
        Tool::Airbrush => "airbrush",
        Tool::Rect => "rect",
        Tool::Circle => "circle",
        Tool::Ellipse => "ellipse",
        Tool::Polygon => "polygon",
        Tool::BrushPick => "brush",
        Tool::Lasso => "lasso",
        Tool::Text => "text",
        Tool::Eyedropper => "picker",
    }
}

fn tool_from(s: &str) -> Option<Tool> {
    [
        Tool::Dotted,
        Tool::Freehand,
        Tool::Line,
        Tool::Curve,
        Tool::Fill,
        Tool::Airbrush,
        Tool::Rect,
        Tool::Circle,
        Tool::Ellipse,
        Tool::Polygon,
        Tool::BrushPick,
        Tool::Lasso,
        Tool::Text,
        Tool::Eyedropper,
    ]
    .into_iter()
    .find(|t| tool_name(*t) == s)
}

impl Macro {
    pub fn to_text(&self) -> String {
        let mut s = format!("DELUXE-PIXEL-MACRO 1\nname {}\n", self.name);
        for e in &self.events {
            let line = match e {
                MacroEv::Char(c) => format!("char {}", *c as u32),
                MacroEv::Key(k, ctrl, alt, shift) => format!("key {} {} {} {}", k.name(), *ctrl as u8, *alt as u8, *shift as u8),
                MacroEv::Ptr(p) => format!(
                    "ptr {} {} {}{}{}{}{}{}{}",
                    p.pos.0,
                    p.pos.1,
                    p.hovered as u8,
                    p.pressed_p as u8,
                    p.pressed_s as u8,
                    p.down_p as u8,
                    p.down_s as u8,
                    p.released as u8,
                    p.dbl as u8
                ),
                MacroEv::Fg(c) => format!("fg {c}"),
                MacroEv::Bg(c) => format!("bg {c}"),
                MacroEv::Tool(t, f) => format!("tool {} {}", tool_name(*t), *f as u8),
                MacroEv::Mode(m) => format!("mode {}", m.label()),
                MacroEv::Builtin(sh, n) => format!("brush {:?} {}", sh, n),
                MacroEv::Cmd(c) => format!("cmd {}", c.name()),
            };
            s.push_str(&line);
            s.push('\n');
        }
        s
    }

    pub fn from_text(t: &str) -> Result<Macro, String> {
        let mut lines = t.lines();
        if !lines.next().unwrap_or("").starts_with("DELUXE-PIXEL-MACRO") {
            return Err("Not a macro file.".into());
        }
        let mut m = Macro::default();
        for l in lines {
            let mut it = l.split_whitespace();
            let Some(kind) = it.next() else { continue };
            let rest: Vec<&str> = it.collect();
            let num = |i: usize| rest.get(i).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
            let ev = match kind {
                "name" => {
                    m.name = rest.join(" ");
                    continue;
                }
                "char" => char::from_u32(num(0) as u32).map(MacroEv::Char),
                "key" => egui::Key::from_name(rest.first().copied().unwrap_or("")).map(|k| MacroEv::Key(k, num(1) == 1, num(2) == 1, num(3) == 1)),
                "ptr" => {
                    let f: Vec<bool> = rest.get(2).unwrap_or(&"0000000").bytes().map(|b| b == b'1').collect();
                    let g = |i: usize| f.get(i).copied().unwrap_or(false);
                    Some(MacroEv::Ptr(PtrIn {
                        pos: (num(0) as i32, num(1) as i32),
                        hovered: g(0),
                        pressed_p: g(1),
                        pressed_s: g(2),
                        down_p: g(3),
                        down_s: g(4),
                        released: g(5),
                        dbl: g(6),
                    }))
                }
                "fg" => Some(MacroEv::Fg(num(0) as u8)),
                "bg" => Some(MacroEv::Bg(num(0) as u8)),
                "tool" => tool_from(rest.first().copied().unwrap_or("")).map(|t| MacroEv::Tool(t, num(1) == 1)),
                "mode" => {
                    // Older macros wrote "Repl" and could use Mix.
                    let name = rest.join(" ");
                    match name.as_str() {
                        "Repl" => Some(MacroEv::Mode(Mode::Replace)),
                        "Mix" => Some(MacroEv::Mode(Mode::Mix)),
                        _ => Mode::ALL.into_iter().find(|md| md.label() == name || md.menu_name() == name).map(MacroEv::Mode),
                    }
                }
                "brush" => {
                    let sh = match rest.first().copied().unwrap_or("") {
                        "Square" => BuiltinShape::Square,
                        "Dither" => BuiltinShape::Dither,
                        _ => BuiltinShape::Round,
                    };
                    Some(MacroEv::Builtin(sh, num(1) as i32))
                }
                "cmd" => crate::actions::Cmd::from_name(rest.first().copied().unwrap_or("")).map(MacroEv::Cmd),
                _ => None,
            };
            if let Some(e) = ev {
                m.events.push(e);
            }
        }
        Ok(m)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.to_text()).map_err(|e| format!("Can't save file: {e}"))
    }
    pub fn load(path: &Path) -> Result<Macro, String> {
        let t = std::fs::read_to_string(path).map_err(|e| format!("Can't load file: {e}"))?;
        let mut m = Macro::from_text(&t)?;
        if m.name.is_empty() {
            m.name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        }
        Ok(m)
    }
}
