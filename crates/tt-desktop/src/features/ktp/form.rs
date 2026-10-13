use chrono::{Datelike, NaiveDate};
use eframe::egui::{self, RichText};
use tt_core::generator::{random_seed, rng};
use tt_core::ktp::cat::{CatLook, Fur};
use tt_core::ktp::data::{self, REGIONS};
use tt_core::ktp::identity::{self, Gender, Identity, IdentityParams};
use tt_core::ktp::{KtpParams, today};
use crate::ui::icons;
use crate::ui::theme::Tone;
use crate::widgets::button::icon_button;
use crate::widgets::segmented::segmented;

fn random_identity() -> Identity {
    identity::resolve(&IdentityParams::default(), &mut rng(random_seed()), today())
}

/// Name, gender, city and randomize: VCam's quick create.
pub fn quick(ui: &mut egui::Ui, params: &mut KtpParams) -> bool {
    let mut changed = false;
    if icon_button(ui, icons::DICE_FIVE, "Random person and cat").clicked() {
        *params = KtpParams::random(&mut rng(random_seed()), today());
        changed = true;
    }
    let id = &mut params.identity;
    egui::Grid::new("ktp_quick").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Name");
        changed |= upper_text(ui, &mut id.name);
        ui.end_row();
        ui.label("Gender");
        changed |= gender(ui, id);
        ui.end_row();
        ui.label("City");
        changed |= city_combo(ui, id);
        ui.end_row();
    });
    changed
}

pub fn full(ui: &mut egui::Ui, params: &mut KtpParams) -> bool {
    let mut changed = false;
    let id = &mut params.identity;
    egui::Grid::new("ktp_full").num_columns(3).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Name");
        changed |= upper_text(ui, &mut id.name);
        if dice(ui) {
            id.name = Some(random_name_for(id.gender));
            changed = true;
        }
        ui.end_row();

        ui.label("Gender");
        changed |= gender(ui, id);
        ui.end_row();

        ui.label("NIK");
        changed |= nik(ui, id);
        ui.end_row();

        ui.label("Birth place");
        changed |= upper_text(ui, &mut id.birth_place);
        if dice(ui) {
            id.birth_place = Some(random_identity().birth_place);
            changed = true;
        }
        ui.end_row();

        ui.label("Birth date");
        changed |= date(ui, "ktp_birth", &mut id.birth);
        if dice(ui) {
            id.birth = Some(random_identity().birth);
            changed = true;
        }
        ui.end_row();

        ui.label("Blood type");
        changed |= choice(ui, "ktp_blood", &mut id.blood, data::BLOOD);
        ui.end_row();

        ui.label("City");
        changed |= city_combo(ui, id);
        ui.end_row();

        ui.label("District");
        changed |= district_combo(ui, id);
        ui.end_row();

        ui.label("Village");
        changed |= village_combo(ui, id);
        ui.end_row();

        ui.label("Address");
        changed |= upper_text(ui, &mut id.address);
        if dice(ui) {
            id.address = Some(random_identity().address);
            changed = true;
        }
        ui.end_row();

        ui.label("RT/RW");
        changed |= upper_text(ui, &mut id.rtrw);
        if dice(ui) {
            id.rtrw = Some(random_identity().rtrw);
            changed = true;
        }
        ui.end_row();

        ui.label("Religion");
        changed |= choice(ui, "ktp_religion", &mut id.religion, data::RELIGIONS);
        ui.end_row();

        ui.label("Marital status");
        changed |= choice(ui, "ktp_marital", &mut id.marital, data::MARITAL);
        ui.end_row();

        ui.label("Job");
        changed |= job(ui, &mut id.job);
        if dice(ui) {
            id.job = Some(random_identity().job);
            changed = true;
        }
        ui.end_row();

        ui.label("Issued");
        changed |= date(ui, "ktp_issued", &mut id.issued);
        if dice(ui) {
            id.issued = Some(random_identity().issued);
            changed = true;
        }
        ui.end_row();
    });
    changed
}

fn dice(ui: &mut egui::Ui) -> bool {
    icon_button(ui, icons::DICE_FIVE, "Random").clicked()
}

fn random_name_for(gender: Option<Gender>) -> String {
    let params = IdentityParams { gender, ..Default::default() };
    identity::resolve(&params, &mut rng(random_seed()), today()).name
}

pub fn cat(ui: &mut egui::Ui, params: &mut KtpParams) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut drawn = params.photo.is_none();
        if segmented(ui, &mut drawn, &[(true, "Drawn cat"), (false, "Photo file")]) {
            changed = true;
            if drawn {
                params.photo = None;
            } else if let Some(path) = crate::util::output::pick_image() {
                params.photo = Some(path);
            }
        }
        if icon_button(ui, icons::DICE_FIVE, "Another cat").clicked() {
            params.look = Some(CatLook::random(&mut rng(random_seed())));
            changed = true;
        }
    });
    if let Some(path) = params.photo.clone() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(path.display().to_string()).small().weak());
            if ui.small_button("Change…").clicked()
                && let Some(picked) = crate::util::output::pick_image()
            {
                params.photo = Some(picked);
                changed = true;
            }
        });
    } else if let Some(look) = &mut params.look {
        egui::ComboBox::from_id_salt("ktp_fur").selected_text(look.fur.label()).show_ui(ui, |ui| {
            for fur in Fur::ALL {
                if ui.selectable_label(look.fur == fur, fur.label()).clicked() && look.fur != fur {
                    look.fur = fur;
                    changed = true;
                }
            }
        });
    }
    changed
}

