use std::sync::atomic::Ordering;
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use tt_core::vcam::engine::{Output, Recording, Stats};
use tt_core::vcam::scene::Scene;
use super::{FRAME_RATES, RESOLUTIONS, VcamState, add, canvas, layers, now_ms, properties, scenes};
use crate::app::command::Commands;
use crate::features::settings::SettingsPrefs;
use crate::ui::icons;
use crate::ui::theme::Tone;
use crate::util::{clipboard, output};
use crate::widgets::button::{icon_button, primary};
use crate::widgets::section::section;
use crate::widgets::toast;

pub fn show(ui: &mut egui::Ui, state: &mut VcamState, settings: &SettingsPrefs, cmds: &mut Commands) {
    state.shown_at.store(now_ms(ui.ctx()), Ordering::Relaxed);
    // A static scene sends no new frames, so refresh the stats line on a slow timer.
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    add::poll(ui, state);
    sync_engine(ui.ctx(), state);
    let stats = state.engine.as_ref().map(|e| e.stats()).unwrap_or_default();

    egui::Panel::top("vcam_toolbar").show(ui, |ui| {
        ui.add_space(2.0);
        toolbar(ui, state, &stats, settings);
        ui.add_space(4.0);
    });
    egui::Panel::left("vcam_left").resizable(true).default_size(250.0).size_range(200.0..=420.0).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            section(ui, "Scenes");
            scenes::show(ui, state);
            crate::widgets::section::section_with(ui, "Layers", |ui| add::menu(ui, state));
            layers::show(ui, state, &stats.errors);
        });
    });
    egui::Panel::right("vcam_properties").resizable(true).default_size(300.0).size_range(240.0..=460.0).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| properties::show(ui, state, &stats.sizes, cmds));
    });
    egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(4)).show(ui, |ui| {
        let preview = state.preview.as_ref().map(|(_, texture)| texture.clone());
        let active = state.prefs.active.min(state.prefs.scenes.len() - 1);
        let scene = &mut state.prefs.scenes[active];
        let out = canvas::show(ui, &mut state.canvas, scene, &mut state.selected, preview.as_ref());
        files_dropped(ui, state, &out);
        if out.response.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    out.response.id,
                    egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, escape: true, tab: false },
                )
            });
            keyboard(ui, state);
        }
    });

    let interacting = ui.input(|i| i.pointer.any_down()) || ui.ctx().egui_wants_keyboard_input();
    let scene = state.prefs.scene().clone();
    state.history.observe(&scene, interacting);
    add::quick_create(ui.ctx(), state);
    sync_engine(ui.ctx(), state);
}

fn sync_engine(ctx: &egui::Context, state: &mut VcamState) {
    let scene = state.prefs.scene().clone();
    state.engine(ctx);
    if state.sent.as_ref() != Some(&scene) {
        state.engine.as_ref().expect("started").set_scene(&scene);
        state.sent = Some(scene);
    }
    let engine = state.engine.as_ref().expect("started");
    if let Some(frame) = engine.preview()
        && state.preview.as_ref().is_none_or(|(generation, _)| *generation != frame.generation)
    {
        let size = [frame.image.width() as usize, frame.image.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, frame.image.as_raw());
        match &mut state.preview {
            Some((generation, texture)) => {
                texture.set(color, egui::TextureOptions::LINEAR);
                *generation = frame.generation;
            }
            None => state.preview = Some((frame.generation, ctx.load_texture("vcam_preview", color, egui::TextureOptions::LINEAR))),
        }
    }
}

