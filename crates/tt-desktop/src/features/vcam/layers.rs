use eframe::egui::{self, RichText};
use tt_core::vcam::scene::{Layer, LayerId, new_id};
use super::VcamState;
use crate::ui::icons;
use crate::ui::theme::{Palette, Tone};
use crate::widgets::button::icon_button;
use crate::widgets::rename::{Outcome, Rename};

impl VcamState {
    /// Moves a layer up (towards the top of the stack) or down.
    pub fn restack(&mut self, id: LayerId, up: bool) {
        let layers = &mut self.prefs.scene_mut().layers;
        let Some(index) = layers.iter().position(|l| l.id == id) else { return };
        let target = if up { index + 1 } else { index.wrapping_sub(1) };
        if target < layers.len() {
            layers.swap(index, target);
        }
    }

    pub fn duplicate(&mut self, id: LayerId) {
        let scene = self.prefs.scene_mut();
        let Some(index) = scene.index_of(id) else { return };
        let mut copy: Layer = scene.layers[index].clone();
        copy.id = new_id();
        copy.name = format!("{} copy", copy.name);
        copy.transform.x += 20.0;
        copy.transform.y += 20.0;
        self.selected = Some(copy.id);
        scene.layers.insert(index + 1, copy);
    }

    pub fn remove(&mut self, id: LayerId) {
        self.prefs.scene_mut().layers.retain(|l| l.id != id);
        if self.selected == Some(id) {
            self.selected = None;
        }
    }

    fn move_to(&mut self, from: usize, to: usize) {
        let layers = &mut self.prefs.scene_mut().layers;
        if from < layers.len() && to < layers.len() && from != to {
            let layer = layers.remove(from);
            layers.insert(to, layer);
        }
    }
}

/// The layer stack, topmost first. Drag a row onto another to reorder.
pub fn show(ui: &mut egui::Ui, state: &mut VcamState, errors: &[(LayerId, String)]) {
    let palette = Palette::of(ui.visuals());
    let count = state.prefs.scene().layers.len();
    if count == 0 {
        ui.weak("No layers. Add one with the + button.");
    }
    let mut moved = None;
    for index in (0..count).rev() {
        let layer = state.prefs.scene().layers[index].clone();
        let selected = state.selected == Some(layer.id);
        let row = egui::Frame::new()
            .fill(if selected { palette.accent.gamma_multiply(0.18) } else { egui::Color32::TRANSPARENT })
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let eye = if layer.visible { icons::EYE } else { icons::EYE_SLASH };
                    if icon_button(ui, eye, if layer.visible { "Hide" } else { "Show" }).clicked()
                        && let Some(l) = state.prefs.scene_mut().layer_mut(layer.id)
                    {
                        l.visible = !l.visible;
                    }
                    let lock = if layer.locked { icons::LOCK } else { icons::LOCK_OPEN };
                    if icon_button(ui, lock, if layer.locked { "Unlock" } else { "Lock in place" }).clicked()
                        && let Some(l) = state.prefs.scene_mut().layer_mut(layer.id)
                    {
                        l.locked = !l.locked;
                    }
                    ui.add_space(4.0);
                    name(ui, state, &layer);
                    if let Some((_, error)) = errors.iter().find(|(id, _)| *id == layer.id) {
                        ui.label(RichText::new(icons::WARNING).color(Tone::Danger.color(ui.visuals()))).on_hover_text(error);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(layer.source.kind_label()).small().weak());
                    });
                });
            })
            .response
            .interact(egui::Sense::click_and_drag());
        if row.clicked() {
            state.selected = Some(layer.id);
        }
        if row.double_clicked() {
            state.renaming = Some((layer.id, Rename::new(layer.name.clone())));
        }
        row.dnd_set_drag_payload(index);
        if let Some(from) = row.dnd_release_payload::<usize>() {
            moved = Some((*from, index));
        }
        if row.dnd_hover_payload::<usize>().is_some() {
            ui.painter().hline(row.rect.x_range(), row.rect.top(), egui::Stroke::new(2.0, palette.accent));
        }
    }
    if let Some((from, to)) = moved {
        state.move_to(from, to);
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let Some(id) = state.selected else {
            ui.weak("Select a layer to edit it.");
            return;
        };
        if icon_button(ui, icons::ARROW_UP, "Move up (Ctrl+])").clicked() {
            state.restack(id, true);
        }
        if icon_button(ui, icons::ARROW_DOWN, "Move down (Ctrl+[)").clicked() {
            state.restack(id, false);
        }
        if icon_button(ui, icons::COPY, "Duplicate (Ctrl+D)").clicked() {
            state.duplicate(id);
        }
        if icon_button(ui, icons::TRASH, "Delete (Del)").clicked() {
            state.remove(id);
        }
    });
}

fn name(ui: &mut egui::Ui, state: &mut VcamState, layer: &Layer) {
    if let Some((id, rename)) = &mut state.renaming
        && *id == layer.id
    {
        match rename.show(ui, 140.0) {
            Outcome::Editing => {}
            Outcome::Save(name) => {
                if let Some(l) = state.prefs.scene_mut().layer_mut(layer.id) {
                    l.name = name;
                }
                state.renaming = None;
            }
            Outcome::Cancel => state.renaming = None,
        }
        return;
    }
    let label = if layer.name.is_empty() { layer.source.kind_label() } else { &layer.name };
    ui.add(egui::Label::new(label).truncate().selectable(false));
}
