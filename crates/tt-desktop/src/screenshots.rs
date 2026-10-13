//! Offscreen renders of each tab for eyeballing layout without opening a window:
//! `cargo test -p tt-desktop screenshots -- --ignored`, then look in `target/screenshots/`.

use std::path::PathBuf;
use eframe::egui;
use egui_dock::TabViewer as _;
use egui_kittest::Harness;
use crate::app::command::Commands;
use crate::app::state::AppState;
use crate::ui::dock::{Tab, TabViewer};

pub const SIZE: egui::Vec2 = egui::vec2(1180.0, 760.0);

pub fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Draws `draw` in a full-size panel with the app's fonts and style, then saves `<name>.png`.
pub fn shoot<S>(name: &str, theme: egui::Theme, state: S, mut draw: impl FnMut(&mut egui::Ui, &mut S)) {
    let mut styled = false;
    let mut harness = Harness::builder().with_size(SIZE).with_theme(theme).wgpu().build_ui_state(
        |ui, state| {
            // The harness runs a frame while building, before the context can be styled from outside,
            // and a style set mid-frame only applies from the next one.
            if !std::mem::replace(&mut styled, true) {
                crate::ui::theme::install_fonts(ui.ctx());
                crate::ui::theme::apply_style(ui.ctx());
                ui.ctx().request_repaint();
                return;
            }
            egui::CentralPanel::default().show(ui, |ui| draw(ui, state));
        },
        state,
    );
    harness.run();
    let image = harness.render().expect("offscreen render");
    image.save(out_dir().join(format!("{name}.png"))).unwrap();
}

pub fn shoot_tab(name: &str, mut tab: Tab, state: AppState, prepare: impl FnOnce(&mut AppState)) {
    let mut state = state;
    prepare(&mut state);
    shoot(name, egui::Theme::Dark, state, |ui, state| {
        let mut cmds = Commands::default();
        let visuals = ui.visuals().clone();
        let mut viewer = TabViewer { state, cmds: &mut cmds, focused: None, visuals };
        viewer.ui(ui, &mut tab);
    });
}

#[test]
#[ignore = "renders offscreen with wgpu; run on demand"]
fn screenshots() {
    shoot_tab("home", Tab::Home, AppState::default(), |_| {});
    shoot_tab("settings", Tab::Settings, AppState::default(), |_| {});
    crate::features::qris::screenshots();
}
