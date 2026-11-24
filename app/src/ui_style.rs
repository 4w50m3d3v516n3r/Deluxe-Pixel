//! The classic look, taken from the reference pictures: grey (160,160,160) screens and requesters,
//! teal (0,96,128) highlights and title bars, black and white bevels, bold pixel-style lettering.

use egui::{Color32, CornerRadius, Pos2, Rect, Response, Sense, Shadow, Stroke, Ui, Vec2, Visuals};

pub const GREY: Color32 = Color32::from_rgb(160, 160, 160);
pub const TEAL: Color32 = Color32::from_rgb(0, 96, 128);
pub const BLACK: Color32 = Color32::BLACK;
pub const WHITE: Color32 = Color32::WHITE;
pub const ACCENT: Color32 = TEAL;
pub const DIM_TEXT: Color32 = Color32::from_rgb(64, 64, 64);

pub fn apply(ctx: &egui::Context) {
    load_fonts(ctx);
    let mut v = Visuals::light();
    v.dark_mode = false;
    v.override_text_color = None;
    v.panel_fill = GREY;
    v.window_fill = GREY;
    v.faint_bg_color = Color32::from_rgb(150, 150, 150);
    v.extreme_bg_color = GREY;
    v.code_bg_color = GREY;
    v.window_stroke = Stroke::new(2.0, BLACK);
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    v.window_shadow = Shadow::NONE;
    v.popup_shadow = Shadow::NONE;
    v.selection.bg_fill = TEAL;
    v.selection.stroke = Stroke::new(1.0, WHITE);
    v.hyperlink_color = TEAL;
    let w = &mut v.widgets;
    for s in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        s.corner_radius = CornerRadius::ZERO;
        s.expansion = 0.0;
    }
    w.noninteractive.bg_fill = GREY;
    w.noninteractive.weak_bg_fill = GREY;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BLACK);
    w.noninteractive.fg_stroke = Stroke::new(1.0, BLACK);
    w.inactive.bg_fill = Color32::from_rgb(140, 140, 140);
    w.inactive.weak_bg_fill = GREY;
    w.inactive.bg_stroke = Stroke::new(1.0, BLACK);
    w.inactive.fg_stroke = Stroke::new(1.5, BLACK);
    w.hovered.bg_fill = TEAL;
    w.hovered.weak_bg_fill = TEAL;
    w.hovered.bg_stroke = Stroke::new(1.0, BLACK);
    w.hovered.fg_stroke = Stroke::new(1.5, WHITE);
    w.active.bg_fill = TEAL;
    w.active.weak_bg_fill = TEAL;
    w.active.bg_stroke = Stroke::new(1.0, BLACK);
    w.active.fg_stroke = Stroke::new(1.5, WHITE);
    w.open.bg_fill = TEAL;
    w.open.weak_bg_fill = TEAL;
    w.open.bg_stroke = Stroke::new(1.0, BLACK);
    w.open.fg_stroke = Stroke::new(1.5, WHITE);
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.set_visuals_of(egui::Theme::Light, v.clone());
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        s.spacing.menu_margin = egui::Margin::same(4);
        s.spacing.window_margin = egui::Margin::same(8);
        use egui::{FontFamily, FontId, TextStyle};
        s.text_styles.insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(14.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Heading, FontId::new(15.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Small, FontId::new(11.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace));
    });
}

