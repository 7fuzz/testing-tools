use eframe::egui::{self, RichText};
use egui_dock::DockState;
use crate::app::command::{AppCommand, Commands};
use crate::ui::dock::{self, Tab};
use crate::ui::{icons, theme};
use crate::widgets::button::icon_button;

pub fn show(ui: &mut egui::Ui, dock: &DockState<Tab>, cmds: &mut Commands) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new(icons::FLASK).color(theme::Palette::of(ui.visuals()).accent));
        ui.label(RichText::new("Testing Tools").family(theme::strong_family()));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            theme_toggle(ui);
            add_panel_menu(ui, dock, cmds);
            if icon_button(ui, icons::MAGNIFYING_GLASS, "Command palette (Ctrl+K)").clicked() {
                cmds.push(AppCommand::OpenPalette);
            }
        });
    });
}

fn theme_toggle(ui: &mut egui::Ui) {
    let (icon, hint, next) = if ui.visuals().dark_mode {
        (icons::SUN, "Light theme", egui::ThemePreference::Light)
    } else {
        (icons::MOON, "Dark theme", egui::ThemePreference::Dark)
    };
    if icon_button(ui, icon, hint).clicked() {
        ui.ctx().set_theme(next);
    }
}

fn add_panel_menu(ui: &mut egui::Ui, dock: &DockState<Tab>, cmds: &mut Commands) {
    let button = egui::Button::new(icons::PLUS).frame_when_inactive(false);
    let (response, _) = egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
        if let Some(tab) = dock::panel_menu(ui, |tab| dock.find_tab(&tab).is_some()) {
            cmds.push(AppCommand::OpenTab(tab));
        }
    });
    response.on_hover_text("Add panel");
}
