use std::path::Path;
use eframe::egui::{self, RichText};
use tt_core::qris::{self, Merchant, tlv};
use super::{Mode, QrisState, fields, form, staging};
use crate::app::command::{AppCommand, Commands};
use crate::features::settings::SettingsPrefs;
use crate::ui::icons;
use crate::util::job::{Job, take_finished};
use crate::util::{clipboard, output};
use crate::widgets::button::icon_button;
use crate::widgets::section::{section, section_with};
use crate::widgets::segmented::segmented;
use crate::widgets::toast;

const PREVIEW_MAX: f32 = 340.0;

pub fn show(ui: &mut egui::Ui, state: &mut QrisState, settings: &SettingsPrefs, cmds: &mut Commands) {
    if let Some(result) = take_finished(&mut state.decoding) {
        match result {
            Ok(text) => state.prefs.inspect_input = text,
            Err(error) => toast::push(ui.ctx(), Err(error)),
        }
    }
    egui::Panel::top("qris_mode").show(ui, |ui| {
        ui.add_space(2.0);
        segmented(ui, &mut state.prefs.mode, &[(Mode::Generate, "Generate"), (Mode::Inspect, "Inspect")]);
        ui.add_space(4.0);
    });
    match state.prefs.mode {
        Mode::Generate => generate(ui, state, &settings.output_dir(), cmds),
        Mode::Inspect => inspect(ui, state),
    }
}

fn generate(ui: &mut egui::Ui, state: &mut QrisState, output_dir: &Path, cmds: &mut Commands) {
    egui::Panel::left("qris_form").resizable(true).default_size(360.0).size_range(300.0..=560.0).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            section_with(ui, "Merchant", |ui| {
                if icon_button(ui, icons::DICE_FIVE, "Random merchant").clicked() {
                    state.prefs.merchant = Merchant::random(&mut tt_core::generator::rng(tt_core::generator::random_seed()));
                }
            });
            form::full(ui, &mut state.prefs.merchant);
        });
    });

    let payload = state.prefs.merchant.payload();
    refresh_preview(ui.ctx(), state, &payload);
    egui::CentralPanel::default().frame(content_frame()).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.add_space(4.0);
            let side = ui.available_width().min(PREVIEW_MAX);
            ui.vertical_centered(|ui| state.preview.show_fit(ui, egui::vec2(side, side)));
            ui.add_space(8.0);
            ui.add(egui::Label::new(RichText::new(&payload).family(egui::FontFamily::Monospace).size(crate::ui::theme::SMALL_SIZE)).selectable(true).wrap());
            ui.add_space(6.0);
            actions(ui, state, &payload, output_dir, cmds);
            section(ui, "Fields");
            fields::show(ui, &payload);
            section(ui, "Staging");
            staging::show(ui, &mut state.staging, &mut state.prefs.staging, &payload);
        });
    });
}

fn content_frame() -> egui::Frame {
    egui::Frame::new().inner_margin(egui::Margin { left: 12, ..Default::default() })
}

fn refresh_preview(ctx: &egui::Context, state: &mut QrisState, payload: &str) {
    if state.preview.is_current(payload) {
        return;
    }
    match tt_core::qr::render(payload, qris::QR_SCALE, qris::QR_MARGIN) {
        Ok(image) => {
            state.preview.set(ctx, payload, &image, egui::TextureOptions::NEAREST);
            state.image = Some(image);
        }
        Err(error) => {
            state.preview.clear();
            state.image = None;
            toast::push(ctx, Err(error.to_string()));
        }
    }
}

