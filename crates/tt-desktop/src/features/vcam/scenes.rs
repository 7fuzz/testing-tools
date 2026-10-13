use eframe::egui;
use tt_core::vcam::scene::{Scene, new_id};
use super::VcamState;
use crate::ui::icons;
use crate::widgets::button::icon_button;
use crate::widgets::rename::{Outcome, Rename};

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

/// Scene list: click to switch instantly (or Ctrl+1…9), double-click to rename.
pub fn show(ui: &mut egui::Ui, state: &mut VcamState) {
    let mut switch = None;
    let mut delete = None;
    let mut renamed = None;
    for (index, scene) in state.prefs.scenes.iter().enumerate() {
        let active = index == state.prefs.active;
        ui.horizontal(|ui| {
            if let Some((at, rename)) = &mut state.renaming_scene
                && *at == index
            {
                match rename.show(ui, 150.0) {
                    Outcome::Editing => {}
                    Outcome::Save(name) => renamed = Some(Some((index, name))),
                    Outcome::Cancel => renamed = Some(None),
                }
                return;
            }
            let label = egui::Button::selectable(active, format!("{}  {}", index + 1, scene.name)).frame_when_inactive(false);
            let response = ui.add(label);
            if response.clicked() {
                switch = Some(index);
            }
            if response.double_clicked() {
                state.renaming_scene = Some((index, Rename::new(scene.name.clone())));
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
    if let Some(done) = renamed {
        state.renaming_scene = None;
        if let Some((index, name)) = done
            && let Some(scene) = state.prefs.scenes.get_mut(index)
        {
            scene.name = name;
        }
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
