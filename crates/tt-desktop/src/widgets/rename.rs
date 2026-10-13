use eframe::egui::{self, Key};

/// An inline rename field: takes focus once when it appears, saves on Enter or click-away, cancels on Esc.
pub struct Rename {
    pub text: String,
    frames: u8,
}

pub enum Outcome {
    Editing,
    Save(String),
    Cancel,
}

impl Rename {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), frames: 0 }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, width: f32) -> Outcome {
        let edit = ui.add(egui::TextEdit::singleline(&mut self.text).desired_width(width));
        if self.frames == 0 {
            edit.request_focus();
        }
        self.frames = self.frames.saturating_add(1);
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            return Outcome::Cancel;
        }
        // Focus lands a frame after it's requested; after that, losing it (Enter, a click elsewhere) ends the edit.
        if edit.lost_focus() || (self.frames > 2 && !edit.has_focus()) {
            let name = self.text.trim();
            return if name.is_empty() { Outcome::Cancel } else { Outcome::Save(name.to_string()) };
        }
        Outcome::Editing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(events: Vec<Vec<egui::Event>>) -> Vec<&'static str> {
        let ctx = egui::Context::default();
        let mut rename = Rename::new("Old name");
        let mut seen = Vec::new();
        for (i, events) in events.into_iter().enumerate() {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0))),
                events,
                time: Some(i as f64 * 0.1),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                seen.push(match rename.show(ui, 150.0) {
                    Outcome::Editing => "editing",
                    Outcome::Save(_) => "save",
                    Outcome::Cancel => "cancel",
                });
            });
            output.textures_delta.clear();
        }
        seen
    }

    fn key(key: Key) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }
    }

    #[test]
    fn enter_saves_and_escape_cancels() {
        assert_eq!(run(vec![vec![], vec![], vec![key(Key::Enter)]]).last(), Some(&"save"));
        assert_eq!(run(vec![vec![], vec![], vec![key(Key::Escape)]]).last(), Some(&"cancel"));
    }

    #[test]
    fn clicking_elsewhere_saves_and_it_keeps_editing_otherwise() {
        let away = egui::pos2(350.0, 180.0);
        let click = vec![
            egui::Event::PointerMoved(away),
            egui::Event::PointerButton { pos: away, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
            egui::Event::PointerButton { pos: away, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
        ];
        let seen = run(vec![vec![], vec![], vec![], click, vec![]]);
        assert_eq!(&seen[..3], ["editing", "editing", "editing"]);
        assert!(seen[3..].contains(&"save"), "{seen:?}");
    }
}
