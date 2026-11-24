#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod amiga;
mod amigafiles;
mod amigafont;
mod anim;
mod app;
mod capture;
mod colorset;
mod convert;
mod dialogs;
mod dialogs5;
mod doc;
mod fileio;
mod framepal;
mod gallery;
mod keys;
mod macros;
mod mixer;
mod move3d;
mod paint;
mod palette;
mod panels;
mod persp3d;
mod pix;
mod reqs8;
mod prefs;
mod text;
mod ui_panels;
mod ui_style;
mod v52;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod anim_tests;
#[cfg(test)]
mod v52_tests;
#[cfg(test)]
mod amiga_tests;
#[cfg(test)]
mod amigafont_tests;
#[cfg(test)]
mod persp3d_tests;
#[cfg(test)]
mod modes_tests;
#[cfg(test)]
mod media_tests;
#[cfg(test)]
mod move3d_tests;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let mut screenshot: Option<PathBuf> = None;
    let mut open: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--screenshot" {
            screenshot = args.next().map(PathBuf::from);
        } else if !a.starts_with("--") {
            open = Some(PathBuf::from(a));
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Deluxe Pixel")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([800.0, 560.0]);
    if screenshot.is_some() {
        // Open without taking focus and (on Windows) behind other windows, so a check never gets in the way.
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        options.event_loop_builder = Some(Box::new(|b| {
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    let shot = screenshot.clone();
    eframe::run_native("Deluxe Pixel", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, shot, open)))))
}