/// A bold, compact system font gives the blocky lettering of the original's menus.
fn load_fonts(ctx: &egui::Context) {
    let dir = std::env::var("WINDIR").map(|w| std::path::PathBuf::from(w).join("Fonts")).unwrap_or_else(|_| "C:\\Windows\\Fonts".into());
    let mut fonts = egui::FontDefinitions::default();
    for (name, file, family) in [
        ("ui-bold", "tahomabd.ttf", egui::FontFamily::Proportional),
        ("ui-mono", "consolab.ttf", egui::FontFamily::Monospace),
    ] {
        if let Ok(data) = std::fs::read(dir.join(file)) {
            fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            fonts.families.entry(family).or_default().insert(0, name.to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

/// Raised bevel: white on the top and left, black on the bottom and right.
pub fn bevel(p: &egui::Painter, r: Rect, w: f32, sunken: bool) {
    let (tl, br) = if sunken { (BLACK, WHITE) } else { (WHITE, BLACK) };
    p.rect_filled(Rect::from_min_max(r.min, Pos2::new(r.max.x, r.min.y + w)), 0.0, tl);
    p.rect_filled(Rect::from_min_max(r.min, Pos2::new(r.min.x + w, r.max.y)), 0.0, tl);
    p.rect_filled(Rect::from_min_max(Pos2::new(r.min.x, r.max.y - w), r.max), 0.0, br);
    p.rect_filled(Rect::from_min_max(Pos2::new(r.max.x - w, r.min.y), r.max), 0.0, br);
}

/// A requester button: grey, bevelled, black lettering; pressed it sinks and turns teal.
pub fn button(ui: &mut Ui, text: impl Into<String>) -> Response {
    let text: String = text.into();
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(text.clone(), font.clone(), BLACK);
    let size = Vec2::new(galley.size().x + 22.0, galley.size().y + 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    let down = resp.is_pointer_button_down_on();
    let p = ui.painter();
    p.rect_filled(rect, 0.0, if down { TEAL } else { GREY });
    bevel(p, rect, 1.5, down);
    let col = if !enabled {
        Color32::from_gray(110)
    } else if down {
        WHITE
    } else {
        BLACK
    };
    p.text(rect.center(), egui::Align2::CENTER_CENTER, text, font, col);
    resp
}

/// A button that stays pressed (teal) while `on`, like the original's toggle gadgets.
pub fn toggle(ui: &mut Ui, text: impl Into<String>, on: bool) -> Response {
    let text: String = text.into();
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(text.clone(), font.clone(), BLACK);
    let size = Vec2::new(galley.size().x + 22.0, galley.size().y + 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let lit = on || resp.is_pointer_button_down_on();
    let p = ui.painter();
    p.rect_filled(rect, 0.0, if lit { TEAL } else { GREY });
    bevel(p, rect, 1.5, lit);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, text, font, if lit { WHITE } else { BLACK });
    resp
}

/// A toggle button showing a pixel-art bitmap instead of text (arrows and the like).
pub fn toggle_icon(ui: &mut Ui, rows: &[&str], on: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(40.0, 24.0), Sense::click());
    let lit = on || resp.is_pointer_button_down_on();
    let p = ui.painter();
    p.rect_filled(rect, 0.0, if lit { TEAL } else { GREY });
    bevel(p, rect, 1.5, lit);
    bitmap(p, rect.center(), rows, 2.0, if lit { WHITE } else { BLACK });
    resp
}

/// Direction arrows: forward, backward, ping-pong (AnimBrush Settings).
pub const ICON_FWD: [&str; 7] = ["#...", "##..", "###.", "####", "###.", "##..", "#..."];
pub const ICON_BACK: [&str; 7] = ["...#", "..##", ".###", "####", ".###", "..##", "...#"];
pub const ICON_PING: [&str; 7] = ["...#.#...", "..##.##..", ".###.###.", "####.####", ".###.###.", "..##.##..", "...#.#..."];

/// Draw a pixel-art bitmap ('#' = ink) centred at `c`, each pixel `px` screen pixels.
pub fn bitmap(p: &egui::Painter, c: Pos2, rows: &[&str], px: f32, col: Color32) {
    let h = rows.len() as f32;
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0) as f32;
    let origin = c - Vec2::new(w * px / 2.0, h * px / 2.0);
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.bytes().enumerate() {
            if ch == b'#' {
                let r = Rect::from_min_size(origin + Vec2::new(x as f32 * px, y as f32 * px), Vec2::splat(px));
                p.rect_filled(r, 0.0, col);
            }
        }
    }
}

// ---------------------------------------------------------------- tool icons (16 × 13)

pub const ICON_DOTTED: &[&str] = &[
    "................", "......##........", "................", "........##......", "................",
    ".........##.....", "................", ".........##.....", "................", "........##......",
    "................", "......##........", "................",
];
pub const ICON_FREEHAND: &[&str] = &[
    "................", "..#####.........", ".#.....#........", ".#.....#.######.", ".#.....##.....#.",
    ".#....#########.", ".#...##########.", ".#...##########.", ".#...##########.", ".#...##########.",
    ".#...###########", ".......#########", "................",
];
pub const ICON_LINE: &[&str] = &[
    "................", "..............#.", ".............#..", "...........##...", ".........##.....",
    ".......##.......", ".....##.........", "...##...........", "..#.............", ".#..............",
    "................", "................", "................",
];
pub const ICON_CURVE: &[&str] = &[
    "................", "..######........", "........##......", "..........#.....", "...........#....",
    "............#...", ".............#..", ".............#..", "..............#.", "..............#.",
    "..............#.", "................", "................",
];
pub const ICON_FILL: &[&str] = &[
    "................", ".....####.......", "....#....#......", "...#......#.....", "...#......#.....",
    "....#....#......", ".....#..#.......", "......##........", "......##........", "..#######.......",
    "..#######.......", "................", "................",
];
pub const ICON_AIRBRUSH: &[&str] = &[
    "................", ".........#.#....", "..........#..#..", "..#....#.......#", "...#..#..#..#...",
    "#######..#....#.", "...#..#..#..#...", "..#....#.......#", "..........#..#..", ".........#.#....",
    "................", "................", "................",
];
pub const ICON_RECT: &[&str] = &[
    "................", ".##############.", ".#............#.", ".#...........##.", ".#..........###.",
    ".#.........####.", ".#........#####.", ".#.......######.", ".#......#######.", ".#.....########.",
    ".##############.", "................", "................",
];
pub const ICON_CIRCLE: &[&str] = &[
    ".....######.....", "...##......##...", "..#.........###.", ".#.........####.", ".#........#####.",
    "#........#######", "#.......########", "#......#########", ".#....#########.", ".#...##########.",
    "..#.#########...", "...##########...", ".....######.....",
];
pub const ICON_ELLIPSE: &[&str] = &[
    "................", "......######....", "...###......#...", ".##.........###.", "#.........######",
    "#.......########", "#.....#########.", ".#..##########..", "..###########...", "....######......",
    "................", "................", "................",
];
pub const ICON_POLYGON: &[&str] = &[
    "................", "......##........", ".....#..#.......", "....#....#......", "...#......#.....",
    "..#.......###...", ".#.......#####..", "..#.....#######.", "...#...#######..", "....#.#######...",
    ".....#######....", "......####......", "................",
];
pub const ICON_BRUSH: &[&str] = &[
    "................", ".###........###.", ".###........###.", ".##..........##.", "................",
    "................", "................", "................", ".##..........##.", ".###........###.",
    ".###........###.", "................", "................",
];
pub const ICON_TEXT: &[&str] = &[
    "................", ".....######.....", "....##....##....", "....##....##....", "....##....##....",
    "....########....", "....##....##....", "....##....##....", "....##....##....", "....##....##....",
    "................", "................", "................",
];
pub const ICON_GRID: &[&str] = &[
    "................", ".##############.", ".#......#.....#.", ".#......#.....#.", ".#......#.....#.",
    ".#......#.....#.", ".##############.", ".#......#.....#.", ".#......#.....#.", ".#......#.....#.",
    ".##############.", "................", "................",
];
pub const ICON_SYMMETRY: &[&str] = &[
    "................", ".......##.......", ".....#.##.#.....", "....#......#....", "..##..#..#..##..",
    ".###...##...###.", "..##..#..#..##..", "....#......#....", ".....#.##.#.....", ".......##.......",
    "................", "................", "................",
];
pub const ICON_MAGNIFY: &[&str] = &[
    ".....#####......", "....#.....#.....", "...#.......#....", "...#.......#....", "...#.......#....",
    "....#.....#.....", "....######......", "...##...........", "..##............", ".##.............",
    "##..............", "................", "................",
];
pub const ICON_ZOOM: &[&str] = &[
    "................", ".##############.", ".#............#.", ".#.##########.#.", ".#.#........#.#.",
    ".#.#..####..#.#.", ".#.#..#..#..#.#.", ".#.#..####..#.#.", ".#.#........#.#.", ".#.##########.#.",
    ".#............#.", ".##############.", "................",
];
pub const ICON_UNDO: &[&str] = &[
    "................", "..##..##.#...#..", "..##..##.##..#..", "..##..##.###.#..", "..##..##.#.###..",
    "..######.#..##..", "................", "..####...#####..", "..##.##..##.##..", "..##.##..##.##..",
    "..##.##..##.##..", "..####...#####..", "................",
];
pub const ICON_CLR: &[&str] = &[
    "................", ".###.#....###...", "#....#....#..#..", "#....#....#..#..", "#....#....###...",
    "#....#....#.#...", "#....#....#..#..", "#....#....#..#..", ".###.####.#..#..", "................",
    "................", "................", "................",
];
pub const ICON_PICKER: &[&str] = &[
    "................", "...........###..", "..........#####.", ".........######.", "........#####...",
    ".......#.###....", "......#.#.......", ".....#.#........", "....#.#.........", "...#.#..........",
    "..##............", ".#..............", "................",
];

/// A colour picker (pipette) icon drawn in `r`: a round rubber bulb top right, a collar, a glass
/// tube with the colour inside running down to a fine tip bottom left.
pub fn pipette_icon(p: &egui::Painter, r: Rect, ink: Color32, fill: Color32) {
    let s = r.width().min(r.height());
    let c = r.center();
    let at = |x: f32, y: f32| c + Vec2::new(x * s, y * s);
    // Glass tube: dark outline, light glass, colour in the lower half.
    let (t0, t1) = (at(0.12, -0.12), at(-0.24, 0.24));
    p.line_segment([t0, t1], Stroke::new(s * 0.22, ink));
    p.line_segment([at(0.08, -0.08), t1], Stroke::new(s * 0.12, Color32::from_rgb(220, 235, 245)));
    p.line_segment([at(-0.04, 0.04), t1], Stroke::new(s * 0.12, fill));
    // Fine tip.
    p.line_segment([t1, at(-0.38, 0.38)], Stroke::new(s * 0.07, ink));
    // Collar across the tube.
    p.line_segment([at(0.06, -0.26), at(0.26, -0.06)], Stroke::new(s * 0.09, ink));
    // Rubber bulb.
    p.circle_filled(at(0.27, -0.27), s * 0.15, ink);
}
