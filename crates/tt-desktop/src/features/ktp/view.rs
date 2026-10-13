use std::path::{Path, PathBuf};
use eframe::egui::{self, RichText};
use image::RgbaImage;
use tt_core::generator::{random_seed, rng};
use tt_core::ktp::identity::IdentityParams;
use tt_core::ktp::{self, KtpParams, today};
use super::{KtpState, Rendered, form};
use crate::app::command::{AppCommand, Commands};
use crate::features::settings::SettingsPrefs;
use crate::ui::icons;
use crate::util::job::{Job, take_finished};
use crate::util::{clipboard, output};
use crate::widgets::button::{icon_button, primary};
use crate::widgets::section::{section, section_with};
use crate::widgets::toast;

const MAX_BATCH: u32 = 500;

impl KtpState {
    fn key(&self) -> String {
        serde_json::to_string(&(&self.prefs.params, self.prefs.seed, self.prefs.selfie)).unwrap_or_default()
    }

    /// Renders on this thread; the view normally renders in the background.
    #[cfg(test)]
    pub fn render_now(&mut self) {
        let key = self.key();
        let result = render(&self.prefs.params, self.prefs.seed, self.prefs.selfie);
        self.finish(key, result);
    }

    fn finish(&mut self, key: String, result: Rendered) {
        match result {
            Ok(images) => {
                self.images = Some(images);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
        self.shown_key = Some(key);
        self.card.clear();
        self.selfie.clear();
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some((key, job)) = &self.rendering
            && let Some(result) = job.poll()
        {
            let key = key.clone();
            self.rendering = None;
            self.finish(key, result);
        }
        let key = self.key();
        if self.rendering.is_none() && self.shown_key.as_ref() != Some(&key) {
            let (params, seed, selfie) = (self.prefs.params.clone(), self.prefs.seed, self.prefs.selfie);
            self.rendering = Some((key, Job::spawn(ctx, move || render(&params, seed, selfie))));
        }
        if let Some((card, selfie)) = &self.images {
            if !self.card.is_current("card") {
                self.card.set(ctx, "card", card, egui::TextureOptions::LINEAR);
            }
            if let Some(selfie) = selfie
                && !self.selfie.is_current("selfie")
            {
                self.selfie.set(ctx, "selfie", selfie, egui::TextureOptions::LINEAR);
            }
        }
    }
}

fn render(params: &KtpParams, seed: u64, with_selfie: bool) -> Rendered {
    let set = ktp::render_set(params, seed, with_selfie).map_err(|e| e.to_string())?;
    Ok((set.card, set.selfie))
}

pub fn show(ui: &mut egui::Ui, state: &mut KtpState, settings: &SettingsPrefs, cmds: &mut Commands) {
    state.poll(ui.ctx());
    if let Some(result) = take_finished(&mut state.exporting) {
        toast::push(ui.ctx(), result);
    }
    let output_dir = settings.output_dir();
    egui::Panel::left("ktp_form").resizable(true).default_size(420.0).size_range(340.0..=600.0).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| sidebar(ui, state, &output_dir));
    });
    egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(egui::Margin { left: 12, ..Default::default() })).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| preview(ui, state, &output_dir, cmds));
    });
}

fn sidebar(ui: &mut egui::Ui, state: &mut KtpState, output_dir: &Path) {
    let prefs = &mut state.prefs;
    section_with(ui, "Person", |ui| {
        if icon_button(ui, icons::DICE_FIVE, "Random person and cat").clicked() {
            prefs.params = KtpParams::random(&mut rng(random_seed()), today());
        }
    });
    form::full(ui, &mut prefs.params);

    section(ui, "Photo");
    form::cat(ui, &mut prefs.params);

    section(ui, "Card");
    ui.horizontal(|ui| {
        ui.label("Seed");
        ui.add(egui::DragValue::new(&mut prefs.seed)).on_hover_text("Background, signature, NIK serial and selfie pose");
        if icon_button(ui, icons::DICE_FIVE, "New seed").clicked() {
            prefs.seed = random_seed();
        }
        ui.checkbox(&mut prefs.selfie, "Selfie");
    });

    section(ui, "Files");
    ui.horizontal_wrapped(|ui| {
        if ui.button("Import data.json…").on_hover_text("EKTP-Generator field names").clicked() {
            match import_json(prefs.seed) {
                Ok(Some(params)) => prefs.params.identity = params,
                Ok(None) => {}
                Err(error) => toast::push(ui.ctx(), Err(error)),
            }
        }
        if ui.button("Export data.json…").clicked() {
            match export_json(&prefs.params, prefs.seed, output_dir) {
                Ok(Some(path)) => toast::push(ui.ctx(), Ok(format!("Saved {}", path.display()))),
                Ok(None) => {}
                Err(error) => toast::push(ui.ctx(), Err(error)),
            }
        }
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut prefs.batch_count).range(1..=MAX_BATCH).suffix(" cards"));
        let busy = state.exporting.is_some();
        if ui.add_enabled(!busy, egui::Button::new("Export random batch")).on_hover_text(output_dir.display().to_string()).clicked() {
            let (count, selfie, dir) = (prefs.batch_count, prefs.selfie, output_dir.to_path_buf());
            state.exporting = Some(Job::spawn(ui.ctx(), move || export_batch(count, selfie, &dir)));
        }
        if busy {
            ui.spinner();
        }
    });
}

