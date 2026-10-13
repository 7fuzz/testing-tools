use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use crate::app::command::{AppCommand, Commands};
use crate::ui::dock::Tab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    OpenPalette,
    OpenTab(Tab),
    FocusTabAt(usize),
    NextTab,
    PreviousTab,
}

impl Action {
    pub fn label(self) -> String {
        match self {
            Action::OpenPalette => "Command palette".to_string(),
            Action::OpenTab(tab) => format!("Open {}", tab.name()),
            Action::FocusTabAt(index) => format!("Tab {}", index + 1),
            Action::NextTab => "Next tab".to_string(),
            Action::PreviousTab => "Previous tab".to_string(),
        }
    }

    pub fn command(self) -> AppCommand {
        match self {
            Action::OpenPalette => AppCommand::OpenPalette,
            Action::OpenTab(tab) => AppCommand::OpenTab(tab),
            Action::FocusTabAt(index) => AppCommand::FocusTabAt(index),
            Action::NextTab => AppCommand::CycleTab { forward: true },
            Action::PreviousTab => AppCommand::CycleTab { forward: false },
        }
    }
}

pub const SHORTCUTS: &[(KeyboardShortcut, Action)] = &[
    (KeyboardShortcut::new(Modifiers::COMMAND, Key::K), Action::OpenPalette),
    (KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma), Action::OpenTab(Tab::Settings)),
    // Shift variants first: consume_shortcut also matches a shortcut when extra shift is held.
    (KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), Key::Tab), Action::PreviousTab),
    (KeyboardShortcut::new(Modifiers::CTRL, Key::Tab), Action::NextTab),
    (KeyboardShortcut::new(Modifiers::ALT, Key::Num1), Action::FocusTabAt(0)),
    (KeyboardShortcut::new(Modifiers::ALT, Key::Num2), Action::FocusTabAt(1)),
    (KeyboardShortcut::new(Modifiers::ALT, Key::Num3), Action::FocusTabAt(2)),
    (KeyboardShortcut::new(Modifiers::ALT, Key::Num4), Action::FocusTabAt(3)),
    (KeyboardShortcut::new(Modifiers::ALT, Key::Num5), Action::FocusTabAt(4)),
    (KeyboardShortcut::new(Modifiers::ALT, Key::Num6), Action::FocusTabAt(5)),
    (KeyboardShortcut::new(Modifiers::ALT, Key::Num7), Action::FocusTabAt(6)),
    (KeyboardShortcut::new(Modifiers::ALT, Key::Num8), Action::FocusTabAt(7)),
    (KeyboardShortcut::new(Modifiers::ALT, Key::Num9), Action::FocusTabAt(8)),
];

/// Rows for the Settings list: keys and action. Alt+N runs collapse into one row.
pub fn listing(ctx: &egui::Context) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut focus_keys: Vec<String> = Vec::new();
    for &(shortcut, action) in SHORTCUTS {
        let keys = ctx.format_shortcut(&shortcut);
        if let Action::FocusTabAt(_) = action {
            focus_keys.push(keys);
            continue;
        }
        rows.push((keys, action.label()));
    }
    if let (Some(first), Some(last)) = (focus_keys.first(), focus_keys.last()) {
        rows.push((format!("{first} … {last}"), "Focus tab by position".to_string()));
    }
    rows
}

pub fn shortcut_for(action: Action) -> Option<KeyboardShortcut> {
    SHORTCUTS.iter().find(|(_, a)| *a == action).map(|(shortcut, _)| *shortcut)
}

pub fn palette_actions() -> Vec<Action> {
    let mut actions: Vec<Action> = Tab::ALL.into_iter().map(Action::OpenTab).collect();
    actions.extend([Action::NextTab, Action::PreviousTab]);
    actions
}

pub fn consume(ctx: &egui::Context, cmds: &mut Commands) {
    for &(shortcut, action) in SHORTCUTS {
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut)) {
            cmds.push(action.command());
        }
    }
}
