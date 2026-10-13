use eframe::egui::{self, RichText};
use tt_core::qris::tlv::{self, CrcCheck, Field};
use crate::ui::icons;
use crate::ui::theme::Tone;

/// The parsed TLV fields of `payload` as a table, then the CRC verdict.
pub fn show(ui: &mut egui::Ui, payload: &str) {
    let fields = match tlv::parse(payload.trim()) {
        Ok(fields) => fields,
        Err(error) => {
            ui.label(RichText::new(format!("{} Can't parse: {error}", icons::WARNING)).color(Tone::Danger.color(ui.visuals())));
            return;
        }
    };
    crc_line(ui, payload.trim());
    ui.add_space(6.0);
    egui::Grid::new("qris_fields").num_columns(3).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
        for field in &fields {
            row(ui, &field.tag, tlv::tag_name(&field.tag), field, field.children.is_empty());
            for child in &field.children {
                let name = tlv::sub_tag_name(&field.tag, &child.tag);
                row(ui, &format!("  └ {}", child.tag), name, child, true);
            }
        }
    });
}

fn row(ui: &mut egui::Ui, tag: &str, name: &str, field: &Field, show_value: bool) {
    ui.label(RichText::new(tag).monospace().weak());
    ui.label(name);
    if show_value {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(RichText::new(&field.value).monospace()).selectable(true));
            if let Some(description) = tlv::describe(&field.tag, &field.value) {
                ui.weak(description);
            }
        });
    } else {
        ui.label("");
    }
    ui.end_row();
}

fn crc_line(ui: &mut egui::Ui, payload: &str) {
    let visuals = ui.visuals().clone();
    let (icon, tone, text) = match tlv::check_crc(payload) {
        CrcCheck::Valid => (icons::CHECK, Tone::Success, "CRC valid".to_string()),
        CrcCheck::Invalid { expected, found } => (icons::WARNING, Tone::Danger, format!("CRC wrong: has {found}, should be {expected}")),
        CrcCheck::Missing => (icons::WARNING, Tone::Warn, "No CRC (tag 63) at the end".to_string()),
    };
    ui.label(RichText::new(format!("{icon} {text}")).color(tone.color(&visuals)));
}
