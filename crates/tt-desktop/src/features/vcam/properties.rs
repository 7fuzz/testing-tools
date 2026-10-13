use eframe::egui::{self, RichText};
use tt_core::generator::random_seed;
use tt_core::vcam::scene::{Crop, Layer, Scene, Source, Transform};
use super::VcamState;
use crate::app::command::{AppCommand, Commands};
use crate::generators;
use crate::ui::icons;
use crate::widgets::button::icon_button;
use crate::widgets::section::section;

pub fn show(ui: &mut egui::Ui, state: &mut VcamState, sizes: &std::collections::HashMap<u64, (u32, u32)>, cmds: &mut Commands) {
    let Some(id) = state.selected else {
        ui.weak("Select a layer on the canvas or in the list.");
        scene_settings(ui, state);
        return;
    };
    let webcams = state.webcams();
    let engine = state.engine.as_ref();
    let mut lock_aspect = ui.data(|d| d.get_temp::<bool>(egui::Id::new("vcam_lock_aspect"))).unwrap_or(true);
    let scene: &mut Scene = state.prefs.scene_mut();
    let canvas = (scene.width as f32, scene.height as f32);
    let Some(layer) = scene.layer_mut(id) else {
        state.selected = None;
        return;
    };

    section(ui, "Layer");
    ui.add(egui::TextEdit::singleline(&mut layer.name).hint_text(layer.source.kind_label()));

    section(ui, "Source");
    source(ui, layer, &webcams, cmds);
    if let (Source::Video { .. }, Some(engine)) = (&layer.source, engine)
        && ui.button(format!("{} Restart", icons::ARROW_CLOCKWISE)).clicked()
    {
        engine.reload(layer.id);
    }

    section(ui, "Position");
    let t = &mut layer.transform;
    egui::Grid::new("vcam_transform").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("X");
        ui.add(egui::DragValue::new(&mut t.x).speed(1.0));
        ui.label("Y");
        ui.add(egui::DragValue::new(&mut t.y).speed(1.0));
        ui.end_row();
        let (w0, h0) = (t.w, t.h);
        ui.label("W");
        let w = ui.add(egui::DragValue::new(&mut t.w).speed(1.0).range(1.0..=20_000.0));
        ui.label("H");
        let h = ui.add(egui::DragValue::new(&mut t.h).speed(1.0).range(1.0..=20_000.0));
        ui.end_row();
        if lock_aspect && w.changed() && w0 > 0.0 {
            t.h = h0 * t.w / w0;
        } else if lock_aspect && h.changed() && h0 > 0.0 {
            t.w = w0 * t.h / h0;
        }
        ui.label("");
        ui.checkbox(&mut lock_aspect, "Keep ratio");
        ui.end_row();
        ui.label("Rotate");
        ui.add(egui::DragValue::new(&mut t.rotation).speed(0.5).range(-180.0..=180.0).suffix("°"));
        ui.label("");
        ui.horizontal(|ui| {
            ui.checkbox(&mut t.flip_h, "Flip H");
            ui.checkbox(&mut t.flip_v, "Flip V");
        });
        ui.end_row();
    });
    ui.data_mut(|d| d.insert_temp(egui::Id::new("vcam_lock_aspect"), lock_aspect));
    ui.horizontal(|ui| {
        ui.label("Opacity");
        ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).show_value(true));
    });

    let natural = sizes.get(&layer.id).map(|&(w, h)| (w as f32, h as f32));
    ui.horizontal_wrapped(|ui| {
        let size = natural.unwrap_or((layer.transform.w, layer.transform.h));
        if ui.button("Fit").on_hover_text("Fit inside the canvas").clicked() {
            layer.transform = keep_turn(Transform::fit(size, canvas), &layer.transform);
        }
        if ui.button("Fill").on_hover_text("Cover the canvas, cropping the overflow").clicked() {
            layer.transform = keep_turn(Transform::fill(size, canvas), &layer.transform);
        }
        if ui.button("Stretch").clicked() {
            layer.transform = keep_turn(Transform { w: canvas.0, h: canvas.1, ..Default::default() }, &layer.transform);
        }
        if ui.button("Center").clicked() {
            layer.transform.x = (canvas.0 - layer.transform.w) / 2.0;
            layer.transform.y = (canvas.1 - layer.transform.h) / 2.0;
        }
        if let Some((w, h)) = natural
            && ui.button("Natural size").clicked()
        {
            let (cx, cy) = layer.transform.center();
            layer.transform = Transform { x: cx - w / 2.0, y: cy - h / 2.0, w, h, ..layer.transform };
        }
        if layer.transform.rotation != 0.0 && ui.button("Straighten").clicked() {
            layer.transform.rotation = 0.0;
        }
    });

    section(ui, "Crop");
    crop(ui, &mut layer.crop);
}