fn toolbar(ui: &mut egui::Ui, state: &mut VcamState, stats: &Stats, settings: &SettingsPrefs) {
    ui.horizontal_wrapped(|ui| {
        let outputs = state.output_devices();
        if state.prefs.device.as_ref().is_none_or(|d| !outputs.iter().any(|o| &o.path == d)) {
            state.prefs.device = outputs.iter().find(|o| o.is_loopback()).or(outputs.first()).map(|o| o.path.clone());
        }
        let live = matches!(stats.output, Output::Live { .. });
        if outputs.is_empty() {
            ui.label(RichText::new(format!("{} No virtual camera device", icons::WARNING)).color(Tone::Warn.color(ui.visuals())));
            if ui.button("Copy setup command").on_hover_text(tt_core::vcam::v4l2::LOOPBACK_HINT).clicked() {
                ui.ctx().copy_text(tt_core::vcam::v4l2::LOOPBACK_HINT.to_string());
                toast::push(ui.ctx(), Ok("Copied; run it in a terminal, then rescan".into()));
            }
        } else {
            let current = state.prefs.device.clone().unwrap_or_default();
            let label = outputs.iter().find(|o| o.path == current).map_or(current.clone(), |o| format!("{}  {}", o.label, o.path));
            ui.add_enabled_ui(!live, |ui| {
                egui::ComboBox::from_id_salt("vcam_device").selected_text(label).width(220.0).show_ui(ui, |ui| {
                    for device in &outputs {
                        ui.selectable_value(&mut state.prefs.device, Some(device.path.clone()), format!("{}  {}", device.label, device.path));
                    }
                });
            });
        }
        if icon_button(ui, icons::ARROW_CLOCKWISE, "Rescan devices").clicked() {
            state.devices = None;
        }

        let scene = state.prefs.scene_mut();
        let label = format!("{}×{}", scene.width, scene.height);
        egui::ComboBox::from_id_salt("vcam_resolution").selected_text(label).width(100.0).show_ui(ui, |ui| {
            for (w, h) in RESOLUTIONS {
                if ui.selectable_label((scene.width, scene.height) == (w, h), format!("{w}×{h}")).clicked() {
                    resize_scene(scene, w, h);
                }
            }
        });
        egui::ComboBox::from_id_salt("vcam_fps").selected_text(format!("{} fps", scene.fps)).width(70.0).show_ui(ui, |ui| {
            for fps in FRAME_RATES {
                ui.selectable_value(&mut scene.fps, fps, format!("{fps} fps"));
            }
        });

        if live {
            if ui.button(format!("{} Stop camera", icons::STOP)).clicked()
                && let Some(engine) = &state.engine
            {
                engine.stop_output();
            }
        } else if let Some(device) = state.prefs.device.clone()
            && ui.add(primary(ui, &format!("{} Start camera", icons::PLAY))).clicked()
            && let Some(engine) = &state.engine
        {
            engine.start_output(&device);
        }

        if icon_button(ui, icons::CAMERA, "Snapshot: save the current frame and copy it").clicked() {
            snapshot(ui, state, &settings.output_dir());
        }
        record_button(ui, state, stats, &settings.output_dir());
        ui.add_enabled_ui(state.history.can_undo(), |ui| {
            if icon_button(ui, icons::ARROW_U_UP_LEFT, "Undo (Ctrl+Z)").clicked() {
                undo(state, false);
            }
        });
        ui.add_enabled_ui(state.history.can_redo(), |ui| {
            if icon_button(ui, icons::ARROW_U_UP_RIGHT, "Redo (Ctrl+Shift+Z)").clicked() {
                undo(state, true);
            }
        });
        status(ui, stats);
    });
}

fn record_button(ui: &mut egui::Ui, state: &mut VcamState, stats: &Stats, dir: &std::path::Path) {
    let Some(engine) = &state.engine else { return };
    match &stats.recording {
        Recording::Active { frames, dropped, .. } => {
            let fps = state.prefs.scene().fps.max(1) as u64;
            let secs = frames / fps;
            let label = format!("{} Stop {:02}:{:02}", icons::STOP, secs / 60, secs % 60);
            let hint = if *dropped > 0 { format!("{dropped} frames dropped (encoder too slow)") } else { "Stop recording".into() };
            if ui.button(RichText::new(label).color(Tone::Danger.color(ui.visuals()))).on_hover_text(hint).clicked() {
                engine.stop_recording();
            }
        }
        Recording::Saving(_) => {
            ui.spinner();
            ui.weak("Saving video…");
        }
        _ => {
            if icon_button(ui, icons::RECORD, "Record the output to MP4 in the output folder").clicked() {
                let _ = std::fs::create_dir_all(dir);
                engine.start_recording(&dir.join(output::file_name("vcam", "mp4")));
            }
        }
    }
    // Report a finished recording once.
    if state.last_recording.as_ref() != Some(&stats.recording) {
        match &stats.recording {
            Recording::Saved(path) => toast::push(ui.ctx(), Ok(format!("Saved {}", path.display()))),
            Recording::Failed(error) => toast::push(ui.ctx(), Err(format!("Recording failed: {error}"))),
            _ => {}
        }
        state.last_recording = Some(stats.recording.clone());
    }
}

