#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod app;
mod chrome;
mod commands;
mod grid;
mod labs;
mod input;
#[cfg(target_os = "macos")]
mod macos_menu;
mod theme;

use eframe::egui::{self, ViewportBuilder};

fn main() -> eframe::Result {
    if std::env::args().any(|arg| arg == "--host") {
        app::serve_host();
        std::process::exit(0);
    }
    #[cfg(target_os = "macos")]
    macos_menu::disable_automatic_window_tabs();

    let icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icons/app-linux.png"))
            .unwrap_or_else(|_| egui::IconData::default());
    let viewport = ViewportBuilder::default()
        .with_app_id("com.github.thealicanacar.nerd")
        .with_inner_size([1020.0, 650.0])
        .with_min_inner_size([420.0, 260.0])
        .with_icon(icon);

    eframe::run_native(
        "Nerd",
        eframe::NativeOptions {
            viewport,
            renderer: eframe::Renderer::Wgpu,
            persist_window: false,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(app::TerminalApp::new(cc)))),
    )
}
