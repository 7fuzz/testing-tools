use std::path::PathBuf;
use eframe::egui::{self, RichText};
use serde::{Deserialize, Serialize};
use crate::ui::{icons, shortcuts};
use crate::widgets::button::icon_button;
use crate::widgets::section::section;
use crate::widgets::segmented::segmented;
use crate::widgets::toast;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsPrefs {
    /// Where generated files are saved; `None` means the default under the data directory.
    pub output_dir: Option<PathBuf>,
}

impl SettingsPrefs {
    pub fn output_dir(&self) -> PathBuf {
        self.output_dir.clone().unwrap_or_else(default_output_dir)
    }
}

pub fn data_dir() -> Option<PathBuf> {
    eframe::storage_dir(crate::APP_ID)
}

fn default_output_dir() -> PathBuf {
    data_dir().unwrap_or_else(|| PathBuf::from(".")).join("output")
}

pub fn show(ui: &mut egui::Ui, prefs: &mut SettingsPrefs) {
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.set_max_width(640.0);
        appearance(ui);
        output(ui, prefs);
        shortcut_list(ui);
        data(ui);
    });
}

fn appearance(ui: &mut egui::Ui) {
    section(ui, "Theme");
    let mut preference = ui.ctx().options(|opt| opt.theme_preference);
    let choices = [
        (egui::ThemePreference::System, "System"),
        (egui::ThemePreference::Light, "Light"),
        (egui::ThemePreference::Dark, "Dark"),
    ];
    if segmented(ui, &mut preference, &choices) {
        ui.ctx().set_theme(preference);
    }
}

fn output(ui: &mut egui::Ui, prefs: &mut SettingsPrefs) {
    section(ui, "Output folder");
    let dir = prefs.output_dir();
    ui.horizontal(|ui| {
        ui.label(RichText::new(dir.display().to_string()).monospace());
        if icon_button(ui, icons::FOLDER_OPEN, "Choose folder").clicked()
            && let Some(picked) = rfd::FileDialog::new().set_directory(&dir).pick_folder()
        {
            prefs.output_dir = Some(picked);
        }
        if icon_button(ui, icons::ARROW_SQUARE_OUT, "Open folder").clicked() {
            let opened = std::fs::create_dir_all(&dir).and_then(|_| open::that_detached(&dir));
            if let Err(error) = opened {
                toast::push(ui.ctx(), Err(format!("Couldn't open {}: {error}", dir.display())));
            }
        }
        if prefs.output_dir.is_some() && ui.button("Reset").clicked() {
            prefs.output_dir = None;
        }
    });
}

fn shortcut_list(ui: &mut egui::Ui) {
    section(ui, "Shortcuts");
    egui::Grid::new("shortcuts").num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
        for (keys, action) in shortcuts::listing(ui.ctx()) {
            ui.label(RichText::new(keys).monospace());
            ui.label(action);
            ui.end_row();
        }
    });
}

fn data(ui: &mut egui::Ui) {
    section(ui, "Data");
    match data_dir() {
        Some(dir) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(dir.display().to_string()).monospace());
                if icon_button(ui, icons::ARROW_SQUARE_OUT, "Open folder").clicked() {
                    let _ = open::that_detached(&dir);
                }
            });
            ui.weak("Window layout and preferences are saved in app.ron here.");
        }
        None => {
            ui.weak("No data directory; preferences won't be saved.");
        }
    }
}
