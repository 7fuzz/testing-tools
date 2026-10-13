// Nothing registers yet: QRIS (phase 1) and KTP (phase 2) are the first entries, VCam (phase 4) the first reader.
#![allow(dead_code)]

use eframe::egui;
use serde_json::Value;
use tt_core::generator::{to_json, ErasedGenerator, Generator};
use crate::ui::dock::Tab;

/// The desktop half of a generator: the compact form other tools embed, such as VCam's quick create.
pub trait GeneratorUi: Generator {
    /// The generator's own tab, for "Open in …".
    const TAB: Tab;

    /// A few key fields and a randomize button. Returns true when a param changed.
    fn quick_form(ui: &mut egui::Ui, params: &mut Self::Params) -> bool;
}

#[derive(Clone, Copy)]
pub struct GeneratorEntry {
    pub core: ErasedGenerator,
    pub tab: Tab,
    pub quick_form: fn(&mut egui::Ui, &mut Value) -> bool,
}

impl GeneratorEntry {
    pub const fn of<G: GeneratorUi>() -> Self {
        Self { core: ErasedGenerator::of::<G>(), tab: G::TAB, quick_form: quick_form_json::<G> }
    }
}

fn quick_form_json<G: GeneratorUi>(ui: &mut egui::Ui, params: &mut Value) -> bool {
    let mut typed: G::Params = serde_json::from_value(params.clone()).unwrap_or_default();
    let changed = G::quick_form(ui, &mut typed);
    if changed {
        *params = to_json(&typed);
    }
    changed
}

pub const REGISTRY: &[GeneratorEntry] = &[];

pub fn find(id: &str) -> Option<&'static GeneratorEntry> {
    REGISTRY.iter().find(|entry| entry.core.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;
    use serde::{Deserialize, Serialize};
    use tt_core::generator::Rng;

    struct Blank;

    #[derive(Clone, Default, Serialize, Deserialize)]
    struct BlankParams {
        size: u32,
    }

    impl Generator for Blank {
        const ID: &'static str = "blank";
        const NAME: &'static str = "Blank";
        type Params = BlankParams;

        fn randomize(_: &mut Rng) -> BlankParams {
            BlankParams { size: 1 }
        }

        fn generate(params: &BlankParams, _: u64) -> anyhow::Result<RgbaImage> {
            Ok(RgbaImage::new(params.size, params.size))
        }
    }

    impl GeneratorUi for Blank {
        const TAB: Tab = Tab::Home;

        fn quick_form(_: &mut egui::Ui, params: &mut BlankParams) -> bool {
            params.size += 4;
            true
        }
    }

    #[test]
    fn quick_form_writes_typed_edits_back_to_json() {
        let entry = GeneratorEntry::of::<Blank>();
        let mut params = serde_json::json!({ "size": 0 });
        let ctx = egui::Context::default();
        let mut changed = false;
        let mut output = ctx.run_ui(Default::default(), |ui| changed = (entry.quick_form)(ui, &mut params));
        output.textures_delta.clear();
        assert!(changed);
        assert_eq!(params["size"], 4);
        assert_eq!((entry.core.generate)(&params, 0).unwrap().dimensions(), (4, 4));
    }
}
