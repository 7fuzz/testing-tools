mod form;
mod view;

use eframe::egui;
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use tt_core::generator::{random_seed, rng};
use tt_core::ktp::{Ktp, KtpParams, KtpSelfie, today};
use crate::generators::GeneratorUi;
use crate::ui::dock::Tab;
use crate::util::job::Job;
use crate::util::texture::CachedTexture;
pub use view::show;

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct KtpPrefs {
    pub params: KtpParams,
    /// Seeds the card background, signature, NIK serial and selfie pose.
    pub seed: u64,
    pub selfie: bool,
    pub batch_count: u32,
}

impl Default for KtpPrefs {
    fn default() -> Self {
        Self { params: KtpParams::random(&mut rng(random_seed()), today()), seed: random_seed(), selfie: true, batch_count: 10 }
    }
}

type Rendered = Result<(RgbaImage, Option<RgbaImage>), String>;

#[derive(Default)]
pub struct KtpState {
    pub prefs: KtpPrefs,
    card: CachedTexture,
    selfie: CachedTexture,
    images: Option<(RgbaImage, Option<RgbaImage>)>,
    error: Option<String>,
    /// The key of the render in flight, and of the last finished one.
    rendering: Option<(String, Job<Rendered>)>,
    shown_key: Option<String>,
    exporting: Option<Job<Result<String, String>>>,
}

impl GeneratorUi for Ktp {
    const TAB: Tab = Tab::Ktp;

    fn quick_form(ui: &mut egui::Ui, params: &mut KtpParams) -> bool {
        form::quick(ui, params)
    }
}

impl GeneratorUi for KtpSelfie {
    const TAB: Tab = Tab::Ktp;

    fn quick_form(ui: &mut egui::Ui, params: &mut KtpParams) -> bool {
        form::quick(ui, params)
    }
}

#[cfg(test)]
pub fn screenshots() {
    use crate::app::state::AppState;
    use crate::screenshots::shoot_tab;
    shoot_tab("ktp", Tab::Ktp, AppState::default(), |state| {
        state.ktp.prefs = KtpPrefs { params: KtpParams::random(&mut rng(100), today()), seed: 0, ..Default::default() };
        state.ktp.render_now();
    });
}