fn status(ui: &mut egui::Ui, stats: &Stats) {
    let visuals = ui.visuals().clone();
    let timing = format!("{:.0} fps · {:.1} ms", stats.fps, stats.frame_ms);
    match &stats.output {
        Output::Live { device, frames, held_at, .. } => {
            ui.label(RichText::new(format!("● LIVE {device}")).color(Tone::Danger.color(&visuals)).strong());
            ui.label(RichText::new(format!("{timing} · {frames} frames")).small().weak());
            if let Some((w, h)) = held_at {
                ui.label(RichText::new(format!("{} held at {w}×{h}", icons::WARNING)).color(Tone::Warn.color(&visuals))).on_hover_text(
                    "A camera app is reading the device, so it can't change size. Frames are scaled to fit until that app releases the camera.",
                );
            }
        }
        Output::Failed(error) => {
            ui.label(RichText::new(format!("{} {error}", icons::WARNING)).color(visuals.error_fg_color));
        }
        Output::Off => {
            ui.label(RichText::new(format!("Preview only · {timing}")).small().weak());
        }
    }
    if stats.loading > 0 {
        ui.spinner();
    }
}

/// Keeps the layout when the canvas changes size. Layers that cover the whole canvas keep covering it;
/// the rest keep their relative centre and scale by the geometric mean of the two factors, so switching
/// to another size and back restores the scene exactly.
pub fn resize_scene(scene: &mut Scene, width: u32, height: u32) {
    let (old_w, old_h) = (scene.width.max(1) as f32, scene.height.max(1) as f32);
    let (sx, sy) = (width as f32 / old_w, height as f32 / old_h);
    let factor = (sx * sy).sqrt();
    let near = |a: f32, b: f32| (a - b).abs() < 1.0;
    for layer in &mut scene.layers {
        let t = &mut layer.transform;
        let full = near(t.x, 0.0) && near(t.y, 0.0) && near(t.w, old_w) && near(t.h, old_h) && t.rotation == 0.0;
        if full {
            (t.w, t.h) = (width as f32, height as f32);
            continue;
        }
        let (cx, cy) = t.center();
        t.w *= factor;
        t.h *= factor;
        t.x = cx * sx - t.w / 2.0;
        t.y = cy * sy - t.h / 2.0;
    }
    scene.width = width;
    scene.height = height;
}

fn snapshot(ui: &egui::Ui, state: &VcamState, dir: &std::path::Path) {
    let Some(frame) = state.engine.as_ref().and_then(|e| e.preview()) else { return };
    let saved = output::save_png_in(dir, "vcam", &frame.image);
    let copied = clipboard::copy_image(&frame.image);
    let message = match (saved, copied) {
        (Ok(path), Ok(())) => Ok(format!("Saved {} and copied it", path.display())),
        (Ok(path), Err(_)) => Ok(format!("Saved {}", path.display())),
        (Err(error), _) => Err(error),
    };
    toast::push(ui.ctx(), message);
}

fn undo(state: &mut VcamState, redo: bool) {
    let mut scene = state.prefs.scene().clone();
    let changed = if redo { state.history.redo(&mut scene) } else { state.history.undo(&mut scene) };
    if changed {
        *state.prefs.scene_mut() = scene;
        if state.selected.is_some_and(|id| state.prefs.scene().layer(id).is_none()) {
            state.selected = None;
        }
    }
}

