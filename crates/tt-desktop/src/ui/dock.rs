use eframe::egui;
use egui_dock::{DockState, NodePath, TabIndex};
use crate::app::command::Commands;
use crate::app::state::AppState;
use crate::features;
use crate::ui::icons;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Tab {
    Home,
    Qris,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Home, Tab::Qris, Tab::Settings];

    /// The testing tools, in the order Home and the menus list them.
    pub const TOOLS: &'static [Tab] = &[Tab::Qris];

    pub fn name(&self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Qris => "QRIS",
            Tab::Settings => "Settings",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Tab::Home => icons::HOUSE,
            Tab::Qris => icons::QR_CODE,
            Tab::Settings => icons::GEAR,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Tab::Home => "Every tool at a glance",
            Tab::Qris => "Generate and inspect QRIS codes for fictional merchants",
            Tab::Settings => "Theme, output folder and shortcuts",
        }
    }
}

pub fn initial_layout() -> DockState<Tab> {
    DockState::new(vec![Tab::Home])
}

pub fn open_tab(dock: &mut DockState<Tab>, tab: Tab) {
    match dock.find_tab(&tab) {
        Some(path) => {
            let _ = dock.set_active_tab(path);
        }
        None => dock.push_to_focused_leaf(tab),
    }
}

/// egui_dock's tab bar background only senses hover, so a right-click on its empty part goes to an area
/// registered here before the dock draws. Tabs are added later and still take their own clicks.
pub fn tab_bar_menus(ui: &mut egui::Ui, dock: &DockState<Tab>) -> Option<(NodePath, Tab)> {
    let mut picked = None;
    for (path, leaf) in dock.iter_leaves() {
        if !path.surface.is_main() || leaf.tab_bar_hidden || leaf.collapsed {
            continue;
        }
        let bar = egui::Rect::from_min_max(leaf.rect.min, egui::pos2(leaf.rect.max.x, leaf.viewport.min.y));
        if !bar.is_positive() {
            continue;
        }
        let response = ui.interact(bar, ui.id().with(("tab_bar_menu", path)), egui::Sense::click());
        response.context_menu(|ui| {
            if let Some(tab) = panel_menu(ui, |tab| leaf.tabs.contains(&tab)) {
                picked = Some((path, tab));
            }
        });
    }
    picked
}

/// Home, the tools, then Settings; `is_open` ticks the ones already shown. Returns the clicked tab.
pub fn panel_menu(ui: &mut egui::Ui, is_open: impl Fn(Tab) -> bool) -> Option<Tab> {
    let mut picked = None;
    let groups: [&[Tab]; 3] = [&[Tab::Home], Tab::TOOLS, &[Tab::Settings]];
    for (index, group) in groups.into_iter().filter(|g| !g.is_empty()).enumerate() {
        if index > 0 {
            ui.separator();
        }
        for &tab in group {
            if ui.selectable_label(is_open(tab), tab.name()).clicked() {
                picked = Some(tab);
                ui.close();
            }
        }
    }
    picked
}

/// Opens `tab` in the leaf at `target`, moving it there if it's open in another leaf.
pub fn open_in_leaf(dock: &mut DockState<Tab>, target: NodePath, tab: Tab) {
    let anchor = dock.leaf(target).ok().and_then(|leaf| leaf.tabs.first().copied());
    if let Some(from) = dock.find_tab(&tab) {
        if from.surface == target.surface && from.node == target.node {
            let _ = dock.set_active_tab(from);
            return;
        }
        dock.remove_tab(from);
    }
    // Removing the tab can collapse its old leaf and renumber the tree, so find the target again by a tab it holds.
    let target = anchor
        .and_then(|anchor| dock.find_tab(&anchor))
        .map_or(target, |p| NodePath { surface: p.surface, node: p.node });
    if let Ok(leaf) = dock.leaf_mut(target) {
        leaf.append_tab(tab);
    }
    dock.set_focused_node_and_surface(target);
}

pub fn focus_tab_at(dock: &mut DockState<Tab>, index: usize) {
    let Some(path) = dock.focused_leaf() else { return };
    if let Ok(leaf) = dock.leaf_mut(path)
        && index < leaf.tabs.len()
    {
        leaf.active = TabIndex(index);
    }
}

pub fn cycle_tab(dock: &mut DockState<Tab>, forward: bool) {
    let Some(path) = dock.focused_leaf() else { return };
    if let Ok(leaf) = dock.leaf_mut(path) {
        let count = leaf.tabs.len();
        if count > 0 {
            let step = if forward { 1 } else { count - 1 };
            leaf.active = TabIndex((leaf.active.0 + step) % count);
        }
    }
}

pub struct TabViewer<'a> {
    pub state: &'a mut AppState,
    pub cmds: &'a mut Commands,
    pub focused: Option<Tab>,
    pub visuals: egui::Visuals,
}

impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        tab.name().into()
    }

    fn on_tab_button(&mut self, tab: &mut Tab, response: &egui::Response) {
        if self.focused == Some(*tab) {
            let rect = response.rect;
            let accent = self.visuals.selection.stroke.color;
            response.ctx.layer_painter(response.layer_id).hline(rect.x_range(), rect.bottom() - 1.0, egui::Stroke::new(2.0, accent));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            Tab::Home => features::home::show(ui, self.cmds),
            Tab::Qris => features::qris::show(ui, &mut self.state.qris, &self.state.settings),
            Tab::Settings => features::settings::show(ui, &mut self.state.settings),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_dock::{NodeIndex, TabViewer as _};

    fn leaf_of(dock: &DockState<Tab>, tab: Tab) -> NodePath {
        let p = dock.find_tab(&tab).unwrap();
        NodePath { surface: p.surface, node: p.node }
    }

    #[test]
    fn opening_into_a_leaf_moves_the_tab_and_collapses_the_old_leaf() {
        let mut dock = DockState::new(vec![Tab::Home]);
        dock.main_surface_mut().split_right(NodeIndex::root(), 0.5, vec![Tab::Settings]);
        let left = leaf_of(&dock, Tab::Home);

        open_in_leaf(&mut dock, left, Tab::Settings);
        let home_leaf = leaf_of(&dock, Tab::Home);
        assert_eq!(leaf_of(&dock, Tab::Settings), home_leaf);
        assert_eq!(dock.iter_leaves().count(), 1);
        let leaf = dock.leaf(home_leaf).unwrap();
        assert_eq!(leaf.tabs[leaf.active.0], Tab::Settings);
    }

    /// Draws the shell and every tab in both themes, to catch panics in view code.
    #[test]
    fn every_tab_and_the_shell_render_in_both_themes() {
        let mut state = AppState::default();
        let dock = initial_layout();
        let ctx = egui::Context::default();
        crate::ui::theme::install_fonts(&ctx);
        crate::ui::theme::apply_style(&ctx);
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
            ..Default::default()
        };

        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            ctx.set_theme(theme);
            // The second frame lays out with sizes measured in the first.
            for _ in 0..2 {
                let mut cmds = Commands::default();
                let mut output = ctx.run_ui(input(), |ui| {
                    crate::ui::top_bar::show(ui, &dock, &mut cmds);
                    for mut tab in Tab::ALL {
                        let visuals = ui.visuals().clone();
                        let mut viewer = TabViewer { state: &mut state, cmds: &mut cmds, focused: None, visuals };
                        ui.push_id(tab, |ui| viewer.ui(ui, &mut tab));
                    }
                });
                output.textures_delta.clear();
            }
        }
    }
}