fn preview(ui: &mut egui::Ui, state: &mut KtpState, output_dir: &Path, cmds: &mut Commands) {
    if let Some(error) = &state.error {
        ui.label(RichText::new(format!("{} {error}", icons::WARNING)).color(ui.visuals().error_fg_color));
    }
    let width = ui.available_width();
    let (card_width, selfie_width) = if state.prefs.selfie { (width * 0.62, width * 0.34) } else { (width.min(900.0), 0.0) };
    ui.horizontal_top(|ui| {
        state.card.show_fit(ui, egui::vec2(card_width, f32::INFINITY));
        if state.prefs.selfie {
            state.selfie.show_fit(ui, egui::vec2(selfie_width, card_width * 0.63 * 1.6));
        }
        if state.rendering.is_some() {
            ui.spinner();
        }
    });
    ui.add_space(8.0);
    let Some((card, selfie)) = &state.images else { return };
    ui.horizontal_wrapped(|ui| {
        if ui.add(primary(ui, &format!("{} Save", icons::DOWNLOAD_SIMPLE))).on_hover_text(output_dir.display().to_string()).clicked() {
            toast::push(ui.ctx(), save_pair(output_dir, card, selfie.as_ref()));
        }
        if ui.button(format!("{} Copy card", icons::COPY)).clicked() {
            toast::push(ui.ctx(), clipboard::copy_image(card).map(|_| "Card copied".into()));
        }
        if let Some(selfie) = selfie
            && ui.button(format!("{} Copy selfie", icons::COPY)).clicked()
        {
            toast::push(ui.ctx(), clipboard::copy_image(selfie).map(|_| "Selfie copied".into()));
        }
        if ui.button(format!("{} Send to camera", icons::VIDEO_CAMERA)).on_hover_text("Add the selfie (or the card) as a virtual camera layer").clicked() {
            let generator = if state.prefs.selfie { "ktp-selfie" } else { "ktp" };
            let params = tt_core::generator::to_json(&state.prefs.params);
            cmds.push(AppCommand::SendToCamera { generator: generator.into(), params, seed: state.prefs.seed });
        }
        if ui.button("Save card as…").clicked() {
            match output::save_png_as(output_dir, "ktp", card) {
                Ok(Some(path)) => toast::push(ui.ctx(), Ok(format!("Saved {}", path.display()))),
                Ok(None) => {}
                Err(error) => toast::push(ui.ctx(), Err(error)),
            }
        }
    });
    if let Ok(resolved) = state.prefs.params.resolve(state.prefs.seed, today()) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.weak("NIK");
            ui.add(egui::Label::new(RichText::new(&resolved.identity.nik).monospace()).selectable(true));
            if icon_button(ui, icons::COPY, "Copy NIK").clicked() {
                ui.ctx().copy_text(resolved.identity.nik.clone());
            }
            ui.weak("Name");
            ui.add(egui::Label::new(RichText::new(&resolved.identity.name).monospace()).selectable(true));
            if icon_button(ui, icons::COPY, "Copy name").clicked() {
                ui.ctx().copy_text(resolved.identity.name.clone());
            }
        });
    }
}

/// Saves the card, and the selfie beside it as `<card>_selfie.png`.
fn save_pair(dir: &Path, card: &RgbaImage, selfie: Option<&RgbaImage>) -> Result<String, String> {
    let path = output::save_png_in(dir, "ktp", card)?;
    if let Some(selfie) = selfie {
        let selfie_path = selfie_path(&path);
        selfie.save(&selfie_path).map_err(|e| format!("Couldn't save {}: {e}", selfie_path.display()))?;
    }
    Ok(format!("Saved {}", path.display()))
}

fn selfie_path(card: &Path) -> PathBuf {
    let stem = card.file_stem().and_then(|s| s.to_str()).unwrap_or("ktp");
    card.with_file_name(format!("{stem}_selfie.png"))
}

fn export_batch(count: u32, with_selfie: bool, dir: &Path) -> Result<String, String> {
    let mut manifest = Vec::new();
    for _ in 0..count {
        let (params, seed) = (KtpParams::random(&mut rng(random_seed()), today()), random_seed());
        let set = ktp::render_set(&params, seed, with_selfie).map_err(|e| e.to_string())?;
        let path = output::save_png_in(dir, "ktp", &set.card)?;
        if let Some(selfie) = &set.selfie {
            let selfie_path = selfie_path(&path);
            selfie.save(&selfie_path).map_err(|e| format!("Couldn't save {}: {e}", selfie_path.display()))?;
        }
        let mut row = ktp::to_ektp_json(&set.identity);
        row["file"] = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().into();
        manifest.push(row);
    }
    let manifest_path = dir.join(output::file_name("ktp-batch", "json"));
    let json = serde_json::to_string_pretty(&manifest).unwrap_or_default();
    std::fs::write(&manifest_path, json).map_err(|e| format!("Couldn't write {}: {e}", manifest_path.display()))?;
    Ok(format!("Saved {count} cards and {}", manifest_path.display()))
}

/// Missing fields are filled now and pinned, so the import doesn't drift with the seed later.
fn import_json(seed: u64) -> Result<Option<IdentityParams>, String> {
    let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() else { return Ok(None) };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("Not JSON: {e}"))?;
    let given = ktp::from_ektp_json(&value);
    let resolved = ktp::identity::resolve(&given, &mut rng(seed), today());
    Ok(Some(IdentityParams { nik: given.nik, ..IdentityParams::pinned(&resolved) }))
}

fn export_json(params: &KtpParams, seed: u64, dir: &Path) -> Result<Option<PathBuf>, String> {
    let identity = params.resolve(seed, today()).map_err(|e| e.to_string())?.identity;
    let Some(path) = rfd::FileDialog::new().set_directory(dir).set_file_name("data.json").add_filter("JSON", &["json"]).save_file()
    else {
        return Ok(None);
    };
    let json = serde_json::to_string_pretty(&ktp::to_ektp_json(&identity)).unwrap_or_default();
    std::fs::write(&path, json).map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
    Ok(Some(path))
}
