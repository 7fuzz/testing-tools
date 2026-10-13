use std::path::Path;
use eframe::egui::{self, RichText};
use tt_core::generator::random_seed;
use tt_core::vcam::scene::{Layer, Scene, Source, TextSource, Transform};
use tt_core::vcam::video;
use super::{Draft, VcamState};
use crate::generators::{self, GeneratorEntry};
use crate::ui::icons;
use crate::util::job::{Job, take_finished};
use crate::widgets::button::primary;
use crate::widgets::toast;

/// Media larger than this share of the canvas is scaled down to fit when added.
const MAX_SHARE: f32 = 0.8;

/// Centred at natural size, or scaled to fit if it would cover most of the canvas.
pub fn placement(size: (u32, u32), scene: &Scene) -> Transform {
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let (cw, ch) = (scene.width as f32, scene.height as f32);
    let scale = (cw * MAX_SHARE / w).min(ch * MAX_SHARE / h).min(1.0);
    let (w, h) = (w * scale, h * scale);
    Transform { x: (cw - w) / 2.0, y: (ch - h) / 2.0, w, h, ..Default::default() }
}

fn stem(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("File").to_string()
}

/// A layer for a dropped or picked file: video by extension, otherwise an image.
pub fn media_layer(path: &Path, scene: &Scene) -> Result<Layer, String> {
    if video::is_video_path(path) {
        let size = video::probe_size(path).map_err(|e| format!("{e:#}"))?;
        let source = Source::Video { path: path.to_path_buf(), looping: true, speed: 1.0 };
        Ok(Layer::new(stem(path), source, placement(size, scene)))
    } else {
        let size = image::image_dimensions(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
        Ok(Layer::new(stem(path), Source::Image { path: path.to_path_buf() }, placement(size, scene)))
    }
}

/// Probing a video takes a moment, so files are turned into layers off the UI thread.
pub fn add_files(ui: &egui::Ui, state: &mut VcamState, paths: Vec<std::path::PathBuf>, at: Option<(f32, f32)>) {
    let scene = state.prefs.scene().clone();
    state.pending_add = Some(Job::spawn(ui.ctx(), move || {
        let mut layers = Vec::new();
        let mut errors = Vec::new();
        for (i, path) in paths.iter().enumerate() {
            match media_layer(path, &scene) {
                Ok(mut layer) => {
                    if let Some((x, y)) = at {
                        layer.transform.x = x - layer.transform.w / 2.0 + i as f32 * 20.0;
                        layer.transform.y = y - layer.transform.h / 2.0 + i as f32 * 20.0;
                    }
                    layers.push(layer);
                }
                Err(error) => errors.push(error),
            }
        }
        (layers, errors)
    }));
}

pub fn poll(ui: &egui::Ui, state: &mut VcamState) {
    if let Some((layers, errors)) = take_finished(&mut state.pending_add) {
        for layer in layers {
            state.add_layer(layer);
        }
        if !errors.is_empty() {
            toast::push(ui.ctx(), Err(errors.join("\n")));
        }
    }
}

pub fn menu(ui: &mut egui::Ui, state: &mut VcamState) {
    let button = egui::Button::new(format!("{} Add", icons::PLUS));
    egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
        if ui.button(format!("{} Image or video…", icons::IMAGE)).clicked() {
            ui.close();
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter("Images and videos", &["png", "jpg", "jpeg", "webp", "bmp", "gif", "mp4", "mkv", "webm", "mov", "avi", "m4v"])
                .pick_files()
            {
                add_files(ui, state, paths, None);
            }
        }
        if ui.button(format!("{} Text", icons::TEXT_T)).clicked() {
            ui.close();
            let scene = state.prefs.scene();
            let t = Transform { x: 40.0, y: 40.0, w: 360.0, h: 80.0, ..Default::default() };
            let text = TextSource { content: "{datetime}".into(), ..Default::default() };
            let mut layer = Layer::new("Clock", Source::Text(text), t);
            layer.transform.x = (scene.width as f32 - t.w) / 2.0;
            state.add_layer(layer);
        }
        if ui.button(format!("{} Color", icons::PALETTE)).clicked() {
            ui.close();
            let scene = state.prefs.scene();
            let t = Transform { w: scene.width as f32, h: scene.height as f32, ..Default::default() };
            state.add_layer(Layer::new("Color", Source::Color { rgba: [40, 40, 48, 255] }, t));
        }
        if ui.button(format!("{} Test pattern", icons::SQUARES_FOUR)).clicked() {
            ui.close();
            let scene = state.prefs.scene();
            let t = Transform { w: scene.width as f32, h: scene.height as f32, ..Default::default() };
            state.add_layer(Layer::new("Test pattern", Source::TestPattern, t));
        }
        ui.menu_button(format!("{} Webcam", icons::WEBCAM), |ui| {
            let cams = state.webcams();
            if cams.is_empty() {
                ui.weak("No cameras found");
            }
            for cam in cams {
                if ui.button(format!("{}  {}", cam.label, cam.path)).clicked() {
                    ui.close();
                    let scene = state.prefs.scene();
                    let t = Transform::fit((16.0, 9.0), (scene.width as f32, scene.height as f32));
                    state.add_layer(Layer::new(cam.label.clone(), Source::Webcam { device: cam.path.clone() }, t));
                }
            }
        });
        ui.separator();
        ui.label(RichText::new("GENERATE").small().weak());
        for entry in generators::REGISTRY {
            if ui.button(format!("{} {}…", icons::SHUFFLE, entry.core.name)).clicked() {
                ui.close();
                state.draft = Some(Draft::new(entry));
            }
        }
    });
}

