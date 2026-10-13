use crate::ui::dock::Tab;

#[derive(Clone, Debug, PartialEq)]
pub enum AppCommand {
    OpenPalette,
    OpenTab(Tab),
    FocusTabAt(usize),
    CycleTab { forward: bool },
    /// Opens a generator's own tab with these params (from a VCam layer).
    OpenGenerated { generator: String, params: serde_json::Value, seed: u64 },
    /// Opens VCam's quick create prefilled with these params (from a generator tab).
    SendToCamera { generator: String, params: serde_json::Value, seed: u64 },
}

#[derive(Default)]
pub struct Commands(Vec<AppCommand>);

impl Commands {
    pub fn push(&mut self, cmd: impl Into<AppCommand>) {
        self.0.push(cmd.into());
    }

    pub fn take(&mut self) -> Vec<AppCommand> {
        std::mem::take(&mut self.0)
    }
}
