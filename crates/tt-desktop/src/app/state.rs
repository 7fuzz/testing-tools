use crate::features::ktp::KtpState;
use crate::features::qris::QrisState;
use crate::features::settings::SettingsPrefs;

#[derive(Default)]
pub struct AppState {
    pub settings: SettingsPrefs,
    pub qris: QrisState,
    pub ktp: KtpState,
}