fn keyboard(ui: &mut egui::Ui, state: &mut VcamState) {
    let pressed = |ui: &mut egui::Ui, modifiers: Modifiers, key: Key| ui.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(modifiers, key)));
    if pressed(ui, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || pressed(ui, Modifiers::COMMAND, Key::Y) {
        undo(state, true);
    }
    if pressed(ui, Modifiers::COMMAND, Key::Z) {
        undo(state, false);
    }
    let digits = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
    for (index, key) in digits.into_iter().enumerate() {
        if pressed(ui, Modifiers::COMMAND, key) {
            state.switch_scene(index);
        }
    }
    let Some(id) = state.selected else { return };
    if pressed(ui, Modifiers::NONE, Key::Escape) {
        state.selected = None;
        return;
    }
    if pressed(ui, Modifiers::NONE, Key::Delete) || pressed(ui, Modifiers::NONE, Key::Backspace) {
        state.remove(id);
        return;
    }
    if pressed(ui, Modifiers::COMMAND, Key::D) {
        state.duplicate(id);
    }
    if pressed(ui, Modifiers::COMMAND, Key::CloseBracket) {
        state.restack(id, true);
    }
    if pressed(ui, Modifiers::COMMAND, Key::OpenBracket) {
        state.restack(id, false);
    }
    let shift = ui.input(|i| i.modifiers.shift);
    let step = if shift { 10.0 } else { 1.0 };
    let (mut dx, mut dy) = (0.0, 0.0);
    ui.input(|i| {
        for (key, delta) in [(Key::ArrowLeft, (-step, 0.0)), (Key::ArrowRight, (step, 0.0)), (Key::ArrowUp, (0.0, -step)), (Key::ArrowDown, (0.0, step))] {
            if i.key_pressed(key) {
                dx += delta.0;
                dy += delta.1;
            }
        }
    });
    if (dx, dy) != (0.0, 0.0)
        && let Some(layer) = state.prefs.scene_mut().layer_mut(id)
        && !layer.locked
    {
        layer.transform.x += dx;
        layer.transform.y += dy;
    }
}

fn files_dropped(ui: &mut egui::Ui, state: &mut VcamState, out: &canvas::CanvasOutput) {
    let hovering = ui.input(|i| !i.raw.hovered_files.is_empty());
    if hovering {
        ui.painter().rect_filled(out.rect, 4, egui::Color32::from_black_alpha(120));
        ui.painter().text(out.rect.center(), egui::Align2::CENTER_CENTER, "Drop to add as layers", egui::FontId::proportional(18.0), egui::Color32::WHITE);
    }
    let dropped: Vec<std::path::PathBuf> = ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
    if dropped.is_empty() {
        return;
    }
    let at = ui.input(|i| i.pointer.hover_pos()).filter(|p| out.rect.contains(*p)).map(|p| out.view.to_scene(p));
    add::add_files(ui, state, dropped, at);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tt_core::vcam::scene::{Layer, Source, Transform};

    #[test]
    fn resizing_keeps_full_canvas_layers_full_and_round_trips() {
        let mut scene = Scene { width: 1280, height: 720, ..Default::default() };
        scene.layers.push(Layer::new("full", Source::TestPattern, Transform { w: 1280.0, h: 720.0, ..Default::default() }));
        scene.layers.push(Layer::new("qr", Source::TestPattern, Transform { x: 424.0, y: 144.0, w: 432.0, h: 432.0, ..Default::default() }));
        let original = scene.clone();
        resize_scene(&mut scene, 640, 480);
        let full = scene.layers[0].transform;
        assert_eq!((full.x, full.y, full.w, full.h), (0.0, 0.0, 640.0, 480.0));
        let qr = scene.layers[1].transform;
        assert!((qr.center().0 - 320.0).abs() < 0.01 && (qr.center().1 - 240.0).abs() < 0.01, "{qr:?}");
        resize_scene(&mut scene, 1280, 720);
        for (a, b) in scene.layers.iter().zip(&original.layers) {
            let (a, b) = (a.transform, b.transform);
            assert!((a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01 && (a.w - b.w).abs() < 0.01 && (a.h - b.h).abs() < 0.01, "{a:?} vs {b:?}");
        }
    }
}