impl Draft {
    pub fn new(entry: &GeneratorEntry) -> Self {
        let seed = random_seed();
        Self {
            generator: entry.core.id,
            params: (entry.core.randomize)(seed),
            seed,
            preview: Default::default(),
            rendering: None,
            rendered: None,
        }
    }

    fn key(&self) -> String {
        format!("{}|{}|{}", self.generator, self.params, self.seed)
    }

    /// Keeps a background render of the current params going; returns the size once rendered.
    fn refresh(&mut self, ctx: &egui::Context, entry: &GeneratorEntry) -> Option<(u32, u32)> {
        if let Some((key, job)) = &self.rendering
            && let Some(result) = job.poll()
        {
            let key = key.clone();
            self.rendering = None;
            match result {
                Ok(image) => {
                    self.preview.set(ctx, "vcam_draft", &image, egui::TextureOptions::LINEAR);
                    self.rendered = Some((key, Ok(image.dimensions())));
                }
                Err(error) => self.rendered = Some((key, Err(error))),
            }
        }
        let key = self.key();
        if self.rendering.is_none() && self.rendered.as_ref().is_none_or(|(k, _)| *k != key) {
            let (generate, params, seed) = (entry.core.generate, self.params.clone(), self.seed);
            self.rendering = Some((key.clone(), Job::spawn(ctx, move || generate(&params, seed).map_err(|e| e.to_string()))));
        }
        match &self.rendered {
            Some((k, Ok(size))) if *k == key => Some(*size),
            _ => None,
        }
    }
}

/// The quick-create popup: the generator's compact form, a preview, and Add layer.
pub fn quick_create(ctx: &egui::Context, state: &mut VcamState) {
    let Some(draft) = &mut state.draft else { return };
    let Some(entry) = generators::find(draft.generator) else {
        state.draft = None;
        return;
    };
    let size = draft.refresh(ctx, entry);
    let mut open = true;
    let mut add = false;
    let mut cancel = false;
    egui::Window::new(format!("Add {}", entry.core.name))
        .id(egui::Id::new("vcam_quick_create"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(360.0)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            (entry.quick_form)(ui, &mut draft.params);
            ui.add_space(6.0);
            let max = egui::vec2(340.0, 260.0);
            if draft.preview.show_fit(ui, max).is_none() || draft.rendering.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.weak("Rendering…");
                });
            }
            if let Some((_, Err(error))) = &draft.rendered {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(size.is_some(), primary(ui, "Add layer")).clicked() {
                    add = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                ui.weak(format!("Opens in full in the {} tab", entry.tab.name()));
            });
        });
    if add && let (Some(size), Some(draft)) = (size, state.draft.take()) {
        let scene = state.prefs.scene();
        let mut t = placement(size, scene);
        // Generated images are often large (selfies are 900×1200); start them at a comfortable size.
        let cap = (scene.height as f32 * 0.6) / t.h;
        if cap < 1.0 {
            t = Transform { w: t.w * cap, h: t.h * cap, ..t };
            t.x = (scene.width as f32 - t.w) / 2.0;
            t.y = (scene.height as f32 - t.h) / 2.0;
        }
        let source = Source::Generated { generator: draft.generator.to_string(), params: draft.params, seed: draft.seed };
        state.add_layer(Layer::new(entry.core.name, source, t));
    } else if cancel || !open {
        state.draft = None;
    }
}
