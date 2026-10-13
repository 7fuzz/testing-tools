use eframe::egui::{self, RichText};
use crate::app::command::{AppCommand, Commands};
use crate::ui::dock::Tab;
use crate::ui::theme::{self, Palette};
use crate::widgets::section::section;

const CARD_WIDTH: f32 = 240.0;

pub fn show(ui: &mut egui::Ui, cmds: &mut Commands) {
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        section(ui, "Tools");
        if Tab::TOOLS.is_empty() {
            ui.weak("No tools yet. QRIS, KTP specimen and the virtual camera land here as they're built.");
        } else {
            cards(ui, Tab::TOOLS, cmds);
        }
        section(ui, "App");
        cards(ui, &[Tab::Settings], cmds);
    });
}

fn cards(ui: &mut egui::Ui, tabs: &[Tab], cmds: &mut Commands) {
    ui.horizontal_wrapped(|ui| {
        for &tab in tabs {
            if card(ui, tab).clicked() {
                cmds.push(AppCommand::OpenTab(tab));
            }
        }
    });
}

fn card(ui: &mut egui::Ui, tab: Tab) -> egui::Response {
    let palette = Palette::of(ui.visuals());
    let frame = egui::Frame::new()
        .fill(palette.raised)
        .stroke(egui::Stroke::new(1.0, palette.border))
        .corner_radius(6)
        .inner_margin(egui::Margin::same(12));
    let response = frame
        .show(ui, |ui| {
            ui.set_width(CARD_WIDTH);
            ui.horizontal(|ui| {
                ui.label(RichText::new(tab.icon()).size(22.0).color(palette.accent));
                ui.vertical(|ui| {
                    ui.label(RichText::new(tab.name()).family(theme::strong_family()));
                    ui.label(RichText::new(tab.description()).small().weak());
                });
            });
        })
        .response
        .interact(egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_stroke(response.rect, 6, egui::Stroke::new(1.0, palette.accent), egui::StrokeKind::Inside);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
