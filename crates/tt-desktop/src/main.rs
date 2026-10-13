mod app;
mod features;
mod generators;
mod ui;
mod util;
mod widgets;

#[cfg(test)]
mod screenshots;

pub const APP_ID: &str = "testing-tools";

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Testing Tools")
            // Matches packaging/linux/testing-tools.desktop so Wayland docks show the right icon.
            .with_app_id(APP_ID)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/icon-256.png"))
                    .expect("embedded icon is a valid PNG"),
            ),
        ..Default::default()
    };

    eframe::run_native(APP_ID, native_options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