fn upper_text(ui: &mut egui::Ui, value: &mut Option<String>) -> bool {
    let text = value.get_or_insert_with(String::new);
    let changed = ui.add(egui::TextEdit::singleline(text).desired_width(220.0)).changed();
    if changed {
        *text = text.to_uppercase();
    }
    changed
}

fn gender(ui: &mut egui::Ui, id: &mut IdentityParams) -> bool {
    let mut current = id.gender.unwrap_or_default();
    let options: Vec<(Gender, &str)> = Gender::ALL.iter().map(|g| (*g, g.label())).collect();
    let changed = segmented(ui, &mut current, &options);
    if changed {
        id.gender = Some(current);
    }
    changed
}

fn nik(ui: &mut egui::Ui, id: &mut IdentityParams) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut auto = id.nik.is_none();
        if ui.checkbox(&mut auto, "Auto").on_hover_text("Built from region, birth date and gender, plus a serial from the seed").changed() {
            id.nik = if auto { None } else { Some(String::new()) };
            changed = true;
        }
        if let Some(nik) = &mut id.nik {
            if ui.add(egui::TextEdit::singleline(nik).char_limit(16).desired_width(150.0)).changed() {
                nik.retain(|c| c.is_ascii_digit());
                changed = true;
            }
            if let Err(problem) = identity::parse_nik(nik) {
                ui.label(RichText::new(icons::WARNING).color(Tone::Warn.color(ui.visuals()))).on_hover_text(problem);
            }
        }
    });
    changed
}

/// Day, month and year spinners; an impossible day (31 February) is pulled back to the month's last day.
fn date(ui: &mut egui::Ui, id: &str, value: &mut Option<NaiveDate>) -> bool {
    let current = value.unwrap_or_else(today);
    let (mut d, mut m, mut y) = (current.day(), current.month(), current.year());
    let mut changed = false;
    ui.push_id(id, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            changed |= ui.add(egui::DragValue::new(&mut d).range(1..=31)).changed();
            ui.weak("-");
            changed |= ui.add(egui::DragValue::new(&mut m).range(1..=12)).changed();
            ui.weak("-");
            changed |= ui.add(egui::DragValue::new(&mut y).range(1900..=2100)).changed();
        });
    });
    if changed {
        let date = (0..4).find_map(|back| NaiveDate::from_ymd_opt(y, m, d.saturating_sub(back)));
        *value = date.or(*value);
    }
    changed
}

fn choice(ui: &mut egui::Ui, id: &str, value: &mut Option<String>, options: &[&str]) -> bool {
    let mut changed = false;
    let current = value.clone().unwrap_or_default();
    egui::ComboBox::from_id_salt(id).selected_text(&current).width(220.0).show_ui(ui, |ui| {
        for option in options {
            if ui.selectable_label(current == *option, *option).clicked() && current != *option {
                *value = Some((*option).to_string());
                changed = true;
            }
        }
    });
    changed
}

fn job(ui: &mut egui::Ui, value: &mut Option<String>) -> bool {
    let mut options: Vec<&str> = data::JOBS.to_vec();
    options.extend([data::STUDENT, data::RETIRED, data::HOMEMAKER]);
    choice(ui, "ktp_job", value, &options)
}

fn city_combo(ui: &mut egui::Ui, id: &mut IdentityParams) -> bool {
    let mut changed = false;
    let current = id.city.clone().unwrap_or_default();
    egui::ComboBox::from_id_salt("ktp_city").selected_text(&current).width(220.0).show_ui(ui, |ui| {
        for region in REGIONS {
            if ui.selectable_label(current == region.city, region.city).clicked() && current != region.city {
                id.province = Some(region.province.into());
                id.city = Some(region.city.into());
                set_district(id, region, 0);
                changed = true;
            }
        }
    });
    changed
}

fn set_district(id: &mut IdentityParams, region: &data::Region, index: usize) {
    let (district, villages) = region.districts[index];
    id.district = Some(district.into());
    id.village = Some(villages[0].into());
    id.region = Some(format!("{}{:02}", region.code, index + 1));
}

fn district_combo(ui: &mut egui::Ui, id: &mut IdentityParams) -> bool {
    let Some(region) = id.city.as_deref().and_then(identity::region_for_city) else {
        return upper_text(ui, &mut id.district);
    };
    let mut changed = false;
    let current = id.district.clone().unwrap_or_default();
    egui::ComboBox::from_id_salt("ktp_district").selected_text(&current).width(220.0).show_ui(ui, |ui| {
        for (index, (district, _)) in region.districts.iter().enumerate() {
            if ui.selectable_label(current == *district, *district).clicked() && current != *district {
                set_district(id, region, index);
                changed = true;
            }
        }
    });
    changed
}

fn village_combo(ui: &mut egui::Ui, id: &mut IdentityParams) -> bool {
    let villages = id
        .city
        .as_deref()
        .and_then(identity::region_for_city)
        .and_then(|r| r.districts.iter().find(|(d, _)| Some(*d) == id.district.as_deref()))
        .map(|(_, v)| *v);
    match villages {
        Some(villages) => choice(ui, "ktp_village", &mut id.village, villages),
        None => upper_text(ui, &mut id.village),
    }
}
