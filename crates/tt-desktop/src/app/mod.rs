pub mod command;
pub mod prefs;
pub mod state;

use eframe::egui;
use egui_dock::{DockArea, DockState};
use command::{AppCommand, Commands};
use state::AppState;
use crate::ui::dock::{self, Tab, TabViewer};
use crate::ui::palette::{self, PaletteState};
use crate::ui::{shortcuts, theme, top_bar};

pub struct App {
    dock: DockState<Tab>,
    state: AppState,
    cmds: Commands,
    palette: PaletteState,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_style(&cc.egui_ctx);
        // egui otherwise keeps every widget's saved state in app.ron until each type passes 256 KB.
        cc.egui_ctx.data_mut(|d| d.set_max_bytes_per_type(16 * 1024));
        let mut state = AppState::default();
        let dock = prefs::restore(cc.storage, &mut state);
        Self { dock, state, cmds: Commands::default(), palette: PaletteState::default() }
    }

    fn run_command(&mut self, cmd: AppCommand) {
        match cmd {
            AppCommand::OpenPalette => self.palette.open(),
            AppCommand::OpenTab(tab) => dock::open_tab(&mut self.dock, tab),
            AppCommand::FocusTabAt(index) => dock::focus_tab_at(&mut self.dock, index),
            AppCommand::CycleTab { forward } => dock::cycle_tab(&mut self.dock, forward),
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        prefs::persist(storage, &self.dock, &self.state);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let focused_tab = self.dock.find_active_focused().map(|(_, tab)| *tab);
        shortcuts::consume(ui.ctx(), &mut self.cmds);
        let palette = theme::Palette::of(ui.visuals());
        egui::Panel::top("top_panel")
            .frame(egui::Frame::new().fill(palette.base).inner_margin(egui::Margin::symmetric(8, 5)))
            .show(ui, |ui| top_bar::show(ui, &self.dock, &mut self.cmds));

        if let Some((path, tab)) = dock::tab_bar_menus(ui, &self.dock) {
            dock::open_in_leaf(&mut self.dock, path, tab);
        }
        let mut viewer = TabViewer {
            state: &mut self.state,
            cmds: &mut self.cmds,
            focused: focused_tab,
            visuals: ui.visuals().clone(),
        };
        DockArea::new(&mut self.dock).style(theme::dock_style(ui.style())).show_inside(ui, &mut viewer);
        palette::show(ui.ctx(), &mut self.palette, &mut self.cmds);
        crate::widgets::toast::show(ui.ctx());

        let cmds = self.cmds.take();
        if !cmds.is_empty() {
            for cmd in cmds {
                self.run_command(cmd);
            }
            ui.ctx().request_repaint();
        }
    }
}
