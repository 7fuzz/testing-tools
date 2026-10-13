use crate::ui::dock::Tab;

#[derive(Clone, Debug, PartialEq)]
pub enum AppCommand {
    OpenPalette,
    OpenTab(Tab),
    FocusTabAt(usize),
    CycleTab { forward: bool },
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