fn actions(ui: &mut egui::Ui, state: &mut QrisState, payload: &str, output_dir: &Path, cmds: &mut Commands) {
    ui.horizontal_wrapped(|ui| {
        if ui.button(format!("{} Copy payload", icons::COPY)).clicked() {
            ui.ctx().copy_text(payload.to_string());
            toast::push(ui.ctx(), Ok("Payload copied".into()));
        }
        let Some(image) = &state.image else { return };
        if ui.button(format!("{} Copy image", icons::IMAGE)).clicked() {
            toast::push(ui.ctx(), clipboard::copy_image(image).map(|_| "Image copied".into()));
        }
        if ui.button(format!("{} Save", icons::DOWNLOAD_SIMPLE)).on_hover_text(output_dir.display().to_string()).clicked() {
            let saved = output::save_png_in(output_dir, "qris", image);
            toast::push(ui.ctx(), saved.map(|p| format!("Saved {}", p.display())));
        }
        if ui.button("Save as…").clicked() {
            match output::save_png_as(output_dir, "qris", image) {
                Ok(Some(path)) => toast::push(ui.ctx(), Ok(format!("Saved {}", path.display()))),
                Ok(None) => {}
                Err(error) => toast::push(ui.ctx(), Err(error)),
            }
        }
        if ui.button(format!("{} Send to camera", icons::VIDEO_CAMERA)).on_hover_text("Add as a layer in the virtual camera").clicked() {
            let params = tt_core::generator::to_json(&state.prefs.merchant);
            cmds.push(AppCommand::SendToCamera { generator: "qris".into(), params, seed: 0 });
        }
        if ui.button(format!("{} Inspect", icons::MAGNIFYING_GLASS)).clicked() {
            state.prefs.inspect_input = payload.to_string();
            state.prefs.mode = Mode::Inspect;
        }
    });
}

fn inspect(ui: &mut egui::Ui, state: &mut QrisState) {
    egui::Panel::left("qris_inspect_input").resizable(true).default_size(360.0).size_range(300.0..=560.0).show(ui, |ui| {
        section(ui, "Payload");
        ui.add(
            egui::TextEdit::multiline(&mut state.prefs.inspect_input)
                .font(egui::TextStyle::Monospace)
                .desired_rows(8)
                .desired_width(f32::INFINITY)
                .hint_text("Paste a QRIS string, or load a QR image"),
        );
        ui.horizontal_wrapped(|ui| {
            if ui.button(format!("{} Open image…", icons::FOLDER_OPEN)).clicked()
                && let Some(path) = output::pick_image()
            {
                state.decoding = Some(Job::spawn(ui.ctx(), move || decode_file(&path)));
            }
            if ui.button(format!("{} Paste image", icons::IMAGE)).clicked() {
                state.decoding = Some(Job::spawn(ui.ctx(), decode_clipboard));
            }
            if state.decoding.is_some() {
                ui.spinner();
            }
        });
        let input = state.prefs.inspect_input.trim().to_string();
        if input.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if matches!(tlv::check_crc(&input), tlv::CrcCheck::Invalid { .. } | tlv::CrcCheck::Missing)
                && ui.button("Fix CRC").on_hover_text("Recompute tag 63").clicked()
            {
                state.prefs.inspect_input = tlv::fix_crc(&input);
            }
            if let Ok(merchant) = Merchant::from_payload(&input)
                && ui.button(format!("{} Edit in generator", icons::ARROW_SQUARE_OUT)).clicked()
            {
                state.prefs.merchant = merchant;
                state.prefs.mode = Mode::Generate;
            }
        });
    });
    let input = state.prefs.inspect_input.clone();
    egui::CentralPanel::default().frame(content_frame()).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            if input.trim().is_empty() {
                ui.weak("Nothing to inspect yet.");
                return;
            }
            section(ui, "Fields");
            fields::show(ui, &input);
            section(ui, "Staging");
            staging::show(ui, &mut state.staging, &mut state.prefs.staging, &input);
        });
    });
}

fn decode_file(path: &Path) -> Result<String, String> {
    let image = image::open(path).map_err(|e| format!("Couldn't open {}: {e}", path.display()))?;
    tt_core::qr::decode(&image).map_err(|e| e.to_string())
}

fn decode_clipboard() -> Result<String, String> {
    let image = clipboard::paste_image()?;
    tt_core::qr::decode(&image::DynamicImage::ImageRgba8(image)).map_err(|e| e.to_string())
}