/// A new box with the old rotation and flips.
fn keep_turn(new: Transform, old: &Transform) -> Transform {
    Transform { rotation: old.rotation, flip_h: old.flip_h, flip_v: old.flip_v, ..new }
}

fn crop(ui: &mut egui::Ui, crop: &mut Crop) {
    egui::Grid::new("vcam_crop").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        for (label, value) in [("Left", &mut crop.left), ("Top", &mut crop.top), ("Right", &mut crop.right), ("Bottom", &mut crop.bottom)] {
            ui.label(label);
            ui.add(egui::Slider::new(value, 0.0..=0.9).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
            ui.end_row();
        }
    });
    if !crop.is_none() && ui.small_button("Reset crop").clicked() {
        *crop = Crop::default();
    }
}

fn source(ui: &mut egui::Ui, layer: &mut Layer, webcams: &[tt_core::vcam::v4l2::DeviceInfo], cmds: &mut Commands) {
    match &mut layer.source {
        Source::Color { rgba } => {
            let mut color = egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
            if ui.color_edit_button_srgba(&mut color).changed() {
                *rgba = color.to_srgba_unmultiplied();
            }
        }
        Source::TestPattern => {
            ui.weak("Colour bars and a centre cross, drawn at the canvas size.");
        }
        Source::Image { path } => {
            ui.label(RichText::new(path.display().to_string()).small().monospace());
            if ui.button("Replace…").clicked()
                && let Some(picked) = crate::util::output::pick_image()
            {
                *path = picked;
            }
        }
        Source::Video { path, looping, speed } => {
            ui.label(RichText::new(path.display().to_string()).small().monospace());
            ui.horizontal(|ui| {
                ui.checkbox(looping, "Loop");
                ui.label("Speed");
                ui.add(egui::DragValue::new(speed).range(0.1..=8.0).speed(0.05).suffix("×"));
            });
        }
        Source::Generated { generator, params, seed } => {
            let Some(entry) = generators::find(generator) else {
                ui.colored_label(ui.visuals().error_fg_color, format!("Unknown generator {generator}"));
                return;
            };
            ui.label(RichText::new(entry.core.name).strong());
            (entry.quick_form)(ui, params);
            ui.horizontal(|ui| {
                ui.label("Seed");
                ui.add(egui::DragValue::new(seed));
                if icon_button(ui, icons::DICE_FIVE, "New seed").clicked() {
                    *seed = random_seed();
                }
            });
            if ui.button(format!("{} Open in {}", icons::ARROW_SQUARE_OUT, entry.tab.name())).clicked() {
                cmds.push(AppCommand::OpenGenerated { generator: generator.clone(), params: params.clone(), seed: *seed });
            }
        }
        Source::Text(text) => {
            ui.add(egui::TextEdit::multiline(&mut text.content).desired_rows(2).desired_width(f32::INFINITY));
            ui.weak("{time}, {date} and {datetime} update live.");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut text.size).range(6.0..=400.0).suffix(" px"));
                let mut color = egui::Color32::from_rgba_unmultiplied(text.color[0], text.color[1], text.color[2], text.color[3]);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    text.color = color.to_srgba_unmultiplied();
                }
                ui.checkbox(&mut text.bold, "Bold");
                ui.checkbox(&mut text.outline, "Outline");
            });
            ui.horizontal(|ui| {
                let mut boxed = text.background.is_some();
                if ui.checkbox(&mut boxed, "Background").changed() {
                    text.background = boxed.then_some([0, 0, 0, 160]);
                }
                if let Some(bg) = &mut text.background {
                    let mut color = egui::Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]);
                    if ui.color_edit_button_srgba(&mut color).changed() {
                        *bg = color.to_srgba_unmultiplied();
                    }
                }
            });
        }
        Source::Webcam { device } => {
            egui::ComboBox::from_id_salt("vcam_webcam").selected_text(device.as_str()).show_ui(ui, |ui| {
                for cam in webcams {
                    ui.selectable_value(device, cam.path.clone(), format!("{}  {}", cam.label, cam.path));
                }
            });
        }
    }
}

fn scene_settings(ui: &mut egui::Ui, state: &mut VcamState) {
    let scene = state.prefs.scene_mut();
    section(ui, "Scene");
    ui.horizontal(|ui| {
        ui.label("Background");
        let [r, g, b] = scene.background;
        let mut color = egui::Color32::from_rgb(r, g, b);
        if ui.color_edit_button_srgba(&mut color).changed() {
            scene.background = [color.r(), color.g(), color.b()];
        }
    });
    ui.add_space(8.0);
    ui.weak("Drag layers to move them. Corner handles keep the ratio (hold Shift to stretch, Alt to resize from the centre). The round handle rotates (Shift snaps to 15°). Ctrl turns off snapping.");
}
