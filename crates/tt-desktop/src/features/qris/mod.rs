mod fields;
mod form;
mod staging;
mod view;

use eframe::egui;
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use tt_core::qris::{Merchant, Qris};
use crate::generators::GeneratorUi;
use crate::ui::dock::Tab;
use crate::util::job::Job;
use crate::util::texture::CachedTexture;
pub use view::show;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Generate,
    Inspect,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct QrisPrefs {
    pub mode: Mode,
    pub merchant: Merchant,
    pub inspect_input: String,
    pub staging: staging::StagingPrefs,
}

#[derive(Default)]
pub struct QrisState {
    pub prefs: QrisPrefs,
    preview: CachedTexture,
    image: Option<RgbaImage>,
    staging: staging::Staging,
    decoding: Option<Job<Result<String, String>>>,
}

impl GeneratorUi for Qris {
    const TAB: Tab = Tab::Qris;

    fn quick_form(ui: &mut egui::Ui, merchant: &mut Merchant) -> bool {
        form::quick(ui, merchant)
    }
}

#[cfg(test)]
pub fn screenshots() {
    use crate::screenshots::shoot_tab;
    use crate::app::state::AppState;
    shoot_tab("qris-generate", Tab::Qris, AppState::default(), |state| {
        state.qris.prefs.merchant = Merchant { dynamic: true, amount: Some(15_000), ..Default::default() };
    });
    shoot_tab("qris-inspect", Tab::Qris, AppState::default(), |state| {
        state.qris.prefs.mode = Mode::Inspect;
        let mut payload = Merchant::default().payload();
        payload.replace_range(payload.len() - 4.., "0000");
        state.qris.prefs.inspect_input = payload;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::command::Commands;
    use crate::app::state::AppState;
    use crate::ui::dock::TabViewer;
    use egui_dock::TabViewer as _;

    #[test]
    fn registry_quick_form_edits_the_merchant_json() {
        let entry = crate::generators::find("qris").expect("qris is registered");
        let mut params = tt_core::generator::to_json(&Merchant::default());
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            (entry.quick_form)(ui, &mut params);
        });
        output.textures_delta.clear();
        assert_eq!(params["name"], "QA KUCING STORE");
        assert!((entry.core.generate)(&params, 0).is_ok());
    }

    #[test]
    fn inspect_mode_renders_a_broken_payload() {
        let mut state = AppState::default();
        state.qris.prefs.mode = Mode::Inspect;
        state.qris.prefs.inspect_input = "00020101021163040000".into();
        let ctx = egui::Context::default();
        crate::ui::theme::install_fonts(&ctx);
        crate::ui::theme::apply_style(&ctx);
        for _ in 0..2 {
            let mut cmds = Commands::default();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                let visuals = ui.visuals().clone();
                let mut viewer = TabViewer { state: &mut state, cmds: &mut cmds, focused: None, visuals };
                viewer.ui(ui, &mut Tab::Qris);
            });
            output.textures_delta.clear();
        }
    }
}
