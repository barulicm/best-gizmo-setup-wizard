mod app;
mod pages;
mod utils;

fn main() {
    let mut options = eframe::NativeOptions::default();
    options.centered = true;
    options.viewport = options.viewport.with_inner_size([500.0, 300.0]);

    eframe::run_native(
        "BEST Gizmo Software Installer",
        options,
        Box::new(|cc| Ok(Box::new(crate::app::MyApp::new(cc)))),
    )
    .expect("Unhandled error encountered.");
}
