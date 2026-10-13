use eframe::egui::{self, Align2, Key, Modifiers, RichText};
use crate::app::command::{AppCommand, Commands};
use crate::ui::shortcuts;

#[derive(Default)]
pub struct PaletteState {
    pub open: bool,
    query: String,
    selected: usize,
    /// Set when the keyboard moves the selection; scrolling every frame would undo mouse-wheel scrolling.
    scroll_to_selected: bool,
}

impl PaletteState {
    pub fn open(&mut self) {
        *self = Self { open: true, ..Default::default() };
    }
}

#[derive(Clone)]
pub struct PaletteItem {
    pub label: String,
    pub command: AppCommand,
    pub shortcut: Option<String>,
}

pub fn palette_items(ctx: &egui::Context) -> Vec<PaletteItem> {
    shortcuts::palette_actions()
        .into_iter()
        .map(|action| PaletteItem {
            label: action.label(),
            command: action.command(),
            shortcut: shortcuts::shortcut_for(action).map(|s| ctx.format_shortcut(&s)),
        })
        .collect()
}

pub fn show(ctx: &egui::Context, state: &mut PaletteState, cmds: &mut Commands) {
    if !state.open {
        return;
    }
    let all_items = palette_items(ctx);
    let matches = matching(&all_items, &state.query);
    state.selected = state.selected.min(matches.len().saturating_sub(1));

    let id = egui::Id::new("command_palette");
    let area = egui::Modal::default_area(id).anchor(Align2::CENTER_TOP, egui::vec2(0.0, 80.0));
    let mut chosen = None;
    let modal = egui::Modal::new(id).area(area).show(ctx, |ui| {
        ui.set_width(420.0);
        let search = crate::widgets::search::search_field(ui, &mut state.query, "Type a command");
        search.request_focus();
        if search.changed() {
            state.selected = 0;
            state.scroll_to_selected = true;
        }
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                state.selected = (state.selected + 1).min(matches.len().saturating_sub(1));
                state.scroll_to_selected = true;
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                state.selected = state.selected.saturating_sub(1);
                state.scroll_to_selected = true;
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                chosen = matches.get(state.selected).cloned();
            }
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for (index, item) in matches.iter().enumerate() {
                let selected = index == state.selected;
                let response = egui::Sides::new().show(
                    ui,
                    |ui| ui.add(egui::Button::selectable(selected, &item.label).frame_when_inactive(false)),
                    |ui| {
                        if let Some(shortcut) = &item.shortcut {
                            ui.label(RichText::new(shortcut).weak().monospace());
                        }
                    },
                );
                if selected && std::mem::take(&mut state.scroll_to_selected) {
                    response.0.scroll_to_me(None);
                }
                if response.0.clicked() {
                    chosen = Some(item.clone());
                }
            }
        });
    });

    if let Some(item) = chosen {
        cmds.push(item.command);
        state.open = false;
    } else if modal.should_close() {
        state.open = false;
    }
}

fn matching(items: &[PaletteItem], query: &str) -> Vec<PaletteItem> {
    let query = query.trim().to_lowercase();
    let mut scored: Vec<(usize, PaletteItem)> = items
        .iter()
        .filter_map(|item| fuzzy_score(&item.label.to_lowercase(), &query).map(|score| (score, item.clone())))
        .collect();
    scored.sort_by_key(|(score, _)| *score);
    scored.into_iter().map(|(_, item)| item).collect()
}

// Lower is better: substring hits rank first, then subsequences by how spread out they are.
fn fuzzy_score(label: &str, query: &str) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }
    if let Some(pos) = label.find(query) {
        return Some(pos);
    }
    let mut chars = label.char_indices();
    let mut first = None;
    let mut last = 0;
    for q in query.chars() {
        let (i, _) = chars.by_ref().find(|(_, c)| *c == q)?;
        first.get_or_insert(i);
        last = i;
    }
    Some(1000 + last - first.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::dock::Tab;

    #[test]
    fn ranks_substring_before_subsequence() {
        let substring = fuzzy_score("open settings", "set").unwrap();
        let subsequence = fuzzy_score("open settings", "ost").unwrap();
        assert!(substring < subsequence);
        assert_eq!(fuzzy_score("open home", "xyz"), None);
    }

    #[test]
    fn palette_opens_every_tab() {
        let items = palette_items(&egui::Context::default());
        for tab in Tab::ALL {
            assert!(items.iter().any(|i| i.command == AppCommand::OpenTab(tab)), "{tab:?} missing");
        }
    }
}
