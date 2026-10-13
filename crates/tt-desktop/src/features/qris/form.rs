use eframe::egui::{self, RichText};
use tt_core::generator::{random_seed, rng};
use tt_core::qris::{self, Merchant, Tip, data};
use crate::ui::icons;
use crate::ui::theme::Tone;
use crate::widgets::button::icon_button;
use crate::widgets::segmented::segmented;

const DEFAULT_AMOUNT: u64 = 25_000;

/// Name, amount and randomize: what VCam's quick create and the top of the full form show.
pub fn quick(ui: &mut egui::Ui, m: &mut Merchant) -> bool {
    let mut changed = false;
    if icon_button(ui, icons::DICE_FIVE, "Random merchant").clicked() {
        *m = Merchant::random(&mut rng(random_seed()));
        changed = true;
    }
    egui::Grid::new("qris_quick").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Name");
        changed |= name_field(ui, &mut m.name);
        ui.end_row();
        ui.label("Amount");
        changed |= amount_fields(ui, m);
        ui.end_row();
    });
    changed
}

pub fn full(ui: &mut egui::Ui, m: &mut Merchant) -> bool {
    let mut changed = false;
    egui::Grid::new("qris_full").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Name");
        changed |= name_field(ui, &mut m.name);
        ui.end_row();
        ui.label("City");
        changed |= upper_field(ui, &mut m.city, qris::CITY_MAX, "Max 15 characters");
        ui.end_row();
        ui.label("Postal code");
        changed |= digits_field(ui, &mut m.postal, 10);
        ui.end_row();
        ui.label("Amount");
        changed |= amount_fields(ui, m);
        ui.end_row();
        ui.label("Tip");
        changed |= tip_fields(ui, &mut m.tip);
        ui.end_row();
    });
    egui::CollapsingHeader::new("Advanced").id_salt("qris_advanced").show(ui, |ui| {
        egui::Grid::new("qris_advanced_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Category (MCC)");
            changed |= code_combo(ui, "qris_mcc", &mut m.mcc, data::MCCS);
            ui.end_row();
            ui.label("Criteria");
            changed |= code_combo(ui, "qris_criteria", &mut m.criteria, data::CRITERIA);
            ui.end_row();
            ui.label("NMID");
            changed |= ui.add(egui::TextEdit::singleline(&mut m.nmid).char_limit(15)).changed();
            ui.end_row();
            ui.label("PAN");
            changed |= pan_field(ui, &mut m.pan);
            ui.end_row();
            ui.label("Merchant ID");
            changed |= digits_field(ui, &mut m.mid, 15);
            ui.end_row();
        });
    });
    changed
}

fn name_field(ui: &mut egui::Ui, name: &mut String) -> bool {
    upper_field(ui, name, qris::NAME_MAX, "Max 25 characters")
}

fn upper_field(ui: &mut egui::Ui, text: &mut String, max: usize, hint: &str) -> bool {
    let changed = ui.add(egui::TextEdit::singleline(text).char_limit(max).hint_text(hint)).changed();
    if changed {
        *text = text.to_uppercase();
    }
    changed
}

fn digits_field(ui: &mut egui::Ui, text: &mut String, max: usize) -> bool {
    let changed = ui.add(egui::TextEdit::singleline(text).char_limit(max)).changed();
    if changed {
        text.retain(|c| c.is_ascii_digit());
    }
    changed
}

fn amount_fields(ui: &mut egui::Ui, m: &mut Merchant) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        if segmented(ui, &mut m.dynamic, &[(false, "Flexible"), (true, "Fixed")]) {
            changed = true;
            if m.dynamic && m.amount.is_none() {
                m.amount = Some(DEFAULT_AMOUNT);
            }
        }
        if m.dynamic {
            let mut amount = m.amount.unwrap_or(DEFAULT_AMOUNT);
            let drag = egui::DragValue::new(&mut amount).range(1..=9_999_999_999_999u64).speed(100.0).prefix("Rp ");
            if ui.add(drag).changed() {
                m.amount = Some(amount);
                changed = true;
            }
        }
    })
    .response
    .on_hover_text("Flexible: static code, the payer types the amount. Fixed: dynamic code with the amount inside.");
    changed
}

fn tip_fields(ui: &mut egui::Ui, tip: &mut Tip) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let label = match tip {
            Tip::None => "None",
            Tip::Prompt => "Ask payer",
            Tip::Fixed(_) => "Fixed",
            Tip::Percent(_) => "Percent",
        };
        egui::ComboBox::from_id_salt("qris_tip").selected_text(label).show_ui(ui, |ui| {
            let options = [
                (Tip::None, "None"),
                (Tip::Prompt, "Ask payer"),
                (Tip::Fixed(2_000), "Fixed"),
                (Tip::Percent("5".into()), "Percent"),
            ];
            for (option, text) in options {
                let same_kind = std::mem::discriminant(tip) == std::mem::discriminant(&option);
                if ui.selectable_label(same_kind, text).clicked() && !same_kind {
                    *tip = option;
                    changed = true;
                }
            }
        });
        match tip {
            Tip::Fixed(value) => changed |= ui.add(egui::DragValue::new(value).speed(100.0).prefix("Rp ")).changed(),
            Tip::Percent(value) => changed |= ui.add(egui::TextEdit::singleline(value).desired_width(48.0).char_limit(5)).changed(),
            Tip::None | Tip::Prompt => {}
        }
    });
    changed
}

fn code_combo(ui: &mut egui::Ui, id: &str, code: &mut String, options: &[(&str, &str)]) -> bool {
    let mut changed = false;
    let label = options.iter().find(|(c, _)| c == code).map_or_else(|| code.clone(), |(c, d)| format!("{c}  {d}"));
    egui::ComboBox::from_id_salt(id).selected_text(label).width(200.0).show_ui(ui, |ui| {
        for (option, description) in options {
            if ui.selectable_label(code == option, format!("{option}  {description}")).clicked() && code != option {
                *code = (*option).to_string();
                changed = true;
            }
        }
    });
    changed
}

fn pan_field(ui: &mut egui::Ui, pan: &mut String) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= digits_field(ui, pan, 19);
        if qris::luhn_valid(pan) {
            ui.label(RichText::new(icons::CHECK).color(Tone::Success.color(ui.visuals()))).on_hover_text("Luhn check digit is valid");
        } else if pan.len() >= 2 {
            ui.label(RichText::new(icons::WARNING).color(Tone::Warn.color(ui.visuals()))).on_hover_text("Luhn check digit is wrong");
            if ui.small_button("Fix").on_hover_text("Replace the last digit with the right check digit").clicked() {
                let body = &pan[..pan.len() - 1];
                *pan = format!("{body}{}", qris::luhn_digit(body));
                changed = true;
            }
        }
    });
    changed
}
