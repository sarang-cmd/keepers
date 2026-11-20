#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(test)]
mod shot;

mod app;
mod decode;
mod model;
mod theme;
mod thumbs;

fn main() -> eframe::Result<()> {
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([980.0, 620.0])
        .with_title("Keepers");
    let opts = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Keepers",
        opts,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
