#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assistant;
mod data;
mod db;
mod sync;
mod ui;
mod vault;

use eframe::egui;
use std::path::PathBuf;

/// The window and taskbar icon, built into the program so it works without any files.
fn app_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/icons/256x256.png")).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width, height })
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    // Optional, for checking one screen: --page <name> (e.g. budgets), and --wizard to show setup.
    let start_page = args.iter().position(|a| a == "--page").and_then(|i| args.get(i + 1)).cloned();
    let force_wizard = args.iter().any(|a| a == "--wizard");

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("CoinCraft")
        .with_inner_size([1320.0, 860.0])
        .with_min_inner_size([980.0, 640.0]);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }

    if screenshot.is_some() {
        // Open quietly so a check never takes focus from what the person is doing.
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "CoinCraft",
        options,
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, screenshot, start_page, force_wizard)))),
    )
}
