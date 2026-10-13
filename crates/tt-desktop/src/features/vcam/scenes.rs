use eframe::egui;
use tt_core::vcam::scene::{Scene, new_id};
use super::VcamState;
use crate::ui::icons;
use crate::widgets::button::icon_button;

impl VcamState {
    pub fn switch_scene(&mut self, index: usize) {
        if index < self.prefs.scenes.len() && index != self.prefs.active {
            self.prefs.active = index;
            self.selected = None;
            self.history.reset();
        }
    }

    fn add_scene(&mut self, copy: bool) {
        let current = self.prefs.scene();
        let mut scene = if copy {
            Scene { id: new_id(), name: format!("{} copy", current.name), ..current.clone() }
        } else {
            Scene {
                name: format!("Scene {}", self.prefs.scenes.len() + 1),
                width: current.width,
                height: current.height,
                fps: current.fps,
                ..Default::default()
            }
        };
        if copy {
            for layer in &mut scene.layers {
                layer.id = new_id();
            }
        }
        self.prefs.scenes.push(scene);
        self.switch_scene(self.prefs.scenes.len() - 1);
    }
}

/// Scene list: click to switch instantly (Ctrl+1…9 while the canvas has focus).
pub fn show(ui: &mut egui::Ui, state: &mut VcamState) {
    let mut switch = None;
    let mut delete = None;
    for (index, scene) in state.prefs.scenes.iter().enumerate() {
        let active = index == state.prefs.active;
        let renaming = ui.data(|d| d.get_temp::<usize>(egui::Id::new("vcam_scene_renaming"))) == Some(index);
        ui.horizontal(|ui| {
            if renaming {
                let mut name = scene.name.clone();
                let edit = ui.add(egui::TextEdit::singleline(&mut name).desired_width(150.0));
                edit.request_focus();
                if edit.changed() {
                    ui.data_mut(|d| d.insert_temp(egui::Id::new("vcam_scene_name"), name));
                }
                if edit.lost_focus() {
                    ui.data_mut(|d| d.remove::<usize>(egui::Id::new("vcam_scene_renaming")));
                }
                return;
            }
            let label = egui::Button::selectable(active, format!("{}  {}", index + 1, scene.name)).frame_when_inactive(false);
            let response = ui.add(label);
            if response.clicked() {
                switch = Some(index);
            }
            if response.double_clicked() {
                ui.data_mut(|d| {
                    d.insert_temp(egui::Id::new("vcam_scene_renaming"), index);
                    d.insert_temp(egui::Id::new("vcam_scene_name"), scene.name.clone());
                });
            }
            if active && state.prefs.scenes.len() > 1 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, icons::TRASH, "Delete scene").clicked() {
                        delete = Some(index);
                    }
                });
            }
        });
    }
    if let Some(name) = ui.data(|d| d.get_temp::<String>(egui::Id::new("vcam_scene_name")))
        && let Some(index) = ui.data(|d| d.get_temp::<usize>(egui::Id::new("vcam_scene_renaming")))
        && let Some(scene) = state.prefs.scenes.get_mut(index)
        && !name.trim().is_empty()
    {
        scene.name = name.trim().to_string();
    }
    if let Some(index) = switch {
        state.switch_scene(index);
    }
    if let Some(index) = delete {
        state.prefs.scenes.remove(index);
        state.prefs.active = state.prefs.active.min(state.prefs.scenes.len() - 1);
        state.selected = None;
        state.history.reset();
    }
    ui.horizontal(|ui| {
        if ui.small_button(format!("{} New", icons::PLUS)).clicked() {
            state.add_scene(false);
        }
        if ui.small_button(format!("{} Duplicate", icons::COPY)).clicked() {
            state.add_scene(true);
        }
    });
}
